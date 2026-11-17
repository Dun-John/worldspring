//! (Roadmap 6 R5) A generated town's or village's own streets, lanes and park paths removed by
//! hand (`TownEdit::removed`), and the ids they are removed by:
//! - `tr:<layout>:<8 hex>` a street or lane: an entry of `Layout::roads` (approaches, edge
//!   strips, main and bridge streets, alleys, castle and palace lanes) or of `Layout::streets` (a
//!   village's lanes; a town's are its main streets' paths, never drawn). Keyed by which list and
//!   its two ends (to 1 ft, either way round), told apart by length and middle as stretches' `rd:`
//!   are (`lod::roads::distinct`), over the entries the town lays itself (R4's added streets keep
//!   their `ts:` and move no id). The same layout gives the same ids: unrelated edits, renames and
//!   remakes that leave the town as it was keep them; a ward edit that moves a street's ends gives
//!   it another (the removal lapses).
//! - `pp:<layout>:<spot>:<j>` a park's path: `<spot>` the park's `sq:` suffix (`p<patch>`,
//!   `g<building>`, `r<road>`; a park with none `n<k>`, its place in `Layout::parks`) and `<j>` the
//!   path's place in its battlemap layout's `paths` (`park::layout`, pure in the park).
//!
//! Removal is drawn, never laid out: the plan, the build, buildings, lots, fronts, interiors and
//! sewers never see it. Whoever draws a street, a lane, a deck or a park's gravel asks `Cut::of`
//! (None, and nothing asked, when the town has no removal that applies): the ground beneath shows,
//! a park's lawn closes over its path, a bridge deck goes with the streets over it (when no street
//! still on the map runs over it). The town's gates stay.

use super::*;
use crate::core::hash::{FastMap, FastSet};
use crate::world::TownEdit;

/// Which list a street id names an entry of (`Layout::roads`, `Layout::streets`).
pub const ROADS: u8 = b'r';
pub const STREETS: u8 = b's';

/// How near (ft, past its half width) a drawn line passes a deck's middle to run over it.
const OVER_DECK_FT: f64 = 2.0;

/// The ids (numbers) of layout `l`'s own streets and lanes: (list `ROADS`/`STREETS`, index in it,
/// number), in list order, `Layout::roads` first; R4's added streets left out (`added::ids`).
pub fn street_ids(world: &World, t0: &T0, l: &Layout) -> Vec<(u8, usize, u32)> {
    let added: FastSet<usize> = super::added::ids(world, t0, l).into_iter().map(|(k, _)| k).collect();
    let q = |p: P| (crate::core::round(p[0]) as i64, crate::core::round(p[1]) as i64);
    let mut items: Vec<(u8, usize)> = Vec::new();
    let mut keyed = Vec::new();
    let lines = l.roads.iter().enumerate().filter(|(k, _)| !added.contains(k)).map(|(k, r)| (ROADS, k, &r.0)).chain(l.streets.iter().enumerate().map(|(k, s)| (STREETS, k, s)));
    for (tag, k, pts) in lines {
        let (a, b) = (q(pts.first().copied().unwrap_or_default()), q(pts.last().copied().unwrap_or_default()));
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        let key = (crate::core::rng::hash2(crate::core::rng::hash2(0x7E_5C00 + tag as u64, a.0, a.1), b.0, b.1) & 0xffff_ffff) as u32;
        let n = pts.len();
        let m = if n == 0 { (0, 0) } else { q(pts[n / 2]).min(q(pts[(n - 1) / 2])) };
        let length: f64 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
        keyed.push((key, (crate::core::round(length) as i64, m)));
        items.push((tag, k));
    }
    items.into_iter().zip(crate::lod::roads::distinct(keyed)).map(|((tag, k), id)| (tag, k, id)).collect()
}

/// A street's or lane's id (`tr:<layout>:<8 hex>`).
#[inline]
pub fn street_id(li: usize, n: u32) -> String {
    format!("tr:{li}:{n:08x}")
}

/// A park's spot (`<spot>` of its `pp:` and `sq:` ids): `p<patch>`, `g<building>`, `r<road>`,
/// else `n<k>` (its place in `l.parks`).
pub fn park_spot(l: &Layout, k: usize) -> String {
    match l.parks.get(k).map(|p| p.spot) {
        Some(places::Spot::Patch(i)) => format!("p{i}"),
        Some(places::Spot::Garden(b)) => format!("g{b}"),
        Some(places::Spot::Road(r)) => format!("r{r}"),
        _ => format!("n{k}"),
    }
}

/// A park's path's id (`pp:<layout>:<spot>:<j>`).
#[inline]
pub fn path_id(li: usize, spot: &str, j: usize) -> String {
    format!("pp:{li}:{spot}:{j}")
}

/// A `tr:` or `pp:` id as (layout, its key in `TownEdit::removed`).
pub fn parse(id: &str) -> Option<(usize, String)> {
    let (head, rest) = id.split_once(':')?;
    let (li, local) = rest.split_once(':')?;
    let li: usize = li.parse().ok()?;
    let ok = match head {
        "tr" => local.len() == 8 && u32::from_str_radix(local, 16).is_ok() && local.bytes().all(|b| !b.is_ascii_uppercase()),
        "pp" => local.split_once(':').is_some_and(|(spot, j)| spot.len() >= 2 && j.parse::<usize>().is_ok()),
        _ => false,
    };
    ok.then(|| (li, local.to_string()))
}

