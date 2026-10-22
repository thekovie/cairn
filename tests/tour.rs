//! Whether someone has seen the first-run tour is kept in their own settings,
//! not in the browser: Cairn listens on a different port each time, and a
//! browser keeps what it stores separately for every port.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::server::{AppState, api::open_at, build_router};
use http_body_util::BodyExt;
use tower::ServiceExt;

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

async fn post(st: &Arc<AppState>, uri: &str, json: &str) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
        .header("x-cairn-token", &st.token)
        .header("content-type", "application/json")
        .body(Body::from(json.to_string()))
        .unwrap();
    let res = build_router(st.clone()).oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap_or_default())
}

#[test]
fn a_new_person_has_not_seen_the_tour() {
    assert!(!AppConfig::default().tour_seen);
}

#[test]
fn settings_files_from_before_the_tour_still_load() {
    let cfg: AppConfig = serde_json::from_str(r#"{"display_name":"Ana"}"#).unwrap();
    assert!(!cfg.tour_seen);
}

#[tokio::test]
async fn the_tour_flag_is_saved_and_can_be_cleared_to_replay_it() {
    let ws = common::workspace();
    let (home, st) = state(&ws);

    let (status, view) = post(&st, "/api/settings", r#"{"tour_seen":true}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["config"]["tour_seen"], true);
    assert!(
        cairn::config::load(home.path()).0.tour_seen,
        "the flag has to be on disk, not only in memory"
    );

    let (_, view) = post(&st, "/api/settings", r#"{"tour_seen":false}"#).await;
    assert_eq!(view["config"]["tour_seen"], false);
}
