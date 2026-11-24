//! Transposition table — port of the C# TranspositionTable (4-bucket,
//! generation-tagged, depth-preferred, same-hash update in place).

pub const MATE_SCORE: i32 = 1_000_000;
pub const MATE_RANGE: i32 = 500;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy)]
pub struct TtEntry {
    pub hash: u64,
    pub generation: u8,
    pub depth: i32,
    pub score: i32,
    pub best_move: Option<(u8, u8)>, // None = no move
    pub bound: Bound,
}

const BUCKET_COUNT: usize = 4;
const MAX_GEN_LAG: u8 = 1;

pub struct TranspositionTable {
    table: Vec<TtEntry>,
    slot_count: usize,
    generation: u8,
}

impl TranspositionTable {
    pub fn new(size: usize) -> Self {
        let slot_count = (size / BUCKET_COUNT).max(1);
        let table = vec![
            TtEntry {
                hash: 0,
                generation: 0,
                depth: 0,
                score: 0,
                best_move: None,
                bound: Bound::Exact
            };
            slot_count * BUCKET_COUNT
        ];
        TranspositionTable {
            table,
            slot_count,
            generation: 0,
        }
    }

    pub fn new_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    fn is_fresh(entry_gen: u8, current_gen: u8) -> bool {
        current_gen.wrapping_sub(entry_gen) <= MAX_GEN_LAG
    }

    pub fn store(
        &mut self,
        hash: u64,
        depth: i32,
        score: i32,
        best_move: Option<(u8, u8)>,
        bound: Bound,
    ) {
        let base = (hash as usize & (self.slot_count - 1)) * BUCKET_COUNT;
        // Same hash: update in place.
        for i in 0..BUCKET_COUNT {
            if self.table[base + i].hash == hash {
                self.table[base + i] = TtEntry {
                    hash,
                    generation: self.generation,
                    depth,
                    score,
                    best_move,
                    bound,
                };
                return;
            }
        }
        // Stale entries first.
        for i in 0..BUCKET_COUNT {
            if !Self::is_fresh(self.table[base + i].generation, self.generation) {
                self.table[base + i] = TtEntry {
                    hash,
                    generation: self.generation,
                    depth,
                    score,
                    best_move,
                    bound,
                };
                return;
            }
        }
        // Depth-preferred: overwrite the shallowest only if at least as deep.
        let mut shallowest = 0;
        for i in 1..BUCKET_COUNT {
            if self.table[base + i].depth < self.table[base + shallowest].depth {
                shallowest = i;
            }
        }
        if depth >= self.table[base + shallowest].depth {
            self.table[base + shallowest] = TtEntry {
                hash,
                generation: self.generation,
                depth,
                score,
                best_move,
                bound,
            };
        }
    }

    /// Returns (cut, score, tt_move). On a stale/shallow/non-cutting hash match,
    /// `cut` is false but `tt_move` still carries the ordering move.
    pub fn try_probe(
        &self,
        hash: u64,
        depth: i32,
        alpha: i32,
        beta: i32,
        ply: i32,
    ) -> (bool, i32, Option<(u8, u8)>) {
        let base = (hash as usize & (self.slot_count - 1)) * BUCKET_COUNT;
        for i in 0..BUCKET_COUNT {
            let e = &self.table[base + i];
            if e.hash != hash {
                continue;
            }
            if !Self::is_fresh(e.generation, self.generation) || e.depth < depth {
                return (false, 0, e.best_move);
            }
            let mut s = e.score;
            if s > MATE_SCORE - MATE_RANGE {
                s -= ply;
            } else if s < -(MATE_SCORE - MATE_RANGE) {
                s += ply;
            }
            match e.bound {
                Bound::Exact => return (true, s, e.best_move),
                Bound::Lower => {
                    if s >= beta {
                        return (true, s, e.best_move);
                    }
                }
                Bound::Upper => {
                    if s <= alpha {
                        return (true, s, e.best_move);
                    }
                }
            }
            return (false, 0, e.best_move);
        }
        (false, 0, None)
    }
}
