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

**Discovery.** When you choose a folder, Cairn checks it and then each parent folder for a marker, stopping at the top of the drive (`C:\`) or network share (`\\server\share`). It never scans a whole drive, never looks sideways into other folders, and shows you the name and location before opening anything. If it finds a marker it can't read, it stops there instead of continuing upward to a different workspace.

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
- The page path is lower-cased before hashing, because Windows paths ignore case.
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
- If the same Windows user on the same computer starts Cairn again, and the old lock has had no heartbeat for 90 seconds, they are offered **Continue where you left off**. It's their own lock, and their draft is on that computer.
- Anyone else needs a **maintainer** to release it. See [Troubleshooting → Abandoned edit locks](troubleshooting.md#abandoned-edit-locks).

## Private drafts

Unsaved changes are written to the editor's own computer (`%LOCALAPPDATA%\Cairn\drafts`) within about 10 seconds of the last change (normally 2.5 seconds), including pictures that have not been published yet. They are never written into the shared folder. A draft is deleted only after a verified publish, or when the user chooses **Discard my changes**. If persistent drafts are turned off, or the drafts folder can't be written, drafts live only in the running program and the editor says so.

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

**How replacement works on Windows.** Rust's `std::fs::rename` replaces an existing file atomically (MoveFileEx with replace-existing semantics), both locally and over SMB. The new file gets its permissions from the folder it's in. If an individual page file had custom permissions different from its folder, they are not carried over, so set permissions on folders instead.

## Filesystem requirements and limits

Cairn's guarantees rest on two operations: **exclusive file creation** and **atomic rename-over**.

| Storage | Status |
| --- | --- |
| Local NTFS drive | Supported |
| Windows file server share (SMB 2 or 3) | Supported. This is the intended setup for teams. |
| NAS appliances over SMB 2/3 | Usually fine; check the storage probe result |
| SMB 1 | Not supported (unreliable, and disabled on modern Windows) |
| OneDrive, Dropbox, Google Drive, iCloud synced folders | **Not safe for editing by several people.** Sync services can create conflict copies, restore deleted lock files, or apply changes late. Fine for one person. |
| Mapped drives to non-Windows servers | Varies; rely on the storage probe |

**The storage probe.** Each time a folder is opened, Cairn tests exclusive creation and rename-over in `_system/.probe` and checks whether the path looks like a cloud-synced folder. If anything fails, a banner says **Editing by several people at once is not safe in this folder**, and Settings shows the reason. The probe can't prove that storage is safe under every condition; it catches the common problems.

**Offline caches.** Windows "Offline Files" (client-side caching) can make a share look writable while disconnected. Turn it off for the documentation share.

**Clocks.** Lock times are shown using each computer's clock. Lock safety doesn't depend on clocks, but the "possibly left open" label does. Keep clocks synchronized (the Windows default).
