//! Renaming and moving pages and folders.
//!
//! Every operation first takes the edit locks of the pages it touches, so it
//! never runs while someone is editing them. A page moves together with its
//! `.assets` pictures folder and its earlier versions. Links are kept right:
//! links inside moved pages are adjusted to their new place, and links in
//! other pages are updated like any publish (the old text is kept as an
//! earlier version). A page someone is editing is left alone and reported,
//! so the person can fix its link later.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::article::title_from_filename;
use crate::error::{CairnError, Result};
use crate::fsutil::{can_write_dir, create_new_with, sha256_hex, write_atomic};
use crate::history;
use crate::links::{PathMap, may_link_to, rewrite_links, set_title};
use crate::locks::{self, Acquire, Identity};
use crate::paths::{Root, is_system_path, normalize_relative, split_relative, validate_name};
use crate::publish::{self, PublishOptions, PublishOutcome, PublishRequest, assets_dir_rel};
use crate::search::{parent_of, skip_dir_name};
use crate::templates::TEMPLATES_DIR;

/// A page whose links pointed at the moved place but couldn't be updated.
#[derive(Debug, Clone, Serialize)]
pub struct Skipped {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct LinkUpdate {
    /// Other pages whose links were updated.
    pub updated: Vec<String>,
    pub skipped: Vec<Skipped>,
}

/// One page that moved (or whose text changed because of the move).
#[derive(Debug, Clone)]
pub struct Moved {
    pub from: String,
    pub to: String,
    pub old_hash: String,
    pub new_hash: String,
}

#[derive(Debug, Clone)]
pub struct MoveOutcome {
    /// The page's or folder's new path.
    pub path: String,
    pub map: PathMap,
    pub moved: Vec<Moved>,
    pub links: LinkUpdate,
}

// ------------------------------------------------------------------ locks

/// Edit locks taken for an operation; released when dropped.
pub(crate) struct HeldLocks<'a> {
    root: &'a Root,
    me: &'a Identity,
    paths: Vec<String>,
}

impl<'a> HeldLocks<'a> {
    /// Lock every page in `paths`, or none. `verb` completes "this can't be
    /// … right now" in the message shown when someone is editing one.
    pub(crate) fn take(
        root: &'a Root,
        me: &'a Identity,
        paths: &[String],
        verb: &str,
    ) -> Result<HeldLocks<'a>> {
        let mut held = HeldLocks {
            root,
            me,
            paths: Vec::new(),
        };
        for p in paths {
            if held.paths.iter().any(|h| h.eq_ignore_ascii_case(p)) {
                continue;
            }
            if locks::status(root, p, me)?.is_some_and(|l| l.is_mine) {
                return Err(CairnError::Conflict(format!(
                    "You have “{}” open in the editor. Publish your changes or close the editor \
                     first, then try again.",
                    title_from_filename(p)
                )));
            }
            match locks::acquire(root, p, me)? {
                Acquire::Acquired(_) => held.paths.push(p.clone()),
                Acquire::HeldBy(v) => {
                    return Err(CairnError::Locked(format!(
                        "{} is editing “{}”, so this can't be {verb} right now. Try again when \
                         they have finished.",
                        v.info.display_name,
                        title_from_filename(p)
                    )));
                }
            }
        }
        Ok(held)
    }
}

impl Drop for HeldLocks<'_> {
    fn drop(&mut self) {
        for p in &self.paths {
            let _ = locks::release(self.root, p, self.me);
        }
    }
}

// ---------------------------------------------------------------- helpers

/// A folder name people can choose (the same rules as creating a folder).
pub fn validate_folder_name(name: &str) -> Result<()> {
    validate_name(name)?;
    let lower = name.to_ascii_lowercase();
    if name.starts_with('.') || lower.ends_with(".assets") || lower.ends_with(".md") {
        return Err(CairnError::BadRequest(
            "Please choose a different folder name.".into(),
        ));
    }
    if is_system_path(name) || name.eq_ignore_ascii_case(TEMPLATES_DIR) {
        return Err(CairnError::BadRequest(
            "That name is reserved for Cairn.".into(),
        ));
    }
    Ok(())
}

/// A folder that can be renamed, moved, or deleted: not the top level, and
/// not one of Cairn's own folders.
pub fn managed_folder(raw: &str) -> Result<String> {
    let rel = normalize_relative(raw)?;
    if rel.is_empty() {
        return Err(CairnError::BadRequest("Choose a folder first.".into()));
    }
    for part in split_relative(&rel)? {
        validate_folder_name(&part)
            .map_err(|_| CairnError::PathRejected("That folder can't be changed here.".into()))?;
    }
    Ok(rel)
}

