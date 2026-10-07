//! The local HTTP server: shared state, routing, and background work.

pub mod api;
pub mod api_export;
pub mod api_manage;
pub mod api_templates;
pub mod api_update;
pub mod idle;
pub mod security;

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use rust_embed::RustEmbed;
use serde::Serialize;
use tokio::net::TcpListener;

use crate::config::AppConfig;
use crate::drafts::DraftStore;
use crate::error::CairnError;
use crate::locks::{self, Identity};
use crate::paths::Root;
use crate::search::SearchIndex;
use crate::workspace::{Marker, StorageReport};

#[derive(RustEmbed)]
#[folder = "assets/"]
struct Assets;

/// An opened workspace.
pub struct OpenWorkspace {
    pub root: Root,
    pub marker: RwLock<Marker>,
    /// Why writing is disabled, if it is (newer schema, no write access).
    pub read_only: Option<String>,
    pub storage: StorageReport,
    pub index: Mutex<SearchIndex>,
    pub last_refresh: Mutex<Option<Instant>>,
}

impl OpenWorkspace {
    pub fn instance_id(&self) -> String {
        self.marker.read().expect("marker lock").instance_id.clone()
    }

    /// Refresh the page index unless it was refreshed in the last few seconds.
    pub fn refresh_index(&self, force: bool) {
        let mut last = self.last_refresh.lock().expect("refresh lock");
        if !force && last.is_some_and(|t| t.elapsed() < Duration::from_secs(3)) {
            return;
        }
        let _ = self.index.lock().expect("index lock").refresh(&self.root);
        *last = Some(Instant::now());
    }
}

/// One page this client is editing.
#[derive(Debug, Clone)]
pub struct EditSession {
    pub article: String,
    pub instance_id: String,
    pub lock_held: bool,
    pub last_activity: Instant,
    /// Why the lock was released while the editor stayed open (e.g. idle).
    pub released_reason: Option<String>,
}

pub struct AppState {
    pub token: String,
    pub read_key: String,
    port: AtomicU16,
    pub config_dir: PathBuf,
    pub config: RwLock<AppConfig>,
    pub config_warning: Option<String>,
    pub identity: RwLock<Identity>,
    pub workspace: RwLock<Option<Arc<OpenWorkspace>>>,
    pub edits: Mutex<HashMap<String, EditSession>>,
    pub drafts: RwLock<Arc<DraftStore>>,
    /// Folders where a write probe failed this session.
    pub denied_dirs: Mutex<HashSet<String>>,
    /// Folder and whole-documentation downloads being prepared.
    pub exports: api_export::ExportJobs,
    /// New versions of Cairn: the last check and any install in progress.
    pub updates: api_update::Updates,
    pub shutdown: tokio::sync::Notify,
    /// After an update on a Mac or Linux: the program this one becomes once
    /// the server has stopped (see `api_update::restart`).
    pub restart_into: Mutex<Option<PathBuf>>,
}

