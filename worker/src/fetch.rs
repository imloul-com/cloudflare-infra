use crate::constants::{HEALTH_PATH, MISSING_BINDING_PREFIX, SITEMAP_INDEX_PATH};
use crate::errors::router_error;
use crate::router;
use crate::routes;
use crate::sitemap;
use serde_json::json;
use worker::*;

pub async fn handle(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let url = req.url()?;
    let pathname = url.path().to_string();
    let method = req.method().to_string();
    let start = Date::now().as_millis();

    if pathname == HEALTH_PATH {
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
            return router_error("Router configuration error", 503, "route_definitions_error");
        }
    };

    let request_origin = sitemap::normalize_origin(&url.origin().ascii_serialization());

    if pathname == SITEMAP_INDEX_PATH {
        return sitemap::index_response(&request_origin, &route_list);
    }

    if let Some(route_key) = sitemap::extract_route_key(&pathname) {
        if let Some(route) = sitemap::route_for_key(&route_list, &route_key) {
            let rewrite_prefix = if route.prefix == "/" {
                None
            } else {
                Some(route.prefix.as_str())
            };
            return sitemap::proxy_upstream_sitemap(
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
            router_error("No matching route", 502, "no_match")
        }
        Some(m) if m.route.origin.starts_with(MISSING_BINDING_PREFIX) => {
            let missing_binding = m.route.origin[MISSING_BINDING_PREFIX.len()..].to_string();
            console_error!(
                "{}",
                json!({
                    "event": "route_misconfigured",
                    "pathname": &pathname,
                    "matched_prefix": &m.route.prefix,
                    "missing_binding": &missing_binding,
                })
            );
            let mut response = router_error("Route is misconfigured", 503, "missing_binding")?;
            response
                .headers_mut()
                .set("x-router-missing-binding", &missing_binding)?;
            Ok(response)
        }
        Some(m) => {
            let matched_prefix = m.route.prefix.clone();
            let upstream_origin = m.upstream.origin().unicode_serialization();
            let upstream_path = m.upstream.path().to_string();
            let request_origin_norm = sitemap::normalize_origin(&url.origin().ascii_serialization());
            let upstream_origin_normalized =
                sitemap::normalize_origin(&m.upstream.origin().ascii_serialization());

            if request_origin_norm == upstream_origin_normalized {
                console_error!(
                    "{}",
                    json!({
                        "event": "proxy_loop_detected",
                        "pathname": &pathname,
                        "matched_prefix": &matched_prefix,
                        "upstream": &upstream_origin,
                    })
                );
                return router_error("Route origin causes proxy loop", 503, "proxy_loop_detected");
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
                    router_error("Upstream error", 502, "upstream_failure")
                }
            }
        }
    }
}
