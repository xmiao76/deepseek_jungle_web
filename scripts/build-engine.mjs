// Builds the Rust artifacts and wraps the WIT components.
import { spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

const root = path.resolve(new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
const engine = path.join(root, 'engine');
const target = path.join(engine, 'target', 'wasm32-unknown-unknown', 'release');
const build = path.join(root, 'build');
mkdirSync(build, { recursive: true });

function run(cmd, args, cwd = root) {
  console.log(`> ${cmd} ${args.join(' ')}`);
  const result = spawnSync(cmd, args, { cwd, stdio: 'inherit', shell: process.platform === 'win32' });
  if (result.status !== 0) throw new Error(`${cmd} failed (${result.status})`);
}

function wasmTools(...args) {
  run('wasm-tools', args);
}

run('cargo', ['build', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'jungle-component', '-p', 'jungle-webengine', '-p', 'jungle-fixture'], engine);

for (const [core, out] of [
  ['jungle_component.wasm', 'jungle-component.wasm'],
  ['jungle_fixture.wasm', 'jungle-fixture.wasm'],
]) {
  wasmTools('component', 'new', path.join(target, core), '-o', path.join(build, out));
  wasmTools('validate', path.join(build, out));
}
copyFileSync(path.join(target, 'jungle_webengine.wasm'), path.join(build, 'jungle_webengine.wasm'));

const component = readFileSync(path.join(build, 'jungle-component.wasm'));
const header = component.subarray(0, 8);
const expected = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00]);
if (!header.equals(expected)) throw new Error('component preamble is not the Component Model header');
if (component.length > 25_165_824) throw new Error(`component is too large: ${component.length}`);

const wit = spawnSync('wasm-tools', ['component', 'wit', path.join(build, 'jungle-component.wasm')], {
  cwd: root,
  encoding: 'utf8',
  shell: process.platform === 'win32',
});
if (wit.status !== 0) throw new Error('wasm-tools component wit failed');
const witText = wit.stdout;
const imports = [...witText.matchAll(/^\s*import\s+([^;]+);/gm)].map((m) => m[1].trim());
const exports = [...witText.matchAll(/^\s*export\s+([^;]+);/gm)].map((m) => m[1].trim());
if (imports.length !== 1 || imports[0] !== 'jungle:arena/display@1.0.0') {
  throw new Error(`unexpected component imports: ${JSON.stringify(imports)}`);
}
if (exports.length !== 1 || exports[0] !== 'jungle:arena/game@1.0.0') {
  throw new Error(`unexpected component exports: ${JSON.stringify(exports)}`);
}

const webengine = path.join(build, 'jungle_webengine.wasm');
if (!existsSync(webengine)) throw new Error('standalone wasm missing');
console.log(
  `engine OK: component ${(component.length / 1024 / 1024).toFixed(2)} MiB, standalone ${(statSync(webengine).size / 1024).toFixed(0)} KiB`,
);
