//! Finding, checking, and installing new versions of Cairn.
//!
//! 1. Ask GitHub for the latest release (a small JSON request). Nothing
//!    about the person or their documentation is sent.
//! 2. Download this system's file (see [`Platform`]) and its `.minisig`
//!    signature, and check the signature against the public key built into
//!    this program. Only files signed by the release workflow pass.
//! 3. Take the program out of it (from the zip on Windows and macOS; on
//!    Linux the download is the program), and ask it for its version to
//!    make sure it runs and is the expected, newer version.
//! 4. Keep the current program as `cairn.previous` (to go back), then put
//!    the new one in its place. Windows allows renaming a running program;
//!    elsewhere the new file is renamed over the old one, and the running
//!    program keeps the old file until it restarts.
//!
//! A copy of Cairn on a network drive, in a folder the person can't change,
//! or run from inside a zip doesn't replace itself; see [`install_blocker`].

use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use semver::Version;
use serde::{Deserialize, Serialize};

use crate::error::{CairnError, Result};
use crate::fsutil::{can_write_dir, write_atomic};

/// Where the latest release is described.
pub const RELEASES_API: &str = "https://api.github.com/repos/thekovie/cairn/releases/latest";
/// The page people can download from by hand.
pub const RELEASES_PAGE: &str = "https://github.com/thekovie/cairn/releases/latest";
/// Public half of the key the release workflow signs downloads with
/// (made by `cargo run --example release_sign -- keygen`).
pub const PUBLIC_KEY: &str = "RWQlB4K4jmDHW+uKZF2cUKQtuiQjEUC1kohGcVNbczB/Ej6IbyD2lCCm";
/// The program kept from before the last update, for going back.
pub const PREVIOUS_EXE: &str = if cfg!(windows) {
    "cairn.previous.exe"
} else {
    "cairn.previous"
};
const PREVIOUS_VERSION: &str = "cairn.previous.version";
/// Ending of program files: kept beside the program, so they look alike.
const EXE_SUFFIX: &str = std::env::consts::EXE_SUFFIX;

/// How a release brings the program to this kind of computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// `cairn-<v>-windows-x64.zip`, holding `cairn.exe`.
    Windows,
    /// `cairn-<v>-macos-universal.zip`, holding `Cairn.app` (whose
    /// `Contents/MacOS/cairn` is the program).
    Mac,
    /// `cairn-<v>-linux-x86_64.AppImage`: the AppImage file is the program.
    LinuxAppImage,
    /// `cairn-<v>-linux-x86_64`: the program itself, for copies unpacked
    /// from the `.tar.gz`.
    Linux,
}

impl Platform {
    /// This computer, or `None` where no release is built for it.
    pub fn current() -> Option<Platform> {
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => Some(Platform::Windows),
            ("macos", "x86_64" | "aarch64") => Some(Platform::Mac),
            ("linux", "x86_64") if appimage_path().is_some() => Some(Platform::LinuxAppImage),
            ("linux", "x86_64") => Some(Platform::Linux),
            _ => None,
        }
    }

    /// The release file for `version`.
    pub fn asset_name(self, version: &Version) -> String {
        match self {
            Platform::Windows => format!("cairn-{version}-windows-x64.zip"),
            Platform::Mac => format!("cairn-{version}-macos-universal.zip"),
            Platform::LinuxAppImage => format!("cairn-{version}-linux-x86_64.AppImage"),
            Platform::Linux => format!("cairn-{version}-linux-x86_64"),
        }
    }

    /// The program's file name inside the zip, or `None` when the download
    /// is the program itself.
    pub fn program_in_zip(self) -> Option<&'static str> {
        match self {
            Platform::Windows => Some("cairn.exe"),
            Platform::Mac => Some("cairn"),
            Platform::LinuxAppImage | Platform::Linux => None,
        }
    }
}

/// The AppImage file this program was started from, if it was: the running
/// program is then a read-only copy inside it, and the AppImage file is
/// what gets updated.
fn appimage_path() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
}

/// The program file an update replaces: the AppImage when started from one,
/// otherwise this program.
pub fn program_path() -> Result<PathBuf> {
    match appimage_path() {
        Some(path) => Ok(path),
        None => std::env::current_exe()
            .map_err(|_| CairnError::Io("Cairn's program file couldn't be found.".into())),
    }
}

