//! Roadmap 7 item 170: cliffs on battlemaps. A square whose ground is steeper than `CLIFF_SLOPE`
//! (a landform's riser, a canyon's wall, any sheer drop) is a cliff face: hard going (`edges` bit
//! 16); the ground's drawing paints it as rock with ledges, an inked top and a shadow at its foot
//! (`ground.frag`). Below a face, up to `SCREE_SQ` squares out, its scree: hard going too, and
//! `edges` bit 64, where the drawing scatters loose stones. Judged from the terrain tile's own
//! samples (pure per global square, two squares past the chunk in its halo), so chunks agree;
//! ground the chunk made (town ground, mounds, pads) is neither face nor scree.

use super::*;

/// The grade (ft per ft, about 50 degrees) from which ground is a cliff face.
pub const CLIFF_SLOPE: f64 = 1.2;
/// How far (squares) below a face its scree reaches.
const SCREE_SQ: i64 = 2;
/// How much higher (ft) a face's square must stand than a square below it for scree there.
const SCREE_RISE: f32 = 6.0;
/// How far (squares) an object on a face looks for ground off it.
const FACE_REACH: i64 = 8;
/// The grade the chunk's own heights must still have on a face (made ground is flatter).
const OWN_SLOPE: f64 = 0.6;
/// `marks`' bit for a square drawn as a face that is not one (masked off `edges`).
pub(super) const DRAWN: u8 = 128;

/// The chunk's cliff faces and scree per square (`edges` bits 16, and 16 | 64) from the tile's
/// samples (`sidx`: a square's centre sample, squares -2..=SQ + 1). Only open, dry, unbuilt ground.
/// Also `DRAWN` (not an `edges` bit): open ground the drawing paints as a face, steep by the chunk's
/// own heights, on natural slopes too steep to have been made, though its samples' finer grade
/// is below a face's (a riser's shoulder): kept clear like a face, not hard going.
pub(super) fn marks(c: &Chunk, padded: &[f32], sidx: &dyn Fn(i64, i64) -> usize) -> Vec<u8> {
    let mut out = vec![0u8; SQ * SQ];
    let m = SQ as i64 + 2 * SCREE_SQ;
    let at = |i: i64, j: i64| (j + SCREE_SQ) as usize * m as usize + (i + SCREE_SQ) as usize;
    let mut face = vec![false; (m * m) as usize];
    let mut h = vec![0f32; (m * m) as usize];
    let mut made_sq = vec![false; (m * m) as usize];
    let mut drawn = vec![false; (m * m) as usize];
    for j in -SCREE_SQ..SQ as i64 + SCREE_SQ {
        for i in -SCREE_SQ..SQ as i64 + SCREE_SQ {
            let s = sidx(i, j);
            let (gx, gy) = ((padded[s + 1] - padded[s - 1]) as f64 / 5.0, (padded[s + PADDED] - padded[s - PADDED]) as f64 / 5.0);
            // (Ground the chunk made itself, a quay, a castle's mound, a building's pad, is no face
            // and gets no scree: the chunk's own heights, its halo included, must be steep too.)
            let mut own = 0.0;
            let made = (0..SQ as i64).contains(&i) && (0..SQ as i64).contains(&j) && {
                let hh = |di: i64, dj: i64| {
                    let (a, b) = (i + di, j + dj);
                    (if (0..SQ as i64).contains(&a) && (0..SQ as i64).contains(&b) { c.height[b as usize * SQ + a as usize] } else { c.halo_height[(b + 1) as usize * HS + (a + 1) as usize] }) as f64
                };
                let (cx, cy) = ((hh(1, 0) - hh(-1, 0)) / (2.0 * SQUARE_FT), (hh(0, 1) - hh(0, -1)) / (2.0 * SQUARE_FT));
                own = cx * cx + cy * cy;
                own < OWN_SLOPE * OWN_SLOPE
            };
            drawn[at(i, j)] = own >= CLIFF_SLOPE * CLIFF_SLOPE && gx * gx + gy * gy >= OWN_SLOPE * OWN_SLOPE;
            face[at(i, j)] = !made && gx * gx + gy * gy >= CLIFF_SLOPE * CLIFF_SLOPE;
            made_sq[at(i, j)] = made && gx * gx + gy * gy >= CLIFF_SLOPE * CLIFF_SLOPE;
            h[at(i, j)] = padded[s];
        }
    }
    // (No face anywhere, as on most ground: no square looks round itself for one.)
    let any_face = face.contains(&true);
    for j in 0..SQ as i64 {
        for i in 0..SQ as i64 {
            let k = j as usize * SQ + i as usize;
            let open = !c.surface[k].is_road() && c.building[k] == 0 && c.urban[k] == 0 && c.water_level[k] <= c.height[k];
            if !open || made_sq[at(i, j)] {
                continue;
            }
            let own = at(i, j);
            if face[own] {
                out[k] = 16;
                continue;
            }
            let scree = any_face && (-SCREE_SQ..=SCREE_SQ).any(|dy| (-SCREE_SQ..=SCREE_SQ).any(|dx| face[at(i + dx, j + dy)] && h[at(i + dx, j + dy)] >= h[own] + SCREE_RISE));
            if scree {
                out[k] = 16 | 64;
            } else if drawn[own] {
                out[k] = DRAWN;
            }
        }
    }
    out
}

/// Whether a square of `marks` is kept clear: a face, or drawn as one.
pub(super) fn keep_clear(m: u8) -> bool {
    m == 16 || m == DRAWN
}

/// Nothing generated stands on a face (or what is drawn as one): each object there moves to the nearest square off it (open,
/// dry ground, its foot or its top), keeping its cover; with none within reach it goes. (Run
/// before the guarantee, which keeps off faces and fills any gap this leaves, mid-face on a broad
/// one with a boulder lodged there, its `lodge` squares.)
pub(super) fn clear_faces(c: &mut Chunk, marks: &[u8]) {
    let face = |k: usize| keep_clear(marks[k]);
    let mut gone = vec![false; c.objects.len()];
    for n in 0..c.objects.len() {
        let o = c.objects[n];
        let (x, y) = ((o.x.max(0.0) as usize).min(SQ - 1), (o.y.max(0.0) as usize).min(SQ - 1));
        if !face(y * SQ + x) {
            continue;
        }
        let spot = (1..=FACE_REACH).find_map(|r| {
            (-r..=r).flat_map(move |dy| (-r..=r).map(move |dx| (dx, dy))).filter(|&(dx, dy)| dx.abs() == r || dy.abs() == r).find_map(|(dx, dy)| {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= SQ as i64 || ny >= SQ as i64 {
                    return None;
                }
                let nk = ny as usize * SQ + nx as usize;
                let open = !face(nk) && !c.surface[nk].is_road() && c.building[nk] == 0 && c.urban[nk] == 0 && c.water_level[nk] <= c.height[nk];
                let moved = Object { x: o.x + dx as f32, y: o.y + dy as f32, ..o };
                (open && clear_of_buildings(c, &moved)).then_some(moved)
            })
        });
        match spot {
            Some(m) => c.objects[n] = m,
            None => gone[n] = true,
        }
    }
    let mut it = gone.iter();
    c.objects.retain(|_| !*it.next().unwrap());
}
