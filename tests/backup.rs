//! Workspace backups: they hold everything needed to recover (pages,
//! pictures, templates, earlier versions, deleted pages, edit records) and
//! none of the moment-to-moment state (locks); they verify; they restore
//! only into a new, empty folder; and damaged or forged archives fail
//! without writing anything outside it.

mod common;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use cairn::backup::{
    MANIFEST_NAME, Progress, create, create_backup, create_checked, restore, restore_checked,
    restore_space_needed, restore_with, verify, verify_with,
};
use cairn::export::archive::enumerate_backup;
use cairn::workspace::{MarkerStatus, read_marker};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

const HISTORY: &str = "_system/history/Guides/setup.md/20260101T000000000Z-abcd1234.md";

fn sample(ws: &common::TestWorkspace) {
    let p = &ws.path;
    common::write(
        &p.join("Guides/setup.md"),
        "# Set up\n\n![s](setup.assets/shot.png)\n",
    );
    std::fs::create_dir_all(p.join("Guides/setup.assets")).unwrap();
    std::fs::write(
        p.join("Guides/setup.assets/shot.png"),
        common::png(8, 8, 40),
    )
    .unwrap();
    common::write(&p.join("Guides/Résumé 日本語.md"), "# Résumé\n");
    common::write(&p.join("Root page.md"), "# Root\n");
    common::write(
        &p.join("_templates/meeting.md"),
        "---\ntemplate_name: Meeting\n---\n# {{title}}\n",
    );
    // Cairn's own records: these are what a plain download leaves out.
    common::write(&p.join(HISTORY), "# Set up (older)\n");
    common::write(
        &p.join("_system/history/Guides/setup.md/20260101T000000000Z-abcd1234.json"),
        "{\"by\":\"Sam\"}",
    );
    common::write(
        &p.join("_system/edited/Guides/setup.md.json"),
        "{\"by\":\"Sam\",\"at\":1,\"hash\":\"x\"}",
    );
    common::write(
        &p.join("_system/trash/1-abcd1234/item.json"),
        "{\"kind\":\"page\"}",
    );
    common::write(
        &p.join("_system/trash/1-abcd1234/content/Old.md"),
        "# Old\n",
    );
    // Moment-to-moment state and scratch files: never backed up.
    common::write(&p.join("_system/locks/aaaa.lock"), "{}");
    common::write(&p.join("_system/locks/released/1-x.json"), "{}");
    common::write(&p.join("_system/.probe/probe"), "x");
    common::write(&p.join("Guides/.hidden.md"), "# Hidden\n");
    common::write(&p.join("Guides/x.md.cairn-tmp-123"), "partial");
}

fn out_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn names(zip: &Path) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).unwrap()).unwrap();
    let mut out: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    out.sort();
    out
}

/// Every file under `dir` with its bytes, keyed by relative path.
fn tree(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(dir).into_iter().flatten() {
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(dir).unwrap();
            out.insert(
                rel.to_string_lossy().replace('\\', "/"),
                std::fs::read(entry.path()).unwrap(),
            );
        }
    }
    out
}

#[test]
fn a_backup_holds_everything_needed_to_recover_and_no_locks() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let summary = create_backup(&ws.root, &zip).unwrap();

    let names = names(&zip);
    for expected in [
        MANIFEST_NAME,
        "shared-docs.json",
        "Guides/setup.md",
        "Guides/setup.assets/shot.png",
        "Guides/Résumé 日本語.md",
        "_templates/meeting.md",
        HISTORY,
        "_system/edited/Guides/setup.md.json",
        "_system/trash/1-abcd1234/item.json",
        "_system/trash/1-abcd1234/content/Old.md",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "{expected} missing from {names:?}"
        );
    }
    for left_out in ["_system/locks", "_system/.probe", ".hidden", "cairn-tmp"] {
        assert!(
            !names.iter().any(|n| n.contains(left_out)),
            "{left_out} found in {names:?}"
        );
    }
    assert_eq!(summary.passes, 1);

    let manifest = verify(&zip).unwrap();
    assert_eq!(manifest.format, "cairn-backup");
    assert_eq!(manifest.format_version, 1);
    assert_eq!(manifest.cairn_version, env!("CARGO_PKG_VERSION"));
    let MarkerStatus::Valid(marker) = read_marker(&ws.path) else {
        panic!("marker")
    };
    assert_eq!(manifest.instance_id, marker.instance_id);
    assert_eq!(manifest.display_name, marker.display_name);
    let shot = manifest
        .files
        .iter()
        .find(|f| f.path == "Guides/setup.assets/shot.png")
        .unwrap();
    assert_eq!(shot.size, common::png(8, 8, 40).len() as u64);
    assert_eq!(
        shot.sha256,
        hex::encode(Sha256::digest(common::png(8, 8, 40)))
    );
}

