//! Workspace marker, discovery, initialization, and storage probing.
//!
//! A workspace is any folder whose root contains `shared-docs.json`:
//!
//! ```json
//! { "kind": "shared-docs", "schema_version": 1,
//!   "instance_id": "<uuid>", "display_name": "My Documentation" }
//! ```

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CairnError, Result};
use crate::fsutil::{create_new_with, read_optional, write_atomic};
use crate::paths::{SYSTEM_DIR, display_path, validate_name};

pub const MARKER_FILE: &str = "shared-docs.json";
pub const MARKER_KIND: &str = "shared-docs";
pub const SUPPORTED_SCHEMA: u64 = 1;
const INIT_JOURNAL: &str = ".shared-docs-init.json";
pub const DEFAULT_CATEGORIES: &[&str] =
    &["Getting-Started", "Guides", "Troubleshooting", "Reference"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Marker {
    pub kind: String,
    pub schema_version: u64,
    pub instance_id: String,
    pub display_name: String,
}

impl Marker {
    pub fn is_supported(&self) -> bool {
        self.schema_version <= SUPPORTED_SCHEMA
    }
}

#[derive(Debug, Clone)]
pub enum MarkerStatus {
    Missing,
    Valid(Marker),
    Invalid(String),
}

/// Read and validate the marker in `dir` (not its ancestors).
pub fn read_marker(dir: &Path) -> MarkerStatus {
    let bytes = match read_optional(&dir.join(MARKER_FILE)) {
        Ok(Some(b)) => b,
        Ok(None) => return MarkerStatus::Missing,
        Err(err) => {
            return MarkerStatus::Invalid(format!("The marker file could not be read: {err}"));
        }
    };
    parse_marker(&bytes)
}

fn parse_marker(bytes: &[u8]) -> MarkerStatus {
    let value: Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return MarkerStatus::Invalid("The marker file is not valid JSON.".into()),
    };
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if kind != MARKER_KIND {
        return MarkerStatus::Invalid(format!(
            "The marker file has kind {kind:?}, expected \"shared-docs\"."
        ));
    }
    let Some(schema_version) = value.get("schema_version").and_then(Value::as_u64) else {
        return MarkerStatus::Invalid("The marker file has no schema_version.".into());
    };
    let instance_id = value
        .get("instance_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if uuid::Uuid::parse_str(instance_id).is_err() {
        return MarkerStatus::Invalid("The marker file has no valid instance_id.".into());
    }
    let display_name = value
        .get("display_name")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("Documentation")
        .to_string();
    MarkerStatus::Valid(Marker {
        kind: kind.into(),
        schema_version,
        instance_id: instance_id.into(),
        display_name,
    })
}

// ---------------------------------------------------------------- discovery

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Discovery {
    /// A valid marker was found at `root` (the start folder or an ancestor).
    Found {
        root: String,
        marker: Marker,
        read_only_reason: Option<String>,
    },
    /// A marker exists but is unusable; discovery stops here rather than
    /// silently climbing to some other workspace.
    Invalid { path: String, reason: String },
    /// No marker between the start folder and the volume/share root.
    NotFound {
        start: String,
        is_empty: bool,
        searched: Vec<String>,
    },
}

/// The top of the volume or network share containing `path`
/// (`C:\`, `\\server\share`). Discovery never climbs past it.
fn boundary_of(path: &Path) -> PathBuf {
    let mut boundary = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => boundary.push(comp.as_os_str()),
            _ => break,
        }
    }
    boundary
}

/// Whether `dir` is where another disk or network share is mounted (on a
/// Mac or Linux, shares appear inside the one file tree, so the top of a
/// share is where the device changes, not a drive letter).
#[cfg(unix)]
fn is_mount_point(dir: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (dir.parent().map(fs::metadata), fs::metadata(dir)) {
        (Some(Ok(parent)), Ok(here)) => parent.dev() != here.dev(),
        _ => false,
    }
}

#[cfg(not(unix))]
fn is_mount_point(_: &Path) -> bool {
    false
}

/// Files a Mac's Finder leaves in any folder it shows; they don't make a
/// folder "not empty".
fn is_finder_litter(name: &str) -> bool {
    name == ".DS_Store" || name.starts_with("._")
}

