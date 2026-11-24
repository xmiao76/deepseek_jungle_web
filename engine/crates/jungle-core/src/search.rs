//! PVS search with node-budget and wall-clock limits, v2 draw awareness
//! (threefold repetition and the 100-ply no-capture rule) and deterministic
//! component mode.

use crate::board::*;
use crate::eval::{evaluate, evaluate_static};
use crate::movegen::{
    count_legal_moves, generate_captures, generate_moves, is_adjacent_to_opp_den,
    is_enemy_trap_square, MAX_MOVES,
};
use crate::moveorder::MoveOrdering;
use crate::tt::{Bound, TranspositionTable, MATE_RANGE, MATE_SCORE};

const MATE_THRESHOLD: i32 = MATE_SCORE - MATE_RANGE;
const MAX_PLY: usize = 64;
const ASPIRATION_DELTA: i32 = 25;
const FUTILITY_MARGIN: i32 = 300;
const DELTA_MARGIN: i32 = 200;
const NO_CAPTURE_PLIES: u32 = 100;
const REPETITION_OCCURRENCES: usize = 3;
const TIME_CHECK_MASK: u64 = 1023;

const EMPTY_MOVE: Move = Move {
    from: 0,
    to: 0,
    captured_id: 0,
    enters_den: false,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Limits {
    pub max_nodes: Option<u64>,
    /// Wall-clock budget in milliseconds. Requires a clock on the Search.
    pub budget_ms: Option<f64>,
}

impl Limits {
    pub fn nodes(max: u64) -> Self {
        Limits {
            max_nodes: Some(max),
            budget_ms: None,
        }
    }

    pub fn time(ms: f64) -> Self {
        Limits {
            max_nodes: None,
            budget_ms: Some(ms),
        }
    }

    pub fn nodes_or_time(max: u64, ms: f64) -> Self {
        Limits {
            max_nodes: Some(max),
            budget_ms: Some(ms),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchOutcome {
    pub best: Option<Move>,
    pub score: i32,
    pub depth: i32,
    pub nodes: u64,
    pub aborted: bool,
    pub elapsed_ms: f64,
}

pub struct Search {
    tt: TranspositionTable,
    order: MoveOrdering,
    rep: Vec<u64>,
    prev_move: [i32; MAX_PLY],
    pub nodes: u64,
    pub last_completed_depth: i32,
    pub last_score: i32,
    aborted: bool,
    max_nodes: Option<u64>,
    deadline_ms: Option<f64>,
    now_ms: Option<fn() -> f64>,
    start_ms: f64,
    quiet: u32,
    draw_awareness: bool,
}

impl Search {
    /// A search with no clock. Use `with_clock` for wall-clock limits.
    pub fn new(tt_entries: usize) -> Self {
        Search {
            tt: TranspositionTable::new(tt_entries),
            order: MoveOrdering::new(false, false),
            rep: Vec::with_capacity(256),
            prev_move: [-1; MAX_PLY],
            nodes: 0,
            last_completed_depth: 0,
            last_score: 0,
            aborted: false,
            max_nodes: None,
            deadline_ms: None,
            now_ms: None,
            start_ms: 0.0,
            quiet: 0,
            draw_awareness: true,
        }
    }

    /// Disables repetition/no-capture draw scoring (used to isolate search
    /// behaviour from the v2 draw rules in the strength harness).
    pub fn set_draw_awareness(&mut self, enabled: bool) {
        self.draw_awareness = enabled;
    }

    pub fn with_clock(tt_entries: usize, now_ms: fn() -> f64) -> Self {
        let mut search = Self::new(tt_entries);
        search.now_ms = Some(now_ms);
        search
    }

    /// Runs iterative deepening. `history` holds every position hash from the
    /// start up to and including the root; `quiet` is the root no-capture
    /// counter. With `fresh`, TT and ordering history are reset first (the
    /// deterministic Arena mode).
    pub fn search(
        &mut self,
        board: &mut Board,
        history: &[u64],
        quiet: u32,
        limits: Limits,
        max_depth: i32,
        fresh: bool,
    ) -> SearchOutcome {
        if fresh {
            self.tt.clear();
            self.order = MoveOrdering::new(false, false);
            self.prev_move = [-1; MAX_PLY];
        } else {
            self.tt.new_generation();
        }
        self.nodes = 1; // the root position counts
        self.last_completed_depth = 0;
        self.last_score = 0;
        self.aborted = false;
        self.max_nodes = limits.max_nodes;
        self.quiet = quiet;
        self.start_ms = self.now_ms.map(|now| now()).unwrap_or(0.0);
        self.deadline_ms = match (limits.budget_ms, self.now_ms) {
            (Some(ms), Some(_)) => Some(self.start_ms + ms.max(0.0)),
            _ => None,
        };

        self.rep.clear();
        self.rep.extend_from_slice(history);
        if self.rep.last() != Some(&board.hash) {
            self.rep.push(board.hash);
        }

        let side = board.turn;
        let mut root_moves = [EMPTY_MOVE; MAX_MOVES];
        let move_count = generate_moves(board, side, &mut root_moves);
        if move_count == 0 {
            return self.finish(None, 0);
        }
        if move_count == 1 {
            self.last_completed_depth = 1;
            return self.finish(Some(root_moves[0]), 0);
        }
        if self.max_nodes == Some(0) {
            self.aborted = true;
            return self.finish(Some(root_moves[0]), 0);
        }

        let mut best_move = root_moves[0];
        let mut best_score = i32::MIN;
        let mut best_for_ordering = Some((root_moves[0].from, root_moves[0].to));

        for depth in 1..=max_depth.max(1) {
            if self.should_stop() {
                break;
            }
            let mut iteration_best = best_move;
            let mut iteration_score = i32::MIN;
            let mut search_alpha = if best_score == i32::MIN {
                -MATE_SCORE * 2
            } else {
                best_score - ASPIRATION_DELTA
            };
            let mut search_beta = if best_score == i32::MIN {
                MATE_SCORE * 2
            } else {
                best_score + ASPIRATION_DELTA
            };

            loop {
                let mut current_best = i32::MIN;
                self.order.order_moves(
                    board,
                    &mut root_moves[..move_count],
                    move_count,
                    best_for_ordering,
                    0,
                    -1,
                );

                for i in 0..move_count {
                    if self.should_stop() {
                        break;
                    }
                    let mv = root_moves[i];
                    let child_quiet = if mv.is_capture() { 0 } else { self.quiet + 1 };
                    let undo = board.make_move(&mv);
                    self.prev_move[0] = mv.from as i32 | ((mv.to as i32) << 6);
                    self.rep.push(board.hash);
                    let mut score;
                    if i == 0 {
                        score = -self.pv_search(
                            board,
                            depth - 1,
                            search_alpha,
                            search_beta,
                            1,
                            child_quiet,
                            0,
                        );
                    } else {
                        score = -self.pv_search(
                            board,
                            depth - 1,
                            -(current_best + 1),
                            -current_best,
                            1,
                            child_quiet,
                            0,
                        );
                        if !self.aborted && score > current_best {
                            score = -self.pv_search(
                                board,
                                depth - 1,
                                -search_beta,
                                -current_best,
                                1,
                                child_quiet,
                                0,
                            );
                        }
                    }
                    self.rep.pop();
                    board.unmake_move(&mv, &undo);
                    if self.aborted {
                        break;
                    }

                    if score > current_best {
                        current_best = score;
                        iteration_best = mv;
                    }
                }

                if self.aborted {
                    break;
                }
                if current_best <= search_alpha {
                    search_alpha = -MATE_SCORE * 2;
                    continue;
                }
                if current_best >= search_beta {
                    search_beta = MATE_SCORE * 2;
                    continue;
                }
                iteration_score = current_best;
                break;
            }

            if self.aborted {
                break;
            }
            best_move = iteration_best;
            best_score = iteration_score;
            best_for_ordering = Some((best_move.from, best_move.to));
            self.order.age_history();
            self.last_completed_depth = depth;
            self.last_score = best_score;
            if best_score >= MATE_THRESHOLD || best_score <= -MATE_THRESHOLD {
                break; // proven win or loss: more search cannot change the move
            }
        }

        if best_score == i32::MIN {
            best_score = 0;
        }
        self.finish(Some(best_move), best_score)
    }

    fn finish(&self, best: Option<Move>, score: i32) -> SearchOutcome {
        let elapsed_ms = match self.now_ms {
            Some(now) => (now() - self.start_ms).max(0.0),
            None => 0.0,
        };
        SearchOutcome {
            best,
            score,
            depth: self.last_completed_depth,
            nodes: self.nodes,
            aborted: self.aborted,
            elapsed_ms,
        }
    }

    fn should_stop(&mut self) -> bool {
        if self.aborted {
            return true;
        }
        if let Some(max) = self.max_nodes {
            if self.nodes >= max {
                self.aborted = true;
                return true;
            }
        }
        if let Some(deadline) = self.deadline_ms {
            if self.nodes & TIME_CHECK_MASK == 0 {
                if let Some(now) = self.now_ms {
                    if now() >= deadline {
                        self.aborted = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn pv_search(
        &mut self,
        board: &mut Board,
        depth: i32,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        quiet: u32,
        extensions: i32,
    ) -> i32 {
        if self.should_stop() {
            return 0;
        }
        self.nodes += 1;

        if board.winner != NO_WINNER {
            return terminal_score(board.winner, board.turn, ply);
        }
        if self.draw_awareness && self.rep_count(board.hash) >= REPETITION_OCCURRENCES {
            return 0;
        }
        if self.draw_awareness && quiet >= NO_CAPTURE_PLIES {
            return 0;
        }

        let (cut, tt_score, tt_move) = self
            .tt
            .try_probe(board.hash, depth, alpha, beta, ply as i32);
        if cut {
            return tt_score;
        }

        if depth <= 0 {
            return self.quiescence(board, alpha, beta, ply);
        }

        let side = board.turn;

        // Null-move pruning with verification.
        if depth >= 3
            && beta < MATE_THRESHOLD
            && (board.count[0] + board.count[1]) as i32 > 6
            && board.count[side as usize] > 2
        {
            const NULL_REDUCTION: i32 = 2;
            board.make_null_move();
            self.prev_move[ply] = -1;
            let null_score = -self.pv_search(
                board,
                depth - 1 - NULL_REDUCTION,
                -beta,
                -(beta - 1),
                ply + 1,
                quiet + 1,
                extensions,
            );
            board.unmake_null_move();
            if self.aborted {
                return 0;
            }
            if null_score >= beta {
                let verify = self.pv_search(
                    board,
                    depth - 1 - NULL_REDUCTION,
                    alpha,
                    beta,
                    ply,
                    quiet,
                    extensions,
                );
                if self.aborted {
                    return 0;
                }
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

        let prev_packed = if ply > 0 { self.prev_move[ply - 1] } else { -1 };
        self.order.order_moves(
            board,
            &mut moves[..move_count],
            move_count,
            tt_move,
            ply,
            prev_packed,
        );

        let mut best_move = (moves[0].from, moves[0].to);
        let mut best_score = i32::MIN;
        let mut bound = Bound::Upper;
        let mut any_searched = false;

        let futility_applicable = depth == 1
            && alpha > -MATE_THRESHOLD
            && alpha < MATE_THRESHOLD
            && (board.count[0] + board.count[1]) as i32 > 8;
        let mut static_eval = 0;
        if futility_applicable {
            static_eval = evaluate_static(board, side, false, false);
        }

        for i in 0..move_count {
            if self.should_stop() {
                return 0;
            }
            let mv = moves[i];
            let den_setup =
                !mv.is_capture() && !mv.enters_den && is_adjacent_to_opp_den(mv.to, side);

            if futility_applicable
                && !mv.is_capture()
                && !mv.enters_den
                && !is_enemy_trap_square(mv.to, side)
                && eff_rank(board.occupant(mv.from as usize), mv.from) != 0
                && static_eval + FUTILITY_MARGIN <= alpha
            {
                continue;
            }

            if depth <= 3
                && !mv.is_capture()
                && !mv.enters_den
                && !den_setup
                && i as i32 >= 4 + depth * depth
                && alpha > -MATE_THRESHOLD
                && alpha < MATE_THRESHOLD
                && board.count[side as usize] > 2
            {
                continue;
            }

            any_searched = true;

            let extend = den_setup
                && (2..=4).contains(&depth)
                && extensions < 2
                && alpha > -MATE_THRESHOLD
                && beta < MATE_THRESHOLD;
            let child_depth = depth - 1 + if extend { 1 } else { 0 };
            let child_extensions = extensions + if extend { 1 } else { 0 };
            let child_quiet = if mv.is_capture() { 0 } else { quiet + 1 };

            let undo = board.make_move(&mv);
            self.prev_move[ply] = mv.from as i32 | ((mv.to as i32) << 6);
            self.rep.push(board.hash);

            let mut score;
            if i == 0 {
                score = -self.pv_search(
                    board,
                    child_depth,
                    -beta,
                    -alpha,
                    ply + 1,
                    child_quiet,
                    child_extensions,
                );
            } else if i >= 3 && depth >= 3 && !mv.is_capture() {
                let reduce = !mv.enters_den && !den_setup && i < move_count - 2;
                let reduced_depth = if reduce {
                    (depth - 1 - (1 + if i >= 6 { 1 } else { 0 } + if depth >= 8 { 1 } else { 0 }))
                        .max(1)
                } else {
                    depth - 2
                };
                score = -self.pv_search(
                    board,
                    reduced_depth,
                    -(alpha + 1),
                    -alpha,
                    ply + 1,
                    child_quiet,
                    child_extensions,
                );
                if !self.aborted && score > alpha {
                    score = -self.pv_search(
                        board,
                        child_depth,
                        -(alpha + 1),
                        -alpha,
                        ply + 1,
                        child_quiet,
                        child_extensions,
                    );
                }
                if !self.aborted && score > alpha && score < beta {
                    score = -self.pv_search(
                        board,
                        child_depth,
                        -beta,
                        -alpha,
                        ply + 1,
                        child_quiet,
                        child_extensions,
                    );
                }
            } else {
                score = -self.pv_search(
                    board,
                    child_depth,
                    -(alpha + 1),
                    -alpha,
                    ply + 1,
                    child_quiet,
                    child_extensions,
                );
                if !self.aborted && score > alpha && score < beta {
                    score = -self.pv_search(
                        board,
                        child_depth,
                        -beta,
                        -alpha,
                        ply + 1,
                        child_quiet,
                        child_extensions,
                    );
                }
            }

            self.rep.pop();
            board.unmake_move(&mv, &undo);
            if self.aborted {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = (mv.from, mv.to);
            }

            if score >= beta {
                self.order.on_beta_cutoff(&mv, depth, ply, prev_packed);
                self.tt.store(
                    board.hash,
                    depth,
                    adjust_mate_for_store(score, ply as i32),
                    Some((mv.from, mv.to)),
                    Bound::Lower,
                );
                return score;
            }

            if score > alpha {
                alpha = score;
                bound = Bound::Exact;
            }
        }

        if !any_searched {
            return static_eval;
        }

        self.tt.store(
            board.hash,
            depth,
            adjust_mate_for_store(best_score, ply as i32),
            Some(best_move),
            bound,
        );
        best_score
    }

    fn quiescence(&mut self, board: &mut Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        if self.should_stop() {
            return 0;
        }
        self.nodes += 1;

        if board.winner != NO_WINNER {
            return terminal_score(board.winner, board.turn, ply);
        }

        let side = board.turn;
        let q_mob_side = count_legal_moves(board, side);
        if q_mob_side == 0 {
            return -(MATE_SCORE - ply as i32);
        }
        let q_mob_opp = count_legal_moves(board, side ^ 1);
        let mut stand_pat = evaluate_static(board, side, false, false);

        if stand_pat >= beta {
            return beta;
        }
        if stand_pat > alpha {
            stand_pat = evaluate(board, side, q_mob_side, q_mob_opp, false, false);
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
        self.order
            .order_captures_by_see(board, &mut moves[..move_count], move_count);

        for i in 0..move_count {
            if self.should_stop() {
                return 0;
            }
            let mv = moves[i];
            if mv.is_capture() {
                let victim_gain = eff_rank(mv.captured_id, mv.to) as i32 * 100;
                if stand_pat + victim_gain + DELTA_MARGIN <= alpha {
                    continue;
                }
                let see_score = self.order.see_scores[i];
                if see_score < 0 && !is_enemy_trap_square(mv.to, side) {
                    continue;
                }
            }

            let undo = board.make_move(&mv);
            let score = -self.quiescence(board, -beta, -alpha, ply + 1);
            board.unmake_move(&mv, &undo);
            if self.aborted {
                return 0;
            }

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    fn rep_count(&self, hash: u64) -> usize {
        self.rep.iter().filter(|&&h| h == hash).count()
    }
}

fn terminal_score(winner: u8, side_to_move: u8, ply: usize) -> i32 {
    if winner == side_to_move {
        MATE_SCORE - ply as i32
    } else {
        ply as i32 - MATE_SCORE
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameState;
    use crate::Board;

    fn search_nodes(game: &GameState, budget: u64) -> SearchOutcome {
        let mut board = game.board;
        let mut search = Search::new(1 << 16);
        search.search(
            &mut board,
            &game.hashes,
            game.quiet,
            Limits::nodes(budget),
            16,
            true,
        )
    }

    #[test]
    fn node_budget_is_a_ceiling_and_deterministic() {
        let game = GameState::initial();
        let a = search_nodes(&game, 5000);
        let b = search_nodes(&game, 5000);
        assert!(a.nodes <= 5000, "nodes {} > budget", a.nodes);
        assert_eq!(a.nodes, b.nodes);
        assert_eq!(a.depth, b.depth);
        assert_eq!(
            a.best.map(|m| (m.from, m.to)),
            b.best.map(|m| (m.from, m.to))
        );
        let best = a.best.expect("a move");
        assert!(game
            .legal_moves()
            .iter()
            .any(|m| m.from == best.from && m.to == best.to));
    }

    #[test]
    fn tiny_budget_still_returns_a_legal_move() {
        let game = GameState::initial();
        let out = search_nodes(&game, 1);
        let best = out.best.expect("fallback");
        assert_eq!(out.nodes, 1);
        assert!(game
            .legal_moves()
            .iter()
            .any(|m| m.from == best.from && m.to == best.to));
    }

    #[test]
    fn quiet_limit_is_scored_as_a_draw() {
        // Sparse synthetic race with no captures available.
        let board = Board::from_packed(&[2 + RED, 4 + BLUE], &[42, 6], RED);
        let game = GameState::from_board(board, RED, 99);
        let out = search_nodes(&game, 4000);
        assert_eq!(out.score, 0, "every move hits the 100-ply draw");
    }
}
