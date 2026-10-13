//! Download routes: one page as Markdown, a zip, or a PDF, and background
//! jobs for whole folders or the entire documentation (so the browser can
//! show real progress and offer Cancel).

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use axum::Json;
use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::io::ReaderStream;

use super::api::{ApiResult, blocking};
use super::{ApiError, AppState, OpenWorkspace};
use crate::backup;
use crate::error::{CairnError, Result};
use crate::export::archive::{count, name_list};
use crate::export::html::{self, PrintInput, printable_html};
use crate::export::{archive, pdf};
use crate::linkcheck::{self, CheckProgress};
use crate::paths::{is_system_path, normalize_relative};
use crate::publish::validate_article_path;
use crate::search::{PageSummary, ScanProblem};
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
    result: Mutex<Option<JobResult>>,
}

/// A finished download: small ones stay in memory, a backup (which can be
/// large) waits in a file in the temporary folder until it is fetched.
enum JobResult {
    Bytes(Vec<u8>),
    TempFile(PathBuf),
    /// The outcome of a documentation check (fetched as JSON, not a file).
    Report(Value),
}

impl Drop for Job {
    /// A backup that was never fetched must not stay on disk.
    fn drop(&mut self) {
        if let Some(JobResult::TempFile(path)) = self.result.get_mut().ok().and_then(Option::take) {
            let _ = std::fs::remove_file(path);
        }
    }
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
                "Another download, backup, or check is still being prepared. Please wait for it \
                 to finish."
                    .into(),
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
    (download_headers(file_name, content_type), bytes).into_response()
}

fn download_headers(file_name: &str, content_type: &str) -> [(header::HeaderName, String); 3] {
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
    [
        (header::CONTENT_TYPE, content_type.to_string()),
        (header::CONTENT_DISPOSITION, disposition),
        (header::CACHE_CONTROL, "no-store".to_string()),
    ]
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

// ------------------------------------------------------ PDF link warning

#[derive(Deserialize)]
pub struct PdfLinksQuery {
    scope: String,
    #[serde(default)]
    path: String,
}

/// Before a PDF is made: how many links to other pages and files it will
/// have to turn into plain text with a note (a PDF can't follow them), and
/// on how many pages, so the person can be told first.
pub async fn pdf_links(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PdfLinksQuery>,
) -> ApiResult {
    blocking(state, move |st| {
        let ws = st.workspace()?;
        let pages: Vec<String> = match q.scope.as_str() {
            "page" => vec![validate_article_path(&q.path)?],
            "folder" | "all" => {
                let scope = if q.scope == "all" {
                    Scope::All
                } else {
                    Scope::Folder
                };
                let folder = folder_for(
                    &ws,
                    &JobBody {
                        scope,
                        path: q.path.clone(),
                        format: Format::Pdf,
                    },
                )?;
                archive::pages_in(&ws.root, &folder)?
            }
            _ => {
                return Err(CairnError::BadRequest(
                    "Choose page, folder, or all.".into(),
                ));
            }
        };
        let (mut links, mut with_links) = (0usize, 0usize);
        let mut examples = Vec::new();
        for rel in &pages {
            // A page that can't be read fails the PDF itself, loudly.
            let Some(text) = ws
                .root
                .resolve(rel)
                .ok()
                .and_then(|p| std::fs::read(p).ok())
                .map(|b| String::from_utf8_lossy(&b).into_owned())
            else {
                continue;
            };
            let found = html::converted_links(&ws.root, rel, &text);
            if found.is_empty() {
                continue;
            }
            links += found.len();
            with_links += 1;
            if examples.len() < 5 {
                let mut targets: Vec<String> = Vec::new();
                for target in found {
                    if !targets.contains(&target) && targets.len() < 4 {
                        targets.push(target);
                    }
                }
                examples.push(json!({ "page": rel, "targets": targets }));
            }
        }
        Ok(json!({ "links": links, "pages": with_links, "examples": examples }))
    })
    .await
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
    result: JobResult,
    changed: Vec<String>,
    /// Anything else the person should read when it finishes.
    note: Option<String>,
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
        result: JobResult::Bytes(zip.finish()?),
        changed,
        note: None,
    }))
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
            *job.result.lock().expect("job result") = Some(built.result);
            let message = match (finish_note(&built.changed, skipped), built.note) {
                (Some(a), Some(b)) => Some(format!("{a} {b}")),
                (a, b) => a.or(b),
            };
            job.set_state(JobState::Finished, message);
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

