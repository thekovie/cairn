// The Templates page (#/templates).
//
// Team templates live in this documentation folder's `_templates` folder and
// are edited with the normal editor. Built-in templates can be used as they
// are or copied and customized. Deleting asks first, and every deleted
// template can be brought back from "Deleted templates".

import {
  get, post, h, clear, icon, button, linkButton, emptyState, href, toast, openDialog,
  confirmDialog, whileBusy, errorText, relativeTime, fieldError,
} from './core.js';

const newPageFrom = (id) => `#/new?${new URLSearchParams({ template: id })}`;

let seq = 0;

/** Ask for a name, description, and starting point; then open the editor. */
async function newTemplateDialog(ctx, templates, preselect) {
  const uid = ++seq;
  const nameInput = h('input', { type: 'text', id: `nt-name-${uid}`, maxlength: '80', autocomplete: 'off', 'aria-describedby': `nt-name-help-${uid}` });
  const descInput = h('textarea', { id: `nt-desc-${uid}`, rows: '2', maxlength: '300', 'aria-describedby': `nt-desc-help-${uid}` });
  const nameError = h('div');
  const formError = h('div');
  const sources = [{ id: '', name: 'Blank', description: 'Just a title. Add your own headings.' }, ...templates];
  const startFrom = h('fieldset', { class: 'choices' },
    h('legend', null, 'Start from'),
    sources.map((t) => h('label', { class: 'choice' },
      h('input', { type: 'radio', name: `nt-from-${uid}`, value: t.id, checked: t.id === (preselect || '') }),
      h('span', null, h('strong', null, t.name), t.description ? h('span', null, t.description) : null))));

  let dialog = null;
  let working = false;
  await openDialog({
    title: 'New template', iconName: 'template', tone: 'info', wide: true,
    body: [
      h('p', null, 'A template gives new pages a ready-made structure. Everyone using this documentation folder can use it.'),
      h('div', { class: 'field' },
        h('label', { for: `nt-name-${uid}` }, 'Template name (required)'), nameInput,
        h('p', { class: 'help', id: `nt-name-help-${uid}` }, 'For example: Meeting notes, or Incident report.'),
        nameError),
      h('div', { class: 'field' },
        h('label', { for: `nt-desc-${uid}` }, 'Description'), descInput,
        h('p', { class: 'help', id: `nt-desc-help-${uid}` }, 'One sentence about when to use it. People see this when they choose a template.')),
      startFrom,
      formError,
    ],
    actions: [
      { label: 'Go back', value: 'cancel' },
      { label: 'Create template', value: 'ok', kind: 'primary', submit: true, icon: 'template' },
    ],
    onOpen: (dlg) => { dialog = dlg; setTimeout(() => nameInput.focus(), 0); },
    onSubmit: (value) => {
      if (value !== 'ok') return true;
      clear(nameError);
      clear(formError);
      nameInput.removeAttribute('aria-invalid');
      if (!nameInput.value.trim()) {
        nameError.append(fieldError('Please give the template a name.'));
        nameInput.setAttribute('aria-invalid', 'true');
        nameInput.focus();
        return false;
      }
      if (working) return false;
      working = true;
      const btn = dialog.querySelector('button[value="ok"]');
      const from = dialog.querySelector(`input[name="nt-from-${uid}"]:checked`)?.value || null;
      whileBusy(btn, 'Creating…', () => post('/api/templates/new',
        { name: nameInput.value.trim(), description: descInput.value.trim(), from }))
        .then((res) => {
          try { sessionStorage.setItem(`cairn.new.${res.path}`, res.content); } catch { /* the editor asks the server */ }
          dialog.close('done');
          ctx.navigate(`${href.edit(res.path)}?new=1`);
        })
        .catch((err) => { formError.append(fieldError(errorText(err))); })
        .finally(() => { working = false; });
      return false;
    },
  });
}

async function deleteTemplate(ctx, t, btn) {
  const ok = await confirmDialog({
    title: `Delete the template “${t.name}”?`,
    message: 'Pages already made from it are not affected. You can bring it back from “Deleted templates” on this page.',
    confirmLabel: 'Delete template', cancelLabel: 'Keep it', danger: true, iconName: 'trash',
  });
  if (!ok) return;
  try {
    await whileBusy(btn, 'Deleting…', async () => {
      const page = await get('/api/page', { path: t.id });
      await post('/api/templates/delete', { path: t.id, base_hash: page.hash });
    });
    toast(`The template “${t.name}” was deleted. You can bring it back from Deleted templates.`);
    window.dispatchEvent(new HashChangeEvent('hashchange'));
  } catch (err) {
    toast(errorText(err), { error: true });
  }
}

