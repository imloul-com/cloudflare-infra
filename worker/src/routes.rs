use serde::Deserialize;
use worker::Env;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteDefinition {
    prefix: String,
    rewrite_prefix_to: String,
    project_name: String,
    origin_var: String,
}

#[derive(Clone)]
pub struct Route {
    pub prefix: String,
    pub origin: String,
    pub rewrite_prefix_to: String,
}

pub fn build_routes(env: &Env) -> Vec<Route> {
    let definitions: Vec<RouteDefinition> =
        serde_json::from_str(include_str!("route-definitions.json"))
            .expect("Invalid route-definitions.json");

    for def in &definitions {
        validate_route_definition(def).expect("Invalid route-definitions.json");
    }

    definitions
        .into_iter()
        .map(|def| Route {
            prefix: def.prefix,
            rewrite_prefix_to: def.rewrite_prefix_to,
            origin: env
                .var(&def.origin_var)
                .map(|v| v.to_string())
                .unwrap_or_else(|_| format!("missing-binding:{}", def.origin_var)),
        })
        .collect()
}

fn validate_route_definition(def: &RouteDefinition) -> Result<(), String> {
    if !def.prefix.starts_with('/') {
        return Err("prefix must start with '/'".to_string());
    }
    if !def.rewrite_prefix_to.starts_with('/') {
        return Err("rewritePrefixTo must start with '/'".to_string());
    }
    if def.origin_var.trim().is_empty() {
        return Err("originVar must be non-empty".to_string());
    }
    if def.project_name.trim().is_empty() {
        return Err("projectName must be non-empty".to_string());
    }
    Ok(())
}
