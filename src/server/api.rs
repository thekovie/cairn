//! JSON API handlers. Filesystem work runs on the blocking thread pool so a
//! slow network share never stalls the server.

use std::collections::HashMap;
use std::path::{Path as FsPath, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use super::idle;
use super::{ApiError, AppState, EditSession, OpenWorkspace, draft_store_for};
use crate::article::{
    self, RenderContext, encode_path, extract_title, parse_front_matter, title_from_filename,
};
use crate::config;
use crate::drafts::{Draft, StagedImage};
use crate::editors::{self, LastEdit};
use crate::error::{CairnError, Result};
use crate::fsutil::{can_write_dir, read_optional, sha256_hex};
use crate::history;
use crate::images::{asset_file_name, sniff_mime, validate_image};
use crate::locks::{self, Acquire};
use crate::paths::{Root, display_path, is_system_path, normalize_relative, validate_name};
use crate::publish::{
    self, PublishOptions, PublishOutcome, PublishRequest, assets_dir_rel, image_link_for,
    validate_article_path,
};
use crate::search::{list_folder, parent_of};
use crate::workspace::{self, Discovery, InitTarget};

pub(super) type ApiResult = std::result::Result<Json<Value>, ApiError>;

pub(super) async fn blocking<F>(state: Arc<AppState>, f: F) -> ApiResult
where
    F: FnOnce(&AppState) -> Result<Value> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&state))
        .await
        .map_err(|_| {
            ApiError(CairnError::Io(
                "Something went wrong inside Cairn. Please try again.".into(),
            ))
        })?
        .map(Json)
        .map_err(ApiError)
}

fn breadcrumbs(folder: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut acc = String::new();
    for part in folder.split('/').filter(|p| !p.is_empty()) {
        if !acc.is_empty() {
            acc.push('/');
        }
        acc.push_str(part);
        out.push(json!({ "name": part, "path": acc }));
    }
    out
}

fn line_diff(old: &str, new: &str) -> Vec<Value> {
    use similar::{ChangeTag, TextDiff};
    TextDiff::from_lines(old, new)
        .iter_all_changes()
        .take(5000)
        .map(|c| {
            let kind = match c.tag() {
                ChangeTag::Equal => "same",
                ChangeTag::Delete => "removed",
                ChangeTag::Insert => "added",
            };
            json!({ "kind": kind, "text": c.value().trim_end_matches(['\r', '\n']) })
        })
        .collect()
}

pub(super) fn require_writable(ws: &OpenWorkspace) -> Result<()> {
    match &ws.read_only {
        Some(reason) => Err(CairnError::ReadOnly(reason.clone())),
        None => Ok(()),
    }
}

fn render_ctx<'a>(
    st: &'a AppState,
    root: &'a Root,
    rel: &'a str,
    staged: &'a HashMap<String, String>,
) -> RenderContext<'a> {
    RenderContext {
        root,
        article_rel: rel,
        read_key: &st.read_key,
        staged,
    }
}

fn workspace_view(state: &AppState, ws: &OpenWorkspace) -> Value {
    let marker = ws.marker.read().expect("marker lock").clone();
    json!({
        "name": marker.display_name,
        "root": ws.root.display(),
        "instance_id": marker.instance_id,
        "schema_version": marker.schema_version,
        "read_only": ws.read_only,
        "storage": ws.storage,
        "drafts_persistent": state.drafts().is_persistent(),
    })
}

fn state_view(state: &AppState) -> Value {
    let cfg = state.config();
    let me = state.identity();
    let ws = state.workspace.read().expect("workspace lock").clone();
    json!({
        "version": env!("CARGO_PKG_VERSION"),
        "workspace": ws.as_ref().map(|w| workspace_view(state, w)),
        "user": { "display_name": me.display_name, "os_user": me.os_user, "host": me.host },
        "config": {
            "display_name": cfg.display_name,
            "idle_warning_minutes": cfg.idle_warning_minutes,
            "idle_release_minutes": cfg.idle_release_minutes,
            "persistent_drafts": cfg.persistent_drafts,
            "appearance": cfg.appearance,
            "text_size": cfg.text_size,
            "max_image_mb": cfg.max_image_mb,
            "last_workspace": cfg.last_workspace.as_ref().map(|p| p.display().to_string()),
            "timezone": cfg.timezone,
            "system_timezone": crate::timefmt::zone_name(&crate::timefmt::user_zone(None)),
            "pdf_paper": cfg.pdf_paper,
            "toolbar_labels": cfg.toolbar_labels,
            "update_check": cfg.update_check,
            "pdf_available":
                !crate::export::pdf::find_browsers(cfg.pdf_browser.as_deref()).is_empty(),
        },
        "drafts_persistent": state.drafts().is_persistent(),
        "config_warning": state.config_warning,
    })
}

