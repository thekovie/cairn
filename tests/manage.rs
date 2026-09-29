//! Renaming, moving, and deleting pages and folders: files, pictures, and
//! earlier versions move together, links stay right, pages being edited are
//! never touched, and deleted things can be restored exactly.

mod common;

use std::fs;

use cairn::error::CairnError;
use cairn::fsutil::sha256_hex;
use cairn::history;
use cairn::locks::{self, Acquire};
use cairn::manage::{move_folder, move_page, pages_under};
use cairn::trash;

fn read(ws: &common::TestWorkspace, rel: &str) -> String {
    fs::read_to_string(ws.root.resolve(rel).unwrap()).unwrap()
}

fn exists(ws: &common::TestWorkspace, rel: &str) -> bool {
    ws.root.resolve(rel).is_ok()
}

/// Guides/printer.md with a picture and an earlier version, plus pages that
/// link to it from the top level and from its own folder.
fn seed(ws: &common::TestWorkspace) {
    common::write(
        &ws.path.join("Guides/printer.md"),
        "# Printer\n\n![Tray](printer.assets/tray.png)\n\nSee [setup](setup.md).\n",
    );
    common::write(
        &ws.path.join("Guides/setup.md"),
        "# Setup\n\n[Printer](printer.md)\n",
    );
    common::write(
        &ws.path.join("index.md"),
        "# Index\n\n- [Printer help](Guides/printer.md#paper)\n",
    );
    fs::create_dir_all(ws.path.join("Guides/printer.assets")).unwrap();
    fs::write(
        ws.path.join("Guides/printer.assets/tray.png"),
        common::png(4, 4, 9),
    )
    .unwrap();
    history::save_version(&ws.root, "Guides/printer.md", b"# Printer (old)\n").unwrap();
    fs::create_dir_all(ws.path.join("Hardware")).unwrap();
}

fn all(ws: &common::TestWorkspace) -> Vec<String> {
    pages_under(&ws.root, "").unwrap()
}

#[test]
fn moving_a_page_takes_its_pictures_and_versions_and_fixes_every_link() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");

    let out = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/printer.md",
        "Hardware/printer.md",
        None,
    )
    .unwrap();
    assert_eq!(out.path, "Hardware/printer.md");
    assert!(!exists(&ws, "Guides/printer.md"));
    assert!(exists(&ws, "Hardware/printer.assets/tray.png"));
    assert!(!exists(&ws, "Guides/printer.assets"));

    // Its own links now work from the new folder.
    let moved = read(&ws, "Hardware/printer.md");
    assert!(moved.contains("(printer.assets/tray.png)"));
    assert!(moved.contains("(../Guides/setup.md)"));

    // Links to it were updated, with the old text kept as an earlier version.
    assert!(read(&ws, "index.md").contains("(Hardware/printer.md#paper)"));
    assert!(read(&ws, "Guides/setup.md").contains("(../Hardware/printer.md)"));
    assert_eq!(out.links.updated.len(), 2);
    assert!(out.links.skipped.is_empty());
    assert_eq!(
        history::list_versions(&ws.root, "index.md").unwrap().len(),
        1
    );

    // Earlier versions came along, plus the text from just before the move.
    let versions = history::list_versions(&ws.root, "Hardware/printer.md").unwrap();
    assert_eq!(versions.len(), 2);
    assert!(
        history::list_versions(&ws.root, "Guides/printer.md")
            .unwrap()
            .is_empty()
    );

    // No edit locks are left behind.
    assert!(locks::list_locks(&ws.root).unwrap().is_empty());
}

#[test]
fn renaming_a_page_changes_its_title_and_file_name() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let out = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/printer.md",
        "Guides/office-printer.md",
        Some("Office printer"),
    )
    .unwrap();
    let text = read(&ws, &out.path);
    assert!(text.starts_with("# Office printer\n"));
    assert!(text.contains("(office-printer.assets/tray.png)"));
    assert!(read(&ws, "Guides/setup.md").contains("(office-printer.md)"));

    // Same file name, new title: only the heading changes.
    let out = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/setup.md",
        "Guides/setup.md",
        Some("Setting up"),
    )
    .unwrap();
    assert!(read(&ws, &out.path).starts_with("# Setting up\n"));
}

#[test]
fn a_page_being_edited_is_not_moved_and_nothing_changes() {
    let ws = common::workspace();
    seed(&ws);
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");
    assert!(matches!(
        locks::acquire(&ws.root, "Guides/printer.md", &alex).unwrap(),
        Acquire::Acquired(_)
    ));
    let before = read(&ws, "index.md");
    let err = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/printer.md",
        "Hardware/printer.md",
        None,
    )
    .unwrap_err();
    assert!(matches!(err, CairnError::Locked(ref m) if m.contains("Alex is editing")));
    assert!(exists(&ws, "Guides/printer.md"));
    assert!(!exists(&ws, "Hardware/printer.md"));
    assert_eq!(read(&ws, "index.md"), before);
    // Alex still holds the lock.
    let lock = locks::status(&ws.root, "Guides/printer.md", &alex)
        .unwrap()
        .unwrap();
    assert!(lock.is_mine);
}

#[test]
fn a_linking_page_someone_is_editing_is_skipped_and_reported() {
    let ws = common::workspace();
    seed(&ws);
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");
    locks::acquire(&ws.root, "index.md", &alex).unwrap();
    let before = read(&ws, "index.md");
    let out = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/printer.md",
        "Hardware/printer.md",
        None,
    )
    .unwrap();
    assert_eq!(read(&ws, "index.md"), before);
    assert_eq!(out.links.skipped.len(), 1);
    assert_eq!(out.links.skipped[0].path, "index.md");
    assert!(out.links.skipped[0].reason.contains("Alex"));
}

