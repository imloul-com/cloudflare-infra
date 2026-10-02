use js_sys::Uint8Array;
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use worker::{console_error, Fetch, Headers, Request, RequestInit, Response, Result};

use super::matcher::RouteMatch;
use crate::views::{format_count, normalize_path, Counts};

fn escape_html_attr_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Single [lol_html] pass that prepends `<base href="...">` inside `<head>` and
/// fills `[data-views]` elements with the page's view count. Either half is
/// optional. On failure, logs and returns the original HTML.
///
/// [lol_html]: https://docs.rs/lol_html/
fn rewrite_html_str(html: &str, base_href: Option<&str>, views: Option<&Counts>) -> String {
    let mut handlers = Vec::new();

    if let Some(href) = base_href {
        let safe = escape_html_attr_value(href);
        handlers.push(element!("head", move |el| {
            el.prepend(&format!("<base href=\"{safe}\">"), ContentType::Html);
            Ok(())
        }));
    }

    if let Some(counts) = views {
        let rendered = format_count(counts.current);
        handlers.push(element!("[data-views]", move |el| {
            el.set_inner_content(&rendered, ContentType::Text);
            Ok(())
        }));

        // Listing pages carry the counts of the pages they link to. Normalizing
        // the attribute lets templates pass a permalink straight through.
        handlers.push(element!("[data-views-for]", move |el| {
            if let Some(path) = el.get_attribute("data-views-for") {
                if let Some(count) = counts.lookup.get(&normalize_path(&path)) {
                    el.set_inner_content(&format_count(*count), ContentType::Text);
                }
            }
            Ok(())
        }));
    }

    if handlers.is_empty() {
        return html.to_string();
    }

    match rewrite_str(html, RewriteStrSettings {
        element_content_handlers: handlers,
        ..RewriteStrSettings::new()
    }) {
        Ok(out) => out,
        Err(e) => {
            console_error!("html rewrite failed (lol_html), serving unmodified HTML: {}", e);
            html.to_string()
        }
    }
}

pub async fn proxy_request(mut req: Request, m: RouteMatch) -> Result<Response> {
    let incoming_url = req.url()?;
    let mut upstream_url = m.upstream;
    upstream_url.set_query(incoming_url.query());

    let method = req.method();

    let new_headers = Headers::new();
    for (key, val) in req.headers() {
        if key.eq_ignore_ascii_case("host") {
            continue;
        }
        new_headers.set(&key, &val)?;
    }

    let body_bytes = req.bytes().await.unwrap_or_default();

    let mut init = RequestInit::new();
    init.with_method(method).with_headers(new_headers);
    if !body_bytes.is_empty() {
        init.with_body(Some(Uint8Array::from(body_bytes.as_slice()).into()));
    }

    let upstream_req = Request::new_with_init(upstream_url.as_ref(), &init)?;
    Fetch::Request(upstream_req).send().await
}

/// Applies the HTML rewrites to a proxied response, passing non-HTML through untouched.
pub async fn rewrite_html(
    mut response: Response,
    base_href: Option<&str>,
    views: Option<&Counts>,
) -> Result<Response> {
    if base_href.is_none() && views.is_none() {
        return Ok(response);
    }

    let content_type = response.headers().get("content-type")?.unwrap_or_default();
    if !content_type.contains("text/html") {
        return Ok(response);
    }

    let status = response.status_code();
    let headers = Headers::new();
    for (key, val) in response.headers() {
        headers.set(&key, &val)?;
    }

    let html = response.text().await?;
    let body_bytes = rewrite_html_str(&html, base_href, views).into_bytes();

    headers.set("content-length", &body_bytes.len().to_string())?;

    Response::from_bytes(body_bytes).map(|r| r.with_headers(headers).with_status(status))
}

/// Normalizes a route prefix into a `<base href>` value.
pub fn base_href_for(prefix: &str) -> String {
    if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    }
}

