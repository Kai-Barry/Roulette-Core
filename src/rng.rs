//! Deterministic Pseudo-Random Number Generator (PRNG).
//!
//! Provides a seeded Mulberry32 generator guaranteeing cross-platform bit-exact determinism
//! for combat simulations, replay validation, and networked synchronization.

/// Pure deterministic seedable Mulberry32 PRNG generator.
///
/// Ensures identical execution sequences across all platforms and operating systems
/// given the same 32-bit seed or seed string.
#[derive(Debug, Clone)]
pub struct Rng {
    /// Internal 32-bit state value.
    state: u32,
}

impl Rng {
    /// Creates a new PRNG instance initialized with a 32-bit integer seed.
    ///
    /// # Arguments
    /// * `seed` - The initial 32-bit seed state.
    ///
    /// # Examples
    /// ```rust
    /// use roulette_core::rng::Rng;
    /// let mut rng = Rng::new(42);
    /// let val = rng.next_u32();
    /// ```
    pub fn new(seed: u32) -> Self {
        Rng { state: seed }
    }

    /// Creates a new PRNG instance from a string seed using djb2 string hashing.
    ///
    /// # Arguments
    /// * `seed` - String slice used to compute the initial 32-bit seed.
    ///
    /// # Examples
    /// ```rust
    /// use roulette_core::rng::Rng;
    /// let mut rng = Rng::from_str("combat_seed_101");
    /// ```
    pub fn from_str(seed: &str) -> Self {
        let mut hash: u32 = 5381;
        for b in seed.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(b as u32);
        }
        Rng::new(hash)
    }

    /// Advances internal state and returns the next pseudo-random 32-bit unsigned integer.
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B79F5);
        let mut z = self.state;
        z = (z ^ (z >> 15)).wrapping_mul(z | 1);
        z ^= z.wrapping_add((z ^ (z >> 7)).wrapping_mul(z | 61));
        z ^ (z >> 14)
    }

    /// Generates a floating-point number in the semi-open range `[0.0, 1.0)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u32() as f64) / (u32::MAX as f64 + 1.0)
    }

    /// Generates an integer in the inclusive range `[min, max]`.
    ///
    /// # Arguments
    /// * `min` - Lower bound of range (inclusive).
    /// * `max` - Upper bound of range (inclusive).
    pub fn range_i32(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let range = (max - min + 1) as u32;
        min + (self.next_u32() % range) as i32
    }

    /// Shuffles a slice in-place deterministically using the Fisher-Yates algorithm.
    ///
    /// # Arguments
    /// * `slice` - Mutable slice of elements to shuffle.
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

    #[test]
    fn test_string_seed_consistency() {
        let mut rng1 = Rng::from_str("test_seed");
        let mut rng2 = Rng::from_str("test_seed");
        assert_eq!(rng1.next_u32(), rng2.next_u32());
    }
}
