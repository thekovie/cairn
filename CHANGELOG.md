# Changelog

All notable changes that people using Cairn would notice are listed here.

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
