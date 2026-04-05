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
    route: RouteConfig,
    env: EnvConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteConfig {
    prefix: String,
    rewrite: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvConfig {
    prod: EnvEntry,
    dev: EnvEntry,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnvEntry {
    version: String,
    pages: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = "src/apps.yaml";
    let raw = fs::read_to_string(path)?;
    let catalog: AppCatalog = serde_yaml::from_str(&raw)?;

    if catalog.apps.is_empty() {
        return Err("app catalog must include at least one app".into());
    }

    let mut app_ids = HashSet::new();
    let mut prefixes = HashSet::new();
    let mut pages_names = HashSet::new();

    for app in &catalog.apps {
        ensure_non_empty(&app.id, "id")?;
        ensure_non_empty(&app.image, "image")?;
        ensure_non_empty(&app.route.prefix, "route.prefix")?;
        ensure_non_empty(&app.route.rewrite, "route.rewrite")?;
        ensure_non_empty(&app.env.prod.version, "env.prod.version")?;
        ensure_non_empty(&app.env.prod.pages, "env.prod.pages")?;
        ensure_non_empty(&app.env.dev.version, "env.dev.version")?;
        ensure_non_empty(&app.env.dev.pages, "env.dev.pages")?;

        if !app_ids.insert(app.id.clone()) {
            return Err(format!("duplicate id '{}'", app.id).into());
        }
        if !prefixes.insert(app.route.prefix.clone()) {
            return Err(format!("duplicate prefix '{}'", app.route.prefix).into());
        }
        if !pages_names.insert(app.env.prod.pages.clone()) {
            return Err(format!("duplicate env.prod.pages '{}'", app.env.prod.pages).into());
        }
        if !pages_names.insert(app.env.dev.pages.clone()) {
            return Err(format!("duplicate env.dev.pages '{}'", app.env.dev.pages).into());
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
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            return Err(format!("invalid id '{}'", app.id).into());
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
