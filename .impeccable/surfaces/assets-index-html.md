---
version: 1
slug: "assets-index-html"
primary_target: "assets/index.html"
related_targets: []
---

Scope: the whole Cairn web app (shell, home, folder, reading page, search, editor, settings, setup, dialogs). Visitor modes: Read for pages, Operate for the rest.
Audience: a mixed-age, low-tech office team, mostly readers who are mid-task and looking something up. Constraints: PRODUCT.md (light default, Inter only, worded buttons, 44px targets, strict CSP, offline).

## Direction contract

THESIS: The docs-site standard played straight: Notion's quiet canvas, GitBook's three-column docs layout, Microsoft Learn's clarity and accessibility. It refuses the current centred feed column with a boxed folder strip and serif body.

OWN-WORLD: White page. The sidebar is a soft warm-grey (#F7F7F5) panel with a full page tree. Text is warm near-black (#37352F). One Fluent blue (#0F6CBD) for links and primary actions. Hairline #E9E9E7 dividers, 6px radius, 1px borders. Shadows only on menus and dialogs. Inter everywhere.

STORY: Readers see where they are in the tree, read the answer in a calm centred column, and jump sections from "On this page". Writers find New page and Edit where Notion puts them.

FIRST VIEWPORT: A full-width top bar: workspace name at left, centred search, Settings at right. A fixed left tree panel runs from edge to bottom: New page, Home, folders expandable to pages, current page highlighted; Templates and Recently deleted sit at the foot. The article is a centred 70ch column: breadcrumbs, a large title, one meta line, page buttons. "On this page" sits on the right with a left rule and the current section highlighted. On a phone, the tree is a drawer opened by a "Pages" button.

FORM: canon (the category standard, chosen by the user on the decision page); seed key c24f5318.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
