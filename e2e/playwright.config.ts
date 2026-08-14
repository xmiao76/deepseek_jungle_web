import { defineConfig } from '@playwright/test';

export default defineConfig({
    testDir: './tests',
    timeout: 240_000,
    expect: { timeout: 10_000 },
    fullyParallel: false,
    retries: 0,
    reporter: [['list']],
    use: {
        baseURL: 'http://127.0.0.1:8123',
        headless: true,
    },
    webServer: {
        command: 'node serve.mjs',
        url: 'http://127.0.0.1:8123',
        reuseExistingServer: true,
        timeout: 30_000,
    },
    projects: [
        { name: 'chromium', use: { browserName: 'chromium' } },
    ],
});
