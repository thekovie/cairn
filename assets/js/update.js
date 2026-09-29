// New versions of Cairn: the notice at the top of the window, the Updates
// section in Settings, and the "Updating Cairn…" screen while it restarts.

import {
  get, post, h, icon, button, linkButton, banner, href, toast, errorText, confirmDialog,
  whileBusy, relativeTime, clear, appendChildren,
} from './core.js';

const DISMISSED_KEY = 'cairn.updateDismissed';
/** How often the page asks this computer's Cairn what it last found. */
const REFRESH_EVERY_MS = 30 * 60 * 1000;
/** Cairn's own daily check runs a minute after it starts. */
const FIRST_REFRESH_MS = 90 * 1000;
const RESTART_POLL_MS = 1000;
const RESTART_GIVE_UP_MS = 60 * 1000;

let status = null;
const listeners = new Set();

function setStatus(next) {
  status = next;
  for (const listener of [...listeners]) listener(status);
}

async function refresh() {
  try {
    setStatus(await get('/api/update'));
  } catch { /* not important enough to interrupt anyone */ }
  return status;
}

/** Keep the update status fresh. `onChange` redraws what depends on it. */
export function watchUpdates(onChange) {
  listeners.add(onChange);
  refresh();
  setTimeout(refresh, FIRST_REFRESH_MS);
  setInterval(refresh, REFRESH_EVERY_MS);
}

function dismissedVersion() {
  try { return localStorage.getItem(DISMISSED_KEY); } catch { return null; }
}

function dismiss(version) {
  try { localStorage.setItem(DISMISSED_KEY, version); } catch { /* shown again next time */ }
}

function downloadLink(url) {
  return h('a', { class: 'btn', href: url, target: '_blank', rel: 'noopener noreferrer' },
    icon('download'), h('span', null, 'Open the download page'));
}

/** The notice at the top of the window, when a new version is out. */
export function updateBanner() {
  const next = status?.available;
  if (!next || status.installing || dismissedVersion() === next.version) return null;
  return banner({
    tone: 'info',
    title: `Cairn ${next.version} is available`,
    text: status.can_install
      ? 'Updating takes a few seconds. Your pages and unsaved changes are kept.'
      : status.cannot_install_reason,
    actions: [
      status.can_install
        ? button('Update and restart', { icon: 'download', kind: 'primary', onClick: (e) => startUpdate(e.currentTarget) })
        : downloadLink(next.page_url),
      linkButton('What’s new', href.settings(), { icon: 'info' }),
      button('Not now', { kind: 'quiet', onClick: () => { dismiss(next.version); setStatus(status); } }),
    ],
    role: 'status',
  });
}

/** Install the new version (or go back to the kept one) and restart. */
async function startUpdate(btn, { back = false } = {}) {
  const version = back ? status.previous : status.available?.version;
  if (!version) return;
  const ok = await confirmDialog({
    title: back ? `Go back to Cairn ${version}?` : `Update Cairn to ${version}?`,
    message: 'Cairn closes and starts again by itself, which takes a few seconds. This tab reconnects on its own. Your pages and unsaved changes are kept.',
    confirmLabel: back ? `Go back to ${version}` : 'Update and restart',
    cancelLabel: 'Not now',
    iconName: back ? 'history' : 'download',
  });
  if (!ok) return;
  try {
    const res = await whileBusy(btn, back ? 'Going back…' : 'Downloading…',
      () => post(back ? '/api/update/rollback' : '/api/update/install'));
    showRestarting(res.version, back);
  } catch (err) {
    toast(errorText(err), { error: true });
    refresh();
  }
}

