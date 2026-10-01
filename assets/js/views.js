// Reading and navigation screens: home, folders, search, articles, earlier
// versions, new page, and settings.

import {
  get, post, h, clear, icon, button, linkButton, banner, emptyState, breadcrumbsNav, statusChip,
  relativeTime, formatDay, formatTime, formatDateTime, formatIsoDateTime, formatSize, href, toast,
  confirmDialog, formDialog, whileBusy, guardAction, errorText, diffView, fieldError, announce, applyPrefs,
  appendChildren, isToday, enablePictureZoom, ApiError,
} from './core.js';
import { openPageDownload, openBulkDownload } from './export.js';
import { pageMoreActions, folderOrganizeSection } from './manage.js';
import { updatesSection } from './update.js';
import { timezoneSection } from './timezone.js';

// ---------------------------------------------------------------- pieces

/** "Edited by Priya Shah, today at 4:44 AM", or when Cairn can't say who:
 *  "Changed today at 4:44 AM" (and "outside Cairn" if it changed elsewhere). */
function changedLine(when, by, outside) {
  if (!when) return '';
  const t = relativeTime(when);
  if (by) return `Edited by ${by}, ${t}`;
  return outside ? `Changed ${t}, outside Cairn` : `Changed ${t}`;
}

function pageRow(page, { showFolder = false } = {}) {
  const folder = page.path.includes('/') ? page.path.slice(0, page.path.lastIndexOf('/')) : '';
  return h('li', null, h('a', { class: 'row-link', href: href.page(page.path) },
    icon('page'),
    h('span', { class: 'row-title' }, page.title),
    h('span', { class: 'row-side' }, changedLine(page.edited_at || page.modified, page.edited_by, page.edited_outside)),
    h('span', { class: 'row-meta' },
      statusChip(page.status),
      page.owner ? h('span', null, `Owner: ${page.owner}`) : null,
      page.last_reviewed
        ? (reviewDue(page.last_reviewed)
          ? h('span', { class: 'review-due' }, icon('alert'), 'Review due')
          : h('span', null, `Reviewed ${formatDay(page.last_reviewed)}`))
        : null,
      showFolder && folder ? h('span', null, icon('folder'), ' ', folder.replaceAll('/', ' › ')) : null)));
}

// Unpublished work kept on this computer: new pages never published, and
// pages with changes. Each row opens the editor where the person left off.
function draftRow(d) {
  const target = `${href.edit(d.path)}${d.is_new ? '?new=1' : ''}`;
  return h('li', null, h('a', { class: 'row-link', href: target },
    icon('edit'),
    h('span', { class: 'row-title' }, d.title),
    h('span', { class: 'row-side' }, `Saved ${relativeTime(d.updated_at)}`),
    h('span', { class: 'row-meta' },
      h('span', { class: 'chip chip-draft' }, icon('edit'),
        d.is_new ? 'New page, not published yet' : 'Changes not published yet'),
      d.folder ? h('span', null, icon('folder'), ' ', d.folder.replaceAll('/', ' › ')) : null)));
}

function draftsSection(drafts, { id, title, text }) {
  if (!drafts.length) return null;
  return h('section', { class: 'section', 'aria-labelledby': id },
    h('h2', { id }, title),
    h('p', { class: 'help' }, text),
    h('ul', { class: 'list' }, drafts.map(draftRow)));
}

const loadDrafts = () => get('/api/drafts').then((r) => r.drafts).catch(() => []);

function folderRow(folder) {
  const n = folder.page_count;
  return h('li', null, h('a', { class: 'row-link', href: href.folder(folder.path) },
    icon('folder'),
    h('span', { class: 'row-title' }, folder.name),
    h('span', { class: 'row-side' }, n === 1 ? '1 page' : `${n} pages`)));
}

async function createFolder(ctx, parent) {
  const values = await formDialog({
    title: 'Create a new folder',
    iconName: 'folderPlus',
    intro: parent
      ? `The new folder will be inside “${parent.split('/').pop()}”.`
      : 'The new folder will appear in the list of folders.',
    fields: [{ name: 'name', label: 'Folder name', required: true, help: 'For example: Printers, or New staff.' }],
    submitLabel: 'Create folder',
  });
  if (!values) return;
  try {
    const res = await guardAction('Creating the folder…',
      () => post('/api/folder/create', { parent: parent || '', name: values.name.trim() }));
    toast(`Folder “${values.name.trim()}” created.`);
    window.dispatchEvent(new Event('cairn:nav-changed'));
    ctx.navigate(href.folder(res.path));
  } catch (err) {
    toast(errorText(err), { error: true });
  }
}

export function lockSentence(lock) {
  const when = isToday(lock.created_at) ? formatTime(lock.created_at) : formatDateTime(lock.created_at);
  return `Being edited by ${lock.display_name} since ${when}`;
}

// ------------------------------------------------------------------ home

const WELCOMED_KEY = 'cairn.welcomed';

/** First visit: the three things worth knowing, and who you'll appear as. */
function welcomePanel(ctx) {
  let seen = false;
  try { seen = localStorage.getItem(WELCOMED_KEY) === '1'; } catch { seen = false; }
  if (seen) return null;
  const { user, config } = ctx.app.state;
  const panel = h('section', { class: 'welcome', 'aria-labelledby': 'welcome-h' },
    h('h2', { id: 'welcome-h' }, 'Welcome to Cairn'),
    h('ul', { class: 'welcome-points' },
      h('li', null, h('strong', null, 'Reading: '), 'choose a folder on the left, or search for any word.'),
      h('li', null, h('strong', null, 'Changing a page: '), 'choose “Edit this page”, write as you would in a letter, then “Publish”. Nobody else sees your changes until you publish.'),
      h('li', null, h('strong', null, 'Mistakes can be undone: '), `every page keeps its ${ctx.app.state.workspace?.version_cleanup?.enabled ? 'recent ' : ''}earlier versions, and deleted pages wait in “Recently deleted”.`)),
    config.display_name
      ? null
      : h('p', null, `While you edit, others see you as “${user.os_user}”. `,
        h('a', { href: href.settings() }, 'Set the name they see')),
    h('div', { class: 'actions' },
      button('Got it', {
        icon: 'check', kind: 'primary',
        onClick: () => {
          try { localStorage.setItem(WELCOMED_KEY, '1'); } catch { /* shown again next time */ }
          panel.remove();
          document.getElementById('home-search')?.focus();
        },
      })));
  return panel;
}