#[test]
fn a_backup_restores_into_a_new_folder_exactly() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();

    let dest = out.path().join("Restored");
    let restored = restore(&zip, &dest).unwrap();
    assert!(restored.files > 5);

    // The restored folder holds the same backed-up files, byte for byte.
    let expected: BTreeMap<String, Vec<u8>> = enumerate_backup(&ws.root)
        .unwrap()
        .entries
        .iter()
        .map(|e| (e.rel.clone(), std::fs::read(ws.path.join(&e.rel)).unwrap()))
        .collect();
    assert_eq!(tree(&dest), expected);
    assert!(
        !dest.join("_system/locks").exists(),
        "locks are never restored"
    );
    let MarkerStatus::Valid(original) = read_marker(&ws.path) else {
        panic!("marker")
    };
    let MarkerStatus::Valid(copy) = read_marker(&dest) else {
        panic!("restored marker")
    };
    assert_eq!(copy, original);
    assert!(cairn::paths::Root::new(&dest).is_ok());
}

#[test]
fn restoring_never_touches_a_folder_that_already_has_files() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();

    let busy = out.path().join("Busy");
    common::write(&busy.join("keep.md"), "# Keep\n");
    let err = restore(&zip, &busy).unwrap_err();
    assert!(err.to_string().contains("already has files"), "{err}");
    assert_eq!(tree(&busy).len(), 1);

    // Not inside a live documentation folder either.
    let err = restore(&zip, &ws.path.join("Guides/Copy")).unwrap_err();
    assert!(
        err.to_string().contains("inside a documentation folder"),
        "{err}"
    );
    assert!(!ws.path.join("Guides/Copy").exists());

    // An existing empty folder is fine.
    let empty = out.path().join("Empty");
    std::fs::create_dir(&empty).unwrap();
    restore(&zip, &empty).unwrap();
    assert!(empty.join("shared-docs.json").exists());
}

#[test]
fn a_damaged_archive_fails_and_leaves_nothing_behind() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();
    let bytes = std::fs::read(&zip).unwrap();

    let cut = out.path().join("cut.zip");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    let not_a_zip = out.path().join("text.zip");
    std::fs::write(&not_a_zip, "this is not a zip").unwrap();

    for bad in [&cut, &not_a_zip] {
        assert!(verify(bad).is_err());
        let dest = out.path().join("Dest");
        assert!(restore(bad, &dest).is_err());
        assert!(!dest.exists(), "nothing is created for a bad archive");
    }
    let leftovers: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("cairn-restore"))
        .collect();
    assert!(leftovers.is_empty(), "no staging folder left behind");
}

// ------------------------------------------------------------ forged archives

/// Build a zip by hand: `manifest` JSON plus the listed entries.
fn forge(path: &Path, manifest: Option<&str>, entries: &[(&str, &[u8])]) {
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    let options = SimpleFileOptions::default();
    if let Some(manifest) = manifest {
        writer.start_file(MANIFEST_NAME, options).unwrap();
        writer.write_all(manifest.as_bytes()).unwrap();
    }
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap();
}

const ID: &str = "11111111-2222-4333-8444-555555555555";

fn marker_bytes(id: &str) -> Vec<u8> {
    format!(
        "{{\"kind\":\"shared-docs\",\"schema_version\":1,\"instance_id\":\"{id}\",\"display_name\":\"Docs\"}}"
    )
    .into_bytes()
}

fn file_json(path: &str, bytes: &[u8]) -> String {
    format!(
        "{{\"path\":{path:?},\"size\":{},\"sha256\":\"{}\"}}",
        bytes.len(),
        hex::encode(Sha256::digest(bytes))
    )
}

fn manifest_json(format: &str, version: u32, id: &str, files: &[String]) -> String {
    format!(
        "{{\"format\":{format:?},\"format_version\":{version},\"instance_id\":{id:?},\
         \"display_name\":\"Docs\",\"created_at\":\"2026-01-01T00:00:00Z\",\
         \"cairn_version\":\"1.0.1\",\"files\":[{}]}}",
        files.join(",")
    )
}

