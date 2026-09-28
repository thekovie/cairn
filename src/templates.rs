//! Page templates.
//!
//! Built-in templates ship with Cairn. Team templates are ordinary Markdown
//! files in `<documentation>/_templates/`, so each documentation folder has
//! its own set, readable outside Cairn and included in backups. They are
//! edited, locked, versioned, and published exactly like pages.
//!
//! A template's front matter may hold `template_name` and
//! `template_description` (stripped from new pages) plus any page defaults.
//! Placeholders `{{title}}`, `{{date}}`, `{{author}}`, and `{{folder}}` are
//! filled in when a page is created.

use std::fs;
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::article::parse_front_matter;
use crate::error::{CairnError, Result};
use crate::fsutil::is_link_like;
use crate::paths::{Root, validate_name};

pub const TEMPLATES_DIR: &str = "_templates";

#[derive(Debug, Clone, Serialize)]
pub struct Template {
    /// "builtin:how-to" or "_templates/meeting-notes.md".
    pub id: String,
    pub name: String,
    pub description: String,
    pub builtin: bool,
    /// Unix seconds, team templates only.
    pub modified: Option<u64>,
}

/// Values substituted into placeholders.
pub struct Vars<'a> {
    pub title: &'a str,
    pub date: &'a str,
    pub author: &'a str,
    pub folder: &'a str,
}

const PAGE_DEFAULTS: &str = "---\nowner: \"{{author}}\"\nstatus: draft\nlast_reviewed: {{date}}\ntags: []\n---\n\n# {{title}}\n\n";

/// (id, name, description, body after the shared page defaults)
const BUILTIN: &[(&str, &str, &str, &str)] = &[
    (
        "blank",
        "Blank page",
        "Start with an empty page.",
        "Start writing here.\n",
    ),
    (
        "how-to",
        "Step-by-step guide",
        "Explain how to do a task, one step at a time.",
        "Use this page to explain how to do one task, step by step.\n\n\
         ## Before you start\n\n- What you need\n\n\
         ## Steps\n\n1. First step\n2. Second step\n3. Third step\n\n\
         ## If something goes wrong\n\nWhat to check, or who to ask.\n",
    ),
    (
        "troubleshooting",
        "Problem and fix",
        "Describe a problem and how to solve it.",
        "## The problem\n\nDescribe what people see (an error message, a symptom).\n\n\
         ## Why it happens\n\nA short explanation.\n\n\
         ## How to fix it\n\n1. First thing to try\n2. Next thing to try\n\n\
         ## Still not working?\n\nWho to contact.\n",
    ),
    (
        "reference",
        "Reference list",
        "A table of facts or settings to look up.",
        "A short summary of what this page lists.\n\n\
         | Item | Details |\n| --- | --- |\n| Example | Description |\n",
    ),
];

/// Whether a workspace-relative path is a team template.
pub fn is_template_path(rel: &str) -> bool {
    rel.split_once('/').is_some_and(|(dir, file)| {
        dir.eq_ignore_ascii_case(TEMPLATES_DIR)
            && !file.contains('/')
            && file.to_ascii_lowercase().ends_with(".md")
    })
}

fn builtin_raw(key: &str) -> Option<String> {
    BUILTIN
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, _, _, body)| format!("{PAGE_DEFAULTS}{body}"))
}

fn scalar(meta: &serde_yaml::Value, key: &str) -> Option<String> {
    meta.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// The raw YAML between the opening `---` and closing `---`, if present.
fn front_matter_block(text: &str) -> Option<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut out = String::new();
    for line in rest.split_inclusive('\n') {
        let t = line.trim_end_matches(['\r', '\n']);
        if t == "---" || t == "..." {
            return Some(out);
        }
        out.push_str(line);
    }
    None
}

/// Name and description from a template's front matter (lenient).
fn describe(text: &str, file_name: &str) -> (String, String) {
    let fallback = crate::article::title_from_filename(file_name);
    let block = front_matter_block(text).unwrap_or_default();
    let meta: serde_yaml::Value = serde_yaml::from_str(&block).unwrap_or(serde_yaml::Value::Null);
    (
        scalar(&meta, "template_name").unwrap_or(fallback),
        scalar(&meta, "template_description").unwrap_or_default(),
    )
}

/// Built-in templates followed by this documentation folder's templates.
pub fn list(root: &Root) -> Result<Vec<Template>> {
    let mut out: Vec<Template> = BUILTIN
        .iter()
        .map(|(key, name, description, _)| Template {
            id: format!("builtin:{key}"),
            name: (*name).into(),
            description: (*description).into(),
            builtin: true,
            modified: None,
        })
        .collect();
    let mut team = Vec::new();
    if let Ok(entries) = fs::read_dir(root.path().join(TEMPLATES_DIR)) {
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if file_name.starts_with('.') || !file_name.to_ascii_lowercase().ends_with(".md") {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if !meta.is_file() || is_link_like(&meta) {
                continue;
            }
            let text = fs::read_to_string(entry.path()).unwrap_or_default();
            let (name, description) = describe(&text, &file_name);
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            team.push(Template {
                id: format!("{TEMPLATES_DIR}/{file_name}"),
                name,
                description,
                builtin: false,
                modified,
            });
        }
    }
    team.sort_by_key(|t| t.name.to_lowercase());
    out.extend(team);
    Ok(out)
}

