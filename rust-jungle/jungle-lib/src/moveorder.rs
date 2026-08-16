//! Move ordering — port of the C# MoveOrdering (killers, history, counter-move,
//! capture history, SEE-sign), with precomputed score scratch.

use crate::board::*;
use crate::movegen::MAX_MOVES;
use crate::see::see;

const MAX_PLY: usize = 64;
const TT_MOVE_BONUS: i32 = 1_000_000;
const KILLER1: i32 = 900;
const KILLER2: i32 = 800;
const COUNTER1: i32 = 700;
const COUNTER2: i32 = 600;
const SEE_SIGN_BONUS: i32 = 50;
const HISTORY_CAP: i32 = 500;
const COUNTER_TABLE_SIZE: usize = 1 << 12;

pub struct MoveOrdering {
    pub killers: [[i32; 2]; MAX_PLY],
    pub history: [[i32; 63]; 63],
    pub counter_move0: [i32; COUNTER_TABLE_SIZE],
    pub counter_move1: [i32; COUNTER_TABLE_SIZE],
    capture_history: [[[i32; 63]; 63]; 9],
    score_scratch: [i32; MAX_MOVES],
    pub see_scores: [i32; MAX_MOVES],
    legacy_p5: bool,
    legacy_p6: bool,
}

impl MoveOrdering {
    pub fn new(legacy_p5: bool, legacy_p6: bool) -> Self {
        MoveOrdering {
            killers: [[0; 2]; MAX_PLY],
            history: [[0; 63]; 63],
            counter_move0: [0; COUNTER_TABLE_SIZE],
            counter_move1: [0; COUNTER_TABLE_SIZE],
            capture_history: [[[0; 63]; 63]; 9],
            score_scratch: [0; MAX_MOVES],
            see_scores: [0; MAX_MOVES],
            legacy_p5,
            legacy_p6,
        }
    }

    pub fn on_beta_cutoff(&mut self, m: &Move, depth: i32, ply: usize, prev_packed: i32) {
        if ply >= MAX_PLY {
            return;
        }
        if m.is_capture() {
            if !self.legacy_p6 {
                self.capture_history[rank_of(m.captured_id) as usize][m.from as usize][m.to as usize] += depth * depth;
            }
            return;
        }
        let packed = m.from as i32 | ((m.to as i32) << 6);
        self.killers[ply][1] = self.killers[ply][0];
        self.killers[ply][0] = packed;
        self.history[m.from as usize][m.to as usize] += depth * depth;
        if !self.legacy_p5 && prev_packed >= 0 {
            let pp = prev_packed as usize;
            self.counter_move1[pp] = self.counter_move0[pp];
            self.counter_move0[pp] = packed;
        }
    }

    pub fn age_history(&mut self) {
        for f in 0..63 {
            for t in 0..63 {
                self.history[f][t] /= 2;
                for r in 1..=8 {
                    self.capture_history[r][f][t] /= 2;
                }
            }
        }
    }

    pub fn order_moves(&mut self, board: &Board, moves: &mut [Move], count: usize, tt_move: Option<(u8, u8)>, ply: usize, prev_packed: i32) {
        for i in 0..count {
            self.score_scratch[i] = self.move_score(board, &moves[i], tt_move, ply, prev_packed);
        }
        for i in 1..count {
            let m = moves[i];
            let score = self.score_scratch[i];
            let mut j = i as isize - 1;
            while j >= 0 && self.score_scratch[j as usize] < score {
                self.score_scratch[j as usize + 1] = self.score_scratch[j as usize];
                moves[j as usize + 1] = moves[j as usize];
                j -= 1;
            }
            self.score_scratch[j as usize + 1] = score;
            moves[j as usize + 1] = m;
        }
    }

    pub fn sort_captures(&mut self, board: &Board, moves: &mut [Move], count: usize) {
        for i in 0..count {
            self.score_scratch[i] = self.capture_score(board, &moves[i]);
        }
        for i in 1..count {
            let m = moves[i];
            let score = self.score_scratch[i];
            let mut j = i as isize - 1;
            while j >= 0 && self.score_scratch[j as usize] < score {
                self.score_scratch[j as usize + 1] = self.score_scratch[j as usize];
                moves[j as usize + 1] = moves[j as usize];
                j -= 1;
            }
            self.score_scratch[j as usize + 1] = score;
            moves[j as usize + 1] = m;
        }
    }

    pub fn order_captures_by_see(&mut self, board: &Board, moves: &mut [Move], count: usize) {
        for i in 0..count {
            let m = &moves[i];
            self.see_scores[i] = if m.is_capture() {
                see(board, board.occupant(m.from as usize), m.captured_id, m.to)
            } else {
                i32::MAX
            };
        }
        for i in 1..count {
            let m = moves[i];
            let score = self.see_scores[i];
            let mut j = i as isize - 1;
            while j >= 0 && self.see_scores[j as usize] < score {
                self.see_scores[j as usize + 1] = self.see_scores[j as usize];
                moves[j as usize + 1] = moves[j as usize];
                j -= 1;
            }
            self.see_scores[j as usize + 1] = score;
            moves[j as usize + 1] = m;
        }
    }

    fn move_score(&self, board: &Board, m: &Move, tt_move: Option<(u8, u8)>, ply: usize, prev_packed: i32) -> i32 {
        if let Some((tf, tt)) = tt_move {
            if tf == m.from && tt == m.to {
                return TT_MOVE_BONUS;
            }
        }
        if m.is_capture() {
            let victim_rank = rank_of(m.captured_id) as i32;
            let attacker_rank = rank_of(board.occupant(m.from as usize)) as i32;
            let mut score = victim_rank * 100 - attacker_rank;
            if !self.legacy_p6 {
                score += if see(board, board.occupant(m.from as usize), m.captured_id, m.to) >= 0 { SEE_SIGN_BONUS } else { -SEE_SIGN_BONUS };
                score += self.capture_history[victim_rank as usize][m.from as usize][m.to as usize].min(HISTORY_CAP);
            }
            return score;
        }
        let mut score = 0;
        let packed = m.from as i32 | ((m.to as i32) << 6);
        if ply < MAX_PLY {
            if packed == self.killers[ply][0] {
                score = KILLER1;
            } else if packed == self.killers[ply][1] {
                score = KILLER2;
            }
        }
        if !self.legacy_p5 && prev_packed >= 0 {
            let pp = prev_packed as usize;
            if packed == self.counter_move0[pp] {
                score += COUNTER1;
            } else if packed == self.counter_move1[pp] {
                score += COUNTER2;
            }
        }
        score += self.history[m.from as usize][m.to as usize].min(HISTORY_CAP);
        score
    }

    fn capture_score(&self, board: &Board, m: &Move) -> i32 {
        let victim_rank = if m.is_capture() { rank_of(m.captured_id) as i32 } else { 9 };
        let attacker_rank = rank_of(board.occupant(m.from as usize)) as i32;
        let mut score = victim_rank * 10 - attacker_rank;
        if m.is_capture() && !self.legacy_p6 {
            score += if see(board, board.occupant(m.from as usize), m.captured_id, m.to) >= 0 { SEE_SIGN_BONUS } else { -SEE_SIGN_BONUS };
            score += self.capture_history[victim_rank as usize][m.from as usize][m.to as usize].min(HISTORY_CAP);
        }
        score
    }
}
