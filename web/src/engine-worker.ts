// Web Worker hosting the standalone wasm engine. State and rendering stay off
// the UI thread; the main thread moves the authoritative ply list across.
import { WasmEngine } from './wasm';
import type { Move, RenderOptions, WireSide } from './protocol';

interface InitRequest {
  id: number;
  op: 'init';
  wasm: ArrayBuffer;
  first: WireSide;
  seed: number;
}
interface NewRequest {
  id: number;
  op: 'new';
  first: WireSide;
  seed: number;
}
interface LoadRequest {
  id: number;
  op: 'load';
  moves: Move[];
  first: WireSide;
}
interface ApplyRequest {
  id: number;
  op: 'apply';
  from: number;
  to: number;
}
interface SearchRequest {
  id: number;
  op: 'search';
  nodes: number;
  ms: number;
  depth: number;
}
interface RenderRequest {
  id: number;
  op: 'render';
  options: RenderOptions;
}
interface SnapshotRequest {
  id: number;
  op: 'snapshot';
}
type Request = InitRequest | NewRequest | LoadRequest | ApplyRequest | SearchRequest | RenderRequest | SnapshotRequest;

let engine: WasmEngine | null = null;

function reply(id: number, payload: Record<string, unknown>, transfer: Transferable[] = []) {
  (self as unknown as { postMessage: (msg: unknown, transfer?: Transferable[]) => void }).postMessage(
    { id, ok: true, ...payload },
    transfer,
  );
}

self.onmessage = async (event: MessageEvent<Request>) => {
  const msg = event.data;
  try {
    if (msg.op === 'init') {
      engine = await WasmEngine.load(msg.wasm);
      const snapshot = engine.newGame(msg.first, msg.seed);
      reply(msg.id, { snapshot });
      return;
    }
    if (!engine) throw new Error('engine not initialized');
    switch (msg.op) {
      case 'new': {
        reply(msg.id, { snapshot: engine.newGame(msg.first, msg.seed) });
        break;
      }
      case 'load': {
        reply(msg.id, { snapshot: engine.load(msg.moves, msg.first) });
        break;
      }
      case 'apply': {
        const code = engine.apply(msg.from, msg.to);
        reply(msg.id, { code, snapshot: engine.snapshot() });
        break;
      }
      case 'search': {
        reply(msg.id, { result: engine.search(msg.nodes, msg.ms, msg.depth) });
        break;
      }
      case 'render': {
        const frame = await engine.render(msg.options);
        if (frame.bitmap) {
          reply(msg.id, { frame: { bitmap: frame.bitmap, width: frame.width, height: frame.height } }, [frame.bitmap]);
        } else {
          reply(msg.id, {
            frame: { data: frame.data, width: frame.width, height: frame.height },
          });
        }
        break;
      }
      case 'snapshot': {
        reply(msg.id, { snapshot: engine.snapshot() });
        break;
      }
    }
  } catch (error) {
    (self as unknown as { postMessage: (msg: unknown) => void }).postMessage({
      id: msg.id,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    });
  }
};
