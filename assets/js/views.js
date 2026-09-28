// Reading and navigation screens: home, folders, search, articles, earlier
// versions, new page, and settings.

import {
  get, post, h, clear, icon, button, linkButton, banner, emptyState, breadcrumbsNav, statusChip,
  relativeTime, formatDay, formatTime, formatDateTime, formatIsoDateTime, formatSize, href, toast,
  confirmDialog, formDialog, whileBusy, errorText, diffView, fieldError, announce, applyPrefs,
  appendChildren, isToday,
} from './core.js';
import { openPageDownload, openBulkDownload } from './export.js';
import { timezoneSection } from './timezone.js';

// ---------------------------------------------------------------- pieces

function pageRow(page, { showFolder = false } = {}) {
  const folder = page.path.includes('/') ? page.path.slice(0, page.path.lastIndexOf('/')) : '';
  return h('li', null, h('a', { class: 'row-link', href: href.page(page.path) },
    icon('page'),
    h('span', { class: 'row-title' }, page.title),
    h('span', { class: 'row-side' }, page.modified ? `Changed ${relativeTime(page.modified)}` : ''),
    h('span', { class: 'row-meta' },
      statusChip(page.status),
      page.owner ? h('span', null, `Owner: ${page.owner}`) : null,
      page.last_reviewed ? h('span', null, `Reviewed ${formatDay(page.last_reviewed)}`) : null,
      showFolder && folder ? h('span', null, icon('folder'), ' ', folder.replaceAll('/', ' › ')) : null)));
}

