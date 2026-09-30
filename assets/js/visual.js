// Visual editing: the page as it will look, and editable. Markdown shortcuts
// work as you type (# and a space makes a heading, - a bullet list, **words**
// bold, and so on).
//
// Built on Milkdown (ProseMirror + remark, MIT), which is loaded only the
// first time someone switches to visual editing. The Markdown text stays the
// single source of truth: every change made here is written back to it, so
// drafts, locks, publishing, and history work exactly as before.

import { h, encodePath } from './core.js';

let lib = null;
const load = async () => {
  lib ??= await import('/static/vendor/milkdown.js');
  return lib;
};

function decodeSafe(s) {
  try { return decodeURIComponent(s); } catch { return s; }
}

/** A picture link relative to the page ("page.assets/x.png") → workspace path. */
function resolveRelative(pagePath, src) {
  const parts = pagePath.split('/').slice(0, -1);
  for (const seg of decodeSafe(src.split(/[?#]/)[0]).split('/')) {
    if (seg === '..') parts.pop();
    else if (seg && seg !== '.') parts.push(seg);
  }
  return parts.join('/');
}

const isExternal = (src) => /^[a-z][a-z0-9+.-]*:/i.test(src) || src.startsWith('//');

/** Pictures and links without a title come out of the Markdown parser with
 *  `title: null`, which the editor's schema rejects (dropping the picture).
 *  An empty title is equivalent and is still written back without one. */
function emptyTitles() {
  const walk = (node) => {
    if (['image', 'link', 'definition'].includes(node.type) && node.title == null) node.title = '';
    if (node.children) node.children.forEach(walk);
  };
  return walk;
}

/**
 * Create the visual editor in `root`. `onChange(markdown)` is called with
 * the page body after every edit made here.
 */
/** The visual editor writes a callout's `[!WARNING]` as `\[!WARNING]`, which
 *  would make it a plain quote: put the callout back. */
const CALLOUT_ESCAPED = /^(\s*(?:>\s*)+)\\?\[!(NOTE|TIP|IMPORTANT|WARNING|CAUTION)\]\\?[ \t]*$/gim;
export const unescapeCallouts = (md) => md.replace(CALLOUT_ESCAPED, '$1[!$2]');

export async function createVisualEditor({ root, markdown, pagePath, readKey, staged = [], onChange }) {
  const m = await load();
  const draftUrl = (name) => `/draft-file?${new URLSearchParams({ article: pagePath, name, k: readKey })}`;
  const stagedUrls = new Map(staged.map((s) => [s.target_rel.toLowerCase(), draftUrl(s.name)]));

  // Pictures are shown from the same places the preview uses; the Markdown
  // keeps its ordinary relative links.
  const imageView = (node) => {
    const src = node.attrs.src || '';
    if (isExternal(src)) {
      const label = node.attrs.alt || 'picture';
      return { dom: h('span', { class: 'external-image' }, `External picture (not loaded): ${label}`) };
    }
    const rel = resolveRelative(pagePath, src);
    const url = stagedUrls.get(rel.toLowerCase()) || `/ws-file/${encodePath(rel)}?k=${readKey}`;
    return { dom: h('img', { src: url, alt: node.attrs.alt || '', title: node.attrs.title || null }) };
  };

  let known = markdown; // the Markdown this editor last produced or was given
  const editor = await m.Editor.make()
    .config((ctx) => {
      ctx.set(m.rootCtx, root);
      ctx.set(m.defaultValueCtx, markdown);
      ctx.update(m.remarkPluginsCtx, (plugins) => [...plugins, { plugin: emptyTitles, options: {} }]);
      // Write Markdown the way Cairn's own buttons do.
      ctx.update(m.remarkStringifyOptionsCtx, (opts) => ({ ...opts, bullet: '-', emphasis: '_', strong: '*', rule: '-' }));
      ctx.update(m.editorViewOptionsCtx, (prev) => ({
        ...prev,
        attributes: {
          class: 'visual-doc md-body',
          spellcheck: 'true',
          role: 'textbox',
          'aria-multiline': 'true',
          'aria-label': 'Page text',
        },
        nodeViews: { image: imageView },
      }));
      ctx.get(m.listenerCtx).markdownUpdated((_ctx, written) => {
        const md = unescapeCallouts(written);
        if (md === known) return;
        known = md;
        onChange(md);
      });
    })
    .use(m.commonmark)
    // Typing right after a link continues as normal text (as in a word
    // processor), instead of making the link longer.
    .use(m.linkSchema.extendSchema((prev) => (ctx) => ({ ...prev(ctx), inclusive: false })))
    .use(m.gfm)
    .use(m.history)
    .use(m.listener)
    .use(m.clipboard)
    .create();
  // Milkdown writes Markdown in its own tidy form; remember that form so
  // simply opening the page doesn't count as a change.
  known = unescapeCallouts(editor.action(m.getMarkdown()));

  const view = () => editor.ctx.get(m.editorViewCtx);

  // Checklists: the ticked state is kept on each list item, and the box is
  // drawn in front of it (see .visual-doc li[data-item-type="task"]).
  // Clicking the box, or Ctrl+Enter in the item, ticks or unticks it; the
  // change reaches the Markdown like any other edit.
  const toggleTask = (pos) => {
    const v = view();
    const $pos = v.state.doc.resolve(pos);
    for (let depth = $pos.depth; depth > 0; depth--) {
      const node = $pos.node(depth);
      if (node.type.name !== 'list_item') continue;
      if (node.attrs.checked == null) return false;
      v.dispatch(v.state.tr.setNodeMarkup($pos.before(depth), undefined,
        { ...node.attrs, checked: !node.attrs.checked }));
      return true;
    }
    return false;
  };
  root.addEventListener('mousedown', (e) => {
    const item = e.target.closest?.('li[data-item-type="task"]');
    if (!item || !root.contains(item)) return;
    // Only the box itself, which sits in the item's left padding.
    const box = item.getBoundingClientRect();
    const padding = parseFloat(getComputedStyle(item).paddingLeft) || 0;
    if (e.clientX > box.left + padding) return;
    e.preventDefault();
    toggleTask(view().posAtDOM(item, 0));
  });
  root.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey)) return;
    if (toggleTask(view().state.selection.from)) e.preventDefault();
  });
  // Tab in a table's last cell adds a row and moves into it, as in Word.
  // (Caught before the editor's own Tab, which would leave the table.)
  root.addEventListener('keydown', (e) => {
    if (e.key !== 'Tab' || e.shiftKey || e.ctrlKey || e.altKey) return;
    const { state } = view();
    if (!m.isInTable(state)) return;
    const rect = m.selectedRect(state);
    if (rect.top !== rect.map.height - 1 || rect.left !== rect.map.width - 1) return;
    e.preventDefault();
    e.stopPropagation();
    editor.action(m.callCommand(m.addRowAfterCommand.key));
    const after = view().state;
    const r = m.selectedRect(after);
    const pos = r.tableStart + r.map.map[(r.top + 1) * r.map.width];
    view().dispatch(after.tr.setSelection(m.TextSelection.near(after.doc.resolve(pos + 1))));
  }, true);

  const commands = {
    bold: m.toggleStrongCommand,
    italic: m.toggleEmphasisCommand,
    code: m.toggleInlineCodeCommand,
    bullet: m.wrapInBulletListCommand,
    ordered: m.wrapInOrderedListCommand,
    heading: m.wrapInHeadingCommand,
    paragraph: m.turnIntoTextCommand,
    table: m.insertTableCommand,
    strike: m.toggleStrikethroughCommand,
    quote: m.wrapInBlockquoteCommand,
    divider: m.insertHrCommand,
    undo: m.undoCommand,
    redo: m.redoCommand,
  };

  const inListItem = () => {
    const { $from } = view().state.selection;
    for (let d = $from.depth; d > 0; d--) if ($from.node(d).type.name === 'list_item') return true;
    return false;
  };

  return {
    /** Run a formatting command by name (see `commands`). */
    run(name, payload) {
      if (name === 'divider') {
        // A line goes in after the paragraph (or list) the cursor is in:
        // it never replaces words or splits a sentence.
        const v = view();
        const { state } = v;
        const { $to } = state.selection;
        const pos = $to.depth ? $to.after(1) : $to.pos;
        v.dispatch(state.tr.insert(pos, state.schema.nodes.hr.create()).scrollIntoView());
        v.focus();
        return;
      }
      if (name === 'table') {
        // A table goes in after the selected words; it never replaces them.
        const { state } = view();
        view().dispatch(state.tr.setSelection(m.TextSelection.create(state.doc, state.selection.to)));
      }
      editor.action(m.callCommand(commands[name].key, payload));
      view().focus();
    },
    /**
     * Where the cursor is in a table, or null: row and column (0-based; row
     * 0 is the heading row), how many of each, and the column's alignment.
     */
    tableState() {
      const { state } = view();
      if (!m.isInTable(state)) return null;
      const rect = m.selectedRect(state);
      const cell = rect.table.nodeAt(rect.map.map[rect.top * rect.map.width + rect.left]);
      return {
        row: rect.top, col: rect.left, rows: rect.map.height, cols: rect.map.width,
        align: cell?.attrs.alignment || 'left',
      };
    },
    /**
     * Change the table the cursor is in. Markdown tables always have one
     * heading row and at least one other row, so actions that would break
     * that do nothing (the table bar disables them).
     */
    tableAction(name, arg) {
      const v = view();
      const t = this.tableState();
      if (!t) return;
      const call = (cmd, payload) => editor.action(m.callCommand(cmd.key, payload));
      const run = (command) => command(v.state, v.dispatch);
      // Moving or deleting can leave the cursor outside the table: put it
      // back in the given cell (or the nearest one left), so the next
      // action still works.
      const start = m.selectedRect(v.state).tableStart;
      const caretTo = (row, col) => {
        const table = v.state.doc.nodeAt(start - 1);
        if (!table || table.type.name !== 'table') return;
        const r = Math.min(row, table.childCount - 1);
        let pos = start;
        for (let i = 0; i < r; i++) pos += table.child(i).nodeSize;
        pos += 1; // into the row
        const c = Math.min(col, table.child(r).childCount - 1);
        for (let i = 0; i < c; i++) pos += table.child(r).child(i).nodeSize;
        v.dispatch(v.state.tr.setSelection(m.TextSelection.near(v.state.doc.resolve(pos + 1))));
      };
      const move = (mover, from, to, row, col) => {
        run(mover({ from, to, select: false }));
        caretTo(row, col);
      };
      switch (name) {
        case 'rowAbove': if (t.row > 0) call(m.addRowBeforeCommand); break;
        case 'rowBelow': call(m.addRowAfterCommand); break;
        case 'colLeft': call(m.addColBeforeCommand); break;
        case 'colRight': call(m.addColAfterCommand); break;
        case 'deleteRow': if (t.row > 0 && t.rows > 2) { run(m.deleteRow); caretTo(t.row, t.col); } break;
        case 'deleteCol': if (t.cols > 1) { run(m.deleteColumn); caretTo(t.row, t.col); } break;
        case 'deleteTable': run(m.deleteTable); break;
        case 'rowUp': if (t.row > 1) move(m.moveTableRow, t.row, t.row - 1, t.row - 1, t.col); break;
        case 'rowDown': if (t.row > 0 && t.row < t.rows - 1) move(m.moveTableRow, t.row, t.row + 1, t.row + 1, t.col); break;
        case 'colLeftMove': if (t.col > 0) move(m.moveTableColumn, t.col, t.col - 1, t.row, t.col - 1); break;
        case 'colRightMove': if (t.col < t.cols - 1) move(m.moveTableColumn, t.col, t.col + 1, t.row, t.col + 1); break;
        case 'align': {
          // Markdown aligns whole columns, so every cell in it changes.
          const { state } = v;
          const rect = m.selectedRect(state);
          let tr = state.tr;
          for (let r = 0; r < rect.map.height; r++) {
            const pos = rect.tableStart + rect.map.map[r * rect.map.width + rect.left];
            const cell = state.doc.nodeAt(pos);
            if (cell) tr = tr.setNodeMarkup(pos, undefined, { ...cell.attrs, alignment: arg });
          }
          v.dispatch(tr);
          break;
        }
        default: break;
      }
      v.focus();
    },
    /** When the cursor ends a paragraph that is just "/word", the word
     *  (maybe empty); otherwise null. Drives the slash menu. */
    slashQuery() {
      const { selection } = view().state;
      const { $from } = selection;
      if (!selection.empty || $from.parent.type.name !== 'paragraph') return null;
      const text = $from.parent.textContent;
      if ($from.parentOffset !== text.length) return null;
      const match = /^\/(\S{0,20})$/.exec(text);
      return match ? match[1] : null;
    },
    /** Empty the "/word" paragraph, ready for what the slash menu inserts. */
    clearSlash() {
      const v = view();
      const { $from } = v.state.selection;
      v.dispatch(v.state.tr.delete($from.start(), $from.end()));
    },
    /** Where the cursor is on screen, to place the slash menu beside it. */
    caretRect() {
      const v = view();
      return v.coordsAtPos(v.state.selection.from);
    },
    /** Put the paragraph in a callout box of this kind ("warning", …). */
    callout(kind) {
      const inQuote = () => {
        const { $from } = view().state.selection;
        for (let d = $from.depth; d > 0; d--) if ($from.node(d).type.name === 'blockquote') return d;
        return 0;
      };
      if (!inQuote()) editor.action(m.callCommand(m.wrapInBlockquoteCommand.key));
      const v = view();
      const depth = inQuote();
      if (depth) {
        const { $from } = v.state.selection;
        const first = $from.node(depth).firstChild;
        if (!first?.textContent.startsWith('[!')) {
          const s = v.state.schema;
          const pos = $from.start(depth) + 1; // inside its first paragraph
          v.dispatch(v.state.tr.insert(pos, [s.text(`[!${kind.toUpperCase()}]`), s.nodes.hardbreak.create()]));
        }
      }
      v.focus();
    },
    /** Turn the selected lines into a checklist, or back into a plain list
     *  when they already are one. */
    checklist() {
      if (!inListItem()) editor.action(m.callCommand(m.wrapInBulletListCommand.key));
      const v = view();
      const { from, to } = v.state.selection;
      const items = [];
      v.state.doc.nodesBetween(from, to, (node, pos) => {
        if (node.type.name === 'list_item') items.push([node, pos]);
      });
      const makeTasks = items.some(([node]) => node.attrs.checked == null);
      let tr = v.state.tr;
      for (const [node, pos] of items) {
        tr = tr.setNodeMarkup(pos, undefined, { ...node.attrs, checked: makeTasks ? (node.attrs.checked ?? false) : null });
      }
      if (tr.docChanged) v.dispatch(tr);
      v.focus();
    },
    /** Insert Markdown at the cursor, replacing any selection. */
    insert(md, inline = false) {
      editor.action(m.insert(md, inline));
      view().focus();
    },
    /** Pass on any edit not yet reported (changes are reported after a short pause). */
    flush() {
      const md = unescapeCallouts(editor.action(m.getMarkdown()));
      if (md === known) return;
      known = md;
      onChange(md);
    },
    /** Replace the whole page body, e.g. after editing the Markdown directly. */
    setMarkdown(md) {
      if (md === known) return;
      editor.action(m.replaceAll(md, true));
      known = unescapeCallouts(editor.action(m.getMarkdown()));
    },
    /** Replace the page's text as one change that Undo can take back
     *  (used by Replace all). */
    replaceText(md) {
      editor.action(m.replaceAll(md));
      view().focus();
    },
    selectedText() {
      const { from, to } = view().state.selection;
      return view().state.doc.textBetween(from, to, ' ');
    },
    /** Heading level of the block the cursor is in; 0 for normal text. */
    headingLevel() {
      const node = view().state.selection.$from.parent;
      return node.type.name === 'heading' ? node.attrs.level : 0;
    },
    /** A picture was just added to the draft: show it from there. */
    addStaged(link, name) {
      stagedUrls.set(resolveRelative(pagePath, link).toLowerCase(), draftUrl(name));
    },
    focus: () => view().focus(),
    destroy: () => editor.destroy(),
  };
}
