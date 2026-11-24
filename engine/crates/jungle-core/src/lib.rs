//! Jungle (Dou Shou Qi) core: board, movegen, Zobrist, v2 rules, search.
//!
//! Several performance-sensitive loops deliberately index fixed-size arrays;
//! they mirror the original engine and keep the port auditable.
#![allow(clippy::needless_range_loop)]

pub mod board;
pub mod eval;
pub mod game;
pub mod movegen;
pub mod moveorder;
pub mod netrand;
pub mod search;
pub mod see;
pub mod tt;
pub mod zobrist;

#[cfg(test)]
mod conformance;

pub use board::{Board, Move, BLUE, NO_WINNER, RED, SQUARES};
pub use eval::{evaluate, evaluate_static};
pub use game::{AppliedMove, GameState, MoveError, Status, RULES_DIGEST, RULES_ID};
pub use movegen::{
    can_jump_to, column_major, generate_captures, generate_moves, neighbors, MAX_MOVES,
};
pub use search::{Limits, Search, SearchOutcome};

/// Counts nodes reachable in exactly `depth` plies via make/unmake (count
/// parity only; the order parity lives in the CLI).
pub fn perft(board: &Board, side: u8, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut buf = [Move {
        from: 0,
        to: 0,
        captured_id: 0,
        enters_den: false,
    }; MAX_MOVES];
    let count = generate_moves(board, side, &mut buf);
    if depth == 1 {
        return count as u64;
    }
    let mut nodes = 0u64;
    let mut child = *board;
    for i in 0..count {
        let m = buf[i];
        let undo = child.make_move(&m);
        nodes += perft(&child, side ^ 1, depth - 1);
        child.unmake_move(&m, &undo);
    }
    nodes
}
