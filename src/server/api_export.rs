//! Download routes: one page as Markdown, a zip, or a PDF, and background
//! jobs for whole folders or the entire documentation (so the browser can
//! show real progress and offer Cancel).

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::api::{ApiResult, blocking};
use super::{ApiError, AppState, OpenWorkspace};
use crate::error::{CairnError, Result};
use crate::export::html::{PrintInput, printable_html};
use crate::export::{archive, pdf};
use crate::paths::{is_system_path, normalize_relative};
use crate::publish::validate_article_path;
use crate::timefmt;

/// Finished or abandoned jobs are dropped after this long.
const JOB_TTL: Duration = Duration::from_secs(10 * 60);

// ------------------------------------------------------------------ jobs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Running,
    Finished,
    Failed,
    Cancelled,
}

pub struct Job {
    total: usize,
    done: AtomicUsize,
    cancel: AtomicBool,
    created: Instant,
    file_name: String,
    state: Mutex<(JobState, Option<String>)>,
    result: Mutex<Option<Vec<u8>>>,
}

impl Job {
    fn state(&self) -> (JobState, Option<String>) {
        self.state.lock().expect("job state").clone()
    }

    fn set_state(&self, state: JobState, message: Option<String>) {
        *self.state.lock().expect("job state") = (state, message);
    }
}

/// This app's export jobs (in memory only).
#[derive(Default)]
pub struct ExportJobs {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}

impl ExportJobs {
    fn prune(jobs: &mut HashMap<String, Arc<Job>>) {
        jobs.retain(|_, j| j.created.elapsed() < JOB_TTL || j.state().0 == JobState::Running);
    }

    fn get(&self, id: &str) -> Result<Arc<Job>> {
        let mut jobs = self.jobs.lock().expect("jobs lock");
        Self::prune(&mut jobs);
        jobs.get(id).cloned().ok_or_else(|| {
            CairnError::NotFound(
                "This download is no longer available. Please start it again.".into(),
            )
        })
    }

    fn start(&self, total: usize, file_name: String) -> Result<(String, Arc<Job>)> {
        let mut jobs = self.jobs.lock().expect("jobs lock");
        Self::prune(&mut jobs);
        if jobs.values().any(|j| j.state().0 == JobState::Running) {
            return Err(CairnError::BadRequest(
                "Another download is still being prepared. Please wait for it to finish.".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let job = Arc::new(Job {
            total,
            done: AtomicUsize::new(0),
            cancel: AtomicBool::new(false),
            created: Instant::now(),
            file_name,
            state: Mutex::new((JobState::Running, None)),
            result: Mutex::new(None),
        });
        jobs.insert(id.clone(), job.clone());
        Ok((id, job))
    }

    fn remove(&self, id: &str) {
        self.jobs.lock().expect("jobs lock").remove(id);
    }
}

// --------------------------------------------------------------- helpers

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Md,
    Zip,
    Pdf,
}

/// Settings needed to print pages, captured once per request.
struct PrintSettings {
    workspace_name: String,
    zone: jiff::tz::TimeZone,
    paper: String,
    browsers: Vec<std::path::PathBuf>,
}

fn print_settings(st: &AppState, ws: &OpenWorkspace) -> Result<PrintSettings> {
    let cfg = st.config();
    let browsers = pdf::find_browsers(cfg.pdf_browser.as_deref());
    if browsers.is_empty() {
        return Err(pdf::unavailable());
    }
    Ok(PrintSettings {
        workspace_name: ws.marker.read().expect("marker lock").display_name.clone(),
        zone: timefmt::user_zone(cfg.timezone.as_deref()),
        paper: cfg.pdf_paper.clone(),
        browsers,
    })
}

fn now_secs() -> i64 {
    jiff::Timestamp::now().as_second()
}

fn modified_secs(path: &std::path::Path) -> Option<i64> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    jiff::Timestamp::try_from(modified)
        .ok()
        .map(|t| t.as_second())
}

fn page_pdf(ws: &OpenWorkspace, settings: &PrintSettings, rel: &str) -> Result<Vec<u8>> {
    let path = ws.root.resolve(rel)?;
    let bytes = std::fs::read(&path)?;
    let text = String::from_utf8_lossy(&bytes);
    let html = printable_html(&PrintInput {
        root: &ws.root,
        rel,
        text: &text,
        workspace_name: &settings.workspace_name,
        zone: &settings.zone,
        paper: &settings.paper,
        modified: modified_secs(&path),
        exported_at: now_secs(),
    });
    pdf::render_pdf(&settings.browsers, &html)
}

/// Characters Windows doesn't allow in file names are replaced.
fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"\/:*?"<>|"#.contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "cairn-export".into()
    } else {
        trimmed.to_string()
    }
}

