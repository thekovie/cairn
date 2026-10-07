// Nothing is lost, and nothing is left half done, when someone closes the
// editor, follows a link, or closes the tab while Cairn is still working.
import {
  test, expect, apiGet, leavingAsksFirst, holdRequests, failRequests, openEditor, editorText,
  typeAtEnd, addPicture, toast,
} from './cairn.js';

const PAGE = 'Guides/Printer.md';

test.describe('when the draft can’t be saved', () => {
  test('Close editor keeps the editor open and the page locked', async ({ page }) => {
    await openEditor(page, PAGE);
    await failRequests(page, '**/api/draft/save');
    await typeAtEnd(page, 'Important words');

    await page.getByRole('button', { name: 'Close editor' }).click();

    await expect(toast(page, 'could not be saved, so the editor stayed open')).toBeVisible();
    await expect(page).toHaveURL(/#\/edit\//);
    expect((await apiGet(page, `/api/edit/status?path=${PAGE}`)).lock_held).toBe(true);
    expect(await editorText(page)).toContain('Important words');

    // Once saving works again, closing keeps the words as unsaved changes.
    await page.unroute('**/api/draft/save');
    await page.getByRole('button', { name: 'Close editor' }).click();
    await expect(page).toHaveURL(/#\/page\//);
    const { drafts } = await apiGet(page, '/api/drafts');
    expect(drafts.map((d) => d.path)).toContain(PAGE);
  });

  test('following a link and choosing to leave stays in the editor', async ({ page }) => {
    await openEditor(page, PAGE);
    await failRequests(page, '**/api/draft/save');
    await typeAtEnd(page, 'Important words');

    await page.getByRole('navigation', { name: 'Main' }).getByRole('link', { name: 'Home' }).click();
    await page.getByRole('button', { name: 'Leave and keep my changes' }).click();

    await expect(toast(page, 'could not be saved')).toBeVisible();
    await expect(page).toHaveURL(/#\/edit\//);
    expect((await apiGet(page, `/api/edit/status?path=${PAGE}`)).lock_held).toBe(true);
  });
});

test.describe('pictures in the visual editor', () => {
  test('a picture added at the page title goes below it, not into it', async ({ page }) => {
    await openEditor(page, PAGE);
    // The editor opens with the cursor at the start of the title.
    await addPicture(page);
    await expect.poll(() => editorText(page)).toContain('![dot]');

    const text = await editorText(page);
    expect(text).not.toMatch(/^#\s*$/m); // no empty heading left behind
    expect(text.indexOf('# Printer setup')).toBeLessThan(text.indexOf('![dot]'));
  });

  test('a picture added mid-sentence goes after the paragraph', async ({ page }) => {
    await openEditor(page, PAGE);
    await page.locator('[contenteditable="true"]').getByText('See the guide').click({ position: { x: 20, y: 5 } });
    // The editor has the cursor in the paragraph once Text style says so.
    await expect(page.getByRole('combobox', { name: 'Text style' })).toHaveValue('p');
    await addPicture(page);
    await expect.poll(() => editorText(page)).toContain('![dot]');

    expect(await editorText(page)).toMatch(/See the guide before you start\.\n\n!\[dot\]/);
  });
});

test('while a picture uploads, the tab and the editor can’t be closed', async ({ page }) => {
  await openEditor(page, PAGE);
  const upload = await holdRequests(page, '**/api/draft/image**');
  await addPicture(page);

  await expect(page.locator('.working-note')).toContainText('Adding the picture');
  expect(await leavingAsksFirst(page)).toBe(true);
  await page.getByRole('button', { name: 'Close editor' }).click();
  await expect(toast(page, 'Adding the picture is still going')).toBeVisible();
  await expect(page).toHaveURL(/#\/edit\//);

  upload.release();
  await expect.poll(() => editorText(page)).toContain('![dot]');
  await expect(page.locator('.working-note')).toHaveCount(0);
});

test('while publishing, changes can’t be discarded and the screen can’t change', async ({ page }) => {
  await openEditor(page, PAGE);
  await typeAtEnd(page, 'New line');
  const publish = await holdRequests(page, '**/api/publish');
  await page.getByRole('button', { name: 'Publish changes' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Publish', exact: true }).click();

  await expect(page.locator('.working-note')).toContainText('Publishing');
  expect(await leavingAsksFirst(page)).toBe(true);

  await page.getByRole('button', { name: 'Discard my changes' }).click();
  await expect(toast(page, 'Publishing is still going')).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);

  await page.evaluate(() => { location.hash = '#/settings'; });
  await expect(page).toHaveURL(/#\/edit\//);

  publish.release();
  await expect(page).toHaveURL(/#\/page\//);
  expect(await leavingAsksFirst(page)).toBe(false);
});

test('a download of everything keeps the tab open until it finishes', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  // Hold the first progress check, so the download is still "preparing".
  let held = false;
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  await page.route('**/api/export/jobs/*', async (route) => {
    if (!held && route.request().method() === 'GET') {
      held = true;
      await gate;
    }
    await route.continue();
  });

  await page.getByRole('button', { name: 'Download everything' }).click();
  await page.getByRole('dialog').getByText('Markdown with pictures').click();
  await page.getByRole('dialog').getByRole('button', { name: 'Download' }).click();

  await expect(page.locator('.working-note')).toContainText('Preparing the download');
  expect(await leavingAsksFirst(page)).toBe(true);

  release();
  await expect(page.getByRole('dialog')).toContainText('is in your Downloads folder');
  await expect(page.locator('.working-note')).toHaveCount(0);
  expect(await leavingAsksFirst(page)).toBe(false);
});

test('opening a different folder says so when it fails', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/settings'; });
  await page.route('**/api/workspace/close', (route) => route.fulfill({
    status: 500, contentType: 'application/json',
    body: JSON.stringify({ error: 'io', message: 'The folder could not be closed.' }),
  }));

  await page.getByRole('button', { name: 'Open a different documentation folder' }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Choose another folder' }).click();

  await expect(toast(page, 'The folder could not be closed.')).toBeVisible();
  await expect(page).toHaveURL(/#\/settings/);
});

test('a screen that can’t be shown gets its own tab title', async ({ page }) => {
  await page.evaluate(() => { location.hash = '#/folder/NoSuch'; });
  await expect(page.getByText('This screen could not be shown')).toBeVisible();
  await expect(page).toHaveTitle(/^Could not be shown/);
});

test('while Cairn asks a question, it doesn’t say the folder is slow', async ({ page }) => {
  // Leave unsaved changes behind, so opening the editor asks about them.
  await openEditor(page, PAGE);
  await typeAtEnd(page, 'Kept for later');
  await page.getByRole('button', { name: 'Close editor' }).click();
  await expect(page).toHaveURL(/#\/page\//);

  await page.evaluate((p) => { location.hash = `#/edit/${p}`; }, PAGE);
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.waitForTimeout(7_000); // past the 6 second "responding slowly" note

  await expect(page.locator('.loading-slow')).toBeHidden();
});
