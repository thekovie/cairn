//! How update requests reach GitHub: which proxy to use, what went wrong
//! when a request fails, and what to tell the person.
//!
//! The proxy is chosen in this order:
//!
//! 1. the proxy typed in Settings (an explicit choice, so `NO_PROXY` doesn't
//!    apply to it);
//! 2. the `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY` environment variables
//!    (and their lowercase forms), with `NO_PROXY` respected;
//! 3. Windows' own proxy settings, with their bypass list;
//! 4. no proxy.
//!
//! Cairn never asks for, stores, or logs a proxy password. A proxy that wants
//! a Windows sign-in (NTLM or Kerberos) is handled on Windows by
//! `crate::winhttp`, which signs in as the person using Windows itself.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use crate::error::{CairnError, Result};

/// Where the proxy to use came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxySource {
    /// `host:port` typed in Settings.
    Manual(String),
    /// A proxy environment variable.
    Environment,
    /// Windows' proxy settings.
    WindowsSystem,
    /// No proxy.
    Direct,
}

/// The proxy to use for a request, and where it came from.
pub struct Route {
    pub source: ProxySource,
    pub proxy: Option<ureq::Proxy>,
}

const PROXY_VARIABLES: [&str; 6] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
];

/// Choose the proxy. `env` reads an environment variable and `system` finds
/// the proxy from the environment and, on Windows, from Windows' settings;
/// both are passed in so tests don't depend on the computer they run on.
pub fn resolve(
    manual: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
    system: impl FnOnce() -> Option<ureq::Proxy>,
) -> Result<Route> {
    if let Some(host_port) = manual {
        // A proxy chosen in Settings that can't be used is an error, never
        // a quiet switch to a direct connection.
        let proxy = ureq::Proxy::new(&format!("http://{host_port}")).map_err(|_| {
            CairnError::BadRequest(
                "The proxy in Settings isn't valid. Type it as address:port.".into(),
            )
        })?;
        return Ok(Route {
            source: ProxySource::Manual(host_port.to_string()),
            proxy: Some(proxy),
        });
    }
    let proxy = system();
    let env_is_set = PROXY_VARIABLES
        .iter()
        .any(|name| env(name).is_some_and(|v| !v.trim().is_empty()));
    let source = match (env_is_set, &proxy) {
        (true, _) => ProxySource::Environment,
        (false, Some(_)) => ProxySource::WindowsSystem,
        (false, None) => ProxySource::Direct,
    };
    Ok(Route { source, proxy })
}

impl Route {
    /// `host:port` of the proxy, never any login in its address.
    pub fn host_port(&self) -> Option<String> {
        self.proxy
            .as_ref()
            .map(|p| format!("{}:{}", p.host(), p.port()))
    }

    pub fn info(&self, windows_login: bool) -> NetworkInfo {
        NetworkInfo {
            via: match self.source {
                ProxySource::Manual(_) => "manual",
                ProxySource::Environment => "environment",
                ProxySource::WindowsSystem => "windows",
                ProxySource::Direct => "direct",
            },
            proxy: self.host_port(),
            windows_login,
        }
    }
}

/// How the last update request was sent, for Settings → Updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkInfo {
    /// `manual`, `environment`, `windows` or `direct`.
    pub via: &'static str,
    pub proxy: Option<String>,
    /// The proxy accepted the person's Windows sign-in.
    pub windows_login: bool,
}

static LAST_NETWORK: Mutex<Option<NetworkInfo>> = Mutex::new(None);

pub fn record(info: NetworkInfo) {
    if let Ok(mut last) = LAST_NETWORK.lock() {
        *last = Some(info);
    }
}

pub fn last_network() -> Option<NetworkInfo> {
    LAST_NETWORK.lock().ok().and_then(|last| last.clone())
}

static PREFER_WINHTTP: AtomicBool = AtomicBool::new(false);

/// Once Windows' own networking got through a proxy that ureq couldn't, the
/// rest of the downloads go straight to it instead of failing first.
pub fn prefer_winhttp() -> bool {
    cfg!(windows) && PREFER_WINHTTP.load(Ordering::Relaxed)
}