fn stem(rel: &str) -> String {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    let stem = file
        .strip_suffix(".md")
        .or_else(|| file.strip_suffix(".MD"))
        .unwrap_or(file);
    safe_file_name(stem)
}

fn download(bytes: Vec<u8>, file_name: &str, content_type: &str) -> Response {
    let ascii: String = file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let disposition = format!(
        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{}",
        crate::article::encode_path(file_name)
    );
    let headers = [
        (header::CONTENT_TYPE, content_type.to_string()),
        (header::CONTENT_DISPOSITION, disposition),
        (header::CACHE_CONTROL, "no-store".to_string()),
    ];
    (headers, bytes).into_response()
}

async fn run_blocking<T, F>(state: Arc<AppState>, f: F) -> std::result::Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&AppState) -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || f(&state))
        .await
        .map_err(|_| {
            ApiError(CairnError::Io(
                "Something went wrong inside Cairn. Please try again.".into(),
            ))
        })?
        .map_err(ApiError)
}

// ------------------------------------------------------------ single page

#[derive(Deserialize)]
pub struct PageQuery {
    path: String,
    format: Format,
}

pub async fn page(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> std::result::Result<Response, ApiError> {
    let (bytes, name, mime) = run_blocking(state, move |st| {
        let ws = st.workspace()?;
        let rel = validate_article_path(&q.path)?;
        let path = ws.root.resolve(&rel)?;
        if !path.is_file() {
            return Err(CairnError::NotFound("This page could not be found.".into()));
        }
        let stem = stem(&rel);
        Ok(match q.format {
            Format::Md => (
                std::fs::read(&path)?,
                format!("{stem}.md"),
                "text/markdown; charset=utf-8",
            ),
            Format::Zip => (
                archive::zip_files(&ws.root, &archive::page_files(&ws.root, &rel)?)?,
                format!("{stem}.zip"),
                "application/zip",
            ),
            Format::Pdf => {
                let settings = print_settings(st, &ws)?;
                (
                    page_pdf(&ws, &settings, &rel)?,
                    format!("{stem}.pdf"),
                    "application/pdf",
                )
            }
        })
    })
    .await?;
    Ok(download(bytes, &name, mime))
}

// ------------------------------------------------------------- bulk jobs

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Folder,
    All,
}

#[derive(Deserialize)]
pub struct JobBody {
    scope: Scope,
    #[serde(default)]
    path: String,
    format: Format,
}

fn folder_for(ws: &OpenWorkspace, body: &JobBody) -> Result<String> {
    if body.scope == Scope::All {
        return Ok(String::new());
    }
    let rel = normalize_relative(&body.path)?;
    if rel.is_empty() || is_system_path(&rel) {
        return Err(CairnError::PathRejected(
            "Please choose a folder to download.".into(),
        ));
    }
    if !ws.root.resolve(&rel)?.is_dir() {
        return Err(CairnError::NotFound(
            "This folder could not be found.".into(),
        ));
    }
    Ok(rel)
}