export async function homeView(ctx) {
  const [data, drafts] = await Promise.all([get('/api/home'), loadDrafts()]);
  const readOnly = Boolean(ctx.app.state.workspace?.read_only);
  const searchInput = h('input', { type: 'search', id: 'home-search', name: 'q', autocomplete: 'off' });
  const count = data.total_pages === 1 ? '1 page' : `${data.total_pages} pages`;

  ctx.main.append(
    h('div', { class: 'page-head' },
      h('h1', null, data.name),
      h('p', { class: 'lede' }, `${count} of shared documentation. Choose a folder, or search for what you need.`),
      h('p', { class: 'ws-location' }, 'Stored in ', h('span', { class: 'path' }, ctx.app.state.workspace.root))),
    welcomePanel(ctx),
    h('form', {
      role: 'search',
      onsubmit: (e) => { e.preventDefault(); ctx.navigate(href.search(searchInput.value.trim())); },
    },
    h('label', { for: 'home-search' }, 'Search all pages'),
    h('div', { class: 'hero-search' }, searchInput,
      button('Search', { icon: 'search', type: 'submit', kind: 'primary', large: true }))),

    draftsSection(drafts, {
      id: 'drafts-h',
      title: 'Your unsaved changes',
      text: 'Pages you started or changed but haven’t published. They are kept only on this computer. Choose one to carry on.',
    }),

    h('section', { class: 'section', 'aria-labelledby': 'folders-h' },
      h('div', { class: 'page-head', style: 'margin-bottom: var(--space-4)' },
        h('h2', { id: 'folders-h' }, 'Folders'),
        readOnly ? null : button('New folder', { icon: 'folderPlus', onClick: () => createFolder(ctx, '') })),
      data.categories.length
        ? h('ul', { class: 'list' }, data.categories.map(folderRow))
        : emptyState({ title: 'No folders yet', text: 'Folders keep related pages together, such as “Guides” or “Troubleshooting”.' })),

    h('section', { class: 'section', 'aria-labelledby': 'recent-h' },
      h('h2', { id: 'recent-h' }, 'Recently changed'),
      data.recent.length
        ? h('ul', { class: 'list' }, data.recent.map((p) => pageRow(p, { showFolder: true })))
        : emptyState({
          title: 'No pages yet',
          text: 'Create the first page and it will show up here.',
          actions: readOnly ? [] : [linkButton('Create a page', href.newPage(''), { icon: 'pagePlus', kind: 'primary' })],
        })),

    data.root_pages.length
      ? h('section', { class: 'section', 'aria-labelledby': 'root-h' },
        h('h2', { id: 'root-h' }, 'Pages at the top level'),
        h('ul', { class: 'list' }, data.root_pages.map((p) => pageRow(p))))
      : null);
  return { title: 'Home' };
}

// ---------------------------------------------------------------- folder

export async function folderView(ctx) {
  const [data, drafts] = await Promise.all([get('/api/folder', { path: ctx.path }), loadDrafts()]);
  const newHereDrafts = drafts.filter((d) => d.is_new && d.folder.toLowerCase() === data.path.toLowerCase());
  const newHere = () => linkButton('New page here', href.newPage(data.path), { icon: 'pagePlus', kind: 'primary' });
  const pageCount = data.pages.length + data.folders.reduce((n, f) => n + (f.page_count || 0), 0);
  const downloadBtn = button('Download this folder', {
    icon: 'download',
    onClick: () => openBulkDownload({ scope: 'folder', path: data.path, name: data.name, pageCount }),
  });
  ctx.main.append(
    breadcrumbsNav(data.breadcrumbs.slice(0, -1)),
    h('div', { class: 'page-head' },
      h('h1', null, h('span', { class: 'visually-hidden' }, 'Folder: '), data.name),
      h('div', { class: 'actions' },
        data.can_write ? newHere() : null,
        data.can_write
          ? button('New folder inside', { icon: 'folderPlus', onClick: () => createFolder(ctx, data.path) })
          : null,
        pageCount ? downloadBtn : null)),
    data.folders.length
      ? h('section', { class: 'section', style: 'margin-top: 0', 'aria-labelledby': 'sub-h' },
        h('h2', { id: 'sub-h' }, 'Folders inside'),
        h('ul', { class: 'list' }, data.folders.map(folderRow)))
      : null,
    draftsSection(newHereDrafts, {
      id: 'new-drafts-h',
      title: 'New pages you haven’t published',
      text: 'Only you can see these, on this computer. Publish them when they’re ready.',
    }),
    h('section', { class: 'section', 'aria-labelledby': 'pages-h' },
      h('h2', { id: 'pages-h' }, 'Pages'),
      data.pages.length
        ? h('ul', { class: 'list' }, data.pages.map((p) => pageRow(p)))
        : emptyState({
          title: 'This folder has no pages yet',
          text: data.can_write ? 'Create the first page for this folder.' : 'Pages added here will appear in this list.',
          actions: data.can_write ? [newHere()] : [],
        })),
    folderOrganizeSection(ctx, data, pageCount));
  return { title: data.name || 'Folder' };
}

// ---------------------------------------------------------------- search

