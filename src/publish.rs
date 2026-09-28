//! Safe publishing.
//!
//! 1. Validate the lock, permissions, document, and pictures.
//! 2. Re-hash the live article; if it changed since editing began, stop and
//!    report a conflict (the draft is untouched).
//! 3. Write every new picture completely *before* any Markdown refers to it.
//! 4. Save the current published article to version history.
//! 5. Write the new Markdown to a temporary file in the same folder.
//! 6. Replace the published file by rename (never truncate in place).
//! 7. Read back and verify the article and its new pictures.
//! 8. On failure, keep (or restore) the previous article and remove only the
//!    pictures this attempt created. Pictures are never removed while an
//!    article might refer to them.

use std::fs;
use std::path::PathBuf;

use serde::Serialize;

use crate::article::{encode_path, parse_front_matter, referenced_local_targets};
use crate::error::{CairnError, Result};
use crate::fsutil::{
    can_write_dir, create_new_with, read_optional, sha256_hex, write_atomic, write_temp_beside,
};
use crate::history;
use crate::images::{ImageLimits, validate_image};
use crate::locks::{Identity, verify_held};
use crate::paths::{Root, is_system_path, normalize_relative};

pub const MAX_ARTICLE_BYTES: usize = 5 * 1024 * 1024;

/// Test hook: make publishing fail at a given step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailPoint {
    AfterAssets,
    AfterHistory,
    BeforeReplace,
    AfterReplace,
}

#[derive(Debug, Clone, Default)]
pub struct PublishOptions {
    pub image_limits: ImageLimits,
    pub fail_at: Option<FailPoint>,
}

pub struct PublishRequest<'a> {
    pub root: &'a Root,
    pub article_rel: &'a str,
    pub identity: &'a Identity,
    /// Hash of the published article when editing began; `None` for a new page.
    pub base_hash: Option<&'a str>,
    pub content: &'a str,
    /// Staged pictures: (workspace-relative target, bytes).
    pub staged: Vec<(String, Vec<u8>)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum PublishOutcome {
    Published {
        hash: String,
        new_assets: Vec<String>,
        history_id: Option<String>,
    },
    Conflict {
        current_content: Option<String>,
        current_hash: Option<String>,
    },
}

/// Normalize and check a page path: `.md`, not in `_system`, not inside an
/// `.assets` folder, no hidden segments.
pub fn validate_article_path(rel: &str) -> Result<String> {
    let rel = normalize_relative(rel)?;
    if rel.is_empty() || !rel.to_ascii_lowercase().ends_with(".md") {
        return Err(CairnError::PathRejected(
            "Pages must be Markdown files ending in .md.".into(),
        ));
    }
    if is_system_path(&rel) {
        return Err(CairnError::PathRejected(
            "The _system folder is reserved for Cairn.".into(),
        ));
    }
    let parts: Vec<&str> = rel.split('/').collect();
    let (folders, _) = parts.split_at(parts.len() - 1);
    if parts.iter().any(|p| p.starts_with('.'))
        || folders
            .iter()
            .any(|p| p.to_ascii_lowercase().ends_with(".assets"))
    {
        return Err(CairnError::PathRejected(
            "That isn't a valid place for a page.".into(),
        ));
    }
    Ok(rel)
}

/// `Guides/new-user-setup.md` → `Guides/new-user-setup.assets`.
pub fn assets_dir_rel(article_rel: &str) -> String {
    let stem = if article_rel.to_ascii_lowercase().ends_with(".md") {
        &article_rel[..article_rel.len() - 3]
    } else {
        article_rel
    };
    format!("{stem}.assets")
}

/// Markdown-ready relative link from the article to one of its pictures:
/// `new-user-setup.assets/screenshot-a1b2c3d4.png` (percent-encoded).
pub fn image_link_for(article_rel: &str, file_name: &str) -> String {
    let assets = assets_dir_rel(article_rel);
    let folder = assets.rsplit('/').next().unwrap_or(&assets);
    encode_path(&format!("{folder}/{file_name}"))
}

pub fn current_hash(root: &Root, article_rel: &str) -> Result<Option<String>> {
    let path = root.resolve_for_create(article_rel)?;
    Ok(read_optional(&path)?.map(|b| sha256_hex(&b)))
}

fn remove_created(created: &[(PathBuf, usize)]) {
    for (p, _) in created {
        let _ = fs::remove_file(p);
    }
}

fn unchanged_msg(what: &str) -> CairnError {
    CairnError::Io(format!(
        "{what} The published page was not changed and your text is kept."
    ))
}

/// Write one staged picture. Returns true if this call created the file.
fn write_picture(path: &std::path::Path, bytes: &[u8]) -> Result<bool> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let same = |existing: Option<Vec<u8>>| match existing {
        Some(e) if e == bytes => Ok(false),
        Some(_) => Err(CairnError::Conflict(
            "A different picture already uses this file name.".into(),
        )),
        None => Err(CairnError::Io("The picture could not be written.".into())),
    };
    if let Some(existing) = read_optional(path)? {
        return same(Some(existing));
    }
    if create_new_with(path, bytes)? {
        Ok(true)
    } else {
        same(read_optional(path)?)
    }
}