/// Open the workspace whose marker is exactly at `root`.
pub fn open_at(state: &AppState, root: &FsPath) -> Result<Arc<OpenWorkspace>> {
    let (marker, read_only_reason) = match workspace::discover(root)? {
        Discovery::Found {
            root: found,
            marker,
            read_only_reason,
        } => {
            let same = std::fs::canonicalize(&found).ok() == std::fs::canonicalize(root).ok();
            if !same {
                return Err(CairnError::BadRequest(format!(
                    "That folder is inside the documentation folder at {found}. Open that one \
                     instead."
                )));
            }
            (marker, read_only_reason)
        }
        Discovery::Invalid { reason, .. } => return Err(CairnError::BadRequest(reason)),
        Discovery::NotFound { .. } => {
            return Err(CairnError::NotFound(
                "That folder is not a documentation folder.".into(),
            ));
        }
    };
    let root = Root::new(root)?;
    let storage = if marker.is_supported() {
        workspace::probe_storage(root.path())
    } else {
        Default::default()
    };
    let read_only = read_only_reason.or_else(|| {
        (!storage.writable).then(|| {
            "You can read this documentation but you don't have permission to change it."
                .to_string()
        })
    });
    let ws = Arc::new(OpenWorkspace {
        root,
        marker: RwLock::new(marker),
        read_only,
        storage,
        index: Mutex::new(Default::default()),
        last_refresh: Mutex::new(None),
    });
    ws.refresh_index(true);

    state.release_all_locks();
    state.denied_dirs.lock().expect("denied lock").clear();
    *state.workspace.write().expect("workspace lock") = Some(ws.clone());

    let mut cfg = state.config();
    cfg.last_workspace = Some(PathBuf::from(ws.root.display()));
    let _ = config::save(&state.config_dir, &cfg);
    *state.config.write().expect("config lock") = cfg;
    Ok(ws)
}

// -------------------------------------------------------------- workspace

pub async fn get_state(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| Ok(state_view(st))).await
}

pub async fn pick_folder(State(_state): State<Arc<AppState>>) -> ApiResult {
    let picked = tokio::task::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a documentation folder")
            .pick_folder()
    })
    .await
    .ok()
    .flatten();
    Ok(Json(json!({ "path": picked.map(|p| display_path(&p)) })))
}

#[derive(Deserialize)]
pub struct PathBody {
    path: String,
}

pub async fn discover(State(state): State<Arc<AppState>>, Json(body): Json<PathBody>) -> ApiResult {
    blocking(state, move |_| {
        let path = body.path.trim();
        if path.is_empty() {
            return Err(CairnError::BadRequest("Please choose a folder.".into()));
        }
        Ok(serde_json::to_value(workspace::discover(FsPath::new(path))?).expect("serializes"))
    })
    .await
}

#[derive(Deserialize)]
pub struct RootBody {
    root: String,
}

pub async fn open_workspace(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RootBody>,
) -> ApiResult {
    blocking(state, move |st| {
        open_at(st, FsPath::new(body.root.trim()))?;
        Ok(state_view(st))
    })
    .await
}

#[derive(Deserialize)]
pub struct InitBody {
    path: String,
    /// "here" or "child"
    mode: String,
    child_name: Option<String>,
    display_name: String,
}

pub async fn init_workspace(
    State(state): State<Arc<AppState>>,
    Json(body): Json<InitBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let path = PathBuf::from(body.path.trim());
        let target = match body.mode.as_str() {
            "here" => InitTarget::Here(path),
            "child" => InitTarget::NewChild {
                parent: path,
                name: body.child_name.unwrap_or_default().trim().to_string(),
            },
            _ => return Err(CairnError::BadRequest("Unknown setup choice.".into())),
        };
        let outcome = workspace::initialize(&target, &body.display_name)?;
        open_at(st, FsPath::new(&outcome.root))?;
        Ok(json!({ "created": outcome.created, "root": outcome.root, "state": state_view(st) }))
    })
    .await
}

#[derive(Deserialize)]
pub struct NameBody {
    display_name: String,
}

pub async fn rename_workspace(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NameBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let marker = workspace::rename_workspace(ws.root.path(), &body.display_name)?;
        *ws.marker.write().expect("marker lock") = marker;
        Ok(state_view(st))
    })
    .await
}

pub async fn close_workspace(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        st.release_all_locks();
        *st.workspace.write().expect("workspace lock") = None;
        let mut cfg = st.config();
        cfg.last_workspace = None;
        let _ = config::save(&st.config_dir, &cfg);
        *st.config.write().expect("config lock") = cfg;
        Ok(state_view(st))
    })
    .await
}

// ------------------------------------------------------------ navigation

