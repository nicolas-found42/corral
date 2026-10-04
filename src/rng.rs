//! CPython's Mersenne Twister, ported exactly.
//!
//! The Python app seeds `random.Random(--rng)` and relies on it for reproducible
//! eavesdrops ("--rng, random seed, for reproducible leaks"). Rust's `rand` crate
//! produces a different sequence, so we reproduce CPython's generator bit-for-bit:
//! `init_by_array` seeding from the integer's little-endian 32-bit words, and
//! `genrand_res53` for `random()`. `uniform(a, b) = a + (b - a) * random()`.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// A drop-in for `random.Random(seed)`'s two draws the app uses: `random()` and
/// `uniform(a, b)`.
#[derive(Debug, Clone)]
pub struct PyRandom {
    mt: [u32; N],
    mti: usize,
}

impl PyRandom {
    /// Seed exactly as `random.Random(n)` does: the absolute value's 32-bit words
    /// (little-endian), fed to `init_by_array`.
    pub fn new(seed: i64) -> Self {
        let n = seed.unsigned_abs();
        let bits = 64 - n.leading_zeros() as usize;
        let keyused = if bits == 0 { 1 } else { (bits - 1) / 32 + 1 };
        let mut key = Vec::with_capacity(keyused);
        let mut x = n;
        for _ in 0..keyused {
            key.push((x & 0xffff_ffff) as u32);
            x >>= 32;
        }
        let mut r = PyRandom {
            mt: [0u32; N],
            mti: N,
        };
        r.init_by_array(&key);
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.mti = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let key_length = key.len();
        let mut i: usize = 1;
        let mut j: usize = 0;
        let mut k = if N > key_length { N } else { key_length };
        while k > 0 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_664_525)))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key_length {
                j = 0;
            }
            k -= 1;
        }
        let mut k = N - 1;
        while k > 0 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ ((prev ^ (prev >> 30)).wrapping_mul(1_566_083_941)))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        self.mt[0] = 0x8000_0000;
        self.mti = N;
    }

    fn genrand_u32(&mut self) -> u32 {
        if self.mti >= N {
            let mag01 = [0u32, MATRIX_A];
            let mut kk = 0;
            while kk < N - M {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M] ^ (y >> 1) ^ mag01[(y & 1) as usize];
                kk += 1;
            }
            while kk < N - 1 {
                let y = (self.mt[kk] & UPPER_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + M - N] ^ (y >> 1) ^ mag01[(y & 1) as usize];
                kk += 1;
            }
            let y = (self.mt[N - 1] & UPPER_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[N - 1] = self.mt[M - 1] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            self.mti = 0;
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `random.random()`: a double in [0, 1) with 53 bits of randomness.
    pub fn random(&mut self) -> f64 {
        let a = (self.genrand_u32() >> 5) as f64;
        let b = (self.genrand_u32() >> 6) as f64;
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `random.uniform(a, b)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::PyRandom;

    #[test]
    fn matches_cpython_random_for_seed_7() {
        // Python: r = random.Random(7); [round(r.random(), 12) for _ in range(4)]
        let mut r = PyRandom::new(7);
        let got: Vec<f64> = (0..4).map(|_| (r.random() * 1e12).round() / 1e12).collect();
        assert_eq!(
            got,
            vec![
                0.323832764833,
                0.150849173925,
                0.650934473040,
                0.072436286668
            ]
        );
    }

    #[test]
    fn uniform_matches_cpython_for_seed_7() {
        // Python: r = random.Random(7); [round(r.uniform(0.75, 1.05), 12) for _ in range(3)]
        let mut r = PyRandom::new(7);
        let got: Vec<f64> = (0..3)
            .map(|_| (r.uniform(0.75, 1.05) * 1e12).round() / 1e12)
            .collect();
        assert_eq!(got, vec![0.847149829450, 0.795254752177, 0.945280341912]);
    }

    #[test]
    fn matches_cpython_random_for_seed_0() {
        // Python: r = random.Random(0); [round(r.random(), 12) for _ in range(4)]
        let mut r = PyRandom::new(0);
        let got: Vec<f64> = (0..4).map(|_| (r.random() * 1e12).round() / 1e12).collect();
        assert_eq!(
            got,
            vec![
                0.844421851525,
                0.757954402940,
                0.420571580831,
                0.258916750293
            ]
        );
    }
}