/// Look for a marker in `start` and each ancestor, stopping at the
/// filesystem or network-share boundary. Never scans sideways or downward.
pub fn discover(start: &Path) -> Result<Discovery> {
    let canonical = fs::canonicalize(start).map_err(|_| {
        CairnError::NotFound(format!(
            "The folder {} could not be opened.",
            start.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(CairnError::BadRequest(format!(
            "{} is not a folder.",
            start.display()
        )));
    }
    let boundary = boundary_of(&canonical);
    let mut searched = Vec::new();
    let mut current: Option<&Path> = Some(&canonical);
    while let Some(dir) = current {
        searched.push(display_path(dir));
        match read_marker(dir) {
            MarkerStatus::Valid(marker) => {
                let read_only_reason = (!marker.is_supported()).then(|| schema_message(&marker));
                return Ok(Discovery::Found {
                    root: display_path(dir),
                    marker,
                    read_only_reason,
                });
            }
            MarkerStatus::Invalid(reason) => {
                return Ok(Discovery::Invalid {
                    path: display_path(dir),
                    reason,
                });
            }
            MarkerStatus::Missing => {}
        }
        if dir == boundary || searched.len() > 64 || is_mount_point(dir) {
            break;
        }
        current = dir.parent();
    }
    Ok(Discovery::NotFound {
        start: display_path(&canonical),
        is_empty: is_dir_empty(&canonical),
        searched,
    })
}

pub fn schema_message(marker: &Marker) -> String {
    format!(
        "This documentation folder was set up by a newer version of the app (format {}). \
         This version understands format {SUPPORTED_SCHEMA}, so you can read pages but not \
         change them. Update the app to edit.",
        marker.schema_version
    )
}

fn is_dir_empty(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|mut it| {
            it.all(|e| e.is_ok_and(|e| is_finder_litter(&e.file_name().to_string_lossy())))
        })
        .unwrap_or(false)
}

// ----------------------------------------------------------- initialization

#[derive(Debug, Clone)]
pub enum InitTarget {
    /// Initialize this exact folder; it must be empty.
    Here(PathBuf),
    /// Create a new folder named `name` inside `parent` and initialize it.
    NewChild { parent: PathBuf, name: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct InitOutcome {
    pub root: String,
    pub marker: Marker,
    /// False when an existing workspace (possibly created concurrently by
    /// someone else) was found instead of creating a new one.
    pub created: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct InitJournal {
    instance_id: String,
    display_name: String,
    entries: Vec<String>,
}

fn starter_entries() -> Vec<String> {
    let mut entries: Vec<String> = DEFAULT_CATEGORIES.iter().map(|s| s.to_string()).collect();
    entries.push("README.md".into());
    entries.push(SYSTEM_DIR.into());
    entries
}

/// Create a workspace.
///
/// Safe against concurrent and interrupted runs:
/// - An init journal is created with exclusive creation *before* anything
///   else. It fixes the instance id; every racer and every retry reuses it,
///   so the workspace ends up with exactly one identity.
/// - Only an empty folder, or a folder containing nothing but entries this
///   same journal promised to create, is accepted. Unrelated files cause a
///   refusal, never an overwrite.
/// - The marker is written last and never overwrites an existing marker.
pub fn initialize(target: &InitTarget, display_name: &str) -> Result<InitOutcome> {
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(CairnError::BadRequest(
            "Please give the documentation a name.".into(),
        ));
    }
    let dir = match target {
        InitTarget::Here(dir) => dir.clone(),
        InitTarget::NewChild { parent, name } => {
            validate_name(name.trim())?;
            let dir = parent.join(name.trim());
            match fs::create_dir(&dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(err) => return Err(err.into()),
            }
            dir
        }
    };
    if !dir.is_dir() {
        return Err(CairnError::NotFound(format!(
            "{} is not a folder.",
            dir.display()
        )));
    }

    // Already a workspace (maybe someone else just finished): adopt it.
    if let Some(done) = adopt_existing(&dir)? {
        return Ok(done);
    }

    let journal_path = dir.join(INIT_JOURNAL);
    let proposed = InitJournal {
        instance_id: uuid::Uuid::new_v4().to_string(),
        display_name: display_name.to_string(),
        entries: starter_entries(),
    };
    let proposed_bytes = serde_json::to_vec_pretty(&proposed).expect("journal serializes");

    // Refuse folders with unrelated content. Entries promised by an existing
    // journal (an interrupted earlier attempt) are allowed. A concurrent
    // initializer may finish (writing the marker and removing its journal)
    // at any point, so every "someone else got here first" path re-checks
    // for a finished workspace before failing.
    let existing_journal = read_journal(&journal_path)?;
    let allowed = match &existing_journal {
        Some(j) => j.entries.clone(),
        None => Vec::new(),
    };
    if let Err(err) = check_only_expected_entries(&dir, &allowed) {
        return match adopt_existing(&dir)? {
            Some(done) => Ok(done),
            None => Err(err),
        };
    }

    let journal = match existing_journal {
        Some(j) => j,
        None => match claim_or_join(&dir, &journal_path, &proposed_bytes)? {
            Claim::Journal(j) => j,
            Claim::Ours => proposed,
            Claim::Finished(done) => return Ok(done),
        },
    };

    // Starter content. Every step is idempotent and never overwrites.
    for category in DEFAULT_CATEGORIES {
        fs::create_dir_all(dir.join(category))?;
    }
    fs::create_dir_all(dir.join(SYSTEM_DIR))?;
    create_new_with(
        &dir.join("README.md"),
        starter_readme(&journal.display_name).as_bytes(),
    )?;

    let marker = Marker {
        kind: MARKER_KIND.into(),
        schema_version: SUPPORTED_SCHEMA,
        instance_id: journal.instance_id.clone(),
        display_name: journal.display_name.clone(),
    };
    let marker_bytes = serde_json::to_vec_pretty(&marker).expect("marker serializes");
    let created = create_new_with(&dir.join(MARKER_FILE), &marker_bytes)?;
    let final_marker = match read_marker(&dir) {
        MarkerStatus::Valid(m) => m,
        _ => {
            return Err(CairnError::Io(
                "The marker file could not be verified after writing.".into(),
            ));
        }
    };
    let _ = fs::remove_file(&journal_path);
    Ok(InitOutcome {
        root: display_path(&canonical(&dir)),
        marker: final_marker,
        created,
    })
}

/// If `dir` already has a valid marker, the outcome of adopting it.
fn adopt_existing(dir: &Path) -> Result<Option<InitOutcome>> {
    match read_marker(dir) {
        MarkerStatus::Valid(marker) => {
            let _ = fs::remove_file(dir.join(INIT_JOURNAL));
            Ok(Some(InitOutcome {
                root: display_path(&canonical(dir)),
                marker,
                created: false,
            }))
        }
        MarkerStatus::Invalid(reason) => Err(CairnError::BadRequest(format!(
            "This folder already has a {MARKER_FILE} file that can't be used: {reason}"
        ))),
        MarkerStatus::Missing => Ok(None),
    }
}

enum Claim {
    /// We created the journal; our proposed identity is the workspace's.
    Ours,
    /// Another initializer's journal: adopt its identity and finish its work.
    Journal(InitJournal),
    /// Another initializer already finished.
    Finished(InitOutcome),
}

/// Create the init journal, or join whichever initializer beat us to it.
fn claim_or_join(dir: &Path, journal_path: &Path, proposed: &[u8]) -> Result<Claim> {
    for attempt in 0..40u64 {
        if create_new_with(journal_path, proposed)? {
            return Ok(Claim::Ours);
        }
        if let Some(done) = adopt_existing(dir)? {
            return Ok(Claim::Finished(done));
        }
        if let Some(journal) = read_journal(journal_path)? {
            return Ok(Claim::Journal(journal));
        }
        // The journal vanished between our attempt and the read (its owner
        // is finishing). Give the marker a moment to appear, then retry.
        std::thread::sleep(Duration::from_millis(10 + 10 * attempt.min(10)));
    }
    Err(CairnError::Io(
        "Another setup of this folder is in progress. Please try again in a moment.".into(),
    ))
}

fn canonical(dir: &Path) -> PathBuf {
    fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

/// Read the init journal, retrying briefly in case a concurrent initializer
/// is mid-write on storage without atomic hard links.
fn read_journal(path: &Path) -> Result<Option<InitJournal>> {
    for attempt in 0..20 {
        match read_optional(path)? {
            None => return Ok(None),
            Some(bytes) => {
                if let Ok(journal) = serde_json::from_slice::<InitJournal>(&bytes)
                    && uuid::Uuid::parse_str(&journal.instance_id).is_ok()
                {
                    return Ok(Some(journal));
                }
            }
        }
        std::thread::sleep(Duration::from_millis(25 * (attempt + 1)));
    }
    Err(CairnError::Io(
        "A previous setup left an unreadable journal file (.shared-docs-init.json). \
         Delete it and try again."
            .into(),
    ))
}

fn check_only_expected_entries(dir: &Path, allowed: &[String]) -> Result<()> {
    let mut unexpected = Vec::new();
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let ok = name == INIT_JOURNAL
            || is_finder_litter(&name)
            || name.contains(".cairn-tmp-")
            || allowed.iter().any(|a| a.eq_ignore_ascii_case(&name));
        if !ok {
            unexpected.push(name);
        }
    }
    if unexpected.is_empty() {
        return Ok(());
    }
    unexpected.sort();
    let shown: Vec<_> = unexpected.iter().take(5).cloned().collect();
    Err(CairnError::BadRequest(format!(
        "This folder already contains other files ({}{}). To keep them safe, choose an empty \
         folder or let Cairn create a new folder inside this one.",
        shown.join(", "),
        if unexpected.len() > shown.len() {
            ", …"
        } else {
            ""
        }
    )))
}

fn starter_readme(name: &str) -> String {
    format!(
        "# {name}\n\n\
         Welcome. This folder holds shared documentation as ordinary Markdown files.\n\n\
         You can read and change these pages with Cairn, or open the `.md` files in any text \
         editor.\n\n\
         ## Folders\n\n\
         - **Getting-Started**: first steps for new people\n\
         - **Guides**: how to do common tasks\n\
         - **Troubleshooting**: what to do when something goes wrong\n\
         - **Reference**: facts, lists, and settings to look up\n\n\
         The `_system` folder is used by Cairn for edit locks and earlier versions of pages. \
         Please leave it in place.\n"
    )
}

/// Change the workspace display name, preserving any unknown marker fields.
/// Save the team's choice about removing old earlier versions in the
/// marker file, keeping every other field as it is.
pub fn set_version_cleanup(root: &Path, cleanup: &crate::history::Cleanup) -> Result<()> {
    cleanup.validate()?;
    let path = root.join(MARKER_FILE);
    let bytes = read_optional(&path)?
        .ok_or_else(|| CairnError::NotFound("The marker file is missing.".into()))?;
    match parse_marker(&bytes) {
        MarkerStatus::Valid(m) if m.is_supported() => {}
        MarkerStatus::Valid(m) => return Err(CairnError::ReadOnly(schema_message(&m))),
        _ => {
            return Err(CairnError::BadRequest(
                "The marker file is not valid JSON.".into(),
            ));
        }
    }
    let mut value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| CairnError::BadRequest("The marker file is not valid JSON.".into()))?;
    value["version_cleanup"] = serde_json::to_value(cleanup).expect("cleanup serializes");
    write_atomic(
        &path,
        &serde_json::to_vec_pretty(&value).expect("marker serializes"),
    )?;
    Ok(())
}

pub fn rename_workspace(root: &Path, new_name: &str) -> Result<Marker> {
    let new_name = new_name.trim();
    if new_name.is_empty() || new_name.chars().count() > 120 {
        return Err(CairnError::BadRequest(
            "Please enter a name up to 120 characters.".into(),
        ));
    }
    let path = root.join(MARKER_FILE);
    let bytes = read_optional(&path)?
        .ok_or_else(|| CairnError::NotFound("The marker file is missing.".into()))?;
    let mut value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| CairnError::BadRequest("The marker file is not valid JSON.".into()))?;
    if let MarkerStatus::Valid(m) = parse_marker(&bytes)
        && !m.is_supported()
    {
        return Err(CairnError::ReadOnly(schema_message(&m)));
    }
    value["display_name"] = Value::String(new_name.to_string());
    let out = serde_json::to_vec_pretty(&value).expect("marker serializes");
    write_atomic(&path, &out)?;
    match parse_marker(&out) {
        MarkerStatus::Valid(m) => Ok(m),
        _ => Err(CairnError::Io(
            "The marker could not be verified after renaming.".into(),
        )),
    }
}

// ----------------------------------------------------------- storage probe

#[derive(Debug, Clone, Serialize, Default)]
pub struct StorageReport {
    /// Whether the current user can write to the workspace at all.
    pub writable: bool,
    /// Whether exclusive-create and atomic-replace behaved correctly, so
    /// single-editor guarantees can be relied on.
    pub reliable_locking: bool,
    /// Plain-language notes shown in Settings and as a banner when unreliable.
    pub notes: Vec<String>,
}

const SYNC_MARKERS: &[&str] = &[
    "onedrive",
    "dropbox",
    "google drive",
    "googledrive",
    "my drive",
    "icloud",
    "mobile documents", // iCloud Drive on a Mac: ~/Library/Mobile Documents
    "box sync",
];

/// Test the storage behavior Cairn depends on, inside `_system/.probe`.
pub fn probe_storage(root: &Path) -> StorageReport {
    let mut report = StorageReport::default();
    let lower = display_path(root).to_lowercase();
    let in_sync_folder = SYNC_MARKERS.iter().any(|m| lower.contains(m))
        || ["OneDrive", "OneDriveCommercial", "OneDriveConsumer"]
            .iter()
            .any(|var| {
                std::env::var(var)
                    .map(|p| !p.is_empty() && lower.starts_with(&p.to_lowercase()))
                    .unwrap_or(false)
            });

    let probe_dir = root.join(SYSTEM_DIR).join(".probe");
    if fs::create_dir_all(&probe_dir).is_err() || !crate::fsutil::can_write_dir(&probe_dir) {
        report.notes.push(
            "You can read this documentation but you don't have permission to change it.".into(),
        );
        return report;
    }
    report.writable = true;

    let token = uuid::Uuid::new_v4().simple().to_string();
    let a = probe_dir.join(format!("a-{token}"));
    let checks = (|| -> std::io::Result<bool> {
        let first = create_new_with(&a, b"one")?;
        let second = create_new_with(&a, b"two")?;
        write_atomic(&a, b"three")?;
        let replaced = fs::read(&a)? == b"three";
        Ok(first && !second && replaced)
    })();
    let _ = fs::remove_file(&a);
    let _ = fs::remove_dir(&probe_dir);

    match checks {
        Ok(true) => report.reliable_locking = true,
        Ok(false) => report.notes.push(
            "This storage did not honor exclusive file creation or safe replacement. \
             Editing by several people at once is not safe here."
                .into(),
        ),
        Err(err) => report.notes.push(format!(
            "The storage check failed ({err}). Editing by several people at once may not be \
             safe here."
        )),
    }
    if in_sync_folder {
        report.reliable_locking = false;
        report.notes.push(
            "This folder appears to be synced by a cloud service (such as OneDrive or Dropbox). \
             Sync services can duplicate or restore files on their own, so edit locks are not \
             reliable here. Use a network share for editing by several people."
                .into(),
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_validation() {
        assert!(matches!(
            parse_marker(b"not json"),
            MarkerStatus::Invalid(_)
        ));
        assert!(matches!(
            parse_marker(br#"{"kind":"other","schema_version":1,"instance_id":"x"}"#),
            MarkerStatus::Invalid(_)
        ));
        let ok = format!(
            r#"{{"kind":"shared-docs","schema_version":2,"instance_id":"{}","display_name":"D"}}"#,
            uuid::Uuid::new_v4()
        );
        match parse_marker(ok.as_bytes()) {
            MarkerStatus::Valid(m) => assert!(!m.is_supported()),
            other => panic!("unexpected {other:?}"),
        }
    }
}
