// First run and "open a different folder": choose an existing documentation
// folder or create a new one. Nothing is opened or created without showing
// the folder's name and location first.

import {
  post, h, clear, icon, button, banner, fieldError, whileBusy, errorText, toast, href, IS_MAC, IS_WINDOWS,
} from './core.js';

const EXAMPLE_PATHS = IS_WINDOWS
  ? 'C:\\Documentation or \\\\server\\shared\\Documentation'
  : IS_MAC
    ? '/Users/you/Documents/Documentation, or /Volumes/Shared/Documentation for a shared drive'
    : '/home/you/Documentation, or /mnt/shared/Documentation for a shared drive';
const WINDOW_LIST = IS_WINDOWS ? 'the taskbar at the bottom of the screen' : IS_MAC ? 'the Dock' : 'your list of open windows';

const STEPS = ['Choose', 'Find the folder', 'Confirm'];

function stepper(current) {
  return h('ol', { class: 'setup-steps', 'aria-label': 'Progress' },
    STEPS.map((label, i) => h('li', { 'aria-current': i === current ? 'step' : null },
      `Step ${i + 1}: ${label}`)));
}

/** "Choose folder…" (native window) plus a typed-location fallback. */
function folderChooser({ intro, onChosen }) {
  const pathInput = h('input', {
    type: 'text', id: 'folder-path', autocomplete: 'off', spellcheck: 'false', 'aria-describedby': 'folder-path-help',
  });
  const error = h('div');
  const pick = button('Choose folder…', {
    icon: 'folder', kind: 'primary', large: true,
    onClick: async (e) => {
      clear(error);
      try {
        const res = await whileBusy(e.currentTarget, 'Waiting for you to choose…', () => post('/api/workspace/pick'), { guard: false });
        if (res.path) {
          pathInput.value = res.path;
          onChosen(res.path);
        }
      } catch (err) {
        error.append(fieldError(errorText(err)));
      }
    },
  });
  const typed = h('form', {
    onsubmit: (e) => {
      e.preventDefault();
      clear(error);
      const value = pathInput.value.trim();
      if (!value) {
        error.append(fieldError('Please type or paste the folder’s location first.'));
        pathInput.focus();
        return;
      }
      onChosen(value);
    },
  },
  h('label', { for: 'folder-path' }, 'Or type or paste the folder’s location'),
  h('div', { class: 'inline-form' },
    h('div', { class: 'field' }, pathInput),
    button('Use this location', { type: 'submit' })),
  h('p', { class: 'help', id: 'folder-path-help' }, `For example: ${EXAMPLE_PATHS}`));

  return h('div', { class: 'panel' },
    intro ? h('p', null, intro) : null,
    h('div', null, pick,
      h('p', { class: 'help' }, `A folder window will open. If you don’t see it, it may be behind this browser window: check ${WINDOW_LIST}.`)),
    h('p', { class: 'or-divider' }, 'or'),
    typed,
    error);
}