/// New path for a folder called `name` inside `parent`.
pub fn folder_path(parent: &str, name: &str) -> Result<String> {
    let name = name.trim();
    validate_folder_name(name)?;
    Ok(if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    })
}

/// Every page (`.md` file) at or below `folder`, as workspace paths.
pub fn pages_under(root: &Root, folder: &str) -> Result<Vec<String>> {
    let dir = root.resolve(folder)?;
    let walker = WalkDir::new(&dir)
        .follow_links(false)
        .max_depth(24)
        .into_iter()
        .filter_entry(|e| {
            e.depth() == 0
                || !e.file_type().is_dir()
                || !skip_dir_name(&e.file_name().to_string_lossy())
        });
    let mut out = Vec::new();
    for entry in walker.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if entry.file_type().is_file()
            && name.ends_with(".md")
            && !name.starts_with('.')
            && let Some(rel) = root.relative(entry.path())
        {
            out.push(rel);
        }
    }
    out.sort();
    Ok(out)
}

/// Plain-language error for a rename that the system refused.
pub(crate) fn not_moved(err: io::Error, what: &str) -> CairnError {
    CairnError::Io(format!(
        "{what} couldn't be changed ({err}). A file inside may be open in another program. \
         Close it and try again. Nothing was lost."
    ))
}

pub(crate) fn require_writable_dir(dir: &Path) -> Result<()> {
    if can_write_dir(dir) {
        Ok(())
    } else {
        Err(CairnError::PermissionDenied(
            "You don't have permission to change files in this folder. Ask whoever manages the \
             shared folder for write access."
                .into(),
        ))
    }
}

fn history_path(root: &Root, rel: &str) -> Result<PathBuf> {
    let mut dir = root.system_dir().join("history");
    for part in split_relative(rel)? {
        dir.push(part);
    }
    Ok(dir)
}

/// Move `src` to `dst`, merging into `dst` if it already exists (files
/// already there are kept). Best effort: used for earlier versions.
fn move_merge(src: &Path, dst: &Path) {
    if !src.is_dir() {
        return;
    }
    if !dst.exists() {
        if let Some(parent) = dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::rename(src, dst).is_ok() {
            return;
        }
    }
    let _ = fs::create_dir_all(dst);
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let target = dst.join(entry.file_name());
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                move_merge(&entry.path(), &target);
            } else if !target.exists() {
                let _ = fs::rename(entry.path(), &target);
            }
        }
    }
    let _ = fs::remove_dir(src);
}

fn move_history(root: &Root, from: &str, to: &str) {
    if let (Ok(src), Ok(dst)) = (history_path(root, from), history_path(root, to)) {
        move_merge(&src, &dst);
    }
}

// ------------------------------------------------------------ other pages

/// Update links in `pages` (other than those `exclude` says) that point into
/// the moved place. Pages being edited are skipped and reported.
fn update_links_elsewhere(
    root: &Root,
    me: &Identity,
    pages: &[String],
    map: &PathMap,
    exclude: impl Fn(&str) -> bool,
) -> LinkUpdate {
    let mut out = LinkUpdate::default();
    for p in pages.iter().filter(|p| !exclude(p)) {
        let Ok(path) = root.resolve(p) else { continue };
        let Ok(bytes) = fs::read(&path) else { continue };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        if !may_link_to(&text, map) {
            continue;
        }
        let Some(rw) = rewrite_links(&text, p, p, map) else {
            continue;
        };
        let skip = |reason: String| Skipped {
            path: p.clone(),
            reason,
        };
        if rw.updated == 0 {
            out.skipped.push(skip(
                "Its link is written in a way Cairn couldn't update.".into(),
            ));
            continue;
        }
        match locks::status(root, p, me) {
            Ok(Some(l)) if l.is_mine => {
                out.skipped
                    .push(skip("You have it open in the editor.".into()));
                continue;
            }
            Ok(Some(l)) => {
                out.skipped
                    .push(skip(format!("{} is editing it.", l.info.display_name)));
                continue;
            }
            _ => {}
        }
        let locks = match HeldLocks::take(root, me, std::slice::from_ref(p), "updated") {
            Ok(l) => l,
            Err(e) => {
                out.skipped.push(skip(e.to_string()));
                continue;
            }
        };
        let base = sha256_hex(text.as_bytes());
        let request = PublishRequest {
            root,
            article_rel: p,
            identity: me,
            base_hash: Some(&base),
            content: &rw.text,
            staged: Vec::new(),
        };
        match publish::publish(request, &PublishOptions::default()) {
            Ok(PublishOutcome::Published { .. }) => out.updated.push(p.clone()),
            Ok(PublishOutcome::Conflict { .. }) => {
                out.skipped
                    .push(skip("It was changed at the same moment.".into()));
            }
            Err(e) => out.skipped.push(skip(e.to_string())),
        }
        drop(locks);
    }
    out
}

