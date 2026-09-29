// Shared client helpers: API access, DOM building, icons, dialogs, feedback.
//
// Text from the server is always inserted as text nodes. The only HTML ever
// inserted is article HTML that the server has already sanitized.

const TOKEN_KEY = 'cairn.token';
let token = null;

export function bootstrapToken() {
  const m = location.hash.match(/(?:^#|[?&])t=([0-9a-f]{64})/);
  if (m) {
    token = m[1];
    try { sessionStorage.setItem(TOKEN_KEY, token); } catch { /* private mode: keep in memory */ }
    history.replaceState(null, '', `${location.pathname}#/`);
    return;
  }
  try { token = sessionStorage.getItem(TOKEN_KEY); } catch { token = null; }
}

export const hasToken = () => Boolean(token);

/** Apply the appearance and text-size settings to the page. */
export function applyPrefs(config) {
  const root = document.documentElement;
  root.dataset.theme = config?.appearance || 'light';
  root.dataset.text = config?.text_size || 'normal';
  root.dataset.toolbar = config?.toolbar_labels ? 'words' : 'icons';
  setTimeZone(config?.timezone || null);
}

// ------------------------------------------------------------------- API

export class ApiError extends Error {
  constructor(status, code, message, data) {
    super(message);
    this.status = status;
    this.code = code;
    this.data = data;
  }
}

export async function api(method, path, body, { keepalive = false, raw = false } = {}) {
  const headers = { 'X-Cairn-Token': token || '' };
  let payload;
  if (raw) {
    payload = body;
    headers['Content-Type'] = 'application/octet-stream';
  } else if (body !== undefined) {
    payload = JSON.stringify(body);
    headers['Content-Type'] = 'application/json';
  }
  let res;
  try {
    res = await fetch(path, { method, headers, body: payload, keepalive, cache: 'no-store' });
  } catch {
    throw new ApiError(0, 'offline',
      "Cairn isn't responding. Check that the Cairn window is still open, then try again.");
  }
  const text = await res.text();
  let data = null;
  try { data = text ? JSON.parse(text) : null; } catch { data = null; }
  if (!res.ok) {
    throw new ApiError(res.status, data?.error || 'error',
      data?.message || 'Something went wrong. Please try again.', data);
  }
  return data;
}

export const get = (path, query) =>
  api('GET', query ? `${path}?${new URLSearchParams(query)}` : path);
export const post = (path, body) => api('POST', path, body ?? {});

// ------------------------------------------------------------------- DOM

export function h(tag, props, ...children) {
  const el = document.createElement(tag);
  if (props) {
    for (const [key, value] of Object.entries(props)) {
      if (value === null || value === undefined || value === false) continue;
      if (key === 'class') el.className = value;
      // CSSOM, not the style attribute: the Content-Security-Policy forbids inline style attributes.
      else if (key === 'style') el.style.cssText = value;
      else if (key === 'dataset') Object.assign(el.dataset, value);
      else if (key === 'trustedHtml') el.innerHTML = value; // server-sanitized article HTML only
      else if (key.startsWith('on') && typeof value === 'function') {
        el.addEventListener(key.slice(2).toLowerCase(), value);
      } else if (typeof value === 'boolean' && key in el) {
        el[key] = value;
      } else if (typeof value === 'boolean') {
        el.setAttribute(key, '');
      } else if (key === 'value' && 'value' in el) {
        el.value = value;
      } else {
        el.setAttribute(key, value);
      }
    }
  }
  appendChildren(el, children);
  return el;
}

export function appendChildren(el, children) {
  for (const child of children.flat(Infinity)) {
    if (child === null || child === undefined || child === false) continue;
    Element.prototype.append.call(el, child instanceof Node ? child : document.createTextNode(String(child)));
  }
  return el;
}

export function clear(el) {
  while (el.firstChild) el.firstChild.remove();
  return el;
}

// ----------------------------------------------------------------- icons
// Simple line icons drawn for Cairn (24 × 24 grid).

const ICONS = {
  search: '<circle cx="11" cy="11" r="7"/><path d="M16.5 16.5 21 21"/>',
  home: '<path d="M3 11 12 3l9 8"/><path d="M5 10v10h5v-6h4v6h5V10"/>',
  folder: '<path d="M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z"/>',
  folderPlus: '<path d="M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2.5h8.5A1.5 1.5 0 0 1 21 9v9.5a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 18.5z"/><path d="M12 11v5M9.5 13.5h5"/>',
  page: '<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M9 13h7M9 17h7"/>',
  pagePlus: '<path d="M6 3h8l5 5v13H6z"/><path d="M14 3v5h5M12.5 11.5v6M9.5 14.5h6"/>',
  edit: '<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="m13.5 6.5 4 4"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  settings: '<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>',
  lock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>',
  unlock: '<rect x="5" y="11" width="14" height="10" rx="2"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/>',
  check: '<path d="m5 12.5 4.5 4.5L19 7"/>',
  checkCircle: '<circle cx="12" cy="12" r="9"/><path d="m8 12.5 3 3 5-6"/>',
  alert: '<path d="M12 3 2 20h20z"/><path d="M12 10v4M12 17.5v.01"/>',
  info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.01"/>',
  x: '<path d="M6 6l12 12M18 6 6 18"/>',
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="m21 16-5-5-9 9"/>',
  history: '<path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v5h5M12 8v4l3 2"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  back: '<path d="M19 12H5M11 6l-6 6 6 6"/>',
  link: '<path d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1"/><path d="M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1"/>',
  table: '<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 10h18M3 15h18M9 4v16"/>',
  list: '<path d="M9 6h11M9 12h11M9 18h11"/><path d="M4.5 6h.01M4.5 12h.01M4.5 18h.01" stroke-width="3"/>',
  listNumbered: '<path d="M10 6h10M10 12h10M10 18h10"/><path d="M4 5h1.5v4M4 9h3M4 14.5a1.5 1.5 0 0 1 3 .5l-3 3h3"/>',
  heading: '<path d="M6 4v16M18 4v16M6 12h12"/>',
  bold: '<path d="M7 4h6a4 4 0 0 1 0 8H7zM7 12h7a4 4 0 0 1 0 8H7z"/>',
  italic: '<path d="M10 4h8M6 20h8M14 4 10 20"/>',
  code: '<path d="m8 7-5 5 5 5M16 7l5 5-5 5"/>',
  trash: '<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>',
  publish: '<path d="M12 15V4M7 9l5-5 5 5"/><path d="M4 15v4a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-4"/>',
  exit: '<path d="M14 4h5v16h-5"/><path d="m10 8-4 4 4 4M6 12h9"/>',
  eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
  refresh: '<path d="M20 11a8 8 0 1 0-2.3 5.7"/><path d="M20 4v7h-7"/>',
  power: '<path d="M12 3v9"/><path d="M6.3 7.3a8 8 0 1 0 11.4 0"/>',
  help: '<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17.5v.01"/>',
  user: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
  download: '<path d="M12 4v11M7 10l5 5 5-5"/><path d="M4 15v4a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-4"/>',
  template: '<rect x="4" y="3" width="16" height="18" rx="1.5"/><path d="M8 7h8M8 11h8M8 15h4" stroke-dasharray="2 2"/>',
  copy: '<rect x="8" y="8" width="12" height="12" rx="1.5"/><path d="M16 8V5.5A1.5 1.5 0 0 0 14.5 4h-9A1.5 1.5 0 0 0 4 5.5v9A1.5 1.5 0 0 0 5.5 16H8"/>',
  globe: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/>',
};

export function icon(name) {
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 24 24');
  svg.setAttribute('class', 'icon');
  svg.setAttribute('aria-hidden', 'true');
  svg.setAttribute('focusable', 'false');
  svg.innerHTML = ICONS[name] || ICONS.info; // constant markup defined above
  return svg;
}

/**
 * Every button has visible words. Icons only ever sit beside the label.
 * kind: 'primary' | 'danger' | 'quiet' | undefined
 */
export function button(label, { icon: iconName, kind, onClick, type = 'button', large, ...rest } = {}) {
  if (!label || !String(label).trim()) throw new Error('Buttons must have a visible text label');
  const cls = ['btn', kind && `btn-${kind}`, large && 'btn-large'].filter(Boolean).join(' ');
  return h('button', { type, class: cls, onclick: onClick, ...rest },
    iconName ? icon(iconName) : null, h('span', null, label));
}

/**
 * A compact toolbar button. The label is always in the button (screen
 * readers read it, and it becomes visible when "Show words" is on or on
 * touch screens); otherwise it appears as a tooltip on hover and keyboard
 * focus, with the keyboard shortcut if there is one.
 */
export function iconButton(label, { icon: iconName, shortcut, onClick, ...rest } = {}) {
  if (!label || !String(label).trim()) throw new Error('Buttons must have a visible text label');
  const tip = shortcut ? `${label} (${shortcut})` : label;
  return h('button', {
    type: 'button', class: 'btn btn-icon', onclick: onClick, dataset: { tip },
    'aria-keyshortcuts': shortcut ? shortcut.replace('Ctrl', 'Control') : null, ...rest,
  }, icon(iconName), h('span', { class: 'btn-icon-label' }, label));
}

/** Arrow keys, Home, and End move between the controls of a toolbar. */
export function toolbarKeys(toolbar) {
  toolbar.addEventListener('keydown', (e) => {
    if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(e.key)) return;
    const items = [...toolbar.querySelectorAll('button:not([disabled]), select')]
      .filter((el) => el.offsetParent !== null);
    const i = items.indexOf(e.target);
    if (i === -1) return;
    const next = { ArrowLeft: i - 1, ArrowRight: i + 1, Home: 0, End: items.length - 1 }[e.key];
    e.preventDefault();
    items[(next + items.length) % items.length].focus();
  });
  return toolbar;
}

export function linkButton(label, target, { icon: iconName, kind, large } = {}) {
  const cls = ['btn', kind && `btn-${kind}`, large && 'btn-large'].filter(Boolean).join(' ');
  return h('a', { class: cls, href: target }, iconName ? icon(iconName) : null, h('span', null, label));
}

/** Show a working state on a button while `fn` runs. */
export async function whileBusy(btn, busyLabel, fn) {
  const original = Array.from(btn.childNodes);
  btn.disabled = true;
  btn.setAttribute('aria-busy', 'true');
  clear(btn).append(h('span', { class: 'spinner', 'aria-hidden': 'true' }), h('span', null, busyLabel));
  try {
    return await fn();
  } finally {
    btn.disabled = false;
    btn.removeAttribute('aria-busy');
    clear(btn).append(...original);
  }
}

// ------------------------------------------------------------- feedback

export function announce(message) {
  const live = document.getElementById('live');
  if (!live) return;
  live.textContent = '';
  setTimeout(() => { live.textContent = message; }, 50);
}

export function toast(message, { error = false } = {}) {
  const host = document.getElementById('toasts');
  if (!host) return;
  const el = h('div', { class: `toast${error ? ' is-error' : ''}`, role: error ? 'alert' : 'status' },
    icon(error ? 'alert' : 'checkCircle'), h('span', null, message));
  host.append(el);
  setTimeout(() => el.remove(), error ? 9000 : 5000);
}

export function banner({ tone = 'info', title, text, actions = [], role }) {
  const iconName = { info: 'info', warn: 'alert', danger: 'alert', ok: 'checkCircle' }[tone];
  return h('div', { class: `banner banner-${tone}`, role: role || (tone === 'danger' ? 'alert' : null) },
    icon(iconName),
    h('div', { class: 'banner-body' },
      title ? h('strong', null, title) : null,
      text ? (text instanceof Node ? text : h('p', null, text)) : null,
      actions.length ? h('div', { class: 'actions' }, actions) : null));
}

export function emptyState({ title, text, actions = [] }) {
  return h('div', { class: 'empty' }, h('h3', null, title), text ? h('p', null, text) : null,
    actions.length ? h('div', { class: 'actions' }, actions) : null);
}

export function fieldError(message) {
  return h('p', { class: 'field-error', role: 'alert' }, icon('alert'), h('span', null, message));
}

export function errorText(err) {
  return err instanceof ApiError ? err.message : 'Something went wrong. Please try again.';
}

// --------------------------------------------------------------- dialogs

let dialogSeq = 0;

/**
 * Open a modal dialog. Resolves with the value of the button used, or
 * 'cancel' for Escape. Buttons use verbs, not OK/Cancel.
 */
export function openDialog({ title, iconName = 'info', tone = 'info', body = [], actions, wide = false, onSubmit, onOpen }) {
  return new Promise((resolve) => {
    const id = `dlg-title-${++dialogSeq}`;
    const dlg = h('dialog', { class: `dlg${wide ? ' dlg-wide' : ''}`, 'aria-labelledby': id });
    const form = h('form', { method: 'dialog', class: 'dlg-form', novalidate: '' });
    const foot = h('div', { class: 'dlg-foot' });
    for (const a of actions) {
      const b = button(a.label, { icon: a.icon, kind: a.kind, type: a.submit ? 'submit' : 'button', value: a.value });
      if (!a.submit) b.addEventListener('click', () => dlg.close(a.value));
      if (a.autofocus) b.autofocus = true;
      if (a.left) b.classList.add('push-left');
      foot.append(b);
    }
    form.append(
      h('div', { class: `dlg-head is-${tone}` }, icon(iconName), h('h2', { id }, title)),
      h('div', { class: 'dlg-body' }, body),
      foot);
    form.addEventListener('submit', (e) => {
      e.preventDefault();
      const value = e.submitter?.value || 'submit';
      if (onSubmit && onSubmit(value, form) === false) return;
      dlg.close(value);
    });
    dlg.append(form);
    dlg.addEventListener('close', () => {
      const value = dlg.returnValue || 'cancel';
      dlg.remove();
      resolve(value);
    });
    document.body.append(dlg);
    dlg.showModal();
    if (onOpen) onOpen(dlg);
  });
}

// --------------------------------------------------------- picture viewer

const PICTURE_MAX_ENLARGE = 3;

/** Show a picture large, over a dimmed page. Esc, "Close", or a click on
 *  the dimmed area closes it. Small pictures are enlarged (up to 3×) and
 *  big ones are fitted to the window. */
export function openPicture(src, alt) {
  const img = h('img', { src, alt: alt || '' });
  const close = button('Close', { icon: 'x', kind: 'quiet' });
  close.classList.add('lightbox-close');
  const dlg = h('dialog', { class: 'lightbox', 'aria-label': alt ? `Picture: ${alt}` : 'Picture' },
    close,
    h('figure', null, img, alt ? h('figcaption', null, alt) : null));
  const fit = () => {
    if (!img.naturalWidth) return;
    const room = Math.min(
      (window.innerWidth * 0.92) / img.naturalWidth,
      (window.innerHeight * 0.8) / img.naturalHeight);
    const scale = Math.min(room, PICTURE_MAX_ENLARGE);
    img.style.width = `${Math.round(img.naturalWidth * scale)}px`;
  };
  img.addEventListener('load', fit);
  window.addEventListener('resize', fit);
  close.addEventListener('click', () => dlg.close());
  dlg.addEventListener('click', (e) => { if (e.target === dlg) dlg.close(); });
  dlg.addEventListener('close', () => {
    window.removeEventListener('resize', fit);
    dlg.remove();
  });
  document.body.append(dlg);
  dlg.showModal();
  if (img.complete) fit();
  close.focus();
}

/** Make every picture in `container` open large when clicked (or with
 *  Enter/Space when reached with Tab). */
export function enablePictureZoom(container) {
  for (const img of container.querySelectorAll('img')) {
    img.classList.add('zoomable');
    img.tabIndex = 0;
    img.setAttribute('role', 'button');
    img.setAttribute('aria-label', `Show larger: ${img.alt || 'picture'}`);
  }
  const open = (img) => openPicture(img.currentSrc || img.src, img.alt);
  container.addEventListener('click', (e) => {
    const img = e.target.closest('img.zoomable');
    if (img) open(img);
  });
  container.addEventListener('keydown', (e) => {
    const img = e.target.closest('img.zoomable');
    if (img && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
      open(img);
    }
  });
}

export async function confirmDialog({ title, message, details, confirmLabel, cancelLabel, danger = false, iconName }) {
  const value = await openDialog({
    title,
    tone: danger ? 'danger' : 'warn',
    iconName: iconName || 'alert',
    body: [message ? h('p', null, message) : null, details || null],
    actions: [
      { label: cancelLabel, value: 'cancel', autofocus: true },
      { label: confirmLabel, value: 'confirm', kind: danger ? 'danger' : 'primary' },
    ],
  });
  return value === 'confirm';
}

/**
 * A small form in a dialog. fields: [{ name, label, type, value, help,
 * options, required, min, max }]. Resolves with values or null.
 */
export async function formDialog({ title, intro, media, fields, submitLabel, cancelLabel = 'Go back', iconName = 'edit' }) {
  const inputs = {};
  const rows = fields.map((f) => {
    const fid = `fd-${++dialogSeq}`;
    let input;
    if (f.type === 'select') {
      input = h('select', { id: fid, name: f.name },
        f.options.map((o) => h('option', { value: o.value, selected: o.value === f.value }, o.label)));
    } else {
      input = h('input', {
        id: fid, name: f.name, type: f.type || 'text', value: f.value ?? '',
        min: f.min, max: f.max, autocomplete: 'off',
      });
    }
    if (f.help) input.setAttribute('aria-describedby', `${fid}-help`);
    inputs[f.name] = input;
    return h('div', { class: 'field' },
      h('label', { for: fid }, f.label, f.required ? ' (required)' : ''),
      input,
      f.help ? h('p', { class: 'help', id: `${fid}-help` }, f.help) : null,
      h('div', { class: 'error-slot' }));
  });
  let result = null;
  const value = await openDialog({
    title, iconName, tone: 'info',
    body: [intro ? h('p', null, intro) : null, media || null, ...rows],
    actions: [
      { label: cancelLabel, value: 'cancel' },
      { label: submitLabel, value: 'ok', kind: 'primary', submit: true },
    ],
    onSubmit: (v) => {
      if (v !== 'ok') return true;
      let firstBad = null;
      for (const f of fields) {
        const input = inputs[f.name];
        const slot = input.parentElement.querySelector('.error-slot');
        clear(slot);
        input.removeAttribute('aria-invalid');
        if (f.required && !String(input.value).trim()) {
          slot.append(fieldError(`Please fill in “${f.label}”.`));
          input.setAttribute('aria-invalid', 'true');
          firstBad ??= input;
        }
      }
      if (firstBad) {
        firstBad.focus();
        return false;
      }
      result = Object.fromEntries(fields.map((f) => [f.name, inputs[f.name].value]));
      return true;
    },
  });
  return value === 'ok' ? result : null;
}

// ------------------------------------------------------------ formatting

// Every stored time is UTC. Times are shown in the timezone chosen in
// Settings (or this computer's), and clock times carry an offset label such
// as "GMT+8" so nobody has to guess which zone a time is in.

/** This computer's own timezone, e.g. "Asia/Manila". */
export const systemTimeZone = () => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';

let formats = makeFormats(undefined);

function makeFormats(timeZone) {
  const tz = timeZone ? { timeZone } : {};
  const make = (opts, locale) => new Intl.DateTimeFormat(locale, { ...opts, ...tz });
  const day = { year: 'numeric', month: 'short', day: 'numeric' };
  const clock = { hour: 'numeric', minute: '2-digit' };
  return {
    zone: timeZone || systemTimeZone(),
    dateTime: make({ ...day, ...clock, timeZoneName: 'shortOffset' }),
    time: make({ ...clock, timeZoneName: 'shortOffset' }),
    clock: make(clock),
    day: make(day),
    ymd: make({ year: 'numeric', month: '2-digit', day: '2-digit' }, 'en-CA'),
  };
}

/** Use `name` (an IANA timezone) for every time shown; null = this computer's. */
export function setTimeZone(name) {
  try {
    formats = makeFormats(name || undefined);
  } catch {
    formats = makeFormats(undefined); // unknown to this browser: fall back safely
  }
}

/** The timezone times are shown in, e.g. "Asia/Manila". */
export const timeZoneName = () => formats.zone;

/** "GMT+8" for `zone` at `date` (now by default). */
export function zoneOffsetLabel(zone, date = new Date()) {
  try {
    const parts = new Intl.DateTimeFormat('en-US', { timeZone: zone, timeZoneName: 'shortOffset' })
      .formatToParts(date);
    return parts.find((p) => p.type === 'timeZoneName')?.value || 'GMT';
  } catch {
    return '';
  }
}

export const formatDateTime = (secs) => formats.dateTime.format(new Date(secs * 1000));
export const formatTime = (secs) => formats.time.format(new Date(secs * 1000));

export function formatIsoDateTime(iso) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : formats.dateTime.format(d);
}

