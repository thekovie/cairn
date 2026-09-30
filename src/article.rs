//! Parsing and rendering a single Markdown article.
//!
//! Rendering is done server-side so the article view and the editor preview
//! use the exact same pipeline:
//!
//! Markdown → pulldown-cmark events (raw HTML turned into visible text, links
//! and images rewritten or flagged) → HTML → ammonia allowlist sanitizer.
//!
//! Nothing here fetches remote content. Remote images become a labeled link.

use std::collections::{HashMap, HashSet};

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use crate::paths::{Root, join_relative_link};

// ------------------------------------------------------------ front matter

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct ArticleMeta {
    pub owner: Option<String>,
    pub status: Option<String>,
    pub last_reviewed: Option<String>,
    pub tags: Vec<String>,
    /// Non-fatal problems with the front matter, in plain language.
    pub warnings: Vec<String>,
}

const STATUSES: &[&str] = &["draft", "active", "retired"];

/// Split optional YAML front matter from the body. Invalid metadata never
/// makes the article unreadable: problems become warnings and the body is
/// always returned.
pub fn parse_front_matter(text: &str) -> (ArticleMeta, &str) {
    let mut meta = ArticleMeta::default();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some(after_open) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        return (meta, text);
    };
    // Find the closing fence on its own line.
    let mut offset = 0;
    let mut close = None;
    for line in after_open.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed == "---" || trimmed == "..." {
            close = Some((offset, offset + line.len()));
            break;
        }
        offset += line.len();
    }
    let Some((yaml_end, body_start)) = close else {
        meta.warnings
            .push("The page information block at the top is not closed with ---.".into());
        return (meta, text);
    };
    let yaml = &after_open[..yaml_end];
    let body = &after_open[body_start..];

    let value: serde_yaml::Value = match serde_yaml::from_str(yaml) {
        Ok(v) => v,
        Err(_) => {
            meta.warnings
                .push("The page information block at the top couldn't be read.".into());
            return (meta, body);
        }
    };
    let Some(map) = value.as_mapping() else {
        if !value.is_null() {
            meta.warnings
                .push("The page information block at the top couldn't be read.".into());
        }
        return (meta, body);
    };
    let get = |key: &str| map.get(serde_yaml::Value::String(key.into()));

    if let Some(owner) = get("owner") {
        match scalar_to_string(owner) {
            Some(s) if !s.trim().is_empty() => meta.owner = Some(s.trim().to_string()),
            _ => meta.warnings.push("The owner should be a name.".into()),
        }
    }
    if let Some(status) = get("status") {
        match scalar_to_string(status).map(|s| s.trim().to_lowercase()) {
            Some(s) if STATUSES.contains(&s.as_str()) => meta.status = Some(s),
            _ => meta
                .warnings
                .push("The status should be one of: draft, active, retired.".into()),
        }
    }
    if let Some(reviewed) = get("last_reviewed") {
        match scalar_to_string(reviewed) {
            Some(s) if is_valid_date(s.trim()) => meta.last_reviewed = Some(s.trim().to_string()),
            _ => meta.warnings.push(
                "The last reviewed date should look like 2026-01-31 (year-month-day).".into(),
            ),
        }
    }
    if let Some(tags) = get("tags") {
        match tags {
            serde_yaml::Value::Sequence(items) => {
                meta.tags = items
                    .iter()
                    .filter_map(scalar_to_string)
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
            other => match scalar_to_string(other) {
                Some(s) => {
                    meta.tags = s
                        .split(',')
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect()
                }
                None => meta.warnings.push("Tags should be a list of words.".into()),
            },
        }
    }
    (meta, body)
}

