use serde::Deserialize;
use std::collections::HashSet;
use worker::Env;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteDefinition {
    route_key: String,
    prefix: String,
    rewrite_to: String,
    project_name: String,
}

#[derive(Clone)]
pub struct Route {
    pub prefix: String,
    pub origin: String,
    pub rewrite_to: String,
}

pub fn build_routes(env: &Env) -> Vec<Route> {
    let raw = env
        .var("ROUTE_DEFINITIONS")
        .expect("missing ROUTE_DEFINITIONS binding")
        .to_string();

    let mut definitions: Vec<RouteDefinition> =
        serde_json::from_str(&raw).expect("invalid ROUTE_DEFINITIONS JSON");

    let mut seen_keys = HashSet::new();
    for def in &definitions {
        if !seen_keys.insert(def.route_key.as_str()) {
            panic!("duplicate routeKey '{}'", def.route_key);
        }
        validate_route_definition(def).expect("invalid ROUTE_DEFINITIONS");
    }

    definitions.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));

    definitions
        .into_iter()
        .map(|def| Route {
            prefix: def.prefix,
            rewrite_to: def.rewrite_to,
            origin: env
                .var(&origin_var_name(&def.route_key))
                .map(|v| v.to_string())
                .unwrap_or_else(|_| format!("missing-binding:{}", origin_var_name(&def.route_key))),
        })
        .collect()
}

fn validate_route_definition(def: &RouteDefinition) -> Result<(), String> {
    if !def.prefix.starts_with('/') {
        return Err("prefix must start with '/'".to_string());
    }
    if !def.rewrite_to.starts_with('/') {
        return Err("rewritePrefixTo must start with '/'".to_string());
    }
    if def.route_key.trim().is_empty() {
        return Err("routeKey must be non-empty".to_string());
    }
    if !def
        .route_key
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err("routeKey must match [a-z0-9_]+".to_string());
    }
    if def.project_name.trim().is_empty() {
        return Err("projectName must be non-empty".to_string());
    }
    Ok(())
}

fn origin_var_name(route_key: &str) -> String {
    format!("{}_ORIGIN", route_key.to_ascii_uppercase())
}