/** Today's calendar date as "YYYY-MM-DD" in the chosen timezone. */
export const todayYmd = () => formats.ymd.format(new Date());

/** "2026-01-31" → "Jan 31, 2026" (a calendar date, never shifted by timezone). */
export function formatDay(ymd) {
  if (!ymd) return '';
  const [y, m, d] = ymd.split('-').map(Number);
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeZone: 'UTC' })
    .format(new Date(Date.UTC(y, m - 1, d)));
}

function dayNumber(date) {
  const [y, m, d] = formats.ymd.format(date).split('-').map(Number);
  return Date.UTC(y, m - 1, d) / 86400000;
}

/** Whether a moment falls on today's date in the chosen timezone. */
export const isToday = (secs) => dayNumber(new Date()) === dayNumber(new Date(secs * 1000));

/** "today at 2:30 PM", "yesterday at …", "3 days ago", or a date. Days are
 *  counted in the chosen timezone; the zone label is left out because it is
 *  implied and only adds noise here. */
export function relativeTime(secs) {
  if (!secs) return '';
  const then = new Date(secs * 1000);
  const days = dayNumber(new Date()) - dayNumber(then);
  if (days === 0) return `today at ${formats.clock.format(then)}`;
  if (days === 1) return `yesterday at ${formats.clock.format(then)}`;
  if (days > 1 && days < 7) return `${days} days ago`;
  return formats.day.format(then);
}

