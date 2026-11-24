//! Game-level rules: canonical opening, legal list, move application,
//! adjudication (den/elimination/immobilization/repetition/no-capture),
//! history and replay-based undo.

use crate::board::{Board, Move, BLUE, NO_WINNER, RED, SQUARES};
use crate::movegen::{generate_moves, MAX_MOVES};

pub const RULES_ID: &str = "desktop-jungle-v2";
pub const RULES_DIGEST: &str = "a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57";
pub const NO_CAPTURE_PLIES: u32 = 100;
pub const REPETITION_OCCURRENCES: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Playing,
    Won,
    Drawn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppliedMove {
    pub mv: Move,
    pub mover: u8,
    pub captured: bool,
    pub status: Status,
    pub winner: Option<u8>,
    pub reason: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveError {
    Illegal,
    GameOver,
}

#[derive(Clone, Debug)]
pub struct GameState {
    pub board: Board,
    /// The side that moved first in this game, fixed for the match.
    pub first_side: u8,
    /// Consecutive plies without a capture.
    pub quiet: u32,
    pub rev: u32,
    pub plies: Vec<Move>,
    /// Position hashes: index 0 is the starting position, then one per ply.
    pub hashes: Vec<u64>,
    pub status: Status,
    pub winner: Option<u8>,
    pub reason: &'static str,
}

impl GameState {
    /// A fresh canonical game with `first_side` to move (Red by default).
    pub fn new(first_side: u8) -> GameState {
        let first = if first_side == RED { RED } else { BLUE };
        let board = Board::initial_with_first(first);
        GameState {
            hashes: vec![board.hash],
            board,
            first_side: first,
            quiet: 0,
            rev: 0,
            plies: Vec::new(),
            status: Status::Playing,
            winner: None,
            reason: "",
        }
    }

    pub fn initial() -> GameState {
        Self::new(RED)
    }

    /// A synthetic position (conformance fixtures, wire snapshots with
    /// incomplete history). The starting position counts once for repetition.
    pub fn from_board(board: Board, first_side: u8, quiet: u32) -> GameState {
        let hashes = vec![board.hash];
        GameState {
            board,
            first_side,
            quiet,
            rev: 0,
            plies: Vec::new(),
            hashes,
            status: Status::Playing,
            winner: None,
            reason: "",
        }
    }

    /// Replays a complete move sequence from the canonical opening.
    pub fn replay(first_side: u8, plies: &[(u8, u8)]) -> Result<GameState, MoveError> {
        let mut state = GameState::new(first_side);
        for &(from, to) in plies {
            if state.status != Status::Playing {
                return Err(MoveError::GameOver);
            }
            state.apply(from, to)?;
        }
        Ok(state)
    }

    pub fn to_play(&self) -> u8 {
        self.board.turn
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let mut buf = [Move {
            from: 0,
            to: 0,
            captured_id: 0,
            enters_den: false,
        }; MAX_MOVES];
        let n = generate_moves(&self.board, self.board.turn, &mut buf);
        let mut moves = buf[..n].to_vec();
        moves.sort_by_key(|m| (m.from, m.to));
        moves
    }

    pub fn legal_move_count(&self) -> usize {
        let mut buf = [Move {
            from: 0,
            to: 0,
            captured_id: 0,
            enters_den: false,
        }; MAX_MOVES];
        generate_moves(&self.board, self.board.turn, &mut buf)
    }

    /// Applies a legal move and adjudicates the resulting position.
    pub fn apply(&mut self, from: u8, to: u8) -> Result<AppliedMove, MoveError> {
        if self.status != Status::Playing {
            return Err(MoveError::GameOver);
        }
        let mover = self.board.turn;
        let mut buf = [Move {
            from: 0,
            to: 0,
            captured_id: 0,
            enters_den: false,
        }; MAX_MOVES];
        let n = generate_moves(&self.board, mover, &mut buf);
        let mv = buf[..n]
            .iter()
            .copied()
            .find(|m| m.from == from && m.to == to)
            .ok_or(MoveError::Illegal)?;

        let captured = mv.captured_id != 0;
        self.board.make_move(&mv);
        self.plies.push(mv);
        self.rev += 1;
        self.hashes.push(self.board.hash);
        self.quiet = if captured { 0 } else { self.quiet + 1 };

        let opponent = self.board.turn;
        let (status, winner, reason) = if mv.enters_den {
            (Status::Won, Some(mover), "den")
        } else if self.board.count[opponent as usize] == 0 {
            (Status::Won, Some(mover), "elimination")
        } else if self.legal_move_count() == 0 {
            (Status::Won, Some(mover), "immobilization")
        } else if self.count_hash(self.board.hash) >= REPETITION_OCCURRENCES {
            (Status::Drawn, None, "repetition")
        } else if self.quiet >= NO_CAPTURE_PLIES {
            (Status::Drawn, None, "no-capture")
        } else {
            (Status::Playing, None, "")
        };

        self.status = status;
        self.winner = winner;
        self.reason = reason;

        Ok(AppliedMove {
            mv,
            mover,
            captured,
            status,
            winner,
            reason,
        })
    }

    /// Applies a move on a clone (used by the strength harness).
    pub fn after_move(&self, from: u8, to: u8) -> Result<GameState, MoveError> {
        let mut next = self.clone();
        next.apply(from, to)?;
        Ok(next)
    }

    pub fn count_hash(&self, hash: u64) -> usize {
        self.hashes.iter().filter(|&&h| h == hash).count()
    }

    pub fn is_terminal(&self) -> bool {
        self.status != Status::Playing
    }

    pub fn result_reason(&self) -> &'static str {
        self.reason
    }

    /// The contents of one square as a piece id (0 = empty).
    pub fn piece_at(&self, sq: u8) -> u8 {
        self.board.squares[sq as usize]
    }

    /// Rebuilds a state from a wire snapshot. The full history is replayed when
    /// it is consistent with the revision, so repetition counts are exact;
    /// otherwise the supplied board becomes the single counted occurrence.
    #[allow(clippy::too_many_arguments)]
    pub fn from_snapshot(
        board: Board,
        first_side: u8,
        _to_play: u8,
        quiet: u32,
        rev: u32,
        history: &[(u8, u8)],
    ) -> GameState {
        if rev as usize == history.len() && (first_side == RED || first_side == BLUE) {
            if let Ok(replayed) = GameState::replay(first_side, history) {
                if replayed.rev == rev
                    && replayed.board.turn == board.turn
                    && replayed.board.squares == board.squares
                {
                    return replayed;
                }
            }
        }
        GameState::from_board(board, first_side, quiet)
    }

    /// True when `sq` is a legal destination for a piece on `from`.
    pub fn is_legal(&self, from: u8, to: u8) -> bool {
        self.legal_moves()
            .iter()
            .any(|m| m.from == from && m.to == to)
    }

    /// Flat board as piece ids (0 = empty), convenient for the wire/renderer.
    pub fn board_cells(&self) -> [u8; SQUARES] {
        self.board.squares
    }
}

