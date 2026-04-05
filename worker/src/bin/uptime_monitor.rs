use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::time::Duration;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSource {
    prefix: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let zone = required_env("CLOUDFLARE_ZONE_NAME")?;
    let app_sources_path = parse_app_sources_path(env::args().collect());
    let app_sources: Vec<AppSource> = serde_json::from_str(&fs::read_to_string(&app_sources_path)?)?;

    if app_sources.is_empty() {
        return Err("app-sources.json must not be empty".into());
    }

    let base_origin = normalize_zone_to_origin(&zone)?;
    let endpoints = build_endpoints(&base_origin, &app_sources);

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

fn build_endpoints(base_origin: &str, app_sources: &[AppSource]) -> Vec<String> {
    let mut endpoints = BTreeSet::new();
    endpoints.insert(format!("{base_origin}/_health"));

    for source in app_sources {
        let prefix = normalize_prefix(&source.prefix);
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

fn parse_app_sources_path(args: Vec<String>) -> String {
    let mut i = 1usize;
    let mut path = String::from("worker/src/app-sources.json");

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

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name).map_err(|_| format!("missing required environment variable: {name}").into())
}
