use serde_json::json;
use url::Url;
use worker::*;

mod router;
mod routes;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let pathname = url.path().to_string();
    let method = req.method().to_string();
    let start = Date::now().as_millis();

    if pathname == "/_health" {
        return Response::ok("ok");
    }

    let route_list = match routes::build_routes(&env) {
        Ok(routes) => routes,
        Err(err) => {
            console_error!(
                "{}",
                json!({
                    "event": "route_definitions_error",
                    "error": err,
                    "pathname": &pathname,
                })
            );
            let mut response = Response::error("Router configuration error", 503)?;
            response
                .headers_mut()
                .set("x-router-error", "route_definitions_error")?;
            return Ok(response);
        }
    };
    let request_origin = normalize_origin(&url.origin().ascii_serialization());

    if pathname == "/sitemap.xml" {
        return sitemap_index_response(&request_origin, &route_list);
    }

    if let Some(route_key) = extract_sitemap_route_key(&pathname) {
        if let Some(route) = route_for_key(&route_list, &route_key) {
            let rewrite_prefix = if route.prefix == "/" {
                None
            } else {
                Some(route.prefix.as_str())
            };
            return proxy_sitemap(
                &route.origin,
                Some(&request_origin),
                rewrite_prefix,
            )
            .await;
        }
        return Response::error("Sitemap route is not configured", 404);
    }

    match router::match_route(&pathname, &route_list) {
        None => {
            console_error!(
                "{}",
                json!({ "event": "no_route_match", "pathname": &pathname })
            );
            let mut response = Response::error("No matching route", 502)?;
            response.headers_mut().set("x-router-error", "no_match")?;
            Ok(response)
        }
        Some(m) if m.route.origin.starts_with("missing-binding:") => {
            let missing_binding = m.route.origin["missing-binding:".len()..].to_string();
            console_error!(
                "{}",
                json!({
                    "event": "route_misconfigured",
                    "pathname": &pathname,
                    "matched_prefix": &m.route.prefix,
                    "missing_binding": &missing_binding,
                })
            );
            let mut response = Response::error("Route is misconfigured", 503)?;
            response
                .headers_mut()
                .set("x-router-error", "missing_binding")?;
            response
                .headers_mut()
                .set("x-router-missing-binding", &missing_binding)?;
            Ok(response)
        }
        Some(m) => {
            let matched_prefix = m.route.prefix.clone();
            let upstream_origin = m.upstream.origin().unicode_serialization();
            let upstream_path = m.upstream.path().to_string();
            let request_origin = normalize_origin(&url.origin().ascii_serialization());
            let upstream_origin_normalized =
                normalize_origin(&m.upstream.origin().ascii_serialization());

            // Safety guard: avoid recursive self-proxy loops if a route origin
            // accidentally points back to the same domain fronted by this Worker.
            if request_origin == upstream_origin_normalized {
                console_error!(
                    "{}",
                    json!({
                        "event": "proxy_loop_detected",
                        "pathname": &pathname,
                        "matched_prefix": &matched_prefix,
                        "upstream": &upstream_origin,
                    })
                );
                let mut response = Response::error("Route origin causes proxy loop", 503)?;
                response
                    .headers_mut()
                    .set("x-router-error", "proxy_loop_detected")?;
                return Ok(response);
            }

            match router::proxy_request(req, m).await {
                Ok(response) => {
                    let elapsed = Date::now().as_millis() - start;
                    console_log!(
                        "{}",
                        json!({
                            "event": "request",
                            "method": &method,
                            "pathname": &pathname,
                            "matched_prefix": &matched_prefix,
                            "upstream": &upstream_origin,
                            "upstream_path": &upstream_path,
                            "status": response.status_code(),
                            "duration_ms": elapsed,
                        })
                    );
                    Ok(response)
                }
                Err(err) => {
                    let elapsed = Date::now().as_millis() - start;
                    console_error!(
                        "{}",
                        json!({
                            "event": "proxy_error",
                            "pathname": &pathname,
                            "matched_prefix": &matched_prefix,
                            "upstream": &upstream_origin,
                            "error": err.to_string(),
                            "duration_ms": elapsed,
                        })
                    );
                    let mut response = Response::error("Upstream error", 502)?;
                    response
                        .headers_mut()
                        .set("x-router-error", "upstream_failure")?;
                    Ok(response)
                }
            }
        }
    }
}

fn route_for_key<'a>(routes: &'a [routes::Route], route_key: &str) -> Option<&'a routes::Route> {
    routes
        .iter()
        .find(|route| route.route_key == route_key && !route.origin.starts_with("missing-binding:"))
}

fn normalize_origin(origin: &str) -> String {
    origin.trim_end_matches('/').to_string()
}

fn sitemap_index_response(request_origin: &str, routes: &[routes::Route]) -> Result<Response> {
    let xml = build_sitemap_index_xml(request_origin, routes);
    let mut response = Response::from_html(xml)?;
    response
        .headers_mut()
        .set("content-type", "application/xml; charset=utf-8")?;
    response
        .headers_mut()
        .set("cache-control", "public, max-age=300")?;
    Ok(response)
}

