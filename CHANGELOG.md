# Changelog

All notable changes that people using Cairn would notice are listed here.

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