const MAX_JSON_BYTES: u64 = 2 * 1024 * 1024;
const MAX_ZIP_BYTES: u64 = 300 * 1024 * 1024;
const MAX_EXE_BYTES: u64 = 200 * 1024 * 1024;
const MAX_SIG_BYTES: u64 = 64 * 1024;
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const VERSION_TIMEOUT: Duration = Duration::from_secs(20);

/// Where to look for updates and which key they must be signed with.
#[derive(Debug, Clone)]
pub struct UpdateSource {
    pub api_url: String,
    pub public_key: String,
    /// Allow plain `http://` addresses (only for tests on this computer).
    pub allow_http: bool,
    /// Proxy chosen in Settings (`host:port`). `None` uses Windows' proxy
    /// settings.
    pub proxy: Option<String>,
}

impl UpdateSource {
    pub fn with_proxy(proxy: Option<String>) -> Self {
        UpdateSource {
            proxy,
            ..UpdateSource::default()
        }
    }
}

/// A proxy typed in Settings, tidied to `host:port`. `http://` in front and
/// a `/` at the end are accepted, since that's how proxies are often written.
pub fn normalize_proxy(input: &str) -> Result<String> {
    let bad = || {
        CairnError::BadRequest(
            "Type the proxy as address:port, for example proxy.office.local:8080.".into(),
        )
    };
    let s = input.trim();
    let s = match s.get(..7) {
        Some(scheme) if scheme.eq_ignore_ascii_case("http://") => &s[7..],
        _ => s,
    };
    let s = s.trim_end_matches('/');
    let (host, port) = s.rsplit_once(':').ok_or_else(bad)?;
    let port: u16 = port.parse().ok().filter(|p| *p != 0).ok_or_else(bad)?;
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if !host_ok {
        return Err(bad());
    }
    Ok(format!("{host}:{port}"))
}

impl Default for UpdateSource {
    /// GitHub and the built-in key. Debug builds can point elsewhere with
    /// `CAIRN_UPDATE_API` and `CAIRN_UPDATE_KEY`, to try updates locally.
    fn default() -> Self {
        let mut source = UpdateSource {
            api_url: RELEASES_API.into(),
            public_key: PUBLIC_KEY.into(),
            allow_http: false,
            proxy: None,
        };
        if cfg!(debug_assertions) {
            if let Ok(url) = std::env::var("CAIRN_UPDATE_API") {
                source.allow_http = url.starts_with("http://127.0.0.1");
                source.api_url = url;
            }
            if let Ok(key) = std::env::var("CAIRN_UPDATE_KEY") {
                source.public_key = key;
            }
        }
        source
    }
}

/// A newer version that can be installed.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Release {
    pub version: String,
    /// Release notes (Markdown).
    pub notes: String,
    pub page_url: String,
    pub published_at: Option<String>,
    /// This computer's download; empty where none is built.
    #[serde(skip)]
    pub asset_name: String,
    #[serde(skip)]
    pub asset_url: String,
    #[serde(skip)]
    pub sig_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// This computer's download for a version, e.g. `cairn-0.5.0-windows-x64.zip`,
/// or `None` where no release is built for it.
pub fn asset_name(version: &Version) -> Option<String> {
    Platform::current().map(|p| p.asset_name(version))
}

/// This program's version.
pub fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is valid")
}

fn unreadable() -> CairnError {
    CairnError::Io("The information about new versions couldn't be read. Try again later.".into())
}

/// The release described by GitHub's JSON, if it is newer than `current`.
pub fn parse_release(json: &[u8], current: &Version) -> Result<Option<Release>> {
    let gh: GhRelease = serde_json::from_slice(json).map_err(|_| unreadable())?;
    let version = Version::parse(gh.tag_name.trim_start_matches('v')).map_err(|_| unreadable())?;
    if gh.draft || gh.prerelease || !version.pre.is_empty() || version <= *current {
        return Ok(None);
    }
    let url_of = |name: &str| {
        gh.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
    };
    // On a computer releases aren't built for, say a new version is out
    // anyway; `install_blocker` explains that it's installed by hand.
    let (asset, asset_url, sig_url) = match asset_name(&version) {
        None => (String::new(), String::new(), String::new()),
        Some(asset) => {
            let sig = format!("{asset}.minisig");
            let (Some(asset_url), Some(sig_url)) = (url_of(&asset), url_of(&sig)) else {
                return Err(CairnError::Io(format!(
                    "Cairn {version} is out, but its download isn't ready yet. Try again later."
                )));
            };
            (asset, asset_url, sig_url)
        }
    };
    Ok(Some(Release {
        version: version.to_string(),
        notes: gh.body.unwrap_or_default(),
        page_url: if gh.html_url.is_empty() {
            RELEASES_PAGE.into()
        } else {
            gh.html_url
        },
        published_at: gh.published_at,
        asset_name: asset,
        asset_url,
        sig_url,
    }))
}

