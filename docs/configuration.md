# Configuration

## Settings screen

Most people only need **Settings** in Cairn:

| Setting | What it does | Default |
| --- | --- | --- |
| Your name | Shown to others while you edit. Empty means your user name on this computer. | empty |
| Colors | Light, Dark, or High contrast | Light |
| Text size | Normal (16 px), Large (18 px), or Larger (20 px). Works together with browser zoom. | Normal |
| Ask “Are you still editing?” after | Minutes without editing activity | 15 |
| Unlock the page after | Minutes without editing activity; must be later than the question | 20 |
| Keep my unsaved changes on this computer | Save drafts to disk so they survive closing Cairn | On |
| Editor toolbar | Icons and words, or Icons only (names on hover and keyboard focus) | Icons and words |
| Time and timezone | Automatic (this computer's timezone) or a chosen region and city. All times shown are converted to it. | Automatic |
| PDF downloads: Paper size | A4 or US Letter | A4 |
| Name of this documentation | The workspace display name, shown to everyone | set at creation |
| Download everything | Save every page, picture, and template as one .zip (Markdown or PDFs) | |
| Open a different documentation folder | Close this folder and choose another | |
| Updates: Look for new versions | Every day, or only when you choose **Check now** | Every day |
| Updates: Proxy server | Address and port of the proxy your office uses to reach the internet, such as `proxy.office.local:8080`. Empty means Windows' proxy settings on Windows; on a Mac or Linux, no proxy (or the `HTTPS_PROXY` environment variable). | empty |

Settings are personal and apply to your computer only. The one exception is the documentation name, which is stored in the shared marker file.

## Settings file

Settings are stored in `config.json` in Cairn's settings folder, which only you can open:

| System | Settings folder |
| --- | --- |
| Windows | `%LOCALAPPDATA%\Cairn` |
| Mac | `~/Library/Application Support/Cairn` |
| Linux | `~/.local/share/Cairn` (or `$XDG_DATA_HOME/Cairn`) |

For example:

```json
{
  "display_name": "Alex",
  "last_workspace": "\\\\server\\shared\\Documentation",
  "idle_warning_minutes": 15,
  "idle_release_minutes": 20,
  "persistent_drafts": true,
  "draft_dir": null,
  "max_image_mb": 10,
  "max_image_dimension": 8000,
  "appearance": "light",
  "text_size": "normal",
  "timezone": null,
  "pdf_paper": "a4",
  "pdf_browser": null,
  "update_check": "daily",
  "update_proxy": null
}
```

| Field | Allowed values |
| --- | --- |
| `display_name` | Up to 60 characters, or `null` |
| `last_workspace` | Folder opened automatically at start-up, or `null` |
| `idle_warning_minutes` | 1–1440 |
| `idle_release_minutes` | Greater than `idle_warning_minutes`, at most 1440 |
| `persistent_drafts` | `true` or `false`. Set `false` where storing drafts on disk isn't allowed. |
| `draft_dir` | Folder for private drafts, or `null` for `drafts` in the settings folder. It should be private to the user. |
| `max_image_mb` | 1–50 |
| `max_image_dimension` | 100–20000 (pixels, width or height) |
| `appearance` | `light`, `dark`, `contrast` |
| `text_size` | `normal`, `large`, `larger` |
| `timezone` | An IANA timezone name such as `"Asia/Manila"` or `"America/New_York"`, or `null` for this computer's timezone. Only affects how times are shown. |
| `pdf_paper` | `a4` or `letter` |
| `toolbar_labels` | `true` shows words beside the editor's toolbar icons; `false` (default) shows them on hover and keyboard focus |
| `pdf_browser` | Full path to the browser used to make PDFs (`msedge.exe` or `chrome.exe` on Windows; on a Mac the app, such as `/Applications/Google Chrome.app`; on Linux the program, such as `/usr/bin/chromium`), or `null` to find Microsoft Edge, Google Chrome, or Chromium in their usual places. If the path doesn't exist, PDFs fall back to the print view. |
| `update_proxy` | `"host:port"` (for example `"proxy.office.local:8080"`), or `null` to use Windows' proxy settings (on a Mac or Linux, the `HTTPS_PROXY` environment variable if set). Used only to reach GitHub for updates. |
| `update_check` | `daily` (look for a new version once a day) or `manual` (only when someone chooses **Check now**). An administrator can pre-deploy `manual` where computers must not go online on their own. |

If the file is missing, Cairn uses the defaults. If it's invalid, Cairn uses the defaults and shows a notice. An administrator can pre-deploy this file, for example to turn off persistent drafts.

**Nothing in this file is a security control.** Anyone can edit their own settings. What people can read or change is decided by the shared folder's permissions.

### Environment variable

`CAIRN_HOME` moves the whole settings folder (settings and, by default, drafts). This is useful for portable setups and for testing:

```
set CAIRN_HOME=D:\CairnProfile
cairn.exe
```

On a Mac or Linux: `CAIRN_HOME=~/cairn-profile ./cairn`.

## Updates

Cairn can tell you about new versions and install them itself.

- **Checking.** With **Every day** (the default), Cairn asks GitHub once a day, starting a minute after it opens, whether a newer version is out. **Settings → Updates → Check now** asks straight away. The request only asks for the latest version; nothing about you or your documentation is sent. It uses the computer's trusted certificates, and on Windows its proxy settings, so it works on most office networks. If your office reaches the internet through a proxy server that Cairn doesn't pick up (on a Mac or Linux, any proxy; on Windows, one set by a setup script), type it in **Settings → Updates → Proxy server** as address and port, for example `proxy.office.local:8080`. Proxies that ask for a login aren't supported. If the computer is offline, Cairn tries again a few hours later and says so in Settings.
- **When there's a new version,** a notice appears at the top, and **Settings → Updates** shows what's new. **Update and restart** downloads it, checks it, installs it, and restarts Cairn. The browser tab reconnects by itself after a few seconds. Publish or close any page you're editing first; your unsaved changes are kept either way.
- **Safety.** Every download is signed by Cairn's release process. Cairn installs a download only if its signature matches the key built into Cairn, and only if it really is the newer version. Anything else is refused and nothing changes.
- **Going back.** The version you had before is kept next to Cairn's program as `cairn.previous.exe` (`cairn.previous` on a Mac or Linux). **Settings → Updates → Go back to …** switches back (and keeps the newer one, so you can switch again).
- **Where Cairn updates itself.** Wherever the person can change the program's folder: the installer's folder (`%LOCALAPPDATA%\Programs\Cairn`) or an unzipped copy of their own on Windows; **Cairn** in the Applications folder on a Mac (not run straight from Downloads, where the Mac runs it from a read-only copy); the AppImage, or the unpacked `cairn`, in a folder of your own on Linux. On a Mac or Linux the update replaces the program while Cairn keeps running in the same Terminal window. A Cairn run **from a network drive** doesn't update itself, because that would change it for everyone using that copy at once; it shows the notice with a link to the download page, and whoever looks after the shared copy replaces `cairn.exe` there. The same goes for a folder the person can't change, or a copy run from inside a zip.

## Command line

```
cairn [--no-browser] [--port <n>] [--workspace <folder>]
cairn init <folder> --name <name>
cairn locks list <workspace-folder>
cairn locks release <workspace-folder> <page-path> --session <session-id>
cairn --help
cairn --version
```

| Option | Meaning |
| --- | --- |
| `--no-browser` | Don't open the browser; copy the printed address instead |
| `--port <n>` | Use a fixed local port instead of a random free one |
| `--workspace <folder>` | Open this documentation folder, or the one it belongs to |
| `init` | Create documentation in a new or empty folder |
| `locks list`, `locks release` | Maintainer tools; see [Troubleshooting](troubleshooting.md#abandoned-edit-locks) |

## Workspace format (schema version 1)

A workspace is a folder with `shared-docs.json` at its root:

| Field | Type | Meaning |
| --- | --- | --- |
| `kind` | string | Always `"shared-docs"` |
| `schema_version` | integer | `1` for this release. Higher values open read-only. |
| `instance_id` | UUID string | Generated once at creation; never changes |
| `display_name` | string | Name shown in Cairn |
| `version_cleanup` | object, optional | Whether old earlier versions are removed: `{ "enabled": false, "keep_newest": 3, "older_than_days": 30 }`. Set in **Settings → Earlier versions**. Missing or invalid means off: every version is kept. `keep_newest` is 1–100, `older_than_days` 1–3650. |

Other fields are preserved when Cairn rewrites the file.

Inside the workspace:

| Path | Meaning |
| --- | --- |
| `**/*.md` | Pages, at any folder depth |
| `<page>.assets/` | Pictures belonging to `<page>.md` |
| `_templates/*.md` | This documentation's team templates (not shown as pages or in search) |
| `_system/locks/` | Edit locks (don't edit these by hand; use `cairn locks`) |
| `_system/history/` | Earlier versions of pages. All are kept unless `version_cleanup` is turned on. |
| `_system/edited/` | Who last published each page, and a fingerprint of what they published |
| `_system/trash/` | Recently deleted pages and folders, restorable in Cairn. Deleting a folder in here removes it for good. |
| `_system/.probe/` | Temporary files from the storage check |
| Files and folders starting with `.` | Ignored |

### Page front matter

An optional block at the very top of a page:

```
---
owner: Sam
status: active          # draft | active | retired
last_reviewed: 2026-01-31
tags: [printers, office]
---
```

Every field is optional. Values that can't be understood are ignored with a notice on the page; the page itself always opens. The page title is the first level-1 heading (`# Title`). Without one, the file name is used.

`last_reviewed` is a calendar date and is never shifted by timezone.

### Template front matter

A team template is a page in `_templates/` with two extra fields, which are removed from pages made from it:

```
---
template_name: Meeting notes
template_description: Agenda, attendees, decisions, and action items.
status: draft           # any other fields become the new page's defaults
---
# {{title}}

Held on {{date}} by {{author}} in {{folder}}.
```

When a page is created, `{{title}}` becomes its title, `{{date}}` today's date in the creator's timezone (YYYY-MM-DD), `{{author}}` the creator's name, and `{{folder}}` the folder it is created in.
