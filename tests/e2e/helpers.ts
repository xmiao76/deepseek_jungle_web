import { expect, type Page } from '@playwright/test';

export interface TestState {
  screen: 'menu' | 'game';
  settings: { mode: 'human' | 'aiai'; difficulty: 'easy' | 'medium' | 'hard'; first: string; muted: boolean };
  turn: 'red' | 'blue' | null;
  status: number | null;
  result: string | null;
  ply: number;
  thinking: boolean;
  paused: boolean;
  engineMode: 'worker' | 'main';
  selected: number | null;
  flipped: boolean;
  humanSide: 'red' | 'blue' | null;
  legal: Array<[number, number]>;
  board: number[];
  moves: Array<[number, number]>;
  latency: Array<{ requestId: number; requestedAt: number; repliedAt: number; committedAt: number; ms: number; difficulty: string }>;
}

export async function getState(page: Page): Promise<TestState> {
  return page.evaluate(() => (window as unknown as { __jungleTest: { state: () => TestState } }).__jungleTest.state());
}

export async function boot(page: Page, url = '/'): Promise<TestState> {
  await page.goto(url, { waitUntil: 'load' });
  await page.waitForFunction(() => (window as unknown as { __jungleReady?: boolean }).__jungleReady === true, null, { timeout: 60_000 });
  await expect(page.locator('[data-screen]')).toHaveCount(1);
  return getState(page);
}

export async function start(page: Page): Promise<void> {
  const s = await getState(page);
  if (s.screen === 'menu') {
    await page.click('[data-action="start"]');
  }
  await expect.poll(async () => (await getState(page)).screen, { timeout: 15_000 }).toBe('game');
}

export async function waitForPly(page: Page, minimum: number, timeout = 60_000): Promise<TestState> {
  await expect
    .poll(async () => (await getState(page)).ply, { timeout, intervals: [100, 200, 500] })
    .toBeGreaterThanOrEqual(minimum);
  return getState(page);
}

export async function waitUntilTerminal(page: Page, timeout = 240_000): Promise<TestState> {
  await expect
    .poll(
      async () => {
        const s = await getState(page);
        return s.status === 0 && s.ply < 400 ? 0 : 1;
      },
      { timeout, intervals: [200, 500, 1000] },
    )
    .toBe(1);
  return getState(page);
}

export function watchErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('pageerror', (error) => errors.push(error.message));
  return errors;
}

export async function choose(page: Page, from: number, to: number): Promise<void> {
  await page.evaluate(
    ([f, t]) => (window as unknown as { __jungleTest: { choose: (a: number, b: number) => Promise<void> } }).__jungleTest.choose(f, t),
    [from, to],
  );
}