export async function searchView(ctx) {
  const q = ctx.query.get('q') || '';
  const input = h('input', { type: 'search', id: 'search-q', name: 'q', value: q, autocomplete: 'off' });
  ctx.main.append(
    h('div', { class: 'page-head' }, h('h1', null, 'Search')),
    h('form', { role: 'search', onsubmit: (e) => { e.preventDefault(); ctx.navigate(href.search(input.value.trim())); } },
      h('label', { for: 'search-q' }, 'Words to look for'),
      h('div', { class: 'hero-search' }, input,
        button('Search', { icon: 'search', type: 'submit', kind: 'primary', large: true })),
      h('p', { class: 'help' }, 'Pages that contain all of your words are shown. Words in the title count more.')));
  if (!q) {
    setTimeout(() => input.focus(), 0);
    return { title: 'Search', keepFocus: true };
  }
  const data = await get('/api/search', { q });
  const n = data.results.length;
  const results = h('section', { class: 'section', 'aria-labelledby': 'results-h' },
    h('h2', { id: 'results-h' }, n === 0 ? 'No pages found' : n === 1 ? '1 page found' : `${n} pages found`));
  if (n === 0) {
    // Never a dead end: a likely spelling, then the folders to browse.
    const folders = ctx.app.nav || [];
    appendChildren(results, [
      data.suggestion
        ? h('p', { class: 'suggestion' }, 'Did you mean ',
          h('a', { href: href.search(data.suggestion) }, `“${data.suggestion}”`), '?')
        : null,
      h('p', { class: 'help' }, `Nothing matches “${q}”. Check the spelling, try fewer words, or look through a folder:`),
      folders.length
        ? h('ul', { class: 'list' }, folders.map(folderRow))
        : null,
    ]);
  } else {
    results.append(h('ul', { class: 'list' }, data.results.map((r) => {
      const row = pageRow(r.page, { showFolder: true });
      row.firstChild.append(h('span', { class: 'excerpt' },
        r.excerpt.map((seg) => (seg.hit ? h('mark', null, seg.text) : seg.text))));
      return row;
    })));
  }
  ctx.main.append(results);
  announce(n === 1 ? '1 page found' : `${n} pages found`);
  return { title: `Search: ${q}` };
}

// --------------------------------------------------------------- article

/** An unavailable Edit button that can still be reached with the keyboard
 *  and says why, instead of a silent disabled button. */
function lockedEditButton(reasonId) {
  return button('Edit this page', {
    icon: 'lock', 'aria-disabled': 'true', 'aria-describedby': reasonId,
    onClick: () => {
      const reason = document.getElementById(reasonId);
      if (reason) announce(reason.textContent);
    },
  });
}

/** The page's buttons, and a note when someone is editing it. */
function editArea(ctx, data) {
  const lock = data.lock;
  const reasonId = 'edit-reason';
  const secondary = [
    linkButton('Earlier versions', href.history(data.path), { icon: 'history' }),
    button('Download', { icon: 'download', onClick: () => openPageDownload(ctx, data) }),
  ];
  const note = (iconName, text, extra) => h('div', { class: 'edit-note', id: reasonId },
    icon(iconName), h('p', null, text, extra ? h('span', { class: 'help' }, ` ${extra}`) : null));

  if (lock && !lock.is_mine) {
    if (lock.reclaimable_by_me) {
      return [h('div', { class: 'actions' },
        button('Continue where you left off', {
          icon: 'edit', kind: 'primary',
          onClick: async (e) => {
            try {
              await whileBusy(e.currentTarget, 'Opening…', () => post('/api/edit/reclaim', { path: data.path }));
              ctx.navigate(href.edit(data.path));
            } catch (err) { toast(errorText(err), { error: true }); }
          },
        }),
        secondary),
      note('edit', 'You left this page open for editing on this computer earlier. You can pick up where you stopped.')];
    }
    const extra = lock.possibly_abandoned
      ? 'They haven’t been active for a while, so it may have been left open by accident. A maintainer can release it (see “Abandoned edit locks” in the Troubleshooting guide).'
      : `You can keep reading. It opens for editing when they finish, or by itself if they stop typing for a while. You can also ask ${lock.display_name}.`;
    return [h('div', { class: 'actions' }, lockedEditButton(reasonId), secondary),
      note('lock', `${lockSentence(lock)}.`, extra)];
  }
  if (data.cannot_edit_reason) {
    return [h('div', { class: 'actions' }, lockedEditButton(reasonId), secondary),
      note('lock', data.cannot_edit_reason)];
  }
  const label = data.editing_here || data.has_draft ? 'Continue editing' : 'Edit this page';
  return [h('div', { class: 'actions' },
    linkButton(label, href.edit(data.path), { icon: 'edit', kind: 'primary' }),
    secondary,
    pageMoreActions(ctx, data))];
}

const REVIEW_DUE_DAYS = 365;

/** Whether a "last reviewed" date (YYYY-MM-DD) is more than a year ago. */
export function reviewDue(ymd) {
  const reviewed = Date.parse(`${ymd}T00:00:00`);
  return Number.isFinite(reviewed) && Date.now() - reviewed > REVIEW_DUE_DAYS * 86400000;
}

/** One quiet line under the title: status, owner, review, last change. */
function articleMeta(data) {
  const m = data.meta;
  const due = m.last_reviewed && reviewDue(m.last_reviewed);
  return h('p', { class: 'article-meta' },
    statusChip(m.status),
    m.owner ? h('span', null, `Owner: ${m.owner}`) : null,
    m.last_reviewed
      ? h('span', { class: due ? 'review-due' : null },
        due ? icon('alert') : null, `${due ? 'Review due: last reviewed' : 'Reviewed'} ${formatDay(m.last_reviewed)}`)
      : null,
    data.edited
      ? h('span', null, changedLine(data.edited.at, data.edited.by))
      : data.modified ? h('span', null, changedLine(data.modified, null, data.edited_outside)) : null,
    m.tags.length ? h('span', null, `Tags: ${m.tags.join(', ')}`) : null);
}