fn agent(timeout: Duration, proxy: Option<&str>) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    // The proxy chosen in Settings, or else Windows' proxy settings; and the
    // Windows certificate store, so offices with their own certificates work.
    let proxy = match proxy {
        Some(p) => ureq::Proxy::new(&format!("http://{p}")).ok(),
        None => ureq::Proxy::try_from_env(),
    };
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(format!("Cairn/{}", env!("CARGO_PKG_VERSION")))
        .proxy(proxy)
        .tls_config(
            TlsConfig::builder()
                .root_certs(RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn fetch(source: &UpdateSource, url: &str, limit: u64, timeout: Duration) -> Result<Vec<u8>> {
    if !url.starts_with("https://") && !(source.allow_http && url.starts_with("http://")) {
        return Err(CairnError::Io(
            "The update address isn't secure (https), so it wasn't used.".into(),
        ));
    }
    let proxy = source.proxy.as_deref();
    let response = agent(timeout, proxy)
        .get(url)
        .header(
            "Accept",
            "application/vnd.github+json, application/octet-stream",
        )
        .call();
    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(407)) => {
            return Err(CairnError::Io(
                "The proxy asked for a login, which Cairn can't give. Ask your IT team for a \
                 proxy that GitHub can be reached through without one."
                    .into(),
            ));
        }
        Err(e) if proxy.is_some() && !matches!(e, ureq::Error::StatusCode(_)) => {
            return Err(CairnError::Io(format!(
                "Cairn couldn't reach GitHub through the proxy {} ({e}). Check the proxy \
                 address in Settings → Updates.",
                proxy.unwrap_or_default()
            )));
        }
        Err(ureq::Error::StatusCode(404)) => {
            return Err(CairnError::NotFound(
                "No released version of Cairn was found.".into(),
            ));
        }
        Err(ureq::Error::StatusCode(code)) => {
            return Err(CairnError::Io(format!(
                "GitHub didn't answer normally (error {code}). Try again later."
            )));
        }
        Err(ureq::Error::Timeout(_)) => {
            return Err(CairnError::Io(
                "Checking took too long. The internet connection may be slow or blocked.".into(),
            ));
        }
        Err(e) => {
            return Err(CairnError::Io(format!(
                "Cairn couldn't reach GitHub ({e}). Check the internet connection."
            )));
        }
    };
    response
        .body_mut()
        .with_config()
        .limit(limit)
        .read_to_vec()
        .map_err(|e| CairnError::Io(format!("The download didn't finish ({e}). Try again.")))
}

/// Ask for the latest release. `Ok(None)` means this is the latest version.
pub fn check(source: &UpdateSource, current: &Version) -> Result<Option<Release>> {
    let json = fetch(source, &source.api_url, MAX_JSON_BYTES, CHECK_TIMEOUT)?;
    parse_release(&json, current)
}

fn not_trusted() -> CairnError {
    CairnError::Io(
        "The download didn't pass Cairn's safety check (it isn't signed by Cairn's makers), so \
         nothing was installed."
            .into(),
    )
}

/// Check that `data` (the file called `file_name`) was signed with the key.
pub fn verify_signature(
    public_key: &str,
    data: &[u8],
    signature: &[u8],
    file_name: &str,
) -> Result<()> {
    let key = minisign_verify::PublicKey::from_base64(public_key).map_err(|_| not_trusted())?;
    let text = std::str::from_utf8(signature).map_err(|_| not_trusted())?;
    let sig = minisign_verify::Signature::decode(text).map_err(|_| not_trusted())?;
    key.verify(data, &sig, false).map_err(|_| not_trusted())?;
    // The signed comment names the file, so a validly signed file can't be
    // passed off as a different one.
    if sig.trusted_comment() != format!("file:{file_name}") {
        return Err(not_trusted());
    }
    Ok(())
}

