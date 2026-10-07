//! Turning printable HTML into a PDF with Microsoft Edge, Google Chrome, or
//! Chromium in headless mode. Edge is part of Windows 10 and 11, so there
//! nothing extra has to be installed; on a Mac or Linux any of the three works.
//!
//! Every render uses a private temporary folder and a separate browser
//! profile, so it never touches the person's open browser windows, and the
//! folder is always deleted afterwards. Only one render runs at a time.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::{CairnError, Result};

/// A render that takes longer than this is stopped.
const RENDER_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

static RENDER_LOCK: Mutex<()> = Mutex::new(());

/// Usual install locations, most likely first.
#[cfg(windows)]
fn candidate_paths() -> Vec<PathBuf> {
    let edge = ["Microsoft", "Edge", "Application", "msedge.exe"];
    let chrome = ["Google", "Chrome", "Application", "chrome.exe"];
    let mut out = Vec::new();
    for exe in [&edge, &chrome] {
        for var in ["ProgramFiles(x86)", "ProgramFiles", "LOCALAPPDATA"] {
            if let Some(base) = std::env::var_os(var) {
                out.push(exe.iter().fold(PathBuf::from(base), |p, seg| p.join(seg)));
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn candidate_paths() -> Vec<PathBuf> {
    let apps = ["Google Chrome", "Microsoft Edge", "Chromium"];
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Applications"));
    }
    roots
        .iter()
        .flat_map(|root| {
            apps.iter()
                .map(move |app| app_executable(&root.join(format!("{app}.app"))))
        })
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn candidate_paths() -> Vec<PathBuf> {
    let names = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
    ];
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    names
        .iter()
        .flat_map(|name| dirs.iter().map(move |d| d.join(name)))
        .collect()
}

/// The program inside a Mac app, so "/Applications/Google Chrome.app" can
/// be set in Settings as it appears in Finder.
fn app_executable(path: &Path) -> PathBuf {
    let is_app = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("app"));
    match path.file_stem() {
        Some(stem) if is_app => path.join("Contents").join("MacOS").join(stem),
        _ => path.to_path_buf(),
    }
}

/// Browsers to try, in order. A browser set in Settings is the only choice
/// (if it exists); otherwise Edge, then Chrome, in their usual places. More
/// than one is kept because a browser can be installed but not work (for
/// example when a tool redirects every Edge launch to another browser).
pub fn find_browsers(configured: Option<&Path>) -> Vec<PathBuf> {
    match configured {
        Some(path) => {
            let program = app_executable(path);
            program.is_file().then_some(program).into_iter().collect()
        }
        None => candidate_paths()
            .into_iter()
            .filter(|p| p.is_file())
            .collect(),
    }
}

/// The browser that last made a PDF, tried first next time.
static LAST_GOOD: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn unavailable() -> CairnError {
    CairnError::PdfUnavailable(
        "PDFs can't be made automatically on this computer \
         (no working Microsoft Edge, Google Chrome, or Chromium was found)."
            .into(),
    )
}

fn failed(detail: &str) -> CairnError {
    CairnError::Io(format!(
        "The PDF could not be made: {detail}. Please try again."
    ))
}

/// `file:///C:/...` URL for a local path.
fn file_url(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let encoded: String = s
        .split('/')
        .map(|seg| crate::article::encode_path(seg).replace("%3A", ":"))
        .collect::<Vec<_>>()
        .join("/");
    format!("file:///{}", encoded.trim_start_matches('/'))
}

fn hide_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

fn run_browser(browser: &Path, dir: &Path) -> Result<Vec<u8>> {
    let page = dir.join("page.html");
    let out = dir.join("page.pdf");
    let profile = dir.join("profile");
    let mut cmd = Command::new(browser);
    cmd.arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-pdf-header-footer")
        .arg("--disable-extensions")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-sync")
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(format!("--print-to-pdf={}", out.display()))
        .arg(file_url(&page))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_window(&mut cmd);
    let mut child = cmd.spawn().map_err(|_| unavailable())?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        // Chrome on a Mac can stay running after writing the PDF: a whole
        // PDF on disk means it's done.
        if std::fs::read(&out).is_ok_and(|b| is_whole_pdf(&b)) {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        if started.elapsed() > RENDER_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failed("the browser took too long"));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    let bytes = std::fs::read(&out).map_err(|_| failed("the browser didn't produce a file"))?;
    if !bytes.starts_with(b"%PDF") {
        return Err(failed("the browser produced something that isn't a PDF"));
    }
    Ok(bytes)
}

/// A PDF that has been written to the end (its last line is `%%EOF`).
fn is_whole_pdf(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF") && bytes.trim_ascii_end().ends_with(b"%%EOF")
}

/// Where a render's files go. A browser installed as a Linux snap can't see
/// the system's temporary folder, only its own folder in ~/snap.
fn work_root(browser: &Path) -> PathBuf {
    let snap = browser
        .strip_prefix("/snap/bin")
        .ok()
        .and_then(|name| name.to_str())
        .map(|name| name.split('.').next().unwrap_or(name).to_string());
    match (snap, dirs::home_dir()) {
        (Some(name), Some(home)) => home.join("snap").join(name).join("common"),
        _ => std::env::temp_dir(),
    }
}

fn render_with(browser: &Path, html: &str) -> Result<Vec<u8>> {
    let dir = work_root(browser).join(format!("cairn-pdf-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&dir)?;
    let result = std::fs::write(dir.join("page.html"), html)
        .map_err(CairnError::from)
        .and_then(|()| run_browser(browser, &dir));
    // The browser can hold its profile files for a moment after exiting.
    for _ in 0..10 {
        if std::fs::remove_dir_all(&dir).is_ok() || !dir.exists() {
            break;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    result
}

/// Render one self-contained HTML document to PDF bytes, trying each
/// browser in turn (the one that worked last time first).
pub fn render_pdf(browsers: &[PathBuf], html: &str) -> Result<Vec<u8>> {
    let _guard = RENDER_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let last = LAST_GOOD.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut ordered: Vec<&PathBuf> = browsers
        .iter()
        .filter(|b| Some(*b) == last.as_ref())
        .collect();
    ordered.extend(browsers.iter().filter(|b| Some(*b) != last.as_ref()));
    for browser in ordered {
        if let Ok(bytes) = render_with(browser, html) {
            *LAST_GOOD.lock().unwrap_or_else(|e| e.into_inner()) = Some(browser.clone());
            return Ok(bytes);
        }
    }
    // Nothing worked: the browser's print window is still a way to a PDF.
    Err(unavailable())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_url_encodes_spaces_and_keeps_drive() {
        let url = file_url(Path::new(r"C:\Users\A B\page.html"));
        assert_eq!(url, "file:///C:/Users/A%20B/page.html");
    }

    #[test]
    fn a_pdf_counts_as_written_only_once_it_ends() {
        assert!(is_whole_pdf(b"%PDF-1.4\n...\n%%EOF\n"));
        assert!(!is_whole_pdf(b"%PDF-1.4\n...half"));
        assert!(!is_whole_pdf(b"<html>%%EOF"));
    }

    #[test]
    fn a_mac_app_means_the_program_inside_it() {
        assert_eq!(
            app_executable(Path::new("/Applications/Google Chrome.app")),
            Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
        );
        assert_eq!(
            app_executable(Path::new("/usr/bin/chromium")),
            Path::new("/usr/bin/chromium")
        );
    }

    #[test]
    fn snap_browsers_work_in_their_own_folder() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            work_root(Path::new("/snap/bin/chromium")),
            home.join("snap/chromium/common")
        );
        assert_eq!(
            work_root(Path::new("/usr/bin/chromium")),
            std::env::temp_dir()
        );
    }

    #[test]
    fn missing_configured_browser_is_not_replaced() {
        assert!(find_browsers(Some(Path::new(r"Z:\nope\msedge.exe"))).is_empty());
    }
}
