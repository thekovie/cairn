//! Workspace backups: one zip that can bring a documentation folder back.
//!
//! A backup holds the pages, pictures and templates, the workspace marker,
//! and Cairn's own records under `_system` (earlier versions, who edited
//! what, deleted pages). It leaves out edit locks and scratch files. It also
//! holds `cairn-backup.json`, a contents list with a SHA-256 for every file,
//! so a copy can be checked later and a damaged one is refused.
//!
//! A backup is checked for changes, not snapshotted: the shared folder is
//! never locked. Each file's size and modified time are noted before it is
//! read and compared again at the end. If something changed, the backup is
//! made again once; if it keeps changing, the backup fails and names the
//! files, rather than hand over an archive that may mix versions.
//!
//! Restoring only ever writes into a new or empty folder, and checks every
//! path and checksum before it can touch the disk outside that folder.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::error::{CairnError, Result};
use crate::export::archive::{Enumeration, count, enumerate_backup, name_list, read_problem};
use crate::fsutil::{available_space, human_size};
use crate::locks::LOCKS_DIR;
use crate::paths::{Root, SYSTEM_DIR, split_relative};
use crate::workspace::{MARKER_FILE, Marker, MarkerStatus, parse_marker, read_marker};

/// The contents list inside every backup.
pub const MANIFEST_NAME: &str = "cairn-backup.json";
pub const BACKUP_FORMAT: &str = "cairn-backup";
/// Raised when the layout changes in a way older versions can't read.
pub const BACKUP_FORMAT_VERSION: u32 = 1;

/// A backup is started again at most this many times if a file keeps
/// changing while it is being copied.
const MAX_PASSES: u32 = 3;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MARKER_BYTES: u64 = 1024 * 1024;
const CHUNK: usize = 1024 * 1024;
/// Room kept free beyond what a backup or restore needs, for the folder
/// entries and file system overhead.
const SPACE_MARGIN: u64 = 8 * 1024 * 1024;

/// How much free space is on the drive holding a folder (an error: unknown).
pub type SpaceFn<'a> = &'a dyn Fn(&Path) -> io::Result<u64>;
/// Files from this size up are written with zip64 headers.
const LARGE_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub instance_id: String,
    pub display_name: String,
    pub created_at: String,
    pub cairn_version: String,
    pub files: Vec<ManifestFile>,
    /// Links that were not followed, as workspace paths.
    #[serde(default)]
    pub skipped: Vec<String>,
}

/// How a backup went.
#[derive(Debug)]
pub struct Summary {
    pub files: usize,
    pub bytes: u64,
    pub skipped_links: Vec<String>,
    /// 1 normally; more when a file changed while it was being copied.
    pub passes: u32,
    /// Files someone saved, added, or removed while the backup ran. The backup
    /// holds each as it was when it was copied (and not the files created
    /// later). Each file is still a whole, unchanged version.
    pub changed_during: Vec<String>,
    /// Said when there may not have been room for the backup after all.
    pub space_note: Option<String>,
}

/// How one attempt at a backup ended.
enum PassResult {
    Cancelled,
    /// This file changed while it was being copied, or disappeared before
    /// it could be: start again with a fresh list.
    Unstable(String),
    Done(Summary),
}

/// What a restore wrote.
#[derive(Debug)]
pub struct Restored {
    pub files: usize,
    pub bytes: u64,
    pub display_name: String,
    pub instance_id: String,
}

/// Lets a long backup report progress and be cancelled.
pub struct Progress<'a> {
    pub cancelled: &'a dyn Fn() -> bool,
    /// Called after each file, with how many are done in this attempt.
    pub on_file: &'a dyn Fn(usize),
}

fn bad(msg: impl Into<String>) -> CairnError {
    CairnError::BadRequest(msg.into())
}

fn damaged(detail: impl std::fmt::Display) -> CairnError {
    bad(format!(
        "The backup file is damaged or incomplete ({detail}). Use another copy of the backup."
    ))
}

