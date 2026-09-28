//! Times are stored as UTC and only shown in each person's chosen timezone.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::server::{AppState, api::open_at, build_router};
use cairn::timefmt::{format_moment, today, user_zone, validate_zone};
use http_body_util::BodyExt;
use jiff::Timestamp;
use jiff::tz::TimeZone;
use tower::ServiceExt;

const PORT: u16 = 4323;

fn secs(iso: &str) -> i64 {
    iso.parse::<Timestamp>().unwrap().as_second()
}

#[test]
fn one_moment_reads_correctly_in_each_zone() {
    let moment = secs("2026-09-28T06:30:00Z");
    let manila = TimeZone::get("Asia/Manila").unwrap();
    let london = TimeZone::get("Europe/London").unwrap();
    assert_eq!(
        format_moment(moment, &manila),
        "Sep 28, 2026, 2:30 PM GMT+8"
    );
    assert_eq!(
        format_moment(moment, &london),
        "Sep 28, 2026, 7:30 AM GMT+1"
    );
    assert_eq!(
        format_moment(moment, &TimeZone::UTC),
        "Sep 28, 2026, 6:30 AM GMT"
    );
}

#[test]
fn daylight_saving_changes_the_label() {
    let new_york = TimeZone::get("America/New_York").unwrap();
    assert_eq!(
        format_moment(secs("2026-01-15T17:00:00Z"), &new_york),
        "Jan 15, 2026, 12:00 PM GMT-5"
    );
    assert_eq!(
        format_moment(secs("2026-07-15T16:00:00Z"), &new_york),
        "Jul 15, 2026, 12:00 PM GMT-4"
    );
}

#[test]
fn today_depends_on_the_zone() {
    // 25 hours apart, so these two are never on the same calendar date.
    let ahead = TimeZone::get("Pacific/Kiritimati").unwrap();
    let behind = TimeZone::get("Pacific/Pago_Pago").unwrap();
    assert_ne!(today(&ahead), today(&behind));
    assert_eq!(today(&ahead).len(), "2026-09-28".len());
}

#[test]
fn unknown_zones_are_rejected_and_fall_back_safely() {
    assert!(validate_zone("Asia/Manila").is_ok());
    assert!(validate_zone("Mars/Olympus").is_err());
    let cfg = AppConfig {
        timezone: Some("Mars/Olympus".into()),
        ..AppConfig::default()
    };
    assert!(cfg.validate().is_err());
    // A bad name read from somewhere never breaks display.
    let _ = user_zone(Some("Mars/Olympus"));
}

// ------------------------------------------------------------------ routes

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

#[tokio::test]
async fn the_timezone_setting_is_saved_and_can_go_back_to_automatic() {
    let ws = common::workspace();
    let (_home, st) = state(&ws);

    let (status, view) = post(&st, "/api/settings", r#"{"timezone":"America/New_York"}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["config"]["timezone"], "America/New_York");
    assert!(view["config"]["system_timezone"].is_string());

    let (status, _) = post(&st, "/api/settings", r#"{"timezone":"Nowhere/Land"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(st.config().timezone.as_deref(), Some("America/New_York"));

    let (_, view) = post(&st, "/api/settings", r#"{"timezone":""}"#).await;
    assert!(view["config"]["timezone"].is_null());
}

#[tokio::test]
async fn new_pages_are_dated_in_the_chosen_zone() {
    let ws = common::workspace();
    let (_home, st) = state(&ws);
    post(&st, "/api/settings", r#"{"timezone":"Pacific/Kiritimati"}"#).await;
    let (status, page) = post(
        &st,
        "/api/page/new",
        r#"{"folder":"","title":"Dated","template":"builtin:how-to"}"#,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    let expected = today(&TimeZone::get("Pacific/Kiritimati").unwrap());
    let content = page["content"].as_str().unwrap();
    assert!(
        content.contains(&format!("last_reviewed: {expected}")),
        "{content}"
    );
}
