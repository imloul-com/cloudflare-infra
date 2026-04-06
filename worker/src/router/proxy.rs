use js_sys::Uint8Array;
use lol_html::html_content::ContentType;
use lol_html::{element, rewrite_str, RewriteStrSettings};
use worker::{console_error, Fetch, Headers, Request, RequestInit, Response, Result};

use super::matcher::RouteMatch;

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

/// Injects `<base href="...">` as the first content inside `<head>` using [lol_html] (selector +
/// streaming rewriter), tolerating real-world HTML. On failure, logs and returns the original HTML.
///
/// [lol_html]: https://docs.rs/lol_html/
fn inject_base_into_head(html: &str, base_href: &str) -> String {
    let safe = escape_html_attr_value(base_href);
    match rewrite_str(html, RewriteStrSettings {
        element_content_handlers: vec![element!("head", move |el| {
            let snippet = format!("<base href=\"{safe}\">");
            el.prepend(&snippet, ContentType::Html);
            Ok(())
        })],
        ..RewriteStrSettings::new()
    }) {
        Ok(out) => out,
        Err(e) => {
            console_error!(
                "base tag injection failed (lol_html), serving unmodified HTML: {}",
                e
            );
            html.to_string()
        }
    }
}

pub async fn proxy_request(mut req: Request, m: RouteMatch) -> Result<Response> {
    let incoming_url = req.url()?;
    let mut upstream_url = m.upstream;
    upstream_url.set_query(incoming_url.query());

    let method = req.method();

    let mut new_headers = Headers::new();
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

    let upstream_req = Request::new_with_init(&upstream_url.to_string(), &init)?;
    let response = Fetch::Request(upstream_req).send().await?;

    if m.route.rewrite_to == "/" && m.route.prefix != "/" {
        let content_type = response.headers().get("content-type")?.unwrap_or_default();
        if content_type.contains("text/html") {
            return inject_base_tag(response, &m.route.prefix).await;
        }
    }

    Ok(response)
}

async fn inject_base_tag(mut response: Response, prefix: &str) -> Result<Response> {
    let status = response.status_code();

    let mut headers = Headers::new();
    for (key, val) in response.headers() {
        headers.set(&key, &val)?;
    }

    let html = response.text().await?;

    let base_href = if prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{}/", prefix)
    };
    let injected = inject_base_into_head(&html, &base_href);
    let body_bytes = injected.into_bytes();

    headers.set("content-length", &body_bytes.len().to_string())?;

    Response::from_bytes(body_bytes).map(|r| r.with_headers(headers).with_status(status))
}

#[cfg(test)]
mod tests {
    use super::{escape_html_attr_value, inject_base_into_head};

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
        let out = inject_base_into_head(html, "/tools/ast-viz/");
        assert!(out.contains("href=\"/tools/ast-viz/\"") || out.contains("href='/tools/ast-viz/'"));
        let base_pos = out.find("<base").expect("base tag");
        let title_pos = out.find("<title").expect("title");
        assert!(base_pos < title_pos);
    }

    #[test]
    fn inject_base_with_head_attributes() {
        let html = "<html><head lang=\"en\"><title>T</title></head></html>";
        let out = inject_base_into_head(html, "/tools/");
        assert!(out.contains("/tools/"));
        assert!(out.contains("lang=\"en\"") || out.contains("lang='en'"));
    }

    #[test]
    fn inject_base_whitespace_in_head() {
        let html = "<html><head>\n  <title>T</title>\n</head></html>";
        let out = inject_base_into_head(html, "/p/");
        assert!(out.contains("/p/"));
        assert!(out.contains("<title"));
    }
}
