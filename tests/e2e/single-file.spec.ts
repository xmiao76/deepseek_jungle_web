import { expect, test } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { boot, choose, getState, start, waitForPly, watchErrors } from './helpers';

const singleFile = pathToFileURL(path.resolve('release/index.html')).href;

test.describe('single-file release over file://', () => {
  test('boots, plays, downloads the exact component and makes no network calls', async ({ page }, testInfo) => {
    const errors = watchErrors(page);
    const external: string[] = [];
    page.on('request', (request) => {
      if (/^https?:/i.test(request.url())) external.push(request.url());
    });
    const initial = await boot(page, singleFile);
    expect(initial.screen).toBe('menu');
    expect(['worker', 'main']).toContain(initial.engineMode);
    await page.screenshot({
      path: `docs/screenshots/single-file-menu-${testInfo.project.name}.png`,
      fullPage: true,
    });

    await start(page);
    await page.waitForTimeout(250);
    const state = await getState(page);
    expect(state.screen).toBe('game');
    await choose(page, 42, 35);
    const after = await waitForPly(page, 2, 60_000);
    expect(after.ply).toBeGreaterThanOrEqual(2);
    expect(after.latency.length).toBeGreaterThanOrEqual(1);
    await page.screenshot({
      path: `docs/screenshots/single-file-game-${testInfo.project.name}.png`,
      fullPage: true,
    });

    if (testInfo.project.name === 'chromium') {
      await page.click('[data-action="menu"]');
      await expect.poll(async () => (await getState(page)).screen, { timeout: 15_000 }).toBe('menu');
      await page.click('[data-action="download"]');
      const downloadPromise = page.waitForEvent('download');
      await page.click('[data-action="download-wasm"]');
      const download = await downloadPromise;
      expect(download.suggestedFilename()).toBe('deepseek-v4-pro-claude-code-2.0.0.wasm');
      const savePath = testInfo.outputPath('downloaded.wasm');
      await download.saveAs(savePath);
      const downloaded = createHash('sha256').update(readFileSync(savePath)).digest('hex');
      const committed = createHash('sha256')
        .update(readFileSync(path.resolve('release/deepseek-v4-pro-claude-code-2.0.0.wasm')))
        .digest('hex');
      expect(downloaded).toBe(committed);
    }

    expect(external).toEqual([]);
    expect(errors).toEqual([]);
  });
});
