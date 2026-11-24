import { expect, test } from '@playwright/test';
import { boot, waitForPly, watchErrors } from './helpers';

test('hosted smoke — boot, autostart, two AI plies', async ({ page }) => {
  const errors = watchErrors(page);
  await boot(page, '/?mode=aiai&diff=easy&first=red&autostart=1');
  const state = await waitForPly(page, 2, 60_000);
  expect(state.screen).toBe('game');
  expect(['worker', 'main']).toContain(state.engineMode);
  expect(state.latency.length).toBeGreaterThanOrEqual(1);
  expect(errors).toEqual([]);
});
