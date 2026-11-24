//! 2+3-piece WDL tablebase probe — a faithful port of the C# TablebaseIndex +
//! TablebaseFile (read path) + TablebaseProbe (WDL-only). The table is built
//! offline by the C# Bench --tb-build and shipped as a gzip asset; this module
//! only needs the index math and the 2-bit WDL lookup.

use crate::board::*;
use crate::tt::MATE_SCORE;

pub const USABLE_SQUARES: usize = 61;
pub const PIECE_TYPE_COUNT: usize = 16;
pub const COMBO2_COUNT: usize = 36;
pub const PLACEMENTS2: usize = USABLE_SQUARES * (USABLE_SQUARES - 1); // 3660
pub const ENTRIES_PER_COMBO2: usize = PLACEMENTS2 * 2;
pub const COMBO3_COUNT: usize = 288;
pub const PLACEMENTS3: usize = USABLE_SQUARES * (USABLE_SQUARES - 1) * (USABLE_SQUARES - 2); // 215940
pub const ENTRIES_PER_COMBO3: usize = PLACEMENTS3 * 2;

const DEN_BLUE_SQ: usize = 3;
const DEN_RED_SQ: usize = 59;

#[inline]
pub fn type_of(piece_id: u8) -> usize {
    ((piece_id - 1) % PIECE_TYPE_COUNT as u8) as usize
}

/// Square -> usable index, or usize::MAX for a den.
fn usable_of_table() -> &'static [usize; SQUARES] {
    static T: std::sync::OnceLock<[usize; SQUARES]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut u = [usize::MAX; SQUARES];
        let mut usable = 0usize;
        for sq in 0..SQUARES {
            if sq == DEN_BLUE_SQ || sq == DEN_RED_SQ {
                continue;
            }
            u[sq] = usable;
            usable += 1;
        }
        u
    })
}

/// combo2[blueType, redType] -> orbit id, or -1.
fn combo2_of_table() -> &'static [[i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT] {
    static T: std::sync::OnceLock<[[i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT]> =
        std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut c = [[-1i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT];
        let mut combo2 = 0i16;
        for x in (0..PIECE_TYPE_COUNT).step_by(2) {
            for y in (1..PIECE_TYPE_COUNT).step_by(2) {
                if c[x][y] >= 0 {
                    continue;
                }
                c[x][y] = combo2;
                c[y ^ 1][x ^ 1] = combo2;
                combo2 += 1;
            }
        }
        c
    })
}

/// combo3[a][b][c] -> 2v1 combo id (blue pair a <= b, red type c), or -1.
fn combo3_of_table() -> &'static [[[i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT] {
    static T: std::sync::OnceLock<[[[i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT]> =
        std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut c = [[[-1i16; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT]; PIECE_TYPE_COUNT];
        let mut combo3 = 0i16;
        for a in (0..PIECE_TYPE_COUNT).step_by(2) {
            for b in (a..PIECE_TYPE_COUNT).step_by(2) {
                for cc in (1..PIECE_TYPE_COUNT).step_by(2) {
                    c[a][b][cc] = combo3;
                    combo3 += 1;
                }
            }
        }
        c
    })
}

#[inline]
pub fn rotate_square(sq: usize) -> usize {
    (8 - sq / 7) * 7 + (6 - sq % 7)
}

#[inline]
fn rank2(a: usize, b: usize) -> usize {
    a * (USABLE_SQUARES - 1) + if b < a { b } else { b - 1 }
}

#[inline]
fn rank3(a: usize, b: usize, c: usize) -> usize {
    let mut r = a * (USABLE_SQUARES - 1) * (USABLE_SQUARES - 2);
    let bb = if b < a { b } else { b - 1 };
    let cc = if c < a { c } else { c - 1 };
    r += bb * (USABLE_SQUARES - 2) + if cc < bb { cc } else { cc - 1 };
    r
}

