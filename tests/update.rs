//! Updates: finding a newer release, refusing anything not signed with the
//! right key, installing next to a (stand-in) program, going back, and the
//! hand-over to the restarted program.

use std::io::{Cursor, Write};
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::routing::get;
use cairn::error::CairnError;
use cairn::server::api_update::take_handoff;
use cairn::update::{self, UpdateSource};
use minisign::KeyPair;
use semver::Version;
use zip::write::SimpleFileOptions;

const NEW_EXE: &[u8] = b"MZ pretend program for 0.9.0";

/// This computer's download for `version`, as the release workflow names it.
fn asset(version: &str) -> String {
    update::asset_name(&v(version)).expect("releases are built for this computer")
}

/// Where a downloaded program waits beside the current one.
fn new_program_name(version: &str) -> String {
    format!("cairn-{version}.new{}", std::env::consts::EXE_SUFFIX)
}

/// This computer's release file holding `exe`: a zip with `cairn.exe` on
/// Windows, a zip with `Cairn.app` on a Mac, the program itself on Linux.
fn release_file(exe: &[u8]) -> Vec<u8> {
    let platform = update::Platform::current().unwrap();
    let Some(program) = platform.program_in_zip() else {
        return exe.to_vec();
    };
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();
    let mut add = |name: &str, data: &[u8]| {
        w.start_file(name, opts).unwrap();
        w.write_all(data).unwrap();
    };
    add("README.md", b"# Cairn");
    if platform == update::Platform::Mac {
        // The launcher sits beside the program; only `cairn` is the program.
        add("Cairn.app/Contents/Info.plist", b"<plist/>");
        add(
            "Cairn.app/Contents/MacOS/start-cairn",
            b"#!/bin/sh\nopen -a Terminal",
        );
        add(&format!("Cairn.app/Contents/MacOS/{program}"), exe);
    } else {
        add(program, exe);
    }
    add("docs/getting-started.md", b"# Start");
    w.finish().unwrap().into_inner()
}

fn sign(pair: &KeyPair, data: &[u8], name: &str) -> Vec<u8> {
    minisign::sign(
        None,
        &pair.sk,
        Cursor::new(data),
        Some(&format!("file:{name}")),
        None,
    )
    .unwrap()
    .into_string()
    .into_bytes()
}

/// A stand-in for GitHub: the release JSON, the zip, and its signature.
struct FakeGitHub {
    addr: SocketAddr,
    _runtime: tokio::runtime::Runtime,
}

impl FakeGitHub {
    fn start(tag: &str, zip: Vec<u8>, sig: Vec<u8>) -> FakeGitHub {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let version = tag.trim_start_matches('v').to_string();
        let name = asset(&version);
        let json = serde_json::json!({
            "tag_name": tag,
            "body": "## What's new\n\n- Something **better**.",
            "html_url": format!("http://{addr}/release"),
            "published_at": "2026-10-01T09:00:00Z",
            "assets": [
                { "name": name, "browser_download_url": format!("http://{addr}/zip") },
                { "name": format!("{name}.minisig"), "browser_download_url": format!("http://{addr}/sig") },
            ],
        })
        .to_string();
        let (zip, sig) = (Arc::new(zip), Arc::new(sig));
        let app = Router::new()
            .route("/latest", get(move || async move { json }))
            .route("/zip", get(move || async move { zip.as_ref().clone() }))
            .route("/sig", get(move || async move { sig.as_ref().clone() }));
        runtime.spawn(async move { axum::serve(listener, app).await.unwrap() });
        FakeGitHub {
            addr,
            _runtime: runtime,
        }
    }

    fn source(&self, pair: &KeyPair) -> UpdateSource {
        UpdateSource {
            api_url: format!("http://{}/latest", self.addr),
            public_key: pair.pk.to_base64(),
            allow_http: true,
            proxy: None,
        }
    }
}

