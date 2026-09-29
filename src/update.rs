//! Finding, checking, and installing new versions of Cairn.
//!
//! 1. Ask GitHub for the latest release (a small JSON request). Nothing
//!    about the person or their documentation is sent.
//! 2. Download `cairn-<version>-windows-x64.zip` and its `.minisig`
//!    signature, and check the signature against the public key built into
//!    this program. Only files signed by the release workflow pass.
//! 3. Take `cairn.exe` out of the zip, and ask it for its version to make
//!    sure it runs and is the expected, newer version.
//! 4. Keep the current program as `cairn.previous.exe` (to go back), then
//!    put the new one in its place (Windows allows renaming a running
//!    program, which is how it can be replaced while running).
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
pub const PREVIOUS_EXE: &str = "cairn.previous.exe";
const PREVIOUS_VERSION: &str = "cairn.previous.version";

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
}

impl Default for UpdateSource {
    /// GitHub and the built-in key. Debug builds can point elsewhere with
    /// `CAIRN_UPDATE_API` and `CAIRN_UPDATE_KEY`, to try updates locally.
    fn default() -> Self {
        let mut source = UpdateSource {
            api_url: RELEASES_API.into(),
            public_key: PUBLIC_KEY.into(),
            allow_http: false,
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
    #[serde(skip)]
    pub zip_name: String,
    #[serde(skip)]
    pub zip_url: String,
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

/// The download for a version: `cairn-0.5.0-windows-x64.zip`.
pub fn zip_name(version: &Version) -> String {
    format!("cairn-{version}-windows-x64.zip")
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
    let zip = zip_name(&version);
    let sig = format!("{zip}.minisig");
    let url_of = |name: &str| {
        gh.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.browser_download_url.clone())
    };
    let (Some(zip_url), Some(sig_url)) = (url_of(&zip), url_of(&sig)) else {
        return Err(CairnError::Io(format!(
            "Cairn {version} is out, but its download isn't ready yet. Try again later."
        )));
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
        zip_name: zip,
        zip_url,
        sig_url,
    }))
}

fn agent(timeout: Duration) -> ureq::Agent {
    use ureq::tls::{RootCerts, TlsConfig};
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(format!("Cairn/{}", env!("CARGO_PKG_VERSION")))
        // The Windows proxy settings and certificate store, so offices with
        // a proxy or their own certificates work.
        .proxy(ureq::Proxy::try_from_env())
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
    let response = agent(timeout)
        .get(url)
        .header(
            "Accept",
            "application/vnd.github+json, application/octet-stream",
        )
        .call();
    let mut response = match response {
        Ok(r) => r,
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

/// The `cairn.exe` inside a release zip.
pub fn extract_exe(zip: &[u8]) -> Result<Vec<u8>> {
    let bad = || CairnError::Io("The download doesn't contain Cairn. Try again later.".into());
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|_| bad())?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|_| bad())?;
        let name = file.name().replace('\\', "/");
        let depth = name.matches('/').count();
        let is_exe = name
            .rsplit('/')
            .next()
            .is_some_and(|n| n.eq_ignore_ascii_case("cairn.exe"));
        if !is_exe || depth > 1 || file.size() > MAX_EXE_BYTES {
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
/// `cairn-<version>.new.exe`. Returns that path.
pub fn download(source: &UpdateSource, release: &Release, dest_dir: &Path) -> Result<PathBuf> {
    let zip = fetch(source, &release.zip_url, MAX_ZIP_BYTES, DOWNLOAD_TIMEOUT)?;
    let sig = fetch(source, &release.sig_url, MAX_SIG_BYTES, CHECK_TIMEOUT)?;
    verify_signature(&source.public_key, &zip, &sig, &release.zip_name)?;
    let exe = extract_exe(&zip)?;
    let path = dest_dir.join(format!("cairn-{}.new.exe", release.version));
    write_atomic(&path, &exe)?;
    Ok(path)
}

/// The version a Cairn program reports (`cairn.exe --version`).
pub fn program_version(exe: &Path) -> Result<Version> {
    let broken =
        || CairnError::Io("The new version of Cairn didn't start, so it wasn't installed.".into());
    let mut command = Command::new(exe);
    command
        .arg("--version")
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
    let dir = exe.parent()?;
    if !can_write_dir(dir) {
        return Some(format!(
            "You don't have permission to change the folder Cairn is in ({}). Install Cairn with \
             the installer, which puts it in a folder of your own, or ask whoever looks after \
             this computer.",
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
    let swap = dir.join(format!("cairn-{current}.swap.exe"));
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

    #[test]
    fn only_newer_stable_releases_count() {
        let current = Version::parse("0.4.0").unwrap();
        let files = [
            "cairn-0.5.0-windows-x64.zip",
            "cairn-0.5.0-windows-x64.zip.minisig",
        ];
        let r = parse_release(&release_json("v0.5.0", &files), &current)
            .unwrap()
            .unwrap();
        assert_eq!(r.version, "0.5.0");
        assert_eq!(r.zip_url, "https://x.test/cairn-0.5.0-windows-x64.zip");
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
