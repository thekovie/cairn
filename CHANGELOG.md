# Changelog

All notable changes that people using Cairn would notice are listed here.

## Unreleased

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
