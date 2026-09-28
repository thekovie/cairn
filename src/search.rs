//! In-memory page index: folder listings, recently changed pages, and search.
//!
//! The index is rebuilt incrementally by comparing each file's modified time
//! and size, so a refresh on a large share only re-reads what changed. It
//! never leaves the workspace root, never follows links, and skips `_system`,
//! `.assets` folders, and hidden entries.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::time::UNIX_EPOCH;

use serde::Serialize;
use walkdir::WalkDir;

use crate::article::{extract_title, parse_front_matter, plain_text, title_from_filename};
use crate::error::Result;
use crate::fsutil::is_link_like;
use crate::paths::{Root, SYSTEM_DIR};

#[derive(Debug, Clone, Serialize)]
pub struct PageSummary {
    pub path: String,
    pub title: String,
    pub owner: Option<String>,
    pub status: Option<String>,
    pub last_reviewed: Option<String>,
    pub tags: Vec<String>,
    /// Unix seconds.
    pub modified: u64,
}

struct Entry {
    summary: PageSummary,
    text: String,
    text_lower: String,
    title_lower: String,
    size: u64,
}

#[derive(Default)]
pub struct SearchIndex {
    entries: HashMap<String, Entry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    pub text: String,
    pub hit: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub page: PageSummary,
    /// Excerpt split into plain and highlighted parts, so the UI can render
    /// highlights without inserting HTML.
    pub excerpt: Vec<Segment>,
}

pub fn skip_dir_name(name: &str) -> bool {
    name.starts_with('.')
        || name.eq_ignore_ascii_case(SYSTEM_DIR)
        || name.to_ascii_lowercase().ends_with(".assets")
}

pub fn summarize(path: &str, text: &str, modified: u64) -> (PageSummary, String) {
    let (meta, body) = parse_front_matter(text);
    let title = extract_title(body).unwrap_or_else(|| title_from_filename(path));
    let summary = PageSummary {
        path: path.to_string(),
        title,
        owner: meta.owner,
        status: meta.status,
        last_reviewed: meta.last_reviewed,
        tags: meta.tags,
        modified,
    };
    (summary, plain_text(body))
}

impl SearchIndex {
    /// Bring the index up to date with the files on disk.
    pub fn refresh(&mut self, root: &Root) -> Result<()> {
        let mut seen = HashSet::new();
        let walker = WalkDir::new(root.path())
            .follow_links(false)
            .max_depth(24)
            .into_iter()
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if e.file_type().is_dir() {
                    !skip_dir_name(&name)
                } else {
                    !name.starts_with('.')
                }
            });
        for entry in walker.flatten() {
            if !entry.file_type().is_file()
                || !entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .ends_with(".md")
            {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if is_link_like(&meta) {
                continue;
            }
            let Some(rel) = root.relative(entry.path()) else {
                continue;
            };
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let size = meta.len();
            seen.insert(rel.clone());
            if let Some(existing) = self.entries.get(&rel)
                && existing.summary.modified == modified
                && existing.size == size
            {
                continue;
            }
            let Ok(bytes) = fs::read(entry.path()) else {
                continue;
            };
            let (summary, plain) = summarize(&rel, &String::from_utf8_lossy(&bytes), modified);
            let entry = Entry {
                title_lower: summary.title.to_lowercase(),
                text_lower: plain.to_lowercase(),
                text: plain,
                summary,
                size,
            };
            self.entries.insert(rel, entry);
        }
        self.entries.retain(|k, _| seen.contains(k));
        Ok(())
    }

    pub fn get(&self, rel: &str) -> Option<PageSummary> {
        self.entries.get(rel).map(|e| e.summary.clone())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every page, sorted by path.
    pub fn all(&self) -> Vec<PageSummary> {
        let mut all: Vec<_> = self.entries.values().map(|e| e.summary.clone()).collect();
        all.sort_by_key(|p| p.path.to_lowercase());
        all
    }

    /// Most recently modified pages.
    pub fn recent(&self, limit: usize) -> Vec<PageSummary> {
        let mut all: Vec<_> = self.entries.values().map(|e| e.summary.clone()).collect();
        all.sort_by(|a, b| {
            b.modified
                .cmp(&a.modified)
                .then_with(|| a.title.cmp(&b.title))
        });
        all.truncate(limit);
        all
    }

    /// Pages directly inside `folder` ("" = workspace root).
    pub fn pages_in(&self, folder: &str) -> Vec<PageSummary> {
        let mut out: Vec<_> = self
            .entries
            .values()
            .filter(|e| parent_of(&e.summary.path).eq_ignore_ascii_case(folder))
            .map(|e| e.summary.clone())
            .collect();
        out.sort_by_key(|p| p.title.to_lowercase());
        out
    }

    /// Number of pages at or below `folder`.
    pub fn count_under(&self, folder: &str) -> usize {
        let prefix = format!("{}/", folder.to_lowercase());
        self.entries
            .keys()
            .filter(|k| k.to_lowercase().starts_with(&prefix))
            .count()
    }

    /// All words must appear in the title or body. Title matches rank higher.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let terms: Vec<String> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .take(8)
            .collect();
        if terms.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(usize, &Entry)> = self
            .entries
            .values()
            .filter_map(|e| {
                let mut score = 0;
                for t in &terms {
                    let in_title = e.title_lower.matches(t.as_str()).count();
                    let in_body = e.text_lower.matches(t.as_str()).count();
                    if in_title + in_body == 0 {
                        return None;
                    }
                    score += in_title * 20 + in_body.min(20);
                }
                Some((score, e))
            })
            .collect();
        scored.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.summary.title.cmp(&b.1.summary.title))
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, e)| SearchHit {
                page: e.summary.clone(),
                excerpt: excerpt(&e.text, &terms),
            })
            .collect()
    }
}