pub fn publish(req: PublishRequest, opts: &PublishOptions) -> Result<PublishOutcome> {
    let root = req.root;
    let rel = validate_article_path(req.article_rel)?;
    let injected = |at: FailPoint| opts.fail_at == Some(at);

    // 1. Validate.
    verify_held(root, &rel, req.identity)?;
    if req.content.len() > MAX_ARTICLE_BYTES {
        return Err(CairnError::BadRequest(
            "This page is too long to publish (over 5 MB).".into(),
        ));
    }
    let article_path = root.resolve_for_create(&rel)?;
    let parent = article_path
        .parent()
        .ok_or_else(|| CairnError::PathRejected("Invalid page path.".into()))?
        .to_path_buf();
    if !parent.exists() {
        fs::create_dir_all(&parent)?;
    }
    if !can_write_dir(&parent) {
        return Err(CairnError::PermissionDenied(
            "You can read this page, but you don't have permission to change files in this \
             folder. Ask whoever manages the shared folder for write access. Your text is kept."
                .into(),
        ));
    }

    let (_, body) = parse_front_matter(req.content);
    let referenced: Vec<String> = referenced_local_targets(&rel, body)
        .into_iter()
        .map(|r| r.to_lowercase())
        .collect();
    let assets_prefix = format!("{}/", assets_dir_rel(&rel)).to_lowercase();
    let mut to_write: Vec<(String, Vec<u8>)> = Vec::new();
    for (target, bytes) in req.staged {
        let target = normalize_relative(&target)?;
        let lower = target.to_lowercase();
        if !referenced.contains(&lower) {
            continue; // removed from the text before publishing: never written
        }
        let rest = lower.strip_prefix(&assets_prefix).unwrap_or("");
        if rest.is_empty() || rest.contains('/') {
            return Err(CairnError::PathRejected(
                "A picture was staged for a different page.".into(),
            ));
        }
        validate_image(&bytes, Some(&target), &opts.image_limits)?;
        to_write.push((target, bytes));
    }

    // 2. Conflict check against what is on disk right now.
    let current = read_optional(&article_path)?;
    let current_hash = current.as_ref().map(|b| sha256_hex(b));
    let unchanged = match (req.base_hash, current_hash.as_deref()) {
        (Some(base), Some(now)) => base == now,
        (None, None) => true,
        _ => false,
    };
    if !unchanged {
        return Ok(PublishOutcome::Conflict {
            current_content: current.map(|b| String::from_utf8_lossy(&b).into_owned()),
            current_hash,
        });
    }

    // 3. New pictures first. `created` records (path, index into to_write).
    let mut created: Vec<(PathBuf, usize)> = Vec::new();
    for (i, (target, bytes)) in to_write.iter().enumerate() {
        let result = root
            .resolve_for_create(target)
            .and_then(|p| Ok((write_picture(&p, bytes)?, p)));
        match result {
            Ok((true, path)) => created.push((path, i)),
            Ok((false, _)) => {}
            Err(e) => {
                remove_created(&created);
                return Err(e);
            }
        }
    }
    if injected(FailPoint::AfterAssets) {
        remove_created(&created);
        return Err(unchanged_msg("Publishing failed while saving pictures."));
    }

    // 4. Version history.
    let history_id = match &current {
        Some(prev) => match history::save_version(root, &rel, prev) {
            Ok(id) => Some(id),
            Err(e) => {
                remove_created(&created);
                return Err(e);
            }
        },
        None => None,
    };
    if injected(FailPoint::AfterHistory) {
        remove_created(&created);
        return Err(unchanged_msg(
            "Publishing failed while saving the earlier version.",
        ));
    }

    // 5. Temporary file beside the article.
    let temp = match write_temp_beside(&article_path, req.content.as_bytes()) {
        Ok(t) => t,
        Err(e) => {
            remove_created(&created);
            return Err(e.into());
        }
    };
    if injected(FailPoint::BeforeReplace) {
        let _ = fs::remove_file(&temp);
        remove_created(&created);
        return Err(unchanged_msg(
            "Publishing failed before the page was replaced.",
        ));
    }

    // 6. Replace.
    if let Err(e) = fs::rename(&temp, &article_path) {
        let _ = fs::remove_file(&temp);
        remove_created(&created);
        return Err(e.into());
    }

    // 7. Verify.
    let article_ok = fs::read(&article_path).is_ok_and(|b| b == req.content.as_bytes());
    let pictures_ok = created
        .iter()
        .all(|(p, i)| fs::read(p).is_ok_and(|b| b == to_write[*i].1));
    if injected(FailPoint::AfterReplace) || !article_ok || !pictures_ok {
        // 8. Roll back to the previous article; only then remove new pictures.
        let rolled_back = match &current {
            Some(prev) => {
                write_atomic(&article_path, prev).is_ok()
                    && fs::read(&article_path).is_ok_and(|b| b == *prev)
            }
            None => fs::remove_file(&article_path).is_ok(),
        };
        if rolled_back {
            remove_created(&created);
            return Err(CairnError::Io(
                "The page could not be verified after publishing, so the previous version was \
                 put back. Your text is kept."
                    .into(),
            ));
        }
        return Err(CairnError::Io(
            "The page could not be verified after publishing and the previous version could not \
             be put back automatically. Your text is kept. Check the page and its earlier \
             versions, or ask a maintainer."
                .into(),
        ));
    }

    let new_assets = created
        .iter()
        .map(|(_, i)| to_write[*i].0.clone())
        .collect();
    Ok(PublishOutcome::Published {
        hash: sha256_hex(req.content.as_bytes()),
        new_assets,
        history_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_path_rules() {
        assert_eq!(
            validate_article_path("Guides\\a.md").unwrap(),
            "Guides/a.md"
        );
        assert!(validate_article_path("_system/x.md").is_err());
        assert!(validate_article_path("Guides/a.assets/b.md").is_err());
        assert!(validate_article_path("Guides/a.txt").is_err());
        assert!(validate_article_path("../a.md").is_err());
    }

    #[test]
    fn links_and_asset_dirs() {
        assert_eq!(
            assets_dir_rel("Guides/new-user-setup.md"),
            "Guides/new-user-setup.assets"
        );
        assert_eq!(
            image_link_for("Guides/My Page.md", "s-1.png"),
            "My%20Page.assets/s-1.png"
        );
    }
}
