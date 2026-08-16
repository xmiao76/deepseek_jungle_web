//! Board representation: 63 squares holding piece type+1 (1..=16, 0 = empty),
//! turn byte (0 = Blue, 1 = Red). Piece type = (animal-1)*2 + owner, matching
//! the C# SearchBoard/Zobrist convention. Den entries and captures are
//! resolved inline by the move generator (winner tracking not needed for
//! perft parity — moves beyond a den entry or elimination are never
//! generated because the parent position is terminal).

pub const SQUARES: usize = 63;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Board {
    pub squares: [u8; SQUARES],
    pub turn: u8,
}

impl Default for Board {
    fn default() -> Self {
        Board { squares: [0; SQUARES], turn: 0 }
    }
}

impl Board {
    /// Builds a board from packed (type, square) lists — types are
    /// (animal-1)*2 + owner (0..=15), squares are row*7+col.
    pub fn from_packed(types: &[u8], squares: &[u8], turn: u8) -> Board {
        let mut b = Board { squares: [0; SQUARES], turn };
        for i in 0..types.len() {
            b.squares[squares[i] as usize] = types[i] + 1;
        }
        b
    }

    /// The standard start position (same layout as GameState.CreateInitial).
    pub fn initial() -> Board {
        let specs: [(u8, u8, u8); 16] = [
            // (col, row, (animal-1)*2 + owner); Blue owner=0, Red owner=1.
            (0, 0, (6 - 1) * 2),     // Blue Tiger
            (6, 0, (7 - 1) * 2),     // Blue Lion
            (1, 1, (2 - 1) * 2),     // Blue Cat
            (5, 1, (3 - 1) * 2),     // Blue Dog
            (0, 2, (8 - 1) * 2),     // Blue Elephant
            (2, 2, (4 - 1) * 2),     // Blue Wolf
            (4, 2, (5 - 1) * 2),     // Blue Leopard
            (6, 2, (1 - 1) * 2),     // Blue Rat
            (0, 6, (1 - 1) * 2 + 1), // Red Rat
            (2, 6, (5 - 1) * 2 + 1), // Red Leopard
            (4, 6, (4 - 1) * 2 + 1), // Red Wolf
            (6, 6, (8 - 1) * 2 + 1), // Red Elephant
            (1, 7, (3 - 1) * 2 + 1), // Red Dog
            (5, 7, (2 - 1) * 2 + 1), // Red Cat
            (0, 8, (7 - 1) * 2 + 1), // Red Lion
            (6, 8, (6 - 1) * 2 + 1), // Red Tiger
        ];
        let mut b = Board { squares: [0; SQUARES], turn: 0 };
        for (c, r, t) in specs {
            b.squares[(r * 7 + c) as usize] = t + 1;
        }
        b
    }

    #[inline]
    pub fn occupant(&self, sq: usize) -> u8 {
        self.squares[sq]
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
}
