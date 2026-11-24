import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ANIMALS, applyMove, initial, legalMoves } from '../referee.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const wasmPath = path.join(here, '..', '..', 'build', 'jungle_webengine.wasm');
if (!existsSync(wasmPath)) throw new Error('build/jungle_webengine.wasm missing; run npm run build:engine');

const bytes = readFileSync(wasmPath);
const { instance } = await WebAssembly.instantiate(bytes, { env: { now_ms: () => performance.now() } });
const e = instance.exports;

function snapshot() {
  const len = e.jg_snapshot_len();
  const ptr = e.jg_alloc(len);
  const written = e.jg_snapshot(ptr, len);
  const copy = new Uint8Array(e.memory.buffer, ptr, written).slice();
  e.jg_dealloc(ptr, len);
  const dv = new DataView(copy.buffer);
  const rev = dv.getUint32(0, true);
  const quiet = dv.getUint16(4, true);
  const turn = dv.getUint8(6);
  const status = dv.getUint8(8);
  const winner = dv.getUint8(9);
  const reason = dv.getUint8(10);
  const plyCount = dv.getUint16(12, true);
  const legalCount = dv.getUint16(14, true);
  const board = copy.slice(16, 79);
  let off = 79;
  const plies = [];
  for (let i = 0; i < plyCount; i++) {
    const v = dv.getUint16(off, true);
    plies.push([v >> 8, v & 0xff]);
    off += 2;
  }
  const legal = [];
  for (let i = 0; i < legalCount; i++) {
    const v = dv.getUint16(off, true);
    legal.push([v >> 8, v & 0xff]);
    off += 2;
  }
  return { rev, quiet, turn, status, winner, reason, board, plies, legal };
}

function idFor(piece) {
  return (piece.rank - 1) * 2 + (piece.owner === 'red' ? 1 : 0) + 1;
}

function assertSame(js, wasm, label) {
  assert.equal(wasm.rev, js.rev, `${label}: rev`);
  assert.equal(wasm.quiet, js.quiet, `${label}: quiet`);
  assert.equal(wasm.turn, js.turn === 'red' ? 0 : 1, `${label}: turn`);
  assert.equal(wasm.status, js.status === 'playing' ? 0 : js.status === 'won' ? 1 : 2, `${label}: status`);
  assert.equal(
    wasm.winner === 255 ? null : wasm.winner === 0 ? 'red' : 'blue',
    js.winner ?? null,
    `${label}: winner`,
  );
  const reasonCode = { '': 0, den: 1, elimination: 2, immobilization: 3, repetition: 4, 'no-capture': 5 };
  assert.equal(wasm.reason, reasonCode[js.reason], `${label}: reason`);
  assert.equal(wasm.plies.length, js.plies.length, `${label}: history length`);
  for (let sq = 0; sq < 63; sq++) {
    const piece = js.cells[sq];
    const expected = piece ? idFor(piece) : 0;
    assert.equal(wasm.board[sq], expected, `${label}: square ${sq}`);
  }
  if (js.status === 'playing') {
    const expected = legalMoves(js).map((m) => [m.src, m.dst]);
    assert.deepEqual(wasm.legal, expected, `${label}: legal list`);
  }
}

function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

test('wasm opening matches the referee', () => {
  e.jg_new(0, 1);
  assertSame(initial('red'), snapshot(), 'opening');
});

test('random games stay byte-for-byte rule-equivalent', () => {
  const rng = mulberry32(0x5eed1234);
  let plies = 0;
  let terminals = 0;
  for (let game = 0; game < 24; game++) {
    const first = game % 2 === 0 ? 'red' : 'blue';
    let js = initial(first);
    e.jg_new(first === 'red' ? 0 : 1, 0x1000 + game);
    assertSame(js, snapshot(), `game ${game} opening`);
    for (let ply = 0; ply < 220; ply++) {
      const legal = legalMoves(js);
      if (js.status !== 'playing') break;
      assert.ok(legal.length > 0, `game ${game} ply ${ply}: playing with no moves`);
      const pick = legal[Math.floor(rng() * legal.length)];
      const next = applyMove(js, pick.src, pick.dst);
      assert.ok(next);
      const code = e.jg_apply(pick.src, pick.dst);
      assert.equal(code, 0, `game ${game} ply ${ply}: wasm rejected ${pick.src}->${pick.dst}`);
      js = next;
      plies++;
      assertSame(js, snapshot(), `game ${game} ply ${ply}`);
      if (js.status !== 'playing') {
        terminals++;
        break;
      }
    }
  }
  assert.ok(plies > 500, `only ${plies} plies compared`);
  assert.ok(terminals >= 5, `only ${terminals} terminal games`);
});

test('all opening legal moves round-trip in both implementations', () => {
  const js = initial('red');
  e.jg_new(0, 2);
  for (const move of legalMoves(js)) {
    assert.equal(e.jg_apply(move.src, move.dst), 0);
    const wasm = snapshot();
    assert.equal(wasm.rev, 1);
    assert.equal(wasm.turn, 1);
    assert.equal(wasm.legal.length, 24);
    // Reset for the next sibling.
    e.jg_new(0, 2);
  }
  // Keep the AI/tablebase code paths linked and check the rules strings.
  const id = new TextDecoder().decode(new Uint8Array(e.memory.buffer, e.jg_rules_id_ptr(), e.jg_rules_id_len()));
  assert.equal(id, 'desktop-jungle-v2');
  assert.equal(ANIMALS.length, 8);
});
