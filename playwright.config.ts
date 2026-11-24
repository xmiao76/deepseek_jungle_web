import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 300_000,
  expect: { timeout: 10_000 },
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:8123',
    headless: true,
    acceptDownloads: true,
    viewport: { width: 1440, height: 1000 },
  },
  webServer: {
    command: 'node scripts/serve.mjs',
    url: 'http://127.0.0.1:8123',
    reuseExistingServer: true,
    timeout: 30_000,
  },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium' } },
    {
      name: 'firefox',
      use: { browserName: 'firefox' },
      testMatch: /single-file|smoke/,
    },
    {
      name: 'webkit',
      use: { browserName: 'webkit' },
      testMatch: /single-file|smoke/,
    },
  ],
});