export async function setupView(ctx) {
  const main = ctx.main;
  const state = ctx.app.state;

  async function openRoot(root, btn) {
    try {
      await whileBusy(btn, 'Opening…', () => post('/api/workspace/open', { root }));
      await ctx.refreshState();
      toast('Documentation folder opened.');
      ctx.navigate(href.home());
    } catch (err) {
      toast(errorText(err), { error: true });
    }
  }

  function showChoose() {
    clear(main);
    const last = state.config.last_workspace;
    main.append(
      stepper(0),
      h('h1', null, 'Welcome to Cairn'),
      h('p', { class: 'lede', style: 'margin-top: var(--space-3)' },
        'Cairn keeps your team’s documentation in a shared folder. Everyone reads and edits the same pages from their own computer.'),
      last && !state.workspace
        ? banner({
          tone: 'warn', title: 'The documentation folder used last time isn’t available',
          text: h('div', null,
            h('p', null, 'It may be on a network drive that is not connected right now:'),
            h('p', { class: 'path-box' }, last)),
          actions: [button('Try it again', { icon: 'refresh', onClick: () => lookUp(last, 'open') })],
        })
        : null,
      h('div', { class: 'big-choices' },
        h('button', { type: 'button', class: 'big-choice', onclick: () => showFind('open') },
          icon('folder'),
          h('strong', null, 'Open a documentation folder we already use'),
          h('span', null, 'Choose this if someone has already set up the shared documentation.')),
        h('button', { type: 'button', class: 'big-choice', onclick: () => showFind('create') },
          icon('folderPlus'),
          h('strong', null, 'Create a new documentation folder'),
          h('span', null, 'Choose this to start new documentation for your team.'))));
    main.querySelector('.big-choice')?.focus();
  }

  function showFind(mode) {
    clear(main);
    const intro = mode === 'open'
      ? 'Choose the shared documentation folder, or any folder inside it. Cairn will find the documentation it belongs to.'
      : 'Choose where the new documentation should live, such as a shared network folder. Cairn will create a new folder inside it for you.';
    main.append(
      stepper(1),
      h('h1', null, mode === 'open' ? 'Find your documentation folder' : 'Where should the documentation live?'),
      h('div', { style: 'margin-top: var(--space-5)' }, folderChooser({ intro, onChosen: (p) => lookUp(p, mode) })),
      h('div', { class: 'actions', style: 'margin-top: var(--space-5)' },
        button('Go back', { icon: 'back', onClick: showChoose })));
    main.querySelector('.btn-primary')?.focus();
  }

  function showFound(result, mode) {
    const openBtn = button('Open this documentation', { icon: 'folder', kind: 'primary', large: true });
    openBtn.addEventListener('click', () => openRoot(result.root, openBtn));
    main.append(
      h('h1', null, mode === 'create' ? 'This folder already has documentation' : 'Documentation found'),
      h('div', { class: 'found-card', style: 'margin-top: var(--space-5)' },
        h('span', { class: 'found-name' }, result.marker.display_name),
        h('span', null, 'Location:'),
        h('span', { class: 'path-box' }, result.root),
        result.read_only_reason ? banner({ tone: 'warn', text: result.read_only_reason }) : null),
      h('div', { class: 'actions', style: 'margin-top: var(--space-5)' }, openBtn,
        button('Choose a different folder', { onClick: () => showFind(mode) })));
    openBtn.focus();
  }

  async function lookUp(path, mode) {
    let result;
    try {
      result = await post('/api/workspace/discover', { path });
    } catch (err) {
      toast(errorText(err), { error: true });
      return;
    }
    clear(main);
    main.append(stepper(2));
    if (result.result === 'found') {
      showFound(result, mode);
      return;
    }
    if (result.result === 'invalid') {
      main.append(h('h1', null, 'This folder can’t be used'),
        banner({
          tone: 'danger', title: 'There is a problem with the documentation marker file',
          text: h('div', null, h('p', null, result.reason), h('p', { class: 'path-box' }, result.path)),
        }),
        h('div', { class: 'actions' }, button('Choose a different folder', { kind: 'primary', onClick: () => showFind(mode) })));
      return;
    }
    if (mode === 'open') {
      main.append(h('h1', null, 'No documentation found there'),
        banner({
          tone: 'warn', title: 'This folder isn’t part of any Cairn documentation',
          text: h('div', null,
            h('p', null, 'Cairn looked in this folder and the folders above it:'),
            h('p', { class: 'path-box' }, result.start)),
        }),
        h('div', { class: 'actions' },
          button('Choose a different folder', { kind: 'primary', onClick: () => showFind('open') }),
          button('Create new documentation here instead', { icon: 'folderPlus', onClick: () => showCreate(result) })));
      main.querySelector('.btn-primary')?.focus();
      return;
    }
    showCreate(result);
  }

  function showCreate(found) {
    clear(main);
    const nameInput = h('input', { type: 'text', id: 'ws-name', value: 'Team Documentation', autocomplete: 'off' });
    const childInput = h('input', { type: 'text', id: 'ws-child', value: 'Documentation', autocomplete: 'off', 'aria-describedby': 'ws-where' });
    const childRadio = h('input', { type: 'radio', name: 'where', value: 'child', checked: true });
    const hereRadio = h('input', { type: 'radio', name: 'where', value: 'here', disabled: !found.is_empty });
    const errors = h('div');
    const preview = h('p', { class: 'path-box', id: 'ws-where' });
    const sep = found.start.includes('\\') ? '\\' : '/';
    const updatePreview = () => {
      preview.textContent = childRadio.checked
        ? `${found.start.replace(/[\\/]$/, '')}${sep}${childInput.value.trim() || '…'}`
        : found.start;
      childInput.disabled = !childRadio.checked;
    };
    childInput.addEventListener('input', updatePreview);
    childRadio.addEventListener('change', updatePreview);
    hereRadio.addEventListener('change', updatePreview);
    updatePreview();

    const submit = button('Create documentation folder', { icon: 'folderPlus', kind: 'primary', type: 'submit', large: true });
    const form = h('form', {
      class: 'panel', style: 'margin-top: var(--space-5)',
      onsubmit: async (e) => {
        e.preventDefault();
        clear(errors);
        const displayName = nameInput.value.trim();
        const child = childInput.value.trim();
        if (!displayName) {
          errors.append(fieldError('Please give the documentation a name.'));
          nameInput.focus();
          return;
        }
        if (childRadio.checked && !child) {
          errors.append(fieldError('Please give the new folder a name.'));
          childInput.focus();
          return;
        }
        try {
          await whileBusy(submit, 'Creating…', () => post('/api/workspace/init', {
            path: found.start,
            mode: childRadio.checked ? 'child' : 'here',
            child_name: child,
            display_name: displayName,
          }));
          await ctx.refreshState();
          toast('Your documentation folder is ready.');
          ctx.navigate(href.home());
        } catch (err) {
          errors.append(fieldError(errorText(err)));
        }
      },
    },
    h('div', { class: 'field' },
      h('label', { for: 'ws-name' }, 'What should this documentation be called?'),
      nameInput,
      h('p', { class: 'help' }, 'This name is shown at the top of every page. You can change it later.')),
    h('fieldset', { class: 'choices' },
      h('legend', null, 'Where exactly?'),
      h('label', { class: 'choice' }, childRadio,
        h('span', null, h('strong', null, 'Create a new folder inside the one I chose (recommended)'),
          h('span', null, 'Nothing already in that folder is touched.'))),
      h('label', { class: 'choice' }, hereRadio,
        h('span', null, h('strong', null, 'Use the folder I chose'),
          h('span', null, found.is_empty
            ? 'The folder is empty, so it can be used directly.'
            : 'Not available: this folder already contains files, and Cairn won’t mix its pages in with them.')))),
    h('div', { class: 'field' },
      h('label', { for: 'ws-child' }, 'Name of the new folder'),
      childInput),
    h('p', { class: 'label' }, 'The documentation will be created here:'),
    preview,
    errors,
    h('div', { class: 'actions' }, submit, button('Choose a different place', { onClick: () => showFind('create') })));

    main.append(stepper(2), h('h1', null, 'Create new documentation'), form);
    nameInput.focus();
  }

  showChoose();
  return { title: 'Set up Cairn', keepFocus: true };
}
