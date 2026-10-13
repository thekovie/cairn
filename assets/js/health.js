// Documentation health: one list of the pages that need attention, with a
// short row of filters. It never says "all fine" if Cairn could not read
// everything.

import {
  get, post, h, icon, button, banner, emptyState, statusChip, formatDay, href, announce,
  whileBusy, errorText, toast,
} from './core.js';

const FILTERS = [
  ['all', 'All'],
  ['overdue', 'Review due'],
  ['no_review_date', 'No review date'],
  ['no_owner', 'No owner'],
  ['draft', 'Draft'],
  ['retired', 'Retired'],
  ['details', 'Page details to fix'],
];

/** What is wrong with a page, in a few plain words each. */
const PROBLEM_TEXT = {
  overdue: (p) => `Review due: last reviewed ${formatDay(p.last_reviewed)}`,
  no_review_date: () => 'No review date',
  no_owner: () => 'No owner',
  draft: () => 'Draft',
  retired: () => 'Retired',
  details: (p) => `Page details can’t be read: ${p.warnings.join(' ')}`,
};

const NEEDS_ATTENTION = new Set(['overdue', 'details']);
// The status chip beside the title already says these.
const SHOWN_BY_CHIP = new Set(['draft', 'retired']);

const pagesWord = (n) => (n === 1 ? '1 page' : `${n} pages`);

/**
 * "Mark as reviewed today" for one page. Reviewing is not approving: it only
 * sets the page's review date. The screen is redrawn afterwards.
 */
export function markReviewedButton(path, title) {
  const btn = button('Mark as reviewed today', {
    icon: 'check',
    'aria-label': `Mark as reviewed today: ${title}`,
    onClick: async () => {
      try {
        await whileBusy(btn, 'Marking…', () => post('/api/review', { path }));
        toast('Marked as reviewed today.');
        window.dispatchEvent(new HashChangeEvent('hashchange'));
      } catch (err) {
        toast(errorText(err), { error: true });
      }
    },
  });
  return btn;
}

function problemRow(item, canReview) {
  const p = item.page;
  const folder = p.path.includes('/') ? p.path.slice(0, p.path.lastIndexOf('/')).replaceAll('/', ' › ') : '';
  const reviewable = canReview && item.issues.some((key) => key === 'overdue' || key === 'no_review_date');
  return h('li', null,
    h('a', { class: 'row-link', href: href.page(p.path) },
      icon('page'),
      h('span', { class: 'row-title' }, p.title),
      h('span', { class: 'row-side' }, folder),
      h('span', { class: 'row-meta' },
        statusChip(p.status),
        p.owner ? h('span', null, `Owner: ${p.owner}`) : null,
        item.issues.filter((key) => !SHOWN_BY_CHIP.has(key)).map((key) => h('span', { class: NEEDS_ATTENTION.has(key) ? 'review-due' : null },
          NEEDS_ATTENTION.has(key) ? icon('alert') : null, PROBLEM_TEXT[key](p))))),
    reviewable ? h('div', { class: 'row-actions' }, markReviewedButton(p.path, p.title)) : null);
}

const plural = (n, one, many) => (n === 1 ? `1 ${one}` : `${n} ${many}`);

/** Pages with links or pictures that lead nowhere, and what is wrong. */
function findingRow(item) {
  return h('li', null, h('a', { class: 'row-link', href: href.page(item.path) },
    icon('page'),
    h('span', { class: 'row-title' }, item.title),
    h('span', { class: 'row-side' }, plural(item.findings.length, 'problem', 'problems')),
    h('span', { class: 'row-meta row-stack' },
      item.findings.map((f) => h('span', { class: 'review-due' },
        icon('alert'), `Line ${f.line}: ${f.message}`)))));
}

