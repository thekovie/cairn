//! Documentation health: which pages need attention and why, worked out
//! from the page index, and honest about a scan that could not read
//! everything.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::health::report;
use cairn::search::SearchIndex;
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

const TODAY: &str = "2026-10-13";

/// A workspace without the starter page, so every page is one the test wrote.
fn fresh() -> common::TestWorkspace {
    let ws = common::workspace();
    std::fs::remove_file(ws.path.join("README.md")).unwrap();
    ws
}

fn page(ws: &common::TestWorkspace, rel: &str, front: &str) {
    common::write(
        &ws.path.join(rel),
        &format!("---\n{front}\n---\n# {rel}\n\nText.\n"),
    );
}

fn sample(ws: &common::TestWorkspace) {
    // Nothing to fix.
    page(
        ws,
        "Fine.md",
        "owner: Sam\nstatus: active\nlast_reviewed: 2026-09-01",
    );
    // Falls due exactly today (a year after 2025-10-13), and the day before.
    page(ws, "DueToday.md", "owner: Sam\nlast_reviewed: 2025-10-13");
    page(
        ws,
        "DueTomorrow.md",
        "owner: Sam\nlast_reviewed: 2025-10-14",
    );
    page(ws, "NoDate.md", "owner: Sam");
    page(ws, "NoOwner.md", "last_reviewed: 2026-09-01");
    page(
        ws,
        "Draft.md",
        "owner: Sam\nstatus: draft\nlast_reviewed: 2026-09-01",
    );
    // Retired pages need no review or owner: only their status shows.
    page(
        ws,
        "Retired.md",
        "status: retired\nlast_reviewed: 2020-01-01",
    );
    page(
        ws,
        "BadStatus.md",
        "owner: Sam\nstatus: finished\nlast_reviewed: 2026-09-01",
    );
}

fn index(ws: &common::TestWorkspace) -> SearchIndex {
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    index
}

fn issues_of(r: &cairn::health::Report, path: &str) -> Vec<&'static str> {
    r.pages
        .iter()
        .find(|p| p.page.path == path)
        .map(|p| p.issues.clone())
        .unwrap_or_default()
}

#[test]
fn each_page_is_classified_by_what_needs_attention() {
    let ws = fresh();
    sample(&ws);
    let r = report(&index(&ws), TODAY);

    assert_eq!(r.total, 8);
    assert!(r.complete && r.problems.is_empty());
    assert!(issues_of(&r, "Fine.md").is_empty());
    assert_eq!(issues_of(&r, "DueToday.md"), ["overdue"]);
    assert!(
        issues_of(&r, "DueTomorrow.md").is_empty(),
        "a day early is not overdue"
    );
    assert_eq!(issues_of(&r, "NoDate.md"), ["no_review_date"]);
    assert_eq!(issues_of(&r, "NoOwner.md"), ["no_owner"]);
    assert_eq!(issues_of(&r, "Draft.md"), ["draft"]);
    assert_eq!(issues_of(&r, "Retired.md"), ["retired"]);
    assert_eq!(issues_of(&r, "BadStatus.md"), ["details"]);
}

#[test]
fn counts_match_the_list_and_only_pages_with_issues_are_listed() {
    let ws = fresh();
    sample(&ws);
    let r = report(&index(&ws), TODAY);

    for (key, expected) in [
        ("overdue", 1),
        ("no_review_date", 1),
        ("no_owner", 1),
        ("draft", 1),
        ("retired", 1),
        ("details", 1),
    ] {
        assert_eq!(r.counts[key], expected, "{key}");
        let listed = r.pages.iter().filter(|p| p.issues.contains(&key)).count();
        assert_eq!(listed, expected, "{key} listed");
    }
    assert_eq!(
        r.pages.len(),
        6,
        "Fine.md and DueTomorrow.md have nothing to fix, so they are not listed"
    );
    // The same page can have several problems; the worst-off come first.
    let ws2 = fresh();
    page(&ws2, "Many.md", "status: draft");
    page(
        &ws2,
        "One.md",
        "owner: Sam\nlast_reviewed: 2026-09-01\nstatus: draft",
    );
    let r2 = report(&index(&ws2), TODAY);
    assert_eq!(r2.pages[0].page.path, "Many.md");
    assert_eq!(
        issues_of(&r2, "Many.md"),
        ["no_review_date", "no_owner", "draft"]
    );
}

#[cfg(unix)]
#[test]
fn a_scan_that_could_not_read_everything_says_so() {
    use std::os::unix::fs::PermissionsExt;
    let ws = fresh();
    sample(&ws);
    page(
        &ws,
        "Guides/Printer.md",
        "owner: Sam\nlast_reviewed: 2026-09-01",
    );
    let mut idx = index(&ws);
    let folder = ws.path.join("Guides");
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&folder).is_ok() {
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("skipped: this user can read every folder");
        return;
    }
    idx.refresh(&ws.root).unwrap();
    let r = report(&idx, TODAY);
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert!(!r.complete, "a partial scan is never reported as complete");
    assert!(r.problems.iter().any(|p| p.path == "Guides"));
    assert_eq!(
        r.total, 9,
        "pages in the unreadable folder are still counted"
    );
}

// ---------------------------------------------------------------- the route

const PORT: u16 = 4324;

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

#[tokio::test]
async fn the_health_route_needs_the_token_and_returns_the_report() {
    let ws = fresh();
    sample(&ws);
    let (_home, st) = state(&ws);
    let router = || build_router(st.clone());

    let no_token = Request::builder()
        .uri("/api/health")
        .header("host", format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        router().oneshot(no_token).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );

    let ok = Request::builder()
        .uri("/api/health")
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .body(Body::empty())
        .unwrap();
    let res = router().oneshot(ok).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["total"], 8);
    assert_eq!(json["complete"], true);
    assert!(json["counts"]["overdue"].as_u64().is_some());
    assert_eq!(json["today"].as_str().unwrap().len(), 10);
    assert!(
        json["pages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["issues"].is_array())
    );
}
