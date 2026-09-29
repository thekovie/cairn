---
name: Cairn
description: The team handbook, as a plain docs site. Page tree on the left, the answer in the centre, "On this page" on the right.
colors:
  accent: "#0f6cbd"
  accent-hover: "#115ea3"
  accent-fg: "#ffffff"
  accent-soft: "#ebf3fc"
  bg: "#ffffff"
  surface: "#ffffff"
  surface-2: "#f7f7f5"
  surface-hover: "#efefed"
  fg: "#37352f"
  fg-muted: "#6b6a66"
  border: "#e9e9e7"
  border-control: "#d9d8d4"
  border-strong: "#8f8e8a"
  danger: "#b3261e"
  danger-soft: "#fdf0ef"
  warn: "#8a4600"
  warn-soft: "#fbf3db"
  ok: "#1d6b3a"
  ok-soft: "#edf7f0"
  mark: "#fdecc8"
typography:
  display:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "clamp(1.5rem, 1.3rem + 0.8vw, 1.875rem)"
    fontWeight: 700
    lineHeight: 1.15
    letterSpacing: "-0.025em"
  headline:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "clamp(1.1875rem, 1.1rem + 0.4vw, 1.375rem)"
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: "-0.015em"
  title:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "clamp(1.0625rem, 1rem + 0.25vw, 1.1875rem)"
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: "-0.01em"
  reading:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.7
  body:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.6
  label:
    fontFamily: "Inter, Segoe UI, system-ui, -apple-system, Roboto, Arial, sans-serif"
    fontSize: "0.875rem"
    fontWeight: 550
    lineHeight: 1.2
  mono:
    fontFamily: "ui-monospace, Cascadia Mono, Consolas, Courier New, monospace"
    fontSize: "0.875rem"
    fontWeight: 400
    lineHeight: 1.5
rounded:
  xs: "4px"
  md: "6px"
  lg: "10px"
spacing:
  "1": "0.25rem"
  "2": "0.5rem"
  "3": "0.75rem"
  "4": "1rem"
  "5": "1.5rem"
  "6": "clamp(1.25rem, 1.1rem + 0.6vw, 1.5rem)"
  "7": "clamp(1.75rem, 1.4rem + 1vw, 2.25rem)"
  "8": "clamp(2rem, 1.6rem + 1.5vw, 3rem)"
  gutter: "clamp(1rem, 0.5rem + 2vw, 2.5rem)"
  target: "2.25rem"
components:
  button:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    padding: "0.25rem 0.75rem"
    height: "2.25rem"
  button-hover:
    backgroundColor: "{colors.surface-hover}"
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.accent-fg}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    padding: "0.25rem 0.75rem"
    height: "2.25rem"
  button-primary-hover:
    backgroundColor: "{colors.accent-hover}"
  button-quiet:
    backgroundColor: "transparent"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    height: "2.25rem"
  button-quiet-hover:
    backgroundColor: "{colors.surface-hover}"
  button-danger:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.danger}"
    rounded: "{rounded.md}"
    height: "2.25rem"
  button-danger-hover:
    backgroundColor: "{colors.danger-soft}"
  input:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    typography: "{typography.body}"
    rounded: "{rounded.md}"
    padding: "0.5rem 0.75rem"
    height: "2.25rem"
  nav-item:
    backgroundColor: "transparent"
    textColor: "{colors.fg}"
    typography: "{typography.label}"
    rounded: "{rounded.md}"
    padding: "0.25rem 0.5rem"
    height: "2.25rem"
  nav-item-current:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent}"
  tree-row:
    backgroundColor: "transparent"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    padding: "0 0.5rem"
    height: "2rem"
  tree-row-current:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent}"
  list-row:
    backgroundColor: "transparent"
    textColor: "{colors.fg}"
    padding: "0.75rem"
    height: "3rem"
  list-row-hover:
    backgroundColor: "{colors.surface-2}"
  chip-draft:
    backgroundColor: "{colors.warn-soft}"
    textColor: "{colors.warn}"
    rounded: "{rounded.xs}"
    padding: "0.05rem 0.5rem"
  chip-active:
    backgroundColor: "{colors.ok-soft}"
    textColor: "{colors.ok}"
    rounded: "{rounded.xs}"
    padding: "0.05rem 0.5rem"
  banner-info:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.fg}"
    rounded: "{rounded.md}"
    padding: "1rem 1.5rem"
  dialog:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.fg}"
    rounded: "{rounded.lg}"
    width: "min(40rem, calc(100vw - 2rem))"
