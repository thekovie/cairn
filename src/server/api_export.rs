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
    /// When the state last changed. The keep-for-ten-minutes window runs
    /// from the end of a job, not its start, so a slow PDF job is still
    /// there when it finishes.
    updated: Mutex<Instant>,
    file_name: String,
    state: Mutex<(JobState, Option<String>)>,
    result: Mutex<Option<Vec<u8>>>,
}

impl Job {
    fn new(total: usize, file_name: String) -> Job {
        Job {
            total,
            done: AtomicUsize::new(0),
            cancel: AtomicBool::new(false),
            updated: Mutex::new(Instant::now()),
            file_name,
            state: Mutex::new((JobState::Running, None)),
            result: Mutex::new(None),
        }
    }

    fn state(&self) -> (JobState, Option<String>) {
        self.state.lock().expect("job state").clone()
    }

    fn set_state(&self, state: JobState, message: Option<String>) {
        *self.updated.lock().expect("job time") = Instant::now();
        *self.state.lock().expect("job state") = (state, message);
    }

    fn is_expired(&self) -> bool {
        self.state().0 != JobState::Running
            && self.updated.lock().expect("job time").elapsed() >= JOB_TTL
    }
}

/// This app's export jobs (in memory only).
#[derive(Default)]
pub struct ExportJobs {
    jobs: Mutex<HashMap<String, Arc<Job>>>,
}

