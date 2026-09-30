//! New versions of Cairn: checking (daily or on request), installing, going
//! back, and restarting into the new program without breaking the browser
//! tab that is open.
//!
//! The restart hands over to the new program through `handoff.json` in the
//! person's own settings folder: the same port and keys, so the open tab
//! keeps working. The file is read once and deleted straight away.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::extract::State;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::AppState;
use super::api::{ApiResult, blocking};
use crate::article::render_notes;
use crate::error::{CairnError, Result};
use crate::fsutil::{read_optional, write_atomic};
use crate::locks::now_secs;
use crate::update::{self, Release, UpdateSource};

/// With the daily setting: look again this long after a successful check,
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// or this long after a failed one (for example while offline).
const RETRY_AFTER: Duration = Duration::from_secs(3 * 60 * 60);
/// The first automatic check waits until Cairn has been running a minute.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(60);
const HANDOFF_FILE: &str = "handoff.json";
const HANDOFF_MAX_AGE_SECS: u64 = 120;
/// Time for the reply to reach the browser before Cairn restarts.
const RESTART_DELAY: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct Updates {
    state: Mutex<UpdateState>,
}

#[derive(Default)]
struct UpdateState {
    checking: bool,
    installing: bool,
    /// Unix seconds of the last successful check.
    checked_at: Option<u64>,
    last_attempt: Option<Instant>,
    last_ok: bool,
    latest: Option<Release>,
    error: Option<String>,
}

impl Updates {
    fn lock(&self) -> MutexGuard<'_, UpdateState> {
        self.state.lock().expect("updates lock")
    }

    /// Whether the daily check is due. `started` is when Cairn started.
    pub fn due(&self, started: Instant) -> bool {
        let s = self.lock();
        if s.checking || s.installing {
            return false;
        }
        match s.last_attempt {
            None => started.elapsed() >= FIRST_CHECK_AFTER,
            Some(t) => t.elapsed() >= if s.last_ok { CHECK_EVERY } else { RETRY_AFTER },
        }
    }
}

/// Ask GitHub whether there is a newer version, and remember the answer.
pub fn run_check(st: &AppState) {
    {
        let mut s = st.updates.lock();
        if s.checking {
            return;
        }
        s.checking = true;
    }
    let source = UpdateSource::with_proxy(st.config().update_proxy);
    let result = update::check(&source, &update::current_version());
    let mut s = st.updates.lock();
    s.checking = false;
    s.last_attempt = Some(Instant::now());
    match result {
        Ok(latest) => {
            s.latest = latest;
            s.error = None;
            s.last_ok = true;
            s.checked_at = Some(now_secs());
        }
        Err(e) => {
            s.error = Some(e.to_string());
            s.last_ok = false;
        }
    }
}

fn exe_path() -> Result<PathBuf> {
    std::env::current_exe()
        .map_err(|_| CairnError::Io("Cairn's program file couldn't be found.".into()))
}

fn status_view(st: &AppState) -> Value {
    let automatic = st.config().update_check == "daily";
    let exe = exe_path().ok();
    let blocker = match &exe {
        Some(path) => update::install_blocker(path),
        None => Some("Cairn's program file couldn't be found.".into()),
    };
    let previous = exe.as_deref().and_then(update::previous_version);
    let s = st.updates.lock();
    json!({
        "current": env!("CARGO_PKG_VERSION"),
        "automatic": automatic,
        "checking": s.checking,
        "installing": s.installing,
        "checked_at": s.checked_at,
        "error": s.error,
        "available": s.latest.as_ref().map(|r| json!({
            "version": r.version,
            "notes_html": render_notes(&r.notes),
            "page_url": r.page_url,
            "published_at": r.published_at,
        })),
        "can_install": blocker.is_none(),
        "cannot_install_reason": blocker,
        "previous": previous,
        "download_page": update::RELEASES_PAGE,
    })
}

pub async fn status(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| Ok(status_view(st))).await
}

pub async fn check_now(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state, |st| {
        run_check(st);
        Ok(status_view(st))
    })
    .await
}

/// The program to replace, once nothing stands in the way of restarting.
fn ready_to_restart(st: &AppState) -> Result<PathBuf> {
    if !st.edits.lock().expect("edits lock").is_empty() {
        return Err(CairnError::Conflict(
            "Publish or close the pages you are editing first, then update. Your changes are \
             kept either way."
                .into(),
        ));
    }
    let exe = exe_path()?;
    if let Some(reason) = update::install_blocker(&exe) {
        return Err(CairnError::Conflict(reason));
    }
    Ok(exe)
}

/// Mark an install as started, or say why one can't start.
fn begin_install(st: &AppState) -> Result<()> {
    let mut s = st.updates.lock();
    if s.installing {
        return Err(CairnError::Conflict("Cairn is already updating.".into()));
    }
    s.installing = true;
    Ok(())
}