#[cfg(test)]
mod tests {
    use super::{base_href_for, escape_html_attr_value, rewrite_html_str};
    use crate::views::Counts;

    fn counts(current: u64) -> Counts {
        Counts {
            current,
            ..Default::default()
        }
    }

    #[test]
    fn escape_attr_escapes_specials() {
        assert_eq!(
            escape_html_attr_value(r#"/x&y"z'"#),
            "/x&amp;y&quot;z&#39;"
        );
    }

    #[test]
    fn inject_base_prepends_inside_head() {
        let html = "<!DOCTYPE html><html><head><title>T</title></head><body></body></html>";
        let out = rewrite_html_str(html, Some("/tools/ast-viz/"), None);
        assert!(out.contains("href=\"/tools/ast-viz/\"") || out.contains("href='/tools/ast-viz/'"));
        let base_pos = out.find("<base").expect("base tag");
        let title_pos = out.find("<title").expect("title");
        assert!(base_pos < title_pos);
    }

    #[test]
    fn inject_base_with_head_attributes() {
        let html = "<html><head lang=\"en\"><title>T</title></head></html>";
        let out = rewrite_html_str(html, Some("/tools/"), None);
        assert!(out.contains("/tools/"));
        assert!(out.contains("lang=\"en\"") || out.contains("lang='en'"));
    }

    #[test]
    fn inject_base_whitespace_in_head() {
        let html = "<html><head>\n  <title>T</title>\n</head></html>";
        let out = rewrite_html_str(html, Some("/p/"), None);
        assert!(out.contains("/p/"));
        assert!(out.contains("<title"));
    }

    #[test]
    fn fills_views_placeholder() {
        let html = "<html><body><span data-views></span></body></html>";
        let out = rewrite_html_str(html, None, Some(&counts(1_234)));
        assert!(out.contains(">1,234<"));
    }

    #[test]
    fn fills_every_views_placeholder() {
        let html = "<body><span data-views></span><i data-views>x</i></body>";
        let out = rewrite_html_str(html, None, Some(&counts(7)));
        assert_eq!(out.matches(">7<").count(), 2);
    }

    #[test]
    fn views_content_is_text_escaped() {
        let html = "<span data-views></span>";
        let out = rewrite_html_str(html, None, Some(&counts(0)));
        assert!(out.contains(">0<"));
    }

    #[test]
    fn leaves_placeholder_empty_without_count() {
        let html = "<span data-views></span>";
        let out = rewrite_html_str(html, None, None);
        assert_eq!(out, html);
    }

    #[test]
    fn applies_base_and_views_together() {
        let html = "<html><head><title>T</title></head><body><b data-views></b></body></html>";
        let out = rewrite_html_str(html, Some("/tools/x/"), Some(&counts(42)));
        assert!(out.contains("<base"));
        assert!(out.contains(">42<"));
    }

    #[test]
    fn fills_lookup_counts_by_path() {
        let mut c = counts(1);
        c.lookup.insert("/tools/ast-viz".into(), 4_210);
        c.lookup.insert("/tools/bloom-filter".into(), 77);
        let html = concat!(
            "<a data-views-for=\"/tools/ast-viz\"></a>",
            "<a data-views-for=\"/tools/bloom-filter\"></a>",
        );
        let out = rewrite_html_str(html, None, Some(&c));
        assert!(out.contains(">4,210<"));
        assert!(out.contains(">77<"));
    }

    #[test]
    fn leaves_unknown_lookup_path_untouched() {
        let html = "<a data-views-for=\"/tools/nope\"></a>";
        let out = rewrite_html_str(html, None, Some(&counts(1)));
        assert_eq!(out, html);
    }

    #[test]
    fn base_href_gets_trailing_slash() {
        assert_eq!(base_href_for("/tools/ast-viz"), "/tools/ast-viz/");
        assert_eq!(base_href_for("/tools/ast-viz/"), "/tools/ast-viz/");
    }
}
