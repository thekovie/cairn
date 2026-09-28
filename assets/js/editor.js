// The page editor.
//
// Designed so a page can be written and formatted without knowing Markdown:
// worded toolbar buttons insert the right characters, the preview updates as
// you type, and "Page details" edits the information block for you.

import {
  api, get, post, h, clear, icon, button, linkButton, banner, href, toast, announce, whileBusy,
  openDialog, confirmDialog, formDialog, errorText, diffView, formatTime, formatDateTime,
} from './core.js';
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

function todayYmd() {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
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
    ['Heading', '## Heading', 'A section title. Use one # for the page title.'],
    ['Bold', '**words**', 'Makes words stand out.'],
    ['Italic', '_words_', 'Slanted words.'],
    ['Bullet list', '- item', 'One item per line.'],
    ['Numbered list', '1. step', 'Numbers are filled in for you.'],
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
  const actions = [
    linkButton('Back to the page', href.page(path), { icon: 'back', kind: 'primary' }),
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

  // ---------------------------------------------------------------- DOM
  const titleEl = h('h1', null, 'Editing');
  const lockStatus = h('span', { class: 'status-item is-ok' });
  const saveStatus = h('span', { class: 'status-item', role: 'status' });
  const notices = h('div');
  const ta = h('textarea', { id: 'md-text', spellcheck: 'true', 'aria-describedby': 'drop-hint' });
  ta.value = start.content;
  const previewBody = h('div', { class: 'md-body' });
  const writePane = h('div', { class: 'pane pane-write' },
    h('label', { class: 'pane-label', for: 'md-text' }, 'Write'),
    ta,
    h('p', { class: 'drop-hint', id: 'drop-hint' }, 'Tip: you can paste a picture here, or drag one in from a folder.'));
  // Side by side only when there's room; narrow windows start on Write.
  const startView = window.matchMedia('(max-width: 1000px)').matches ? 'write' : 'both';
  const panes = h('div', { class: 'editor-panes', 'data-view': startView },
    writePane,
    h('section', { class: 'pane pane-preview', 'aria-labelledby': 'preview-h' },
      h('h2', { class: 'pane-label', id: 'preview-h' }, 'Preview: how the page will look'),
      previewBody));
  const fileInput = h('input', {
    type: 'file', accept: 'image/png,image/jpeg,image/webp,image/gif',
    class: 'visually-hidden', tabindex: '-1', 'aria-hidden': 'true',
  });
  const publishError = h('div', { style: 'flex-basis: 100%' });
  const publishBtn = button('Publish changes', { icon: 'publish', kind: 'primary', large: true, 'aria-describedby': 'publish-note' });
  const publishNote = h('span', { class: 'publish-note', id: 'publish-note' }, 'Everyone will see this version.');

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
      ? 'Everyone will see this version.'
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
    clearTimeout(s.saveTimer);
    if (!s.dirty || s.closed) return true;
    const text = ta.value;
    setSave(null, 'Saving…');
    try {
      const res = await post('/api/draft/save', { path, content: text });
      s.dirty = ta.value !== text;
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
      const res = await post('/api/preview', { path, content: ta.value });
      previewBody.innerHTML = res.html; // sanitized by the server
      titleEl.textContent = `Editing: ${res.title}`;
      document.title = `Editing: ${res.title} – Cairn`;
    } catch { /* keep the last preview */ }
  }
  function schedulePreview() {
    clearTimeout(s.previewTimer);
    s.previewTimer = setTimeout(refreshPreview, PREVIEW_DELAY_MS);
  }

  // Declared below; used by `changed`.
  let syncDetailsFromText = () => {};

  function changed() {
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
    if (kind === 'h2' || kind === 'h3') {
      const mark = kind === 'h2' ? '## ' : '### ';
      out = lines.map((l) => mark + (l.replace(/^#{1,6}\s*/, '') || 'Section title'));
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
    const sel = ta.value.slice(ta.selectionStart, ta.selectionEnd);
    if (sel.includes('\n')) insertBlock(`\`\`\`\n${sel}\n\`\`\``);
    else wrap('`', '`', 'code');
  }

  async function insertLink() {
    const selStart = ta.selectionStart;
    const selEnd = ta.selectionEnd;
    const selected = ta.value.slice(selStart, selEnd);
    let pages = [];
    try { pages = (await get('/api/pages')).pages.filter((p) => p.path !== path); } catch { pages = []; }
    const values = await formDialog({
      title: 'Add a link', iconName: 'link',
      intro: 'A link can go to another page in this documentation, or to a website.',
      fields: [
        { name: 'text', label: 'Words to show', value: selected, required: true, help: 'For example: the printer guide' },
        {
          name: 'page', label: 'Link to a page in this documentation', type: 'select', value: '',
          options: [{ value: '', label: 'Not a page: I’ll type a web address below' },
            ...pages.map((p) => ({ value: p.path, label: `${p.title} (${p.path})` }))],
        },
        { name: 'url', label: 'Or a web address', value: '', help: 'Starts with https://. Leave empty if you chose a page above.' },
      ],
      submitLabel: 'Add link',
    });
    ta.focus();
    ta.setSelectionRange(selStart, selEnd);
    if (!values) return;
    let target = values.page ? relativeLink(path, values.page) : values.url.trim();
    if (!target) {
      toast('The link needs a page or a web address, so nothing was added.', { error: true });
      return;
    }
    if (!values.page && !/^(https?:|mailto:|#)/i.test(target)) target = `https://${target}`;
    replaceSelection(`[${values.text.trim().replace(/[[\]]/g, '')}](${target})`);
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
    ta.focus();
    ta.setSelectionRange(selStart, selStart);
    if (!values) return;
    const cols = Math.min(8, Math.max(1, parseInt(values.cols, 10) || 3));
    const rows = Math.min(30, Math.max(1, parseInt(values.rows, 10) || 3));
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
    const values = await formDialog({
      title: 'Describe this picture', iconName: 'image',
      intro: 'A short description helps people who can’t see the picture, and is shown if the picture can’t be loaded.',
      fields: [{ name: 'alt', label: 'Description', value: guess }],
      submitLabel: 'Insert picture',
    });
    ta.focus();
    ta.setSelectionRange(pos, pos);
    if (!values) return;
    setSave(null, 'Adding picture…');
    try {
      const q = new URLSearchParams({ path, name: file.name || 'picture' });
      const res = await api('POST', `/api/draft/image?${q}`, file, { raw: true });
      const alt = (values.alt || 'Picture').replace(/[[\]]/g, '');
      insertBlock(`![${alt}](${res.link})`);
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
  const tool = (label, iconName, fn) => button(label, { icon: iconName, onClick: () => { fn(); activity(); } });
  const viewBtn = (label, view) => button(label, {
    'aria-pressed': view === startView ? 'true' : 'false',
    onClick: (e) => {
      panes.dataset.view = view;
      for (const b of e.currentTarget.parentElement.children) b.setAttribute('aria-pressed', String(b === e.currentTarget));
    },
  });
  const toolbar = h('div', { class: 'toolbar', role: 'toolbar', 'aria-label': 'Formatting', 'aria-controls': 'md-text' },
    tool('Heading', 'heading', () => prefixLines('h2')),
    tool('Subheading', 'heading', () => prefixLines('h3')),
    tool('Bold', 'bold', () => wrap('**', '**', 'bold words')),
    tool('Italic', 'italic', () => wrap('_', '_', 'slanted words')),
    h('span', { class: 'toolbar-sep', 'aria-hidden': 'true' }),
    tool('Bullet list', 'list', () => prefixLines('ul')),
    tool('Numbered list', 'listNumbered', () => prefixLines('ol')),
    h('span', { class: 'toolbar-sep', 'aria-hidden': 'true' }),
    tool('Link…', 'link', insertLink),
    tool('Table…', 'table', insertTable),
    tool('Code', 'code', code),
    tool('Insert picture…', 'image', () => fileInput.click()),
    h('div', { class: 'view-switch', role: 'group', 'aria-label': 'What to show' },
      viewBtn('Write and preview', 'both'), viewBtn('Write only', 'write'), viewBtn('Preview only', 'preview')));

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
    const { fields } = readMeta(ta.value);
    for (const key of META_KEYS) {
      if (document.activeElement !== detailInputs[key]) detailInputs[key].value = fields[key] || '';
    }
  };
  function applyDetails() {
    const fields = Object.fromEntries(META_KEYS.map((k) => [k, detailInputs[k].value]));
    const next = writeMeta(ta.value, fields);
    if (next !== ta.value) {
      ta.value = next;
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

  const helpPanel = h('details', { class: 'details-panel' },
    h('summary', null, 'Formatting help'),
    h('div', { class: 'details-body' },
      h('p', { class: 'help', style: 'margin: 0 0 var(--space-3)' }, 'The buttons above do all of this for you. This is only for anyone who prefers typing.'),
      helpTable(),
      h('p', { class: 'help' }, 'Keyboard shortcuts (optional): Ctrl+B bold, Ctrl+I italic, Ctrl+S save now.')));

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
    window.removeEventListener('beforeunload', onBeforeUnload);
  }

  async function doPublish(acceptHash, acceptMissing) {
    clear(publishError);
    let res;
    try {
      res = await whileBusy(publishBtn, 'Publishing…', () => post('/api/publish', {
        path, content: ta.value, accept_current_hash: acceptHash || null, accept_missing: Boolean(acceptMissing),
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
          : h('p', null, 'Below, lines marked − are only in the version that is published now. Lines marked + are only in yours. You can copy anything you need from their version into yours before publishing.'),
        diffView(res.diff, { oldLabel: 'Only in the version published now', newLabel: 'Only in your version' }),
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

  publishBtn.addEventListener('click', () => doPublish(null, false));

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
        ctx.navigate(s.isNew ? href.folder(folderOf(path)) : href.page(path));
      } catch (err) { toast(errorText(err), { error: true }); }
    },
  });

  const closeBtn = button('Close editor', {
    icon: 'exit',
    onClick: async (e) => {
      await whileBusy(e.currentTarget, 'Closing…', closeEditor);
      toast('Your changes are kept on this computer. You can continue later.');
      ctx.navigate(s.isNew ? href.folder(folderOf(path)) : href.page(path));
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
    if (s.dirty && !s.closed) {
      api('POST', '/api/draft/save', { path, content: ta.value }, { keepalive: true }).catch(() => {});
    }
  }, { once: true });

  // ------------------------------------------------------------ render
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
      titleEl,
      h('div', { class: 'editor-status' }, lockStatus, saveStatus)),
    notices,
    toolbar,
    panes,
    fileInput,
    h('div', { class: 'editor-foot' },
      h('div', { class: 'actions' }, closeBtn, discardBtn),
      h('div', { class: 'publish-group' }, publishNote, publishBtn),
      publishError),
    detailsPanel,
    helpPanel);

  refreshPreview();
  setTimeout(() => ta.focus(), 0);

  return {
    title: 'Editing',
    keepFocus: true,
    cleanup: finish,
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
