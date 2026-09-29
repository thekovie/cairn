//! Recently deleted pages and folders.
//!
//! Deleting never destroys anything. The page (with its `.assets` pictures)
//! or the whole folder is moved into `_system/trash/<id>/content/`, next to
//! an `item.json` that records where it came from:
//!
//! ```json
//! { "kind": "page", "path": "Guides/printer.md", "title": "Printer",
//!   "deleted_at": 1790000000, "deleted_by": "Sam", "page_count": 1 }
//! ```
//!
//! Restoring moves it back, exactly as it was. Earlier versions stay in
//! `_system/history` under the original path, so they come back too.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CairnError, Result};
use crate::fsutil::{read_optional, sha256_hex, write_atomic};
use crate::locks::{Identity, now_secs};
use crate::manage::{HeldLocks, managed_folder, not_moved, pages_under, require_writable_dir};
use crate::paths::Root;
use crate::publish::{assets_dir_rel, validate_article_path};
use crate::search::parent_of;

const ITEM_FILE: &str = "item.json";
const CONTENT_DIR: &str = "content";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrashItem {
    #[serde(default)]
    pub id: String,
    /// "page" or "folder".
    pub kind: String,
    /// Where it was, and where restoring puts it back.
    pub path: String,
    pub title: String,
    /// Unix seconds.
    pub deleted_at: u64,
    pub deleted_by: String,
    pub page_count: usize,
}

fn trash_dir(root: &Root) -> PathBuf {
    root.system_dir().join("trash")
}

fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if ok {
        Ok(())
    } else {
        Err(CairnError::PathRejected("Unknown deleted item.".into()))
    }
}

fn last_segment(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// Create the item's folder and record, ready to receive the content.
fn begin(root: &Root, item: &mut TrashItem) -> Result<PathBuf> {
    let short = uuid::Uuid::new_v4().simple().to_string();
    item.id = format!("{}-{}", now_secs(), &short[..8]);
    let dir = trash_dir(root).join(&item.id);
    fs::create_dir_all(dir.join(CONTENT_DIR))?;
    let bytes = serde_json::to_vec_pretty(item).expect("trash item serializes");
    if let Err(e) = write_atomic(&dir.join(ITEM_FILE), &bytes) {
        let _ = fs::remove_dir_all(&dir);
        return Err(e.into());
    }
    Ok(dir)
}

/// Delete a page: move it and its pictures into Recently deleted.
/// `expected_hash`, if given, must match the page as it is now.
pub fn trash_page(
    root: &Root,
    me: &Identity,
    rel: &str,
    title: &str,
    expected_hash: Option<&str>,
) -> Result<TrashItem> {
    let rel = validate_article_path(rel)?;
    if crate::templates::is_template_path(&rel) {
        return Err(CairnError::PathRejected(
            "Templates are managed on the Templates page.".into(),
        ));
    }
    let src = root.resolve(&rel)?;
    if let Some(dir) = src.parent() {
        require_writable_dir(dir)?;
    }
    let _locks = HeldLocks::take(root, me, std::slice::from_ref(&rel), "deleted")?;
    if let Some(expected) = expected_hash
        && sha256_hex(&fs::read(&src)?) != expected
    {
        return Err(CairnError::Conflict(
            "Someone changed this page since you opened it, so it wasn't deleted. Look at the \
             latest version first."
                .into(),
        ));
    }
    let mut item = TrashItem {
        id: String::new(),
        kind: "page".into(),
        path: rel.clone(),
        title: title.to_string(),
        deleted_at: now_secs(),
        deleted_by: me.display_name.clone(),
        page_count: 1,
    };
    let dir = begin(root, &mut item)?;
    let content = dir.join(CONTENT_DIR);
    if let Err(e) = fs::rename(&src, content.join(last_segment(&rel))) {
        let _ = fs::remove_dir_all(&dir);
        return Err(not_moved(e, "The page"));
    }
    let assets_rel = assets_dir_rel(&rel);
    let assets = root.resolve_for_create(&assets_rel)?;
    if assets.is_dir()
        && let Err(e) = fs::rename(&assets, content.join(last_segment(&assets_rel)))
    {
        let _ = fs::rename(content.join(last_segment(&rel)), &src);
        let _ = fs::remove_dir_all(&dir);
        return Err(not_moved(e, "The page's pictures"));
    }
    Ok(item)
}

/// Delete a folder and everything in it: move it into Recently deleted.
pub fn trash_folder(root: &Root, me: &Identity, rel: &str) -> Result<TrashItem> {
    let rel = managed_folder(rel)?;
    let src = root.resolve(&rel)?;
    if !src.is_dir() {
        return Err(CairnError::NotFound(
            "That folder could not be found.".into(),
        ));
    }
    if let Some(dir) = src.parent() {
        require_writable_dir(dir)?;
    }
    let pages = pages_under(root, &rel)?;
    let _locks = HeldLocks::take(root, me, &pages, "deleted")?;
    let mut item = TrashItem {
        id: String::new(),
        kind: "folder".into(),
        path: rel.clone(),
        title: last_segment(&rel).to_string(),
        deleted_at: now_secs(),
        deleted_by: me.display_name.clone(),
        page_count: pages.len(),
    };
    let dir = begin(root, &mut item)?;
    if let Err(e) = fs::rename(&src, dir.join(CONTENT_DIR).join(last_segment(&rel))) {
        let _ = fs::remove_dir_all(&dir);
        return Err(not_moved(e, "The folder"));
    }
    Ok(item)
}

fn read_item(root: &Root, id: &str) -> Result<TrashItem> {
    validate_id(id)?;
    let bytes = read_optional(&trash_dir(root).join(id).join(ITEM_FILE))?
        .ok_or_else(|| CairnError::NotFound("That deleted item could not be found.".into()))?;
    let mut item: TrashItem = serde_json::from_slice(&bytes)
        .map_err(|_| CairnError::Io("That deleted item's record couldn't be read.".into()))?;
    item.id = id.to_string();
    Ok(item)
}

/// Everything in Recently deleted, newest first.
pub fn list(root: &Root) -> Vec<TrashItem> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(trash_dir(root)) else {
        return out;
    };
    for entry in entries.flatten() {
        let id = entry.file_name().to_string_lossy().into_owned();
        if let Ok(item) = read_item(root, &id)
            && entry
                .path()
                .join(CONTENT_DIR)
                .join(last_segment(&item.path))
                .exists()
        {
            out.push(item);
        }
    }
    out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then(b.id.cmp(&a.id)));
    out
}