/** A <time> element (machine-readable UTC for screen readers and tools). */
export function timeEl(secs, text) {
  return h('time', { datetime: new Date(secs * 1000).toISOString() }, text);
}

export function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} bytes`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1048576).toFixed(1)} MB`;
}

// ------------------------------------------------------------ navigation

export const encodePath = (p) => p.split('/').map(encodeURIComponent).join('/');
export const href = {
  home: () => '#/',
  folder: (p) => (p ? `#/folder/${encodePath(p)}` : '#/'),
  page: (p) => `#/page/${encodePath(p)}`,
  edit: (p) => `#/edit/${encodePath(p)}`,
  history: (p) => `#/history/${encodePath(p)}`,
  newPage: (folder) => `#/new?${new URLSearchParams({ folder: folder || '' })}`,
  search: (q) => `#/search?${new URLSearchParams({ q: q || '' })}`,
  settings: () => '#/settings',
  setup: () => '#/setup',
  templates: () => '#/templates',
  deleted: () => '#/deleted',
  print: (p) => `#/print/${encodePath(p)}`,
};

// -------------------------------------------------------------- downloads

/** File name from a Content-Disposition header (prefers the UTF-8 form). */
function dispositionName(header, fallback) {
  const star = header?.match(/filename\*=UTF-8''([^;]+)/i);
  if (star) {
    try { return decodeURIComponent(star[1]); } catch { /* fall through */ }
  }
  return header?.match(/filename="([^"]+)"/i)?.[1] || fallback;
}