pub fn side_name(side: u8) -> &'static str {
    if side == RED {
        "red"
    } else {
        "blue"
    }
}

pub fn other_side(side: u8) -> u8 {
    if side == RED {
        BLUE
    } else {
        RED
    }
}

/// True when the board's winner field was set by den/elimination.
pub fn board_winner(board: &Board) -> Option<u8> {
    if board.winner == NO_WINNER {
        None
    } else {
        Some(board.winner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::RED;

    #[test]
    fn opening_has_24_moves_both_sides() {
        let red = GameState::new(RED);
        assert_eq!(red.legal_move_count(), 24);
        let moves = red.legal_moves();
        assert_eq!((moves[0].from, moves[0].to), (42, 35));
        assert_eq!((moves[23].from, moves[23].to), (62, 61));
        let blue = GameState::new(BLUE);
        assert_eq!(blue.legal_move_count(), 24);
        let moves = blue.legal_moves();
        assert_eq!((moves[0].from, moves[0].to), (0, 1));
    }

    #[test]
    fn opening_handshake_revision_one() {
        let mut game = GameState::new(RED);
        game.apply(42, 35).unwrap();
        assert_eq!(game.rev, 1);
        assert_eq!(game.quiet, 1);
        assert_eq!(game.board.squares[42], 0);
        assert_eq!(game.board.squares[35], (8 - 1) * 2 + RED + 1); // Red elephant
        assert_eq!(game.board.turn, BLUE);
        assert_eq!((game.plies[0].from, game.plies[0].to), (42, 35));
    }
}
