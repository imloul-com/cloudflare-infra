use domain_router::catalog::{parse_app_sources_path, RouteConfig};
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
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
    env: EnvConfig,
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
    route: RouteConfig,
    version: String,
    pages: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let path = parse_app_sources_path(env::args().collect());
    let raw = fs::read_to_string(path)?;
    let catalog: AppCatalog = serde_yaml::from_str(&raw)?;

    if catalog.apps.is_empty() {
        return Err("app catalog must include at least one app".into());
    }

    let mut app_ids = HashSet::new();
    let mut prod_prefixes = HashSet::new();
    let mut dev_prefixes = HashSet::new();
    let mut pages_names = HashSet::new();

    for app in &catalog.apps {
        let prod_route = app.env.prod.route.normalize();
        let dev_route = app.env.dev.route.normalize();

        ensure_non_empty(&app.id, "id")?;
        ensure_non_empty(&app.image, "image")?;
        ensure_non_empty(&prod_route.path_match, "env.prod.route.match")?;
        ensure_non_empty(&prod_route.rewrite, "env.prod.route.rewrite")?;
        ensure_non_empty(&dev_route.path_match, "env.dev.route.match")?;
        ensure_non_empty(&dev_route.rewrite, "env.dev.route.rewrite")?;
        ensure_non_empty(&app.env.prod.version, "env.prod.version")?;
        ensure_non_empty(&app.env.prod.pages, "env.prod.pages")?;
        ensure_non_empty(&app.env.dev.version, "env.dev.version")?;
        ensure_non_empty(&app.env.dev.pages, "env.dev.pages")?;

        if !app_ids.insert(app.id.clone()) {
            return Err(format!("duplicate id '{}'", app.id).into());
        }
        if !prod_prefixes.insert(prod_route.path_match.clone()) {
            return Err(format!(
                "duplicate env.prod.route.match '{}'",
                prod_route.path_match
            )
            .into());
        }
        if !dev_prefixes.insert(dev_route.path_match.clone()) {
            return Err(format!(
                "duplicate env.dev.route.match '{}'",
                dev_route.path_match
            )
            .into());
        }
        if !pages_names.insert(app.env.prod.pages.clone()) {
            return Err(format!("duplicate env.prod.pages '{}'", app.env.prod.pages).into());
        }
        if !pages_names.insert(app.env.dev.pages.clone()) {
            return Err(format!("duplicate env.dev.pages '{}'", app.env.dev.pages).into());
        }

        if !prod_route.path_match.starts_with('/') {
            return Err(format!(
                "env.prod.route.match must start with '/': {}",
                prod_route.path_match
            )
            .into());
        }
        if !prod_route.rewrite.starts_with('/') {
            return Err(format!(
                "env.prod.route.rewrite must start with '/': {}",
                prod_route.rewrite
            )
            .into());
        }
        if !dev_route.path_match.starts_with('/') {
            return Err(format!(
                "env.dev.route.match must start with '/': {}",
                dev_route.path_match
            )
            .into());
        }
        if !dev_route.rewrite.starts_with('/') {
            return Err(format!(
                "env.dev.route.rewrite must start with '/': {}",
                dev_route.rewrite
            )
            .into());
        }
        if !app.image.starts_with("ghcr.io/") {
            return Err(format!("image must start with ghcr.io/: {}", app.image).into());
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
