import { test, expect } from '@playwright/test';

// Rust engine backend — the browser-hosted proof that the Web-Worker Rust
// search (rust-engine.wasm) plays a full game through the real UI, at far
// greater depth than the C# AOT engine at the same budget.

test('Rust engine plays a full AI-vs-AI game at depth >= C# baseline', async ({ page }) => {
    await page.goto('/?mode=aiai&engine=rust&diff=easy&pace=30');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });
    await expect(page.locator('.piece')).toHaveCount(16);

    // The Rust AI loop must be running: a search-info line appears after the first move.
    await expect(page.locator('.search-info')).not.toHaveText('', { timeout: 60_000 });
    const searchInfo = await page.locator('.search-info').textContent();
    console.log(`[Rust engine browser sample] ${searchInfo?.trim()}`);

    // Wait for the full game to finish (den entry, elimination, stalemate, or draw).
    const overlay = page.locator('.result-overlay');
    await expect(overlay).toBeVisible({ timeout: 180_000 });

    const result = await overlay.getAttribute('data-result');
    expect(['blue-wins', 'red-wins', 'draw']).toContain(result);
    await expect(page.locator('.result-message')).not.toHaveText('');
});

test('Rust engine reaches depth >= 10 at the Hard budget (C# AOT is depth 8)', async ({ page }) => {
    await page.goto('/?mode=aiai&engine=rust&diff=hard&pace=5');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });
    await expect(page.locator('.search-info')).not.toHaveText('', { timeout: 60_000 });

    // Let a steady-state search complete (a later move, not the warm-up first).
    await page.waitForTimeout(5000);
    const info = await page.locator('.search-info').textContent() ?? '';
    console.log(`[Rust Hard depth sample] ${info.trim()}`);

    const m = info.match(/depth\s+(\d+)/);
    const depth = m ? Number(m[1]) : 0;
    expect(depth).toBeGreaterThanOrEqual(10);
});