fn write_problem(err: &io::Error) -> CairnError {
    CairnError::Io(format!(
        "Cairn couldn't write the backup ({err}). Check that there is enough free space."
    ))
}

fn zip_problem(err: impl std::fmt::Display) -> CairnError {
    CairnError::Io(format!("The backup could not be packed: {err}"))
}

/// Why a path can't be stored in, or restored from, a backup.
fn check_path(rel: &str) -> std::result::Result<(), String> {
    if rel.contains('\\') {
        return Err("it contains a backslash".into());
    }
    let parts = split_relative(rel).map_err(|e| e.to_string())?;
    if parts.is_empty() {
        return Err("it is empty".into());
    }
    let lower = rel.to_lowercase();
    if lower == MANIFEST_NAME {
        return Err("that name is reserved for the backup's contents list".into());
    }
    let locks = format!("{SYSTEM_DIR}/{LOCKS_DIR}").to_lowercase();
    if lower == locks || lower.starts_with(&format!("{locks}/")) {
        return Err("edit locks are never part of a backup".into());
    }
    Ok(())
}

fn now_utc() -> String {
    let now = jiff::Timestamp::now();
    jiff::Timestamp::from_second(now.as_second())
        .map_or_else(|_| now.to_string(), |t| t.to_string())
}

// ---------------------------------------------------------------- creating

/// Back up the whole workspace into a new file at `dest`.
pub fn create_backup(root: &Root, dest: &Path) -> Result<Summary> {
    let first = enumerate_backup(root)?;
    let never = || false;
    let nothing = |_done: usize| {};
    let progress = Progress {
        cancelled: &never,
        on_file: &nothing,
    };
    create(root, dest, first, &progress)?
        .ok_or_else(|| CairnError::Io("The backup was cancelled.".into()))
}

/// Back up the files in `first` (from [`enumerate_backup`]) into a new file
/// at `dest`. Returns `None` if cancelled. On any failure, or cancel, nothing
/// is left at `dest` or beside it.
pub fn create(
    root: &Root,
    dest: &Path,
    first: Enumeration,
    progress: &Progress,
) -> Result<Option<Summary>> {
    create_checked(root, dest, first, progress, &available_space)
}

/// The room a folder's backup is likely to need, as (least, most): files that
/// are already compressed (pictures, PDFs, archives, office files) stay their
/// size, other files shrink a lot but by an amount no one can know in advance.
fn backup_estimate(listing: &Enumeration) -> (u64, u64) {
    const SQUEEZED: [&str; 18] = [
        "png", "jpg", "jpeg", "gif", "webp", "zip", "gz", "7z", "rar", "pdf", "docx", "xlsx",
        "pptx", "mp3", "mp4", "mov", "mkv", "avi",
    ];
    let (mut least, mut most) = (SPACE_MARGIN, SPACE_MARGIN);
    for entry in &listing.entries {
        let len = entry.size();
        let ext = entry
            .rel
            .rsplit_once('.')
            .map_or("", |(_, e)| e)
            .to_ascii_lowercase();
        most += len;
        least += if SQUEEZED.contains(&ext.as_str()) {
            len
        } else {
            len / 10
        };
    }
    (least, most)
}

fn not_enough_space(what: &str, need: u64, free: u64, folder: &Path) -> CairnError {
    CairnError::Io(format!(
        "There isn't enough free space. {what} needs about {}, but {} has only {} free. \
         Free up space or choose another drive, then try again.",
        human_size(need),
        folder.display(),
        human_size(free)
    ))
}