/// The program called `program` inside a release zip: at most three
/// folders down, as in `Cairn.app/Contents/MacOS/cairn`. The name must
/// match exactly off Windows, where `Cairn` and `cairn` are different files.
pub fn extract_program(zip: &[u8], program: &str) -> Result<Vec<u8>> {
    let bad = || CairnError::Io("The download doesn't contain Cairn. Try again later.".into());
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|_| bad())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|_| bad())?;
        let name = file.name().replace('\\', "/");
        let depth = name.matches('/').count();
        let is_program = name.rsplit('/').next().is_some_and(|n| {
            if cfg!(windows) {
                n.eq_ignore_ascii_case(program)
            } else {
                n == program
            }
        });
        if !is_program || depth > 3 || file.is_dir() || file.size() > MAX_EXE_BYTES {
            continue;
        }
        let mut out = Vec::new();
        file.by_ref()
            .take(MAX_EXE_BYTES)
            .read_to_end(&mut out)
            .map_err(|_| bad())?;
        return Ok(out);
    }
    Err(bad())
}

/// Download a release, check it, and save its program in `dest_dir` as
/// `cairn-<version>.new` (`.new.exe` on Windows). Returns that path.
pub fn download(source: &UpdateSource, release: &Release, dest_dir: &Path) -> Result<PathBuf> {
    let Some(platform) = Platform::current().filter(|_| !release.asset_url.is_empty()) else {
        return Err(CairnError::Io(format!(
            "There's no automatic update for this kind of computer. Download Cairn {} from {}.",
            release.version, release.page_url
        )));
    };
    let file = fetch(source, &release.asset_url, MAX_ZIP_BYTES, DOWNLOAD_TIMEOUT)?;
    let sig = fetch(source, &release.sig_url, MAX_SIG_BYTES, CHECK_TIMEOUT)?;
    verify_signature(&source.public_key, &file, &sig, &release.asset_name)?;
    let program = match platform.program_in_zip() {
        Some(name) => extract_program(&file, name)?,
        None => file,
    };
    let path = dest_dir.join(format!("cairn-{}.new{EXE_SUFFIX}", release.version));
    write_atomic(&path, &program)?;
    make_runnable(&path)?;
    Ok(path)
}

/// Let a program file run (a written file can't, off Windows).
fn make_runnable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    let _ = path;
    Ok(())
}