pub fn set_prefer_winhttp() {
    PREFER_WINHTTP.store(true, Ordering::Relaxed);
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetFail {
    /// The proxy wants a sign-in (HTTP 407).
    ProxyAuth,
    /// GitHub is limiting this network (HTTP 403 or 429).
    RateLimited,
    NotFound,
    Status(u16),
    Timeout,
    /// The certificate couldn't be verified.
    Tls,
    /// The reply stopped part way or was too big.
    Incomplete(String),
    /// Couldn't connect (offline, a wrong or dead proxy, no DNS).
    Connect(String),
}

impl NetFail {
    /// An HTTP answer that isn't a success.
    pub fn from_status(code: u16) -> NetFail {
        match code {
            407 => NetFail::ProxyAuth,
            403 | 429 => NetFail::RateLimited,
            404 => NetFail::NotFound,
            other => NetFail::Status(other),
        }
    }

    pub fn from_ureq(e: &ureq::Error) -> NetFail {
        use ureq::Error;
        match e {
            Error::StatusCode(code) => NetFail::from_status(*code),
            // For https, ureq reports a proxy's refusal to tunnel this way.
            Error::ConnectProxyFailed(reason) if reason.contains("407") => NetFail::ProxyAuth,
            Error::Timeout(_) => NetFail::Timeout,
            Error::Tls(_) | Error::Rustls(_) => NetFail::Tls,
            other => NetFail::Connect(redact(&other.to_string())),
        }
    }
}

/// Whether Windows' own networking is worth trying after `fail`: the proxy
/// asked for a sign-in, or nothing could connect where a proxy script or
/// Windows' own settings might know a way. Never after a proxy chosen by the
/// person failed to connect, and never to skip a proxy.
pub fn wants_winhttp(fail: &NetFail, source: &ProxySource) -> bool {
    match fail {
        NetFail::ProxyAuth => true,
        NetFail::Connect(_) | NetFail::Timeout => {
            matches!(source, ProxySource::Direct | ProxySource::WindowsSystem)
        }
        _ => false,
    }
}

/// What to tell the person. `tried_windows_login` says whether Windows'
/// sign-in was offered to the proxy.
pub fn to_cairn_error(
    fail: NetFail,
    source: &ProxySource,
    tried_windows_login: bool,
) -> CairnError {
    let io = CairnError::Io;
    match fail {
        NetFail::ProxyAuth if tried_windows_login => io(
            "Your network's proxy asked for a sign-in, and it didn't accept your Windows login. \
             Ask your IT team to let Cairn reach github.com through the proxy."
                .into(),
        ),
        NetFail::ProxyAuth => io(
            "Your network's proxy asked for a sign-in. Cairn can't give one, and never asks for \
             a password. Ask your IT team to let Cairn reach github.com through the proxy."
                .into(),
        ),
        NetFail::RateLimited => io(
            "GitHub is limiting requests from this network right now. Try again in an hour or so."
                .into(),
        ),
        NetFail::NotFound => CairnError::NotFound("No released version of Cairn was found.".into()),
        NetFail::Status(code) => io(format!(
            "GitHub didn't answer normally (error {code}). Try again later."
        )),
        NetFail::Timeout => {
            io("Checking took too long. The internet connection may be slow or blocked.".into())
        }
        NetFail::Tls => io(
            "Cairn couldn't verify GitHub's security certificate. A network filter may be \
             replacing it. Ask your IT team to add its certificate to Windows. Cairn never \
             skips this check."
                .into(),
        ),
        NetFail::Incomplete(why) => io(format!("The download didn't finish ({why}). Try again.")),
        NetFail::Connect(why) => match source {
            ProxySource::Manual(proxy) => io(format!(
                "Cairn couldn't reach GitHub through the proxy {proxy} ({why}). Check the proxy \
                 address in Settings → Updates."
            )),
            _ => io(format!(
                "Cairn couldn't reach GitHub ({why}). Check the internet connection."
            )),
        },
    }
}

/// `text` with any login in an address (`scheme://user:pass@host`) hidden.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("://") {
        let (head, tail) = rest.split_at(i + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c == '/' || c.is_whitespace())
            .unwrap_or(tail.len());
        match tail[..end].rfind('@') {
            Some(at) => {
                out.push_str("***@");
                rest = &tail[at + 1..];
            }
            None => rest = tail,
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(host_port: &str) -> Option<ureq::Proxy> {
        Some(ureq::Proxy::new(&format!("http://{host_port}")).unwrap())
    }

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn a_proxy_from_settings_beats_everything_else() {
        let env = |name: &str| (name == "HTTPS_PROXY").then(|| "http://env:1".to_string());
        let route = resolve(Some("settings.local:3128"), env, || proxy("windows:2")).unwrap();
        assert_eq!(
            route.source,
            ProxySource::Manual("settings.local:3128".into())
        );
        assert_eq!(route.host_port().as_deref(), Some("settings.local:3128"));
    }

    #[test]
    fn a_broken_proxy_in_settings_is_an_error_not_a_direct_connection() {
        assert!(resolve(Some("not a proxy"), no_env, || None).is_err());
    }

    #[test]
    fn environment_variables_beat_windows_settings() {
        for name in PROXY_VARIABLES {
            let env = move |n: &str| (n == name).then(|| "http://env:1".to_string());
            let route = resolve(None, env, || proxy("env:1")).unwrap();
            assert_eq!(route.source, ProxySource::Environment, "{name}");
        }
    }

    #[test]
    fn an_empty_variable_counts_as_unset() {
        let env = |n: &str| (n == "HTTPS_PROXY").then(|| "  ".to_string());
        let route = resolve(None, env, || proxy("windows:2")).unwrap();
        assert_eq!(route.source, ProxySource::WindowsSystem);
    }

    #[test]
    fn without_any_proxy_the_connection_is_direct() {
        let route = resolve(None, no_env, || None).unwrap();
        assert_eq!(route.source, ProxySource::Direct);
        assert_eq!(route.info(false).via, "direct");
        assert_eq!(route.info(false).proxy, None);
    }

    #[test]
    fn the_shown_proxy_never_carries_a_login() {
        let with_login = Some(ureq::Proxy::new("http://alice:secret@proxy.corp:8080").unwrap());
        let route = resolve(None, no_env, || with_login).unwrap();
        let shown = serde_json::to_string(&route.info(true)).unwrap();
        assert!(shown.contains("proxy.corp:8080"), "{shown}");
        assert!(
            !shown.contains("alice") && !shown.contains("secret"),
            "{shown}"
        );
    }

    #[test]
    fn failures_are_told_apart() {
        use ureq::Error::{ConnectProxyFailed, StatusCode};
        let proxy_407 = ConnectProxyFailed("proxy server responded 407/407".into());
        let cases = [
            (StatusCode(407), NetFail::ProxyAuth),
            (proxy_407, NetFail::ProxyAuth),
            (StatusCode(403), NetFail::RateLimited),
            (StatusCode(429), NetFail::RateLimited),
            (StatusCode(404), NetFail::NotFound),
            (StatusCode(503), NetFail::Status(503)),
        ];
        for (error, expected) in cases {
            assert_eq!(NetFail::from_ureq(&error), expected, "{error}");
        }
        let refused = ConnectProxyFailed("proxy server responded 502/502".into());
        assert!(matches!(NetFail::from_ureq(&refused), NetFail::Connect(_)));
        let timeout = ureq::Error::Timeout(ureq::Timeout::Global);
        assert_eq!(NetFail::from_ureq(&timeout), NetFail::Timeout);
        assert_eq!(NetFail::from_ureq(&ureq::Error::Tls("bad")), NetFail::Tls);
    }

    #[test]
    fn windows_networking_is_tried_for_sign_in_and_script_proxies_only() {
        let manual = ProxySource::Manual("p:1".into());
        let down = || NetFail::Connect("x".into());
        assert!(wants_winhttp(&NetFail::ProxyAuth, &manual));
        assert!(wants_winhttp(
            &NetFail::ProxyAuth,
            &ProxySource::Environment
        ));
        assert!(wants_winhttp(&NetFail::Timeout, &ProxySource::Direct));
        assert!(wants_winhttp(&down(), &ProxySource::WindowsSystem));
        // A proxy the person chose that is down is reported, not worked around.
        assert!(!wants_winhttp(&down(), &manual));
        assert!(!wants_winhttp(&down(), &ProxySource::Environment));
        assert!(!wants_winhttp(&NetFail::RateLimited, &ProxySource::Direct));
        assert!(!wants_winhttp(&NetFail::Tls, &ProxySource::Direct));
    }

    #[test]
    fn each_failure_has_its_own_message() {
        let direct = ProxySource::Direct;
        let said = |fail, tried| to_cairn_error(fail, &direct, tried).to_string();
        assert!(said(NetFail::ProxyAuth, false).contains("never asks for a password"));
        assert!(said(NetFail::ProxyAuth, true).contains("Windows login"));
        assert!(said(NetFail::RateLimited, false).contains("limiting"));
        assert!(said(NetFail::Tls, false).contains("certificate"));
        assert!(said(NetFail::Timeout, false).contains("too long"));
        let manual = ProxySource::Manual("proxy.corp:8080".into());
        let err = to_cairn_error(NetFail::Connect("refused".into()), &manual, false);
        assert!(err.to_string().contains("proxy.corp:8080"));
    }

    #[test]
    fn logins_in_addresses_are_hidden() {
        assert_eq!(
            redact("failed via http://alice:secret@proxy.corp:8080/x and https://h/"),
            "failed via http://***@proxy.corp:8080/x and https://h/"
        );
        assert_eq!(redact("no address here"), "no address here");
        assert_eq!(redact("a@b.com stays"), "a@b.com stays");
    }
}
