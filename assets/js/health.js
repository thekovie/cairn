// Documentation health: one list of the pages that need attention, with a
// short row of filters. It never says "all fine" if Cairn could not read
// everything.

import {
  get, h, icon, banner, emptyState, statusChip, formatDay, href, announce,
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

function problemRow(item) {
  const p = item.page;
  const folder = p.path.includes('/') ? p.path.slice(0, p.path.lastIndexOf('/')).replaceAll('/', ' › ') : '';
  return h('li', null, h('a', { class: 'row-link', href: href.page(p.path) },
    icon('page'),
    h('span', { class: 'row-title' }, p.title),
    h('span', { class: 'row-side' }, folder),
    h('span', { class: 'row-meta' },
      statusChip(p.status),
      p.owner ? h('span', null, `Owner: ${p.owner}`) : null,
      item.issues.filter((key) => !SHOWN_BY_CHIP.has(key)).map((key) => h('span', { class: NEEDS_ATTENTION.has(key) ? 'review-due' : null },
        NEEDS_ATTENTION.has(key) ? icon('alert') : null, PROBLEM_TEXT[key](p))))));
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

  const clean = data.complete && data.pages.length === 0;
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
      ? h('ul', { class: 'list' }, shown.map(problemRow))
      : emptyState({ title: 'Nothing in this list', text: 'No page has this problem. Choose another filter to see the rest.' })));
  announce(clean ? 'Nothing needs attention' : `${shown.length} of ${pagesWord(data.total)} listed`);
  return { title: 'Documentation health' };
}
