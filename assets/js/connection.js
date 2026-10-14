// Tells the person at once if Cairn stops while this tab is open, so they
// are not left typing into a page that can no longer save.

import { api, h, button, banner, START_SHORT } from './core.js';

const POLL_MS = 3000;
/** One missed answer can be a busy moment; two in a row is a stop. */
const MISSES_BEFORE_BANNER = 2;

let misses = 0;
let paused = false;
let checking = false;
let timer = null;
let notice = null;

/** Whether Cairn answers. Any answer counts, except "you are not who I know". */
async function answers() {
  try {
    await api('GET', '/api/ping');
    return true;
  } catch (err) {
    return err.status !== 0 && err.status !== 401 && err.status !== 403;
  }
}

function hide() {
  notice?.remove();
  notice = null;
}

function show() {
  if (notice) return;
  notice = h('div', { class: 'connection-lost' }, banner({
    tone: 'danger',
    title: 'Cairn has stopped',
    text: `${START_SHORT}; it opens in a new tab. Copy any text you were typing first.`,
    actions: [button('Check again', { icon: 'refresh', onClick: () => check() })],
  }));
  document.body.prepend(notice);
}

async function check() {
  if (paused || checking) return;
  checking = true;
  try {
    if (await answers()) {
      misses = 0;
      hide();
    } else if (++misses >= MISSES_BEFORE_BANNER) {
      show();
    }
  } finally {
    checking = false;
    schedule();
  }
}

function schedule() {
  clearTimeout(timer);
  if (!paused) timer = setTimeout(check, POLL_MS);
}

/** Cairn is going away on purpose (Quit, or an update restarting it). */
export function pauseWatching() {
  paused = true;
  clearTimeout(timer);
  misses = 0;
  hide();
}

export function resumeWatching() {
  if (!paused) return;
  paused = false;
  check();
}

/** Start watching. A failed request, or coming back to the tab, looks at once. */
export function watchConnection() {
  window.addEventListener('cairn:offline', () => { misses = Math.max(misses, 1); check(); });
  document.addEventListener('visibilitychange', () => { if (!document.hidden) check(); });
  schedule();
}
