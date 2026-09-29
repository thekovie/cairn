//! Renaming, moving, and deleting pages and folders, and Recently deleted.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::{Value, json};

use super::api::{ApiResult, blocking, folder_rel, require_writable, slugify_title};
use super::{AppState, OpenWorkspace};
use crate::article::title_from_filename;
use crate::drafts::{Draft, StagedImage};
use crate::error::{CairnError, Result};
use crate::links::rewrite_links;
use crate::manage::{self, MoveOutcome};
use crate::publish::{assets_dir_rel, validate_article_path};
use crate::search::parent_of;
use crate::trash;

fn title_of(ws: &OpenWorkspace, rel: &str) -> String {
    ws.index
        .lock()
        .expect("index lock")
        .get(rel)
        .map(|p| p.title)
        .unwrap_or_else(|| title_from_filename(rel))
}

/// Refuse while this person has an affected page open in the editor here.
fn ensure_not_editing(
    st: &AppState,
    ws: &OpenWorkspace,
    affected: impl Fn(&str) -> bool,
) -> Result<()> {
    let instance = ws.instance_id();
    let open = st
        .edits
        .lock()
        .expect("edits lock")
        .values()
        .find(|s| s.instance_id == instance && affected(&s.article))
        .map(|s| s.article.clone());
    match open {
        Some(rel) => Err(CairnError::Conflict(format!(
            "You have “{}” open in the editor. Publish your changes or close the editor first, \
             then try again.",
            title_of(ws, &rel)
        ))),
        None => Ok(()),
    }
}

fn under(folder: &str) -> impl Fn(&str) -> bool {
    let prefix = format!("{}/", folder.to_lowercase());
    move |rel: &str| rel.to_lowercase().starts_with(&prefix)
}

fn all_pages(ws: &OpenWorkspace) -> Vec<String> {
    ws.refresh_index(true);
    let index = ws.index.lock().expect("index lock");
    index.all().into_iter().map(|p| p.path).collect()
}

/// Move this person's unsaved changes along with the pages they belong to,
/// so they can still be published.
fn relocate_drafts(st: &AppState, ws: &OpenWorkspace, outcome: &MoveOutcome) {
    let drafts = st.drafts();
    for draft in drafts.list(&ws.instance_id()) {
        let Some(new_rel) = outcome.map.apply(&draft.article) else {
            continue;
        };
        if new_rel == draft.article {
            continue;
        }
        let content = rewrite_links(&draft.content, &draft.article, &new_rel, &outcome.map)
            .map_or_else(|| draft.content.clone(), |r| r.text);
        let moved = outcome
            .moved
            .iter()
            .find(|m| m.from.eq_ignore_ascii_case(&draft.article));
        let base_hash = match moved {
            Some(m) if draft.base_hash.as_deref() == Some(m.old_hash.as_str()) => {
                Some(m.new_hash.clone())
            }
            _ => draft.base_hash.clone(),
        };
        let assets = assets_dir_rel(&new_rel);
        let staged = draft
            .staged
            .iter()
            .map(|s| StagedImage {
                target_rel: format!("{assets}/{}", s.name),
                ..s.clone()
            })
            .collect();
        let new = Draft {
            article: new_rel,
            content,
            base_hash,
            staged,
            ..draft.clone()
        };
        let _ = drafts.relocate(&draft, new);
    }
}

fn finish_move(st: &AppState, ws: &OpenWorkspace, outcome: &MoveOutcome) -> Value {
    relocate_drafts(st, ws, outcome);
    ws.refresh_index(true);
    let skipped: Vec<Value> = outcome
        .links
        .skipped
        .iter()
        .map(|s| json!({ "path": s.path, "title": title_of(ws, &s.path), "reason": s.reason }))
        .collect();
    json!({
        "path": outcome.path,
        "links_updated": outcome.links.updated.len(),
        "links_skipped": skipped,
    })
}

/// A free `<stem>.md` (or `<stem>-2.md`, …) in `folder`. `current` (the
/// page being renamed) counts as free.
fn free_page_path(
    st: &AppState,
    ws: &OpenWorkspace,
    folder: &str,
    stem: &str,
    current: &str,
) -> Result<String> {
    let instance = ws.instance_id();
    for n in 1..500 {
        let file = if n == 1 {
            format!("{stem}.md")
        } else {
            format!("{stem}-{n}.md")
        };
        let rel = if folder.is_empty() {
            file
        } else {
            format!("{folder}/{file}")
        };
        let rel = validate_article_path(&rel)?;
        if rel.eq_ignore_ascii_case(current) {
            return Ok(rel);
        }
        let taken = ws.root.resolve_for_create(&rel)?.exists()
            || ws.root.resolve_for_create(&assets_dir_rel(&rel))?.exists()
            || st.drafts().load(&instance, &rel).is_some();
        if !taken {
            return Ok(rel);
        }
    }
    Err(CairnError::Conflict(
        "Couldn't find a free file name for this page.".into(),
    ))
}

