//! Windows' own networking (WinHTTP), for networks Cairn's built-in client
//! can't get through: a proxy that wants the person's Windows sign-in (NTLM
//! or Kerberos), or one found by a proxy script (PAC / WPAD).
//!
//! Windows does the sign-in itself, as the person who is logged in. Cairn
//! never sees, asks for, or keeps a password, and only offers the sign-in
//! methods that use the Windows login (Negotiate, NTLM); a proxy that wants a
//! typed password (Basic, Digest) is left unanswered. Certificates are
//! checked by Windows against its own store, and no check is ever switched off.

use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ACCESS_TYPE_NAMED_PROXY,
    WINHTTP_AUTH_SCHEME_NEGOTIATE, WINHTTP_AUTH_SCHEME_NTLM, WINHTTP_AUTH_TARGET_PROXY,
    WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle,
    WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryAuthSchemes, WinHttpQueryHeaders,
    WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetCredentials,
    WinHttpSetTimeouts,
};

use crate::update_net::NetFail;

/// Sign-in rounds before giving up: a handshake takes a couple of rounds,
/// and a proxy that still says no after that won't change its mind.
const SIGN_IN_ROUNDS: u32 = 4;
/// Longest wait for any one step; the whole request has its own limit.
const STEP_TIMEOUT: Duration = Duration::from_secs(60);
const READ_CHUNK: usize = 16 * 1024;

// Windows' certificate errors (ERROR_WINHTTP_SECURE_*, ERROR_WINHTTP_*CERT*).
const CERTIFICATE_ERRORS: [u32; 7] = [12037, 12038, 12044, 12045, 12169, 12170, 12175];
const ERROR_WINHTTP_TIMEOUT: u32 = 12002;

