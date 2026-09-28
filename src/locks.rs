//! Article edit locks.
//!
//! One lock file per article in `_system/locks/`, created with exclusive
//! creation (atomic on NTFS and SMB2+). Reading never takes a lock.
//!
//! A lock is never taken over because of its age. A stale heartbeat only
//! changes how it is *described* ("may have been left open"). Recovery is an
//! explicit maintainer action ([`maintainer_release`]) or, for a lock left
//! by this same user on this same computer, an explicit reclaim.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{CairnError, Result};
use crate::fsutil::{create_new_with, read_optional, sha256_hex, write_atomic};
use crate::paths::Root;

/// Seconds between heartbeats written by a running client.
pub const HEARTBEAT_SECS: u64 = 30;
/// A lock whose heartbeat is older than this is described as possibly
/// abandoned (a live client refreshes it every [`HEARTBEAT_SECS`]).
pub const STALE_AFTER_SECS: u64 = 5 * 60;
/// Minimum heartbeat age before a user may reclaim their own lock.
const RECLAIM_AFTER_SECS: u64 = 3 * HEARTBEAT_SECS;

/// Who this running client is. `session_id` is unique per launch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Identity {
    pub session_id: String,
    pub display_name: String,
    pub os_user: String,
    pub host: String,
}

impl Identity {
    pub fn current(display_name: Option<&str>) -> Identity {
        let os_user = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "unknown".into());
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown".into());
        Identity {
            session_id: uuid::Uuid::new_v4().to_string(),
            display_name: display_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| os_user.clone()),
            os_user,
            host,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockInfo {
    pub session_id: String,
    pub display_name: String,
    pub os_user: String,
    pub host: String,
    pub article: String,
    /// Unix seconds.
    pub created_at: u64,
    /// Unix seconds; refreshed every [`HEARTBEAT_SECS`] by the holder.
    pub heartbeat_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LockView {
    #[serde(flatten)]
    pub info: LockInfo,
    pub is_mine: bool,
    pub possibly_abandoned: bool,
    /// Left by this same user on this computer by an earlier run: the user
    /// may reclaim it explicitly.
    pub reclaimable_by_me: bool,
}

pub enum Acquire {
    Acquired(LockInfo),
    HeldBy(LockView),
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn locks_dir(root: &Root) -> PathBuf {
    root.system_dir().join("locks")
}

/// Lock file for an article. Case-insensitive, because Windows paths are.
pub fn lock_path(root: &Root, article_rel: &str) -> PathBuf {
    let key = sha256_hex(article_rel.to_lowercase().as_bytes());
    locks_dir(root).join(format!("{}.lock", &key[..32]))
}

fn new_info(article_rel: &str, me: &Identity) -> LockInfo {
    let now = now_secs();
    LockInfo {
        session_id: me.session_id.clone(),
        display_name: me.display_name.clone(),
        os_user: me.os_user.clone(),
        host: me.host.clone(),
        article: article_rel.to_string(),
        created_at: now,
        heartbeat_at: now,
    }
}

fn to_bytes(info: &LockInfo) -> Vec<u8> {
    serde_json::to_vec_pretty(info).expect("lock serializes")
}

fn read_lock(path: &Path) -> Result<Option<LockInfo>> {
    match read_optional(path)? {
        None => Ok(None),
        Some(bytes) => match serde_json::from_slice::<LockInfo>(&bytes) {
            Ok(info) => Ok(Some(info)),
            // Unreadable lock: treat as held by an unknown client. Never delete it.
            Err(_) => Ok(Some(LockInfo {
                session_id: String::new(),
                display_name: "an unknown editor".into(),
                os_user: String::new(),
                host: String::new(),
                article: String::new(),
                created_at: 0,
                heartbeat_at: 0,
            })),
        },
    }
}

fn view(info: LockInfo, me: &Identity, now: u64) -> LockView {
    let age = now.saturating_sub(info.heartbeat_at);
    let is_mine = !info.session_id.is_empty() && info.session_id == me.session_id;
    let reclaimable_by_me = !is_mine
        && !info.session_id.is_empty()
        && info.host.eq_ignore_ascii_case(&me.host)
        && info.os_user.eq_ignore_ascii_case(&me.os_user)
        && age >= RECLAIM_AFTER_SECS;
    LockView {
        possibly_abandoned: age >= STALE_AFTER_SECS,
        is_mine,
        reclaimable_by_me,
        info,
    }
}

/// Try to take the edit lock for an article. Re-entrant for the same session.
pub fn acquire(root: &Root, article_rel: &str, me: &Identity) -> Result<Acquire> {
    fs::create_dir_all(locks_dir(root))?;
    let path = lock_path(root, article_rel);
    let info = new_info(article_rel, me);
    let bytes = to_bytes(&info);
    for _ in 0..2 {
        if create_new_with(&path, &bytes)? {
            return Ok(Acquire::Acquired(info));
        }
        match read_lock(&path)? {
            Some(existing) if existing.session_id == me.session_id => {
                return Ok(Acquire::Acquired(existing));
            }
            Some(existing) => return Ok(Acquire::HeldBy(view(existing, me, now_secs()))),
            // Released between our attempt and the read: try once more.
            None => continue,
        }
    }
    Err(CairnError::Locked(
        "Someone else started editing this page just now.".into(),
    ))
}

/// Current lock state of an article, if any.
pub fn status(root: &Root, article_rel: &str, me: &Identity) -> Result<Option<LockView>> {
    Ok(read_lock(&lock_path(root, article_rel))?.map(|info| view(info, me, now_secs())))
}

/// Fail unless this session holds the lock.
pub fn verify_held(root: &Root, article_rel: &str, me: &Identity) -> Result<()> {
    match read_lock(&lock_path(root, article_rel))? {
        Some(info) if info.session_id == me.session_id => Ok(()),
        Some(info) => Err(CairnError::Locked(format!(
            "{} is editing this page now, so your changes can't be published yet. Your text is \
             kept.",
            info.display_name
        ))),
        None => Err(CairnError::Locked(
            "This page is no longer locked for you (it may have been released while you were \
             away). Choose \"Continue editing\" to lock it again. Your text is kept."
                .into(),
        )),
    }
}

/// Refresh the heartbeat on a lock this session holds.
pub fn heartbeat(root: &Root, article_rel: &str, me: &Identity) -> Result<()> {
    let path = lock_path(root, article_rel);
    let mut info = match read_lock(&path)? {
        Some(info) if info.session_id == me.session_id => info,
        _ => {
            return Err(CairnError::Locked(
                "The edit lock is no longer held.".into(),
            ));
        }
    };
    info.heartbeat_at = now_secs();
    // Replacing via rename keeps the lock file present at all times, so no
    // other client can slip in an exclusive create.
    write_atomic(&path, &to_bytes(&info))?;
    Ok(())
}

/// Release a lock this session holds. Releasing a lock we don't hold is a
/// no-op, never a deletion of someone else's lock.
pub fn release(root: &Root, article_rel: &str, me: &Identity) -> Result<bool> {
    let path = lock_path(root, article_rel);
    match read_lock(&path)? {
        Some(info) if info.session_id == me.session_id => {
            fs::remove_file(&path)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub fn held_error(v: &LockView) -> CairnError {
    CairnError::Locked(format!("{} is editing this page.", v.info.display_name))
}

/// Take over a lock left behind by this same user on this same computer by
/// an earlier run that stopped heartbeating (for example after a crash).
pub fn reclaim_own_stale(root: &Root, article_rel: &str, me: &Identity) -> Result<LockInfo> {
    let path = lock_path(root, article_rel);
    let Some(existing) = read_lock(&path)? else {
        return match acquire(root, article_rel, me)? {
            Acquire::Acquired(info) => Ok(info),
            Acquire::HeldBy(v) => Err(held_error(&v)),
        };
    };
    let v = view(existing.clone(), me, now_secs());
    if v.is_mine {
        return Ok(existing);
    }
    if !v.reclaimable_by_me {
        return Err(held_error(&v));
    }
    let info = new_info(article_rel, me);
    write_atomic(&path, &to_bytes(&info))?;
    Ok(info)
}

/// All lock files in the workspace, for the maintainer CLI.
pub fn list_locks(root: &Root) -> Result<Vec<LockInfo>> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(locks_dir(root)) else {
        return Ok(out);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("lock") {
            continue;
        }
        if let Some(info) = read_lock(&path)? {
            out.push(info);
        }
    }
    out.sort_by(|a, b| a.article.cmp(&b.article));
    Ok(out)
}

/// Maintainer recovery for an abandoned lock.
///
/// Refuses unless `expected_session` matches the lock currently on disk (so a
/// lock freshly re-acquired by someone else is never removed) and the
/// heartbeat is older than [`STALE_AFTER_SECS`] (so a live editor is never
/// cut off). The released lock is kept as an audit record. Drafts are never
/// touched: they live privately on the editor's own computer.
pub fn maintainer_release(
    root: &Root,
    article_rel: &str,
    expected_session: &str,
) -> Result<LockInfo> {
    let path = lock_path(root, article_rel);
    let info = read_lock(&path)?
        .ok_or_else(|| CairnError::NotFound(format!("{article_rel} is not locked.")))?;
    if info.session_id != expected_session {
        return Err(CairnError::Conflict(format!(
            "The lock on {article_rel} belongs to session {:?}, not {expected_session:?}. \
             Run `cairn locks list` again and check before releasing.",
            info.session_id
        )));
    }
    let age = now_secs().saturating_sub(info.heartbeat_at);
    if age < STALE_AFTER_SECS {
        return Err(CairnError::Locked(format!(
            "{} is still active on this page (last seen {age} seconds ago). A lock can only be \
             released after {} minutes without a heartbeat from its owner.",
            info.display_name,
            STALE_AFTER_SECS / 60
        )));
    }
    let audit_dir = locks_dir(root).join("released");
    fs::create_dir_all(&audit_dir)?;
    let short: String = info.session_id.chars().take(8).collect();
    write_atomic(
        &audit_dir.join(format!("{}-{short}.json", now_secs())),
        &to_bytes(&info),
    )?;
    fs::remove_file(&path)?;
    Ok(info)
}