/// A manifest and entries for a small, valid backup, plus `extra` files.
fn valid(extra: &[(&str, &[u8])]) -> (String, Vec<(String, Vec<u8>)>) {
    let mut entries: Vec<(String, Vec<u8>)> = vec![
        ("shared-docs.json".into(), marker_bytes(ID)),
        ("Page.md".into(), b"# Page\n".to_vec()),
    ];
    entries.extend(extra.iter().map(|(n, b)| (n.to_string(), b.to_vec())));
    let files: Vec<String> = entries.iter().map(|(n, b)| file_json(n, b)).collect();
    (manifest_json("cairn-backup", 1, ID, &files), entries)
}

fn as_refs(entries: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8])> {
    entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect()
}

fn forge_valid(path: &Path, extra: &[(&str, &[u8])]) {
    let (manifest, entries) = valid(extra);
    forge(path, Some(&manifest), &as_refs(&entries));
}

/// A valid backup whose contents list also names `listed`, which the zip
/// itself holds only once (a zip can't hold the same name twice).
fn forge_listed_only(path: &Path, listed: &str, bytes: &[u8]) {
    let (_, entries) = valid(&[]);
    let mut files: Vec<String> = entries.iter().map(|(n, b)| file_json(n, b)).collect();
    files.push(file_json(listed, bytes));
    let manifest = manifest_json("cairn-backup", 1, ID, &files);
    forge(path, Some(&manifest), &as_refs(&entries));
}

#[test]
fn a_hand_made_valid_archive_is_accepted() {
    let out = out_dir();
    let zip = out.path().join("ok.zip");
    forge_valid(&zip, &[("Guides/Other.md", b"# Other\n")]);
    assert_eq!(verify(&zip).unwrap().files.len(), 3);
    restore(&zip, &out.path().join("Restored")).unwrap();
}

type Build = Box<dyn Fn(&Path)>;

fn bad_archives() -> Vec<(&'static str, Build)> {
    let payload: &'static [u8] = b"# Evil\n";
    vec![
        (
            "no manifest",
            Box::new(|p| forge(p, None, &[("Page.md", payload)])),
        ),
        (
            "wrong format",
            Box::new(|p| {
                let m = manifest_json(
                    "something-else",
                    1,
                    ID,
                    &[file_json("shared-docs.json", &marker_bytes(ID))],
                );
                forge(p, Some(&m), &[("shared-docs.json", &marker_bytes(ID))]);
            }),
        ),
        (
            "newer format",
            Box::new(|p| {
                let m = manifest_json(
                    "cairn-backup",
                    2,
                    ID,
                    &[file_json("shared-docs.json", &marker_bytes(ID))],
                );
                forge(p, Some(&m), &[("shared-docs.json", &marker_bytes(ID))]);
            }),
        ),
        (
            "parent folder",
            Box::new(|p| forge_valid(p, &[("../escape.md", payload)])),
        ),
        (
            "deep parent folder",
            Box::new(|p| forge_valid(p, &[("Guides/../../escape.md", payload)])),
        ),
        (
            "absolute path",
            Box::new(|p| forge_valid(p, &[("/tmp/escape.md", payload)])),
        ),
        (
            "backslash path",
            Box::new(|p| forge_valid(p, &[("..\\escape.md", payload)])),
        ),
        (
            "drive path",
            Box::new(|p| forge_valid(p, &[("C:escape.md", payload)])),
        ),
        (
            "alternate stream",
            Box::new(|p| forge_valid(p, &[("Page.md:stream", payload)])),
        ),
        (
            "reserved name",
            Box::new(|p| forge_valid(p, &[("Guides/CON.md", payload)])),
        ),
        (
            "lock file",
            Box::new(|p| forge_valid(p, &[("_system/locks/aaaa.lock", payload)])),
        ),
        (
            "manifest as a file",
            Box::new(|p| forge_listed_only(p, MANIFEST_NAME, payload)),
        ),
        (
            "duplicate path",
            Box::new(|p| forge_listed_only(p, "Page.md", b"# Page\n")),
        ),
        (
            "case-only duplicate",
            Box::new(|p| forge_listed_only(p, "page.MD", b"# Page\n")),
        ),
        (
            "no marker",
            Box::new(|p| {
                let m = manifest_json("cairn-backup", 1, ID, &[file_json("Page.md", b"# Page\n")]);
                forge(p, Some(&m), &[("Page.md", b"# Page\n")]);
            }),
        ),
        (
            "marker for another workspace",
            Box::new(|p| {
                let other = marker_bytes("99999999-2222-4333-8444-555555555555");
                let m = manifest_json(
                    "cairn-backup",
                    1,
                    ID,
                    &[file_json("shared-docs.json", &other)],
                );
                forge(p, Some(&m), &[("shared-docs.json", &other)]);
            }),
        ),
        (
            "file not in the manifest",
            Box::new(|p| {
                let (m, mut e) = valid(&[]);
                e.push(("Sneaky.md".into(), payload.to_vec()));
                forge(p, Some(&m), &as_refs(&e));
            }),
        ),
        (
            "file missing from the zip",
            Box::new(|p| {
                let (m, e) = valid(&[]);
                forge(p, Some(&m), &as_refs(&e[..1]));
            }),
        ),
        (
            "changed contents",
            Box::new(|p| {
                let (m, mut e) = valid(&[]);
                e[1].1 = b"# Page, quietly edited\n".to_vec();
                forge(p, Some(&m), &as_refs(&e));
            }),
        ),
        (
            "same size, different contents",
            Box::new(|p| {
                let (m, mut e) = valid(&[]);
                e[1].1 = b"# Pagf\n".to_vec();
                forge(p, Some(&m), &as_refs(&e));
            }),
        ),
    ]
}

