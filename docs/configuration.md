# Configuration

## Settings screen

Most people only need **Settings** in Cairn:

| Setting | What it does | Default |
| --- | --- | --- |
| Your name | Shown to others while you edit. Empty means your Windows user name. | empty |
| Colors | Light, Dark, or High contrast | Light |
| Text size | Normal (16 px), Large (18 px), or Larger (20 px). Works together with browser zoom. | Normal |
| Ask “Are you still editing?” after | Minutes without editing activity | 15 |
| Unlock the page after | Minutes without editing activity; must be later than the question | 20 |
| Keep my unsaved changes on this computer | Save drafts to disk so they survive closing Cairn | On |
| Editor toolbar | Icons only (names on hover and keyboard focus) or Icons and words | Icons only |
| Time and timezone | Automatic (this computer's timezone) or a chosen region and city. All times shown are converted to it. | Automatic |
| PDF downloads: Paper size | A4 or US Letter | A4 |
| Name of this documentation | The workspace display name, shown to everyone | set at creation |
| Download everything | Save every page, picture, and template as one .zip (Markdown or PDFs) | |
| Open a different documentation folder | Close this folder and choose another | |

Settings are personal and apply to your computer only. The one exception is the documentation name, which is stored in the shared marker file.

## Settings file

Settings are stored in `%LOCALAPPDATA%\Cairn\config.json`:

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
  "pdf_browser": null
}
```

| Field | Allowed values |
| --- | --- |
| `display_name` | Up to 60 characters, or `null` |
| `last_workspace` | Folder opened automatically at start-up, or `null` |
| `idle_warning_minutes` | 1–1440 |
| `idle_release_minutes` | Greater than `idle_warning_minutes`, at most 1440 |
| `persistent_drafts` | `true` or `false`. Set `false` where storing drafts on disk isn't allowed. |
| `draft_dir` | Folder for private drafts, or `null` for `%LOCALAPPDATA%\Cairn\drafts`. It should be private to the user. |
| `max_image_mb` | 1–50 |
| `max_image_dimension` | 100–20000 (pixels, width or height) |
| `appearance` | `light`, `dark`, `contrast` |
| `text_size` | `normal`, `large`, `larger` |
| `timezone` | An IANA timezone name such as `"Asia/Manila"` or `"America/New_York"`, or `null` for this computer's timezone. Only affects how times are shown. |
| `pdf_paper` | `a4` or `letter` |
| `toolbar_labels` | `true` shows words beside the editor's toolbar icons; `false` (default) shows them on hover and keyboard focus |
| `pdf_browser` | Full path to `msedge.exe` or `chrome.exe` used to make PDFs, or `null` to find Microsoft Edge, then Google Chrome, in their usual places. If the path doesn't exist, PDFs fall back to the print view. |

If the file is missing, Cairn uses the defaults. If it's invalid, Cairn uses the defaults and shows a notice. An administrator can pre-deploy this file, for example to turn off persistent drafts.

**Nothing in this file is a security control.** Anyone can edit their own settings. What people can read or change is decided by the shared folder's Windows permissions.

### Environment variable

`CAIRN_HOME` moves the whole settings folder (settings and, by default, drafts). This is useful for portable setups and for testing:

```
set CAIRN_HOME=D:\CairnProfile
cairn.exe
```

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

Other fields are preserved when Cairn rewrites the file.

Inside the workspace:

| Path | Meaning |
| --- | --- |
| `**/*.md` | Pages, at any folder depth |
| `<page>.assets/` | Pictures belonging to `<page>.md` |
| `_templates/*.md` | This documentation's team templates (not shown as pages or in search) |
| `_system/locks/` | Edit locks (don't edit these by hand; use `cairn locks`) |
| `_system/history/` | Earlier versions of pages |
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
