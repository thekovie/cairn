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
      ctx.update(m.remarkStringifyOptionsCtx, (opts) => ({ ...opts, bullet: '-', emphasis: '_', strong: '*' }));
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
      ctx.get(m.listenerCtx).markdownUpdated((_ctx, md) => {
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
  known = editor.action(m.getMarkdown());

  const view = () => editor.ctx.get(m.editorViewCtx);
  const commands = {
    bold: m.toggleStrongCommand,
    italic: m.toggleEmphasisCommand,
    code: m.toggleInlineCodeCommand,
    bullet: m.wrapInBulletListCommand,
    ordered: m.wrapInOrderedListCommand,
    heading: m.wrapInHeadingCommand,
    paragraph: m.turnIntoTextCommand,
    table: m.insertTableCommand,
  };

  return {
    /** Run a formatting command by name (see `commands`). */
    run(name, payload) {
      if (name === 'table') {
        // A table goes in after the selected words; it never replaces them.
        const { state } = view();
        view().dispatch(state.tr.setSelection(m.TextSelection.create(state.doc, state.selection.to)));
      }
      editor.action(m.callCommand(commands[name].key, payload));
      view().focus();
    },
    /** Insert Markdown at the cursor, replacing any selection. */
    insert(md, inline = false) {
      editor.action(m.insert(md, inline));
      view().focus();
    },
    /** Pass on any edit not yet reported (changes are reported after a short pause). */
    flush() {
      const md = editor.action(m.getMarkdown());
      if (md === known) return;
      known = md;
      onChange(md);
    },
    /** Replace the whole page body, e.g. after editing the Markdown directly. */
    setMarkdown(md) {
      if (md === known) return;
      editor.action(m.replaceAll(md, true));
      known = editor.action(m.getMarkdown());
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