/**
 * Fetch a file with the per-launch token and hand it to the browser as a
 * download. Resolves with the saved file name; throws ApiError on failure.
 */
export async function downloadFile(path, query, fallbackName = 'download') {
  const url = query ? `${path}?${new URLSearchParams(query)}` : path;
  let res;
  try {
    res = await fetch(url, { headers: { 'X-Cairn-Token': token || '' }, cache: 'no-store' });
  } catch {
    throw new ApiError(0, 'offline',
      "Cairn isn't responding. Check that the Cairn window is still open, then try again.");
  }
  if (!res.ok) {
    let data = null;
    try { data = await res.json(); } catch { data = null; }
    throw new ApiError(res.status, data?.error || 'error',
      data?.message || 'The download could not be made. Please try again.', data);
  }
  const name = dispositionName(res.headers.get('Content-Disposition'), fallbackName);
  const blobUrl = URL.createObjectURL(await res.blob());
  const a = h('a', { href: blobUrl, download: name, class: 'visually-hidden' });
  document.body.append(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(blobUrl), 60000);
  return name;
}

export function breadcrumbsNav(crumbs, { includeHome = true } = {}) {
  const items = [];
  if (includeHome) items.push(h('li', null, h('a', { href: href.home() }, 'Home')));
  for (const c of crumbs) items.push(h('li', null, h('a', { href: href.folder(c.path) }, c.name)));
  return h('nav', { class: 'breadcrumbs', 'aria-label': 'You are here' }, h('ol', null, items));
}