// --------------------------------------------------------------- backups

/// Backups wait in the computer's temporary folder until they are fetched.
const BACKUP_FILE_PREFIX: &str = "cairn-backup-";
/// Backups left by a crash are removed after this long.
const STALE_BACKUP_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// Whether a file name is one Cairn made for a backup it is preparing
/// (`cairn-backup-<32 hex>.zip`, or its hidden half-written form). Someone's
/// own backup that happens to start with "cairn-backup-" is not matched.
fn is_backup_temp_name(name: &str) -> bool {
    let name = name.strip_prefix('.').unwrap_or(name);
    let Some(rest) = name.strip_prefix(BACKUP_FILE_PREFIX) else {
        return false;
    };
    let (id, after) = rest.split_at_checked(32).unwrap_or((rest, ""));
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()) && after.starts_with(".zip")
}

/// Remove backup files in `dir` (finished or half-written) older than `max_age`.
fn sweep_old_backups(dir: &std::path::Path, max_age: Duration) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if !is_backup_temp_name(&entry.file_name().to_string_lossy()) {
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age >= max_age);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn run_backup(
    job: &Job,
    ws: &OpenWorkspace,
    file: &std::path::Path,
    found: archive::Enumeration,
) -> Result<Option<Built>> {
    let cancelled = || job.cancel.load(Ordering::SeqCst);
    let on_file = |done: usize| job.done.store(done, Ordering::SeqCst);
    let progress = backup::Progress {
        cancelled: &cancelled,
        on_file: &on_file,
    };
    Ok(
        backup::create(&ws.root, file, found, &progress)?.map(|done| Built {
            result: JobResult::TempFile(file.to_path_buf()),
            changed: done.changed_during,
            note: done.space_note,
        }),
    )
}

/// Start a workspace backup. Progress, cancel and the file itself use the
/// same job routes as downloads.
pub async fn start_backup(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state.clone(), move |st| {
        let ws = st.workspace()?;
        let found = archive::enumerate_backup(&ws.root)?;
        let total = found.entries.len();
        let name = ws.marker.read().expect("marker lock").display_name.clone();
        let today = timefmt::today(&timefmt::user_zone(st.config().timezone.as_deref()));
        let file_name = format!("{}-backup-{today}.zip", safe_file_name(&name));
        let temp = std::env::temp_dir();
        sweep_old_backups(&temp, STALE_BACKUP_AFTER);
        let file = temp.join(format!(
            "{BACKUP_FILE_PREFIX}{}.zip",
            uuid::Uuid::new_v4().simple()
        ));
        let (id, job) = st.exports.start(total, file_name.clone())?;
        let skipped = found.skipped_links.clone();
        std::thread::spawn(move || {
            settle(&job, || run_backup(&job, &ws, &file, found), &skipped);
        });
        Ok(json!({ "id": id, "total": total, "file_name": file_name }))
    })
    .await
}

