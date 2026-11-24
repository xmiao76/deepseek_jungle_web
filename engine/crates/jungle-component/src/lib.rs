//! Arena competitor component: v2 rules/search plus the shared software
//! renderer. Only `display` is imported; no clock, files, threads or WASI.

wit_bindgen::generate!({
    path: "../../../wit",
    world: "competitor",
});

use exports::jungle::arena::game::{
    Animal, Guest, Level, Metadata, Phase, Piece, PlyMove, Settings, Side, Snapshot,
};
use jungle::arena::display;
use jungle_core::game::GameState;
use jungle_core::{Board, Limits, Search, BLUE, RED};
use jungle_identity as identity;
use jungle_render::{FramePhase, Renderer, View};
use std::cell::RefCell;

const TT_ENTRIES: usize = 1 << 17;
const MAX_DEPTH: i32 = 32;

struct Instance {
    difficulty: Level,
    search: Search,
    renderer: Renderer,
}

thread_local! {
    static INSTANCE: RefCell<Option<Instance>> = const { RefCell::new(None) };
}

fn with_instance<R>(f: impl FnOnce(&mut Instance) -> R) -> R {
    INSTANCE.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = Some(Instance {
                difficulty: Level::Medium,
                search: Search::new(TT_ENTRIES),
                renderer: Renderer::new(),
            });
        }
        f(slot.as_mut().expect("instance"))
    })
}

struct Component;

fn side_bit(side: Side) -> u8 {
    match side {
        Side::Red => RED,
        Side::Blue => BLUE,
    }
}

fn animal_rank(animal: Animal) -> u8 {
    match animal {
        Animal::Rat => 1,
        Animal::Cat => 2,
        Animal::Dog => 3,
        Animal::Wolf => 4,
        Animal::Leopard => 5,
        Animal::Tiger => 6,
        Animal::Lion => 7,
        Animal::Elephant => 8,
    }
}

fn board_from_snapshot(state: &Snapshot) -> Board {
    let mut cells = [0u8; 63];
    for (sq, piece) in state.board.iter().enumerate().take(63) {
        if let Some(piece) = piece {
            cells[sq] = (animal_rank(piece.kind) - 1) * 2 + side_bit(piece.owner) + 1;
        }
    }
    Board::from_grid(&cells, side_bit(state.to_play))
}

fn game_from_snapshot(state: &Snapshot) -> GameState {
    let board = board_from_snapshot(state);
    let history: Vec<(u8, u8)> = state.history.iter().map(|m| (m.src, m.dst)).collect();
    GameState::from_snapshot(
        board,
        side_bit(state.first_side),
        side_bit(state.to_play),
        state.no_capture_plies,
        state.rev,
        &history,
    )
}

fn budget_for(budget: u32, level: Level) -> u64 {
    let b = u64::from(budget.max(1));
    match level {
        Level::Easy => (b / 4).max(1),
        Level::Medium => (b / 2).max(1),
        Level::Hard => b,
    }
}

impl Guest for Component {
    fn info() -> Metadata {
        Metadata {
            title: identity::title(),
            game_version: identity::GAME_VERSION.to_string(),
            model: identity::MODEL.to_string(),
            code_agent: identity::CODE_AGENT.to_string(),
            model_platform: identity::MODEL_PLATFORM.to_string(),
            github_url: identity::GITHUB_URL.to_string(),
            engine_version: identity::ENGINE_VERSION.to_string(),
            runtime: identity::RUNTIME.to_string(),
            rules_id: identity::RULES_ID.to_string(),
            rules_digest: identity::RULES_DIGEST.to_string(),
        }
    }

    fn init(config: Settings) {
        with_instance(|instance| {
            instance.difficulty = config.difficulty;
        });
    }

    fn choose_move(state: Snapshot, legal: Vec<PlyMove>, node_budget: u32) -> u32 {
        if state.status != Phase::Playing || legal.is_empty() {
            return 0;
        }
        let mut game = game_from_snapshot(&state);
        if game.status != jungle_core::Status::Playing {
            return 0;
        }
        let budget = with_instance(|instance| budget_for(node_budget, instance.difficulty));
        let outcome = with_instance(|instance| {
            instance.search.search(
                &mut game.board,
                &game.hashes,
                game.quiet,
                Limits::nodes(budget),
                MAX_DEPTH,
                true,
            )
        });
        let Some(best) = outcome.best else {
            return 0;
        };
        legal
            .iter()
            .position(|m| m.src == best.from && m.dst == best.to)
            .map(|i| i as u32)
            .unwrap_or(0)
    }

    fn render(state: Snapshot, flipped: bool, progress: u16) {
        let mut cells = [0u8; 63];
        for (sq, piece) in state.board.iter().enumerate().take(63) {
            if let Some(piece) = piece {
                cells[sq] = (animal_rank(piece.kind) - 1) * 2 + side_bit(piece.owner) + 1;
            }
        }
        let plies: Vec<(u8, u8)> = state.history.iter().map(|m| (m.src, m.dst)).collect();
        let phase = match state.status {
            Phase::Playing => FramePhase::Playing,
            Phase::Won => FramePhase::Won,
            Phase::Drawn => FramePhase::Drawn,
            Phase::Forfeited => FramePhase::Forfeited,
        };
        let view = View {
            cells: &cells,
            to_play: side_bit(state.to_play),
            first_side: side_bit(state.first_side),
            phase,
            winner: state.winner.map(side_bit),
            reason: &state.reason,
            plies: &plies,
            selected: None,
            hover: None,
            cursor: None,
            legal: &[],
            captured: None,
            show_result: true,
        };
        with_instance(|instance| {
            let frame = instance.renderer.render(&view, flipped, progress, 1);
            display::present(448, 576, frame);
        });
    }
}

export!(Component);

// Keep the generated WIT types referenced even when a helper is optimized out.
#[allow(dead_code)]
fn _types(_p: Piece, _l: Level) {}