/// What the message for each bad archive must say, so a case can't pass
/// by failing for the wrong reason.
fn reason_for(what: &str) -> &'static str {
    match what {
        "no manifest" => "no cairn-backup.json",
        "wrong format" => "isn't a Cairn backup",
        "newer format" => "newer Cairn",
        "parent folder" | "deep parent folder" | "absolute path" | "backslash path"
        | "drive path" | "alternate stream" | "reserved name" => "unsafe",
        "lock file" => "edit locks",
        "manifest as a file" => "reserved",
        "duplicate path" | "case-only duplicate" => "more than once",
        "no marker" => "no shared-docs.json",
        "marker for another workspace" => "different documentation folder",
        "file not in the manifest" => "not in its contents list",
        "file missing from the zip" => "incomplete",
        "changed contents" => "not the size",
        "same size, different contents" => "checksum",
        other => panic!("no expected reason for {other}"),
    }
}

#[test]
fn unsafe_or_unsupported_archives_are_refused_and_write_nothing() {
    let out = out_dir();
    let work = out.path().join("work");
    std::fs::create_dir(&work).unwrap();
    let dest = work.join("Dest");
    let escape = work.join("escape.md");

    for (what, build) in bad_archives() {
        let zip = out.path().join("forged.zip");
        build(&zip);
        let reason = reason_for(what);
        let err = verify(&zip).expect_err(what).to_string();
        assert!(err.contains(reason), "verify said {err:?} for: {what}");
        let err = restore(&zip, &dest).expect_err(what).to_string();
        assert!(err.contains(reason), "restore said {err:?} for: {what}");
        assert!(!dest.exists(), "left a folder behind: {what}");
        assert!(!escape.exists(), "wrote outside the destination: {what}");
        std::fs::remove_file(&zip).unwrap();
    }
    let strays: Vec<_> = std::fs::read_dir(&work).unwrap().flatten().collect();
    assert!(
        strays.is_empty(),
        "left files next to the destination: {strays:?}"
    );
}

#[test]
fn a_link_inside_an_archive_is_refused() {
    let out = out_dir();
    let zip = out.path().join("link.zip");
    let (manifest, entries) = valid(&[]);
    {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
        let options = SimpleFileOptions::default();
        writer.start_file(MANIFEST_NAME, options).unwrap();
        writer.write_all(manifest.as_bytes()).unwrap();
        writer.start_file("shared-docs.json", options).unwrap();
        writer.write_all(&entries[0].1).unwrap();
        // "Page.md" is a link to somewhere else instead of a file.
        writer
            .add_symlink("Page.md", "/etc/passwd", options)
            .unwrap();
        writer.finish().unwrap();
    }
    assert!(verify(&zip).is_err());
    assert!(restore(&zip, &out.path().join("Dest")).is_err());
    assert!(!out.path().join("Dest").exists());
}

// ------------------------------------------------------ files that change

