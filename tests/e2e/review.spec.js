// A page falls due for review a year after its last review, counted in days
// (so 365 days ago is due and 364 is not), and the page list and the page
// itself agree.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './cairn.js';

/** YYYY-MM-DD for `days` ago, in this computer's timezone (as Cairn's
 *  default "Automatic" timezone also uses). */
function daysAgo(days) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  const pad = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function page(docs, name, reviewed) {
  writeFileSync(join(docs, name), `---\nlast_reviewed: ${reviewed}\n---\n# ${name}\n\nText.\n`);
}

test('a page is due for review exactly a year after its last review', async ({ page: browser, cairn }) => {
  page(cairn.docs, 'Old.md', daysAgo(365));
  page(cairn.docs, 'Recent.md', daysAgo(364));
  const row = (title) => browser.locator('main .list a', { hasText: title }).first();
  // The list is refreshed a few seconds after the folder opens: look again.
  await expect(async () => {
    await browser.reload();
    await expect(row('Old.md')).toBeVisible({ timeout: 1000 });
  }).toPass({ timeout: 15000 });
  await expect(row('Old.md')).toContainText('Review due');
  await expect(row('Recent.md')).not.toContainText('Review due');

  // The page itself says the same.
  await row('Old.md').click();
  await expect(browser.locator('.article-meta .review-due')).toBeVisible();
});
