# Two-computer test checklist

Automated tests cover the file operations on one machine (`cargo test`). This checklist covers what only real network storage and two real computers can show. Run it before each release, and whenever you move documentation to a new kind of storage.

## Status: not yet run

Deferred, as of October 2026. The project has one maintainer and no second person or second computer to run this with, so none of the steps below has been done on a real share. Nothing here is marked Pass.

What has been done in its place, on one computer:

- The automated tests (`cargo test`), which cover locks, conflicts, publishing, backup, restore, and export against a local folder.
- Two Cairn processes opened on one folder: the edit lock was respected, a backup ran while the other process held a lock, and review worked once the lock was released.
- A real small drive that was too full for a backup or a restore, a disk image detached in the middle of a backup, a 1 GB workspace, and a case-sensitive volume with spaces and accents in its path.

What none of that can show, and so stays unchecked:

- How a real SMB share behaves: exclusive file creation, rename over an open file, and how fast a change made on one computer shows up on the other.
- Two different people, accounts, and permissions at once, including the read-only folder (step 10).
- Network loss in the middle of a publish or a backup on a real network (steps 9 and the backup rows).
- Mac and Linux against a Windows share.

Until it is run, treat network-share behavior as untested. A release note should say so, and the first team that puts Cairn on a shared drive is, in effect, running this checklist. If you do run it, even only steps 1 to 5 and the backup rows, record the result in the table below.

| Date | Build | Share type | Steps run | Result |
| --- | --- | --- | --- | --- |
| *(none yet)* | | | | |

## Setup

