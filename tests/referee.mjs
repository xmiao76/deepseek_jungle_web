// Independent JavaScript referee for jungle-v2. It is deliberately separate
// from the Rust core: the conformance host and the parity tests compare the two.

export const ANIMALS = ['rat', 'cat', 'dog', 'wolf', 'leopard', 'tiger', 'lion', 'elephant'];
export const RED = 'red';
export const BLUE = 'blue';
export const SQUARES = 63;

const WATER = new Set([22, 23, 25, 26, 29, 30, 32, 33, 36, 37, 39, 40]);
const TRAP_BLUE = new Set([2, 4, 10]);
const TRAP_RED = new Set([52, 58, 60]);
const DEN_RED = 59;
const DEN_BLUE = 3;
const RED_OPENING = [48, 50, 54, 44, 46, 56, 62, 42];
const BLUE_OPENING = [14, 12, 8, 18, 16, 6, 0, 20];
const NEIGHBORS = (() => {
  const table = [];
  for (let sq = 0; sq < SQUARES; sq++) {
    const col = sq % 7;
    const row = Math.floor(sq / 7);
    const n = [];
    if (row > 0) n.push(sq - 7);
    if (row < 8) n.push(sq + 7);
    if (col > 0) n.push(sq - 1);
    if (col < 6) n.push(sq + 1);
    table.push(n);
  }
  return table;
})();

export const isWater = (sq) => WATER.has(sq);
export const isTrap = (sq) => TRAP_BLUE.has(sq) || TRAP_RED.has(sq);
export const trapOwner = (sq) => (TRAP_BLUE.has(sq) ? BLUE : RED);
export const isDen = (sq) => sq === DEN_RED || sq === DEN_BLUE;