function scrollToSection(id) {
  const target = id && document.getElementById(id);
  if (!target) return;
  target.scrollIntoView({ block: 'start' });
  target.setAttribute('tabindex', '-1');
  target.focus({ preventScroll: true });
}

function wireArticleLinks(article, path) {
  for (const a of article.querySelectorAll('a[href^="http://"], a[href^="https://"]')) {
    a.target = '_blank';
    a.rel = 'noopener noreferrer';
  }
  article.addEventListener('click', (e) => {
    const a = e.target.closest('a');
    const target = a?.getAttribute('href') || '';
    if (target.startsWith('#') && !target.startsWith('#/')) {
      e.preventDefault();
      history.replaceState(null, '', `${href.page(path)}?section=${encodeURIComponent(target.slice(1))}`);
      scrollToSection(decodeURIComponent(target.slice(1)));
    }
  });
}

/** "On this page": beside the article on wide screens, above it on narrow
 *  ones (folded away until opened, so the article comes first). */
function tocNav(data) {
  const entries = data.toc.filter((t) => t.level > 1);
  if (entries.length < 2) return null;
  const narrow = window.matchMedia('(max-width: 1180px)');
  const box = h('details', { class: 'toc-box', open: !narrow.matches },
    h('summary', { id: 'toc-h' }, icon('chevron'), h('span', null, 'On this page')),
    h('ul', { class: 'toc' }, entries.map((t) =>
      h('li', { class: `lvl-${t.level}` }, h('a', { href: `#${t.id}` }, t.text)))));
  // Beside the text it's always open; above it, folded, so the text comes first.
  narrow.addEventListener('change', (e) => { if (box.isConnected) box.open = !e.matches; });
  return h('nav', { class: 'toc-nav', 'aria-labelledby': 'toc-h' }, box);
}

/** Marks the section being read in "On this page" as the page scrolls.
 *  Returns a function that stops following. */
function followSections(article, rail) {
  const headings = [...article.querySelectorAll('h2[id], h3[id], h4[id], h5[id], h6[id]')];
  if (headings.length < 2) return () => {};
  let queued = false;
  const update = () => {
    queued = false;
    // The last heading that has scrolled up past the top bar.
    const line = (document.querySelector('.topbar')?.offsetHeight || 0) + 96;
    let current = null;
    for (const hd of headings) {
      if (hd.getBoundingClientRect().top > line) break;
      current = hd;
    }
    for (const a of rail.querySelectorAll('.toc a')) {
      const on = current !== null && a.getAttribute('href') === `#${current.id}`;
      a.classList.toggle('is-current', on);
      if (on) a.setAttribute('aria-current', 'location'); else a.removeAttribute('aria-current');
    }
  };
  const onScroll = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(update);
  };
  window.addEventListener('scroll', onScroll, { passive: true });
  update();
  return () => window.removeEventListener('scroll', onScroll);
}

export async function pageView(ctx) {
  let data = await get('/api/page', { path: ctx.path });
  const notices = h('div', { class: 'article-notices' });
  const rail = h('div', { class: 'article-rail' });
  const head = h('header', { class: 'article-head' });

  function renderAll() {
    clear(notices);
    clear(rail);
    clear(head);
    if (data.has_draft && !data.editing_here && !data.cannot_edit_reason && !(data.lock && !data.lock.is_mine)) {
      notices.append(banner({
        tone: 'info', title: 'You have unsaved changes to this page',
        text: 'Your changes are kept on this computer and have not been published yet.',
        actions: [linkButton('Continue editing', href.edit(data.path), { icon: 'edit', kind: 'primary' })],
      }));
    }
    if (data.meta.warnings.length) {
      notices.append(banner({
        tone: 'warn', title: 'Some page details couldn’t be read',
        text: h('ul', null, data.meta.warnings.map((w) => h('li', null, w))),
      }));
    }
    if (data.broken_links.length) {
      const n = data.broken_links.length;
      notices.append(banner({
        tone: 'warn',
        title: n === 1 ? '1 link on this page is broken' : `${n} links on this page are broken`,
        text: 'They point to pages, pictures, or headings that don’t exist. They are marked with a dashed underline and the word “missing”.',
      }));
    }
    appendChildren(head, [h('h1', null, data.title), articleMeta(data), ...editArea(ctx, data)]);
    appendChildren(rail, [tocNav(data)]);
  }

  const article = h('article', { class: 'md-body', trustedHtml: data.html });
  // The first heading is already shown as the page title above.
  const firstH1 = article.firstElementChild;
  if (firstH1?.tagName === 'H1' && firstH1.textContent.trim() === data.title) firstH1.remove();
  wireArticleLinks(article, data.path);
  enablePictureZoom(article);
  rail.addEventListener('click', (e) => {
    const a = e.target.closest('.toc a');
    if (!a) return;
    e.preventDefault();
    const id = a.getAttribute('href').slice(1);
    history.replaceState(null, '', `${href.page(data.path)}?section=${encodeURIComponent(id)}`);
    scrollToSection(id);
  });
  renderAll();
  ctx.main.append(h('div', { class: 'article-page' },
    breadcrumbsNav(data.breadcrumbs), notices, head, rail, article));
  const section = ctx.query.get('section');
  if (section) setTimeout(() => scrollToSection(section), 0);
  const stopFollowing = followSections(article, rail);

  // Keep the lock state fresh and notice when someone publishes a change.
  const shownHash = data.hash;
  let updateShown = false;
  const timer = setInterval(async () => {
    try {
      const fresh = await get('/api/page', { path: ctx.path });
      const lockChanged = JSON.stringify(fresh.lock) !== JSON.stringify(data.lock);
      // Keep what is shown (including its hash, so Delete refuses if the
      // page changed since it was read); refresh only the editing state.
      data = { ...fresh, title: data.title, meta: data.meta, toc: data.toc, broken_links: data.broken_links, hash: data.hash };
      if (lockChanged) renderAll();
      if (fresh.hash !== shownHash && !updateShown) {
        updateShown = true;
        notices.prepend(banner({
          tone: 'info', title: 'This page has just been changed',
          text: 'Someone published a new version while you were reading.',
          actions: [button('Show the latest version', {
            icon: 'refresh', kind: 'primary',
            onClick: () => window.dispatchEvent(new HashChangeEvent('hashchange')),
          })],
          role: 'status',
        }));
      }
    } catch (err) {
      // The page was renamed, moved, or deleted while being read.
      if (err instanceof ApiError && err.status === 404 && !goneShown) {
        goneShown = true;
        clearInterval(timer);
        clear(rail);
        notices.prepend(banner({
          tone: 'warn', title: 'This page is no longer here',
          text: 'Someone renamed, moved, or deleted it while you were reading. You can still read this copy.',
          actions: [
            linkButton('Search for it', href.search(data.title), { icon: 'search', kind: 'primary' }),
            linkButton('Recently deleted', href.deleted(), { icon: 'trash' }),
          ],
          role: 'status',
        }));
      }
      /* otherwise the next tick will try again */
    }
  }, 30000);
  let goneShown = false;

  return {
    title: data.title,
    keepFocus: Boolean(section),
    cleanup: () => { clearInterval(timer); stopFollowing(); },
  };
}

