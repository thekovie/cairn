// Cairn browser app: bootstrap, routing, and the persistent layout.
//
// Routes (hash based, so every screen can be bookmarked):
//   #/                  home            #/page/<path>      article
//   #/folder/<path>     folder          #/edit/<path>      editor
//   #/new?folder=...    new page        #/history/<path>   earlier versions
//   #/search?q=...      search          #/settings         settings
//   #/setup             choose or create a documentation folder
//   #/templates         team and built-in templates
//   #/deleted           recently deleted pages and folders
//   #/print/<path>      a page laid out for printing (PDF fallback)

import {
  bootstrapToken, hasToken, get, h, clear, icon, button, iconButton, banner, href, announce, errorText, applyPrefs,
  appendChildren, actionInProgress, toast, linkButton,
} from './core.js';
import * as views from './views.js';
import { setupView } from './setup.js';
import { editorView } from './editor.js';
import { templatesView } from './templates.js';
import { printView } from './export.js';
import { deletedView } from './manage.js';
import { watchUpdates, updateBanner } from './update.js';

const app = {
  state: null,
  nav: [],
  tree: null,
  // Folders open in the page tree (lower-case paths).
  open: new Set(),
  current: null,
  currentHash: null,
  shell: null,
  routeSeq: 0,
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
  templates: templatesView,
  deleted: deletedView,
  print: printView,
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

/** Folders and pages as one tree, sorted by name, each folder counting
 *  the pages inside it (subfolders included). */
function buildTree(folderPaths, pages) {
  const root = { name: '', path: '', folders: [], pages: [], count: 0 };
  const byPath = new Map([['', root]]);
  const node = (path) => {
    const key = path.toLowerCase();
    if (byPath.has(key)) return byPath.get(key);
    const cut = path.lastIndexOf('/');
    const n = { name: path.slice(cut + 1), path, folders: [], pages: [], count: 0 };
    byPath.set(key, n);
    node(cut === -1 ? '' : path.slice(0, cut)).folders.push(n);
    return n;
  };
  folderPaths.forEach(node);
  for (const p of pages) node(parentOf(p.path)).pages.push(p);
  const byName = (a, b) => a.localeCompare(b, undefined, { sensitivity: 'base', numeric: true });
  const finish = (n) => {
    n.folders.sort((a, b) => byName(a.name, b.name));
    n.pages.sort((a, b) => byName(a.title, b.title));
    n.count = n.pages.length + n.folders.reduce((sum, f) => sum + finish(f), 0);
    return n.count;
  };
  finish(root);
  return root;
}

const parentOf = (path) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '');

