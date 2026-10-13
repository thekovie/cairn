//! The workspace-wide check for broken links and pictures: valid references
//! are never flagged, every kind of break is found with its line and a
//! reason, the check agrees with what a page itself shows, and a check that
//! could not read everything says so.

mod common;

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::article::{RenderContext, parse_front_matter, render};
use cairn::config::AppConfig;
use cairn::linkcheck::{
    CheckProgress, Headings, check_page, check_workspace, check_workspace_with,
};
use cairn::search::SearchIndex;
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn fresh() -> common::TestWorkspace {
    let ws = common::workspace();
    std::fs::remove_file(ws.path.join("README.md")).unwrap();
    ws
}

/// Pages and files that valid links can point to.
fn targets(ws: &common::TestWorkspace) {
    let p = &ws.path;
    common::write(
        &p.join("Guides/Printer.md"),
        "# Printer\n\n## Steps\n\ntext\n\n## Steps\n\nmore\n",
    );
    std::fs::create_dir_all(p.join("Guides/Printer.assets")).unwrap();
    std::fs::write(
        p.join("Guides/Printer.assets/shot.png"),
        common::png(8, 8, 40),
    )
    .unwrap();
    std::fs::write(p.join("Guides/Printer.assets/logo.svg"), "<svg/>").unwrap();
    common::write(&p.join("Other/My Page.md"), "# My Page\n\n## Heading One\n");
    common::write(&p.join("Guides/Résumé.md"), "# Résumé\n");
    std::fs::create_dir_all(p.join("Reference")).unwrap();
    std::fs::write(p.join("Reference/file.pdf"), b"%PDF-1.4").unwrap();
}

fn check(ws: &common::TestWorkspace, rel: &str, body: &str) -> Vec<cairn::linkcheck::Finding> {
    check_page(&ws.root, rel, body, &mut Headings::default())
}

const VALID: &str = "# Index\n\n\
[same folder](Printer.md) [with anchor](Printer.md#steps) \
[second heading](Printer.md#steps-1) [other folder](../Other/My%20Page.md#heading-one)\n\n\
[unicode](R%C3%A9sum%C3%A9.md) [pdf](../Reference/file.pdf) [web](https://example.com/x) \
[mail](mailto:a@b.co) [self](#intro)\n\n\
## Intro\n\n![shot](Printer.assets/shot.png)\n";

#[test]
fn valid_references_are_never_flagged() {
    let ws = fresh();
    targets(&ws);
    let found = check(&ws, "Guides/Index.md", VALID);
    assert!(found.is_empty(), "{found:#?}");
}

const BROKEN: &str = "# Broken\n\
\n\
[missing page](Nope.md)\n\
[missing file](../Reference/gone.pdf)\n\
![missing picture](Printer.assets/none.png)\n\
[bad anchor](#nowhere)\n\
[bad page anchor](Printer.md#no-such-heading)\n\
[outside](../../escape.md)\n\
[cairn's own folder](../_system/history/x.md)\n\
![vector picture](Printer.assets/logo.svg)\n\
[blocked](javascript:alert(1))\n";

#[test]
fn each_kind_of_break_is_found_with_its_line_and_a_reason() {
    let ws = fresh();
    targets(&ws);
    let found = check(&ws, "Guides/Broken.md", BROKEN);
    let got: Vec<(&str, usize)> = found.iter().map(|f| (f.kind, f.line)).collect();
    assert_eq!(
        got,
        [
            ("missing_page", 3),
            ("missing_file", 4),
            ("missing_image", 5),
            ("bad_anchor", 6),
            ("bad_anchor", 7),
            ("outside", 8),
            ("outside", 9),
            ("unsupported_image", 10),
            ("blocked", 11),
        ],
        "{found:#?}"
    );
    assert!(found.iter().all(|f| !f.message.trim().is_empty()));
    assert_eq!(found[0].target, "Nope.md");
    assert_eq!(found[0].resolved.as_deref(), Some("Guides/Nope.md"));
    assert!(
        found[7].message.contains("png, jpg, jpeg, gif, or webp"),
        "{}",
        found[7].message
    );
    assert!(
        found[4].message.contains("Printer.md"),
        "names the page: {}",
        found[4].message
    );
}