pub async fn home(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        ws.refresh_index(false);
        let index = ws.index.lock().expect("index lock");
        let listing = list_folder(&ws.root, &index, "")?;
        Ok(json!({
            "name": ws.marker.read().expect("marker lock").display_name,
            "categories": listing.folders,
            "root_pages": listing.pages,
            "recent": index.recent(10),
            "total_pages": index.len(),
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: String,
}

pub(super) fn folder_rel(raw: &str) -> Result<String> {
    let rel = normalize_relative(raw)?;
    if is_system_path(&rel) {
        return Err(CairnError::PathRejected(
            "The _system folder is reserved for Cairn.".into(),
        ));
    }
    Ok(rel)
}

fn folder_denied(st: &AppState, folder: &str) -> bool {
    st.denied_dirs
        .lock()
        .expect("denied lock")
        .contains(&folder.to_lowercase())
}

pub async fn folder(State(state): State<Arc<AppState>>, Query(q): Query<PathQuery>) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = folder_rel(&q.path)?;
        ws.refresh_index(false);
        let index = ws.index.lock().expect("index lock");
        let listing = list_folder(&ws.root, &index, &rel)?;
        let name = rel.rsplit('/').next().unwrap_or("").to_string();
        Ok(json!({
            "path": rel,
            "name": name,
            "breadcrumbs": breadcrumbs(&rel),
            "folders": listing.folders,
            "pages": listing.pages,
            "can_write": ws.read_only.is_none() && !folder_denied(st, &rel),
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct CreateFolderBody {
    parent: String,
    name: String,
}

pub async fn create_folder(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateFolderBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let parent = folder_rel(&body.parent)?;
        let name = body.name.trim();
        validate_name(name)?;
        if name.starts_with('.') || name.to_ascii_lowercase().ends_with(".assets") {
            return Err(CairnError::BadRequest(
                "Please choose a different folder name.".into(),
            ));
        }
        let rel = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        };
        if is_system_path(&rel) {
            return Err(CairnError::BadRequest(
                "That name is reserved for Cairn.".into(),
            ));
        }
        let path = ws.root.resolve_for_create(&rel)?;
        if path.exists() {
            return Err(CairnError::Conflict(
                "A folder with that name already exists.".into(),
            ));
        }
        std::fs::create_dir(&path)?;
        Ok(json!({ "path": rel }))
    })
    .await
}

pub async fn all_pages(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        ws.refresh_index(false);
        let pages = ws.index.lock().expect("index lock").all();
        Ok(json!({ "pages": pages }))
    })
    .await
}

pub async fn all_folders(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        Ok(json!({ "folders": crate::search::all_folders(&ws.root) }))
    })
    .await
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let query = q.get("q").map(|s| s.trim().to_string()).unwrap_or_default();
        ws.refresh_index(false);
        let index = ws.index.lock().expect("index lock");
        let hits = index.search(&query, 50);
        let suggestion = if hits.is_empty() {
            index.suggest(&query)
        } else {
            None
        };
        Ok(json!({ "query": query, "results": hits, "suggestion": suggestion }))
    })
    .await
}

// ------------------------------------------------------------------ pages

fn cannot_edit_reason(
    st: &AppState,
    ws: &OpenWorkspace,
    rel: &str,
    path: &FsPath,
) -> Option<String> {
    if let Some(r) = &ws.read_only {
        return Some(r.clone());
    }
    if folder_denied(st, parent_of(rel)) {
        return Some("You can read this page but you don't have permission to change it.".into());
    }
    if std::fs::metadata(path).is_ok_and(|m| m.permissions().readonly()) {
        return Some("This page's file is marked read-only, so it can't be changed.".into());
    }
    None
}

pub async fn page(State(state): State<Arc<AppState>>, Query(q): Query<PathQuery>) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&q.path)?;
        let path = ws.root.resolve(&rel)?;
        let bytes = std::fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        let (meta, body) = parse_front_matter(&text);
        let title = extract_title(body).unwrap_or_else(|| title_from_filename(&rel));
        let staged = HashMap::new();
        let rendered = article::render(body, &render_ctx(st, &ws.root, &rel, &staged));
        let lock = locks::status(&ws.root, &rel, &st.identity())?;
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let instance = ws.instance_id();
        let key = AppState::session_key(&instance, &rel);
        let editing_here = st.edits.lock().expect("edits lock").contains_key(&key);
        let (edited, edited_outside) = match editors::last_edit(&ws.root, &rel, &bytes) {
            LastEdit::By(rec) => (Some(json!({ "by": rec.by, "at": rec.at })), false),
            LastEdit::Outside => (None, true),
            LastEdit::Unknown => (None, false),
        };
        Ok(json!({
            "edited": edited,
            "edited_outside": edited_outside,
            "path": rel,
            "title": title,
            "meta": meta,
            "html": rendered.html,
            "toc": rendered.toc,
            "broken_links": rendered.broken_links,
            "modified": modified,
            "hash": sha256_hex(&bytes),
            "lock": lock,
            "cannot_edit_reason": cannot_edit_reason(st, &ws, &rel, &path),
            "has_draft": st
                .drafts()
                .load(&instance, &rel)
                .is_some_and(|d| draft_has_changes(&d, Some(&text))),
            "editing_here": editing_here,
            "folder": parent_of(&rel),
            "breadcrumbs": breadcrumbs(parent_of(&rel)),
            "storage_reliable": ws.storage.reliable_locking,
        }))
    })
    .await
}

pub(super) fn slugify_title(title: &str) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for c in title.chars().flat_map(char::to_lowercase) {
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
    if slug.is_empty() { "page".into() } else { slug }
}

/// Today's date in the timezone this person chose (or this computer's).
pub(super) fn today_for(st: &AppState) -> String {
    let cfg = st.config();
    crate::timefmt::today(&crate::timefmt::user_zone(cfg.timezone.as_deref()))
}

