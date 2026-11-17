//! Stateless hash RNG keyed by (seed, coordinates) plus a small sequential PCG32.
//! Stateless hashing is what makes generation order-independent: a value at a lattice
//! point never depends on which tile asked for it first.

#[inline]
pub const fn mix64(mut z: u64) -> u64 {
    // SplitMix64 finalizer.
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[inline]
pub fn hash2(seed: u64, x: i64, y: i64) -> u64 {
    mix64(seed ^ mix64((x as u64) ^ mix64((y as u64) ^ 0x9e37_79b9_7f4a_7c15)))
}

#[inline]
pub fn hash3(seed: u64, a: i64, b: i64, c: i64) -> u64 {
    mix64(seed ^ mix64((a as u64) ^ mix64((b as u64) ^ mix64((c as u64) ^ 0x632b_e59b_d9b4_e019))))
}

/// Uniform in [0, 1).
#[inline]
pub fn unit(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Uniform in [-1, 1).
#[inline]
pub fn signed(h: u64) -> f64 {
    unit(h) * 2.0 - 1.0
}

/// Derive a named sub-seed so independent generators never share random streams.
#[inline]
pub fn stream(seed: u64, name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    mix64(seed ^ h)
}

/// PCG32 (XSH RR) for sequential sampling inside a single generator run.
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

impl Pcg32 {
    #[inline]
    pub fn new(seed: u64, seq: u64) -> Self {
        let mut r = Self { state: 0, inc: (seq << 1) | 1 };
        r.next_u32();
        r.state = r.state.wrapping_add(seed);
        r.next_u32();
        r
    }

    /// Its whole state (`from_state` makes the same stream again: a hand town's block kept as it
    /// was laid out, `town::hand`).
    #[inline]
    pub fn state(&self) -> (u64, u64) {
        (self.state, self.inc)
    }

    #[inline]
    pub fn from_state(state: u64, inc: u64) -> Self {
        Self { state, inc }
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        let hi = (self.next_u32() as u64) << 21;
        let lo = (self.next_u32() >> 11) as u64;
        (hi | lo) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    #[inline]
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }

    /// Uniform integer in [0, n).
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }
}
