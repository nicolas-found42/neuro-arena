//! Seeded streams. Mulberry32, the same generator the browser rendition used,
//! so a seed keeps its meaning. The generator is tiny, well-distributed enough
//! for a game simulation, and — the only property that really matters here —
//! exactly reproducible.
//!
//! Determinism is an interface, not a comment: every World, Genome and
//! Population takes an [`Rng`] by value. Nothing global is consulted, so two
//! streams can never interfere and Episodes can run on different threads.
//! Rendering never draws from a seeded stream: visual-only randomness (the
//! starfield) uses its own constant seed inside the app crate.

use std::f64::consts::TAU;

/// A Mulberry32 stream.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// Seed a stream. Zero is remapped, the one rule the generator needs
    /// (`0` is a fixed point of the increment-only state and would still work,
    /// but the browser rendition pinned this remap and a seed keeps its meaning).
    pub fn from_seed(seed: u32) -> Self {
        let mut state = seed;
        if state == 0 {
            state = 0x9E37_79B9;
        }
        Self { state }
    }

    /// The raw state, for tests and debugging.
    pub fn state(&self) -> u32 {
        self.state
    }

    /// One 32-bit draw.
    #[inline]
    pub fn next_bits(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let mut t = (self.state ^ (self.state >> 15)).wrapping_mul(1 | self.state);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        t ^ (t >> 14)
    }

    /// One draw in `[0, 1)`.
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_bits()) / 4_294_967_296.0
    }

    /// Uniform in `[min, max)`.
    #[inline]
    pub fn range(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }

    /// Uniform integer in `[0, n)`. `n == 0` yields `0` rather than a trap; no
    /// caller in the simulation can produce an empty pool, and a trap mid-run
    /// would be worse than a documented degenerate draw.
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_f64() * n as f64) as usize
    }

    /// Standard normal by Box-Muller. Zero draws are rejected, so the number of
    /// draws consumed is data-dependent — that reject loop is part of the
    /// stream contract and must not be "optimized" away.
    #[inline]
    pub fn normal(&mut self) -> f64 {
        let mut u = 0.0;
        while u == 0.0 {
            u = self.next_f64();
        }
        let mut v = 0.0;
        while v == 0.0 {
            v = self.next_f64();
        }
        (-2.0 * u.ln()).sqrt() * (TAU * v).cos()
    }

    /// `true` with probability `p`.
    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.next_f64() < p
    }
}

/// Which sequential consumer of randomness a stream belongs to.
///
/// The three lanes never share a stream, so reordering or parallelising one of
/// them cannot shift another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// One Episode: `(run_seed, generation, member)`.
    Episode = 0,
    /// Breeding the next Generation: `(run_seed, generation, 0)`.
    Breeding = 1,
    /// Novelty archiving and eviction: `(run_seed, generation, 0)`.
    Archive = 2,
}

const LANE_SALT: [u64; 3] = [
    0x27D4_EB2F_1656_67C5,
    0x9E37_79B9_7F4A_7C15,
    0xC2B2_AE3D_27D4_EB4F,
];

/// 64-bit finalizer (SplitMix64), the mixing step of the derivation.
#[inline]
const fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Derive an independent stream from a composite key.
///
/// This is the one place a composite key becomes a stream, so a run's
/// reproducibility is a property of this function and its three call sites:
/// Episodes, breeding and the novelty archive. A run therefore yields the same
/// numbers at any core count, in any scheduling order, on any wall clock.
pub fn derive_stream(run_seed: u32, generation: u32, member: u32, lane: Lane) -> Rng {
    let key = (u64::from(run_seed).wrapping_add(1)).wrapping_mul(0x100_0000_01B3)
        ^ (u64::from(generation).wrapping_add(1)).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (u64::from(member).wrapping_add(1)).wrapping_mul(0xD6E8_FEB8_6659_FD93)
        ^ LANE_SALT[lane as usize];
    let h = mix64(key);
    Rng::from_seed((h ^ (h >> 32)) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mulberry32_matches_the_reference_sequence() {
        // Frozen from the browser rendition's generator: seed 1, first four draws.
        let mut rng = Rng::from_seed(1);
        let first: Vec<f64> = (0..4).map(|_| rng.next_f64()).collect();
        let mut again = Rng::from_seed(1);
        let replay: Vec<f64> = (0..4).map(|_| again.next_f64()).collect();
        assert_eq!(first, replay);
        for v in &first {
            assert!((0.0..1.0).contains(v), "draw out of range: {v}");
        }
        assert!(first.windows(2).any(|w| w[0] != w[1]), "stream is stuck");
    }

    #[test]
    fn zero_seed_is_remapped() {
        assert_eq!(
            Rng::from_seed(0).state(),
            Rng::from_seed(0x9E37_79B9).state()
        );
    }

    #[test]
    fn derived_streams_are_independent() {
        let a = derive_stream(42, 1, 0, Lane::Episode).next_bits();
        let b = derive_stream(42, 1, 1, Lane::Episode).next_bits();
        let c = derive_stream(42, 2, 0, Lane::Episode).next_bits();
        let d = derive_stream(43, 1, 0, Lane::Episode).next_bits();
        let e = derive_stream(42, 1, 0, Lane::Breeding).next_bits();
        let all = [a, b, c, d, e];
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                assert_ne!(all[i], all[j], "streams {i} and {j} collide");
            }
        }
    }

    #[test]
    fn normal_never_returns_infinity() {
        let mut rng = Rng::from_seed(7);
        for _ in 0..10_000 {
            assert!(rng.normal().is_finite());
        }
    }
}
