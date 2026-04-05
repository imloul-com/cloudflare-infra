use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSource {
    repo: String,
    route_key: String,
    prefix: String,
    rewrite_to: String,
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
    let (app_sources_path, output_path) = parse_args(env::args().collect());
    let github_token = env::var("GH_TOKEN")
        .or_else(|_| env::var("GITHUB_TOKEN"))
        .map_err(|_| "missing GH_TOKEN or GITHUB_TOKEN environment variable")?;

    let sources: Vec<AppSource> = serde_json::from_str(&fs::read_to_string(&app_sources_path)?)?;

    if sources.is_empty() {
        return Err("app-sources.json must not be empty".into());
    }

    let client = Client::builder().build()?;
    let mut route_defs = Vec::with_capacity(sources.len());

    for source in sources {
        let project_name = fetch_project_name(&client, &github_token, &source.repo)?;
        eprintln!("{} -> projectName={}", source.repo, project_name);

        route_defs.push(RouteDefinition {
            route_key: source.route_key,
            prefix: source.prefix,
            rewrite_to: source.rewrite_to,
            project_name,
        });
    }

    let json = serde_json::to_string_pretty(&route_defs)?;
    fs::write(&output_path, format!("{}\n", json))?;
    eprintln!("Wrote {}", output_path);

    Ok(())
}

fn fetch_project_name(client: &Client, token: &str, repo: &str) -> Result<String, Box<dyn Error>> {
    let url = format!(
        "https://api.github.com/repos/{}/contents/wrangler.toml?ref=main",
        repo
    );

    let toml_content = client
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github.raw")
        .header("User-Agent", "domain-router-ci")
        .send()?
        .error_for_status()
        .map_err(|e| format!("failed to fetch wrangler.toml from {}: {}", repo, e))?
        .text()?;

    extract_toml_name(&toml_content)
        .ok_or_else(|| format!("no 'name' field found in {}/wrangler.toml", repo).into())
}

fn extract_toml_name(toml: &str) -> Option<String> {
    for line in toml.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let val = rest.trim().trim_matches('"');
                if !val.is_empty() {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn parse_args(args: Vec<String>) -> (String, String) {
    let mut app_sources = String::from("src/app-sources.json");
    let mut output = String::from("src/route-definitions.json");
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
            _ => i += 1,
        }
    }

    (app_sources, output)
}
