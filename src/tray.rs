//! Windows only: Cairn without a window of its own.
//!
//! Started from the Start menu or a shortcut, Cairn used to keep a black
//! console window open, and closing it was how you stopped Cairn. Now that
//! window is let go, and a small icon in the notification area (the tray)
//! takes its place: click it to open Cairn in the browser, or right-click it
//! for **Open Cairn** and **Quit Cairn**.
//!
//! The icon belongs to a hidden window on its own thread. That window also
//! hears what the console used to: Windows asking programs to close (the
//! installer does, to update Cairn) and signing out or shutting down. Each
//! releases the edit locks, as closing the console window did.
//!
//! Started from a terminal, none of this applies: that terminal stays the
//! window, as before.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock, mpsc};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::Console::{
    AllocConsole, FreeConsole, GetConsoleProcessList, STD_ERROR_HANDLE, STD_INPUT_HANDLE,
    STD_OUTPUT_HANDLE, SetStdHandle,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_SETVERSION, NIN_SELECT,
    NOTIFY_ICON_MESSAGE, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, ENDSESSION_CLOSEAPP, FindWindowW, GetMessageW, IDI_APPLICATION, LoadIconW,
    MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MF_STRING, MSG, MessageBoxW, PostMessageW,
    PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SMTO_ABORTIFHUNG, SendMessageTimeoutW,
    SetForegroundWindow, SetMenuDefaultItem, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    TrackPopupMenu, TranslateMessage, WM_APP, WM_CLOSE, WM_CONTEXTMENU, WM_DESTROY, WM_ENDSESSION,
    WM_NULL, WNDCLASSW, WS_OVERLAPPED,
};

use crate::server::AppState;

const CLASS_NAME: &str = "CairnTrayWindow";
const TOOLTIP: &str = "Cairn is running";
/// What the icon's callback messages arrive as.
const WM_TRAY: u32 = WM_APP + 1;
/// Sent by a second copy of Cairn that was started while this one runs.
const WM_OPEN: u32 = WM_APP + 2;
const MENU_OPEN: usize = 1;
const MENU_QUIT: usize = 2;
/// The id of the program's icon in `cairn.exe` (see `build.rs`).
const EXE_ICON_ID: usize = 1;
/// What a keyboard press on the icon (Enter) arrives as: a select with the
/// key flag set.
const NIN_KEYSELECT: u32 = NIN_SELECT | 1;
/// Explorer may not be ready when Cairn starts at sign-in, so adding the icon
/// is tried for a while before giving up on it.
const ADD_TRIES: u32 = 20;
const ADD_PAUSE: Duration = Duration::from_millis(500);
/// Longer than all the tries, so the wait only runs out if something is stuck.
const READY_WAIT: Duration = Duration::from_secs(15);
/// How long stopping waits for the icon's thread before leaving it behind.
const STOP_WAIT: Duration = Duration::from_secs(3);
/// How long a second copy waits for the running one to answer.
const HANDOVER_WAIT_MS: u32 = 2000;

/// True once Cairn has let go of its console window.
static WINDOWLESS: AtomicBool = AtomicBool::new(false);
/// Set when Cairn itself is closing the hidden window, so that the closing
/// is not mistaken for Windows asking Cairn to quit.
static CLOSING: AtomicBool = AtomicBool::new(false);
/// Cairn is stopping (Quit, or Windows asking it to close), so a second copy
/// started now must not hand over to it.
static QUITTING: AtomicBool = AtomicBool::new(false);
/// The hidden window, as a number so it can be shared between threads.
static WINDOW: AtomicUsize = AtomicUsize::new(0);
/// Windows' message for "the taskbar was rebuilt, add your icon again".
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct Context {
    url: String,
    state: Arc<AppState>,
}

static CONTEXT: OnceLock<Context> = OnceLock::new();

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Whether Cairn has no window, so errors must be shown some other way.
pub fn windowless() -> bool {
    WINDOWLESS.load(Ordering::SeqCst)
}

/// Let go of the console window if it was opened just for Cairn (a shortcut
/// or the Start menu). Returns whether Cairn is now without a window. From a
/// terminal it keeps using that terminal.
///
/// With no console at all, Cairn was started by a script or another program,
/// which may be reading its output, so it stays as it is. The exception is
/// `handoff`: an update starts the new program with no console on purpose.
pub fn detach_console_if_ours(handoff: bool) -> bool {
    let mut attached = [0u32; 2];
    // SAFETY: the buffer holds the two entries it is told about.
    let count = unsafe { GetConsoleProcessList(attached.as_mut_ptr(), 2) };
    let ours = match count {
        0 => handoff,
        // Only Cairn is using this console, so it was made for Cairn.
        1 => {
            // SAFETY: plain calls. The handles that pointed at the console are
            // cleared too, so nothing printed later can land on an unrelated
            // handle that Windows reuses for a file or a connection.
            unsafe {
                FreeConsole();
                SetStdHandle(STD_INPUT_HANDLE, std::ptr::null_mut());
                SetStdHandle(STD_OUTPUT_HANDLE, std::ptr::null_mut());
                SetStdHandle(STD_ERROR_HANDLE, std::ptr::null_mut());
            }
            true
        }
        _ => false,
    };
    WINDOWLESS.store(ours, Ordering::SeqCst);
    ours
}