pub fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
}

fn floor_boundary(text: &str, mut i: usize) -> usize {
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(text: &str, mut i: usize) -> usize {
    while i < text.len() && !text.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Build an excerpt around the first matching term, split into segments.
fn excerpt(text: &str, terms: &[String]) -> Vec<Segment> {
    const RADIUS: usize = 90;
    let lower = text.to_lowercase();
    // Lowercasing can change byte lengths for some scripts; only highlight
    // when positions line up, otherwise show the start of the text.
    let aligned = lower.len() == text.len();
    let first = if aligned {
        terms
            .iter()
            .filter_map(|t| lower.find(t.as_str()))
            .min()
            .unwrap_or(0)
    } else {
        0
    };
    let start = floor_boundary(text, first.saturating_sub(RADIUS));
    let end = ceil_boundary(text, (first + RADIUS * 2).min(text.len()));
    let window = &text[start..end];

    let mut segments = Vec::new();
    if start > 0 {
        segments.push(Segment {
            text: "… ".into(),
            hit: false,
        });
    }
    let mut pos = 0;
    if aligned {
        let window_lower = &lower[start..end];
        while pos < window.len() {
            let next = terms
                .iter()
                .filter_map(|t| {
                    window_lower[pos..]
                        .find(t.as_str())
                        .map(|i| (pos + i, t.len()))
                })
                .min_by_key(|(i, _)| *i);
            let Some((i, len)) = next else { break };
            if !window.is_char_boundary(i) || !window.is_char_boundary(i + len) {
                break;
            }
            if i > pos {
                segments.push(Segment {
                    text: window[pos..i].to_string(),
                    hit: false,
                });
            }
            segments.push(Segment {
                text: window[i..i + len].to_string(),
                hit: true,
            });
            pos = i + len;
        }
    }
    if pos < window.len() {
        segments.push(Segment {
            text: window[pos..].to_string(),
            hit: false,
        });
    }
    if end < text.len() {
        segments.push(Segment {
            text: " …".into(),
            hit: false,
        });
    }
    segments
}

// ------------------------------------------------------------- folder tree

#[derive(Debug, Clone, Serialize)]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    pub page_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderListing {
    pub path: String,
    pub folders: Vec<FolderEntry>,
    pub pages: Vec<PageSummary>,
}

/// Every folder in the workspace (excluding `_system`, `.assets`, hidden
/// folders, and links), as workspace-relative paths sorted alphabetically.
pub fn all_folders(root: &Root) -> Vec<String> {
    let walker = WalkDir::new(root.path())
        .follow_links(false)
        .min_depth(1)
        .max_depth(12)
        .into_iter()
        .filter_entry(|e| {
            e.file_type().is_dir() && !skip_dir_name(&e.file_name().to_string_lossy())
        });
    let mut out: Vec<String> = walker
        .flatten()
        .filter(|e| fs::symlink_metadata(e.path()).is_ok_and(|m| !is_link_like(&m)))
        .filter_map(|e| root.relative(e.path()))
        .collect();
    out.sort_by_key(|p| p.to_lowercase());
    out
}

/// Subfolders and pages directly inside `folder_rel`.
pub fn list_folder(root: &Root, index: &SearchIndex, folder_rel: &str) -> Result<FolderListing> {
    let dir = root.resolve(folder_rel)?;
    let mut folders = Vec::new();
    for entry in fs::read_dir(&dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if !meta.is_dir() || is_link_like(&meta) || skip_dir_name(&name) {
            continue;
        }
        let path = if folder_rel.is_empty() {
            name.clone()
        } else {
            format!("{folder_rel}/{name}")
        };
        folders.push(FolderEntry {
            page_count: index.count_under(&path),
            name,
            path,
        });
    }
    folders.sort_by_key(|f| f.name.to_lowercase());
    Ok(FolderListing {
        path: folder_rel.to_string(),
        folders,
        pages: index.pages_in(folder_rel),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excerpt_highlights_terms() {
        let segs = excerpt(
            "Reset the printer by holding the power button.",
            &["printer".into()],
        );
        assert!(segs.iter().any(|s| s.hit && s.text == "printer"));
    }
}