/// Text for a new page from a template id: "builtin:<key>", a bare
/// built-in key (older clients), or "_templates/<file>.md".
fn page_from_template(
    st: &AppState,
    root: &Root,
    id: &str,
    title: &str,
    folder: &str,
) -> Result<String> {
    let id = if id.contains(':') || id.contains('/') {
        id.to_string()
    } else {
        format!("builtin:{id}")
    };
    let raw = crate::templates::load(root, &id)?;
    let author = st.identity().display_name;
    let date = today_for(st);
    let vars = crate::templates::Vars {
        title,
        date: &date,
        author: &author,
        folder,
    };
    Ok(crate::templates::instantiate(&raw, &vars))
}

#[derive(Deserialize)]
pub struct NewPageBody {
    folder: String,
    title: String,
    template: Option<String>,
}

pub async fn new_page(
    State(state): State<Arc<AppState>>,
    Json(body): Json<NewPageBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let folder = folder_rel(&body.folder)?;
        let title = body.title.trim();
        if title.is_empty() || title.chars().count() > 150 {
            return Err(CairnError::BadRequest(
                "Please give the page a title (up to 150 characters).".into(),
            ));
        }
        if !ws.root.resolve(&folder)?.is_dir() {
            return Err(CairnError::NotFound(
                "That folder could not be found.".into(),
            ));
        }
        let slug = slugify_title(title);
        let me = st.identity();
        for n in 1..500 {
            let file = if n == 1 {
                format!("{slug}.md")
            } else {
                format!("{slug}-{n}.md")
            };
            let rel = if folder.is_empty() {
                file
            } else {
                format!("{folder}/{file}")
            };
            let rel = validate_article_path(&rel)?;
            if ws.root.resolve_for_create(&rel)?.exists() {
                continue;
            }
            if locks::status(&ws.root, &rel, &me)?.is_some_and(|l| !l.is_mine) {
                continue;
            }
            // An unpublished new page with this name is waiting in the drafts.
            if st.drafts().load(&ws.instance_id(), &rel).is_some() {
                continue;
            }
            let id = body.template.as_deref().unwrap_or("builtin:blank");
            let content = page_from_template(st, &ws.root, id, title, &folder)?;
            return Ok(json!({ "path": rel, "content": content }));
        }
        Err(CairnError::Conflict(
            "Couldn't find a free file name for this page.".into(),
        ))
    })
    .await
}

// ---------------------------------------------------------------- editing

#[derive(Deserialize)]
pub struct EditStartBody {
    path: String,
    #[serde(default)]
    is_new: bool,
    initial_content: Option<String>,
}

fn check_write_permission(st: &AppState, ws: &OpenWorkspace, rel: &str) -> Result<()> {
    let folder = parent_of(rel);
    let mut dir = ws.root.resolve_for_create(folder)?;
    while !dir.exists() {
        match dir.parent() {
            Some(p) => dir = p.to_path_buf(),
            None => break,
        }
    }
    let mut denied = st.denied_dirs.lock().expect("denied lock");
    if can_write_dir(&dir) {
        denied.remove(&folder.to_lowercase());
        Ok(())
    } else {
        denied.insert(folder.to_lowercase());
        Err(CairnError::PermissionDenied(
            "You can read this page, but you don't have permission to change files in this \
             folder. Ask whoever manages the shared folder for write access."
                .into(),
        ))
    }
}

fn idle_config(st: &AppState) -> (Duration, Duration) {
    let cfg = st.config();
    (
        Duration::from_secs(cfg.idle_warning_minutes * 60),
        Duration::from_secs(cfg.idle_release_minutes * 60),
    )
}

pub async fn edit_start(
    State(state): State<Arc<AppState>>,
    Json(body): Json<EditStartBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        check_write_permission(st, &ws, &rel)?;
        let current = read_optional(&ws.root.resolve_for_create(&rel)?)?;
        let is_new = current.is_none();
        if body.is_new && !is_new {
            return Err(CairnError::Conflict(
                "A page with this name already exists. Please choose another title.".into(),
            ));
        }
        if !body.is_new && is_new {
            return Err(CairnError::NotFound(
                "This page no longer exists. It may have been moved or deleted.".into(),
            ));
        }
        let me = st.identity();
        if let Acquire::HeldBy(view) = locks::acquire(&ws.root, &rel, &me)? {
            return Ok(json!({ "status": "locked", "lock": view }));
        }
        let current_text = current
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        let current_hash = current.as_ref().map(|b| sha256_hex(b));
        let instance = ws.instance_id();
        let key = AppState::session_key(&instance, &rel);
        let drafts = st.drafts();
        let resumed = st.edits.lock().expect("edits lock").contains_key(&key);
        let published = current_text.clone().unwrap_or_default();

        let mut restore = Value::Null;
        let draft = match drafts.load(&instance, &rel) {
            Some(d) if resumed => d,
            // A new page that was never published: there is nothing to
            // choose between, so just carry on with it.
            Some(d) if current.is_none() => d,
            Some(d) if d.content != published || !d.staged.is_empty() => {
                restore = json!({
                    "updated_at": d.updated_at,
                    "published_changed": d.base_hash != current_hash,
                    "staged_pictures": d.staged.len(),
                    "diff": line_diff(&published, &d.content),
                });
                d
            }
            _ => {
                let d = Draft {
                    article: rel.clone(),
                    instance_id: instance.clone(),
                    base_hash: current_hash.clone(),
                    content: current_text
                        .clone()
                        .or(body.initial_content)
                        .unwrap_or_default(),
                    updated_at: locks::now_secs(),
                    is_new,
                    staged: Vec::new(),
                };
                drafts.save(&d);
                d
            }
        };
        let session = EditSession {
            article: rel.clone(),
            instance_id: instance,
            lock_held: true,
            last_activity: Instant::now(),
            released_reason: None,
        };
        st.edits.lock().expect("edits lock").insert(key, session);
        let cfg = st.config();
        Ok(json!({
            "status": "editing",
            "path": rel,
            "content": draft.content,
            "is_new": draft.is_new,
            "published_content": current_text,
            "published_hash": current_hash,
            "published_changed_since_start": draft.base_hash != current_hash,
            "restore": restore,
            "resumed": resumed,
            "persistent": drafts.is_persistent(),
            "idle_warning_minutes": cfg.idle_warning_minutes,
            "idle_release_minutes": cfg.idle_release_minutes,
            "storage_reliable": ws.storage.reliable_locking,
            // For the visual editor, which shows pictures itself.
            "read_key": st.read_key,
            "staged": draft.staged,
        }))
    })
    .await
}

