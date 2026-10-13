// Search can be narrowed by folder, status and tag from the screen; the
// choices are applied with the Search button and can be cleared; retired
// pages are found unless a status is chosen.
import { writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './cairn.js';

function write(docs, rel, front, title) {
  mkdirSync(join(docs, rel.split('/').slice(0, -1).join('/') || '.'), { recursive: true });
  writeFileSync(join(docs, rel), `---\n${front}\n---\n# ${title}\n\nAbout the kettle.\n`);
}

test('search narrows by status, folder and tag, and clears again', async ({ page, cairn }) => {
  write(cairn.docs, 'Guides/active.md', 'status: active\ntags: network', 'Active kettle');
  write(cairn.docs, 'Guides/retired.md', 'status: retired', 'Retired kettle');
  write(cairn.docs, 'Other/elsewhere.md', 'status: active', 'Kettle elsewhere');

  await page.evaluate(() => { location.hash = '#/search?q=kettle'; });
  const rows = page.locator('main .list li');
  // The page list is refreshed a few seconds after the folder opens.
  await expect(async () => {
    await page.reload();
    await expect(rows).toHaveCount(3, { timeout: 1000 });
  }).toPass({ timeout: 15000 });
  // Retired pages are found by default, and say so.
  await expect(rows.filter({ hasText: 'Retired kettle' })).toContainText('Retired');

  const search = () => page.locator('main form').getByRole('button', { name: 'Search' }).click();

  await page.getByLabel('Status').selectOption('retired');
  await search();
  await expect(page).toHaveURL(/status=retired/);
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('Retired kettle');

  await page.getByRole('link', { name: 'Clear filters' }).click();
  await expect(rows).toHaveCount(3);

  await page.getByLabel('Folder').selectOption('Guides');
  await page.getByLabel('Tag').selectOption('network');
  await search();
  await expect(page).toHaveURL(/folder=Guides/);
  await expect(page).toHaveURL(/tag=network/);
  await expect(rows).toHaveCount(1);
  await expect(rows.first()).toContainText('Active kettle');

  // Filters alone (no words) list what fits.
  await page.locator('#search-q').fill('');
  await search();
  await expect(rows).toHaveCount(1);
});

test('a search with filters that match nothing says so and offers a way out', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/search?q=zzzznothing&status=retired'; });
  await expect(page.getByText('No pages found')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Clear filters' })).toBeVisible();
});
