// A `test` whose `cairn` fixture is the real program running on a fresh
// documentation folder, and whose `page` is already open on it.
import { test as base, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = fileURLToPath(new URL('../..', import.meta.url));
const TARGET = process.env.CARGO_TARGET_DIR || join(ROOT, 'target');
const BIN = join(TARGET, 'debug', process.platform === 'win32' ? 'cairn.exe' : 'cairn');

export const PRINTER = `# Printer setup

## Steps

See the guide before you start.

### Sub

text
`;

/** Starts cairn; resolves to { url, docs, stop }. */
async function startCairn() {
  const dir = mkdtempSync(join(tmpdir(), 'cairn-e2e-'));
  const home = join(dir, 'home');
  const docs = join(dir, 'docs');
  mkdirSync(home);
  // No look for new versions: tests stay offline.
  writeFileSync(join(home, 'config.json'), JSON.stringify({ update_check: 'manual', display_name: 'Tester' }));
  execFileSync(BIN, ['init', docs, '--name', 'E2E Docs'], { env: { ...process.env, CAIRN_HOME: home } });
  mkdirSync(join(docs, 'Guides'), { recursive: true });
  writeFileSync(join(docs, 'Guides', 'Printer.md'), PRINTER);

  const child = spawn(BIN, ['--no-browser', '--workspace', docs], {
    env: { ...process.env, CAIRN_HOME: home },
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  const url = await new Promise((resolve, reject) => {
    let out = '';
    child.stdout.on('data', (chunk) => {
      out += chunk;
      const m = out.match(/http:\/\/127\.0\.0\.1:\d+\/#t=[0-9a-f]+/);
      if (m) resolve(m[0]);
    });
    child.on('exit', (code) => reject(new Error(`cairn exited (${code}) before it started:\n${out}`)));
  });
  const stop = () => {
    child.kill();
    rmSync(dir, { recursive: true, force: true, maxRetries: 5 });
  };
  return { url, docs, stop };
}

export const test = base.extend({
  cairn: async ({}, use) => {
    const cairn = await startCairn();
    await use(cairn);
    cairn.stop();
  },
  page: async ({ page, cairn }, use) => {
    await page.goto(cairn.url);
    await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
    await use(page);
  },
});

export { expect };

// ------------------------------------------------------------------ helpers

/** Calls the Cairn API from inside the page, with the page's token. */
export const apiGet = (page, path) => page.evaluate(async (p) => {
  const res = await fetch(p, { headers: { 'X-Cairn-Token': sessionStorage.getItem('cairn.token') } });
  return res.json();
}, path);

/** Whether closing or reloading the tab would ask "Leave site?" first. */
export const leavingAsksFirst = (page) => page.evaluate(() => {
  const e = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(e);
  return e.defaultPrevented;
});

/**
 * Holds requests matching `pattern` until `release()` is called, as on a
 * slow shared folder.
 */
export async function holdRequests(page, pattern) {
  let release;
  const gate = new Promise((resolve) => { release = resolve; });
  await page.route(pattern, async (route) => {
    await gate;
    await route.continue();
  });
  return { release: () => release() };
}

/** Makes every request matching `pattern` fail, as when the shared folder is unreachable. */
export const failRequests = (page, pattern) => page.route(pattern, (route) => route.abort('failed'));

export async function openEditor(page, path) {
  await page.evaluate((p) => { location.hash = `#/edit/${p}`; }, path);
  await expect(page.getByRole('button', { name: 'Publish changes' })).toBeVisible();
}

/** The page's Markdown, as the editor will save it. */
export const editorText = (page) => page.locator('#md-text').inputValue();

/** Type at the end of the page in the visual editor. */
export async function typeAtEnd(page, text) {
  const body = page.locator('[contenteditable="true"]');
  await body.click();
  await page.keyboard.press('ControlOrMeta+End');
  await page.keyboard.type(text);
}

const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64');

/** Choose a picture with "Insert picture…" and describe it. */
export async function addPicture(page, name = 'dot.png') {
  await page.locator('input[type=file]').setInputFiles({ name, mimeType: 'image/png', buffer: PNG });
  await page.getByRole('dialog').getByRole('button', { name: 'Insert picture' }).click();
}

export const toast = (page, text) => page.locator('.toast', { hasText: text });