/// Whether a draft holds work that isn't already published. Opening the
/// editor saves a copy of the published page, which is not unsaved work.
fn draft_has_changes(draft: &Draft, published: Option<&str>) -> bool {
    !draft.staged.is_empty() || published != Some(draft.content.as_str())
}

fn with_session<T>(st: &AppState, rel: &str, f: impl FnOnce(&mut EditSession) -> T) -> Result<T> {
    let ws = st.workspace()?;
    let key = AppState::session_key(&ws.instance_id(), rel);
    let mut edits = st.edits.lock().expect("edits lock");
    let session = edits.get_mut(&key).ok_or_else(|| {
        CairnError::BadRequest(
            "This page isn't open for editing. Choose \"Edit this page\" first.".into(),
        )
    })?;
    Ok(f(session))
}

fn session_status(st: &AppState, rel: &str) -> Result<Value> {
    let (warn, release) = idle_config(st);
    let (lock_held, reason, idle_for) = with_session(st, rel, |s| {
        (
            s.lock_held,
            s.released_reason.clone(),
            s.last_activity.elapsed(),
        )
    })?;
    let ws = st.workspace()?;
    let idle_state = if lock_held {
        idle::evaluate(idle_for, warn, release)
    } else {
        idle::IdleState::Release
    };
    let lock = locks::status(&ws.root, rel, &st.identity())?;
    Ok(
        json!({ "lock_held": lock_held, "released_reason": reason, "idle": idle_state, "lock": lock }),
    )
}

pub async fn edit_activity(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PathBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let rel = validate_article_path(&body.path)?;
        with_session(st, &rel, |s| s.last_activity = Instant::now())?;
        session_status(st, &rel)
    })
    .await
}

pub async fn edit_status(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PathQuery>,
) -> ApiResult {
    blocking(state, move |st| {
        let rel = validate_article_path(&q.path)?;
        session_status(st, &rel)
    })
    .await
}

#[derive(Deserialize)]
pub struct ReleaseBody {
    path: String,
    /// Keep the editing session (and its draft) open without the lock, as
    /// after "Unlock the page now". Otherwise the editor is being closed.
    #[serde(default)]
    keep_session: bool,
}

pub async fn edit_release(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ReleaseBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&body.path)?;
        locks::release(&ws.root, &rel, &st.identity())?;
        let key = AppState::session_key(&ws.instance_id(), &rel);
        let mut edits = st.edits.lock().expect("edits lock");
        if body.keep_session {
            if let Some(session) = edits.get_mut(&key) {
                session.lock_held = false;
                session.released_reason = Some("manual".into());
            }
        } else {
            edits.remove(&key);
            drop(edits);
            // Closing without changes: forget the untouched copy, so the page
            // doesn't claim there are unsaved changes.
            let instance = ws.instance_id();
            let drafts = st.drafts();
            if let Some(draft) = drafts.load(&instance, &rel) {
                let published = read_optional(&ws.root.resolve_for_create(&rel)?)?
                    .map(|b| String::from_utf8_lossy(&b).into_owned());
                if !draft.is_new && !draft_has_changes(&draft, published.as_deref()) {
                    drafts.discard(&instance, &rel)?;
                }
            }
        }
        Ok(json!({ "released": true }))
    })
    .await
}

pub async fn edit_reclaim(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PathBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        locks::reclaim_own_stale(&ws.root, &rel, &st.identity())?;
        Ok(json!({ "reclaimed": true }))
    })
    .await
}

#[derive(Deserialize)]
pub struct ContentBody {
    path: String,
    content: String,
}

