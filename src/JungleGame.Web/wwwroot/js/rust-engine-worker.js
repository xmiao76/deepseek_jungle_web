// Web worker hosting the Rust search engine (rust-engine.wasm). Runs the
// CPU-bound search off the main thread so the UI stays responsive. Protocol:
//   in : { id, hex (126 chars), turn (0|1), history (hex string of 16-char
//         u64s, may be ""), budgetMs, maxDepth }
//   out: { id, nodes, depth, from, to, noMove } or { id, error }
let instance = null;
let tbLoaded = false;

async function load() {
  if (instance) return instance;
  const imports = {
    env: { now_ms: () => performance.now() },
  };
  const result = await WebAssembly.instantiateStreaming(
    fetch(new URL("rust-engine.wasm", self.location.href)),
    imports,
  );
  instance = result.instance;
  return instance;
}

// Loads the 2+3-piece tablebase (gzip asset, decompressed in JS) once.
async function loadTablebase(inst) {
  if (tbLoaded) return;
  try {
    const resp = await fetch(new URL("tablebases/jungle3.tb.gz", self.location.href));
    if (!resp.ok) return;
    const gz = await resp.arrayBuffer();
    const decompressed = new DecompressionStream("gzip");
    const stream = new Response(new Blob([gz]).stream().pipeThrough(decompressed)).arrayBuffer();
    const bytes = new Uint8Array(await stream);

    // Allocate space in wasm memory and copy the bytes in.
    const mem = inst.exports.memory;
    const ptr = inst.exports.alloc(bytes.length);
    new Uint8Array(mem.buffer, ptr, bytes.length).set(bytes);
    const ok = inst.exports.load_tablebase(ptr, bytes.length);
    tbLoaded = ok === 1;
  } catch {
    tbLoaded = false; // degrade to pure search
  }
}

function parseHistory(hex) {
  if (!hex) return new BigInt64Array(0);
  const count = Math.floor(hex.length / 16);
  const arr = new BigInt64Array(count);
  for (let i = 0; i < count; i++) {
    arr[i] = BigInt("0x" + hex.slice(i * 16, i * 16 + 16));
  }
  return arr;
}

onmessage = async (e) => {
  const { id, hex, turn, history, budgetMs, maxDepth } = e.data;
  try {
    const inst = await load();
    await loadTablebase(inst);
    const mem = new Uint8Array(inst.exports.memory.buffer);

    const hexBytes = new TextEncoder().encode(hex);
    const hexPtr = inst.exports.hex_buf();
    mem.set(hexBytes, hexPtr);

    const historyArr = parseHistory(history);
    let historyLen = 0;
    if (historyArr.length > 0) {
      const histPtr = inst.exports.history_buf();
      const histMem = new BigInt64Array(inst.exports.memory.buffer, histPtr, historyArr.length);
      histMem.set(historyArr);
      historyLen = historyArr.length;
    }

    const encoded = inst.exports.search_timed(turn, historyLen, budgetMs, maxDepth);
    const b = BigInt.asUintN(64, encoded);
    postMessage({
      id,
      nodes: Number(b >> 32n),
      depth: Number((b >> 24n) & 0xffn),
      from: Number((b >> 16n) & 0xffn),
      to: Number((b >> 8n) & 0xffn),
      noMove: Number(b & 1n) === 1,
    });
  } catch (err) {
    postMessage({ id, error: String(err) });
  }
};
