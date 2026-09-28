//! Acceptance criteria 10 and 11: filesystem permissions are the authority,
//! and the local server is reachable only from this computer with the
//! per-launch token.

mod common;

use std::net::{IpAddr, TcpStream, UdpSocket};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use cairn::config::AppConfig;
use cairn::server::{AppState, bind_loopback, build_router};
use tower::ServiceExt;

// ----------------------------------------------------------------- AC 10

#[cfg(windows)]
struct DenyWrite {
    dir: std::path::PathBuf,
    who: String,
}

#[cfg(windows)]
impl DenyWrite {
    fn apply(dir: &std::path::Path) -> DenyWrite {
        let who = format!(
            "{}\\{}",
            std::env::var("USERDOMAIN").unwrap_or_default(),
            std::env::var("USERNAME").unwrap()
        );
        let out = std::process::Command::new("icacls")
            .arg(dir)
            .args(["/deny", &format!("{who}:(OI)(CI)(WD,AD,DC,WA)")])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "icacls failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        DenyWrite {
            dir: dir.to_path_buf(),
            who,
        }
    }
}

#[cfg(windows)]
impl Drop for DenyWrite {
    fn drop(&mut self) {
        let _ = std::process::Command::new("icacls")
            .arg(&self.dir)
            .args(["/remove:d", &self.who, "/T"])
            .output();
    }
}

#[cfg(windows)]
#[test]
fn a_user_without_write_permission_cannot_publish_into_a_protected_folder() {
    use cairn::error::CairnError;
    use cairn::fsutil::{can_write_dir, sha256_hex};
    use cairn::locks;
    use cairn::publish::{PublishOptions, PublishRequest, publish};

    let ws = common::workspace();
    let page = "Protected/policy.md";
    let original = "# Policy\n\nOnly some people may change this.\n";
    common::write(&ws.path.join(page), original);
    let alex = common::identity("Alex");
    locks::acquire(&ws.root, page, &alex).unwrap();

    let guard = DenyWrite::apply(&ws.path.join("Protected"));
    assert!(
        !can_write_dir(&ws.path.join("Protected")),
        "the deny rule should be in effect"
    );

    let base = sha256_hex(original.as_bytes());
    let result = publish(
        PublishRequest {
            root: &ws.root,
            article_rel: page,
            identity: &alex,
            base_hash: Some(&base),
            content: "# Policy\n\nChanged by someone without rights.\n",
            staged: vec![],
        },
        &PublishOptions::default(),
    );
    assert!(
        matches!(result, Err(CairnError::PermissionDenied(_))),
        "{result:?}"
    );
    drop(guard);
    assert_eq!(
        std::fs::read_to_string(ws.path.join(page)).unwrap(),
        original
    );
}

// ----------------------------------------------------------------- AC 11

const PORT: u16 = 4321;

fn state() -> (tempfile::TempDir, Arc<AppState>) {
    let home = tempfile::tempdir().unwrap();
    let cfg = AppConfig {
        persistent_drafts: false,
        ..AppConfig::default()
    };
    let state = AppState::new(home.path().to_path_buf(), cfg, None);
    state.set_port(PORT);
    (home, state)
}

async fn send(state: &Arc<AppState>, req: Request<Body>) -> axum::response::Response {
    build_router(state.clone()).oneshot(req).await.unwrap()
}

fn local(uri: &str) -> axum::http::request::Builder {
    Request::builder()
        .uri(uri)
        .header("host", format!("127.0.0.1:{PORT}"))
}

#[tokio::test]
async fn api_requires_the_per_launch_token() {
    let (_home, st) = state();
    let res = send(&st, local("/api/state").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let wrong = local("/api/state").header("x-cairn-token", "0".repeat(64));
    assert_eq!(
        send(&st, wrong.body(Body::empty()).unwrap()).await.status(),
        StatusCode::UNAUTHORIZED
    );

    let right = local("/api/state").header("x-cairn-token", &st.token);
    let res = send(&st, right.body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["cache-control"], "no-store");
}

#[tokio::test]
async fn write_endpoints_reject_other_origins_and_rebinding_hosts() {
    let (_home, st) = state();
    let cross = local("/api/settings")
        .method("POST")
        .header("x-cairn-token", &st.token)
        .header("origin", "https://evil.example")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&st, cross).await.status(), StatusCode::FORBIDDEN);

    let fetch_meta = local("/api/settings")
        .method("POST")
        .header("x-cairn-token", &st.token)
        .header("sec-fetch-site", "cross-site")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(send(&st, fetch_meta).await.status(), StatusCode::FORBIDDEN);

    // DNS rebinding: a web page's own host name pointed at 127.0.0.1.
    let rebinding = Request::builder()
        .uri("/api/state")
        .header("host", format!("evil.example:{PORT}"))
        .header("x-cairn-token", &st.token)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&st, rebinding).await.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn file_urls_need_the_read_key_and_every_response_has_a_strict_csp() {
    let (_home, st) = state();
    let res = send(
        &st,
        local("/ws-file/README.md").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

    let with_key = local(&format!("/ws-file/README.md?k={}", st.read_key));
    let res = send(&st, with_key.body(Body::empty()).unwrap()).await;
    assert_ne!(
        res.status(),
        StatusCode::UNAUTHORIZED,
        "key accepted (no workspace, so not found)"
    );

    let res = send(&st, local("/").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    let csp = res.headers()["content-security-policy"].to_str().unwrap();
    assert!(csp.contains("default-src 'none'") && csp.contains("script-src 'self'"));
    assert!(!csp.contains("unsafe-inline"));
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
}

/// This computer's address on the local network, if it has one.
fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // UDP "connect" sends nothing; it only selects the outgoing interface.
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[tokio::test]
async fn the_server_listens_on_loopback_only() {
    let listener = bind_loopback(0).await.unwrap();
    let addr = listener.local_addr().unwrap();
    assert!(addr.ip().is_loopback());

    // Connecting through the machine's network address, as another computer
    // would, must fail; loopback works.
    let port = addr.port();
    let accept = tokio::spawn(async move {
        let _ = tokio::time::timeout(Duration::from_secs(3), listener.accept()).await;
    });
    let loopback_ok = tokio::task::spawn_blocking(move || {
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2)).is_ok()
    })
    .await
    .unwrap();
    assert!(loopback_ok);
    if let Some(ip) = lan_address() {
        let reachable = tokio::task::spawn_blocking(move || {
            TcpStream::connect_timeout(&(ip, port).into(), Duration::from_secs(2)).is_ok()
        })
        .await
        .unwrap();
        assert!(!reachable, "the server must not be reachable on {ip}");
    }
    accept.abort();
}
