# Architecture

Cairn is one Rust program per user plus a folder of files. There is no central server and no database.

```
 Alex's computer                           Shared folder (local or \\server\share)
 ┌────────────────────────────────┐        ┌───────────────────────────────────┐
 │ Browser ──HTTP──▶ cairn        │─files─▶│ shared-docs.json                  │
 │           (127.0.0.1 only)     │        │ Guides/page.md                    │
 │                                │        │ Guides/page.assets/pic-1a2b3c4d.png│
 │ settings folder (private)      │        │ _system/locks/  _system/history/  │
 │   config.json, drafts\         │        └───────────────────────────────────┘
 └────────────────────────────────┘                        ▲
 Sam's computer: the same program, the same folder ────────┘
```

Coordination between people happens entirely through files in the shared folder, using each person's own permissions on it. The same program runs on Windows, macOS, and Linux; people on different systems can share one folder.

## The local program

The program (`cairn.exe` on Windows, `cairn` elsewhere; `src/main.rs`) loads the user's settings, reopens the last documentation folder, binds an HTTP server to **127.0.0.1 on a random free port**, opens the default browser, opens the last documentation folder while the page shows “Starting up Cairn” (`starting` in `GET /api/state`; the address is printed on the console once the folder is open), and runs until its window is closed (the Terminal window on a Mac or Linux), Ctrl+C is pressed, the computer signs out or shuts down, or **Quit Cairn** is chosen. On exit it releases any edit locks it holds.

On Windows, a plain start (a shortcut or the Start menu) lets go of the console window if it was opened just for Cairn, and `src/tray.rs` adds a notification-area icon in its place. A hidden window on its own thread owns the icon and hears what the console used to: Windows asking Cairn to close (the installer does this), signing out, and shutting down, each of which releases the edit locks. A second plain start finds that window and asks the running copy to open the browser instead of starting another. Started from a terminal, Cairn keeps that terminal and shows no icon. If the icon can't be added, a console window is opened again so there is still a way to quit. The update restart starts the new program with no console of its own.