fn random_secret() -> String {
    // Two v4 UUIDs: 244 random bits from the OS random source.
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

pub fn draft_store_for(cfg: &AppConfig, config_dir: &Path) -> DraftStore {
    if !cfg.persistent_drafts {
        return DraftStore::new(None);
    }
    let dir = cfg
        .draft_dir
        .clone()
        .unwrap_or_else(|| config_dir.join("drafts"));
    DraftStore::new(Some(dir))
}

impl AppState {
    pub fn new(
        config_dir: PathBuf,
        config: AppConfig,
        config_warning: Option<String>,
    ) -> Arc<AppState> {
        Self::with_secrets(config_dir, config, config_warning, None)
    }

    /// As [`AppState::new`], but reusing the access token and read key of
    /// the program this one replaced (after an update), so the browser tab
    /// that is already open keeps working.
    pub fn with_secrets(
        config_dir: PathBuf,
        config: AppConfig,
        config_warning: Option<String>,
        secrets: Option<(String, String)>,
    ) -> Arc<AppState> {
        let identity = Identity::current(config.display_name.as_deref());
        let drafts = draft_store_for(&config, &config_dir);
        let (token, read_key) = secrets.unwrap_or_else(|| (random_secret(), random_secret()));
        Arc::new(AppState {
            token,
            read_key,
            port: AtomicU16::new(0),
            config_dir,
            config: RwLock::new(config),
            config_warning,
            identity: RwLock::new(identity),
            workspace: RwLock::new(None),
            edits: Mutex::new(HashMap::new()),
            drafts: RwLock::new(Arc::new(drafts)),
            denied_dirs: Mutex::new(HashSet::new()),
            exports: Default::default(),
            updates: Default::default(),
            shutdown: tokio::sync::Notify::new(),
            restart_into: Mutex::new(None),
        })
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }

    pub fn set_port(&self, port: u16) {
        self.port.store(port, Ordering::SeqCst);
    }

    pub fn identity(&self) -> Identity {
        self.identity.read().expect("identity lock").clone()
    }

    pub fn config(&self) -> AppConfig {
        self.config.read().expect("config lock").clone()
    }

    pub fn drafts(&self) -> Arc<DraftStore> {
        self.drafts.read().expect("drafts lock").clone()
    }

    pub fn workspace(&self) -> Result<Arc<OpenWorkspace>, CairnError> {
        self.workspace
            .read()
            .expect("workspace lock")
            .clone()
            .ok_or_else(|| CairnError::BadRequest("No documentation folder is open yet.".into()))
    }

    pub fn session_key(instance_id: &str, article: &str) -> String {
        format!("{instance_id}|{}", article.to_lowercase())
    }

    /// Release every lock this client holds (on close, quit, or switch).
    pub fn release_all_locks(&self) {
        let me = self.identity();
        let ws = self.workspace.read().expect("workspace lock").clone();
        let mut edits = self.edits.lock().expect("edits lock");
        if let Some(ws) = ws {
            for session in edits.values().filter(|s| s.lock_held) {
                let _ = locks::release(&ws.root, &session.article, &me);
            }
        }
        edits.clear();
    }

    /// Heartbeat held locks and release those idle past the limit.
    /// `now` is injectable for tests.
    pub fn sweep(&self, now: Instant, heartbeat: bool) {
        let Ok(ws) = self.workspace() else { return };
        let cfg = self.config();
        let me = self.identity();
        let warn = Duration::from_secs(cfg.idle_warning_minutes * 60);
        let release = Duration::from_secs(cfg.idle_release_minutes * 60);
        let mut edits = self.edits.lock().expect("edits lock");
        for session in edits.values_mut().filter(|s| s.lock_held) {
            let idle_for = now.saturating_duration_since(session.last_activity);
            if idle::evaluate(idle_for, warn, release) == idle::IdleState::Release {
                let _ = locks::release(&ws.root, &session.article, &me);
                session.lock_held = false;
                session.released_reason = Some("idle".into());
            } else if heartbeat && locks::heartbeat(&ws.root, &session.article, &me).is_err() {
                // Our lock was removed (maintainer recovery): stop claiming it.
                session.lock_held = false;
                session.released_reason = Some("lost".into());
            }
        }
    }
}

// ------------------------------------------------------------------ errors

pub struct ApiError(pub CairnError);

impl From<CairnError> for ApiError {
    fn from(e: CairnError) -> Self {
        ApiError(e)
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            CairnError::NotFound(_) => StatusCode::NOT_FOUND,
            CairnError::BadRequest(_) | CairnError::PathRejected(_) => StatusCode::BAD_REQUEST,
            CairnError::PermissionDenied(_) | CairnError::ReadOnly(_) => StatusCode::FORBIDDEN,
            CairnError::Conflict(_) | CairnError::PdfUnavailable(_) => StatusCode::CONFLICT,
            CairnError::Locked(_) => StatusCode::LOCKED,
            CairnError::InvalidImage(_) => StatusCode::UNPROCESSABLE_ENTITY,
            CairnError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = ErrorBody {
            error: self.0.code(),
            message: self.0.to_string(),
        };
        (status, axum::Json(body)).into_response()
    }
}

// ------------------------------------------------------------ static files

fn serve_asset(path: &str) -> Response {
    match Assets::get(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            let headers = [
                (header::CONTENT_TYPE, mime.as_ref().to_string()),
                (header::CACHE_CONTROL, "no-cache".to_string()),
            ];
            (headers, file.data.into_owned()).into_response()
        }
        None => (StatusCode::NOT_FOUND, "Not found").into_response(),
    }
}

async fn index_html() -> Response {
    serve_asset("index.html")
}

async fn static_file(axum::extract::Path(path): axum::extract::Path<String>) -> Response {
    if path.split('/').any(|seg| seg == ".." || seg.is_empty()) {
        return (StatusCode::NOT_FOUND, "Not found").into_response();
    }
    serve_asset(&path)
}

/// Names of every embedded asset (used by the accessibility markup test).
pub fn embedded_asset_names() -> Vec<String> {
    Assets::iter().map(|c| c.into_owned()).collect()
}

pub fn embedded_asset(path: &str) -> Option<Vec<u8>> {
    Assets::get(path).map(|f| f.data.into_owned())
}

// ------------------------------------------------------------------ router