export function initial(first = RED) {
  const cells = new Array(SQUARES).fill(null);
  BLUE_OPENING.forEach((sq, i) => {
    cells[sq] = { owner: BLUE, rank: i + 1 };
  });
  RED_OPENING.forEach((sq, i) => {
    cells[sq] = { owner: RED, rank: i + 1 };
  });
  const state = {
    cells,
    first,
    turn: first,
    quiet: 0,
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

export function cloneState(state) {
  return {
    cells: state.cells.slice(),
    first: state.first,
    turn: state.turn,
    quiet: state.quiet,
    rev: state.rev,
    plies: state.plies.slice(),
    keys: state.keys.slice(),
    status: state.status,
    winner: state.winner,
    reason: state.reason,
  };
}

export const other = (side) => (side === RED ? BLUE : RED);

export function positionKey(state) {
  let s = '';
  for (const piece of state.cells) {
    s += piece ? (piece.owner === RED ? 'R' : 'B') + piece.rank : '..';
  }
  return s + (state.turn === RED ? 'r' : 'b');
}

export function effectiveRank(piece, sq) {
  if (!piece) return 0;
  const enemyTrap = piece.owner === RED ? TRAP_BLUE.has(sq) : TRAP_RED.has(sq);
  return enemyTrap ? 0 : piece.rank;
}

export function isOwnDen(sq, side) {
  return sq === (side === RED ? DEN_RED : DEN_BLUE);
}
export function isEnemyDen(sq, side) {
  return sq === (side === RED ? DEN_BLUE : DEN_RED);
}

/** Capture legality for a piece on `src` taking a defender on `dst`. */
export function canCapture(attacker, src, defender, dst) {
  if (isWater(src) !== isWater(dst)) return false;
  const aEff = effectiveRank(attacker, src);
  const dEff = effectiveRank(defender, dst);
  if (dEff === 0) return true;
  if (aEff === 0) return false;
  if (attacker.rank === 1 && defender.rank === 8) return true;
  if (attacker.rank === 8 && defender.rank === 1) return false;
  return aEff >= dEff;
}

function canLandOnEmpty(piece, sq) {
  if (isOwnDen(sq, piece.owner)) return false;
  if (isWater(sq) && piece.rank !== 1) return false;
  return true;
}

function makeMoveIfLegal(state, src, dst) {
  const piece = state.cells[src];
  if (!piece || piece.owner !== state.turn) return null;
  if (isOwnDen(dst, piece.owner)) return null;
  const target = state.cells[dst];
  if (!target) {
    if (!canLandOnEmpty(piece, dst)) return null;
    return { src, dst, captured: null };
  }
  if (target.owner === piece.owner) return null;
  if (!canCapture(piece, src, target, dst)) return null;
  return { src, dst, captured: target };
}

function jumpMoves(state, src) {
  const piece = state.cells[src];
  if (!piece || (piece.rank !== 6 && piece.rank !== 7)) return [];
  if (isWater(src)) return [];
  const allowedMids = piece.rank === 7 ? new Set([2, 3]) : new Set([2]);
  const out = [];
  for (const [dc, dr] of [
    [0, -1],
    [0, 1],
    [-1, 0],
    [1, 0],
  ]) {
    const col = src % 7;
    const row = Math.floor(src / 7);
    let c = col + dc;
    let r = row + dr;
    let mid = 0;
    let blocked = false;
    while (c >= 0 && c <= 6 && r >= 0 && r <= 8) {
      const sq = r * 7 + c;
      if (isWater(sq)) {
        mid++;
        if (state.cells[sq]) blocked = true; // only rats can be in water
        if (mid > 3) break;
        c += dc;
        r += dr;
        continue;
      }
      if (mid > 0 && !blocked && allowedMids.has(mid)) {
        const move = makeMoveIfLegal(state, src, sq);
        if (move) out.push(move);
      }
      break;
    }
  }
  return out;
}

/** Sorted legal move list: `{src, dst, captured}`. */
export function legalMoves(state) {
  const moves = [];
  for (let src = 0; src < SQUARES; src++) {
    const piece = state.cells[src];
    if (!piece || piece.owner !== state.turn) continue;
    for (const dst of NEIGHBORS[src]) {
      const move = makeMoveIfLegal(state, src, dst);
      if (move) moves.push(move);
    }
    moves.push(...jumpMoves(state, src));
  }
  moves.sort((a, b) => a.src - b.src || a.dst - b.dst);
  return moves;
}

export function countLegal(state, side) {
  const probe = { ...state, cells: state.cells.slice(), turn: side };
  return legalMoves(probe).length;
}

export function applyMove(state, src, dst) {
  if (state.status !== 'playing') return null;
  const legal = legalMoves(state);
  const move = legal.find((m) => m.src === src && m.dst === dst);
  if (!move) return null;
  const next = cloneState(state);
  const piece = next.cells[src];
  next.cells[src] = null;
  next.cells[dst] = piece;
  next.turn = other(state.turn);
  next.rev += 1;
  next.quiet = move.captured ? 0 : next.quiet + 1;
  next.plies.push({ src, dst });
  next.keys.push(positionKey(next));
  settle(next, piece, move);
  return next;
}

function settle(state, mover, move) {
  const opponent = state.turn;
  if (isEnemyDen(move.dst, mover.owner)) {
    state.status = 'won';
    state.winner = mover.owner;
    state.reason = 'den';
    return;
  }
  if (!state.cells.some((p) => p && p.owner === opponent)) {
    state.status = 'won';
    state.winner = mover.owner;
    state.reason = 'elimination';
    return;
  }
  if (countLegal(state, opponent) === 0) {
    state.status = 'won';
    state.winner = mover.owner;
    state.reason = 'immobilization';
    return;
  }
  const key = state.keys[state.keys.length - 1];
  if (state.keys.filter((k) => k === key).length >= 3) {
    state.status = 'drawn';
    state.winner = null;
    state.reason = 'repetition';
    return;
  }
  if (state.quiet >= 100) {
    state.status = 'drawn';
    state.winner = null;
    state.reason = 'no-capture';
  }
}

/** Applies a whole sequence, throwing on an illegal ply. */
export function replay(first, moves) {
  let state = initial(first);
  for (const move of moves) {
    const next = applyMove(state, move.src, move.dst);
    if (!next) throw new Error(`illegal replay move ${move.src}->${move.dst}`);
    state = next;
  }
  return state;
}

/** Wire snapshot for the component/host (owner/kind strings). */
export function toWire(state) {
  return {
    rev: state.rev,
    board: state.cells.map((p) => (p ? { owner: p.owner, kind: ANIMALS[p.rank - 1] } : undefined)),
    firstSide: state.first,
    toPlay: state.turn,
    history: state.plies.map((m) => ({ src: m.src, dst: m.dst })),
    noCapturePlies: state.quiet,
    status: state.status === 'won' ? 'won' : state.status === 'drawn' ? 'drawn' : 'playing',
    winner: state.winner ?? undefined,
    reason: state.reason,
  };
}

/** Legal list in wire form, matching the Arena's sorted contract. */
export function toWireLegal(legal) {
  return legal.map((m) => ({ src: m.src, dst: m.dst }));
}

export function fromWire(snapshot) {
  const cells = new Array(SQUARES).fill(null);
  (snapshot.board ?? []).forEach((p, sq) => {
    if (p) cells[sq] = { owner: p.owner, rank: ANIMALS.indexOf(p.kind) + 1 };
  });
  const state = {
    cells,
    first: snapshot.firstSide,
    turn: snapshot.toPlay,
    quiet: snapshot.noCapturePlies,
    rev: snapshot.rev,
    plies: (snapshot.history ?? []).map((m) => ({ src: m.src, dst: m.dst })),
    keys: [positionKey({ cells, turn: snapshot.toPlay })],
    status: snapshot.status,
    winner: snapshot.winner ?? null,
    reason: snapshot.reason ?? '',
  };
  return state;
}
