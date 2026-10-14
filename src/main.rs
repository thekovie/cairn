//! Cairn command-line entry point.
//!
//! ```text
//! cairn                                   start and open the browser
//! cairn --no-browser [--port N]           start without opening a browser
//! cairn --workspace <folder>              open this documentation folder
//! cairn init <folder> --name "<name>"     create a documentation folder
//! cairn locks list <workspace>            show edit locks (maintainers)
//! cairn locks release <workspace> <page> --session <id>
//! cairn backup create <workspace> <backup.zip>
//! cairn backup verify <backup.zip>
//! cairn restore <backup.zip> <new-folder>
//! ```

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use cairn::backup;
use cairn::config;
use cairn::error::{CairnError, Result as CairnResult};
use cairn::export::archive;
use cairn::fsutil;
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
    cairn backup create <workspace-folder> <backup.zip>
    cairn backup verify <backup.zip>
    cairn restore <backup.zip> <new-folder>

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
    /// Started with no options at all, as a shortcut does (used on Windows).
    #[cfg_attr(not(windows), allow(dead_code))]
    plain: bool,
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
        Some("backup") => cmd_backup(&args[1..]),
        Some("restore") => cmd_restore(&args[1..]),
        _ => cmd_run(RunArgs {
            open_browser: !args.iter().any(|a| a == "--no-browser"),
            port: value_after(&args, "--port")
                .and_then(|p| p.parse().ok())
                .unwrap_or(0),
            workspace: value_after(&args, "--workspace").map(PathBuf::from),
            handoff: args.iter().any(|a| a == "--handoff"),
            plain: args.is_empty(),
        }),
    }
}

/// Text from a file we didn't write (a backup's names, say) must not be able
/// to send terminal commands: control characters become "?".
fn plain(text: impl std::fmt::Display) -> String {
    text.to_string()
        .chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    let text = plain(msg);
    eprintln!("Error: {text}");
    // With no console window, nobody would see it.
    #[cfg(windows)]
    if cairn::tray::windowless() {
        cairn::tray::show_error(&text);
    }
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
    // On Windows, Cairn runs from its tray icon rather than a console window.
    // A second shortcut click asks the running copy to open the browser.
    #[cfg(windows)]
    if args.plain && cairn::tray::hand_over_to_running_copy() {
        return ExitCode::SUCCESS;
    }
    #[cfg(windows)]
    let windowless = cairn::tray::detach_console_if_ours();
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
    let after_stop = state.clone();

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
    let code = runtime.block_on(async move {
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
        // The icon stays until the server has stopped.
        #[cfg(windows)]
        let _tray = start_tray(windowless, &state, &url);
        if open_browser && let Err(e) = open::that_detached(&url) {
            eprintln!("  (Could not open the browser automatically: {e})");
        }
        match server::run(state, listener).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(e),
        }
    });
    let restart_into = after_stop.restart_into.lock().expect("restart lock").take();
    match restart_into {
        Some(exe) if code == ExitCode::SUCCESS => become_new_version(&exe, after_stop.port()),
        _ => {
            // The window may already be closed, so a failed print is fine.
            let _ = writeln!(std::io::stdout(), "Cairn has stopped.");
            code
        }
    }
}

/// The icon that stands in for the console window on Windows. If it can't be
/// made, the console comes back, so there is still a way to quit.
#[cfg(windows)]
fn start_tray(
    windowless: bool,
    state: &std::sync::Arc<AppState>,
    url: &str,
) -> Option<cairn::tray::Tray> {
    if !windowless {
        return None;
    }
    match cairn::tray::start(url.to_string(), state.clone()) {
        Ok(tray) => Some(tray),
        Err(why) => {
            cairn::tray::show_console_again();
            eprintln!("Note: Cairn's icon could not be added ({why}).");
            print_banner(url);
            None
        }
    }
}

/// After an update on a Mac or Linux: turn this process into the new
/// program, in the same Terminal window, handing over the same port.
#[cfg(unix)]
fn become_new_version(exe: &Path, port: u16) -> ExitCode {
    use std::os::unix::process::CommandExt;
    println!("Starting the new version of Cairn…");
    let err = std::process::Command::new(exe)
        .args(["--no-browser", "--handoff", "--port", &port.to_string()])
        .exec();
    fail(format!(
        "the new version couldn't be started ({err}). Start Cairn again."
    ))
}

