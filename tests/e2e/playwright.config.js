import { defineConfig } from '@playwright/test';

// Each test starts its own Cairn on a fresh documentation folder (see
// cairn.js), so tests can run side by side.
export default defineConfig({
  testDir: '.',
  globalSetup: './build.js',
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: 'list',
  use: { browserName: 'chromium', trace: 'retain-on-failure' },
});