// ------------------------------------------------------------------ pages

/// Move a page to `to` (another folder, a new file name, or both), and
/// optionally give it a new title. `pages` lists every page, for updating
/// links to it.
pub fn move_page(
    root: &Root,
    me: &Identity,
    pages: &[String],
    from: &str,
    to: &str,
    new_title: Option<&str>,
) -> Result<MoveOutcome> {
    let from = publish::validate_article_path(from)?;
    let to = publish::validate_article_path(to)?;
    let template = crate::templates::is_template_path;
    if template(&from) || template(&to) {
        return Err(CairnError::PathRejected(
            "Templates are managed on the Templates page.".into(),
        ));
    }
    let same_file = from.eq_ignore_ascii_case(&to);
    let src = root.resolve(&from)?;
    let dst = root.resolve_for_create(&to)?;
    if !same_file && dst.exists() {
        return Err(CairnError::Conflict(
            "A page with that name is already in that folder.".into(),
        ));
    }
    if !root.resolve(parent_of(&to))?.is_dir() {
        return Err(CairnError::NotFound(
            "That folder could not be found.".into(),
        ));
    }
    let locks = HeldLocks::take(root, me, &[from.clone(), to.clone()], "renamed or moved")?;
    let old = fs::read(&src)?;
    let old_text = String::from_utf8(old.clone()).map_err(|_| {
        CairnError::BadRequest("This page isn't readable text, so it can't be changed here.".into())
    })?;
    let map = PathMap::Page {
        from: from.clone(),
        to: to.clone(),
    };
    let mut new_text = rewrite_links(&old_text, &from, &to, &map)
        .map(|r| r.text)
        .unwrap_or_else(|| old_text.clone());
    if let Some(title) = new_title {
        new_text = set_title(&new_text, title);
    }
    let moved = vec![Moved {
        from: from.clone(),
        to: to.clone(),
        old_hash: sha256_hex(&old),
        new_hash: sha256_hex(new_text.as_bytes()),
    }];

    if from == to {
        // Only the title changes: an ordinary publish, with an earlier version.
        let base = sha256_hex(&old);
        let request = PublishRequest {
            root,
            article_rel: &from,
            identity: me,
            base_hash: Some(&base),
            content: &new_text,
            staged: Vec::new(),
        };
        if let PublishOutcome::Conflict { .. } =
            publish::publish(request, &PublishOptions::default())?
        {
            return Err(CairnError::Conflict(
                "The page changed at the same moment. Please try again.".into(),
            ));
        }
        return Ok(MoveOutcome {
            path: to,
            map,
            moved,
            links: LinkUpdate::default(),
        });
    }

    for dir in [src.parent(), dst.parent()].into_iter().flatten() {
        require_writable_dir(dir)?;
    }
    relocate_page(root, &from, &to, &src, &dst, &old, &new_text)?;
    drop(locks);
    let links = update_links_elsewhere(root, me, pages, &map, |p| {
        p.eq_ignore_ascii_case(&from) || p.eq_ignore_ascii_case(&to)
    });
    Ok(MoveOutcome {
        path: to,
        map,
        moved,
        links,
    })
}