#[test]
fn a_file_saved_during_the_backup_is_copied_whole_and_reported() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let first = enumerate_backup(&ws.root).unwrap();
    let copied_first = first.entries[0].rel.clone();
    let old_first = std::fs::read(ws.path.join(&copied_first)).unwrap();
    let calls = Cell::new(0usize);
    let not_cancelled = || false;
    let on_file = |_done: usize| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            // A page not copied yet, and the page that has just been copied.
            common::write(
                &ws.path.join("Root page.md"),
                "# Root, edited while the backup was running\n",
            );
            common::write(
                &ws.path.join(&copied_first),
                "{\"changed\":\"after it was copied\"}",
            );
        }
    };
    let progress = Progress {
        cancelled: &not_cancelled,
        on_file: &on_file,
    };
    let summary = create(&ws.root, &zip, first, &progress).unwrap().unwrap();

    // No need to start over: each file was copied as one whole version.
    assert_eq!(summary.passes, 1);
    assert!(summary.changed_during.contains(&"Root page.md".to_string()));
    assert!(summary.changed_during.contains(&copied_first));

    let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
    let mut read = |name: &str| {
        let mut bytes = Vec::new();
        archive
            .by_name(name)
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    };
    let root = String::from_utf8(read("Root page.md")).unwrap();
    assert!(
        root.contains("edited while the backup"),
        "copied after the edit"
    );
    assert_eq!(read(&copied_first), old_first, "copied before the edit");
    verify(&zip).unwrap();
}

#[test]
fn a_file_that_vanishes_once_is_handled_by_trying_again() {
    let ws = common::workspace();
    sample(&ws);
    common::write(&ws.path.join("zz1.md"), "# Last\n");
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let not_cancelled = || false;
    let on_file = |done: usize| {
        if done == 1 {
            let _ = std::fs::remove_file(ws.path.join("zz1.md"));
        }
    };
    let progress = Progress {
        cancelled: &not_cancelled,
        on_file: &on_file,
    };
    let first = enumerate_backup(&ws.root).unwrap();
    let summary = create(&ws.root, &zip, first, &progress).unwrap().unwrap();
    assert_eq!(summary.passes, 2, "the list was made again without it");
    assert!(!names(&zip).contains(&"zz1.md".to_string()));
    verify(&zip).unwrap();
}

#[test]
fn files_that_keep_disappearing_make_the_backup_give_up_and_name_them() {
    let ws = common::workspace();
    sample(&ws);
    for i in 1..=3 {
        common::write(&ws.path.join(format!("zz{i}.md")), "# Last\n");
    }
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let pass = Cell::new(0usize);
    let not_cancelled = || false;
    // Each attempt, a different page that hasn't been copied yet is removed.
    let on_file = |done: usize| {
        if done == 1 {
            pass.set(pass.get() + 1);
            let _ = std::fs::remove_file(ws.path.join(format!("zz{}.md", pass.get())));
        }
    };
    let progress = Progress {
        cancelled: &not_cancelled,
        on_file: &on_file,
    };
    let first = enumerate_backup(&ws.root).unwrap();
    let err = create(&ws.root, &zip, first, &progress).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("kept changing"), "{message}");
    for name in ["zz1.md", "zz2.md", "zz3.md"] {
        assert!(message.contains(name), "{name} not named: {message}");
    }
    assert!(!zip.exists(), "an unreliable backup is not left behind");
    assert!(
        std::fs::read_dir(out.path()).unwrap().next().is_none(),
        "no partial file is left behind"
    );
}

#[test]
fn cancelling_removes_the_partial_backup() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let seen = Cell::new(0usize);
    let cancelled = || seen.get() >= 2;
    let on_file = |_done: usize| seen.set(seen.get() + 1);
    let progress = Progress {
        cancelled: &cancelled,
        on_file: &on_file,
    };
    let first = enumerate_backup(&ws.root).unwrap();
    assert!(create(&ws.root, &zip, first, &progress).unwrap().is_none());
    assert!(std::fs::read_dir(out.path()).unwrap().next().is_none());
}

#[cfg(unix)]
#[test]
fn an_unreadable_file_fails_the_backup_and_names_it() {
    use std::os::unix::fs::PermissionsExt;
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let secret = ws.path.join("Guides/private.md");
    common::write(&secret, "# Private\n");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    let can_read_anyway = std::fs::read(&secret).is_ok();
    let result = create_backup(&ws.root, &zip);
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
    if can_read_anyway {
        eprintln!("skipped: this user can read every file");
        return;
    }
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Guides/private.md"), "{err}");
    assert!(std::fs::read_dir(out.path()).unwrap().next().is_none());
}

#[test]
fn a_backup_never_overwrites_an_existing_file() {
    let ws = common::workspace();
    let out = out_dir();
    let zip: PathBuf = out.path().join("backup.zip");
    std::fs::write(&zip, "precious").unwrap();
    assert!(create_backup(&ws.root, &zip).is_err());
    assert_eq!(std::fs::read(&zip).unwrap(), b"precious");
}

