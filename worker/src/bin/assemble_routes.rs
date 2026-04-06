use domain_router::catalog::{parse_assemble_args, RouteConfig};
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
    env: EnvConfig,
}

#[derive(Debug, Deserialize)]
struct EnvConfig {
    prod: EnvEntry,
    dev: EnvEntry,
}

#[derive(Debug, Deserialize)]
struct EnvEntry {
    route: RouteConfig,
    pages: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteDefinition {
    route_key: String,
    prefix: String,
    rewrite_to: String,
    project_name: String,
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
        let route = env_entry.route.normalize();

        route_defs.push(RouteDefinition {
            route_key: source.id,
            prefix: route.path_match,
            rewrite_to: route.rewrite,
            project_name: env_entry.pages.clone(),
        });
    }

    let json = serde_json::to_string_pretty(&route_defs)?;
    fs::write(&output_path, format!("{}\n", json))?;
    eprintln!("Wrote {}", output_path);

    Ok(())
}
