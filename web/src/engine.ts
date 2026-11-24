// Engine facade: a Web Worker when available, otherwise the same wasm on the
// main thread. The main thread owns the authoritative ply list.
import { WasmEngine } from './wasm';
import type { AiResult, Frame, Move, RenderOptions, Snapshot, WireSide } from './protocol';

declare global {
  interface Window {
    __JUNGLE_WORKER_URL__?: string;
    __JUNGLE_EMBEDDED__?: {
      wasmBase64?: string;
      componentBase64?: string;
      sha256?: string;
      filename?: string;
    };
  }
}

type Reply = {
  id: number;
  ok: boolean;
  snapshot?: Snapshot;
  result?: AiResult;
  frame?: Frame;
  code?: number;
  error?: string;
};

export class EngineClient {
  mode: 'worker' | 'main' = 'main';
  private worker: Worker | null = null;
  private local: WasmEngine | null = null;
  private bytes: Uint8Array | null = null;
  private pending = new Map<number, { resolve: (r: Reply) => void; reject: (e: Error) => void }>();
  private nextId = 1;

  async init(bytes: Uint8Array, first: WireSide, seed: number, forceLocal = false): Promise<Snapshot> {
    this.bytes = bytes.slice();
    if (!forceLocal) {
      try {
        await this.startWorker(first, seed);
        return await this.snapshot();
      } catch (error) {
        console.warn('worker engine unavailable; using main-thread fallback', error);
        this.terminateWorker();
      }
    }
    this.local = await WasmEngine.load(this.bytes);
    this.mode = 'main';
    return this.local.newGame(first, seed);
  }

  private async startWorker(first: WireSide, seed: number): Promise<void> {
    if (!this.bytes) throw new Error('engine bytes missing');
    const source = document.getElementById('worker-source')?.textContent ?? undefined;
    const url = source
      ? URL.createObjectURL(new Blob([source], { type: 'text/javascript' }))
      : window.__JUNGLE_WORKER_URL__;
    if (!url) throw new Error('no worker source or URL');
    const worker = new Worker(url);
    this.worker = worker;
    worker.onmessage = (event: MessageEvent<Reply>) => this.onMessage(event.data);
    worker.onerror = (event) => this.failAll(new Error(`worker error: ${event.message || 'unknown'}`));
    worker.onmessageerror = () => this.failAll(new Error('worker message error'));
    this.mode = 'worker';
    const buffer = this.bytes.slice().buffer;
    await this.request({ op: 'init', wasm: buffer, first, seed }, [buffer], 15_000);
  }

  private onMessage(reply: Reply) {
    const pending = this.pending.get(reply.id);
    if (!pending) return;
    this.pending.delete(reply.id);
    if (!reply.ok) pending.reject(new Error(reply.error ?? 'engine error'));
    else pending.resolve(reply);
  }

  private failAll(error: Error) {
    for (const [, p] of this.pending) p.reject(error);
    this.pending.clear();
  }

  private request(payload: Record<string, unknown>, transfer: Transferable[] = [], timeoutMs = 15_000): Promise<Reply> {
    const worker = this.worker;
    if (!worker) return Promise.reject(new Error('worker not running'));
    const id = this.nextId++;
    return new Promise<Reply>((resolve, reject) => {
      const timer = window.setTimeout(() => {
        this.pending.delete(id);
        reject(new Error('engine request timed out'));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (r) => {
          clearTimeout(timer);
          resolve(r);
        },
        reject: (e) => {
          clearTimeout(timer);
          reject(e);
        },
      });
      worker.postMessage({ id, ...payload }, transfer);
    });
  }

  async snapshot(): Promise<Snapshot> {
    if (this.mode === 'main') return this.requireLocal().snapshot();
    const reply = await this.request({ op: 'snapshot' });
    return reply.snapshot!;
  }

  async newGame(first: WireSide, seed: number): Promise<Snapshot> {
    if (this.mode === 'main') return this.requireLocal().newGame(first, seed);
    const reply = await this.request({ op: 'new', first, seed });
    return reply.snapshot!;
  }

  async load(moves: Move[], first: WireSide): Promise<Snapshot> {
    if (this.mode === 'main') return this.requireLocal().load(moves, first);
    const reply = await this.request({ op: 'load', moves, first });
    return reply.snapshot!;
  }

  async apply(from: number, to: number): Promise<{ code: number; snapshot: Snapshot }> {
    if (this.mode === 'main') {
      const local = this.requireLocal();
      const code = local.apply(from, to);
      return { code, snapshot: local.snapshot() };
    }
    const reply = await this.request({ op: 'apply', from, to });
    return { code: reply.code ?? 0, snapshot: reply.snapshot! };
  }

  async search(nodes: number, ms: number, depth: number): Promise<AiResult> {
    if (this.mode === 'main') return this.requireLocal().search(nodes, ms, depth);
    const reply = await this.request({ op: 'search', nodes, ms, depth }, [], Math.max(10_000, ms + 8_000));
    return reply.result!;
  }

  async render(options: RenderOptions): Promise<Frame> {
    if (this.mode === 'main') return this.requireLocal().render(options);
    const reply = await this.request({ op: 'render', options }, [], 10_000);
    return reply.frame!;
  }

  /** Restarts the worker (or reuses the local engine), then replays `moves`. */
  async restart(moves: Move[], first: WireSide): Promise<Snapshot> {
    if (this.mode === 'worker') {
      this.terminateWorker();
      // Seed is irrelevant to replay; keep 0.
      await this.startWorker(first, 0);
      return this.load(moves, first);
    }
    const local = this.requireLocal();
    local.newGame(first, 0);
    return local.load(moves, first);
  }

  /** Cancels any in-flight search and starts a fresh game. */
  async reset(first: WireSide, seed = 0): Promise<Snapshot> {
    if (this.mode === 'worker') {
      this.terminateWorker();
      await this.startWorker(first, seed);
      return this.snapshot();
    }
    return this.requireLocal().newGame(first, seed);
  }

  terminate(): void {
    if (this.mode === 'worker') this.terminateWorker();
  }

  private terminateWorker() {
    this.failAll(new Error('engine restarted'));
    this.worker?.terminate();
    this.worker = null;
  }

  private requireLocal(): WasmEngine {
    if (!this.local) throw new Error('main-thread engine not initialized');
    return this.local;
  }

  async fallback(moves: Move[], first: WireSide): Promise<Snapshot> {
    this.terminateWorker();
    if (!this.local) {
      if (!this.bytes) throw new Error('engine bytes missing');
      this.local = await WasmEngine.load(this.bytes);
    }
    this.mode = 'main';
    this.local.newGame(first, 0);
    return this.local.load(moves, first);
  }
}
