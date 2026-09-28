# Changelog

All notable changes that people using Cairn would notice are listed here.

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
