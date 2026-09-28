//! Team templates belong to one documentation folder, stay out of the page
//! lists, fill in placeholders, and can be deleted and brought back.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::fsutil::sha256_hex;
use cairn::search::{SearchIndex, all_folders};
use cairn::server::{AppState, api::open_at, build_router};
use cairn::templates;
use http_body_util::BodyExt;
use tower::ServiceExt;

const PORT: u16 = 4324;
const MEETING: &str = "---\ntemplate_name: Meeting notes\n\
                       template_description: Agenda and actions.\nstatus: draft\n---\n\
                       # {{title}}\n\nHeld on {{date}} by {{author}} in {{folder}}.\n";

fn with_template(ws: &common::TestWorkspace) {
    common::write(&ws.path.join("_templates/meeting-notes.md"), MEETING);
    common::write(&ws.path.join("Guides/setup.md"), "# Set up\n");
}

#[test]
fn templates_belong_to_their_own_documentation_folder() {
    let a = common::workspace();
    let b = common::workspace();
    with_template(&a);
    let in_a = templates::list(&a.root).unwrap();
    let in_b = templates::list(&b.root).unwrap();
    assert!(
        in_a.iter()
            .any(|t| t.id == "_templates/meeting-notes.md" && t.name == "Meeting notes")
    );
    assert!(in_b.iter().all(|t| t.builtin), "{in_b:?}");
    assert_eq!(in_a.iter().filter(|t| t.builtin).count(), 4);
}

#[test]
fn templates_never_show_up_as_pages_or_folders() {
    let ws = common::workspace();
    with_template(&ws);
    let mut index = SearchIndex::default();
    index.refresh(&ws.root).unwrap();
    assert!(
        index
            .all()
            .iter()
            .all(|p| !p.path.starts_with("_templates"))
    );
    assert!(index.search("Agenda", 10).is_empty());
    assert!(
        !all_folders(&ws.root)
            .iter()
            .any(|f| f.starts_with("_templates"))
    );
    assert!(index.get("Guides/setup.md").is_some());
}

// ------------------------------------------------------------------ routes

fn state(ws: &common::TestWorkspace) -> (tempfile::TempDir, Arc<AppState>) {
    let home = tempfile::tempdir().unwrap();
    let cfg = AppConfig {
        persistent_drafts: false,
        display_name: Some("Sam".into()),
        ..AppConfig::default()
    };
    let state = AppState::new(home.path().to_path_buf(), cfg, None);
    state.set_port(PORT);
    open_at(&state, &ws.path).unwrap();
    (home, state)
}

async fn call(
    st: &Arc<AppState>,
    method: &str,
    uri: &str,
    json: &str,
) -> (StatusCode, serde_json::Value) {
    let body = if method == "GET" {
        Body::empty()
    } else {
        Body::from(json.to_string())
    };
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .header("content-type", "application/json")
        .body(body)
        .unwrap();
    let res = build_router(st.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or_default())
}

#[tokio::test]
async fn a_page_made_from_a_team_template_has_its_placeholders_filled() {
    let ws = common::workspace();
    with_template(&ws);
    let (_home, st) = state(&ws);
    let (status, page) = call(
        &st,
        "POST",
        "/api/page/new",
        r#"{"folder":"Guides","title":"Weekly sync","template":"_templates/meeting-notes.md"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let content = page["content"].as_str().unwrap();
    assert!(!content.contains("template_"), "{content}");
    assert!(!content.contains("{{"), "{content}");
    assert!(content.contains("# Weekly sync"));
    assert!(content.contains("by Sam in Guides."));
    assert!(content.contains("status: draft"));
}

#[tokio::test]
async fn new_templates_get_a_free_path_inside_templates() {
    let ws = common::workspace();
    with_template(&ws);
    let (_home, st) = state(&ws);
    let (status, made) = call(
        &st,
        "POST",
        "/api/templates/new",
        r#"{"name":"Meeting notes","description":"Copy","from":"builtin:how-to"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["path"], "_templates/meeting-notes-2.md");
    assert!(
        made["content"]
            .as_str()
            .unwrap()
            .contains("template_name: \"Meeting notes\"")
    );

    let (status, _) = call(&st, "POST", "/api/templates/new", r#"{"name":"   "}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deleting_a_template_keeps_it_restorable() {
    let ws = common::workspace();
    with_template(&ws);
    let (_home, st) = state(&ws);
    let rel = "_templates/meeting-notes.md";

    let stale = format!(r#"{{"path":"{rel}","base_hash":"{}"}}"#, "0".repeat(64));
    let (status, _) = call(&st, "POST", "/api/templates/delete", &stale).await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a changed template is not deleted"
    );
    assert!(ws.path.join(rel).exists());

    let hash = sha256_hex(MEETING.as_bytes());
    let body = format!(r#"{{"path":"{rel}","base_hash":"{hash}"}}"#);
    let (status, done) = call(&st, "POST", "/api/templates/delete", &body).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert!(!ws.path.join(rel).exists());

    let (_, deleted) = call(&st, "GET", "/api/templates/deleted", "").await;
    let entry = &deleted["deleted"][0];
    assert_eq!(entry["path"], rel);
    assert_eq!(entry["name"], "Meeting notes");

    let version = entry["latest_version"].as_str().unwrap();
    let body = format!(r#"{{"path":"{rel}","id":"{version}"}}"#);
    let (status, restored) = call(&st, "POST", "/api/history/restore", &body).await;
    assert_eq!(status, StatusCode::OK, "{restored}");
    assert_eq!(std::fs::read_to_string(ws.path.join(rel)).unwrap(), MEETING);
    let (_, deleted) = call(&st, "GET", "/api/templates/deleted", "").await;
    assert_eq!(deleted["deleted"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn only_templates_can_be_deleted_through_the_template_route() {
    let ws = common::workspace();
    with_template(&ws);
    let (_home, st) = state(&ws);
    let hash = sha256_hex(b"# Set up\n");
    let body = format!(r#"{{"path":"Guides/setup.md","base_hash":"{hash}"}}"#);
    let (status, _) = call(&st, "POST", "/api/templates/delete", &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(ws.path.join("Guides/setup.md").exists());
}
