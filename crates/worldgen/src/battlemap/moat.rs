//! Castles' moats on the battlemap (v63 castles; `town::moat`): the ground dug and the water let
//! in on the height and water grid (the ground's own water drawing, deep in the middle), or a dry
//! ditch with steep sides and a low bank on its outer lip; the drawbridge a deck over it, walked
//! like a bridge's. Every square by its world position alone (seamless across chunks).

use super::{HS, SQUARE_FT};
use crate::core::noise::smoothstep;
use crate::town::moat::{self, Moat};

/// A wet moat's water's edge: its depth (water over ground, ft) falls this much a foot out from
/// its rim and rises as much in from it, the same both ways and straight over every square the
/// edge can cross (`SHORE_BAND` ft either side: a square's diagonal), so the ground's drawing,
/// which blends the squares' depths, puts its edge on the moat's own line, not on the squares.
/// Its bank (`SHORE_BAND` ft wide, level, `LIP` ft over the water) eases back to the ground over
/// `EASE` ft beyond it.
const SHORE: f32 = 1.2;
const SHORE_BAND: f32 = 7.1;
const LIP: f32 = 2.0;
const EASE: f32 = 3.0;
/// The deepest a wet moat is dug (ft under its water).
const BED: f32 = 10.0;
/// How deep (ft) a dry ditch is drawn, and how high its outer bank: its floor eased down over 6 ft
/// (the ground's drawing then shades it gently, an earthen ditch, not a rock cut); its tiers keep
/// the depth it is dug to.
const DITCH_SHOWN: f32 = 2.0;
const BANK_SHOWN: f32 = 1.0;

/// What `dig` did, by halo square: a dry ditch's floor (difficult ground), a wet moat's water and
/// the bank round it (`SHORE_BAND`) (where the ground's drawing keeps to the water's edge: `edges` bit 32), and how
/// much higher a dry ditch's ground is drawn than dug (`DITCH_SHOWN`; added once its tiers are set).
pub(super) struct Dug {
    pub ditch: Vec<bool>,
    pub wet: Vec<bool>,
    pub soft: Vec<f32>,
}

/// Dig `moats` into the halo grid (square (i, j)'s middle at `hx0 + (i + 0.5) * SQUARE_FT`, likewise
/// y): heights `hh`, water `hw`, and the drawbridges' decks (`road_h` a bridge's planks, `lift` the
/// deck's height over the moat's bed). Squares already under water are left, and causeways. A wet
/// moat's water deepens `SHORE` a foot in from its rim (to `BED`), its bank level round it (`LIP`),
/// the depth of the water held over the bank's squares falling as fast out from it (`SHORE`); a
/// road in it (but the drawbridge's) under the water. A dry ditch leaves the roads.
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
                if hw[h] > hh[h] {
                    continue;
                }
                let p = [hx0 + (i as f64 + 0.5) * SQUARE_FT, hy0 + (j as f64 + 0.5) * SQUARE_FT];
                let ground = hh[h];
                if m.wet {
                    // (Ft in from its rim; out from it, less than 0: past its bound and inside the
                    // curtain too, where only the water's level is set.)
                    let Some(s) = m.spot_by(p, (SHORE_BAND + EASE) as f64, false) else { continue };
                    let sd = s.inner.min(s.outer) as f32;
                    let kept = m.spot(p).is_some();
                    if sd >= 0.0 {
                        hh[h] = level - (SHORE * sd).min(BED);
                        hw[h] = level;
                        // (A road crossing it but the drawbridge's way: under the water.)
                        road_h[h] = 0;
                    } else if sd >= -SHORE_BAND || kept {
                        if kept && road_h[h] == 0 {
                            let t = smoothstep(SHORE_BAND as f64, (SHORE_BAND + EASE) as f64, -sd as f64) as f32;
                            hh[h] = level + LIP + (ground - level - LIP) * t;
                        }
                        if sd >= -SHORE_BAND {
                            hw[h] = hh[h] + SHORE * sd;
                        }
                    } else {
                        continue;
                    }
                    if sd >= -SHORE_BAND {
                        dug.wet[h] = true;
                    }
                    if !s.inside() || !kept {
                        continue;
                    }
                } else {
                    if road_h[h] != 0 {
                        continue;
                    }
                    let Some(s) = m.spot(p) else { continue };
                    if !s.inside() {
                        continue;
                    }
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
                if m.bridge.covers(p) {
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
