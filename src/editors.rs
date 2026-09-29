//! Who last edited each page.
//!
//! When Cairn publishes a page it notes who did it, when, and a fingerprint
//! of what was published, in `_system/edited/<page path>.json`. Pages stay
//! plain Markdown. If a page no longer matches its fingerprint, it was
//! changed outside Cairn (in Notepad, say), and nobody is named.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::fsutil::{read_optional, sha256_hex, write_atomic};
use crate::locks::now_secs;
use crate::paths::{Root, split_relative};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditRecord {
    /// The name the person uses in Cairn.
    pub by: String,
    /// Unix seconds.
    pub at: u64,
    /// SHA-256 of the page as published.
    pub hash: String,
}

/// What Cairn knows about a page's last change.
#[derive(Debug, Clone, PartialEq)]
pub enum LastEdit {
    /// Last published in Cairn, and unchanged since.
    By(EditRecord),
    /// Changed since Cairn last published it.
    Outside,
    /// No record (published before Cairn kept them, or never through Cairn).
    Unknown,
}

/// Where the records for pages inside folder `rel` live.
pub fn edited_path(root: &Root, rel: &str) -> Option<PathBuf> {
    let mut path = root.system_dir().join("edited");
    for part in split_relative(rel).ok()? {
        path.push(part);
    }
    Some(path)
}

fn record_path(root: &Root, rel: &str) -> Option<PathBuf> {
    let mut path = edited_path(root, rel)?.into_os_string();
    path.push(".json");
    Some(path.into())
}

/// Note that `by` has just published `content` as `rel`. Best effort: a
/// page that published fine is never failed because this note wasn't saved.
pub fn record(root: &Root, rel: &str, by: &str, content: &[u8]) {
    let Some(path) = record_path(root, rel) else {
        return;
    };
    let rec = EditRecord {
        by: by.to_string(),
        at: now_secs(),
        hash: sha256_hex(content),
    };
    if let (Some(dir), Ok(bytes)) = (path.parent(), serde_json::to_vec_pretty(&rec)) {
        let _ = fs::create_dir_all(dir).and_then(|_| write_atomic(&path, &bytes));
    }
}

pub fn lookup(root: &Root, rel: &str) -> Option<EditRecord> {
    let bytes = read_optional(&record_path(root, rel)?).ok()??;
    serde_json::from_slice(&bytes).ok()
}

/// Who last edited `rel`, given the page's current bytes.
pub fn last_edit(root: &Root, rel: &str, current: &[u8]) -> LastEdit {
    match lookup(root, rel) {
        Some(rec) if rec.hash == sha256_hex(current) => LastEdit::By(rec),
        Some(_) => LastEdit::Outside,
        None => LastEdit::Unknown,
    }
}

/// Cairn itself rewrote the page (links fixed after a move): keep who
/// wrote it, and match the new text.
pub fn rehash(root: &Root, rel: &str, content: &[u8]) {
    let (Some(rec), Some(path)) = (lookup(root, rel), record_path(root, rel)) else {
        return;
    };
    let rec = EditRecord {
        hash: sha256_hex(content),
        ..rec
    };
    if let Ok(bytes) = serde_json::to_vec_pretty(&rec) {
        let _ = write_atomic(&path, &bytes);
    }
}

/// A page moved: its record goes with it. Best effort, like the earlier
/// versions beside it. (A folder's records are the folder
/// [`edited_path`], which the mover merges like earlier versions.)
pub fn move_record(root: &Root, from: &str, to: &str) {
    let (Some(src), Some(dst)) = (record_path(root, from), record_path(root, to)) else {
        return;
    };
    if !src.is_file() {
        return;
    }
    if let Some(parent) = dst.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::rename(&src, &dst);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, Root) {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("_system")).unwrap();
        let root = Root::new(dir.path()).unwrap();
        (dir, root)
    }

    #[test]
    fn names_the_editor_until_the_file_changes_elsewhere() {
        let (_d, root) = root();
        assert_eq!(last_edit(&root, "Guides/a.md", b"x"), LastEdit::Unknown);
        record(&root, "Guides/a.md", "Priya", b"one");
        match last_edit(&root, "Guides/a.md", b"one") {
            LastEdit::By(rec) => assert_eq!(rec.by, "Priya"),
            other => panic!("{other:?}"),
        }
        assert_eq!(last_edit(&root, "Guides/a.md", b"two"), LastEdit::Outside);
        rehash(&root, "Guides/a.md", b"two");
        assert!(
            matches!(last_edit(&root, "Guides/a.md", b"two"), LastEdit::By(r) if r.by == "Priya")
        );
    }

    #[test]
    fn records_follow_moved_pages() {
        let (_d, root) = root();
        record(&root, "Guides/a.md", "Priya", b"one");
        move_record(&root, "Guides/a.md", "Guides/b.md");
        assert!(lookup(&root, "Guides/b.md").is_some());
        assert!(lookup(&root, "Guides/a.md").is_none());
    }
}
