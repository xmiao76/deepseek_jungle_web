import { test, expect } from '@playwright/test';

// WASM runtime smoke test — the browser-hosted analog of the desktop
// `JungleGame.Bench --selfplay` crash smoke (the desktop UI has no --smoke-test mode):
// boots the real published AOT bundle, verifies cross-origin isolation (the prerequisite
// for the background-thread AI), then plays a FULL AI-vs-AI game to a terminal state.

test('cross-origin isolation is active (WASM threading available)', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });

    const isolated = await page.evaluate(() => self.crossOriginIsolated);
    expect(isolated).toBe(true);

    await expect(page.locator('.badge', { hasText: 'threading: active' })).toBeVisible();
});

test('AI vs AI plays a full game to a terminal state', async ({ page }) => {
    // Easy (300 ms budget) with a short pace keeps the full game fast while still
    // exercising the real search, move generation, capture rules, and win detection in WASM.
    await page.goto('/?mode=aiai&diff=easy&pace=30');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });
    await expect(page.locator('.piece')).toHaveCount(16);

    // The AI loop must be running: a search-info line appears after the first move.
    await expect(page.locator('.search-info')).not.toHaveText('', { timeout: 60_000 });
    const searchInfo = await page.locator('.search-info').textContent();
    console.log(`[browser NPS sample] ${searchInfo?.trim()}`);

    // Wait for the full game to finish (den entry, elimination, stalemate, or draw).
    const overlay = page.locator('.result-overlay');
    await expect(overlay).toBeVisible({ timeout: 180_000 });

    const result = await overlay.getAttribute('data-result');
    expect(['blue-wins', 'red-wins', 'draw']).toContain(result);
    await expect(page.locator('.result-message')).not.toHaveText('');
    await expect(page.locator('.move-history .move-line').first()).toBeVisible();
});