function folderCard(folder) {
  const n = folder.page_count;
  return h('a', { class: 'folder-card', href: href.folder(folder.path) },
    icon('folder'),
    h('span', null, h('strong', null, folder.name), h('span', null, n === 1 ? '1 page' : `${n} pages`)));
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
    const res = await post('/api/folder/create', { parent: parent || '', name: values.name.trim() });
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

export async function homeView(ctx) {
  const data = await get('/api/home');
  const readOnly = Boolean(ctx.app.state.workspace?.read_only);
  const searchInput = h('input', { type: 'search', id: 'home-search', name: 'q', autocomplete: 'off' });
  const count = data.total_pages === 1 ? '1 page' : `${data.total_pages} pages`;

  ctx.main.append(
    h('div', { class: 'page-head' },
      h('h1', null, data.name),
      h('p', { class: 'lede' }, `${count} of shared documentation. Choose a folder, or search for what you need.`)),
    h('form', {
      role: 'search',
      onsubmit: (e) => { e.preventDefault(); ctx.navigate(href.search(searchInput.value.trim())); },
    },
    h('label', { for: 'home-search' }, 'Search all pages'),
    h('div', { class: 'hero-search' }, searchInput,
      button('Search', { icon: 'search', type: 'submit', kind: 'primary', large: true }))),

    h('section', { class: 'section', 'aria-labelledby': 'folders-h' },
      h('div', { class: 'page-head', style: 'margin-bottom: var(--space-4)' },
        h('h2', { id: 'folders-h' }, 'Folders'),
        readOnly ? null : button('New folder', { icon: 'folderPlus', onClick: () => createFolder(ctx, '') })),
      data.categories.length
        ? h('div', { class: 'folder-grid' }, data.categories.map(folderCard))
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
  const data = await get('/api/folder', { path: ctx.path });
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
        h('div', { class: 'folder-grid' }, data.folders.map(folderCard)))
      : null,
    h('section', { class: 'section', 'aria-labelledby': 'pages-h' },
      h('h2', { id: 'pages-h' }, 'Pages'),
      data.pages.length
        ? h('ul', { class: 'list' }, data.pages.map((p) => pageRow(p)))
        : emptyState({
          title: 'This folder has no pages yet',
          text: data.can_write ? 'Create the first page for this folder.' : 'Pages added here will appear in this list.',
          actions: data.can_write ? [newHere()] : [],
        })));
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
    results.append(h('p', { class: 'help' }, `Nothing matches “${q}”. Try fewer words, or different words with the same meaning.`));
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

function editArea(ctx, data) {
  const lock = data.lock;
  const reasonId = 'edit-reason';
  const versionsLink = [
    linkButton('Earlier versions', href.history(data.path), { icon: 'history' }),
    button('Download', { icon: 'download', onClick: () => openPageDownload(ctx, data) }),
  ];
  if (lock && !lock.is_mine) {
    if (lock.reclaimable_by_me) {
      return h('div', { class: 'actions' },
        button('Continue where you left off', {
          icon: 'edit', kind: 'primary', large: true,
          onClick: async (e) => {
            try {
              await whileBusy(e.currentTarget, 'Opening…', () => post('/api/edit/reclaim', { path: data.path }));
              ctx.navigate(href.edit(data.path));
            } catch (err) { toast(errorText(err), { error: true }); }
          },
        }),
        versionsLink,
        h('p', { class: 'help', style: 'flex-basis: 100%' },
          'You left this page open for editing on this computer earlier. You can pick up where you stopped.'));
    }
    return h('div', { class: 'actions' },
      button('Edit this page', { icon: 'lock', disabled: true, 'aria-describedby': reasonId }),
      versionsLink,
      h('p', { id: reasonId, class: 'help', style: 'flex-basis: 100%' },
        `${lockSentence(lock)}. You can keep reading. Editing opens up when they finish.`));
  }
  if (data.cannot_edit_reason) {
    return h('div', { class: 'actions' },
      button('Edit this page', { icon: 'lock', disabled: true, 'aria-describedby': reasonId }),
      versionsLink,
      h('p', { id: reasonId, class: 'help', style: 'flex-basis: 100%' }, data.cannot_edit_reason));
  }
  const label = data.editing_here || data.has_draft ? 'Continue editing' : 'Edit this page';
  return h('div', { class: 'actions' },
    linkButton(label, href.edit(data.path), { icon: 'edit', kind: 'primary', large: true }), versionsLink);
}

function lockBox(data) {
  const lock = data.lock;
  let body;
  if (!lock) {
    body = h('p', { class: 'lock-state is-free' }, icon('unlock'), h('span', null, 'Nobody is editing this page right now.'));
  } else if (lock.is_mine) {
    body = h('p', { class: 'lock-state is-locked' }, icon('edit'), h('span', null, 'You have this page open for editing.'));
  } else if (lock.possibly_abandoned) {
    body = h('div', null,
      h('p', { class: 'lock-state is-locked' }, icon('alert'),
        h('span', null, `${lockSentence(lock)}, but they haven't been active for a while.`)),
      h('p', { class: 'help' }, 'The page may have been left open by accident. A maintainer can release it by following “Abandoned edit locks” in the Troubleshooting guide.'));
  } else {
    body = h('p', { class: 'lock-state is-locked' }, icon('lock'), h('span', null, `${lockSentence(lock)}.`));
  }
  return h('section', { class: 'rail-box', 'aria-labelledby': 'lock-h' }, h('h2', { id: 'lock-h' }, 'Editing'), body);
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

export async function pageView(ctx) {
  let data = await get('/api/page', { path: ctx.path });
  const notices = h('div');
  const rail = h('aside', { class: 'article-rail', 'aria-label': 'About this page' });
  const head = h('div', { class: 'page-head' });

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
        text: 'They point to pages or pictures that don’t exist. They are marked with a dashed underline and the word “missing”.',
      }));
    }
    head.append(h('h1', null, data.title), editArea(ctx, data));

    const m = data.meta;
    appendChildren(rail, [
      h('section', { class: 'rail-box', 'aria-labelledby': 'about-h' },
        h('h2', { id: 'about-h' }, 'About this page'),
        h('dl', { class: 'meta-list' },
          h('div', null, h('dt', null, 'Owner'), h('dd', null, m.owner || 'Not set')),
          h('div', null, h('dt', null, 'Status'), h('dd', null, statusChip(m.status) || 'Not set')),
          h('div', null, h('dt', null, 'Last reviewed'), h('dd', null, m.last_reviewed ? formatDay(m.last_reviewed) : 'Not set')),
          h('div', null, h('dt', null, 'Last changed'), h('dd', null, data.modified ? relativeTime(data.modified) : '')),
          m.tags.length ? h('div', null, h('dt', null, 'Tags'), h('dd', null, m.tags.join(', '))) : null)),
      lockBox(data),
      data.toc.filter((t) => t.level > 1).length > 1
        ? h('nav', { class: 'rail-box', 'aria-labelledby': 'toc-h' },
          h('h2', { id: 'toc-h' }, 'On this page'),
          h('ul', { class: 'toc' }, data.toc.filter((t) => t.level > 1).map((t) =>
            h('li', { class: `lvl-${t.level}` }, h('a', { href: `#${t.id}` }, t.text)))))
        : null]);
  }

  const article = h('article', { class: 'md-body', trustedHtml: data.html });
  // The first heading is already shown as the page title above.
  const firstH1 = article.firstElementChild;
  if (firstH1?.tagName === 'H1' && firstH1.textContent.trim() === data.title) firstH1.remove();
  wireArticleLinks(article, data.path);
  rail.addEventListener('click', (e) => {
    const a = e.target.closest('.toc a');
    if (!a) return;
    e.preventDefault();
    const id = a.getAttribute('href').slice(1);
    history.replaceState(null, '', `${href.page(data.path)}?section=${encodeURIComponent(id)}`);
    scrollToSection(id);
  });
  renderAll();
  ctx.main.append(breadcrumbsNav(data.breadcrumbs), notices, head,
    h('div', { class: 'article-layout' }, article, rail));
  const section = ctx.query.get('section');
  if (section) setTimeout(() => scrollToSection(section), 0);

  // Keep the lock state fresh and notice when someone publishes a change.
  const shownHash = data.hash;
  let updateShown = false;
  const timer = setInterval(async () => {
    try {
      const fresh = await get('/api/page', { path: ctx.path });
      const lockChanged = JSON.stringify(fresh.lock) !== JSON.stringify(data.lock);
      data = { ...fresh, title: data.title, meta: data.meta, toc: data.toc, broken_links: data.broken_links };
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
    } catch { /* the next tick will try again */ }
  }, 30000);

  return { title: data.title, keepFocus: Boolean(section), cleanup: () => clearInterval(timer) };
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
      const v = await whileBusy(btn, 'Opening…', () => get('/api/history/version', { path: data.path, id: version.id }));
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
      h('p', { class: 'lede' }, 'A copy of the page is kept here every time someone publishes a change.')),
    data.versions.length
      ? h('ul', { class: 'list' }, data.versions.map((v) => h('li', null,
        h('div', { class: 'row-link' },
          icon('clock'),
          h('span', { class: 'row-title' }, formatIsoDateTime(v.saved_at)),
          button('View this version', { icon: 'eye', onClick: (e) => show(v, e.currentTarget) }),
          h('span', { class: 'row-meta' }, formatSize(v.size))))))
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
        await post('/api/workspace/rename', { display_name: wsNameInput.value });
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
    h('p', { class: 'help' }, '“Download everything” saves every page, picture, and template as one .zip file (Markdown or PDFs), for a backup or to share.'));
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
  const warnInput = h('input', { type: 'number', id: 's-warn', min: '1', max: '1440', value: String(cfg.idle_warning_minutes) });
  const releaseInput = h('input', { type: 'number', id: 's-release', min: '2', max: '1440', value: String(cfg.idle_release_minutes) });
  const draftsCheck = h('input', { type: 'checkbox', id: 's-drafts', checked: cfg.persistent_drafts });
  const timeError = h('div');

  ctx.main.append(
    h('div', { class: 'page-head' }, h('h1', null, 'Settings')),

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
      ], cfg.text_size, (v) => saveSettings(ctx, { text_size: v }, 'Text size changed.'))),

    timezoneSection(cfg, (body, message) => saveSettings(ctx, body, message)),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-edit' },
      h('h2', { id: 'set-edit' }, 'When you step away while editing'),
      h('p', { class: 'help' }, 'Only one person can edit a page at a time. If you stop typing for a while, Cairn asks whether you are still there, and later unlocks the page so others can edit it. Your text is always kept.'),
      h('form', {
        onsubmit: async (e) => {
          e.preventDefault();
          clear(timeError);
          const warn = Number(warnInput.value);
          const release = Number(releaseInput.value);
          if (!Number.isInteger(warn) || !Number.isInteger(release) || warn < 1 || release <= warn) {
            timeError.append(fieldError('Use whole minutes, and make the unlock time later than the question.'));
            return;
          }
          await saveSettings(ctx, { idle_warning_minutes: warn, idle_release_minutes: release }, 'Editing times saved.');
        },
      },
      h('div', { class: 'field' }, h('label', { for: 's-warn' }, 'Ask “Are you still editing?” after this many minutes'), warnInput),
      h('div', { class: 'field' }, h('label', { for: 's-release' }, 'Unlock the page after this many minutes'), releaseInput),
      timeError,
      button('Save editing times', { type: 'submit', kind: 'primary' }))),

    h('section', { class: 'settings-section', 'aria-labelledby': 'set-drafts' },
      h('h2', { id: 'set-drafts' }, 'Unsaved changes'),
      h('label', { class: 'check', for: 's-drafts' }, draftsCheck,
        h('span', null, h('strong', null, 'Keep my unsaved changes on this computer'),
          h('span', { class: 'help', style: 'display:block' }, 'Your changes are saved privately on this computer every few seconds, so they survive closing Cairn or a crash. If this is off, changes are kept only while Cairn is open.'))),
      h('div', { class: 'actions', style: 'margin-top: var(--space-4)' },
        button('Save', { kind: 'primary', onClick: () => saveSettings(ctx, { persistent_drafts: draftsCheck.checked }, 'Saved.') }))),

    pdfSection(ctx, cfg),
    state.workspace ? workspaceSection(ctx, state.workspace) : null,
    quitSection());
  return { title: 'Settings' };
}
