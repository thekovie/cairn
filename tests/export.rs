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
use cairn::export::archive::{
    ZipBuilder, enumerate, folder_files, page_files, pages_in, zip_files,
};
use cairn::export::html::{PrintInput, converted_links, printable_html};
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

/// A folder link pointing outside the documentation: a junction on
/// Windows, a symbolic link elsewhere.
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
fn folder_exports_never_follow_folder_links() {
    let ws = common::workspace();
    sample(&ws);
    let outside = tempfile::tempdir().unwrap();
    common::write(&outside.path().join("secret.md"), "# Secret\n");
    link_folder(&ws.path.join("Guides").join("linked"), outside.path());
    let files = folder_files(&ws.root, "").unwrap();
    assert!(!files.iter().any(|p| p.contains("secret")), "{files:?}");
}

#[test]
fn links_left_out_of_an_export_are_reported() {
    let ws = common::workspace();
    sample(&ws);
    let outside = tempfile::tempdir().unwrap();
    common::write(&outside.path().join("secret.md"), "# Secret\n");
    link_folder(&ws.path.join("Guides").join("linked"), outside.path());
    let found = enumerate(&ws.root, "").unwrap();
    assert!(
        found.skipped_links.contains(&"Guides/linked".to_string()),
        "{:?}",
        found.skipped_links
    );
    assert!(!found.entries.iter().any(|e| e.rel.contains("secret")));
}

#[cfg(unix)]
#[test]
fn an_unreadable_folder_stops_the_export_and_is_named() {
    use std::os::unix::fs::PermissionsExt;
    let ws = common::workspace();
    sample(&ws);
    let locked = ws.path.join("Guides/locked");
    common::write(&locked.join("hidden-from-cairn.md"), "# Locked\n");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
    let can_read_anyway = std::fs::read_dir(&locked).is_ok();
    let result = folder_files(&ws.root, "");
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
    if can_read_anyway {
        eprintln!("skipped: this user can read every folder");
        return;
    }
    let err = result.expect_err("an export must not quietly skip a folder it cannot read");
    assert!(err.to_string().contains("Guides/locked"), "{err}");
}

#[test]
fn a_whole_export_leaves_out_the_workspace_marker() {
    let ws = common::workspace();
    sample(&ws);
    let all = folder_files(&ws.root, "").unwrap();
    assert!(
        !all.iter()
            .any(|p| p.eq_ignore_ascii_case("shared-docs.json")),
        "{all:?}"
    );
}

#[test]
fn a_file_that_changes_during_the_export_is_reported() {
    let ws = common::workspace();
    sample(&ws);
    let found = enumerate(&ws.root, "Guides").unwrap();
    let find = |rel: &str| found.entries.iter().find(|e| e.rel == rel).unwrap();
    common::write(
        &ws.path.join("Guides/other.md"),
        "# Other, and now much longer than it was a moment ago\n",
    );
    let mut zip = ZipBuilder::default();
    assert!(
        zip.add_entry(&ws.root, find("Guides/other.md")).unwrap(),
        "the edited page is flagged"
    );
    assert!(
        !zip.add_entry(&ws.root, find("Guides/setup.md")).unwrap(),
        "an untouched page is not"
    );
}

#[test]
fn a_file_removed_during_the_export_fails_with_its_name() {
    let ws = common::workspace();
    sample(&ws);
    let found = enumerate(&ws.root, "Guides").unwrap();
    let entry = found
        .entries
        .iter()
        .find(|e| e.rel == "Guides/other.md")
        .unwrap();
    std::fs::remove_file(ws.path.join("Guides/other.md")).unwrap();
    let err = ZipBuilder::default()
        .add_entry(&ws.root, entry)
        .expect_err("a vanished file must stop the export");
    assert!(err.to_string().contains("Guides/other.md"), "{err}");
}

