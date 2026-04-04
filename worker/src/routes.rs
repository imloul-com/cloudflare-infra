use serde::Deserialize;
use worker::Env;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RouteDefinition {
    prefix: String,
    strip_prefix: bool,
    origin_var: String,
}

#[derive(Clone)]
pub struct Route {
    pub prefix: String,
    pub origin: String,
    pub strip_prefix: bool,
}

pub fn build_routes(env: &Env) -> Vec<Route> {
    let definitions: Vec<RouteDefinition> =
        serde_json::from_str(include_str!("route-definitions.json"))
            .expect("Invalid route-definitions.json");

    definitions
        .into_iter()
        .map(|def| Route {
            prefix: def.prefix,
            strip_prefix: def.strip_prefix,
            origin: env
                .var(&def.origin_var)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| format!("missing-binding:{}", def.origin_var)),
        })
        .collect()
}
