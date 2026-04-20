use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fs;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteDefinition {
    route_key: String,
    prefix: String,
    rewrite_to: String,
    project_name: String,
    #[serde(default)]
    #[allow(dead_code)]
    sitemap: Option<String>,
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
    let mut seen_keys = HashSet::new();
    let mut seen_binding_keys = HashSet::new();

    for route in routes {
        if route.route_key.trim().is_empty() {
            return Err("each route must include a non-empty routeKey".into());
        }
        if !route
            .route_key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err("each route routeKey must match [a-z0-9_-]+".into());
        }
        if !seen_keys.insert(route.route_key.clone()) {
            return Err(format!("duplicate routeKey: {}", route.route_key).into());
        }
        if !route.prefix.starts_with('/') {
            return Err("each route must include a prefix starting with '/'".into());
        }
        if !route.rewrite_to.starts_with('/') {
            return Err("each route must include rewritePrefixTo starting with '/'".into());
        }
        if route.project_name.trim().is_empty() {
            return Err("each route must include a non-empty projectName".into());
        }

        let origin_url = resolve_origin(&client, &api_token, &account_id, &route.project_name)?;
        let binding_key = binding_route_key(&route.route_key);
        if !seen_binding_keys.insert(binding_key.clone()) {
            return Err(format!(
                "routeKey '{}' conflicts with another routeKey when normalized for origin binding ('{}')",
                route.route_key, binding_key
            )
            .into());
        }
        deploy_var_args.push_str(&format!(
            " --var {}:{}",
            origin_var_name(&route.route_key),
            origin_url
        ));
    }

    println!("{}", deploy_var_args);
    Ok(())
}

fn origin_var_name(route_key: &str) -> String {
    format!("{}_ORIGIN", binding_route_key(route_key).to_ascii_uppercase())
}

fn binding_route_key(route_key: &str) -> String {
    route_key.replace('-', "_")
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
        .ok_or_else(|| {
            format!(
                "failed to resolve subdomain for Pages project: {}",
                project_name
            )
        })?;

    Ok(format!("https://{}", subdomain))
}
