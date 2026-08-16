//! Jungle (Dou Shou Qi) core for the WASM port spike: board, movegen, perft.
//! Line-for-line port of the C# `SearchBoard` (this spike covers board +
//! movegen + perft only; parity is move-count parity against the C# oracle).

pub mod board;
pub mod movegen;
pub mod zobrist;

pub use board::Board;
pub use movegen::{generate_moves, perft};
