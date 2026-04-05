use serde::Deserialize;
use std::collections::HashSet;
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
    versions: VersionsConfig,
    route: RouteConfig,
    pages: PagesConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionsConfig {
    prod: String,
    dev: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteConfig {
    key: String,
    prefix: String,
    rewrite: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PagesConfig {
    prod: String,
    dev: String,
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
        ensure_non_empty(&app.id, "id")?;
        ensure_non_empty(&app.image, "image")?;
        ensure_non_empty(&app.versions.prod, "versions.prod")?;
        ensure_non_empty(&app.versions.dev, "versions.dev")?;
        ensure_non_empty(&app.route.key, "route.key")?;
        ensure_non_empty(&app.route.prefix, "route.prefix")?;
        ensure_non_empty(&app.route.rewrite, "route.rewrite")?;
        ensure_non_empty(&app.pages.prod, "pages.prod")?;
        ensure_non_empty(&app.pages.dev, "pages.dev")?;

        if !app_ids.insert(app.id.clone()) {
            return Err(format!("duplicate id '{}'", app.id).into());
        }
        if !route_keys.insert(app.route.key.clone()) {
            return Err(format!("duplicate route.key '{}'", app.route.key).into());
        }
        if !prefixes.insert(app.route.prefix.clone()) {
            return Err(format!("duplicate prefix '{}'", app.route.prefix).into());
        }
        if !project_names.insert(app.pages.prod.clone()) {
            return Err(format!("duplicate pages.prod '{}'", app.pages.prod).into());
        }
        if !project_names.insert(app.pages.dev.clone()) {
            return Err(format!("duplicate pages.dev '{}'", app.pages.dev).into());
        }

        if !app.route.prefix.starts_with('/') {
            return Err(format!("prefix must start with '/': {}", app.route.prefix).into());
        }
        if !app.image.starts_with("ghcr.io/") {
            return Err(format!("image must start with ghcr.io/: {}", app.image).into());
        }
        if !app.route.rewrite.starts_with('/') {
            return Err(format!("route.rewrite must start with '/': {}", app.route.rewrite).into());
        }
        if !app
            .route
            .key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("invalid route.key '{}'", app.route.key).into());
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
