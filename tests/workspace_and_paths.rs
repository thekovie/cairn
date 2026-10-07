//! Acceptance criteria 1, 12, and 13: discovery from a subfolder, path and
//! link escapes, and concurrent or interrupted first-run setup.

mod common;

use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Barrier};

use cairn::error::CairnError;
use cairn::paths::Root;
use cairn::workspace::{Discovery, InitTarget, MARKER_FILE, discover, initialize};

// ------------------------------------------------------------------ AC 1

#[test]
fn second_user_selecting_a_subfolder_discovers_the_same_root() {
    let ws = common::workspace();
    let deep = ws.path.join("Guides").join("Printers").join("Floor 2");
    fs::create_dir_all(&deep).unwrap();

    let id = |d: Discovery| match d {
        Discovery::Found { marker, root, .. } => (marker.instance_id, root),
        other => panic!("expected Found, got {other:?}"),
    };
    assert_eq!(
        id(discover(&ws.path).unwrap()),
        id(discover(&deep).unwrap())
    );
}

#[test]
fn discovery_without_marker_reports_not_found_and_stops_at_an_invalid_marker() {
    let dir = tempfile::tempdir().unwrap();
    let child = dir.path().join("child");
    fs::create_dir_all(&child).unwrap();
    assert!(matches!(
        discover(&child).unwrap(),
        Discovery::NotFound { .. }
    ));

    // An unusable marker stops discovery instead of silently choosing another workspace.
    fs::write(dir.path().join(MARKER_FILE), "{ not json").unwrap();
    assert!(matches!(
        discover(&child).unwrap(),
        Discovery::Invalid { .. }
    ));
}

#[test]
fn newer_schema_opens_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let marker = format!(
        r#"{{"kind":"shared-docs","schema_version":9,"instance_id":"{}","display_name":"Future"}}"#,
        uuid::Uuid::new_v4()
    );
    fs::write(dir.path().join(MARKER_FILE), marker).unwrap();
    match discover(dir.path()).unwrap() {
        Discovery::Found {
            read_only_reason: Some(reason),
            ..
        } => {
            assert!(reason.contains("newer version"))
        }
        other => panic!("expected read-only Found, got {other:?}"),
    }
}

// ----------------------------------------------------------------- AC 13

#[test]
fn concurrent_initialization_produces_one_identity() {
    // Races depend on timing, so repeat to make a lost race likely to show up.
    for _ in 0..25 {
        concurrent_initialization_round();
    }
}

fn concurrent_initialization_round() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("Shared");
    fs::create_dir_all(&target).unwrap();
    let threads = 8;
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|i| {
            let barrier = barrier.clone();
            let target = target.clone();
            std::thread::spawn(move || {
                barrier.wait();
                initialize(&InitTarget::Here(target), &format!("Docs {i}")).unwrap()
            })
        })
        .collect();
    let outcomes: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let ids: HashSet<_> = outcomes
        .iter()
        .map(|o| o.marker.instance_id.clone())
        .collect();
    assert_eq!(ids.len(), 1, "every racer must end with the same identity");
    assert_eq!(
        outcomes.iter().filter(|o| o.created).count(),
        1,
        "exactly one creator"
    );
    assert!(
        !target.join(".shared-docs-init.json").exists(),
        "journal cleaned up"
    );
}

#[test]
fn populated_unmarked_folder_is_refused_and_untouched() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("notes.txt"), "keep me").unwrap();
    let err = initialize(&InitTarget::Here(dir.path().to_path_buf()), "Docs").unwrap_err();
    assert!(matches!(err, CairnError::BadRequest(_)));
    assert_eq!(
        fs::read_to_string(dir.path().join("notes.txt")).unwrap(),
        "keep me"
    );
    assert!(!dir.path().join(MARKER_FILE).exists());
    assert_eq!(
        fs::read_dir(dir.path()).unwrap().count(),
        1,
        "nothing else was created"
    );
}

#[test]
fn a_folder_with_only_finder_files_counts_as_empty() {
    // A Mac's Finder leaves .DS_Store (and ._ files on shared drives) in
    // any folder it has shown.
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".DS_Store"), "finder").unwrap();
    fs::write(dir.path().join("._README"), "finder").unwrap();
    match discover(dir.path()).unwrap() {
        Discovery::NotFound { is_empty, .. } => assert!(is_empty),
        other => panic!("unexpected: {other:?}"),
    }
    let out = initialize(&InitTarget::Here(dir.path().to_path_buf()), "Docs").unwrap();
    assert!(out.created);
    assert!(dir.path().join(MARKER_FILE).exists());
}

