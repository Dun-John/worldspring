//! Roadmap 7 item 170: cliffs on battlemaps. A square whose ground is steeper than `CLIFF_SLOPE`
//! (a landform's riser, a canyon's wall, any sheer drop) is a cliff face: hard going (`edges` bit
//! 16); the ground's drawing paints it as rock with ledges, an inked top and a shadow at its foot
//! (`ground.frag`). Below a face, up to `SCREE_SQ` squares out, its scree: hard going too, and
//! `edges` bit 64, where the drawing scatters loose stones. Judged from the terrain tile's own
//! samples (pure per global square, two squares past the chunk in its halo), so chunks agree.

use super::*;

/// The grade (ft per ft, about 50 degrees) from which ground is a cliff face.
pub const CLIFF_SLOPE: f64 = 1.2;
/// How far (squares) below a face its scree reaches.
const SCREE_SQ: i64 = 2;
/// How much higher (ft) a face's square must stand than a square below it for scree there.
const SCREE_RISE: f32 = 6.0;

/// Marks the chunk's cliff faces and scree (`edges` bits 16 and 64) from the tile's samples
/// (`sidx`: a square's centre sample, squares -2..=SQ + 1). Only open, dry, unbuilt ground.
pub(super) fn mark(c: &mut Chunk, padded: &[f32], sidx: &dyn Fn(i64, i64) -> usize) {
    let m = SQ as i64 + 2 * SCREE_SQ;
    let at = |i: i64, j: i64| (j + SCREE_SQ) as usize * m as usize + (i + SCREE_SQ) as usize;
    let mut face = vec![false; (m * m) as usize];
    let mut h = vec![0f32; (m * m) as usize];
    for j in -SCREE_SQ..SQ as i64 + SCREE_SQ {
        for i in -SCREE_SQ..SQ as i64 + SCREE_SQ {
            let s = sidx(i, j);
            let (gx, gy) = ((padded[s + 1] - padded[s - 1]) as f64 / 5.0, (padded[s + PADDED] - padded[s - PADDED]) as f64 / 5.0);
            face[at(i, j)] = gx * gx + gy * gy >= CLIFF_SLOPE * CLIFF_SLOPE;
            h[at(i, j)] = padded[s];
        }
    }
    for j in 0..SQ as i64 {
        for i in 0..SQ as i64 {
            let k = j as usize * SQ + i as usize;
            let open = !c.surface[k].is_road() && c.building[k] == 0 && c.urban[k] == 0 && c.water_level[k] <= c.height[k];
            if !open {
                continue;
            }
            let own = at(i, j);
            if face[own] {
                c.edges[k] |= 16;
                continue;
            }
            let scree = (-SCREE_SQ..=SCREE_SQ).any(|dy| (-SCREE_SQ..=SCREE_SQ).any(|dx| face[at(i + dx, j + dy)] && h[at(i + dx, j + dy)] >= h[own] + SCREE_RISE));
            if scree {
                c.edges[k] |= 16 | 64;
            }
        }
    }
}
