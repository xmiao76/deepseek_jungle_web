//! WASM exports for the browser backend. The search reads its inputs from
//! exported scratch buffers (`hex_buf`, `history_buf`) so the JS side never has
//! to manage wasm memory directly. The clock is imported from JS (bare
//! wasm32-unknown-unknown has no clock).

use jungle_lib::tablebase::Tablebase;
use jungle_lib::{Board, Search};
use std::alloc::{alloc as std_alloc, Layout};

// JS-provided wall clock (ms since an arbitrary epoch; performance.now()).
#[link(wasm_import_module = "env")]
extern "C" {
    fn now_ms() -> f64;
}

const MAX_HISTORY: usize = 256;
const MAX_TB_BYTES: usize = 32_000_000; // WDL-only jungle3.tb is ~31 MB decompressed
static mut HEX_BUF: [u8; 126] = [0; 126];
static mut HISTORY_BUF: [u64; MAX_HISTORY] = [0; MAX_HISTORY];
static mut TB: Option<Tablebase> = None;

/// Allocates a buffer from the Rust global allocator (the same allocator the
/// search's Vec/Box use), so the JS side can copy the tablebase bytes into wasm
/// memory without clobbering static data or the heap. Returns 0 on failure.
/// Memory is intentionally never freed (a single tablebase load per worker).
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let layout = match Layout::from_size_align(len, 8) {
        Ok(l) => l,
        Err(_) => return 0,
    };
    unsafe { std_alloc(layout) as usize }
}

/// Scratch buffer for the 126-char position hex (63 squares x 2 hex chars).
#[no_mangle]
pub extern "C" fn hex_buf() -> *mut u8 {
    std::ptr::addr_of_mut!(HEX_BUF).cast()
}

/// Scratch buffer for up to 256 game-history hashes (u64 each).
#[no_mangle]
pub extern "C" fn history_buf() -> *mut u64 {
    std::ptr::addr_of_mut!(HISTORY_BUF).cast()
}

/// Loads the tablebase from a byte slice the JS side has already written into
/// wasm memory. Returns 1 on success, 0 on a hash/corruption failure. The bytes
/// must be the decompressed jungle3.tb (the JS side gunzips the asset).
#[no_mangle]
pub extern "C" fn load_tablebase(ptr: *const u8, len: usize) -> u32 {
    if len == 0 || len > MAX_TB_BYTES {
        return 0;
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    match Tablebase::load(bytes) {
        Some(tb) => {
            unsafe { TB = Some(tb) };
            1
        }
        None => 0,
    }
}

/// Returns 1 when the tablebase is loaded and active (the search uses it).
#[no_mangle]
pub extern "C" fn tb_loaded() -> u32 {
    let tb = unsafe { (&raw const TB).as_ref().unwrap() };
    if tb.is_some() {
        1
    } else {
        0
    }
}

/// Runs a time-bounded iterative-deepening search over the position in
/// `hex_buf` and the first `history_len` hashes in `history_buf`. Returns a
/// packed result:
///   bits 32..  : node count
///   bits 24..31: last completed depth
///   bits 16..23: best move from-square
///   bits  8..15: best move to-square
///   bit 0      : 1 when no legal move
#[no_mangle]
pub extern "C" fn search_timed(turn: u32, history_len: usize, budget_ms: f64, max_depth: u32) -> u64 {
    let hex = unsafe { HEX_BUF };
    let mut grid = [0u8; 63];
    for sq in 0..63 {
        grid[sq] = (hex_val(hex[sq * 2]) << 4) | hex_val(hex[sq * 2 + 1]);
    }
    let mut board = Board::from_grid(&grid, turn as u8);

    let history: &[u64] = unsafe { &HISTORY_BUF[..history_len.min(MAX_HISTORY)] };

    let mut search = Search::with_clock(false, false, false, false, wasm_now_ms);
    let tb = unsafe { (&raw const TB).as_ref().unwrap() };
    if let Some(tb) = tb {
        search.set_tablebase(tb.clone());
    }
    let best = search.search_best_move(&mut board, max_depth as i32, history, Some(budget_ms));

    let (bf, bt) = best.unwrap_or((0, 0));
    ((search.nodes as u64) << 32)
        | ((search.last_completed_depth as u64) << 24)
        | ((bf as u64) << 16)
        | ((bt as u64) << 8)
        | (if best.is_none() { 1 } else { 0 })
}

/// perft smoke (movegen parity check in the browser).
#[no_mangle]
pub extern "C" fn perft(depth: u32) -> u64 {
    let b = Board::initial();
    jungle_lib::perft(&b, 0, depth as u8)
}

fn wasm_now_ms() -> f64 {
    unsafe { now_ms() }
}

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}
