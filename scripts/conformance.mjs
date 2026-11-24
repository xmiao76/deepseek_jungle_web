// Arena conformance runner: admission checklist, deterministic smoke tests and
// complete matches against the fixture and itself, using the independent JS
// referee. Writes docs/CONFORMANCE.md and Arena frames.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
  EXPECTED,
  jcoCli,
  loadTranspiled,
  makeDisplay,
  root,
  transpileComponent,
} from './lib/components.mjs';
import { applyMove, initial, legalMoves, toWire, toWireLegal } from '../tests/referee.mjs';
import { spawnSync } from 'node:child_process';
import { writePng } from './lib/png.mjs';

const docs = path.join(root, 'docs');
const shots = path.join(docs, 'screenshots');
mkdirSync(shots, { recursive: true });
const releaseJson = JSON.parse(readFileSync(path.join(root, 'release', 'RELEASE.json'), 'utf8'));
const componentPath = path.join(root, 'release', releaseJson.filename);
if (!existsSync(componentPath)) throw new Error(`missing ${componentPath}; run npm run build`);
const fixturePath = path.join(root, 'build', 'jungle-fixture.wasm');

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const componentBytes = readFileSync(componentPath);
const checks = [];
const record = (name, ok, detail = '') => {
  checks.push({ name, ok, detail });
  if (!ok) throw new Error(`${name}: ${detail}`);
  console.log(`PASS ${name}${detail ? ` — ${detail}` : ''}`);
};

// ---- Admission: file, header, size, hash ----------------------------------
record('1. size <= 24 MiB', componentBytes.length <= EXPECTED.maxComponentBytes, `${componentBytes.length} bytes`);
record('2. component preamble', componentBytes.subarray(0, 8).equals(EXPECTED.header));
record('3. SHA-256 identity', componentBytes.length > 0, sha256(componentBytes));

const wit = spawnSync('wasm-tools', ['component', 'wit', componentPath], {
  cwd: root,
  encoding: 'utf8',
  shell: process.platform === 'win32',
});
record('4a. wasm-tools component wit', wit.status === 0);
const imports = [...wit.stdout.matchAll(/^\s*import\s+([^;]+);/gm)].map((m) => m[1].trim());
const exports = [...wit.stdout.matchAll(/^\s*export\s+([^;]+);/gm)].map((m) => m[1].trim());
record(
  '4b. contract imports/exports',
  imports.length === 1 && imports[0] === 'jungle:arena/display@1.0.0' && exports.length === 1 && exports[0] === 'jungle:arena/game@1.0.0',
  `${imports.join(', ')} / ${exports.join(', ')}`,
);

// ---- Transpilation --------------------------------------------------------
const outDir = path.join(root, 'build', 'conformance-release');
const transpiled = transpileComponent(componentPath, outDir, 'release');
record('5. jco transpile < 30 s', transpiled.elapsedMs < 30_000, `${transpiled.elapsedMs.toFixed(0)} ms`);
const transpileVersion = JSON.parse(readFileSync(path.join(root, 'node_modules/@bytecodealliance/jco-transpile/package.json'), 'utf8')).version;
record('5b. jco-transpile 0.24.0', transpileVersion === '0.24.0', transpileVersion);

const fixtureOut = path.join(root, 'build', 'conformance-fixture');
const fixtureTranspiled = transpileComponent(fixturePath, fixtureOut, 'fixture');
const release = await loadTranspiled(transpiled);
const fixture = await loadTranspiled(fixtureTranspiled);

function newInstance(display) {
  return release.instantiate(display.imports);
}
function newFixture(display) {
  return fixture.instantiate(display.imports);
}

// ---- Metadata + smoke -----------------------------------------------------
const metaDisplay = makeDisplay();
const metaInstance = newInstance(metaDisplay);
const info = metaInstance.game.info();
const fields = ['title', 'gameVersion', 'model', 'codeAgent', 'modelPlatform', 'githubUrl', 'engineVersion', 'runtime', 'rulesId', 'rulesDigest'];
record('6a. metadata fields present', fields.every((f) => typeof info[f] === 'string' && info[f].length > 0));
record('6b. metadata length <= 160', fields.every((f) => info[f].length <= 160));
record('6c. rules identity exact', info.rulesId === EXPECTED.rulesId && info.rulesDigest === EXPECTED.rulesDigest);
record('6d. github url', info.githubUrl.startsWith('https://github.com/'));