fn install_release(exe: &Path, release: &Release, source: &UpdateSource) -> Result<()> {
    let dir = exe
        .parent()
        .ok_or_else(|| CairnError::Io("Cairn's folder couldn't be found.".into()))?;
    let new_exe = update::download(source, release, dir)?;
    let runs = update::program_version(&new_exe).and_then(|v| {
        if v.to_string() == release.version {
            Ok(())
        } else {
            Err(CairnError::Io(
                "The downloaded program isn't the expected version, so it wasn't installed.".into(),
            ))
        }
    });
    if let Err(e) = runs {
        let _ = fs::remove_file(&new_exe);
        return Err(e);
    }
    update::install(exe, &new_exe, &update::current_version())
}

/// Run `work` (which replaces the program) off the async threads, then
/// restart into the new program once the reply has been sent.
async fn replace_and_restart<F>(state: Arc<AppState>, work: F) -> ApiResult
where
    F: FnOnce(&AppState, &Path) -> Result<String> + Send + 'static,
{
    let st = state.clone();
    let out = tokio::task::spawn_blocking(move || -> Result<(String, PathBuf)> {
        let exe = ready_to_restart(&st)?;
        begin_install(&st)?;
        match work(&st, &exe) {
            Ok(version) => Ok((version, exe)),
            Err(e) => {
                st.updates.lock().installing = false;
                Err(e)
            }
        }
    })
    .await
    .map_err(|_| CairnError::Io("Something went wrong inside Cairn. Please try again.".into()))?;
    let (version, exe) = out?;
    schedule_restart(state, exe);
    Ok(axum::Json(
        json!({ "restarting": true, "version": version }),
    ))
}

pub async fn install(State(state): State<Arc<AppState>>) -> ApiResult {
    replace_and_restart(state, |st, exe| {
        let release = st.updates.lock().latest.clone().ok_or_else(|| {
            CairnError::BadRequest(
                "There is no new version to install. Choose Check now first.".into(),
            )
        })?;
        install_release(
            exe,
            &release,
            &UpdateSource::with_proxy(st.config().update_proxy),
        )?;
        Ok(release.version)
    })
    .await
}

pub async fn rollback(State(state): State<Arc<AppState>>) -> ApiResult {
    replace_and_restart(state, |_, exe| {
        update::rollback(exe, &update::current_version())
    })
    .await
}

// ---------------------------------------------------------------- restart

/// What the new program needs to take over from this one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Handoff {
    pub token: String,
    pub read_key: String,
    pub port: u16,
    /// Unix seconds.
    pub created_at: u64,
}

/// Read and delete the hand-over left by the program that just updated.
/// Anything old or malformed is ignored.
pub fn take_handoff(config_dir: &Path) -> Option<Handoff> {
    let path = config_dir.join(HANDOFF_FILE);
    let bytes = read_optional(&path).ok()??;
    let _ = fs::remove_file(&path);
    let h: Handoff = serde_json::from_slice(&bytes).ok()?;
    let secret = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
    let fresh = now_secs().saturating_sub(h.created_at) <= HANDOFF_MAX_AGE_SECS;
    (fresh && secret(&h.token) && secret(&h.read_key) && h.port != 0).then_some(h)
}

/// Start the program at `exe` in this one's place, then stop this one.
fn restart(st: &AppState, exe: &Path) -> Result<()> {
    let handoff = Handoff {
        token: st.token.clone(),
        read_key: st.read_key.clone(),
        port: st.port(),
        created_at: now_secs(),
    };
    fs::create_dir_all(&st.config_dir)?;
    let file = st.config_dir.join(HANDOFF_FILE);
    write_atomic(
        &file,
        &serde_json::to_vec(&handoff).expect("handoff serializes"),
    )?;
    st.release_all_locks();
    let spawned = Command::new(exe)
        .args([
            "--no-browser",
            "--handoff",
            "--port",
            &handoff.port.to_string(),
        ])
        .spawn();
    if let Err(e) = spawned {
        let _ = fs::remove_file(&file);
        return Err(CairnError::Io(format!(
            "The new version couldn't be started ({e})."
        )));
    }
    st.shutdown.notify_one();
    Ok(())
}

fn schedule_restart(state: Arc<AppState>, exe: PathBuf) {
    tokio::spawn(async move {
        tokio::time::sleep(RESTART_DELAY).await;
        let st = state.clone();
        let outcome = tokio::task::spawn_blocking(move || restart(&st, &exe)).await;
        if !matches!(outcome, Ok(Ok(()))) {
            let mut s = state.updates.lock();
            s.installing = false;
            s.error = Some(
                "Cairn was updated but couldn't restart by itself. Close the Cairn window and \
                 start Cairn again."
                    .into(),
            );
        }
    });
}
