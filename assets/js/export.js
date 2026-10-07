// Downloads: one page as PDF / Markdown with pictures / Markdown, a folder
// or the whole documentation as a zip, and the print view used when PDFs
// can't be made automatically.
//
// Choices are radio cards so nothing downloads by accident; the Download
// button shows "Preparing…" while it works; folder downloads show real
// progress and can be cancelled; errors appear inside the dialog with a way
// forward.

import {
  get, post, h, clear, button, linkButton, banner, href, toast, announce, openDialog,
  errorText, downloadFile, statusChip, formatDay, relativeTime, guardAction, IS_WINDOWS,
} from './core.js';

const POLL_MS = 700;
const ANNOUNCE_EVERY = 5;

function choiceCards(name, options) {
  return h('fieldset', { class: 'choices' },
    h('legend', null, 'Download as'),
    options.map((o, i) => h('label', { class: 'choice' },
      h('input', { type: 'radio', name, value: o.value, checked: i === 0 }),
      h('span', null, h('strong', null, o.label), h('span', null, o.text)))));
}

const chosen = (dlg, name) => dlg.querySelector(`input[name="${name}"]:checked`)?.value;

function setBusy(btn, busy, label) {
  btn.disabled = busy;
  btn.toggleAttribute('aria-busy', busy);
  btn.querySelector('span:last-child').textContent = label;
  const spinner = btn.querySelector('.spinner');
  if (busy && !spinner) btn.prepend(h('span', { class: 'spinner', 'aria-hidden': 'true' }));
  if (!busy && spinner) spinner.remove();
}

function doneMessage(name) {
  return banner({
    tone: 'ok', title: 'Done',
    text: h('p', null, h('strong', null, name), ' is in your Downloads folder.'),
    role: 'status',
  });
}

// ------------------------------------------------------------- one page

/** Download dialog for a single page. `data` is the /api/page response. */
export async function openPageDownload(ctx, data) {
  const status = h('div', { class: 'dl-status' });
  let dialog = null;
  let working = false;

  async function run() {
    const goBtn = dialog.querySelector('button[value="go"]');
    const format = chosen(dialog, 'dl-format');
    clear(status);
    working = true;
    setBusy(goBtn, true, 'Preparing…');
    try {
      const name = await guardAction('Preparing the download…',
        () => downloadFile('/api/export/page', { path: data.path, format }, `${data.title}.${format}`));
      status.append(doneMessage(name));
      toast(`${name} is in your Downloads folder.`);
    } catch (err) {
      status.append(err.code === 'pdf_unavailable'
        ? banner({
          tone: 'warn', title: 'A PDF can’t be made automatically on this computer',
          text: h('p', null, errorText(err), ' You can still make one: open the print window and choose ',
            h('strong', null, 'Save as PDF'), ' as the printer.'),
          actions: [button('Open the print window instead', {
            icon: 'page', kind: 'primary',
            onClick: () => { dialog.close('print'); ctx.navigate(href.print(data.path)); },
          })],
        })
        : banner({ tone: 'danger', title: 'The download didn’t work', text: `${errorText(err)} Try again, or choose another kind of file.` }));
    } finally {
      working = false;
      setBusy(goBtn, false, 'Download');
    }
  }

  await openDialog({
    title: 'Download this page', iconName: 'download', tone: 'info',
    body: [
      h('p', null, `“${data.title}”`),
      choiceCards('dl-format', [
        { value: 'pdf', label: 'PDF document', text: 'Best for reading, printing, or sending to someone. Includes pictures.' },
        { value: 'zip', label: 'Markdown with pictures (.zip)', text: 'Best for backups or moving to another tool. Keeps pictures and links working.' },
        { value: 'md', label: 'Markdown file (.md)', text: 'Just the text of this page.' },
      ]),
      status,
    ],
    actions: [
      { label: 'Close', value: 'cancel' },
      { label: 'Download', value: 'go', kind: 'primary', submit: true, icon: 'download' },
    ],
    onOpen: (dlg) => { dialog = dlg; },
    onSubmit: (value) => {
      if (value === 'go' && !working) run();
      return false; // stay open to show progress and the result
    },
  });
}

// ------------------------------------------------- folder or everything

/**
 * Download dialog for a folder (`scope: 'folder'`) or all documentation
 * (`scope: 'all'`). `pageCount` is shown before starting, when known.
 */
