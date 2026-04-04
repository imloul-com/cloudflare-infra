use serde_json::json;
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

    let route_list = routes::build_routes(&env);

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
            response.headers_mut().set("x-router-error", "missing_binding")?;
            response
                .headers_mut()
                .set("x-router-missing-binding", &missing_binding)?;
            Ok(response)
        }
        Some(m) => {
            let matched_prefix = m.route.prefix.clone();
            let upstream_origin = m.upstream.origin().unicode_serialization();
            let upstream_path = m.upstream.path().to_string();

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
