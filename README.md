<p align="center">
  <img src="assets/favicon.svg" alt="Cairn logo: a stack of four blue stones" width="112">
</p>

<h1 align="center">Cairn</h1>

<p align="center">
  <strong>Stop opening Word files one by one.<br>Search your team's documentation, right on the shared drive you already have.</strong>
</p>

<p align="center">
  <a href="https://github.com/thekovie/cairn/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/thekovie/cairn?color=2563eb"></a>
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/github/license/thekovie/cairn?color=2563eb"></a>
  <img alt="Windows, macOS, and Linux" src="https://img.shields.io/badge/platforms-Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-2563eb">
</p>

<p align="center">
  <a href="#quick-start">Download</a> ·
  <a href="docs/getting-started.md">Getting started</a> ·
  <a href="#why-cairn">Why Cairn</a> ·
  <a href="#documentation">Docs</a>
</p>

<p align="center">
  <img src="docs/screenshots/home.png" alt="Cairn home screen with the page tree, search, folders, and recently changed pages" width="860">
</p>

Cairn is a documentation app for teams that keep their files on a **shared network drive**. Point it at a folder, and everyone on the team can read, search, and write pages in their normal web browser: no server to run, no accounts to manage, and no cloud service. The pages are ordinary Markdown files in your folder, so your documentation stays yours.

## Why Cairn

Many companies still keep their documentation on a shared drive as a tree of Word files. It works, until you need an answer in a hurry:

- **Finding something is slow.** To learn which file mentions "VPN" or "month-end close", you open files one by one, and each large Word file takes a moment to load.
- **Searching doesn't reach inside the files.** The drive can find a file *name*, but looking for the words *in* your documents usually means opening Word.
- **Editing gets in each other's way.** Someone has the file open, someone else saves a copy called `final_v2`, and nobody is sure which is current.
- **Not everyone is comfortable with the tools.** The people who most need the answer are often the least likely to fight with software to find it.

Cairn keeps the shared drive you already have and puts a simple, searchable handbook on top of it:

| On a shared drive of Word files | With Cairn |
| --- | --- |
| Open files one at a time to find a keyword | Type a word in the search box and see every page that mentions it |
| Wait for each document to open | Pages open right in your browser, with a table of contents |
| "The file is locked for editing by someone" | One person edits a page at a time, and others can keep reading it, with their name shown |
| `Policy_final_v2_REAL.docx` | Every page keeps its earlier versions, and you can bring one back |
| Broken links when someone renames or moves a file | Rename or move a page and the links to it are updated for you |
| Documents tied to one program | Plain `.md` files and pictures that any text editor can open |

You don't need to be technical to use it. Every button has words on it, you can format a page without knowing Markdown, and the text can be made larger in Settings.

## How it works

1. **Pick a folder.** Choose a folder on your shared drive (or create a new one). That folder *is* your documentation.
2. **Everyone runs Cairn on their own computer.** It opens the documentation in their usual browser. Nothing is installed on the drive and nothing leaves it.
3. **Read, search, and write.** Cairn coordinates people through small files in the folder itself, using the permissions the drive already has.

If you stop using Cairn, your documentation is still just a folder of `.md` files and pictures. The only thing Cairn fetches from the internet is news of a new version of itself, once a day (you can turn that off).

> **Moving from Word?** Cairn doesn't import `.docx` files automatically. Start new pages in Cairn and bring over the ones people use most; the old files can stay on the drive for as long as you like.

## Made for everyone on the team

- Every button has words on it. Nothing is hidden behind icons, hamburger menus, or keyboard shortcuts.
- You can write and format a page without knowing Markdown: toolbar buttons such as **Heading**, **Bullet list**, **Link…**, and **Insert picture…** do it for you, and a live preview shows how the page will look.
- Text can be made larger in Settings, and the layout adapts to any window size, from a full desktop screen to half a screen or a phone. There are Light, Dark, and High contrast color schemes.
- Messages use plain words and always say what to do next.

## What it does

- **Read and search** pages, with a table of contents, owner, status, and last-reviewed date for each page.
- **Edit safely.** One person edits a page at a time. Others see “Being edited by Alex since 2:30 PM” and can keep reading.
- **Never lose work.** Unsaved changes are kept privately on your own computer every few seconds. If someone changed the page while you were writing, Cairn shows both versions instead of overwriting either one.
- **Pictures** by button, paste, or drag and drop. They are stored next to the page in a normal folder.
- **Earlier versions** of every page are kept and can be restored.
- **Rename, move, and delete** pages and folders. Links between pages are updated for you, and anything deleted can be brought back from **Recently deleted**.
- **Team templates** for pages you write often, kept with each documentation folder, with fields such as the title and today's date filled in for you.
- **Download** a page as PDF or Markdown, or a whole folder or all documentation as a .zip. PDFs are made on your own computer.
- **Your timezone.** Times are stored in one standard form and shown in each person's timezone, marked like “2:30 PM GMT+8”.
- **Updates itself.** Cairn tells you when a new version is out and installs it with one click, after checking it's signed by Cairn's makers. You can always go back to the version before.

## Quick start