#[test]
fn the_check_and_the_page_itself_agree() {
    let ws = fresh();
    targets(&ws);
    let found = check(&ws, "Guides/Broken.md", BROKEN);
    let ctx = RenderContext {
        root: &ws.root,
        article_rel: "Guides/Broken.md",
        read_key: "k",
        staged: &HashMap::new(),
    };
    let shown: BTreeSet<String> = render(BROKEN, &ctx).broken_links.into_iter().collect();
    // What the page marks as broken is what the check reports, apart from
    // what only the check looks into: headings on other pages and blocked
    // link types.
    let reported: BTreeSet<String> = found
        .iter()
        .filter(|f| f.kind != "blocked")
        .filter(|f| !(f.kind == "bad_anchor" && !f.target.starts_with('#')))
        .map(|f| f.resolved.clone().unwrap_or_else(|| f.target.clone()))
        .collect();
    assert_eq!(shown, reported);
}

#[test]
fn a_renamed_page_shows_up_on_the_next_check_and_clears_when_it_returns() {
    let ws = fresh();
    common::write(&ws.path.join("A.md"), "# A\n\n[to b](B.md)\n");
    common::write(&ws.path.join("B.md"), "# B\n");
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    assert_eq!(check_workspace(&ws.root, &index.all()).total_findings, 0);

    std::fs::rename(ws.path.join("B.md"), ws.path.join("C.md")).unwrap();
    index.refresh(&ws.root).unwrap();
    let report = check_workspace(&ws.root, &index.all());
    assert_eq!(report.total_findings, 1);
    assert_eq!(report.pages[0].path, "A.md");
    assert_eq!(report.pages[0].findings[0].kind, "missing_page");

    std::fs::rename(ws.path.join("C.md"), ws.path.join("B.md")).unwrap();
    index.refresh(&ws.root).unwrap();
    assert_eq!(check_workspace(&ws.root, &index.all()).total_findings, 0);
}

/// A folder link pointing elsewhere: a junction on Windows, a symbolic link
/// elsewhere.
fn link_folder(link: &std::path::Path, target: &std::path::Path) {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
}

#[test]
fn a_link_through_a_shortcut_folder_is_reported_not_followed() {
    let ws = fresh();
    targets(&ws);
    // Separate joins: `mklink /J` on Windows rejects a path mixing `\\` and `/`.
    link_folder(
        &ws.path.join("Guides").join("shortcut"),
        &ws.path.join("Other"),
    );
    let found = check(
        &ws,
        "Guides/Index.md",
        "# Index\n\n[through](shortcut/My%20Page.md)\n",
    );
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].kind, "missing_page");
}

#[test]
fn web_addresses_are_never_looked_up_and_templates_are_not_checked() {
    let ws = fresh();
    common::write(
        &ws.path.join("Web.md"),
        "# Web\n\n[a](https://nonexistent.invalid/page) [b](http://192.0.2.1/x) [c](mailto:x@y.z)\n",
    );
    common::write(&ws.path.join("_templates/t.md"), "# T\n\n[x](gone.md)\n");
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    let report = check_workspace(&ws.root, &index.all());
    assert_eq!(report.checked, 1, "only the page, not the template");
    assert_eq!(report.total_findings, 0);
    assert!(report.complete);
}

#[test]
fn front_matter_is_not_mistaken_for_page_text() {
    let ws = fresh();
    common::write(
        &ws.path.join("P.md"),
        "---\nowner: Sam\n---\n# P\n\n[gone](Nope.md)\n",
    );
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    let report = check_workspace(&ws.root, &index.all());
    // Lines count from the start of the page text, as the editor shows it.
    assert_eq!(report.pages[0].findings[0].line, 3);
    let text = std::fs::read_to_string(ws.path.join("P.md")).unwrap();
    assert_eq!(
        parse_front_matter(&text).1.lines().nth(2),
        Some("[gone](Nope.md)")
    );
}

#[cfg(unix)]
#[test]
fn a_page_that_cannot_be_read_makes_the_check_incomplete() {
    use std::os::unix::fs::PermissionsExt;
    let ws = fresh();
    common::write(&ws.path.join("Fine.md"), "# Fine\n\n[gone](Nope.md)\n");
    common::write(&ws.path.join("Secret.md"), "# Secret\n");
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    let secret = ws.path.join("Secret.md");
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&secret).is_ok() {
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();
        eprintln!("skipped: this user can read every file");
        return;
    }
    let report = check_workspace(&ws.root, &index.all());
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o644)).unwrap();

    assert!(
        !report.complete,
        "an unfinished check is never a clean result"
    );
    assert!(report.unreadable.iter().any(|p| p.path == "Secret.md"));
    assert_eq!(
        report.total_findings, 1,
        "the pages it could read were still checked"
    );
}

