//! WASM spike exports: pure perft (the JS side drives the time loop — bare
//! wasm32-unknown-unknown has no clock).

use jungle_lib::perft as lib_perft;
use jungle_lib::Board;

/// perft(depth) from the start position (parity smoke + JS-driven bench).
#[no_mangle]
pub extern "C" fn perft(depth: u32) -> u64 {
    let b = Board::initial();
    lib_perft(&b, 0, depth as u8)
}