const smokeState = initial('red');
const smokeLegal = toWireLegal(legalMoves(smokeState));
metaInstance.game.init({ seat: 'red', difficulty: 'medium', seed: 1 });
const smokeIndex = metaInstance.game.chooseMove(toWire(smokeState), smokeLegal, 1000);
record('7a. choose-move opening index', Number.isInteger(smokeIndex) && smokeIndex >= 0 && smokeIndex < smokeLegal.length, `index ${smokeIndex}`);
for (const flipped of [false, true]) {
  metaDisplay.reset();
  metaInstance.game.render(toWire(smokeState), flipped, 1000);
  const frame = metaDisplay.frame;
  record(
    `7b. render ${flipped ? 'flipped' : 'unflipped'} frame`,
    metaDisplay.calls === 1 && frame.width === EXPECTED.width && frame.height === EXPECTED.height && frame.bytes.length === EXPECTED.bytes,
    `${metaDisplay.calls} present, ${frame?.width}x${frame?.height}, ${frame?.bytes.length} bytes`,
  );
  record(`7c. frame ${flipped ? 'flipped' : 'unflipped'} opaque`, frame.bytes.every((_, i) => i % 4 !== 3 || frame.bytes[i] === 255));
}

// ---- Frame checks ---------------------------------------------------------
function renderFrame(instance, display, state, flipped) {
  display.reset();
  const start = performance.now();
  instance.game.render(toWire(state), flipped, 1000);
  const ms = performance.now() - start;
  assert.equal(display.calls, 1, 'render must call present exactly once');
  const frame = display.frame;
  assert.equal(frame.width, EXPECTED.width);
  assert.equal(frame.height, EXPECTED.height);
  assert.equal(frame.bytes.length, EXPECTED.bytes);
  assert.ok(ms < 2000, `render took ${ms.toFixed(0)}ms`);
  return { ...frame, ms };
}

const hashFrame = (frame) => sha256(Buffer.from(frame.bytes));
const opening = initial('red');
const openDisplay = makeDisplay();
const openInstance = newInstance(openDisplay);
openInstance.game.init({ seat: 'red', difficulty: 'medium', seed: 20261120 });
const openA = renderFrame(openInstance, openDisplay, opening, false);
const openB = renderFrame(openInstance, openDisplay, opening, false);
const openFlip = renderFrame(openInstance, openDisplay, opening, true);
record('8a. unflipped render is deterministic', hashFrame(openA) === hashFrame(openB));
record('8b. flip changes only the view', hashFrame(openA) !== hashFrame(openFlip));
const fixtureDisplay = makeDisplay();
const fixtureInstance = newFixture(fixtureDisplay);
fixtureInstance.game.init({ seat: 'blue', difficulty: 'medium', seed: 20261120 });
const fixtureOpen = renderFrame(fixtureInstance, fixtureDisplay, opening, false);
record('8c. fixture draws a distinct board', hashFrame(fixtureOpen) !== hashFrame(openA));

writePng(path.join(shots, 'arena-opening.png'), openA);
writePng(path.join(shots, 'arena-opening-flipped.png'), openFlip);
writePng(path.join(shots, 'arena-fixture.png'), fixtureOpen);

// ---- Determinism / rendering does not mutate strategy ---------------------
function freshRelease(seat) {
  const display = makeDisplay();
  const instance = newInstance(display);
  instance.game.init({ seat, difficulty: 'hard', seed: 20261120 });
  return { instance, display };
}
const d1 = freshRelease('red');
const d2 = freshRelease('red');
const legalWire = toWireLegal(legalMoves(opening));
const pick1 = d1.instance.game.chooseMove(toWire(opening), legalWire, 20_000);
renderFrame(d1.instance, d1.display, opening, false);
const pick2 = d1.instance.game.chooseMove(toWire(opening), legalWire, 20_000);
const pick3 = d2.instance.game.chooseMove(toWire(opening), legalWire, 20_000);
record('9a. choose-move stable after renders', pick1 === pick2, `${pick1} vs ${pick2}`);
record('9b. fresh instances agree', pick1 === pick3, `${pick1} vs ${pick3}`);

// ---- Complete matches -----------------------------------------------------
const REASONS = new Set(['den', 'elimination', 'immobilization', 'repetition', 'no-capture']);
const matches = [];

function makeSeat(instance, display, seat, seed = 20261120) {
  instance.game.init({ seat, difficulty: 'medium', seed });
  return { instance, display, seat };
}

function renderBoth(seats, state) {
  const frames = {};
  for (const key of ['red', 'blue']) {
    const seat = seats[key];
    frames[key] = renderFrame(seat.instance, seat.display, state, key === 'blue');
  }
  return frames;
}