/// Refuse a backup that certainly won't fit, and note one that might not.
fn check_backup_space(
    dest: &Path,
    listing: &Enumeration,
    space: SpaceFn,
) -> Result<Option<String>> {
    let folder = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    // If the amount can't be found out, go ahead: running out of room is
    // still handled cleanly (nothing is left behind).
    let Ok(free) = space(folder) else {
        return Ok(None);
    };
    let (least, most) = backup_estimate(listing);
    if free < least {
        return Err(not_enough_space(
            "The backup (even squeezed as small as it can go)",
            least,
            free,
            folder,
        ));
    }
    if free < most {
        return Ok(Some(format!(
            "{} has {} free, and the backup may need up to {} if the files don't squeeze \
             well. If it runs out of room, it stops and leaves nothing behind.",
            folder.display(),
            human_size(free),
            human_size(most)
        )));
    }
    Ok(None)
}

/// [`create`] with the free-space lookup supplied (so it can be tested).
pub fn create_checked(
    root: &Root,
    dest: &Path,
    first: Enumeration,
    progress: &Progress,
    space: SpaceFn,
) -> Result<Option<Summary>> {
    if dest.exists() {
        return Err(bad(format!(
            "{} already exists. Choose a new file name.",
            dest.display()
        )));
    }
    let space_note = check_backup_space(dest, &first, space)?;
    let part = part_path(dest)?;
    let outcome = attempts(root, &part, first, progress);
    match outcome {
        Ok(Some(mut summary)) => {
            summary.space_note = space_note;
            if let Err(e) = fs::rename(&part, dest) {
                let _ = fs::remove_file(&part);
                return Err(write_problem(&e));
            }
            Ok(Some(summary))
        }
        other => {
            let _ = fs::remove_file(&part);
            other
        }
    }
}

/// Where the backup is written until it is complete.
fn part_path(dest: &Path) -> Result<PathBuf> {
    let name = dest
        .file_name()
        .ok_or_else(|| bad("Choose a file name for the backup."))?
        .to_string_lossy();
    let dir = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    Ok(dir.join(format!(".{name}.cairn-part-{}", &suffix[..12])))
}

fn attempts(
    root: &Root,
    part: &Path,
    first: Enumeration,
    progress: &Progress,
) -> Result<Option<Summary>> {
    let marker = match read_marker(root.path()) {
        MarkerStatus::Valid(m) => m,
        MarkerStatus::Missing => {
            return Err(CairnError::Io(format!(
                "This folder has no {MARKER_FILE}, so it isn't a documentation folder."
            )));
        }
        MarkerStatus::Invalid(why) => return Err(CairnError::Io(why)),
    };
    let mut listing = first;
    let mut unstable: Vec<String> = Vec::new();
    for pass in 1..=MAX_PASSES {
        let mut summary = match write_pass(root, &marker, part, &listing, progress)? {
            PassResult::Cancelled => return Ok(None),
            PassResult::Unstable(rel) => {
                unstable.push(rel);
                if pass == MAX_PASSES {
                    unstable.sort_by_key(|p| p.to_lowercase());
                    unstable.dedup();
                    return Err(CairnError::Conflict(format!(
                        "{} kept changing while the backup was made ({}). Cairn checks for \
                         changes but doesn't lock the shared folder. Try again when fewer \
                         people are editing.",
                        count(unstable.len(), "file"),
                        name_list(&unstable)
                    )));
                }
                let _ = fs::remove_file(part);
                listing = enumerate_backup(root)?;
                continue;
            }
            PassResult::Done(summary) => summary,
        };
        // Read the finished file back: the destination is often a network
        // share, and a backup that can't be restored (for example two names
        // that differ only in capitals) must not be reported as a success.
        verify(part).map_err(|e| {
            CairnError::Io(format!(
                "The backup was written but did not pass its own check, so it was not kept. {e}"
            ))
        })?;
        summary.passes = pass;
        summary.changed_during = changed_files(&listing, &enumerate_backup(root)?);
        return Ok(Some(summary));
    }
    Err(CairnError::Io("The backup could not be completed.".into()))
}

