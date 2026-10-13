//! Search: filters by folder, status and tag (applied before the result
//! limit); sorting; exact titles and tags counting for more; a predictable
//! order; retired pages still found unless filtered out.

mod common;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::search::{SearchIndex, SearchOptions, SearchResults, SortBy};
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn fresh() -> common::TestWorkspace {
    let ws = common::workspace();
    std::fs::remove_file(ws.path.join("README.md")).unwrap();
    ws
}

fn page(ws: &common::TestWorkspace, rel: &str, front: &str, title: &str, body: &str) {
    let block = if front.is_empty() {
        String::new()
    } else {
        format!("---\n{front}\n---\n")
    };
    common::write(&ws.path.join(rel), &format!("{block}# {title}\n\n{body}\n"));
}

fn indexed(ws: &common::TestWorkspace) -> SearchIndex {
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    index
}

fn paths(r: &SearchResults) -> Vec<String> {
    r.hits.iter().map(|h| h.page.path.clone()).collect()
}

fn with(f: impl FnOnce(&mut SearchOptions)) -> SearchOptions {
    let mut o = SearchOptions::default();
    f(&mut o);
    o
}

#[test]
fn filters_are_applied_before_the_result_limit() {
    let ws = fresh();
    // 51 pages that mention widgets in their titles outrank one that only
    // mentions it in the text, which lives in another folder.
    for i in 0..51 {
        page(
            &ws,
            &format!("A/p{i:02}.md"),
            "",
            &format!("Widget guide {i}"),
            "x",
        );
    }
    page(&ws, "Z/quiet.md", "", "Quiet", "Mentions a widget once.");
    let index = indexed(&ws);

    let all = index.search_with("widget", &SearchOptions::default(), 50);
    assert_eq!(all.total, 52);
    assert_eq!(all.hits.len(), 50);
    assert!(
        !paths(&all).contains(&"Z/quiet.md".to_string()),
        "cut by the limit"
    );

    let only_z = index.search_with("widget", &with(|o| o.folder = Some("Z".into())), 50);
    assert_eq!(paths(&only_z), ["Z/quiet.md"], "found once filtered first");
    assert_eq!(only_z.total, 1);
}

#[test]
fn a_folder_filter_includes_nested_folders_but_not_look_alikes() {
    let ws = fresh();
    page(&ws, "Guides/a.md", "", "Alpha", "printer");
    page(&ws, "Guides/Deep/Deeper/b.md", "", "Beta", "printer");
    page(&ws, "GuidesExtra/c.md", "", "Gamma", "printer");
    page(&ws, "root.md", "", "Root", "printer");
    let index = indexed(&ws);

    let r = index.search_with("printer", &with(|o| o.folder = Some("guides".into())), 50);
    assert_eq!(paths(&r), ["Guides/a.md", "Guides/Deep/Deeper/b.md"]);
    let deep = index.search_with(
        "printer",
        &with(|o| o.folder = Some("Guides/Deep".into())),
        50,
    );
    assert_eq!(paths(&deep), ["Guides/Deep/Deeper/b.md"]);
}

#[test]
fn status_and_tag_filters_work_alone_and_together() {
    let ws = fresh();
    page(
        &ws,
        "a.md",
        "status: active\ntags: network, vpn",
        "A",
        "printer",
    );
    page(
        &ws,
        "b.md",
        "status: retired\ntags: network",
        "B",
        "printer",
    );
    page(&ws, "c.md", "status: draft\ntags: [vpn]", "C", "printer");
    page(&ws, "d.md", "", "D", "printer");
    let index = indexed(&ws);
    let run = |status: Option<&str>, tag: Option<&str>| {
        let o = with(|o| {
            o.status = status.map(str::to_string);
            o.tag = tag.map(str::to_string);
        });
        let mut found = paths(&index.search_with("printer", &o, 50));
        found.sort();
        found
    };
    assert_eq!(run(Some("retired"), None), ["b.md"]);
    assert_eq!(run(Some("none"), None), ["d.md"], "pages with no status");
    assert_eq!(
        run(None, Some("NETWORK")),
        ["a.md", "b.md"],
        "tags ignore case"
    );
    assert_eq!(run(Some("active"), Some("network")), ["a.md"]);
    assert!(run(Some("retired"), Some("vpn")).is_empty());
    assert!(run(None, Some("nonesuch")).is_empty());
    assert_eq!(run(None, None).len(), 4);
}

#[test]
fn retired_pages_stay_discoverable_unless_filtered_out() {
    let ws = fresh();
    page(&ws, "old.md", "status: retired", "Old VPN notes", "vpn");
    page(&ws, "new.md", "status: active", "VPN guide", "vpn");
    let index = indexed(&ws);
    let r = index.search_with("vpn", &SearchOptions::default(), 50);
    assert_eq!(r.total, 2);
    assert!(
        r.hits
            .iter()
            .any(|h| h.page.status.as_deref() == Some("retired"))
    );
    let active = index.search_with("vpn", &with(|o| o.status = Some("active".into())), 50);
    assert_eq!(paths(&active), ["new.md"]);
}

#[test]
fn an_exact_title_comes_first_even_against_pages_that_say_it_more() {
    let ws = fresh();
    page(
        &ws,
        "long.md",
        "",
        "Printer setup and printer fixes",
        "printer printer printer printer",
    );
    page(&ws, "exact.md", "", "Printer", "x");
    page(&ws, "net.md", "", "Network drive", "x");
    page(
        &ws,
        "net2.md",
        "",
        "Mapping a network drive on Windows",
        "network drive network drive",
    );
    let index = indexed(&ws);
    let top = |q: &str| paths(&index.search_with(q, &SearchOptions::default(), 50))[0].clone();
    assert_eq!(top("printer"), "exact.md");
    assert_eq!(top("Network  Drive"), "net.md");
}

