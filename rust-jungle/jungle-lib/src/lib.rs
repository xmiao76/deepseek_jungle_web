//! Jungle (Dou Shou Qi) core for the WASM port: board, movegen, Zobrist.

pub mod board;
pub mod eval;
pub mod movegen;
pub mod moveorder;
pub mod netrand;
pub mod rep;
pub mod search;
pub mod see;
pub mod tablebase;
pub mod tt;
pub mod zobrist;

pub use board::{Board, Move, NO_WINNER, SQUARES};
pub use eval::{evaluate, evaluate_static};
pub use movegen::{generate_moves, generate_captures, can_jump_to, neighbors, column_major, MAX_MOVES};
pub use search::Search;

/// Counts nodes reachable in exactly `depth` plies via make/unmake (count
/// parity only; the order parity lives in the CLI).
pub fn perft(board: &Board, side: u8, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut buf = [Move { from: 0, to: 0, captured_id: 0, enters_den: false }; MAX_MOVES];
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
