//! Cairn command-line entry point.
//!
//! ```text
//! cairn                                   start and open the browser
//! cairn --no-browser [--port N]           start without opening a browser
//! cairn --workspace <folder>              open this documentation folder
//! cairn init <folder> --name "<name>"     create a documentation folder
//! cairn locks list <workspace>            show edit locks (maintainers)
//! cairn locks release <workspace> <page> --session <id>
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use cairn::config;
use cairn::locks;
use cairn::paths::Root;
use cairn::server::{self, AppState};
use cairn::workspace::{self, Discovery, InitTarget};

const HELP: &str = "Cairn: shared Markdown documentation in a folder you control.

USAGE:
    cairn [--no-browser] [--port <n>] [--workspace <folder>]
    cairn init <folder> --name <name>
    cairn locks list <workspace-folder>
    cairn locks release <workspace-folder> <page-path> --session <session-id>

OPTIONS:
    --no-browser        Don't open the web browser automatically
    --port <n>          Use this local port instead of a random free one
    --workspace <dir>   Open this documentation folder (or the one containing it)
    -h, --help          Show this help
    -V, --version       Show the version
";

struct RunArgs {
    open_browser: bool,
    port: u16,
    workspace: Option<PathBuf>,
    /// Started by the previous version after an update: take over its port
    /// and keys (see `server::api_update`).
    handoff: bool,
}

/// How long a restarted Cairn waits for the old one to free its port.
const HANDOFF_BIND_WAIT: Duration = Duration::from_secs(20);

