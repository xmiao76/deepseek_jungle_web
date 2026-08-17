//! WASM exports for the browser backend: a fixed-depth search from the start
//! position (native search speed measurement in Chromium) plus the perft smoke.

use jungle_lib::perft as lib_perft;
use jungle_lib::{Board, Search};

/// perft(depth) from the start position (movegen smoke).
#[no_mangle]
pub extern "C" fn perft(depth: u32) -> u64 {
    let b = Board::initial();
    lib_perft(&b, 0, depth as u8)
}

/// Fixed-depth search from the start position; returns the node count. The JS
/// side derives nps from wall time (bare wasm has no clock).
#[no_mangle]
pub extern "C" fn search_nodes(depth: u32) -> u64 {
    let mut board = Board::initial();
    let mut search = Search::new(false, false, false, false);
    search.search_best_move(&mut board, depth as i32, &[]);
    search.nodes
}

/// Fixed-depth search from a packed position (hex 126-char layout + turn).
/// Returns (nodes << 8) | best_from<<4 | best_to (or 0 if no move). Layout is
/// the same square->type+1 hex encoding as the tuning/parity dumps.
#[no_mangle]
pub extern "C" fn search_packed(hex_ptr: *const u8, hex_len: usize, turn: u32, depth: u32) -> u64 {
    let slice = unsafe { std::slice::from_raw_parts(hex_ptr, hex_len) };
    if slice.len() < 126 {
        return 0;
    }
    let mut grid = [0u8; 63];
    for sq in 0..63 {
        let hi = hex_val(slice[sq * 2]);
        let lo = hex_val(slice[sq * 2 + 1]);
        grid[sq] = (hi << 4) | lo;
    }
    let mut board = Board::from_grid(&grid, turn as u8);
    let mut search = Search::new(false, false, false, false);
    let best = search.search_best_move(&mut board, depth as i32, &[]);
    let packed = match best {
        Some((f, t)) => ((f as u64) << 4) | (t as u64),
        None => 0,
    };
    (search.nodes << 8) | packed
}

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}
