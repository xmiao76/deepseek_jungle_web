// Arena admission checklist against the committed release file.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { EXPECTED, loadTranspiled, makeDisplay, root, transpileComponent } from './lib/components.mjs';
import { initial, legalMoves, toWire, toWireLegal } from '../tests/referee.mjs';

const releaseJson = JSON.parse(readFileSync(path.join(root, 'release', 'RELEASE.json'), 'utf8'));
const componentPath = path.join(root, 'release', releaseJson.filename);
if (!existsSync(componentPath)) throw new Error(`missing ${componentPath}; run npm run build`);
const bytes = readFileSync(componentPath);
const sha = createHash('sha256').update(bytes).digest('hex');
const rows = [];
const check = (name, fn) => {
  try {
    const detail = fn() ?? '';
    rows.push({ name, ok: true, detail: String(detail) });
    console.log(`PASS ${name}${detail ? ` — ${detail}` : ''}`);
  } catch (error) {
    rows.push({ name, ok: false, detail: String(error.message ?? error) });
    console.error(`FAIL ${name} — ${error.message ?? error}`);
    process.exitCode = 1;
  }
};

check('1. size <= 24 MiB', () => {
  assert.ok(bytes.length <= EXPECTED.maxComponentBytes, `${bytes.length} bytes`);
  return `${bytes.length} bytes`;
});
check('2. component preamble', () => {
  assert.ok(bytes.subarray(0, 8).equals(EXPECTED.header), 'missing component header');
  return '00 61 73 6d 0d 00 01 00';
});
check('3. SHA-256 identity', () => sha);

const wit = spawnSync('wasm-tools', ['component', 'wit', componentPath], { cwd: root, encoding: 'utf8', shell: process.platform === 'win32' });
check('4a. wasm-tools component wit', () => assert.equal(wit.status, 0) ?? 'ok');
const imports = [...String(wit.stdout ?? '').matchAll(/^\s*import\s+([^;]+);/gm)].map((m) => m[1].trim());
const exports = [...String(wit.stdout ?? '').matchAll(/^\s*export\s+([^;]+);/gm)].map((m) => m[1].trim());
check('4b. exact import/export contract', () => {
  assert.deepEqual(imports, ['jungle:arena/display@1.0.0']);
  assert.deepEqual(exports, ['jungle:arena/game@1.0.0']);
  return 'display in / game out';
});

const version = JSON.parse(readFileSync(path.join(root, 'node_modules/@bytecodealliance/jco-transpile/package.json'), 'utf8')).version;
const transpiled = transpileComponent(componentPath, path.join(root, 'build', 'admission'), 'release');
check('5. jco-transpile 0.24.0 < 30 s', () => {
  assert.equal(version, '0.24.0');
  assert.ok(transpiled.elapsedMs < 30_000, `${transpiled.elapsedMs.toFixed(0)} ms`);
  return `${transpiled.elapsedMs.toFixed(0)} ms`;
});

const loaded = await loadTranspiled(transpiled);
const display = makeDisplay();
const instance = loaded.instantiate(display.imports);
const info = instance.game.info();
check('6. metadata', () => {
  const fields = ['title', 'gameVersion', 'model', 'codeAgent', 'modelPlatform', 'githubUrl', 'engineVersion', 'runtime', 'rulesId', 'rulesDigest'];
  for (const field of fields) {
    assert.equal(typeof info[field], 'string', `${field} type`);
    assert.ok(info[field].length > 0, `${field} empty`);
    assert.ok(info[field].length <= 160, `${field} too long`);
    assert.ok(!/[\u0000-\u001f\u007f]/.test(info[field]), `${field} control characters`);
  }
  assert.equal(info.rulesId, EXPECTED.rulesId);
  assert.equal(info.rulesDigest, EXPECTED.rulesDigest);
  assert.ok(info.githubUrl.startsWith('https://github.com/'));
  assert.equal(info.title, releaseJson.title);
  assert.equal(info.gameVersion, releaseJson.version);
  return `${info.title} · ${info.gameVersion}`;
});

check('7. opening smoke + frames', () => {
  const state = initial('red');
  const legal = toWireLegal(legalMoves(state));
  assert.equal(legal.length, 24);
  instance.game.init({ seat: 'red', difficulty: 'medium', seed: 1 });
  const index = instance.game.chooseMove(toWire(state), legal, 1000);
  assert.ok(Number.isInteger(index) && index >= 0 && index < legal.length, `index ${index}`);
  for (const flipped of [false, true]) {
    display.reset();
    instance.game.render(toWire(state), flipped, 1000);
    assert.equal(display.calls, 1, 'present count');
    assert.equal(display.frame.width, 448);
    assert.equal(display.frame.height, 576);
    assert.equal(display.frame.bytes.length, 1_032_192);
    assert.ok(display.frame.bytes.every((value, i) => i % 4 !== 3 || value === 255), 'alpha');
  }
  return `index ${index}, 448×576 RGBA ×2`;
});

const md = `# Arena admission checklist

Release: \`${releaseJson.filename}\`
SHA-256: \`${sha}\`

| # | Check | Result | Detail |
|---|---|---|---|
${rows.map((r) => `| ${r.name} | ${r.ok ? 'PASS' : 'FAIL'} | ${r.detail.replace(/\|/g, '\\|')} |`).join('\n')}

Run with \`npm run admission\`.
`;
mkdirSync(path.join(root, 'docs'), { recursive: true });
writeFileSync(path.join(root, 'docs', 'ADMISSION.md'), md);
if (process.exitCode) {
  console.error('admission FAILED');
} else {
  console.log(`admission PASS: ${rows.length}/${rows.length} checks, sha256 ${sha}`);
}
