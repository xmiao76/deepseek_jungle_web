// Shared wire/snapshot definitions for the standalone wasm engine.

export type WireSide = 0 | 1; // 0 = red, 1 = blue (wire convention)
export type Move = readonly [number, number];

export const SIDE_RED: WireSide = 0;
export const SIDE_BLUE: WireSide = 1;

export interface Snapshot {
  rev: number;
  quiet: number;
  turn: WireSide;
  first: WireSide;
  status: 0 | 1 | 2; // playing, won, drawn
  winner: WireSide | null;
  reasonCode: number;
  lastCapture: number;
  board: Uint8Array;
  plies: Move[];
  legal: Move[];
}

export interface AiResult {
  from: number;
  to: number;
  depth: number;
  nodes: number;
  score: number;
  elapsedMs: number;
  aborted: boolean;
  wallMs: number;
}

export interface RenderOptions {
  scale: number;
  flipped: boolean;
  progress: number;
  selected: number | null;
  hover: number | null;
  cursor: number | null;
}

export interface Frame {
  bitmap?: ImageBitmap;
  data?: Uint8ClampedArray;
  width: number;
  height: number;
}

export const ANIMAL_NAMES = ['Rat', 'Cat', 'Dog', 'Wolf', 'Leopard', 'Tiger', 'Lion', 'Elephant'] as const;
export const ANIMAL_SLUGS = ['rat', 'cat', 'dog', 'wolf', 'leopard', 'tiger', 'lion', 'elephant'] as const;

/** Piece id uses owner bit 0 = blue, 1 = red. */
export function pieceOwner(id: number): 0 | 1 {
  return ((id - 1) & 1) as 0 | 1;
}

export function pieceRank(id: number): number {
  return Math.floor(((id - 1) % 16) / 2) + 1;
}

export function ownerName(id: number): 'blue' | 'red' {
  return pieceOwner(id) === 1 ? 'red' : 'blue';
}

export function animalName(id: number): string {
  return ANIMAL_NAMES[pieceRank(id) - 1] ?? '?';
}

export function sideName(side: WireSide): 'red' | 'blue' {
  return side === SIDE_RED ? 'red' : 'blue';
}

export function opposite(side: WireSide): WireSide {
  return side === SIDE_RED ? SIDE_BLUE : SIDE_RED;
}

export function turnAfter(first: WireSide, plies: number): WireSide {
  return plies % 2 === 0 ? first : opposite(first);
}

export function moveKey(m: Move): string {
  return `${m[0]}-${m[1]}`;
}

export function formatMove(m: Move): string {
  return `${m[0]}→${m[1]}`;
}

export function reasonText(code: number): string {
  switch (code) {
    case 1: return 'Den reached';
    case 2: return 'All pieces captured';
    case 3: return 'No legal moves';
    case 4: return 'Threefold repetition';
    case 5: return '100 plies without a capture';
    default: return '';
  }
}

export function parseSnapshot(buf: ArrayBuffer): Snapshot {
  const dv = new DataView(buf);
  const rev = dv.getUint32(0, true);
  const quiet = dv.getUint16(4, true);
  const turn = dv.getUint8(6) as WireSide;
  const first = dv.getUint8(7) as WireSide;
  const status = dv.getUint8(8) as 0 | 1 | 2;
  const winnerByte = dv.getUint8(9);
  const reasonCode = dv.getUint8(10);
  const lastCapture = dv.getUint8(11);
  const plyCount = dv.getUint16(12, true);
  const legalCount = dv.getUint16(14, true);
  const board = new Uint8Array(buf.slice(16, 79));
  const plies: Move[] = [];
  let off = 79;
  for (let i = 0; i < plyCount; i++) {
    const v = dv.getUint16(off, true);
    plies.push([v >> 8, v & 0xff]);
    off += 2;
  }
  const legal: Move[] = [];
  for (let i = 0; i < legalCount; i++) {
    const v = dv.getUint16(off, true);
    legal.push([v >> 8, v & 0xff]);
    off += 2;
  }
  return {
    rev,
    quiet,
    turn,
    first,
    status,
    winner: winnerByte === 255 ? null : (winnerByte as WireSide),
    reasonCode,
    lastCapture,
    board,
    plies,
    legal,
  };
}

/** Result token for data attributes / tests. */
export function resultToken(snapshot: Snapshot): string {
  if (snapshot.status === 0) return 'playing';
  if (snapshot.status === 2) return `draw-${snapshot.reasonCode}`;
  return `${sideName(snapshot.winner ?? 0)}-${snapshot.reasonCode}`;
}
