// Things found by an accessibility pass that must stay fixed: heading order,
// where keyboard focus lands after a dialog step, how the chosen filter looks
// in Windows High Contrast, and radio buttons big enough to hit.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './cairn.js';

async function headingLevels(page) {
  return page.locator('main h1, main h2, main h3, main h4').evaluateAll((hs) => hs.map((h) => Number(h.tagName[1])));
}

test('empty screens never skip a heading level', async ({ page }) => {
  for (const hash of ['#/deleted', '#/history/Guides/Printer.md']) {
    await page.evaluate((h) => { location.hash = h; }, hash);
    await expect(page.locator('main h1')).toBeVisible();
    const levels = await headingLevels(page);
    levels.reduce((previous, level) => {
      expect(level, `${hash}: ${levels.join(',')}`).toBeLessThanOrEqual(previous + 1);
      return level;
    }, 0);
  }
});

test('focus stays in the dialog when the PDF warning appears', async ({ page, cairn }) => {
  writeFileSync(join(cairn.docs, 'Guides', 'Linky.md'), '# Linky\n\nSee [the printer](Printer.md).\n');
  await page.evaluate(() => { location.hash = '#/page/Guides/Linky.md'; });
  await page.getByRole('button', { name: 'Download' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Download', exact: true }).click();
  const anyway = dialog.getByRole('button', { name: 'Download anyway' });
  await expect(anyway).toBeVisible();
  await expect(anyway).toBeFocused();
});

test('focus stays in the dialog when a backup finishes', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Back up workspace' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Back up', exact: true }).click();
  await expect(dialog).toContainText('is in your Downloads folder');
  await expect(dialog.getByRole('button', { name: 'Back up', exact: true })).toBeFocused();
});

test('in High Contrast the chosen filter is told apart from the others', async ({ page }) => {
  await page.emulateMedia({ forcedColors: 'active' });
  await page.evaluate(() => { location.hash = '#/health'; });
  const chips = page.locator('.filters a');
  await expect(chips.first()).toBeVisible();
  const current = await chips.evaluateAll((as) => as.map((a) => getComputedStyle(a).backgroundColor));
  const [chosen, ...others] = current;
  for (const other of others) expect(other).not.toBe(chosen);
});

test('radio buttons meet the 24 pixel minimum', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  const sizes = await page.locator('main input[type=radio]').evaluateAll((rs) => rs.map((r) => r.getBoundingClientRect()).map((b) => Math.min(b.width, b.height)));
  expect(sizes.length).toBeGreaterThan(0);
  for (const size of sizes) expect(size).toBeGreaterThanOrEqual(24);
});
