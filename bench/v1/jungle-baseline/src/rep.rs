//! Repetition path table — port of the C# RepetitionTable (open-addressing
//! hash->count with backward-shift deletion). Semantics: a position is a
//! three-fold repetition when its hash occurs >= 2 times on the path.

const PATH_CAPACITY: usize = 256;
const TABLE_SLOTS: usize = 512;
const MAX_PLY: usize = 64;

pub struct RepetitionTable {
    hashes: Vec<u64>,
    counts: Vec<u8>,
    path_length: usize,
}

impl RepetitionTable {
    pub fn new() -> Self {
        RepetitionTable {
            hashes: vec![0; TABLE_SLOTS],
            counts: vec![0; TABLE_SLOTS],
            path_length: 0,
        }
    }

    pub fn reset(&mut self) {
        self.hashes.fill(0);
        self.counts.fill(0);
        self.path_length = 0;
    }

    pub fn seed_path(&mut self, history: &[u64]) {
        let seed = history.len().min(PATH_CAPACITY - MAX_PLY);
        for i in 0..seed {
            self.push(history[i]);
        }
    }

    pub fn can_push(&self) -> bool {
        self.path_length < PATH_CAPACITY
    }

    pub fn push(&mut self, hash: u64) {
        let idx = self.find(hash);
        if self.counts[idx] == 0 {
            self.hashes[idx] = hash;
        }
        self.counts[idx] += 1;
        self.path_length += 1;
    }

    pub fn pop(&mut self, hash: u64) {
        let idx = self.find(hash);
        self.path_length -= 1;
        self.counts[idx] -= 1;
        if self.counts[idx] == 0 {
            self.hashes[idx] = 0;
            let mut next = (idx + 1) & (TABLE_SLOTS - 1);
            while self.hashes[next] != 0 {
                let h = self.hashes[next];
                let c = self.counts[next];
                self.hashes[next] = 0;
                self.counts[next] = 0;
                let mut pos = (h as usize) & (TABLE_SLOTS - 1);
                while self.hashes[pos] != 0 {
                    pos = (pos + 1) & (TABLE_SLOTS - 1);
                }
                self.hashes[pos] = h;
                self.counts[pos] = c;
                next = (next + 1) & (TABLE_SLOTS - 1);
            }
        }
    }

    pub fn is_repetition(&self, hash: u64) -> bool {
        let idx = self.find(hash);
        self.counts[idx] >= 2
    }

    fn find(&self, hash: u64) -> usize {
        let mut idx = (hash as usize) & (TABLE_SLOTS - 1);
        while self.hashes[idx] != 0 && self.hashes[idx] != hash {
            idx = (idx + 1) & (TABLE_SLOTS - 1);
        }
        idx
    }
}