/// Files added, removed, or modified between two listings.
fn changed_files(before: &Enumeration, after: &Enumeration) -> Vec<String> {
    let old: HashMap<&str, _> = before
        .entries
        .iter()
        .map(|e| (e.rel.as_str(), e.stamp))
        .collect();
    let new: HashMap<&str, _> = after
        .entries
        .iter()
        .map(|e| (e.rel.as_str(), e.stamp))
        .collect();
    let mut changed: Vec<String> = old
        .iter()
        .filter(|(rel, stamp)| new.get(*rel) != Some(stamp))
        .map(|(rel, _)| (*rel).to_string())
        .chain(
            new.keys()
                .filter(|rel| !old.contains_key(*rel))
                .map(|rel| (*rel).to_string()),
        )
        .collect();
    changed.sort_by_key(|p| p.to_lowercase());
    changed
}

fn write_pass(
    root: &Root,
    marker: &Marker,
    part: &Path,
    listing: &Enumeration,
    progress: &Progress,
) -> Result<PassResult> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // Only the owner may read it: on a shared Linux computer the temporary
    // folder is open to everyone, and a backup holds all the documentation.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(part).map_err(|e| write_problem(&e))?;
    let mut writer = ZipWriter::new(file);
    let mut files = Vec::with_capacity(listing.entries.len());
    let mut buf = vec![0u8; CHUNK];
    let mut bytes = 0u64;
    for (i, entry) in listing.entries.iter().enumerate() {
        if (progress.cancelled)() {
            return Ok(PassResult::Cancelled);
        }
        check_path(&entry.rel).map_err(|why| {
            CairnError::Io(format!(
                "“{}” can't be stored in a backup: {why}. Rename or remove it, then try again.",
                entry.rel
            ))
        })?;
        let item = match add_streamed(&mut writer, root, &entry.rel, &mut buf) {
            Ok(Some(item)) => item,
            // Changed while being copied, or gone before it was reached.
            Ok(None) | Err(CairnError::NotFound(_)) => {
                return Ok(PassResult::Unstable(entry.rel.clone()));
            }
            Err(e) => return Err(e),
        };
        bytes += item.size;
        files.push(item);
        (progress.on_file)(i + 1);
    }
    let manifest = Manifest {
        format: BACKUP_FORMAT.into(),
        format_version: BACKUP_FORMAT_VERSION,
        instance_id: marker.instance_id.clone(),
        display_name: marker.display_name.clone(),
        created_at: now_utc(),
        cairn_version: env!("CARGO_PKG_VERSION").into(),
        files,
        skipped: listing.skipped_links.clone(),
    };
    let json = serde_json::to_vec_pretty(&manifest).map_err(zip_problem)?;
    writer
        .start_file(MANIFEST_NAME, zip_options())
        .map_err(zip_problem)?;
    writer.write_all(&json).map_err(|e| write_problem(&e))?;
    let file = writer.finish().map_err(zip_problem)?;
    file.sync_all().map_err(|e| write_problem(&e))?;
    Ok(PassResult::Done(Summary {
        files: manifest.files.len(),
        bytes,
        skipped_links: manifest.skipped,
        passes: 1,
        changed_during: Vec::new(),
        space_note: None,
    }))
}

fn zip_options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

/// Open a workspace file for reading, refusing anything that is not an
/// ordinary file. Someone with write access to the share could swap a file
/// for a link between Cairn's check and this open; on Unix the open itself
/// refuses links (and never waits on a pipe), and the opened file is checked.
fn open_regular(path: &Path, rel: &str) -> Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|e| read_problem(rel, &e))?;
    let is_file = file
        .metadata()
        .map_err(|e| read_problem(rel, &e))?
        .is_file();
    if !is_file {
        return Err(CairnError::Io(format!(
            "“{rel}” turned into something other than a file while the backup was made. \
             Try again."
        )));
    }
    Ok(file)
}

