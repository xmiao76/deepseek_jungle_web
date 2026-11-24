import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  ANIMALS,
  RED,
  applyMove,
  initial,
  isWater,
  legalMoves,
  positionKey,
  replay,
  effectiveRank,
} from '../referee.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixtures = path.join(here, '..', 'fixtures');
const portable = JSON.parse(readFileSync(path.join(fixtures, 'portable-cases.json'), 'utf8'));
const adjudication = JSON.parse(readFileSync(path.join(fixtures, 'adjudication.json'), 'utf8'));
const opening = JSON.parse(readFileSync(path.join(fixtures, 'opening-handshake.json'), 'utf8'));

function fromBoard(map, turn = RED, quiet = 0) {
  const cells = new Array(63).fill(null);
  for (const [sq, desc] of Object.entries(map)) {
    const [owner, animal] = desc.split(':');
    cells[Number(sq)] = { owner, rank: ANIMALS.indexOf(animal) + 1 };
  }
  const state = {
    cells,
    first: RED,
    turn,
    quiet,
    rev: 0,
    plies: [],
    keys: [],
    status: 'playing',
    winner: null,
    reason: '',
  };
  state.keys.push(positionKey(state));
  return state;
}

test('portable conformance cases', () => {
  for (const [name, board, move, expected] of portable.cases) {
    const state = fromBoard(board);
    const legal = legalMoves(state);
    const actual = legal.some((m) => m.src === move[0] && m.dst === move[1]);
    assert.equal(actual, expected, `${name}: ${move[0]}->${move[1]}`);
  }
});

test('adjudication rows', () => {
  for (const row of adjudication.rows) {
    let state = fromBoard(row.position, RED, row.quietBefore);
    for (const [src, dst] of row.moves) {
      const next = applyMove(state, src, dst);
      assert.ok(next, `${row.name}: legal ${src}->${dst}`);
      state = next;
    }
    assert.equal(state.status, row.status, `${row.name}: status`);
    assert.equal(state.quiet, row.quietAfter, `${row.name}: quiet`);
    assert.equal(state.reason, row.reason, `${row.name}: reason`);
    assert.equal(state.winner ?? null, row.winner, `${row.name}: winner`);
    assert.equal(state.rev, row.moves.length, `${row.name}: revision`);
    assert.equal(state.plies.length, row.moves.length, `${row.name}: history`);
    const expectedTurn = row.moves.length % 2 === 0 ? RED : 'blue';
    assert.equal(state.turn, expectedTurn, `${row.name}: to-play`);
  }
});

test('opening handshake', () => {
  for (const [side, spec] of [['red', opening.red], ['blue', opening.blue]]) {
    const state = initial(side);
    const legal = legalMoves(state);
    assert.equal(legal.length, spec.legalCount, `${side}: legal count`);
    assert.deepEqual([legal[0].src, legal[0].dst], spec.firstMove, `${side}: first`);
    if (spec.lastMove) assert.deepEqual([legal.at(-1).src, legal.at(-1).dst], spec.lastMove, `${side}: last`);
  }
  const after = applyMove(initial('red'), 42, 35);
  assert.ok(after);
  assert.equal(after.rev, opening.red.afterFirst.rev);
  assert.equal(after.cells[opening.red.afterFirst.emptySquare], null);
  const piece = after.cells[opening.red.afterFirst.square];
  assert.equal(piece.owner, 'red');
  assert.equal(ANIMALS[piece.rank - 1], 'elephant');
  assert.equal(after.quiet, opening.red.afterFirst.quiet);
  assert.equal(after.turn, 'blue');
});

test('capture precedence includes trapped defenders and water boundaries', () => {
  const trappedRat = fromBoard({ 51: 'red:elephant', 52: 'blue:rat' });
  assert.ok(legalMoves(trappedRat).some((m) => m.src === 51 && m.dst === 52));
  const trappedAttacker = fromBoard({ 10: 'red:tiger', 11: 'blue:cat' });
  assert.ok(!legalMoves(trappedAttacker).some((m) => m.src === 10 && m.dst === 11));
  const waterLand = fromBoard({ 22: 'red:rat', 21: 'blue:elephant' });
  assert.ok(!legalMoves(waterLand).some((m) => m.src === 22 && m.dst === 21));
  const landWater = fromBoard({ 21: 'red:rat', 22: 'blue:rat' });
  assert.ok(!legalMoves(landWater).some((m) => m.src === 21 && m.dst === 22));
  const waterWater = fromBoard({ 22: 'red:rat', 23: 'blue:rat' });
  assert.ok(legalMoves(waterWater).some((m) => m.src === 22 && m.dst === 23));
});

test('trap effective rank is zero only on the opponent trap', () => {
  const red = { owner: 'red', rank: 8 };
  const blue = { owner: 'blue', rank: 1 };
  assert.equal(effectiveRank(red, 10), 0, 'red on blue trap');
  assert.equal(effectiveRank(red, 52), 8, 'red on own trap');
  assert.equal(effectiveRank(blue, 52), 0, 'blue on red trap');
  assert.equal(effectiveRank(blue, 10), 1, 'blue on own trap');
});

test('referee perft depth 1-3 matches the Rust counts', () => {
  function perft(state, depth) {
    if (depth === 0) return 1;
    const moves = legalMoves(state);
    let nodes = 0;
    for (const m of moves) {
      const next = applyMove(state, m.src, m.dst);
      nodes += perft(next, depth - 1);
    }
    return nodes;
  }
  assert.equal(legalMoves(initial('red')).length, 24);
  assert.equal(perft(initial('red'), 2), 576);
  assert.equal(perft(initial('red'), 3), 12240);
});

test('replay helper reproduces a state', () => {
  const state = replay('red', [
    { src: 42, dst: 43 },
    { src: 6, dst: 5 },
  ]);
  assert.equal(state.rev, 2);
  assert.equal(state.turn, 'red');
  assert.equal(state.plies.length, 2);
  assert.ok(!isWater(42));
});
