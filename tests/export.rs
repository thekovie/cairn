//! Downloads: Markdown zips keep pages, pictures, and folder structure (and
//! never Cairn's own files); printable HTML is self-contained; PDFs are real
//! PDFs when a browser is available; the routes need the per-launch token.

mod common;

use std::io::{Cursor, Read};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::export::archive::{folder_files, page_files, pages_in, zip_files};
use cairn::export::html::{PrintInput, printable_html};
use cairn::export::pdf::{find_browsers, render_pdf};
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

const PORT: u16 = 4322;

fn sample(ws: &common::TestWorkspace) {
    common::write(
        &ws.path.join("Guides/setup.md"),
        "---\nowner: Sam\nstatus: active\n---\n# Set up\n\n\
         See [the other page](other.md) and [the web](https://example.com).\n\n\
         ![Screen](setup.assets/shot.png)\n",
    );
    std::fs::create_dir_all(ws.path.join("Guides/setup.assets")).unwrap();
    std::fs::write(
        ws.path.join("Guides/setup.assets/shot.png"),
        common::png(8, 8, 40),
    )
    .unwrap();
    common::write(&ws.path.join("Guides/other.md"), "# Other\n");
    common::write(&ws.path.join("Guides/.hidden.md"), "# Hidden\n");
    common::write(&ws.path.join("Guides/x.md.cairn-tmp-123"), "partial");
    common::write(
        &ws.path.join("_templates/meeting.md"),
        "---\ntemplate_name: Meeting\n---\n# {{title}}\n",
    );
    common::write(&ws.path.join("Root page.md"), "# Root page\n");
}

fn zip_names(bytes: &[u8]) -> Vec<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().to_string())
        .collect();
    names.sort();
    names
}

fn zip_entry(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut out = Vec::new();
    archive
        .by_name(name)
        .unwrap()
        .read_to_end(&mut out)
        .unwrap();
    out
}

#[test]
fn page_zip_holds_the_page_and_its_pictures_only() {
    let ws = common::workspace();
    sample(&ws);
    let files = page_files(&ws.root, "Guides/setup.md").unwrap();
    let bytes = zip_files(&ws.root, &files).unwrap();
    assert_eq!(
        zip_names(&bytes),
        ["Guides/setup.assets/shot.png", "Guides/setup.md"]
    );
    assert_eq!(
        zip_entry(&bytes, "Guides/setup.assets/shot.png"),
        common::png(8, 8, 40)
    );
}

#[test]
fn folder_exports_skip_system_hidden_and_temporary_files() {
    let ws = common::workspace();
    sample(&ws);
    let folder = folder_files(&ws.root, "Guides").unwrap();
    assert_eq!(
        folder,
        [
            "Guides/other.md",
            "Guides/setup.assets/shot.png",
            "Guides/setup.md"
        ]
    );

    let all = folder_files(&ws.root, "").unwrap();
    assert!(
        all.iter()
            .all(|p| !p.to_lowercase().starts_with("_system/")),
        "{all:?}"
    );
    assert!(
        all.contains(&"_templates/meeting.md".to_string()),
        "templates travel with everything"
    );
    assert!(all.contains(&"Root page.md".to_string()));

    let pages = pages_in(&ws.root, "").unwrap();
    assert!(pages.iter().all(|p| p.ends_with(".md")));
    assert!(!pages.iter().any(|p| p.contains(".assets/")));
}

#[cfg(windows)]
#[test]
fn folder_exports_never_follow_junctions() {
    let ws = common::workspace();
    sample(&ws);
    let outside = tempfile::tempdir().unwrap();
    common::write(&outside.path().join("secret.md"), "# Secret\n");
    let link = ws.path.join("Guides").join("linked");
    let out = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(outside.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let files = folder_files(&ws.root, "").unwrap();
    assert!(!files.iter().any(|p| p.contains("secret")), "{files:?}");
}

fn print_input<'a>(
    ws: &'a common::TestWorkspace,
    text: &'a str,
    zone: &'a jiff::tz::TimeZone,
) -> PrintInput<'a> {
    PrintInput {
        root: &ws.root,
        rel: "Guides/setup.md",
        text,
        workspace_name: "Test Docs",
        zone,
        paper: "a4",
        modified: Some(1_790_000_000),
        exported_at: 1_790_000_000,
    }
}

#[test]
fn printable_html_is_self_contained() {
    let ws = common::workspace();
    sample(&ws);
    let text = std::fs::read_to_string(ws.path.join("Guides/setup.md")).unwrap();
    let zone = jiff::tz::TimeZone::get("Asia/Manila").unwrap();
    let html = printable_html(&print_input(&ws, &text, &zone));

    assert!(!html.to_lowercase().contains("<script"));
    assert!(html.contains("default-src 'none'"));
    assert!(
        html.contains("src=\"data:image/png;base64,"),
        "picture embedded"
    );
    assert!(!html.contains("/ws-file/"), "no links back into the app");
    assert!(
        !html.contains("#/page/"),
        "in-app page links become plain text"
    );
    assert!(
        html.contains("href=\"https://example.com\""),
        "web links stay"
    );
    assert!(!html.contains("/static/fonts/"), "font is embedded");
    assert!(
        html.contains("GMT+8"),
        "times are labelled with the chosen zone"
    );
    assert!(html.contains("<dd>Sam</dd>"));
    assert_eq!(html.matches("<h1").count(), 1, "the title is printed once");
}

