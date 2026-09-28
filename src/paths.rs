//! The single gate every request-supplied path passes through.
//!
//! Callers never join user input onto the workspace root themselves. They ask
//! [`Root::resolve`] (for things that must exist) or
//! [`Root::resolve_for_create`] (for things about to be written), which:
//!
//! 1. reject anything that is not a plain relative path made of ordinary
//!    names (no `..`, drive letters, UNC prefixes, alternate data streams,
//!    reserved device names, or names Windows would silently rewrite);
//! 2. refuse to pass through any symlink, junction, or other reparse point;
//! 3. canonicalize the deepest existing ancestor and confirm it is still
//!    inside the canonical root.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{CairnError, Result};
use crate::fsutil::is_link_like;

/// Folder inside the workspace reserved for Cairn's own bookkeeping.
pub const SYSTEM_DIR: &str = "_system";

const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM0", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
    "COM8", "COM9", "LPT0", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    "CONIN$", "CONOUT$",
];

fn reject(msg: impl Into<String>) -> CairnError {
    CairnError::PathRejected(msg.into())
}

/// Validate a single path component (a file or folder name).
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(reject("A file or folder name can't be empty."));
    }
    if name == "." || name == ".." {
        return Err(reject("Paths can't step outside the documentation folder."));
    }
    if name.len() > 200 {
        return Err(reject("That name is too long."));
    }
    if let Some(bad) = name.chars().find(|c| {
        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '/' | '\\')
    }) {
        return Err(reject(format!(
            "Names can't contain the character {bad:?}."
        )));
    }
    if name.ends_with('.') || name.ends_with(' ') || name.starts_with(' ') {
        return Err(reject(
            "Names can't start or end with a space, or end with a dot.",
        ));
    }
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    if RESERVED_NAMES.contains(&stem.as_str()) {
        return Err(reject(format!("{name:?} is a reserved name on Windows.")));
    }
    Ok(())
}

/// Split and validate a workspace-relative path such as `Guides/setup.md`.
/// Both `/` and `\` are accepted as separators. The empty string means the
/// workspace root itself.
pub fn split_relative(rel: &str) -> Result<Vec<String>> {
    if rel.contains('\0') {
        return Err(reject("Invalid path."));
    }
    if rel.is_empty() {
        return Ok(Vec::new());
    }
    if rel.starts_with('/') || rel.starts_with('\\') {
        return Err(reject(
            "Paths must be relative to the documentation folder.",
        ));
    }
    if Path::new(rel)
        .components()
        .any(|c| matches!(c, Component::Prefix(_) | Component::RootDir))
    {
        return Err(reject(
            "Paths must be relative to the documentation folder.",
        ));
    }
    let parts: Vec<String> = rel.split(['/', '\\']).map(str::to_string).collect();
    for part in &parts {
        validate_name(part)?;
    }
    Ok(parts)
}

/// Normalize a workspace-relative path to `/`-separated form.
pub fn normalize_relative(rel: &str) -> Result<String> {
    Ok(split_relative(rel)?.join("/"))
}

/// Whether a relative path points into Cairn's own `_system` folder.
pub fn is_system_path(rel: &str) -> bool {
    rel.split(['/', '\\'])
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case(SYSTEM_DIR))
}

/// Turn a canonical Windows path back into something readable
/// (`\\?\C:\x` → `C:\x`, `\\?\UNC\srv\share` → `\\srv\share`).
pub fn display_path(path: &Path) -> String {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.into_owned()
    }
}

/// An opened workspace root. All access to workspace files goes through it.
#[derive(Debug, Clone)]
pub struct Root {
    canonical: PathBuf,
}

impl Root {
    pub fn new(path: &Path) -> Result<Root> {
        let canonical = fs::canonicalize(path).map_err(|_| {
            CairnError::NotFound(format!(
                "The folder {} could not be opened.",
                path.display()
            ))
        })?;
        if !canonical.is_dir() {
            return Err(CairnError::BadRequest(format!(
                "{} is not a folder.",
                path.display()
            )));
        }
        Ok(Root { canonical })
    }

    /// Canonical root path (may carry a `\\?\` prefix on Windows).
    pub fn path(&self) -> &Path {
        &self.canonical
    }

    /// Human-readable root path.
    pub fn display(&self) -> String {
        display_path(&self.canonical)
    }

    /// Resolve a path that must already exist.
    pub fn resolve(&self, rel: &str) -> Result<PathBuf> {
        let path = self.resolve_for_create(rel)?;
        if !path.exists() {
            return Err(CairnError::NotFound(format!("{rel} could not be found.")));
        }
        Ok(path)
    }

    /// Resolve a path that may not exist yet (it is about to be created).
    /// Every existing component along the way is checked for links, and the
    /// deepest existing ancestor must canonicalize to somewhere inside the root.
    pub fn resolve_for_create(&self, rel: &str) -> Result<PathBuf> {
        let parts = split_relative(rel)?;
        let mut current = self.canonical.clone();
        let mut deepest_existing = self.canonical.clone();
        for part in &parts {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(meta) => {
                    if is_link_like(&meta) {
                        return Err(reject(
                            "That path goes through a shortcut link, which Cairn doesn't follow.",
                        ));
                    }
                    deepest_existing = current.clone();
                }
                Err(_) => break,
            }
        }
        let resolved = fs::canonicalize(&deepest_existing).map_err(|_| reject("Invalid path."))?;
        if !is_within(&resolved, &self.canonical) {
            return Err(reject("That path is outside the documentation folder."));
        }
        let mut full = self.canonical.clone();
        for part in &parts {
            full.push(part);
        }
        Ok(full)
    }

    /// Workspace-relative `/`-separated form of an absolute path inside the root.
    pub fn relative(&self, abs: &Path) -> Option<String> {
        let stripped = abs.strip_prefix(&self.canonical).ok()?;
        let parts: Vec<String> = stripped
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect();
        Some(parts.join("/"))
    }

    pub fn system_dir(&self) -> PathBuf {
        self.canonical.join(SYSTEM_DIR)
    }
}

/// Component-wise prefix check (so `C:\docs2` is not "inside" `C:\docs`).
fn is_within(path: &Path, root: &Path) -> bool {
    let mut p = path.components();
    for rc in root.components() {
        match p.next() {
            Some(pc) if component_eq(&pc, &rc) => {}
            _ => return false,
        }
    }
    true
}

fn component_eq(a: &Component, b: &Component) -> bool {
    let (a, b) = (
        a.as_os_str().to_string_lossy(),
        b.as_os_str().to_string_lossy(),
    );
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

/// Resolve `target` (a link found inside the article at `from_rel`) to a
/// workspace-relative path using lexical rules only. Returns `None` if it
/// would climb above the workspace root.
pub fn join_relative_link(from_rel: &str, target: &str) -> Option<String> {
    let mut stack: Vec<&str> = from_rel.split('/').collect();
    stack.pop(); // drop the file name, keep its folder
    for part in target.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other),
        }
    }
    Some(stack.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_join_is_lexical_and_bounded() {
        assert_eq!(
            join_relative_link("Guides/a.md", "b.md").unwrap(),
            "Guides/b.md"
        );
        assert_eq!(
            join_relative_link("Guides/a.md", "../Ref/c.md").unwrap(),
            "Ref/c.md"
        );
        assert!(join_relative_link("Guides/a.md", "../../x.md").is_none());
    }

    #[test]
    fn display_strips_verbatim_prefix() {
        assert_eq!(display_path(Path::new(r"\\?\C:\docs")), r"C:\docs");
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\srv\share\d")),
            r"\\srv\share\d"
        );
    }
}