#[test]
fn unicode_file_names_survive_an_export() {
    let ws = common::workspace();
    common::write(&ws.path.join("Guides/Résumé 日本語.md"), "# Résumé\n");
    let files = folder_files(&ws.root, "Guides").unwrap();
    assert_eq!(files, ["Guides/Résumé 日本語.md"]);
    let bytes = zip_files(&ws.root, &files).unwrap();
    assert_eq!(zip_names(&bytes), ["Guides/Résumé 日本語.md"]);
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

/// The printable HTML of `Guides/page.md` with this text, in a workspace
/// that also has the pages used in the links below.
fn printed_full(text: &str) -> String {
    let ws = common::workspace();
    sample(&ws);
    common::write(&ws.path.join("Reference/api.md"), "# API Guide\n");
    std::fs::write(ws.path.join("Reference/sheet.pdf"), b"%PDF-1.4").unwrap();
    common::write(&ws.path.join("Guides/Résumé.md"), "# Résumé\n");
    // Entities in a title become real angle brackets and quotes in its text.
    common::write(
        &ws.path.join("Guides/evil.md"),
        "# Tom &amp; Jerry &lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;\n",
    );
    common::write(&ws.path.join("Guides/page.md"), text);
    let zone = jiff::tz::TimeZone::UTC;
    let mut input = print_input(&ws, text, &zone);
    input.rel = "Guides/page.md";
    printable_html(&input)
}

/// Just the printed page (without the stylesheets), so a failure is readable.
fn printed(text: &str) -> String {
    let html = printed_full(text);
    html[html.find("<body").unwrap()..].to_string()
}

#[test]
fn a_link_to_another_page_keeps_its_meaning_in_a_pdf() {
    let html =
        printed("# Page\n\nSee [the other page](other.md) and [the web](https://example.com).\n");
    assert!(
        html.contains("the other page</a> <span class=\"print-ref\">(see “Other”)</span>"),
        "{html}"
    );
    assert!(!html.contains("#/page/"));
    assert!(
        html.contains("href=\"https://example.com\""),
        "web links stay"
    );
}

#[test]
fn no_note_is_added_when_the_link_already_says_the_page_name() {
    let html = printed("# Page\n\nSee [Other](other.md) and [other](Other.md).\n");
    assert!(!html.contains("print-ref"), "{html}");
}

#[test]
fn a_page_in_another_folder_says_where_it_is() {
    let html = printed("# Page\n\nSee [the reference](../Reference/api.md).\n");
    assert!(html.contains("(see “API Guide” in Reference)"), "{html}");
}

#[test]
fn a_link_to_a_file_shows_which_file() {
    let html = printed("# Page\n\nSee [the sheet](../Reference/sheet.pdf).\n");
    assert!(html.contains("(file “Reference/sheet.pdf”)"), "{html}");
    assert!(!html.contains("/ws-file/"));
}

#[test]
fn accented_names_and_encoded_links_read_correctly() {
    let html = printed("# Page\n\nSee [my CV](R%C3%A9sum%C3%A9.md).\n");
    assert!(html.contains("(see “Résumé”)"), "{html}");
}

#[test]
fn a_page_name_is_escaped_in_the_note() {
    let html = printed("# Page\n\nSee [it](evil.md).\n");
    assert!(
        html.contains("(see “Tom &amp; Jerry &lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt;”)"),
        "{html}"
    );
    assert!(!html.contains("<script"));
}

#[test]
fn broken_links_are_left_exactly_as_they_were() {
    let html = printed("# Page\n\nSee [gone](Nope.md).\n");
    assert!(html.contains("broken-link"), "{html}");
    assert!(!html.contains("print-ref"));
}

#[test]
fn links_to_headings_work_inside_the_pdf_including_the_title() {
    let html = printed(
        "# Page title\n\n[jump](#steps) [top](#page-title) [same page](page.md#steps) [self](page.md)\n\n## Steps\n\ntext\n",
    );
    assert!(
        html.contains("<h1 id=\"page-title\">"),
        "the title can be linked: {html}"
    );
    assert_eq!(html.matches("id=\"page-title\"").count(), 1);
    assert!(html.contains("href=\"#steps\""));
    assert!(html.contains("href=\"#page-title\""));
    assert_eq!(
        html.matches("href=\"#steps\"").count(),
        2,
        "a link to a heading on the same page becomes a plain heading link"
    );
    assert!(html.contains("<h2 id=\"steps\">"));
    assert!(!html.contains("print-ref"), "no note for the page itself");
    assert!(!html.contains("#/page/"));
}

#[test]
fn nothing_is_loaded_from_the_internet() {
    let html = printed_full(
        "# Page\n\n![remote](https://example.com/x.png)\n\n[web](https://example.com)\n",
    );
    for banned in [
        "<link",
        "@import",
        "src=\"http",
        "url(http",
        "<script",
        "<iframe",
    ] {
        assert!(
            !html.contains(banned),
            "{banned} found in the printable page"
        );
    }
    assert!(html.contains("default-src 'none'"));
    assert!(html.contains("External picture (not loaded)"));
}

#[test]
fn only_links_a_pdf_cannot_follow_are_counted_as_converted() {
    let ws = common::workspace();
    sample(&ws);
    std::fs::create_dir_all(ws.path.join("Reference")).unwrap();
    std::fs::write(ws.path.join("Reference/sheet.pdf"), b"%PDF-1.4").unwrap();
    let text = "# Page\n\n\
        [a page](other.md) [again](other.md) [a file](../Reference/sheet.pdf)\n\n\
        [heading here](#steps) [this page](page.md#steps) [and again](page.md)\n\n\
        [web](https://example.com) [mail](mailto:a@b.co) [gone](Nope.md)\n\n\
        ## Steps\n";
    let found = converted_links(&ws.root, "Guides/page.md", text);
    assert_eq!(
        found,
        ["Guides/other.md", "Guides/other.md", "Reference/sheet.pdf"],
        "links to other pages and files are converted; headings here, the page itself, \
         web and email addresses, and broken links are not"
    );
    assert!(converted_links(&ws.root, "Guides/page.md", "# Page\n\nNo links.\n").is_empty());
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

#[tokio::test]
async fn the_pdf_link_report_counts_what_a_download_would_convert() {
    let ws = common::workspace();
    sample(&ws);
    common::write(&ws.path.join("Guides/plain.md"), "# Plain\n\nNo links.\n");
    common::write(
        &ws.path.join("Root page.md"),
        "# Root page\n\n[guide](Guides/other.md)\n",
    );
    let (_home, st) = state(AppConfig::default(), &ws);
    let ask = |uri: &str| {
        let uri = uri.to_string();
        let st = st.clone();
        async move {
            let (status, _, body) = send(&st, get(&st, &uri)).await;
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&body).unwrap_or_default(),
            )
        }
    };

    // One page: sample's setup.md links to other.md (and to the web).
    let (status, one) = ask("/api/export/pdf-links?scope=page&path=Guides%2Fsetup.md").await;
    assert_eq!(status, StatusCode::OK, "{one}");
    assert_eq!(
        (one["links"].as_u64(), one["pages"].as_u64()),
        (Some(1), Some(1))
    );
    assert_eq!(one["examples"][0]["page"], "Guides/setup.md");
    assert_eq!(one["examples"][0]["targets"][0], "Guides/other.md");

    // A page with no links to convert.
    let (_, none) = ask("/api/export/pdf-links?scope=page&path=Guides%2Fplain.md").await;
    assert_eq!(
        (none["links"].as_u64(), none["pages"].as_u64()),
        (Some(0), Some(0))
    );

    // A folder, and everything.
    let (_, folder) = ask("/api/export/pdf-links?scope=folder&path=Guides").await;
    assert_eq!(
        (folder["links"].as_u64(), folder["pages"].as_u64()),
        (Some(1), Some(1))
    );
    let (_, all) = ask("/api/export/pdf-links?scope=all").await;
    assert_eq!(
        (all["links"].as_u64(), all["pages"].as_u64()),
        (Some(2), Some(2))
    );
}

#[tokio::test]
async fn the_pdf_link_report_refuses_what_it_does_not_understand() {
    let ws = common::workspace();
    sample(&ws);
    let (_home, st) = state(AppConfig::default(), &ws);
    for bad in [
        "/api/export/pdf-links?scope=bogus",
        "/api/export/pdf-links?scope=page&path=..%2Fx.md",
        "/api/export/pdf-links?scope=page&path=notes.txt",
        "/api/export/pdf-links?scope=folder&path=_system",
    ] {
        let (status, _, _) = send(&st, get(&st, bad)).await;
        assert!(
            status == StatusCode::BAD_REQUEST || status == StatusCode::FORBIDDEN,
            "{bad}: {status}"
        );
    }
    let no_token = Request::builder()
        .uri("/api/export/pdf-links?scope=all")
        .header("host", format!("127.0.0.1:{PORT}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&st, no_token).await.0, StatusCode::UNAUTHORIZED);
}
