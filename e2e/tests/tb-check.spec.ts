import { test, expect } from '@playwright/test';

// The browser Rust engine must load the 2+3-piece tablebase asset (a gzip
// asset decompressed in the worker and handed to the wasm probe), so endgames
// resolve exactly instead of degrading to pure search.
test('browser Rust engine loads the tablebase', async ({ page }) => {
    await page.goto('/');
    await page.waitForTimeout(500);
    const out = await page.evaluate(async () => {
        const worker = new Worker('js/rust-engine-worker.js');
        const result = await new Promise((resolve) => {
            worker.onmessage = (e) => resolve(e.data);
            worker.onerror = (e) => resolve({ id: 0, error: 'workererror:' + e.message });
            worker.postMessage({
                id: 1,
                // Start position (Blue to move).
                hex: '0b00000000000d000300000005000f00070009000100000000000000000000000000000000000000000002000a00080010000600000004000e00000000000c',
                turn: 0, history: '', budgetMs: 500, maxDepth: 32,
            });
        });
        worker.terminate();
        return result;
    });
    expect(out.error).toBeUndefined();
    expect(out.tbLoaded).toBe(true);
    expect(out.nodes).toBeGreaterThan(0);
    expect(out.noMove).toBe(false);
});