fn scalar_to_string(v: &serde_yaml::Value) -> Option<String> {
    match v {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn is_valid_date(s: &str) -> bool {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    s.len() == 10 && time::Date::parse(s, &format).is_ok()
}

// ------------------------------------------------------------------ title

pub(crate) fn md_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        // Callouts: "> [!NOTE]", "> [!WARNING]" and so on, as on GitHub.
        | Options::ENABLE_GFM
}

/// The text of the first level-1 heading, if any.
pub fn extract_title(body: &str) -> Option<String> {
    let mut in_h1 = false;
    let mut title = String::new();
    for event in Parser::new_ext(body, md_options()) {
        match event {
            Event::Start(Tag::Heading {
                level: HeadingLevel::H1,
                ..
            }) => in_h1 = true,
            Event::End(TagEnd::Heading(HeadingLevel::H1)) if in_h1 => {
                let t = title.trim().to_string();
                return (!t.is_empty()).then_some(t);
            }
            Event::Text(t) | Event::Code(t) if in_h1 => title.push_str(&t),
            _ => {}
        }
    }
    None
}

/// Fallback display title from a file name: `new-user-setup.md` → `New user setup`.
pub fn title_from_filename(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let stem = name.strip_suffix(".md").unwrap_or(name);
    let spaced = stem.replace(['-', '_'], " ");
    let mut chars = spaced.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Untitled".into(),
    }
}

