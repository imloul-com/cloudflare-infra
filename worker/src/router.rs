use js_sys::Uint8Array;
use url::Url;
use worker::{Fetch, Headers, Request, RequestInit, Response, Result};

use crate::routes::Route;

pub struct RouteMatch {
    pub route: Route,
    pub upstream: Url,
}

pub fn match_route(pathname: &str, routes: &[Route]) -> Option<RouteMatch> {
    for route in routes {
        if route.prefix == "/" {
            let upstream = build_upstream_url(&route.origin, pathname)?;
            return Some(RouteMatch {
                route: route.clone(),
                upstream,
            });
        }

        let prefix_slash = route.prefix.clone() + "/";
        if pathname == route.prefix || pathname.starts_with(&prefix_slash) {
            let new_path = rewrite_prefix(pathname, &route.prefix, &route.rewrite_prefix_to);
            let upstream = build_upstream_url(&route.origin, &new_path)?;
            return Some(RouteMatch {
                route: route.clone(),
                upstream,
            });
        }
    }

    None
}

// Constructs the upstream URL by replacing only the path, preserving percent-encoding.
// Using string construction instead of url.set_path() to avoid double-encoding.
fn build_upstream_url(origin: &str, path: &str) -> Option<Url> {
    let origin_url = Url::parse(origin).ok()?;
    let mut s = format!("{}://{}", origin_url.scheme(), origin_url.host_str()?);
    if let Some(port) = origin_url.port() {
        s.push_str(&format!(":{}", port));
    }
    if !path.starts_with('/') {
        s.push('/');
    }
    s.push_str(path);
    Url::parse(&s).ok()
}

fn rewrite_prefix(pathname: &str, matched_prefix: &str, rewrite_prefix_to: &str) -> String {
    let suffix = &pathname[matched_prefix.len()..];
    if suffix.is_empty() {
        return rewrite_prefix_to.to_string();
    }

    if rewrite_prefix_to == "/" {
        return suffix.to_string();
    }

    if rewrite_prefix_to.ends_with('/') && suffix.starts_with('/') {
        format!("{}{}", rewrite_prefix_to.trim_end_matches('/'), suffix)
    } else if !rewrite_prefix_to.ends_with('/') && !suffix.starts_with('/') {
        format!("{}/{}", rewrite_prefix_to, suffix)
    } else {
        format!("{}{}", rewrite_prefix_to, suffix)
    }
}

pub async fn proxy_request(mut req: Request, m: RouteMatch) -> Result<Response> {
    let incoming_url = req.url()?;
    let mut upstream_url = m.upstream;
    upstream_url.set_query(incoming_url.query());

    let method = req.method();

    // Capture headers before consuming the body
    let mut new_headers = Headers::new();
    for (key, val) in req.headers() {
        // Prevent proxy loops: never forward the original Host header.
        // The upstream URL host should be authoritative.
        if key.eq_ignore_ascii_case("host") {
            continue;
        }
        new_headers.set(&key, &val)?;
    }

    let body_bytes = req.bytes().await.unwrap_or_default();

    let mut init = RequestInit::new();
    init.with_method(method).with_headers(new_headers);
    if !body_bytes.is_empty() {
        init.with_body(Some(Uint8Array::from(body_bytes.as_slice()).into()));
    }

    let upstream_req = Request::new_with_init(&upstream_url.to_string(), &init)?;
    let response = Fetch::Request(upstream_req).send().await?;

    if m.route.rewrite_prefix_to == "/" && m.route.prefix != "/" {
        let content_type = response
            .headers()
            .get("content-type")?
            .unwrap_or_default();
        if content_type.contains("text/html") {
            return inject_base_tag(response, &m.route.prefix).await;
        }
    }

    Ok(response)
}

async fn inject_base_tag(mut response: Response, prefix: &str) -> Result<Response> {
    let status = response.status_code();

    // Copy headers before consuming the body
    let mut headers = Headers::new();
    for (key, val) in response.headers() {
        headers.set(&key, &val)?;
    }

    let html = response.text().await?;

    let base_href = if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{}/", prefix)
    };
    let injected = inject_into_head(&html, &format!("<base href=\"{}\">", base_href));
    let body_bytes = injected.into_bytes();

    headers.set("content-length", &body_bytes.len().to_string())?;

    Response::from_bytes(body_bytes).map(|r| r.with_headers(headers).with_status(status))
}

