//! Move generation + make/unmake — a line-for-line port of the C# SearchBoard.
//! Move order follows the C# ImmutableDictionary iteration order, which is
//! ascending `Position.GetHashCode()` = `Col*9+Row` (column-major).

use crate::board::*;

pub const MAX_MOVES: usize = 128;

/// Column-major square order (ascending Col*9+Row), matching the C# piece-id
/// list order that drives move generation.
pub fn column_major() -> &'static [u8; SQUARES] {
    static ORDER: std::sync::OnceLock<[u8; SQUARES]> = std::sync::OnceLock::new();
    ORDER.get_or_init(|| {
        let mut o = [0u8; SQUARES];
        let mut i = 0;
        for col in 0..7u8 {
            for row in 0..9u8 {
                o[i] = row * 7 + col;
                i += 1;
            }
        }
        o
    })
}

/// Mirror of the C# CaptureResolver/SearchBoard.CanCapture.
#[inline]
pub fn can_capture(attacker: u8, attacker_sq: u8, defender: u8, defender_sq: u8) -> bool {
    let a_rank = rank_of(attacker);
    let d_rank = rank_of(defender);
    if a_rank == 1 && d_rank == 8 {
        return !is_water(attacker_sq);
    }
    if a_rank == 8 && d_rank == 1 {
        return false;
    }
    if a_rank == 1 && d_rank == 1 {
        return true;
    }
    if d_rank == 1 && is_water(defender_sq) && !is_water(attacker_sq) {
        return false;
    }
    eff_rank(attacker, attacker_sq) >= eff_rank(defender, defender_sq)
}

// ---- Static move tables -----------------------------------------------------

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
                break;
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

/// Returns the neighbors of a square.
pub fn neighbors(sq: u8) -> &'static [u8] {
    &tables().neighbors[sq as usize]
}

/// True when the square is a trap of the given side's opponent.
#[inline]
pub fn is_enemy_trap_square(sq: u8, side: u8) -> bool {
    if side == 0 {
        TRAP_RED.contains(&sq)
    } else {
        TRAP_BLUE.contains(&sq)
    }
}

/// True when the square is orthogonally adjacent to the side's opponent den
/// (a den-entry setup square).
#[inline]
pub fn is_adjacent_to_opp_den(sq: u8, side: u8) -> bool {
    let den = if side == 0 { DEN_RED } else { DEN_BLUE };
    neighbors(den).contains(&sq)
}

/// Counts the legal moves for `side` (uses a stack scratch buffer).
pub fn count_legal_moves(board: &Board, side: u8) -> i32 {
    let mut buf = [Move { from: 0, to: 0, captured_id: 0, enters_den: false }; MAX_MOVES];
    generate_moves(board, side, &mut buf) as i32
}

/// True when the Lion/Tiger at `from` has a legal, rat-free jump to `target`.
pub fn can_jump_to(board: &Board, id: u8, from: u8, target: u8) -> bool {
    let rank = rank_of(id);
    let jumps: Option<&Vec<Jump>> = if rank == 7 {
        Some(&tables().lion_jumps[from as usize])
    } else if rank == 6 {
        Some(&tables().tiger_jumps[from as usize])
    } else {
        None
    };
    match jumps {
        None => false,
        Some(jumps) => {
            for j in jumps {
                if j.target == target && !jump_blocked(board, j) {
                    return true;
                }
            }
            false
        }
    }
}

/// True when the Lion/Tiger has a rat-free jump whose landing lies on the enemy
/// half of the board (the P5 clear-jump-lane eval term).
pub fn has_clear_jump_to_enemy_half(board: &Board, id: u8, from: u8) -> bool {
    let rank = rank_of(id);
    let jumps: Option<&Vec<Jump>> = if rank == 7 {
        Some(&tables().lion_jumps[from as usize])
    } else if rank == 6 {
        Some(&tables().tiger_jumps[from as usize])
    } else {
        None
    };
    match jumps {
        None => false,
        Some(jumps) => {
            let side = owner_of(id);
            for j in jumps {
                let target_row = j.target / 7;
                let enemy_half = if side == 0 { target_row >= 6 } else { target_row <= 2 };
                if enemy_half && !jump_blocked(board, j) {
                    return true;
                }
            }
            false
        }
    }
}

fn jump_blocked(board: &Board, j: &Jump) -> bool {
    for m in 0..j.mid_count as usize {
        let mid = j.mids[m];
        let occ = board.squares[mid as usize];
        if occ != 0 && rank_of(occ) == 1 {
            return true;
        }
    }
    false
}

// ---- Make / unmake ----------------------------------------------------------

pub struct Undo {
    pub prev_winner: u8,
}

