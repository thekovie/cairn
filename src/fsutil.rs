//! Filesystem primitives used for every write Cairn makes.
//!
//! Two rules hold throughout:
//! - A live file is never truncated or opened for writing in place. New content
//!   goes to a temporary file in the *same directory* (same volume, so the
//!   final rename is atomic) and then replaces the target.
//! - "Create only if absent" uses exclusive creation, which is atomic on NTFS
//!   and on SMB2+ shares. This is what edit locks and workspace initialization
//!   are built on.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn temp_path_for(target: &Path) -> io::Result<PathBuf> {
    let dir = target
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "target has no parent"))?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    Ok(dir.join(format!(".{name}.cairn-tmp-{}", &suffix[..12])))
}

/// Write a complete temporary file next to `target` and flush it to disk.
/// Returns the temp path; the caller decides how to move it into place.
pub fn write_temp_beside(target: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    let temp = temp_path_for(target)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    Ok(temp)
}

/// Replace `target` with `bytes` without ever truncating the live file.
///
/// On Windows `std::fs::rename` replaces an existing destination atomically
/// (MoveFileEx with replace-existing semantics); on Unix it is `rename(2)`.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    let temp = write_temp_beside(target, bytes)?;
    if let Err(err) = fs::rename(&temp, target) {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    Ok(())
}

/// Create `target` with the complete `bytes` only if it does not exist yet.
///
/// Returns `Ok(true)` if this call created the file and `Ok(false)` if it
/// already existed. The content is published atomically by hard-linking a
/// fully written temp file, so a concurrent reader never sees a half-written
/// file. If the storage does not support hard links, falls back to exclusive
/// creation followed by a write.
pub fn create_new_with(target: &Path, bytes: &[u8]) -> io::Result<bool> {
    let temp = write_temp_beside(target, bytes)?;
    let linked = fs::hard_link(&temp, target);
    let _ = fs::remove_file(&temp);
    match linked {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => {
            // Hard links unsupported here: exclusive create, then write.
            match OpenOptions::new().write(true).create_new(true).open(target) {
                Ok(mut file) => {
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    Ok(true)
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(false),
                Err(err) => Err(err),
            }
        }
    }
}

/// Read a whole file, returning `None` if it does not exist.
pub fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match File::open(path) {
        Ok(mut file) => {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            Ok(Some(buf))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Whether the current user can create files in `dir`, tested by actually
/// creating and removing a probe file. Filesystem permissions are the only
/// authority Cairn trusts.
pub fn can_write_dir(dir: &Path) -> bool {
    let probe = dir.join(format!(
        ".cairn-write-probe-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..12]
    ));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(file) => {
            drop(file);
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Whether a path is a symlink, junction, or other reparse point.
pub fn is_link_like(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.md");
        write_atomic(&target, b"one").unwrap();
        write_atomic(&target, b"two").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"two");
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("cairn-tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn create_new_with_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("lock");
        assert!(create_new_with(&target, b"first").unwrap());
        assert!(!create_new_with(&target, b"second").unwrap());
        assert_eq!(fs::read(&target).unwrap(), b"first");
    }
}