/// The raw text of a template (built-in or team).
pub fn load(root: &Root, id: &str) -> Result<String> {
    if let Some(key) = id.strip_prefix("builtin:") {
        return builtin_raw(key)
            .ok_or_else(|| CairnError::NotFound("That template doesn't exist.".into()));
    }
    if !is_template_path(id) {
        return Err(CairnError::PathRejected("That isn't a template.".into()));
    }
    let path = root.resolve(id)?;
    fs::read_to_string(path)
        .map_err(|_| CairnError::NotFound("That template could not be read.".into()))
}

/// Make a new page's text from a template: drop `template_*` front-matter
/// keys and fill in placeholders.
pub fn instantiate(raw: &str, vars: &Vars) -> String {
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let text = match front_matter_block(raw) {
        Some(block) => {
            let kept: Vec<&str> = block
                .lines()
                .filter(|l| !l.trim_start().starts_with("template_"))
                .collect();
            let (_, body) = parse_front_matter(raw);
            if kept.iter().all(|l| l.trim().is_empty()) {
                body.trim_start_matches(['\r', '\n']).to_string()
            } else {
                format!("---\n{}\n---\n{body}", kept.join("\n"))
            }
        }
        None => raw.to_string(),
    };
    // Keep the author's name safe inside a quoted YAML value.
    let author = vars.author.replace(['"', '\\'], "'");
    text.replace("{{title}}", vars.title)
        .replace("{{date}}", vars.date)
        .replace("{{author}}", &author)
        .replace("{{folder}}", vars.folder)
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
            dash = false;
        } else if !dash && !slug.is_empty() {
            slug.push('-');
            dash = true;
        }
        if slug.len() >= 60 {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "template".into()
    } else {
        slug
    }
}

/// A free `_templates/<slug>.md` path for a new template.
pub fn new_template_path(root: &Root, name: &str) -> Result<String> {
    let slug = slugify(name);
    for n in 1..500 {
        let file = if n == 1 {
            format!("{slug}.md")
        } else {
            format!("{slug}-{n}.md")
        };
        validate_name(&file)?;
        let rel = format!("{TEMPLATES_DIR}/{file}");
        if !root.resolve_for_create(&rel)?.exists() {
            return Ok(rel);
        }
    }
    Err(CairnError::Conflict(
        "Couldn't find a free file name for this template.".into(),
    ))
}

fn yaml_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Starting text for a new team template, optionally copied from another.
pub fn scaffold(name: &str, description: &str, from: Option<&str>) -> String {
    let header = format!(
        "template_name: {}\ntemplate_description: {}",
        yaml_quote(name.trim()),
        yaml_quote(description.trim())
    );
    let source = from.unwrap_or("---\nstatus: draft\n---\n\n# {{title}}\n\n");
    match front_matter_block(source) {
        Some(block) => {
            let (_, body) = parse_front_matter(source);
            let rest: Vec<&str> = block
                .lines()
                .filter(|l| !l.trim_start().starts_with("template_"))
                .collect();
            let rest = rest.join("\n");
            let rest = rest.trim();
            if rest.is_empty() {
                format!("---\n{header}\n---\n{body}")
            } else {
                format!("---\n{header}\n{rest}\n---\n{body}")
            }
        }
        None => format!("---\n{header}\n---\n\n{source}"),
    }
}

/// Starting text copied from a built-in (`builtin:<key>`) or team template.
pub fn source_for_copy(root: &Root, id: &str) -> Result<String> {
    load(root, id)
}

#[derive(Debug, Clone, Serialize)]
pub struct DeletedTemplate {
    /// The template's former path, for `/api/history/restore`.
    pub path: String,
    pub name: String,
    /// Newest saved version id.
    pub latest_version: String,
}

/// Team templates that were deleted but still have saved versions.
pub fn deleted(root: &Root) -> Result<Vec<DeletedTemplate>> {
    let dir = root.system_dir().join("history").join(TEMPLATES_DIR);
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{TEMPLATES_DIR}/{file_name}");
        if !is_template_path(&rel) || root.resolve_for_create(&rel)?.exists() {
            continue;
        }
        let versions = crate::history::list_versions(root, &rel)?;
        let Some(latest) = versions.first() else {
            continue;
        };
        let bytes = crate::history::read_version(root, &rel, &latest.id)?;
        let (name, _) = describe(&String::from_utf8_lossy(&bytes), &file_name);
        out.push(DeletedTemplate {
            path: rel,
            name,
            latest_version: latest.id.clone(),
        });
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instantiate_strips_template_keys_and_fills_placeholders() {
        let raw = "---\ntemplate_name: Notes\ntemplate_description: x\nstatus: draft\n---\n\
                   # {{title}}\n{{date}} by {{author}} in {{folder}}\n";
        let vars = Vars {
            title: "Weekly",
            date: "2026-09-28",
            author: "Sam \"S\"",
            folder: "Guides",
        };
        let out = instantiate(raw, &vars);
        assert!(!out.contains("template_"));
        assert!(out.starts_with("---\nstatus: draft\n---\n"));
        assert!(out.contains("# Weekly\n2026-09-28 by Sam 'S' in Guides"));
    }

    #[test]
    fn template_paths_are_single_level_markdown_in_templates() {
        assert!(is_template_path("_templates/a.md"));
        assert!(!is_template_path("_templates/sub/a.md"));
        assert!(!is_template_path("Guides/a.md"));
        assert!(!is_template_path("_templates/a.txt"));
    }

    #[test]
    fn scaffold_keeps_source_defaults() {
        let s = scaffold(
            "Meeting \"notes\"",
            "Agenda",
            builtin_raw("how-to").as_deref(),
        );
        assert!(s.contains("template_name: \"Meeting \\\"notes\\\"\""));
        assert!(s.contains("status: draft"));
        assert!(s.contains("## Steps"));
    }
}