function checkResults(report) {
  const where = (p) => `“${p.path}”: ${p.reason}`;
  const partial = report.complete ? null : banner({
    tone: 'warn',
    title: 'Cairn couldn’t read some pages',
    text: h('div', null,
      h('p', null, 'They were not checked, so this result is partial. Check that the shared folder is reachable, then check again:'),
      h('ul', null, report.unreadable.map((p) => h('li', null, where(p))))),
  });
  if (report.complete && report.total_findings === 0) {
    return [banner({
      tone: 'ok',
      title: 'No broken links or pictures found',
      text: `Checked ${plural(report.checked, 'page', 'pages')}.`,
      role: 'status',
    })];
  }
  return [
    partial,
    report.total_findings === 0 ? null : h('p', { class: 'help' },
      `Checked ${plural(report.checked, 'page', 'pages')}. Found ${plural(report.total_findings, 'problem', 'problems')} on ${plural(report.pages.length, 'page', 'pages')}. Line numbers count from the page title.`),
    report.pages.length ? h('ul', { class: 'list' }, report.pages.map(findingRow)) : null,
  ];
}

const POLL_MS = 400;
const ANNOUNCE_EVERY = 25;

function checkProgress(done, total, onStop) {
  return h('div', { class: 'dl-progress' },
    h('label', { for: 'check-bar' }, `Checking page ${Math.min(done + 1, total)} of ${total}…`),
    h('progress', { id: 'check-bar', max: String(Math.max(total, 1)), value: String(Math.min(done, total)) }),
    h('div', { class: 'actions' }, button('Stop', { icon: 'x', onClick: onStop })));
}

/**
 * "Check documentation": looks through every page when asked, showing how
 * far it has got and letting the person stop it. Returns the section and a
 * `cleanup` that stops a check still running when the screen is left.
 */
function linkCheckSection() {
  const results = h('div', { class: 'check-results', tabindex: '-1' });
  let job = null; // { id, total, done }
  let timer = 0;

  const go = button('Check documentation', { icon: 'link', onClick: start });

  function finish() {
    clearTimeout(timer);
    job = null;
    go.disabled = false;
  }

  function show(...children) {
    results.replaceChildren(...children.filter(Boolean));
    results.focus({ preventScroll: true });
    results.scrollIntoView({ block: 'nearest' });
  }

  function stop() {
    if (!job) return;
    post(`/api/export/jobs/${job.id}/cancel`).catch(() => {});
    const stopBtn = results.querySelector('button');
    if (stopBtn) stopBtn.disabled = true;
  }

  async function poll() {
    if (!job) return;
    let state;
    try {
      state = await get(`/api/export/jobs/${job.id}`);
    } catch (err) {
      finish();
      show(banner({ tone: 'danger', title: 'The check stopped', text: errorText(err) }));
      return;
    }
    if (state.state === 'running') {
      if (Math.floor(state.done / ANNOUNCE_EVERY) > Math.floor(job.done / ANNOUNCE_EVERY)) {
        announce(`Checked ${state.done} of ${state.total} pages.`);
      }
      job.done = state.done;
      results.replaceChildren(checkProgress(state.done, state.total, stop));
      timer = setTimeout(poll, POLL_MS);
      return;
    }
    const { id } = job;
    finish();
    if (state.state === 'cancelled') {
      show(h('p', { class: 'help' }, 'The check was stopped. Nothing was changed.'));
      announce('The check was stopped');
    } else if (state.state === 'failed') {
      show(banner({ tone: 'danger', title: 'The check didn’t finish', text: state.message || '' }));
    } else {
      try {
        const report = await get(`/api/health/check/jobs/${id}/result`);
        show(...checkResults(report));
        announce(report.total_findings === 0 && report.complete
          ? 'No broken links or pictures found'
          : `${plural(report.total_findings, 'problem', 'problems')} found`);
      } catch (err) {
        show(banner({ tone: 'danger', title: 'The check didn’t finish', text: errorText(err) }));
      }
    }
  }

  async function start() {
    go.disabled = true;
    try {
      const res = await post('/api/health/check/jobs');
      job = { id: res.id, total: res.total, done: 0 };
      show(checkProgress(0, res.total, stop));
      timer = setTimeout(poll, POLL_MS / 2);
    } catch (err) {
      go.disabled = false;
      show(banner({ tone: 'danger', title: 'The check didn’t start', text: errorText(err) }));
    }
  }

  const element = h('section', { class: 'section', 'aria-labelledby': 'check-title' },
    h('h2', { id: 'check-title' }, 'Links and pictures'),
    h('p', { class: 'help' }, 'Looks through every page for links to pages, files, or headings that don’t exist, and for missing pictures. Web addresses are not visited. It reads every page once, so on a large shared folder it can take a while: a bar shows how far it has got, and you can stop it.'),
    h('div', { class: 'actions' }, go),
    results);

  return {
    element,
    cleanup() {
      if (job) post(`/api/export/jobs/${job.id}/cancel`).catch(() => {});
      finish();
    },
  };
}

