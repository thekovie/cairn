// Words and shortcuts follow the computer Cairn runs on: ⌘ on a Mac,
// Ctrl elsewhere.
import { test, expect, openEditor } from './cairn.js';

const MAC = process.platform === 'darwin';

test('toolbar shortcuts are shown the way this computer writes them', async ({ page }) => {
  await openEditor(page, 'Guides/Printer.md');
  const bold = page.getByRole('button', { name: 'Bold' });
  await expect(bold).toHaveAttribute('data-tip', MAC ? 'Bold (⌘B)' : 'Bold (Ctrl+B)');
  await expect(bold).toHaveAttribute('aria-keyshortcuts', MAC ? 'Meta+B' : 'Control+B');
  await expect(page.getByRole('button', { name: 'Redo' }))
    .toHaveAttribute('data-tip', MAC ? 'Redo (⇧⌘Z)' : 'Redo (Ctrl+Y)');
});

test('the Replace shortcut opens Replace', async ({ page }) => {
  await openEditor(page, 'Guides/Printer.md');
  await page.locator('[contenteditable="true"]').click();
  // ⌘H would hide the browser on a Mac, so there it's ⇧⌘H.
  await page.keyboard.press(MAC ? 'Meta+Shift+H' : 'Control+H');
  await expect(page.getByRole('dialog', { name: 'Replace words in this page' })).toBeVisible();
});

test('setup gives example folders for this computer', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/setup'; });
  await page.getByRole('button', { name: /Open a documentation folder we already use/ }).click();
  const example = page.locator('#folder-path-help');
  if (MAC) await expect(example).toContainText('/Volumes/');
  else if (process.platform === 'win32') await expect(example).toContainText('C:\\');
  else await expect(example).toContainText('/home/');
});
