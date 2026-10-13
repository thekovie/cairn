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
