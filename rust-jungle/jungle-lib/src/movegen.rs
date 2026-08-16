//! Move generation and perft — a line-for-line port of the C# SearchBoard's
//! GenerateMoves/TryMakeMove/CanCapture for the parity spike.

use crate::board::{Board, SQUARES};

pub const MAX_MOVES: usize = 128;

#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
}

// ---- Terrain ---------------------------------------------------------------

const WATER: [u8; 12] = [22, 23, 29, 30, 36, 37, 25, 26, 32, 33, 39, 40];
const TRAP_BLUE: [u8; 3] = [2, 4, 10];
const TRAP_RED: [u8; 3] = [58, 60, 52];
const DEN_BLUE: u8 = 3;
const DEN_RED: u8 = 59;

#[inline]
pub fn is_water(sq: u8) -> bool {
    WATER.contains(&sq)
}

#[inline]
fn is_own_den(sq: u8, side: u8) -> bool {
    if side == 0 {
        sq == DEN_BLUE
    } else {
        sq == DEN_RED
    }
}

/// Effective rank: 0 when standing on the opponent's trap.
#[inline]
fn eff_rank(id: u8, sq: u8) -> u8 {
    let rank = Board::rank_of(id);
    let owner = Board::owner_of(id);
    let on_enemy_trap = if owner == 0 {
        TRAP_RED.contains(&sq)
    } else {
        TRAP_BLUE.contains(&sq)
    };
    if on_enemy_trap {
        0
    } else {
        rank
    }
}

/// Mirror of the C# CaptureResolver/SearchBoard.CanCapture.
fn can_capture(attacker: u8, attacker_sq: u8, defender: u8, defender_sq: u8) -> bool {
    let a_rank = Board::rank_of(attacker);
    let d_rank = Board::rank_of(defender);

    // Rat captures Elephant from land only
    if a_rank == 1 && d_rank == 8 {
        return !is_water(attacker_sq);
    }
    // Elephant cannot capture Rat
    if a_rank == 8 && d_rank == 1 {
        return false;
    }
    // Rat vs Rat is unconditional (land or water, even from a trap square)
    if a_rank == 1 && d_rank == 1 {
        return true;
    }
    // Rat in water cannot be captured by any other piece on land
    if d_rank == 1 && is_water(defender_sq) && !is_water(attacker_sq) {
        return false;
    }

    eff_rank(attacker, attacker_sq) >= eff_rank(defender, defender_sq)
}

// ---- Static move tables -----------------------------------------------------

/// Neighbors per square (orthogonal, in-bounds).
fn build_neighbors() -> Vec<Vec<u8>> {
    let mut table = Vec::with_capacity(SQUARES);
    for sq in 0..SQUARES as u8 {
        let c = (sq % 7) as i8;
        let r = (sq / 7) as i8;
        let mut n = Vec::with_capacity(4);
        for (dc, dr) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let nc = c + dc;
            let nr = r + dr;
            if (0..=6).contains(&nc) && (0..=8).contains(&nr) {
                n.push((nr * 7 + nc) as u8);
            }
        }
        table.push(n);
    }
    table
}

#[derive(Clone, Copy)]
struct Jump {
    target: u8,
    mids: [u8; 3],
    mid_count: u8,
}

/// All jump targets from (c, r) whose entire path is water and whose landing
/// is land, mirroring SearchBoard.BuildJumps: distances scanned from 8 down
/// to 2, first valid crossing wins (exactly one per direction on this
/// terrain). Row-changing jumps are included only for the Lion.
fn build_jumps(c: i8, r: i8, include_row_change: bool) -> Vec<Jump> {
    let mut jumps = Vec::with_capacity(4);
    let dirs: &[(i8, i8)] = if include_row_change {
        &[(0, -1), (0, 1), (-1, 0), (1, 0)]
    } else {
        &[(-1, 0), (1, 0)]
    };
    for &(dc, dr) in dirs {
        for dist in (2i8..=8).rev() {
            let tc = c + dc * dist;
            let tr = r + dr * dist;
            if !(0..=6).contains(&tc) || !(0..=8).contains(&tr) {
                continue;
            }
            let target = (tr * 7 + tc) as u8;
            // Every crossed square must be water; the landing must be land.
            let mut all_water = true;
            let mut mids = [0u8; 3];
            let mut mid_count = 0;
            for step in 1..dist {
                let mc = c + dc * step;
                let mr = r + dr * step;
                let msq = (mr * 7 + mc) as u8;
                if !is_water(msq) {
                    all_water = false;
                    break;
                }
                mids[mid_count] = msq;
                mid_count += 1;
            }
            if all_water && mid_count > 0 && !is_water(target) {
                jumps.push(Jump { target, mids, mid_count: mid_count as u8 });
                break; // first valid crossing wins
            }
        }
    }
    jumps
}

