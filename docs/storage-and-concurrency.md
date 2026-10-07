# Storage and concurrency

This document explains exactly how Cairn keeps several people from damaging each other's work, and where those guarantees stop.

## The workspace marker

A folder is Cairn documentation if its root contains `shared-docs.json`:

```json
{
  "kind": "shared-docs",
  "schema_version": 1,
  "instance_id": "3f2b8c1e-0000-4000-8000-000000000000",
  "display_name": "My Documentation"
}
```

- `instance_id` is generated once, when the folder is set up, and never changes. Private drafts are filed under it, so drafts for two different documentation folders never mix.
- The marker never holds credentials, roles, or drafts.
- Renaming the documentation rewrites only `display_name`; unknown fields are kept.

**Discovery.** When you choose a folder, Cairn checks it and then each parent folder for a marker, stopping at the top of the drive (`C:\`) or network share (`\\server\share`), or on a Mac or Linux at the top of the disk or share it's on (such as `/Volumes/Shared`). It never scans a whole drive, never looks sideways into other folders, and shows you the name and location before opening anything. If it finds a marker it can't read, it stops there instead of continuing upward to a different workspace.

**Newer formats.** If `schema_version` is higher than this version of Cairn understands (currently `1`), the folder opens read-only with an explanation.

**Setup is safe to repeat and safe to race.**

1. Cairn refuses a folder that already contains other files, unless you ask it to create a new folder inside.
2. It creates `.shared-docs-init.json` using exclusive creation. That file fixes the `instance_id`. If two people set up the same folder at the same moment, only one creation succeeds and the other adopts its id.
3. It creates the starter folders and `README.md`, never overwriting anything.
4. It writes `shared-docs.json` last, again with exclusive creation, and then removes the journal.

If setup is interrupted, running it again reuses the same journal and finishes the job with the same id.

## Edit locks

Reading a page never takes a lock. Choosing **Edit this page** tries to create `_system/locks/<hash of page path>.lock`:

```json
{
  "session_id": "per-launch id",
  "display_name": "Alex",
  "os_user": "alex",
  "host": "ALEX-PC",
  "article": "Guides/printer.md",
  "created_at": 1790000000,
  "heartbeat_at": 1790000030
}
```

- The file is created with **exclusive creation**: a fully written temporary file is hard-linked into place, which fails if the lock already exists. Exactly one client wins, and nobody ever sees a half-written lock. (If the storage doesn't support hard links, Cairn falls back to exclusive create-then-write.)
- The page path is lower-cased before hashing, because Windows (and Macs, by default) ignore case in file names. On disks where case matters (Linux), `a.md` and `A.md` therefore share one lock: harmless, just cautious.
- While editing, Cairn refreshes `heartbeat_at` every **30 seconds** by atomically replacing the file. The lock file exists at every moment, so nobody can slip in.
- Other people see “Being edited by Alex since 2:30 PM” and cannot enter the editor.
- Different pages have different lock files, so people edit different pages at the same time freely.
- Publishing checks that you still hold the lock.

Cairn can only name editors that use Cairn. Someone editing a `.md` file in Notepad is invisible to locks; the pre-publish check described below catches their change.

## Idle time-out

| After this long without editing activity | What happens |
| --- | --- |
| 15 minutes (setting `idle_warning_minutes`) | “Are you still editing?” with **Keep editing** and **Unlock the page now** |
| 20 minutes (setting `idle_release_minutes`) | The lock is released. The text stays in the editor and in the private draft. |

Activity means typing, pasting, inserting a picture, or using an editor button. Having the page open or moving the mouse does not count.

After an idle release, **Publish changes** is disabled until the user chooses **Continue editing**, which takes the lock again (if nobody else has) and then goes through the normal conflict check. Nothing is published or discarded automatically.

## Crashed or disconnected clients

If Cairn stops without releasing a lock (power cut, crash, network cable pulled), the lock stays. **Cairn never takes over a lock because of its age.**

- After 5 minutes without a heartbeat, the lock is shown as possibly left open (“… but they haven't been active for a while”). It is still respected.
- If the same user on the same computer starts Cairn again, and the old lock has had no heartbeat for 90 seconds, they are offered **Continue where you left off**. It's their own lock, and their draft is on that computer.
- Anyone else needs a **maintainer** to release it. See [Troubleshooting → Abandoned edit locks](troubleshooting.md#abandoned-edit-locks).

## Private drafts

Unsaved changes are written to the editor's own computer (`drafts` in Cairn's [settings folder](configuration.md#settings-file), which only that user can open) within about 10 seconds of the last change (normally 2.5 seconds), including pictures that have not been published yet. They are never written into the shared folder. A draft is deleted only after a verified publish, or when the user chooses **Discard my changes**. If persistent drafts are turned off, or the drafts folder can't be written, drafts live only in the running program and the editor says so.

## Times are stored as UTC

Every time Cairn writes (lock `created_at` and `heartbeat_at`, draft `updated_at`, earlier-version names) is UTC, so computers in different timezones agree on every moment and on how old a lock is. Each person's timezone setting only changes how times are displayed. The one date-only value, `last_reviewed`, is a calendar date and is never shifted.

## Templates

Team templates are ordinary pages in `_templates/`, so they use the same edit locks, drafts, conflict checks, publishing sequence, and earlier versions as any page. Deleting a template requires its edit lock and an unchanged hash, and saves it to Earlier versions before removing the file, which is how **Restore** works.

## Renaming, moving, and deleting

These take the edit lock of every page involved (the page, or every page in the folder) before touching anything, and give them back when done. If anyone holds one of those locks, nothing changes and the person is told who is editing. A page Cairn has open in this person's own editor is refused too.

- **Moving a page** writes it at its new path with exclusive creation, moves its `.assets` folder, then removes the old file. If a step fails, the steps before it are undone. Its folder in `_system/history` moves with it.
- **Moving a folder** first adjusts links in its pages that point outside it (saving each page's previous text to Earlier versions), then renames the folder in one step. If the rename fails, those pages are put back.
- **Links in other pages** are then updated one page at a time, each like a publish: under that page's edit lock, only if it hasn't changed since it was read, and with its previous text saved to Earlier versions. A page someone is editing is skipped and reported, never overwritten.
- **Deleting** renames the page (with its `.assets`) or folder into `_system/trash/<id>/content/`, after writing `item.json` there. **Restore** renames it back, and refuses if something now exists at the original path.

This person's own unsaved changes to moved pages move with them. Unsaved changes on other people's computers stay under the old path: publishing them shows the usual conflict screen, because the page is no longer there.

## Conflicts

When editing starts, Cairn records a SHA-256 hash of the published page. Immediately before publishing it hashes the page again. If the hashes differ, for any reason (another Cairn user, Notepad, a restored backup), **nothing is written**. The user sees both versions and chooses whether to keep editing or to publish their version anyway. Either way the other version is preserved: it stays on disk, or it goes to Earlier versions.

## The publishing sequence

`src/publish.rs` performs these steps in order:

1. Check the lock, write permission on the folder, the text size, and every new picture (content check and full decode).
2. Re-hash the published page; stop with a conflict if it changed.
3. Write each new picture completely into `<page>.assets/` using exclusive creation, **before** any page refers to it.
4. Copy the current page into `_system/history/`.
5. Write the new text into a temporary file **in the same folder**, and flush it to disk.
6. Replace the page by renaming the temporary file over it. The live file is never truncated or opened for writing.
7. Read the page and the new pictures back and compare them byte for byte.
8. If anything fails: before step 6, the old page is untouched; after step 6, the old page is written back. Only then are the pictures created by this attempt removed. If the old page can't be put back, the new pictures are kept, so no page ever refers to a missing picture. The draft is kept in every case.

Each failure point is covered by an automated test (`tests/editing.rs`).

**How replacement works.** Rust's `std::fs::rename` replaces an existing file atomically: MoveFileEx with replace-existing semantics on Windows, `rename(2)` on a Mac or Linux, both locally and over SMB. The new file gets its permissions from the folder it's in. If an individual page file had custom permissions different from its folder, they are not carried over, so set permissions on folders instead.

## Filesystem requirements and limits

Cairn's guarantees rest on two operations: **exclusive file creation** and **atomic rename-over**.

| Storage | Status |
| --- | --- |
| Local drive (NTFS on Windows, APFS on a Mac, ext4 and similar on Linux) | Supported |
| Windows file server share (SMB 2 or 3) | Supported. This is the intended setup for teams. Macs and Linux computers can use the same share (on a Mac, **Go → Connect to Server**; on Linux, mounted with CIFS). |
| NAS appliances over SMB 2/3 | Usually fine; check the storage probe result |
| SMB 1 | Not supported (unreliable, and disabled on modern Windows) |
| OneDrive, Dropbox, Google Drive, iCloud synced folders | **Not safe for editing by several people.** Sync services can create conflict copies, restore deleted lock files, or apply changes late. Fine for one person. |
| Mapped drives to non-Windows servers, NFS | Varies; rely on the storage probe |

**The storage probe.** Each time a folder is opened, Cairn tests exclusive creation and rename-over in `_system/.probe` and checks whether the path looks like a cloud-synced folder. If anything fails, a banner says **Editing by several people at once is not safe in this folder**, and Settings shows the reason. The probe can't prove that storage is safe under every condition; it catches the common problems.

**Offline caches.** Windows "Offline Files" (client-side caching) can make a share look writable while disconnected. Turn it off for the documentation share.

**Capital letters in names.** Windows and Macs (by default) treat `Guide.md` and `guide.md` as the same file; Linux doesn't. Cairn refuses to rename a page or folder onto another one that differs only in capitals, so nothing is overwritten, but on a team that mixes systems, avoid names that differ only in capitals.

**Clocks.** Lock times are shown using each computer's clock. Lock safety doesn't depend on clocks, but the "possibly left open" label does. Keep clocks synchronized (the default on Windows, Macs, and most Linux).
