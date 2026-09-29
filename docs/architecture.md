# Architecture

Cairn is one Rust program per user plus a folder of files. There is no central server and no database.

```
 Alex's computer                           Shared folder (local or \\server\share)
 ┌────────────────────────────────┐        ┌───────────────────────────────────┐
 │ Browser ──HTTP──▶ cairn.exe    │─files─▶│ shared-docs.json                  │
 │           (127.0.0.1 only)     │        │ Guides/page.md                    │
 │                                │        │ Guides/page.assets/pic-1a2b3c4d.png│
 │ %LOCALAPPDATA%\Cairn\          │        │ _system/locks/  _system/history/  │
 │   config.json, drafts\         │        └───────────────────────────────────┘
 └────────────────────────────────┘                        ▲
 Sam's computer: the same program, the same folder ────────┘
```

Coordination between people happens entirely through files in the shared folder, using each person's own Windows permissions.

## The local program

`cairn.exe` (`src/main.rs`) loads the user's settings, reopens the last documentation folder, binds an HTTP server to **127.0.0.1 on a random free port**, opens the default browser, and runs until its window is closed, Ctrl+C is pressed, or **Quit Cairn** is chosen. On exit it releases any edit locks it holds.

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
| Tracking or data leaks | No telemetry, no update checks, no remote fonts or scripts. Remote images in pages are shown as a labeled link and never fetched. |

The token reaches the browser in the URL fragment (`#t=…`), which browsers never send to servers, and is then kept in that tab's `sessionStorage`.

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
| `export/` | Choosing files and zipping them (`archive.rs`), self-contained printable HTML (`html.rs`), PDFs via headless Edge or Chrome (`pdf.rs`). |
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
_system/trash/<id>/item.json                      a deleted page or folder: where it was, who, when
_system/trash/<id>/content/                       the deleted page (with its .assets) or folder
```

**On each person's computer** (private, under `%LOCALAPPDATA%\Cairn`, which Windows restricts to that user by default):

```
config.json
drafts/<workspace id>/<page hash>/draft.json
drafts/<workspace id>/<page hash>/staged/<pictures not yet published>
```

## Search

When a folder is opened, Cairn reads every `.md` file once into an in-memory index of titles and plain text. After that it re-reads only files whose size or modified time changed, at most every few seconds. `_system`, `_templates`, `.assets` folders, hidden files, and links are skipped. Nothing is stored on disk.

## Downloads and PDFs

Markdown downloads are zips of the files exactly as stored, with workspace-relative paths so links between pages and pictures keep working after unzipping. `_system`, hidden and temporary files, and anything reached through a link or junction are never included.

For a PDF, the page is rendered with the same sanitizing renderer, pictures are embedded as `data:` URIs, in-app links become plain text, and the styles and font are inlined. The resulting HTML carries its own Content-Security-Policy (`default-src 'none'`, no scripts). Cairn writes it to a private temporary folder and runs Microsoft Edge (or Chrome) headless with a separate throwaway profile and `--print-to-pdf`, so the person's open browser is never touched. The run is limited to 60 seconds and the temporary folder is always deleted. If one browser fails (for example, when a tool redirects every Edge launch), the next is tried. If none works, the API answers `409 pdf_unavailable` and the interface offers its print view instead.

Folder and whole-documentation downloads run as in-memory jobs, one at a time, so the interface can show progress and cancel. Finished files are handed over once and dropped after 10 minutes.

## Timezones

Everything written to disk stays UTC: lock and draft times are Unix seconds, and earlier-version names are UTC stamps. Only display converts. The browser formats times with `Intl.DateTimeFormat` in the chosen zone, labelled with `shortOffset` ("GMT+8"). The server uses the [jiff](https://github.com/BurntSushi/jiff) crate with bundled timezone data (Windows has no tz database Rust can read) for PDF headers and for `{{date}}` in templates.