- **Computer A** and **Computer B**, with different user accounts. Run it with two Windows 10/11 computers; for a release that changes how Cairn works on a Mac or Linux, run it again with a Mac or Linux computer as B (see [Mac and Linux](#mac-and-linux)).
- A Windows file share (SMB 2/3) both can reach, for example `\\fileserver\team`. Both users need **Modify** permission.
- A subfolder that user B can **read but not write** (for step 10), for example `…\Documentation\Protected` with B denied *Write*.
- The same Cairn build on both computers.
- In **Settings → Your name**, set A to “Alex” and B to “Sam”.

Record the build version (`cairn --version`), the share type, and the date. Mark each step Pass or Fail.

| # | Step | Expected result | Pass |
| --- | --- | --- | --- |
| 1a | **A:** create a new documentation folder in `\\fileserver\team`, named “Team Docs”. | Home shows “Team Docs”. `\\fileserver\team\Documentation\shared-docs.json` exists. No banner about unsafe storage. | ☐ |
| 1b | **B:** choose **Open a documentation folder we already use** and pick `\\fileserver\team\Documentation\Guides` (a subfolder). | Cairn shows “Team Docs” at `\\fileserver\team\Documentation`. **Open this documentation** works. | ☐ |
| 1c | Compare `instance_id` in `shared-docs.json` as seen from both computers. | Identical. | ☐ |
| 2a | **A:** create “Printer guide” in Guides and publish. Choose **Edit this page** again and leave the editor open. | A sees **Locked for you**. | ☐ |
| 2b | **B:** open “Printer guide”. | B can read it, sees “Being edited by Alex since *time*”, and **Edit this page** is greyed out with that reason. | ☐ |
| 2c | **B:** go to the page's `#/edit/…` address directly. | “This page is being edited” screen; no editor. | ☐ |
| 3 | **A:** keep editing “Printer guide”. **B:** create and edit “Email setup” at the same time. Both publish. | Both publish successfully. | ☐ |
| 4 | **A:** change the printer page's text and publish. **B:** stay on the page for up to 30 seconds, or refresh. | B sees “This page has just been changed” (or the new text after refreshing). | ☐ |
| 5 | **A:** open “Email setup” for editing and type something. **B:** open `…\Guides\email-setup.md` in Notepad, add a line, save. **A:** choose **Publish changes**. | A sees “Someone else changed this page while you were writing”, with B's Notepad line marked −. The file still contains B's line. | ☐ |
| 6 | For a quick test, set **A's** editing times in Settings to 1 and 2 minutes. **A:** open a page for editing, type, then don't touch the keyboard. | After about 1 minute: “Are you still editing?”. After 2 minutes: the page is unlocked, the text stays in A's editor, and **Publish changes** is disabled. **B** can now edit the page. | ☐ |
| 7 | **A:** edit a page, paste a screenshot, type text. Wait for “Draft saved at…”. Close the Cairn window. Start Cairn again and choose **Edit this page**. | “Continue with your unsaved changes?” appears. Continuing brings back the text **and** the screenshot in the preview. | ☐ |
| 8 | **A:** publish the page with the screenshot. **B:** open the page. Also look at `…\Guides\<page>.md` and the `.assets` folder in File Explorer. | B sees the picture. The `.md` file uses a relative link such as `page.assets/screenshot-1a2b3c4d.png`, and that file exists. | ☐ |
| 9 | **A:** start editing a page and insert a new picture. Disconnect A's network (cable or Wi-Fi) and choose **Publish changes**. Reconnect. | A sees “The page was not published”. On B, the page still shows the previous text and has no broken picture. A's text is still in the editor and publishes after reconnecting. | ☐ |
| 10 | **B:** open a page in the Protected folder and choose **Edit this page**. | B can read it. Editing is refused with “you don't have permission to change files in this folder”, and after that the button stays greyed out with the reason. | ☐ |
| 11 | **B:** read A's port from A's Cairn window (the number after `127.0.0.1:`). On B, browse to `http://<A's IP address>:<port>/`, and try `curl http://<A's IP>:<port>/api/state`. | Connection refused or timed out. Nothing is served. | ☐ |
| 12 | **A:** browse to `#/page/..%2F..%2FWindows%2Fwin.ini`, and try to create a folder named `..`. | Refused with a plain message. Nothing outside the documentation folder is shown or created. | ☐ |
| 13 | **A and B at the same moment:** both choose **Create a new documentation folder** in the same empty share folder, with **Use the folder I chose**. | Both end up in the same documentation: one `shared-docs.json` with a single `instance_id`. | ☐ |
| 14 | **A:** open **Earlier versions** of “Printer guide”, view the oldest version, and choose **Restore this version**. | The page shows the old text on both computers. The version it replaced is now in the list. | ☐ |
| 15 | Give a new person only this repository. Ask them to follow `CONTRIBUTING.md` and `docs/getting-started.md`. | They build Cairn, create a sample documentation folder, and publish a page without help. | ☐ |

## Crash recovery (optional)

| Step | Expected result | Pass |
| --- | --- | --- |
| **A:** edit a page, type, then end `cairn.exe` in Task Manager. **B:** open the page within 5 minutes, and again after 5 minutes. | B sees “Being edited by Alex…”, then “…but they haven't been active for a while”. B still can't edit. | ☐ |
| **Maintainer:** `cairn locks list <share>`, then `cairn locks release <share> <page> --session <id>`. | Released; a record appears in `_system/locks/released/`. B can edit. | ☐ |
| **A:** start Cairn again and edit the page. | A is offered their unsaved changes. If B published meanwhile, A is shown both versions before publishing. | ☐ |

## Downloads and backups on the share

These need a real share, so they are not covered by the automatic tests.

| Step | Expected result | Pass |
| --- | --- | --- |
| **A:** **Settings → Download everything → Markdown**. While it prepares, **B** publishes a change to a page. | A's download finishes. If B's change landed in time, a “Please note” message names the changed file. Nothing is silently missing. | ☐ |
| **A:** start **Download everything**, then disconnect A from the network (or sleep the file server) before it finishes. | The download fails with a message naming a file or folder it couldn't read. No zip is saved. Reconnect and it works again. | ☐ |
| **A:** **Settings → Back up workspace** while **B** keeps publishing different pages every few seconds. | The backup finishes and a “Please note” message lists the pages that changed while it ran. (If a file keeps changing or vanishing, it stops with “kept changing” and names it.) No half-finished file is left in Downloads or the temporary folder. | ☐ |
| **A:** disconnect from the network in the middle of **Back up workspace**. | The backup fails with a message saying Cairn can't reach the folder or naming the file it couldn't read, and no half-finished file is left behind. | ☐ |
| **A:** `cairn backup create <share> D:\Backups\test.zip`, then `cairn backup verify D:\Backups\test.zip`. | Both succeed. `verify` says all checksums match. | ☐ |
| **A:** `cairn restore D:\Backups\test.zip D:\Restored`, then open `D:\Restored` in Cairn. | Pages, pictures, earlier versions (Earlier versions on a page), and Recently deleted pages are all there. No page is shown as being edited. | ☐ |
| **A:** `cairn restore` again into `D:\Restored`, and into a folder inside the share. | Both are refused with a clear message and nothing changes. | ☐ |
| **A:** change one byte inside `test.zip` (with a hex editor), then `cairn backup verify`. | It reports the damaged file and does not say OK. | ☐ |
| **Mac or Linux B:** `cairn backup create` on the same share, then `cairn restore` on a Windows computer (and the reverse). | Works both ways. Names with accents or other scripts come through unchanged. | ☐ |

## Mac and Linux

Run steps 1–14 with B on a Mac or Linux computer that has the same share mounted (on a Mac, **Go → Connect to Server** with `smb://fileserver/team`; on Linux, mounted with CIFS). Then:

| Step | Expected result | Pass |
| --- | --- | --- |
| **Mac:** download the release zip in Safari, open it, drag **Cairn** to Applications, and open it. | The Mac says it can't check Cairn. After **Open Anyway** in System Settings → Privacy & Security, Cairn opens in a Terminal window and the browser shows it. | ☐ |
| **Mac:** **Choose folder…** in setup. | Finder's folder window appears in front; the chosen folder is used. | ☐ |
| **Mac:** edit a page, then close the Terminal window (confirm **Terminate**). **A:** open the page. | A can edit it straight away: closing the window unlocked it. | ☐ |
| **Linux desktop:** double-click the AppImage (make it executable first if needed). | A terminal opens with Cairn running, and the browser shows it. | ☐ |
| **Linux desktop:** **Choose folder…** in setup. | The desktop's folder window appears (or, without one, typing the location works). | ☐ |
| **Linux:** with pages `Guides/printer.md` and `Guides/Printer.md` both there (make the second in a file manager), rename one onto the other in Cairn. Then rename a page changing only its capitals. | The first is refused and both pages stay; the second works, and links and pictures follow. | ☐ |
| **Windows:** look at the Start menu shortcut, the taskbar while Cairn runs, and **Settings → Apps → Cairn**. | All show the Cairn logo, not a blank program icon. | ☐ |

## Windows: no window, tray icon

One Windows computer is enough. Only partly run: the maintainer used the Start menu, the tray icon, and **Quit Cairn** on a work laptop and they worked. Every other row is not yet run.

| Step | Expected result | Pass |
| --- | --- | --- |
| Start Cairn from the Start menu. | The browser opens straight away and the tab says “Starting up Cairn” before it shows your documentation. There is no black window left open (it may flash for a moment: Windows makes it before Cairn can run). A Cairn icon is in the notification area, possibly behind the **^** arrow. Hovering says “Cairn is running”. | ☐ |
| Click the icon. | Cairn opens in the browser. | ☐ |
| With the keyboard (Win+B, arrow to the icon), press Enter, then Shift+F10 or the menu key. | Enter opens Cairn. The menu key opens the menu beside the icon. | ☐ |
| Right-click the icon. | A menu with **Open Cairn** (bold) and **Quit Cairn**. Clicking elsewhere closes it. | ☐ |
| Choose **Quit Cairn**, then start Cairn from the Start menu within a second or two. | Cairn starts afresh (it does not hand over to the copy that is stopping). | ☐ |
| Start Cairn from the Start menu again while it runs. | The browser opens Cairn. No second icon appears and Task Manager shows one `cairn.exe`. | ☐ |
| Edit a page, then choose **Quit Cairn** from the icon menu. | The icon goes away at once. The page is unlocked for others. Within a few seconds the open tab shows a red notice, **Cairn has stopped**, that says how to start Cairn again. | ☐ |
| Edit a page, then sign out of Windows (or restart). | The page is unlocked afterwards (`cairn locks list <folder>` shows nothing). | ☐ |
| With Cairn running, run the installer for a newer version. | The installer closes Cairn by itself, installs, and offers to start it. The old icon is gone. | ☐ |
| **Settings → Updates → Update and restart.** | The tab reconnects. Exactly one icon, with no console window opening. | ☐ |
| In Task Manager, end Explorer and start it again. | The icon comes back. | ☐ |
| Run `cairn.exe` from PowerShell. | The terminal shows “Cairn is running” as before. No icon. Ctrl+C stops it. | ☐ |
| Start Cairn so that it can't listen (give `--port` of one already in use, from a shortcut with that option). | A message box says Cairn couldn't start, instead of nothing. | ☐ |
| **Settings → Quit Cairn.** | Cairn stops, the icon goes, and the page says Cairn has stopped (once, with no red notice on top). | ☐ |
| Start Cairn with the last folder on a network drive that is slow or unplugged. | The browser opens at once and says “Starting up Cairn”, then “taking longer than usual” after a few seconds. Cairn keeps trying. Quit Cairn from the icon still works meanwhile. | ☐ |

## Updates on each system (before a release)

Publish a pre-release (tag `vX.Y.Z-rc.1`) and check that no copy offers it as an update. Then, on each system, with the previous release installed:

| Step | Expected result | Pass |
| --- | --- | --- |
| **Settings → Updates → Check now**, then **Update and restart**. | The tab reconnects by itself; Settings shows the new version and **Go back to** the old one. On a Mac and Linux, Cairn keeps running in the same Terminal window. | ☐ |
| **Go back to …** | The old version runs again, and **Go back to** the new one is offered. | ☐ |
| **Windows, behind an office proxy that asks for a Windows sign-in (NTLM or Kerberos):** **Settings → Updates → Check now**, with the proxy box empty. | The check succeeds, and Settings says it went through the proxy and was signed in with your Windows login. | ☐ |
| **Windows:** type a wrong proxy such as `proxy.invalid:8080` in **Proxy server**, then **Check now**. | The message names that proxy and says to check the address. Cairn doesn't quietly go direct. Empty the box afterwards. | ☐ |