export async function openBulkDownload({ scope, path = '', name, pageCount }) {
  const status = h('div', { class: 'dl-status' });
  const what = scope === 'all' ? 'all documentation' : 'this folder';
  let dialog = null;
  let job = null; // { id, total, done }
  let timer = 0;
  let endGuard = null; // ends the "keep this tab open" guard while a job runs

  const countLine = pageCount === undefined ? null
    : h('p', { class: 'help' },
      `${scope === 'all' ? 'The documentation' : 'This folder'} has ${pageCount === 1 ? '1 page' : `${pageCount} pages`}. PDFs may take a few minutes; Markdown takes seconds.`);

  function progressView(done, total) {
    return h('div', { class: 'dl-progress' },
      h('label', { for: 'dl-bar' }, `Preparing page ${Math.min(done + 1, total)} of ${total}…`),
      h('progress', { id: 'dl-bar', max: String(total), value: String(done) }));
  }

  function stop() {
    clearTimeout(timer);
    job = null;
    endGuard?.();
    endGuard = null;
  }

  async function poll(goBtn) {
    if (!job) return;
    let s;
    try {
      s = await get(`/api/export/jobs/${job.id}`);
    } catch (err) {
      stop();
      clear(status).append(banner({ tone: 'danger', title: 'The download stopped', text: errorText(err) }));
      setBusy(goBtn, false, 'Try again');
      return;
    }
    if (s.state === 'running') {
      if (Math.floor(s.done / ANNOUNCE_EVERY) > Math.floor(job.done / ANNOUNCE_EVERY)) {
        announce(`Prepared ${s.done} of ${s.total} pages.`);
      }
      job.done = s.done;
      clear(status).append(progressView(s.done, s.total));
      timer = setTimeout(() => poll(goBtn), POLL_MS);
      return;
    }
    const { id } = job;
    stop();
    if (s.state === 'finished') {
      try {
        const saved = await guardAction('Preparing the download…',
          () => downloadFile(`/api/export/jobs/${id}/file`, null, s.file_name));
        clear(status).append(doneMessage(saved));
        toast(`${saved} is in your Downloads folder.`);
      } catch (err) {
        clear(status).append(banner({ tone: 'danger', title: 'The download didn’t work', text: errorText(err) }));
      }
    } else if (s.state === 'failed') {
      clear(status).append(banner({ tone: 'danger', title: 'The download didn’t work', text: `${s.message || ''} Try again, or choose Markdown instead.` }));
    }
    setBusy(goBtn, false, 'Download');
  }

  async function start() {
    const goBtn = dialog.querySelector('button[value="go"]');
    const format = chosen(dialog, 'dl-format');
    clear(status);
    setBusy(goBtn, true, 'Preparing…');
    try {
      const res = await post('/api/export/jobs', { scope, path, format });
      job = { id: res.id, total: res.total, done: 0 };
      guardAction('Preparing the download…', () => new Promise((resolve) => { endGuard = resolve; }));
      status.append(progressView(0, res.total));
      timer = setTimeout(() => poll(goBtn), POLL_MS);
    } catch (err) {
      setBusy(goBtn, false, 'Download');
      status.append(err.code === 'pdf_unavailable'
        ? banner({
          tone: 'warn', title: 'PDFs can’t be made automatically on this computer',
          text: `${errorText(err)} Choose “Markdown with pictures” instead, or open a single page and use its Download button to print it as a PDF.`,
        })
        : banner({ tone: 'danger', title: 'The download didn’t start', text: errorText(err) }));
    }
  }

  await openDialog({
    title: `Download ${what}`, iconName: 'download', tone: 'info',
    body: [
      name ? h('p', null, `“${name}”`) : null,
      choiceCards('dl-format', [
        { value: 'pdf', label: 'PDFs (.zip)', text: 'One PDF for each page, in the same folders. Best for reading and printing. Includes pictures.' },
        { value: 'md', label: 'Markdown with pictures (.zip)', text: 'Best for backups or moving to another tool. Keeps pictures and links working.' },
      ]),
      countLine,
      status,
    ],
    actions: [
      { label: 'Close', value: 'cancel' },
      { label: 'Download', value: 'go', kind: 'primary', submit: true, icon: 'download' },
    ],
    onOpen: (dlg) => { dialog = dlg; },
    onSubmit: (value) => {
      if (value === 'go' && !job) start();
      return false;
    },
  });
  if (job) {
    // Closed while preparing: stop the work and clean up.
    const { id } = job;
    stop();
    post(`/api/export/jobs/${id}/cancel`).catch(() => {});
    toast('The download was cancelled.');
  }
}

// ----------------------------------------------------------- print view

/** #/print/<path>: the page laid out for paper; opens the print window. */
export async function printView(ctx) {
  const data = await get('/api/page', { path: ctx.path });
  const m = data.meta;
  const article = h('div', { class: 'md-body', trustedHtml: data.html });
  const firstH1 = article.firstElementChild;
  if (firstH1?.tagName === 'H1' && firstH1.textContent.trim() === data.title) firstH1.remove();
  const details = [
    m.owner ? ['Owner', m.owner] : null,
    m.status ? ['Status', statusChip(m.status)?.textContent || m.status] : null,
    m.last_reviewed ? ['Last reviewed', formatDay(m.last_reviewed)] : null,
    data.modified ? ['Last changed', relativeTime(data.modified)] : null,
  ].filter(Boolean);

  const printNow = () => window.print();
  ctx.main.append(
    h('div', { class: 'no-print print-bar' },
      h('h1', null, `Print “${data.title}”`),
      banner({
        tone: 'info', title: 'To save as a PDF',
        text: h('p', null, 'In the print window, choose ', h('strong', null, 'Save as PDF'),
          IS_WINDOWS ? [' (or ', h('strong', null, 'Microsoft Print to PDF'), ')'] : null,
          ' as the printer, then choose Save.'),
      }),
      h('div', { class: 'actions' },
        button('Open the print window', { icon: 'page', kind: 'primary', large: true, onClick: printNow }),
        linkButton('Back to the page', href.page(data.path), { icon: 'back' }))),
    h('article', { class: 'print-doc', 'aria-label': 'Page as it will print' },
      h('header', { class: 'print-head' },
        h('p', { class: 'print-ws' }, ctx.app.state.workspace?.name || ''),
        h('p', { class: 'print-title' }, data.title),
        h('dl', { class: 'print-meta' }, details.map(([k, v]) => h('div', null, h('dt', null, k), h('dd', null, v))))),
      article));
  // Let pictures load before the print window takes over.
  setTimeout(printNow, 600);
  return { title: `Print: ${data.title}` };
}
