//! Static exchange evaluation — port of the C# SeeEvaluator.

use crate::board::*;
use crate::movegen::{can_capture, can_jump_to, neighbors};

const MAX_SEE_PLY: i32 = 6;

pub fn see(board: &Board, attacker: u8, victim: u8, square: u8) -> i32 {
    rank_of(victim) as i32 * 100 - exchange(board, square, board.turn ^ 1, attacker, 1)
}

fn exchange(board: &Board, square: u8, side: u8, victim: u8, depth: i32) -> i32 {
    if depth >= MAX_SEE_PLY {
        return 0;
    }
    let mut best = 0i32;
    for from in 0..SQUARES as u8 {
        let id = board.squares[from as usize];
        if id == 0 || owner_of(id) != side {
            continue;
        }
        if !attacks(board, id, from, square) {
            continue;
        }
        if !can_capture(id, from, victim, square) {
            continue;
        }
        let score = rank_of(victim) as i32 * 100 - exchange(board, square, side ^ 1, id, depth + 1);
        if score > best {
            best = score;
        }
    }
    best
}

fn attacks(board: &Board, id: u8, from: u8, square: u8) -> bool {
    for &n in neighbors(from) {
        if n == square {
            return true;
        }
    }
    can_jump_to(board, id, from, square)
}