/// Copy one workspace file into the zip in chunks, hashing as it goes.
fn add_streamed(
    writer: &mut ZipWriter<File>,
    root: &Root,
    rel: &str,
    buf: &mut [u8],
) -> Result<Option<ManifestFile>> {
    let path = root.resolve(rel).map_err(|e| match e {
        CairnError::NotFound(_) => read_problem(rel, &io::ErrorKind::NotFound.into()),
        other => other,
    })?;
    let mut file = open_regular(&path, rel)?;
    let before = file.metadata().map_err(|e| read_problem(rel, &e))?;
    let len = before.len();
    let mut options = zip_options();
    if len >= LARGE_FILE_BYTES {
        options = options.large_file(true);
    }
    writer.start_file(rel, options).map_err(zip_problem)?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    loop {
        let n = file.read(buf).map_err(|e| read_problem(rel, &e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        writer.write_all(&buf[..n]).map_err(|e| write_problem(&e))?;
        size += n as u64;
    }
    // Written to while it was read (an editor saving in place): what was
    // copied may be half old and half new.
    let after = file.metadata().map_err(|e| read_problem(rel, &e))?;
    if size != len || after.len() != len || after.modified().ok() != before.modified().ok() {
        return Ok(None);
    }
    Ok(Some(ManifestFile {
        path: rel.to_string(),
        size,
        sha256: hex::encode(hasher.finalize()),
    }))
}

// --------------------------------------------------------------- verifying

fn open(zip: &Path) -> Result<ZipArchive<File>> {
    let shown = zip.display().to_string();
    let file = File::open(zip).map_err(|e| read_problem(&shown, &e))?;
    ZipArchive::new(file).map_err(|e| {
        bad(format!(
            "“{shown}” isn't a usable Cairn backup: it can't be opened as a zip file ({e})."
        ))
    })
}

fn read_manifest(archive: &mut ZipArchive<File>) -> Result<Manifest> {
    let entry = archive.by_name(MANIFEST_NAME).map_err(|_| {
        bad(format!(
            "This zip has no {MANIFEST_NAME}, so it isn't a Cairn backup. \
             (A zip from “Download everything” is not a backup.)"
        ))
    })?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(bad("The backup's contents list is far too large."));
    }
    let mut text = Vec::new();
    entry
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut text)
        .map_err(damaged)?;
    let manifest: Manifest = serde_json::from_slice(&text)
        .map_err(|e| bad(format!("The backup's contents list can't be read ({e}).")))?;
    check_manifest(&manifest)?;
    Ok(manifest)
}

fn check_manifest(manifest: &Manifest) -> Result<()> {
    if manifest.format != BACKUP_FORMAT {
        return Err(bad(format!(
            "This isn't a Cairn backup (its format is “{}”).",
            manifest.format
        )));
    }
    if manifest.format_version > BACKUP_FORMAT_VERSION {
        return Err(bad(format!(
            "This backup was made by a newer Cairn (backup format {}). \
             Update Cairn to restore it.",
            manifest.format_version
        )));
    }
    let mut seen = HashSet::new();
    for file in &manifest.files {
        check_path(&file.path).map_err(|why| {
            bad(format!(
                "The backup lists “{}”, which is unsafe: {why}.",
                file.path
            ))
        })?;
        if file.sha256.len() != 64 || !file.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad(format!(
                "The backup has no valid checksum for “{}”.",
                file.path
            )));
        }
        if !seen.insert(file.path.to_lowercase()) {
            return Err(bad(format!(
                "The backup lists “{}” more than once (names that differ only in capitals clash \
                 on Windows and Mac).",
                file.path
            )));
        }
    }
    if !manifest.files.iter().any(|f| f.path == MARKER_FILE) {
        return Err(bad(format!(
            "The backup has no {MARKER_FILE}, so it can't be restored."
        )));
    }
    Ok(())
}

fn is_link(entry: &zip::read::ZipFile<'_, File>) -> bool {
    entry
        .unix_mode()
        .is_some_and(|mode| mode & 0o170_000 == 0o120_000)
}