/// Give Cairn a console window again, for when the icon could not be made
/// and there would otherwise be no way to quit.
pub fn show_console_again() {
    // SAFETY: plain call.
    unsafe { AllocConsole() };
    WINDOWLESS.store(false, Ordering::SeqCst);
}

/// Show an error where a person will see it, since there is no console.
pub fn show_error(message: &str) {
    let text = wide(message);
    let title = wide("Cairn");
    // SAFETY: both strings are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
        );
    }
}

/// If Cairn is already running (with its icon), ask it to open the browser
/// and return true, so a second copy isn't started that nobody can see. If it
/// is stopping, or doesn't answer, return false and let this copy start.
pub fn hand_over_to_running_copy() -> bool {
    let class = wide(CLASS_NAME);
    let mut answer = 0usize;
    // SAFETY: the class name is NUL-terminated and `answer` outlives the call.
    unsafe {
        let window = FindWindowW(class.as_ptr(), std::ptr::null());
        !window.is_null()
            && SendMessageTimeoutW(
                window,
                WM_OPEN,
                0,
                0,
                SMTO_ABORTIFHUNG,
                HANDOVER_WAIT_MS,
                &mut answer,
            ) != 0
            && answer == 1
    }
}

/// The icon in the notification area. It goes away when this is dropped.
pub struct Tray {
    finished: mpsc::Receiver<()>,
}