/// Plain text of the body for search indexing. A heading ends like a
/// sentence, so search excerpts don't run it into the text after it.
pub fn plain_text(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    for event in Parser::new_ext(body, md_options()) {
        match event {
            Event::End(TagEnd::Heading(_)) => {
                let kept = out.trim_end().len();
                out.truncate(kept);
                if kept > 0 && !out.ends_with(['.', ':', '?', '!']) {
                    out.push('.');
                }
                out.push(' ');
            }
            Event::Text(t) | Event::Code(t) => {
                out.push_str(&t);
                out.push(' ');
            }
            Event::SoftBreak | Event::HardBreak | Event::End(_) => out.push(' '),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// -------------------------------------------------------------- rendering

#[derive(Debug, Clone, Serialize)]
pub struct TocEntry {
    pub level: u8,
    pub text: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Rendered {
    pub html: String,
    pub toc: Vec<TocEntry>,
    /// Local link/image targets that don't exist (workspace-relative).
    pub broken_links: Vec<String>,
}

/// What the renderer needs to know to rewrite local links.
pub struct RenderContext<'a> {
    pub root: &'a Root,
    /// Workspace-relative path of the article being rendered.
    pub article_rel: &'a str,
    /// Per-launch key appended to file URLs so `<img>` requests are authorized.
    pub read_key: &'a str,
    /// Draft pictures not yet published: workspace-relative target → preview URL.
    pub staged: &'a HashMap<String, String>,
}

enum LinkKind {
    Local,
    External,
    Anchor,
    Unsafe,
}

fn classify(dest: &str) -> LinkKind {
    let lower = dest.trim().to_ascii_lowercase();
    if lower.starts_with('#') {
        LinkKind::Anchor
    } else if lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
    {
        LinkKind::External
    } else if lower.contains(':') || lower.starts_with("//") || lower.starts_with('\\') {
        // javascript:, data:, file:, C:\..., protocol-relative, UNC
        LinkKind::Unsafe
    } else {
        LinkKind::Local
    }
}

/// Whether a link target is a relative path inside the documentation.
pub(crate) fn is_local_link(dest: &str) -> bool {
    matches!(classify(dest), LinkKind::Local)
}

/// Minimal percent-decoding for link targets (`%20` → space).
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(hi * 16 + lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    (b as char).to_digit(16).map(|d| d as u8)
}

/// Percent-encode a workspace-relative path for use in a URL, segment by segment.
pub fn encode_path(rel: &str) -> String {
    rel.split('/')
        .map(|seg| {
            let mut out = String::new();
            for b in seg.bytes() {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Resolve a local link target from an article to a workspace-relative path.
/// Returns `None` if it climbs out of the workspace.
pub fn resolve_local_target(article_rel: &str, dest: &str) -> Option<String> {
    let without_fragment = dest.split(['#', '?']).next().unwrap_or("");
    if without_fragment.is_empty() {
        return None;
    }
    join_relative_link(article_rel, &percent_decode(without_fragment))
}

/// All local link and image targets in `body`, resolved to workspace-relative
/// paths. Used by publishing to decide which staged pictures are referenced.
pub fn referenced_local_targets(article_rel: &str, body: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for event in Parser::new_ext(body, md_options()) {
        if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event
            && matches!(classify(&dest_url), LinkKind::Local)
            && let Some(rel) = resolve_local_target(article_rel, &dest_url)
        {
            out.insert(rel);
        }
    }
    out
}

fn slugify(text: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            slug.push(c);
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        "section".into()
    } else {
        slug
    }
}

fn is_image_ext(rel: &str) -> bool {
    let lower = rel.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .iter()
        .any(|e| lower.ends_with(e))
}

/// Render a Markdown body to sanitized HTML.
pub fn render(body: &str, ctx: &RenderContext) -> Rendered {
    let events: Vec<Event> = Parser::new_ext(body, md_options()).collect();

    // Pass 1: heading texts → unique ids.
    let mut toc = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut heading_ids = Vec::new();
    let mut current: Option<(u8, String)> = None;
    for event in &events {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((*level as u8, String::new()))
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, text)) = current.as_mut() {
                    text.push_str(t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    let base = slugify(&text);
                    let n = used.entry(base.clone()).or_insert(0);
                    let id = if *n == 0 {
                        base.clone()
                    } else {
                        format!("{base}-{n}")
                    };
                    *n += 1;
                    heading_ids.push(id.clone());
                    toc.push(TocEntry {
                        level,
                        text: text.trim().to_string(),
                        id,
                    });
                }
            }
            _ => {}
        }
    }

    // Pass 2: rewrite.
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut broken = Vec::new();
    let mut heading_index = 0;
    // Stack of link rewrites so the matching end tag can be adjusted.
    let mut link_stack: Vec<bool> = Vec::new(); // true = emitted raw <a>, close with raw </a>
    let mut skip_image: Option<(String, String)> = None; // remote image: (url, alt so far)
    let mut broken_image: Option<(String, String)> = None; // missing local image: (rel, alt)

    for event in events {
        if let Some((url, alt)) = skip_image.as_mut() {
            match event {
                Event::End(TagEnd::Image) => {
                    let label = if alt.trim().is_empty() {
                        "picture".to_string()
                    } else {
                        alt.trim().to_string()
                    };
                    out.push(Event::Html(CowStr::from(format!(
                        "<a class=\"external-image\" href=\"{}\">External picture (not loaded): {}</a>",
                        escape_html(url),
                        escape_html(&label)
                    ))));
                    skip_image = None;
                }
                Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                _ => {}
            }
            continue;
        }
        if let Some((rel, alt)) = broken_image.as_mut() {
            match event {
                Event::End(TagEnd::Image) => {
                    out.push(Event::Html(CowStr::from(format!(
                        "<span class=\"broken-image\" title=\"This picture file is missing\">Missing picture: {}</span>",
                        escape_html(if alt.trim().is_empty() { rel } else { alt.trim() })
                    ))));
                    broken_image = None;
                }
                Event::Text(t) | Event::Code(t) => alt.push_str(&t),
                _ => {}
            }
            continue;
        }

        match event {
            // Raw HTML is never passed through; it is shown as text.
            Event::Html(raw) | Event::InlineHtml(raw) => out.push(Event::Text(raw)),

            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                let id = heading_ids.get(heading_index).cloned().unwrap_or_default();
                heading_index += 1;
                out.push(Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(id)),
                    classes,
                    attrs,
                }));
            }

            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => match classify(&dest_url) {
                LinkKind::Anchor | LinkKind::External => {
                    link_stack.push(false);
                    out.push(Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        id,
                    }));
                }
                LinkKind::Unsafe => {
                    link_stack.push(true);
                    out.push(Event::Html(CowStr::from(
                        "<a class=\"blocked-link\" title=\"This link type is not allowed\">",
                    )));
                }
                LinkKind::Local => {
                    let fragment = dest_url.split_once('#').map(|(_, f)| f.to_string());
                    match resolve_local_target(ctx.article_rel, &dest_url) {
                        Some(rel) if ctx.root.resolve(&rel).is_ok() => {
                            let href = if rel.to_ascii_lowercase().ends_with(".md") {
                                let mut h = format!("#/page/{}", encode_path(&rel));
                                if let Some(f) = fragment {
                                    h.push_str(&format!("?section={}", encode_path(&f)));
                                }
                                h
                            } else {
                                format!("/ws-file/{}?k={}", encode_path(&rel), ctx.read_key)
                            };
                            link_stack.push(false);
                            out.push(Event::Start(Tag::Link {
                                link_type,
                                dest_url: CowStr::from(href),
                                title,
                                id,
                            }));
                        }
                        Some(rel) => {
                            broken.push(rel.clone());
                            link_stack.push(true);
                            out.push(Event::Html(CowStr::from(format!(
                                "<a class=\"broken-link\" title=\"Missing page or file: {}\">",
                                escape_html(&rel)
                            ))));
                        }
                        None => {
                            broken.push(dest_url.to_string());
                            link_stack.push(true);
                            out.push(Event::Html(CowStr::from(
                                "<a class=\"broken-link\" title=\"This link points outside the documentation folder\">",
                            )));
                        }
                    }
                }
            },
            Event::End(TagEnd::Link) => {
                if link_stack.pop().unwrap_or(false) {
                    out.push(Event::Html(CowStr::from("</a>")));
                } else {
                    out.push(Event::End(TagEnd::Link));
                }
            }

            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => match classify(&dest_url) {
                LinkKind::External => skip_image = Some((dest_url.to_string(), String::new())),
                LinkKind::Local => match resolve_local_target(ctx.article_rel, &dest_url) {
                    Some(rel) if ctx.staged.contains_key(&rel) => {
                        let url = ctx.staged[&rel].clone();
                        out.push(Event::Start(Tag::Image {
                            link_type,
                            dest_url: CowStr::from(url),
                            title,
                            id,
                        }));
                    }
                    Some(rel) if is_image_ext(&rel) && ctx.root.resolve(&rel).is_ok() => {
                        let url = format!("/ws-file/{}?k={}", encode_path(&rel), ctx.read_key);
                        out.push(Event::Start(Tag::Image {
                            link_type,
                            dest_url: CowStr::from(url),
                            title,
                            id,
                        }));
                    }
                    Some(rel) => {
                        broken.push(rel.clone());
                        broken_image = Some((rel, String::new()));
                    }
                    None => {
                        broken.push(dest_url.to_string());
                        broken_image = Some((dest_url.to_string(), String::new()));
                    }
                },
                LinkKind::Anchor | LinkKind::Unsafe => {
                    broken_image = Some((dest_url.to_string(), String::new()));
                }
            },

            other => out.push(other),
        }
    }

    let mut raw_html = String::new();
    pulldown_cmark::html::push_html(&mut raw_html, out.into_iter());
    broken.sort();
    broken.dedup();
    Rendered {
        html: sanitize(&raw_html),
        toc,
        broken_links: broken,
    }
}

