use serde::Deserialize;
use std::cell::RefCell;
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
    pub route_key: String,
    pub prefix: String,
    pub origin: String,
    pub rewrite_to: String,
}

thread_local! {
    static CACHED_ROUTES: RefCell<Option<Vec<Route>>> = const { RefCell::new(None) };
}

/// Returns the parsed route list, caching the result for the lifetime of
/// the isolate. Env vars are constant per deployment so re-parsing on
/// every request is wasted work.
pub fn build_routes(env: &Env) -> Result<Vec<Route>, String> {
    CACHED_ROUTES.with(|cell| {
        if let Some(cached) = cell.borrow().as_ref() {
            return Ok(cached.clone());
        }
        let routes = parse_routes(env)?;
        *cell.borrow_mut() = Some(routes.clone());
        Ok(routes)
    })
}

fn parse_routes(env: &Env) -> Result<Vec<Route>, String> {
    let raw = env
        .var("ROUTE_DEFINITIONS")
        .map_err(|_| "missing ROUTE_DEFINITIONS binding".to_string())?
        .to_string();

    let mut definitions: Vec<RouteDefinition> =
        serde_json::from_str(&raw).map_err(|e| format!("invalid ROUTE_DEFINITIONS JSON: {e}"))?;

    let mut seen_keys = HashSet::new();
    let mut seen_binding_keys = HashSet::new();
    for def in &definitions {
        validate_route_definition(def)?;
        if !seen_keys.insert(def.route_key.as_str()) {
            return Err(format!("duplicate routeKey '{}'", def.route_key));
        }
        let binding_key = binding_route_key(&def.route_key);
        if !seen_binding_keys.insert(binding_key.clone()) {
            return Err(format!(
                "routeKey '{}' conflicts with another routeKey when normalized for origin binding ('{}')",
                def.route_key, binding_key
            ));
        }
    }

    definitions.sort_by(|a, b| b.prefix.len().cmp(&a.prefix.len()));

    Ok(definitions
        .into_iter()
        .map(|def| Route {
            route_key: def.route_key.clone(),
            prefix: def.prefix,
            rewrite_to: def.rewrite_to,
            origin: env
                .var(&origin_var_name(&def.route_key))
                .map(|v| v.to_string())
                .unwrap_or_else(|_| format!("missing-binding:{}", origin_var_name(&def.route_key))),
        })
        .collect())
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
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err("routeKey must match [a-z0-9_-]+".to_string());
    }
    if def.project_name.trim().is_empty() {
        return Err("projectName must be non-empty".to_string());
    }
    Ok(())
}

fn binding_route_key(route_key: &str) -> String {
    route_key.replace('-', "_")
}

fn origin_var_name(route_key: &str) -> String {
    format!("{}_ORIGIN", binding_route_key(route_key).to_ascii_uppercase())
}
