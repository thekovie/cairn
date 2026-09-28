# Writing pages

## Opening a page for editing

On any page, choose **Edit this page**. While you edit:

- The page is **locked for you**. Others can still read it, and they see “Being edited by *your name* since *time*”.
- If someone else is already editing, the button is greyed out and the reason is shown underneath. You can keep reading; editing opens up when they finish.

Your name comes from **Settings → Your name**. If you leave it empty, your Windows user name is used.

## The editor

| Part | What it's for |
| --- | --- |
| **Write** (left) | The text of the page. |
| **Preview** (right) | How the page will look. It updates as you type. |
| Toolbar | Buttons that format text for you. |
| **Write and preview / Write only / Preview only** | Choose what to see. |
| **Page details** | Owner, status, last reviewed date, and tags. |
| **Formatting help** | A short table of formatting, for anyone who prefers typing it. |

### Formatting with the toolbar

| Button | What it does |
| --- | --- |
| **Heading**, **Subheading** | Turn the current line into a section title. |
| **Bold**, **Italic** | Style the selected words. With nothing selected, example words are inserted for you to replace. |
| **Bullet list**, **Numbered list** | Turn the selected lines into a list. Choose again to undo. |
| **Link…** | Link to another page in this documentation (pick it from the list) or to a website. |
| **Table…** | Insert a table with the number of columns and rows you choose. |
| **Code** | Mark text as a command or exact text to type. |
| **Insert picture…** | Add a picture from your computer. |

Optional keyboard shortcuts: **Ctrl+B** bold, **Ctrl+I** italic, **Ctrl+S** save now. Everything a shortcut does is also available as a button.

### Markdown, if you want it

Pages are stored as Markdown, a plain-text format. The toolbar writes it for you, but you can type it directly:

