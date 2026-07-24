/// Pure deterministic seedable Mulberry32 PRNG generator.
/// Guarantees identical sequence across platforms given the same seed.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// Create a new RNG with a u32 seed.
    pub fn new(seed: u32) -> Self {
        Rng { state: seed }
    }

    /// Create an RNG from a string seed (e.g. "seed_123").
    pub fn from_str(seed: &str) -> Self {
        let mut hash: u32 = 5381;
        for b in seed.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u32);
        }
        Rng::new(hash)
    }

    /// Generate next raw u32 integer.
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut z = self.state;
        z = (z ^ (z >> 15)).wrapping_mul(z | 1);
        z ^= z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
        z ^ (z >> 14)
    }

    /// Generate f64 floating point number in [0.0, 1.0).
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }

    /// Generate an integer in range [min, max] inclusive.
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u32;
        min + (self.next_u32() % range) as i32
    }

    /// Shuffle a mutable slice in-place deterministically (Fisher-Yates).
    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        let len = slice.len();
        for i in (1..len).rev() {
            let j = (self.next_u32() % ((i + 1) as u32)) as usize;
            slice.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let mut rng1 = Rng::new(42);
        let mut rng2 = Rng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
        }
    }
}