#[cfg(unix)]
#[test]
fn only_the_owner_can_read_a_backup() {
    use std::os::unix::fs::PermissionsExt;
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();
    let mode = std::fs::metadata(&zip).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "backup readable by others: {mode:o}");
}

#[test]
fn a_backup_that_could_not_be_restored_is_not_kept() {
    let ws = common::workspace();
    common::write(&ws.path.join("Notes.md"), "# Notes\n");
    common::write(&ws.path.join("notes.md"), "# notes\n");
    // On a case-insensitive disk the second write replaced the first.
    let both = std::fs::read_dir(&ws.path)
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("notes.md")
        })
        .count();
    if both < 2 {
        eprintln!("skipped: this disk treats Notes.md and notes.md as one file");
        return;
    }
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let err = create_backup(&ws.root, &zip).unwrap_err();
    assert!(err.to_string().contains("more than once"), "{err}");
    assert!(
        std::fs::read_dir(out.path()).unwrap().next().is_none(),
        "an unrestorable backup is not left behind"
    );
}

#[cfg(unix)]
#[test]
fn a_backup_never_follows_a_file_swapped_for_a_link() {
    let ws = common::workspace();
    sample(&ws);
    let outside = tempfile::tempdir().unwrap();
    common::write(&outside.path().join("secret.txt"), "private");
    // A page that is a link to a file outside the documentation.
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        ws.path.join("Guides/linked.md"),
    )
    .unwrap();
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let summary = create_backup(&ws.root, &zip).unwrap();
    assert!(
        summary
            .skipped_links
            .contains(&"Guides/linked.md".to_string())
    );
    let bytes = std::fs::read(&zip).unwrap();
    assert!(
        !bytes.windows(7).any(|w| w == b"private"),
        "content from outside the documentation is in the backup"
    );
}

/// Someone keeps saving pages while backups are made, the way an editor or
/// Notepad on another computer would (a temporary file renamed over the
/// page). Every backup must either verify as a whole, or fail saying files
/// kept changing, and never leave a half-written file behind.
fn backups_while_people_save(pause: std::time::Duration) -> (usize, usize) {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let ws = common::workspace();
    sample(&ws);
    for i in 0..40 {
        common::write(
            &ws.path.join(format!("Bulk/page{i:02}.md")),
            &format!("# Page {i}\n"),
        );
    }
    let out = out_dir();
    let stop = Arc::new(AtomicBool::new(false));
    let saves = Arc::new(AtomicUsize::new(0));

    let writer = {
        let (stop, saves) = (stop.clone(), saves.clone());
        let pages = [
            ws.path.join("Root page.md"),
            ws.path.join("Guides/setup.md"),
            ws.path.join("Bulk/page07.md"),
            ws.path.join("Bulk/page33.md"),
        ];
        std::thread::spawn(move || {
            let mut n = 0usize;
            while !stop.load(Ordering::SeqCst) {
                let page = &pages[n % pages.len()];
                // Different lengths each time, so the change is always visible.
                let text = format!("# Saved {n}\n\n{}\n", "x".repeat(n % 50));
                let _ = cairn::fsutil::write_atomic(page, text.as_bytes());
                saves.fetch_add(1, Ordering::SeqCst);
                n += 1;
                std::thread::sleep(pause);
            }
        })
    };

    let (mut finished, mut gave_up) = (0, 0);
    for round in 0..25 {
        let zip = out.path().join(format!("backup-{round}.zip"));
        match create_backup(&ws.root, &zip) {
            Ok(_) => {
                verify(&zip).expect("a backup that was kept must verify");
                finished += 1;
            }
            Err(cairn::error::CairnError::Conflict(message)) => {
                assert!(message.contains("kept changing"), "{message}");
                assert!(!zip.exists(), "an unreliable backup was left behind");
                gave_up += 1;
            }
            Err(other) => panic!("unexpected failure while people were saving: {other}"),
        }
    }
    stop.store(true, Ordering::SeqCst);
    writer.join().unwrap();

    assert!(saves.load(Ordering::SeqCst) > 5, "the writer barely ran");
    assert_eq!(finished + gave_up, 25);
    let leftovers: Vec<String> = std::fs::read_dir(out.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("cairn-part") || n.starts_with('.'))
        .collect();
    assert!(
        leftovers.is_empty(),
        "half-written files left: {leftovers:?}"
    );
    (finished, gave_up)
}

