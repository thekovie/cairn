//! Team template routes. Templates live in `<documentation>/_templates/` and
//! are edited with the normal editor (locks, drafts, publishing, history);
//! these routes only list, create, and delete them.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;

use super::AppState;
use super::api::{ApiResult, blocking, require_writable};
use crate::error::CairnError;
use crate::locks::{self, Acquire};
use crate::templates;

pub async fn list(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        Ok(json!({
            "templates": templates::list(&ws.root)?,
            "can_write": ws.read_only.is_none(),
        }))
    })
    .await
}

pub async fn deleted(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        Ok(json!({ "deleted": templates::deleted(&ws.root)? }))
    })
    .await
}

#[derive(Deserialize)]
pub struct NewTemplateBody {
    name: String,
    #[serde(default)]
    description: String,
    /// Copy from this template id ("builtin:how-to" or "_templates/x.md").
    from: Option<String>,
}

/// Reserve a path and return starting text; the editor then creates it
/// through the usual new-page flow.
pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewTemplateBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let name = body.name.trim();
        if name.is_empty() || name.chars().count() > 80 {
            return Err(CairnError::BadRequest(
                "Please give the template a name (up to 80 characters).".into(),
            ));
        }
        if body.description.chars().count() > 300 {
            return Err(CairnError::BadRequest(
                "Please keep the description under 300 characters.".into(),
            ));
        }
        let source = match body.from.as_deref().filter(|s| !s.is_empty()) {
            Some(id) => Some(templates::load(&ws.root, id)?),
            None => None,
        };
        let path = templates::new_template_path(&ws.root, name)?;
        let content = templates::scaffold(name, &body.description, source.as_deref());
        Ok(json!({ "path": path, "content": content }))
    })
    .await
}

#[derive(Deserialize)]
pub struct DeleteTemplateBody {
    path: String,
    /// Hash of the template as the person last saw it.
    base_hash: String,
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DeleteTemplateBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = crate::publish::validate_article_path(&body.path)?;
        if !templates::is_template_path(&rel) {
            return Err(CairnError::PathRejected(
                "Only templates can be deleted here.".into(),
            ));
        }
        let me = st.identity();
        if let Acquire::HeldBy(v) = locks::acquire(&ws.root, &rel, &me)? {
            return Err(CairnError::Locked(format!(
                "{} is editing this template, so it can't be deleted right now.",
                v.info.display_name
            )));
        }
        let result = crate::publish::delete_article(&ws.root, &rel, &me, &body.base_hash);
        let _ = locks::release(&ws.root, &rel, &me);
        let version = result?;
        Ok(json!({ "deleted": true, "path": rel, "version": version }))
    })
    .await
}