| To get | Type |
| --- | --- |
| Page title | `# Title` (the first one is the page's title) |
| Section heading | `## Heading` |
| Bold | `**words**` |
| Italic | `_words_` |
| Bullet list | `- item` |
| Numbered list | `1. step` |
| Link | `[text](https://example.com)` or `[text](other-page.md)` |
| Picture | `![description](my-page.assets/picture.png)` |
| Table | `\| A \| B \|`, then `\| --- \| --- \|`, then one line per row |
| Code | `` `code` ``, or three backticks on their own lines around a block |

Raw HTML typed into a page is shown as text; it never runs.

## Pictures

Add a picture in any of three ways:

- Choose **Insert picture…** and pick a file.
- Copy a picture (for example a screenshot) and paste it into the Write box.
- Drag a picture file from a folder onto the Write box.

Cairn asks for a short **description**. It's read aloud to people who can't see the picture and shown if the picture can't be loaded.

**Accepted:** PNG, JPEG, WebP, and GIF, up to 10 MB and 8000 pixels wide or tall by default. SVG and other formats are refused, with a message explaining why. Cairn checks the file's actual content, not just its name.

**Where pictures go.** Until you publish, a new picture is kept privately with your unsaved changes and shown only in your preview. When you publish, it is copied next to the page:

```
Guides/new-user-setup.md
Guides/new-user-setup.assets/screenshot-a1b2c3d4.png
```

The page refers to it with a relative link (`new-user-setup.assets/screenshot-a1b2c3d4.png`), so it works on every computer and in other Markdown viewers. The name ends with part of a fingerprint of the picture, so two different pictures never collide.

If you remove a picture from the text before publishing, it is never copied. If you remove a picture from an already published page, the file stays in the `.assets` folder, because earlier versions of the page may still use it.

## Page details

Open **Page details** in the editor to set:

- **Owner**: who looks after the page.
- **Status**: *Draft* (still being written), *Active* (up to date), or *Retired* (no longer used).
- **Last reviewed**: choose a date, or choose **Today**.
- **Tags**: words separated by commas.

These are stored in a small block at the top of the page:

```
---
owner: Sam
status: active
last_reviewed: 2026-01-31
tags: [printers, office]
---
```

If this block is typed wrongly, the page still opens. A notice explains which detail couldn't be read.

## Unsaved changes

Your unsaved changes are saved on **your own computer** every few seconds, never in the shared folder. The editor shows:

- **Draft saved at 2:32 PM**: safely saved on this computer.
- **Kept in this window only**: persistent saving is turned off (see [Configuration](configuration.md)). Publish before closing Cairn.
- An error message in red: saving failed. Keep the window open, then publish or try again.

If you close the editor, close Cairn, or your computer restarts, your changes are kept. Next time you edit the page, Cairn offers **Continue with my changes** or **Start again from the published page**, and can show the differences. Pictures you added are kept with your unsaved changes too.

## Publishing

Choose **Publish changes**. Before anything is written, Cairn checks that:

1. You still have the page locked.
2. You're allowed to change files in that folder.
3. Nobody changed the page since you started editing, whether in Cairn or in another program.

Then it saves the new pictures, keeps a copy of the old page under **Earlier versions**, and replaces the page. It reads the result back to make sure it was saved correctly before saying **Published**. If anything fails along the way, the published page is left exactly as it was and your text is kept.

### When someone else changed the page

If the page changed while you were writing, nothing is published. Cairn shows **Someone else changed this page while you were writing**, with the differences:

- lines marked **−** are only in the version that's published now;
- lines marked **+** are only in yours.

Choose **Keep editing** to copy anything you need into your version first. If you're sure, choose **Publish my version anyway**. The other version is still kept under **Earlier versions**.

### Discarding or closing

- **Discard my changes** removes your unsaved changes and leaves the published page as it was. Cairn asks you to confirm, because this can't be undone.
- **Close editor** keeps your unsaved changes on this computer and unlocks the page so others can edit.

## Stepping away

If you stop typing for **15 minutes**, Cairn asks **Are you still editing?**

- **Keep editing**: carry on.
- **Unlock the page now**: let others edit. Your text stays in the editor.

After **20 minutes** without typing, the page is unlocked automatically so others aren't blocked. Your text stays in the editor and on your computer. To publish it, choose **Continue editing** to lock the page again. If someone published in the meantime, you'll be shown both versions first.

Only typing, pasting, adding pictures, and using editor buttons count as activity. Moving the mouse or leaving the page open does not. The times can be changed in **Settings**.

## Earlier versions

On any page, choose **Earlier versions** to see every version that was replaced by a publish. For each one you can:

- **View this version** to read it, and see what's different from the current page;
- **Restore this version** to make it the current page again. The page it replaces is kept in the list, so a restore can itself be undone.

Earlier versions are ordinary Markdown files in `_system/history/`.

## Templates

A template gives new pages a ready-made structure, such as headings for meeting notes or an incident report. Choose **Templates** in the sidebar to see them:

- **Your team's templates** belong to this documentation folder. Other documentation folders have their own.
- **Built in** templates come with Cairn.

To make one, choose **New template**, give it a name and a short description, and pick what to start from (Blank, or a copy of another template). It opens in the editor with a notice that you're editing a template for the whole team. In the editor:

- **Template details** holds the name and description people see when they choose a template.
- The **Insert** buttons add fields that are filled in for each new page: **Page title**, **Today's date**, **Author's name**, and **Folder name**. You'll see them written as `{{title}}`, `{{date}}`, `{{author}}`, and `{{folder}}`.
- The preview shows example values in their place.

Choose **Publish template** to make it available. Editing a template later only affects pages created afterwards.

**Delete** asks first. Deleted templates are listed under **Deleted templates** on the same page, with **Restore** to bring one back.

To use a template, choose **Use for a new page** beside it, or pick it under **Start from** on the New page screen.

## Downloads

Choose **Download** on any page:

- **PDF document**: for reading, printing, or sending to someone. Pictures are included, with the owner, status, and dates at the top.
- **Markdown with pictures (.zip)**: for backups or moving to another tool.
- **Markdown file (.md)**: just the text.

On a folder, **Download this folder** saves all its pages (and folders inside it) as one .zip, either as Markdown with pictures or as one PDF per page. **Settings → Download everything** does the same for the whole documentation, including templates. A progress bar shows how far along it is, and **Close** stops it.

PDFs are made on your own computer by Microsoft Edge or Google Chrome; nothing is uploaded. If neither works on your computer, Cairn offers to **open the print window instead**: choose **Save as PDF** (or **Microsoft Print to PDF**) as the printer.

## Times and timezones

Every time in Cairn is shown in your timezone, with its difference from GMT, for example “2:30 PM GMT+8”. By default that's your computer's timezone. To change it, go to **Settings → Time and timezone**, choose **Choose a timezone**, then a region and a city. Cairn stores every time in one standard form, so people in different timezones always see the same moment in their own local time.
