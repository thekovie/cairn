// The page editor.
//
// Designed so a page can be written and formatted without knowing Markdown:
// worded toolbar buttons insert the right characters, the preview updates as
// you type, and "Page details" edits the information block for you.

import {
  api, get, post, h, clear, icon, button, linkButton, banner, href, toast, announce, whileBusy,
  openDialog, confirmDialog, formDialog, errorText, diffView, formatTime, formatDateTime, todayYmd,
  iconButton, menuButton, toolbarKeys, formatSize, diffSummary, lineDiff,
} from './core.js';
import { createVisualEditor } from './visual.js';
import { scrollTogether } from './scrollsync.js';
import { lockSentence } from './views.js';

const SAVE_DELAY_MS = 2500;
const SAVE_MAX_WAIT_MS = 8000;
const PREVIEW_DELAY_MS = 400;
const ACTIVITY_EVERY_MS = 20000;
const STATUS_EVERY_MS = 10000;

// ------------------------------------------------------ page details block

const META_KEYS = ['owner', 'status', 'last_reviewed', 'tags'];
const FRONT = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/;

function unquote(v) {
  const t = v.trim();
  if ((t.startsWith('"') && t.endsWith('"')) || (t.startsWith("'") && t.endsWith("'"))) {
    return t.slice(1, -1).replace(/\\"/g, '"').replace(/\\\\/g, '\\');
  }
  return t;
}

function yamlValue(v) {
  if (v === '') return '""';
  return /[:#,[\]{}&*!|>'"%@`]|^\s|\s$|^(true|false|null|yes|no|~)$/i.test(v)
    ? `"${v.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`
    : v;
}

export function readMeta(text) {
  const m = text.match(FRONT);
  const fields = { owner: '', status: '', last_reviewed: '', tags: '' };
  if (!m) return { fields, hasBlock: false };
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_]+):\s*(.*)$/);
    if (!kv || !META_KEYS.includes(kv[1])) continue;
    let value = kv[2];
    if (kv[1] === 'tags') {
      value = value.trim().replace(/^\[|\]$/g, '');
      value = value.split(',').map(unquote).filter(Boolean).join(', ');
    } else {
      value = unquote(value);
    }
    fields[kv[1]] = value;
  }
  return { fields, hasBlock: true };
}

export function writeMeta(text, fields) {
  const m = text.match(FRONT);
  const others = [];
  if (m) {
    for (const line of m[1].split(/\r?\n/)) {
      const kv = line.match(/^([A-Za-z_]+):/);
      if (kv && META_KEYS.includes(kv[1])) continue;
      if (line.trim() !== '') others.push(line);
    }
  }
  const lines = [];
  if (fields.owner.trim()) lines.push(`owner: ${yamlValue(fields.owner.trim())}`);
  if (fields.status) lines.push(`status: ${fields.status}`);
  if (fields.last_reviewed) lines.push(`last_reviewed: ${fields.last_reviewed}`);
  const tags = fields.tags.split(',').map((t) => t.trim()).filter(Boolean);
  if (tags.length) lines.push(`tags: [${tags.map(yamlValue).join(', ')}]`);
  lines.push(...others);
  const body = m ? text.slice(m[0].length) : text;
  if (!lines.length) return body.replace(/^\r?\n/, '');
  return `---\n${lines.join('\n')}\n---\n${m ? '' : '\n'}${body}`;
}

// ------------------------------------------------------------ visual mode

const VIEW_KEY = 'cairn.editView';
const SCROLL_KEY = 'cairn.scrollTogether';
/** The Link dialog's choice for "a heading on this page". */
const THIS_PAGE = ':this-page';
const HEIGHT_KEY = 'cairn.editorHeight';
const EDITOR_MIN_HEIGHT = 224;
const VIEWS = ['visual', 'both', 'write'];

/** Split the page details block off the top, so visual editing never touches it. */
export function splitFront(text) {
  const m = text.match(FRONT);
  if (!m) return { front: '', body: text };
  const rest = text.slice(m[0].length);
  const gap = rest.match(/^(?:\r?\n)*/)[0];
  return { front: m[0] + gap, body: rest.slice(gap.length) };
}

// --------------------------------------------------------- template details

const TEMPLATE_KEYS = ['template_name', 'template_description'];

export const isTemplatePath = (path) => /^_templates\/[^/]+\.md$/i.test(path);

export function readTemplateMeta(text) {
  const fields = { template_name: '', template_description: '' };
  const m = text.match(FRONT);
  if (!m) return fields;
  for (const line of m[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z_]+):\s*(.*)$/);
    if (kv && TEMPLATE_KEYS.includes(kv[1])) fields[kv[1]] = unquote(kv[2]);
  }
  return fields;
}

export function writeTemplateMeta(text, fields) {
  const m = text.match(FRONT);
  const others = m
    ? m[1].split(/\r?\n/).filter((line) => {
      const kv = line.match(/^([A-Za-z_]+):/);
      return !(kv && TEMPLATE_KEYS.includes(kv[1])) && line.trim() !== '';
    })
    : [];
  const lines = [`template_name: ${yamlValue(fields.template_name.trim())}`];
  if (fields.template_description.trim()) {
    lines.push(`template_description: ${yamlValue(fields.template_description.trim())}`);
  }
  const body = m ? text.slice(m[0].length) : `\n${text}`;
  return `---\n${[...lines, ...others].join('\n')}\n---\n${body}`;
}

/** Fill-in fields shown with example values in the template preview. */
function withExampleValues(text, author) {
  return text
    .replaceAll('{{title}}', 'Example page title')
    .replaceAll('{{date}}', todayYmd())
    .replaceAll('{{author}}', author || 'Your name')
    .replaceAll('{{folder}}', 'Example folder');
}

// ------------------------------------------------------------ small helpers

function relativeLink(fromPath, toPath) {
  const from = fromPath.split('/').slice(0, -1);
  const to = toPath.split('/');
  let i = 0;
  while (i < from.length && i < to.length - 1 && from[i].toLowerCase() === to[i].toLowerCase()) i++;
  const up = from.slice(i).map(() => '..');
  return [...up, ...to.slice(i).map(encodeURIComponent)].join('/');
}

function folderOf(path) {
  return path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '';
}

function helpTable() {
  const rows = [
    ['Heading', '## Heading', 'A section title. Use one # for the page title, and ### to ###### for smaller headings.'],
    ['Bold', '**words**', 'Makes words stand out.'],
    ['Italic', '_words_', 'Slanted words.'],
    ['Strikethrough', '~~words~~', 'Crossed out, for something that no longer applies.'],
    ['Bullet list', '- item', 'One item per line.'],
    ['Numbered list', '1. step', 'Numbers are filled in for you.'],
    ['Checklist', '- [ ] item', 'A box to tick. - [x] is ticked.'],
    ['Note box', '> text', 'A tinted box for tips and warnings.'],
    ['Callout', '> [!WARNING]', 'On its own line, then > text. Also NOTE, TIP, IMPORTANT, CAUTION.'],
    ['Divider line', '---', 'On a line of its own, with an empty line above and below.'],
    ['Link', '[text](https://example.com)', 'Or a link to another page.'],
    ['Picture', '![description](picture.png)', 'Use “Insert picture…”; it does this for you.'],
    ['Code', '`code`', 'For commands or exact text to type.'],
  ];
  return h('table', { class: 'help-table' },
    h('thead', null, h('tr', null,
      h('th', { scope: 'col' }, 'To get'), h('th', { scope: 'col' }, 'Type'), h('th', { scope: 'col' }, 'Notes'))),
    h('tbody', null, rows.map(([a, b, c]) => h('tr', null, h('td', null, a), h('td', null, h('code', null, b)), h('td', null, c)))));
}

// --------------------------------------------------------------- the view

export async function editorView(ctx) {
  const path = ctx.path;
  const isNew = ctx.query.get('new') === '1';
  const newKey = `cairn.new.${path}`;
  let initial = null;
  if (isNew) {
    try { initial = sessionStorage.getItem(newKey); } catch { initial = null; }
  }

  let start = await post('/api/edit/start', { path, is_new: isNew, initial_content: initial });
  if (start.status === 'locked') return lockedScreen(ctx, path, start.lock);
  if (isNew) {
    try { sessionStorage.removeItem(newKey); } catch { /* ignore */ }
  }

  if (start.restore) {
    const useDraft = await restoreDialog(start.restore);
    if (!useDraft) {
      await post('/api/draft/discard', { path, keep_lock: true });
      start = await post('/api/edit/start', { path, is_new: start.is_new, initial_content: initial });
      if (start.status === 'locked') return lockedScreen(ctx, path, start.lock);
    }
  }

  return mountEditor(ctx, path, start);
}

