//! Jungle v2 CLI: rules inspection, perft, node-budget benchmarks and (later)
//! rendering, arena and strength measurements.

use jungle_core::{perft, Board, GameState, Limits, Search};
use jungle_render::{Phase, Renderer, View};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("rules") => {
            println!("rules-id: {}", jungle_core::RULES_ID);
            println!("rules-digest: {}", jungle_core::RULES_DIGEST);
            let game = GameState::initial();
            println!("first-side: red");
            println!("opening-legal: {}", game.legal_move_count());
            for (i, m) in game.legal_moves().iter().enumerate() {
                if i == 0 || i == 23 {
                    println!("  index {i}: {} -> {}", m.from, m.to);
                }
            }
        }
        Some("perft") => {
            let depth: u8 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(4);
            let board = Board::initial();
            println!("perft({depth}) = {}", perft(&board, board.turn, depth));
        }
        Some("openings") => {
            for first in [jungle_core::RED, jungle_core::BLUE] {
                let game = GameState::new(first);
                println!(
                    "{} first: {} moves, first={}->{}, last={}->{}",
                    if first == jungle_core::RED {
                        "red"
                    } else {
                        "blue"
                    },
                    game.legal_move_count(),
                    game.legal_moves()[0].from,
                    game.legal_moves()[0].to,
                    game.legal_moves().last().unwrap().from,
                    game.legal_moves().last().unwrap().to
                );
            }
        }
        Some("bench") => {
            let budget: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20_000);
            let game = GameState::initial();
            let mut board = game.board;
            let mut search = Search::new(1 << 20);
            let out = search.search(
                &mut board,
                &game.hashes,
                game.quiet,
                Limits::nodes(budget),
                32,
                true,
            );
            let nps = if out.elapsed_ms > 0.0 {
                out.nodes as f64 / (out.elapsed_ms / 1000.0)
            } else {
                0.0
            };
            println!(
                "bench nodes={} budget={} depth={} aborted={} best={:?} score={} elapsed={:.1}ms nps={:.0}",
                out.nodes, budget, out.depth, out.aborted,
                out.best.map(|m| (m.from, m.to)), out.score, out.elapsed_ms, nps
            );
        }
        Some("bench-time") => {
            let ms: f64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1000.0);
            let game = GameState::initial();
            let mut board = game.board;
            let mut search = Search::with_clock(1 << 20, native_now_ms);
            let out = search.search(
                &mut board,
                &game.hashes,
                game.quiet,
                Limits::time(ms),
                32,
                false,
            );
            let nps = out.nodes as f64 / (out.elapsed_ms / 1000.0).max(0.0001);
            println!(
                "bench-time nodes={} depth={} aborted={} best={:?} score={} elapsed={:.1}ms nps={:.0}",
                out.nodes, out.depth, out.aborted,
                out.best.map(|m| (m.from, m.to)), out.score, out.elapsed_ms, nps
            );
        }
        Some("opening-json") => {
            let game = GameState::initial();
            let board: Vec<serde_json::Value> = (0..63u8)
                .map(|sq| {
                    let id = game.board.squares[sq as usize];
                    if id == 0 {
                        serde_json::Value::Null
                    } else {
                        let owner = if id.is_multiple_of(2) { "red" } else { "blue" };
                        let rank = ((id - 1) % 16) / 2 + 1;
                        let kind = match rank {
                            1 => "rat",
                            2 => "cat",
                            3 => "dog",
                            4 => "wolf",
                            5 => "leopard",
                            6 => "tiger",
                            7 => "lion",
                            _ => "elephant",
                        };
                        serde_json::json!({"owner": owner, "kind": kind})
                    }
                })
                .collect();
            let legal: Vec<serde_json::Value> = game
                .legal_moves()
                .iter()
                .map(|m| serde_json::json!({"src": m.from, "dst": m.to}))
                .collect();
            let snapshot = serde_json::json!({
                "rev": 0,
                "board": board,
                "firstSide": "red",
                "toPlay": "red",
                "history": [],
                "noCapturePlies": 0,
                "status": "playing",
                "winner": null,
                "reason": "",
                "legal": legal,
            });
            println!("{}", serde_json::to_string(&snapshot).unwrap());
        }
        Some("eval") => {
            let game = GameState::initial();
            println!(
                "static red={} blue={}",
                jungle_core::evaluate_static(&game.board, jungle_core::RED, false, false),
                jungle_core::evaluate_static(&game.board, jungle_core::BLUE, false, false)
            );
            let mut board = game.board;
            for m in game.legal_moves() {
                let undo = board.make_move(&m);
                let score = -jungle_core::evaluate_static(&board, board.turn, false, false);
                board.unmake_move(&m, &undo);
                println!("{}->{} static(for mover)={}", m.from, m.to, score);
            }
        }
        Some("depth-search") => {
            let depth: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
            let game = GameState::initial();
            for d in 1..=depth {
                let mut board = game.board;
                let mut search = Search::new(1 << 20);
                if std::env::var("NO_DRAWS").is_ok() {
                    search.set_draw_awareness(false);
                }
                let out = search.search(
                    &mut board,
                    &game.hashes,
                    game.quiet,
                    Limits::default(),
                    d,
                    true,
                );
                println!(
                    "depth={} best={:?} score={} nodes={} aborted={}",
                    d,
                    out.best.map(|m| (m.from, m.to)),
                    out.score,
                    out.nodes,
                    out.aborted
                );
            }
        }
        Some("baseline-depth") => {
            let depth: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
            let game = GameState::initial();
            let mut board = jungle_baseline::Board::from_grid(&game.board.squares, game.board.turn);
            let mut search = jungle_baseline::Search::new(false, false, false, false);
            let best = search.search_best_move(&mut board, depth, &[], None);
            println!(
                "baseline depth={} best={:?} nodes={} completed={}",
                depth, best, search.nodes, search.last_completed_depth
            );
        }
        Some("eval-diff") => {
            let mut checked = 0usize;
            let mut mismatches = 0usize;
            let mut rng = 0x1234_5678u64;
            for game in 0..80 {
                let mut game_state = GameState::new(if game % 2 == 0 {
                    jungle_core::RED
                } else {
                    jungle_core::BLUE
                });
                for ply in 0..40 {
                    let legal = game_state.legal_moves();
                    if game_state.is_terminal() || legal.is_empty() {
                        break;
                    }
                    let base = jungle_baseline::Board::from_grid(
                        &game_state.board.squares,
                        game_state.board.turn,
                    );
                    for side in 0..2u8 {
                        let a = jungle_core::evaluate_static(&game_state.board, side, false, false);
                        let b = jungle_baseline::evaluate_static(&base, side, false, false);
                        checked += 1;
                        if a != b {
                            mismatches += 1;
                            if mismatches <= 5 {
                                eprintln!("EVAL MISMATCH game {game} ply {ply} side {side}: core {a} baseline {b}");
                            }
                        }
                    }
                    rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let move_index = (rng >> 33) as usize % legal.len();
                    game_state
                        .apply(legal[move_index].from, legal[move_index].to)
                        .unwrap();
                }
            }
            println!("eval-diff: {checked} checked, {mismatches} mismatches");
        }
        Some("baseline-arena") => {
            let games_per_opening: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
            let depth: i32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(7);
            let openings: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(24);
            let mut rng: u64 = 0x9e37_79b9_7f4a_7c15;
            let mut new_wins = 0u32;
            let mut base_wins = 0u32;
            let mut draws = 0u32;
            let mut game_no = 0u32;
            for opening_idx in 0..openings {
                // Seeded random opening, 4 plies.
                let mut opening_state = GameState::new(jungle_core::RED);
                for _ in 0..4 {
                    let legal = opening_state.legal_moves();
                    if opening_state.is_terminal() || legal.is_empty() {
                        break;
                    }
                    rng = rng
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    let pick = legal[(rng >> 33) as usize % legal.len()];
                    opening_state.apply(pick.from, pick.to).unwrap();
                }
                for variant in 0..games_per_opening {
                    if opening_state.is_terminal() {
                        continue;
                    }
                    let new_side = if variant % 2 == 0 {
                        jungle_core::RED
                    } else {
                        jungle_core::BLUE
                    };
                    let mut game = opening_state.clone();
                    let mut new_search = Search::new(1 << 20);
                    let persistent = std::env::var("FRESH").map(|v| v == "0").unwrap_or(false);
                    if std::env::var("NO_DRAWS").is_ok() {
                        new_search.set_draw_awareness(false);
                    }
                    let mut base_search = jungle_baseline::Search::new(false, false, false, false);
                    let mut plies = 0usize;
                    while !game.is_terminal() && plies < 300 {
                        let legal = game.legal_moves();
                        if legal.is_empty() {
                            break;
                        }
                        let chosen = if game.to_play() == new_side {
                            let out = new_search.search(
                                &mut game.board,
                                &game.hashes,
                                game.quiet,
                                Limits::default(),
                                depth,
                                !persistent,
                            );
                            out.best
                                .and_then(|m| {
                                    legal
                                        .iter()
                                        .copied()
                                        .find(|x| x.from == m.from && x.to == m.to)
                                })
                                .unwrap_or(legal[0])
                        } else {
                            let mut base_board = jungle_baseline::Board::from_grid(
                                &game.board.squares,
                                game.board.turn,
                            );
                            let best = base_search.search_best_move(
                                &mut base_board,
                                depth,
                                &game.hashes,
                                None,
                            );
                            best.and_then(|(f, t)| {
                                legal.iter().copied().find(|x| x.from == f && x.to == t)
                            })
                            .unwrap_or(legal[0])
                        };
                        game.apply(chosen.from, chosen.to).unwrap();
                        plies += 1;
                    }
                    let winner = if game.status == jungle_core::Status::Won {
                        game.winner
                    } else {
                        None
                    };
                    let outcome = if let Some(w) = winner {
                        if w == new_side {
                            new_wins += 1;
                            "new"
                        } else {
                            base_wins += 1;
                            "baseline"
                        }
                    } else {
                        draws += 1;
                        "draw"
                    };
                    game_no += 1;
                    println!(
                        "game {game_no} opening {opening_idx} new={} outcome={} reason={} plies={}",
                        if new_side == jungle_core::RED {
                            "red"
                        } else {
                            "blue"
                        },
                        outcome,
                        game.reason,
                        plies
                    );
                }
            }
            println!("summary games={game_no} new_wins={new_wins} baseline_wins={base_wins} draws={draws}");
        }
        Some("fixture-match") => {
            let budget: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(20_000);
            let max_plies: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(200);
            let mut game = GameState::new(jungle_core::RED);
            let mut search = Search::new(1 << 20);
            let mut moves = Vec::new();
            for ply in 0..max_plies {
                if game.is_terminal() {
                    break;
                }
                let legal = game.legal_moves();
                let chosen = if game.to_play() == jungle_core::RED {
                    let out = search.search(
                        &mut game.board,
                        &game.hashes,
                        game.quiet,
                        Limits::nodes(budget),
                        32,
                        true,
                    );
                    out.best
                        .and_then(|m| {
                            legal
                                .iter()
                                .copied()
                                .find(|x| x.from == m.from && x.to == m.to)
                        })
                        .unwrap_or(legal[0])
                } else {
                    legal[0]
                };
                game.apply(chosen.from, chosen.to).expect("legal");
                moves.push(format!("{}>{}", chosen.from, chosen.to));
                if (ply + 1) % 2 == 0 {
                    println!("{} {}", ply.div_ceil(2), moves[moves.len() - 2..].join(" "));
                }
            }
            println!(
                "result={:?} winner={:?} reason={} plies={}",
                game.status,
                game.winner,
                game.reason,
                game.plies.len()
            );
            println!("moves={}", moves.join(","));
        }
        Some("render") => {
            let path = args
                .get(2)
                .cloned()
                .unwrap_or_else(|| "board.png".to_string());
            let scale: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2);
            let flipped = args.get(4).map(|s| s == "flip").unwrap_or(false);
            let game = GameState::initial();
            let cells = game.board_cells();
            let mut renderer = Renderer::new();
            let frame = renderer
                .render(
                    &View {
                        cells: &cells,
                        to_play: game.to_play(),
                        first_side: game.first_side,
                        phase: Phase::Playing,
                        winner: None,
                        reason: "",
                        plies: &[],
                        selected: None,
                        hover: None,
                        cursor: None,
                        legal: &[],
                        captured: None,
                        show_result: false,
                    },
                    flipped,
                    1000,
                    scale,
                )
                .to_vec();
            write_png(
                &path,
                jungle_render::frame_size(scale).0,
                jungle_render::frame_size(scale).1,
                &frame,
            );
            println!("wrote {path}");
        }
        _ => {
            eprintln!("usage: jungle-cli <rules|perft|openings|bench|bench-time|render>");
            std::process::exit(1);
        }
    }
}

fn write_png(path: &str, width: u32, height: u32, rgba: &[u8]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(rgba).expect("png data");
}

fn native_now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}