/// Read every file in the archive, checking it against the contents list.
/// With `extract_to`, also write each one there (never overwriting).
/// Returns the number of bytes checked.
fn read_entries(
    archive: &mut ZipArchive<File>,
    manifest: &Manifest,
    extract_to: Option<&Path>,
) -> Result<u64> {
    let expected: HashMap<&str, &ManifestFile> = manifest
        .files
        .iter()
        .map(|f| (f.path.as_str(), f))
        .collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut saw_manifest = false;
    let mut marker_bytes: Vec<u8> = Vec::new();
    let mut buf = vec![0u8; CHUNK];
    let mut total = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(damaged)?;
        let name = entry.name().to_string();
        if name == MANIFEST_NAME {
            if std::mem::replace(&mut saw_manifest, true) {
                return Err(bad("The backup has two contents lists."));
            }
            continue;
        }
        if entry.is_dir() || name.ends_with('/') {
            return Err(bad(format!(
                "The backup contains a folder entry “{name}”, which Cairn backups never do."
            )));
        }
        if is_link(&entry) {
            return Err(bad(format!(
                "“{name}” in the backup is a link, not a file. Cairn backups never contain links."
            )));
        }
        let Some(file) = expected.get(name.as_str()) else {
            return Err(bad(format!(
                "The backup contains “{name}”, which is not in its contents list."
            )));
        };
        if !seen.insert(name.clone()) {
            return Err(bad(format!("The backup contains “{name}” twice.")));
        }
        if entry.size() != file.size {
            return Err(bad(format!(
                "“{name}” is not the size the backup says it should be, so the backup is damaged."
            )));
        }
        let mut out = extract_to.map(|dir| create_out(dir, &name)).transpose()?;
        let keep_marker = name == MARKER_FILE;
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        let mut reader = (&mut entry).take(file.size.saturating_add(1));
        loop {
            let n = reader.read(&mut buf).map_err(damaged)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > file.size {
                return Err(bad(format!(
                    "“{name}” is larger than the backup says it should be."
                )));
            }
            hasher.update(&buf[..n]);
            if let Some(out) = out.as_mut() {
                out.write_all(&buf[..n]).map_err(|e| write_problem(&e))?;
            }
            if keep_marker && marker_bytes.len() as u64 <= MAX_MARKER_BYTES {
                marker_bytes.extend_from_slice(&buf[..n]);
            }
        }
        if size != file.size || hex::encode(hasher.finalize()) != file.sha256.to_lowercase() {
            return Err(bad(format!(
                "“{name}” doesn't match its checksum, so the backup is damaged."
            )));
        }
        // Write errors on a network share or a full disk can show up only
        // when the file is flushed.
        if let Some(out) = out {
            out.sync_all().map_err(|e| write_problem(&e))?;
        }
        total += size;
    }
    let missing: Vec<String> = manifest
        .files
        .iter()
        .filter(|f| !seen.contains(&f.path))
        .map(|f| f.path.clone())
        .collect();
    if !missing.is_empty() {
        return Err(bad(format!(
            "The backup is incomplete: {} missing ({}).",
            count(missing.len(), "file"),
            name_list(&missing)
        )));
    }
    match parse_marker(&marker_bytes) {
        MarkerStatus::Valid(m) if m.instance_id == manifest.instance_id => Ok(total),
        MarkerStatus::Valid(_) => Err(bad(
            "The workspace file inside the backup belongs to a different documentation folder.",
        )),
        _ => Err(bad(format!(
            "The {MARKER_FILE} inside the backup can't be used."
        ))),
    }
}

/// Create the file for one entry under `dir`, never replacing anything.
fn create_out(dir: &Path, name: &str) -> Result<File> {
    let mut path = dir.to_path_buf();
    for part in split_relative(name)? {
        path.push(part);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| write_problem(&e))?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| write_problem(&e))
}

