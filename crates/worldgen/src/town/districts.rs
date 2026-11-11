//! (R5 towns) Districts drawn by hand (`PatchEdit::district`): a patch put in another district,
//! or in one made by hand and named (`TownEdit::districts`). The generated districts keep their
//! indices (`d:<layout>:<q>`), an emptied one too (no patches, no label); those made by hand
//! follow, in the order they were made. A district whose patches changed (one in or out, or a
//! ward set by hand) takes its kind from its wards by area and its label at its largest connected
//! piece; the rest stay as planned.

use super::catalog::Ward;
use super::geom::*;
use super::wards::Changes;
use super::{Quarter, QuarterKind, TownPlan};
use crate::core::hash::FastMap;

/// The district a patch is put in by hand.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Target {
    /// A generated district (its index).
    Quarter(usize),
    /// One made by hand, by name.
    Named(String),
}

/// A patch's district as stored (`PatchEdit::district`): a generated district's id or a name.
pub(super) fn parse(plan: &TownPlan, s: &str) -> Result<Target, String> {
    if let Some(rest) = s.strip_prefix("d:") {
        let q = rest.rsplit(':').next().and_then(|q| q.parse::<usize>().ok()).ok_or_else(|| format!("{s}: a district's id is d:<layout>:<n>"))?;
        if q >= plan.quarters.len() {
            return Err(format!("no such district: {s}"));
        }
        return Ok(Target::Quarter(q));
    }
    let n = s.trim();
    if n.is_empty() { Err("a district needs an id or a name".into()) } else { Ok(Target::Named(n.to_string())) }
}

/// The kind a patch gives its district (as `quarters` gives a seed's).
fn ward_kind(plan: &TownPlan, ward: &[Option<Ward>], p: usize) -> QuarterKind {
    match ward[p] {
        Some(Ward::Castle) => QuarterKind::Castle,
        Some(Ward::Park) => QuarterKind::Park,
        Some(Ward::Docks) => QuarterKind::Docks,
        _ if p < plan.n_inner => QuarterKind::Regular,
        _ => QuarterKind::Sprawl,
    }
}

/// A changed district's kind: what most of its area gives (the centre stays the centre while its
/// patches are mostly homes and trades).
fn kind_by_area(plan: &TownPlan, ward: &[Option<Ward>], members: &[usize], was: Option<QuarterKind>) -> QuarterKind {
    const KINDS: [QuarterKind; 5] = [QuarterKind::Castle, QuarterKind::Park, QuarterKind::Docks, QuarterKind::Regular, QuarterKind::Sprawl];
    let mut by = [0.0f64; 5];
    for &p in members {
        let k = ward_kind(plan, ward, p);
        by[KINDS.iter().position(|x| *x == k).unwrap_or(3)] += area(&plan.mesh.face_pts(p)).abs().max(1.0);
    }
    let mut best = 3;
    for k in 0..5 {
        if by[k] > by[best] {
            best = k;
        }
    }
    let k = KINDS[best];
    match was {
        Some(QuarterKind::Center) if matches!(k, QuarterKind::Regular | QuarterKind::Sprawl) => QuarterKind::Center,
        _ => k,
    }
}

/// The town's districts with the patches put in others by hand: each with its patches and
/// whether it changed (a patch in or out, or a ward set by hand). Patches and labels are left
/// empty (`build_town` lays them).
pub(super) fn of(plan: &TownPlan, ch: &Changes) -> Vec<(Quarter, Vec<usize>, bool)> {
    let mut out: Vec<(Quarter, Vec<usize>, bool)> = plan.quarters.iter().map(|(q, m)| (Quarter { name: q.name.clone(), kind: q.kind, patches: Vec::new(), label: Vec::new() }, m.clone(), false)).collect();
    if ch.district.is_empty() && ch.ward.is_empty() && ch.merge.is_empty() {
        return out;
    }
    let n = out.len();
    for name in &ch.districts {
        out.push((Quarter { name: name.clone(), kind: QuarterKind::Regular, patches: Vec::new(), label: Vec::new() }, Vec::new(), true));
    }
    for (&p, t) in &ch.district {
        let to = match t {
            Target::Quarter(q) => *q,
            Target::Named(name) => n + ch.districts.iter().position(|x| x == name).unwrap_or(0),
        };
        if to >= out.len() || out[to].1.contains(&p) {
            continue;
        }
        for (_, m, touched) in out.iter_mut() {
            if let Some(i) = m.iter().position(|&x| x == p) {
                m.remove(i);
                *touched = true;
            }
        }
        out[to].1.push(p);
        out[to].2 = true;
    }
    let (ward, _) = ch.wards(plan);
    for (qi, (q, m, touched)) in out.iter_mut().enumerate() {
        *touched |= m.iter().any(|&p| ward[p] != plan.ward[p]);
        if *touched && !m.is_empty() {
            q.kind = kind_by_area(plan, &ward, m, (qi < n).then(|| plan.quarters[qi].0.kind));
        }
    }
    out
}