---

# Design System: Cairn

## Overview

**Creative North Star: "The Plain Handbook"**

Cairn is the docs-site standard played straight. The shell is the one people already know from Notion, GitBook and Microsoft Learn: a full-width top bar with search in the middle, a fixed page tree on a soft warm-grey panel at the left, the page in a centred column, and "On this page" riding on a thin rule at the right. Nothing is reinvented; the craft is in doing the convention cleanly, with obvious states and plain words.

Density is normal-website density, not a scaled-up accessibility look. Controls are 36px (2.25rem) at the normal text size, body and reading text are 16px, headings stay modest and step down fluidly on narrow windows. Every size is in rem, so the whole interface grows with the Text size setting (16 / 18 / 20px base), browser zoom and Windows text scaling. Three themes remap the same roles: light (default), dark (warm charcoal, not inverted) and high contrast (black on white, 2px borders, no soft shadows).

The surface is flat, white and quiet. Structure comes from hairline dividers and one tinted panel; blue is spent only on links, the primary action and "you are here".

**Key Characteristics:**
- Three-column docs layout: tree, centred article (72ch), "On this page" rail.
- Inter for everything; no second display face.
- One blue for links, the primary action and the current location.
- Hairline dividers and 6px corners; shadows only where something floats.
- 36px controls that scale with the text-size setting; worded buttons with icons beside the words.
- Colour is never the only signal: states pair colour with an icon, a word or weight.

## Colors

Warm neutrals with a single Fluent blue; status colours appear only as tinted notices and labels.

### Primary
- **Fluent Blue** (`accent`): links, the one filled button on a screen, the text of the current tree item or nav link, the current "On this page" marker, focus rings, checked choices. Darkens to **Pressed Blue** (`accent-hover`) on hover.
- **Blue Wash** (`accent-soft`): the fill behind "you are here" in the page tree and side nav, info banners, tip blockquotes, the first-visit welcome, checked choices and pressed view toggles.

### Neutral
- **Page White** (`bg`, `surface`): the page, cards, inputs, dialogs, the top bar.
- **Warm Paper** (`surface-2`): the page-tree panel, dialog footers, code blocks, table heads, list-row hover.
- **Warm Hover Grey** (`surface-hover`): hover on buttons, tree rows and nav links.
- **Warm Ink** (`fg`): all body text and headings. Never pure black outside the contrast theme.
- **Muted Ink** (`fg-muted`): meta lines, help text, icons inside neutral buttons, tree counts, breadcrumbs' separators.
- **Hairline** (`border`): dividers, list rules, card and dialog edges.
- **Control Edge** (`border-control`): neutral button borders, the tree's nesting rule, the "On this page" rule, table cells.
- **Field Edge** (`border-strong`): text inputs and selects, empty-state dashed outlines, the diff frame.

### Status
- **Danger Red / Danger Wash** (`danger`, `danger-soft`): destructive buttons, errors, broken links and pictures, removed diff lines.
- **Amber Text / Amber Wash** (`warn`, `warn-soft`): drafts, locks, reviews due, the edit note.
- **Green Text / Green Wash** (`ok`, `ok-soft`): published state, saved status, success toasts, added diff lines.
- **Highlighter** (`mark`): search-term highlights.

Dark and contrast values for every role live in `assets/css/tokens.css` and the sidecar; components only ever use the role names.

### Named Rules
**The One Blue Rule.** Blue means "you can go here", "do this" or "you are here". It is never decoration, and no second accent hue exists.

**The Checked Contrast Rule.** Every text/background pair is tested in all three themes: body text at 7:1 or better, secondary and state text at 4.5:1. A new colour ships only with its test.

## Typography

**Body Font:** Inter (variable, bundled locally, with Segoe UI, system-ui fallbacks)
**Mono Font:** ui-monospace / Cascadia Mono / Consolas

**Character:** One family, set with care: weight and size carry hierarchy, a slight negative tracking tightens headings, and pages read in the same Inter as the app with more line spacing.

