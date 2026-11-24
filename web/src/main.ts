import { JungleApp } from './app';
import { EngineClient } from './engine';
import { IDENTITY } from './identity.generated';
import { SIDE_RED, type WireSide } from './protocol';
import { decodeBase64 } from './wasm';

declare global {
  interface Window {
    __jungleReady?: boolean;
    __jungleBootError?: string;
  }
}

const root = document.querySelector<HTMLElement>('#app');
const bootStatus = document.querySelector<HTMLElement>('#boot-status');
const bootBar = document.querySelector<HTMLElement>('#boot-bar');
const bootNote = document.querySelector<HTMLElement>('#boot-note');

function progress(pct: number, status: string): void {
  if (bootBar) bootBar.style.width = `${Math.max(0, Math.min(100, pct))}%`;
  if (bootStatus) bootStatus.textContent = status;
}

async function loadWasm(): Promise<Uint8Array> {
  const embedded = window.__JUNGLE_EMBEDDED__?.wasmBase64;
  if (embedded) {
    progress(25, 'Unpacking the engine…');
    const bytes = decodeBase64(embedded);
    progress(55, 'Compiling WebAssembly…');
    return bytes;
  }
  const url = new URL('assets/jungle_webengine.wasm', document.baseURI);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`engine download failed (${response.status})`);
  const total = Number(response.headers.get('content-length') ?? 0);
  if (!response.body) return new Uint8Array(await response.arrayBuffer());
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let received = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    if (value) {
      chunks.push(value);
      received += value.length;
      if (total > 0) progress(10 + Math.round((received / total) * 55), `Loading the engine… ${Math.round((received / total) * 100)}%`);
    }
  }
  const bytes = new Uint8Array(received);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.length;
  }
  return bytes;
}

async function applyQueryPresets(app: JungleApp): Promise<void> {
  const params = new URLSearchParams(location.search);
  const patch: Record<string, string> = {};
  const mode = params.get('mode');
  if (mode === 'human' || mode === 'aiai') patch.mode = mode;
  const diff = params.get('diff') ?? params.get('difficulty');
  if (diff === 'easy' || diff === 'medium' || diff === 'hard') patch.difficulty = diff;
  const first = params.get('first');
  if (first === 'human' || first === 'ai' || first === 'red' || first === 'blue') patch.first = first;
  if (Object.keys(patch).length) {
    (window as unknown as { __jungleTest: { setSettings: (p: unknown) => void } }).__jungleTest.setSettings(patch);
  }
  if (params.get('autostart') === '1') {
    await (window as unknown as { __jungleTest: { start: () => Promise<void> } }).__jungleTest.start();
  }
}

async function boot(): Promise<void> {
  try {
    if (!root) throw new Error('missing #app');
    progress(5, 'Preparing the board…');
    await document.fonts.ready.catch(() => undefined);
    const bytes = await loadWasm();
    progress(65, 'Waking the engine…');
    const engine = new EngineClient();
    const forceLocal = new URLSearchParams(location.search).get('engine') === 'main';
    const opening = await engine.init(bytes, SIDE_RED as WireSide, 1, forceLocal);
    progress(90, 'Setting the pieces…');
    const app = new JungleApp(root, engine, opening);
    progress(100, 'Ready.');
    await applyQueryPresets(app);
    window.__jungleReady = true;
    document.documentElement.dataset.ready = 'true';
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    window.__jungleBootError = message;
    if (bootStatus) bootStatus.textContent = `The jungle could not open: ${message}`;
    if (bootBar) bootBar.classList.add('is-error');
    if (bootNote) bootNote.textContent = 'Reload the page to try again.';
    console.error(error);
  }
}

void boot();

// Keep the identity visible in the boot overlay and the document title.
const bootTitle = document.querySelector<HTMLElement>('#boot-title');
if (bootTitle) bootTitle.textContent = IDENTITY.title;
document.title = `Jungle — ${IDENTITY.title}`;