#[test]
fn the_check_reports_each_page_it_finishes_and_can_be_stopped() {
    use std::cell::{Cell, RefCell};

    let ws = fresh();
    for i in 1..=5 {
        common::write(
            &ws.path.join(format!("P{i}.md")),
            &format!("# P{i}\n\n[gone](Nope{i}.md)\n"),
        );
    }
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    let pages = index.all();

    // Every page is reported once, in order, counting up to the total.
    let seen = RefCell::new(Vec::new());
    let never = || false;
    let note = |n: usize| seen.borrow_mut().push(n);
    let report = check_workspace_with(
        &ws.root,
        &pages,
        &CheckProgress {
            cancelled: &never,
            on_page: &note,
        },
    )
    .expect("not stopped");
    assert_eq!(*seen.borrow(), [1, 2, 3, 4, 5]);
    assert_eq!(report.total_findings, 5);

    // Stopped part way: no half-finished result is handed back.
    let done = Cell::new(0usize);
    let stop_after_two = || done.get() >= 2;
    let count = |n: usize| done.set(n);
    let stopped = check_workspace_with(
        &ws.root,
        &pages,
        &CheckProgress {
            cancelled: &stop_after_two,
            on_page: &count,
        },
    );
    assert!(stopped.is_none());
    assert_eq!(done.get(), 2, "it stopped before the third page");
}

// ---------------------------------------------------------------- the route

const PORT: u16 = 4325;

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

async fn send(
    st: &Arc<AppState>,
    method: &str,
    uri: &str,
    token: bool,
) -> (StatusCode, serde_json::Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("content-type", "application/json");
    if token {
        req = req.header("x-cairn-token", &st.token);
    }
    let res = build_router(st.clone())
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn a_check_runs_as_a_job_with_progress_and_hands_over_its_result_once() {
    let ws = fresh();
    common::write(&ws.path.join("A.md"), "# A\n\n[gone](Nope.md)\n");
    common::write(&ws.path.join("B.md"), "# B\n");
    let (_home, st) = state(&ws);

    assert_eq!(
        send(&st, "POST", "/api/health/check/jobs", false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, started) = send(&st, "POST", "/api/health/check/jobs", true).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["total"], 2);
    let id = started["id"].as_str().unwrap().to_string();

    // Its progress is on the same route as downloads'.
    let mut finished = false;
    for _ in 0..200 {
        let (_, s) = send(&st, "GET", &format!("/api/export/jobs/{id}"), true).await;
        assert_eq!(s["total"], 2);
        match s["state"].as_str().unwrap() {
            "finished" => {
                assert_eq!(s["done"], 2);
                finished = true;
                break;
            }
            "running" => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
            other => panic!("the check ended as {other}: {s}"),
        }
    }
    assert!(finished);

    let result_uri = format!("/api/health/check/jobs/{id}/result");
    assert_eq!(
        send(&st, "GET", &result_uri, false).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, json) = send(&st, "GET", &result_uri, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["checked"], 2);
    assert_eq!(json["complete"], true);
    assert_eq!(json["total_findings"], 1);
    assert_eq!(json["pages"][0]["path"], "A.md");
    assert_eq!(json["pages"][0]["findings"][0]["kind"], "missing_page");
    assert_eq!(json["pages"][0]["findings"][0]["line"], 3);

    // Handed over once.
    assert_eq!(
        send(&st, "GET", &result_uri, true).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn a_check_cannot_start_while_another_job_is_running_and_says_why() {
    let ws = fresh();
    for i in 0..300 {
        common::write(
            &ws.path.join(format!("P{i:03}.md")),
            "# P\n\n[gone](Nope.md)\n",
        );
    }
    let (_home, st) = state(&ws);
    let (status, first) = send(&st, "POST", "/api/health/check/jobs", true).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    // Started again at once: refused while the first may still be running,
    // otherwise (it was quick) accepted. Either way it is never silent.
    let (status, second) = send(&st, "POST", "/api/health/check/jobs", true).await;
    if status != StatusCode::OK {
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            second["message"].as_str().unwrap().contains("check"),
            "{second}"
        );
    }
}