async function refreshNav() {
  if (!app.state?.workspace) {
    app.nav = [];
    app.tree = null;
    return;
  }
  try {
    const [{ pages }, { folders }] = await Promise.all([get('/api/pages'), get('/api/folders')]);
    app.tree = buildTree(folders, pages);
    app.nav = app.tree.folders.map((f) => ({ name: f.name, path: f.path, page_count: f.count }));
  } catch {
    app.nav = [];
    app.tree = null;
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

  const navToggle = button('Pages', {
    icon: 'menu', class: 'btn btn-quiet nav-toggle',
    'aria-expanded': 'false', 'aria-controls': 'sidenav',
    onClick: () => (app.shell.root.classList.contains('nav-open') ? closeNav(true) : openNav()),
  });
  const topbar = h('header', { class: 'topbar' },
    navToggle,
    h('a', { class: 'brand', href: href.home() },
      h('img', { class: 'brand-mark', src: '/static/favicon.svg', alt: '' }),
      h('span', { class: 'brand-text' },
        h('span', { class: 'brand-name' }, 'Cairn'),
        h('span', { class: 'brand-ws' }, ws.name))),
    searchForm,
    h('div', { class: 'topbar-actions' },
      h('a', { class: 'btn btn-quiet', href: href.settings() }, icon('settings'), h('span', null, 'Settings'))));

  const banners = h('div', { class: 'banners', id: 'banners' });
  const nav = h('nav', { class: 'sidenav', id: 'sidenav', 'aria-label': 'Main' });
  const scrim = h('div', { class: 'nav-scrim', hidden: true, onclick: () => closeNav(true) });
  const main = h('main', { id: 'main', tabindex: '-1' });
  const shell = h('div', { class: 'shell' }, topbar, banners, nav, scrim, main);

  const root = clear(document.getElementById('app'));
  root.append(shell);
  root.removeAttribute('aria-busy');
  app.shell = { root: shell, main, nav, scrim, navToggle, banners, searchInput };
  renderBanners();
  renderNav();
}

// On narrow windows the page tree is a drawer.
function openNav() {
  const { root, scrim, navToggle, nav } = app.shell;
  root.classList.add('nav-open');
  scrim.hidden = false;
  navToggle.setAttribute('aria-expanded', 'true');
  (nav.querySelector('[aria-current="page"]') || nav.querySelector('a, button'))?.focus();
}

function closeNav(returnFocus = false) {
  if (!app.shell?.root.classList.contains('nav-open')) return;
  const { root, scrim, navToggle } = app.shell;
  root.classList.remove('nav-open');
  scrim.hidden = true;
  navToggle.setAttribute('aria-expanded', 'false');
  if (returnFocus) navToggle.focus();
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
  // A new version is news, not a task: only where people look for it.
  const { route } = parseHash();
  const update = route === '' || route === 'settings' ? updateBanner() : null;
  if (update) host.append(update);
}

function navLink(label, target, iconName, active) {
  return h('li', null, h('a', { class: 'navlink', href: target, 'aria-current': active ? 'page' : null },
    icon(iconName), h('span', null, label)));
}

/** The page and folder being shown, if any. */
function currentLocation() {
  const { route, path } = parseHash();
  if (['page', 'edit', 'history'].includes(route)) return { page: path.toLowerCase(), folder: parentOf(path).toLowerCase() };
  if (route === 'folder') return { page: null, folder: path.toLowerCase() };
  return { page: null, folder: null };
}

/** Open the folders that lead to what's on screen, so it's visible in the tree. */
function revealCurrent() {
  const { folder } = currentLocation();
  if (!folder) return;
  const parts = folder.split('/');
  parts.forEach((_, i) => app.open.add(parts.slice(0, i + 1).join('/')));
}

function treeCount(n) {
  return h('span', { class: 'tree-count' }, String(n), h('span', { class: 'visually-hidden' }, n === 1 ? ' page' : ' pages'));
}

function treeFolder(folder, loc) {
  const key = folder.path.toLowerCase();
  const isOpen = app.open.has(key);
  const hasItems = folder.folders.length + folder.pages.length > 0;
  let toggle = h('span', { class: 'tree-spacer' });
  if (hasItems) {
    toggle = iconButton(`${isOpen ? 'Hide' : 'Show'} what’s in ${folder.name}`, {
      icon: 'chevron', class: 'btn btn-icon tree-toggle', 'aria-expanded': String(isOpen),
      onClick: () => {
        if (isOpen) app.open.delete(key); else app.open.add(key);
        renderNav();
        app.shell.nav.querySelector(`[data-folder="${CSS.escape(key)}"]`)?.focus();
      },
    });
    toggle.dataset.folder = key;
  }
  return h('li', { class: 'tree-folder' },
    h('div', { class: 'tree-row' },
      toggle,
      h('a', {
        class: 'tree-link', href: href.folder(folder.path), title: folder.name,
        'aria-current': !loc.page && loc.folder === key ? 'page' : null,
      }, h('span', { class: 'tree-name' }, folder.name), treeCount(folder.count))),
    isOpen && hasItems ? treeList(folder, loc) : null);
}

function treeList(node, loc, labelledBy = null) {
  return h('ul', { class: 'tree', 'aria-labelledby': labelledBy },
    node.folders.map((f) => treeFolder(f, loc)),
    node.pages.map((p) => h('li', null, h('div', { class: 'tree-row' },
      h('span', { class: 'tree-spacer' }),
      h('a', {
        class: 'tree-link', href: href.page(p.path), title: p.title,
        'aria-current': loc.page === p.path.toLowerCase() ? 'page' : null,
      }, h('span', { class: 'tree-name' }, p.title))))));
}

function renderNav() {
  const nav = app.shell?.nav;
  if (!nav) return;
  const { route, path } = parseHash();
  const top = (path.split('/')[0] || '').toLowerCase();
  const tree = app.tree;
  clear(nav);
  // The tree is labelled but isn't a heading, so the page's own title
  // stays the first heading screen readers meet.
  nav.append(
    button('Close', { icon: 'x', class: 'btn btn-quiet sidenav-close', onClick: () => closeNav(true) }),
    app.state.workspace?.read_only
      ? null
      : h('a', { class: 'btn btn-new', href: href.newPage(route === 'folder' ? path : '') },
        icon('pagePlus'), h('span', null, 'New page')),
    h('ul', { class: 'navlist' }, navLink('Home', href.home(), 'home', route === '')),
    h('div', { class: 'sidenav-tree' },
      h('p', { class: 'nav-label', id: 'nav-pages' }, 'Pages'),
      tree && tree.count + tree.folders.length === 0
        ? h('p', { class: 'help tree-empty' }, 'No pages yet.')
        : null,
      tree ? treeList(tree, currentLocation(), 'nav-pages') : null),
    h('div', { class: 'sidenav-more' },
      h('ul', { class: 'navlist', 'aria-label': 'More' },
        navLink('Templates', href.templates(), 'template',
          route === 'templates' || (route === 'edit' && top === '_templates')),
        navLink('Recently deleted', href.deleted(), 'trash', route === 'deleted'))));
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

// ------------------------------------------------------------- loading
// A shared folder on a slow network can take a while. If a screen isn't
// ready after a moment, show a skeleton with plain words about what's
// happening; if it's slow, say so, so nobody thinks Cairn has frozen.

const SHOW_LOADER_AFTER_MS = 250;
const SLOW_AFTER_MS = 6000;
const LOADING_TEXT = {
  '': 'Opening the home page…',
  page: 'Opening the page…',
  edit: 'Opening the editor…',
  folder: 'Opening the folder…',
  search: 'Searching…',
  history: 'Opening earlier versions…',
  new: 'Getting ready…',
  templates: 'Opening templates…',
  deleted: 'Opening recently deleted…',
  settings: 'Opening settings…',
  print: 'Preparing the page for printing…',
};

/** Shows the loader in `main` if loading takes a moment; returns `done()`. */
function routeLoader(main, route) {
  let loader = null;
  const timers = [setTimeout(() => {
    const slow = h('p', { class: 'loading-slow', hidden: true },
      'The shared folder is responding slowly. Cairn is still working on it, so there’s no need to click again.');
    loader = h('div', { class: 'route-loading', role: 'status' },
      h('p', { class: 'loading-label' },
        h('span', { class: 'spinner', 'aria-hidden': 'true' }), LOADING_TEXT[route] || 'Loading…'),
      h('div', { class: 'skeleton', 'aria-hidden': 'true' },
        h('span', { class: 'sk sk-title' }), h('span', { class: 'sk' }),
        h('span', { class: 'sk' }), h('span', { class: 'sk sk-short' })),
      slow);
    main.setAttribute('aria-busy', 'true');
    Element.prototype.prepend.call(main, loader);
    // Not while a question is open: then Cairn is waiting for the person, not the folder.
    timers.push(setTimeout(() => { slow.hidden = Boolean(document.querySelector('dialog[open]')); }, SLOW_AFTER_MS - SHOW_LOADER_AFTER_MS));
  }, SHOW_LOADER_AFTER_MS)];
  return () => {
    timers.forEach(clearTimeout);
    if (loader) loader.remove();
    main.removeAttribute('aria-busy');
  };
}

async function onRoute() {
  // Something is being saved to the shared folder: stay until it's done.
  const working = actionInProgress();
  if (working && app.currentHash) {
    history.replaceState(null, '', app.currentHash);
    renderNav();
    toast(`${working.replace(/…$/, '')} is still going. Please wait for it to finish, then try again.`, { error: true });
    return;
  }
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
  // A dialog belongs to the screen that opened it (e.g. after the Back button).
  for (const dlg of document.querySelectorAll('dialog[open]')) dlg.close('cancel');

  let parsed = parseHash();
  if (!app.state.workspace && parsed.route !== 'setup') {
    history.replaceState(null, '', href.setup());
    parsed = parseHash();
  }
  app.currentHash = location.hash || '#/';

  let host;
  if (parsed.route === 'setup') {
    app.shell = null;
    const root = clear(document.getElementById('app'));
    root.removeAttribute('aria-busy');
    host = h('main', { id: 'main', class: 'setup', tabindex: '-1' });
    root.append(host);
  } else {
    if (!app.shell) renderShell();
    closeNav();
    revealCurrent();
    renderNav();
    renderBanners();
    refreshNav(); // page counts may have changed
    host = clear(app.shell.main);
    // Home and Search have their own big search box; one per screen.
    app.shell.searchInput.value = '';
    app.shell.root.classList.toggle('has-own-search', parsed.route === '' || parsed.route === 'search');
  }

  // Each screen draws into its own container. If the person moves on before
  // it has finished loading (easy on a slow drive), its container is already
  // detached, so late content can never mix into the new screen.
  const seq = ++app.routeSeq;
  const isCurrent = () => seq === app.routeSeq;
  const main = h('div', { class: 'route-view' });
  Element.prototype.append.call(host, main);
  const loaded = routeLoader(main, parsed.route);

  // Views pass optional sections as null; skip them like h() does.
  main.append = (...children) => {
    if (!isCurrent()) return;
    loaded();
    appendChildren(main, children);
  };

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
    if (!isCurrent()) {
      // Too late: let it tidy up (an editor gives its lock back).
      if (result?.abandon) result.abandon();
      else if (result?.cleanup) result.cleanup();
      return;
    }
    loaded();
    app.current = result || null;
    document.title = [result?.title, app.state.workspace?.name, 'Cairn'].filter(Boolean).join(' – ');
    if (!parsed.query.get('section')) window.scrollTo(0, 0);
    if (!result?.keepFocus) {
      // Move focus to the new heading so screen readers announce the page.
      const heading = main.querySelector('h1') || host;
      if (heading !== host) heading.tabIndex = -1;
      heading.focus({ preventScroll: true });
    }
    if (result?.title) announce(`${result.title} opened`);
  } catch (err) {
    if (!isCurrent()) return;
    document.title = ['Could not be shown', app.state.workspace?.name, 'Cairn'].filter(Boolean).join(' – ');
    // A page that is gone won't come back by trying again: offer ways to find it.
    const gone = ['not_found', 'path_rejected'].includes(err?.code);
    main.append(banner({
      tone: 'danger', title: 'This screen could not be shown', text: errorText(err),
      actions: gone
        ? [linkButton('Go to the home page', href.home(), { icon: 'home' }),
          linkButton('Search all pages', href.search(''), { icon: 'search' })]
        : [button('Try again', { icon: 'refresh', onClick: () => onRoute() })],
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
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && app.shell?.root.classList.contains('nav-open')) closeNav(true);
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
  watchUpdates(() => renderBanners());
}

boot();