function playMatch(label, redFactory, blueFactory) {
  const redDisplay = makeDisplay();
  const blueDisplay = makeDisplay();
  const redInstance = redFactory(redDisplay);
  const blueInstance = blueFactory(blueDisplay);
  const seats = {
    red: makeSeat(redInstance, redDisplay, 'red'),
    blue: makeSeat(blueInstance, blueDisplay, 'blue'),
  };
  let state = initial('red');
  let maxMoveMs = 0;
  let maxRenderMs = 0;
  const revisions = [];
  const startingFrames = renderBoth(seats, state);
  revisions.push({ rev: state.rev, frames: startingFrames });
  maxRenderMs = Math.max(maxRenderMs, startingFrames.red.ms, startingFrames.blue.ms);
  let plies = 0;
  while (state.status === 'playing' && plies < 400) {
    const legal = legalMoves(state);
    assert.ok(legal.length > 0);
    const active = seats[state.turn];
    const wire = toWire(state);
    const start = performance.now();
    const idx = active.instance.game.chooseMove(wire, toWireLegal(legal), 20_000);
    const ms = performance.now() - start;
    maxMoveMs = Math.max(maxMoveMs, ms);
    assert.ok(Number.isInteger(idx) && idx >= 0 && idx < legal.length, `${label}: index ${idx}/${legal.length}`);
    assert.ok(ms < 5000, `${label}: choose-move ${ms.toFixed(0)}ms`);
    const chosen = legal[idx];
    const next = applyMove(state, chosen.src, chosen.dst);
    assert.ok(next, `${label}: referee rejected component move`);
    state = next;
    plies++;
    const frames = renderBoth(seats, state);
    maxRenderMs = Math.max(maxRenderMs, frames.red.ms, frames.blue.ms);
    revisions.push({ rev: state.rev, frames });
  }
  assert.equal(state.status === 'playing', false, `${label}: match did not finish`);
  assert.ok(REASONS.has(state.reason), `${label}: reason ${state.reason}`);
  assert.ok(state.reason !== '' && state.winner !== undefined);
  const result = {
    label,
    plies,
    status: state.status,
    winner: state.winner,
    reason: state.reason,
    maxMoveMs,
    maxRenderMs,
    revisions: revisions.length,
    final: revisions.at(-1).frames,
  };
  matches.push(result);
  writePng(path.join(shots, `arena-${label.replace(/\s+/g, '-').toLowerCase()}.png`), result.final.red);
  return result;
}

const releaseFactory = (display) => newInstance(display);
const fixtureFactory = (display) => newFixture(display);
playMatch('release-red-vs-fixture', releaseFactory, fixtureFactory);
playMatch('release-blue-vs-fixture', fixtureFactory, releaseFactory);
playMatch('release-self-play', releaseFactory, releaseFactory);

// ---- Evidence -------------------------------------------------------------
const hash = sha256(componentBytes);
const fixtureHash = sha256(readFileSync(fixturePath));
const md = `# Arena conformance evidence

Generated by \`npm run test:conformance\` against the committed release file.

- Release: \`${releaseJson.filename}\`
- Release SHA-256: \`${hash}\`
- Fixture SHA-256: \`${fixtureHash}\`
- Rules: \`${info.rulesId}\` / \`${info.rulesDigest}\`
- jco-transpile: 0.24.0; transpile time ${transpiled.elapsedMs.toFixed(0)} ms
- Title: ${info.title}

## Admission checklist

| Check | Result | Detail |
|---|---|---|
${checks.map((c) => `| ${c.name} | ${c.ok ? 'PASS' : 'FAIL'} | ${c.detail.replace(/\|/g, '\\|')} |`).join('\n')}

## Match behaviour

| Match | Plies | Result | Max choose-move | Max render | Revisions |
|---|---|---|---|---|---|
${matches.map((m) => `| ${m.label} | ${m.plies} | ${m.status}${m.winner ? ` / ${m.winner}` : ''} / ${m.reason} | ${m.maxMoveMs.toFixed(1)} ms | ${m.maxRenderMs.toFixed(1)} ms | ${m.revisions} |`).join('\n')}

Every revision was rendered by both boards; both boards received the same
official snapshot. The fixture always returns legal index 0. Frames:
\`docs/screenshots/arena-opening.png\`, \`arena-opening-flipped.png\`,
\`arena-fixture.png\`, and one settled frame per match.

## Determinism checks

- Same snapshot, legal list, seed, difficulty and budget: ${pick1} (after renders) and ${pick2} (same instance), ${pick3} (fresh instance).
- Unflipped render hash: \`${hashFrame(openA)}\`
- Flipped render hash: \`${hashFrame(openFlip)}\`
`;
writeFileSync(path.join(docs, 'CONFORMANCE.md'), md);

console.log(`\nconformance PASS: ${checks.length} admission/smoke checks, ${matches.length} complete matches`);
console.log(`release sha256 ${hash}`);
console.log(`fixture sha256 ${fixtureHash}`);

