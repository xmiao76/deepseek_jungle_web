import { expect, test } from '@playwright/test';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { boot, choose, getState, start, waitForPly, waitUntilTerminal, watchErrors } from './helpers';

const releaseInfo = JSON.parse(readFileSync(path.resolve('release/RELEASE.json'), 'utf8'));
const identityInfo = JSON.parse(readFileSync(path.resolve('identity/identity.json'), 'utf8'));

test.describe('standalone web game', () => {
  test('menu, identity, how-to and download panel', async ({ page }) => {
    const errors = watchErrors(page);
    const state = await boot(page);
    expect(state.screen).toBe('menu');
    await expect(page.locator('.hero h1')).toContainText('Jungle');
    const identity = await page.locator('.identity-line').innerText();
    expect(identity).toContain(identityInfo.model);
    expect(identity).toContain(identityInfo.codeAgent);
    expect(identity).toContain('official');
    await expect(page.locator('#menu-canvas')).toBeVisible();
    await page.screenshot({ path: 'docs/screenshots/menu.png', fullPage: true });

    await page.click('[data-action="how-to"]');
    await expect(page.locator('.modal-panel')).toContainText('third time');
    await expect(page.locator('.modal-panel')).toContainText('100 plies');
    await page.screenshot({ path: 'docs/screenshots/how-to-play.png', fullPage: true });
    await page.click('.modal-close');

    await page.click('[data-action="download"]');
    await expect(page.locator('.download-name')).toContainText(releaseInfo.filename);
    const hashLocator = page.locator('.meta-list .hash').first();
    await expect(hashLocator).toHaveText(/^[0-9a-f]{64}$/);
    const shown = await hashLocator.innerText();
    const committed = createHash('sha256')
      .update(readFileSync(path.resolve('release', releaseInfo.filename)))
      .digest('hex');
    expect(shown).toBe(committed);
    await page.screenshot({ path: 'docs/screenshots/download-panel.png', fullPage: true });
    await page.click('.modal-close');
    expect(errors).toEqual([]);
  });

  test('human move, AI reply, undo, flip, mute', async ({ page }) => {
    const errors = watchErrors(page);
    await boot(page, '/?autostart=1');
    await waitForPly(page, 0);
    let state = await getState(page);
    expect(state.humanSide).toBe('red');
    expect(state.flipped).toBe(false);
    await choose(page, 42, 35);
    state = await waitForPly(page, 2);
    expect(state.status).toBe(0);
    expect(state.latency.length).toBeGreaterThanOrEqual(1);
    expect(state.latency[0].ms).toBeLessThan(1000);
    await page.screenshot({ path: 'docs/screenshots/game.png', fullPage: true });

    await page.click('[data-action="undo"]');
    await expect.poll(async () => (await getState(page)).ply, { timeout: 15_000 }).toBe(0);
    state = await getState(page);
    expect(state.turn).toBe('red');

    await page.click('[data-action="flip"]');
    await expect.poll(async () => (await getState(page)).flipped, { timeout: 5000 }).toBe(true);
    await page.click('[data-action="flip"]');
    await expect.poll(async () => (await getState(page)).flipped, { timeout: 5000 }).toBe(false);

    await page.click('[data-action="mute"]');
    await expect(page.locator('#mute-button')).toHaveText('🔇');
    await page.click('[data-action="mute"]');
    await expect(page.locator('#mute-button')).toHaveText('🔊');
    expect(errors).toEqual([]);
  });

  test('undo during thought cancels the AI reply', async ({ page }) => {
    await boot(page, '/?autostart=1');
    await page.evaluate(() => (window as unknown as { __jungleTest: { setSettings: (s: unknown) => void } }).__jungleTest.setSettings({ difficulty: 'hard' }));
    await choose(page, 42, 35);
    await expect.poll(async () => (await getState(page)).thinking, { timeout: 5000 }).toBe(true);
    await page.click('[data-action="undo"]');
    await expect.poll(async () => (await getState(page)).ply, { timeout: 15_000 }).toBe(0);
    const state = await getState(page);
    expect(state.thinking).toBe(false);
    expect(state.turn).toBe('red');
    await page.waitForTimeout(1200);
    expect((await getState(page)).ply).toBe(0);
  });

  test('AI-first makes the human Blue and flipped', async ({ page }) => {
    await boot(page, '/?mode=human&first=ai&diff=medium&autostart=1');
    const state = await waitForPly(page, 1);
    expect(state.humanSide).toBe('blue');
    expect(state.flipped).toBe(true);
    expect(state.turn).toBe('blue');
    expect(state.ply).toBe(1);
    const legal = state.legal;
    expect(legal.length).toBeGreaterThan(0);
    const [from, to] = legal[0];
    await choose(page, from, to);
    await waitForPly(page, 3);
    await page.screenshot({ path: 'docs/screenshots/game-human-blue.png', fullPage: true });
  });

  test('Hard keeps every move under one second', async ({ page }) => {
    test.setTimeout(300_000);
    await boot(page, '/?mode=aiai&diff=hard&first=red&autostart=1');
    await waitForPly(page, 12, 90_000);
    const state = await getState(page);
    expect(state.latency.length).toBeGreaterThanOrEqual(12);
    const max = Math.max(...state.latency.map((entry) => entry.ms));
    expect(max).toBeLessThanOrEqual(1000);
    console.log(`hard latency max ${max.toFixed(1)} ms over ${state.latency.length} moves`);
  });

  test('AI vs AI completes a full game and review hides the result card', async ({ page }) => {
    test.setTimeout(300_000);
    await boot(page, '/?mode=aiai&diff=easy&first=red&autostart=1');
    const state = await waitUntilTerminal(page);
    expect([1, 2]).toContain(state.status);
    expect(state.result).not.toBe('playing');
    await expect(page.locator('#result-overlay')).toBeVisible();
    await page.screenshot({ path: 'docs/screenshots/result.png', fullPage: true });
    await page.click('[data-action="review"]');
    await expect(page.locator('#result-overlay')).toBeHidden();
    await page.screenshot({ path: 'docs/screenshots/result-review.png', fullPage: true });
  });

  test('new game resets the state', async ({ page }) => {
    await boot(page, '/?autostart=1');
    await choose(page, 42, 35);
    await waitForPly(page, 2);
    await page.click('[data-action="new-game"]');
    await expect.poll(async () => (await getState(page)).ply, { timeout: 15_000 }).toBe(0);
    const state = await getState(page);
    expect(state.thinking).toBe(false);
    expect(state.turn).toBe('red');
  });

  test('mobile layout has no horizontal overflow', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await boot(page);
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    expect(overflow).toBeLessThanOrEqual(1);
    await page.screenshot({ path: 'docs/screenshots/mobile-menu.png', fullPage: true });
    await start(page);
    await page.waitForTimeout(600);
    await page.screenshot({ path: 'docs/screenshots/mobile-game.png', fullPage: true });
  });

  test('hosted download is byte-identical to the release file', async ({ page }) => {
    await boot(page);
    await page.click('[data-action="download"]');
    const downloadPromise = page.waitForEvent('download');
    await page.click('[data-action="download-wasm"]');
    const download = await downloadPromise;
    expect(download.suggestedFilename()).toBe(releaseInfo.filename);
    const savePath = test.info().outputPath('downloaded.wasm');
    await download.saveAs(savePath);
    const downloaded = createHash('sha256').update(readFileSync(savePath)).digest('hex');
    const committed = createHash('sha256')
      .update(readFileSync(path.resolve('release', releaseInfo.filename)))
      .digest('hex');
    expect(downloaded).toBe(committed);
  });
});