### Hierarchy
- **Display** (700, 24 to 30px fluid, 1.15, -0.025em): the page title on a reading page, and page-level h1 elsewhere. Balanced wrapping.
- **Headline** (700, 19 to 22px fluid, 1.25): h2 inside a page, with a hairline under it; the editor title.
- **Title** (700, 17 to 19px fluid, 1.25): section headings in the app (h2), h3 inside pages; h3 in the app and h4 to h6 in pages at 17px.
- **Reading** (400, 16px, 1.7, 72ch measure): page text. Links weigh 500 with a 1px underline that thickens to 2px on hover.
- **Body** (400, 16px, 1.6, 70ch measure): app text, list-row titles (600), form labels (600).
- **Label** (500 to 650, 14px): buttons (550), tree and nav items, breadcrumbs, meta lines, help text, chips (600), "On this page".

### Named Rules
**The Inter Only Rule.** Inter is the only face. Hierarchy comes from weight (400 / 500 / 550 / 600 / 650 / 700) and the fluid size steps, never from a second family.

**The Upright Text Rule.** Tips, notes and reading text stay upright; italics are kept to tiny fallback labels (such as an absent diff cell), never paragraphs.

## Layout

A CSS grid shell: a sticky top bar (3.25rem) spans the window; below it, the page-tree panel (clamp 15rem to 17.5rem, in rem so it widens with text size) sits sticky at the left, full height, scrolling on its own. The main area centres one column (72ch text plus a 15rem rail) with equal side margins, never less than the gutter (16px to 40px). The editor may widen to 100rem.

The reading page is a two-column grid inside that: breadcrumbs, notices, the article head and the text share the left column (72ch); "On this page" is sticky on the right (12 to 15rem), separated by the largest space step.

Spacing follows a 4/8 rhythm (`spacing.1` to `spacing.5` fixed; `6` to `8` tighten on narrow windows). Sections are separated by `spacing.7`; headings sit `spacing.4` above their content.

Responsive steps: below 1180px "On this page" moves above the text as a folded, bordered disclosure; below 1000px editor panes stack; below 800px the tree becomes a left drawer (20rem, with scrim) opened by a "Pages" button in the top bar; below 600px the search drops to its own row, page and dialog buttons go full width, and list-row side details wrap under the title.

### Named Rules
**The One Search Rule.** One search box per screen: Home and Search show their own large box, so the top-bar search hides there.

**The Scale With Text Rule.** Sizes are in rem (or clamp of rem) so the 16 / 18 / 20px text setting scales controls, panel width and spacing together.

## Elevation & Depth

Flat by default. Depth comes from the warm-grey tree panel against the white page and from hairlines. Shadows appear only on things that float over the page: the "More" menu, dialogs, toasts, and the tree drawer on narrow windows. Neutral buttons carry a 1px ambient lift so they read as pressable; quiet buttons, toolbar buttons and the article head's secondary buttons drop it.

### Shadow Vocabulary
- **Control lift** (`box-shadow: 0 1px 2px rgba(15, 15, 15, 0.06)`): neutral buttons only.
- **Overlay** (`box-shadow: 0 12px 32px rgba(15, 15, 15, 0.12), 0 2px 6px rgba(15, 15, 15, 0.06)`): menus, dialogs, toasts, the drawer. In the contrast theme it becomes a 3px solid black ring.

### Named Rules
**The Float Only Rule.** A surface gets the overlay shadow only if it sits above the page. Cards, panels, banners and lists stay flat with a hairline.

## Shapes

Gently rounded, mostly one radius. Controls, tree rows, nav links, banners, inputs, code blocks, pictures and the editor frame use 6px (`rounded.md`). Dialogs, setup choice cards, panels and the welcome box use 10px (`rounded.lg`). Chips, inline code and tooltips use 4px (`rounded.xs`). Borders are 1px (2px in the contrast theme). Nested tree levels and "On this page" hang off a thin vertical rule; the current TOC entry marks that rule with a 2px blue segment. Icons are 1.75-stroke line SVGs with round caps, sized to the text (1.15em).

## Components

### Buttons
Worded, compact and calm; the icon accompanies the word.
- **Shape:** gently rounded (6px), 36px minimum height, 14px label at 550.
- **Default:** white with a control-edge border and control lift; icon in muted ink. Hover fills warm grey; press nudges down 1px.
- **Primary:** Fluent Blue fill, white text, hover to Pressed Blue. One per group: on a reading page, Edit is the only filled button and every other page button is quiet.
- **Quiet:** no border, no fill, no shadow; warm grey on hover. Used for secondary page actions, the "More" summary, tree toggles.
- **Danger:** white with red text and a red border; red wash on hover. In dialog footers it is pushed to the left, away from the confirm button.
- **Large:** 40px tall, 16px label, for hero search and setup.
- **Disabled / busy:** 45% opacity with a not-allowed cursor; busy shows a spinner.