function templateRow(ctx, t, canWrite, templates) {
  const actions = [linkButton('Use for a new page', newPageFrom(t.id), { icon: 'pagePlus' })];
  if (canWrite && t.builtin) {
    actions.push(button('Make a team copy', { icon: 'copy', onClick: () => newTemplateDialog(ctx, templates, t.id) }));
  }
  if (canWrite && !t.builtin) {
    actions.push(linkButton('Edit', href.edit(t.id), { icon: 'edit' }));
    const del = button('Delete', {
      icon: 'trash', kind: 'danger', onClick: (e) => deleteTemplate(ctx, t, e.currentTarget),
    });
    del.classList.add('push-right'); // away from the safe actions
    actions.push(del);
  }
  return h('li', { class: 'tpl-row' },
    icon('template'),
    h('div', { class: 'tpl-text' },
      h('h3', { class: 'row-title' }, t.name),
      t.description ? h('p', null, t.description) : null,
      t.modified ? h('p', { class: 'row-meta' }, `Changed ${relativeTime(t.modified)}`) : null),
    h('div', { class: 'actions tpl-actions' }, actions));
}

function deletedSection(ctx, deleted, canWrite) {
  if (!deleted.length) return null;
  return h('details', { class: 'details-panel' },
    h('summary', null, `Deleted templates (${deleted.length})`),
    h('div', { class: 'details-body' },
      h('p', { class: 'help' }, 'Bring a deleted template back exactly as it was.'),
      h('ul', { class: 'list' }, deleted.map((d) => h('li', { class: 'tpl-row' },
        icon('history'),
        h('div', { class: 'tpl-text' }, h('h3', { class: 'row-title' }, d.name)),
        canWrite
          ? h('div', { class: 'actions tpl-actions' }, button('Restore', {
            icon: 'history',
            onClick: async (e) => {
              try {
                await whileBusy(e.currentTarget, 'Restoring…',
                  () => post('/api/history/restore', { path: d.path, id: d.latest_version }));
                toast(`The template “${d.name}” is back.`);
                window.dispatchEvent(new HashChangeEvent('hashchange'));
              } catch (err) { toast(errorText(err), { error: true }); }
            },
          }))
          : null)))));
}

export async function templatesView(ctx) {
  const [{ templates, can_write: canWrite }, { deleted }] =
    await Promise.all([get('/api/templates'), get('/api/templates/deleted')]);
  const team = templates.filter((t) => !t.builtin);
  const builtin = templates.filter((t) => t.builtin);
  const newBtn = () => button('New template', {
    icon: 'template', kind: 'primary', onClick: () => newTemplateDialog(ctx, templates, ''),
  });

  ctx.main.append(
    h('div', { class: 'page-head' },
      h('h1', null, 'Templates'),
      canWrite ? newBtn() : null,
      h('p', { class: 'lede' }, 'Templates give new pages a ready-made structure. Your team’s templates belong to this documentation folder.')),

    h('section', { class: 'section', 'aria-labelledby': 'tpl-team-h' },
      h('h2', { id: 'tpl-team-h' }, 'Your team’s templates'),
      team.length
        ? h('ul', { class: 'list' }, team.map((t) => templateRow(ctx, t, canWrite, templates)))
        : emptyState({
          title: 'Your team hasn’t made any templates yet',
          text: canWrite
            ? 'Templates save time for pages you write often, like meeting notes or incident reports. Choose “New template” above, or “Make a team copy” beside a built-in one below.'
            : 'Templates save time for pages you write often, like meeting notes or incident reports.',
        })),

    h('section', { class: 'section', 'aria-labelledby': 'tpl-builtin-h' },
      h('h2', { id: 'tpl-builtin-h' }, 'Built in'),
      h('p', { class: 'help' }, 'These come with Cairn. “Make a team copy” gives your team its own version to change.'),
      h('ul', { class: 'list' }, builtin.map((t) => templateRow(ctx, t, canWrite, templates)))),

    deletedSection(ctx, deleted, canWrite));
  return { title: 'Templates' };
}