/// The version a Cairn program reports (`cairn.exe --version`).
pub fn program_version(exe: &Path) -> Result<Version> {
    let broken =
        || CairnError::Io("The new version of Cairn didn't start, so it wasn't installed.".into());
    let mut command = Command::new(exe);
    command
        .arg("--version")
        // An AppImage can then run without FUSE, which not every Linux has.
        .env("APPIMAGE_EXTRACT_AND_RUN", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|_| broken())?;
    let started = Instant::now();
    loop {
        match child.try_wait().map_err(|_| broken())? {
            Some(_) => break,
            None if started.elapsed() > VERSION_TIMEOUT => {
                let _ = child.kill();
                return Err(broken());
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
    let mut out = String::new();
    child
        .stdout
        .take()
        .ok_or_else(broken)?
        .read_to_string(&mut out)
        .map_err(|_| broken())?;
    out.trim()
        .strip_prefix("cairn ")
        .and_then(|v| Version::parse(v.trim()).ok())
        .ok_or_else(broken)
}

/// Why the Cairn at `exe` can't replace itself, if it can't. It still says
/// when a new version is out; someone installs it by hand.
pub fn install_blocker(exe: &Path) -> Option<String> {
    let shown = crate::paths::display_path(exe);
    let lower = shown.to_lowercase();
    if lower.starts_with(r"\\") || is_remote_drive(exe) {
        return Some(format!(
            "Cairn is running from a network drive ({shown}), so it doesn't update itself: that \
             would change it for everyone who uses it there. Whoever looks after that folder can \
             put the new version in place."
        ));
    }
    if lower.contains(r"\appdata\local\temp\") {
        return Some(
            "Cairn is running from inside a zip file. Unzip it first (or use the installer), and \
             then it can update itself."
                .into(),
        );
    }
    // A Mac runs an app straight from Downloads from a hidden, read-only copy.
    if lower.contains("/apptranslocation/") {
        return Some(
            "Cairn is running from your Downloads in a way that can't be changed. Move Cairn to \
             your Applications folder, open it from there, and then it can update itself."
                .into(),
        );
    }
    if Platform::current().is_none() {
        return Some(format!(
            "Cairn doesn't update itself on this kind of computer. Download new versions from \
             {RELEASES_PAGE}."
        ));
    }
    let dir = exe.parent()?;
    if !can_write_dir(dir) {
        let advice = if cfg!(windows) {
            "Install Cairn with the installer, which puts it in a folder of your own"
        } else if cfg!(target_os = "macos") {
            "Move Cairn to your Applications folder (or another folder of your own)"
        } else {
            "Put Cairn in a folder of your own, such as your home folder"
        };
        return Some(format!(
            "You don't have permission to change the folder Cairn is in ({}). {advice}, or ask \
             whoever looks after this computer.",
            crate::paths::display_path(dir)
        ));
    }
    None
}

#[cfg(windows)]
fn is_remote_drive(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
    const DRIVE_REMOTE: u32 = 4;
    let text = crate::paths::display_path(path);
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[1] != b':' || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let root: Vec<u16> = std::ffi::OsStr::new(&format!("{}:\\", bytes[0] as char))
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: `root` is a NUL-terminated wide string that outlives the call.
    unsafe { GetDriveTypeW(root.as_ptr()) == DRIVE_REMOTE }
}

#[cfg(not(windows))]
fn is_remote_drive(_path: &Path) -> bool {
    false
}

/// Replace `target` with the program at `new_exe`. When `target` is this
/// running program, it is swapped out safely (it can't simply be
/// overwritten on Windows).
#[cfg(windows)]
fn replace(target: &Path, new_exe: &Path) -> Result<()> {
    let running = std::env::current_exe()
        .and_then(fs::canonicalize)
        .ok()
        .zip(fs::canonicalize(target).ok())
        .is_some_and(|(a, b)| a == b);
    let result = if running {
        self_replace::self_replace(new_exe)
    } else {
        fs::copy(new_exe, target).map(|_| ())
    };
    result.map_err(|e| {
        CairnError::Io(format!(
            "Cairn couldn't replace its program file ({e}). Nothing was changed."
        ))
    })
}

/// Replace `target` with a copy of `new_exe`, renamed into place in one
/// step. A running program (or a mounted AppImage) keeps reading its old
/// file, which stays until it stops; writing over it would break it.
#[cfg(not(windows))]
fn replace(target: &Path, new_exe: &Path) -> Result<()> {
    let failed = |e: std::io::Error| {
        CairnError::Io(format!(
            "Cairn couldn't replace its program file ({e}). Nothing was changed."
        ))
    };
    let dir = dir_of(target)?;
    let temp = dir.join(format!(".cairn-replace-{}", uuid::Uuid::new_v4().simple()));
    fs::copy(new_exe, &temp).map_err(failed)?;
    make_runnable(&temp)?;
    fs::rename(&temp, target).map_err(|e| {
        let _ = fs::remove_file(&temp);
        failed(e)
    })
}

fn dir_of(exe: &Path) -> Result<&Path> {
    exe.parent()
        .ok_or_else(|| CairnError::Io("Cairn's folder couldn't be found.".into()))
}

/// Keep `target` as `cairn.previous.exe`, then put `new_exe` in its place.
pub fn install(target: &Path, new_exe: &Path, current: &Version) -> Result<()> {
    let dir = dir_of(target)?;
    fs::copy(target, dir.join(PREVIOUS_EXE))?;
    write_atomic(&dir.join(PREVIOUS_VERSION), current.to_string().as_bytes())?;
    let result = replace(target, new_exe);
    let _ = fs::remove_file(new_exe);
    result
}

/// The version kept by the last update, if it can be gone back to.
pub fn previous_version(target: &Path) -> Option<String> {
    let dir = target.parent()?;
    if !dir.join(PREVIOUS_EXE).is_file() {
        return None;
    }
    let text = fs::read_to_string(dir.join(PREVIOUS_VERSION)).ok()?;
    Version::parse(text.trim()).ok().map(|v| v.to_string())
}

/// Go back to the version kept by the last update. The current one is kept
/// in its place, so going back can itself be undone. Returns the version
/// now installed.
pub fn rollback(target: &Path, current: &Version) -> Result<String> {
    let dir = dir_of(target)?;
    let previous = previous_version(target).ok_or_else(|| {
        CairnError::NotFound("There is no earlier version of Cairn to go back to.".into())
    })?;
    let kept = dir.join(PREVIOUS_EXE);
    let swap = dir.join(format!("cairn-{current}.swap{EXE_SUFFIX}"));
    fs::copy(target, &swap)?;
    if let Err(e) = replace(target, &kept) {
        let _ = fs::remove_file(&swap);
        return Err(e);
    }
    fs::rename(&swap, &kept)?;
    write_atomic(&dir.join(PREVIOUS_VERSION), current.to_string().as_bytes())?;
    Ok(previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str, assets: &[&str]) -> Vec<u8> {
        let assets: Vec<_> = assets
            .iter()
            .map(|n| {
                serde_json::json!({ "name": n, "browser_download_url": format!("https://x.test/{n}") })
            })
            .collect();
        serde_json::to_vec(&serde_json::json!({
            "tag_name": tag, "body": "## Notes", "html_url": "https://x.test/r",
            "draft": false, "prerelease": false, "assets": assets,
        }))
        .unwrap()
    }

    /// This computer's download for 0.5.0 and its signature.
    fn this_computers_files() -> [String; 2] {
        let asset = asset_name(&Version::parse("0.5.0").unwrap()).expect("a supported computer");
        [asset.clone(), format!("{asset}.minisig")]
    }

    #[test]
    fn only_newer_stable_releases_count() {
        let current = Version::parse("0.4.0").unwrap();
        let [asset, sig] = this_computers_files();
        let files = [asset.as_str(), sig.as_str()];
        let r = parse_release(&release_json("v0.5.0", &files), &current)
            .unwrap()
            .unwrap();
        assert_eq!(r.version, "0.5.0");
        assert_eq!(r.asset_url, format!("https://x.test/{asset}"));
        assert!(
            parse_release(&release_json("v0.4.0", &files), &current)
                .unwrap()
                .is_none()
        );
        assert!(
            parse_release(&release_json("v0.3.9", &files), &current)
                .unwrap()
                .is_none()
        );
        assert!(
            parse_release(&release_json("v0.6.0-beta.1", &files), &current)
                .unwrap()
                .is_none()
        );
        // Newer, but the signature isn't uploaded yet.
        assert!(parse_release(&release_json("v0.5.0", &files[..1]), &current).is_err());
        assert!(parse_release(b"not json", &current).is_err());
    }

    #[test]
    fn proxies_are_tidied_to_host_and_port() {
        assert_eq!(
            normalize_proxy(" proxy.office.local:8080 ").unwrap(),
            "proxy.office.local:8080"
        );
        assert_eq!(
            normalize_proxy("HTTP://10.0.0.5:3128/").unwrap(),
            "10.0.0.5:3128"
        );
        for bad in [
            "",
            "proxy.office.local",
            "proxy:0",
            "proxy:99999",
            ":8080",
            "https://proxy:8080",
            "user@proxy:8080",
            "a b:80",
        ] {
            assert!(normalize_proxy(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn each_computer_has_its_own_download() {
        let v = Version::parse("1.2.3").unwrap();
        let names = [
            Platform::Windows,
            Platform::Mac,
            Platform::LinuxAppImage,
            Platform::Linux,
        ]
        .map(|p| p.asset_name(&v));
        assert_eq!(
            names,
            [
                "cairn-1.2.3-windows-x64.zip",
                "cairn-1.2.3-macos-universal.zip",
                "cairn-1.2.3-linux-x86_64.AppImage",
                "cairn-1.2.3-linux-x86_64",
            ]
        );
        // Windows keeps the name 0.9 copies look for.
        assert_eq!(Platform::Windows.program_in_zip(), Some("cairn.exe"));
    }

    #[test]
    fn a_mac_app_run_from_downloads_does_not_self_update() {
        let reason = install_blocker(Path::new(
            "/private/var/folders/x/T/AppTranslocation/ABC/d/Cairn.app/Contents/MacOS/cairn",
        ))
        .unwrap();
        assert!(reason.contains("Applications folder"));
    }

    #[test]
    fn network_paths_and_zips_do_not_self_update() {
        let reason = install_blocker(Path::new(r"\\server\share\Cairn\cairn.exe")).unwrap();
        assert!(reason.contains("network drive"));
        let reason = install_blocker(Path::new(
            r"C:\Users\x\AppData\Local\Temp\Temp1_cairn.zip\cairn.exe",
        ))
        .unwrap();
        assert!(reason.contains("zip"));
    }
}
