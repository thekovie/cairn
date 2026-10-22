// The first-run tour and the Help button: the tour opens once, stays away once
// it has been seen (kept in Cairn's own settings, because the browser forgets
// everything when Cairn picks a new port), can be shown again from Settings,
// and Help explains whichever screen is open.
import { test, expect, apiGet } from './cairn.js';

test.use({ tourSeen: false });

const card = (page) => page.locator('dialog.tour-card');
const seen = async (page) => (await apiGet(page, '/api/state')).config.tour_seen;

test('the tour opens by itself and walks through every step', async ({ page }) => {
  await expect(card(page)).toBeVisible();
  await expect(card(page)).toContainText('Step 1 of 5');
  await expect(card(page).getByRole('button', { name: 'Next' })).toBeFocused();
  for (let step = 2; step <= 5; step += 1) {
    await card(page).getByRole('button', { name: 'Next' }).click();
    await expect(card(page)).toContainText(`Step ${step} of 5`);
  }
  await card(page).getByRole('button', { name: 'Done' }).click();
  await expect(card(page)).toHaveCount(0);
  await expect.poll(() => seen(page)).toBe(true);
});

test('Back goes to the step before, and the first step has no Back', async ({ page }) => {
  await expect(card(page).getByRole('button', { name: 'Back' })).toHaveCount(0);
  await card(page).getByRole('button', { name: 'Next' }).click();
  await card(page).getByRole('button', { name: 'Back' }).click();
  await expect(card(page)).toContainText('Step 1 of 5');
});

test('skipping keeps the tour away for good', async ({ page }) => {
  await card(page).getByRole('button', { name: 'Skip tour' }).click();
  await expect(card(page)).toHaveCount(0);
  await expect.poll(() => seen(page)).toBe(true);
  await page.reload();
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await expect(card(page)).toHaveCount(0);
});

test('Escape ends the tour and counts as seen', async ({ page }) => {
  await expect(card(page)).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(card(page)).toHaveCount(0);
  await expect.poll(() => seen(page)).toBe(true);
});

test('Settings can show the tour again', async ({ page }) => {
  await card(page).getByRole('button', { name: 'Skip tour' }).click();
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Show the tour again' }).click();
  await expect(page).toHaveURL(/#\/$/);
  await expect(card(page)).toContainText('Step 1 of 5');
});

test('Help explains the screen that is open, and only when asked', async ({ page }) => {
  await card(page).getByRole('button', { name: 'Skip tour' }).click();
  await page.getByRole('button', { name: 'Help' }).click();
  const help = page.getByRole('dialog', { name: 'What you can do here' });
  await expect(help).toContainText('search box');
  await help.getByRole('button', { name: 'Close' }).click();
  await expect(help).toHaveCount(0);

  await page.evaluate(() => { location.hash = '#/page/Guides/Printer.md'; });
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  await page.getByRole('button', { name: 'Help' }).click();
  await expect(page.getByRole('dialog', { name: 'What you can do here' })).toContainText('Edit this page');
});

test('Help can start the tour', async ({ page }) => {
  await card(page).getByRole('button', { name: 'Skip tour' }).click();
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Help' }).click();
  await page.getByRole('dialog', { name: 'What you can do here' }).getByRole('button', { name: 'Take the tour' }).click();
  await expect(card(page)).toContainText('Step 1 of 5');
});

test('on a phone the Help button leaves room for the name of the documentation', async ({ page }) => {
  await card(page).getByRole('button', { name: 'Skip tour' }).click();
  await page.setViewportSize({ width: 390, height: 800 });
  const name = page.locator('.brand-ws');
  await expect(name).toHaveText('E2E Docs');
  const { clientWidth, scrollWidth } = await name.evaluate((el) => ({ clientWidth: el.clientWidth, scrollWidth: el.scrollWidth }));
  expect(scrollWidth, 'the name is cut off').toBeLessThanOrEqual(clientWidth);
  // Icon only here, but screen readers still hear the names.
  await expect(page.getByRole('button', { name: 'Help' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Settings' })).toBeVisible();
});

test('on a narrow window the tour opens the page list for its steps and closes it after', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await page.reload();
  await expect(card(page)).toContainText('Step 1 of 5');
  const shell = page.locator('.shell');
  await card(page).getByRole('button', { name: 'Next' }).click();
  await expect(card(page)).toContainText('Step 2 of 5');
  await expect(shell).toHaveClass(/nav-open/);
  // The spotlight is on the list once it has slid into view (the ring sits
  // 4px outside the list, which touches the edge of the window).
  await expect.poll(async () => (await page.locator('.tour-ring').boundingBox()).x).toBeGreaterThanOrEqual(-4);
  await card(page).getByRole('button', { name: 'Next' }).click();
  await expect(card(page)).toContainText('Step 3 of 5');
  await expect(shell).not.toHaveClass(/nav-open/);
});
