use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppCatalog {
    apps: Vec<AppDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppDefinition {
    id: String,
    image: String,
    route: RouteConfig,
    env: EnvConfig,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RouteConfig {
    Prefix(String),
    Expanded(ExpandedRouteConfig),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpandedRouteConfig {
    #[serde(rename = "match")]
    path_match: String,
    #[serde(default = "default_route_rewrite")]
    rewrite: String,
}

#[derive(Debug)]
struct NormalizedRouteConfig {
    path_match: String,
    rewrite: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvConfig {
    prod: EnvEntry,
    dev: EnvEntry,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvEntry {
    version: String,
    pages: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = parse_app_sources_path(env::args().collect());
    let raw = fs::read_to_string(path)?;
    let catalog: AppCatalog = serde_yaml::from_str(&raw)?;

    if catalog.apps.is_empty() {
        return Err("app catalog must include at least one app".into());
    }

    let mut app_ids = HashSet::new();
    let mut prefixes = HashSet::new();
    let mut pages_names = HashSet::new();

    for app in &catalog.apps {
        let route = app.route.normalized();
        ensure_non_empty(&app.id, "id")?;
        ensure_non_empty(&app.image, "image")?;
        ensure_non_empty(&route.path_match, "route.match")?;
        ensure_non_empty(&route.rewrite, "route.rewrite")?;
        ensure_non_empty(&app.env.prod.version, "env.prod.version")?;
        ensure_non_empty(&app.env.prod.pages, "env.prod.pages")?;
        ensure_non_empty(&app.env.dev.version, "env.dev.version")?;
        ensure_non_empty(&app.env.dev.pages, "env.dev.pages")?;

        if !app_ids.insert(app.id.clone()) {
            return Err(format!("duplicate id '{}'", app.id).into());
        }
        if !prefixes.insert(route.path_match.clone()) {
            return Err(format!("duplicate route.match '{}'", route.path_match).into());
        }
        if !pages_names.insert(app.env.prod.pages.clone()) {
            return Err(format!("duplicate env.prod.pages '{}'", app.env.prod.pages).into());
        }
        if !pages_names.insert(app.env.dev.pages.clone()) {
            return Err(format!("duplicate env.dev.pages '{}'", app.env.dev.pages).into());
        }

        if !route.path_match.starts_with('/') {
            return Err(format!(
                "route.match must start with '/': {}",
                route.path_match
            )
            .into());
        }
        if !app.image.starts_with("ghcr.io/") {
            return Err(format!("image must start with ghcr.io/: {}", app.image).into());
        }
        if !route.rewrite.starts_with('/') {
            return Err(format!("route.rewrite must start with '/': {}", route.rewrite).into());
        }
        if !app
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("invalid id '{}'", app.id).into());
        }
    }

    println!("Catalog validation passed for {} app(s)", catalog.apps.len());
    Ok(())
}

fn ensure_non_empty(value: &str, field_name: &str) -> Result<(), Box<dyn Error>> {
    if value.trim().is_empty() {
        return Err(format!("{field_name} must be non-empty").into());
    }
    Ok(())
}

fn parse_app_sources_path(args: Vec<String>) -> String {
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

fn default_route_rewrite() -> String {
    "/".to_string()
}

impl RouteConfig {
    fn normalized(&self) -> NormalizedRouteConfig {
        match self {
            RouteConfig::Prefix(prefix) => NormalizedRouteConfig {
                path_match: prefix.clone(),
                rewrite: default_route_rewrite(),
            },
            RouteConfig::Expanded(route) => NormalizedRouteConfig {
                path_match: route.path_match.clone(),
                rewrite: route.rewrite.clone(),
            },
        }
    }
}
