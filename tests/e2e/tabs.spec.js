// A page opened in a second tab (a link opened in a new tab, or the address
// pasted) must work, so someone can keep one page open to refer to while
// reading or writing another. Browsers keep Cairn's access token per tab, so
// the new tab borrows it from a Cairn tab that is already open.
import { test, expect } from './cairn.js';

test('a pasted address opens the page in a second tab', async ({ page, context }) => {
  await page.evaluate(() => { location.hash = '#/page/Guides/Printer.md'; });
  await expect(page.locator('h1')).toHaveText('Printer setup');

  const second = await context.newPage();
  await second.goto(page.url());
  await expect(second.locator('h1')).toHaveText('Printer setup');

  // Each tab then goes its own way.
  await second.evaluate(() => { location.hash = '#/health'; });
  await expect(second.locator('h1')).toHaveText('Documentation health');
  await expect(page.locator('h1')).toHaveText('Printer setup');

  // The second tab keeps working after a reload, with the first one closed.
  await page.close();
  await second.reload();
  await expect(second.locator('h1')).toHaveText('Documentation health');
});

test('a tab with no Cairn tab to ask still says what to do', async ({ page, context }) => {
  const address = page.url();
  await page.close();
  const lonely = await context.newPage();
  await lonely.goto(address);
  await expect(lonely.locator('h1')).toHaveText('Please open Cairn again');
});
