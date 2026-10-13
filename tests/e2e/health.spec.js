// Documentation health: reached from the sidebar, lists only the pages that
// need attention with the reason, filters by problem, and says "nothing
// needs attention" only when that is true.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './cairn.js';

function daysAgo(days) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function write(docs, name, front) {
  writeFileSync(join(docs, name), `---\n${front}\n---\n# ${name}\n\nText.\n`);
}

test('the sidebar opens a list of pages that need attention, with filters', async ({ page, cairn }) => {
  write(cairn.docs, 'Overdue.md', `owner: Sam\nstatus: active\nlast_reviewed: ${daysAgo(400)}`);
  write(cairn.docs, 'Fine.md', `owner: Sam\nstatus: active\nlast_reviewed: ${daysAgo(10)}`);
  write(cairn.docs, 'Draft.md', `owner: Sam\nstatus: draft\nlast_reviewed: ${daysAgo(10)}`);

  await page.getByRole('link', { name: 'Documentation health' }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Documentation health' })).toBeVisible();

  const list = page.locator('main .list');
  await expect(list.getByRole('link', { name: /Overdue\.md/ })).toContainText('Review due');
  await expect(list.getByRole('link', { name: /Draft\.md/ })).toBeVisible();
  await expect(list.getByRole('link', { name: /Fine\.md/ })).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Review due (1)' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Draft (1)' })).toBeVisible();

  // A filter shows only its pages, is announced as the current one, and
  // can be bookmarked.
  await page.getByRole('link', { name: 'Draft (1)' }).click();
  await expect(page).toHaveURL(/#\/health\?show=draft/);
  await expect(page.getByRole('link', { name: 'Draft (1)' })).toHaveAttribute('aria-current', 'true');
  await expect(list.getByRole('link', { name: /Draft\.md/ })).toBeVisible();
  await expect(list.getByRole('link', { name: /Overdue\.md/ })).toHaveCount(0);

  // A page in the list opens.
  await list.getByRole('link', { name: /Draft\.md/ }).click();
  await expect(page.getByRole('heading', { level: 1, name: 'Draft.md' })).toBeVisible();
});

test('it says nothing needs attention only when every page is in order', async ({ page, cairn }) => {
  const fine = `owner: Sam\nstatus: active\nlast_reviewed: ${daysAgo(10)}`;
  write(cairn.docs, 'README.md', fine);
  write(cairn.docs, 'Guides/Printer.md', fine);

  await page.getByRole('link', { name: 'Documentation health' }).click();
  await expect(page.getByText('Nothing needs attention')).toBeVisible();
  await expect(page.locator('main .list')).toHaveCount(0);

  // One page loses its owner: the all-clear goes away.
  write(cairn.docs, 'Guides/Printer.md', `status: active\nlast_reviewed: ${daysAgo(10)}`);
  await page.reload();
  await expect(page.getByText('Nothing needs attention')).toHaveCount(0);
  await expect(page.locator('main .list').getByRole('link', { name: /Printer/ })).toContainText('No owner');
});

test('Check documentation finds broken links and pictures, then goes quiet once they are fixed', async ({ page, cairn }) => {
  const fine = `owner: Sam\nstatus: active\nlast_reviewed: ${daysAgo(10)}`;
  write(cairn.docs, 'README.md', fine);
  write(cairn.docs, 'Guides/Printer.md', fine);
  writeFileSync(join(cairn.docs, 'Guides', 'Links.md'),
    `---\n${fine}\n---\n# Links\n\n[a page](Gone.md) [heading](Printer.md#nowhere) [web](https://example.com)\n\n![shot](Links.assets/none.png)\n`);

  await page.getByRole('link', { name: 'Documentation health' }).click();
  await page.getByRole('button', { name: 'Check documentation' }).click();

  const results = page.locator('.check-results');
  await expect(results).toContainText('Found 3 problems on 1 page');
  await expect(results).toContainText('“Guides/Gone.md” doesn’t exist'.replace('’', "'"));
  await expect(results).toContainText('no heading called “nowhere”');
  await expect(results).toContainText('picture “Guides/Links.assets/none.png” is missing');
  await expect(results.getByText('example.com')).toHaveCount(0);

  // Fix the page; checking again says so.
  writeFileSync(join(cairn.docs, 'Guides', 'Links.md'),
    `---\n${fine}\n---\n# Links\n\n[a page](Printer.md) [web](https://example.com)\n`);
  await page.getByRole('button', { name: 'Check documentation' }).click();
  await expect(results).toContainText('No broken links or pictures found');
  await expect(results.locator('.list')).toHaveCount(0);
});