#[test]
fn tags_are_searchable_and_count_like_title_words() {
    let ws = fresh();
    page(
        &ws,
        "tagged.md",
        "tags: vpn, remote",
        "Working away",
        "Nothing about it here.",
    );
    page(&ws, "mention.md", "", "Other", "vpn vpn vpn");
    let index = indexed(&ws);
    let r = index.search_with("vpn", &SearchOptions::default(), 50);
    assert_eq!(
        paths(&r),
        ["tagged.md", "mention.md"],
        "a tag outweighs a few mentions"
    );
    assert_eq!(index.all_tags(), ["remote", "vpn"]);
}

#[test]
fn equal_matches_come_in_a_predictable_order() {
    let ws = fresh();
    // Same score. Capital letters must not decide the order, and identical
    // titles fall back to the path.
    page(&ws, "x/one.md", "", "Beta", "needle");
    page(&ws, "x/two.md", "", "alpha", "needle");
    page(&ws, "Other/same.md", "", "Same", "needle");
    page(&ws, "Guides/same.md", "", "Same", "needle");
    let index = indexed(&ws);
    let first = paths(&index.search_with("needle", &SearchOptions::default(), 50));
    assert_eq!(
        first,
        ["x/two.md", "x/one.md", "Guides/same.md", "Other/same.md"]
    );
    let again = paths(&index.search_with("needle", &SearchOptions::default(), 50));
    assert_eq!(first, again);
}

fn set_modified(path: &std::path::Path, days_ago: u64) {
    let when = SystemTime::now() - Duration::from_secs(days_ago * 86_400);
    let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
    file.set_modified(when).unwrap();
}

#[test]
fn results_can_be_sorted_by_best_match_recent_change_or_title() {
    let ws = fresh();
    page(&ws, "b.md", "", "Bravo", "needle");
    page(&ws, "a.md", "", "alpha", "needle needle needle");
    page(&ws, "c.md", "", "Charlie", "needle");
    set_modified(&ws.path.join("b.md"), 1);
    set_modified(&ws.path.join("a.md"), 30);
    set_modified(&ws.path.join("c.md"), 10);
    let index = indexed(&ws);

    let sorted = |sort| paths(&index.search_with("needle", &with(|o| o.sort = sort), 50));
    assert_eq!(
        sorted(SortBy::Best),
        ["a.md", "b.md", "c.md"],
        "most mentions, then by title"
    );
    assert_eq!(sorted(SortBy::Updated), ["b.md", "c.md", "a.md"]);
    assert_eq!(sorted(SortBy::Title), ["a.md", "b.md", "c.md"]);
}

#[test]
fn a_filter_without_words_lists_the_pages_that_match_it() {
    let ws = fresh();
    page(&ws, "old.md", "status: retired", "Old", "text");
    page(&ws, "older.md", "status: retired", "Ancient", "text");
    page(&ws, "new.md", "status: active", "New", "text");
    let index = indexed(&ws);
    let r = index.search_with("", &with(|o| o.status = Some("retired".into())), 50);
    assert_eq!(
        paths(&r),
        ["older.md", "old.md"],
        "by title when there are no words"
    );
    // No words and no filter is still nothing.
    assert!(
        index
            .search_with("  ", &SearchOptions::default(), 50)
            .hits
            .is_empty()
    );
}

// ---------------------------------------------------------------- the route

const PORT: u16 = 4327;

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

async fn get(st: &Arc<AppState>, uri: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .body(Body::empty())
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
async fn the_route_filters_sorts_and_still_works_without_any_options() {
    let ws = fresh();
    page(
        &ws,
        "Guides/a.md",
        "status: active\ntags: network",
        "Alpha",
        "printer",
    );
    page(
        &ws,
        "Guides/b.md",
        "status: retired\ntags: network",
        "Bravo",
        "printer",
    );
    page(&ws, "Other/c.md", "status: retired", "Charlie", "printer");
    let (_home, st) = state(&ws);

    // As before: just words.
    let (status, json) = get(&st, "/api/search?q=printer").await;
    assert_eq!(status, StatusCode::OK, "{json}");
    assert_eq!(json["results"].as_array().unwrap().len(), 3);
    assert_eq!(json["total"], 3);
    assert_eq!(json["tags"], serde_json::json!(["network"]));

    let (_, json) = get(
        &st,
        "/api/search?q=printer&folder=Guides&status=retired&sort=title",
    )
    .await;
    let found: Vec<&str> = json["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["page"]["path"].as_str().unwrap())
        .collect();
    assert_eq!(found, ["Guides/b.md"]);
    assert_eq!(json["filters"]["folder"], "Guides");
    assert_eq!(json["filters"]["status"], "retired");
    assert_eq!(json["filters"]["sort"], "title");

    let (_, json) = get(&st, "/api/search?q=&tag=network").await;
    assert_eq!(json["total"], 2, "a tag alone lists its pages");
}

#[tokio::test]
async fn the_route_rejects_options_it_does_not_understand() {
    let ws = fresh();
    let (_home, st) = state(&ws);
    for bad in [
        "/api/search?q=x&status=finished",
        "/api/search?q=x&sort=random",
        "/api/search?q=x&folder=..%2Fescape",
        "/api/search?q=x&folder=_system",
    ] {
        let (status, _) = get(&st, bad).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::FORBIDDEN,
            "{bad}: {status}"
        );
    }
}