impl ExportJobs {
    fn prune(jobs: &mut HashMap<String, Arc<Job>>) {
        jobs.retain(|_, j| !j.is_expired());
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
        let job = Arc::new(Job::new(total, file_name));
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

/// A finished archive, and the files that changed while it was being made.
struct Built {
    zip: Vec<u8>,
    changed: Vec<String>,
}

/// `Guides/setup.md` and `Guides/setup.MD` both become `Guides/setup`.
fn without_md(rel: &str) -> &str {
    match rel.len().checked_sub(3) {
        Some(at) if rel.is_char_boundary(at) && rel[at..].eq_ignore_ascii_case(".md") => &rel[..at],
        _ => rel,
    }
}

fn run_job(
    job: &Job,
    ws: &OpenWorkspace,
    items: &[archive::Entry],
    settings: Option<&PrintSettings>,
) -> Result<Option<Built>> {
    let mut zip = archive::ZipBuilder::default();
    let mut changed = Vec::new();
    for entry in items {
        if job.cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let moved = match settings {
            Some(settings) => {
                let bytes = page_pdf(ws, settings, &entry.rel).map_err(|e| {
                    CairnError::Io(format!("Couldn't make a PDF of \"{}\": {e}", entry.rel))
                })?;
                zip.add(&format!("{}.pdf", without_md(&entry.rel)), &bytes)?;
                entry.changed_since(&ws.root)?
            }
            None => zip.add_entry(&ws.root, entry)?,
        };
        if moved {
            changed.push(entry.rel.clone());
        }
        job.done.fetch_add(1, Ordering::SeqCst);
    }
    Ok(Some(Built {
        zip: zip.finish()?,
        changed,
    }))
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The first few names, then how many more.
fn name_list(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    let shown = paths.iter().take(SHOWN).cloned().collect::<Vec<_>>();
    match paths.len().saturating_sub(SHOWN) {
        0 => shown.join(", "),
        more => format!("{}, and {more} more", shown.join(", ")),
    }
}

/// What the person should know about a download that did finish.
fn finish_note(changed: &[String], skipped: &[String]) -> Option<String> {
    let mut parts = Vec::new();
    if !changed.is_empty() {
        parts.push(format!(
            "{} changed while this download was being prepared, so it may mix older and \
             newer versions ({}). Download again for the latest.",
            count(changed.len(), "file"),
            name_list(changed)
        ));
    }
    if !skipped.is_empty() {
        parts.push(format!(
            "Left out {} because Cairn doesn't follow links out of the documentation ({}).",
            count(skipped.len(), "link"),
            name_list(skipped)
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// Run a job and record how it ended. A crash inside the work is a failed
/// job, never one that stays "running" and blocks every later download.
fn settle(job: &Job, run: impl FnOnce() -> Result<Option<Built>>, skipped: &[String]) {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(Some(built))) => {
            *job.result.lock().expect("job result") = Some(built.zip);
            job.set_state(JobState::Finished, finish_note(&built.changed, skipped));
        }
        Ok(Ok(None)) => job.set_state(JobState::Cancelled, None),
        Ok(Err(e)) => job.set_state(JobState::Failed, Some(e.to_string())),
        Err(_) => job.set_state(
            JobState::Failed,
            Some(
                "Something went wrong inside Cairn while preparing this download. \
                 Please try again."
                    .into(),
            ),
        ),
    }
}

pub async fn start_job(State(state): State<Arc<AppState>>, Json(body): Json<JobBody>) -> ApiResult {
    blocking(state.clone(), move |st| {
        let ws = st.workspace()?;
        let folder = folder_for(&ws, &body)?;
        let (found, settings) = match body.format {
            Format::Pdf => (
                archive::enumerate_pages(&ws.root, &folder)?,
                Some(print_settings(st, &ws)?),
            ),
            Format::Md | Format::Zip => (archive::enumerate(&ws.root, &folder)?, None),
        };
        let archive::Enumeration {
            entries: items,
            skipped_links,
        } = found;
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
        std::thread::spawn(move || {
            settle(
                &job,
                || run_job(&job, &ws, &items, settings.as_ref()),
                &skipped_links,
            )
        });
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

    #[test]
    fn pdf_names_drop_md_in_any_case() {
        assert_eq!(without_md("Guides/setup.md"), "Guides/setup");
        assert_eq!(without_md("Guides/Setup.MD"), "Guides/Setup");
        assert_eq!(without_md("Guides/notes.txt"), "Guides/notes.txt");
        assert_eq!(without_md("md"), "md");
        assert_eq!(without_md("é.md"), "é");
    }

    #[test]
    fn the_finish_note_names_changed_and_skipped_files() {
        assert_eq!(finish_note(&[], &[]), None);
        let one = vec!["Guides/a.md".to_string()];
        let note = finish_note(&one, &[]).unwrap();
        assert!(note.starts_with("1 file changed"), "{note}");
        assert!(note.contains("Guides/a.md"));
        let many: Vec<String> = (1..=5).map(|i| format!("p{i}.md")).collect();
        let note = finish_note(&[], &many).unwrap();
        assert!(
            note.contains("5 links") && note.contains("p3.md, and 2 more"),
            "{note}"
        );
        assert!(!note.contains("p4.md"));
    }

    fn running_job() -> Arc<Job> {
        Arc::new(Job::new(1, "x.zip".into()))
    }

    #[test]
    fn a_crash_in_the_work_fails_the_job_and_frees_the_next_download() {
        let jobs = ExportJobs::default();
        let (_, job) = jobs.start(1, "x.zip".into()).unwrap();
        settle(&job, || panic!("simulated crash"), &[]);
        let (state, message) = job.state();
        assert_eq!(state, JobState::Failed);
        assert!(message.unwrap().contains("Something went wrong"));
        assert!(jobs.start(1, "y.zip".into()).is_ok(), "no longer blocked");
    }

    #[test]
    fn a_finished_job_is_kept_for_ten_minutes_after_it_ends() {
        let Some(long_ago) = Instant::now().checked_sub(JOB_TTL + Duration::from_secs(60)) else {
            return; // the clock started less than eleven minutes ago
        };
        // Started long ago, finished just now: still there.
        let slow = running_job();
        *slow.updated.lock().unwrap() = long_ago;
        slow.set_state(JobState::Finished, None);
        assert!(!slow.is_expired());
        // Finished long ago: gone.
        *slow.updated.lock().unwrap() = long_ago;
        assert!(slow.is_expired());
        // Never dropped while it is still working.
        let busy = running_job();
        *busy.updated.lock().unwrap() = long_ago;
        assert!(!busy.is_expired());
    }
}
