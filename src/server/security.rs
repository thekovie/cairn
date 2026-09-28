//! Request gatekeeping for the local server.
//!
//! - Every request must carry `Host: 127.0.0.1:<port>` (or `localhost`),
//!   which defeats DNS-rebinding attacks from web pages.
//! - Every `/api/` request must carry the per-launch token in the
//!   `X-Cairn-Token` header. Browsers only send custom headers cross-origin
//!   after a CORS preflight, which this server never approves, so unrelated
//!   web pages cannot call the API. The token also stops other local programs
//!   that don't have it.
//! - `<img>` requests can't carry headers, so workspace files and draft
//!   pictures require a separate per-launch read key in the query string.
//! - Every response carries a strict Content-Security-Policy.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::AppState;

pub const TOKEN_HEADER: &str = "x-cairn-token";

const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'; \
                   font-src 'self'; connect-src 'self'; base-uri 'none'; form-action 'none'; \
                   frame-ancestors 'none'";

/// Constant-time comparison so the token can't be guessed byte by byte.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn deny(status: StatusCode, msg: &str) -> Response {
    let body = serde_json::json!({ "error": "forbidden", "message": msg });
    (status, axum::Json(body)).into_response()
}

fn host_ok(headers: &HeaderMap, port: u16) -> bool {
    let Some(host) = headers.get(header::HOST).and_then(|h| h.to_str().ok()) else {
        return false;
    };
    host == format!("127.0.0.1:{port}") || host == format!("localhost:{port}")
}

fn origin_ok(headers: &HeaderMap, port: u16) -> bool {
    match headers.get(header::ORIGIN).and_then(|h| h.to_str().ok()) {
        None => true,
        Some(origin) => {
            origin == format!("http://127.0.0.1:{port}")
                || origin == format!("http://localhost:{port}")
        }
    }
}

fn cross_site(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("cross-site"))
}

fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == name).then_some(v)
    })
}

pub async fn guard(State(state): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let port = state.port();
    let headers = req.headers();
    if !host_ok(headers, port) {
        return deny(
            StatusCode::FORBIDDEN,
            "This address is only available on this computer.",
        );
    }
    if !origin_ok(headers, port) || cross_site(headers) {
        return deny(
            StatusCode::FORBIDDEN,
            "Requests from other websites are not allowed.",
        );
    }
    let path = req.uri().path();
    let is_api = path.starts_with("/api/");
    if is_api {
        let supplied = headers
            .get(TOKEN_HEADER)
            .map(|v| v.as_bytes())
            .unwrap_or_default();
        if !constant_time_eq(supplied, state.token.as_bytes()) {
            return deny(
                StatusCode::UNAUTHORIZED,
                "This page has lost its connection to Cairn. Please reopen Cairn from its \
                 program window.",
            );
        }
    } else if path.starts_with("/ws-file/") || path == "/draft-file" {
        let key = req
            .uri()
            .query()
            .and_then(|q| query_param(q, "k"))
            .unwrap_or_default();
        if !constant_time_eq(key.as_bytes(), state.read_key.as_bytes()) {
            return deny(StatusCode::UNAUTHORIZED, "Not allowed.");
        }
    }

    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(CSP),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    h.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    if is_api {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_compare() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
