//! Choosing which files to export and packing them into zips.
//!
//! Paths inside a zip are workspace-relative (`Guides/setup.md`,
//! `Guides/setup.assets/shot-1a2b3c4d.png`), so relative links between pages
//! and pictures keep working after unzipping. Cairn's `_system` folder,
//! `shared-docs.json`, hidden files, temporary files, and anything reached
//! through a link are never included.
//!
//! An export never drops a file without saying so. A folder or file Cairn
//! cannot read stops the export and is named. Links that are left out on
//! purpose are listed, and a file that changes while the export runs is
//! flagged.

use std::fs;
use std::io::{self, Cursor, Write};
use std::time::SystemTime;

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{CairnError, Result};
use crate::fsutil::is_link_like;
use crate::locks::LOCKS_DIR;
use crate::paths::{Root, SYSTEM_DIR};
use crate::publish::assets_dir_rel;
use crate::templates::TEMPLATES_DIR;
use crate::workspace::MARKER_FILE;

fn is_hidden_or_temp(name: &str) -> bool {
    name.starts_with('.') || name.contains(".cairn-tmp-") || name.contains(".cairn-write-probe-")
}

/// A file's size and modification time at one moment. If it differs later,
/// the file changed in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
}

impl Stamp {
    fn of(meta: &fs::Metadata) -> Stamp {
        Stamp {
            len: meta.len(),
            modified: meta.modified().ok(),
        }
    }
}

/// One file chosen for export, and how it looked when it was chosen.
#[derive(Debug, Clone)]
pub struct Entry {
    pub rel: String,
    pub stamp: Stamp,
}

/// What an export will contain, and what was left out on purpose.
#[derive(Debug, Default)]
pub struct Enumeration {
    pub entries: Vec<Entry>,
    /// Links and shortcuts that were not followed, as workspace paths.
    pub skipped_links: Vec<String>,
}

impl Enumeration {
    /// Keep only Markdown pages (for PDF export).
    fn pages_only(mut self) -> Enumeration {
        self.entries.retain(|e| is_page(&e.rel));
        self
    }
}

fn is_page(rel: &str) -> bool {
    rel.to_ascii_lowercase().ends_with(".md")
        && !rel
            .split('/')
            .any(|seg| seg.to_ascii_lowercase().ends_with(".assets"))
}

/// A file or folder that can't be read stops the export. Leaving it out
/// quietly would hand over an archive that looks complete and isn't.
/// "1 file" / "3 files".
pub fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The first few names, then how many more.
pub fn name_list(paths: &[String]) -> String {
    const SHOWN: usize = 3;
    let shown = paths.iter().take(SHOWN).cloned().collect::<Vec<_>>();
    match paths.len().saturating_sub(SHOWN) {
        0 => shown.join(", "),
        more => format!("{}, and {more} more", shown.join(", ")),
    }
}

pub(crate) fn read_problem(name: &str, err: &io::Error) -> CairnError {
    match err.kind() {
        io::ErrorKind::NotFound => CairnError::NotFound(format!(
            "“{name}” was removed while the download was being prepared. Please try again."
        )),
        io::ErrorKind::PermissionDenied => CairnError::PermissionDenied(format!(
            "Cairn isn't allowed to read “{name}”, so the download was stopped instead of \
             leaving it out. Ask whoever manages the shared folder for access, then try again."
        )),
        _ => CairnError::Io(format!(
            "Cairn couldn't read “{name}” ({err}), so the download was stopped instead of \
             leaving it out. Check that the shared folder is still reachable, then try again."
        )),
    }
}

/// Every exportable file under `folder_rel` ("" = the whole documentation),
/// plus the links that were not followed. `_templates` is included only for
/// a whole-documentation export.
pub fn enumerate(root: &Root, folder_rel: &str) -> Result<Enumeration> {
    let whole = folder_rel.is_empty();
    walk(root, folder_rel, |e| {
        let name = e.file_name().to_string_lossy();
        let is_dir = e.file_type().is_dir();
        let system = is_dir && name.eq_ignore_ascii_case(SYSTEM_DIR);
        let templates = is_dir && !whole && name.eq_ignore_ascii_case(TEMPLATES_DIR);
        // The marker carries the workspace's identity: a copy of it
        // would pose as the original.
        let marker = whole && e.depth() == 1 && !is_dir && name.eq_ignore_ascii_case(MARKER_FILE);
        !(is_hidden_or_temp(&name) || system || templates || marker)
    })
}

/// Everything a workspace backup holds: the whole documentation, including
/// templates, the workspace marker, and Cairn's own records (earlier
/// versions, who edited what, deleted pages). Edit locks and scratch files
/// are left out: they describe the moment, not the documentation.
pub fn enumerate_backup(root: &Root) -> Result<Enumeration> {
    walk(root, "", |e| {
        let name = e.file_name().to_string_lossy();
        let in_system = e
            .path()
            .parent()
            .and_then(|p| p.file_name())
            .is_some_and(|n| n.eq_ignore_ascii_case(SYSTEM_DIR));
        let locks = e.depth() == 2
            && in_system
            && e.file_type().is_dir()
            && name.eq_ignore_ascii_case(LOCKS_DIR);
        !(is_hidden_or_temp(&name) || locks)
    })
}

