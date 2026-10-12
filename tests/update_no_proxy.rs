//! `NO_PROXY` is respected. This is its own test program because it sets
//! environment variables, which other tests in the same program would see.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use cairn::update::{self, UpdateSource};
use semver::Version;

/// A stand-in for GitHub that answers the release question.
fn fake_github() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for client in listener.incoming().flatten() {
            let mut reader = BufReader::new(client.try_clone().unwrap());
            let mut out = client;
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) && line != "\r\n" {
                line.clear();
            }
            let body = r#"{"tag_name":"v0.0.1"}"#;
            let _ = write!(
                out,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    format!("http://{addr}/latest")
}

#[test]
fn addresses_in_no_proxy_are_reached_directly() {
    let api_url = fake_github();
    let dead = {
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        closed.local_addr().unwrap()
    };
    // SAFETY: this is the only test in this program, so no other thread is
    // reading the environment while it is changed.
    unsafe {
        std::env::set_var("HTTP_PROXY", format!("http://{dead}"));
        std::env::set_var("HTTPS_PROXY", format!("http://{dead}"));
        std::env::set_var("NO_PROXY", "127.0.0.1");
    }
    let source = UpdateSource {
        api_url,
        public_key: String::new(),
        allow_http: true,
        proxy: None,
    };
    let current = Version::parse("1.0.0").unwrap();
    // Reached directly, and there is nothing newer than this version.
    assert!(update::check(&source, &current).unwrap().is_none());

    // Without the exception the dead proxy is used, and says so.
    // SAFETY: as above.
    unsafe { std::env::remove_var("NO_PROXY") };
    let err = update::check(&source, &current).unwrap_err();
    assert!(err.to_string().contains("couldn't reach GitHub"), "{err}");
}
