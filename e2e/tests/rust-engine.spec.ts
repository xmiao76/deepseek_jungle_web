import { test, expect } from '@playwright/test';

// Rust engine backend smoke — the browser-hosted proof that the Web-Worker Rust
// search (rust-engine.wasm) plays a full game through the real UI. This is the
// G5 gate from the strength program: a full AI-vs-AI game via ?engine=rust.

test('Rust engine plays a full AI-vs-AI game to a terminal state', async ({ page }) => {
    // Easy (300 ms budget) + short pace keeps the game fast while exercising the
    // real Rust search, move serialization, and win detection in the browser.
    await page.goto('/?mode=aiai&engine=rust&diff=easy&pace=30');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });
    await expect(page.locator('.piece')).toHaveCount(16);

    // The Rust AI loop must be running: a search-info line appears after the first move.
    await expect(page.locator('.search-info')).not.toHaveText('', { timeout: 60_000 });
    const searchInfo = await page.locator('.search-info').textContent();
    console.log(`[Rust engine browser sample] ${searchInfo?.trim()}`);

    // Wait for the full game to finish.
    const overlay = page.locator('.result-overlay');
    await expect(overlay).toBeVisible({ timeout: 180_000 });

    const result = await overlay.getAttribute('data-result');
    expect(['blue-wins', 'red-wins', 'draw']).toContain(result);
    await expect(page.locator('.result-message')).not.toHaveText('');
});
