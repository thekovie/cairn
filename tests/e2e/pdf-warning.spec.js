// Before a PDF is made, a person is told that links to other pages can't be
// followed in a PDF and will become a note beside the link's words. They
// must choose "Download anyway" to carry on; another kind of file, or a page
// with no such links, needs no warning.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { test, expect } from './cairn.js';

function pages(docs) {
  writeFileSync(join(docs, 'Guides', 'Linky.md'),
    '# Linky\n\nSee the [printer guide](Printer.md) and the [web](https://example.com).\n');
  writeFileSync(join(docs, 'Guides', 'Plain.md'), '# Plain\n\nNo links here.\n');
}

test('a page with links to other pages warns before it becomes a PDF', async ({ page, cairn }) => {
  pages(cairn.docs);
  await page.evaluate(() => { location.hash = '#/page/Guides/Linky.md'; });
  await page.getByRole('button', { name: 'Download' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('radio', { name: /PDF document/ })).toBeChecked();

  await dialog.getByRole('button', { name: 'Download', exact: true }).click();
  await expect(dialog).toContainText('Links to other pages will change in the PDF');
  await expect(dialog).toContainText('1 link to other pages or files');
  await expect(dialog).toContainText('Guides/Printer.md');
  await expect(dialog).toContainText('(see “Printer setup”)');
  await expect(dialog.getByRole('button', { name: 'Download anyway' })).toBeVisible();
  // Nothing has been downloaded yet.
  await expect(dialog).not.toContainText('Downloads folder');

  // Another kind of file: the warning goes, and the button is plain again.
  await dialog.getByRole('radio', { name: /Markdown file/ }).check();
  await expect(dialog).not.toContainText('Links to other pages will change');
  await expect(dialog.getByRole('button', { name: 'Download', exact: true })).toBeVisible();
});

test('after the warning, Download anyway carries on', async ({ page, cairn }) => {
  pages(cairn.docs);
  await page.evaluate(() => { location.hash = '#/page/Guides/Linky.md'; });
  await page.getByRole('button', { name: 'Download' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Download', exact: true }).click();
  await dialog.getByRole('button', { name: 'Download anyway' }).click();
  // Either the PDF is made, or (no browser here) it says so: it did proceed.
  await expect(dialog).toContainText(/is in your Downloads folder|can.t be made automatically/);
});

test('a page without such links goes straight ahead', async ({ page, cairn }) => {
  pages(cairn.docs);
  await page.evaluate(() => { location.hash = '#/page/Guides/Plain.md'; });
  await page.getByRole('button', { name: 'Download' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('button', { name: 'Download', exact: true }).click();
  await expect(dialog).toContainText(/is in your Downloads folder|can.t be made automatically/);
  await expect(dialog).not.toContainText('Links to other pages will change');
});

test('a folder of PDFs warns with how many links and pages', async ({ page, cairn }) => {
  pages(cairn.docs);
  await page.evaluate(() => { location.hash = '#/folder/Guides'; });
  await page.getByRole('button', { name: 'Download this folder' }).click();
  const dialog = page.getByRole('dialog');
  await expect(dialog.getByRole('radio', { name: /PDFs/ })).toBeChecked();
  await dialog.getByRole('button', { name: 'Download', exact: true }).click();
  await expect(dialog).toContainText('Links to other pages will change in the PDF');
  await expect(dialog).toContainText('1 link to other pages or files');
  await expect(dialog.getByRole('button', { name: 'Download anyway' })).toBeVisible();

  // Markdown has no such problem.
  await dialog.getByRole('radio', { name: /Markdown with pictures/ }).check();
  await expect(dialog).not.toContainText('Links to other pages will change');
});
