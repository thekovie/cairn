//! "Mark as reviewed today": the front-matter edit changes only the review
//! date; the write goes through the same safe publishing as every other
//! change (locks respected, an earlier version kept, the author's credit
//! unchanged); it never touches the status; and it refuses politely when the
//! page can't or shouldn't be changed.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::article::{parse_front_matter, set_last_reviewed};
use cairn::config::AppConfig;
use cairn::editors::{self, LastEdit};
use cairn::history;
use cairn::locks;
use cairn::manage::mark_reviewed;
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

const DAY: &str = "2026-10-13";

// -------------------------------------------------------------- the editor

fn set(text: &str) -> String {
    set_last_reviewed(text, DAY).unwrap()
}

#[test]
fn a_new_line_is_added_to_an_existing_block() {
    assert_eq!(
        set("---\nowner: Sam\n---\n# T\n"),
        "---\nowner: Sam\nlast_reviewed: 2026-10-13\n---\n# T\n"
    );
    // A block closed with "..." works too.
    assert_eq!(
        set("---\nowner: Sam\n...\n# T\n"),
        "---\nowner: Sam\nlast_reviewed: 2026-10-13\n...\n# T\n"
    );
}

#[test]
fn an_existing_date_is_replaced_in_place_and_nothing_else_moves() {
    assert_eq!(
        set("---\nlast_reviewed: 2020-01-01\nowner: Sam\n---\n# T\n"),
        "---\nlast_reviewed: 2026-10-13\nowner: Sam\n---\n# T\n"
    );
    assert_eq!(
        set("---\nowner: Sam\nlast_reviewed: \"2020-01-01\"\ntags: a, b\n---\n# T\n"),
        "---\nowner: Sam\nlast_reviewed: 2026-10-13\ntags: a, b\n---\n# T\n"
    );
}

#[test]
fn unknown_keys_and_comments_survive_byte_for_byte() {
    let before = "---\n# kept\ncustom: [1, 2]\nnested:\n  - x\nowner: Sam\n---\n# T\n\ntext\n";
    let after = set(before);
    let expected = before.replace("---\n# T", "last_reviewed: 2026-10-13\n---\n# T");
    assert_eq!(after, expected);
}

#[test]
fn a_page_without_a_block_gets_one_and_its_text_is_untouched() {
    assert_eq!(
        set("# T\n\ntext\n"),
        "---\nlast_reviewed: 2026-10-13\n---\n# T\n\ntext\n"
    );
    // A rule further down is not a block.
    assert_eq!(
        set("# T\n\n---\ntext\n---\n"),
        "---\nlast_reviewed: 2026-10-13\n---\n# T\n\n---\ntext\n---\n"
    );
    assert_eq!(set(""), "---\nlast_reviewed: 2026-10-13\n---\n");
}

#[test]
fn windows_line_endings_and_a_byte_order_mark_are_kept() {
    assert_eq!(
        set("---\r\nowner: Sam\r\n---\r\n# T\r\n"),
        "---\r\nowner: Sam\r\nlast_reviewed: 2026-10-13\r\n---\r\n# T\r\n"
    );
    assert_eq!(
        set("---\r\nlast_reviewed: 2020-01-01\r\n---\r\n"),
        "---\r\nlast_reviewed: 2026-10-13\r\n---\r\n"
    );
    assert_eq!(
        set("\u{feff}---\nowner: Sam\n---\n# T\n"),
        "\u{feff}---\nowner: Sam\nlast_reviewed: 2026-10-13\n---\n# T\n"
    );
    assert_eq!(
        set("# T\r\n"),
        "---\r\nlast_reviewed: 2026-10-13\r\n---\r\n# T\r\n"
    );
}

#[test]
fn the_result_reads_back_and_setting_twice_changes_nothing() {
    for before in [
        "---\nowner: Sam\nstatus: active\n---\n# T\n",
        "# T\n",
        "---\nlast_reviewed: 2020-01-01\n---\n",
    ] {
        let once = set(before);
        assert_eq!(
            parse_front_matter(&once).0.last_reviewed.as_deref(),
            Some(DAY)
        );
        assert_eq!(set(&once), once);
    }
    // The status is not touched.
    assert_eq!(
        parse_front_matter(&set("---\nstatus: draft\n---\n# T\n"))
            .0
            .status
            .as_deref(),
        Some("draft")
    );
}

