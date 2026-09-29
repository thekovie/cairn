# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

A small office team of mixed ages and mixed confidence with computers. They share one documentation folder on a network drive. Most people only read: they look up how to do something, often mid-task and in a hurry. A few people write and maintain pages. Many readers are not comfortable with software, and some need bigger text or high contrast.

## Product Purpose

Cairn is the team's handbook. It lets anyone read, search, write and publish documentation pages that live as plain Markdown files in a shared folder. Success means two things. Someone who has never used it can find the answer they need. Someone who has never written docs can publish a clear page without help or fear of breaking anything.

## Positioning

Cairn is local-first and needs no server. It is a single Windows program that each person runs on their own computer, pointed at a shared folder. The documentation stays ordinary Markdown files and folders that outlive the app. Several people coordinate through edit locks and earlier versions kept in the folder itself, with no database and no cloud account.

## Operating Context

- Runs in the person's usual browser at a `127.0.0.1` address, with a small console window that must stay open.
- It is installed per user, so no administrator rights are needed. It updates itself from signed GitHub releases.
- Workflows:
  - **Readers:** browse folders, search, read pages, and export them to PDF.
  - **Writers:** create pages from templates, edit visually or in Markdown, insert pictures, and publish with a review of changes. They can also rename, move and delete pages and folders, with links kept working, and restore earlier versions.
- The team thinks of it as their handbook. They compare it with Notion and GitBook-style docs sites.

## Capabilities and Constraints

- Works fully offline. All fonts and scripts are bundled; there are no external requests.
- A strict Content-Security-Policy applies: no inline scripts or style attributes.
- Plain JavaScript and CSS, with no front-end framework. The Rust (axum) back end is embedded into one exe.
- Themes: light (the default), dark, and high contrast. Text sizes: normal, large, and larger.
- Settings belong to each person. The shared folder holds only the docs, `shared-docs.json` and `_system/`.

## Brand Commitments

- The name Cairn and the existing favicon.
- Light is the default appearance.
- Buttons say what they do in words; icons only accompany the words. People can choose icon-only for the editor toolbar.
- Plain, friendly, specific wording for non-technical people.
- Inter is the typeface. The user asked to stop using Source Serif; another face is acceptable only if it reads better.
- Standing visual preference: the docs-site standard, played straight. Notion, GitBook and Microsoft Learn set the craft bar. That means a page tree on the left, the article in the centre, "On this page" on the right, and search in the top bar. Conventions are embraced, not reinvented.

## Evidence on Hand

- The real docs in `docs/` (getting started, writing pages, configuration, troubleshooting) serve as sample content.
- There are no testimonials, customer names or usage figures, and none may be invented.

## Product Principles

1. The answer comes first. Reading and finding beat every other concern.
2. Nothing scary. Every action says what will happen, and mistakes can be undone.
3. Calm by default. Show one clear next step, and put the rest behind a clear disclosure.
4. Files stay plain. Nothing in the UI should imply the docs are locked inside Cairn.

## Accessibility & Inclusion

- WCAG 2.2 AA at minimum; the high-contrast theme goes further.
- Normal website density: controls are about 36px at the normal text size, above WCAG's 24px minimum. The user asked for this over a scaled-up look. Controls grow with the Text size setting. Every function works from the keyboard with visible focus.
- Long names never break layout: in the page tree they stay on one line, end in "…", and show in full on hover.
- The layout must work at larger text sizes, down to phone width, and with reduced motion.
