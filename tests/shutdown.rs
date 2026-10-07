//! Stopping Cairn without Ctrl+C (closing its window, signing out, or a
//! SIGTERM) must still give back every edit lock, or the team sees
//! "Being edited by…" until a maintainer clears it by hand.
//!
//! Runs the real program. Unix only: there's no way to send a Windows
//! close-window event to a child process from a test.
#![cfg(unix)]

mod common;

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use cairn::locks::lock_path;

struct Running {
    child: Child,
    base: String,
    token: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn start(workspace: &std::path::Path, home: &std::path::Path) -> Running {
    std::fs::write(home.join("config.json"), r#"{"update_check":"manual"}"#).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_cairn"))
        .args(["--no-browser", "--workspace"])
        .arg(workspace)
        .env("CAIRN_HOME", home)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start cairn");
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let url = stdout
        .lines()
        .map_while(Result::ok)
        .find_map(|l| {
            l.trim()
                .starts_with("http://")
                .then(|| l.trim().to_string())
        })
        .expect("cairn printed its address");
    let (base, token) = url.split_once("/#t=").unwrap();
    Running {
        child,
        base: base.to_string(),
        token: token.to_string(),
    }
}

fn post(cairn: &Running, path: &str, body: serde_json::Value) -> serde_json::Value {
    ureq::post(format!("{}{path}", cairn.base))
        .header("X-Cairn-Token", &cairn.token)
        .content_type("application/json")
        .send(body.to_string())
        .expect("request")
        .body_mut()
        .read_to_string()
        .map(|text| serde_json::from_str(&text).expect("json"))
        .expect("response body")
}

fn wait_for_exit(child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("cairn did not stop within 10 seconds");
}

#[test]
fn sigterm_gives_back_edit_locks() {
    let ws = common::workspace();
    let home = tempfile::tempdir().unwrap();
    let mut cairn = start(&ws.path, home.path());

    let started = post(
        &cairn,
        "/api/edit/start",
        serde_json::json!({ "path": "README.md", "is_new": false }),
    );
    assert_ne!(started["status"], "locked", "{started}");
    let lock = lock_path(&ws.root, "README.md");
    assert!(lock.exists(), "editing should take the lock");

    let pid = cairn.child.id().to_string();
    let sent = Command::new("kill").args(["-TERM", &pid]).status().unwrap();
    assert!(sent.success());
    wait_for_exit(&mut cairn.child);

    assert!(!lock.exists(), "the lock was left behind after SIGTERM");
}
