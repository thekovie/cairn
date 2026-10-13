// Settings → Back up workspace: the dialog says what a backup holds, makes
// the file, and the saved zip carries the pages and its contents list.
import { readFileSync } from 'node:fs';
import { test, expect } from './cairn.js';

test('Back up workspace saves a zip with the pages and its contents list', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Back up workspace' }).click();

  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('earlier versions, and deleted pages');
  await expect(dialog).toContainText('does not lock the shared folder');

  const saved = page.waitForEvent('download');
  await dialog.getByRole('button', { name: 'Back up', exact: true }).click();
  const file = await saved;
  expect(file.suggestedFilename()).toMatch(/-backup-\d{4}-\d{2}-\d{2}\.zip$/);
  await expect(dialog).toContainText('is in your Downloads folder');

  // File names sit uncompressed in a zip, so they can be checked directly.
  const bytes = readFileSync(await file.path()).toString('latin1');
  expect(bytes).toContain('cairn-backup.json');
  expect(bytes).toContain('shared-docs.json');
  expect(bytes).toContain('Guides/Printer.md');
});

test('Download everything says it is not a backup', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Download everything' }).click();
  await expect(page.getByRole('dialog')).toContainText('Not a backup');
});
