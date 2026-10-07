# Two-computer test checklist

Automated tests cover the file operations on one machine (`cargo test`). This checklist covers what only real network storage and two real computers can show. Run it before each release, and whenever you move documentation to a new kind of storage.

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

## Updates on each system (before a release)

Publish a pre-release (tag `vX.Y.Z-rc.1`) and check that no copy offers it as an update. Then, on each system, with the previous release installed:

| Step | Expected result | Pass |
| --- | --- | --- |
| **Settings → Updates → Check now**, then **Update and restart**. | The tab reconnects by itself; Settings shows the new version and **Go back to** the old one. On a Mac and Linux, Cairn keeps running in the same Terminal window. | ☐ |
| **Go back to …** | The old version runs again, and **Go back to** the new one is offered. | ☐ |
