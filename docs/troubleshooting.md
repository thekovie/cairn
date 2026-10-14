# Troubleshooting

## Cairn doesn't open in the browser

- Look for the small Cairn window. It shows an address like `http://127.0.0.1:52341/#t=…`. Copy the **whole** address into your browser.
- If the browser says **Please open Cairn from its program window**, the tab isn't connected to the running Cairn (for example, it was bookmarked during an earlier run, or it was opened when no other Cairn tab was open). Close it and use the address from the Cairn window. Every start uses a new address, for security. While any Cairn tab is open, a new tab (a duplicated tab, a link opened in a new tab, or the pasted address) connects by itself.
- On Windows, if Cairn starts but you don't see anything, look for its icon in the notification area (behind the **^** arrow). If Cairn can't start at all, a message box says why. If it doesn't respond and there is no icon, end **cairn.exe** in Task Manager and start it again.
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

The link points to a page, file, or picture that Cairn can't find. To see every such link in the documentation at once, open **Documentation health** and choose **Check documentation** (see [Writing and editing](authoring.md#documentation-health)). Common reasons:

- The page was deleted. Look in **Recently deleted** and choose **Restore**.
- The page was renamed or moved in File Explorer (Cairn updates links only when this is done in Cairn).
- It was renamed or moved in Cairn while someone was editing this page, so this page couldn't be updated.
- The link goes to a heading that was renamed. The check says which heading is missing.
- The link goes outside the documentation folder, or through a shortcut (link or junction) to another folder. Cairn doesn't follow those.
- The picture is a type Cairn doesn't show, such as `.svg`. Use png, jpg, jpeg, gif, or webp.
- Capital letters differ. A link to `printer.md` finds `Printer.md` on Windows and most Macs but not on Linux, so it can work for some people and not others. Use the exact name.

Edit the page and fix the link with **Link…**.

## “PDFs can't be made automatically on this computer”

Cairn makes PDFs with Microsoft Edge, Google Chrome, or (on a Mac or Linux) Chromium. This message means none was found, or none worked. Some computers have tools that send every Edge launch to another browser, which stops Edge from making PDFs.

- Choose **Open the print window instead**, then **Save as PDF** (or, on Windows, **Microsoft Print to PDF**) as the printer.
- Installing Google Chrome usually makes one-click PDFs work.
- An administrator can point Cairn at a specific browser with `pdf_browser` in the [settings file](configuration.md#settings-file).

Markdown downloads don't need a browser, so they work without one.

## “Cairn isn't allowed to read …” or “… was removed while the download was being prepared”

Cairn stops a download instead of handing you a zip with pages missing. The message names the folder or file.

- **Not allowed to read:** you don't have permission for that folder on the shared drive. Ask whoever manages it for access, then download again.
- **Removed, or couldn't read (the share may be offline):** someone moved or deleted the file during the download, or the connection to the shared folder dropped. Check that the folder opens in File Explorer or Finder, then try again.
- **“Please note” after a finished download:** some files changed while it was being prepared, or some links were left out because Cairn doesn't follow links out of the documentation. Download again to get the latest versions.

## Times look wrong

Check **Settings → Time and timezone**. Times are shown in the timezone chosen there, marked with their difference from GMT (for example “GMT+8”). If it says Automatic, check the computer's timezone (Windows: **Settings → Time & language**; Mac: **System Settings → General → Date & Time**).

## “Couldn't check for a new version”

**Settings → Updates** shows the reason, and which connection the last check went through. What the messages mean:

- **“Your network's proxy asked for a sign-in.”** The office proxy wants a login. On Windows, Cairn offers your Windows login (NTLM or Kerberos) by itself. If it still says this, the proxy didn't accept that login, or it wants a typed password, which Cairn never gives. Ask your IT team to let Cairn reach `api.github.com`, `github.com` and `*.githubusercontent.com` through the proxy, or to leave them off the sign-in list.
- **“Cairn couldn't reach GitHub through the proxy …”.** The proxy typed in Settings is wrong or down. Check the address and port, or empty the box to use the proxy that Windows (or the `HTTPS_PROXY` variable) provides.
- **“Cairn couldn't verify GitHub's security certificate.”** A network filter is replacing GitHub's certificate. Ask your IT team to add its certificate to Windows. Cairn never skips this check.
- **“GitHub is limiting requests from this network.”** Many computers share one internet address and used up GitHub's allowance for it. Try again in an hour.
- **“Checking took too long.”** The connection is slow or blocked.

Your IT team can find out what the proxy wants with `curl -v -x http://PROXY:PORT https://api.github.com`: look for a `Proxy-Authenticate:` line (`NTLM` or `Negotiate` means a Windows sign-in, `Basic` a typed password).

## Where are the log files?

Cairn prints problems in its window. It doesn't write log files or send reports anywhere.
