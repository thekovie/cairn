// Renaming, moving, and deleting pages and folders, and the Recently deleted
// screen. Every change keeps links between pages working, and anything
// deleted can be brought back.

import {
  get, post, h, icon, button, emptyState, href, toast, confirmDialog, formDialog, openDialog,
  whileBusy, errorText, relativeTime, announce,
} from './core.js';

const TOP_LEVEL = '';
const readable = (path) => (path ? path.replaceAll('/', ' › ') : 'Top level');
const parentOf = (path) => (path.includes('/') ? path.slice(0, path.lastIndexOf('/')) : '');
const navChanged = () => window.dispatchEvent(new Event('cairn:nav-changed'));

/** "Where should it go?" choices: the top level and every folder. */
async function folderChoices(exclude) {
  const { folders } = await get('/api/folders');
  const all = [TOP_LEVEL, ...folders].filter((f) => !exclude(f));
  return all.map((f) => ({ value: f, label: f ? readable(f) : 'Top level (not in a folder)' }));
}

/** After a rename or move: say how many links were updated, and list any
 *  page that couldn't be (usually because someone is editing it). */
async function reportLinks(res, done) {
  const n = res.links_updated;
  const also = n === 0 ? '' : n === 1 ? ' 1 link in another page was updated.' : ` ${n} links in other pages were updated.`;
  toast(`${done}${also}`);
  const skipped = res.links_skipped || [];
  if (!skipped.length) return;
  await openDialog({
    title: skipped.length === 1 ? '1 page still links to the old place' : `${skipped.length} pages still link to the old place`,
    iconName: 'link',
    tone: 'warn',
    body: [
      h('p', null, 'These pages couldn’t be updated. Their links will show as missing until someone changes them.'),
      h('ul', { class: 'plain-list' }, skipped.map((s) =>
        h('li', null, h('a', { href: href.page(s.path) }, s.title), h('span', { class: 'help' }, ` ${s.reason}`)))),
    ],
    actions: [{ label: 'Close', value: 'close', kind: 'primary', autofocus: true }],
  });
}

async function run(btn, busyLabel, fn) {
  try {
    return await whileBusy(btn, busyLabel, fn);
  } catch (err) {
    toast(errorText(err), { error: true });
    return null;
  }
}

// ------------------------------------------------------------------ pages

async function renamePage(ctx, data, btn) {
  const values = await formDialog({
    title: 'Rename this page',
    intro: 'Links to this page from other pages are updated for you.',
    fields: [{ name: 'title', label: 'Page title', value: data.title, required: true }],
    submitLabel: 'Rename page',
  });
  if (!values || values.title.trim() === data.title) return;
  const res = await run(btn, 'Renaming…', () => post('/api/page/rename', { path: data.path, title: values.title.trim() }));
  if (!res) return;
  navChanged();
  await reportLinks(res, 'The page has been renamed.');
  ctx.navigate(href.page(res.path));
}

async function movePage(ctx, data, btn) {
  let options;
  try {
    options = await folderChoices((f) => f.toLowerCase() === data.folder.toLowerCase());
  } catch (err) {
    toast(errorText(err), { error: true });
    return;
  }
  if (!options.length) {
    toast('There is no other folder to move this page to. Create a folder first.', { error: true });
    return;
  }
  const values = await formDialog({
    title: 'Move this page',
    iconName: 'folder',
    intro: `It is now in: ${readable(data.folder)}. Its pictures and earlier versions move with it, and links to it are updated.`,
    fields: [{ name: 'folder', label: 'Move it to', type: 'select', options, value: options[0].value }],
    submitLabel: 'Move page',
  });
  if (!values) return;
  const res = await run(btn, 'Moving…', () => post('/api/page/move', { path: data.path, folder: values.folder }));
  if (!res) return;
  navChanged();
  await reportLinks(res, `The page has been moved to ${readable(values.folder)}.`);
  ctx.navigate(href.page(res.path));
}

async function deletePage(ctx, data, btn) {
  const ok = await confirmDialog({
    title: `Delete “${data.title}”?`,
    message: 'The page and its pictures go to Recently deleted, where anyone can bring them back. While it is deleted, links to it from other pages show as missing.',
    confirmLabel: 'Delete page',
    cancelLabel: 'Keep the page',
    danger: true,
    iconName: 'trash',
  });
  if (!ok) return;
  const res = await run(btn, 'Deleting…', () => post('/api/page/delete', { path: data.path, hash: data.hash }));
  if (!res) return;
  navChanged();
  toast(`“${data.title}” was deleted. You can bring it back from Recently deleted.`);
  ctx.navigate(href.folder(data.folder));
}

/** "More actions for this page": rename, move, and delete, out of the way
 *  of reading until someone asks for them. */
export function pageMoreActions(ctx, data) {
  if (data.cannot_edit_reason || (data.lock && !data.lock.is_mine)) return null;
  const body = data.editing_here
    ? h('p', { class: 'help' }, 'Close the editor first to rename, move, or delete this page.')
    : h('div', { class: 'more-actions-list' },
      button('Rename this page', { icon: 'edit', onClick: (e) => renamePage(ctx, data, e.currentTarget) }),
      button('Move to another folder', { icon: 'folder', onClick: (e) => movePage(ctx, data, e.currentTarget) }),
      button('Delete this page', { icon: 'trash', kind: 'danger', onClick: (e) => deletePage(ctx, data, e.currentTarget) }));
  return h('details', { class: 'more-actions' },
    h('summary', { class: 'btn' }, icon('settings'), h('span', null, 'More actions for this page')),
    body);
}