fn key2(type_a: usize, sq_a: usize, type_b: usize, sq_b: usize, stm: usize) -> usize {
    let ua = usable_of_table()[sq_a];
    let ub = usable_of_table()[sq_b];
    let self_orbit = (type_b ^ 1) == type_a;
    if self_orbit {
        let rb = usable_of_table()[rotate_square(sq_b)];
        let ra = usable_of_table()[rotate_square(sq_a)];
        let use_rotated = rb < ua || (rb == ua && ra < ub);
        if use_rotated {
            return rank2(rb, ra) * 2 + (stm ^ 1);
        }
        return rank2(ua, ub) * 2 + stm;
    }
    let rotated_smaller =
        ((type_b ^ 1) < type_a) || ((type_b ^ 1) == type_a && (type_a ^ 1) < type_b);
    if !rotated_smaller {
        return rank2(ua, ub) * 2 + stm;
    }
    let blue = usable_of_table()[rotate_square(sq_b)];
    let red = usable_of_table()[rotate_square(sq_a)];
    rank2(blue, red) * 2 + (stm ^ 1)
}

fn key3(combo: usize, sq_a: usize, sq_b: usize, sq_c: usize, stm: usize) -> usize {
    let ua = usable_of_table()[sq_a];
    let ub = usable_of_table()[sq_b];
    let uc = usable_of_table()[sq_c];
    combo * ENTRIES_PER_COMBO3 + rank3(ua, ub, uc) * 2 + stm
}

/// Unpacks the 2-bit WDL value (1 = loss, 2 = draw, 3 = win; 0 never stored).
#[inline]
fn get_wdl(packed: &[u8], entry: usize) -> u8 {
    let bit = entry * 2;
    (packed[bit >> 3] >> (bit & 7)) & 3
}

/// A loaded WDL table (2-piece + 3-piece packed arrays).
#[derive(Clone)]
pub struct Tablebase {
    wdl2: Box<[u8]>,
    wdl3: Box<[u8]>,
}

impl Tablebase {
    pub fn load(bytes: &[u8]) -> Option<Tablebase> {
        // Header: magic "JGLTB3" (6), version (1), flags (1), header size (2),
        // reserved (22), SHA-256 (32), then the payload.
        const HEADER: usize = 64;
        if bytes.len() < HEADER {
            return None;
        }
        if &bytes[0..6] != b"JGLTB3" {
            return None;
        }
        if bytes[6] != 1 {
            return None;
        }
        let has_dtm = (bytes[7] & 1) != 0;
        let header_size = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        if header_size != HEADER {
            return None;
        }
        let stored_hash = &bytes[32..64];
        let payload = &bytes[HEADER..];

        // SHA-256 verification (simple, self-contained).
        let computed = sha256(payload);
        if computed.as_slice() != stored_hash {
            return None;
        }

        let wdl2_bytes = (COMBO2_COUNT * ENTRIES_PER_COMBO2 * 2 + 7) / 8;
        let wdl3_bytes = (COMBO3_COUNT * ENTRIES_PER_COMBO3 * 2 + 7) / 8;
        let dtm2_bytes = COMBO2_COUNT * ENTRIES_PER_COMBO2;
        let dtm3_bytes = COMBO3_COUNT * ENTRIES_PER_COMBO3;
        let expected = wdl2_bytes + wdl3_bytes + if has_dtm { dtm2_bytes + dtm3_bytes } else { 0 };
        if payload.len() != expected {
            return None;
        }

        let wdl2 = payload[0..wdl2_bytes].to_vec().into_boxed_slice();
        let wdl3 = payload[wdl2_bytes..wdl2_bytes + wdl3_bytes]
            .to_vec()
            .into_boxed_slice();
        Some(Tablebase { wdl2, wdl3 })
    }

