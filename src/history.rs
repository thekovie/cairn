//! Earlier published versions of articles.
//!
//! Before a publish replaces an article, the previous content is copied to
//! `_system/history/<article path>/<UTC timestamp>-<short hash>.md`. These are
//! ordinary Markdown files, readable without Cairn.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::editors::{self, LastEdit};
use crate::error::{CairnError, Result};
use crate::fsutil::{create_new_with, sha256_hex};
use crate::paths::{Root, split_relative};

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    /// File name, used as the id when viewing or restoring.
    pub id: String,
    /// RFC 3339 UTC time the version was saved.
    pub saved_at: String,
    pub size: u64,
    /// Who published this version, when Cairn knows.
    pub by: Option<String>,
}

/// Beside each version, who published it: `<version>.json`.
fn author_file(dir: &std::path::Path, id: &str) -> PathBuf {
    dir.join(format!("{}.json", id.trim_end_matches(".md")))
}

#[derive(Serialize, Deserialize)]
struct Author {
    by: String,
}

fn history_dir(root: &Root, article_rel: &str) -> Result<PathBuf> {
    let mut dir = root.system_dir().join("history");
    for part in split_relative(article_rel)? {
        dir.push(part);
    }
    Ok(dir)
}

/// Each page keeps at least this many of its newest versions, however old.
pub const KEEP_NEWEST: usize = 3;
/// Versions older than this many days are removed, beyond those.
pub const KEEP_DAYS: i64 = 30;

/// Tidy one page's history folder: remove versions saved before `cutoff`
/// (a version-id time stamp), except the newest [`KEEP_NEWEST`]. Only files
/// named like versions are ever touched. Returns how many were removed.
fn prune_dir(dir: &Path, cutoff: &str) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| validate_id(id).is_ok())
        .collect();
    // Ids start with their UTC time stamp, so this is newest first.
    ids.sort_by(|a, b| b.cmp(a));
    let mut removed = 0;
    for id in ids.iter().skip(KEEP_NEWEST) {
        let old = id.get(..19).is_some_and(|s| s < &cutoff[..19]);
        if old && fs::remove_file(dir.join(id)).is_ok() {
            let _ = fs::remove_file(author_file(dir, id));
            removed += 1;
        }
    }
    removed
}

fn cutoff(now: OffsetDateTime) -> String {
    stamp(now - time::Duration::days(KEEP_DAYS))
}

/// Tidy every page's earlier versions. Best effort: a folder that can't be
/// read or a file that can't be removed is left for next time.
pub fn prune_all(root: &Root) -> usize {
    let cutoff = cutoff(OffsetDateTime::now_utc());
    walkdir::WalkDir::new(root.system_dir().join("history"))
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_dir())
        .map(|e| prune_dir(e.path(), &cutoff))
        .sum()
}

fn stamp(now: OffsetDateTime) -> String {
    let fmt = time::macros::format_description!(
        "[year][month][day]T[hour][minute][second][subsecond digits:3]Z"
    );
    now.format(&fmt)
        .unwrap_or_else(|_| "00000000T000000000Z".into())
}

/// Save `bytes` as a version of `article_rel`. Never overwrites an existing
/// version file.
pub fn save_version(root: &Root, article_rel: &str, bytes: &[u8]) -> Result<String> {
    let dir = history_dir(root, article_rel)?;
    fs::create_dir_all(&dir)?;
    let hash = sha256_hex(bytes);
    for attempt in 0..5 {
        let base = format!("{}-{}", stamp(OffsetDateTime::now_utc()), &hash[..8]);
        let id = if attempt == 0 {
            format!("{base}.md")
        } else {
            format!("{base}-{attempt}.md")
        };
        if create_new_with(&dir.join(&id), bytes)? {
            // Best effort: who published the text being kept.
            if let LastEdit::By(rec) = editors::last_edit(root, article_rel, bytes)
                && let Ok(note) = serde_json::to_vec(&Author { by: rec.by })
            {
                let _ = create_new_with(&author_file(&dir, &id), &note);
            }
            prune_dir(&dir, &cutoff(OffsetDateTime::now_utc()));
            return Ok(id);
        }
    }
    Err(CairnError::Io(
        "Could not save the previous version.".into(),
    ))
}

