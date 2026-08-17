//! The PVS search — a line-for-line port of the C# MinimaxEngine (PVS +
//! aspiration, null-move, futility, LMR, LMP, den-setup extension, qsearch with
//! SEE ordering). Fixed-depth only (no time abort); tablebase off.

use crate::board::*;
use crate::eval::{evaluate, evaluate_static};
use crate::movegen::{
    count_legal_moves, generate_captures, generate_moves, is_adjacent_to_opp_den, is_enemy_trap_square, MAX_MOVES,
};
use crate::moveorder::MoveOrdering;
use crate::rep::RepetitionTable;
use crate::tablebase::Tablebase;
use crate::tt::{Bound, TranspositionTable, MATE_RANGE, MATE_SCORE};

const MATE_THRESHOLD: i32 = MATE_SCORE - MATE_RANGE;
const MAX_PLY: usize = 64;
const ASPIRATION_DELTA: i32 = 25;
const FUTILITY_MARGIN: i32 = 300;
const DELTA_MARGIN: i32 = 200;

const EMPTY_MOVE: Move = Move { from: 0, to: 0, captured_id: 0, enters_den: false };

pub struct Search {
    tt: TranspositionTable,
    order: MoveOrdering,
    rep: RepetitionTable,
    prev_move: [i32; MAX_PLY],
    pub nodes: u64,
    pub last_completed_depth: i32,
    /// Clock in milliseconds since an arbitrary epoch (native: process start;
    /// wasm: performance.now() via an imported function).
    now_ms: fn() -> f64,
    legacy_eval: bool,
    legacy_search: bool,
    legacy_p5: bool,
    legacy_p6: bool,
    tb: Option<Tablebase>,
}

impl Search {
    pub fn new(legacy_eval: bool, legacy_search: bool, legacy_p5: bool, legacy_p6: bool) -> Self {
        Self::with_clock(legacy_eval, legacy_search, legacy_p5, legacy_p6, native_now_ms)
    }

    /// Enables tablebase probing (the caller passes the loaded table). When the
    /// table is absent, the search degrades to pure search (like the C# engine).
    pub fn set_tablebase(&mut self, tb: Tablebase) {
        self.tb = Some(tb);
    }

    /// Constructor with a caller-supplied clock (the wasm backend passes a JS
    /// `performance.now()` import; native uses `Instant`).
    pub fn with_clock(
        legacy_eval: bool,
        legacy_search: bool,
        legacy_p5: bool,
        legacy_p6: bool,
        now_ms: fn() -> f64,
    ) -> Self {
        Search {
            tt: TranspositionTable::new(1 << 20),
            order: MoveOrdering::new(legacy_p5, legacy_p6),
            rep: RepetitionTable::new(),
            prev_move: [-1; MAX_PLY],
            nodes: 0,
            last_completed_depth: 0,
            now_ms,
            legacy_eval,
            legacy_search,
            legacy_p5,
            legacy_p6,
            tb: None,
        }
    }