/// This person's unpublished work in the open documentation folder: new
/// pages that were never published, and pages with changes. Newest first.
pub async fn drafts_list(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        let ws = st.workspace()?;
        let mut out: Vec<Value> = Vec::new();
        let mut drafts = st.drafts().list(&ws.instance_id());
        drafts.sort_by_key(|d| std::cmp::Reverse(d.updated_at));
        for draft in drafts {
            let Ok(rel) = validate_article_path(&draft.article) else {
                continue;
            };
            let published = ws
                .root
                .resolve_for_create(&rel)
                .ok()
                .and_then(|p| read_optional(&p).ok().flatten())
                .map(|b| String::from_utf8_lossy(&b).into_owned());
            if !draft_has_changes(&draft, published.as_deref()) {
                continue;
            }
            let (_, body) = parse_front_matter(&draft.content);
            out.push(json!({
                "path": rel,
                "title": extract_title(body).unwrap_or_else(|| title_from_filename(&rel)),
                "updated_at": draft.updated_at,
                "is_new": published.is_none(),
                "folder": parent_of(&rel),
            }));
        }
        Ok(json!({ "drafts": out }))
    })
    .await
}

fn load_draft(st: &AppState, instance: &str, rel: &str) -> Result<Draft> {
    st.drafts().load(instance, rel).ok_or_else(|| {
        CairnError::NotFound(
            "Your unsaved copy could not be found. Please reopen the editor.".into(),
        )
    })
}

pub async fn draft_save(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ContentBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&body.path)?;
        if body.content.len() > publish::MAX_ARTICLE_BYTES {
            return Err(CairnError::BadRequest(
                "This page is too long (over 5 MB).".into(),
            ));
        }
        let mut draft = load_draft(st, &ws.instance_id(), &rel)?;
        if draft.content != body.content {
            with_session(st, &rel, |s| s.last_activity = Instant::now())?;
        }
        draft.content = body.content;
        draft.updated_at = locks::now_secs();
        let outcome = st.drafts().save(&draft);
        let mut status = session_status(st, &rel)?;
        status["saved_at"] = json!(draft.updated_at);
        status["persisted"] = json!(outcome.persisted);
        status["save_error"] = json!(outcome.error);
        Ok(status)
    })
    .await
}

#[derive(Deserialize)]
pub struct DiscardBody {
    path: String,
    #[serde(default)]
    keep_lock: bool,
}

pub async fn draft_discard(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DiscardBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&body.path)?;
        let instance = ws.instance_id();
        st.drafts().discard(&instance, &rel)?;
        if !body.keep_lock {
            locks::release(&ws.root, &rel, &st.identity())?;
        }
        // Forget the session so the next start creates a fresh draft.
        st.edits
            .lock()
            .expect("edits lock")
            .remove(&AppState::session_key(&instance, &rel));
        Ok(json!({ "discarded": true }))
    })
    .await
}

pub async fn draft_image(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
    bytes: Bytes,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(q.get("path").map(String::as_str).unwrap_or(""))?;
        let original = q.get("name").map(String::as_str);
        let info = validate_image(&bytes, original, &st.config().image_limits())?;
        with_session(st, &rel, |s| s.last_activity = Instant::now())?;

        // Content-addressed name; lengthen the hash if a different file
        // already has this name in the published assets folder.
        let assets = assets_dir_rel(&rel);
        let mut name = asset_file_name(original, &bytes, info.ext, 8);
        let existing = ws.root.resolve_for_create(&format!("{assets}/{name}"))?;
        if read_optional(&existing)?.is_some_and(|b| b != bytes.as_ref()) {
            name = asset_file_name(original, &bytes, info.ext, 20);
        }
        let staged = StagedImage {
            name: name.clone(),
            target_rel: format!("{assets}/{name}"),
            mime: info.mime.into(),
            size: bytes.len() as u64,
        };
        let outcome = st
            .drafts()
            .add_image(&ws.instance_id(), &rel, staged, bytes.to_vec())?;
        Ok(json!({
            "link": image_link_for(&rel, &name),
            "name": name,
            "width": info.width,
            "height": info.height,
            "persisted": outcome.persisted,
            "save_error": outcome.error,
        }))
    })
    .await
}

