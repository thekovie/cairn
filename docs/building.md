# Building Cairn

This guide takes a Windows 10 or 11 computer with nothing installed to a working `cairn.exe` built from the source code. It takes about 20–40 minutes, mostly waiting for downloads. On a Mac or Linux it's shorter: see [On a Mac or Linux](#on-a-mac-or-linux).

If you only want to *use* Cairn, you don't need any of this: download it from the [Releases page](https://github.com/thekovie/cairn/releases/latest) instead.

## What you need

| Tool | Why | Size |
| --- | --- | --- |
| Git | To download (clone) the source code | ~300 MB |
| Microsoft C++ Build Tools | Rust uses Microsoft's linker to make Windows programs | ~2–6 GB |
| Rust 1.88 or newer | The compiler and `cargo`, the build tool | ~1 GB |

You need administrator rights to install the Build Tools. Everything else installs for your user only.

All commands below are for **PowerShell** (press Start, type *PowerShell*, open it).

> **Don't use Git Bash for building.** Git Bash has its own `link` command that hides Microsoft's linker, and the build fails with `link: extra operand`. Clone with whatever you like, but run `cargo` from PowerShell or Command Prompt.

## 1. Install the tools

The quickest way is `winget`, which is built into Windows 11 and recent Windows 10:

```powershell
winget install --id Git.Git -e
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install --id Rustlang.Rustup -e
```

The Build Tools installer shows a progress window and can take a while. When all three finish, **close PowerShell and open a new one** so it picks up the new programs.

<details>
<summary>Without winget</summary>

1. Git: download from <https://git-scm.com/download/win> and install with the default options.
2. Build Tools: download **Build Tools for Visual Studio 2022** from <https://visualstudio.microsoft.com/downloads/> (under *Tools for Visual Studio*). In the installer, tick **Desktop development with C++** and choose Install.
3. Rust: download `rustup-init.exe` from <https://rustup.rs> and run it. Press Enter to accept the default installation.

</details>

Check that everything is found:

```powershell
git --version
rustc --version     # must say 1.88 or newer
cargo --version
```

If `rustc` is older than 1.88, run `rustup update stable`.

## 2. Get the source code

Pick a folder for it (here, your Documents folder) and clone:

```powershell
cd $HOME\Documents
git clone https://github.com/thekovie/cairn.git
cd cairn
```

To build a specific released version instead of the latest code, check out its tag, for example:

```powershell
git checkout v0.2.0
```

## 3. Build the program

```powershell
cargo build --release --locked
```

The first build downloads the libraries Cairn uses and compiles everything, which takes a few minutes. Later builds are much faster. `--locked` makes sure you get exactly the library versions listed in `Cargo.lock`, the same ones the official release uses.

When the last line starts with **Finished**, the program is here:

```
target\release\cairn.exe
```

That single file is the whole program. The interface (HTML, CSS, JavaScript, and the font) is built into it. No installer is needed.

## 4. Check it works (optional but recommended)

Run the automated tests:

```powershell
cargo test
```

They should all pass. One test makes a real PDF if Microsoft Edge or Google Chrome is installed, and says it was skipped otherwise.

Then start the program you built:

```powershell
.\target\release\cairn.exe
```

Your browser opens Cairn. Close the Cairn window (or press Ctrl+C) to stop it.

## 5. Use or share the exe

Copy `target\release\cairn.exe` anywhere, for example to your Desktop or a shared folder, and double-click it. Each person runs their own copy on their own computer; see [Getting started](getting-started.md).

To make the same zip that the Releases page offers:

```powershell
$v = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
$name = "cairn-$v-windows-x64"
New-Item -ItemType Directory -Force "dist\$name" | Out-Null
Copy-Item target\release\cairn.exe, README.md, LICENSE, CHANGELOG.md "dist\$name\"
Copy-Item docs "dist\$name\docs" -Recurse -Force
Compress-Archive -Path "dist\$name\*" -DestinationPath "dist\$name.zip" -Force
```

The zip is in the `dist` folder.

Windows SmartScreen may warn the first time someone runs an exe you built, because it isn't code-signed. Choose **More info → Run anyway**.

To make the installer too, install [Inno Setup 6](https://jrsoftware.org/isinfo.php) (free) and run:

```powershell
pwsh installer\build.ps1 -Version $v -SourceDir "dist\$name" -OutDir dist
```

A Cairn you build yourself tells you when an official release is newer, and can install it, because official releases are signed with the project's key.

## On a Mac or Linux

1. Install the tools:
   - **Mac:** open Terminal and run `xcode-select --install` (Apple's command-line tools, which include Git and the linker).
   - **Linux:** install Git and a C toolchain, for example `sudo apt install git build-essential` on Ubuntu or Debian.
2. Install Rust from <https://rustup.rs> (the command on that page), then open a new terminal.
3. Get the code and build it:

   ```sh
   git clone https://github.com/thekovie/cairn.git
   cd cairn
   cargo build --release --locked
   ```

The program is `target/release/cairn`. Start it with `./target/release/cairn`; your browser opens Cairn, and closing the terminal window (or Ctrl+C) stops it. `cargo test` runs the tests; the PDF test uses Google Chrome, Microsoft Edge, or Chromium if one is installed.

To make the same downloads the Releases page offers:

- **Mac app:** `installer/macos/build-app.sh <version> <Apple silicon cairn> <Intel cairn> dist` makes `Cairn.app` in a zip. It needs both builds (`cargo build --release --target aarch64-apple-darwin` and `--target x86_64-apple-darwin`, after `rustup target add` for each).
- **Linux:** `installer/linux/build-linux.sh <version> target/release/cairn dist` makes the AppImage and the `.tar.gz`. It needs [appimagetool](https://github.com/AppImage/appimagetool) on your PATH.

## Updating later

To build a newer version on the same computer (in PowerShell on Windows; the same commands work in a terminal on a Mac or Linux):

```powershell
cd $HOME\Documents\cairn
git pull
cargo build --release --locked
```

## If something goes wrong

| Message | What to do |
| --- | --- |
| `error: linker 'link.exe' not found` | The C++ Build Tools are missing or incomplete. Run the Build Tools installer again and make sure **Desktop development with C++** is ticked. Then open a new PowerShell window. |
| `link: extra operand` | You're building from Git Bash. Use PowerShell. |
| `'cargo' is not recognized` | Open a new PowerShell window after installing Rust. If it still fails, sign out of Windows and back in. |
| `package requires rustc 1.88` or similar | Run `rustup update stable`. |
| `failed to remove file ... cairn.exe` / `Access is denied` | Cairn is still running from that folder. Close it (or use **Settings → Quit Cairn**) and build again. |
| `the lock file needs to be updated but --locked was passed` | Your copy of the code is out of step with `Cargo.lock`. Run `git status`; if you didn't mean to change anything, run `git checkout Cargo.lock` and try again. |
| Downloads fail behind a company proxy | Set the proxy for cargo: `$env:HTTPS_PROXY = "http://proxy.example.com:8080"` (use your company's address), then build again. |

For working on Cairn itself (running in debug mode, tests, code style), see [CONTRIBUTING.md](../CONTRIBUTING.md).