    /// Runs an iterative-deepening search. `max_depth` caps the depth; an
    /// optional `time_budget_ms` (None = unlimited) stops the loop at the next
    /// depth boundary once the budget is spent — only fully completed depths
    /// are accepted, matching the C# engine. Returns the best move.
    pub fn search_best_move(&mut self, board: &mut Board, max_depth: i32, history: &[u64], time_budget_ms: Option<f64>) -> Option<(u8, u8)> {
        self.tt.new_generation();
        self.rep.reset();
        self.nodes = 0;
        self.last_completed_depth = 0;
        self.rep.seed_path(history);

        let side = board.turn;
        let mut root_moves = [EMPTY_MOVE; MAX_MOVES];
        let move_count = generate_moves(board, side, &mut root_moves);
        if move_count == 0 {
            return None;
        }
        if move_count == 1 {
            self.last_completed_depth = 1;
            return Some((root_moves[0].from, root_moves[0].to));
        }

        let start = (self.now_ms)();
        let mut best_move = (root_moves[0].from, root_moves[0].to);
        let mut best_move_for_ordering = Some(best_move);
        let mut best_score = i32::MIN;

        for depth in 1..=max_depth {
            // Time check at the depth boundary (before starting this iteration).
            if let Some(budget) = time_budget_ms {
                if (self.now_ms)() - start >= budget {
                    break;
                }
            }

            let mut current_best_move = best_move;
            let mut search_alpha = if best_score == i32::MIN { i32::MIN + 1 } else { best_score - ASPIRATION_DELTA };
            let mut search_beta = if best_score == i32::MIN { i32::MAX } else { best_score + ASPIRATION_DELTA };

            let mut current_best;
            loop {
                current_best = i32::MIN;
                self.order.order_moves(board, &mut root_moves[..move_count], move_count, best_move_for_ordering, 0, -1);

                for i in 0..move_count {
                    let m = root_moves[i];
                    let undo = board.make_move(&m);
                    if !self.legacy_p5 {
                        self.prev_move[0] = m.from as i32 | ((m.to as i32) << 6);
                    }
                    let mut score;
                    if i == 0 {
                        // Root PV child searched with the UN-negated window (pinned quirk).
                        score = -self.pv_search(board, depth - 1, search_alpha, search_beta, 1, 0);
                    } else {
                        score = -self.pv_search(board, depth - 1, -(current_best + 1), -current_best, 1, 0);
                        if score > current_best {
                            score = -self.pv_search(board, depth - 1, -search_beta, -current_best, 1, 0);
                        }
                    }
                    board.unmake_move(&m, &undo);

                    if score > current_best {
                        current_best = score;
                        current_best_move = (m.from, m.to);
                    }
                }

                if current_best <= search_alpha || current_best >= search_beta {
                    if current_best <= search_alpha {
                        search_alpha = i32::MIN + 1;
                    }
                    if current_best >= search_beta {
                        search_beta = i32::MAX;
                    }
                } else {
                    break;
                }
            }

            best_score = current_best;
            best_move = current_best_move;
            best_move_for_ordering = Some(current_best_move);
            self.order.age_history();
            self.last_completed_depth = depth;
            if best_score >= MATE_THRESHOLD {
                break;
            }
        }

        Some(best_move)
    }

