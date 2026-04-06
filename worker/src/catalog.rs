//! Shared YAML route shape and CLI arg parsing for host binaries (`assemble_routes`, etc.).

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum RouteConfig {
    Prefix(String),
    Expanded(ExpandedRoute),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ExpandedRoute {
    #[serde(rename = "match")]
    pub path_match: String,
    #[serde(default = "default_route_rewrite")]
    pub rewrite: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedRoute {
    pub path_match: String,
    pub rewrite: String,
}

pub fn default_route_rewrite() -> String {
    "/".to_string()
}

impl RouteConfig {
    pub fn normalize(&self) -> NormalizedRoute {
        match self {
            RouteConfig::Prefix(prefix) => NormalizedRoute {
                path_match: prefix.clone(),
                rewrite: default_route_rewrite(),
            },
            RouteConfig::Expanded(route) => NormalizedRoute {
                path_match: route.path_match.clone(),
                rewrite: route.rewrite.clone(),
            },
        }
    }
}

/// App-level default route, optionally overridden per environment (`env.prod.route` / `env.dev.route`).
pub fn resolve_route(app_default: &RouteConfig, env_override: Option<&RouteConfig>) -> NormalizedRoute {
    match env_override {
        Some(r) => r.normalize(),
        None => app_default.normalize(),
    }
}

pub fn parse_app_sources_path(args: Vec<String>) -> String {
    let mut i = 1usize;
    let mut path = String::from("apps.yaml");

    while i < args.len() {
        if args[i] == "--app-sources-path" && i + 1 < args.len() {
            path = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }

    path
}

pub fn parse_assemble_args(args: Vec<String>) -> (String, String, String) {
    let mut app_sources = String::from("apps.yaml");
    let mut output = String::from("src/route-definitions.json");
    let mut environment = String::from("prod");
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "--app-sources-path" if i + 1 < args.len() => {
                app_sources = args[i + 1].clone();
                i += 2;
            }
            "--output-path" if i + 1 < args.len() => {
                output = args[i + 1].clone();
                i += 2;
            }
            "--environment" if i + 1 < args.len() => {
                environment = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    (app_sources, output, environment)
}

pub fn parse_uptime_args(args: Vec<String>) -> (String, String) {
    let mut path = String::from("apps.yaml");
    let mut environment = String::from("prod");
    let mut i = 1usize;

    while i < args.len() {
        match args[i].as_str() {
            "--app-sources-path" if i + 1 < args.len() => {
                path = args[i + 1].clone();
                i += 2;
            }
            "--environment" if i + 1 < args.len() => {
                environment = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    (path, environment)
}