/// Strictly validate a version id so it can only name a file in the
/// article's own history folder.
fn validate_id(id: &str) -> Result<()> {
    let ok = id.len() <= 64
        && id.ends_with(".md")
        && id.starts_with(|c: char| c.is_ascii_digit())
        && id[..id.len() - 3]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CairnError::PathRejected("Unknown version.".into()))
    }
}

fn parse_stamp(id: &str) -> Option<String> {
    let s = id.get(..19)?; // 20260928T143000123Z
    let (y, mo, d) = (s.get(0..4)?, s.get(4..6)?, s.get(6..8)?);
    let (h, mi, se, ms) = (
        s.get(9..11)?,
        s.get(11..13)?,
        s.get(13..15)?,
        s.get(15..18)?,
    );
    Some(format!("{y}-{mo}-{d}T{h}:{mi}:{se}.{ms}Z"))
}

/// Versions of an article, newest first.
pub fn list_versions(root: &Root, article_rel: &str) -> Result<Vec<Version>> {
    let dir = history_dir(root, article_rel)?;
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if validate_id(&id).is_err() {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let by = fs::read(author_file(&dir, &id))
            .ok()
            .and_then(|b| serde_json::from_slice::<Author>(&b).ok())
            .map(|a| a.by);
        out.push(Version {
            saved_at: parse_stamp(&id).unwrap_or_default(),
            id,
            size: meta.len(),
            by,
        });
    }
    out.sort_by(|a, b| b.id.cmp(&a.id));
    Ok(out)
}

pub fn read_version(root: &Root, article_rel: &str, id: &str) -> Result<Vec<u8>> {
    validate_id(id)?;
    let path = history_dir(root, article_rel)?.join(id);
    fs::read(&path)
        .map_err(|_| CairnError::NotFound("That earlier version could not be found.".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_versions_go_but_the_newest_three_stay() {
        let dir = tempfile::tempdir().unwrap();
        let page = dir.path();
        let ids = [
            "20260101T090000000Z-aaaaaaaa.md", // old
            "20260201T090000000Z-bbbbbbbb.md", // old
            "20260301T090000000Z-cccccccc.md", // old, but one of the newest three
            "20260310T090000000Z-dddddddd.md", // old, one of the newest three
            "20260320T090000000Z-eeeeeeee.md", // newest
        ];
        for id in ids {
            fs::write(page.join(id), id).unwrap();
        }
        fs::write(author_file(page, ids[0]), "{}").unwrap();
        fs::write(page.join("notes.txt"), "not a version").unwrap();

        let cutoff = "20260315T000000000Z";
        assert_eq!(prune_dir(page, cutoff), 2);
        for id in &ids[..2] {
            assert!(!page.join(id).exists(), "{id} should be gone");
        }
        assert!(!author_file(page, ids[0]).exists());
        for id in &ids[2..] {
            assert!(page.join(id).exists(), "{id} should stay");
        }
        assert!(page.join("notes.txt").exists());

        // Newer versions push the old ones out of the newest three; recent
        // versions all stay, even when there are more than three.
        for id in [
            "20260316T000000000Z-ffffffff.md",
            "20260317T000000000Z-gggggggg.md",
            "20260318T000000000Z-hhhhhhhh.md",
        ] {
            fs::write(page.join(id), id).unwrap();
        }
        assert_eq!(prune_dir(page, cutoff), 2);
        let left = fs::read_dir(page)
            .unwrap()
            .flatten()
            .filter(|e| validate_id(&e.file_name().to_string_lossy()).is_ok())
            .count();
        assert_eq!(left, 4);
    }

    #[test]
    fn id_validation_rejects_traversal() {
        assert!(validate_id("20260928T143000123Z-a1b2c3d4.md").is_ok());
        assert!(validate_id("../../secret.md").is_err());
        assert!(validate_id("20260928T143000123Z-a1b2c3d4.md/../x").is_err());
        assert_eq!(
            parse_stamp("20260928T143000123Z-a1b2c3d4.md").unwrap(),
            "2026-09-28T14:30:00.123Z"
        );
    }
}
