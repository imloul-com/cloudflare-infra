//! Bumps `apps.yaml`'s `version:` field for a specific (app, environment) pair.
//!
//! Distributed as a prebuilt binary to app repos via GHCR so each app's CI can
//! atomically pin the new artifact SHA into `cloudflare-infra/main` after a
//! successful build, replacing the old `repository_dispatch` flow.
//!
//! Validates the (app, env) path with serde_yaml before AND after editing, but
//! the edit itself is line-based — comments, blank lines, indentation, and
//! field order in `apps.yaml` are preserved exactly. The first run still
//! produces a clean one-line diff.

use domain_router::catalog::parse_bump_args;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::process;

fn main() {
    if let Err(e) = run() {
        eprintln!("bump_version: {e}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (app, environment, version, path) = parse_bump_args(env::args().collect());

    if app.is_empty() {
        return Err("missing --app <id>".into());
    }
    if environment.is_empty() {
        return Err("missing --env <prod|dev>".into());
    }
    if environment != "prod" && environment != "dev" {
        return Err(format!("--env must be 'prod' or 'dev', got '{environment}'").into());
    }
    if version.is_empty() {
        return Err("missing --version <tag>".into());
    }

    let original = fs::read_to_string(&path)
        .map_err(|e| format!("read {path}: {e}"))?;

    match bump_in_text(&original, &app, &environment, &version)? {
        BumpOutcome::NoChange => {
            println!("no-op: {app} {environment} already pinned to {version}");
            Ok(())
        }
        BumpOutcome::Changed { new_text, old_version } => {
            fs::write(&path, &new_text)
                .map_err(|e| format!("write {path}: {e}"))?;
            println!("bumped: {app} {environment} {old_version} -> {version}");
            Ok(())
        }
    }
}

#[derive(Debug, PartialEq)]
enum BumpOutcome {
    NoChange,
    Changed { new_text: String, old_version: String },
}

#[derive(Debug, Deserialize)]
struct CatalogShape {
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

/// Pure function: validates the input, finds the target version line, and
/// returns the modified text. Returns `NoChange` if the value is already
/// what was requested (idempotent).
fn bump_in_text(
    text: &str,
    app_id: &str,
    env_key: &str,
    new_version: &str,
) -> Result<BumpOutcome, String> {
    // 1. Parse to validate structure and read the current version.
    let parsed: CatalogShape = serde_yaml::from_str(text)
        .map_err(|e| format!("parse apps.yaml: {e}"))?;

    let app = parsed
        .apps
        .iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| format!("app id '{app_id}' not found in apps.yaml"))?;

    let old_version = match env_key {
        "prod" => app.env.prod.version.clone(),
        "dev" => app.env.dev.version.clone(),
        _ => unreachable!("env_key validated by caller"),
    };

    if old_version == new_version {
        return Ok(BumpOutcome::NoChange);
    }

    // 2. Line-based edit. Walk lines tracking which app/env block we're in by
    //    indentation and content. Replace the first `version:` line inside
    //    the target env block.
    let mut output = String::with_capacity(text.len() + new_version.len());
    let mut found_app = false;
    let mut app_indent: Option<usize> = None;
    let mut in_target_env = false;
    let mut env_indent: Option<usize> = None;
    let mut replaced = false;

    let app_marker = format!("- id: {app_id}");
    let env_marker = format!("{env_key}:");

    for line in text.split_inclusive('\n') {
        if replaced {
            output.push_str(line);
            continue;
        }

        let stripped = line.trim_end_matches(['\n', '\r']);
        let trimmed = stripped.trim_start();
        let indent = stripped.len() - trimmed.len();

        // Empty / comment lines: pass through unchanged, don't update state.
        if trimmed.is_empty() || trimmed.starts_with('#') {
            output.push_str(line);
            continue;
        }

        if !found_app {
            if trimmed == app_marker {
                found_app = true;
                app_indent = Some(indent);
            }
            output.push_str(line);
            continue;
        }

        // We're inside the target app block. Detect when we've left it: a
        // sibling `- id:` at the same indent, or any content at a strictly
        // lower indent.
        let still_in_app = match app_indent {
            Some(ai) => indent > ai || (indent == ai && !trimmed.starts_with("- ")),
            None => true,
        };
        if !still_in_app {
            // Left the target app without finding the version line. The
            // serde_yaml parse above said it exists, so this is a bug.
            return Err(format!(
                "internal: walked off app '{app_id}' before finding env '{env_key}' version line"
            ));
        }

        if !in_target_env {
            if trimmed == env_marker {
                in_target_env = true;
                env_indent = Some(indent);
            }
            output.push_str(line);
            continue;
        }

        // Inside target env. Bail if we've left the env block.
        let still_in_env = match env_indent {
            Some(ei) => indent > ei,
            None => true,
        };
        if !still_in_env {
            return Err(format!(
                "internal: walked off env '{env_key}' before finding version line"
            ));
        }

        // Looking for `version:` directly under the env. We require the
        // `version:` line to be at the immediate-child indent of the env
        // block (env indent + 2 in normal YAML, but we just check it's
        // strictly greater than env_indent and matches `version:` content).
        if trimmed.starts_with("version:") {
            let leading_ws = &stripped[..stripped.len() - trimmed.len()];
            let trailing_newline = if line.ends_with("\r\n") {
                "\r\n"
            } else if line.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            output.push_str(leading_ws);
            output.push_str("version: ");
            output.push_str(new_version);
            output.push_str(trailing_newline);
            replaced = true;
            continue;
        }

        output.push_str(line);
    }

    if !replaced {
        return Err(format!(
            "internal: never found version line for app '{app_id}' env '{env_key}'"
        ));
    }

    // 3. Re-parse to verify the result is still valid AND the new value
    //    actually round-trips through the parser.
    let reparsed: CatalogShape = serde_yaml::from_str(&output)
        .map_err(|e| format!("post-edit parse failed (likely a line-edit bug): {e}"))?;

    let actual = reparsed
        .apps
        .iter()
        .find(|a| a.id == app_id)
        .map(|a| match env_key {
            "prod" => &a.env.prod.version,
            "dev" => &a.env.dev.version,
            _ => unreachable!(),
        })
        .ok_or("post-edit verification: app not found in result")?;

    if actual != new_version {
        return Err(format!(
            "post-edit verification: expected '{new_version}', got '{actual}'"
        ));
    }

    Ok(BumpOutcome::Changed {
        new_text: output,
        old_version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
apps:
  - id: aimloul-blog
    image: ghcr.io/imloul-com/aimloul-blog-static
    route: /
    env:
      prod:
        version: prod-latest
        pages: portfolio
      dev:
        version: dev-latest
        pages: portfolio-dev

  - id: ast-viz
    image: ghcr.io/imloul-com/ast-viz-static
    route: /tools/ast-viz
    env:
      prod:
        version: prod-latest
        pages: ast-viz
      dev:
        version: dev-latest
        pages: ast-viz-dev

  - id: bloom-filter
    image: ghcr.io/imloul-com/bloom-filter-static
    route: /tools/bloom-filter
    env:
      prod:
        version: prod-latest
        pages: bloom-filter
      dev:
        version: dev-latest
        pages: bloom-filter-dev
";

    fn assert_changed(result: BumpOutcome) -> (String, String) {
        match result {
            BumpOutcome::Changed { new_text, old_version } => (new_text, old_version),
            BumpOutcome::NoChange => panic!("expected Changed, got NoChange"),
        }
    }

    #[test]
    fn bumps_blog_prod_first_app() {
        let (new, old) = assert_changed(
            bump_in_text(SAMPLE, "aimloul-blog", "prod", "sha-abc123def456").unwrap(),
        );
        assert_eq!(old, "prod-latest");
        assert!(new.contains("        version: sha-abc123def456\n        pages: portfolio\n"));
        // Other version lines unchanged.
        assert!(new.contains("        version: dev-latest\n        pages: portfolio-dev\n"));
        assert!(new.contains("        version: prod-latest\n        pages: ast-viz\n"));
    }

    #[test]
    fn bumps_middle_app_dev() {
        let (new, _) = assert_changed(
            bump_in_text(SAMPLE, "ast-viz", "dev", "sha-aaaaaaaaaaaa").unwrap(),
        );
        assert!(new.contains("        version: sha-aaaaaaaaaaaa\n        pages: ast-viz-dev\n"));
        // Blog and bloom-filter version lines unchanged.
        assert!(new.contains("        version: prod-latest\n        pages: portfolio\n"));
        assert!(new.contains("        version: prod-latest\n        pages: bloom-filter\n"));
    }

    #[test]
    fn bumps_last_app_prod() {
        let (new, _) = assert_changed(
            bump_in_text(SAMPLE, "bloom-filter", "prod", "sha-bbbbbbbbbbbb").unwrap(),
        );
        assert!(new.contains("        version: sha-bbbbbbbbbbbb\n        pages: bloom-filter\n"));
        assert!(new.contains("        version: dev-latest\n        pages: bloom-filter-dev\n"));
    }

    #[test]
    fn idempotent_when_already_pinned() {
        let once = assert_changed(
            bump_in_text(SAMPLE, "ast-viz", "prod", "sha-cccccccccccc").unwrap(),
        )
        .0;
        let twice = bump_in_text(&once, "ast-viz", "prod", "sha-cccccccccccc").unwrap();
        assert_eq!(twice, BumpOutcome::NoChange);
    }

    #[test]
    fn preserves_blank_lines_between_apps() {
        let (new, _) = assert_changed(
            bump_in_text(SAMPLE, "aimloul-blog", "prod", "sha-dddddddddddd").unwrap(),
        );
        // Blank line between blog and ast-viz blocks must survive.
        assert!(new.contains("        pages: portfolio-dev\n\n  - id: ast-viz\n"));
    }

    #[test]
    fn preserves_comments() {
        let with_comment = SAMPLE.replace(
            "  - id: ast-viz",
            "  # the AST visualizer tool\n  - id: ast-viz",
        );
        let (new, _) = assert_changed(
            bump_in_text(&with_comment, "ast-viz", "prod", "sha-eeeeeeeeeeee").unwrap(),
        );
        assert!(new.contains("# the AST visualizer tool"));
    }

    #[test]
    fn preserves_diff_minimality() {
        let (new, _) = assert_changed(
            bump_in_text(SAMPLE, "ast-viz", "dev", "sha-ffffffffffff").unwrap(),
        );
        let mut differing_lines = 0;
        for (a, b) in SAMPLE.lines().zip(new.lines()) {
            if a != b {
                differing_lines += 1;
            }
        }
        assert_eq!(
            differing_lines, 1,
            "exactly one line should differ; got {differing_lines}"
        );
        assert_eq!(SAMPLE.lines().count(), new.lines().count());
    }

    #[test]
    fn errors_on_unknown_app() {
        let err = bump_in_text(SAMPLE, "nope", "prod", "sha-x").unwrap_err();
        assert!(err.contains("'nope'"));
    }

    #[test]
    fn errors_on_invalid_yaml() {
        let err = bump_in_text("not: [valid: yaml", "x", "prod", "sha-x").unwrap_err();
        assert!(err.contains("parse apps.yaml"));
    }
}