async fn proxy_sitemap(
    upstream_origin: &str,
    domain_origin: Option<&str>,
    prefix: Option<&str>,
) -> Result<Response> {
    let upstream = format!("{}/sitemap.xml", normalize_origin(upstream_origin));
    let mut upstream_resp = Fetch::Url(upstream.parse()?).send().await?;
    let status = upstream_resp.status_code();

    let mut headers = Headers::new();
    for (key, val) in upstream_resp.headers() {
        headers.set(&key, &val)?;
    }
    headers.set("content-type", "application/xml; charset=utf-8")?;
    headers.set("cache-control", "public, max-age=300")?;

    let mut body = upstream_resp.text().await?;
    if let (Some(site_origin), Some(path_prefix)) = (domain_origin, prefix) {
        body = rewrite_sitemap_to_domain_path(&body, site_origin, path_prefix);
    }

    Response::from_bytes(body.into_bytes()).map(|r| r.with_headers(headers).with_status(status))
}

fn build_sitemap_index_xml(request_origin: &str, routes: &[routes::Route]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
</sitemapindex>"#,
    );

    let items = routes
        .iter()
        .filter(|route| !route.origin.starts_with("missing-binding:"))
        .map(|route| {
            format!(
                "  <sitemap>\n    <loc>{}/sitemaps/{}.xml</loc>\n  </sitemap>",
                request_origin, route.route_key
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    xml = xml.replace("</sitemapindex>", &format!("{}\n</sitemapindex>", items));
    xml
}

fn extract_sitemap_route_key(pathname: &str) -> Option<String> {
    let prefix = "/sitemaps/";
    if !pathname.starts_with(prefix) || !pathname.ends_with(".xml") {
        return None;
    }
    let key = &pathname[prefix.len()..pathname.len() - 4];
    if key.is_empty() {
        return None;
    }
    if key
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        Some(key.to_string())
    } else {
        None
    }
}

fn rewrite_sitemap_to_domain_path(xml: &str, domain_origin: &str, path_prefix: &str) -> String {
    let domain = normalize_origin(domain_origin);
    let prefix = if path_prefix.ends_with('/') {
        path_prefix.trim_end_matches('/').to_string()
    } else {
        path_prefix.to_string()
    };

    let mut result = String::with_capacity(xml.len() + 128);
    let mut cursor = 0;
    while let Some(start_rel) = xml[cursor..].find("<loc>") {
        let start = cursor + start_rel;
        let value_start = start + "<loc>".len();
        result.push_str(&xml[cursor..value_start]);

        if let Some(end_rel) = xml[value_start..].find("</loc>") {
            let value_end = value_start + end_rel;
            let loc = &xml[value_start..value_end];
            result.push_str(&rewrite_loc_url(loc, &domain, &prefix));
            cursor = value_end;
        } else {
            result.push_str(&xml[value_start..]);
            return result;
        }
    }
    result.push_str(&xml[cursor..]);
    result
}

fn rewrite_loc_url(loc: &str, domain_origin: &str, path_prefix: &str) -> String {
    let Ok(url) = Url::parse(loc) else {
        return loc.to_string();
    };

    let mut output = format!("{}{}", domain_origin, path_prefix);
    let suffix = url.path().trim_start_matches('/');
    if suffix.is_empty() {
        output.push('/');
    } else {
        output.push('/');
        output.push_str(suffix);
    }
    if let Some(query) = url.query() {
        output.push('?');
        output.push_str(query);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::Route;

    fn make_route(route_key: &str, prefix: &str, origin: &str) -> Route {
        Route {
            route_key: route_key.to_string(),
            prefix: prefix.to_string(),
            origin: origin.to_string(),
            rewrite_to: "/".to_string(),
        }
    }

    #[test]
    fn sitemap_index_points_to_dynamic_child_sitemaps() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev"),
            make_route("ast_viz", "/tools/ast-viz", "https://worker-ast-viz.pages.dev"),
        ];
        let xml = build_sitemap_index_xml("https://imloul.com", &routes);
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/portfolio.xml</loc>"));
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/ast_viz.xml</loc>"));
    }

    #[test]
    fn sitemap_index_skips_missing_bindings() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev"),
            make_route("broken", "/broken", "missing-binding:BROKEN_ORIGIN"),
        ];
        let xml = build_sitemap_index_xml("https://imloul.com", &routes);
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/portfolio.xml</loc>"));
        assert!(!xml.contains("/sitemaps/broken.xml"));
    }

    #[test]
    fn ast_sitemap_urls_are_rewritten_to_domain_prefix() {
        let input = r#"<?xml version="1.0"?>
<urlset>
  <url><loc>https://worker-ast-viz.pages.dev/</loc></url>
  <url><loc>https://worker-ast-viz.pages.dev/grammar</loc></url>
</urlset>"#;
        let rewritten = rewrite_sitemap_to_domain_path(
            input,
            "https://imloul.com",
            "/tools/ast-viz",
        );

        assert!(rewritten.contains("<loc>https://imloul.com/tools/ast-viz/</loc>"));
        assert!(rewritten.contains("<loc>https://imloul.com/tools/ast-viz/grammar</loc>"));
    }

    #[test]
    fn extract_route_key_from_sitemap_path() {
        assert_eq!(
            extract_sitemap_route_key("/sitemaps/ast_viz.xml"),
            Some("ast_viz".to_string())
        );
        assert_eq!(extract_sitemap_route_key("/sitemaps/.xml"), None);
        assert_eq!(
            extract_sitemap_route_key("/sitemaps/ast-viz.xml"),
            Some("ast-viz".to_string())
        );
        assert_eq!(extract_sitemap_route_key("/sitemap.xml"), None);
    }
}
