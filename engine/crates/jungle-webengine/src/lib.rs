//! Standalone browser engine: raw C ABI over the shared rules, search and
//! renderer. The shell (Web Worker or main thread) copies snapshots/frames out
//! of linear memory; the only import is `env.now_ms`.
//!
//! The exported functions are called by JavaScript with pointers into this
//! module's linear memory; the ABI is inherently unsafe at the boundary.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use jungle_core::game::GameState;
use jungle_core::{Board, Limits, Search, Status, BLUE, RED};
use jungle_render::{Phase, Renderer, View};
use std::alloc::{alloc, dealloc, Layout};
use std::slice;

const TT_ENTRIES: usize = 1 << 20;
const SNAPSHOT_HEADER: usize = 16;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn now_ms() -> f64;
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> f64 {
    0.0
}

struct AiStats {
    from: u8,
    to: u8,
    depth: i32,
    nodes: u64,
    score: i32,
    aborted: bool,
    elapsed_ms: f64,
}

struct Engine {
    state: GameState,
    search: Search,
    renderer: Renderer,
    seed: u32,
    last_capture: u8,
    ai: AiStats,
    snapshot: Vec<u8>,
}

impl Engine {
    fn new(first_wire: u32, seed: u32) -> Engine {
        let first = from_wire_side(first_wire);
        Engine {
            state: GameState::new(first),
            search: Search::with_clock(TT_ENTRIES, wasm_now_ms),
            renderer: Renderer::new(),
            seed,
            last_capture: 0,
            ai: AiStats {
                from: 0,
                to: 0,
                depth: 0,
                nodes: 0,
                score: 0,
                aborted: true,
                elapsed_ms: 0.0,
            },
            snapshot: Vec::with_capacity(512),
        }
    }

    fn write_snapshot(&mut self) -> usize {
        let plies = &self.state.plies;
        let legal = self.state.legal_moves();
        let total = SNAPSHOT_HEADER + 63 + plies.len() * 2 + legal.len() * 2;
        self.snapshot.resize(total, 0);
        let buf = &mut self.snapshot;
        buf[0..4].copy_from_slice(&self.state.rev.to_le_bytes());
        buf[4..6].copy_from_slice(&(self.state.quiet.min(u16::MAX as u32) as u16).to_le_bytes());
        buf[6] = to_wire_side(self.state.to_play());
        buf[7] = to_wire_side(self.state.first_side);
        buf[8] = match self.state.status {
            Status::Playing => 0,
            Status::Won => 1,
            Status::Drawn => 2,
        };
        buf[9] = match self.state.winner {
            Some(side) => to_wire_side(side),
            None => 255,
        };
        buf[10] = reason_code(self.state.reason);
        buf[11] = self.last_capture;
        buf[12..14].copy_from_slice(&(plies.len() as u16).to_le_bytes());
        buf[14..16].copy_from_slice(&(legal.len() as u16).to_le_bytes());
        buf[16..79].copy_from_slice(&self.state.board.squares);
        let mut off = 79;
        for m in plies {
            buf[off..off + 2]
                .copy_from_slice(&(((m.from as u16) << 8) | m.to as u16).to_le_bytes());
            off += 2;
        }
        for m in &legal {
            buf[off..off + 2]
                .copy_from_slice(&(((m.from as u16) << 8) | m.to as u16).to_le_bytes());
            off += 2;
        }
        total
    }
}

fn to_wire_side(side: u8) -> u8 {
    if side == RED {
        0
    } else {
        1
    }
}

fn from_wire_side(side: u32) -> u8 {
    if side == 0 {
        RED
    } else {
        BLUE
    }
}

fn reason_code(reason: &str) -> u8 {
    match reason {
        "den" => 1,
        "elimination" => 2,
        "immobilization" => 3,
        "repetition" => 4,
        "no-capture" => 5,
        _ => 0,
    }
}

// `static mut` is safe here: a wasm instance is single-threaded and the shell
// serialises calls in its worker.
static mut ENGINE: Option<Engine> = None;