/// A minimal office-style proxy: answers `CONNECT` and tunnels every
/// connection to `target`, whatever name was asked for. Counts tunnels.
fn connect_proxy(target: SocketAddr) -> (SocketAddr, Arc<std::sync::atomic::AtomicUsize>) {
    use std::io::{BufRead, BufReader};
    use std::net::{TcpListener, TcpStream};
    use std::sync::atomic::{AtomicUsize, Ordering};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let tunnels = Arc::new(AtomicUsize::new(0));
    let count = tunnels.clone();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let count = count.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(client.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if !line.starts_with("CONNECT ") {
                    return;
                }
                while line != "\r\n" {
                    line.clear();
                    if reader.read_line(&mut line).unwrap() == 0 {
                        return;
                    }
                }
                count.fetch_add(1, Ordering::SeqCst);
                let upstream = TcpStream::connect(target).unwrap();
                let mut client_w = client;
                client_w
                    .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                    .unwrap();
                let (mut up_r, mut up_w) = (upstream.try_clone().unwrap(), upstream);
                let mut down_w = client_w.try_clone().unwrap();
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut reader, &mut up_w);
                });
                let _ = std::io::copy(&mut up_r, &mut down_w);
            });
        }
    });
    (addr, tunnels)
}

#[test]
fn updates_go_through_the_proxy_set_in_settings() {
    let pair = KeyPair::generate_unencrypted_keypair().unwrap();
    let zip = release_file(NEW_EXE);
    let sig = sign(&pair, &zip, &asset("0.9.0"));
    let github = FakeGitHub::start("v0.9.0", zip, sig);
    let (proxy, tunnels) = connect_proxy(github.addr);
    // A name that doesn't exist: only the proxy can reach it.
    let source = UpdateSource {
        api_url: "http://updates.cairn.invalid/latest".into(),
        public_key: pair.pk.to_base64(),
        allow_http: true,
        proxy: Some(proxy.to_string()),
    };
    let found = update::check(&source, &v("0.5.0")).unwrap().unwrap();
    assert_eq!(found.version, "0.9.0");
    assert!(tunnels.load(std::sync::atomic::Ordering::SeqCst) >= 1);

    // A proxy that isn't there says so.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead = closed.local_addr().unwrap().to_string();
    drop(closed);
    let err = update::check(
        &UpdateSource {
            proxy: Some(dead.clone()),
            ..source
        },
        &v("0.5.0"),
    )
    .unwrap_err();
    assert!(err.to_string().contains(&dead), "{err}");
}

fn v(s: &str) -> Version {
    Version::parse(s).unwrap()
}

#[test]
fn a_signed_newer_release_is_found_downloaded_and_unpacked() {
    let pair = KeyPair::generate_unencrypted_keypair().unwrap();
    let zip = release_file(NEW_EXE);
    let sig = sign(&pair, &zip, &asset("0.9.0"));
    let gh = FakeGitHub::start("v0.9.0", zip, sig);
    let source = gh.source(&pair);

    let release = update::check(&source, &v("0.4.0")).unwrap().unwrap();
    assert_eq!(release.version, "0.9.0");
    assert!(release.notes.contains("Something **better**"));
    // The same or a newer version installed: nothing to do.
    assert!(update::check(&source, &v("0.9.0")).unwrap().is_none());

    let dir = tempfile::tempdir().unwrap();
    let path = update::download(&source, &release, dir.path()).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), NEW_EXE);
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        new_program_name("0.9.0")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o755, "the new program can run");
    }
}

/// Off Windows, `Cairn` and `cairn` are different files: the Mac app's
/// launcher must never be taken for the program.
#[cfg(not(windows))]
#[test]
fn only_the_exact_program_name_is_taken_from_a_zip() {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default();
    w.start_file("Cairn.app/Contents/MacOS/Cairn", opts)
        .unwrap();
    w.write_all(b"launcher").unwrap();
    w.start_file("Cairn.app/Contents/MacOS/cairn", opts)
        .unwrap();
    w.write_all(NEW_EXE).unwrap();
    let zip = w.finish().unwrap().into_inner();
    assert_eq!(update::extract_program(&zip, "cairn").unwrap(), NEW_EXE);
}

#[test]
fn plain_http_is_refused_outside_tests() {
    let pair = KeyPair::generate_unencrypted_keypair().unwrap();
    let zip = release_file(NEW_EXE);
    let gh = FakeGitHub::start("v0.9.0", zip.clone(), sign(&pair, &zip, &asset("0.9.0")));
    let source = UpdateSource {
        allow_http: false,
        ..gh.source(&pair)
    };
    let err = update::check(&source, &v("0.4.0")).unwrap_err();
    assert!(err.to_string().contains("https"), "{err}");
}