#[test]
fn a_block_that_cannot_be_edited_safely_is_refused() {
    for (what, text) in [
        ("not closed", "---\nowner: Sam\n# T\n"),
        (
            "two dates",
            "---\nlast_reviewed: 2020-01-01\nlast_reviewed: 2021-01-01\n---\n",
        ),
        (
            "date on its own line",
            "---\nlast_reviewed:\n  2020-01-01\n---\n",
        ),
        ("broken YAML", "---\nowner: [unclosed\n---\n# T\n"),
    ] {
        assert!(set_last_reviewed(text, DAY).is_err(), "accepted: {what}");
    }
    assert!(set_last_reviewed("# T\n", "yesterday").is_err());
    assert!(set_last_reviewed("# T\n", "2026-13-40").is_err());
}

// ---------------------------------------------------------------- the write

fn page(ws: &common::TestWorkspace, rel: &str, text: &str) {
    common::write(&ws.path.join(rel), text);
}

#[test]
fn marking_reviewed_keeps_the_credit_the_status_and_an_earlier_version() {
    let ws = common::workspace();
    let before = "---\nowner: Sam\nstatus: draft\nlast_reviewed: 2020-01-01\n---\n# A\n\nText.\n";
    page(&ws, "A.md", before);
    editors::record(&ws.root, "A.md", "Sam", before.as_bytes());
    let me = common::identity("Alex");

    assert!(mark_reviewed(&ws.root, &me, "A.md", DAY).unwrap());

    let after = std::fs::read_to_string(ws.path.join("A.md")).unwrap();
    assert_eq!(
        after,
        "---\nowner: Sam\nstatus: draft\nlast_reviewed: 2026-10-13\n---\n# A\n\nText.\n"
    );
    // The page as it was is kept as an earlier version.
    let versions = history::list_versions(&ws.root, "A.md").unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(
        history::read_version(&ws.root, "A.md", &versions[0].id).unwrap(),
        before.as_bytes()
    );
    // Sam still wrote the page: a review is not an edit.
    match editors::last_edit(&ws.root, "A.md", after.as_bytes()) {
        LastEdit::By(rec) => assert_eq!(rec.by, "Sam"),
        other => panic!("credit lost: {other:?}"),
    }
    // No lock is left behind.
    assert!(locks::status(&ws.root, "A.md", &me).unwrap().is_none());
}

#[test]
fn already_reviewed_today_changes_nothing() {
    let ws = common::workspace();
    page(&ws, "A.md", "---\nlast_reviewed: 2026-10-13\n---\n# A\n");
    let me = common::identity("Alex");
    assert!(!mark_reviewed(&ws.root, &me, "A.md", DAY).unwrap());
    assert!(history::list_versions(&ws.root, "A.md").unwrap().is_empty());
}

#[test]
fn a_page_someone_is_editing_is_not_touched() {
    let ws = common::workspace();
    let text = "---\nowner: Sam\n---\n# A\n";
    page(&ws, "A.md", text);
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, "A.md", &alex).unwrap();

    let err = mark_reviewed(&ws.root, &common::identity("Sam"), "A.md", DAY).unwrap_err();
    assert!(err.to_string().contains("Alex"), "{err}");
    assert_eq!(std::fs::read_to_string(ws.path.join("A.md")).unwrap(), text);

    // Neither the person editing it.
    let err = mark_reviewed(&ws.root, &alex, "A.md", DAY).unwrap_err();
    assert!(err.to_string().contains("open in the editor"), "{err}");
}

#[test]
fn a_page_that_cannot_be_edited_automatically_is_left_alone() {
    let ws = common::workspace();
    let text = "---\nowner: [unclosed\n---\n# A\n";
    page(&ws, "A.md", text);
    let err = mark_reviewed(&ws.root, &common::identity("Sam"), "A.md", DAY).unwrap_err();
    assert!(err.to_string().contains("Page details"), "{err}");
    assert_eq!(std::fs::read_to_string(ws.path.join("A.md")).unwrap(), text);
}