#[test]
fn existing_marker_is_never_overwritten() {
    let ws = common::workspace();
    let before = fs::read(ws.path.join(MARKER_FILE)).unwrap();
    let again = initialize(&InitTarget::Here(ws.path.clone()), "Another Name").unwrap();
    assert!(!again.created);
    assert_eq!(fs::read(ws.path.join(MARKER_FILE)).unwrap(), before);
}

#[test]
fn interrupted_initialization_is_retryable_with_the_same_identity() {
    let dir = tempfile::tempdir().unwrap();
    // Simulate a crash after the journal and some starter content were written.
    let fixed_id = uuid::Uuid::new_v4().to_string();
    let journal = serde_json::json!({
        "instance_id": fixed_id,
        "display_name": "Half Done",
        "entries": ["Getting-Started", "Guides", "Troubleshooting", "Reference", "README.md", "_system"]
    });
    fs::write(
        dir.path().join(".shared-docs-init.json"),
        journal.to_string(),
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("Guides")).unwrap();
    fs::write(dir.path().join("README.md"), "# Half Done\n").unwrap();

    let out = initialize(&InitTarget::Here(dir.path().to_path_buf()), "Ignored").unwrap();
    assert_eq!(out.marker.instance_id, fixed_id);
    assert_eq!(out.marker.display_name, "Half Done");
    assert_eq!(
        fs::read_to_string(dir.path().join("README.md")).unwrap(),
        "# Half Done\n",
        "existing README is not overwritten"
    );
}

// ----------------------------------------------------------------- AC 12

#[test]
fn crafted_paths_are_rejected() {
    let ws = common::workspace();
    let bad = [
        "../outside.md",
        "Guides/../../outside.md",
        "..\\outside.md",
        "/etc/passwd",
        "\\Windows\\win.ini",
        "C:\\Windows\\win.ini",
        "C:/Windows/win.ini",
        "\\\\server\\share\\x.md",
        "Guides/a.md:stream",
        "Guides/CON",
        "Guides/nul.txt",
        "Guides/trailing.",
        "Guides/ leading",
        "Guides//double.md",
        "Guides/./dot.md",
        "Guides/\0nul.md",
    ];
    for path in bad {
        assert!(
            matches!(
                ws.root.resolve_for_create(path),
                Err(CairnError::PathRejected(_))
            ),
            "{path:?} should be rejected"
        );
    }
    assert!(ws.root.resolve_for_create("Guides/ok page.md").is_ok());
}

#[cfg(windows)]
fn make_dir_link(link: &std::path::Path, target: &std::path::Path) -> bool {
    // Junctions need no special privilege on Windows.
    std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn make_dir_link(link: &std::path::Path, target: &std::path::Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[test]
fn directory_links_cannot_escape_the_root() {
    let ws = common::workspace();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.md"), "secret").unwrap();
    let link = ws.path.join("Guides").join("escape");
    assert!(
        make_dir_link(&link, outside.path()),
        "could not create a directory link"
    );

    for path in [
        "Guides/escape/secret.md",
        "Guides/escape",
        "Guides/escape/new.md",
    ] {
        assert!(
            matches!(
                ws.root.resolve_for_create(path),
                Err(CairnError::PathRejected(_))
            ),
            "{path:?} must not pass through a link"
        );
    }
    assert!(Root::new(&ws.path).unwrap().resolve("README.md").is_ok());
}

#[cfg(windows)]
#[test]
fn file_symlinks_are_rejected_when_the_os_allows_creating_them() {
    let ws = common::workspace();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.md");
    fs::write(&secret, "secret").unwrap();
    let link = ws.path.join("Guides").join("sneaky.md");
    // Needs Developer Mode or admin rights; nothing to test without them.
    if std::os::windows::fs::symlink_file(&secret, &link).is_err() {
        return;
    }
    assert!(matches!(
        ws.root.resolve("Guides/sneaky.md"),
        Err(CairnError::PathRejected(_))
    ));
}