/// Someone saving constantly (thousands of times a second). Saving never
/// leaves a page half written, so every backup is a whole, verified copy
/// (and says what changed while it ran).
#[test]
fn backups_during_constant_saving_still_finish_and_verify() {
    let (finished, gave_up) = backups_while_people_save(std::time::Duration::from_micros(300));
    assert_eq!((finished, gave_up), (25, 0));
}

/// Someone saving now and then, which is a busy office.
#[test]
fn backups_during_ordinary_saving_finish_and_verify() {
    let (finished, gave_up) = backups_while_people_save(std::time::Duration::from_millis(40));
    assert_eq!((finished, gave_up), (25, 0));
}

// ---------------------------------------------------------------- progress

#[test]
fn verifying_and_restoring_report_each_file_as_it_is_checked() {
    use std::cell::RefCell;
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();
    let total = verify(&zip).unwrap().files.len();

    let seen = RefCell::new(Vec::new());
    verify_with(&zip, &|done, of| seen.borrow_mut().push((done, of))).unwrap();
    let expected: Vec<(usize, usize)> = (1..=total).map(|n| (n, total)).collect();
    assert_eq!(*seen.borrow(), expected);

    seen.borrow_mut().clear();
    let room = |_: &Path| Ok(u64::MAX);
    restore_with(&zip, &out.path().join("Restored"), &room, &|done, of| {
        seen.borrow_mut().push((done, of));
    })
    .unwrap();
    assert_eq!(*seen.borrow(), expected);
}

// ------------------------------------------------------------ free space

fn progress_none<'a>(never: &'a dyn Fn() -> bool, nothing: &'a dyn Fn(usize)) -> Progress<'a> {
    Progress {
        cancelled: never,
        on_file: nothing,
    }
}

#[test]
fn a_backup_with_no_room_is_refused_before_anything_is_written() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let (never, nothing) = (|| false, |_: usize| {});
    let first = enumerate_backup(&ws.root).unwrap();
    let almost_full = |_: &Path| Ok(1000u64);
    let err = create_checked(
        &ws.root,
        &zip,
        first,
        &progress_none(&never, &nothing),
        &almost_full,
    )
    .unwrap_err();
    let message = err.to_string();
    assert!(message.contains("free space"), "{message}");
    assert!(
        message.contains("1000 bytes"),
        "says how much is free: {message}"
    );
    assert!(
        message.contains("another drive"),
        "says what to do: {message}"
    );
    assert!(
        std::fs::read_dir(out.path()).unwrap().next().is_none(),
        "nothing was created"
    );
}

#[test]
fn a_backup_that_may_not_fit_goes_ahead_and_says_so() {
    let ws = common::workspace();
    sample(&ws);
    // Text squeezes to almost nothing, so this fits in far less room than its
    // size, but Cairn can't know that in advance.
    std::fs::write(ws.path.join("Big notes.md"), vec![b'a'; 24 * 1024 * 1024]).unwrap();
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    let (never, nothing) = (|| false, |_: usize| {});
    let first = enumerate_backup(&ws.root).unwrap();
    let tight = |_: &Path| Ok(20 * 1024 * 1024u64);
    let summary = create_checked(
        &ws.root,
        &zip,
        first,
        &progress_none(&never, &nothing),
        &tight,
    )
    .unwrap()
    .unwrap();
    let note = summary.space_note.expect("a warning about tight space");
    assert!(note.contains("free"), "{note}");
    verify(&zip).unwrap();
}

#[test]
fn a_backup_with_plenty_of_room_or_an_unknown_amount_just_goes_ahead() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let (never, nothing) = (|| false, |_: usize| {});
    for (name, space) in [
        (
            "plenty",
            Box::new(|_: &Path| Ok(u64::MAX)) as Box<dyn Fn(&Path) -> std::io::Result<u64>>,
        ),
        (
            "unknown",
            Box::new(|_: &Path| Err(std::io::Error::other("can't tell"))),
        ),
    ] {
        let zip = out.path().join(format!("{name}.zip"));
        let first = enumerate_backup(&ws.root).unwrap();
        let summary = create_checked(
            &ws.root,
            &zip,
            first,
            &progress_none(&never, &nothing),
            &*space,
        )
        .unwrap()
        .unwrap();
        assert!(summary.space_note.is_none(), "{name}");
        verify(&zip).unwrap();
    }
}