/// Put the page's (possibly updated) text at `dst`, move its pictures and
/// earlier versions, then remove the old file. Undone if a step fails.
fn relocate_page(
    root: &Root,
    from: &str,
    to: &str,
    src: &Path,
    dst: &Path,
    old: &[u8],
    new_text: &str,
) -> Result<()> {
    let src_assets = root.resolve_for_create(&assets_dir_rel(from))?;
    let dst_assets = root.resolve_for_create(&assets_dir_rel(to))?;
    let has_assets = src_assets.is_dir();
    let case_only = from.eq_ignore_ascii_case(to);
    if has_assets && !case_only && dst_assets.exists() {
        return Err(CairnError::Conflict(
            "A pictures folder with that name is already there. Choose a different title.".into(),
        ));
    }
    if case_only {
        fs::rename(src, dst).map_err(|e| not_moved(e, "The page"))?;
        if has_assets && let Err(e) = fs::rename(&src_assets, &dst_assets) {
            let _ = fs::rename(dst, src);
            return Err(not_moved(e, "The page"));
        }
        if new_text.as_bytes() != old {
            write_atomic(dst, new_text.as_bytes())?;
        }
    } else {
        if !create_new_with(dst, new_text.as_bytes())? {
            return Err(CairnError::Conflict(
                "A page with that name is already in that folder.".into(),
            ));
        }
        if !fs::read(dst).is_ok_and(|b| b == new_text.as_bytes()) {
            let _ = fs::remove_file(dst);
            return Err(CairnError::Io(
                "The page couldn't be written in its new place. Nothing was changed.".into(),
            ));
        }
        if has_assets && let Err(e) = fs::rename(&src_assets, &dst_assets) {
            let _ = fs::remove_file(dst);
            return Err(not_moved(e, "The page's pictures"));
        }
        if let Err(e) = fs::remove_file(src) {
            if has_assets {
                let _ = fs::rename(&dst_assets, &src_assets);
            }
            let _ = fs::remove_file(dst);
            return Err(not_moved(e, "The page"));
        }
    }
    move_history(root, from, to);
    if new_text.as_bytes() != old {
        let _ = history::save_version(root, to, old);
    }
    Ok(())
}

// ---------------------------------------------------------------- folders

/// Rename or move a folder with everything in it. `to` is its new path.
pub fn move_folder(
    root: &Root,
    me: &Identity,
    pages: &[String],
    from: &str,
    to: &str,
) -> Result<MoveOutcome> {
    let from = managed_folder(from)?;
    let to = managed_folder(to)?;
    let case_only = from.eq_ignore_ascii_case(&to);
    if from == to {
        return Err(CairnError::BadRequest(
            "The folder already has that name and place.".into(),
        ));
    }
    let inside_itself = to
        .to_lowercase()
        .starts_with(&format!("{}/", from.to_lowercase()));
    if !case_only && inside_itself {
        return Err(CairnError::BadRequest(
            "A folder can't be moved inside itself.".into(),
        ));
    }
    let src = root.resolve(&from)?;
    if !src.is_dir() {
        return Err(CairnError::NotFound(
            "That folder could not be found.".into(),
        ));
    }
    let dst = root.resolve_for_create(&to)?;
    if !case_only && dst.exists() {
        return Err(CairnError::Conflict(
            "A folder with that name is already there.".into(),
        ));
    }
    let dst_parent = root.resolve(parent_of(&to))?;
    if !dst_parent.is_dir() {
        return Err(CairnError::NotFound(
            "That folder could not be found.".into(),
        ));
    }
    for dir in [src.parent(), Some(dst_parent.as_path())]
        .into_iter()
        .flatten()
    {
        require_writable_dir(dir)?;
    }
    let inside = pages_under(root, &from)?;
    let locks = HeldLocks::take(root, me, &inside, "renamed or moved")?;
    let map = PathMap::Folder {
        from: from.clone(),
        to: to.clone(),
    };

    // Adjust links inside the moved pages, in place, before the rename.
    let mut moved = Vec::new();
    let mut written: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let undo = |written: &[(PathBuf, Vec<u8>)]| {
        for (path, old) in written {
            let _ = write_atomic(path, old);
        }
    };
    for p in &inside {
        let path = root.resolve(p)?;
        let old = fs::read(&path)?;
        let new_rel = map.apply(p).unwrap_or_else(|| p.clone());
        let new_text = std::str::from_utf8(&old)
            .ok()
            .and_then(|t| rewrite_links(t, p, &new_rel, &map))
            .filter(|r| r.updated > 0)
            .map(|r| r.text);
        if let Some(text) = &new_text {
            let step = history::save_version(root, p, &old)
                .and_then(|_| write_atomic(&path, text.as_bytes()).map_err(Into::into));
            if let Err(e) = step {
                undo(&written);
                return Err(e);
            }
            written.push((path, old.clone()));
        }
        moved.push(Moved {
            from: p.clone(),
            to: new_rel,
            old_hash: sha256_hex(&old),
            new_hash: sha256_hex(new_text.as_deref().map_or(&old[..], str::as_bytes)),
        });
    }
    if let Err(e) = fs::rename(&src, &dst) {
        undo(&written);
        return Err(not_moved(e, "The folder"));
    }
    move_history(root, &from, &to);
    drop(locks);
    let prefix = format!("{}/", from.to_lowercase());
    let links = update_links_elsewhere(root, me, pages, &map, |p| {
        p.to_lowercase().starts_with(&prefix)
    });
    Ok(MoveOutcome {
        path: to,
        map,
        moved,
        links,
    })
}