fn value_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("-h" | "--help" | "help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("-V" | "--version") => {
            println!("cairn {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("init") => cmd_init(&args[1..]),
        Some("locks") => cmd_locks(&args[1..]),
        _ => cmd_run(RunArgs {
            open_browser: !args.iter().any(|a| a == "--no-browser"),
            port: value_after(&args, "--port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(0),
            workspace: value_after(&args, "--workspace").map(PathBuf::from),
            handoff: args.iter().any(|a| a == "--handoff"),
        }),
    }
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("Error: {msg}");
    ExitCode::FAILURE
}

fn open_initial_workspace(state: &AppState, path: &Path) {
    match workspace::discover(path) {
        Ok(Discovery::Found { root, marker, .. }) => {
            match server::api::open_at(state, Path::new(&root)) {
                Ok(_) => println!("Opened \"{}\" at {root}", marker.display_name),
                Err(e) => eprintln!("Could not open {root}: {e}"),
            }
        }
        Ok(_) => eprintln!(
            "{} is not a documentation folder. Choose one in the browser.",
            path.display()
        ),
        Err(e) => eprintln!(
            "The last documentation folder is not available ({e}). Choose one in the browser."
        ),
    }
}

fn print_banner(url: &str) {
    println!();
    println!("  Cairn is running.");
    println!("  Your documentation opens in your web browser.");
    println!("  If it doesn't, copy this address into your browser:");
    println!();
    println!("    {url}");
    println!();
    println!("  Keep this window open while you use Cairn.");
    println!("  To stop Cairn, close this window or press Ctrl+C.");
    println!();
}

fn cmd_run(args: RunArgs) -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();
    let dir = config::config_dir();
    let (cfg, warning) = config::load(&dir);
    if let Some(w) = &warning {
        eprintln!("Note: {w}");
    }
    let last = cfg.last_workspace.clone();
    let handoff = if args.handoff {
        server::api_update::take_handoff(&dir)
    } else {
        None
    };
    // Without a valid hand-over the open tab can't reconnect, so open a new one.
    let open_browser = args.open_browser || (args.handoff && handoff.is_none());
    let port = handoff.as_ref().map_or(args.port, |h| h.port);
    let secrets = handoff
        .as_ref()
        .map(|h| (h.token.clone(), h.read_key.clone()));
    let state = AppState::with_secrets(dir, cfg, warning, secrets);

    // Open the requested workspace, or the one used last time.
    if let Some(path) = args.workspace.clone().or(last) {
        open_initial_workspace(&state, &path);
    }

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => return fail(format!("could not start: {e}")),
    };
    runtime.block_on(async move {
        // After an update the old program is still letting go of the port.
        let deadline = Instant::now() + HANDOFF_BIND_WAIT;
        let listener = loop {
            match server::bind_loopback(port).await {
                Ok(l) => break l,
                Err(_) if args.handoff && Instant::now() < deadline => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                Err(e) => return fail(format!("could not listen on 127.0.0.1:{port}: {e}")),
            }
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(port);
        let url = format!("http://127.0.0.1:{port}/#t={}", state.token);
        if args.handoff {
            println!("Cairn {} has started.", env!("CARGO_PKG_VERSION"));
        }
        print_banner(&url);
        if open_browser && let Err(e) = open::that_detached(&url) {
            eprintln!("  (Could not open the browser automatically: {e})");
        }
        match server::run(state, listener).await {
            Ok(()) => {
                println!("Cairn has stopped.");
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        }
    })
}

fn cmd_init(args: &[String]) -> ExitCode {
    let Some(folder) = args.first().filter(|a| !a.starts_with("--")) else {
        return fail("usage: cairn init <folder> --name <name>");
    };
    let name = value_after(args, "--name").unwrap_or_else(|| "My Documentation".into());
    let path = PathBuf::from(folder);
    let target = if path.exists() {
        InitTarget::Here(path)
    } else {
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
        match (parent, path.file_name()) {
            (Some(parent), Some(child)) => InitTarget::NewChild {
                parent: parent.to_path_buf(),
                name: child.to_string_lossy().into_owned(),
            },
            _ => return fail("please give a full folder path"),
        }
    };
    match workspace::initialize(&target, &name) {
        Ok(out) if out.created => {
            println!(
                "Created \"{}\" at {} (id {})",
                out.marker.display_name, out.root, out.marker.instance_id
            );
            ExitCode::SUCCESS
        }
        Ok(out) => {
            println!(
                "{} is already a documentation folder: \"{}\"",
                out.root, out.marker.display_name
            );
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn open_root(folder: &str) -> Result<Root, String> {
    match workspace::discover(Path::new(folder)).map_err(|e| e.to_string())? {
        Discovery::Found { root, .. } => Root::new(Path::new(&root)).map_err(|e| e.to_string()),
        Discovery::Invalid { reason, .. } => Err(reason),
        Discovery::NotFound { .. } => Err(format!("{folder} is not inside a documentation folder")),
    }
}

fn cmd_locks_list(folder: &str) -> ExitCode {
    let root = match open_root(folder) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    let all = match locks::list_locks(&root) {
        Ok(l) => l,
        Err(e) => return fail(e),
    };
    if all.is_empty() {
        println!("No pages are locked for editing.");
        return ExitCode::SUCCESS;
    }
    let now = locks::now_secs();
    for l in all {
        let age = now.saturating_sub(l.heartbeat_at);
        let state = if age >= locks::STALE_AFTER_SECS {
            "POSSIBLY ABANDONED"
        } else {
            "active"
        };
        println!("{}", l.article);
        println!(
            "    editor:    {} ({}@{})",
            l.display_name, l.os_user, l.host
        );
        println!("    session:   {}", l.session_id);
        println!("    last seen: {age} seconds ago [{state}]");
    }
    ExitCode::SUCCESS
}

fn cmd_locks_release(args: &[String]) -> ExitCode {
    let (Some(folder), Some(page)) = (args.get(1), args.get(2)) else {
        return fail("usage: cairn locks release <workspace> <page-path> --session <id>");
    };
    let Some(session) = value_after(args, "--session") else {
        return fail("--session <id> is required. Run `cairn locks list` to see it.");
    };
    let root = match open_root(folder) {
        Ok(r) => r,
        Err(e) => return fail(e),
    };
    match locks::maintainer_release(&root, &page.replace('\\', "/"), &session) {
        Ok(info) => {
            println!(
                "Released the lock on {} held by {}.",
                info.article, info.display_name
            );
            println!("A record was kept in _system/locks/released/.");
            println!(
                "If {} had unpublished changes, they are still saved on their own computer",
                info.display_name
            );
            println!("and will be offered to them the next time they edit this page.");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}

fn cmd_locks(args: &[String]) -> ExitCode {
    match (args.first().map(String::as_str), args.get(1)) {
        (Some("list"), Some(folder)) => cmd_locks_list(folder),
        (Some("release"), Some(_)) => cmd_locks_release(args),
        _ => fail(
            "usage: cairn locks list <workspace> | \
             cairn locks release <workspace> <page> --session <id>",
        ),
    }
}
