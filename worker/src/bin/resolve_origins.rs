use reqwest::blocking::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteDefinition {
    prefix: String,
    rewrite_prefix_to: String,
    project_name: String,
    origin_var: String,
}

#[derive(Debug, Deserialize)]
struct CloudflareResponse {
    result: Option<CloudflareProject>,
}

#[derive(Debug, Deserialize)]
struct CloudflareProject {
    subdomain: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let route_definitions_path = parse_route_definitions_path(env::args().collect());
    let api_token = required_env("CLOUDFLARE_API_TOKEN")?;
    let account_id = required_env("CLOUDFLARE_ACCOUNT_ID")?;

    let content = fs::read_to_string(&route_definitions_path)?;
    let routes: Vec<RouteDefinition> = serde_json::from_str(&content)?;
    if routes.is_empty() {
        return Err("route definitions must not be empty".into());
    }

    let client = Client::builder().build()?;
    let mut deploy_var_args = String::new();

    for route in routes {
        if !route.prefix.starts_with('/') {
            return Err("each route must include a prefix starting with '/'".into());
        }
        if !route.rewrite_prefix_to.starts_with('/') {
            return Err("each route must include rewritePrefixTo starting with '/'".into());
        }
        if route.project_name.trim().is_empty() {
            return Err("each route must include a non-empty projectName".into());
        }
        if route.origin_var.trim().is_empty() {
            return Err("each route must include a non-empty originVar".into());
        }

        let origin_url = resolve_origin(&client, &api_token, &account_id, &route.project_name)?;
        deploy_var_args.push_str(&format!(" --var {}:{}", route.origin_var, origin_url));
    }

    println!("{}", deploy_var_args);
    Ok(())
}

fn parse_route_definitions_path(args: Vec<String>) -> String {
    let mut i = 1usize;
    let mut path = String::from("src/route-definitions.json");

    while i < args.len() {
        if args[i] == "--route-definitions-path" && i + 1 < args.len() {
            path = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }

    path
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    env::var(name).map_err(|_| format!("missing required environment variable: {}", name).into())
}

fn resolve_origin(
    client: &Client,
    api_token: &str,
    account_id: &str,
    project_name: &str,
) -> Result<String, Box<dyn Error>> {
    let mut url = Url::parse(&format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/pages/projects",
        account_id
    ))?;
    {
        let mut segments = url.path_segments_mut().map_err(|_| "invalid URL path")?;
        segments.push(project_name);
    }

    let resp = client
        .get(url)
        .bearer_auth(api_token)
        .send()?
        .error_for_status()?
        .json::<CloudflareResponse>()?;

    let subdomain = resp
        .result
        .and_then(|r| r.subdomain)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("failed to resolve subdomain for Pages project: {}", project_name))?;

    Ok(format!("https://{}", subdomain))
}
