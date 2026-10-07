# Contributing

## Prerequisites (Windows)

Setting up a new computer? [docs/building.md](docs/building.md) walks through every step, from installing the tools to a finished `cairn.exe`.

1. **Rust** 1.88 or newer (the project uses the 2024 edition). Install from <https://rustup.rs>. `rustup` installs `cargo`, `rustfmt`, and `clippy`.
2. **Microsoft C++ Build Tools**, which Rust needs to link Windows programs:

   ```
   winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
   ```

   Or install **Visual Studio Build Tools** and select **Desktop development with C++**.
3. **Git**.

Nothing else is needed: no Node.js, no database. The browser interface is plain HTML, CSS, and JavaScript in `assets/`, embedded into the program when it's built.

> **Git Bash users:** run cargo from PowerShell or Command Prompt. Git Bash ships a Unix `link` command that shadows the Microsoft linker (`link.exe`), and builds fail with `link: extra operand`.

## Build and run

```powershell
git clone <repository-url> cairn
cd cairn
cargo run                     # debug build; opens your browser
cargo build --release         # optimized build: target\release\cairn.exe
```

Useful while developing:

```powershell
# Keep your real settings and drafts out of the way:
$env:CAIRN_HOME = "$env:TEMP\cairn-dev"
cargo run -- --no-browser --port 7878
```

In debug builds, files in `assets/` are read from disk, so reloading the browser shows changes to HTML, CSS, and JavaScript without rebuilding. Release builds embed them.

## Create a sample workspace

```powershell
cargo run -- init "$env:TEMP\SampleDocs" --name "Sample Documentation"
cargo run -- --workspace "$env:TEMP\SampleDocs"
```

Don't commit real documentation, drafts, or `config.json` files. `.gitignore` excludes `/sample-workspace/` for local experiments.

## Test

```powershell
cargo test                                  # all tests
cargo test --test editing                   # one file
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

| Test file | Covers |
| --- | --- |
| unit tests in `src/` | Individual functions |
| `tests/workspace_and_paths.rs` | Discovery, concurrent and interrupted setup, path and link escapes |
| `tests/editing.rs` | Locks, idle release, drafts, conflicts, failure injection during publish, history |
| `tests/content.rs` | Picture validation and publishing, rendering and sanitizing, search |
| `tests/security.rs` | Write permissions (uses `icacls` on Windows); token, origin, and host checks; loopback-only binding |
| `tests/ui_quality.rs` | Color contrast in every theme, labels, focus styles, no remote assets |
| `tests/shutdown.rs` | Stopping Cairn without Ctrl+C still gives back edit locks (runs the real program; Unix only) |

### Browser tests (optional)

`tests/e2e/` drives the real program in a browser with [Playwright](https://playwright.dev): unsaved changes survive a failed save, and closing, leaving, or reloading is held back while Cairn is publishing, adding a picture, or preparing a download. They need **Node.js** 22 or newer; `cargo test` doesn't need them. CI doesn't run them, so run them yourself before a release or after changing the editor, leaving, or download code.

```powershell
cd tests\e2e
npm ci
npx playwright install chromium
npm test
```

Each test starts its own Cairn on a fresh temporary folder. A slow or failing shared folder is imitated by holding or failing requests in the browser.

Network-share behavior is checked by hand with [docs/manual-two-computer-checklist.md](docs/manual-two-computer-checklist.md).

## Code layout

See [docs/architecture.md](docs/architecture.md). The short version:

- `src/paths.rs` is the only place request paths become filesystem paths. Anything that touches workspace files goes through `Root::resolve` or `Root::resolve_for_create`.
- `src/fsutil.rs` holds the write primitives. Never open a live workspace file for writing; use `write_atomic` or `create_new_with`.
- `src/server/api.rs` holds the HTTP handlers; blocking file work goes through `blocking(...)`.
- In the browser code, build elements with `h()` and `button()` from `assets/js/core.js`. Never insert text from files as HTML. `button()` refuses to create a button without a visible label.

## UI guidelines

Cairn's users include people who are not comfortable with modern software. Please keep:

- visible words on every button (icons only beside words);
- large click targets (at least 44 px) and the always-visible sidebar (no hamburger menus);
- plain-language messages that say what happened and what to do next;
- colors from `assets/css/tokens.css` only. `tests/ui_quality.rs` checks their contrast.

## Documentation

Update the docs in the same change as the behavior they describe, and add a line to `CHANGELOG.md` for anything a user would notice.

## Commits

Use short, conventional messages such as `feat: …`, `fix: …`, `docs: …`, `test: …`.

## Releases and updates

Pushing a tag such as `v0.5.0` (matching `Cargo.toml`) runs `.github/workflows/release.yml`. It builds `cairn.exe`, the zip, and the installer (`installer/cairn.iss`, built with the free Inno Setup by `installer/build.ps1`), signs the zip and the installer, and publishes them.

**Signing.** Cairn installs an update only if the zip is signed with the release key, whose public half is `PUBLIC_KEY` in `src/update.rs`. The secret half is the `MINISIGN_SECRET_KEY` repository secret; the release fails without it. The key was made with:

```
cargo run --example release_sign -- keygen <folder outside the repository>
```

Keep a backup of `cairn-release.key` somewhere safe (a password manager works). If it is lost, make a new pair, put the new public key in `src/update.rs`, and release: copies from before that release can't verify the next update, so people install it by hand once. Never commit the secret key.

**Trying an update locally.** Debug builds read `CAIRN_UPDATE_API` (a `http://127.0.0.1…` address serving a release JSON like GitHub's) and `CAIRN_UPDATE_KEY` (a test public key), so the whole update can be tried with a test key and a local web server. Release builds ignore both. `tests/update.rs` does the same with a fake server.