/// Each patch's district (`of`).
pub(super) fn membership(qs: &[(Quarter, Vec<usize>, bool)]) -> FastMap<usize, usize> {
    let mut m = FastMap::default();
    for (qi, (_, members, _)) in qs.iter().enumerate() {
        for &p in members {
            m.insert(p, qi);
        }
    }
    m
}

/// The patches a changed district's label goes over: its largest connected piece (by area), in
/// the district's order; all of them when they hang together.
pub(super) fn label_piece(plan: &TownPlan, cells: &[Labeled], members: &[usize]) -> Vec<usize> {
    let mut piece = vec![usize::MAX; members.len()];
    let mut pieces: Vec<f64> = Vec::new();
    for s in 0..members.len() {
        if piece[s] != usize::MAX {
            continue;
        }
        let id = pieces.len();
        let (mut stack, mut a) = (vec![s], 0.0);
        piece[s] = id;
        while let Some(i) = stack.pop() {
            a += area(&cells[members[i]].pts).abs();
            for &nb in &plan.mesh.labels[members[i]] {
                if let Some(j) = members.iter().position(|&x| x as i32 == nb)
                    && piece[j] == usize::MAX
                {
                    piece[j] = id;
                    stack.push(j);
                }
            }
        }
        pieces.push(a);
    }
    if pieces.len() <= 1 {
        return members.to_vec();
    }
    let mut best = 0;
    for k in 1..pieces.len() {
        if pieces[k] > pieces[best] {
            best = k;
        }
    }
    members.iter().zip(&piece).filter(|(_, k)| **k == best).map(|(&p, _)| p).collect()
}

/// A district's name as it is now (renamed by hand, or as made).
pub(super) fn name_now(world: &crate::World, index: usize, qi: usize, made: &str) -> String {
    world.file.edits.renames.get(&format!("d:{index}:{qi}")).cloned().unwrap_or_else(|| made.to_string())
}

/// What a change asks of patch `p`'s district (`asked`: a district's id, a name, or "auto"), as
/// the edit stores it (`None`: as generated); a new name makes a district (`e.districts`). A name
/// that is a district's (as it is now, any case) is that district.
pub(super) fn resolve(world: &crate::World, plan: &TownPlan, index: usize, e: &mut crate::world::TownEdit, p: usize, asked: &str) -> Result<Option<String>, String> {
    let n = plan.quarters.len();
    let own = |q: usize| plan.quarters[q].1.contains(&p);
    let generated = |q: usize| if own(q) { None } else { Some(format!("d:{index}:{q}")) };
    let asked = asked.trim();
    if asked == "auto" {
        return Ok(None);
    }
    if let Some(rest) = asked.strip_prefix("d:") {
        let mut it = rest.split(':').map(|x| x.parse::<usize>().ok());
        let (Some(Some(li)), Some(Some(q)), None) = (it.next(), it.next(), it.next()) else { return Err(format!("{asked}: a district's id is d:<layout>:<n>")) };
        if li != index {
            return Err(format!("{asked} is another town's district"));
        }
        return match q {
            q if q < n => Ok(generated(q)),
            q if q - n < e.districts.len() => Ok(Some(e.districts[q - n].clone())),
            _ => Err(format!("no such district: {asked}")),
        };
    }
    if asked.is_empty() || asked.chars().count() > 80 {
        return Err("a district's name is 1 to 80 characters".into());
    }
    let same = |a: &str| a.trim().eq_ignore_ascii_case(asked);
    if let Some(q) = (0..n).find(|&q| same(&name_now(world, index, q, &plan.quarters[q].0.name))) {
        return Ok(generated(q));
    }
    if let Some(k) = (0..e.districts.len()).find(|&k| same(&name_now(world, index, n + k, &e.districts[k]))) {
        return Ok(Some(e.districts[k].clone()));
    }
    e.districts.push(asked.to_string());
    Ok(Some(asked.to_string()))
}