// -------------------------------------------------------- shared widgets

export function statusChip(status) {
  if (!status) return null;
  const map = {
    active: ['checkCircle', 'Active'],
    draft: ['edit', 'Draft'],
    retired: ['x', 'Retired'],
  };
  const [iconName, label] = map[status] || ['info', status];
  return h('span', { class: `chip chip-${status}` }, icon(iconName), label);
}

const DIFF_CONTEXT = 3;

/** Pair a line diff into side-by-side rows: [kind, old text, new text].
 *  A run of removed lines followed by added lines becomes "changed" rows. */
function diffRows(lines) {
  const rows = [];
  for (let i = 0; i < lines.length;) {
    if (lines[i].kind === 'same') {
      rows.push(['same', lines[i].text, lines[i].text]);
      i += 1;
      continue;
    }
    const removed = [];
    const added = [];
    while (i < lines.length && lines[i].kind === 'removed') removed.push(lines[i++].text);
    while (i < lines.length && lines[i].kind === 'added') added.push(lines[i++].text);
    for (let k = 0; k < Math.max(removed.length, added.length); k++) {
      const [was, now] = [removed[k], added[k]];
      const kind = was !== undefined && now !== undefined ? 'changed' : was !== undefined ? 'removed' : 'added';
      rows.push([kind, was, now]);
    }
  }
  return rows;
}

