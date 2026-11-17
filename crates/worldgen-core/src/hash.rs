//! Content hashing for determinism checks (FNV-1a 64).

pub struct Fnv64(u64);

impl Default for Fnv64 {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Fnv64 {
    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    pub fn write_f32s(&mut self, v: &[f32]) {
        for x in v {
            self.write(&x.to_le_bytes());
        }
    }

    pub fn finish(&self) -> u64 {
        self.0
    }
}

pub fn fnv64(bytes: &[u8]) -> u64 {
    let mut h = Fnv64::default();
    h.write(bytes);
    h.finish()
}

/// A fast, fixed-key hasher for the generators' lookup maps (spatial grids, graph
/// adjacency). std's default SipHash is randomly seeded and slow in WASM; neither matters
/// for lookups, and no output may depend on map iteration order either way.
#[derive(Default, Clone, Copy)]
pub struct FxHasher(u64);

impl FxHasher {
    #[inline]
    fn add(&mut self, w: u64) {
        self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

impl core::hash::Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for c in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..c.len()].copy_from_slice(c);
            self.add(u64::from_le_bytes(w));
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(i as u64);
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_i64(&mut self, i: i64) {
        self.add(i as u64);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

pub type FastMap<K, V> = std::collections::HashMap<K, V, core::hash::BuildHasherDefault<FxHasher>>;
pub type FastSet<K> = std::collections::HashSet<K, core::hash::BuildHasherDefault<FxHasher>>;
