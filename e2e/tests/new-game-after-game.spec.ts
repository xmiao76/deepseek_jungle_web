import { test, expect } from '@playwright/test';

test('first-move change + new game after a game does not error', async ({ page }) => {
    const errors = [];
    page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
    page.on('pageerror', (e) => errors.push('PAGEERROR: ' + e.message));

    await page.goto('/');
    await page.locator('.board-grid').waitFor({ timeout: 120_000 });

    // Complete a fast game (AI-vs-AI easy).
    await page.locator('select').nth(0).selectOption('AiVsAi');
    await page.locator('select').nth(2).selectOption('Easy');
    await page.locator('.result-overlay').waitFor({ timeout: 300_000 });

    // Switch to Human-vs-AI, AI-first, then click New game (the reported repro).
    await page.locator('select').nth(0).selectOption('HumanVsAi');
    await page.locator('select').nth(1).selectOption('AiFirst');
    await page.locator('button', { hasText: 'New game' }).click();

    // The AI (Blue) must actually move: the new game must not be stuck.
    await expect(page.locator('.move-history .move-line').first()).toBeVisible({ timeout: 30_000 });
    await expect(page.locator('.search-info')).not.toHaveText('', { timeout: 30_000 });

    const errorVisible = await page.locator('#blazor-error-ui').isVisible();
    console.log('error UI visible:', errorVisible);
    console.log('errors:', JSON.stringify(errors));
    expect(errorVisible).toBe(false);
    expect(errors.filter(e => !e.includes('AI turn loop failed'))).toEqual([]);
});
