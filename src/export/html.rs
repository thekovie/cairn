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

/// Text between tags, with the few entities the renderer writes undone, so
/// it can be compared with a page title.
fn plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Lower case, runs of spaces made single: what two names are compared by.
fn same_words(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A page's title (its first heading, else its file name), read once.
fn page_title(root: &Root, rel: &str, cache: &mut HashMap<String, String>) -> String {
    cache
        .entry(rel.to_string())
        .or_insert_with(|| {
            let text = root
                .resolve(rel)
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            article::extract_title(article::parse_front_matter(&text).1)
                .unwrap_or_else(|| article::title_from_filename(rel))
        })
        .clone()
}

/// The value of attribute `name` in an opening tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let needle = format!(" {name}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].replace("&amp;", "&"))
}

/// In a PDF a link inside the documentation can't lead anywhere, so it keeps
/// its meaning instead: the text stays, and the page (or file) it pointed at
/// is named beside it. A link to a heading on the page itself stays a real
/// link. Web links, and links that were already broken, are left alone.
fn readable_internal_links(html: &str, root: &Root, current: &str) -> String {
    let mut titles: HashMap<String, String> = HashMap::new();
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<a ") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(tag_end) = after.find('>') else {
            break;
        };
        let tag = &after[..=tag_end];
        let href = attribute(tag, "href")
            .filter(|h| h.starts_with("#/page/") || h.starts_with("/ws-file/"));
        let close = after[tag_end + 1..].find("</a>");
        let (Some(href), Some(close)) = (href, close) else {
            out.push_str(tag);
            rest = &after[tag_end + 1..];
            continue;
        };
        let inner = &after[tag_end + 1..tag_end + 1 + close];
        rest = &after[tag_end + 1 + close + "</a>".len()..];

        let (path_part, query) = match href.split_once('?') {
            Some((p, q)) => (p, q),
            None => (href.as_str(), ""),
        };
        let encoded = path_part
            .strip_prefix("#/page/")
            .or_else(|| path_part.strip_prefix("/ws-file/"))
            .unwrap_or(path_part);
        let target = article::percent_decode(encoded);
        let text = plain_text(inner);

        if href.starts_with("#/page/") && target.eq_ignore_ascii_case(current) {
            // A link into this very page: a heading link works in the PDF.
            let section = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("section="))
                .map(article::percent_decode)
                .filter(|s| !s.is_empty());
            match section {
                Some(id) => out.push_str(&format!("<a href=\"#{}\">{inner}</a>", escape(&id))),
                None => out.push_str(&format!("<a>{inner}</a>")),
            }
            continue;
        }
        out.push_str(&format!("<a>{inner}</a>"));
        if href.starts_with("#/page/") {
            let title = page_title(root, &target, &mut titles);
            if same_words(&text) != same_words(&title) {
                let folder = target.rsplit_once('/').map_or("", |(f, _)| f);
                let here = current.rsplit_once('/').map_or("", |(f, _)| f);
                // Say where it is only when that isn't where this page is.
                let place = if folder.is_empty() || folder.eq_ignore_ascii_case(here) {
                    String::new()
                } else {
                    format!(" in {}", escape(&folder.replace('/', " › ")))
                };
                out.push_str(&format!(
                    " <span class=\"print-ref\">(see “{}”{place})</span>",
                    escape(&title)
                ));
            }
        } else {
            let file = target.rsplit('/').next().unwrap_or(&target);
            if same_words(&text) != same_words(file) {
                out.push_str(&format!(
                    " <span class=\"print-ref\">(file “{}”)</span>",
                    escape(&target)
                ));
            }
        }
    }
    out.push_str(rest);
    out
}

/// The page body as self-contained HTML: pictures embedded, links inside
/// the documentation turned into readable references, and the first heading
/// removed (the header shows it). Also returns that heading's id, so links
/// to the title still work.
fn body_html(root: &Root, rel: &str, body: &str) -> (String, Option<String>) {
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
    let html = readable_internal_links(&html, root, rel);
    match (html.trim_start().starts_with("<h1"), html.find("</h1>")) {
        (true, Some(end)) => {
            let title_id = html
                .find('>')
                .and_then(|open| attribute(&html[..=open], "id"));
            (html[end + "</h1>".len()..].to_string(), title_id)
        }
        _ => (html, None),
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
    let (content, title_id) = body_html(input.root, input.rel, body);
    let title_attr = title_id
        .map(|id| format!(" id=\"{}\"", escape(&id)))
        .unwrap_or_default();

    // Every font the stylesheet names goes inside the file, so a saved
    // page looks the same anywhere.
    let tokens =
        ["InterVariable-latin.woff2"]
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
         <header class=\"print-head\"><p class=\"print-ws\">{ws}</p><h1{title_attr}>{title}</h1>\
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
}