/// Put a deleted page or folder back where it was.
pub fn restore(root: &Root, me: &Identity, id: &str) -> Result<TrashItem> {
    let item = read_item(root, id)?;
    let is_page = item.kind == "page";
    let rel = if is_page {
        validate_article_path(&item.path)?
    } else {
        managed_folder(&item.path)?
    };
    let content = trash_dir(root).join(id).join(CONTENT_DIR);
    let target = root.resolve_for_create(&rel)?;
    if target.exists() {
        let what = if is_page { "A page" } else { "A folder" };
        return Err(CairnError::Conflict(format!(
            "{what} with the same name is already in that place. Rename or move it first, then \
             restore this one."
        )));
    }
    let parent = root.resolve_for_create(parent_of(&rel))?;
    fs::create_dir_all(&parent)?;
    require_writable_dir(&parent)?;
    if !is_page {
        fs::rename(content.join(last_segment(&rel)), &target)
            .map_err(|e| not_moved(e, "The folder"))?;
        let _ = fs::remove_dir_all(trash_dir(root).join(id));
        return Ok(TrashItem { path: rel, ..item });
    }
    let _locks = HeldLocks::take(root, me, std::slice::from_ref(&rel), "restored")?;
    let assets_rel = assets_dir_rel(&rel);
    let assets_src = content.join(last_segment(&assets_rel));
    let assets_dst = root.resolve_for_create(&assets_rel)?;
    if assets_src.is_dir() && assets_dst.exists() {
        return Err(CairnError::Conflict(
            "A pictures folder with the same name is already in that place, so the page wasn't \
             restored."
                .into(),
        ));
    }
    fs::rename(content.join(last_segment(&rel)), &target).map_err(|e| not_moved(e, "The page"))?;
    if assets_src.is_dir()
        && let Err(e) = fs::rename(&assets_src, &assets_dst)
    {
        let _ = fs::rename(&target, content.join(last_segment(&rel)));
        return Err(not_moved(e, "The page's pictures"));
    }
    let _ = fs::remove_dir_all(trash_dir(root).join(id));
    Ok(TrashItem { path: rel, ..item })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_cannot_name_other_folders() {
        assert!(validate_id("1790000000-a1b2c3d4").is_ok());
        assert!(validate_id("../history").is_err());
        assert!(validate_id("").is_err());
    }
}