function lockedScreen(ctx, path, lock) {
  const back = isTemplatePath(path)
    ? linkButton('Back to templates', href.templates(), { icon: 'back', kind: 'primary' })
    : linkButton('Back to the page', href.page(path), { icon: 'back', kind: 'primary' });
  const actions = [
    back,
    button('Try again', { icon: 'refresh', onClick: () => window.dispatchEvent(new HashChangeEvent('hashchange')) }),
  ];
  if (lock.reclaimable_by_me) {
    actions.unshift(button('Continue where you left off', {
      icon: 'edit', kind: 'primary',
      onClick: async (e) => {
        try {
          await whileBusy(e.currentTarget, 'Opening…', () => post('/api/edit/reclaim', { path }));
          window.dispatchEvent(new HashChangeEvent('hashchange'));
        } catch (err) { toast(errorText(err), { error: true }); }
      },
    }));
  }
  ctx.main.append(
    h('div', { class: 'page-head' }, h('h1', null, 'This page is being edited')),
    banner({
      tone: 'warn', title: `${lockSentence(lock)}.`,
      text: lock.possibly_abandoned
        ? 'They haven’t been active for a while, so the page may have been left open by accident. A maintainer can release it by following “Abandoned edit locks” in the Troubleshooting guide.'
        : 'Only one person can edit a page at a time. You can read the page while you wait; editing opens up when they finish.',
      actions,
    }));
  return { title: 'Page is being edited' };
}

async function restoreDialog(restore) {
  const when = formatDateTime(restore.updated_at);
  const pictures = restore.staged_pictures;
  const body = [
    h('p', null, `You have unsaved changes to this page from ${when}. They were kept on this computer.`),
    restore.published_changed
      ? banner({ tone: 'warn', text: 'Someone has published a newer version of the page since then. If you continue with your changes, you will be shown both versions before anything is published.' })
      : null,
    pictures ? h('p', null, pictures === 1 ? 'They include 1 picture you added.' : `They include ${pictures} pictures you added.`) : null,
    h('details', { class: 'details-panel' },
      h('summary', null, 'Show how my changes differ from the published page'),
      h('div', { class: 'details-body' }, diffView(restore.diff))),
  ];
  const choice = await openDialog({
    title: 'Continue with your unsaved changes?', iconName: 'history', tone: 'info', wide: true, body,
    actions: [
      { label: 'Start again from the published page', value: 'published' },
      { label: 'Continue with my changes', value: 'draft', kind: 'primary', autofocus: true },
    ],
  });
  if (choice !== 'published') return true;
  const sure = await confirmDialog({
    title: 'Remove your unsaved changes?',
    message: 'Your unsaved changes will be removed and you will start again from the published page. This can’t be undone.',
    confirmLabel: 'Remove my changes', cancelLabel: 'Keep my changes', danger: true, iconName: 'trash',
  });
  return !sure;
}

