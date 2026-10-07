# Troubleshooting

## Cairn doesn't open in the browser

- Look for the small Cairn window. It shows an address like `http://127.0.0.1:52341/#t=…`. Copy the **whole** address into your browser.
- If the browser says **Please open Cairn from its program window**, the tab isn't connected to the running Cairn (for example, it was bookmarked during an earlier run). Close it and use the address from the Cairn window. Every start uses a new address, for security.
- If the Cairn window closes immediately, open a Command Prompt (Terminal on a Mac or Linux), run Cairn from there (`cairn.exe`; on a Mac `/Applications/Cairn.app/Contents/MacOS/cairn`; on Linux the AppImage or `./cairn`), and read the message.
- **On a Mac, nothing happens or it says Cairn can't be opened:** the first time, choose **Open Anyway** in System Settings → Privacy & Security (see [Getting started](getting-started.md#mac)).

## The shared folder can't be reached

**“The documentation folder used last time isn't available”** at start-up usually means the network drive isn't connected.

1. Open the folder in File Explorer (Finder on a Mac, your file manager on Linux). If it asks for a password, enter it; on a Mac, a shared drive may need connecting again with **Go → Connect to Server**.
2. In Cairn, choose **Try it again**.
3. If the folder moved, choose **Open a documentation folder we already use** and pick the new location.

## “You can read this page but you don't have permission to change it”

Cairn tried to create a file in that folder and the computer refused. Cairn doesn't decide who may edit; the shared folder's permissions do. Ask whoever manages the share to give you **Modify** permission on the folder (and on `_system`, which holds edit locks).

If you *can* write there, check that the page's file isn't marked **Read-only** (right-click it → Properties).

## Publishing failed

The message says what went wrong. In every case **the published page was not changed and your text is kept**.

| Message | What to do |
| --- | --- |
| Someone else changed this page while you were writing | Compare the versions, copy what you need, then publish. See [Writing pages](authoring.md#when-someone-else-changed-the-page). |
| This page was unlocked while you were away | Choose **Continue editing**, then **Publish changes** again. |
| *Name* is editing this page now | Someone took the page after it was unlocked. Wait for them to finish, then continue. Your text is kept on your computer. |
| You don't have permission… | See the section above. |
| The picture … could no longer be found | Remove the picture from the text, insert it again, and publish. |
| A file operation failed, or the page could not be verified | Usually a network interruption. Check that the folder is reachable and try again. |

## “Editing by several people at once is not safe in this folder”

Cairn tested the storage when the folder was opened and something didn't behave safely, or the folder looks like a cloud-synced folder (OneDrive, Dropbox, Google Drive). One person can still use it. For a team, move the documentation to a regular Windows file share. See [Storage and concurrency](storage-and-concurrency.md#filesystem-requirements-and-limits).

## Abandoned edit locks

A page can stay locked if the editor's computer crashed, lost power, or was disconnected while editing.

**If it was you, on the same computer:** start Cairn again and open the page. Choose **Continue where you left off**. Your unsaved changes are still there.

**If it was someone else:**

1. **Ask them first.** They may be editing right now, or have unsaved changes on their computer.
2. If they can start Cairn on that same computer and open the page, they get their lock and changes back, and can publish or close the editor.
3. Otherwise a maintainer can release the lock from a Command Prompt (Terminal on a Mac or Linux; give the folder as a path there, such as `/Volumes/Shared/Documentation`):

   ```
   cairn locks list "\\server\shared\Documentation"
   ```

   This shows each locked page, who has it, their session id, and when they were last active. A lock is marked `POSSIBLY ABANDONED` after 5 minutes without a heartbeat.

   ```
   cairn locks release "\\server\shared\Documentation" "Guides/printer.md" --session 6f1c…
   ```

   Cairn refuses unless:
   - the session id matches the lock that's there right now, so you can't release a lock someone has just taken; and
   - the lock has had no heartbeat for at least 5 minutes, so you can't cut off someone who is still editing.

   A record of the released lock is kept in `_system/locks/released/`.

**What happens to their unsaved changes?** Nothing. They are stored on the editor's own computer, not in the shared folder. When that person next edits the page, Cairn offers to restore them and shows the differences. If someone published in the meantime, they are shown both versions before anything is overwritten.

Never delete files in `_system/locks/` by hand while anyone might be using Cairn.

## Restoring unsaved changes

When you open a page you had unsaved changes for, Cairn asks **Continue with your unsaved changes?**

- **Continue with my changes** brings back your text and any pictures you added.
- **Start again from the published page** removes your unsaved changes (after asking you to confirm).
- **Show how my changes differ from the published page** shows the differences first.

If Cairn didn't offer this:

- **Were persistent drafts on?** Check **Settings → Unsaved changes**. If it was off, changes are lost when Cairn closes.
- **Same computer and user?** Drafts are stored per user, per computer, in `drafts` in Cairn's [settings folder](configuration.md#settings-file).
- **Same documentation folder?** Drafts belong to a specific documentation folder (its `instance_id`). A copy of the folder with a new marker counts as a different folder.

## A page shows “Some page details couldn't be read”

The information block at the top of the page has a mistake in it. Open the page for editing, then use **Page details** to set the values again. The page itself is fine.

## Links show “(missing)”

The link points to a page or picture that doesn't exist. Common reasons:

- The page was deleted. Look in **Recently deleted** and choose **Restore**.
- The page was renamed or moved in File Explorer (Cairn updates links only when this is done in Cairn).
- It was renamed or moved in Cairn while someone was editing this page, so this page couldn't be updated.

Edit the page and fix the link with **Link…**.

## “PDFs can't be made automatically on this computer”

Cairn makes PDFs with Microsoft Edge, Google Chrome, or (on a Mac or Linux) Chromium. This message means none was found, or none worked. Some computers have tools that send every Edge launch to another browser, which stops Edge from making PDFs.

- Choose **Open the print window instead**, then **Save as PDF** (or, on Windows, **Microsoft Print to PDF**) as the printer.
- Installing Google Chrome usually makes one-click PDFs work.
- An administrator can point Cairn at a specific browser with `pdf_browser` in the [settings file](configuration.md#settings-file).

Markdown downloads always work.

## Times look wrong

Check **Settings → Time and timezone**. Times are shown in the timezone chosen there, marked with their difference from GMT (for example “GMT+8”). If it says Automatic, check the computer's timezone (Windows: **Settings → Time & language**; Mac: **System Settings → General → Date & Time**).

## Where are the log files?

Cairn prints problems in its window. It doesn't write log files or send reports anywhere.