    /// Exact WDL for a <= 3-piece position, from the side to move's perspective
    /// in the search's mate convention. Returns (found, score); `found` is
    /// false when the position is not resolvable (or a draw, mapped to 0).
    pub fn try_probe(&self, board: &Board, ply: i32) -> (bool, i32) {
        let count = board.count[0] as usize + board.count[1] as usize;
        if count < 2 || count > 3 || board.winner != NO_WINNER {
            return (false, 0);
        }
        if count == 2 {
            if let Some(entry) = self.try_key2(board) {
                let wdl = get_wdl(&self.wdl2, entry);
                return (wdl != 0, map_score(wdl, 0, ply));
            }
            return (false, 0);
        }
        if let Some(entry) = self.try_key3(board) {
            let wdl = get_wdl(&self.wdl3, entry);
            return (wdl != 0, map_score(wdl, 0, ply));
        }
        (false, 0)
    }

    fn try_key2(&self, board: &Board) -> Option<usize> {
        // Collect the two pieces in the C# order: Blue (side 0) first, then Red.
        let mut types = [0u8; 2];
        let mut squares = [0u8; 2];
        let mut n = 0usize;
        for side in 0..2u8 {
            for sq in 0..SQUARES {
                let id = board.squares[sq];
                if id != 0 && owner_of(id) == side {
                    types[n] = type_of(id) as u8;
                    squares[n] = sq as u8;
                    n += 1;
                }
            }
        }
        if n != 2 {
            return None;
        }
        let combo = combo2_of_table()[types[0] as usize][types[1] as usize];
        if combo < 0 {
            return None;
        }
        let entry = combo as usize * ENTRIES_PER_COMBO2
            + key2(
                types[0] as usize,
                squares[0] as usize,
                types[1] as usize,
                squares[1] as usize,
                board.turn as usize,
            ) % ENTRIES_PER_COMBO2;
        Some(entry)
    }

    fn try_key3(&self, board: &Board) -> Option<usize> {
        let (ta, sa, tb, sb, tc, sc) = extract3(board);
        let combo = combo3_of_table()[ta as usize][tb as usize][tc as usize];
        let stm = if board.count[0] == 2 {
            board.turn
        } else {
            board.turn ^ 1
        };
        let entry = key3(
            combo as usize,
            sa as usize,
            sb as usize,
            sc as usize,
            stm as usize,
        );
        Some(entry)
    }
}

/// Extracts the 3-piece position in the 2v1 orientation.
fn extract3(board: &Board) -> (u8, u8, u8, u8, u8, u8) {
    // (a type, a sq, b type, b sq, c type, c sq)
    if board.count[0] == 2 {
        // Blue pair (a <= b) and the red type.
        let mut blue: Vec<(u8, u8)> = Vec::with_capacity(2);
        let mut red = (0u8, 0u8);
        for sq in 0..SQUARES {
            let id = board.squares[sq];
            if id != 0 {
                let t = type_of(id) as u8;
                if owner_of(id) == 0 {
                    blue.push((t, sq as u8));
                } else {
                    red = (t, sq as u8);
                }
            }
        }
        blue.sort();
        let (a, sa) = blue[0];
        let (b, sb) = blue[1];
        (a, sa, b, sb, red.0, red.1)
    } else {
        // Blue holds one piece: rotate (red pair becomes the blue pair).
        let mut red: Vec<(u8, u8)> = Vec::with_capacity(2);
        let mut blue = (0u8, 0u8);
        for sq in 0..SQUARES {
            let id = board.squares[sq];
            if id != 0 {
                let t = type_of(id) as u8;
                if owner_of(id) == 1 {
                    let rt = t ^ 1;
                    let rq = rotate_square(sq as usize) as u8;
                    red.push((rt, rq));
                } else {
                    let rt = t ^ 1;
                    let rq = rotate_square(sq as usize) as u8;
                    blue = (rt, rq);
                }
            }
        }
        red.sort();
        let (a, sa) = red[0];
        let (b, sb) = red[1];
        (a, sa, b, sb, blue.0, blue.1)
    }
}

fn map_score(wdl: u8, _dtm: i32, ply: i32) -> i32 {
    match wdl {
        3 => MATE_SCORE - ply,
        1 => ply - MATE_SCORE,
        _ => 0,
    }
}

// A minimal SHA-256 (FIPS 180-4), self-contained so the wasm build has no
// external dependency.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut out = [0u8; 32];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}
