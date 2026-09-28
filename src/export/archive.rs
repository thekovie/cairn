//! Choosing which files to export and packing them into zips.
//!
//! Paths inside a zip are workspace-relative (`Guides/setup.md`,
//! `Guides/setup.assets/shot-1a2b3c4d.png`), so relative links between pages
//! and pictures keep working after unzipping. Cairn's `_system` folder,
//! hidden files, temporary files, and anything reached through a link are
//! never included.

use std::fs;
use std::io::{Cursor, Write};

use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{CairnError, Result};
use crate::fsutil::is_link_like;
use crate::paths::{Root, SYSTEM_DIR};
use crate::publish::assets_dir_rel;
use crate::templates::TEMPLATES_DIR;

fn is_hidden_or_temp(name: &str) -> bool {
    name.starts_with('.') || name.contains(".cairn-tmp-") || name.contains(".cairn-write-probe-")
}

/// Every exportable file under `folder_rel` ("" = the whole documentation).
/// `_templates` is included only for a whole-documentation export.
pub fn folder_files(root: &Root, folder_rel: &str) -> Result<Vec<String>> {
    let start = root.resolve(folder_rel)?;
    let whole = folder_rel.is_empty();
    let walker = WalkDir::new(&start)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            let name = e.file_name().to_string_lossy();
            let is_dir = e.file_type().is_dir();
            let system = is_dir && name.eq_ignore_ascii_case(SYSTEM_DIR);
            let templates = is_dir && !whole && name.eq_ignore_ascii_case(TEMPLATES_DIR);
            !(is_hidden_or_temp(&name) || system || templates)
        });
    let mut out = Vec::new();
    for entry in walker.flatten() {
        let Ok(meta) = fs::symlink_metadata(entry.path()) else {
            continue;
        };
        if is_link_like(&meta) || !meta.is_file() {
            continue;
        }
        if let Some(rel) = root.relative(entry.path()) {
            out.push(rel);
        }
    }
    out.sort_by_key(|p| p.to_lowercase());
    Ok(out)
}

/// A page plus everything in its `<page>.assets/` folder.
pub fn page_files(root: &Root, page_rel: &str) -> Result<Vec<String>> {
    let mut out = vec![page_rel.to_string()];
    let assets = assets_dir_rel(page_rel);
    if let Ok(dir) = root.resolve(&assets)
        && dir.is_dir()
    {
        for entry in fs::read_dir(dir)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            if is_hidden_or_temp(&name) || is_link_like(&meta) || !meta.is_file() {
                continue;
            }
            out.push(format!("{assets}/{name}"));
        }
    }
    Ok(out)
}

/// Markdown pages under `folder_rel` (for PDF export). Templates are only
/// included when exporting everything.
pub fn pages_in(root: &Root, folder_rel: &str) -> Result<Vec<String>> {
    Ok(folder_files(root, folder_rel)?
        .into_iter()
        .filter(|p| p.to_ascii_lowercase().ends_with(".md"))
        .filter(|p| {
            !p.split('/')
                .any(|seg| seg.to_ascii_lowercase().ends_with(".assets"))
        })
        .collect())
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
        let bytes = fs::read(root.resolve(rel)?)?;
        self.add(rel, &bytes)
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