1. Download Cairn from the [Releases page](https://github.com/thekovie/cairn/releases/latest):
   - **Windows:** `cairn-…-setup.exe`. Run it; no administrator password is needed.
   - **Mac:** `cairn-…-macos-universal.zip`. Open it and drag **Cairn** to your Applications folder. The first time, choose **Open Anyway** in System Settings → Privacy & Security ([how](docs/getting-started.md#mac)).
   - **Linux:** `cairn-…-linux-x86_64.AppImage`. Double-click it, or run it from a terminal.

   Cairn is installed just for you and can update itself. (Or build it yourself: see [Building Cairn](docs/building.md).)
2. Start **Cairn**. A small window opens (a Terminal window on a Mac or Linux) and your browser shows Cairn.
3. Choose **Create a new documentation folder** (or **Open a documentation folder we already use**).
4. Choose **New page**, give it a title, and start writing. Choose **Publish changes** when you're done.

Keep the small Cairn window open while you use Cairn. Closing it stops Cairn.

The full walkthrough is in [docs/getting-started.md](docs/getting-started.md).

## Screenshots

The home screen: the page tree, search, folders, and recently changed pages with who edited them.

![Cairn home screen with the page tree, folders, and recently changed pages](docs/screenshots/home.png)

A page: its place in the tree, its owner, status, and who last edited it, with "On this page" beside it.

![A page in Cairn with the page tree on the left and "On this page" on the right](docs/screenshots/page.png)

Visual editing: write on the page as it will look. Markdown shortcuts such as `## ` and `- ` format as you type.

![Visual editing in Cairn, with the page editable as it will look](docs/screenshots/visual.png)

Or show the formatting codes, with a live preview beside them.

![The Cairn editor showing formatting codes with a live preview](docs/screenshots/editor.png)

<details>
<summary>More screenshots: templates, downloads, and timezones</summary>

Team templates, kept with each documentation folder.

![The Templates page listing team and built-in templates](docs/screenshots/templates.png)

Download a page as a PDF or as Markdown.

![The download dialog with PDF, Markdown with pictures, and Markdown choices](docs/screenshots/download.png)

Choose the timezone times are shown in.

![Settings: Time and timezone, with Asia and Manila selected](docs/screenshots/timezone.png)

</details>

The screenshots use made-up sample content.

## Documentation

| Guide | What it covers |
| --- | --- |
| [Getting started](docs/getting-started.md) | Installing, opening or creating a documentation folder, your first page |
| [Writing pages](docs/authoring.md) | Formatting, pictures, unsaved changes, publishing, earlier versions |
| [Architecture](docs/architecture.md) | How the local program, the browser, and the files fit together |
| [Storage and concurrency](docs/storage-and-concurrency.md) | The marker file, edit locks, time-outs, conflicts, recovery, and filesystem limits |
| [Configuration](docs/configuration.md) | Settings and the workspace file format |
| [Troubleshooting](docs/troubleshooting.md) | Shared-folder access, failed publishing, abandoned locks, restoring unsaved changes |
| [Two-computer test checklist](docs/manual-two-computer-checklist.md) | Manual checks for network-share behavior |
| [Building Cairn](docs/building.md) | Step by step: from a new computer to your own build of Cairn |
| [Contributing](CONTRIBUTING.md) | Running, testing, and working on the code |
| [Changelog](CHANGELOG.md) | What changed in each release |

## Feedback

Found a problem, have an idea, or got stuck? [Open an issue](https://github.com/thekovie/cairn/issues/new/choose) and pick the kind that fits. Plain words are fine; please leave out anything private from your documentation. Security problems go [privately](SECURITY.md) instead.

## Limitations

- **Systems.** Cairn runs on Windows 10/11 (x64), macOS 11 or newer (Apple silicon and Intel), and 64-bit Linux (x86_64). On other systems it can be built from source, but it doesn't update itself there.
- **Edit locks need suitable storage.** Locks and safe replacement rely on exclusive file creation and atomic rename, which local disks (NTFS, APFS, ext4) and SMB2/SMB3 network shares provide. Cloud-synced folders (OneDrive, Dropbox, Google Drive) and some network storage do not. Cairn checks the storage when a folder is opened and warns when editing by several people at once is not safe. See [storage and concurrency](docs/storage-and-concurrency.md).
- **Cairn only knows about Cairn.** If someone edits a `.md` file directly in another program, Cairn can't show them as the editor. It does notice the change when someone tries to publish, and shows a comparison instead of overwriting.
- **Access control is the filesystem's job.** Cairn hides editing where you can't write, but the real protection is the shared folder's own permissions.
- **Pictures:** PNG, JPEG, WebP, and GIF only. SVG is not accepted.
- **No viewer presence.** Cairn does not show who is currently reading a page.
- **Unsigned download.** Cairn isn't signed with a paid certificate, so Windows SmartScreen may warn the first time you run it (choose **More info → Run anyway**), and a Mac asks you to allow it once in System Settings → Privacy & Security. Each download has a `.sha256` next to it to check it against.

## License

[MIT](LICENSE). The bundled Inter font is under the SIL Open Font License (`assets/fonts/Inter-LICENSE.txt`).
