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
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RouteConfig {
    Prefix(String),
    Expanded {
        #[serde(rename = "match")]
        path_match: String,
        #[serde(default = "default_route_rewrite")]
        rewrite: String,
    },
}

#[derive(Debug)]
struct NormalizedRouteConfig {
    path_match: String,
    rewrite: String,
}

#[derive(Debug, Deserialize)]
struct EnvConfig {
    prod: EnvEntry,
    dev: EnvEntry,
}

#[derive(Debug, Deserialize)]
struct EnvEntry {
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
    let (app_sources_path, output_path, environment) = parse_args(env::args().collect());
    let catalog: AppCatalog = serde_yaml::from_str(&fs::read_to_string(&app_sources_path)?)?;
    let sources = catalog.apps;

    if sources.is_empty() {
        return Err("apps.yaml must not be empty".into());
    }

    let mut route_defs = Vec::with_capacity(sources.len());

    for source in sources {
        let project_name = project_name_for_environment(&source.env, &environment);
        let route = source.route.normalize();

        route_defs.push(RouteDefinition {
            route_key: source.id,
            prefix: route.path_match,
            rewrite_to: route.rewrite,
            project_name,
        });
    }

    let json = serde_json::to_string_pretty(&route_defs)?;
    fs::write(&output_path, format!("{}\n", json))?;
    eprintln!("Wrote {}", output_path);

    Ok(())
}

fn project_name_for_environment(env: &EnvConfig, environment: &str) -> String {
    if environment == "dev" {
        env.dev.pages.clone()
    } else {
        env.prod.pages.clone()
    }
}

fn parse_args(args: Vec<String>) -> (String, String, String) {
    let mut app_sources = String::from("src/apps.yaml");
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

fn default_route_rewrite() -> String {
    "/".to_string()
}

impl RouteConfig {
    fn normalize(self) -> NormalizedRouteConfig {
        match self {
            RouteConfig::Prefix(prefix) => NormalizedRouteConfig {
                path_match: prefix,
                rewrite: default_route_rewrite(),
            },
            RouteConfig::Expanded {
                path_match,
                rewrite,
            } => NormalizedRouteConfig {
                path_match,
                rewrite,
            },
        }
    }
}
