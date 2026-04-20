use url::Url;

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
            let new_path = rewrite_prefix(pathname, &route.prefix, &route.rewrite_to);
            let upstream = build_upstream_url(&route.origin, &new_path)?;
            return Some(RouteMatch {
                route: route.clone(),
                upstream,
            });
        }
    }

    None
}

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

fn rewrite_prefix(pathname: &str, matched_prefix: &str, rewrite_to: &str) -> String {
    let suffix = &pathname[matched_prefix.len()..];
    if suffix.is_empty() {
        return rewrite_to.to_string();
    }

    if rewrite_to == "/" {
        return suffix.to_string();
    }

    if rewrite_to.ends_with('/') && suffix.starts_with('/') {
        format!("{}{}", rewrite_to.trim_end_matches('/'), suffix)
    } else if !rewrite_to.ends_with('/') && !suffix.starts_with('/') {
        format!("{}/{}", rewrite_to, suffix)
    } else {
        format!("{}{}", rewrite_to, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::Route;

    fn make_routes() -> Vec<Route> {
        vec![
            Route {
                route_key: "ast_viz".to_string(),
                prefix: "/tools/ast-viz".to_string(),
                origin: "https://worker-ast-viz.pages.dev".to_string(),
                rewrite_to: "/".to_string(),
                sitemap: None,
            },
            Route {
                route_key: "portfolio".to_string(),
                prefix: "/".to_string(),
                origin: "https://portfolio.pages.dev".to_string(),
                rewrite_to: "/".to_string(),
                sitemap: None,
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
                route_key: "ast_viz".to_string(),
                prefix: "/tools/ast-viz".to_string(),
                origin: "https://ast.pages.dev".to_string(),
                rewrite_to: "/".to_string(),
                sitemap: None,
            },
            Route {
                route_key: "tools".to_string(),
                prefix: "/tools".to_string(),
                origin: "https://tools.pages.dev".to_string(),
                rewrite_to: "/".to_string(),
                sitemap: None,
            },
            Route {
                route_key: "root".to_string(),
                prefix: "/".to_string(),
                origin: "https://root.pages.dev".to_string(),
                rewrite_to: "/".to_string(),
                sitemap: None,
            },
        ];
        assert_eq!(
            match_route("/tools/ast-viz/page", &routes)
                .unwrap()
                .route
                .origin,
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
        assert_eq!(
            rewrite_prefix("/tools/ast-viz/", "/tools/ast-viz", "/"),
            "/"
        );
    }

    #[test]
    fn test_rewrite_to_nested_path() {
        assert_eq!(
            rewrite_prefix("/legacy/blog/post", "/legacy/blog", "/blog"),
            "/blog/post"
        );
    }
}