function filterBar(data, current) {
  return h('nav', { 'aria-label': 'Show pages with' },
    h('ul', { class: 'filters' }, FILTERS.map(([key, label]) => {
      const n = key === 'all' ? data.pages.length : data.counts[key] ?? 0;
      return h('li', null, h('a', {
        class: `btn${key === current ? ' btn-primary' : ''}`,
        href: href.health(key),
        'aria-current': key === current ? 'true' : null,
      }, `${label} (${n})`));
    })));
}

/** Said when Cairn could not read part of the documentation. */
function incompleteBanner(problems) {
  const where = (p) => (p.path ? `“${p.path}”` : 'the documentation folder');
  return banner({
    tone: 'warn',
    title: 'Cairn couldn’t read everything',
    text: h('div', null,
      h('p', null, 'This list may be out of date. Pages Cairn already knew about are still shown as they were. Check that the shared folder is reachable and that you can open these, then reload:'),
      h('ul', null, problems.map((p) => h('li', null, `${where(p)}: ${p.reason}`)))),
  });
}

export async function healthView(ctx) {
  const data = await get('/api/health');
  const asked = ctx.query.get('show') || 'all';
  const current = FILTERS.some(([key]) => key === asked) ? asked : 'all';
  const shown = current === 'all' ? data.pages : data.pages.filter((p) => p.issues.includes(current));

  const check = linkCheckSection();
  const clean = data.complete && data.pages.length === 0;
  // Anyone who can change pages can mark one as reviewed.
  const canReview = !ctx.app.state.workspace?.read_only;
  ctx.main.append(
    h('div', { class: 'page-head' },
      h('h1', null, 'Documentation health'),
      h('p', { class: 'lede' }, 'Pages that need attention: overdue for review, missing an owner or a review date, still drafts, retired, or with page details Cairn can’t read. Open a page and choose Edit, then Page details, to fix it.')),
    data.complete ? null : incompleteBanner(data.problems),
    clean
      ? banner({
        tone: 'ok',
        title: 'Nothing needs attention',
        text: `All ${pagesWord(data.total)} have an owner and a recent review date, and none is a draft or retired.`,
        role: 'status',
      })
      : h('p', { class: 'help' }, `${pagesWord(data.total)} checked. ${data.pages.length === 1 ? '1 page needs' : `${data.pages.length} pages need`} attention.`),
    clean ? null : filterBar(data, current),
    clean ? null : (shown.length
      ? h('ul', { class: 'list' }, shown.map((item) => problemRow(item, canReview)))
      : emptyState({ level: 2, title: 'Nothing in this list', text: 'No page has this problem. Choose another filter to see the rest.' })),
    check.element);
  announce(clean ? 'Nothing needs attention' : `${shown.length} of ${pagesWord(data.total)} listed`);
  return { title: 'Documentation health', cleanup: check.cleanup };
}