The server is [axum](https://github.com/tokio-rs/axum) on tokio. Every filesystem operation runs on the blocking thread pool, so a slow network share never freezes the server.

### Local web security

| Threat | Defense (`src/server/security.rs`) |
| --- | --- |
| Other computers on the network | The listener binds `127.0.0.1` only, never `0.0.0.0`. |
| Other websites calling the API (CSRF) | Every `/api/` call needs a random per-launch token in the `X-Cairn-Token` header. Browsers only send custom headers cross-origin after a CORS preflight, which Cairn never approves. Requests with a foreign `Origin` or `Sec-Fetch-Site: cross-site` are refused. |
| DNS rebinding | The `Host` header must be `127.0.0.1:<port>` or `localhost:<port>`. |
| Other local programs | They don't have the token. It's generated from the OS random source at each launch and compared in constant time. |
| Pictures in `<img>` tags, which can't send headers | Workspace files and draft pictures need a separate per-launch read key in the URL. Anything that isn't a supported picture is sent as a download, never rendered. |
| Script injection | Markdown is rendered on the server; raw HTML becomes visible text; the result goes through the [ammonia](https://github.com/rust-ammonia/ammonia) allowlist sanitizer; a strict Content-Security-Policy (`default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self'`) blocks inline scripts and styles. |
| Tracking or data leaks | No telemetry, no remote fonts or scripts. The only request Cairn makes on its own is the optional once-a-day check for a new version (GitHub's public releases, nothing about the person or their pages; it can be turned off), sent through the office proxy when there is one, and it never asks for or keeps a password. Remote images in pages are shown as a labeled link and never fetched. |

The token reaches the browser in the URL fragment (`#t=…`), which browsers never send to servers, and is then kept in that tab's `sessionStorage`. A tab opened without it (a link opened in a new tab, or the address pasted) asks the Cairn tabs already open for it over a `BroadcastChannel`, which only pages from the same address can join, so the token is never written to disk. With no Cairn tab open to ask, the tab shows “Please open Cairn again”.

A tab asks `GET /api/ping` (which does no work) every three seconds. Two missed answers in a row put a **Cairn has stopped** notice at the top of the page; a failed request or coming back to the tab checks at once, and an answer removes the notice. It is paused while Cairn restarts for an update and after **Quit Cairn**, which are meant.

## The browser interface

Plain HTML, CSS, and JavaScript modules in `assets/`, embedded into the program at build time with [rust-embed](https://github.com/pyrossh/rust-embed). There is no front-end build step and no Node.js; the one third-party library, the visual editor engine, is committed pre-built (see `vendor/milkdown/README.md`).

Visual editing never becomes a second source of truth: the Markdown text box is. Visual edits are converted to Markdown and written back to it immediately, so drafts, locks, conflict checks, publishing, and history are unchanged. The front-matter block is split off first and never passes through the visual editor. The editor's schema only knows Markdown constructs, so pasted HTML is reduced to those, and raw HTML in a page is shown as text. The interface works offline; the Inter font (SIL Open Font License) is included.

| File | Purpose |
| --- | --- |
| `assets/index.html` | Page shell |
| `assets/css/tokens.css` | Colors, type scale, and spacing for the Light, Dark, and High contrast themes |
| `assets/css/app.css`, `markdown.css` | Layout, components, rendered pages |
| `assets/css/print.css` | Paper layout, used by PDFs and the print view |
| `assets/js/core.js` | API calls, safe DOM building, icons, dialogs, messages, timezone-aware time formatting, downloads |
| `assets/js/app.js` | Start-up, routing (`#/page/…`), layout |
| `assets/js/views.js` | Home, folder, search, page, earlier versions, new page, settings |
| `assets/js/setup.js` | Choosing or creating a documentation folder |
| `assets/js/editor.js` | The editor, including template mode |
| `assets/js/templates.js` | The Templates page |
| `assets/js/export.js` | Download dialogs, folder download progress, the print view |
| `assets/js/timezone.js` | Settings → Time and timezone |
| `assets/js/visual.js` | Visual editing, on top of the vendored editor engine |
| `assets/vendor/milkdown.js` | [Milkdown](https://milkdown.dev) (ProseMirror + remark, MIT), pre-built from `vendor/milkdown/` and loaded only when visual editing is first used. Licenses in `milkdown.LICENSES.txt`. |

Text from files is always inserted as text, never as HTML. The only HTML inserted is page HTML the server has already sanitized.

## Library modules (`src/`)

| Module | Responsibility |
| --- | --- |
| `paths.rs` | The single gate for request paths: rejects `..`, absolute and UNC paths, drive letters, alternate data streams, and reserved device names; refuses to pass through symlinks or junctions; checks that the canonical result is inside the root. |
| `fsutil.rs` | Atomic write (temp file in the same folder, flush, rename), exclusive "create if absent", write-permission probe, reparse-point detection. |
| `workspace.rs` | Marker file, discovery, initialization, the storage check. |
| `article.rs` | Front matter, titles, rendering, links, table of contents. |
| `images.rs` | Picture validation and naming. |
| `locks.rs` | Edit locks, heartbeats, maintainer recovery. |
| `drafts.rs` | Private per-user unsaved changes. |
| `publish.rs` | The safe publishing sequence. |
| `history.rs` | Earlier versions. |
| `links.rs` | Rewriting relative links when pages or folders move: finds each link's destination in the source text and changes only that. |
| `manage.rs` | Renaming and moving pages and folders, with their pictures and earlier versions, under the edit locks of every page involved. |
| `trash.rs` | Recently deleted: moving pages and folders into `_system/trash` and restoring them. |
| `search.rs` | In-memory index, folder listings, search. |
| `config.rs` | Per-user settings. |
| `timefmt.rs` | Timezones: validation, the user's zone, "GMT+8" labels, today's date. Stored times are always UTC. |
| `templates.rs` | Built-in and team templates, filling in `{{…}}` fields, finding deleted templates. |
| `export/` | Choosing files and zipping them (`archive.rs`), self-contained printable HTML (`html.rs`), PDFs via headless Edge, Chrome, or Chromium (`pdf.rs`). |
| `server/` | HTTP routes (`api.rs`, `api_templates.rs`, `api_export.rs`, `api_manage.rs`), security, idle time-outs. |

## Files on disk

**In the shared folder** (everyone sees these):

```
shared-docs.json                                  workspace marker
README.md
<Folder>/<page>.md                                pages (Markdown, optional front matter)
<Folder>/<page>.assets/<picture>                  pictures for that page
_templates/<template>.md                          team templates for this documentation
_system/locks/<hash>.lock                         who is editing what
_system/locks/released/*.json                     locks released by a maintainer
_system/history/<page path>/<UTC time>-<hash>.md  earlier versions
_system/edited/<page path>.json                   who last published the page, and a fingerprint of it
_system/.probe/                                   scratch files from the storage check (removed afterwards)
_system/trash/<id>/item.json                      a deleted page or folder: where it was, who, when
_system/trash/<id>/content/                       the deleted page (with its .assets) or folder
```

**On each person's computer** (private, in the [settings folder](configuration.md#settings-file): `%LOCALAPPDATA%\Cairn` on Windows, which Windows restricts to that user; on a Mac or Linux it is created so only that user can open it):

```
config.json
drafts/<workspace id>/<page hash>/draft.json
drafts/<workspace id>/<page hash>/staged/<pictures not yet published>
```

## Search

Filters (folder, status, tag), the order, and the 50-page limit are applied in that order: pages that fail a filter are never scored, and the limit cuts only the sorted matches, so a filtered search can't lose a page to unfiltered ones. A title typed exactly counts far more than anything else, then title and tag words, then up to 20 mentions in the text. Ties break by title and then path, both ignoring capitals.

When a folder is opened, Cairn reads every `.md` file once into an in-memory index of titles and plain text. After that it re-reads only files whose size or modified time changed, at most every few seconds. `_system`, `_templates`, `.assets` folders, hidden files, and links are skipped. Nothing is stored on disk.

## Downloads and PDFs

Markdown downloads are zips of the files exactly as stored, with workspace-relative paths so links between pages and pictures keep working after unzipping. `_system`, `shared-docs.json`, hidden and temporary files, and anything reached through a link or junction are never included.

A download never leaves a file out without saying so. A folder or file Cairn cannot read (no permission, share disconnected, removed during the download) stops the download and is named in the message. Links that are left out on purpose are listed when the download finishes. Each file's size and modified time are noted when the download starts and checked again after it is read, so files that changed meanwhile are listed too. This is a check, not a snapshot: Cairn does not lock the shared folder, and other people can keep editing while a download is prepared.

For a PDF, the page is rendered with the same sanitizing renderer, pictures are embedded as `data:` URIs, links inside the documentation become readable references (see below), and the styles and font are inlined. A link to another page keeps its text and gains a quiet note naming the page it pointed at (its title, and its folder when that differs from the page's own), unless the text already says the title; a link to a file names the file. A link to a heading on the same page, and the title heading itself, stay real links, which work inside the PDF. Before any PDF is made, `GET /api/export/pdf-links?scope=page|folder|all&path=` counts the links this will convert (`html::converted_links`, which makes the same decision as the rewriting: links to other pages and files, not links to a heading on the same page, the page itself, web or email addresses, or broken links) so the download dialog can warn first. Links between separate PDFs are deliberately not attempted: the browser prints each page from a temporary folder, so a relative link would lead nowhere. Web links stay as they are, and pictures are never fetched from the web. The resulting HTML carries its own Content-Security-Policy (`default-src 'none'`, no scripts). Cairn writes it to a private temporary folder and runs Microsoft Edge (or Chrome) headless with a separate throwaway profile and `--print-to-pdf`, so the person's open browser is never touched. The run is limited to 60 seconds and the temporary folder is always deleted. If one browser fails (for example, when a tool redirects every Edge launch), the next is tried. If none works, the API answers `409 pdf_unavailable` and the interface offers its print view instead.

Folder and whole-documentation downloads run as in-memory jobs, one at a time, so the interface can show progress and cancel. Finished files are handed over once and dropped 10 minutes after the job ends (not after it starts, so a slow PDF job is still there when it finishes). A job that crashes is marked failed and does not block the next download. Leftover PDF working folders (`cairn-pdf-*`) older than an hour are removed the first time a PDF is made after Cairn starts.

## Documentation health and the link check

`src/health.rs` turns the page index into the Documentation health list: each page gets the problems it has (review due, no review date, no owner, draft, retired, unreadable details). The index records what its last scan could not read, so the screen shows a warning rather than a clean result when part of the folder was unreachable.

**Check documentation** (`src/linkcheck.rs`) runs as a job like downloads and backups: `POST /api/health/check/jobs` starts it and returns the page count, progress and Stop use the same `/api/export/jobs/{id}` routes, and the finished result is fetched once from `GET /api/health/check/jobs/{id}/result`. It reads every page once and reports links and pictures that lead nowhere. It makes the same decisions as page rendering, because `article::render` calls the same small functions (`judge_local_link`, `judge_local_image`, `anchor_is_missing`, `collect_headings`): a page never marks a link broken that the check calls fine. Links to headings on other pages are the one thing only the check looks into. It never contacts web addresses, reads each page and each linked-to page's headings once, and lists any page it could not read. Whether `Printer.md` and `printer.md` are the same file is up to the computer's file system, as everywhere else in Cairn.

## Backups

A backup (`src/backup.rs`) is a zip of everything under the workspace except edit locks, hidden files, and temporary files, so it includes `shared-docs.json`, `_templates`, and `_system/history`, `edited`, and `trash`. Its last entry, `cairn-backup.json`, lists every file with its size and SHA-256, plus a format name and `format_version` (currently 1), the workspace `instance_id` and name, the date, and the Cairn version. Files are streamed through the hash into a hidden `.part` file that is renamed into place only when complete, so a failed or cancelled backup leaves nothing behind.

The shared folder is not locked, so each file is judged on its own. A file is copied through an open handle and its size and modified time are read before and after: if they differ it was written while being copied (an editor saving in place), and a file that is gone before it is reached has vanished. Either restarts the backup with a fresh list, up to three times, and then it fails naming those files. A file saved, created, or deleted after it was copied does not restart anything: the backup holds it as copied, and the files that changed while it ran are listed in the result (`Summary.changed_during`). Cairn's own saves replace a page with a temporary file renamed over it, so a reader never sees half a page. This is a consistency check, not a snapshot. The finished file is read back and verified before it is kept.

Over HTTP a backup is a job like a download (`POST /api/backup/jobs`, then the same status, cancel, and file routes), but the finished zip waits in the computer's temporary folder and is streamed to the browser, then deleted, instead of being held in memory. Backup files older than a day in the temporary folder are removed when the next backup starts.

Before a backup writes anything, it estimates the size (files that are already compressed count in full, other files at a tenth as the least and in full as the most) and asks the drive how much is free: less than the least is refused, less than the most is allowed with a note. A restore needs the exact total of the files plus a margin (8 MB or 1%, whichever is more) and refuses up front otherwise. Free space comes from `fsutil::available_space` (`statvfs` on Mac and Linux, `GetDiskFreeSpaceExW` on Windows, which reports a network drive's quota for the signed-in user); if it can't be read, the operation goes ahead.

Restore exists only on the command line (`cairn restore`, `cairn backup verify`). It checks the manifest (format, version, safe relative paths with no `..`, absolute or drive paths, backslashes, alternate streams, reserved names, locks, or duplicates), then reads every entry against its size and checksum while extracting into a hidden folder beside the destination. Entries that are links, folders, or not in the list are refused. The destination must be new or empty and not inside another workspace; only after every check passes is the folder moved into place.

## Timezones

Everything written to disk stays UTC: lock and draft times are Unix seconds, and earlier-version names are UTC stamps. Only display converts. The browser formats times with `Intl.DateTimeFormat` in the chosen zone, labelled with `shortOffset` ("GMT+8"). The server uses the [jiff](https://github.com/BurntSushi/jiff) crate with bundled timezone data (Windows has no tz database Rust can read) for PDF headers and for `{{date}}` in templates.