    fn pv_search(&mut self, board: &mut Board, depth: i32, mut alpha: i32, beta: i32, ply: usize, extensions: i32) -> i32 {
        self.nodes += 1;

        if board.winner != NO_WINNER {
            return terminal_score(board.winner, board.turn, ply);
        }

        if self.rep.is_repetition(board.hash) {
            return 0; // contempt = 0 -> draw is neutral
        }

        // Tablebase probe (before the TT): exact WDL for <= 3-piece positions.
        if let Some(tb) = &self.tb {
            let count = board.count[0] as usize + board.count[1] as usize;
            if count >= 2 && count <= 3 {
                let (found, score) = tb.try_probe(board, ply as i32);
                if found {
                    if score != 0 {
                        self.tt.store(board.hash, 127, adjust_mate_for_store(score, ply as i32), None, Bound::Exact);
                        return score;
                    }
                    return 0; // tablebase draw -> neutral (contempt = 0)
                }
            }
        }

        let (cut, tt_score, tt_move) = self.tt.try_probe(board.hash, depth, alpha, beta, ply as i32);
        if cut {
            return tt_score;
        }

        if depth == 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let side = board.turn;

        // Null-move pruning with verification.
        if !self.legacy_search && depth >= 3 && beta < MATE_THRESHOLD
            && (board.count[0] + board.count[1]) as i32 > 6
            && board.count[side as usize] > 2
        {
            const NULL_REDUCTION: i32 = 2;
            board.make_null_move();
            if !self.legacy_p5 {
                self.prev_move[ply] = -1;
            }
            let null_score = -self.pv_search(board, depth - 1 - NULL_REDUCTION, -beta, -(beta - 1), ply + 1, extensions);
            board.unmake_null_move();

            if null_score >= beta {
                let verify = self.pv_search(board, depth - 1 - NULL_REDUCTION, alpha, beta, ply, extensions);
                if verify >= beta {
                    return beta;
                }
            }
        }

        let mut moves = [EMPTY_MOVE; MAX_MOVES];
        let move_count = generate_moves(board, side, &mut moves);
        if move_count == 0 {
            return -(MATE_SCORE - ply as i32);
        }

        let prev_packed = if !self.legacy_p5 && ply > 0 { self.prev_move[ply - 1] } else { -1 };
        self.order.order_moves(board, &mut moves[..move_count], move_count, tt_move, ply, prev_packed);

        let mut best_move = (moves[0].from, moves[0].to);
        let mut best_score = i32::MIN;
        let mut bound = Bound::Upper;
        let mut any_searched = false;

        let futility_applicable = !self.legacy_search && depth == 1
            && alpha > -MATE_THRESHOLD && alpha < MATE_THRESHOLD
            && (board.count[0] + board.count[1]) as i32 > 8;
        let mut static_eval = 0;
        if futility_applicable {
            static_eval = evaluate_static(board, side, self.legacy_eval, self.legacy_p5);
        }

        let path_pushed = self.rep.can_push();
        if path_pushed {
            self.rep.push(board.hash);
        }

        for i in 0..move_count {
            let m = moves[i];
            let den_setup = !self.legacy_p6 && !m.is_capture() && !m.enters_den
                && is_adjacent_to_opp_den(m.to, side);

            if futility_applicable && !m.is_capture() && !m.enters_den
                && !is_enemy_trap_square(m.to, side)
                && eff_rank(board.occupant(m.from as usize), m.from) != 0
                && static_eval + FUTILITY_MARGIN <= alpha
            {
                continue;
            }

            if !self.legacy_p6 && depth <= 3 && !m.is_capture() && !m.enters_den
                && !den_setup && i as i32 >= 4 + depth * depth
                && alpha > -MATE_THRESHOLD && alpha < MATE_THRESHOLD
                && board.count[side as usize] > 2
            {
                continue;
            }

            any_searched = true;

            let extend = den_setup && depth >= 2 && depth <= 4
                && extensions < 2
                && alpha > -MATE_THRESHOLD && beta < MATE_THRESHOLD;
            let child_depth = depth - 1 + if extend { 1 } else { 0 };
            let child_extensions = extensions + if extend { 1 } else { 0 };

            let undo = board.make_move(&m);
            if !self.legacy_p5 {
                self.prev_move[ply] = m.from as i32 | ((m.to as i32) << 6);
            }

            let mut score;
            if i == 0 {
                score = -self.pv_search(board, child_depth, -beta, -alpha, ply + 1, child_extensions);
            } else if i >= 3 && depth >= 3 && !m.is_capture() {
                let reduce = !self.legacy_search && !m.enters_den && !den_setup && i < move_count - 2;
                let reduced_depth = if reduce {
                    (depth - 1 - (1 + if i >= 6 { 1 } else { 0 } + if depth >= 8 { 1 } else { 0 })).max(1)
                } else {
                    depth - 2
                };
                score = -self.pv_search(board, reduced_depth, -(alpha + 1), -alpha, ply + 1, child_extensions);
                if score > alpha {
                    score = -self.pv_search(board, child_depth, -(alpha + 1), -alpha, ply + 1, child_extensions);
                }
                if score > alpha && score < beta {
                    score = -self.pv_search(board, child_depth, -beta, -alpha, ply + 1, child_extensions);
                }
            } else {
                score = -self.pv_search(board, child_depth, -(alpha + 1), -alpha, ply + 1, child_extensions);
                if score > alpha && score < beta {
                    score = -self.pv_search(board, child_depth, -beta, -alpha, ply + 1, child_extensions);
                }
            }

            board.unmake_move(&m, &undo);

            if score > best_score {
                best_score = score;
                best_move = (m.from, m.to);
            }

            if score >= beta {
                self.order.on_beta_cutoff(&m, depth, ply, prev_packed);
                self.tt.store(board.hash, depth, adjust_mate_for_store(score, ply as i32), Some((m.from, m.to)), Bound::Lower);
                if path_pushed {
                    self.rep.pop(board.hash);
                }
                return score;
            }

            if score > alpha {
                alpha = score;
                bound = Bound::Exact;
            }
        }

        if path_pushed {
            self.rep.pop(board.hash);
        }

        if !any_searched {
            return static_eval;
        }

        self.tt.store(board.hash, depth, adjust_mate_for_store(best_score, ply as i32), Some(best_move), bound);
        best_score
    }

    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;

