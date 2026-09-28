# Configuration

## Settings screen

Most people only need **Settings** in Cairn:

| Setting | What it does | Default |
| --- | --- | --- |
| Your name | Shown to others while you edit. Empty means your Windows user name. | empty |
| Colors | Light, Dark, or High contrast | Light |
| Text size | Normal (18 px), Large (20 px), or Larger (23 px). Works together with browser zoom. | Normal |
| Ask “Are you still editing?” after | Minutes without editing activity | 15 |
| Unlock the page after | Minutes without editing activity; must be later than the question | 20 |
| Keep my unsaved changes on this computer | Save drafts to disk so they survive closing Cairn | On |
| Name of this documentation | The workspace display name, shown to everyone | set at creation |
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
  "text_size": "normal"
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
| `_system/locks/` | Edit locks (don't edit these by hand; use `cairn locks`) |
| `_system/history/` | Earlier versions of pages |
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
