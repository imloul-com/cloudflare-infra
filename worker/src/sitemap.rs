use crate::constants::{
    CONTENT_TYPE_XML, DEFAULT_CACHE_CONTROL, SITEMAP_INDEX_PATH, SITEMAP_PREFIX,
};
use crate::routes::Route;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::io::Cursor;
use url::Url;
use worker::{Fetch, Headers, Response, Result};

fn log_sitemap_xml_issue(kind: &'static str, reason: &'static str, detail: impl std::fmt::Display) {
    #[cfg(target_arch = "wasm32")]
    {
        worker::console_error!(
            "{}",
            serde_json::json!({
                "event": "sitemap_xml_issue",
                "kind": kind,
                "reason": reason,
                "detail": detail.to_string(),
            })
        );
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (kind, reason, detail);
    }
}

pub fn normalize_origin(origin: &str) -> String {
    origin.trim_end_matches('/').to_string()
}

pub fn route_for_key<'a>(routes: &'a [Route], route_key: &str) -> Option<&'a Route> {
    routes.iter().find(|route| {
        route.route_key == route_key
            && route.sitemap.is_some()
            && !route.origin.starts_with("missing-binding:")
    })
}

pub fn index_response(request_origin: &str, routes: &[Route]) -> Result<Response> {
    let xml = build_sitemap_index_xml(request_origin, routes);
    let mut response = Response::from_html(xml)?;
    response.headers_mut().set("content-type", CONTENT_TYPE_XML)?;
    response
        .headers_mut()
        .set("cache-control", DEFAULT_CACHE_CONTROL)?;
    Ok(response)
}

pub async fn proxy_upstream_sitemap(
    upstream_origin: &str,
    upstream_path: &str,
    domain_origin: Option<&str>,
    prefix: Option<&str>,
) -> Result<Response> {
    let path = if upstream_path.starts_with('/') {
        upstream_path.to_string()
    } else {
        format!("/{}", upstream_path)
    };
    let upstream = format!("{}{}", normalize_origin(upstream_origin), path);
    let mut upstream_resp = Fetch::Url(upstream.parse()?).send().await?;
    let status = upstream_resp.status_code();

    let mut headers = Headers::new();
    for (key, val) in upstream_resp.headers() {
        headers.set(&key, &val)?;
    }
    headers.set("content-type", CONTENT_TYPE_XML)?;
    headers.set("cache-control", DEFAULT_CACHE_CONTROL)?;

    let mut body = upstream_resp.text().await?;
    if let (Some(site_origin), Some(path_prefix)) = (domain_origin, prefix) {
        body = rewrite_sitemap_to_domain_path(&body, site_origin, path_prefix);
    }

    Response::from_bytes(body.into_bytes()).map(|r| r.with_headers(headers).with_status(status))
}

/// Sitemap index built with `quick-xml` so `loc` values are escaped correctly.
pub fn build_sitemap_index_xml(request_origin: &str, routes: &[Route]) -> String {
    let mut buf = Vec::new();
    {
        let mut w = Writer::new(&mut buf);
        if w
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .is_err()
        {
            log_sitemap_xml_issue("index_build", "sitemap_index_write_decl", "write_event failed");
            return String::new();
        }
        let mut root = BytesStart::new("sitemapindex");
        root.push_attribute(("xmlns", "http://www.sitemaps.org/schemas/sitemap/0.9"));
        if w.write_event(Event::Start(root)).is_err() {
            log_sitemap_xml_issue("index_build", "sitemap_index_write_root", "write_event failed");
            return String::new();
        }

        for route in routes
            .iter()
            .filter(|r| r.sitemap.is_some() && !r.origin.starts_with("missing-binding:"))
        {
            if w.write_event(Event::Start(BytesStart::new("sitemap"))).is_err()
                || w.write_event(Event::Start(BytesStart::new("loc"))).is_err()
            {
                log_sitemap_xml_issue("index_build", "sitemap_index_write_entry", "write_event failed");
                return String::new();
            }
            let loc = format!("{}/sitemaps/{}.xml", request_origin, route.route_key);
            if w.write_event(Event::Text(BytesText::new(&loc))).is_err()
                || w.write_event(Event::End(BytesEnd::new("loc"))).is_err()
                || w.write_event(Event::End(BytesEnd::new("sitemap"))).is_err()
            {
                log_sitemap_xml_issue("index_build", "sitemap_index_write_loc", "write_event failed");
                return String::new();
            }
        }
        if w.write_event(Event::End(BytesEnd::new("sitemapindex"))).is_err() {
            log_sitemap_xml_issue("index_build", "sitemap_index_write_close", "write_event failed");
            return String::new();
        }
    }
    String::from_utf8(buf).unwrap_or_else(|e| {
        log_sitemap_xml_issue(
            "index_build",
            "sitemap_index_utf8",
            format!("invalid UTF-8 in generated index: {e}"),
        );
        String::new()
    })
}