fn staged_urls(st: &AppState, rel: &str, draft: &Draft) -> HashMap<String, String> {
    draft
        .staged
        .iter()
        .map(|s| {
            let url = format!(
                "/draft-file?article={}&name={}&k={}",
                encode_path(rel),
                encode_path(&s.name),
                st.read_key
            );
            (s.target_rel.clone(), url)
        })
        .collect()
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ContentBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&body.path)?;
        let staged = st
            .drafts()
            .load(&ws.instance_id(), &rel)
            .map(|d| staged_urls(st, &rel, &d))
            .unwrap_or_default();
        let (meta, md) = parse_front_matter(&body.content);
        let rendered = article::render(md, &render_ctx(st, &ws.root, &rel, &staged));
        Ok(json!({
            "title": extract_title(md).unwrap_or_else(|| title_from_filename(&rel)),
            "html": rendered.html,
            "toc": rendered.toc,
            "broken_links": rendered.broken_links,
            "meta": meta,
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct PublishBody {
    path: String,
    content: String,
    /// Set after reviewing a conflict: publish over this exact version.
    accept_current_hash: Option<String>,
    /// Set after reviewing a conflict where the page was deleted meanwhile.
    #[serde(default)]
    accept_missing: bool,
}

pub async fn publish(
    State(state): State<Arc<AppState>>,
    Json(body): Json<PublishBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        if !with_session(st, &rel, |s| s.lock_held)? {
            return Err(CairnError::Locked(
                "This page was unlocked while you were away. Choose \"Continue editing\" to lock \
                 it again, then publish. Your text is kept."
                    .into(),
            ));
        }
        let instance = ws.instance_id();
        let drafts = st.drafts();
        let mut draft = load_draft(st, &instance, &rel)?;
        draft.content = body.content.clone();
        draft.updated_at = locks::now_secs();
        drafts.save(&draft);

        let (_, md) = parse_front_matter(&body.content);
        let referenced: Vec<String> = article::referenced_local_targets(&rel, md)
            .into_iter()
            .map(|r| r.to_lowercase())
            .collect();
        let mut staged = Vec::new();
        for s in &draft.staged {
            match drafts.image_bytes(&instance, &rel, &s.name) {
                Some(b) => staged.push((s.target_rel.clone(), b)),
                None if referenced.contains(&s.target_rel.to_lowercase()) => {
                    return Err(CairnError::NotFound(format!(
                        "The picture {} in your text could no longer be found. Remove it and \
                         insert it again.",
                        s.name
                    )));
                }
                None => {}
            }
        }
        let base = if body.accept_missing {
            None
        } else {
            body.accept_current_hash.clone().or(draft.base_hash.clone())
        };
        let opts = PublishOptions {
            image_limits: st.config().image_limits(),
            fail_at: None,
        };
        let me = st.identity();
        let request = PublishRequest {
            root: &ws.root,
            article_rel: &rel,
            identity: &me,
            base_hash: base.as_deref(),
            content: &body.content,
            staged,
        };
        match publish::publish(request, &opts)? {
            PublishOutcome::Published {
                hash, new_assets, ..
            } => {
                drafts.discard(&instance, &rel)?;
                locks::release(&ws.root, &rel, &me)?;
                st.edits
                    .lock()
                    .expect("edits lock")
                    .remove(&AppState::session_key(&instance, &rel));
                ws.refresh_index(true);
                Ok(json!({
                    "result": "published",
                    "path": rel,
                    "hash": hash,
                    "new_pictures": new_assets.len(),
                }))
            }
            PublishOutcome::Conflict {
                current_content,
                current_hash,
            } => {
                let theirs = current_content.clone().unwrap_or_default();
                Ok(json!({
                    "result": "conflict",
                    "current_hash": current_hash,
                    "current_content": current_content,
                    "deleted": current_hash.is_none(),
                    "diff": line_diff(&theirs, &body.content),
                }))
            }
        }
    })
    .await
}

// ---------------------------------------------------------------- history

pub async fn history_list(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PathQuery>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&q.path)?;
        let title = ws
            .index
            .lock()
            .expect("index lock")
            .get(&rel)
            .map(|p| p.title)
            .unwrap_or_else(|| title_from_filename(&rel));
        Ok(json!({
            "path": rel,
            "title": title,
            "versions": history::list_versions(&ws.root, &rel)?,
            "can_restore": ws.read_only.is_none(),
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct VersionQuery {
    path: String,
    id: String,
}

pub async fn history_version(
    State(state): State<Arc<AppState>>,
    Query(q): Query<VersionQuery>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&q.path)?;
        let text =
            String::from_utf8_lossy(&history::read_version(&ws.root, &rel, &q.id)?).into_owned();
        let (_, md) = parse_front_matter(&text);
        let staged = HashMap::new();
        let rendered = article::render(md, &render_ctx(st, &ws.root, &rel, &staged));
        let current = read_optional(&ws.root.resolve_for_create(&rel)?)?
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        Ok(json!({
            "id": q.id,
            "content": text,
            "html": rendered.html,
            "diff": line_diff(&current, &text),
        }))
    })
    .await
}

#[derive(Deserialize)]
pub struct RestoreBody {
    path: String,
    id: String,
}

pub async fn history_restore(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RestoreBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        require_writable(&ws)?;
        let rel = validate_article_path(&body.path)?;
        let key = AppState::session_key(&ws.instance_id(), &rel);
        if st.edits.lock().expect("edits lock").contains_key(&key) {
            return Err(CairnError::Conflict(
                "You have this page open for editing. Close the editor first, then restore.".into(),
            ));
        }
        check_write_permission(st, &ws, &rel)?;
        let content =
            String::from_utf8(history::read_version(&ws.root, &rel, &body.id)?).map_err(|_| {
                CairnError::BadRequest("That earlier version isn't readable text.".into())
            })?;
        let me = st.identity();
        if let Acquire::HeldBy(v) = locks::acquire(&ws.root, &rel, &me)? {
            return Err(locks::held_error(&v));
        }
        let result = publish::current_hash(&ws.root, &rel).and_then(|base| {
            let opts = PublishOptions {
                image_limits: st.config().image_limits(),
                fail_at: None,
            };
            let request = PublishRequest {
                root: &ws.root,
                article_rel: &rel,
                identity: &me,
                base_hash: base.as_deref(),
                content: &content,
                staged: Vec::new(),
            };
            publish::publish(request, &opts)
        });
        let _ = locks::release(&ws.root, &rel, &me);
        match result? {
            PublishOutcome::Published { .. } => {
                ws.refresh_index(true);
                Ok(json!({ "restored": true, "path": rel }))
            }
            PublishOutcome::Conflict { .. } => Err(CairnError::Conflict(
                "The page changed while restoring. Please try again.".into(),
            )),
        }
    })
    .await
}