/// Render release notes (Markdown from GitHub) to sanitized HTML. Raw HTML
/// is shown as text and pictures as their description, as in pages.
pub fn render_notes(md: &str) -> String {
    let mut in_image = false;
    let events = Parser::new_ext(md, md_options()).filter_map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Some(Event::Text(raw)),
        Event::Start(Tag::Image { .. }) => {
            in_image = true;
            None
        }
        Event::End(TagEnd::Image) => {
            in_image = false;
            None
        }
        Event::Text(t) => Some(Event::Text(t)),
        _ if in_image => None,
        other => Some(other),
    });
    let mut raw_html = String::new();
    pulldown_cmark::html::push_html(&mut raw_html, events);
    sanitize(&raw_html)
}

/// The callout kinds a page can use ("> [!NOTE]" …), as the classes the
/// renderer gives their boxes.
const CALLOUT_CLASSES: [&str; 5] = [
    "markdown-alert-note",
    "markdown-alert-tip",
    "markdown-alert-important",
    "markdown-alert-warning",
    "markdown-alert-caution",
];

fn sanitize(html: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(ammonia::UrlRelative::PassThrough)
        .link_rel(Some("noopener noreferrer"))
        .add_tags(["input"])
        .add_tag_attributes("input", ["type", "checked", "disabled"])
        .add_generic_attributes(["title"])
        .add_allowed_classes("a", ["broken-link", "blocked-link", "external-image"])
        .add_allowed_classes("span", ["broken-image"])
        .add_allowed_classes("blockquote", CALLOUT_CLASSES)
        .add_allowed_classes("code", ["language-*"]);
    for h in ["h1", "h2", "h3", "h4", "h5", "h6"] {
        builder.add_tag_attributes(h, ["id"]);
    }
    builder.clean(html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_to_a_section_points_at_its_page() {
        // "Section of that page" in the link dialog writes page.md#heading-id.
        assert_eq!(
            resolve_local_target("Guides/a.md", "../Reference/b.md#who-to-call").as_deref(),
            Some("Reference/b.md")
        );
    }

    #[test]
    fn callouts_keep_their_kind_and_nothing_else() {
        let out = sanitize(&{
            let mut html = String::new();
            pulldown_cmark::html::push_html(
                &mut html,
                Parser::new_ext("> [!WARNING]\n> Unplug it first.\n", md_options()),
            );
            html
        });
        assert!(
            out.contains(r#"<blockquote class="markdown-alert-warning">"#),
            "{out}"
        );
        assert!(!out.contains("[!WARNING]"), "{out}");
        assert!(!sanitize(r#"<blockquote class="evil">x</blockquote>"#).contains("evil"));
        // As the visual editor writes it once tidied (see visual.js).
        let mut html = String::new();
        pulldown_cmark::html::push_html(
            &mut html,
            Parser::new_ext("> [!TIP]\n> Save often.\n", md_options()),
        );
        assert!(html.contains("markdown-alert-tip"), "{html}");
    }

    #[test]
    fn invalid_front_matter_still_returns_body() {
        let (meta, body) = parse_front_matter("---\nstatus: [oops\n---\n# Title\n");
        assert!(!meta.warnings.is_empty());
        assert_eq!(body, "# Title\n");
    }

    #[test]
    fn valid_front_matter_fields() {
        let text =
            "---\nowner: Sam\nstatus: Active\nlast_reviewed: 2026-01-31\ntags: [a, b]\n---\nBody";
        let (meta, body) = parse_front_matter(text);
        assert_eq!(meta.owner.as_deref(), Some("Sam"));
        assert_eq!(meta.status.as_deref(), Some("active"));
        assert_eq!(meta.last_reviewed.as_deref(), Some("2026-01-31"));
        assert_eq!(meta.tags, vec!["a", "b"]);
        assert!(meta.warnings.is_empty());
        assert_eq!(body, "Body");
    }

    #[test]
    fn bad_values_are_warnings_not_errors() {
        let (meta, _) = parse_front_matter("---\nstatus: maybe\nlast_reviewed: 31/01/2026\n---\nx");
        assert_eq!(meta.status, None);
        assert_eq!(meta.warnings.len(), 2);
    }

    #[test]
    fn title_is_first_h1() {
        assert_eq!(
            extract_title("intro\n\n## Sub\n\n# Real Title\n").as_deref(),
            Some("Real Title")
        );
        assert_eq!(
            title_from_filename("Guides/new-user-setup.md"),
            "New user setup"
        );
    }

    #[test]
    fn percent_decoding() {
        assert_eq!(percent_decode("a%20b.md"), "a b.md");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