// --------------------------------------------------------------- history

export async function historyView(ctx) {
  const data = await get('/api/history', { path: ctx.path });
  const preview = h('div', { class: 'version-preview' });
  const folder = data.path.includes('/') ? data.path.slice(0, data.path.lastIndexOf('/')) : '';
  const crumbs = folder ? folder.split('/').map((name, i, all) => ({ name, path: all.slice(0, i + 1).join('/') })) : [];

  async function restore(version, btn) {
    const ok = await confirmDialog({
      title: 'Restore this earlier version?',
      message: `The page will go back to the version saved ${formatIsoDateTime(version.saved_at)}. The current version is kept in this list first, so you can undo this later.`,
      confirmLabel: 'Restore this version',
      cancelLabel: 'Keep the current page',
      iconName: 'history',
    });
    if (!ok) return;
    try {
      await whileBusy(btn, 'Restoring…', () => post('/api/history/restore', { path: data.path, id: version.id }));
      toast('The earlier version has been restored.');
      ctx.navigate(href.page(data.path));
    } catch (err) {
      toast(errorText(err), { error: true });
    }
  }

  async function show(version, btn) {
    try {
      const v = await whileBusy(btn, 'Opening…', () => get('/api/history/version', { path: data.path, id: version.id }), { guard: false });
      clear(preview).append(
        h('div', { class: 'page-head' },
          h('h2', { id: 'ver-h', tabindex: '-1' }, `Version saved ${formatIsoDateTime(version.saved_at)}`),
          data.can_restore
            ? button('Restore this version', { icon: 'history', kind: 'primary', onClick: (e) => restore(version, e.currentTarget) })
            : null),
        h('details', { class: 'details-panel' },
          h('summary', null, 'Show what is different from the current page'),
          h('div', { class: 'details-body' },
            diffView(v.diff, { oldLabel: 'Only in the current page', newLabel: 'Only in this earlier version' }))),
        h('div', { class: 'rail-box', style: 'margin-top: var(--space-5)' },
          h('div', { class: 'md-body', trustedHtml: v.html })));
      enablePictureZoom(preview.querySelector('.md-body'));
      preview.querySelector('#ver-h').focus();
    } catch (err) {
      toast(errorText(err), { error: true });
    }
  }

  ctx.main.append(
    breadcrumbsNav(crumbs),
    h('div', { class: 'page-head' },
      h('h1', null, `Earlier versions of “${data.title}”`),
      linkButton('Back to the page', href.page(data.path), { icon: 'back' }),
      h('p', { class: 'lede' }, `A copy of the page is kept here every time someone publishes a change. ${keptSentence(ctx.app.state.workspace?.version_cleanup)}`)),
    data.versions.length
      ? h('ul', { class: 'list' }, data.versions.map((v) => h('li', null,
        h('div', { class: 'row-link' },
          icon('clock'),
          h('span', { class: 'row-title' }, formatIsoDateTime(v.saved_at)),
          button('View this version', { icon: 'eye', onClick: (e) => show(v, e.currentTarget) }),
          h('span', { class: 'row-meta' },
            v.by ? h('span', null, `Published by ${v.by}`) : null,
            h('span', null, formatSize(v.size)))))))
      : emptyState({ title: 'No earlier versions yet', text: 'Each time this page is published, the version it replaces is kept here.' }),
    preview);
  return { title: `Earlier versions: ${data.title}` };
}

// -------------------------------------------------------------- new page

const FILTER_ABOVE = 8;

/** "Start from": team templates first, then built-in ones; a filter field
 *  appears when there are many. */
function templatePicker(templates, preselect) {
  const selected = templates.some((t) => t.id === preselect) ? preselect : 'builtin:blank';
  const card = (t) => h('label', { class: 'choice', dataset: { search: `${t.name} ${t.description}`.toLowerCase() } },
    h('input', { type: 'radio', name: 'template', value: t.id, checked: t.id === selected }),
    h('span', null, h('strong', null, t.name), t.description ? h('span', null, t.description) : null));
  const group = (legend, list) => (list.length
    ? h('fieldset', { class: 'choices choices-row' }, h('legend', null, legend), list.map(card))
    : null);
  const team = templates.filter((t) => !t.builtin);
  const groups = h('div', null,
    group('Your team’s templates', team),
    group(team.length ? 'Built in' : 'Start from', templates.filter((t) => t.builtin)));
  if (templates.length <= FILTER_ABOVE) return groups;

  const filter = h('input', { type: 'search', id: 'np-filter', autocomplete: 'off' });
  const none = h('p', { class: 'help', hidden: true }, 'No templates match. Try another word.');
  filter.addEventListener('input', () => {
    const words = filter.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    let shown = 0;
    for (const c of groups.querySelectorAll('.choice')) {
      const match = words.every((w) => c.dataset.search.includes(w)) || c.querySelector('input').checked;
      c.hidden = !match;
      if (match) shown += 1;
    }
    for (const fs of groups.querySelectorAll('fieldset')) {
      fs.hidden = ![...fs.querySelectorAll('.choice')].some((c) => !c.hidden);
    }
    none.hidden = shown > 0;
  });
  return h('div', null,
    h('div', { class: 'field' }, h('label', { for: 'np-filter' }, 'Find a template'), filter),
    none, groups);
}