/// Check a backup file completely without restoring it.
pub fn verify(zip: &Path) -> Result<Manifest> {
    let mut archive = open(zip)?;
    let manifest = read_manifest(&mut archive)?;
    read_entries(&mut archive, &manifest, None)?;
    Ok(manifest)
}

// --------------------------------------------------------------- restoring

/// Restore a backup into a new or empty folder. The backup is checked
/// completely while it is written to a hidden folder next to `dest`, and
/// only then moved into place, so a bad backup leaves nothing behind.
pub fn restore(zip: &Path, dest: &Path) -> Result<Restored> {
    restore_checked(zip, dest, &available_space)
}

/// Room a restore of `held` bytes of files needs: the files, and a margin.
pub fn restore_space_needed(held: u64) -> u64 {
    held + SPACE_MARGIN.max(held / 100)
}

/// [`restore`] with the free-space lookup supplied (so it can be tested).
pub fn restore_checked(zip: &Path, dest: &Path, space: SpaceFn) -> Result<Restored> {
    let name = dest
        .file_name()
        .ok_or_else(|| bad("Choose a folder name to restore into."))?;
    let parent = match dest.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let parent = fs::canonicalize(parent).map_err(|_| {
        bad(format!(
            "The folder {} doesn't exist. Create it first, or restore into a new folder \
             inside one that does.",
            parent.display()
        ))
    })?;
    let dest = parent.join(name);
    if dest.exists() {
        if !dest.is_dir() {
            return Err(bad(format!("{} is a file, not a folder.", dest.display())));
        }
        let has_files = fs::read_dir(&dest)
            .map_err(|e| read_problem(&dest.display().to_string(), &e))?
            .next()
            .is_some();
        if has_files {
            return Err(bad(format!(
                "{} already has files in it. Restore into a new or empty folder; \
                 Cairn never writes over an existing one.",
                dest.display()
            )));
        }
    }
    if dest
        .ancestors()
        .skip(1)
        .any(|a| a.join(MARKER_FILE).exists())
    {
        return Err(bad(format!(
            "{} is inside a documentation folder. Restore into a folder of its own.",
            dest.display()
        )));
    }
    let mut archive = open(zip)?;
    let manifest = read_manifest(&mut archive)?;
    // Know it fits before writing anything: a restore that runs out of room
    // halfway is slow to fail and leaves a full drive behind for a moment.
    let need = restore_space_needed(manifest.files.iter().map(|f| f.size).sum());
    if let Ok(free) = space(&parent)
        && free < need
    {
        return Err(not_enough_space("The restore", need, free, &parent));
    }
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let staging = parent.join(format!(".cairn-restore-{}", &suffix[..12]));
    fs::create_dir(&staging).map_err(|e| write_problem(&e))?;
    let outcome = read_entries(&mut archive, &manifest, Some(&staging))
        .and_then(|bytes| move_into_place(&staging, &dest).map(|()| bytes));
    match outcome {
        Ok(bytes) => Ok(Restored {
            files: manifest.files.len(),
            bytes,
            display_name: manifest.display_name,
            instance_id: manifest.instance_id,
        }),
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            Err(e)
        }
    }
}

fn move_into_place(staging: &Path, dest: &Path) -> Result<()> {
    // Cairn expects its own folder to exist, even for a workspace that
    // never had a version saved.
    fs::create_dir_all(staging.join(SYSTEM_DIR)).map_err(|e| write_problem(&e))?;
    // Windows can't rename onto an existing folder. Elsewhere the rename
    // replaces an empty one, or fails if it has since filled up.
    #[cfg(windows)]
    if dest.exists() {
        // Empty (checked above): this fails rather than remove anything.
        fs::remove_dir(dest).map_err(|e| write_problem(&e))?;
    }
    fs::rename(staging, dest).map_err(|e| write_problem(&e))
}