### Chips
- **Style:** a label, not a button. Soft tint, 4px corners, 14px at 600, with an icon: Draft and Locked in amber wash, Published in green wash, Retired in muted ink on warm paper. The contrast theme adds a 1px outline.

### Cards / Containers
- **Corner Style:** 6px for banners, rail boxes and details panels; 10px for dialogs, panels and setup choices.
- **Background:** white; banners and notices use the matching status wash with a status-coloured 1px border.
- **Shadow Strategy:** flat (see Elevation & Depth).
- **Border:** 1px hairline.
- **Internal Padding:** 16px to 24px (`spacing.4` to `spacing.5`).

### Lists
Folders, pages, search results, drafts and history are plain hairline rows, never cards: a 1px rule above the list and under each row, an icon, a 600-weight title, and a muted 14px meta line. Hover fills warm paper and underlines the title in blue.

### Inputs / Fields
- **Style:** white, 1px field-edge border (`border-strong`), 6px corners, 36px minimum; labels above in 600, help text below in 14px muted ink.
- **Focus:** border turns Fluent Blue plus the global 3px focus outline.
- **Error:** red border and a red worded message with an icon beneath.
- **Choices:** radio and checkbox options are bordered rows that turn blue-bordered with a blue wash when checked.

### Navigation
- **Top bar:** white, hairline bottom, sticky. Workspace name (650) at the left aligned over the tree, search centred (max 36rem), Settings at the right.
- **Page tree:** warm-paper panel. New page and Home at the top, a small muted section label, then folders that expand with a rotating chevron to show pages. Rows are 32px, 14px text, single-line with an ellipsis; the full name shows on hover. Folder names at 550 with a muted page count. Templates and Recently deleted sit at the foot above a hairline.
- **You are here:** the current page or section is marked one way everywhere in navigation: Blue Wash fill, Fluent Blue text, 650 weight.
- **Breadcrumbs:** 14px, separated by a muted "›", 36px tap height.
- **On this page:** 14px muted links on a thin left rule; hover darkens the text and the rule, the section being read turns blue with a 2px blue segment on the rule.

### Dialogs and toasts
Dialogs are 40rem (64rem wide variant), 10px corners, overlay shadow over a scrim, with an icon-led head coloured by intent, a scrolling body and a warm-paper footer holding buttons right-aligned. They rise in over 220ms with the out-expo ease. Toasts sit bottom-right with the same entrance, bold text and a green (or red) icon.

### Editor toolbar (signature)
A Google Docs-style single row of quiet 36px icon buttons in groups divided by 1px separators; each name appears as a dark tooltip after a 350ms pause, immediately on keyboard focus. People can switch to words-beside-icons in Settings, and touch screens always get words. "Insert picture" always shows its name. The Write / Preview / Visual switch marks the pressed view with Blue Wash and a blue border.

## Do's and Don'ts

### Do:
- **Do** keep controls at the 2.25rem target and set every size in rem so the text-size setting scales the whole interface.
- **Do** put exactly one filled (primary) button in a group; on the reading page that button is Edit.
- **Do** mark the current location in navigation with Blue Wash fill and Fluent Blue text, and nothing else.
- **Do** show folders and pages as hairline rows with an icon, a title and a muted meta line.
- **Do** keep tree names on one line with an ellipsis and the full name on hover.
- **Do** pair every status colour with an icon or a word.
- **Do** use the global focus ring: 3px Fluent Blue outline, 2px offset.
- **Do** honour reduced motion: durations drop to 0ms and skeleton pulses stop.

### Don't:
- **Don't** add a second typeface; Source Serif was removed on purpose.
- **Don't** ship icon-only buttons outside the editor toolbar and the tree's expand chevrons (whose folder name sits beside them), and never one without a name for screen readers.
- **Don't** fill secondary buttons in the page header; they stay quiet.
- **Don't** put the overlay shadow on anything that does not float above the page; the only resting shadow is the 1px control lift on neutral buttons.
- **Don't** introduce a second accent hue or use blue as decoration.
- **Don't** use italic paragraphs for notes or tips.