/// Add the icon. `url` is what **Open Cairn** opens; **Quit Cairn** stops
/// the server through `state`.
pub fn start(url: String, state: Arc<AppState>) -> Result<Tray, String> {
    let _ = CONTEXT.set(Context { url, state });
    let (ready, waiting) = mpsc::channel();
    let (done, finished) = mpsc::channel();
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(move || {
            run(&ready);
            let _ = done.send(());
        })
        .map_err(|e| format!("the icon could not be started: {e}"))?;
    match waiting.recv_timeout(READY_WAIT) {
        Ok(Ok(())) => Ok(Tray { finished }),
        Ok(Err(why)) => Err(why),
        Err(_) => {
            // Stuck: have it close its window if it ever gets that far, so
            // no icon is left behind (it checks for this before adding one).
            drop(Tray { finished });
            Err("the icon did not appear in time".into())
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        CLOSING.store(true, Ordering::SeqCst);
        let window = WINDOW.load(Ordering::SeqCst) as HWND;
        if !window.is_null() {
            // SAFETY: posting to a window that may already be gone is allowed.
            unsafe { PostMessageW(window, WM_CLOSE, 0, 0) };
        }
        // Not forever: Cairn is on its way out whatever the icon is doing.
        let _ = self.finished.recv_timeout(STOP_WAIT);
    }
}

fn run(ready: &mpsc::Sender<Result<(), String>>) {
    let class = wide(CLASS_NAME);
    let title = wide("Cairn");
    let taskbar = wide("TaskbarCreated");
    // SAFETY: every pointer handed to Windows below points at data that
    // outlives the call, and the window procedure only runs on this thread.
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let mut class_info: WNDCLASSW = std::mem::zeroed();
        class_info.lpfnWndProc = Some(window_proc);
        class_info.hInstance = instance;
        class_info.lpszClassName = class.as_ptr();
        if RegisterClassW(&class_info) == 0 {
            let _ = ready.send(Err("the icon's window could not be registered".into()));
            return;
        }
        TASKBAR_CREATED.store(RegisterWindowMessageW(taskbar.as_ptr()), Ordering::SeqCst);
        // A window that is never shown: it only receives messages.
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        );
        if window.is_null() {
            let _ = ready.send(Err("the icon's window could not be made".into()));
            return;
        }
        WINDOW.store(window as usize, Ordering::SeqCst);
        // Cairn gave up waiting for this and is already closing.
        if CLOSING.load(Ordering::SeqCst) {
            DestroyWindow(window);
            let _ = ready.send(Err("Cairn was already closing".into()));
            return;
        }
        if !add_icon_with_retries(window) {
            DestroyWindow(window);
            let _ = ready.send(Err("Windows did not accept the icon".into()));
            return;
        }
        let _ = ready.send(Ok(()));
        let mut message: MSG = std::mem::zeroed();
        while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Add the icon, and ask Windows for the newer way of telling Cairn what was
/// done to it, which also covers the keyboard.
fn add_icon(window: HWND) -> bool {
    if !notify(window, NIM_ADD, 0) {
        return false;
    }
    notify(window, NIM_SETVERSION, NOTIFYICON_VERSION_4);
    true
}

fn add_icon_with_retries(window: HWND) -> bool {
    for attempt in 0..ADD_TRIES {
        if attempt > 0 {
            std::thread::sleep(ADD_PAUSE);
        }
        if add_icon(window) {
            return true;
        }
    }
    false
}

/// Add, change, or remove the icon. `version` is only used to set it.
fn notify(window: HWND, action: NOTIFY_ICON_MESSAGE, version: u32) -> bool {
    let tip = wide(TOOLTIP);
    // SAFETY: the data is fully set before it is handed over, and the icon
    // handle comes from Windows.
    unsafe {
        let mut data: NOTIFYICONDATAW = std::mem::zeroed();
        data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = window;
        data.uID = 1;
        data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
        data.uCallbackMessage = WM_TRAY;
        data.hIcon = program_icon();
        data.Anonymous.uVersion = version;
        let len = tip.len().min(data.szTip.len());
        data.szTip[..len].copy_from_slice(&tip[..len]);
        Shell_NotifyIconW(action, &data) != 0
    }
}

/// The Cairn logo from `cairn.exe`, or Windows' plain program icon.
fn program_icon() -> *mut c_void {
    // SAFETY: a resource id is passed as a number, as Windows expects.
    unsafe {
        let own = LoadIconW(
            GetModuleHandleW(std::ptr::null()),
            EXE_ICON_ID as *const u16,
        );
        if own.is_null() {
            LoadIconW(std::ptr::null_mut(), IDI_APPLICATION)
        } else {
            own
        }
    }
}

/// Opening the browser can take a moment; the icon must not freeze meanwhile.
fn open_cairn() {
    if let Some(context) = CONTEXT.get() {
        let url = context.url.clone();
        std::thread::spawn(move || {
            let _ = open::that_detached(url);
        });
    }
}

fn quit_cairn() {
    QUITTING.store(true, Ordering::SeqCst);
    if let Some(context) = CONTEXT.get() {
        context.state.shutdown.notify_one();
    }
}

/// The screen position Windows gives with an icon event (packed in `wparam`):
/// where the mouse was, or the icon itself for a key press.
fn anchor(wparam: WPARAM) -> POINT {
    POINT {
        x: i32::from((wparam & 0xFFFF) as u16 as i16),
        y: i32::from(((wparam >> 16) & 0xFFFF) as u16 as i16),
    }
}

fn show_menu(window: HWND, at: POINT) {
    let open_text = wide("Open Cairn");
    let quit_text = wide("Quit Cairn");
    // SAFETY: the strings outlive the calls; the menu is destroyed after use.
    let chosen = unsafe {
        let menu = CreatePopupMenu();
        AppendMenuW(menu, MF_STRING, MENU_OPEN, open_text.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_QUIT, quit_text.as_ptr());
        SetMenuDefaultItem(menu, MENU_OPEN as u32, 0);
        // Without this the menu would not close when clicking elsewhere.
        SetForegroundWindow(window);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            at.x,
            at.y,
            0,
            window,
            std::ptr::null(),
        );
        PostMessageW(window, WM_NULL, 0, 0);
        DestroyMenu(menu);
        chosen
    };
    match usize::try_from(chosen) {
        Ok(MENU_OPEN) => open_cairn(),
        Ok(MENU_QUIT) => quit_cairn(),
        _ => {}
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        // With the newer icon protocol the event is in the low word.
        WM_TRAY => {
            match (lparam as u32) & 0xFFFF {
                NIN_SELECT | NIN_KEYSELECT => open_cairn(),
                WM_CONTEXTMENU => show_menu(window, anchor(wparam)),
                _ => {}
            }
            0
        }
        // From a second copy of Cairn. 1 tells it this copy took over.
        WM_OPEN => {
            if QUITTING.load(Ordering::SeqCst) || CLOSING.load(Ordering::SeqCst) {
                0
            } else {
                open_cairn();
                1
            }
        }
        // Windows (the installer, say) asks Cairn to close: stop properly.
        // Cairn closing its own window is told apart by `CLOSING`.
        WM_CLOSE => {
            if CLOSING.load(Ordering::SeqCst) {
                // SAFETY: this is the window being handled.
                unsafe { DestroyWindow(window) };
            } else {
                quit_cairn();
            }
            0
        }
        // Signing out, shutting down, or an installer closing Cairn so it can
        // replace the program. There is no time to stop the server, but the
        // edit locks must not be left behind for everyone else.
        WM_ENDSESSION => {
            if wparam != 0
                && let Some(context) = CONTEXT.get()
            {
                context.state.release_all_locks();
                if (lparam as u32) & ENDSESSION_CLOSEAPP != 0 {
                    quit_cairn();
                }
            }
            0
        }
        WM_DESTROY => {
            notify(window, NIM_DELETE, 0);
            // SAFETY: plain call, ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            0
        }
        other if other != 0 && other == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Explorer restarted and lost the icon.
            add_icon(window);
            0
        }
        // SAFETY: the arguments are those Windows gave this procedure.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
