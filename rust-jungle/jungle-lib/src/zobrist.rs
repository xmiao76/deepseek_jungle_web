//! Zobrist hashing — byte-identical to the C# `Zobrist` class (Random(42) +
//! NextBytes, little-endian). 63 squares x 16 piece types + a turn key.

use crate::netrand::NetRandom;
use crate::board::SQUARES;

pub const POSITION_COUNT: usize = SQUARES; // 63
pub const PIECE_TYPES: usize = 16;

pub struct Zobrist {
    pub piece_keys: [[u64; PIECE_TYPES]; POSITION_COUNT],
    pub turn_key: u64,
}

impl Zobrist {
    pub fn new() -> Self {
        let mut rng = NetRandom::new(42);
        let mut piece_keys = [[0u64; PIECE_TYPES]; POSITION_COUNT];
        for p in 0..POSITION_COUNT {
            for i in 0..PIECE_TYPES {
                piece_keys[p][i] = rng.next_u64();
            }
        }
        let turn_key = rng.next_u64();
        Zobrist { piece_keys, turn_key }
    }
}

impl Default for Zobrist {
    fn default() -> Self {
        Self::new()
    }
}
