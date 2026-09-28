# Contributing

## Prerequisites (Windows)

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