struct Tables {
    neighbors: Vec<Vec<u8>>,
    lion_jumps: Vec<Vec<Jump>>,
    tiger_jumps: Vec<Vec<Jump>>,
}

fn tables() -> &'static Tables {
    static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();
    TABLES.get_or_init(|| Tables {
        neighbors: build_neighbors(),
        lion_jumps: (0..SQUARES as u8)
            .map(|sq| build_jumps((sq % 7) as i8, (sq / 7) as i8, true))
            .collect(),
        tiger_jumps: (0..SQUARES as u8)
            .map(|sq| build_jumps((sq % 7) as i8, (sq / 7) as i8, false))
            .collect(),
    })
}

// ---- Generation -------------------------------------------------------------

fn try_make_move(board: &Board, side: u8, id: u8, from: u8, to: u8, out: &mut Move) -> bool {
    // Cannot move into own den
    if is_own_den(to, side) {
        return false;
    }
    // Only the Rat may enter water
    if is_water(to) && Board::rank_of(id) != 1 {
        return false;
    }
    let occ = board.occupant(to as usize);
    if occ != 0 {
        if Board::owner_of(occ) == side {
            return false; // own piece
        }
        if !can_capture(id, from, occ, to) {
            return false;
        }
        out.from = from;
        out.to = to;
        return true;
    }
    out.from = from;
    out.to = to;
    true
}

/// Generates all legal moves for `side` into `buf`; returns the count.
pub fn generate_moves(board: &Board, side: u8, buf: &mut [Move]) -> usize {
    let t = tables();
    let mut n = 0;
    for sq in 0..SQUARES as u8 {
        let id = board.occupant(sq as usize);
        if id == 0 || Board::owner_of(id) != side {
            continue;
        }
        let rank = Board::rank_of(id);

        for &to in &t.neighbors[sq as usize] {
            if try_make_move(board, side, id, sq, to, &mut buf[n]) {
                n += 1;
            }
        }

        let jumps: &[Jump] = if rank == 7 {
            &t.lion_jumps[sq as usize]
        } else if rank == 6 {
            &t.tiger_jumps[sq as usize]
        } else {
            &[]
        };
        for jump in jumps {
            // A rat on any crossed water square blocks the jump.
            let mut blocked = false;
            for m in 0..jump.mid_count as usize {
                let mid = jump.mids[m];
                if board.occupant(mid as usize) != 0 && Board::rank_of(board.occupant(mid as usize)) == 1 {
                    blocked = true;
                    break;
                }
            }
            if blocked {
                continue;
            }
            if try_make_move(board, side, id, sq, jump.target, &mut buf[n]) {
                n += 1;
            }
        }
    }
    n
}

/// Counts the nodes reachable in exactly `depth` plies (copy-make recursion;
/// sufficient for the spike's count-parity gate).
pub fn perft(board: &Board, side: u8, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut buf = [Move { from: 0, to: 0 }; MAX_MOVES];
    let count = generate_moves(board, side, &mut buf);
    if depth == 1 {
        return count as u64;
    }
    let mut nodes = 0u64;
    for i in 0..count {
        let m = buf[i];
        let mut child = *board;
        child.squares[m.to as usize] = child.squares[m.from as usize];
        child.squares[m.from as usize] = 0;
        child.turn ^= 1;
        nodes += perft(&child, side ^ 1, depth - 1);
    }
    nodes
}