impl Board {
    fn try_make(&self, side: u8, id: u8, from: u8, to: u8) -> Option<Move> {
        if is_own_den(to, side) {
            return None;
        }
        if is_water(to) && rank_of(id) != 1 {
            return None;
        }
        let occ = self.squares[to as usize];
        if occ != 0 {
            if owner_of(occ) == side {
                return None;
            }
            if !can_capture(id, from, occ, to) {
                return None;
            }
            return Some(Move { from, to, captured_id: occ, enters_den: false });
        }
        Some(Move { from, to, captured_id: 0, enters_den: is_opp_den(to, side) })
    }

    /// Applies a move in place, returning the undo data (mirrors SearchBoard.MakeMove).
    pub fn make_move(&mut self, m: &Move) -> Undo {
        let side = self.turn;
        let id = self.squares[m.from as usize];
        let prev_winner = self.winner;

        let z = zobrist();
        let zid = type_of(id) as usize;
        self.hash ^= z.piece_keys[m.from as usize][zid];
        self.squares[m.from as usize] = 0;

        if m.captured_id != 0 {
            let opp = (side ^ 1) as usize;
            self.count[opp] -= 1;
            self.hash ^= z.piece_keys[m.to as usize][type_of(m.captured_id) as usize];
            if self.count[opp] == 0 {
                self.winner = side; // total elimination
            }
        }

        self.squares[m.to as usize] = id;
        self.hash ^= z.piece_keys[m.to as usize][zid];

        if m.enters_den {
            self.winner = side;
        }

        self.hash ^= z.turn_key;
        self.turn ^= 1;
        Undo { prev_winner }
    }

    /// Undoes make_move (mirrors SearchBoard.UnmakeMove).
    pub fn unmake_move(&mut self, m: &Move, u: &Undo) {
        let side = self.turn ^ 1;
        let id = self.squares[m.to as usize];

        let z = zobrist();
        let zid = type_of(id) as usize;
        self.hash ^= z.turn_key;
        self.squares[m.to as usize] = if m.captured_id != 0 { m.captured_id } else { 0 };
        self.hash ^= z.piece_keys[m.to as usize][zid];

        if m.captured_id != 0 {
            let opp = (side ^ 1) as usize;
            self.count[opp] += 1;
            self.hash ^= z.piece_keys[m.to as usize][type_of(m.captured_id) as usize];
        }

        self.squares[m.from as usize] = id;
        self.hash ^= z.piece_keys[m.from as usize][zid];
        self.winner = u.prev_winner;
        self.turn = side;
    }
}

// ---- Generation -------------------------------------------------------------

/// Generates all legal moves for `side` into `buf`; returns the count.
/// Pieces are iterated in column-major (ascending Col*9+Row) order.
pub fn generate_moves(board: &Board, side: u8, buf: &mut [Move]) -> usize {
    let t = tables();
    let mut n = 0;
    for &sq in column_major() {
        let id = board.squares[sq as usize];
        if id == 0 || owner_of(id) != side {
            continue;
        }
        let rank = rank_of(id);

        for &to in &t.neighbors[sq as usize] {
            if let Some(m) = board.try_make(side, id, sq, to) {
                buf[n] = m;
                n += 1;
            }
        }

        let jumps: Option<&Vec<Jump>> = if rank == 7 {
            Some(&t.lion_jumps[sq as usize])
        } else if rank == 6 {
            Some(&t.tiger_jumps[sq as usize])
        } else {
            None
        };
        if let Some(jumps) = jumps {
            for j in jumps {
                if jump_blocked(board, j) {
                    continue;
                }
                if let Some(m) = board.try_make(side, id, sq, j.target) {
                    buf[n] = m;
                    n += 1;
                }
            }
        }
    }
    n
}

/// Captures plus enemy-den entries (the moves searched by quiescence).
pub fn generate_captures(board: &Board, side: u8, buf: &mut [Move]) -> usize {
    let t = tables();
    let mut n = 0;
    for &sq in column_major() {
        let id = board.squares[sq as usize];
        if id == 0 || owner_of(id) != side {
            continue;
        }
        let rank = rank_of(id);

        for &to in &t.neighbors[sq as usize] {
            if let Some(m) = board.try_make(side, id, sq, to) {
                if m.is_capture() || m.enters_den {
                    buf[n] = m;
                    n += 1;
                }
            }
        }

        let jumps: Option<&Vec<Jump>> = if rank == 7 {
            Some(&t.lion_jumps[sq as usize])
        } else if rank == 6 {
            Some(&t.tiger_jumps[sq as usize])
        } else {
            None
        };
        if let Some(jumps) = jumps {
            for j in jumps {
                if jump_blocked(board, j) {
                    continue;
                }
                if let Some(m) = board.try_make(side, id, sq, j.target) {
                    if m.is_capture() || m.enters_den {
                        buf[n] = m;
                        n += 1;
                    }
                }
            }
        }
    }
    n
}
