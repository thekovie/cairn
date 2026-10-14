// What the open tab says when Cairn stops, comes back, or is still starting.
import { test, expect } from './cairn.js';

const stopped = (page) => page.getByRole('alert').filter({ hasText: 'Cairn has stopped' });

test('the tab says so, in words, when Cairn stops', async ({ page }) => {
  await page.evaluate(() => fetch('/api/quit', {
    method: 'POST',
    headers: { 'X-Cairn-Token': sessionStorage.getItem('cairn.token'), 'Content-Type': 'application/json' },
    body: '{}',
  }));
  const notice = stopped(page);
  await expect(notice).toBeVisible({ timeout: 15000 });
  // Says what to do, and never points at a window Windows no longer shows.
  await expect(notice).toContainText('opens in a new tab');
  await expect(notice).not.toContainText('window');
  await expect(notice.getByRole('button', { name: 'Check again' })).toBeVisible();
});

test('while Cairn is stopped, closing the tab asks first', async ({ page }) => {
  await page.route('**/api/ping', (route) => route.abort());
  await expect(stopped(page)).toBeVisible({ timeout: 15000 });
  await expect(stopped(page)).toContainText('have you copied everything');
  // Browsers only ask after the person has used the page.
  await page.locator('body').click();
  const asked = new Promise((resolve) => page.once('dialog', (dialog) => resolve(dialog)));
  await page.close({ runBeforeUnload: true });
  const dialog = await asked;
  expect(dialog.type()).toBe('beforeunload');
  await dialog.dismiss();
  expect(page.isClosed()).toBe(false);
});

test('a failed request looks at once instead of waiting for the next check', async ({ page }) => {
  await page.route('**/api/ping', (route) => route.abort());
  await page.route('**/api/folder?*', (route) => route.abort());
  await page.evaluate(() => { location.hash = '#/folder/Guides'; });
  await expect(stopped(page)).toBeVisible({ timeout: 4000 });
  await expect(page.getByText('Cairn isn’t running', { exact: true })).toBeVisible();
});

test('the notice goes away by itself when Cairn answers again', async ({ page }) => {
  let down = true;
  await page.route('**/api/ping', (route) => (down ? route.abort() : route.continue()));
  await expect(stopped(page)).toBeVisible({ timeout: 15000 });
  down = false;
  await expect(stopped(page)).toBeHidden({ timeout: 15000 });
});

test('quitting from Settings shows the one stopped page, with no notice on top', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.getByRole('button', { name: 'Quit Cairn' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Quit Cairn' }).click();
  await expect(page.getByRole('heading', { name: 'Cairn has stopped', level: 1 })).toBeVisible();
  // Longer than two checks.
  await page.waitForTimeout(8000);
  await expect(page.locator('.connection-lost')).toHaveCount(0);
});

test('the tab says Cairn is starting up while the folder is still opening', async ({ page }) => {
  let answers = 0;
  await page.route('**/api/state', async (route) => {
    const response = await route.fetch();
    const state = await response.json();
    await route.fulfill({ response, json: { ...state, starting: ++answers <= 3 } });
  });
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Starting up Cairn', level: 1 })).toBeVisible();
  await expect(page.getByRole('status')).toContainText('Opening your documentation');
  // Then the ordinary home page, with no reload.
  await expect(page.getByRole('heading', { name: 'Starting up Cairn' })).toBeHidden({ timeout: 10000 });
  await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
  expect(answers).toBeGreaterThan(3);
});