pub fn extract_route_key(pathname: &str) -> Option<String> {
    if pathname == SITEMAP_INDEX_PATH {
        return None;
    }
    if !pathname.starts_with(SITEMAP_PREFIX) || !pathname.ends_with(".xml") {
        return None;
    }
    let key = &pathname[SITEMAP_PREFIX.len()..pathname.len() - 4];
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

fn is_loc_start(e: &BytesStart<'_>) -> bool {
    e.local_name().as_ref() == b"loc"
}

fn is_loc_end(e: &BytesEnd<'_>) -> bool {
    e.local_name().as_ref() == b"loc"
}

/// Rewrites `<loc>` text using a streaming XML parser/writer (namespace-aware local names,
/// trims ignorable whitespace between elements, supports split text/CDATA nodes).
/// On parse/write failure, logs to the Worker console and returns the original document unchanged.
pub fn rewrite_sitemap_to_domain_path(xml: &str, domain_origin: &str, path_prefix: &str) -> String {
    let domain = normalize_origin(domain_origin);
    let prefix = if path_prefix.ends_with('/') {
        path_prefix.trim_end_matches('/').to_string()
    } else {
        path_prefix.to_string()
    };

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new(Cursor::new(Vec::new()));
    let mut buf = Vec::new();
    let mut inside_loc = false;
    let mut loc_buf = String::new();

    macro_rules! write_ev {
        ($ev:expr) => {
            if let Err(e) = writer.write_event($ev) {
                log_sitemap_xml_issue("rewrite_fallback", "xml_write_error", e);
                return xml.to_string();
            }
        };
    }

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                if is_loc_start(&e) {
                    inside_loc = true;
                    loc_buf.clear();
                }
                write_ev!(Event::Start(e.into_owned()));
            }
            Ok(Event::End(e)) => {
                if inside_loc && is_loc_end(&e) {
                    let trimmed = loc_buf.trim();
                    let replacement = if trimmed.is_empty() {
                        String::new()
                    } else {
                        rewrite_loc_url(trimmed, &domain, &prefix)
                    };
                    write_ev!(Event::Text(BytesText::new(&replacement)));
                    inside_loc = false;
                    loc_buf.clear();
                }
                write_ev!(Event::End(e.into_owned()));
            }
            Ok(Event::Text(e)) => {
                if inside_loc {
                    match e.unescape() {
                        Ok(t) => loc_buf.push_str(&t),
                        Err(_) => loc_buf.push_str(&String::from_utf8_lossy(&*e)),
                    }
                } else {
                    write_ev!(Event::Text(e.into_owned()));
                }
            }
            Ok(Event::CData(e)) => {
                if inside_loc {
                    loc_buf.push_str(std::str::from_utf8(e.as_ref()).unwrap_or(""));
                } else {
                    write_ev!(Event::CData(e.into_owned()));
                }
            }
            Ok(e) => write_ev!(e.into_owned()),
            Err(e) => {
                log_sitemap_xml_issue("rewrite_fallback", "xml_read_error", e);
                return xml.to_string();
            }
        }
        buf.clear();
    }

    let bytes = writer.into_inner().into_inner();
    String::from_utf8(bytes).unwrap_or_else(|e| {
        log_sitemap_xml_issue(
            "rewrite_fallback",
            "xml_output_utf8_error",
            format!("rewritten sitemap is not valid UTF-8: {e}"),
        );
        xml.to_string()
    })
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

    fn make_route(route_key: &str, prefix: &str, origin: &str, sitemap: Option<&str>) -> Route {
        Route {
            route_key: route_key.to_string(),
            prefix: prefix.to_string(),
            origin: origin.to_string(),
            rewrite_to: "/".to_string(),
            sitemap: sitemap.map(|s| s.to_string()),
        }
    }

    #[test]
    fn sitemap_index_points_to_dynamic_child_sitemaps() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev", Some("/sitemap.xml")),
            make_route(
                "ast_viz",
                "/tools/ast-viz",
                "https://worker-ast-viz.pages.dev",
                Some("/sitemap.xml"),
            ),
        ];
        let xml = build_sitemap_index_xml("https://imloul.com", &routes);
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/portfolio.xml</loc>"));
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/ast_viz.xml</loc>"));
    }

    #[test]
    fn sitemap_index_skips_missing_bindings() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev", Some("/sitemap.xml")),
            make_route("broken", "/broken", "missing-binding:BROKEN_ORIGIN", Some("/sitemap.xml")),
        ];
        let xml = build_sitemap_index_xml("https://imloul.com", &routes);
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/portfolio.xml</loc>"));
        assert!(!xml.contains("/sitemaps/broken.xml"));
    }

    #[test]
    fn sitemap_index_skips_routes_without_sitemap() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev", Some("/sitemap.xml")),
            make_route("bloom", "/tools/bloom", "https://bloom.pages.dev", None),
        ];
        let xml = build_sitemap_index_xml("https://imloul.com", &routes);
        assert!(xml.contains("<loc>https://imloul.com/sitemaps/portfolio.xml</loc>"));
        assert!(!xml.contains("/sitemaps/bloom.xml"));
    }

    #[test]
    fn route_for_key_requires_configured_sitemap() {
        let routes = vec![
            make_route("portfolio", "/", "https://portfolio.pages.dev", Some("/sitemap.xml")),
            make_route("bloom", "/tools/bloom", "https://bloom.pages.dev", None),
        ];
        assert!(route_for_key(&routes, "portfolio").is_some());
        assert!(route_for_key(&routes, "bloom").is_none());
    }

    #[test]
    fn ast_sitemap_urls_are_rewritten_to_domain_prefix() {
        let input = r#"<?xml version="1.0"?>
<urlset>
  <url><loc>https://worker-ast-viz.pages.dev/</loc></url>
  <url><loc>https://worker-ast-viz.pages.dev/grammar</loc></url>
</urlset>"#;
        let rewritten =
            rewrite_sitemap_to_domain_path(input, "https://imloul.com", "/tools/ast-viz");

        assert!(rewritten.contains("<loc>https://imloul.com/tools/ast-viz/</loc>"));
        assert!(rewritten.contains("<loc>https://imloul.com/tools/ast-viz/grammar</loc>"));
    }

    #[test]
    fn rewrite_tolerates_whitespace_in_loc() {
        let input = r#"<?xml version="1.0"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url>
    <loc>
      https://worker-ast-viz.pages.dev/deep/path
    </loc>
  </url>
</urlset>"#;
        let out = rewrite_sitemap_to_domain_path(input, "https://imloul.com", "/tools/ast-viz");
        assert!(out.contains(
            "<loc>https://imloul.com/tools/ast-viz/deep/path</loc>"
        ));
    }

    #[test]
    fn rewrite_tolerates_cdata_in_loc() {
        let input = r#"<?xml version="1.0"?><urlset><url><loc><![CDATA[https://worker-ast-viz.pages.dev/hi]]></loc></url></urlset>"#;
        let out = rewrite_sitemap_to_domain_path(input, "https://imloul.com", "/p");
        assert!(out.contains("<loc>https://imloul.com/p/hi</loc>"));
    }

    #[test]
    fn extract_route_key_from_sitemap_path() {
        assert_eq!(
            extract_route_key("/sitemaps/ast_viz.xml"),
            Some("ast_viz".to_string())
        );
        assert_eq!(extract_route_key("/sitemaps/.xml"), None);
        assert_eq!(
            extract_route_key("/sitemaps/ast-viz.xml"),
            Some("ast-viz".to_string())
        );
        assert_eq!(extract_route_key("/sitemap.xml"), None);
    }
}
