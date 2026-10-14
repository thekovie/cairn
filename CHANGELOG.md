# Changelog

All notable changes that people using Cairn would notice are listed here.

## 1.3.0 (2026-10-14)

### Changed

- **A browser tab now says so when Cairn stops.** If Cairn is closed (from its icon, **Quit Cairn**, or a shutdown) while a tab is open, a short red notice, **Cairn has stopped**, appears at the top within a few seconds and says how to start Cairn again and asks whether you have copied what you were typing; closing the tab asks first. It goes away by itself if Cairn comes back. Before, the tab pointed at a Cairn window, which on Windows no longer exists.
- **The browser opens as soon as Cairn is ready to answer.** The tab shows **Starting up Cairn** while the documentation folder from last time is opened, which can take a while on a network drive. Before, nothing appeared until the folder had opened.
- The messages for a tab that is no longer connected no longer tell you to look for a “program window”: they say how to start Cairn in the words of your computer.

### Added

- **`cairn restore … --new-identity`** restores a backup as a copy with an identity of its own, so it can sit beside the original (to look at an old state, or to test) without sharing the original's identity. The default is unchanged: a restore keeps the identity, for replacing a lost folder.

## 1.2.0 (2026-10-14)

### Changed

- **On Windows, Cairn no longer keeps a black window open.** A Cairn icon sits in the notification area instead: click it to open Cairn, or right-click it for **Open Cairn** and **Quit Cairn**. Windows often hides new icons behind the **^** arrow. Signing out, shutting down, and the installer updating Cairn still unlock the pages being edited. Starting Cairn again while it runs opens it in your browser instead of starting a second copy. If the icon can't be added, the window comes back. Started from a Command Prompt or PowerShell, Cairn behaves as before and shows no icon. Mac and Linux are unchanged.

### Fixed

- **A second tab works.** Opening a page in a new tab, or pasting its address into one, used to show “Please open Cairn from its program window”. The new tab now connects by itself while another Cairn tab is open, so you can keep one page open to refer to while you read or write another. Duplicating a tab that is in the editor opens a second editor on the same page; only publish from one of them.

## 1.1.0 (2026-10-14)

### Added

