//! Placeholder Zobrist keys. The spike only needs move-count parity, so the
//! hash values need not match the C# engine yet — but the keys must be
//! deterministic. Full hash parity (C# Random(42) sequence) lands with the
//! full port.

pub fn piece_key(_sq: usize, _piece_type: usize) -> u64 {
    // xorshift64* with a fixed seed, indexed deterministically.
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ (0x0100_0000_0000 * _sq as u64 + 0x0001_0000_0000 * _piece_type as u64 + 1);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    x.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

pub fn turn_key() -> u64 {
    0xDEAD_BEEF_CAFE_F00D
}