// ------------------------------------------------------------------ pages

#[derive(Deserialize)]
pub struct RenamePageBody {
    path: String,
    title: String,
}

pub async fn rename_page(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RenamePageBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        let title = body.title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() || title.chars().count() > 150 {
            return Err(CairnError::BadRequest(
                "Please give the page a title (up to 150 characters).".into(),
            ));
        }
        ensure_not_editing(st, &ws, |a| a.eq_ignore_ascii_case(&rel))?;
        let to = free_page_path(st, &ws, parent_of(&rel), &slugify_title(&title), &rel)?;
        let pages = all_pages(&ws);
        let me = st.identity();
        let outcome = manage::move_page(&ws.root, &me, &pages, &rel, &to, Some(&title))?;
        Ok(finish_move(st, &ws, &outcome))
    })
    .await
}

#[derive(Deserialize)]
pub struct MovePageBody {
    path: String,
    folder: String,
}

pub async fn move_page(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MovePageBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        let folder = folder_rel(&body.folder)?;
        if folder.eq_ignore_ascii_case(parent_of(&rel)) {
            return Err(CairnError::BadRequest(
                "The page is already in that folder.".into(),
            ));
        }
        if !ws.root.resolve(&folder)?.is_dir() {
            return Err(CairnError::NotFound(
                "That folder could not be found.".into(),
            ));
        }
        ensure_not_editing(st, &ws, |a| a.eq_ignore_ascii_case(&rel))?;
        let file = rel.rsplit('/').next().unwrap_or(&rel);
        let stem = &file[..file.len() - 3];
        let to = free_page_path(st, &ws, &folder, stem, "")?;
        let pages = all_pages(&ws);
        let outcome = manage::move_page(&ws.root, &st.identity(), &pages, &rel, &to, None)?;
        Ok(finish_move(st, &ws, &outcome))
    })
    .await
}

#[derive(Deserialize)]
pub struct DeletePageBody {
    path: String,
    /// The page as the person last saw it; it isn't deleted if it changed.
    hash: Option<String>,
}

pub async fn delete_page(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DeletePageBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        ensure_not_editing(st, &ws, |a| a.eq_ignore_ascii_case(&rel))?;
        let title = title_of(&ws, &rel);
        let me = st.identity();
        let item = trash::trash_page(&ws.root, &me, &rel, &title, body.hash.as_deref())?;
        ws.refresh_index(true);
        Ok(json!({ "item": item }))
    })
    .await
}

// ---------------------------------------------------------------- folders

#[derive(Deserialize)]
pub struct RenameFolderBody {
    path: String,
    name: String,
}

pub async fn rename_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RenameFolderBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let from = manage::managed_folder(&body.path)?;
        let to = manage::folder_path(parent_of(&from), &body.name)?;
        ensure_not_editing(st, &ws, under(&from))?;
        let pages = all_pages(&ws);
        let outcome = manage::move_folder(&ws.root, &st.identity(), &pages, &from, &to)?;
        Ok(finish_move(st, &ws, &outcome))
    })
    .await
}

#[derive(Deserialize)]
pub struct MoveFolderBody {
    path: String,
    /// The folder to move it into ("" for the top level).
    parent: String,
}

pub async fn move_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<MoveFolderBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let from = manage::managed_folder(&body.path)?;
        let parent = folder_rel(&body.parent)?;
        if parent.eq_ignore_ascii_case(parent_of(&from)) {
            return Err(CairnError::BadRequest(
                "The folder is already there.".into(),
            ));
        }
        let name = from.rsplit('/').next().unwrap_or(&from);
        let to = manage::folder_path(&parent, name)?;
        ensure_not_editing(st, &ws, under(&from))?;
        let pages = all_pages(&ws);
        let outcome = manage::move_folder(&ws.root, &st.identity(), &pages, &from, &to)?;
        Ok(finish_move(st, &ws, &outcome))
    })
    .await
}

#[derive(Deserialize)]
pub struct FolderBody {
    path: String,
}

pub async fn delete_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FolderBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = manage::managed_folder(&body.path)?;
        ensure_not_editing(st, &ws, under(&rel))?;
        let item = trash::trash_folder(&ws.root, &st.identity(), &rel)?;
        ws.refresh_index(true);
        Ok(json!({ "item": item }))
    })
    .await
}

// -------------------------------------------------------- recently deleted

pub async fn trash_list(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        Ok(json!({
            "items": trash::list(&ws.root),
            "can_restore": ws.read_only.is_none(),
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct RestoreBody {
    id: String,
}

pub async fn trash_restore(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RestoreBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let item = trash::restore(&ws.root, &st.identity(), &body.id)?;
        ws.refresh_index(true);
        Ok(json!({ "item": item }))
    })
    .await
}