/// A WinHTTP handle, closed when dropped.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void) -> Result<Handle, NetFail> {
        if raw.is_null() {
            Err(last_error())
        } else {
            Ok(Handle(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the handle came from a WinHTTP open call and is closed
        // exactly once, here.
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}

fn last_error() -> NetFail {
    // SAFETY: only reads this thread's last error code.
    let code = unsafe { GetLastError() };
    if code == ERROR_WINHTTP_TIMEOUT {
        NetFail::Timeout
    } else if CERTIFICATE_ERRORS.contains(&code) {
        NetFail::Tls
    } else {
        NetFail::Connect(format!("Windows network error {code}"))
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// `https://host:port/path?query` taken apart: (secure, host, port, path).
fn split_url(url: &str, allow_http: bool) -> Option<(bool, &str, u16, &str)> {
    let (secure, rest) = match url.split_once("://")? {
        ("https", rest) => (true, rest),
        ("http", rest) if allow_http => (false, rest),
        _ => return None,
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    (!host.is_empty() && !host.contains('@')).then_some((secure, host, port, path))
}

/// Fetch `url` with Windows' networking. `proxy` is the `host:port` Cairn
/// found; without one, Windows finds the proxy itself (settings, script).
pub fn get(
    url: &str,
    accept: &str,
    limit: u64,
    timeout: Duration,
    proxy: Option<&str>,
    allow_http: bool,
) -> Result<Vec<u8>, NetFail> {
    let (secure, host, port, path) = split_url(url, allow_http)
        .ok_or_else(|| NetFail::Connect("the update address isn't usable".into()))?;
    let deadline = Instant::now() + timeout;

    let agent = wide(&format!("Cairn/{}", env!("CARGO_PKG_VERSION")));
    let proxy = proxy.map(wide);
    let (access, proxy_name) = match &proxy {
        Some(name) => (WINHTTP_ACCESS_TYPE_NAMED_PROXY, name.as_ptr()),
        None => (WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null()),
    };
    // SAFETY: `agent` and the proxy name are NUL-terminated and outlive the
    // call; the bypass list may be null.
    let session =
        Handle::new(unsafe { WinHttpOpen(agent.as_ptr(), access, proxy_name, null(), 0) })?;
    let step = timeout.min(STEP_TIMEOUT).as_millis().min(i32::MAX as u128) as i32;
    // SAFETY: `session` is a valid handle.
    if unsafe { WinHttpSetTimeouts(session.0, step, step, step, step) } == 0 {
        return Err(last_error());
    }

    let host = wide(host);
    // SAFETY: `session` is valid and `host` is NUL-terminated.
    let connection = Handle::new(unsafe { WinHttpConnect(session.0, host.as_ptr(), port, 0) })?;
    let (verb, path) = (wide("GET"), wide(path));
    let flags = if secure { WINHTTP_FLAG_SECURE } else { 0 };
    // SAFETY: `connection` is valid; the strings are NUL-terminated; the
    // null version, referrer and accept-types mean "defaults".
    let request = Handle::new(unsafe {
        WinHttpOpenRequest(
            connection.0,
            verb.as_ptr(),
            path.as_ptr(),
            null(),
            null(),
            null(),
            flags,
        )
    })?;

    let headers = wide(&format!("Accept: {accept}\r\n"));
    for round in 0.. {
        // SAFETY: `request` is valid; `headers` is NUL-terminated (length
        // -1 asks Windows to find the end), and there is no request body.
        let sent =
            unsafe { WinHttpSendRequest(request.0, headers.as_ptr(), u32::MAX, null(), 0, 0, 0) };
        // SAFETY: `request` is valid and the request was just sent.
        if sent == 0 || unsafe { WinHttpReceiveResponse(request.0, null_mut()) } == 0 {
            return Err(last_error());
        }
        match status(&request)? {
            200..=299 => break,
            407 if round < SIGN_IN_ROUNDS => sign_in(&request)?,
            other => return Err(NetFail::from_status(other as u16)),
        }
    }
    read_body(&request, limit, deadline)
}

fn status(request: &Handle) -> Result<u32, NetFail> {
    let mut code = 0u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: `request` is valid; `code` has room for the `size` bytes of
    // the number asked for.
    let ok = unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            null(),
            (&mut code as *mut u32).cast(),
            &mut size,
            null_mut(),
        )
    };
    if ok == 0 { Err(last_error()) } else { Ok(code) }
}

/// Answer a proxy's sign-in request with the person's Windows login: Windows
/// is given no name or password, so it uses the logged-in user's own.
fn sign_in(request: &Handle) -> Result<(), NetFail> {
    let (mut supported, mut first, mut target) = (0u32, 0u32, 0u32);
    // SAFETY: `request` is valid and the three outputs are valid to write.
    let ok = unsafe { WinHttpQueryAuthSchemes(request.0, &mut supported, &mut first, &mut target) };
    if ok == 0 {
        return Err(NetFail::ProxyAuth);
    }
    let scheme = if supported & WINHTTP_AUTH_SCHEME_NEGOTIATE != 0 {
        WINHTTP_AUTH_SCHEME_NEGOTIATE
    } else if supported & WINHTTP_AUTH_SCHEME_NTLM != 0 {
        WINHTTP_AUTH_SCHEME_NTLM
    } else {
        // Basic or Digest: a typed password, which Cairn never gives.
        return Err(NetFail::ProxyAuth);
    };
    // SAFETY: `request` is valid; the null name, password and parameters mean
    // "the current Windows user".
    let ok = unsafe {
        WinHttpSetCredentials(
            request.0,
            WINHTTP_AUTH_TARGET_PROXY,
            scheme,
            null(),
            null(),
            null_mut(),
        )
    };
    if ok == 0 {
        Err(NetFail::ProxyAuth)
    } else {
        Ok(())
    }
}

fn read_body(request: &Handle, limit: u64, deadline: Instant) -> Result<Vec<u8>, NetFail> {
    let mut body = Vec::new();
    let mut chunk = vec![0u8; READ_CHUNK];
    loop {
        if Instant::now() > deadline {
            return Err(NetFail::Timeout);
        }
        let mut read = 0u32;
        // SAFETY: `request` is valid and `chunk` has room for READ_CHUNK bytes.
        let ok = unsafe {
            WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                READ_CHUNK as u32,
                &mut read,
            )
        };
        if ok == 0 {
            return Err(NetFail::Incomplete("the connection was cut".into()));
        }
        if read == 0 {
            return Ok(body);
        }
        if body.len() as u64 + u64::from(read) > limit {
            return Err(NetFail::Incomplete("it is bigger than expected".into()));
        }
        body.extend_from_slice(&chunk[..read as usize]);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::{SocketAddr, TcpListener};

    use super::*;

    const WAIT: Duration = Duration::from_secs(20);

    #[test]
    fn addresses_are_taken_apart() {
        assert_eq!(
            split_url("https://api.github.com/repos/x?y=1", false),
            Some((true, "api.github.com", 443, "/repos/x?y=1"))
        );
        assert_eq!(
            split_url("http://127.0.0.1:8080/a", true),
            Some((false, "127.0.0.1", 8080, "/a"))
        );
        assert_eq!(split_url("http://127.0.0.1:8080/a", false), None);
        assert_eq!(split_url("https://user:pw@host/", false), None);
        assert_eq!(split_url("ftp://host/", true), None);
    }

    /// A server that answers every request on a connection with `reply`.
    fn serve(reply: &'static str) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for client in listener.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(client.try_clone().unwrap());
                    let mut out = client;
                    let mut line = String::new();
                    loop {
                        loop {
                            line.clear();
                            match reader.read_line(&mut line) {
                                Ok(0) | Err(_) => return,
                                Ok(_) if line == "\r\n" => break,
                                Ok(_) => {}
                            }
                        }
                        if out.write_all(reply.as_bytes()).is_err() {
                            return;
                        }
                    }
                });
            }
        });
        addr
    }

    #[test]
    fn a_page_is_fetched_and_its_size_is_limited() {
        let addr = serve("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello");
        let url = format!("http://{addr}/x");
        let body = get(&url, "*/*", 100, WAIT, None, true).unwrap();
        assert_eq!(body, b"hello");
        let too_big = get(&url, "*/*", 3, WAIT, None, true).unwrap_err();
        assert!(matches!(too_big, NetFail::Incomplete(_)), "{too_big:?}");
    }

    #[test]
    fn answers_are_told_apart() {
        let addr = serve("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
        let err = get(&format!("http://{addr}/x"), "*/*", 100, WAIT, None, true).unwrap_err();
        assert_eq!(err, NetFail::NotFound);
    }

    #[test]
    fn a_proxy_that_never_accepts_the_windows_login_is_reported() {
        let proxy = serve(
            "HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: NTLM\r\n\
             Content-Length: 0\r\n\r\n",
        );
        let err = get(
            "http://updates.cairn.invalid/x",
            "*/*",
            100,
            WAIT,
            Some(&proxy.to_string()),
            true,
        )
        .unwrap_err();
        assert_eq!(err, NetFail::ProxyAuth);
    }

    #[test]
    fn a_proxy_that_wants_a_typed_password_is_not_answered() {
        let proxy = serve(
            "HTTP/1.1 407 Proxy Authentication Required\r\n\
             Proxy-Authenticate: Basic realm=\"office\"\r\nContent-Length: 0\r\n\r\n",
        );
        let err = get(
            "http://updates.cairn.invalid/x",
            "*/*",
            100,
            WAIT,
            Some(&proxy.to_string()),
            true,
        )
        .unwrap_err();
        assert_eq!(err, NetFail::ProxyAuth);
    }
}
