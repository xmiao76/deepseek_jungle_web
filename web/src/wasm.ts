// Thin typed wrapper around the standalone wasm exports.
import { parseSnapshot, type AiResult, type Frame, type Move, type RenderOptions, type Snapshot, type WireSide } from './protocol';

interface WasmExports {
  memory: WebAssembly.Memory;
  jg_alloc(len: number): number;
  jg_dealloc(ptr: number, len: number): void;
  jg_new(first: number, seed: number): number;
  jg_load(ptr: number, count: number, first: number): number;
  jg_apply(from: number, to: number): number;
  jg_search(nodes: bigint, ms: number, maxDepth: number): number;
  jg_ai_nodes(): bigint;
  jg_ai_depth(): number;
  jg_ai_score(): number;
  jg_ai_ms(): number;
  jg_ai_aborted(): number;
  jg_snapshot_len(): number;
  jg_snapshot(ptr: number, cap: number): number;
  jg_render(scale: number, flipped: number, progress: number, selected: number, hover: number, cursor: number, ptr: number, cap: number): number;
  jg_frame_len(scale: number): number;
  jg_rules_id_ptr(): number;
  jg_rules_id_len(): number;
  jg_rules_digest_ptr(): number;
  jg_rules_digest_len(): number;
}

export class WasmEngine {
  private constructor(private readonly e: WasmExports) {}

  static async load(bytes: ArrayBuffer | Uint8Array): Promise<WasmEngine> {
    const imports = { env: { now_ms: () => performance.now() } };
    const source = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    const result = await WebAssembly.instantiate(source as unknown as BufferSource, imports);
    const instance = (result as unknown as { instance: WebAssembly.Instance }).instance;
    return new WasmEngine(instance.exports as unknown as WasmExports);
  }

  newGame(first: WireSide, seed: number): Snapshot {
    this.e.jg_new(first, seed >>> 0);
    return this.snapshot();
  }

  load(moves: Move[], first: WireSide): Snapshot {
    const count = moves.length;
    if (count === 0) {
      this.e.jg_load(0, 0, first);
      return this.snapshot();
    }
    const bytes = count * 2;
    const ptr = this.e.jg_alloc(bytes);
    const mem = new Uint8Array(this.e.memory.buffer, ptr, bytes);
    const view = new DataView(mem.buffer, mem.byteOffset, bytes);
    for (let i = 0; i < count; i++) {
      view.setUint16(i * 2, ((moves[i][0] << 8) | moves[i][1]) & 0xffff, true);
    }
    const code = this.e.jg_load(ptr, count, first);
    this.e.jg_dealloc(ptr, bytes);
    if (code !== 0) throw new Error(`load failed (${code})`);
    return this.snapshot();
  }

  apply(from: number, to: number): number {
    return this.e.jg_apply(from, to);
  }

  search(nodeBudget: number, timeBudgetMs: number, maxDepth: number): AiResult {
    const start = performance.now();
    const packed = this.e.jg_search(BigInt(Math.max(0, Math.floor(nodeBudget))), timeBudgetMs, maxDepth);
    const wallMs = performance.now() - start;
    return {
      from: (packed >> 8) & 0xff,
      to: packed & 0xff,
      depth: this.e.jg_ai_depth(),
      nodes: Number(this.e.jg_ai_nodes()),
      score: this.e.jg_ai_score(),
      elapsedMs: this.e.jg_ai_ms(),
      aborted: this.e.jg_ai_aborted() !== 0,
      wallMs,
    };
  }

  snapshot(): Snapshot {
    const len = this.e.jg_snapshot_len();
    const ptr = this.e.jg_alloc(len);
    const written = this.e.jg_snapshot(ptr, len);
    const copy = new Uint8Array(this.e.memory.buffer, ptr, written).slice();
    this.e.jg_dealloc(ptr, len);
    return parseSnapshot(copy.buffer);
  }

  async render(opts: RenderOptions): Promise<Frame> {
    const scale = Math.max(1, Math.min(4, Math.round(opts.scale)));
    const len = this.e.jg_frame_len(scale);
    const ptr = this.e.jg_alloc(len);
    const written = this.e.jg_render(
      scale,
      opts.flipped ? 1 : 0,
      Math.max(0, Math.min(1000, Math.round(opts.progress))),
      opts.selected ?? -1,
      opts.hover ?? -1,
      opts.cursor ?? -1,
      ptr,
      len,
    );
    if (written === 0 || written > len) {
      this.e.jg_dealloc(ptr, len);
      throw new Error('render failed');
    }
    const copy = new Uint8ClampedArray(this.e.memory.buffer, ptr, written).slice();
    this.e.jg_dealloc(ptr, len);
    const width = 448 * scale;
    const height = 576 * scale;
    if (typeof createImageBitmap === 'function') {
      try {
        const clamped = copy as Uint8ClampedArray<ArrayBuffer>;
        const bitmap = await createImageBitmap(new ImageData(clamped, width, height));
        return { bitmap, width, height };
      } catch {
        // fall through to the raw buffer path
      }
    }
    return { data: copy, width, height };
  }

  rules(): { id: string; digest: string } {
    const dec = new TextDecoder();
    const id = dec.decode(new Uint8Array(this.e.memory.buffer, this.e.jg_rules_id_ptr(), this.e.jg_rules_id_len()));
    const digest = dec.decode(new Uint8Array(this.e.memory.buffer, this.e.jg_rules_digest_ptr(), this.e.jg_rules_digest_len()));
    return { id, digest };
  }
}

export function decodeBase64(input: string): Uint8Array {
  const clean = input.replace(/\s+/g, '');
  const padding = clean.endsWith('==') ? 2 : clean.endsWith('=') ? 1 : 0;
  const out = new Uint8Array((clean.length / 4) * 3 - padding);
  let o = 0;
  for (let i = 0; i < clean.length; i += 4) {
    const a = b64(clean.charCodeAt(i));
    const b = b64(clean.charCodeAt(i + 1));
    const c = b64(clean.charCodeAt(i + 2));
    const d = b64(clean.charCodeAt(i + 3));
    if (o < out.length) out[o++] = (a << 2) | (b >> 4);
    if (o < out.length) out[o++] = ((b & 15) << 4) | (c >> 2);
    if (o < out.length) out[o++] = ((c & 3) << 6) | d;
  }
  return out;
}

function b64(code: number): number {
  if (code >= 65 && code <= 90) return code - 65;
  if (code >= 97 && code <= 122) return code - 97 + 26;
  if (code >= 48 && code <= 57) return code - 48 + 52;
  if (code === 43) return 62;
  if (code === 47) return 63;
  return 0;
}