fn run_job(
    job: &Job,
    ws: &OpenWorkspace,
    items: &[String],
    settings: Option<&PrintSettings>,
) -> Result<Option<Vec<u8>>> {
    let mut zip = archive::ZipBuilder::default();
    for rel in items {
        if job.cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        match settings {
            Some(settings) => {
                let bytes = page_pdf(ws, settings, rel).map_err(|e| {
                    CairnError::Io(format!("Couldn't make a PDF of \"{rel}\": {e}"))
                })?;
                let name = format!("{}.pdf", rel.strip_suffix(".md").unwrap_or(rel));
                zip.add(&name, &bytes)?;
            }
            None => zip.add_file(&ws.root, rel)?,
        }
        job.done.fetch_add(1, Ordering::SeqCst);
    }
    zip.finish().map(Some)
}

pub async fn start_job(State(state): State<Arc<AppState>>, Json(body): Json<JobBody>) -> ApiResult {
    blocking(state.clone(), move |st| {
        let ws = st.workspace()?;
        let folder = folder_for(&ws, &body)?;
        let (items, settings) = match body.format {
            Format::Pdf => (
                archive::pages_in(&ws.root, &folder)?,
                Some(print_settings(st, &ws)?),
            ),
            Format::Md | Format::Zip => (archive::folder_files(&ws.root, &folder)?, None),
        };
        if items.is_empty() {
            return Err(CairnError::BadRequest(
                "There are no pages here to download yet.".into(),
            ));
        }
        let base = match folder.rsplit('/').next().filter(|s| !s.is_empty()) {
            Some(name) => name.to_string(),
            None => ws.marker.read().expect("marker lock").display_name.clone(),
        };
        let kind = if settings.is_some() { "-PDF" } else { "" };
        let today = timefmt::today(&timefmt::user_zone(st.config().timezone.as_deref()));
        let file_name = format!("{}{kind}-{today}.zip", safe_file_name(&base));
        let (id, job) = st.exports.start(items.len(), file_name.clone())?;
        let total = items.len();
        std::thread::spawn(
            move || match run_job(&job, &ws, &items, settings.as_ref()) {
                Ok(Some(bytes)) => {
                    *job.result.lock().expect("job result") = Some(bytes);
                    job.set_state(JobState::Finished, None);
                }
                Ok(None) => job.set_state(JobState::Cancelled, None),
                Err(e) => job.set_state(JobState::Failed, Some(e.to_string())),
            },
        );
        Ok(json!({ "id": id, "total": total, "file_name": file_name }))
    })
    .await
}

fn status_json(job: &Job) -> Value {
    let (state, message) = job.state();
    json!({
        "state": state,
        "done": job.done.load(Ordering::SeqCst),
        "total": job.total,
        "message": message,
        "file_name": job.file_name,
    })
}

pub async fn job_status(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let job = state.exports.get(&id)?;
    Ok(Json(status_json(&job)))
}

pub async fn job_cancel(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let job = state.exports.get(&id)?;
    job.cancel.store(true, Ordering::SeqCst);
    Ok(Json(json!({ "cancelled": true })))
}

pub async fn job_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> std::result::Result<Response, ApiError> {
    let job = state.exports.get(&id)?;
    if job.state().0 != JobState::Finished {
        return Err(ApiError(CairnError::Conflict(
            "This download isn't ready yet.".into(),
        )));
    }
    let bytes = job.result.lock().expect("job result").take();
    state.exports.remove(&id);
    match bytes {
        Some(bytes) => Ok(download(bytes, &job.file_name, "application/zip")),
        None => Err(ApiError(CairnError::NotFound(
            "This download was already saved. Please start it again.".into(),
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_safe_for_windows() {
        assert_eq!(safe_file_name("a/b:c?"), "a-b-c-");
        assert_eq!(safe_file_name("  ..  "), "cairn-export");
        assert_eq!(stem("Guides/Set up.md"), "Set up");
    }
}
