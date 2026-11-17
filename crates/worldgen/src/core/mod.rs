pub mod clip;
pub mod geom;
pub mod grid;
pub mod hash;
pub mod noise;
pub mod rng;
pub mod tile;

// Exactly specified IEEE operations: every platform gives the same bits, so these use the
// hardware instructions. libm's software versions are much slower in WASM (sqrt above all).
// Transcendentals (sin, exp, pow, ...) still go through libm.

#[inline]
pub fn sqrt(x: f64) -> f64 {
    x.sqrt()
}

#[inline]
pub fn floor(x: f64) -> f64 {
    x.floor()
}

#[inline]
pub fn ceil(x: f64) -> f64 {
    x.ceil()
}

#[inline]
pub fn round(x: f64) -> f64 {
    x.round()
}

#[inline]
pub fn fabs(x: f64) -> f64 {
    x.abs()
}
