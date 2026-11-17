//! (R5 towns) What was said by hand about a town's buildings, kept when the ward editor lays a
//! patch out anew (`wards::change`): a building renamed or noted (`b:<layout>:<id>`) that the
//! change takes away hands its rename and note to the new building on its patch with the same
//! function (the nearest to where it stood; a castle's to the new keep), and one that finds none
//! is reported, never lost silently.

use super::catalog;
use super::geom::*;
use super::mesh::Mesh;
use super::{Building, Layout};
use crate::World;
use crate::core::hash::FastSet;

/// The renames and notes a change from `before` to `after` would carry (old id, new id, name) and
/// those it can't (old id, name). `mesh_before` and `mesh_after` are the town's patches (local ft,
/// about `center`); `reach`, how far a castle's may go to a new keep (about a patch).
pub(super) fn renames(world: &World, index: usize, center: P, before: &Layout, after: &Layout, mesh_before: &Mesh, mesh_after: &Mesh, reach: f64) -> (Vec<(u32, u32, String)>, Vec<(u32, String)>) {
    let edits = &world.file.edits;
    let key = |id: u32| format!("b:{index}:{id}");
    let marked = |id: u32| {
        let k = key(id);
        edits.renames.contains_key(&k) || edits.notes.contains_key(&k)
    };
    let (mut carried, mut lost) = (Vec::new(), Vec::new());
    if edits.renames.is_empty() && edits.notes.is_empty() {
        return (carried, lost);
    }
    let was: FastSet<u32> = before.buildings.iter().map(|b| b.id).collect();
    let now: FastSet<u32> = after.buildings.iter().map(|b| b.id).collect();
    let castle = catalog::index_of("castle").map(|c| c as u16);
    let name = |b: &Building| edits.renames.get(&key(b.id)).cloned().or_else(|| b.name.clone()).unwrap_or_else(|| b.label().to_string());
    let patch_of = |mesh: &Mesh, at: P| (0..mesh.faces.len()).find(|&p| mesh.faces[p].len() >= 3 && contains(&mesh.face_pts(p), at));
    let mut used: FastSet<u32> = FastSet::default();
    for b in before.buildings.iter().filter(|b| marked(b.id) && !now.contains(&b.id)) {
        let at = sub(centroid(&b.poly), center);
        let patch = patch_of(mesh_before, at);
        let is_castle = b.keep.is_some() || (castle.is_some() && b.func == castle);
        let best = after
            .buildings
            .iter()
            .filter(|c| !was.contains(&c.id) && !marked(c.id) && !used.contains(&c.id))
            .filter(|c| if is_castle { c.keep.is_some() || c.func == b.func } else { c.func == b.func })
            .map(|c| (c, sub(centroid(&c.poly), center)))
            .filter(|(_, ca)| patch.is_some_and(|p| contains(&mesh_after.face_pts(p), *ca)) || (is_castle && dist(*ca, at) <= reach))
            .min_by(|(a, pa), (c, pc)| (is_castle && a.keep.is_none()).cmp(&(is_castle && c.keep.is_none())).then(dist(*pa, at).total_cmp(&dist(*pc, at))).then(a.id.cmp(&c.id)));
        match best {
            Some((c, _)) => {
                used.insert(c.id);
                carried.push((b.id, c.id, name(b)));
            }
            None => lost.push((b.id, name(b))),
        }
    }
    (carried, lost)
}
