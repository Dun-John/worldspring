//! Castles' moats on the battlemap (v63 castles; `town::moat`): the ground dug and the water let
//! in on the height and water grid (the ground's own water drawing, deep in the middle), or a dry
//! ditch with steep sides and a low bank on its outer lip; the drawbridge a deck over it, walked
//! like a bridge's. Every square by its world position alone (seamless across chunks).

use super::{HS, SQUARE_FT};
use crate::core::noise::smoothstep;
use crate::town::moat::{self, Moat};

/// Dig `moats` into the halo grid (square (i, j)'s middle at `hx0 + (i + 0.5) * SQUARE_FT`, likewise
/// y): heights `hh`, water `hw`, and the drawbridges' decks (`road_h` a bridge's planks, `lift` the
/// deck's height over the moat's bed). Squares on a road (a causeway) or already under water are
/// left. Returns the halo squares of dry ditches (difficult ground).
pub(super) fn dig(moats: &[Moat], hx0: f64, hy0: f64, hh: &mut [f32], hw: &mut [f32], road_h: &mut [u8], lift: &mut [f32]) -> Vec<bool> {
    let mut ditch = vec![false; HS * HS];
    let span = HS as f64 * SQUARE_FT;
    for m in moats {
        let b = m.bbox;
        if b[2] < hx0 || b[0] > hx0 + span || b[3] < hy0 || b[1] > hy0 + span {
            continue;
        }
        let i0 = crate::core::floor((b[0] - hx0) / SQUARE_FT).max(0.0) as usize;
        let j0 = crate::core::floor((b[1] - hy0) / SQUARE_FT).max(0.0) as usize;
        let i1 = (crate::core::ceil((b[2] - hx0) / SQUARE_FT).max(0.0) as usize).min(HS - 1);
        let j1 = (crate::core::ceil((b[3] - hy0) / SQUARE_FT).max(0.0) as usize).min(HS - 1);
        let level = m.level;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let h = j * HS + i;
                if road_h[h] != 0 || hw[h] > hh[h] {
                    continue;
                }
                let p = [hx0 + (i as f64 + 0.5) * SQUARE_FT, hy0 + (j as f64 + 0.5) * SQUARE_FT];
                let Some(s) = m.spot(p) else { continue };
                let ground = hh[h];
                if s.inside() {
                    let t = s.inner.min(s.outer);
                    if m.wet {
                        hh[h] = hh[h].min(level - (moat::EDGE_DEEP + (moat::MID_DEEP - moat::EDGE_DEEP) * smoothstep(0.0, 5.0, t) as f32));
                        hw[h] = level;
                    } else {
                        let g = s.outer - moat::BANK_FT;
                        if g >= 0.0 {
                            hh[h] -= moat::DITCH_FT * smoothstep(0.0, 3.0, s.inner.min(g)) as f32;
                            hw[h] = hw[h].min(hh[h] - 4.0);
                            ditch[h] = true;
                        } else {
                            hh[h] += moat::BANK_UP;
                        }
                    }
                } else if m.wet {
                    // (Its banks above its water, the water's level held a square past the rim, as a
                    // pond's.)
                    hh[h] = hh[h].max(level + 0.8);
                    hw[h] = level;
                }
                // The drawbridge's deck at the ground's height (over water, a foot above it at least).
                if m.bridge.covers(p) && s.inside() {
                    let top = if m.wet { ground.max(level + 1.0) } else { ground.max(hh[h]) };
                    lift[h] += top - hh[h];
                    hh[h] = top;
                    road_h[h] = 0x80 | 3;
                    ditch[h] = false;
                }
            }
        }
    }
    ditch
}
