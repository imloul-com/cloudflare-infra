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

/// Parses CLI args for `bump_version`. Returns `(app_id, environment, version, apps_yaml_path)`.
/// `--app`, `--env`, and `--version` are required (validated by the caller); `--apps-yaml`
/// defaults to `worker/apps.yaml`.
pub fn parse_bump_args(args: Vec<String>) -> (String, String, String, String) {
    let mut app = String::new();
    let mut environment = String::new();
    let mut version = String::new();
    let mut path = String::from("worker/apps.yaml");
    let mut i = 1usize;

    while i < args.len() {
        match args[i].as_str() {
            "--app" if i + 1 < args.len() => {
                app = args[i + 1].clone();
                i += 2;
            }
            "--env" if i + 1 < args.len() => {
                environment = args[i + 1].clone();
                i += 2;
            }
            "--version" if i + 1 < args.len() => {
                version = args[i + 1].clone();
                i += 2;
            }
            "--apps-yaml" if i + 1 < args.len() => {
                path = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    (app, environment, version, path)
}

/// Parses CLI args for `diff_versions`. Returns `(old_path, new_path)`.
/// Both required.
pub fn parse_diff_args(args: Vec<String>) -> (String, String) {
    let mut old_path = String::new();
    let mut new_path = String::new();
    let mut i = 1usize;

    while i < args.len() {
        match args[i].as_str() {
            "--old" if i + 1 < args.len() => {
                old_path = args[i + 1].clone();
                i += 2;
            }
            "--new" if i + 1 < args.len() => {
                new_path = args[i + 1].clone();
                i += 2;
            }
            _ => i += 1,
        }
    }

    (old_path, new_path)
}