// ---------------------------------------------------------------- folders

async function renameFolder(ctx, data, btn) {
  const values = await formDialog({
    title: 'Rename this folder',
    iconName: 'folder',
    intro: 'Everything inside stays in the folder, and links to its pages are updated for you.',
    fields: [{ name: 'name', label: 'Folder name', value: data.name, required: true }],
    submitLabel: 'Rename folder',
  });
  if (!values || values.name.trim() === data.name) return;
  const res = await run(btn, 'Renaming…', () => post('/api/folder/rename', { path: data.path, name: values.name.trim() }));
  if (!res) return;
  navChanged();
  await reportLinks(res, 'The folder has been renamed.');
  ctx.navigate(href.folder(res.path));
}

async function moveFolder(ctx, data, btn) {
  const self = data.path.toLowerCase();
  const here = parentOf(data.path).toLowerCase();
  let options;
  try {
    options = await folderChoices((f) => {
      const lower = f.toLowerCase();
      return lower === self || lower.startsWith(`${self}/`) || lower === here;
    });
  } catch (err) {
    toast(errorText(err), { error: true });
    return;
  }
  if (!options.length) {
    toast('There is nowhere else to move this folder. Create another folder first.', { error: true });
    return;
  }
  const values = await formDialog({
    title: 'Move this folder',
    iconName: 'folder',
    intro: `It is now in: ${readable(parentOf(data.path))}. Everything inside moves with it, and links to its pages are updated.`,
    fields: [{ name: 'parent', label: 'Move it into', type: 'select', options, value: options[0].value }],
    submitLabel: 'Move folder',
  });
  if (!values) return;
  const res = await run(btn, 'Moving…', () => post('/api/folder/move', { path: data.path, parent: values.parent }));
  if (!res) return;
  navChanged();
  await reportLinks(res, `The folder has been moved to ${readable(values.parent)}.`);
  ctx.navigate(href.folder(res.path));
}

async function deleteFolder(ctx, data, pageCount, btn) {
  const what = pageCount === 0 ? 'The folder goes' : pageCount === 1 ? 'The folder and its 1 page go' : `The folder and all ${pageCount} pages in it go`;
  const ok = await confirmDialog({
    title: `Delete the folder “${data.name}”?`,
    message: `${what} to Recently deleted, where anyone can bring them back.`,
    confirmLabel: 'Delete folder',
    cancelLabel: 'Keep the folder',
    danger: true,
    iconName: 'trash',
  });
  if (!ok) return;
  const res = await run(btn, 'Deleting…', () => post('/api/folder/delete', { path: data.path }));
  if (!res) return;
  navChanged();
  toast(`The folder “${data.name}” was deleted. You can bring it back from Recently deleted.`);
  ctx.navigate(href.folder(parentOf(data.path)));
}

/** "Organize this folder" at the bottom of a folder. */
export function folderOrganizeSection(ctx, data, pageCount) {
  if (!data.can_write || !data.path) return null;
  return h('section', { class: 'section', 'aria-labelledby': 'organize-h' },
    h('h2', { id: 'organize-h' }, 'Organize this folder'),
    h('div', { class: 'actions' },
      button('Rename folder', { icon: 'edit', onClick: (e) => renameFolder(ctx, data, e.currentTarget) }),
      button('Move folder', { icon: 'folder', onClick: (e) => moveFolder(ctx, data, e.currentTarget) }),
      button('Delete folder', { icon: 'trash', kind: 'danger', onClick: (e) => deleteFolder(ctx, data, pageCount, e.currentTarget) })));
}

// ------------------------------------------------------- recently deleted

function deletedRow(ctx, item, canRestore) {
  const isFolder = item.kind === 'folder';
  const restore = async (btn) => {
    const res = await run(btn, 'Restoring…', () => post('/api/trash/restore', { id: item.id }));
    if (!res) return;
    navChanged();
    toast(`“${item.title}” is back where it was.`);
    ctx.navigate(isFolder ? href.folder(res.item.path) : href.page(res.item.path));
  };
  const count = item.page_count === 1 ? '1 page' : `${item.page_count} pages`;
  return h('li', null, h('div', { class: 'row-link' },
    icon(isFolder ? 'folder' : 'page'),
    h('span', { class: 'row-title' }, isFolder ? `Folder: ${item.title}` : item.title),
    canRestore ? button('Restore', { icon: 'history', onClick: (e) => restore(e.currentTarget) }) : h('span'),
    h('span', { class: 'row-meta' },
      h('span', null, `Was in: ${readable(parentOf(item.path))}`),
      isFolder ? h('span', null, count) : null,
      h('span', null, `Deleted by ${item.deleted_by} ${relativeTime(item.deleted_at)}`))));
}

export async function deletedView(ctx) {
  const data = await get('/api/trash');
  const n = data.items.length;
  ctx.main.append(
    h('div', { class: 'page-head' },
      h('h1', null, 'Recently deleted'),
      h('p', { class: 'lede' }, 'Deleted pages and folders are kept here, with their pictures and earlier versions. Restore puts one back where it was.')),
    n
      ? h('ul', { class: 'list' }, data.items.map((item) => deletedRow(ctx, item, data.can_restore)))
      : emptyState({ title: 'Nothing has been deleted', text: 'When a page or folder is deleted, it appears here so it can be brought back.' }));
  announce(n === 0 ? 'Nothing has been deleted' : n === 1 ? '1 deleted item' : `${n} deleted items`);
  return { title: 'Recently deleted' };
}
