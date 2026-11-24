// Shared helpers for transpiling/inspecting the Arena component in Node.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
export const jcoCli = path.join(root, 'node_modules', '@bytecodealliance', 'jco', 'dist', 'jco.js');

export function transpileComponent(componentPath, outDir, name, quiet = true) {
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  const start = performance.now();
  const result = spawnSync(
    process.execPath,
    [jcoCli, 'transpile', componentPath, '-o', outDir, '-I', 'sync', '--name', name, ...(quiet ? ['--quiet'] : [])],
    { cwd: root, encoding: 'utf8' },
  );
  const elapsedMs = performance.now() - start;
  if (result.status !== 0) {
    throw new Error(`jco transpile failed: ${result.stderr || result.stdout}`);
  }
  const jsPath = path.join(outDir, `${name}.js`);
  if (!existsSync(jsPath)) throw new Error(`jco did not emit ${jsPath}`);
  return { elapsedMs, jsPath, outDir };
}

export async function loadTranspiled(transpiled) {
  const module = await import(pathToFileURL(transpiled.jsPath).href);
  const cores = new Map();
  for (const file of readdirSync(transpiled.outDir).filter((f) => f.endsWith('.wasm'))) {
    cores.set(file, new WebAssembly.Module(readFileSync(path.join(transpiled.outDir, file))));
  }
  const getCoreModule = (p) => {
    const key = path.basename(String(p));
    const mod = cores.get(key);
    if (!mod) throw new Error(`missing core module ${key}`);
    return mod;
  };
  return {
    module,
    instantiate(display) {
      const imports = { 'jungle:arena/display': display };
      return module.instantiate(getCoreModule, imports);
    },
  };
}

export function makeDisplay() {
  const state = { calls: 0, frame: null };
  return {
    imports: {
      present(width, height, rgba) {
        state.calls += 1;
        state.frame = { width, height, bytes: Uint8Array.from(rgba) };
      },
    },
    reset() {
      state.calls = 0;
      state.frame = null;
    },
    get calls() {
      return state.calls;
    },
    get frame() {
      return state.frame;
    },
  };
}

export const EXPECTED = {
  width: 448,
  height: 576,
  bytes: 448 * 576 * 4,
  maxComponentBytes: 25_165_824,
  header: Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00]),
  rulesId: 'desktop-jungle-v2',
  rulesDigest: 'a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57',
};