fn inject_into_head(html: &str, tag: &str) -> String {
    let lower = html.to_lowercase();
    if let Some(pos) = lower.find("<head") {
        if let Some(rel_end) = lower[pos..].find('>') {
            let insert_pos = pos + rel_end + 1;
            return format!("{}{}{}", &html[..insert_pos], tag, &html[insert_pos..]);
        }
    }
    html.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::Route;

    fn make_routes() -> Vec<Route> {
        vec![
            Route {
                prefix: "/tools/ast-viz".to_string(),
                origin: "https://worker-ast-viz.pages.dev".to_string(),
                rewrite_prefix_to: "/".to_string(),
            },
            Route {
                prefix: "/".to_string(),
                origin: "https://portfolio.pages.dev".to_string(),
                rewrite_prefix_to: "/".to_string(),
            },
        ]
    }

    #[test]
    fn test_match_exact_prefix() {
        let m = match_route("/tools/ast-viz", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/tools/ast-viz");
        assert_eq!(m.upstream.path(), "/");
    }

    #[test]
    fn test_match_prefix_with_trailing_slash() {
        let m = match_route("/tools/ast-viz/", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/tools/ast-viz");
        assert_eq!(m.upstream.path(), "/");
    }

    #[test]
    fn test_match_nested_path() {
        let m = match_route("/tools/ast-viz/grammar/code", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/tools/ast-viz");
        assert_eq!(m.upstream.path(), "/grammar/code");
    }

    #[test]
    fn test_match_strips_assets_path() {
        let m = match_route("/tools/ast-viz/assets/index-abc123.js", &make_routes()).unwrap();
        assert_eq!(m.upstream.path(), "/assets/index-abc123.js");
    }

    #[test]
    fn test_does_not_match_partial_prefix() {
        let m = match_route("/tools/ast-vizzer", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/");
        assert_eq!(m.upstream.path(), "/tools/ast-vizzer");
    }

    #[test]
    fn test_root_fallthrough() {
        let m = match_route("/", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/");
        assert_eq!(m.upstream.path(), "/");
    }

    #[test]
    fn test_blog_path_fallthrough() {
        let m = match_route("/blog/my-post", &make_routes()).unwrap();
        assert_eq!(m.route.prefix, "/");
        assert_eq!(m.upstream.path(), "/blog/my-post");
    }

    #[test]
    fn test_no_routes() {
        assert!(match_route("/anything", &[]).is_none());
    }

    #[test]
    fn test_preserves_encoded_path() {
        let m = match_route("/tools/ast-viz/path%20with%20spaces", &make_routes()).unwrap();
        assert_eq!(m.upstream.path(), "/path%20with%20spaces");
    }

    #[test]
    fn test_more_specific_prefix_wins() {
        let routes = vec![
            Route {
                prefix: "/tools/ast-viz".to_string(),
                origin: "https://ast.pages.dev".to_string(),
                rewrite_prefix_to: "/".to_string(),
            },
            Route {
                prefix: "/tools".to_string(),
                origin: "https://tools.pages.dev".to_string(),
                rewrite_prefix_to: "/".to_string(),
            },
            Route {
                prefix: "/".to_string(),
                origin: "https://root.pages.dev".to_string(),
                rewrite_prefix_to: "/".to_string(),
            },
        ];
        assert_eq!(
            match_route("/tools/ast-viz/page", &routes).unwrap().route.origin,
            "https://ast.pages.dev"
        );
        assert_eq!(
            match_route("/tools/other", &routes).unwrap().route.origin,
            "https://tools.pages.dev"
        );
    }

    #[test]
    fn test_rewrite_prefix_nested_to_root() {
        assert_eq!(
            rewrite_prefix("/tools/ast-viz/page", "/tools/ast-viz", "/"),
            "/page"
        );
    }

    #[test]
    fn test_rewrite_prefix_exact_to_root() {
        assert_eq!(rewrite_prefix("/tools/ast-viz", "/tools/ast-viz", "/"), "/");
    }

    #[test]
    fn test_rewrite_prefix_trailing_slash_to_root() {
        assert_eq!(rewrite_prefix("/tools/ast-viz/", "/tools/ast-viz", "/"), "/");
    }

    #[test]
    fn test_rewrite_prefix_to_nested_path() {
        assert_eq!(
            rewrite_prefix("/legacy/blog/post", "/legacy/blog", "/blog"),
            "/blog/post"
        );
    }

    #[test]
    fn test_inject_into_head_simple() {
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let result = inject_into_head(html, "<base href=\"/tools/ast-viz/\">");
        assert!(result.contains("<head><base href=\"/tools/ast-viz/\"><title>T</title>"));
    }

    #[test]
    fn test_inject_into_head_with_attributes() {
        let html = "<html><head lang=\"en\"><title>T</title></head></html>";
        let result = inject_into_head(html, "<base href=\"/tools/\">");
        assert!(result.contains("<head lang=\"en\"><base href=\"/tools/\">"));
    }

    #[test]
    fn test_inject_into_head_uppercase() {
        let html = "<HTML><HEAD><TITLE>T</TITLE></HEAD></HTML>";
        let result = inject_into_head(html, "<base href=\"/tools/\">");
        assert!(result.contains("<HEAD><base href=\"/tools/\">"));
    }
}
