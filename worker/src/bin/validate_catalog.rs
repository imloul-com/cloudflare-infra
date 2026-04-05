use serde::Deserialize;
use std::collections::HashSet;
use std::error::Error;
use std::fs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppCatalog {
    apps: Vec<AppDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppDefinition {
    app_id: String,
    registry: RegistryConfig,
    deploy: DeployConfig,
    route: RouteConfig,
    pages: PagesConfig,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteConfig {
    route_key: String,
    prefix: String,
    rewrite_to: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PagesConfig {
    project_name: String,
    dev_project_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RegistryConfig {
    image: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeployConfig {
    prod_version: String,
    dev_version: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = "src/app-sources.json";
    let raw = fs::read_to_string(path)?;
    let catalog: AppCatalog = serde_json::from_str(&raw)?;

    if catalog.apps.is_empty() {
        return Err("app catalog must include at least one app".into());
    }

    let mut app_ids = HashSet::new();
    let mut route_keys = HashSet::new();
    let mut prefixes = HashSet::new();
    let mut project_names = HashSet::new();

    for app in &catalog.apps {
        ensure_non_empty(&app.app_id, "appId")?;
        ensure_non_empty(&app.registry.image, "registry.image")?;
        ensure_non_empty(&app.deploy.prod_version, "deploy.prodVersion")?;
        ensure_non_empty(&app.deploy.dev_version, "deploy.devVersion")?;
        ensure_non_empty(&app.route.route_key, "route.routeKey")?;
        ensure_non_empty(&app.route.prefix, "route.prefix")?;
        ensure_non_empty(&app.route.rewrite_to, "route.rewriteTo")?;
        ensure_non_empty(&app.pages.project_name, "pages.projectName")?;
        ensure_non_empty(&app.pages.dev_project_name, "pages.devProjectName")?;

        if !app_ids.insert(app.app_id.clone()) {
            return Err(format!("duplicate appId '{}'", app.app_id).into());
        }
        if !route_keys.insert(app.route.route_key.clone()) {
            return Err(format!("duplicate routeKey '{}'", app.route.route_key).into());
        }
        if !prefixes.insert(app.route.prefix.clone()) {
            return Err(format!("duplicate prefix '{}'", app.route.prefix).into());
        }

        if !project_names.insert(app.pages.project_name.clone()) {
            return Err(format!("duplicate pages.projectName '{}'", app.pages.project_name).into());
        }
        if !project_names.insert(app.pages.dev_project_name.clone()) {
            return Err(
                format!(
                    "duplicate pages.devProjectName '{}'",
                    app.pages.dev_project_name
                )
                .into(),
            );
        }

        if !app.route.prefix.starts_with('/') {
            return Err(format!("prefix must start with '/': {}", app.route.prefix).into());
        }
        if !app.registry.image.starts_with("ghcr.io/") {
            return Err(format!("registry.image must start with ghcr.io/: {}", app.registry.image).into());
        }
        if !app.route.rewrite_to.starts_with('/') {
            return Err(format!("rewriteTo must start with '/': {}", app.route.rewrite_to).into());
        }
        if !app
            .route
            .route_key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("invalid routeKey '{}'", app.route.route_key).into());
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
