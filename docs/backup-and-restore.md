# Back up and restore

Cairn can do two different things with your documentation as a file. They are not the same.

| | **Download everything** | **Back up workspace** |
| --- | --- | --- |
| Where | Settings → Download everything | Settings → Back up workspace, or `cairn backup create` |
| For | Sharing, or moving pages to another tool | Bringing the documentation back after a lost or damaged folder |
| Pages, pictures, templates | Yes | Yes |
| Earlier versions of pages | No | Yes |
| Deleted pages (Recently deleted) | No | Yes |
| Who last published each page | No | Yes |
| Checked when used | No | Yes: every file has a checksum |
| Can be restored by Cairn | No | Yes |

If the shared folder is also backed up by your IT team, that backup already holds everything above, because Cairn keeps all of its records as ordinary files in the folder. A Cairn backup is for when you want a copy of your own, or want to check that a copy is whole.

## What a backup holds

One `.zip` file containing:

- every page, picture, and template, with the same folders;
- earlier versions, deleted pages, and the record of who published each page (the `_system` folder, except edit locks);
- `shared-docs.json`, which gives the documentation its name and identity;
- `cairn-backup.json`, a list of every file with its size and SHA-256 checksum, the Cairn version, and the date.

It leaves out:

- **Edit locks.** They describe who is editing right now, not the documentation.
- **Unpublished changes.** Those are saved privately on each person's own computer and are never in the shared folder.
- Hidden files and temporary files, and anything reached through a shortcut or link. Links that were left out are listed when the backup finishes.

A backup is not encrypted. Keep it somewhere only the right people can open, and not in the shared documentation folder itself.

## Making a backup

In Cairn, open **Settings**, choose **Back up workspace**, then **Back up**. A progress bar shows how far along it is, and **Close** stops it. The file goes to your Downloads folder.

To make backups on a schedule, use the command line. This needs a new file name each time, because Cairn never writes over an existing file:

```text
cairn backup create "\\server\Docs" "D:\Backups\Docs-2026-10-13.zip"
```

On Windows, Task Scheduler can run this with a date in the name:

```text
powershell -Command "& 'C:\Program Files\Cairn\cairn.exe' backup create '\\server\Docs' ('D:\Backups\Docs-' + (Get-Date -Format yyyy-MM-dd) + '.zip')"
```

(Use the folder where Cairn is installed.) The command runs with your own Windows permissions, like Cairn itself.

### What if people are editing?

Cairn does not lock the shared folder, and it cannot take a perfect snapshot of it. Instead:

1. It copies each file in one go and checks that the file didn't change while it was being read. Pages saved in Cairn are replaced whole, never half written, so each copy is a complete version.
2. If a file changed while it was being copied (an editor saving it in place, for example), or disappeared before it was reached, Cairn starts over with a fresh list. It tries up to three times.
3. If files still keep changing or disappearing, it stops, deletes the unfinished file, and names them. Try again when fewer people are editing.
4. A page that someone saves, creates, or deletes *after* it was copied doesn't stop the backup. The backup holds each file as it was when copied, and Cairn lists the files that changed while it ran, so you know the backup may mix slightly older and newer versions.

If Cairn cannot read a folder or file (no permission, or the shared drive drops out), the backup also stops and names it. A backup is never handed over with files quietly missing.

## Checking a backup

```text
cairn backup verify Docs-2026-10-13.zip
```

This reads the whole file and checks every checksum. Do this after copying a backup to another place, and now and then for old ones.

## Restoring a backup

Restoring is for an administrator replacing a lost or damaged documentation folder.

```text
cairn restore Docs-2026-10-13.zip "D:\Restored Docs"
```

Cairn:

- checks the entire backup first (names, sizes, checksums), while writing into a hidden folder next to the destination;
- only if everything is correct, moves the result into place;
- refuses a backup that was damaged, edited by hand, made by a newer Cairn, or that contains unsafe paths or links;
- refuses a destination that already has files in it, or that is inside another documentation folder, so it can never write over existing work;
- leaves nothing behind when it refuses.

Then open the restored folder in Cairn from the start screen or **Settings → Open a different documentation folder**.

The restored folder keeps the identity of the original, so people's unpublished changes that are saved on their own computers find their pages again. Use a restore to replace a lost folder. Do not run the original and a restored copy side by side for the same team: they would share one identity.

A backup is an ordinary zip. To recover just one earlier version of one page, open the `.zip`, find it under `_system/history/<page path>/`, and copy it out.

## Limits

- **Checksums find damage, not tampering.** Anyone who can edit a backup can also fix up its checksums. Treat a backup from someone you don't trust as untrusted content: restoring it brings back whatever it holds, including a cloned workspace identity.
- **Restore keeps files and their contents, not empty folders or original “modified” times.** Pages show their last-reviewed date from their own front matter, which is kept.
- **The change check relies on the file server's timestamps.** Windows can cache file details from a share for several seconds, and some servers record times only to the second, so a very quick edit can be missed. That is why a backup is a careful copy, not a snapshot. Run backups when few people are editing.
- **Cairn checks a finished backup before keeping it.** It reads the file back and checks every checksum. If that fails (for example, two pages whose names differ only in capitals, which Windows and Mac can't hold together), no backup is kept and the message says why.
- On a shared Linux computer a backup in progress is readable only by you. Backups are not encrypted.
- Restore does not check free space first. Restore to a drive with room for the whole documentation.

## If something goes wrong

- **“kept changing while the backup was made”**: a file changed or disappeared while it was being copied, three times running. Try again in a quiet moment.
- **“can't reach … right now”**: the drive under the documentation went away (a network drive disconnected, or a USB drive was removed). Reconnect it and try again. No unfinished file is left behind.
- **“isn't allowed to read” or “couldn't read”**: you don't have access to that folder, or the shared drive is offline. Fix access, then try again.
- **“doesn't match its checksum” or “damaged or incomplete”**: that copy of the backup is damaged. Use another copy.
- **“made by a newer Cairn”**: update Cairn, then restore.
- **“has no cairn-backup.json”**: this is not a backup. A zip from “Download everything” cannot be restored.
