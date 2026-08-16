//! Search board: 63 squares holding piece id = type+1 (type = (animal-1)*2 +
//! owner), incremental Zobrist hash, make/unmake. Mirrors the C# SearchBoard.

use crate::zobrist::Zobrist;

pub const SQUARES: usize = 63;
pub const NO_WINNER: u8 = 255;

// Terrain (byte codes match the C# SearchBoard).
pub const WATER: [u8; 12] = [22, 23, 29, 30, 36, 37, 25, 26, 32, 33, 39, 40];
pub const TRAP_BLUE: [u8; 3] = [2, 4, 10];
pub const TRAP_RED: [u8; 3] = [58, 60, 52];
pub const DEN_BLUE: u8 = 3;
pub const DEN_RED: u8 = 59;

pub fn zobrist() -> &'static Zobrist {
    static Z: std::sync::OnceLock<Zobrist> = std::sync::OnceLock::new();
    Z.get_or_init(Zobrist::new)
}

#[inline]
pub fn is_water(sq: u8) -> bool {
    WATER.contains(&sq)
}

#[inline]
pub fn is_own_den(sq: u8, side: u8) -> bool {
    if side == 0 {
        sq == DEN_BLUE
    } else {
        sq == DEN_RED
    }
}

#[inline]
pub fn is_opp_den(sq: u8, side: u8) -> bool {
    if side == 0 {
        sq == DEN_RED
    } else {
        sq == DEN_BLUE
    }
}

/// Rank of a piece id (1..=16 -> 1..=8).
#[inline]
pub fn rank_of(id: u8) -> u8 {
    ((id - 1) % 16) / 2 + 1
}

/// Owner of a piece id (0 = Blue, 1 = Red).
#[inline]
pub fn owner_of(id: u8) -> u8 {
    (id - 1) & 1
}

/// Type of a piece id = (id - 1) % 16 = the Zobrist piece index.
#[inline]
pub fn type_of(id: u8) -> u8 {
    (id - 1) % 16
}

/// Effective rank: 0 when standing on the opponent's trap.
#[inline]
pub fn eff_rank(id: u8, sq: u8) -> u8 {
    let rank = rank_of(id);
    let owner = owner_of(id);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    pub squares: [u8; SQUARES], // square -> piece id (type+1), 0 = empty
    pub count: [u8; 2],
    pub turn: u8,
    pub winner: u8,
    pub hash: u64,
}

impl Board {
    pub fn new() -> Self {
        Board {
            squares: [0; SQUARES],
            count: [0, 0],
            turn: 0,
            winner: NO_WINNER,
            hash: 0,
        }
    }

    /// Builds from a packed (type, square) list — types are (animal-1)*2+owner.
    /// Piece ids are type+1 (the corpus has no duplicate animals per side).
    pub fn from_packed(types: &[u8], squares: &[u8], turn: u8) -> Board {
        let mut b = Board::new();
        b.turn = turn;
        let mut hash = 0u64;
        for i in 0..types.len() {
            let t = types[i];
            let sq = squares[i] as usize;
            let id = t + 1;
            b.squares[sq] = id;
            b.count[(t & 1) as usize] += 1;
            hash ^= zobrist().piece_keys[sq][t as usize];
        }
        if turn == 1 {
            hash ^= zobrist().turn_key;
        }
        b.hash = hash;
        b
    }

    /// Builds from a 63-byte square->type layout (the parity dump's hex format).
    /// `grid[sq]` = 0 (empty) or type+1.
    pub fn from_grid(grid: &[u8; SQUARES], turn: u8) -> Board {
        let mut b = Board::new();
        b.turn = turn;
        let mut hash = 0u64;
        for sq in 0..SQUARES {
            let id = grid[sq];
            if id != 0 {
                let t = (id - 1) % 16;
                b.squares[sq] = id;
                b.count[(t & 1) as usize] += 1;
                hash ^= zobrist().piece_keys[sq][t as usize];
            }
        }
        if turn == 1 {
            hash ^= zobrist().turn_key;
        }
        b.hash = hash;
        b
    }

    #[inline]
    pub fn occupant(&self, sq: usize) -> u8 {
        self.squares[sq]
    }

    /// The standard start position.
    pub fn initial() -> Board {
        let specs: [(u8, u8, u8); 16] = [
            (0, 0, 10), (6, 0, 12), (1, 1, 2), (5, 1, 4), (0, 2, 14), (2, 2, 6),
            (4, 2, 8), (6, 2, 0), (0, 6, 1), (2, 6, 9), (4, 6, 7), (6, 6, 15),
            (1, 7, 5), (5, 7, 3), (0, 8, 13), (6, 8, 11),
        ];
        let mut types = Vec::new();
        let mut squares = Vec::new();
        for (c, r, t) in specs {
            types.push(t);
            squares.push((r * 7 + c) as u8);
        }
        Board::from_packed(&types, &squares, 0)
    }
}

/// A search move with capture / den-entry info (mirrors C# SearchMove).
#[derive(Clone, Copy, Debug)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub captured_id: u8, // 0 = no capture
    pub enters_den: bool,
}

impl Move {
    pub fn is_capture(&self) -> bool {
        self.captured_id != 0
    }
}
