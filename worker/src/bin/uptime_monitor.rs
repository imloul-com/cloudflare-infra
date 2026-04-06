use domain_router::catalog::{parse_uptime_args, RouteConfig};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct AppCatalog {
    apps: Vec<AppSource>,
}

#[derive(Debug, Deserialize)]
struct AppSource {
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
}

fn main() -> Result<(), Box<dyn Error>> {
    let zone = required_env("CLOUDFLARE_ZONE_NAME")?;
    let (app_sources_path, environment) = parse_uptime_args(env::args().collect());
    let catalog: AppCatalog = serde_yaml::from_str(&fs::read_to_string(&app_sources_path)?)?;
    let app_sources = catalog.apps;

    if app_sources.is_empty() {
        return Err("apps.yaml must not be empty".into());
    }

    let base_origin = normalize_zone_to_origin(&zone)?;
    let endpoints = build_endpoints(&base_origin, &app_sources, &environment);

    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
    let mut failed = false;

    for url in endpoints {
        let status = client
            .get(&url)
            .send()
            .map(|resp| resp.status().as_u16())
            .unwrap_or(0);
        println!("{url} -> HTTP {status}");
        if !(200..400).contains(&status) {
            println!("::error::Health check failed for {url} (HTTP {status})");
            failed = true;
        }
    }

    if failed {
        return Err("one or more health checks failed".into());
    }

    println!("All health checks passed");
    Ok(())
}

fn build_endpoints(base_origin: &str, app_sources: &[AppSource], environment: &str) -> Vec<String> {
    let mut endpoints = BTreeSet::new();
    endpoints.insert(format!("{base_origin}/_health"));

    for source in app_sources {
        let env_entry = if environment == "dev" {
            &source.env.dev
        } else {
            &source.env.prod
        };
        let route = env_entry.route.normalize();
        let prefix = normalize_prefix(&route.path_match);
        endpoints.insert(format!("{base_origin}{prefix}"));
    }

    endpoints.into_iter().collect()
}

fn normalize_zone_to_origin(zone: &str) -> Result<String, Box<dyn Error>> {
    let trimmed = zone.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("CLOUDFLARE_ZONE_NAME must be non-empty".into());
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(trimmed.to_string());
    }

    Ok(format!("https://{trimmed}"))
}

fn normalize_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim();
    if trimmed == "/" {
        return "/".to_string();
    }

    let with_leading = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };

    if with_leading.ends_with('/') {
        with_leading
    } else {
        format!("{with_leading}/")
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name).map_err(|_| format!("missing required environment variable: {name}").into())
}