const DIFF_WORD = { same: '', changed: 'Changed', removed: 'Removed', added: 'Added' };

function diffRow([kind, was, now]) {
  const cell = (text, present) => h('div', { class: `diff-cell${present ? '' : ' is-absent'}`, role: 'cell' },
    present ? (text || ' ') : h('span', { class: 'diff-absent' }, 'Not in this version'));
  return h('div', { class: `diff-row diff-${kind}`, role: 'row' },
    h('div', { class: 'diff-kind', role: 'cell' }, DIFF_WORD[kind]),
    cell(was, was !== undefined),
    cell(now, now !== undefined));
}

/**
 * Two versions side by side: the older on the left, the newer on the right.
 * Each changed row says what happened in words, never by color alone, and
 * long unchanged stretches are folded away.
 */
export function diffView(lines, { oldLabel = 'The published page', newLabel = 'Your version' } = {}) {
  const rows = diffRows(lines);
  if (!rows.some(([kind]) => kind !== 'same')) return h('p', null, 'There are no differences.');
  const table = h('div', { class: 'diff', role: 'table', 'aria-label': `${oldLabel} compared with ${newLabel}`, tabindex: '0' },
    h('div', { class: 'diff-row diff-head', role: 'row' },
      h('div', { class: 'diff-kind', role: 'columnheader' }, h('span', { class: 'visually-hidden' }, 'What changed')),
      h('div', { role: 'columnheader' }, oldLabel),
      h('div', { role: 'columnheader' }, newLabel)));
  for (let i = 0; i < rows.length; i++) {
    if (rows[i][0] === 'same') {
      let j = i;
      while (j < rows.length && rows[j][0] === 'same') j++;
      const head = i === 0 ? 0 : DIFF_CONTEXT;
      const tail = j === rows.length ? 0 : DIFF_CONTEXT;
      if (j - i > head + tail + 1) {
        for (let k = i; k < i + head; k++) table.append(diffRow(rows[k]));
        table.append(h('div', { class: 'diff-row diff-fold', role: 'row' },
          h('div', { class: 'diff-fold-text', role: 'cell' }, `${j - i - head - tail} lines that are the same in both`)));
        for (let k = j - tail; k < j; k++) table.append(diffRow(rows[k]));
        i = j - 1;
        continue;
      }
    }
    table.append(diffRow(rows[i]));
  }
  return table;
}