fn engine() -> &'static mut Engine {
    unsafe {
        let ptr = &raw mut ENGINE;
        let slot = (*ptr).get_or_insert_with(|| Engine::new(0, 0));
        slot
    }
}

fn wasm_now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    unsafe {
        now_ms()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        now_ms()
    }
}

#[no_mangle]
pub extern "C" fn jg_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    match Layout::from_size_align(len, 8) {
        Ok(layout) => unsafe { alloc(layout) },
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn jg_dealloc(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    if let Ok(layout) = Layout::from_size_align(len, 8) {
        unsafe { dealloc(ptr, layout) };
    }
}

#[no_mangle]
pub extern "C" fn jg_new(first_side: u32, seed: u32) -> u32 {
    let e = engine();
    e.state = GameState::new(from_wire_side(first_side));
    e.seed = seed;
    e.last_capture = 0;
    0
}

#[no_mangle]
pub extern "C" fn jg_load(ptr: *const u16, count: u32, first_side: u32) -> u32 {
    if count > 0 && ptr.is_null() {
        return 2;
    }
    let moves: Vec<(u8, u8)> = if count == 0 {
        Vec::new()
    } else {
        unsafe { slice::from_raw_parts(ptr, count as usize) }
            .iter()
            .map(|m| ((m >> 8) as u8, (m & 0xff) as u8))
            .collect()
    };
    match GameState::replay(from_wire_side(first_side), &moves) {
        Ok(state) => {
            let e = engine();
            e.last_capture = state.plies.last().map(|m| m.captured_id).unwrap_or(0);
            e.state = state;
            0
        }
        Err(_) => 1,
    }
}

#[no_mangle]
pub extern "C" fn jg_apply(from: u32, to: u32) -> u32 {
    let e = engine();
    match e.state.apply(from as u8, to as u8) {
        Ok(applied) => {
            e.last_capture = applied.mv.captured_id;
            0
        }
        Err(jungle_core::MoveError::Illegal) => 1,
        Err(jungle_core::MoveError::GameOver) => 2,
    }
}

#[no_mangle]
pub extern "C" fn jg_legal(ptr: *mut u16, cap: usize) -> usize {
    let legal = engine().state.legal_moves();
    if !ptr.is_null() && cap >= legal.len() {
        let out = unsafe { slice::from_raw_parts_mut(ptr, legal.len()) };
        for (i, m) in legal.iter().enumerate() {
            out[i] = ((m.from as u16) << 8) | m.to as u16;
        }
    }
    legal.len()
}

#[no_mangle]
pub extern "C" fn jg_snapshot_len() -> usize {
    let e = engine();
    SNAPSHOT_HEADER + 63 + e.state.plies.len() * 2 + e.state.legal_move_count() * 2
}

#[no_mangle]
pub extern "C" fn jg_snapshot(ptr: *mut u8, cap: usize) -> usize {
    let e = engine();
    let len = e.write_snapshot();
    if !ptr.is_null() && cap >= len {
        unsafe {
            std::ptr::copy_nonoverlapping(e.snapshot.as_ptr(), ptr, len);
        }
    }
    len
}

#[no_mangle]
pub extern "C" fn jg_render(
    scale: u32,
    flipped: u32,
    progress: u32,
    selected: i32,
    hover: i32,
    cursor: i32,
    ptr: *mut u8,
    cap: usize,
) -> usize {
    let e = engine();
    let legal_all = e.state.legal_moves();
    let selected = if (0..63).contains(&selected) {
        Some(selected as u8)
    } else {
        None
    };
    let legal: Vec<(u8, u8)> = if let Some(sel) = selected {
        legal_all
            .iter()
            .filter(|m| m.from == sel)
            .map(|m| (m.from, m.to))
            .collect()
    } else {
        Vec::new()
    };
    let phase = match e.state.status {
        Status::Playing => Phase::Playing,
        Status::Won => Phase::Won,
        Status::Drawn => Phase::Drawn,
    };
    let cells = e.state.board_cells();
    let plies: Vec<(u8, u8)> = e.state.plies.iter().map(|m| (m.from, m.to)).collect();
    let view = View {
        cells: &cells,
        to_play: e.state.to_play(),
        first_side: e.state.first_side,
        phase,
        winner: e.state.winner,
        reason: e.state.reason,
        plies: &plies,
        selected,
        hover: if (0..63).contains(&hover) {
            Some(hover as u8)
        } else {
            None
        },
        cursor: if (0..63).contains(&cursor) {
            Some(cursor as u8)
        } else {
            None
        },
        legal: &legal,
        captured: if e.last_capture != 0 {
            Some(e.last_capture)
        } else {
            None
        },
        show_result: false,
    };
    let frame = e
        .renderer
        .render(&view, flipped != 0, progress.min(1000) as u16, scale);
    let len = frame.len();
    if !ptr.is_null() && cap >= len {
        unsafe {
            std::ptr::copy_nonoverlapping(frame.as_ptr(), ptr, len);
        }
    }
    len
}

#[no_mangle]
pub extern "C" fn jg_frame_len(scale: u32) -> usize {
    let (w, h) = jungle_render::frame_size(scale);
    (w * h * 4) as usize
}

#[no_mangle]
pub extern "C" fn jg_search(node_budget: u64, time_budget_ms: f64, max_depth: u32) -> u32 {
    let e = engine();
    let budget = if node_budget == 0 {
        None
    } else {
        Some(node_budget)
    };
    let time = if time_budget_ms > 0.0 {
        Some(time_budget_ms)
    } else {
        None
    };
    let limits = match (budget, time) {
        (Some(n), Some(t)) => Limits::nodes_or_time(n, t),
        (Some(n), None) => Limits::nodes(n),
        (None, Some(t)) => Limits::time(t),
        (None, None) => Limits::time(1.0),
    };
    let mut board: Board = e.state.board;
    let outcome = e.search.search(
        &mut board,
        &e.state.hashes,
        e.state.quiet,
        limits,
        max_depth.max(1) as i32,
        false,
    );
    let (from, to) = outcome.best.map(|m| (m.from, m.to)).unwrap_or((0, 0));
    e.ai = AiStats {
        from,
        to,
        depth: outcome.depth,
        nodes: outcome.nodes,
        score: outcome.score,
        aborted: outcome.aborted,
        elapsed_ms: outcome.elapsed_ms,
    };
    ((from as u32) << 8) | to as u32
}

#[no_mangle]
pub extern "C" fn jg_ai_nodes() -> u64 {
    engine().ai.nodes
}

#[no_mangle]
pub extern "C" fn jg_ai_depth() -> i32 {
    engine().ai.depth
}

#[no_mangle]
pub extern "C" fn jg_ai_score() -> i32 {
    engine().ai.score
}

#[no_mangle]
pub extern "C" fn jg_ai_ms() -> f64 {
    engine().ai.elapsed_ms
}

#[no_mangle]
pub extern "C" fn jg_ai_aborted() -> u32 {
    if engine().ai.aborted {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn jg_ai_from() -> u32 {
    engine().ai.from as u32
}

#[no_mangle]
pub extern "C" fn jg_ai_to() -> u32 {
    engine().ai.to as u32
}

#[no_mangle]
pub extern "C" fn jg_rules_id_ptr() -> *const u8 {
    jungle_core::RULES_ID.as_ptr()
}

#[no_mangle]
pub extern "C" fn jg_rules_id_len() -> usize {
    jungle_core::RULES_ID.len()
}

#[no_mangle]
pub extern "C" fn jg_rules_digest_ptr() -> *const u8 {
    jungle_core::RULES_DIGEST.as_ptr()
}

#[no_mangle]
pub extern "C" fn jg_rules_digest_len() -> usize {
    jungle_core::RULES_DIGEST.len()
}