- **A warning before a PDF changes links.** If a page (or any page in a folder download) has links to other pages or files, Cairn says so first: how many, and that each will keep its words with the name of what it pointed at beside it, since a PDF can't follow them. You choose **Download anyway** to carry on. Nothing is shown when no links will change.
- **PDFs keep the meaning of links.** A link to another page used to turn into plain text. It now keeps its text and shows the page's name beside it (and its folder, when different), and a link to a file shows the file's name. Links to headings on the same page, including the page title, now work inside the PDF. Download dialogs say how links appear. Links between separate PDFs in a folder download are deliberately not made: the PDFs are printed one at a time and can't link to each other reliably.
- **Narrow a search by folder, status, or tag, and choose the order.** Best match, recently changed, or title. Leave the words empty to list every page that fits. Filters apply before the 50-page limit, so a filtered search finds pages an unfiltered one would cut off, and the heading says how many matched in all. Retired pages are still found unless you choose a status.
- **Search ranks better.** A title that is exactly what you typed comes first, tags are searched and count like title words, and pages that tie always come in the same order (title, then path, ignoring capitals).
- **Mark as reviewed today.** On a page that is due for review (and in the Documentation health list), one button sets the review date to today. It changes only that date: not the status, and not who is shown as having written the page. It waits if someone is editing the page, never writes over a change made at the same moment, and keeps the earlier version.
- **Check documentation.** On the Documentation health screen, one button looks through every page for broken links and pictures, with a progress bar (“Checking page 412 of 1280”) and a Stop button: missing pages, files, and headings, missing or unsupported pictures, and links that go outside the documentation. Each problem names the page, the line, and what to do. It never visits web addresses, runs only when asked, and says so if it couldn't read some pages.
- **Documentation health.** A new screen in the left-hand list shows the pages that need attention, with the reason: review due, no review date, no owner, draft, retired, or page details Cairn can't read. Filter by problem. If part of the shared folder couldn't be read, it says so and never reports that everything is fine. See [Writing and editing](docs/authoring.md#documentation-health).
- **Back up workspace.** **Settings → Back up workspace** saves one .zip with everything needed to bring the documentation back: pages, pictures, templates, earlier versions, and deleted pages. Every file has a checksum. It keeps working while people save pages, and lists the files that changed while it ran. “Download everything” stays for sharing, and now says it is not a backup. See [Back up and restore](docs/backup-and-restore.md).
- **A progress counter in the terminal.** `cairn backup create`, `cairn backup verify`, and `cairn restore` show “Copying file 40 of 120” while they work, but only when you are watching, not in logs or scheduled runs.
- **Backups and restores check free space first.** A restore that can't fit stops before writing anything and says how much it needs and how much is free. A backup that certainly can't fit is refused the same way, and one that might not fit goes ahead with a note. Nothing is left behind either way.
- **`cairn backup create`, `cairn backup verify`, and `cairn restore`** for scheduled backups, for checking a backup, and for restoring one into a new, empty folder. A damaged, edited, unsafe, or newer backup is refused and nothing is written.

### Fixed

- **Check documentation no longer says “complete” when a folder could not be read.** If the list of pages was missing a folder Cairn isn't allowed to open, the check only covered the pages it knew about and reported nothing wrong. It now names the folder and says the result is partial.
- **Keyboard focus stays in the Download and Back up windows.** After the PDF link warning appeared, or after a backup finished, focus dropped to the page behind the window. It now stays on the button.
- **The chosen filter and the current page are visible in Windows High Contrast.** Before, every filter button on Documentation health looked the same.
- **Empty screens (Recently deleted, Earlier versions) no longer skip a heading level**, so screen-reader heading lists make sense. The round and square choice buttons are also a little larger, to meet the 24-pixel minimum.
- **When the drive under the documentation can't be reached, Cairn now says so.** A network drive that drops out used to produce “Invalid path.” Messages now say Cairn can't reach the folder and to check that the drive is still connected.
- **A page changed to a same-length text a moment after it was last saved is no longer missed.** Lists and search noticed changes by whole seconds and file size, so a new review date written in the same second looked unchanged. They now compare the exact time.
- **“Review due” now uses your chosen timezone.** It used the browser's own timezone and could be a day off. The day a page falls due (a year after its last review) is now worked out once by Cairn, so the page list and the page itself always agree.
- **A folder or page that can't be read no longer disappears from lists and search.** When the shared drive drops out or a folder is off-limits for a moment, Cairn keeps showing the pages it already knew and records what it couldn't read, instead of quietly dropping them.
- **Downloads no longer leave files out without telling you.** If Cairn can't read a folder or file (no permission, or the shared drive drops), the download stops and names it. Before, the zip looked complete but the pages were missing.
- **Downloads tell you what changed or was left out.** When a download finishes, a note lists files that changed while it was being prepared and links that Cairn didn't follow.
- **Long PDF downloads can be saved when they finish.** A download that took more than 10 minutes used to disappear the moment it finished. The 10 minutes now run from when it ends.
- **A crash while preparing a download no longer blocks every other download** until Cairn restarts.
- **A real PDF problem is now reported as one.** When a browser starts but fails (too slow, no file), the message says so, instead of saying no browser was found.
- **Download everything no longer includes `shared-docs.json`**, which would make an unzipped copy look like the original documentation.
- A page named with capital letters (`Setup.MD`) now gets the right name in a PDF download.
- Leftover PDF working folders from a crash are cleaned up.

## 1.0.1 (2026-10-12)

### Fixed

- **Updates now work through office proxies that ask for a Windows sign-in** (NTLM or Kerberos). Before, the check failed with a vague "couldn't reach GitHub". Cairn now retries through Windows' own networking, which signs in with your Windows login, and also follows proxy scripts. It never asks for or keeps a password.
- **The proxy to use is chosen in a fixed order:** the one in Settings, then the `HTTPS_PROXY`-style environment variables (with `NO_PROXY`), then Windows' proxy settings, then none. **Settings → Updates** shows which one the last check used.
- **Clearer messages when a check fails:** a proxy sign-in, GitHub's request limit, an untrusted certificate, and being offline each have their own.
- A redirect from a secure to a plain `http` address is no longer followed when downloading an update.

## 1.0.0 (2026-10-07)

### Added

- **Cairn for Mac and Linux.** Cairn now runs on macOS 11 or newer (one download for Apple silicon and Intel Macs) and on 64-bit Linux (an AppImage to double-click, or a `.tar.gz`), as well as Windows. People on different systems can share one documentation folder. See [Getting started](docs/getting-started.md).
- **Updates on every system.** **Update and restart** works on a Mac and on Linux too, with the same signature check and **Go back to** the version before. On a Mac and Linux, Cairn keeps running in the same Terminal window.
- **The Cairn logo** on the program, its shortcuts, the taskbar, and the installer on Windows, and as the app icon on a Mac and Linux.
- **One-click PDFs on a Mac and Linux**, made with Google Chrome, Microsoft Edge, or Chromium.

### Fixed

- **Renaming a page to change only its capitals** (`printer` to `Printer`) now updates the links to it and its pictures. They kept the old capitals, which broke them on Linux.
- **On disks where capitals matter (Linux),** renaming a page or folder onto another whose name differs only in capitals is refused instead of overwriting it.
- **Two people setting up the same folder at the same moment** both end up in the same documentation; one of them used to get an error.
- **A page that's gone** ("no longer exists") offers **Go to the home page** and **Search** instead of a **Try again** that couldn't work.
- **"… is editing"** messages name the page by its title instead of its file name, and after a rename or move Cairn says how many other pages had their links updated.

- **Your changes are no longer lost when they can't be saved.** If the shared folder doesn't answer, **Close editor** (or leaving for another screen) now keeps the editor open and says so, instead of closing and unlocking the page without your words.
- **Closing the Cairn window gives back the pages you were editing**, as quitting from Settings already did. Before, closing the window, signing out, or shutting down the computer left them showing "Being edited by…" until someone released them by hand.
- **A picture added at the page title goes below the title**, instead of splitting it and leaving an empty heading. In the middle of a sentence it goes after the paragraph.
- **While a picture is being added, publishing, or a download is being prepared**, closing or reloading the tab asks first, and **Close editor** and **Discard my changes** wait until it's done.
- **Opening a different documentation folder** says what went wrong if it can't, instead of doing nothing.
- **"The shared folder is responding slowly"** no longer appears behind a question Cairn is asking you.
- **A screen that can't be shown** has its own title in the browser tab, instead of the previous screen's.

## 0.9.2 (2026-10-01)

### Added

- **Link to a heading on the same page.** In **Link…**, choose **This page**, then the heading. The link is written as `[text](#heading-name)` and scrolls to that heading when clicked.

### Fixed

- **A link to a heading that no longer exists is marked "missing"**, like a link to a missing page, and counted in the page's broken-links notice.
- **Clicking a same-page link in the editor's preview** scrolls the preview to the heading instead of changing the address.
- **"On this page" indents subheadings clearly**, one step per level, and also for pages whose sections are `#` headings (those used to be left out, so their subheadings showed flat). The Link dialog's heading list is indented the same way.
- **Pages that use `#` for their sections** get space above each one, like other headings.
- **"On this page" marks the section you jumped to**, even near the end of a page where it can't scroll to the top. It goes back to following your scrolling as soon as you scroll yourself.
- **On a phone, "More actions for this page"** lines up with the buttons above it instead of sitting centred.

## 0.9.1 (2026-10-01)

### Added

- **The text and the preview scroll together** in **Show formatting codes**. Scroll either side and the other follows, lined up heading by heading. Untick **Scroll together** above the preview to scroll them separately.
- **Replace… matches whole words** unless you untick **Whole words only**, so replacing "IT" no longer changes "edit" or "with".

### Fixed

- **Callouts look like callouts while you edit.** In **As it will look**, a Warning box is tinted and labelled "Warning", as on the published page, instead of showing the `[!WARNING]` code.
- **The Table bar no longer covers the line above a table.** Tables keep a little room above them while you edit.
- **The template Insert buttons** match the rest of the toolbar.
- **Editing on a phone is one scroll.** The editor grows with the page instead of being a small box that scrolls inside it, and the formatting buttons stay at the top of the screen while you write. The middle view button is called **Page + codes** on a phone, so all three fit on one line.

## 0.9.0 (2026-09-30)

### Added

- **Edit tables where they are.** Click in a table and a small **Table** bar appears just above it, with four menus: **Insert** (rows and columns), **Align**, **Move**, and **Delete**. The page doesn't jump when it appears. **Tab** in the last cell adds a row. Cells can't be merged, because Markdown tables don't support it; the Insert menu says so.
- **Callout boxes.** **Note box** now also offers **Note**, **Tip**, **Important**, **Warning**, and **Caution**. Each has its own colour and a label in words. They're written as `> [!WARNING]`, as on GitHub, so they show the same there.
- **Replace…** (**Ctrl+H**) changes a word or phrase everywhere in the page, leaving link addresses alone. One **Undo** takes it back.
- **Type `/` on an empty line** to pick what to add there: a heading, list, callout, table, picture, and more.
- **Word count** and reading time at the top of the editor.
- **Link to a section of a page.** **Link…** lists the chosen page's headings, so the link opens right at one.

### Changed

- **A tidier editor toolbar.** Every control is the same size and fits on one row. **As it will look / Show formatting codes / Codes only** is now one switch at the top right. **Replace…** sits at the far right, and **Note box** shows a small ▾ because it opens a menu. The word count is under the page.
- **Removing old earlier versions is now a choice, and off unless someone turns it on.** In 0.8.0 it was always on. In **Settings → Earlier versions**, tick **Remove old earlier versions automatically** and choose how many of each page's newest versions to always keep and after how many days the others go. The choice applies to everyone using the documentation folder, and Cairn asks before turning it on.

## 0.8.0 (2026-09-30)

### Added

- **Old earlier versions are tidied away.** Versions older than a month are removed, but every page always keeps its 3 most recent ones. This happens when a page is published and when Cairn opens the documentation.
- **Cairn keeps you on the page while it saves.** While it publishes, renames, moves, deletes, restores, or creates something:
  - closing or reloading the tab makes the browser ask first;
  - Home, the page tree, and the Back button wait until it's done, with a message saying why;
  - a notice at the bottom says to keep the tab open.
  This way nothing is left half done.
- **More formatting in the editor:** **Strikethrough**, **Checklist**, **Note box**, and **Divider line** buttons, plus **Undo** and **Redo**. The text style list has three more heading sizes: Small, Smaller, and Smallest heading. Everything is still plain Markdown.
- **Proxy server setting** (**Settings → Updates**). Offices that reach the internet through a proxy can type its address and port, such as `proxy.office.local:8080`, so Cairn can check for and download updates. Leave it empty to keep using Windows' proxy settings.

### Fixed

- **"Edited by" stays with whoever wrote the page.** When a rename or move fixes links in other pages, those pages are no longer credited to the person who moved things. A new title still counts as that person's edit.
- **Page lists show when a page was published,** not when a link in it was last fixed. "Recently changed" is in that same order.

## 0.7.0 (2026-09-30)

### Added

- **See who last edited a page.** Under the title and in page lists: “Edited by Priya Shah, today at 4:44 PM”. Each earlier version says who published it. If a page was changed outside Cairn, it says so instead of naming the wrong person. The page files themselves stay unchanged.

### Changed

- **A new look, like the docs sites people already know** (Notion, GitBook, Microsoft Learn):
  - A page tree on the left: folders open to show their pages. The page you're on is highlighted and its folders are open. Long titles stay on one line and show in full when you point at them.
  - The page sits in a calm centred column. "On this page" on the right marks the section you're reading as you scroll.
  - On a phone or a narrow window, the tree slides in from the left with the **Pages** button.
- **Normal website sizing.** Text, buttons and spacing are the size you'd expect on any website. **Settings → Text size** still makes everything bigger.
- **One typeface.** Pages are read in Inter, like the rest of Cairn. Source Serif is no longer used.
- **Quieter pages.** **Edit this page** is the one filled button; the others are plain. Folders on Home are a simple list.

## 0.6.0 (2026-09-30)

### Changed

- **Editing starts on the page as it will look.** The editor now opens in visual editing for everyone. Seeing the formatting codes is a choice (**Show formatting codes** or **Codes only**), remembered once made.
- **Checklists work in visual editing**: each item shows its box, and clicking it (or Ctrl+Enter) ticks or unticks it.
- **Publishing shows what you changed first** (“You changed 2 lines”), and that the page as it was is kept in Earlier versions. Closing the editor now says honestly whether anything changed, and that unpublished changes are seen only by you.
- **Earlier versions and conflicts are shown side by side**: the older version on the left, the newer on the right, each different line labelled Changed, Added, or Removed.
- **The editor toolbar shows words** beside its icons by default, and **Insert picture…** always does. (Choose Icons only in Settings for the compact toolbar.)
- **Pages read like a handbook**: text in Source Serif 4, a reading typeface made for screens, at a comfortable size and line length. The page details are one quiet line under the title; Rename, Move, and Delete sit under **More actions for this page**; only “On this page” sits beside the text (folded above it on narrow windows). The editing note appears only when someone is editing, and says who and what happens next.
- Pages due for review (last reviewed over a year ago) are marked **Review due**.
- **One search box per screen**, and a misspelled search suggests what you might have meant (“Did you mean printer?”) and lists the folders to browse.
- **A short welcome** on your first visit, which also offers to set the name others see.
- **Settings**: a list of sections at the top, and the editing times and unsaved-changes choice now save straight away. The editing times are chosen from a list.
- The new-version notice appears only on Home and Settings.
- Templates: “Copy and customize” is now **Make a team copy**, with one **New template** button.
- Tips in pages are no longer italic, status labels no longer look like buttons, and Templates and Recently deleted sit at the foot of the sidebar. The documentation folder's location moved from the sidebar to a quiet line on the Home screen (and is still in Settings).

### Fixed

- Screen readers: the page title is the first heading, toolbar buttons are no longer read twice, the writing box has a proper name, and a locked **Edit this page** button can still be reached with the keyboard and says why it's locked.
- Breadcrumb and “On this page” links are big enough to tap easily.
- On phones, the rows in Recently deleted no longer squeeze their text into a narrow column.

## 0.5.0 (2026-09-29)

### Added

- **Cairn updates itself.** Once a day (or when you choose **Settings → Updates → Check now**) Cairn looks for a new version. When there is one, a notice at the top shows it, with **Update and restart**. Cairn downloads it, checks that it's signed by Cairn's makers, installs it, and restarts; the browser tab reconnects by itself. Publish or close pages you're editing first; unsaved changes are kept either way.
- **Go back** to the version you had before, from **Settings → Updates**, if an update causes trouble.
- **An installer** (`cairn-…-setup.exe`) that needs no administrator password: it installs Cairn just for you, adds it to the Start menu, and can be removed from **Settings → Apps**. Installed this way, Cairn can always update itself.
- **Settings → Updates → Look for new versions**: every day, or only when you choose Check now. Checking sends nothing about you or your documentation.

### Changed

- A Cairn run from a network drive doesn't update itself, because that would change it for everyone using it; it shows the new version with a link to the download page instead.

## 0.4.0 (2026-09-29)

### Added

- **Rename, move, and delete pages.** Beside every page, the new **Organize** box has **Rename** (a new title, and a matching file name), **Move** (to another folder), and **Delete**. A page's pictures and earlier versions go with it.
- **Rename, move, and delete folders**, under **Organize this folder** at the bottom of each folder. Everything inside goes with it.
- **Links keep working.** When a page or folder is renamed or moved, links to it from other pages are updated, and so are the moved pages' own links. Each page that changes keeps its previous text under **Earlier versions**. If someone is editing a page that links to it, that page is left alone and Cairn lists it afterwards.
- **Recently deleted**, in the sidebar. Deleting never destroys anything: deleted pages and folders are listed here with who deleted them and when, and **Restore** puts one back exactly where it was, with its pictures and earlier versions.
- Nothing can be renamed, moved, or deleted while someone is editing it; Cairn says who is. A page isn't deleted if it changed since you opened it.
- If a page you're reading is renamed, moved, or deleted by someone else, Cairn says so, with **Search for it** and **Recently deleted** to find it again.

## 0.3.4 (2026-09-29)

### Added

- When you add a picture, the **Describe this picture** window shows the picture itself (with its file name and size), so you can check it's the right one before inserting it.
- Click a picture on a page (or in **Earlier versions**) to see it larger. Small pictures are enlarged, big ones fit the window, and the picture's description is shown under it. Close it with **Close**, Esc, or by clicking outside it. The keyboard works too: move to the picture and press Enter.

### Changed

- On wide screens, the folder list and the page sit together in the middle of the window, with equal space on both sides (like X or Reddit), instead of everything hugging the left edge. The top bar lines up with them. On laptop-size windows and smaller, nothing changes.

- The editor is now a box of its own height: the text, the preview, and the Visual page scroll inside it, and the page around it stays put, so **Publish changes** is always just below. Drag the bar under the box (or select it and use the arrow keys) to make it taller or shorter; Cairn remembers the height.
- The writing box no longer starts with the technical page-details block (`---`, `owner: …`). Those details are still saved with the page and are edited with **Page details** as before.

### Fixed

- At the Larger text size, the folder list on the left no longer cuts off page counts; it grows with the text.
- The search box at the top no longer keeps your last search after you leave the results.
- In Visual editing, typing right after a link no longer makes the link longer. The words you type after it are ordinary text, as in a word processor.

## 0.3.3 (2026-09-29)

### Changed

- In Visual editing, the page is centred in the editor like a page in a word processor, instead of sitting against the left edge with a large empty space on the right. Its width matches the published page.

### Added

- A loading indicator for slow shared folders. If a page, folder, or the editor takes more than a moment to open, Cairn shows what it's doing (for example “Opening the editor…”) with a placeholder of the page. After a few seconds it also explains that the shared folder is responding slowly and there's no need to click again. The editor's preview says “Preparing the preview…” until it's ready.

### Fixed

- On a slow shared folder, moving to another page before the first one finished loading could show the first page's content on the second. Late content is now discarded, and an editor that opens too late gives its edit lock back.

## 0.3.2 (2026-09-29)

### Fixed

- A new page closed without publishing seemed to disappear: it was kept on your computer, but nothing showed it. The Home screen now has **Your unsaved changes**, listing every page you started or changed but haven't published (it survives restarting Cairn), and each folder lists its **New pages you haven't published**. Choosing one opens the editor where you left off.
- Creating a new page with the same title as an unpublished one no longer reopens the old text; the new page gets its own file.
- In Visual editing, words typed just before **Close editor** are always saved.
- No more stray blue boxes: the page title no longer gets one when a page opens, and the writing area no longer has one while you type (the cursor shows where you are, and the pane's label turns blue). Buttons, links, and fields still show a clear focus ring when you move to them with the keyboard.

## 0.3.1 (2026-09-29)

### Fixed

- In **Markdown and preview** and **Markdown only**, an empty Visual area also appeared and pushed the preview down the page.
- In Visual editing, the **Text style** list now shows the style of the line you click on straight away.

## 0.3.0 (2026-09-29)

### Added

- **Visual editing.** Choose **Visual** in the editor to write on the page as it will look. Markdown shortcuts work as you type: `## ` makes a heading, `- ` a bullet list, `**words**` bold, and so on. The toolbar, pictures (paste, drag, or button), links, and tables all work there, and the page is still saved as Markdown. Cairn remembers whether you last used Visual or Markdown.

### Changed

- The editor toolbar is now one compact row of icons, like Google Docs. Each button's name appears when you point at it or move to it with the keyboard, with its shortcut (for example “Bold (Ctrl+B)”). Heading and Subheading are now a **Text style** list that also shows the style of the current line. To always see the names, choose **Settings → Appearance → Editor toolbar → Icons and words**; on touch screens they are always shown.

### Fixed

- Opening a page in the editor and closing it without changing anything no longer shows “You have unsaved changes to this page” afterwards, and the page's button says **Edit this page** again instead of **Continue editing**.

## 0.2.0 (2026-09-28)

### Added

- **Timezones.** Settings → Time and timezone: keep this computer's timezone (the default) or choose a region and city. Every time Cairn shows is converted to your timezone and marked with its offset, for example “Being edited by Alex since 2:30 PM GMT+8”. Times are still saved in one standard form (UTC), so nothing in the shared folder changes when someone picks a different timezone. The Today button and new pages' review date use your timezone's date.
- **Team templates.** A new Templates page lists your team's templates and the built-in ones. Make a template from scratch or by copying a built-in one, edit it with the normal editor (with buttons to insert the page title, today's date, the author's name, or the folder name), and delete it with a confirmation. Deleted templates can be restored. Templates are stored in the documentation folder's `_templates` folder, so each documentation folder has its own set.
- The New page screen shows your team's templates first, then the built-in ones, and offers a filter when there are many. “Use for a new page” on the Templates page opens it with that template chosen.
- **Downloads.** A Download button on every page saves it as a PDF, as Markdown with its pictures (.zip), or as a single Markdown file. “Download this folder” and Settings → “Download everything” save a .zip of Markdown files and pictures, or of PDFs, with progress and a Cancel button.
- PDFs are made on your computer with Microsoft Edge or Google Chrome, with pictures included and a header showing the owner, status, and dates. If neither browser works, Cairn opens a print view instead, where you can choose “Save as PDF”. Paper size (A4 or US Letter) is in Settings.

### Fixed

- A dialog left open when using the browser's Back button now closes with the screen it belongs to.

## 0.1.1 (2026-09-28)

### Changed

- Text is smaller by default. Normal is now 16px (the size of most websites), Large is 18px, and Larger is 20px. Headings scale with the window.
- The layout adapts to any window size: full screen, half a screen, or a phone. Below 800px wide, the folder list becomes a strip of buttons above the page instead of a sidebar.
- On narrow windows the editor opens on Write only; choose Preview only or Write and preview to switch.

### Fixed

- At half-screen widths the folder list stretched to fill the window and pushed the page far down.
- Setting up the same folder from several computers at the same moment could fail for one of them instead of joining the new documentation.

## 0.1.0 (2026-09-28)

First working release, for Windows.

### Added

- Open existing documentation by choosing its folder or any folder inside it, or create new documentation in an empty or new folder. The name and location are always shown before anything is opened.
- Home screen with folders, recently changed pages, and search.
- Folder view, and a page view with table of contents, owner, status, last reviewed date, and editing state.
- Editor with a worded formatting toolbar, live preview, page details form, and formatting help, so pages can be written without knowing Markdown.
- Pictures by button, paste, or drag and drop (PNG, JPEG, WebP, GIF), stored next to each page.
- One editor per page, with “Being edited by … since …” shown to everyone else.
- “Are you still editing?” after 15 minutes without typing, and automatic unlock after 20 minutes, keeping your text.
- Unsaved changes saved privately on your own computer every few seconds, and offered back when you return.
- Safe publishing: a page is never overwritten if someone else changed it; you see both versions instead.
- Earlier versions of every page, viewable and restorable.
- Light, Dark, and High contrast colors; three text sizes.
- A warning when the documentation folder's storage isn't safe for several editors, for example a cloud-synced folder.
- `cairn init` to create documentation, and `cairn locks list` / `cairn locks release` to recover abandoned edit locks.