/** "You changed 2 lines and added 3 lines." for a line diff; null when identical. */
export function diffSummary(lines) {
  const rows = diffRows(lines);
  const count = (kind) => rows.filter(([k]) => k === kind).length;
  const lineWord = (n) => (n === 1 ? '1 line' : `${n} lines`);
  const parts = [
    count('changed') ? `changed ${lineWord(count('changed'))}` : null,
    count('added') ? `added ${lineWord(count('added'))}` : null,
    count('removed') ? `removed ${lineWord(count('removed'))}` : null,
  ].filter(Boolean);
  if (!parts.length) return null;
  const last = parts.pop();
  return `You ${parts.length ? `${parts.join(', ')} and ${last}` : last}.`;
}

/** A line diff of two texts, in the same shape the server sends. */
export function lineDiff(oldText, newText) {
  const a = oldText.split('\n');
  const b = newText.split('\n');
  // Longest common subsequence, filled from the end so the walk is forward.
  const lcs = Array.from({ length: a.length + 1 }, () => new Uint32Array(b.length + 1));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const out = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    if (a[i] === b[j]) {
      out.push({ kind: 'same', text: a[i] });
      i += 1;
      j += 1;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      out.push({ kind: 'removed', text: a[i++] });
    } else {
      out.push({ kind: 'added', text: b[j++] });
    }
  }
  while (i < a.length) out.push({ kind: 'removed', text: a[i++] });
  while (j < b.length) out.push({ kind: 'added', text: b[j++] });
  return out;
}