#[cfg(not(unix))]
fn become_new_version(_: &Path, _: u16) -> ExitCode {
    ExitCode::SUCCESS
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

/// A counter on the terminal ("Copying file 40 of 120"), redrawn a few times a
/// second, and only when a person is watching: nothing is printed into a log
/// or a pipe.
fn progress_printer(label: &'static str) -> impl Fn(usize, usize) {
    let watched = std::io::stderr().is_terminal();
    let last: std::cell::Cell<Option<Instant>> = std::cell::Cell::new(None);
    move |done, total| {
        if !watched {
            return;
        }
        let now = Instant::now();
        let recent = last
            .get()
            .is_some_and(|t| now.duration_since(t) < Duration::from_millis(100));
        if recent && done < total {
            return;
        }
        last.set(Some(now));
        eprint!("\r{label} {done} of {total}\x1b[K");
    }
}

/// Wipe the counter line once the work is over.
fn end_progress() {
    if std::io::stderr().is_terminal() {
        eprint!("\r\x1b[K");
    }
}

fn create_shown(root: &Root, file: &str) -> CairnResult<backup::Summary> {
    let first = archive::enumerate_backup(root)?;
    let total = first.entries.len();
    let show = progress_printer("Copying file");
    let never = || false;
    let on_file = |done: usize| show(done, total);
    let progress = backup::Progress {
        cancelled: &never,
        on_file: &on_file,
    };
    let result = backup::create(root, Path::new(file), first, &progress);
    end_progress();
    // Nothing here can stop it part way, so "stopped" never happens.
    result?.ok_or_else(|| CairnError::Io("The backup was cancelled.".into()))
}

fn verify_shown(file: &str) -> CairnResult<backup::Manifest> {
    let show = progress_printer("Checking file");
    let checked = backup::verify_with(Path::new(file), &show);
    end_progress();
    checked
}

fn restore_shown(file: &str, folder: &str) -> CairnResult<backup::Restored> {
    let show = progress_printer("Restoring file");
    let restored = backup::restore_with(
        Path::new(file),
        Path::new(folder),
        &fsutil::available_space,
        &show,
    );
    end_progress();
    restored
}

fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0)
}

fn cmd_backup(args: &[String]) -> ExitCode {
    match (args.first().map(String::as_str), args.get(1), args.get(2)) {
        (Some("create"), Some(folder), Some(file)) => {
            let root = match open_root(folder) {
                Ok(r) => r,
                Err(e) => return fail(e),
            };
            match create_shown(&root, file) {
                Ok(done) => {
                    println!(
                        "Backed up {} files ({}) to {file}.",
                        done.files,
                        megabytes(done.bytes)
                    );
                    if done.passes > 1 {
                        println!("A file changed while it was being copied, so it was made again.");
                    }
                    if !done.changed_during.is_empty() {
                        println!(
                            "{} file(s) were saved, added, or removed while it ran, so it holds each as it was when copied:",
                            done.changed_during.len()
                        );
                        for path in done.changed_during.iter().take(10) {
                            println!("    {}", plain(path));
                        }
                    }
                    if let Some(note) = &done.space_note {
                        println!("Note: {}", plain(note));
                    }
                    if !done.skipped_links.is_empty() {
                        println!(
                            "Left out {} link(s) that point out of the documentation.",
                            done.skipped_links.len()
                        );
                    }
                    println!("Check it any time with: cairn backup verify {file}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(e),
            }
        }
        (Some("verify"), Some(file), None) => match verify_shown(file) {
            Ok(m) => {
                println!(
                    "OK: \"{}\" backup made {} by Cairn {}: {} files, all checksums match.",
                    plain(&m.display_name),
                    plain(&m.created_at),
                    plain(&m.cairn_version),
                    m.files.len()
                );
                ExitCode::SUCCESS
            }
            Err(e) => fail(e),
        },
        _ => fail(
            "usage: cairn backup create <workspace> <backup.zip> | cairn backup verify <backup.zip>",
        ),
    }
}

fn cmd_restore(args: &[String]) -> ExitCode {
    let (Some(file), Some(folder)) = (args.first(), args.get(1)) else {
        return fail("usage: cairn restore <backup.zip> <new-folder>");
    };
    match restore_shown(file, folder) {
        Ok(done) => {
            println!(
                "Restored \"{}\" ({} files, {}) to {folder}.",
                plain(&done.display_name),
                done.files,
                megabytes(done.bytes)
            );
            println!(
                "It keeps the identity of the original (id {}).",
                done.instance_id
            );
            println!("Use it to replace a lost folder, not alongside the original.");
            println!("Open the folder from Cairn's start screen to use it.");
            ExitCode::SUCCESS
        }
        Err(e) => fail(e),
    }
}
