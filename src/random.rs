use core::sync::atomic::{AtomicU64, Ordering};

// Thread-safe RNG trait for shared references
pub trait ThreadSafeRng {
    fn next_u64(&self) -> u64;
}

// Exponential random number generator
// the `factor` must be in the range (0.0, 1.0)
pub fn exponential_random(rng: &impl ThreadSafeRng, factor: f64, max: u8) -> u8 {
    let max_power = factor.powi(max as i32 + 1);
    let u = rng.next_u64() as f64 / (u64::MAX as f64 + 1.0);
    let thresh = 1.0 - u * (1.0 - max_power);

    let mut n = 0;
    let mut current = factor;
    while n < max {
        if current <= thresh {
            return n;
        }
        current *= factor;
        n += 1;
    }
    n
}

// Atomic-based thread-safe RNG implementation
pub struct AtomicRng(AtomicU64);

impl AtomicRng {
    pub const fn new(seed: u64) -> Self {
        Self(AtomicU64::new(seed))
    }
}

impl ThreadSafeRng for AtomicRng {
    fn next_u64(&self) -> u64 {
        // Simple LCG parameters (from Numerical Recipes)
        const MULTIPLIER: u64 = 6364136223846793005;
        const INCREMENT: u64 = 1;

        // Atomic update using fetch_add
        let old = self.0.fetch_add(1, Ordering::Relaxed);
        old.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT)
    }
}