#[test]
fn a_real_pdf_is_made_when_a_browser_is_available() {
    let browsers = find_browsers(None);
    if browsers.is_empty() {
        eprintln!("skipped: no Microsoft Edge or Chrome on this computer");
        return;
    }
    let ws = common::workspace();
    sample(&ws);
    let text = std::fs::read_to_string(ws.path.join("Guides/setup.md")).unwrap();
    let zone = jiff::tz::TimeZone::UTC;
    let html = printable_html(&print_input(&ws, &text, &zone));
    let pdf = render_pdf(&browsers, &html).unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}

// ------------------------------------------------------------------ routes

fn state(cfg: AppConfig, ws: &common::TestWorkspace) -> (tempfile::TempDir, Arc<AppState>) {
    let home = tempfile::tempdir().unwrap();
    let cfg = AppConfig {
        persistent_drafts: false,
        ..cfg
    };
    let state = AppState::new(home.path().to_path_buf(), cfg, None);
    state.set_port(PORT);
    open_at(&state, &ws.path).unwrap();
    (home, state)
}

fn get(st: &AppState, uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .body(Body::empty())
        .unwrap()
}

fn post(st: &AppState, uri: &str, json: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .header("content-type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap()
}

async fn send(
    st: &Arc<AppState>,
    req: Request<Body>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let res = build_router(st.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, body)
}

#[tokio::test]
async fn export_routes_need_the_token() {
    let ws = common::workspace();
    sample(&ws);
    let (_home, st) = state(AppConfig::default(), &ws);
    let req = Request::builder()
        .uri("/api/export/page?path=Guides/setup.md&format=md")
        .header("host", format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&st, req).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_page_downloads_as_markdown_with_its_file_name() {
    let ws = common::workspace();
    sample(&ws);
    let (_home, st) = state(AppConfig::default(), &ws);
    let (status, headers, body) = send(
        &st,
        get(&st, "/api/export/page?path=Guides%2Fsetup.md&format=md"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        std::fs::read(ws.path.join("Guides/setup.md")).unwrap()
    );
    let disposition = headers["content-disposition"].to_str().unwrap();
    assert!(disposition.starts_with("attachment;") && disposition.contains("setup.md"));
}

#[tokio::test]
async fn pdf_requests_report_when_no_browser_is_available() {
    let ws = common::workspace();
    sample(&ws);
    let cfg = AppConfig {
        pdf_browser: Some(r"Z:\missing\msedge.exe".into()),
        ..AppConfig::default()
    };
    let (_home, st) = state(cfg, &ws);
    let (status, _, body) = send(
        &st,
        get(&st, "/api/export/page?path=Guides%2Fsetup.md&format=pdf"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"], "pdf_unavailable");
}

#[tokio::test]
async fn a_folder_downloads_as_a_zip_through_a_job() {
    let ws = common::workspace();
    sample(&ws);
    let (_home, st) = state(AppConfig::default(), &ws);
    let (status, _, body) = send(
        &st,
        post(
            &st,
            "/api/export/jobs",
            r#"{"scope":"folder","path":"Guides","format":"md"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let started: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = started["id"].as_str().unwrap().to_string();
    assert_eq!(started["total"], 3);
    assert!(
        started["file_name"]
            .as_str()
            .unwrap()
            .starts_with("Guides-")
    );

    let mut finished = false;
    for _ in 0..100 {
        let (_, _, body) = send(&st, get(&st, &format!("/api/export/jobs/{id}"))).await;
        let s: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if s["state"] == "finished" {
            assert_eq!(s["done"], 3);
            finished = true;
            break;
        }
        assert_eq!(s["state"], "running", "{s}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(finished);

    let (status, headers, zip) = send(&st, get(&st, &format!("/api/export/jobs/{id}/file"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "application/zip");
    assert_eq!(
        zip_names(&zip),
        [
            "Guides/other.md",
            "Guides/setup.assets/shot.png",
            "Guides/setup.md"
        ]
    );
    // A finished download is handed over once, then forgotten.
    let (status, _, _) = send(&st, get(&st, &format!("/api/export/jobs/{id}/file"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn folder_jobs_refuse_cairns_own_folder() {
    let ws = common::workspace();
    let (_home, st) = state(AppConfig::default(), &ws);
    let (status, _, _) = send(
        &st,
        post(
            &st,
            "/api/export/jobs",
            r#"{"scope":"folder","path":"_system","format":"md"}"#,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