function mountEditor(ctx, path, start) {
  const s = {
    lockHeld: true,
    dirty: false,
    firstDirtyAt: 0,
    saveTimer: 0,
    previewTimer: 0,
    lastActivitySent: Date.now(),
    idleOpen: null,
    closed: false,
    persistent: start.persistent,
    isNew: start.is_new,
  };
  const isTemplate = isTemplatePath(path);
  const author = ctx.app.state?.user?.display_name || '';
  // Where "Close" and "Discard" lead back to.
  const leaveTo = () => {
    if (isTemplate) return href.templates();
    return s.isNew ? href.folder(folderOf(path)) : href.page(path);
  };

  // ---------------------------------------------------------------- DOM
  const titleEl = h('h1', null, 'Editing');
  const lockStatus = h('span', { class: 'status-item is-ok' });
  const saveStatus = h('span', { class: 'status-item', role: 'status' });
  // How long the page is, at the foot of whichever view is showing.
  const wordCounts = [h('span', { class: 'word-count' }), h('span', { class: 'word-count' })];
  const notices = h('div');
  const ta = h('textarea', { id: 'md-text', spellcheck: 'true', 'aria-describedby': 'drop-hint' });
  // The page details block (owner, status, …) is kept out of the text box:
  // it looks technical and is edited with "Page details". What is saved,
  // previewed, and published is always the whole page: details + text.
  let front = '';
  const fullText = () => front + ta.value;
  const setFullText = (text) => {
    const parts = splitFront(text);
    front = parts.front;
    ta.value = parts.body;
    countWords();
  };
  setFullText(start.content);
  // Until the first preview arrives (slow on a slow shared folder), say so.
  const previewBody = h('div', { class: 'md-body' },
    h('p', { class: 'loading-label' }, h('span', { class: 'spinner', 'aria-hidden': 'true' }), 'Preparing the preview…'));
  // Tables are changed row by row in the visual view; say so when the
  // cursor is on a table line here.
  const tableCodeHint = h('p', { class: 'help table-code-hint', hidden: true },
    'To add or remove rows and columns, switch to “As it will look”: a table bar appears when you click in the table.');
  const writePane = h('div', { class: 'pane pane-write' },
    h('label', { class: 'pane-label', for: 'md-text' }, 'Page text with formatting codes'),
    tableCodeHint,
    ta,
    h('div', { class: 'pane-foot' },
      h('p', { class: 'drop-hint', id: 'drop-hint' }, 'Tip: you can paste a picture here, or drag one in from a folder. “Formatting help” below lists the codes.'),
      wordCounts[0]));
  // Visual editing: type on the page as it will look (see visual.js).
  const visualRoot = h('div', { class: 'visual-root' });
  const visualPane = h('section', { class: 'pane pane-visual', 'aria-labelledby': 'visual-h' },
    h('h2', { class: 'pane-label', id: 'visual-h' }, 'The page as it will look: click anywhere to write'),
    visualRoot,
    h('div', { class: 'pane-foot' },
      h('p', { class: 'drop-hint' }, 'Tip: use the buttons above, or type / on an empty line. You can paste or drag in pictures.'),
      wordCounts[1]));
  // Everyone starts on the page as it will look. Showing the formatting
  // codes is a choice, remembered in this browser once someone makes it.
  let savedView = null;
  try { savedView = localStorage.getItem(VIEW_KEY); } catch { savedView = null; }
  const startView = VIEWS.includes(savedView) ? savedView : 'visual';
  let scrollPref = null;
  try { scrollPref = localStorage.getItem(SCROLL_KEY); } catch { scrollPref = null; }
  const scrollBox = h('input', { type: 'checkbox', id: 'scroll-together', checked: scrollPref !== 'off' });
  const panes = h('div', { class: 'editor-panes', 'data-view': startView },
    visualPane,
    writePane,
    h('section', { class: 'pane pane-preview', 'aria-labelledby': 'preview-h' },
      h('div', { class: 'pane-head' },
        h('h2', { class: 'pane-label', id: 'preview-h' }, 'Preview: how the page will look'),
        h('label', { class: 'check pane-option', for: 'scroll-together' },
          scrollBox, h('span', null, 'Scroll together'))),
      previewBody));
  // A link to a heading on this page scrolls the preview to it, instead of
  // changing the address (which would lose the editor's place).
  previewBody.addEventListener('click', (e) => {
    const target = e.target.closest('a')?.getAttribute('href') || '';
    if (!target.startsWith('#') || target.startsWith('#/')) return;
    e.preventDefault();
    let id = target.slice(1);
    try { id = decodeURIComponent(id); } catch { /* use it as written */ }
    const heading = id && previewBody.querySelector(`[id="${CSS.escape(id)}"]`);
    if (heading) previewBody.scrollTop += heading.getBoundingClientRect().top - previewBody.getBoundingClientRect().top;
  });
  // The text box and the preview follow each other while this is ticked.
  const syncedScroll = scrollTogether(ta, previewBody,
    () => scrollBox.checked && panes.dataset.view === 'both');
  scrollBox.addEventListener('change', () => {
    try { localStorage.setItem(SCROLL_KEY, scrollBox.checked ? 'on' : 'off'); } catch { /* this visit only */ }
    syncedScroll.refresh();
  });

  // The editor is a box with its own scrolling, so the page around it stays
  // put and "Publish" is always just below it. Drag the bar under the box
  // (or use the arrow keys on it) to make it taller or shorter; the height
  // is remembered in this browser.
  const resizer = h('div', {
    class: 'editor-resize', role: 'separator', tabindex: '0',
    'aria-orientation': 'horizontal', 'aria-label': 'Editor height: drag, or use the up and down arrow keys',
    'aria-valuemin': String(EDITOR_MIN_HEIGHT), 'aria-valuemax': '4000', title: 'Drag to make the editor taller or shorter',
  });
  const setEditorHeight = (px, remember) => {
    const height = Math.max(EDITOR_MIN_HEIGHT, Math.round(px));
    panes.style.setProperty('--editor-height', `${height}px`);
    resizer.setAttribute('aria-valuenow', String(height));
    if (remember) {
      try { localStorage.setItem(HEIGHT_KEY, String(height)); } catch { /* this visit only */ }
    }
  };
  {
    let saved = 0;
    try { saved = Number(localStorage.getItem(HEIGHT_KEY)) || 0; } catch { saved = 0; }
    if (saved) setEditorHeight(saved, false);
  }
  resizer.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    try { resizer.setPointerCapture(e.pointerId); } catch { /* drag still works without capture */ }
    const startY = e.clientY;
    const startHeight = panes.getBoundingClientRect().height;
    const move = (ev) => setEditorHeight(startHeight + ev.clientY - startY, false);
    const up = () => {
      resizer.removeEventListener('pointermove', move);
      resizer.removeEventListener('pointerup', up);
      setEditorHeight(panes.getBoundingClientRect().height, true);
    };
    resizer.addEventListener('pointermove', move);
    resizer.addEventListener('pointerup', up);
  });
  resizer.addEventListener('keydown', (e) => {
    const step = e.shiftKey ? 160 : 40;
    const now = panes.getBoundingClientRect().height;
    const next = { ArrowDown: now + step, ArrowUp: now - step, Home: EDITOR_MIN_HEIGHT }[e.key];
    if (next === undefined) return;
    e.preventDefault();
    setEditorHeight(next, true);
  });
  // The visual editor is created the first time it's shown. The Markdown in
  // the text box stays the source of truth; visual edits are written back.
  let visual = null;
  const inVisual = () => (panes.dataset.view === 'visual' ? visual : null);
  async function ensureVisual() {
    const body = ta.value;
    if (visual) {
      visual.setMarkdown(body);
      return visual;
    }
    try {
      visual = await createVisualEditor({
        root: visualRoot, markdown: body, pagePath: path,
        readKey: start.read_key, staged: start.staged || [],
        onChange: (md) => {
          ta.value = md;
          changed();
        },
      });
    } catch {
      toast('Visual editing could not start, so the page is shown with its formatting codes. You can keep writing.', { error: true });
      return null;
    }
    return visual;
  }

  const fileInput = h('input', {
    type: 'file', accept: 'image/png,image/jpeg,image/webp,image/gif',
    class: 'visually-hidden', tabindex: '-1', 'aria-hidden': 'true',
  });
  const publishError = h('div', { style: 'flex-basis: 100%' });
  const publishBtn = button(isTemplate ? 'Publish template' : 'Publish changes',
    { icon: 'publish', kind: 'primary', large: true, 'aria-describedby': 'publish-note' });
  const readyNote = isTemplate
    ? 'Everyone can use this version for new pages.'
    : 'You’ll see what changed before anything is published.';
  // What is published now, to tell "nothing changed" from real changes.
  const publishedText = start.published_content ?? '';
  const hasChanges = () => {
    if (visual) visual.flush();
    return s.isNew || fullText() !== publishedText;
  };
  const publishNote = h('span', { class: 'publish-note', id: 'publish-note' }, readyNote);

  // ---------------------------------------------------------- statuses
  function setSave(kind, text) {
    saveStatus.className = `status-item${kind ? ` is-${kind}` : ''}`;
    clear(saveStatus).append(icon({ ok: 'checkCircle', warn: 'alert', error: 'alert' }[kind] || 'clock'), h('span', null, text));
  }
  function setLock() {
    lockStatus.className = `status-item ${s.lockHeld ? 'is-ok' : 'is-warn'}`;
    clear(lockStatus).append(icon(s.lockHeld ? 'lock' : 'unlock'),
      h('span', null, s.lockHeld ? 'Locked for you: others can read but not edit' : 'Not locked: others can edit this page'));
    publishBtn.disabled = !s.lockHeld;
    publishNote.textContent = s.lockHeld
      ? readyNote
      : 'Lock the page again before publishing (see the message above).';
  }
  setLock();
  setSave(null, 'No changes yet');

  // ---------------------------------------------------------- activity
  function activity() {
    const now = Date.now();
    if (!s.lockHeld || now - s.lastActivitySent < ACTIVITY_EVERY_MS) return;
    s.lastActivitySent = now;
    post('/api/edit/activity', { path }).then(handleStatus).catch(() => {});
  }

  // ---------------------------------------------------------- saving
  async function saveDraft() {
    // Visual edits reach the text a moment later; take the latest now.
    if (visual) visual.flush();
    clearTimeout(s.saveTimer);
    if (!s.dirty || s.closed) return true;
    const text = fullText();
    setSave(null, 'Saving…');
    try {
      const res = await post('/api/draft/save', { path, content: text });
      s.dirty = fullText() !== text;
      if (!s.dirty) s.firstDirtyAt = 0;
      const at = formatTime(res.saved_at);
      if (res.persisted) setSave('ok', `Draft saved at ${at}`);
      else if (res.save_error) setSave('error', res.save_error);
      else setSave('warn', `Kept in this window only (${at}). Publish before closing Cairn.`);
      handleStatus(res);
      return true;
    } catch (err) {
      setSave('error', `Your changes could not be saved: ${errorText(err)} Keep this window open and try again.`);
      return false;
    }
  }

  function scheduleSave() {
    const now = Date.now();
    if (!s.firstDirtyAt) s.firstDirtyAt = now;
    const wait = Math.max(0, Math.min(SAVE_DELAY_MS, SAVE_MAX_WAIT_MS - (now - s.firstDirtyAt)));
    clearTimeout(s.saveTimer);
    s.saveTimer = setTimeout(saveDraft, wait);
  }

  // --------------------------------------------------------- preview
  async function refreshPreview() {
    try {
      const content = isTemplate ? withExampleValues(fullText(), author) : fullText();
      const res = await post('/api/preview', { path, content });
      previewBody.innerHTML = res.html; // sanitized by the server
      syncedScroll.refresh();
      const heading = isTemplate
        ? `Editing template: ${readTemplateMeta(fullText()).template_name || 'Untitled template'}`
        : `Editing: ${res.title}`;
      titleEl.textContent = heading;
      document.title = `${heading} – Cairn`;
    } catch { /* keep the last preview */ }
  }
  function schedulePreview() {
    clearTimeout(s.previewTimer);
    s.previewTimer = setTimeout(refreshPreview, PREVIEW_DELAY_MS);
  }

  // Declared below; used by `changed`.
  let syncDetailsFromText = () => {};

  // How long the page is, in words and reading time (200 words a minute).
  // Link addresses and formatting codes aren't counted.
  function countWords() {
    const text = ta.value.replace(/\]\([^)\n]*\)/g, ' ').replace(/[#>*_`~|\[\]()!]/g, ' ');
    const n = (text.match(/[\p{L}\p{N}][\p{L}\p{N}'’.-]*/gu) || []).length;
    const minutes = Math.max(1, Math.round(n / 200));
    const words = `${n === 1 ? '1 word' : `${n} words`}, about ${minutes} minute${minutes === 1 ? '' : 's'} to read`;
    for (const el of wordCounts) el.textContent = words;
  }

  function changed() {
    countWords();
    s.dirty = true;
    setSave(null, 'Not saved yet');
    scheduleSave();
    schedulePreview();
    syncDetailsFromText();
    activity();
  }
  ta.addEventListener('input', changed);

  // ------------------------------------------------------ text editing
  function replaceSelection(text, selectFrom, selectTo) {
    ta.focus();
    const startPos = ta.selectionStart;
    let ok = false;
    try { ok = document.execCommand('insertText', false, text); } catch { ok = false; }
    if (!ok) {
      ta.setRangeText(text, ta.selectionStart, ta.selectionEnd, 'end');
      ta.dispatchEvent(new Event('input'));
    }
    if (selectFrom !== undefined) ta.setSelectionRange(startPos + selectFrom, startPos + selectTo);
  }

  function wrap(before, after, exampleText) {
    const sel = ta.value.slice(ta.selectionStart, ta.selectionEnd) || exampleText;
    replaceSelection(before + sel + after, before.length, before.length + sel.length);
  }

  function selectWholeLines() {
    const v = ta.value;
    const from = v.lastIndexOf('\n', ta.selectionStart - 1) + 1;
    const endAt = ta.selectionEnd > ta.selectionStart ? ta.selectionEnd - 1 : ta.selectionEnd;
    let to = v.indexOf('\n', endAt);
    if (to === -1) to = v.length;
    ta.setSelectionRange(from, to);
    return v.slice(from, to);
  }

  function prefixLines(kind) {
    const lines = selectWholeLines().split('\n');
    let out;
    if (kind === 'p') {
      out = lines.map((l) => l.replace(/^#{1,6}\s*/, ''));
    } else if (/^h[1-6]$/.test(kind)) {
      const mark = `${'#'.repeat(Number(kind[1]))} `;
      out = lines.map((l) => mark + (l.replace(/^#{1,6}\s*/, '') || 'Section title'));
    } else if (kind === 'task') {
      // A checklist, or back to a plain list if it already is one.
      const all = lines.every((l) => /^\s*[-*+]\s\[[ xX]\]\s/.test(l));
      out = lines.map((l) => (all
        ? l.replace(/^(\s*[-*+]\s)\[[ xX]\]\s/, '$1')
        : `- [ ] ${l.replace(/^\s*([-*+]|\d+\.)\s(\[[ xX]\]\s)?/, '') || 'Item'}`));
    } else if (kind === 'quote') {
      const all = lines.every((l) => /^\s*>/.test(l));
      out = lines.map((l) => (all ? l.replace(/^(\s*)>\s?/, '$1') : `> ${l}`));
    } else if (kind.startsWith('callout:')) {
      // "> [!WARNING]" on its own line, then the box's text.
      const body = lines
        .filter((l) => !/^\s*>\s*\[![A-Za-z]+\]\s*$/.test(l))
        .map((l) => (/^\s*>/.test(l) ? l : `> ${l || 'Text'}`));
      out = [`> [!${kind.slice(8).toUpperCase()}]`, ...body];
    } else if (kind === 'ul') {
      const all = lines.every((l) => /^\s*[-*+]\s/.test(l));
      out = lines.map((l) => (all ? l.replace(/^(\s*)[-*+]\s/, '$1') : `- ${l.replace(/^\s*\d+\.\s/, '') || 'Item'}`));
    } else {
      const all = lines.every((l) => /^\s*\d+\.\s/.test(l));
      out = lines.map((l, i) => (all ? l.replace(/^(\s*)\d+\.\s/, '$1') : `${i + 1}. ${l.replace(/^\s*[-*+]\s/, '') || 'Step'}`));
    }
    replaceSelection(out.join('\n'));
  }

  function insertBlock(text) {
    const before = ta.value.slice(0, ta.selectionStart);
    const lead = before === '' || before.endsWith('\n\n') ? '' : before.endsWith('\n') ? '\n' : '\n\n';
    replaceSelection(`${lead}${text}\n`);
  }

  function code() {
    if (inVisual()) {
      visual.run('code');
      return;
    }
    const sel = ta.value.slice(ta.selectionStart, ta.selectionEnd);
    if (sel.includes('\n')) insertBlock(`\`\`\`\n${sel}\n\`\`\``);
    else wrap('`', '`', 'code');
  }

  async function insertLink() {
    const vis = inVisual();
    const selStart = ta.selectionStart;
    const selEnd = ta.selectionEnd;
    const selected = vis ? vis.selectedText() : ta.value.slice(selStart, selEnd);
    let pages = [];
    try { pages = (await get('/api/pages')).pages.filter((p) => p.path !== path); } catch { pages = []; }
    const values = await formDialog({
      title: 'Add a link', iconName: 'link',
      intro: 'A link can go to a heading on this page, to another page in this documentation, or to a website.',
      fields: [
        { name: 'text', label: 'Words to show', value: selected, required: true, help: 'For example: the printer guide' },
        {
          name: 'page', label: 'Link to a page in this documentation', type: 'select', value: '',
          options: [{ value: '', label: 'Not a page: I’ll type a web address below' },
            { value: THIS_PAGE, label: 'This page (a heading further up or down)' },
            ...pages.map((p) => ({ value: p.path, label: `${p.title} (${p.path})` }))],
        },
        {
          name: 'section', label: 'Section of that page', type: 'select', value: '',
          options: [{ value: '', label: 'The top of the page' }],
          help: 'Choose a page first. The link then opens at that heading.',
        },
        { name: 'url', label: 'Or a web address', value: '', help: 'Starts with https://. Leave empty if you chose a page above.' },
      ],
      submitLabel: 'Add link',
      // List the chosen page's headings, so a link can open at one of them.
      onOpen: (inputs) => {
        const sections = inputs.section;
        sections.disabled = true;
        inputs.page.addEventListener('change', async () => {
          const chosen = inputs.page.value;
          const here = chosen === THIS_PAGE;
          sections.replaceChildren(h('option', { value: '' }, here ? 'Choose a heading' : 'The top of the page'));
          sections.disabled = true;
          if (!chosen) return;
          let toc = [];
          try {
            // This page's headings come from the text being edited now.
            toc = here
              ? (await post('/api/preview', { path, content: fullText() })).toc
              : (await get('/api/page', { path: chosen })).toc;
          } catch { toc = []; }
          if (inputs.page.value !== chosen) return; // another page was chosen meanwhile
          const headings = toc.filter((t) => t.level > 1);
          sections.append(...headings
            .map((t) => h('option', { value: t.id }, `${'  '.repeat(t.level - 2)}${t.text}`)));
          sections.disabled = headings.length === 0;
        });
      },
    });
    if (vis) vis.focus();
    else {
      ta.focus();
      ta.setSelectionRange(selStart, selEnd);
    }
    if (!values) return;
    if (values.page === THIS_PAGE && !values.section) {
      toast('To link within this page, choose which heading it goes to. Nothing was added.', { error: true });
      return;
    }
    let target;
    if (values.page === THIS_PAGE) target = `#${values.section}`;
    else if (values.page) target = `${relativeLink(path, values.page)}${values.section ? `#${values.section}` : ''}`;
    else target = values.url.trim();
    if (!target) {
      toast('The link needs a page or a web address, so nothing was added.', { error: true });
      return;
    }
    if (!values.page && !/^(https?:|mailto:|#)/i.test(target)) target = `https://${target}`;
    const link = `[${values.text.trim().replace(/[[\]]/g, '')}](${target})`;
    if (vis) vis.insert(link, true);
    else replaceSelection(link);
  }

  async function insertTable() {
    const selStart = ta.selectionStart;
    const values = await formDialog({
      title: 'Add a table', iconName: 'table',
      fields: [
        { name: 'cols', label: 'How many columns?', type: 'number', value: '3', min: '1', max: '8' },
        { name: 'rows', label: 'How many rows, not counting the heading row?', type: 'number', value: '3', min: '1', max: '30' },
      ],
      submitLabel: 'Add table',
    });
    const vis = inVisual();
    if (vis) vis.focus();
    else {
      ta.focus();
      ta.setSelectionRange(selStart, selStart);
    }
    if (!values) return;
    const cols = Math.min(8, Math.max(1, parseInt(values.cols, 10) || 3));
    const rows = Math.min(30, Math.max(1, parseInt(values.rows, 10) || 3));
    if (vis) {
      vis.run('table', { row: rows + 1, col: cols }); // plus the heading row
      return;
    }
    const cells = (fn) => `| ${Array.from({ length: cols }, (_, i) => fn(i)).join(' | ')} |`;
    insertBlock([cells((i) => `Column ${i + 1}`), cells(() => '---'),
      ...Array.from({ length: rows }, () => cells(() => ' '))].join('\n'));
  }

  // --------------------------------------------------------- pictures
  async function addPicture(file) {
    if (!file) return;
    const pos = ta.selectionStart;
    const guess = file.name && !/^image\.(png|jpe?g|gif|webp)$/i.test(file.name)
      ? file.name.replace(/\.[^.]+$/, '').replace(/[-_]+/g, ' ')
      : 'Screenshot';
    // Show the picture itself, so people can check it's the right one.
    const previewUrl = URL.createObjectURL(file);
    const media = h('figure', { class: 'pic-preview' },
      h('img', { src: previewUrl, alt: '' }),
      h('figcaption', null, `${file.name || 'Pasted picture'}, ${formatSize(file.size)}`));
    const values = await formDialog({
      title: 'Describe this picture', iconName: 'image',
      intro: 'A short description helps people who can’t see the picture, and is shown if the picture can’t be loaded.',
      media,
      fields: [{ name: 'alt', label: 'Description', value: guess }],
      submitLabel: 'Insert picture',
    });
    URL.revokeObjectURL(previewUrl);
    const vis = inVisual();
    if (vis) vis.focus();
    else {
      ta.focus();
      ta.setSelectionRange(pos, pos);
    }
    if (!values) return;
    setSave(null, 'Adding picture…');
    try {
      const q = new URLSearchParams({ path, name: file.name || 'picture' });
      const res = await api('POST', `/api/draft/image?${q}`, file, { raw: true });
      const alt = (values.alt || 'Picture').replace(/[[\]]/g, '');
      if (vis) {
        vis.addStaged(res.link, res.name);
        vis.insert(`![${alt}](${res.link})`);
      } else {
        insertBlock(`![${alt}](${res.link})`);
      }
      if (res.save_error) setSave('error', res.save_error);
      announce('Picture inserted.');
    } catch (err) {
      setSave('error', `The picture was not added: ${errorText(err)}`);
      toast(`The picture was not added. ${errorText(err)}`, { error: true });
    }
  }

  fileInput.addEventListener('change', () => {
    const file = fileInput.files[0];
    fileInput.value = '';
    addPicture(file);
  });
  ta.addEventListener('paste', (e) => {
    const file = [...(e.clipboardData?.files || [])].find((f) => f.type.startsWith('image/'));
    if (file) {
      e.preventDefault();
      addPicture(file);
    }
  });
  writePane.addEventListener('dragover', (e) => {
    if ([...(e.dataTransfer?.items || [])].some((i) => i.kind === 'file')) {
      e.preventDefault();
      writePane.classList.add('is-dragging');
    }
  });
  writePane.addEventListener('dragleave', () => writePane.classList.remove('is-dragging'));
  writePane.addEventListener('drop', (e) => {
    writePane.classList.remove('is-dragging');
    const files = [...(e.dataTransfer?.files || [])];
    if (!files.length) return;
    e.preventDefault();
    const file = files.find((f) => f.type.startsWith('image/') || /\.(png|jpe?g|gif|webp)$/i.test(f.name));
    if (file) addPicture(file);
    else toast('Only pictures can be dropped here (PNG, JPEG, WebP, or GIF).', { error: true });
  });

  // --------------------------------------------------------- toolbar
  // Google Docs-style: compact icons, the name on hover or keyboard focus
  // (or always, with "Show words" in Settings, and on touch screens).
  const run = (fn) => () => { fn(); activity(); };
  const tool = (label, iconName, fn, shortcut) => iconButton(label, { icon: iconName, shortcut, onClick: run(fn) });
  const sep = () => h('span', { class: 'toolbar-sep', 'aria-hidden': 'true' });
  // Each action goes to whichever editor is showing.
  const either = (visualAction, textAction) => () => {
    const vis = inVisual();
    if (vis) visualAction(vis);
    else textAction();
  };
  const insertText = (text) => either((v) => v.insert(text, true), () => replaceSelection(text));

  const viewSwitch = h('div', { class: 'view-switch', role: 'group', 'aria-label': 'How to see the page while editing' });
  async function setView(view, { remember = false } = {}) {
    panes.dataset.view = view;
    setTimeout(() => updateTableBar(), 0); // defined below; the bar only shows in the visual view
    if (view === 'both') setTimeout(() => syncedScroll.refresh(), 0);
    for (const b of viewSwitch.children) b.setAttribute('aria-pressed', String(b.dataset.view === view));
    if (remember) {
      try { localStorage.setItem(VIEW_KEY, view); } catch { /* remembered for this visit only */ }
    }
    if (view === 'visual') {
      const vis = await ensureVisual();
      if (vis) vis.focus();
      else setView('both');
    } else {
      ta.focus();
    }
  }
  const viewBtn = (label, view, phoneLabel) => {
    const b = button(label, {
      class: 'btn btn-view', dataset: { view },
      'aria-pressed': view === startView ? 'true' : 'false',
      onClick: () => setView(view, { remember: true }),
    });
    // A shorter name on a phone, so all three fit on one line.
    if (phoneLabel) {
      b.lastChild.classList.add('view-long');
      b.append(h('span', { class: 'view-short' }, phoneLabel));
    }
    return b;
  };
  viewSwitch.append(
    viewBtn('As it will look', 'visual'),
    viewBtn('Show formatting codes', 'both', 'Page + codes'),
    viewBtn('Codes only', 'write'));

  const styleSelect = h('select', { id: 'tb-style', class: 'toolbar-select', 'aria-label': 'Text style' },
    h('option', { value: 'p' }, 'Normal text'),
    h('option', { value: 'h2' }, 'Heading'),
    h('option', { value: 'h3' }, 'Subheading'),
    h('option', { value: 'h4' }, 'Small heading'),
    h('option', { value: 'h5' }, 'Smaller heading'),
    h('option', { value: 'h6' }, 'Smallest heading'),
    h('option', { value: 'h1' }, 'Page title'));
  styleSelect.addEventListener('change', run(either(
    (v) => (styleSelect.value === 'p' ? v.run('paragraph') : v.run('heading', Number(styleSelect.value.slice(1)))),
    () => prefixLines(styleSelect.value))));
  // Show the style of the line the cursor is on.
  const showStyle = (hashes) => { styleSelect.value = hashes ? `h${hashes}` : 'p'; };
  const syncStyle = () => {
    const v = ta.value;
    const from = v.lastIndexOf('\n', ta.selectionStart - 1) + 1;
    showStyle(v.slice(from).match(/^(#{1,6})\s/)?.[1].length || 0);
    tableCodeHint.hidden = !/^\s*\|/.test(v.slice(from));
  };
  for (const ev of ['keyup', 'click', 'focus']) ta.addEventListener(ev, syncStyle);
  // The editor applies a cursor move just after the event, so read it a tick later.
  for (const ev of ['keyup', 'mouseup', 'focusin']) {
    visualRoot.addEventListener(ev, () => setTimeout(() => { if (visual) showStyle(visual.headingLevel()); }, 0));
  }

  // Replace: find words in the page and change them all at once. (Finding
  // alone is the browser's own Ctrl+F, which works in both views.) Link
  // addresses and picture paths are left alone, so links never break.
  const PROTECTED = /(\]\([^)\n]*\)|<https?:[^>\s]*>)/;
  function replaceIn(text, find, replacement, matchCase, wholeWords) {
    const escaped = find.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    // Whole words: "IT" doesn't change "edit" or "with".
    const word = wholeWords ? '(?<![\\p{L}\\p{N}_])' : '';
    const wordEnd = wholeWords ? '(?![\\p{L}\\p{N}_])' : '';
    const pattern = new RegExp(`${word}${escaped}${wordEnd}`, matchCase ? 'gu' : 'giu');
    let count = 0;
    const out = text.split(PROTECTED).map((part, i) => (i % 2 ? part : part.replace(pattern, () => {
      count += 1;
      return replacement;
    }))).join('');
    return { out, count };
  }
  async function openReplace() {
    const selected = inVisual()?.selectedText() || ta.value.slice(ta.selectionStart, ta.selectionEnd);
    const findInput = h('input', { type: 'text', id: 'rp-find', value: selected.includes('\n') ? '' : selected, autocomplete: 'off' });
    const withInput = h('input', { type: 'text', id: 'rp-with', autocomplete: 'off' });
    const caseBox = h('input', { type: 'checkbox', id: 'rp-case' });
    const wordBox = h('input', { type: 'checkbox', id: 'rp-words', checked: true });
    const count = h('p', { class: 'help', role: 'status' });
    const replaceWith = (replacement) => replaceIn(ta.value, findInput.value, replacement, caseBox.checked, wordBox.checked);
    const recount = () => {
      const n = findInput.value ? replaceWith('').count : 0;
      count.textContent = !findInput.value ? 'Type the words to find.'
        : n === 0 ? 'Not found in this page.' : n === 1 ? 'Found once.' : `Found ${n} times.`;
    };
    for (const el of [findInput, caseBox, wordBox]) el.addEventListener('input', recount);
    recount();
    const value = await openDialog({
      title: 'Replace words in this page', iconName: 'search', tone: 'info',
      body: [
        h('div', { class: 'field' }, h('label', { for: 'rp-find' }, 'Find'), findInput),
        h('div', { class: 'field' }, h('label', { for: 'rp-with' }, 'Replace with'), withInput),
        h('label', { class: 'check', for: 'rp-words' }, wordBox, h('span', null, 'Whole words only (so “IT” doesn’t change “edit”)')),
        h('label', { class: 'check', for: 'rp-case' }, caseBox, h('span', null, 'Match capital letters exactly')),
        count,
        h('p', { class: 'help' }, 'Link addresses and picture file names are not changed. Undo takes it all back.'),
      ],
      actions: [
        { label: 'Close', value: 'cancel' },
        { label: 'Replace all', value: 'replace', kind: 'primary', submit: true },
      ],
      onSubmit: () => findInput.value !== '' && replaceWith('').count > 0,
      onOpen: () => findInput.focus(),
    });
    if (value !== 'replace') return;
    const { out, count: n } = replaceWith(withInput.value);
    const vis = inVisual();
    if (vis) {
      vis.replaceText(out);
    } else {
      ta.focus();
      ta.select();
      replaceSelection(out);
    }
    activity();
    toast(n === 1 ? 'Replaced 1 time.' : `Replaced ${n} times.`);
  }

  for (const el of [ta, visualRoot]) {
    el.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'h') {
        e.preventDefault();
        openReplace();
      }
    });
  }

  // Note box: a plain tinted box, or a callout that says what kind it is
  // (written "> [!WARNING]" and so on, as on GitHub).
  const noteKinds = [
    ['Plain box', null], ['Note', 'note'], ['Tip', 'tip'],
    ['Important', 'important'], ['Warning', 'warning'], ['Caution', 'caution'],
  ];
  const noteMenu = menuButton('Note box', {
    icon: 'quote', compact: true,
    items: noteKinds.map(([label, kind]) => ({
      label,
      onSelect: run(kind
        ? either((v) => v.callout(kind), () => prefixLines(`callout:${kind}`))
        : either((v) => v.run('quote'), () => prefixLines('quote'))),
    })),
  });

  // Undo and redo in the Markdown box use its own history (every toolbar
  // change goes in through insertText, so it is recorded there too).
  const undoRedo = (name) => either((v) => v.run(name), () => { ta.focus(); document.execCommand(name); });

  const toolbar = toolbarKeys(h('div', { class: 'toolbar', role: 'toolbar', 'aria-label': 'Formatting', 'aria-controls': 'md-text' },
    tool('Undo', 'undo', undoRedo('undo'), 'Ctrl+Z'),
    tool('Redo', 'redo', undoRedo('redo'), 'Ctrl+Y'),
    sep(),
    styleSelect,
    sep(),
    tool('Bold', 'bold', either((v) => v.run('bold'), () => wrap('**', '**', 'bold words')), 'Ctrl+B'),
    tool('Italic', 'italic', either((v) => v.run('italic'), () => wrap('_', '_', 'slanted words')), 'Ctrl+I'),
    tool('Strikethrough', 'strike', either((v) => v.run('strike'), () => wrap('~~', '~~', 'crossed-out words'))),
    sep(),
    tool('Bullet list', 'list', either((v) => v.run('bullet'), () => prefixLines('ul'))),
    tool('Numbered list', 'listNumbered', either((v) => v.run('ordered'), () => prefixLines('ol'))),
    tool('Checklist', 'checklist', either((v) => v.checklist(), () => prefixLines('task'))),
    sep(),
    noteMenu,
    tool('Divider line', 'divider', either((v) => v.run('divider'), () => insertBlock('---'))),
    tool('Code', 'code', code),
    sep(),
    tool('Link…', 'link', insertLink),
    tool('Insert picture…', 'image', () => fileInput.click()),
    tool('Table…', 'table', insertTable),
    // Find tools sit at the far end, where people look for them.
    h('span', { class: 'toolbar-end' }, tool('Replace…', 'replace', openReplace, 'Ctrl+H'))));

  // Slash menu (visual view): type "/" on an empty line to pick what to add
  // there, then keep typing to narrow the list. Arrow keys and Enter pick,
  // Esc closes. Each choice runs the same action as its toolbar button.
  const slashChoices = [
    ['Heading', (v) => v.run('heading', 2)],
    ['Subheading', (v) => v.run('heading', 3)],
    ['Bullet list', (v) => v.run('bullet')],
    ['Numbered list', (v) => v.run('ordered')],
    ['Checklist', (v) => v.checklist()],
    ...noteKinds.filter(([, kind]) => kind).map(([label, kind]) => [`${label} box`, (v) => v.callout(kind)]),
    ['Table…', () => insertTable()],
    ['Picture…', () => fileInput.click()],
    ['Divider line', (v) => v.run('divider')],
    ['Code', () => code()],
  ];
  const slashList = h('ul', { class: 'slash-menu', role: 'listbox', id: 'slash-menu', 'aria-label': 'Add to this line' });
  slashList.hidden = true;
  let slashShown = [];
  let slashActive = 0;
  const closeSlash = () => {
    slashList.hidden = true;
    visualRoot.removeAttribute('aria-activedescendant');
    visualRoot.removeAttribute('aria-controls');
  };
  const markSlash = () => {
    slashShown.forEach(([, , li], i) => li.setAttribute('aria-selected', String(i === slashActive)));
    const li = slashShown[slashActive]?.[2];
    if (li) {
      visualRoot.setAttribute('aria-activedescendant', li.id);
      li.scrollIntoView({ block: 'nearest' });
    }
  };
  const pickSlash = (i) => {
    const vis = inVisual();
    const choice = slashShown[i];
    closeSlash();
    if (!vis || !choice) return;
    vis.clearSlash();
    choice[1](vis);
    activity();
  };
  const slashItems = slashChoices.map(([label, action], i) => {
    const li = h('li', { role: 'option', id: `slash-${i}`, 'aria-selected': 'false' }, label);
    // mousedown, so the editor keeps the cursor where the "/" was typed.
    li.addEventListener('mousedown', (e) => {
      e.preventDefault();
      pickSlash(slashShown.findIndex(([l]) => l === label));
    });
    return [label, action, li];
  });
  const updateSlash = () => {
    const vis = inVisual();
    const query = vis?.slashQuery();
    if (query == null) { closeSlash(); return; }
    const wanted = query.toLowerCase();
    slashShown = slashItems.filter(([label]) => label.toLowerCase().includes(wanted));
    if (!slashShown.length) { closeSlash(); return; }
    slashList.replaceChildren(...slashShown.map(([, , li]) => li));
    slashActive = Math.min(slashActive, slashShown.length - 1);
    const at = vis.caretRect();
    slashList.style.left = `${Math.max(8, Math.min(at.left, window.innerWidth - 272))}px`;
    slashList.style.top = `${at.bottom + 6}px`;
    slashList.hidden = false;
    visualRoot.setAttribute('aria-controls', 'slash-menu');
    markSlash();
  };
  document.body.append(slashList);
  visualRoot.addEventListener('keydown', (e) => {
    if (slashList.hidden) return;
    const moves = { ArrowDown: 1, ArrowUp: -1 };
    if (e.key in moves) {
      slashActive = (slashActive + moves[e.key] + slashShown.length) % slashShown.length;
      markSlash();
    } else if (e.key === 'Enter' || e.key === 'Tab') {
      pickSlash(slashActive);
    } else if (e.key === 'Escape') {
      closeSlash();
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
  }, true);
  visualRoot.addEventListener('input', () => {
    if (slashList.hidden) slashActive = 0;
    setTimeout(updateSlash, 0);
  });
  visualRoot.addEventListener('focusout', closeSlash);
  visualRoot.addEventListener('mousedown', closeSlash);

  // Table bar: one slim row that floats just above the table the cursor is
  // in (visual view), so the page never jumps. Four worded menus: tables are
  // edited now and then, and nobody should have to guess an icon.
  const tableDo = (action, arg) => () => { inVisual()?.tableAction(action, arg); activity(); updateTableBar(); };
  const confirmDeleteTable = async () => {
    const ok = await confirmDialog({
      title: 'Delete this table?',
      message: 'The whole table and everything in it is removed from the page. You can bring it back with Undo.',
      confirmLabel: 'Delete table', cancelLabel: 'Keep it', danger: true, iconName: 'trash',
    });
    if (ok) tableDo('deleteTable')();
  };
  const alignItems = [['left', 'Align column left'], ['center', 'Align column centre'], ['right', 'Align column right']];
  const tableMenus = {
    insert: menuButton('Insert', {
      icon: 'plus',
      items: [
        { label: 'Row above', icon: 'rowAbove', onSelect: tableDo('rowAbove') },
        { label: 'Row below', icon: 'rowBelow', onSelect: tableDo('rowBelow') },
        { label: 'Column left', icon: 'colLeft', onSelect: tableDo('colLeft') },
        { label: 'Column right', icon: 'colRight', onSelect: tableDo('colRight') },
      ],
    }),
    align: menuButton('Align', {
      icon: 'alignLeft',
      items: alignItems.map(([value, label]) => ({
        label, icon: `align${value[0].toUpperCase()}${value.slice(1)}`, checked: false, onSelect: tableDo('align', value),
      })),
    }),
    move: menuButton('Move', {
      icon: 'move',
      items: [
        { label: 'Move row up', icon: 'rowAbove', onSelect: tableDo('rowUp') },
        { label: 'Move row down', icon: 'rowBelow', onSelect: tableDo('rowDown') },
        { label: 'Move column left', icon: 'colLeft', onSelect: tableDo('colLeftMove') },
        { label: 'Move column right', icon: 'colRight', onSelect: tableDo('colRightMove') },
      ],
    }),
    remove: menuButton('Delete', {
      icon: 'trash', danger: true,
      items: [
        { label: 'Delete row', icon: 'trash', danger: true, onSelect: tableDo('deleteRow') },
        { label: 'Delete column', icon: 'trash', danger: true, onSelect: tableDo('deleteCol') },
        null,
        { label: 'Delete whole table…', icon: 'trash', danger: true, onSelect: confirmDeleteTable },
      ],
    }),
  };
  // How tables work, said once in the menu people open first.
  tableMenus.insert.querySelector('.menu').append(h('p', { class: 'menu-note' },
    'Tab in the last cell also adds a row. Cells can’t be merged: Markdown tables don’t support it.'));
  const tableBar = toolbarKeys(h('div', {
    class: 'table-bar', role: 'toolbar', 'aria-label': 'Table', hidden: true,
  }, tableMenus.insert, tableMenus.align, tableMenus.move, tableMenus.remove));
  visualPane.insertBefore(tableBar, visualRoot);
  // Esc in the bar goes back to the table.
  tableBar.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') inVisual()?.focus();
  });

  function placeTableBar() {
    const tableEl = tableBar.hidden ? null : inVisual()?.tableDom();
    if (!tableEl) return;
    const pane = visualPane.getBoundingClientRect();
    const view = visualRoot.getBoundingClientRect();
    const table = tableEl.getBoundingClientRect();
    const barH = tableBar.offsetHeight;
    // Just above the table; kept inside the visible part of the page while
    // the table is scrolled partly out of view.
    const top = Math.min(Math.max(table.top - barH - 6, view.top + 4), Math.max(view.top + 4, table.bottom - barH));
    const left = Math.max(view.left + 4, Math.min(table.left, view.right - tableBar.offsetWidth - 4));
    tableBar.style.top = `${Math.round(top - pane.top)}px`;
    tableBar.style.left = `${Math.round(left - pane.left)}px`;
  }
  function updateTableBar() {
    const t = panes.dataset.view === 'visual' ? inVisual()?.tableState() : null;
    // Leave the bar up while someone is using it.
    if (!t && tableBar.contains(document.activeElement)) return;
    tableBar.hidden = !t;
    if (!t) {
      for (const menu of Object.values(tableMenus)) menu.close();
      return;
    }
    const body = t.row > 0;
    const { insert, move, remove, align } = tableMenus;
    insert.itemFor('Row above').disabled = !body;
    remove.itemFor('Delete row').disabled = !body || t.rows <= 2;
    remove.itemFor('Delete column').disabled = t.cols <= 1;
    move.itemFor('Move row up').disabled = t.row <= 1;
    move.itemFor('Move row down').disabled = !body || t.row >= t.rows - 1;
    move.itemFor('Move column left').disabled = t.col <= 0;
    move.itemFor('Move column right').disabled = t.col >= t.cols - 1;
    for (const [value, label] of alignItems) {
      align.itemFor(label).setAttribute('aria-checked', String(value === (t.align || 'left')));
    }
    placeTableBar();
  }
  for (const ev of ['keyup', 'mouseup', 'focusin', 'input']) {
    visualRoot.addEventListener(ev, () => setTimeout(updateTableBar, 0));
  }
  visualRoot.addEventListener('scroll', placeTableBar, { passive: true });
  window.addEventListener('resize', placeTableBar);

  // Templates: worded buttons insert fill-in fields so nobody types {{…}}.
  // They stay worded: there is no icon anyone would recognize for them.
  const word = (label, iconName, fn) => button(label, { icon: iconName, onClick: run(fn) });
  const fillInBar = isTemplate
    ? toolbarKeys(h('div', { class: 'toolbar toolbar-insert', role: 'toolbar', 'aria-label': 'Insert a fill-in field', 'aria-controls': 'md-text' },
      h('span', { class: 'toolbar-label', 'aria-hidden': 'true' }, 'Insert:'),
      word('Page title', 'page', insertText('{{title}}')),
      word('Today’s date', 'clock', insertText('{{date}}')),
      word('Author’s name', 'user', insertText('{{author}}')),
      word('Folder name', 'folder', insertText('{{folder}}'))))
    : null;

  // Pictures pasted or dropped into the visual editor go through the same
  // upload as the Markdown box (before the editor would handle them itself).
  const pictureFrom = (files) => [...(files || [])]
    .find((f) => f.type.startsWith('image/') || /\.(png|jpe?g|gif|webp)$/i.test(f.name));
  visualRoot.addEventListener('paste', (e) => {
    const file = pictureFrom(e.clipboardData?.files);
    if (!file) return;
    e.preventDefault();
    e.stopPropagation();
    addPicture(file);
  }, true);
  visualRoot.addEventListener('drop', (e) => {
    if (!e.dataTransfer?.files?.length) return;
    e.preventDefault();
    e.stopPropagation();
    const file = pictureFrom(e.dataTransfer.files);
    if (file) addPicture(file);
    else toast('Only pictures can be dropped here (PNG, JPEG, WebP, or GIF).', { error: true });
  }, true);
  visualRoot.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
      e.preventDefault();
      s.dirty = true;
      saveDraft();
    }
  });

  ta.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const k = e.key.toLowerCase();
    if (k === 'b') { e.preventDefault(); wrap('**', '**', 'bold words'); }
    if (k === 'i') { e.preventDefault(); wrap('_', '_', 'slanted words'); }
    if (k === 's') { e.preventDefault(); s.dirty = true; saveDraft(); }
  });

  // -------------------------------------------------------- page details
  const detailInputs = {
    owner: h('input', { type: 'text', id: 'pd-owner', autocomplete: 'off' }),
    status: h('select', { id: 'pd-status' },
      h('option', { value: '' }, 'Not set'),
      h('option', { value: 'draft' }, 'Draft: still being written'),
      h('option', { value: 'active' }, 'Active: up to date'),
      h('option', { value: 'retired' }, 'Retired: no longer used')),
    last_reviewed: h('input', { type: 'date', id: 'pd-reviewed' }),
    tags: h('input', { type: 'text', id: 'pd-tags', autocomplete: 'off', 'aria-describedby': 'pd-tags-help' }),
  };
  syncDetailsFromText = () => {
    const { fields } = readMeta(fullText());
    for (const key of META_KEYS) {
      if (document.activeElement !== detailInputs[key]) detailInputs[key].value = fields[key] || '';
    }
  };
  function applyDetails() {
    const fields = Object.fromEntries(META_KEYS.map((k) => [k, detailInputs[k].value]));
    const next = writeMeta(fullText(), fields);
    if (next !== fullText()) {
      setFullText(next);
      changed();
    }
  }
  for (const key of META_KEYS) detailInputs[key].addEventListener('change', applyDetails);
  syncDetailsFromText();

  const detailsPanel = h('details', { class: 'details-panel' },
    h('summary', null, 'Page details: owner, status, review date, tags'),
    h('div', { class: 'details-body' },
      h('p', { class: 'help', style: 'margin: 0 0 var(--space-4)' }, 'These appear beside the page. Changes here are written into the page for you.'),
      h('div', { class: 'meta-grid' },
        h('div', { class: 'field' }, h('label', { for: 'pd-owner' }, 'Owner'), detailInputs.owner),
        h('div', { class: 'field' }, h('label', { for: 'pd-status' }, 'Status'), detailInputs.status),
        h('div', { class: 'field' }, h('label', { for: 'pd-reviewed' }, 'Last reviewed'),
          h('div', { class: 'date-row' }, detailInputs.last_reviewed,
            button('Today', { onClick: () => { detailInputs.last_reviewed.value = todayYmd(); applyDetails(); } }))),
        h('div', { class: 'field' }, h('label', { for: 'pd-tags' }, 'Tags'), detailInputs.tags,
          h('p', { class: 'help', id: 'pd-tags-help' }, 'Separate with commas.')))));

  // ------------------------------------------------------ template details
  let templatePanel = null;
  if (isTemplate) {
    const nameInput = h('input', { type: 'text', id: 'td-name', autocomplete: 'off', maxlength: '80', required: true });
    const descInput = h('textarea', { id: 'td-desc', rows: '2', maxlength: '300', 'aria-describedby': 'td-desc-help' });
    const nameError = h('div', { class: 'error-slot' });
    const syncTemplateFromText = () => {
      const fields = readTemplateMeta(fullText());
      if (document.activeElement !== nameInput) nameInput.value = fields.template_name;
      if (document.activeElement !== descInput) descInput.value = fields.template_description;
    };
    const applyTemplate = () => {
      clear(nameError);
      nameInput.removeAttribute('aria-invalid');
      if (!nameInput.value.trim()) {
        nameError.append(h('p', { class: 'field-error', role: 'alert' }, icon('alert'),
          h('span', null, 'A template needs a name, so people can find it.')));
        nameInput.setAttribute('aria-invalid', 'true');
        return;
      }
      const next = writeTemplateMeta(fullText(),
        { template_name: nameInput.value, template_description: descInput.value });
      if (next !== fullText()) {
        setFullText(next);
        changed();
      }
    };
    nameInput.addEventListener('change', applyTemplate);
    descInput.addEventListener('change', applyTemplate);
    const previousSync = syncDetailsFromText;
    syncDetailsFromText = () => { previousSync(); syncTemplateFromText(); };
    syncTemplateFromText();
    templatePanel = h('details', { class: 'details-panel', open: true },
      h('summary', null, 'Template details: name and description'),
      h('div', { class: 'details-body' },
        h('div', { class: 'meta-grid' },
          h('div', { class: 'field' }, h('label', { for: 'td-name' }, 'Template name (required)'), nameInput, nameError),
          h('div', { class: 'field' }, h('label', { for: 'td-desc' }, 'Description'), descInput,
            h('p', { class: 'help', id: 'td-desc-help' }, 'One sentence about when to use it. Shown when someone picks a template.')))));
  }

  const helpPanel = h('details', { class: 'details-panel' },
    h('summary', null, 'Formatting help'),
    h('div', { class: 'details-body' },
      h('p', { class: 'help', style: 'margin: 0 0 var(--space-3)' }, 'The buttons above do all of this for you; you never need to type these codes. They are for anyone who prefers typing, with “Show formatting codes” turned on. To tick a checklist item, click its box.'),
      helpTable(),
      h('p', { class: 'help' }, 'Keyboard shortcuts (optional): Ctrl+Z undo, Ctrl+Y redo, Ctrl+B bold, Ctrl+I italic, Ctrl+S save now.')));

  // ------------------------------------------------------------ lock
  function showReleased(reason, lock) {
    s.lockHeld = false;
    setLock();
    if (s.idleOpen) s.idleOpen.close('closed');
    const title = {
      idle: 'This page was unlocked because you were away',
      manual: 'You unlocked this page',
      lost: 'Your editing lock was removed',
    }[reason] || 'This page is no longer locked for you';
    const someoneElse = lock && !lock.is_mine;
    clear(notices).append(banner({
      tone: 'warn', title,
      text: someoneElse
        ? `${lockSentence(lock)}. Your text is kept here and on this computer. You can publish it after they finish.`
        : 'Your text is kept here and on this computer. To publish it, lock the page again.',
      actions: [button('Continue editing', { icon: 'lock', kind: 'primary', onClick: (e) => relock(e.currentTarget) })],
    }));
    announce(title);
  }

  async function relock(btn) {
    try {
      const res = await whileBusy(btn, 'Locking…', () => post('/api/edit/start', { path, is_new: s.isNew }));
      if (res.status === 'locked') {
        showReleased('taken', res.lock);
        toast(`${lockSentence(res.lock)}. Try again when they finish.`, { error: true });
        return;
      }
      s.lockHeld = true;
      s.lastActivitySent = Date.now();
      setLock();
      clear(notices);
      if (res.published_changed_since_start) {
        notices.append(banner({
          tone: 'info', title: 'The page changed while you were away',
          text: 'Someone published a new version. When you publish, you will be shown both versions first.',
        }));
      }
      toast('The page is locked for you again.');
      ta.focus();
    } catch (err) {
      toast(errorText(err), { error: true });
    }
  }

  async function showIdle(secondsLeft) {
    if (s.idleOpen) return;
    const minutes = Math.max(1, Math.round(secondsLeft / 60));
    const choice = await openDialog({
      title: 'Are you still editing?', iconName: 'clock', tone: 'warn',
      body: [
        h('p', null, `You haven’t typed anything for a while. So that others can edit this page, it will be unlocked in about ${minutes} minute${minutes === 1 ? '' : 's'}.`),
        h('p', null, 'Your text will be kept either way.'),
      ],
      actions: [
        { label: 'Unlock the page now', value: 'release' },
        { label: 'Keep editing', value: 'keep', kind: 'primary', autofocus: true },
      ],
      onOpen: (dlg) => { s.idleOpen = dlg; },
    });
    s.idleOpen = null;
    if (choice === 'keep') {
      s.lastActivitySent = 0;
      activity();
      ta.focus();
    } else if (choice === 'release') {
      await saveDraft();
      try {
        await post('/api/edit/release', { path, keep_session: true });
        showReleased('manual', null);
      } catch (err) { toast(errorText(err), { error: true }); }
    }
  }

  function handleStatus(st) {
    if (!st || s.closed) return;
    if (s.lockHeld && !st.lock_held) showReleased(st.released_reason, st.lock);
    else if (st.lock_held && st.idle?.state === 'warning') showIdle(st.idle.seconds_left);
  }

  const statusTimer = setInterval(() => {
    if (s.closed) return;
    get('/api/edit/status', { path }).then(handleStatus).catch(() => {});
  }, STATUS_EVERY_MS);

  // --------------------------------------------------------- publish
  function finish() {
    clearInterval(statusTimer);
    clearTimeout(s.saveTimer);
    clearTimeout(s.previewTimer);
    if (visual) visual.destroy();
    slashList.remove();
    window.removeEventListener('resize', placeTableBar);
    syncedScroll.destroy();
    window.removeEventListener('beforeunload', onBeforeUnload);
  }

  async function doPublish(acceptHash, acceptMissing) {
    clear(publishError);
    let res;
    try {
      res = await whileBusy(publishBtn, 'Publishing…', () => post('/api/publish', {
        path, content: fullText(), accept_current_hash: acceptHash || null, accept_missing: Boolean(acceptMissing),
      }));
    } catch (err) {
      publishError.append(banner({ tone: 'danger', title: 'The page was not published', text: errorText(err) }));
      if (err.code === 'locked') showReleased('lost', null);
      if (!s.lockHeld) publishBtn.disabled = true;
      publishBtn.focus();
      return;
    }
    if (res.result === 'published') {
      s.closed = true;
      finish();
      if (isTemplate) {
        toast('Template published. Everyone can use it for new pages now.');
        ctx.navigate(href.templates());
        return;
      }
      toast(res.new_pictures
        ? 'Published, with your pictures. Everyone can see this version now.'
        : 'Published. Everyone can see this version now.');
      ctx.navigate(href.page(res.path));
      return;
    }
    await conflict(res);
  }

  async function conflict(res) {
    const choice = await openDialog({
      title: 'Someone else changed this page while you were writing', iconName: 'alert', tone: 'warn', wide: true,
      body: [
        h('p', null, h('strong', null, 'Your version has not been published, and your text is safe.')),
        res.deleted
          ? h('p', null, 'The page has been deleted or moved by someone else since you started.')
          : h('p', null, 'Below, their version (published now) is on the left and yours is on the right. You can copy anything you need from theirs into yours before publishing.'),
        diffView(res.diff, { oldLabel: 'Published now (their version)', newLabel: 'Your version' }),
      ],
      actions: [
        { label: 'Publish my version anyway', value: 'mine', kind: 'danger' },
        { label: 'Keep editing', value: 'keep', kind: 'primary', autofocus: true },
      ],
    });
    if (choice !== 'mine') {
      ta.focus();
      return;
    }
    const sure = await confirmDialog({
      title: 'Replace their changes with yours?',
      message: 'Their version will be kept in “Earlier versions”, so it can be brought back if needed.',
      confirmLabel: 'Replace with my version', cancelLabel: 'Keep editing', danger: true, iconName: 'publish',
    });
    if (sure) await doPublish(res.current_hash, res.deleted);
  }

  // Publishing is the moment that matters most, so it gets a short check:
  // what changed, who will see it, and that the old version is kept.
  async function confirmPublish() {
    if (!hasChanges()) {
      const choice = await openDialog({
        title: 'Nothing to publish yet', iconName: 'info', tone: 'info',
        body: [h('p', null, 'You haven’t changed anything on this page. Make your changes first, or close the editor.')],
        actions: [
          { label: 'Close editor', value: 'close' },
          { label: 'Keep editing', value: 'keep', kind: 'primary', autofocus: true },
        ],
      });
      if (choice === 'close') closeBtn.click();
      return;
    }
    const summary = s.isNew ? null : diffSummary(lineDiff(publishedText, fullText()));
    const noun = isTemplate ? 'template' : 'page';
    const choice = await openDialog({
      title: s.isNew ? `Publish this new ${noun}?` : 'Publish your changes?',
      iconName: 'publish', tone: 'info',
      body: [
        summary ? h('p', null, h('strong', null, summary)) : null,
        h('p', null, isTemplate
          ? 'Everyone can use it for new pages straight away.'
          : 'Everyone who uses this documentation will see it straight away.'),
        s.isNew ? null : h('p', null, `The ${noun} as it was before is kept in Earlier versions, so it can be brought back.`),
      ],
      actions: [
        { label: 'Keep editing', value: 'keep' },
        { label: isTemplate ? 'Publish template' : 'Publish', value: 'publish', kind: 'primary', autofocus: true },
      ],
    });
    if (choice === 'publish') await doPublish(null, false);
    else ta.focus();
  }

  publishBtn.addEventListener('click', confirmPublish);

  // ------------------------------------------------------ leave / discard
  async function closeEditor() {
    await saveDraft();
    try { await post('/api/edit/release', { path }); } catch { /* the lock times out anyway */ }
    s.closed = true;
    finish();
  }

  const discardBtn = button('Discard my changes', {
    icon: 'trash', kind: 'danger',
    onClick: async () => {
      const ok = await confirmDialog({
        title: 'Discard your changes?',
        message: 'Everything you changed since you started editing will be removed. The published page stays as it is. This can’t be undone.',
        confirmLabel: 'Discard my changes', cancelLabel: 'Keep editing', danger: true, iconName: 'trash',
      });
      if (!ok) return;
      try {
        await post('/api/draft/discard', { path });
        s.closed = true;
        finish();
        toast('Your changes were discarded.');
        ctx.navigate(leaveTo());
      } catch (err) { toast(errorText(err), { error: true }); }
    },
  });

  const closeBtn = button('Close editor', {
    icon: 'exit',
    onClick: async (e) => {
      const edited = hasChanges();
      await whileBusy(e.currentTarget, 'Closing…', closeEditor);
      if (s.isNew && !isTemplate) {
        toast('Your new page isn’t published yet. Only you can see it, on this computer: find it under “Your unsaved changes” on the Home screen.');
      } else if (edited) {
        toast('Not published yet: only you can see these changes, on this computer. Carry on any time from “Your unsaved changes” on the Home screen.');
      } else {
        toast('Closed. Nothing on the page was changed.');
      }
      ctx.navigate(leaveTo());
    },
  });

  function onBeforeUnload(e) {
    if (s.dirty && !s.closed) {
      e.preventDefault();
      e.returnValue = '';
    }
  }
  window.addEventListener('beforeunload', onBeforeUnload);
  window.addEventListener('pagehide', () => {
    if (visual && !s.closed) visual.flush();
    if (s.dirty && !s.closed) {
      api('POST', '/api/draft/save', { path, content: fullText() }, { keepalive: true }).catch(() => {});
    }
  }, { once: true });

  // ------------------------------------------------------------ render
  if (isTemplate) {
    const name = readTemplateMeta(fullText()).template_name || 'this template';
    notices.append(banner({
      tone: 'info', title: `You’re editing the template “${name}” for the whole team`,
      text: 'Changes apply to pages created from now on; pages already made from it don’t change. Use the “Insert” buttons to add fields that are filled in for each new page; the preview shows example values for them.',
    }));
  }
  if (!s.persistent) {
    notices.append(banner({
      tone: 'warn', title: 'Unsaved changes are kept only while Cairn is open',
      text: 'Publish before you close Cairn. To keep unsaved changes on this computer, turn on “Keep my unsaved changes on this computer” in Settings.',
    }));
  }
  if (start.published_changed_since_start) {
    notices.append(banner({
      tone: 'info', title: 'The page changed since you started these edits',
      text: 'Someone published a new version. When you publish, you will be shown both versions first.',
    }));
  }

  ctx.main.append(
    h('div', { class: 'editor-head' },
      h('div', { class: 'editor-title' },
        titleEl,
        h('div', { class: 'editor-status' }, lockStatus, saveStatus)),
      viewSwitch),
    notices,
    templatePanel,
    toolbar,
    fillInBar,
    panes,
    resizer,
    fileInput,
    h('div', { class: 'editor-foot' },
      h('div', { class: 'actions' }, closeBtn, discardBtn),
      h('div', { class: 'publish-group' }, publishNote, publishBtn),
      publishError),
    detailsPanel,
    helpPanel);

  refreshPreview();
  requestAnimationFrame(() => resizer.setAttribute('aria-valuenow', String(Math.round(panes.getBoundingClientRect().height))));
  if (startView === 'visual') setView('visual');
  else {
    setTimeout(() => {
      // Start at the top of the page, not scrolled to its end.
      ta.setSelectionRange(0, 0);
      ta.focus({ preventScroll: true });
      ta.scrollTop = 0;
    }, 0);
  }

  return {
    title: 'Editing',
    keepFocus: true,
    cleanup: finish,
    // Opened too late (the person already went elsewhere): give the lock back.
    abandon: closeEditor,
    canLeave: async () => {
      if (s.closed) return true;
      const choice = await openDialog({
        title: 'You are still editing this page', iconName: 'edit', tone: 'warn',
        body: [h('p', null, 'Your changes are kept on this computer, but they are not published yet. If you leave, the page is unlocked so others can edit it.')],
        actions: [
          { label: 'Leave and keep my changes', value: 'leave' },
          { label: 'Stay here', value: 'stay', kind: 'primary', autofocus: true },
        ],
      });
      if (choice !== 'leave') return false;
      await closeEditor();
      return true;
    },
  };
}
