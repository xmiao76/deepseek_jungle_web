//! Conformance fixtures shared with the JS referee. These are the authoritative
//! acceptance cases from `jungle_prompt.md`.

use crate::board::{Board, BLUE, RED};
use crate::game::{GameState, Status};
use serde_json::Value;

const PORTABLE: &str = include_str!("../../../../tests/fixtures/portable-cases.json");
const ADJUDICATION: &str = include_str!("../../../../tests/fixtures/adjudication.json");
const OPENING: &str = include_str!("../../../../tests/fixtures/opening-handshake.json");

fn side_of(name: &str) -> u8 {
    match name {
        "red" => RED,
        "blue" => BLUE,
        other => panic!("unknown side {other}"),
    }
}

fn rank_of_name(name: &str) -> u8 {
    match name {
        "rat" => 1,
        "cat" => 2,
        "dog" => 3,
        "wolf" => 4,
        "leopard" => 5,
        "tiger" => 6,
        "lion" => 7,
        "elephant" => 8,
        other => panic!("unknown animal {other}"),
    }
}

fn piece_desc(desc: &str) -> (u8, u8) {
    let (side, animal) = desc.split_once(':').expect("side:animal");
    (side_of(side), rank_of_name(animal))
}

/// Builds a synthetic board from a `{"square": "side:animal"}` map.
fn board_from(value: &Value, to_play: u8) -> Board {
    let obj = value.as_object().expect("board object");
    let mut types = Vec::new();
    let mut squares = Vec::new();
    for (sq, desc) in obj {
        let sq: u8 = sq.parse().expect("square");
        let (owner, animal) = piece_desc(desc.as_str().expect("desc"));
        types.push((animal - 1) * 2 + owner);
        squares.push(sq);
    }
    Board::from_packed(&types, &squares, to_play)
}

fn move_pair(value: &Value) -> (u8, u8) {
    let arr = value.as_array().expect("move pair");
    (
        arr[0].as_u64().unwrap() as u8,
        arr[1].as_u64().unwrap() as u8,
    )
}

#[test]
fn rules_identity_matches_brief() {
    assert_eq!(crate::game::RULES_ID, "desktop-jungle-v2");
    assert_eq!(
        crate::game::RULES_DIGEST,
        "a457bbc009e975a9eafe5ff1f8fa40ca2548f2a689b121c8c4ed99dd0f3fdc57"
    );
}

#[test]
fn portable_cases() {
    let root: Value = serde_json::from_str(PORTABLE).expect("json");
    let cases = root["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 23);
    for case in cases {
        let name = case[0].as_str().unwrap();
        let board = board_from(&case[1], RED);
        let (from, to) = move_pair(&case[2]);
        let expected = case[3].as_bool().unwrap();
        let game = GameState::from_board(board, RED, 0);
        let legal = game.legal_moves();
        let actual = legal.iter().any(|m| m.from == from && m.to == to);
        assert_eq!(actual, expected, "portable case {name}: {from}->{to}");
    }
}

#[test]
fn adjudication_cases() {
    let root: Value = serde_json::from_str(ADJUDICATION).expect("json");
    let rows = root["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 9);
    for row in rows {
        let name = row["name"].as_str().unwrap();
        let board = board_from(&row["position"], RED);
        let quiet = row["quietBefore"].as_u64().unwrap() as u32;
        let mut game = GameState::from_board(board, RED, quiet);
        let moves = row["moves"].as_array().unwrap();
        for mv in moves {
            let (from, to) = move_pair(mv);
            game.apply(from, to)
                .unwrap_or_else(|e| panic!("{name}: illegal {from}->{to}: {e:?}"));
        }

        let expected_status = match row["status"].as_str().unwrap() {
            "playing" => Status::Playing,
            "won" => Status::Won,
            "drawn" => Status::Drawn,
            other => panic!("{name}: unknown status {other}"),
        };
        assert_eq!(game.status, expected_status, "{name}: status");
        assert_eq!(
            game.quiet,
            row["quietAfter"].as_u64().unwrap() as u32,
            "{name}: quiet"
        );
        assert_eq!(
            game.reason,
            row["reason"].as_str().unwrap(),
            "{name}: reason"
        );
        let expected_winner = row["winner"].as_str().map(side_of);
        assert_eq!(game.winner, expected_winner, "{name}: winner");
        assert_eq!(game.rev as usize, moves.len(), "{name}: revision");
        assert_eq!(game.plies.len(), moves.len(), "{name}: history length");
        let expected_to_play = if moves.len() % 2 == 0 { RED } else { BLUE };
        assert_eq!(game.to_play(), expected_to_play, "{name}: to-play");
        if let Some(rev) = row.get("rev") {
            assert_eq!(
                game.rev,
                rev.as_u64().unwrap() as u32,
                "{name}: rev fixture"
            );
        }
        if let Some(to_play) = row.get("toPlay") {
            assert_eq!(
                game.to_play(),
                side_of(to_play.as_str().unwrap()),
                "{name}: toPlay fixture"
            );
        }
    }
}

#[test]
fn opening_handshake() {
    let root: Value = serde_json::from_str(OPENING).expect("json");
    for side_name in ["red", "blue"] {
        let spec = &root[side_name];
        let first = side_of(spec["firstSide"].as_str().unwrap());
        let game = GameState::new(first);
        assert_eq!(
            game.legal_move_count(),
            spec["legalCount"].as_u64().unwrap() as usize,
            "{side_name}: legal count"
        );
        let moves = game.legal_moves();
        assert_eq!(move_pair(&spec["firstMove"]), (moves[0].from, moves[0].to));
        if let Some(last) = spec.get("lastMove") {
            let last = move_pair(last);
            assert_eq!(last, (moves.last().unwrap().from, moves.last().unwrap().to));
        }
    }

    let after = &root["red"]["afterFirst"];
    let mut game = GameState::new(RED);
    game.apply(42, 35).unwrap();
    assert_eq!(game.rev, after["rev"].as_u64().unwrap() as u32);
    assert_eq!(
        game.board.squares[after["emptySquare"].as_u64().unwrap() as usize],
        0
    );
    let (owner, animal) = piece_desc(after["piece"].as_str().unwrap());
    let expected_id = (animal - 1) * 2 + owner + 1;
    assert_eq!(
        game.board.squares[after["square"].as_u64().unwrap() as usize],
        expected_id
    );
    assert_eq!(game.quiet, after["quiet"].as_u64().unwrap() as u32);
    assert_eq!(game.plies.len(), after["history"].as_array().unwrap().len());
    assert_eq!(game.to_play(), side_of(after["toPlay"].as_str().unwrap()));
}