/** Covers the page until the new Cairn answers, then reloads into it. */
function showRestarting(version, back) {
  const note = h('p', { class: 'help' }, 'There’s no need to close this tab or click anything.');
  const title = back ? `Going back to Cairn ${version}` : `Updating Cairn to ${version}`;
  const dlg = h('dialog', { class: 'dlg', 'aria-labelledby': 'restart-h' },
    h('div', { class: 'dlg-form' },
      h('div', { class: 'dlg-head is-info' }, icon('refresh'), h('h2', { id: 'restart-h' }, title)),
      h('div', { class: 'dlg-body' },
        h('p', { class: 'loading-label', role: 'status' },
          h('span', { class: 'spinner', 'aria-hidden': 'true' }), 'Cairn is restarting…'),
        note)));
  dlg.addEventListener('cancel', (e) => e.preventDefault());
  document.body.append(dlg);
  dlg.showModal();
  const started = Date.now();
  const poll = async () => {
    try {
      const state = await get('/api/state');
      if (state.version === version) {
        location.reload();
        return;
      }
    } catch { /* still restarting */ }
    if (Date.now() - started > RESTART_GIVE_UP_MS) {
      clear(note).append('Cairn hasn’t started again by itself. Start Cairn from the Start menu (or double-click cairn.exe), then reload this tab. Your unsaved changes are kept.');
      note.classList.replace('help', 'field-error');
      return;
    }
    setTimeout(poll, RESTART_POLL_MS);
  };
  setTimeout(poll, RESTART_POLL_MS);
}

function statusLine(s) {
  if (s.checking) return 'Checking for a new version…';
  if (s.available) return `Cairn ${s.available.version} is available.`;
  if (s.checked_at) return `You have the latest version (checked ${relativeTime(s.checked_at)}).`;
  return s.error ? null : 'Cairn hasn’t checked for a new version yet.';
}

async function checkNow(btn) {
  try {
    setStatus(await whileBusy(btn, 'Checking…', () => post('/api/update/check')));
    if (status.error) toast(status.error, { error: true });
    else toast(status.available ? `Cairn ${status.available.version} is available.` : 'You have the latest version.');
  } catch (err) {
    toast(errorText(err), { error: true });
  }
}

function releaseNotes(next) {
  const notes = h('div', { class: 'details-body md-body release-notes', trustedHtml: next.notes_html });
  for (const a of notes.querySelectorAll('a[href^="http"]')) {
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
  }
  return h('details', { class: 'details-panel' }, h('summary', null, `What’s new in ${next.version}`), notes);
}

/** Settings → Updates. `radioGroup` and `save` come from the Settings screen. */
export function updatesSection(cfg, { radioGroup, save }) {
  const body = h('div', { class: 'update-status' });
  const render = (s) => {
    if (!body.isConnected && body.dataset.drawn) {
      listeners.delete(render);
      return;
    }
    body.dataset.drawn = '1';
    clear(body);
    if (!s) {
      body.append(h('p', { class: 'help' }, 'Update information isn’t available right now.'));
      return;
    }
    const line = statusLine(s);
    appendChildren(body, [
      h('p', null, `This computer has Cairn ${s.current}.`),
      line ? h('p', { class: s.available ? 'update-available' : 'help' }, line) : null,
      s.error ? banner({ tone: 'warn', title: 'Couldn’t check for a new version', text: s.error }) : null,
    ]);
    if (s.available) {
      body.append(releaseNotes(s.available));
      if (!s.can_install) body.append(banner({ tone: 'info', text: s.cannot_install_reason, actions: [downloadLink(s.available.page_url)] }));
    }
    body.append(h('div', { class: 'actions' },
      s.available && s.can_install
        ? button('Update and restart', { icon: 'download', kind: 'primary', onClick: (e) => startUpdate(e.currentTarget) })
        : null,
      button('Check now', { icon: 'refresh', onClick: (e) => checkNow(e.currentTarget) }),
      s.previous && s.can_install
        ? button(`Go back to ${s.previous}`, { icon: 'history', kind: 'quiet', onClick: (e) => startUpdate(e.currentTarget, { back: true }) })
        : null));
  };
  listeners.add(render);
  render(status);
  refresh();
  return h('section', { class: 'settings-section', 'aria-labelledby': 'set-updates' },
    h('h2', { id: 'set-updates' }, 'Updates'),
    body,
    radioGroup('update_check', 'Look for new versions', [
      { value: 'daily', label: 'Every day', text: 'Cairn asks GitHub once a day whether a new version is out, and tells you. Nothing about you or your documentation is sent.' },
      { value: 'manual', label: 'Only when I choose Check now', text: 'Cairn never goes online on its own.' },
    ], cfg.update_check, (v) => save({ update_check: v }, 'Update setting saved.')));
}