/// Walk the folder, keep what `keep` accepts (it is not asked about the
/// starting folder), and report anything unreadable.
fn walk(
    root: &Root,
    folder_rel: &str,
    keep: impl Fn(&walkdir::DirEntry) -> bool,
) -> Result<Enumeration> {
    let start = root.resolve(folder_rel)?;
    let walker = WalkDir::new(&start)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || keep(e));
    let mut found = Enumeration::default();
    for entry in walker {
        let entry = entry.map_err(|err| {
            let name = err
                .path()
                .and_then(|p| root.relative(p))
                .unwrap_or_else(|| "a folder".into());
            read_problem(&name, &io::Error::from(err))
        })?;
        let rel = root
            .relative(entry.path())
            .ok_or_else(|| CairnError::Io("A file was found outside the documentation.".into()))?;
        let meta = fs::symlink_metadata(entry.path()).map_err(|e| read_problem(&rel, &e))?;
        if is_link_like(&meta) {
            found.skipped_links.push(rel);
        } else if meta.is_file() {
            found.entries.push(Entry {
                rel,
                stamp: Stamp::of(&meta),
            });
        }
    }
    found.entries.sort_by_key(|e| e.rel.to_lowercase());
    found.skipped_links.sort_by_key(|p| p.to_lowercase());
    Ok(found)
}

/// Markdown pages under `folder_rel`, with the same rules as [`enumerate`].
pub fn enumerate_pages(root: &Root, folder_rel: &str) -> Result<Enumeration> {
    Ok(enumerate(root, folder_rel)?.pages_only())
}

/// Every exportable file under `folder_rel` ("" = the whole documentation).
pub fn folder_files(root: &Root, folder_rel: &str) -> Result<Vec<String>> {
    Ok(enumerate(root, folder_rel)?
        .entries
        .into_iter()
        .map(|e| e.rel)
        .collect())
}

/// A page plus everything in its `<page>.assets/` folder.
pub fn page_files(root: &Root, page_rel: &str) -> Result<Vec<String>> {
    let mut out = vec![page_rel.to_string()];
    let assets = assets_dir_rel(page_rel);
    if let Ok(dir) = root.resolve(&assets)
        && dir.is_dir()
    {
        for entry in fs::read_dir(dir).map_err(|e| read_problem(&assets, &e))? {
            let entry = entry.map_err(|e| read_problem(&assets, &e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = format!("{assets}/{name}");
            let meta = fs::symlink_metadata(entry.path()).map_err(|e| read_problem(&rel, &e))?;
            if is_hidden_or_temp(&name) || is_link_like(&meta) || !meta.is_file() {
                continue;
            }
            out.push(rel);
        }
    }
    Ok(out)
}

/// Markdown pages under `folder_rel` (for PDF export). Templates are only
/// included when exporting everything.
pub fn pages_in(root: &Root, folder_rel: &str) -> Result<Vec<String>> {
    Ok(enumerate_pages(root, folder_rel)?
        .entries
        .into_iter()
        .map(|e| e.rel)
        .collect())
}

impl Entry {
    /// The file's size when it was chosen, in bytes.
    pub fn size(&self) -> u64 {
        self.stamp.len
    }

    /// Whether the file is no longer as it was when it was chosen.
    pub fn changed_since(&self, root: &Root) -> Result<bool> {
        let path = root.resolve(&self.rel).map_err(|e| match e {
            CairnError::NotFound(_) => read_problem(&self.rel, &io::ErrorKind::NotFound.into()),
            other => other,
        })?;
        let meta = fs::metadata(&path).map_err(|e| read_problem(&self.rel, &e))?;
        Ok(Stamp::of(&meta) != self.stamp)
    }
}

fn options() -> SimpleFileOptions {
    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated)
}

fn zip_err(e: impl std::fmt::Display) -> CairnError {
    CairnError::Io(format!("The download could not be packed: {e}"))
}

/// Incremental zip builder, used for long-running exports.
pub struct ZipBuilder {
    writer: ZipWriter<Cursor<Vec<u8>>>,
}

impl Default for ZipBuilder {
    fn default() -> Self {
        ZipBuilder {
            writer: ZipWriter::new(Cursor::new(Vec::new())),
        }
    }
}

impl ZipBuilder {
    pub fn add(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        self.writer.start_file(name, options()).map_err(zip_err)?;
        self.writer.write_all(bytes).map_err(zip_err)?;
        Ok(())
    }

    /// Add a workspace file by its relative path (re-checked through the root).
    pub fn add_file(&mut self, root: &Root, rel: &str) -> Result<()> {
        let path = root.resolve(rel).map_err(|e| match e {
            CairnError::NotFound(_) => read_problem(rel, &io::ErrorKind::NotFound.into()),
            other => other,
        })?;
        let bytes = fs::read(path).map_err(|e| read_problem(rel, &e))?;
        self.add(rel, &bytes)
    }

    /// Add a chosen file. Returns `true` when it changed since it was chosen,
    /// so the caller can warn that the archive mixes old and new versions.
    pub fn add_entry(&mut self, root: &Root, entry: &Entry) -> Result<bool> {
        self.add_file(root, &entry.rel)?;
        entry.changed_since(root)
    }

    pub fn finish(self) -> Result<Vec<u8>> {
        Ok(self.writer.finish().map_err(zip_err)?.into_inner())
    }
}

/// Zip the given workspace files.
pub fn zip_files(root: &Root, files: &[String]) -> Result<Vec<u8>> {
    let mut builder = ZipBuilder::default();
    for rel in files {
        builder.add_file(root, rel)?;
    }
    builder.finish()
}
