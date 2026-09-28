// Cairn browser app: bootstrap, routing, and the persistent layout.
//
// Routes (hash based, so every screen can be bookmarked):
//   #/                  home            #/page/<path>      article
//   #/folder/<path>     folder          #/edit/<path>      editor
//   #/new?folder=...    new page        #/history/<path>   earlier versions
//   #/search?q=...      search          #/settings         settings
//   #/setup             choose or create a documentation folder

import {
  bootstrapToken, hasToken, get, h, clear, icon, button, banner, href, announce, errorText, applyPrefs,
  appendChildren,
} from './core.js';
import * as views from './views.js';
import { setupView } from './setup.js';
import { editorView } from './editor.js';

const app = {
  state: null,
  nav: [],
  current: null,
  currentHash: null,
  shell: null,
};

const ROUTES = {
  '': views.homeView,
  folder: views.folderView,
  page: views.pageView,
  edit: editorView,
  new: views.newPageView,
  search: views.searchView,
  history: views.historyView,
  settings: views.settingsView,
  setup: setupView,
};

function parseHash() {
  const raw = location.hash.replace(/^#\/?/, '');
  const [pathPart, queryPart = ''] = raw.split('?');
  const segments = pathPart.split('/').filter((s) => s !== '');
  const route = segments.shift() || '';
  let path = '';
  try { path = segments.map(decodeURIComponent).join('/'); } catch { path = segments.join('/'); }
  return { route, path, query: new URLSearchParams(queryPart) };
}

async function refreshState() {
  app.state = await get('/api/state');
  applyPrefs(app.state.config);
  return app.state;
}

async function refreshNav() {
  if (!app.state?.workspace) {
    app.nav = [];
    return;
  }
  try {
    const home = await get('/api/home');
    app.nav = home.categories;
  } catch {
    app.nav = [];
  }
  renderNav();
}

// ---------------------------------------------------------------- layout

function renderShell() {
  const ws = app.state.workspace;
  const searchInput = h('input', {
    type: 'search', name: 'q', id: 'top-search', autocomplete: 'off',
    'aria-label': 'Search all pages',
  });
  const searchForm = h('form', {
    class: 'topbar-search', role: 'search',
    onsubmit: (e) => {
      e.preventDefault();
      location.hash = href.search(searchInput.value.trim());
    },
  }, searchInput, button('Search', { icon: 'search', type: 'submit' }));

  const topbar = h('header', { class: 'topbar' },
    h('a', { class: 'brand', href: href.home() },
      h('img', { class: 'brand-mark', src: '/static/favicon.svg', alt: '' }),
      h('span', { class: 'brand-text' },
        h('span', { class: 'brand-name' }, 'Cairn'),
        h('span', { class: 'brand-ws' }, ws.name))),
    searchForm,
    h('div', { class: 'topbar-actions' },
      h('a', { class: 'btn btn-quiet', href: href.settings() }, icon('settings'), h('span', null, 'Settings'))));

  const banners = h('div', { class: 'banners', id: 'banners' });
  const nav = h('nav', { class: 'sidenav', 'aria-label': 'Main' });
  const main = h('main', { id: 'main', tabindex: '-1' });
  const shell = h('div', { class: 'shell' }, topbar, banners, nav, main);

  const root = clear(document.getElementById('app'));
  root.append(shell);
  root.removeAttribute('aria-busy');
  app.shell = { root: shell, main, nav, banners, searchInput };
  renderBanners();
  renderNav();
}

function renderBanners() {
  const host = app.shell?.banners;
  if (!host) return;
  clear(host);
  const ws = app.state.workspace;
  if (ws?.read_only) {
    host.append(banner({ tone: 'warn', title: 'Read only', text: ws.read_only }));
  } else if (ws && !ws.storage?.reliable_locking) {
    const notes = (ws.storage?.notes || []).join(' ');
    host.append(banner({
      tone: 'warn',
      title: 'Editing by several people at once is not safe in this folder',
      text: notes || 'This storage did not pass the safety check Cairn runs when a folder is opened.',
    }));
  }
  if (app.state.config_warning) {
    host.append(banner({ tone: 'warn', text: app.state.config_warning }));
  }
}

function navLink(label, target, iconName, active, count) {
  return h('li', null, h('a', { class: 'navlink', href: target, 'aria-current': active ? 'page' : null },
    icon(iconName), h('span', null, label),
    count !== undefined ? h('span', { class: 'count' }, String(count), h('span', { class: 'visually-hidden' }, count === 1 ? ' page' : ' pages')) : null));
}

function renderNav() {
  const nav = app.shell?.nav;
  if (!nav) return;
  const { route, path } = parseHash();
  const top = (path.split('/')[0] || '').toLowerCase();
  const inFolder = ['folder', 'page', 'edit', 'history'].includes(route);
  clear(nav);
  nav.append(
    h('div', { class: 'sidenav-section' },
      h('ul', { class: 'navlist' },
        navLink('Home', href.home(), 'home', route === ''),
        navLink('Search', href.search(''), 'search', route === 'search'))),
    h('div', { class: 'sidenav-section' },
      h('h2', null, 'Folders'),
      app.nav.length
        ? h('ul', { class: 'navlist' }, app.nav.map((f) =>
          navLink(f.name, href.folder(f.path), 'folder', inFolder && top === f.path.toLowerCase(), f.page_count)))
        : h('p', { class: 'help' }, 'No folders yet.')),
    app.state.workspace?.read_only
      ? null
      : h('div', { class: 'sidenav-section' },
        h('a', { class: 'btn btn-primary', href: href.newPage(route === 'folder' ? path : '') },
          icon('pagePlus'), h('span', null, 'New page'))),
    h('div', { class: 'sidenav-foot' },
      h('span', null, 'Folder location:'), h('br'), app.state.workspace.root));
}

function renderFatal(title, message, actions = []) {
  const root = clear(document.getElementById('app'));
  root.removeAttribute('aria-busy');
  root.append(h('main', { id: 'main', class: 'setup', tabindex: '-1' },
    h('h1', null, title),
    h('p', { class: 'lede' }, message),
    actions.length ? h('div', { class: 'actions', style: 'margin-top: 1.5rem' }, actions) : null));
}

// ---------------------------------------------------------------- routing

function makeContext(main, parsed) {
  return {
    app,
    main,
    ...parsed,
    navigate: (hash) => { location.hash = hash; },
    refreshState: async () => {
      await refreshState();
      if (app.state.workspace && app.shell) {
        const brand = app.shell.root.querySelector('.brand-ws');
        if (brand) brand.textContent = app.state.workspace.name;
        renderBanners();
      }
      if (!app.state.workspace) app.shell = null;
      return app.state;
    },
    refreshNav,
  };
}

async function onRoute() {
  if (app.current?.canLeave) {
    const ok = await app.current.canLeave();
    if (!ok) {
      // Put the address back without triggering another route change.
      history.replaceState(null, '', app.currentHash);
      renderNav();
      return;
    }
  }
  if (app.current?.cleanup) app.current.cleanup();
  app.current = null;

  let parsed = parseHash();
  if (!app.state.workspace && parsed.route !== 'setup') {
    history.replaceState(null, '', href.setup());
    parsed = parseHash();
  }
  app.currentHash = location.hash || '#/';

  let main;
  if (parsed.route === 'setup') {
    app.shell = null;
    const root = clear(document.getElementById('app'));
    root.removeAttribute('aria-busy');
    main = h('main', { id: 'main', class: 'setup', tabindex: '-1' });
    root.append(main);
  } else {
    if (!app.shell) renderShell();
    renderNav();
    refreshNav(); // page counts may have changed
    main = clear(app.shell.main);
    if (parsed.route === 'search') app.shell.searchInput.value = parsed.query.get('q') || '';
  }

  // Views pass optional sections as null; skip them like h() does.
  main.append = (...children) => appendChildren(main, children);

  const view = ROUTES[parsed.route];
  if (!view) {
    main.append(h('h1', null, 'Page not found'),
      h('p', null, 'That address doesn’t match anything in Cairn.'),
      h('p', null, h('a', { href: href.home() }, 'Go to the home page')));
    document.title = 'Not found – Cairn';
    return;
  }
  try {
    const result = await view(makeContext(main, parsed));
    app.current = result || null;
    document.title = [result?.title, app.state.workspace?.name, 'Cairn'].filter(Boolean).join(' – ');
    if (!parsed.query.get('section')) window.scrollTo(0, 0);
    if (!result?.keepFocus) {
      // Move focus to the new heading so screen readers announce the page.
      const heading = main.querySelector('h1') || main;
      if (heading !== main) heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
    }
    if (result?.title) announce(`${result.title} opened`);
  } catch (err) {
    main.append(banner({
      tone: 'danger', title: 'This screen could not be shown', text: errorText(err),
      actions: [button('Try again', { icon: 'refresh', onClick: () => onRoute() })],
    }));
  }
}

// ------------------------------------------------------------------ boot

async function boot() {
  bootstrapToken();
  document.addEventListener('click', (e) => {
    if (e.target.closest('[data-skip-link]')) {
      e.preventDefault();
      document.getElementById('main')?.focus();
    }
  });
  if (!hasToken()) {
    renderFatal('Please open Cairn from its program window',
      'This browser tab is not connected to Cairn. Start Cairn (or find its window) and it will open your documentation in a new tab. You can close this tab.');
    return;
  }
  try {
    await refreshState();
  } catch (err) {
    renderFatal('Cairn is not responding', errorText(err),
      [button('Try again', { icon: 'refresh', kind: 'primary', onClick: () => location.reload() })]);
    return;
  }
  window.addEventListener('hashchange', onRoute);
  window.addEventListener('cairn:nav-changed', () => refreshNav());
  await onRoute();
}

boot();