pub fn build_router(state: Arc<AppState>) -> Router {
    let image_limit = DefaultBodyLimit::max(64 * 1024 * 1024);
    let api = Router::new()
        .route("/api/state", get(api::get_state))
        .route("/api/workspace/pick", post(api::pick_folder))
        .route("/api/workspace/discover", post(api::discover))
        .route("/api/workspace/open", post(api::open_workspace))
        .route("/api/workspace/init", post(api::init_workspace))
        .route("/api/workspace/rename", post(api::rename_workspace))
        .route("/api/workspace/cleanup", post(api::set_version_cleanup))
        .route("/api/workspace/close", post(api::close_workspace))
        .route("/api/home", get(api::home))
        .route("/api/folder", get(api::folder))
        .route("/api/folder/create", post(api::create_folder))
        .route("/api/page", get(api::page))
        .route("/api/page/new", post(api::new_page))
        .route("/api/page/rename", post(api_manage::rename_page))
        .route("/api/page/move", post(api_manage::move_page))
        .route("/api/page/delete", post(api_manage::delete_page))
        .route("/api/folder/rename", post(api_manage::rename_folder))
        .route("/api/folder/move", post(api_manage::move_folder))
        .route("/api/folder/delete", post(api_manage::delete_folder))
        .route("/api/trash", get(api_manage::trash_list))
        .route("/api/trash/restore", post(api_manage::trash_restore))
        .route("/api/search", get(api::search))
        .route("/api/pages", get(api::all_pages))
        .route("/api/folders", get(api::all_folders))
        .route("/api/templates", get(api_templates::list))
        .route("/api/templates/new", post(api_templates::create))
        .route("/api/templates/delete", post(api_templates::delete))
        .route("/api/templates/deleted", get(api_templates::deleted))
        .route("/api/export/page", get(api_export::page))
        .route("/api/export/jobs", post(api_export::start_job))
        .route("/api/export/jobs/{id}", get(api_export::job_status))
        .route("/api/export/jobs/{id}/file", get(api_export::job_file))
        .route("/api/export/jobs/{id}/cancel", post(api_export::job_cancel))
        .route("/api/edit/start", post(api::edit_start))
        .route("/api/edit/activity", post(api::edit_activity))
        .route("/api/edit/status", get(api::edit_status))
        .route("/api/edit/release", post(api::edit_release))
        .route("/api/edit/reclaim", post(api::edit_reclaim))
        .route("/api/drafts", get(api::drafts_list))
        .route("/api/draft/save", post(api::draft_save))
        .route("/api/draft/discard", post(api::draft_discard))
        .route(
            "/api/draft/image",
            post(api::draft_image).layer(image_limit),
        )
        .route("/api/preview", post(api::preview))
        .route("/api/publish", post(api::publish))
        .route("/api/history", get(api::history_list))
        .route("/api/history/version", get(api::history_version))
        .route("/api/history/restore", post(api::history_restore))
        .route("/api/settings", post(api::save_settings))
        .route("/api/update", get(api_update::status))
        .route("/api/update/check", post(api_update::check_now))
        .route("/api/update/install", post(api_update::install))
        .route("/api/update/rollback", post(api_update::rollback))
        .route("/api/quit", post(api::quit));

    Router::new()
        .route("/", get(index_html))
        .route("/static/{*path}", get(static_file))
        .route("/ws-file/{*path}", get(api::workspace_file))
        .route("/draft-file", get(api::draft_file))
        .merge(api)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .with_state(state)
}

/// Bind to the loopback interface only. Never `0.0.0.0`: the UI must not be
/// reachable from other computers.
pub async fn bind_loopback(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await
}

/// Serve until the UI asks to quit or Ctrl-C is pressed.
pub async fn run(state: Arc<AppState>, listener: TcpListener) -> std::io::Result<()> {
    state.set_port(listener.local_addr()?.port());

    let bg = state.clone();
    tokio::spawn(async move {
        let started = Instant::now();
        let mut tick: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            tick += 1;
            let st = bg.clone();
            let heartbeat = tick.is_multiple_of(locks::HEARTBEAT_SECS / 5);
            let _ = tokio::task::spawn_blocking(move || st.sweep(Instant::now(), heartbeat)).await;
            // The daily look for a new version runs on its own, so a slow
            // connection never delays edit-lock heartbeats.
            if bg.config().update_check == "daily" && bg.updates.due(started) {
                let st = bg.clone();
                tokio::task::spawn_blocking(move || api_update::run_check(&st));
            }
        }
    });

    let app = build_router(state.clone());
    let shutdown_state = state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            tokio::select! {
                _ = shutdown_state.shutdown.notified() => {}
                _ = stop_requested() => {}
            }
        })
        .await?;
    let st = state.clone();
    let _ = tokio::task::spawn_blocking(move || st.release_all_locks()).await;
    Ok(())
}

/// Ctrl+C, and the ways Cairn is usually stopped without it: closing the
/// Cairn window, signing out, or shutting down on Windows; SIGTERM, or
/// SIGHUP when the Terminal window closes, elsewhere. Without these, edit locks stayed behind for everyone.
async fn stop_requested() {
    #[cfg(windows)]
    {
        use tokio::signal::windows;
        let (Ok(mut close), Ok(mut logoff), Ok(mut down)) = (
            windows::ctrl_close(),
            windows::ctrl_logoff(),
            windows::ctrl_shutdown(),
        ) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = close.recv() => {}
            _ = logoff.recv() => {}
            _ = down.recv() => {}
        }
    }
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        // SIGHUP: the Terminal window was closed, or the person logged out.
        let (Ok(mut term), Ok(mut hangup)) = (
            signal(SignalKind::terminate()),
            signal(SignalKind::hangup()),
        ) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
            _ = hangup.recv() => {}
        }
    }
}