#[test]
fn a_restore_with_no_room_is_refused_and_leaves_nothing() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();
    let dest = out.path().join("Restored");
    let almost_full = |_: &Path| Ok(1000u64);
    let err = restore_checked(&zip, &dest, &almost_full).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("free space"), "{message}");
    assert!(message.contains("1000 bytes"), "{message}");
    assert!(message.contains("needs about"), "{message}");
    assert!(!dest.exists());
    let leftovers: Vec<_> = std::fs::read_dir(out.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, ["backup.zip"], "no staging folder either");
}

#[test]
fn a_restore_needs_exactly_what_the_backup_holds_plus_a_margin() {
    let ws = common::workspace();
    sample(&ws);
    let out = out_dir();
    let zip = out.path().join("backup.zip");
    create_backup(&ws.root, &zip).unwrap();
    let held: u64 = verify(&zip).unwrap().files.iter().map(|f| f.size).sum();
    let need = restore_space_needed(held);
    assert!(need > held);

    let one_short = move |_: &Path| Ok(need - 1);
    assert!(restore_checked(&zip, &out.path().join("A"), &one_short).is_err());
    let enough = move |_: &Path| Ok(need);
    restore_checked(&zip, &out.path().join("B"), &enough).unwrap();
    // If the amount can't be found out, the restore is not blocked.
    let unknown = |_: &Path| Err(std::io::Error::other("can't tell"));
    restore_checked(&zip, &out.path().join("C"), &unknown).unwrap();
}

// -------------------------------------------------------------- over HTTP

mod over_http {
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use cairn::backup::verify;
    use cairn::config::AppConfig;
    use cairn::server::{AppState, api::open_at, build_router};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::{common, sample};

    const PORT: u16 = 4323;

    fn state(ws: &common::TestWorkspace) -> (tempfile::TempDir, Arc<AppState>) {
        let home = tempfile::tempdir().unwrap();
        let cfg = AppConfig {
            persistent_drafts: false,
            ..AppConfig::default()
        };
        let state = AppState::new(home.path().to_path_buf(), cfg, None);
        state.set_port(PORT);
        open_at(&state, &ws.path).unwrap();
        (home, state)
    }

    fn request(st: &AppState, method: &str, uri: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("host", format!("127.0.0.1:{PORT}"))
            .header("x-cairn-token", &st.token)
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap()
    }

    async fn send(
        st: &Arc<AppState>,
        req: Request<Body>,
    ) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
        let res = build_router(st.clone()).oneshot(req).await.unwrap();
        let (status, headers) = (res.status(), res.headers().clone());
        let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        (status, headers, body)
    }

    /// Backup files currently waiting in the temporary folder.
    fn waiting_backups() -> BTreeSet<String> {
        std::fs::read_dir(std::env::temp_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.starts_with("cairn-backup-") || n.starts_with(".cairn-backup-"))
            .collect()
    }

    #[tokio::test]
    async fn a_backup_downloads_through_a_job_then_its_file_is_removed() {
        let ws = common::workspace();
        sample(&ws);
        let (_home, st) = state(&ws);
        let before = waiting_backups();

        let (status, _, body) = send(&st, request(&st, "POST", "/api/backup/jobs")).await;
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let started: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = started["id"].as_str().unwrap().to_string();
        assert!(started["file_name"].as_str().unwrap().contains("-backup-"));

        let mut finished = false;
        for _ in 0..200 {
            let (_, _, body) =
                send(&st, request(&st, "GET", &format!("/api/export/jobs/{id}"))).await;
            let s: serde_json::Value = serde_json::from_slice(&body).unwrap();
            match s["state"].as_str().unwrap() {
                "finished" => {
                    finished = true;
                    break;
                }
                "running" => tokio::time::sleep(Duration::from_millis(25)).await,
                other => panic!("backup ended as {other}: {s}"),
            }
        }
        assert!(finished);

        let file_uri = format!("/api/export/jobs/{id}/file");
        let (status, headers, zip) = send(&st, request(&st, "GET", &file_uri)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "application/zip");
        assert_eq!(headers["content-length"], zip.len().to_string().as_str());

        let out = tempfile::tempdir().unwrap();
        let saved = out.path().join("downloaded.zip");
        std::fs::write(&saved, &zip).unwrap();
        let manifest = verify(&saved).unwrap();
        assert!(
            manifest
                .files
                .iter()
                .any(|f| f.path.starts_with("_system/history/"))
        );

        // Handed over once, and the file in the temporary folder is gone.
        let (status, _, _) = send(&st, request(&st, "GET", &file_uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let mut leftover = true;
        for _ in 0..50 {
            if waiting_backups().difference(&before).next().is_none() {
                leftover = false;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(!leftover, "{:?}", waiting_backups().difference(&before));
    }
}
