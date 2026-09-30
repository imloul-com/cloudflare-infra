use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use worker::crypto::{DigestStream, DigestStreamAlgorithm};
use worker::worker_sys::web_sys;
use worker::*;

use crate::routes::Route;

pub const COUNTERS_BINDING: &str = "COUNTERS";
pub const SALT_BINDING: &str = "VIEW_SALT";

/// Single instance so every increment serializes through one object and counts stay exact.
pub const COUNTERS_INSTANCE: &str = "counters";

const MS_PER_DAY: f64 = 86_400_000.0;

const CRAWLER_MARKERS: [&str; 12] = [
    "bot",
    "crawler",
    "spider",
    "slurp",
    "curl",
    "wget",
    "python-requests",
    "headlesschrome",
    "facebookexternalhit",
    "embedly",
    "preview",
    "monitor",
];

#[derive(Serialize, Deserialize)]
pub struct Hit {
    pub path: String,
    pub token: String,
    pub day: u64,
    /// Extra paths to read without incrementing, so an index page can show the
    /// counts of the pages it links to in the same round trip.
    #[serde(default)]
    pub lookup: Vec<String>,
}

#[derive(Default, Deserialize, Serialize)]
pub struct Counts {
    pub current: u64,
    #[serde(default)]
    pub lookup: HashMap<String, u64>,
}

pub struct HitInput {
    pub path: String,
    ip: String,
    user_agent: String,
}

pub fn day_bucket(now_ms: f64) -> u64 {
    (now_ms / MS_PER_DAY) as u64
}

pub fn normalize_path(pathname: &str) -> String {
    let trimmed = pathname.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_lowercase()
    }
}

/// Extracts everything needed to count this request, or `None` if it is not a
/// countable human pageview. Returns owned data so the caller can hand the
/// request itself to the proxy and run both concurrently.
pub fn prepare_hit(req: &Request, pathname: &str) -> Option<HitInput> {
    if req.method() != Method::Get {
        return None;
    }

    let headers = req.headers();
    let header = |name: &str| headers.get(name).ok().flatten().unwrap_or_default();

    // Real navigations only. Sec-Fetch-Dest is absent on older browsers, so fall
    // back to content negotiation rather than dropping those visitors entirely.
    let dest = header("sec-fetch-dest");
    if dest.is_empty() {
        if !header("accept").contains("text/html") {
            return None;
        }
    } else if dest != "document" {
        return None;
    }

    if header("sec-purpose").contains("prefetch") {
        return None;
    }

    let user_agent = header("user-agent");
    let lowered = user_agent.to_lowercase();
    if lowered.is_empty() || CRAWLER_MARKERS.iter().any(|m| lowered.contains(m)) {
        return None;
    }

    Some(HitInput {
        path: normalize_path(pathname),
        ip: header("cf-connecting-ip"),
        user_agent,
    })
}

/// Counts for the routes nested directly beneath the requested path, so an
/// index page can render a count for each page it links to.
///
/// Derived from the route table rather than any hardcoded path, so upstream
/// apps can rename or restructure their URLs without a change here.
pub fn lookup_paths(pathname: &str, routes: &[Route]) -> Vec<String> {
    let current = normalize_path(pathname);
    let child_prefix = if current == "/" {
        "/".to_string()
    } else {
        format!("{current}/")
    };

    routes
        .iter()
        .map(|r| normalize_path(&r.prefix))
        .filter(|p| *p != current && p.starts_with(&child_prefix))
        .collect()
}

/// Cookieless per-visitor-per-path token. The day bucket is part of the digest,
/// so the value rotates every 24h and is not a stable identifier.
async fn token(input: &HitInput, salt: &str, day: u64) -> Result<String> {
    let material = format!(
        "{salt}\u{1f}{day}\u{1f}{}\u{1f}{}\u{1f}{}",
        input.ip, input.user_agent, input.path
    );

    let source = web_sys::Response::new_with_opt_str(Some(&material))
        .map_err(|_| Error::RustError("failed to build digest source".into()))?;
    let body = source
        .body()
        .ok_or_else(|| Error::RustError("digest source has no body".into()))?;

    let stream = DigestStream::new(DigestStreamAlgorithm::Sha256);
    let _ = body.pipe_to(stream.raw());
    let digest = stream.digest().await?.to_vec();

    Ok(digest
        .iter()
        .take(10)
        .fold(String::with_capacity(20), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        }))
}