/// The removals of layout `li` that apply (`TownEdit::removed`): a settlement's (the world's own
/// or one added by hand, any tier) not laid out by hand, still where its edit says it was.
pub fn removed_of<'a>(world: &'a World, t0: &T0, li: usize) -> Option<&'a std::collections::BTreeSet<String>> {
    if world.file.edits.towns.is_empty() {
        return None;
    }
    let e = world.file.edits.towns.get(&wards::key(li)).filter(|e| !e.removed.is_empty())?;
    let s = settlement(t0, li)?;
    if hand::is_hand(world, li) || dist(e.at, [s.x, s.y]) > TownEdit::SAME_AT_FT {
        return None;
    }
    Some(&e.removed)
}

/// The lines of `l` drawn as streets and lanes: (list, index, line, width ft): every one of
/// `l.roads`, and a village's lanes (`l.streets`, `village::LANE_W`).
pub fn drawn_lines(l: &Layout) -> impl Iterator<Item = (u8, usize, &[P], f64)> {
    let lanes = if l.tier == Tier::Village { l.streets.as_slice() } else { &[] };
    l.roads.iter().enumerate().map(|(k, r)| (ROADS, k, r.0.as_slice(), r.2)).chain(lanes.iter().enumerate().map(|(k, s)| (STREETS, k, s.as_slice(), village::LANE_W)))
}

/// What of a layout is removed by hand (`Cut::of`).
#[derive(Debug, Default)]
pub struct Cut {
    roads: FastSet<u32>,
    streets: FastSet<u32>,
    decks: FastSet<u32>,
    paths: FastMap<u32, FastSet<u32>>,
}

impl Cut {
    /// What of `l` is removed (`TownEdit::removed`, as it applies: `removed_of`), or None when
    /// nothing is (the town drawn as generated).
    pub fn of(world: &World, t0: &T0, l: &Layout) -> Option<Cut> {
        let li = l.index as usize;
        let removed = removed_of(world, t0, li)?;
        let mut cut = Cut::default();
        let mut parks: Option<FastMap<String, u32>> = None;
        for key in removed {
            if let Some((spot, j)) = key.split_once(':') {
                let parks = parks.get_or_insert_with(|| (0..l.parks.len()).map(|k| (park_spot(l, k), k as u32)).collect());
                if let (Some(&pk), Ok(j)) = (parks.get(spot), j.parse::<u32>()) {
                    cut.paths.entry(pk).or_default().insert(j);
                }
            }
        }
        if removed.iter().any(|k| !k.contains(':')) {
            let want: FastSet<u32> = removed.iter().filter(|k| !k.contains(':')).filter_map(|k| u32::from_str_radix(k, 16).ok()).collect();
            for (tag, k, n) in street_ids(world, t0, l) {
                if want.contains(&n) {
                    if tag == ROADS { &mut cut.roads } else { &mut cut.streets }.insert(k as u32);
                }
            }
        }
        // A deck goes with the streets over it, unless one still drawn runs over it (a metropolis's
        // lined bridge, built on, stays).
        if !cut.roads.is_empty() || !cut.streets.is_empty() {
            let lined = l.lined.as_ref().map(|(d, _)| d);
            for (k, d) in l.bridges.iter().enumerate() {
                let c = centroid(d);
                if lined.is_some_and(|deck| deck == d || contains(deck, c)) {
                    continue;
                }
                let over = |pts: &[P], w: f64| pts.windows(2).any(|s| seg_dist(c, s[0], s[1]) <= 0.5 * w + OVER_DECK_FT);
                let (mut gone, mut kept) = (false, false);
                for (tag, i, pts, w) in drawn_lines(l) {
                    if over(pts, w) {
                        if cut.line(tag, i) { gone = true } else { kept = true }
                    }
                }
                if gone && !kept {
                    cut.decks.insert(k as u32);
                }
            }
        }
        Some(cut)
    }

    /// Whether `l.roads[k]` is removed.
    #[inline]
    pub fn road(&self, k: usize) -> bool {
        self.roads.contains(&(k as u32))
    }

    /// Whether `l.streets[k]` (a village's lane) is removed.
    #[inline]
    pub fn street(&self, k: usize) -> bool {
        self.streets.contains(&(k as u32))
    }

    /// Whether the entry `k` of list `tag` (`ROADS`, `STREETS`) is removed.
    #[inline]
    pub fn line(&self, tag: u8, k: usize) -> bool {
        if tag == ROADS { self.road(k) } else { self.street(k) }
    }

    /// Whether `l.bridges[k]` goes (with the streets over it).
    #[inline]
    pub fn deck(&self, k: usize) -> bool {
        self.decks.contains(&(k as u32))
    }

    /// The paths of park `pk` (`l.parks[pk]`) removed: places in its layout's `paths`.
    #[inline]
    pub fn paths(&self, pk: usize) -> Option<&FastSet<u32>> {
        self.paths.get(&(pk as u32))
    }
}