/// Deletes a file shortly after, retrying while something still has it
/// open (Windows won't delete an open file).
fn remove_soon(path: PathBuf) {
    std::thread::spawn(move || {
        for _ in 0..10 {
            if std::fs::remove_file(&path).is_ok() || !path.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

/// Streams a temporary file to the browser, and deletes it when the stream
/// ends or is dropped, whether the download finished or the browser gave up.
struct TempFileStream {
    inner: Option<ReaderStream<tokio::fs::File>>,
    path: PathBuf,
}

impl Stream for TempFileStream {
    type Item = std::io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut() {
            Some(inner) => Pin::new(inner).poll_next(cx),
            None => Poll::Ready(None),
        }
    }
}

impl Drop for TempFileStream {
    fn drop(&mut self) {
        self.inner = None;
        remove_soon(std::mem::take(&mut self.path));
    }
}

async fn stream_file(path: PathBuf, file_name: &str) -> std::result::Result<Response, ApiError> {
    let opened = async {
        let file = tokio::fs::File::open(&path).await?;
        let len = file.metadata().await?.len();
        Ok::<_, std::io::Error>((file, len))
    }
    .await;
    let (file, len) = match opened {
        Ok(opened) => opened,
        Err(e) => {
            remove_soon(path);
            return Err(ApiError(CairnError::Io(format!(
                "The backup could not be read back ({e}). Please make it again."
            ))));
        }
    };
    let stream = TempFileStream {
        inner: Some(ReaderStream::new(file)),
        path,
    };
    let mut response = (
        download_headers(file_name, "application/zip"),
        Body::from_stream(stream),
    )
        .into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_LENGTH, len.into());
    Ok(response)
}

// ---------------------------------------------------------------- checks

fn run_check(
    job: &Job,
    ws: &OpenWorkspace,
    pages: &[PageSummary],
    unread: &[ScanProblem],
) -> Result<Option<Built>> {
    let cancelled = || job.cancel.load(Ordering::SeqCst);
    let on_page = |done: usize| job.done.store(done, Ordering::SeqCst);
    let progress = CheckProgress {
        cancelled: &cancelled,
        on_page: &on_page,
    };
    let Some(report) = linkcheck::check_workspace_with(&ws.root, pages, &progress) else {
        return Ok(None);
    };
    // What the page list itself could not read counts too.
    let report = report.with_scan_problems(unread);
    let value = serde_json::to_value(report)
        .map_err(|e| CairnError::Io(format!("The result could not be made: {e}")))?;
    Ok(Some(Built {
        result: JobResult::Report(value),
        changed: Vec::new(),
        note: None,
    }))
}

/// Start looking through every page for links and pictures that lead
/// nowhere. Progress and Stop use the same job routes as downloads; the
/// result is fetched once from `check_result`.
pub async fn start_check(State(state): State<Arc<AppState>>) -> ApiResult {
    blocking(state.clone(), move |st| {
        let ws = st.workspace()?;
        ws.refresh_index(true);
        // Take the list and let go of the index: the check can take a while
        // on a slow share, and other screens shouldn't wait for it.
        let (pages, unread) = {
            let index = ws.index.lock().expect("index lock");
            (index.all(), index.problems().to_vec())
        };
        let total = pages.len();
        let (id, job) = st.exports.start(total, "check".into())?;
        std::thread::spawn(move || settle(&job, || run_check(&job, &ws, &pages, &unread), &[]));
        Ok(json!({ "id": id, "total": total }))
    })
    .await
}

/// The finished check, handed over once.
pub async fn check_result(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> ApiResult {
    let job = state.exports.get(&id)?;
    if job.state().0 != JobState::Finished {
        return Err(ApiError(CairnError::Conflict(
            "This check isn't finished yet.".into(),
        )));
    }
    let result = job.result.lock().expect("job result").take();
    state.exports.remove(&id);
    match result {
        Some(JobResult::Report(report)) => Ok(Json(report)),
        _ => Err(ApiError(CairnError::NotFound(
            "This result was already fetched. Please check again.".into(),
        ))),
    }
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
    let result = job.result.lock().expect("job result").take();
    state.exports.remove(&id);
    match result {
        Some(JobResult::Bytes(bytes)) => Ok(download(bytes, &job.file_name, "application/zip")),
        Some(JobResult::TempFile(path)) => stream_file(path, &job.file_name).await,
        Some(JobResult::Report(_)) => Err(ApiError(CairnError::NotFound(
            "That isn't a download.".into(),
        ))),
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

    #[test]
    fn old_backup_files_are_swept_and_other_files_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let ours = format!("cairn-backup-{id}.zip");
        let half = format!(".cairn-backup-{id}.zip.cairn-part-abc123");
        for name in [
            ours.as_str(),
            half.as_str(),
            "cairn-backup-2026-10-13.zip",
            "holiday.zip",
        ] {
            std::fs::write(dir.path().join(name), "x").unwrap();
        }
        // New files (a backup in progress) are kept.
        sweep_old_backups(dir.path(), Duration::from_secs(3600));
        assert!(dir.path().join(&ours).exists());
        sweep_old_backups(dir.path(), Duration::ZERO);
        assert!(!dir.path().join(&ours).exists());
        assert!(!dir.path().join(&half).exists());
        // Someone's own backups are never touched, even with a similar name.
        assert!(dir.path().join("cairn-backup-2026-10-13.zip").exists());
        assert!(dir.path().join("holiday.zip").exists());
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