        if board.winner != NO_WINNER {
            return terminal_score(board.winner, board.turn, ply);
        }

        // Tablebase probe at the qsearch top (a <= 3-piece position is fully
        // resolved; the exact score beats searching captures).
        if let Some(tb) = &self.tb {
            let count = board.count[0] as usize + board.count[1] as usize;
            if count >= 2 && count <= 3 {
                let (found, score) = tb.try_probe(board, ply as i32);
                if found {
                    return score; // includes 0 for a draw
                }
            }
        }

        let side = board.turn;

        let q_mob_side = count_legal_moves(board, side);
        let q_mob_opp = count_legal_moves(board, side ^ 1);
        let mut stand_pat = if self.legacy_search {
            evaluate(board, side, q_mob_side, q_mob_opp, self.legacy_eval, self.legacy_p5)
        } else {
            evaluate_static(board, side, self.legacy_eval, self.legacy_p5)
        };

        if stand_pat >= beta {
            return beta;
        }

        if !self.legacy_search && stand_pat > alpha {
            stand_pat = evaluate(board, side, count_legal_moves(board, side), count_legal_moves(board, side ^ 1), self.legacy_eval, self.legacy_p5);
            if stand_pat >= beta {
                return beta;
            }
        }

        if stand_pat > alpha {
            alpha = stand_pat;
        }

        if ply > 32 {
            return alpha;
        }

        let mut moves = [EMPTY_MOVE; MAX_MOVES];
        let move_count = generate_captures(board, side, &mut moves);

        if !self.legacy_p5 {
            self.order.order_captures_by_see(board, &mut moves[..move_count], move_count);
        } else {
            self.order.sort_captures(board, &mut moves[..move_count], move_count);
        }

        for i in 0..move_count {
            let m = moves[i];

            if !self.legacy_search && m.is_capture() {
                let victim_gain = eff_rank(m.captured_id, m.to) as i32 * 100;
                if stand_pat + victim_gain + DELTA_MARGIN <= alpha {
                    continue;
                }
            }

            if !self.legacy_p5 && m.is_capture() {
                let see_score = self.order.see_scores[i];
                if see_score < 0 && !is_enemy_trap_square(m.to, side) {
                    continue;
                }
            }

            let undo = board.make_move(&m);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.unmake_move(&m, &undo);

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }
}

fn terminal_score(winner: u8, side_to_move: u8, ply: usize) -> i32 {
    if winner == side_to_move {
        MATE_SCORE - ply as i32
    } else {
        ply as i32 - MATE_SCORE
    }
}

/// Native clock: milliseconds since an arbitrary process-local epoch.
fn native_now_ms() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64() * 1000.0
}

fn adjust_mate_for_store(score: i32, ply: i32) -> i32 {
    if score > MATE_THRESHOLD {
        (score + ply).min(MATE_SCORE)
    } else if score < -MATE_THRESHOLD {
        (score - ply).max(-MATE_SCORE)
    } else {
        score
    }
}