#[test]
fn a_missing_page_is_reported_as_missing() {
    let ws = common::workspace();
    let err = mark_reviewed(&ws.root, &common::identity("Sam"), "Nope.md", DAY).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("found"), "{err}");
}

#[test]
fn the_new_date_is_what_a_later_scan_sees() {
    let ws = common::workspace();
    page(
        &ws,
        "A.md",
        "---\nowner: Sam\nlast_reviewed: 2020-01-01\n---\n# A\n",
    );
    mark_reviewed(&ws.root, &common::identity("Sam"), "A.md", DAY).unwrap();
    let mut index = cairn::search::SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    let summary = index.get("A.md").unwrap();
    assert_eq!(summary.last_reviewed.as_deref(), Some(DAY));
    assert_eq!(summary.review_due_on.as_deref(), Some("2027-10-13"));
}

// ---------------------------------------------------------------- the route

const PORT: u16 = 4326;

fn state(ws: &common::TestWorkspace, timezone: Option<&str>) -> (tempfile::TempDir, Arc<AppState>) {
    let home = tempfile::tempdir().unwrap();
    let cfg = AppConfig {
        persistent_drafts: false,
        timezone: timezone.map(str::to_string),
        ..AppConfig::default()
    };
    let state = AppState::new(home.path().to_path_buf(), cfg, None);
    state.set_port(PORT);
    open_at(&state, &ws.path).unwrap();
    (home, state)
}

async fn review(st: &Arc<AppState>, path: &str, token: bool) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/api/review")
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("content-type", "application/json");
    if token {
        req = req.header("x-cairn-token", &st.token);
    }
    let req = req
        .body(Body::from(serde_json::json!({ "path": path }).to_string()))
        .unwrap();
    let res = build_router(st.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn the_route_needs_the_token_and_dates_the_page_in_the_chosen_timezone() {
    let ws = common::workspace();
    page(&ws, "A.md", "---\nowner: Sam\n---\n# A\n");
    // Kiritimati is UTC+14: its "today" differs from most of the world's.
    let zone = "Pacific/Kiritimati";
    let (_home, st) = state(&ws, Some(zone));

    assert_eq!(review(&st, "A.md", false).await.0, StatusCode::UNAUTHORIZED);

    let (status, json) = review(&st, "A.md", true).await;
    assert_eq!(status, StatusCode::OK, "{json}");
    let today = cairn::timefmt::today(&cairn::timefmt::user_zone(Some(zone)));
    assert_eq!(json["last_reviewed"], today);
    assert_eq!(json["changed"], true);
    let text = std::fs::read_to_string(ws.path.join("A.md")).unwrap();
    assert!(text.contains(&format!("last_reviewed: {today}")), "{text}");

    // Again the same day: nothing new is written.
    let (status, json) = review(&st, "A.md", true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["changed"], false);
    assert_eq!(history::list_versions(&ws.root, "A.md").unwrap().len(), 1);
}

#[tokio::test]
async fn a_read_only_documentation_folder_cannot_be_changed_by_a_review() {
    let ws = common::workspace();
    page(&ws, "A.md", "---\nowner: Sam\n---\n# A\n");
    // A folder made by a newer Cairn opens read-only.
    let marker = ws.path.join("shared-docs.json");
    let mut json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&marker).unwrap()).unwrap();
    json["schema_version"] = serde_json::json!(99);
    std::fs::write(&marker, serde_json::to_vec(&json).unwrap()).unwrap();
    let (_home, st) = state(&ws, None);

    let (status, body) = review(&st, "A.md", true).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        std::fs::read_to_string(ws.path.join("A.md")).unwrap(),
        "---\nowner: Sam\n---\n# A\n"
    );
}

#[tokio::test]
async fn the_route_refuses_paths_that_are_not_pages() {
    let ws = common::workspace();
    let (_home, st) = state(&ws, None);
    for bad in ["../x.md", "_system/history/x.md", "notes.txt", ""] {
        let (status, _) = review(&st, bad, true).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::FORBIDDEN,
            "{bad}: {status}"
        );
    }
}
