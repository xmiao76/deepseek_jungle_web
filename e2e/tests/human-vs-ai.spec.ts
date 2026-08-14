import { test, expect, Page } from '@playwright/test';

// Scripted human-vs-AI game: the default mode is Human-vs-AI with the human playing Blue
// and moving first (the desktop StartDialog default). Starting layout (from the engine):
// Blue cat at (1,1), blue dog at (5,1); squares (1,2) and (5,2) are empty land squares,
// so the scripted moves are always legal regardless of the AI's replies (Red moves cannot
// reach rows 0-2 in one move).

async function waitForHumanTurn(page: Page) {
    await expect(page.locator('.status-text', { hasText: 'Blue to move' }))
        .toBeVisible({ timeout: 60_000 });
}

async function playMove(page: Page, from: string, to: string) {
    await page.locator(`.square[data-square="${from}"]`).click();
    await expect(page.locator('.highlight-selected')).toHaveCount(1);
    await expect(page.locator('.highlight-target, .highlight-capture')).not.toHaveCount(0);
    await page.locator(`.square[data-square="${to}"]`).click();
}

test('human plays scripted moves against the AI', async ({ page }) => {
    await page.goto('/');
    await expect(page.locator('.board-grid')).toBeVisible({ timeout: 120_000 });
    await expect(page.locator('.boot-overlay')).toHaveCount(0);
    await expect(page.locator('.piece')).toHaveCount(16);

    // Human (Blue) moves first.
    await expect(page.locator('.status-text', { hasText: 'Blue to move' })).toBeVisible();

    // Move 1: blue cat (1,1) -> (1,2).
    await playMove(page, '1,1', '1,2');
    await expect(page.locator('.move-history .move-line')).toHaveCount(1);

    // AI (Red) replies.
    await waitForHumanTurn(page);
    await expect(page.locator('.move-history .move-line')).toHaveCount(2);

    // Move 2: blue dog (5,1) -> (5,2).
    await playMove(page, '5,1', '5,2');
    await expect(page.locator('.move-history .move-line')).toHaveCount(3);

    // AI replies again; the game must still be in progress.
    await waitForHumanTurn(page);
    await expect(page.locator('.move-history .move-line')).toHaveCount(4);
    await expect(page.locator('.result-overlay')).toHaveCount(0);

    // Selection / legal-move indicators work.
    await page.locator('.square[data-square="0,2"]').click(); // blue elephant
    await expect(page.locator('.highlight-selected')).toHaveCount(1);
    await expect(page.locator('.highlight-target')).not.toHaveCount(0);

    // Undo restores the position and keeps it the human's turn.
    await page.locator('button', { hasText: 'Undo' }).click();
    await expect(page.locator('.highlight-selected')).toHaveCount(0);
    await expect(page.locator('.move-history .move-line')).toHaveCount(2);
    await expect(page.locator('.status-text', { hasText: 'Blue to move' })).toBeVisible();
});