#[test]
fn downloads_not_signed_by_the_right_key_are_refused() {
    let right = KeyPair::generate_unencrypted_keypair().unwrap();
    let wrong = KeyPair::generate_unencrypted_keypair().unwrap();
    let zip = release_file(NEW_EXE);
    let dir = tempfile::tempdir().unwrap();
    let refused = |gh: &FakeGitHub| {
        let source = gh.source(&right);
        let release = update::check(&source, &v("0.4.0")).unwrap().unwrap();
        let err = update::download(&source, &release, dir.path()).unwrap_err();
        assert!(
            matches!(err, CairnError::Io(ref m) if m.contains("safety check")),
            "{err}"
        );
        assert!(!dir.path().join(new_program_name("0.9.0")).exists());
    };

    // Signed with someone else's key.
    refused(&FakeGitHub::start(
        "v0.9.0",
        zip.clone(),
        sign(&wrong, &zip, &asset("0.9.0")),
    ));
    // Right key, but the zip was changed after signing.
    let mut tampered = zip.clone();
    let at = tampered.len() / 2;
    tampered[at] ^= 0xFF;
    refused(&FakeGitHub::start(
        "v0.9.0",
        tampered,
        sign(&right, &zip, &asset("0.9.0")),
    ));
    // Right key, but signed as a different file (an older release, say).
    refused(&FakeGitHub::start(
        "v0.9.0",
        zip.clone(),
        sign(&right, &zip, &asset("0.2.0")),
    ));
    // No real signature at all.
    refused(&FakeGitHub::start(
        "v0.9.0",
        zip,
        b"not a signature".to_vec(),
    ));
}

#[test]
fn release_notes_are_rendered_safely() {
    let html = cairn::article::render_notes(
        "## New\n\n- **Bold** item\n\n<script>alert(1)</script>\n\n![x](https://evil.test/p.png)",
    );
    assert!(html.contains("<strong>Bold</strong>"));
    assert!(!html.contains("<script>"));
    assert!(!html.contains("<img"));
}

#[test]
fn installing_keeps_the_old_program_and_going_back_swaps_them() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("cairn.exe");
    std::fs::write(&target, b"old program 0.4.0").unwrap();
    let new_exe = dir.path().join("cairn-0.9.0.new.exe");
    std::fs::write(&new_exe, NEW_EXE).unwrap();
    assert_eq!(update::previous_version(&target), None);

    update::install(&target, &new_exe, &v("0.4.0")).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), NEW_EXE);
    assert!(!new_exe.exists());
    assert_eq!(update::previous_version(&target).as_deref(), Some("0.4.0"));
    assert_eq!(
        std::fs::read(dir.path().join(update::PREVIOUS_EXE)).unwrap(),
        b"old program 0.4.0"
    );

    // Going back puts 0.4.0 in place and keeps 0.9.0, so it can be undone.
    let back_to = update::rollback(&target, &v("0.9.0")).unwrap();
    assert_eq!(back_to, "0.4.0");
    assert_eq!(std::fs::read(&target).unwrap(), b"old program 0.4.0");
    assert_eq!(update::previous_version(&target).as_deref(), Some("0.9.0"));
    assert_eq!(
        std::fs::read(dir.path().join(update::PREVIOUS_EXE)).unwrap(),
        NEW_EXE
    );
}

#[test]
fn going_back_needs_a_kept_version() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("cairn.exe");
    std::fs::write(&target, b"program").unwrap();
    assert!(matches!(
        update::rollback(&target, &v("0.4.0")).unwrap_err(),
        CairnError::NotFound(_)
    ));
}

#[test]
fn the_handover_file_is_used_once_and_only_while_fresh() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "a".repeat(64);
    let write = |created_at: u64, token: &str| {
        let h = serde_json::json!({
            "token": token, "read_key": secret, "port": 47999, "created_at": created_at,
        });
        std::fs::write(dir.path().join("handoff.json"), h.to_string()).unwrap();
    };
    write(cairn::locks::now_secs(), &secret);
    let h = take_handoff(dir.path()).unwrap();
    assert_eq!((h.port, h.token.len()), (47999, 64));
    assert!(
        take_handoff(dir.path()).is_none(),
        "read once, then deleted"
    );

    write(cairn::locks::now_secs() - 3600, &secret);
    assert!(take_handoff(dir.path()).is_none(), "too old");
    write(cairn::locks::now_secs(), "short");
    assert!(take_handoff(dir.path()).is_none(), "malformed token");
}