/// Records the hit and returns the path's view count including this one, plus
/// the counts of any `lookup` paths.
pub async fn record(env: &Env, input: HitInput, lookup: Vec<String>) -> Result<Counts> {
    let salt = env
        .secret(SALT_BINDING)
        .map(|s| s.to_string())
        .or_else(|_| env.var(SALT_BINDING).map(|v| v.to_string()))?;

    let day = day_bucket(Date::now().as_millis() as f64);
    let hit = Hit {
        token: token(&input, &salt, day).await?,
        path: input.path,
        day,
        lookup,
    };

    let stub = env
        .durable_object(COUNTERS_BINDING)?
        .id_from_name(COUNTERS_INSTANCE)?
        .get_stub()?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_body(Some(serde_json::to_string(&hit)?.into()));
    let req = Request::new_with_init("https://counters.internal/hit", &init)?;

    let mut response = stub.fetch_with_request(req).await?;
    response.json().await
}

pub fn format_count(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{day_bucket, format_count, lookup_paths, normalize_path};
    use crate::routes::Route;

    fn route(prefix: &str) -> Route {
        Route {
            route_key: prefix.trim_matches('/').replace('/', "_"),
            prefix: prefix.to_string(),
            origin: "https://example.pages.dev".to_string(),
            rewrite_to: "/".to_string(),
            sitemap: None,
        }
    }

    fn sample_routes() -> Vec<Route> {
        vec![
            route("/tools/ast-viz"),
            route("/tools/bloom-filter"),
            route("/"),
        ]
    }

    #[test]
    fn lookup_returns_nested_routes() {
        let mut found = lookup_paths("/tools", &sample_routes());
        found.sort();
        assert_eq!(found, vec!["/tools/ast-viz", "/tools/bloom-filter"]);
    }

    #[test]
    fn lookup_matches_trailing_slash_form() {
        assert_eq!(lookup_paths("/tools/", &sample_routes()).len(), 2);
    }

    #[test]
    fn lookup_excludes_the_page_itself() {
        assert!(!lookup_paths("/tools/ast-viz", &sample_routes())
            .contains(&"/tools/ast-viz".to_string()));
    }

    #[test]
    fn lookup_is_empty_for_leaf_pages() {
        assert!(lookup_paths("/blog/some-post", &sample_routes()).is_empty());
    }

    #[test]
    fn lookup_from_root_covers_all_other_routes() {
        let mut found = lookup_paths("/", &sample_routes());
        found.sort();
        assert_eq!(found, vec!["/tools/ast-viz", "/tools/bloom-filter"]);
    }

    /// Renaming a section upstream must not require a change in this repo.
    #[test]
    fn lookup_follows_renamed_sections() {
        let renamed = vec![route("/utilities/ast-viz"), route("/")];
        assert_eq!(lookup_paths("/utilities", &renamed), vec!["/utilities/ast-viz"]);
        assert!(lookup_paths("/tools", &renamed).is_empty());
    }

    #[test]
    fn normalize_strips_trailing_slash() {
        assert_eq!(normalize_path("/blog/post/"), "/blog/post");
        assert_eq!(normalize_path("/blog/post"), "/blog/post");
    }

    #[test]
    fn normalize_keeps_root() {
        assert_eq!(normalize_path("/"), "/");
        assert_eq!(normalize_path(""), "/");
    }

    #[test]
    fn normalize_lowercases() {
        assert_eq!(normalize_path("/Blog/Post"), "/blog/post");
    }

    #[test]
    fn day_bucket_advances_daily() {
        let day = day_bucket(1_700_000_000_000.0);
        assert_eq!(day_bucket(1_700_000_000_000.0 + 3_600_000.0), day);
        assert_eq!(day_bucket(1_700_000_000_000.0 + 86_400_000.0), day + 1);
    }

    #[test]
    fn format_count_groups_thousands() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(1_234_567), "1,234,567");
    }
}
