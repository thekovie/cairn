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

Plain HTML, CSS, and JavaScript modules in `assets/`, embedded into the program at build time with [rust-embed](https://github.com/pyrossh/rust-embed). There is no front-end build step and no Node.js. The interface works offline; the Inter font (SIL Open Font License) is included.

| File | Purpose |
| --- | --- |
| `assets/index.html` | Page shell |
| `assets/css/tokens.css` | Colors, type scale, and spacing for the Light, Dark, and High contrast themes |
| `assets/css/app.css`, `markdown.css` | Layout, components, rendered pages |
| `assets/js/core.js` | API calls, safe DOM building, icons, dialogs, messages |
| `assets/js/app.js` | Start-up, routing (`#/page/…`), layout |
| `assets/js/views.js` | Home, folder, search, page, earlier versions, new page, settings |
| `assets/js/setup.js` | Choosing or creating a documentation folder |
| `assets/js/editor.js` | The editor |

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
| `search.rs` | In-memory index, folder listings, search. |
| `config.rs` | Per-user settings. |
| `server/` | HTTP routes, security, idle time-outs. |

## Files on disk

**In the shared folder** (everyone sees these):

```
shared-docs.json                                  workspace marker
README.md
<Folder>/<page>.md                                pages (Markdown, optional front matter)
<Folder>/<page>.assets/<picture>                  pictures for that page
_system/locks/<hash>.lock                         who is editing what
_system/locks/released/*.json                     locks released by a maintainer
_system/history/<page path>/<UTC time>-<hash>.md  earlier versions
```

**On each person's computer** (private, under `%LOCALAPPDATA%\Cairn`, which Windows restricts to that user by default):

```
config.json
drafts/<workspace id>/<page hash>/draft.json
drafts/<workspace id>/<page hash>/staged/<pictures not yet published>
```

## Search

When a folder is opened, Cairn reads every `.md` file once into an in-memory index of titles and plain text. After that it re-reads only files whose size or modified time changed, at most every few seconds. `_system`, `.assets` folders, hidden files, and links are skipped. Nothing is stored on disk.