#[test]
fn a_page_is_not_moved_onto_another_page() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let err = move_page(
        &ws.root,
        &sam,
        &all(&ws),
        "Guides/printer.md",
        "Guides/setup.md",
        None,
    )
    .unwrap_err();
    assert!(matches!(err, CairnError::Conflict(_)));
    assert!(read(&ws, "Guides/setup.md").starts_with("# Setup"));
}

#[test]
fn renaming_a_folder_moves_everything_and_fixes_links_in_and_out() {
    let ws = common::workspace();
    seed(&ws);
    common::write(
        &ws.path.join("Guides/Deep/tips.md"),
        "# Tips\n\n[Home](../../index.md)\n",
    );
    let sam = common::identity("Sam");
    let out = move_folder(&ws.root, &sam, &all(&ws), "Guides", "Hardware/How-to").unwrap();
    assert_eq!(out.path, "Hardware/How-to");
    assert!(!exists(&ws, "Guides"));
    assert!(exists(&ws, "Hardware/How-to/printer.assets/tray.png"));
    // Links between pages inside the folder were already right and are untouched.
    assert!(read(&ws, "Hardware/How-to/setup.md").contains("(printer.md)"));
    // Links from inside to outside follow the new depth.
    assert!(read(&ws, "Hardware/How-to/Deep/tips.md").contains("(../../../index.md)"));
    // Links from outside point into the new place.
    assert!(read(&ws, "index.md").contains("(Hardware/How-to/printer.md#paper)"));
    // Earlier versions moved with the folder.
    assert_eq!(
        history::list_versions(&ws.root, "Hardware/How-to/printer.md")
            .unwrap()
            .len(),
        1
    );
    assert!(locks::list_locks(&ws.root).unwrap().is_empty());
}

#[test]
fn a_folder_cannot_move_inside_itself_or_onto_another_folder() {
    let ws = common::workspace();
    seed(&ws);
    fs::create_dir_all(ws.path.join("Guides/Sub")).unwrap();
    let sam = common::identity("Sam");
    assert!(move_folder(&ws.root, &sam, &all(&ws), "Guides", "Guides/Sub/Guides").is_err());
    assert!(matches!(
        move_folder(&ws.root, &sam, &all(&ws), "Guides", "Hardware").unwrap_err(),
        CairnError::Conflict(_)
    ));
    assert!(move_folder(&ws.root, &sam, &all(&ws), "_system", "Other").is_err());
    assert!(exists(&ws, "Guides/printer.md"));
}

#[test]
fn a_folder_with_a_page_being_edited_is_not_moved() {
    let ws = common::workspace();
    seed(&ws);
    let alex = common::identity("Alex");
    let sam = common::identity("Sam");
    locks::acquire(&ws.root, "Guides/setup.md", &alex).unwrap();
    let err = move_folder(&ws.root, &sam, &all(&ws), "Guides", "How-to").unwrap_err();
    assert!(matches!(err, CairnError::Locked(_)));
    assert!(exists(&ws, "Guides/printer.md"));
    // Sam's locks on the other pages were given back.
    assert_eq!(locks::list_locks(&ws.root).unwrap().len(), 1);
}

#[test]
fn a_deleted_page_and_its_pictures_can_be_restored() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let text = read(&ws, "Guides/printer.md");
    let hash = sha256_hex(text.as_bytes());
    let item =
        trash::trash_page(&ws.root, &sam, "Guides/printer.md", "Printer", Some(&hash)).unwrap();
    assert!(!exists(&ws, "Guides/printer.md"));
    assert!(!exists(&ws, "Guides/printer.assets"));
    let listed = trash::list(&ws.root);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "Printer");
    assert_eq!(listed[0].deleted_by, "Sam");

    trash::restore(&ws.root, &sam, &item.id).unwrap();
    assert_eq!(read(&ws, "Guides/printer.md"), text);
    assert!(exists(&ws, "Guides/printer.assets/tray.png"));
    assert!(trash::list(&ws.root).is_empty());
    assert_eq!(
        history::list_versions(&ws.root, "Guides/printer.md")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn a_page_changed_since_it_was_viewed_is_not_deleted() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let err = trash::trash_page(
        &ws.root,
        &sam,
        "Guides/printer.md",
        "Printer",
        Some("stale"),
    )
    .unwrap_err();
    assert!(matches!(err, CairnError::Conflict(_)));
    assert!(exists(&ws, "Guides/printer.md"));
    assert!(trash::list(&ws.root).is_empty());
}

#[test]
fn restoring_never_overwrites_a_newer_page() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let item = trash::trash_page(&ws.root, &sam, "Guides/setup.md", "Setup", None).unwrap();
    common::write(&ws.path.join("Guides/setup.md"), "# New setup\n");
    assert!(matches!(
        trash::restore(&ws.root, &sam, &item.id).unwrap_err(),
        CairnError::Conflict(_)
    ));
    assert_eq!(read(&ws, "Guides/setup.md"), "# New setup\n");
    assert_eq!(trash::list(&ws.root).len(), 1);
}

#[test]
fn a_deleted_folder_comes_back_whole() {
    let ws = common::workspace();
    seed(&ws);
    let sam = common::identity("Sam");
    let item = trash::trash_folder(&ws.root, &sam, "Guides").unwrap();
    assert_eq!(item.page_count, 2);
    assert!(!exists(&ws, "Guides"));
    trash::restore(&ws.root, &sam, &item.id).unwrap();
    assert!(exists(&ws, "Guides/printer.md"));
    assert!(exists(&ws, "Guides/setup.md"));
    assert!(exists(&ws, "Guides/printer.assets/tray.png"));
    assert!(locks::list_locks(&ws.root).unwrap().is_empty());
}
