use domain_router::catalog::{parse_assemble_args, resolve_route, RouteConfig};
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;

#[derive(Debug, Deserialize)]
struct AppCatalog {
    apps: Vec<AppDefinition>,
}

#[derive(Debug, Deserialize)]
struct AppDefinition {
    id: String,
    route: RouteConfig,
    env: EnvConfig,
    #[serde(default)]
    sitemap: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EnvConfig {
    prod: EnvEntry,
    dev: EnvEntry,
}

#[derive(Debug, Deserialize)]
struct EnvEntry {
    #[serde(default)]
    route: Option<RouteConfig>,
    pages: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteDefinition {
    route_key: String,
    prefix: String,
    rewrite_to: String,
    project_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sitemap: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let (app_sources_path, output_path, environment) = parse_assemble_args(env::args().collect());
    let catalog: AppCatalog = serde_yaml::from_str(&fs::read_to_string(&app_sources_path)?)?;
    let sources = catalog.apps;

    if sources.is_empty() {
        return Err("apps.yaml must not be empty".into());
    }

    let mut route_defs = Vec::with_capacity(sources.len());

    for source in sources {
        let env_entry = if environment == "dev" {
            &source.env.dev
        } else {
            &source.env.prod
        };
        let route = resolve_route(&source.route, env_entry.route.as_ref());

        route_defs.push(RouteDefinition {
            route_key: source.id,
            prefix: route.path_match,
            rewrite_to: route.rewrite,
            project_name: env_entry.pages.clone(),
            sitemap: source.sitemap,
        });
    }

    let json = serde_json::to_string_pretty(&route_defs)?;
    fs::write(&output_path, format!("{}\n", json))?;
    eprintln!("Wrote {}", output_path);

    Ok(())
}