export async function newPageView(ctx) {
  const [{ folders }, { templates }] = await Promise.all([get('/api/folders'), get('/api/templates')]);
  const wanted = ctx.query.get('folder') || '';
  const titleInput = h('input', {
    type: 'text', id: 'np-title', name: 'title', autocomplete: 'off', 'aria-describedby': 'np-title-help',
  });
  const folderSelect = h('select', { id: 'np-folder', name: 'folder' },
    h('option', { value: '', selected: wanted === '' }, 'Top level (not in a folder)'),
    folders.map((f) => h('option', { value: f, selected: f.toLowerCase() === wanted.toLowerCase() }, f.replaceAll('/', ' › '))));
  const titleError = h('div');
  const submit = button('Create page and start writing', { icon: 'pagePlus', kind: 'primary', type: 'submit', large: true });

  const form = h('form', {
    class: 'panel', style: 'max-width: 46rem',
    onsubmit: async (e) => {
      e.preventDefault();
      clear(titleError);
      titleInput.removeAttribute('aria-invalid');
      const title = titleInput.value.trim();
      if (!title) {
        titleError.append(fieldError('Please give the page a title.'));
        titleInput.setAttribute('aria-invalid', 'true');
        titleInput.focus();
        return;
      }
      const template = form.querySelector('input[name="template"]:checked')?.value || 'builtin:blank';
      try {
        const res = await whileBusy(submit, 'Creating…',
          () => post('/api/page/new', { folder: folderSelect.value, title, template }));
        try { sessionStorage.setItem(`cairn.new.${res.path}`, res.content); } catch { /* server template is used */ }
        ctx.navigate(`${href.edit(res.path)}?new=1`);
      } catch (err) {
        titleError.append(fieldError(errorText(err)));
      }
    },
  },
  h('div', { class: 'field' },
    h('label', { for: 'np-title' }, 'Page title (required)'),
    titleInput,
    h('p', { class: 'help', id: 'np-title-help' }, 'For example: How to connect to the office printer.'),
    titleError),
  h('div', { class: 'field' }, h('label', { for: 'np-folder' }, 'Which folder should it go in?'), folderSelect),
  templatePicker(templates, ctx.query.get('template')),
  h('div', { class: 'actions' }, submit, linkButton('Go back', wanted ? href.folder(wanted) : href.home())));

  ctx.main.append(
    h('div', { class: 'page-head' }, h('h1', null, 'New page'),
      h('p', { class: 'lede' }, 'Give the page a title and choose where it goes. You can change everything later.')),
    form);
  setTimeout(() => titleInput.focus(), 0);
  return { title: 'New page', keepFocus: true };
}

// -------------------------------------------------------------- settings

function radioGroup(name, legend, options, current, onChange) {
  return h('fieldset', { class: 'choices choices-row' },
    h('legend', null, legend),
    options.map((o) => h('label', { class: 'choice' },
      h('input', { type: 'radio', name, value: o.value, checked: o.value === current, onchange: () => onChange(o.value) }),
      h('span', null, h('strong', null, o.label), o.text ? h('span', null, o.text) : null))));
}

async function saveSettings(ctx, body, message) {
  try {
    const state = await post('/api/settings', body);
    ctx.app.state = state;
    applyPrefs(state.config);
    toast(message);
    return true;
  } catch (err) {
    toast(errorText(err), { error: true });
    return false;
  }
}

function workspaceSection(ctx, ws) {
  const wsNameInput = h('input', { type: 'text', id: 's-wsname', value: ws.name });
  const renameForm = h('form', {
    class: 'inline-form', style: 'margin-top: var(--space-5)',
    onsubmit: async (e) => {
      e.preventDefault();
      try {
        await guardAction('Renaming…', () => post('/api/workspace/rename', { display_name: wsNameInput.value }));
        await ctx.refreshState();
        toast('The documentation has been renamed.');
      } catch (err) { toast(errorText(err), { error: true }); }
    },
  },
  h('div', { class: 'field' }, h('label', { for: 's-wsname' }, 'Name of this documentation'), wsNameInput),
  button('Rename', { type: 'submit' }));

  const switchButton = button('Open a different documentation folder', {
    icon: 'folder',
    onClick: async () => {
      const ok = await confirmDialog({
        title: 'Open a different documentation folder?',
        message: 'Any page you are editing will be unlocked. Your unsaved changes are kept on this computer.',
        confirmLabel: 'Choose another folder', cancelLabel: 'Stay here', iconName: 'folder',
      });
      if (!ok) return;
      await post('/api/workspace/close');
      await ctx.refreshState();
      ctx.navigate(href.setup());
    },
  });

  const downloadAll = button('Download everything', {
    icon: 'download',
    onClick: () => openBulkDownload({ scope: 'all', name: ws.name }),
  });

  return h('section', { class: 'settings-section', 'aria-labelledby': 'set-ws' },
    h('h2', { id: 'set-ws' }, 'Documentation folder'),
    h('p', { class: 'label' }, 'Location'),
    h('p', { class: 'path-box' }, ws.root),
    ws.storage?.notes?.length ? banner({ tone: 'warn', text: ws.storage.notes.join(' ') }) : null,
    ws.read_only ? null : renameForm,
    h('div', { class: 'actions', style: 'margin-top: var(--space-5)' }, downloadAll, switchButton),
    h('p', { class: 'help' }, '“Download everything” saves every page, picture, and template as one .zip file (Markdown or PDFs), for a backup or to share.'),
    ws.read_only ? null : cleanupForm(ctx, ws));
}

