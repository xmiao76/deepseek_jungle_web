//! Byte-exact port of .NET's `System.Random` (the compatibility Knuth
//! subtractive generator used by `new Random(seed)`), so the Zobrist keys match
//! the C# engine bit-for-bit. This is the linchpin of the node-count parity gate.

const MBIG: i32 = i32::MAX; // 2147483647

pub struct NetRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl NetRandom {
    pub fn new(seed: i32) -> Self {
        let mut seed_array = [0i32; 56];
        let subtraction = seed.abs();
        let mut mj = 161803398i32 - subtraction; // MSEED - subtraction
        seed_array[55] = mj;
        let mut mk = 1i32;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            seed_array[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += MBIG;
            }
            mj = seed_array[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                seed_array[i] -= seed_array[1 + (i + 30) % 55];
                if seed_array[i] < 0 {
                    seed_array[i] += MBIG;
                }
            }
        }
        NetRandom { seed_array, inext: 0, inextp: 21 }
    }

    /// .NET `Random.InternalSample()`: returns a value in [0, 2147483646].
    pub fn internal_sample(&mut self) -> i32 {
        let mut loc_inext = self.inext + 1;
        if loc_inext >= 56 {
            loc_inext = 1;
        }
        let mut loc_inextp = self.inextp + 1;
        if loc_inextp >= 56 {
            loc_inextp = 1;
        }

        let mut ret_val = self.seed_array[loc_inext] - self.seed_array[loc_inextp];
        if ret_val == MBIG {
            ret_val -= 1;
        }
        if ret_val < 0 {
            ret_val += MBIG;
        }

        self.seed_array[loc_inext] = ret_val;
        self.inext = loc_inext;
        self.inextp = loc_inextp;
        ret_val
    }

    /// .NET `Random.NextBytes`: each byte is the low 8 bits of one sample.
    pub fn next_bytes(&mut self, buf: &mut [u8]) {
        for b in buf.iter_mut() {
            *b = (self.internal_sample() & 0xFF) as u8;
        }
    }

    /// .NET `Random.Next()` returning an unsigned 64-bit (8 NextBytes + little-endian).
    pub fn next_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        self.next_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_key_matches_csharp_oracle() {
        let mut rng = NetRandom::new(42);
        let first = rng.next_u64();
        assert_eq!(first, 0x3bcd04ae96ba173e, "first Zobrist key must match C#");
        let second = rng.next_u64();
        assert_eq!(second, 0x3abfadf0569e8699, "second Zobrist key must match C#");
    }
}
