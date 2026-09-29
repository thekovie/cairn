//! Self-contained printable HTML, used to make PDFs.
//!
//! The document needs nothing outside itself: pictures are embedded as
//! `data:` URIs, the stylesheets and font are inlined, and a strict
//! Content-Security-Policy forbids scripts and network access. The body comes
//! from the same sanitizing Markdown renderer as the app.

use std::collections::HashMap;

use jiff::tz::TimeZone;

use crate::article::{self, RenderContext};
use crate::images::sniff_mime;
use crate::paths::Root;
use crate::timefmt;

/// Everything needed to print one page.
pub struct PrintInput<'a> {
    pub root: &'a Root,
    pub rel: &'a str,
    pub text: &'a str,
    pub workspace_name: &'a str,
    pub zone: &'a TimeZone,
    /// "a4" or "letter".
    pub paper: &'a str,
    /// Unix seconds of the file's last change, if known.
    pub modified: Option<i64>,
    /// Unix seconds of the export.
    pub exported_at: i64,
}

const CSP: &str = "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:";

/// Placeholder picture URLs handed to the renderer, swapped for data after
/// sanitizing (the sanitizer only allows http/https/mailto URLs).
const PICTURE_PLACEHOLDER: &str = "/cairn-export-picture/";

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Standard base64 (RFC 4648) for embedding bytes in `data:` URIs.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let n = (u32::from(chunk[0]) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn asset_text(path: &str) -> String {
    crate::server::embedded_asset(path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}

/// Every picture the page shows that exists and is a real image, as
/// (workspace path, data URI).
fn embedded_pictures(root: &Root, rel: &str, body: &str) -> Vec<(String, String)> {
    let mut targets: Vec<String> = article::referenced_local_targets(rel, body)
        .into_iter()
        .collect();
    targets.sort();
    targets
        .into_iter()
        .filter_map(|target| {
            let bytes = std::fs::read(root.resolve(&target).ok()?).ok()?;
            let mime = sniff_mime(&bytes)?;
            Some((target, format!("data:{mime};base64,{}", base64(&bytes))))
        })
        .collect()
}

/// Remove `href` attributes whose value starts with `prefix`, so the link
/// text stays but no longer points anywhere.
fn strip_hrefs(html: &str, prefix: &str) -> String {
    let needle = format!(" href=\"{prefix}");
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(&needle) {
        out.push_str(&rest[..start]);
        let after = &rest[start + needle.len()..];
        match after.find('"') {
            Some(end) => rest = &after[end + 1..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The page body as self-contained HTML: pictures embedded, in-app links
/// reduced to plain text, and the first heading removed (the header shows it).
fn body_html(root: &Root, rel: &str, body: &str) -> String {
    let pictures = embedded_pictures(root, rel, body);
    let staged: HashMap<String, String> = pictures
        .iter()
        .enumerate()
        .map(|(i, (target, _))| (target.clone(), format!("{PICTURE_PLACEHOLDER}{i}")))
        .collect();
    // A throwaway key: file links get stripped below, so it never matters.
    let key = uuid::Uuid::new_v4().simple().to_string();
    let ctx = RenderContext {
        root,
        article_rel: rel,
        read_key: &key,
        staged: &staged,
    };
    let mut html = article::render(body, &ctx).html;
    // Replace highest indexes first so "/…/1" doesn't match inside "/…/12".
    for (i, (_, data)) in pictures.iter().enumerate().rev() {
        html = html.replace(
            &format!("\"{PICTURE_PLACEHOLDER}{i}\""),
            &format!("\"{data}\""),
        );
    }
    let html = strip_hrefs(&strip_hrefs(&html, "#/"), "/ws-file/");
    match (html.trim_start().starts_with("<h1"), html.find("</h1>")) {
        (true, Some(end)) => html[end + "</h1>".len()..].to_string(),
        _ => html,
    }
}

fn status_label(status: &str) -> &str {
    match status {
        "active" => "Active",
        "draft" => "Draft",
        "retired" => "Retired",
        other => other,
    }
}

/// Build the printable document for one page.
pub fn printable_html(input: &PrintInput) -> String {
    let (meta, body) = article::parse_front_matter(input.text);
    let title =
        article::extract_title(body).unwrap_or_else(|| article::title_from_filename(input.rel));
    let content = body_html(input.root, input.rel, body);

    // Every font the stylesheet names goes inside the file, so a saved
    // page looks the same anywhere.
    let tokens = [
        "InterVariable-latin.woff2",
        "SourceSerif4Variable-latin.woff2",
        "SourceSerif4Variable-Italic-latin.woff2",
    ]
    .iter()
    .fold(asset_text("css/tokens.css"), |css, file| {
        let data = crate::server::embedded_asset(&format!("fonts/{file}"))
            .map(|b| format!("data:font/woff2;base64,{}", base64(&b)))
            .unwrap_or_default();
        css.replace(&format!("/static/fonts/{file}"), &data)
    });
    let markdown = asset_text("css/markdown.css");
    let print = asset_text("css/print.css");
    let size = if input.paper == "letter" {
        "letter"
    } else {
        "A4"
    };

    let mut details: Vec<(&str, String)> = Vec::new();
    if let Some(owner) = &meta.owner {
        details.push(("Owner", owner.clone()));
    }
    if let Some(status) = &meta.status {
        details.push(("Status", status_label(status).to_string()));
    }
    if let Some(reviewed) = &meta.last_reviewed {
        details.push(("Last reviewed", reviewed.clone()));
    }
    if let Some(modified) = input.modified {
        details.push(("Last changed", timefmt::format_moment(modified, input.zone)));
    }
    let details_html: String = details
        .iter()
        .map(|(k, v)| format!("<div><dt>{k}</dt><dd>{}</dd></div>", escape(v)))
        .collect();
    let exported = timefmt::format_moment(input.exported_at, input.zone);

    format!(
        "<!doctype html>\n<html lang=\"en\" data-theme=\"light\" data-text=\"normal\"><head>\
         <meta charset=\"utf-8\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\">\
         <title>{title}</title>\
         <style>{tokens}\n{markdown}\n{print}\n@page {{ size: {size}; margin: 16mm 15mm; }}</style>\
         </head><body class=\"print-doc\">\
         <header class=\"print-head\"><p class=\"print-ws\">{ws}</p><h1>{title}</h1>\
         <dl class=\"print-meta\">{details_html}</dl></header>\
         <main class=\"md-body\">{content}</main>\
         <footer class=\"print-foot\">Exported {exported} from {ws} with Cairn</footer>\
         </body></html>",
        title = escape(&title),
        ws = escape(input.workspace_name),
        exported = escape(&exported),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn strip_hrefs_keeps_link_text() {
        let html = "<a href=\"#/page/a.md\" rel=\"x\">A</a> <a href=\"https://e.com\">E</a>";
        assert_eq!(
            strip_hrefs(html, "#/"),
            "<a rel=\"x\">A</a> <a href=\"https://e.com\">E</a>"
        );
    }
}