const dayCount = (n) => (n === 1 ? '1 day' : `${n} days`);

/** How long earlier versions are kept, in one sentence. */
export function keptSentence(vc) {
  if (!vc?.enabled) return 'Every copy is kept.';
  const copies = vc.keep_newest === 1 ? 'the most recent copy is' : `the ${vc.keep_newest} most recent copies are`;
  return `Copies older than ${dayCount(vc.older_than_days)} are removed, but ${copies} always kept.`;
}

/** Whether old earlier versions are removed: one choice for the whole team,
 *  saved in the documentation folder. */
function cleanupForm(ctx, ws) {
  const vc = ws.version_cleanup || { enabled: false, keep_newest: 3, older_than_days: 30 };
  const enabled = h('input', { type: 'checkbox', id: 's-cleanup', checked: vc.enabled });
  const keep = h('input', { type: 'number', id: 's-keep', min: '1', max: '100', value: String(vc.keep_newest) });
  const days = h('input', { type: 'number', id: 's-days', min: '1', max: '3650', value: String(vc.older_than_days) });
  const error = h('div');
  const showEnabled = () => { keep.disabled = !enabled.checked; days.disabled = !enabled.checked; };
  enabled.addEventListener('change', showEnabled);
  showEnabled();
  const submit = button('Save', { type: 'submit' });
  return h('form', {
    class: 'cleanup-form',
    onsubmit: async (e) => {
      e.preventDefault();
      clear(error);
      const body = { enabled: enabled.checked, keep_newest: Number(keep.value), older_than_days: Number(days.value) };
      if (body.enabled && !vc.enabled) {
        const ok = await confirmDialog({
          title: 'Remove old earlier versions?',
          message: `For everyone using this documentation, earlier versions older than ${dayCount(body.older_than_days)} will be removed, keeping each page's ${body.keep_newest === 1 ? 'most recent one' : `${body.keep_newest} most recent`}. This starts now and can't be undone.`,
          confirmLabel: 'Turn on and remove', cancelLabel: 'Keep everything', danger: true, iconName: 'history',
        });
        if (!ok) return;
      }
      try {
        const state = await whileBusy(submit, 'Saving…', () => post('/api/workspace/cleanup', body));
        ctx.app.state = state;
        Object.assign(vc, state.workspace.version_cleanup);
        const n = state.removed_versions || 0;
        toast(!body.enabled ? 'Earlier versions will all be kept.'
          : n ? `Saved. ${n === 1 ? '1 old version was' : `${n} old versions were`} removed.` : 'Saved. There were no old versions to remove.');
      } catch (err) {
        error.append(fieldError(errorText(err)));
      }
    },
  },
  h('h3', null, 'Earlier versions'),
  h('p', { class: 'help' }, 'Each time a page is published, the version it replaces is kept. This choice applies to everyone using this documentation folder.'),
  h('label', { class: 'check', for: 's-cleanup' }, enabled,
    h('span', null, h('strong', null, 'Remove old earlier versions automatically'))),
  h('div', { class: 'inline-form cleanup-numbers' },
    h('div', { class: 'field' }, h('label', { for: 's-keep' }, 'Newest versions to always keep'), keep),
    h('div', { class: 'field' }, h('label', { for: 's-days' }, 'Remove versions older than (days)'), days)),
  error,
  h('div', { class: 'actions' }, submit));
}

function pdfSection(ctx, cfg) {
  return h('section', { class: 'settings-section', 'aria-labelledby': 'set-pdf' },
    h('h2', { id: 'set-pdf' }, 'PDF downloads'),
    cfg.pdf_available
      ? h('p', { class: 'help' }, 'PDFs are made on this computer with Microsoft Edge or Google Chrome. Nothing is sent anywhere.')
      : banner({
        tone: 'warn', title: 'PDFs can’t be made automatically on this computer',
        text: 'No working Microsoft Edge or Google Chrome was found. When you download a page as a PDF, Cairn opens the print window instead; choose “Save as PDF” there.',
      }),
    radioGroup('pdf_paper', 'Paper size', [
      { value: 'a4', label: 'A4', text: 'Used in most countries.' },
      { value: 'letter', label: 'US Letter', text: 'Used in the US, Canada, and the Philippines.' },
    ], cfg.pdf_paper, (v) => saveSettings(ctx, { pdf_paper: v }, 'Paper size saved.')));
}

function quitSection() {
  return h('section', { class: 'settings-section', 'aria-labelledby': 'set-quit' },
    h('h2', { id: 'set-quit' }, 'Close Cairn'),
    h('p', { class: 'help' }, 'Stops Cairn on this computer. Pages you are editing are unlocked, and unsaved changes are kept.'),
    button('Quit Cairn', {
      icon: 'power', kind: 'danger',
      onClick: async () => {
        const ok = await confirmDialog({
          title: 'Quit Cairn?', message: 'You can start it again from the Cairn shortcut whenever you need it.',
          confirmLabel: 'Quit Cairn', cancelLabel: 'Keep Cairn open', danger: true, iconName: 'power',
        });
        if (!ok) return;
        try { await post('/api/quit'); } catch { /* already stopping */ }
        const root = clear(document.getElementById('app'));
        root.append(h('main', { class: 'setup', id: 'main' }, h('h1', null, 'Cairn has stopped'),
          h('p', { class: 'lede' }, 'You can close this browser tab now.')));
      },
    }));
}

