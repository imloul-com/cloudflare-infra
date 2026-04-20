//! Compares two `apps.yaml` snapshots and emits the changed (app, environment)
//! pairs as JSON, suitable for feeding the `deploy-apps.yml` deployment matrix.
//!
//! Used by `deploy-apps.yml` on `push:main` events: the workflow saves the
//! pre-push `apps.yaml` (via `git show ${before}:worker/apps.yaml`) and the
//! current one, then runs `diff_versions --old <old> --new <new>`. Output is
//! a JSON array like `[{"appId":"ast-viz","environment":"dev"}]` consumed by
//! a downstream `jq` step.
//!
//! Only the `version:` field per (app, env) is compared. Adding or removing
//! an entire app counts as a change for every env of that app — that
//! correctly catches "I added a new app, please deploy it for the first
//! time" and "I removed an app, the deploy-apps job will fail with 'unknown
//! app' which is the right signal."

use domain_router::catalog::parse_diff_args;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("diff_versions: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (old_path, new_path) = parse_diff_args(env::args().collect());

    if old_path.is_empty() {
        return Err("missing --old <path>".into());
    }
    if new_path.is_empty() {
        return Err("missing --new <path>".into());
    }

    let old_text = fs::read_to_string(&old_path)
        .map_err(|e| format!("read {old_path}: {e}"))?;
    let new_text = fs::read_to_string(&new_path)
        .map_err(|e| format!("read {new_path}: {e}"))?;

    let pairs = diff_pairs(&old_text, &new_text)?;

    // Emit as a single JSON array on stdout. The deploy-apps workflow
    // captures this directly into a step output.
    println!("{}", serde_json::to_string(&pairs)?);
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CatalogShape {
    #[serde(default)]
    apps: Vec<AppShape>,
}

#[derive(Debug, Deserialize)]
struct AppShape {
    id: String,
    env: EnvShape,
}

#[derive(Debug, Deserialize)]
struct EnvShape {
    prod: EnvEntryShape,
    dev: EnvEntryShape,
}

#[derive(Debug, Deserialize)]
struct EnvEntryShape {
    version: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct ChangedPair {
    #[serde(rename = "appId")]
    app_id: String,
    environment: String,
}

type ParsedVersions = Vec<(String, BTreeMap<String, String>)>;

fn diff_pairs(old_text: &str, new_text: &str) -> Result<Vec<ChangedPair>, String> {
    let old = parse(old_text).map_err(|e| format!("parse old: {e}"))?;
    let new = parse(new_text).map_err(|e| format!("parse new: {e}"))?;

    let mut out = Vec::new();

    // Walk new in declared order so output is deterministic.
    for (app_id, new_versions) in &new {
        let old_versions = old
            .iter()
            .find(|(id, _)| id == app_id)
            .map(|(_, m)| m);
        for env_key in ["prod", "dev"] {
            let old_v = old_versions.and_then(|m| m.get(env_key));
            let new_v = new_versions.get(env_key);
            if old_v != new_v && new_v.is_some() {
                out.push(ChangedPair {
                    app_id: app_id.clone(),
                    environment: env_key.to_string(),
                });
            }
        }
    }

    Ok(out)
}

/// Returns an ordered map of `app_id -> { env_key -> version }`. Order
/// follows the YAML's apps array.
fn parse(text: &str) -> Result<ParsedVersions, String> {
    let parsed: CatalogShape = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(parsed.apps.len());
    for app in parsed.apps {
        let mut envs = BTreeMap::new();
        envs.insert("prod".to_string(), app.env.prod.version);
        envs.insert("dev".to_string(), app.env.dev.version);
        out.push((app.id, envs));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml_with(blog_prod: &str, blog_dev: &str, ast_prod: &str, ast_dev: &str) -> String {
        format!(
            "apps:
  - id: aimloul-blog
    image: ghcr.io/imloul-com/aimloul-blog-static
    route: /
    env:
      prod:
        version: {blog_prod}
        pages: portfolio
      dev:
        version: {blog_dev}
        pages: portfolio-dev
  - id: ast-viz
    image: ghcr.io/imloul-com/ast-viz-static
    route: /tools/ast-viz
    env:
      prod:
        version: {ast_prod}
        pages: ast-viz
      dev:
        version: {ast_dev}
        pages: ast-viz-dev
"
        )
    }

    #[test]
    fn no_changes() {
        let same = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let pairs = diff_pairs(&same, &same).unwrap();
        assert!(pairs.is_empty());
    }

    #[test]
    fn single_prod_bump() {
        let old = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let new = yaml_with("sha-1", "sha-2", "sha-99", "sha-4");
        let pairs = diff_pairs(&old, &new).unwrap();
        assert_eq!(
            pairs,
            vec![ChangedPair {
                app_id: "ast-viz".to_string(),
                environment: "prod".to_string(),
            }]
        );
    }

    #[test]
    fn single_dev_bump() {
        let old = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let new = yaml_with("sha-1", "sha-99", "sha-3", "sha-4");
        let pairs = diff_pairs(&old, &new).unwrap();
        assert_eq!(
            pairs,
            vec![ChangedPair {
                app_id: "aimloul-blog".to_string(),
                environment: "dev".to_string(),
            }]
        );
    }

    #[test]
    fn multiple_changes_same_app() {
        let old = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let new = yaml_with("sha-1", "sha-2", "sha-99", "sha-100");
        let pairs = diff_pairs(&old, &new).unwrap();
        assert_eq!(
            pairs,
            vec![
                ChangedPair {
                    app_id: "ast-viz".to_string(),
                    environment: "prod".to_string()
                },
                ChangedPair {
                    app_id: "ast-viz".to_string(),
                    environment: "dev".to_string()
                },
            ]
        );
    }

    #[test]
    fn cross_app_changes_preserve_yaml_order() {
        let old = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let new = yaml_with("sha-99", "sha-2", "sha-3", "sha-100");
        let pairs = diff_pairs(&old, &new).unwrap();
        // blog-prod first (declared first), then ast-dev.
        assert_eq!(
            pairs,
            vec![
                ChangedPair {
                    app_id: "aimloul-blog".to_string(),
                    environment: "prod".to_string()
                },
                ChangedPair {
                    app_id: "ast-viz".to_string(),
                    environment: "dev".to_string()
                },
            ]
        );
    }

    #[test]
    fn newly_added_app_counts_as_changed() {
        let old = "apps: []\n".to_string();
        let new = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let pairs = diff_pairs(&old, &new).unwrap();
        assert_eq!(pairs.len(), 4);
    }

    #[test]
    fn removed_app_does_not_appear_in_output() {
        let old = yaml_with("sha-1", "sha-2", "sha-3", "sha-4");
        let new = "apps: []\n".to_string();
        let pairs = diff_pairs(&old, &new).unwrap();
        assert!(pairs.is_empty());
    }
}