// --------------------------------------------------------------- settings

#[derive(Deserialize)]
pub struct SettingsBody {
    display_name: Option<String>,
    appearance: Option<String>,
    text_size: Option<String>,
    idle_warning_minutes: Option<u64>,
    idle_release_minutes: Option<u64>,
    persistent_drafts: Option<bool>,
    /// IANA zone name, or "" for this computer's timezone.
    timezone: Option<String>,
    pdf_paper: Option<String>,
    toolbar_labels: Option<bool>,
    /// "daily" or "manual".
    update_check: Option<String>,
}

pub async fn save_settings(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SettingsBody>,
) -> ApiResult {
    blocking(state, move |st| {
        let mut cfg = st.config();
        if let Some(name) = body.display_name {
            let name = name.trim().to_string();
            cfg.display_name = (!name.is_empty()).then_some(name);
        }
        if let Some(v) = body.appearance {
            cfg.appearance = v;
        }
        if let Some(v) = body.text_size {
            cfg.text_size = v;
        }
        if let Some(v) = body.idle_warning_minutes {
            cfg.idle_warning_minutes = v;
        }
        if let Some(v) = body.idle_release_minutes {
            cfg.idle_release_minutes = v;
        }
        if let Some(zone) = body.timezone {
            let zone = zone.trim().to_string();
            cfg.timezone = (!zone.is_empty()).then_some(zone);
        }
        if let Some(v) = body.pdf_paper {
            cfg.pdf_paper = v;
        }
        if let Some(v) = body.toolbar_labels {
            cfg.toolbar_labels = v;
        }
        if let Some(v) = body.update_check {
            cfg.update_check = v;
        }
        let drafts_changed = body
            .persistent_drafts
            .is_some_and(|v| v != cfg.persistent_drafts);
        if let Some(v) = body.persistent_drafts {
            cfg.persistent_drafts = v;
        }
        cfg.validate()?;
        if drafts_changed && !st.edits.lock().expect("edits lock").is_empty() {
            return Err(CairnError::Conflict(
                "Close any pages you are editing before changing where unsaved copies are kept."
                    .into(),
            ));
        }
        config::save(&st.config_dir, &cfg)?;
        if drafts_changed {
            *st.drafts.write().expect("drafts lock") =
                Arc::new(draft_store_for(&cfg, &st.config_dir));
        }
        {
            let mut id = st.identity.write().expect("identity lock");
            id.display_name = cfg
                .display_name
                .clone()
                .unwrap_or_else(|| id.os_user.clone());
        }
        *st.config.write().expect("config lock") = cfg;
        Ok(state_view(st))
    })
    .await
}

pub async fn quit(State(state): State<Arc<AppState>>) -> ApiResult {
    let st = state.clone();
    let _ = tokio::task::spawn_blocking(move || st.release_all_locks()).await;
    state.shutdown.notify_one();
    Ok(Json(json!({ "quitting": true })))
}

// ------------------------------------------------------------ file serving

fn file_response(bytes: Vec<u8>, file_name: &str) -> Response {
    match sniff_mime(&bytes) {
        Some(mime) => {
            let headers = [
                (header::CONTENT_TYPE, mime.to_string()),
                (header::CACHE_CONTROL, "no-cache".to_string()),
            ];
            (headers, bytes).into_response()
        }
        None => {
            // Anything that isn't a supported picture is downloaded, never rendered.
            let safe: String = file_name
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
                .collect();
            let headers = [
                (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{safe}\""),
                ),
            ];
            (headers, bytes).into_response()
        }
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

pub async fn workspace_file(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
) -> Response {
    let result = tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, String)> {
        let ws = state.workspace()?;
        let rel = normalize_relative(&path)?;
        if is_system_path(&rel) || rel.split('/').any(|p| p.starts_with('.')) {
            return Err(CairnError::PathRejected("Not allowed.".into()));
        }
        let abs = ws.root.resolve(&rel)?;
        if !abs.is_file() {
            return Err(CairnError::NotFound("Not found.".into()));
        }
        let name = rel.rsplit('/').next().unwrap_or("file").to_string();
        Ok((std::fs::read(abs)?, name))
    })
    .await;
    match result {
        Ok(Ok((bytes, name))) => file_response(bytes, &name),
        _ => not_found(),
    }
}

pub async fn draft_file(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let ws = state.workspace()?;
        let rel = validate_article_path(q.get("article").map(String::as_str).unwrap_or(""))?;
        let name = q.get("name").cloned().unwrap_or_default();
        state
            .drafts()
            .image_bytes(&ws.instance_id(), &rel, &name)
            .ok_or_else(|| CairnError::NotFound("Not found.".into()))
    })
    .await;
    match result {
        Ok(Ok(bytes)) if sniff_mime(&bytes).is_some() => file_response(bytes, "picture"),
        _ => not_found(),
    }
}
