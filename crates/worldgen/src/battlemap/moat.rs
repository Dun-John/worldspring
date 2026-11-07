//! Castles' moats on the battlemap (v63 castles; `town::moat`): the ground dug and the water let
//! in on the height and water grid (the ground's own water drawing, deep in the middle), or a dry
//! ditch with steep sides and a low bank on its outer lip; the drawbridge a deck over it, walked
//! like a bridge's. Every square by its world position alone (seamless across chunks).

use super::{HS, SQUARE_FT};
use crate::core::noise::smoothstep;
use crate::town::moat::{self, Moat};

/// The ground's rise (ft a foot) across a wet moat's rim, in from it to its bed and out from it up
/// its bank: the same both ways, so the water's edge between squares (where their heights cross
/// its level) falls on the moat's own line, not on the squares' steps.
const SHORE: f32 = 0.3;
/// How deep (ft) a dry ditch is drawn, and how high its outer bank: its floor eased down over 6 ft
/// (the ground's drawing then shades it gently, an earthen ditch, not a rock cut); its tiers keep
/// the depth it is dug to.
const DITCH_SHOWN: f32 = 2.0;
const BANK_SHOWN: f32 = 1.0;

/// What `dig` did, by halo square: a dry ditch's floor (difficult ground), a wet moat's water and
/// the bank round it (where the ground's drawing keeps to the water's edge: `edges` bit 32), and how
/// much higher a dry ditch's ground is drawn than dug (`DITCH_SHOWN`; added once its tiers are set).
pub(super) struct Dug {
    pub ditch: Vec<bool>,
    pub wet: Vec<bool>,
    pub soft: Vec<f32>,
}

/// Dig `moats` into the halo grid (square (i, j)'s middle at `hx0 + (i + 0.5) * SQUARE_FT`, likewise
/// y): heights `hh`, water `hw`, and the drawbridges' decks (`road_h` a bridge's planks, `lift` the
/// deck's height over the moat's bed). Squares on a road (a causeway) or already under water are
/// left. A wet moat's bed and banks rise `SHORE` a foot from its rim (its bed down to `MID_DEEP` in
/// the middle), its water held over its bank as far as the moat reaches.
pub(super) fn dig(moats: &[Moat], hx0: f64, hy0: f64, hh: &mut [f32], hw: &mut [f32], road_h: &mut [u8], lift: &mut [f32]) -> Dug {
    let mut dug = Dug { ditch: vec![false; HS * HS], wet: vec![false; HS * HS], soft: vec![0.0; HS * HS] };
    let span = HS as f64 * SQUARE_FT;
    for m in moats {
        let b = m.bbox;
        if b[2] < hx0 || b[0] > hx0 + span || b[3] < hy0 || b[1] > hy0 + span {
            continue;
        }
        let i0 = crate::core::floor((b[0] - hx0) / SQUARE_FT - 2.0).max(0.0) as usize;
        let j0 = crate::core::floor((b[1] - hy0) / SQUARE_FT - 2.0).max(0.0) as usize;
        let i1 = (crate::core::ceil((b[2] - hx0) / SQUARE_FT + 2.0).max(0.0) as usize).min(HS - 1);
        let j1 = (crate::core::ceil((b[3] - hy0) / SQUARE_FT + 2.0).max(0.0) as usize).min(HS - 1);
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
                if m.wet {
                    // (Ft in from its rim; out from it, less than 0.)
                    let sd = s.inner.min(s.outer) as f32;
                    if sd >= 0.0 {
                        let rim = 0.1 + SHORE * sd;
                        hh[h] = hh[h].min(level - (rim + (moat::MID_DEEP - rim).max(0.0) * smoothstep(0.5, 4.5, sd as f64) as f32));
                    } else {
                        hh[h] = level + 0.1 - SHORE * sd;
                    }
                    hw[h] = level;
                    dug.wet[h] = true;
                } else if s.inside() {
                    let g = s.outer - moat::BANK_FT;
                    if g >= 0.0 {
                        let t = s.inner.min(g);
                        let d = moat::DITCH_FT * smoothstep(0.0, 3.0, t) as f32;
                        hh[h] -= d;
                        hw[h] = hw[h].min(hh[h] - 4.0);
                        dug.ditch[h] = true;
                        dug.soft[h] = d - DITCH_SHOWN * smoothstep(0.0, 6.0, t) as f32;
                    } else {
                        hh[h] += moat::BANK_UP;
                        dug.soft[h] = BANK_SHOWN - moat::BANK_UP;
                    }
                }
                // The drawbridge's deck at the ground's height (over water, a foot above it at least).
                if m.bridge.covers(p) && s.inside() {
                    let top = if m.wet { ground.max(level + 1.0) } else { ground.max(hh[h]) };
                    lift[h] += top - hh[h];
                    hh[h] = top;
                    road_h[h] = 0x80 | 3;
                    dug.ditch[h] = false;
                    dug.soft[h] = 0.0;
                }
            }
        }
    }
    dug
}
