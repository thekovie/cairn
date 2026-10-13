//! The page index: it carries each page's review date and metadata
//! problems, and it never quietly forgets a page it could not read.

mod common;

use cairn::search::SearchIndex;

fn page(ws: &common::TestWorkspace, rel: &str, front: &str) {
    common::write(
        &ws.path.join(rel),
        &format!("---\n{front}\n---\n# {rel}\n\nText.\n"),
    );
}

fn refreshed(ws: &common::TestWorkspace) -> SearchIndex {
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    index
}

#[test]
fn a_review_falls_due_a_year_after_the_last_one() {
    let ws = common::workspace();
    page(&ws, "A.md", "last_reviewed: 2025-01-01");
    page(&ws, "Leap.md", "last_reviewed: 2024-03-01");
    page(&ws, "Never.md", "owner: Sam");
    page(&ws, "Bad.md", "last_reviewed: yesterday");
    let index = refreshed(&ws);

    assert_eq!(
        index.get("A.md").unwrap().review_due_on.as_deref(),
        Some("2026-01-01")
    );
    // 365 days after a date in a leap year's March is the same date a year on.
    assert_eq!(
        index.get("Leap.md").unwrap().review_due_on.as_deref(),
        Some("2025-03-01")
    );
    assert_eq!(index.get("Never.md").unwrap().review_due_on, None);
    assert_eq!(index.get("Bad.md").unwrap().review_due_on, None);
}

#[test]
fn metadata_problems_travel_with_the_page_into_lists() {
    let ws = common::workspace();
    page(&ws, "Good.md", "owner: Sam\nstatus: active");
    page(&ws, "BadStatus.md", "status: finished");
    page(&ws, "BadYaml.md", "owner: [unclosed");
    let index = refreshed(&ws);

    assert!(index.get("Good.md").unwrap().warnings.is_empty());
    let status = index.get("BadStatus.md").unwrap().warnings;
    assert!(status.iter().any(|w| w.contains("status")), "{status:?}");
    let yaml = index.get("BadYaml.md").unwrap().warnings;
    assert!(
        yaml.iter().any(|w| w.contains("couldn't be read")),
        "{yaml:?}"
    );
}

#[test]
fn a_same_size_change_a_moment_later_is_still_noticed() {
    let ws = common::workspace();
    page(&ws, "A.md", "owner: Sam\nlast_reviewed: 2025-09-08");
    let mut index = refreshed(&ws);
    assert_eq!(
        index.get("A.md").unwrap().last_reviewed.as_deref(),
        Some("2025-09-08")
    );

    // Same length, written well inside the same second as before: the page
    // index must still see it (a review date is exactly this kind of edit).
    std::thread::sleep(std::time::Duration::from_millis(50));
    page(&ws, "A.md", "owner: Sam\nlast_reviewed: 2026-10-13");
    index.refresh(&ws.root).unwrap();
    assert_eq!(
        index.get("A.md").unwrap().last_reviewed.as_deref(),
        Some("2026-10-13")
    );
}

#[test]
fn deleted_pages_leave_the_index_and_a_clean_scan_reports_nothing() {
    let ws = common::workspace();
    page(&ws, "Keep.md", "owner: Sam");
    page(&ws, "Gone.md", "owner: Sam");
    let mut index = refreshed(&ws);
    std::fs::remove_file(ws.path.join("Gone.md")).unwrap();
    index.refresh(&ws.root).unwrap();
    assert!(index.get("Gone.md").is_none());
    assert!(index.get("Keep.md").is_some());
    assert!(index.problems().is_empty());
}

#[cfg(unix)]
mod unreadable {
    use std::fs::Permissions;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn set(path: &std::path::Path, mode: u32) {
        std::fs::set_permissions(path, Permissions::from_mode(mode)).unwrap();
    }

    #[test]
    fn a_page_that_cannot_be_read_stays_listed_and_is_reported() {
        let ws = common::workspace();
        page(&ws, "Guides/Printer.md", "owner: Sam");
        let mut index = refreshed(&ws);
        let file = ws.path.join("Guides/Printer.md");
        // Changed (so it must be read again), and now unreadable.
        std::fs::write(
            &file,
            "---\nowner: Alex\n---\n# Printer, changed\n\nMore text.\n",
        )
        .unwrap();
        set(&file, 0o000);
        if std::fs::read(&file).is_ok() {
            set(&file, 0o644);
            eprintln!("skipped: this user can read every file");
            return;
        }
        index.refresh(&ws.root).unwrap();
        let listed = index.get("Guides/Printer.md");
        let problems = index.problems().to_vec();
        set(&file, 0o644);

        assert_eq!(listed.expect("still listed").owner.as_deref(), Some("Sam"));
        assert!(
            problems.iter().any(|p| p.path == "Guides/Printer.md"),
            "{problems:?}"
        );
        // Readable again: caught up, and nothing is reported.
        index.refresh(&ws.root).unwrap();
        assert_eq!(
            index.get("Guides/Printer.md").unwrap().owner.as_deref(),
            Some("Alex")
        );
        assert!(index.problems().is_empty());
    }

    #[test]
    fn a_folder_that_cannot_be_read_keeps_its_pages_and_is_reported() {
        let ws = common::workspace();
        page(&ws, "Guides/Printer.md", "owner: Sam");
        page(&ws, "Other.md", "owner: Sam");
        let mut index = refreshed(&ws);
        let folder = ws.path.join("Guides");
        set(&folder, 0o000);
        if std::fs::read_dir(&folder).is_ok() {
            set(&folder, 0o755);
            eprintln!("skipped: this user can read every folder");
            return;
        }
        index.refresh(&ws.root).unwrap();
        let kept = index.get("Guides/Printer.md").is_some();
        let problems = index.problems().to_vec();
        set(&folder, 0o755);

        assert!(kept, "pages in an unreadable folder must not vanish");
        assert!(problems.iter().any(|p| p.path == "Guides"), "{problems:?}");
        index.refresh(&ws.root).unwrap();
        assert!(index.problems().is_empty());
    }
}