export async function settingsView(ctx) {
  const state = await ctx.refreshState();
  const cfg = state.config;

  const nameInput = h('input', { type: 'text', id: 's-name', value: cfg.display_name || '', 'aria-describedby': 's-name-help' });
  // Minutes as choices rather than a bare number box; any other value
  // already in the settings file is kept as one more choice.
  const minuteChoices = (selectId, current, choices) => h('select', { id: selectId },
    [...new Set([...choices, current])].sort((a, b) => a - b).map((n) =>
      h('option', { value: String(n), selected: n === current }, n === 60 ? '1 hour' : n > 60 ? `${n / 60} hours` : `${n} minutes`)));
  const warnInput = minuteChoices('s-warn', cfg.idle_warning_minutes, [5, 10, 15, 20, 30, 45, 60]);
  const releaseInput = minuteChoices('s-release', cfg.idle_release_minutes, [10, 15, 20, 30, 45, 60, 90, 120]);
  const draftsCheck = h('input', { type: 'checkbox', id: 's-drafts', checked: cfg.persistent_drafts });
  const timeError = h('div');
  const saveTimes = async () => {
    clear(timeError);
    const warn = Number(warnInput.value);
    const release = Number(releaseInput.value);
    if (release <= warn) {
      timeError.append(fieldError('The page has to unlock later than the question is asked. Choose a longer unlock time.'));
      return;
    }
    await saveSettings(ctx, { idle_warning_minutes: warn, idle_release_minutes: release }, 'Editing times saved.');
  };
  warnInput.addEventListener('change', saveTimes);
  releaseInput.addEventListener('change', saveTimes);
  draftsCheck.addEventListener('change', () => saveSettings(ctx, { persistent_drafts: draftsCheck.checked },
    draftsCheck.checked ? 'Unsaved changes will be kept on this computer.' : 'Unsaved changes will be kept only while Cairn is open.'));

  const jump = [['set-you', 'Your name'], ['set-look', 'Appearance'], ['set-time', 'Time'],
    ['set-edit', 'Stepping away'], ['set-drafts', 'Unsaved changes'], ['set-pdf', 'PDFs'],
    ['set-ws', 'Documentation folder'], ['set-updates', 'Updates'], ['set-quit', 'Close Cairn']];

  ctx.main.append(
    h('div', { class: 'page-head' }, h('h1', null, 'Settings')),
    h('nav', {
      class: 'settings-jump', 'aria-label': 'Settings sections',
      // Addresses in Cairn use the # part, so jump by scrolling instead.
      onclick: (e) => {
        const a = e.target.closest('a');
        if (!a) return;
        e.preventDefault();
        scrollToSection(a.dataset.target);
      },
    },
    h('ul', null, jump.map(([id, label]) =>
      h('li', null, h('a', { href: href.settings(), dataset: { target: id } }, label))))),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-you' },
      h('h2', { id: 'set-you' }, 'Your name'),
      h('form', {
        class: 'inline-form',
        onsubmit: async (e) => {
          e.preventDefault();
          await saveSettings(ctx, { display_name: nameInput.value }, 'Your name has been saved.');
        },
      },
      h('div', { class: 'field' },
        h('label', { for: 's-name' }, 'Name shown to others while you edit'),
        nameInput,
        h('p', { class: 'help', id: 's-name-help' }, `Leave empty to use your Windows user name (${state.user.os_user}).`)),
      button('Save name', { type: 'submit', kind: 'primary' }))),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-look' },
      h('h2', { id: 'set-look' }, 'Appearance'),
      h('p', { class: 'help' }, 'Changes apply straight away.'),
      radioGroup('appearance', 'Colors', [
        { value: 'light', label: 'Light', text: 'Dark text on a light background.' },
        { value: 'dark', label: 'Dark', text: 'Light text on a dark background.' },
        { value: 'contrast', label: 'High contrast', text: 'Black and white with strong outlines.' },
      ], cfg.appearance, (v) => saveSettings(ctx, { appearance: v }, 'Colors changed.')),
      radioGroup('text_size', 'Text size', [
        { value: 'normal', label: 'Normal', text: 'The standard size.' },
        { value: 'large', label: 'Large', text: 'A little bigger.' },
        { value: 'larger', label: 'Larger', text: 'Much bigger.' },
      ], cfg.text_size, (v) => saveSettings(ctx, { text_size: v }, 'Text size changed.')),
      radioGroup('toolbar_labels', 'Editor toolbar', [
        { value: 'icons', label: 'Icons only', text: 'Point at a button, or move to it with Tab, to see its name.' },
        { value: 'words', label: 'Icons and words', text: 'Every button shows its name. Takes more room.' },
      ], cfg.toolbar_labels ? 'words' : 'icons',
      (v) => saveSettings(ctx, { toolbar_labels: v === 'words' }, 'Toolbar changed.'))),

    timezoneSection(cfg, (body, message) => saveSettings(ctx, body, message)),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-edit' },
      h('h2', { id: 'set-edit' }, 'When you step away while editing'),
      h('p', { class: 'help' }, 'Only one person can edit a page at a time. If you stop typing for a while, Cairn asks whether you are still there, and later unlocks the page so others can edit it. Your text is always kept.'),
      h('div', { class: 'field' }, h('label', { for: 's-warn' }, 'Ask “Are you still editing?” after'), warnInput),
      h('div', { class: 'field' }, h('label', { for: 's-release' }, 'Unlock the page after'), releaseInput),
      timeError),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-drafts' },
      h('h2', { id: 'set-drafts' }, 'Unsaved changes'),
      h('label', { class: 'check', for: 's-drafts' }, draftsCheck,
        h('span', null, h('strong', null, 'Keep my unsaved changes on this computer'),
          h('span', { class: 'help', style: 'display:block' }, 'Your changes are saved privately on this computer every few seconds, so they survive closing Cairn or a crash. If this is off, changes are kept only while Cairn is open.')))),

    pdfSection(ctx, cfg),
    state.workspace ? workspaceSection(ctx, state.workspace) : null,
    updatesSection(cfg, { radioGroup, save: (body, message) => saveSettings(ctx, body, message) }),
    quitSection());
  return { title: 'Settings' };
}
