//! The tools every planner shares: neighbours, corridors dug through rock (`dig`), locks, keys and loops (`guard`), the
//! props (`PROPS`, `item`), and what goes in a room (`kit`, `furnish`, `furnish_room`, `boss_chamber`).

use crate::core::rng::Pcg32;
use crate::interior::Item;
use crate::under::*;

pub(crate) fn neighbours(k: usize, nx: usize, ny: usize) -> impl Iterator<Item = usize> {
    let (i, j) = (k % nx, k / nx);
    [(i > 0).then(|| k - 1), (i + 1 < nx).then(|| k + 1), (j > 0).then(|| k - nx), (j + 1 < ny).then(|| k + nx)].into_iter().flatten()
}

/// Dig a corridor from room `a` to room `b`: least cost through rock (reusing corridors is
/// cheaper, turning and running beside other rooms dearer), never through another room;
/// `wide`: two squares wide where that touches no other room.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dig(p: &mut Plan, nx: usize, ny: usize, a: i16, b: i16, passage: i16, wide: bool) -> Vec<(usize, usize)> {
    let n = nx * ny;
    // State: square × heading (0..4).
    let mut cost = vec![u32::MAX; n * 4];
    let mut came = vec![u32::MAX; n * 4];
    let mut heap = std::collections::BinaryHeap::new();
    for k in 0..n {
        if p.cells[k] == a {
            for h in 0..4 {
                cost[k * 4 + h] = 0;
                heap.push(std::cmp::Reverse((0u32, (k * 4 + h) as u32)));
            }
        }
    }
    let other_room = |c: i16| c >= 0 && c != a && c != b && c != passage;
    let near_other = |k: usize| neighbours(k, nx, ny).any(|m| other_room(p.cells[m]));
    let mut goal = None;
    while let Some(std::cmp::Reverse((c, s))) = heap.pop() {
        let s = s as usize;
        if c > cost[s] {
            continue;
        }
        let (k, h) = (s / 4, s % 4);
        if p.cells[k] == b {
            goal = Some(s);
            break;
        }
        let (i, j) = (k % nx, k / nx);
        for (nh, (di, dj)) in [(1isize, 0isize), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
            let (ni, nj) = (i as isize + di, j as isize + dj);
            // Keep a ring of rock round the edge.
            if ni < 1 || nj < 1 || ni >= nx as isize - 1 || nj >= ny as isize - 1 {
                continue;
            }
            let m = nj as usize * nx + ni as usize;
            let cm = p.cells[m];
            if other_room(cm) {
                continue;
            }
            let step = if cm == b || cm == a {
                1
            } else if cm == passage {
                2
            } else {
                4 + if near_other(m) { 6 } else { 0 }
            } + if nh != h && p.cells[k] != a { 3 } else { 0 };
            let nc = c + step;
            let ns = m * 4 + nh;
            if nc < cost[ns] {
                cost[ns] = nc;
                came[ns] = s as u32;
                heap.push(std::cmp::Reverse((nc, ns as u32)));
            }
        }
    }
    let Some(mut s) = goal else { return Vec::new() };
    let mut path = vec![s / 4];
    while came[s] != u32::MAX {
        s = came[s] as usize;
        path.push(s / 4);
    }
    path.reverse();
    let mut made = Vec::new();
    for w in path.windows(2) {
        let (k0, k1) = (w[0], w[1]);
        if p.cells[k1] < 0 {
            p.cells[k1] = passage;
        }
        if p.cells[k0] != p.cells[k1] {
            p.door(k0, k1);
            made.push((k0.min(k1), k0.max(k1)));
        }
    }
    if wide {
        // The second lane: beside each step (below a run along x, right of one along y).
        for w in path.windows(2) {
            let (k0, k1) = (w[0], w[1]);
            if p.cells[k1] != passage {
                continue;
            }
            let m = if k1.abs_diff(k0) == 1 { k1 + nx } else { k1 + 1 };
            let (i, j) = (m % nx, m / nx);
            let other = |c: i16| c >= 0 && c != passage;
            if m < n && i >= 1 && j >= 1 && i + 1 < nx && j + 1 < ny && p.cells[m] < 0 && !neighbours(m, nx, ny).any(|q| other(p.cells[q])) {
                p.cells[m] = passage;
            }
        }
    }
    made
}


impl Item {
    #[allow(clippy::too_many_arguments)]
    pub fn new(kind: &'static str, name: &'static str, x: usize, y: usize, w: usize, h: usize, cover: u8, blocks_move: bool, height_ft: f32, hazard: Option<&'static str>) -> Item {
        Item { kind, name, x: x as u16, y: y as u16, w: w as u16, h: h as u16, cover, blocks_move, height_ft, hazard, sprite: 0, back: None, variant: None }
    }
}

/// A square of a lava tube's lava channel.
pub(crate) fn lava(x: usize, y: usize) -> Item {
    Item::new("lava", "lava channel", x, y, 1, 1, 0, false, 0.0, Some("10d10 fire on entering or starting a turn in it"))
}

/// Props underground: (kind, name, cover, blocks movement, height ft, hazard).
pub const PROPS: &[(&str, &str, u8, bool, f32, Option<&str>)] = &[
    ("pillar", "pillar", 3, true, 15.0, None),
    ("sarcophagus", "sarcophagus", 2, true, 3.5, None),
    ("urn", "burial urn", 1, true, 3.0, None),
    ("brazier", "brazier", 1, true, 4.0, Some("knocked over: 1d6 fire to adjacent creatures")),
    ("bones", "scattered bones", 0, false, 0.5, None),
    ("altar", "altar", 2, true, 3.5, None),
    ("chest", "chest", 1, true, 2.5, None),
    ("hoard", "treasure hoard", 1, false, 2.0, None),
    ("dais", "dais and throne", 1, false, 2.5, None),
    ("table", "table", 1, true, 3.0, None),
    ("cot", "cot", 0, false, 1.5, None),
    ("crate", "crates", 1, true, 4.0, None),
    ("barrel", "barrels", 1, true, 4.0, None),
    ("weapon_rack", "weapon rack", 1, true, 6.0, None),
    ("cage", "iron cage", 1, true, 7.0, None),
    ("rack", "torture rack", 1, true, 3.0, None),
    ("well", "well", 1, true, 3.0, Some("40 ft shaft to water")),
    ("trap", "pressure plate", 0, false, 0.0, Some("hidden (DC 14 Perception); 2d10 piercing darts (DC 13 Dex half)")),
    ("pit", "pit", 0, false, 0.0, Some("10 ft deep: 1d6 bludgeoning (DC 12 Dex to catch the edge)")),
    ("rubble", "rubble", 1, false, 2.0, Some("difficult terrain")),
    ("stalagmite", "stalagmite", 2, true, 6.0, None),
    ("crystal", "crystal cluster", 1, true, 4.0, None),
    ("pool", "pool", 0, false, 0.0, Some("difficult terrain; 3 ft deep")),
    ("fungus", "glowing fungus", 0, false, 1.0, Some("dim light, 10 ft")),
    ("timber", "timber prop", 1, true, 8.0, None),
    ("rail", "rails", 0, false, 0.0, None),
    ("ore_cart", "ore cart", 1, true, 4.0, None),
    ("cave_in", "unstable ceiling", 0, false, 0.0, Some("loud noise: DC 12 Dex save or 2d6 bludgeoning, then difficult terrain")),
    ("basalt", "basalt column", 3, true, 10.0, None),
    ("obsidian", "obsidian shards", 0, false, 1.0, Some("difficult terrain; 1d4 slashing on falling prone")),
    ("vent", "steam vent", 0, false, 0.0, Some("erupts on a d6 roll of 6: 2d6 fire in 10 ft (DC 13 Dex half)")),
    ("niche", "burial niche", 0, false, 1.0, None),
    ("boulder", "boulder", 3, true, 6.0, None),
    ("rock_column", "rock column", 3, true, 20.0, None),
    ("mushroom", "giant mushrooms", 1, true, 6.0, Some("struck: spores, DC 12 Con save or poisoned for 1 minute")),
    ("web", "thick webs", 0, false, 0.0, Some("difficult terrain; DC 12 Dex save or restrained (DC 12 Str to break free)")),
    ("guano", "bat guano", 0, false, 0.0, Some("difficult terrain; open wounds: DC 11 Con save or sewer plague")),
    ("skeleton", "adventurer's remains", 0, false, 0.5, None),
    ("campfire", "cold campfire", 0, false, 0.5, None),
    ("moss", "damp moss", 0, false, 0.0, Some("slippery: DC 10 Dex save when dashing or fall prone")),
    ("ore_vein", "ore vein", 0, false, 0.0, None),
    ("tools", "mining tools", 0, false, 1.0, None),
    ("lantern", "hanging lantern", 0, false, 0.0, Some("bright light 15 ft, dim 15 ft beyond")),
    ("winch", "hoist winch", 2, true, 5.0, None),
    ("powder", "blasting powder kegs", 1, true, 3.0, Some("fire or lightning: explodes, 3d6 fire in 10 ft (DC 13 Dex half)")),
    ("bedroll", "bedroll", 0, false, 0.5, None),
    ("chains", "wall shackles", 0, false, 0.0, None),
    ("sconce", "torch sconce", 0, false, 0.0, Some("bright light 20 ft, dim 20 ft beyond")),
    ("statue", "statue", 3, true, 8.0, None),
    ("fountain", "dry fountain", 2, true, 3.0, None),
    ("iron_maiden", "iron maiden", 2, true, 7.0, None),
    ("stocks", "stocks", 1, true, 3.0, None),
    ("sacks", "grain sacks", 1, true, 3.0, None),
    ("bookshelf", "bookshelf", 3, true, 7.0, None),
    ("banner", "tattered banner", 0, false, 0.0, None),
    ("rug", "faded rug", 0, false, 0.0, None),
    ("coffin", "open coffin", 1, true, 2.0, None),
    ("skulls", "skull pile", 1, true, 2.5, None),
    ("candles", "candelabrum", 0, true, 5.0, Some("dim light 10 ft")),
    ("effigy", "stone effigy", 3, true, 6.0, None),
    ("cobweb", "cobwebs", 0, false, 0.0, None),
    ("offering", "offering bowl", 0, false, 0.5, None),
    ("debris", "flotsam heap", 1, false, 2.0, Some("difficult terrain")),
    ("pipe", "drain outlet", 0, false, 0.0, None),
    ("rat_nest", "rat nest", 0, false, 1.0, Some("a swarm of rats lairs here")),
    ("ladder", "ladder to a street grate", 0, false, 0.0, None),
    ("sulfur", "sulfur crust", 0, false, 0.0, Some("disturbed: DC 12 Con save or poisoned for 1 minute")),
    ("scorched", "scorched remains", 0, false, 0.5, None),
    ("glass_pool", "cooled glassy pool", 0, false, 0.0, Some("slippery: difficult terrain")),
    ("glyph", "ritual circle", 0, false, 0.0, Some("arcane: its purpose is the DM's")),
    ("nest", "beast's nest", 0, false, 1.5, None),
    ("ice", "ice formation", 2, true, 6.0, None),
    ("ice_sheet", "sheet ice", 0, false, 0.0, Some("slippery: DC 10 Dex save on dashing or fall prone")),
    ("forge", "forge", 2, true, 4.0, Some("hot coals: 1d6 fire on falling against it")),
    ("anvil", "anvil", 1, true, 3.0, None),
    ("long_table", "long table", 1, true, 3.0, None),
    ("desk", "desk", 1, true, 3.0, None),
    ("hearth", "hearth", 1, true, 3.0, None),
    ("keg_rack", "keg rack", 1, true, 5.0, None),
    ("vat", "vat", 1, true, 5.0, None),
    ("bed", "bed", 0, false, 2.0, None),
    // (DM-only: opens the locked door of its colour, `variant` 1-7; `Door::lock`.)
    ("key", "key", 0, false, 0.0, None),
    // (Tumbled blocks from a broken wall, spilling into the room from the wall behind it; put down by hand.)
    ("collapsed_wall", "collapsed wall", 1, false, 3.0, Some("difficult terrain")),
    // Terrain (`TERRAIN`): lies on the floor, its full w × h; spans (`SPANS`) lie over it. Deep water and chasms block
    // movement, not sight (height 0), unless a span covers them.
    ("water_shallow", "shallow water", 0, false, 0.0, Some("difficult terrain; 2 ft deep")),
    ("water_deep", "deep water", 0, true, 0.0, Some("deep: swim to cross")),
    ("chasm", "chasm", 0, true, 0.0, Some("a sheer drop: 6d6 bludgeoning on falling in")),
    ("rubble_slope", "rubble slope", 1, false, 2.0, Some("difficult terrain")),
    ("rope_bridge", "rope bridge", 0, false, 0.0, None),
    ("stepping_stone", "stepping stones", 0, false, 0.0, Some("difficult terrain")),
];

/// Items that lie on the floor under a span: water, chasms, rubble slopes.
pub const TERRAIN: [&str; 4] = ["water_shallow", "water_deep", "chasm", "rubble_slope"];
/// Items one walks on over deep water or a chasm.
pub const SPANS: [&str; 2] = ["rope_bridge", "stepping_stone"];

/// The squares `items` block (`nx` × `ny`): whatever blocks movement, but deep water or a chasm with a span over it.
pub fn blocked_squares(items: &[Item], nx: usize, ny: usize) -> Vec<bool> {
    let squares = |f: &Item| {
        let (x0, x1, y1) = (f.x as usize, ((f.x + f.w) as usize).min(nx), ((f.y + f.h) as usize).min(ny));
        (f.y as usize..y1).flat_map(move |j| (x0..x1).map(move |i| j * nx + i))
    };
    let mut b = vec![false; nx * ny];
    let mut gap = vec![false; nx * ny];
    for f in items.iter().filter(|f| f.blocks_move) {
        let terrain = TERRAIN.contains(&f.kind);
        for k in squares(f) {
            if terrain { gap[k] = true } else { b[k] = true }
        }
    }
    for f in items.iter().filter(|f| SPANS.contains(&f.kind)) {
        for k in squares(f) {
            gap[k] = false;
        }
    }
    for (b, g) in b.iter_mut().zip(gap) {
        *b |= g;
    }
    b
}

/// An item of `kind` at (x, y), w × h, with its tactical values (`PROPS`; anything else is clutter).
pub(crate) fn item(kind: &'static str, x: usize, y: usize, w: usize, h: usize) -> Item {
    let (name, cover, blocks, height, hazard) = PROPS.iter().find(|p| p.0 == kind).map(|p| (p.1, p.2, p.3, p.4, p.5)).unwrap_or(("clutter", 0, false, 1.0, None));
    Item::new(kind, name, x, y, w, h, cover, blocks, height, hazard)
}

#[derive(Clone, Copy)]
pub(crate) enum Spot {
    /// Against the room's edge.
    Edge,
    /// Anywhere in the room.
    Any,
}

/// What goes in a room: (item, w, h, where, how many: min, max).
pub(crate) fn kit(kind: &str) -> &'static [(&'static str, usize, usize, Spot, u32, u32)] {
    use Spot::*;
    match kind {
        // Dungeons.
        "guard room" => &[("table", 1, 2, Any, 1, 1), ("weapon_rack", 1, 1, Edge, 1, 2), ("chest", 1, 1, Edge, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("sacks", 1, 1, Edge, 0, 2)],
        "barracks" => &[("cot", 1, 2, Edge, 3, 6), ("chest", 1, 1, Edge, 1, 2), ("bedroll", 1, 2, Any, 0, 2), ("sconce", 1, 1, Edge, 0, 1)],
        "storeroom" => &[("crate", 1, 1, Edge, 2, 5), ("barrel", 1, 1, Edge, 1, 4), ("sacks", 1, 1, Edge, 1, 3)],
        "powder store" => &[("powder", 1, 1, Edge, 2, 4), ("crate", 1, 1, Edge, 1, 3), ("lantern", 1, 1, Edge, 0, 1)],
        "cell block" | "prison" => &[("cage", 2, 2, Edge, 1, 3), ("chains", 1, 1, Edge, 2, 4), ("bones", 1, 1, Any, 0, 2), ("stocks", 2, 1, Any, 0, 1)],
        "torture chamber" => &[("rack", 1, 2, Any, 1, 1), ("iron_maiden", 1, 1, Edge, 0, 1), ("chains", 1, 1, Edge, 1, 3), ("brazier", 1, 1, Edge, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "shrine" => &[("altar", 2, 1, Edge, 1, 1), ("statue", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("brazier", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1)],
        "chapel of rest" => &[("altar", 2, 1, Edge, 1, 1), ("candles", 1, 1, Any, 1, 3), ("offering", 1, 1, Edge, 1, 2), ("urn", 1, 1, Edge, 0, 2)],
        "armory" => &[("weapon_rack", 1, 1, Edge, 2, 5), ("crate", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 0, 1)],
        "well room" => &[("well", 1, 1, Any, 1, 1), ("barrel", 1, 1, Edge, 0, 2), ("moss", 1, 1, Any, 1, 3)],
        "pit room" => &[("pit", 2, 2, Any, 1, 1), ("bones", 1, 1, Any, 1, 2), ("chains", 1, 1, Edge, 0, 2)],
        "entry hall" => &[("statue", 1, 1, Edge, 1, 2), ("banner", 1, 1, Edge, 1, 2), ("sconce", 1, 1, Edge, 1, 2), ("fountain", 2, 2, Any, 0, 1)],
        "antechamber" => &[("effigy", 1, 2, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("cobweb", 1, 1, Edge, 1, 2)],
        "landing" => &[("sconce", 1, 1, Edge, 1, 2), ("pillar", 1, 1, Any, 0, 2), ("bookshelf", 1, 2, Edge, 0, 1)],
        "oubliette" => &[("pit", 2, 2, Any, 1, 1), ("chains", 1, 1, Edge, 1, 3), ("bones", 1, 1, Any, 1, 2)],
        "crypt of the old lords" => &[("sarcophagus", 1, 2, Any, 2, 4), ("effigy", 1, 2, Edge, 0, 2), ("candles", 1, 1, Any, 1, 2), ("cobweb", 1, 1, Edge, 1, 3)],
        "secret vault" => &[("chest", 1, 1, Edge, 2, 4), ("hoard", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 1, 2)],
        "corridor" => &[("trap", 1, 1, Any, 0, 2), ("sconce", 1, 1, Edge, 1, 4), ("chains", 1, 1, Edge, 0, 1)],
        // Crypts and catacombs.
        "burial hall" => &[("sarcophagus", 1, 2, Any, 2, 5), ("coffin", 1, 2, Any, 0, 2), ("candles", 1, 1, Any, 1, 2), ("urn", 1, 1, Edge, 1, 3)],
        "catacomb gallery" => &[("sarcophagus", 1, 2, Any, 1, 3), ("skulls", 1, 1, Edge, 1, 3), ("cobweb", 1, 1, Edge, 1, 3), ("urn", 1, 1, Edge, 0, 2)],
        "ossuary" => &[("skulls", 1, 1, Edge, 2, 5), ("bones", 1, 1, Any, 2, 5), ("urn", 1, 1, Edge, 1, 3)],
        "tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("effigy", 1, 2, Edge, 0, 1), ("offering", 1, 1, Edge, 1, 2), ("urn", 1, 1, Edge, 1, 3), ("chest", 1, 1, Edge, 0, 1)],
        "sealed tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("cobweb", 1, 1, Edge, 2, 4), ("skeleton", 1, 2, Any, 0, 1), ("chest", 1, 1, Edge, 0, 1)],
        "embalming room" => &[("table", 1, 2, Any, 1, 2), ("urn", 1, 1, Edge, 1, 3), ("candles", 1, 1, Any, 0, 1), ("coffin", 1, 2, Edge, 0, 1)],
        "reliquary" => &[("altar", 2, 1, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("offering", 1, 1, Any, 1, 2), ("candles", 1, 1, Any, 1, 2)],
        "passage" | "catacomb passage" => &[("trap", 1, 1, Any, 0, 2), ("cobweb", 1, 1, Edge, 1, 3), ("skulls", 1, 1, Edge, 0, 1)],
        "stair foot" => &[("urn", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 0, 1), ("cobweb", 1, 1, Edge, 0, 2)],
        "junction" => &[("bones", 1, 1, Any, 0, 1), ("urn", 1, 1, Edge, 0, 1), ("candles", 1, 1, Any, 0, 1), ("cobweb", 1, 1, Edge, 0, 1)],
        "burial niches" => &[("niche", 1, 1, Edge, 4, 9), ("bones", 1, 1, Any, 0, 2), ("cobweb", 1, 1, Edge, 0, 2)],
        "charnel pit" => &[("pit", 2, 2, Any, 1, 1), ("skulls", 1, 1, Edge, 1, 3), ("bones", 1, 1, Any, 2, 4)],
        "bone chapel" => &[("altar", 2, 1, Edge, 1, 1), ("skulls", 1, 1, Edge, 2, 5), ("candles", 1, 1, Any, 1, 3), ("urn", 1, 1, Edge, 1, 2)],
        "flooded gallery" | "flooded hall" => &[("pool", 2, 2, Any, 1, 3), ("niche", 1, 1, Edge, 0, 3), ("coffin", 1, 2, Any, 0, 1), ("debris", 1, 1, Any, 0, 2)],
        // Sewers and their undercrofts.
        "drain chamber" => &[("pipe", 1, 1, Edge, 1, 2), ("debris", 1, 1, Any, 0, 2)],
        "culvert" => &[("trap", 1, 1, Any, 0, 1), ("debris", 1, 1, Any, 0, 2), ("rat_nest", 1, 1, Edge, 0, 1)],
        "cistern" => &[("pool", 2, 2, Any, 2, 4), ("pillar", 1, 1, Any, 1, 4), ("pipe", 1, 1, Edge, 1, 2)],
        "overflow chamber" => &[("pool", 2, 2, Any, 1, 2), ("rubble", 1, 1, Any, 1, 3), ("debris", 1, 1, Any, 1, 3), ("pipe", 1, 1, Edge, 1, 2)],
        "smugglers' cache" => &[("crate", 1, 1, Edge, 2, 5), ("barrel", 1, 1, Edge, 1, 3), ("sacks", 1, 1, Edge, 1, 3), ("chest", 1, 1, Edge, 1, 1), ("bedroll", 1, 2, Any, 0, 2), ("lantern", 1, 1, Edge, 1, 1)],
        "old vault" => &[("chest", 1, 1, Edge, 1, 3), ("pillar", 1, 1, Any, 0, 2), ("statue", 1, 1, Edge, 0, 1), ("cobweb", 1, 1, Edge, 1, 2)],
        "rat warren" => &[("rat_nest", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 2, 5), ("rubble", 1, 1, Any, 1, 3), ("debris", 1, 1, Any, 0, 2)],
        "sewer tunnel" => &[("debris", 1, 1, Edge, 3, 7), ("pipe", 1, 1, Edge, 3, 7), ("rat_nest", 1, 1, Edge, 1, 3), ("rubble", 1, 1, Edge, 1, 4), ("bones", 1, 1, Edge, 1, 3), ("barrel", 1, 1, Edge, 0, 2)],
        "service passage" => &[("debris", 1, 1, Any, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        // Caves.
        "cavern" => &[("stalagmite", 1, 1, Any, 2, 6), ("boulder", 2, 2, Any, 0, 2), ("rock_column", 1, 1, Any, 0, 2), ("rubble", 1, 1, Any, 0, 2), ("skeleton", 1, 2, Any, 0, 1), ("web", 1, 1, Edge, 0, 1)],
        "grotto" => &[("stalagmite", 1, 1, Any, 2, 5), ("rock_column", 1, 1, Any, 0, 1), ("moss", 1, 1, Any, 1, 3), ("boulder", 2, 2, Any, 0, 1)],
        "bat roost" => &[("guano", 2, 2, Any, 1, 3), ("stalagmite", 1, 1, Any, 1, 4), ("bones", 1, 1, Any, 0, 2)],
        "crystal grotto" => &[("crystal", 1, 1, Any, 3, 7), ("boulder", 2, 2, Any, 0, 1), ("rock_column", 1, 1, Any, 0, 1)],
        "fungus grotto" => &[("fungus", 1, 1, Any, 3, 6), ("mushroom", 1, 1, Any, 2, 5), ("moss", 1, 1, Any, 1, 2)],
        "pool chamber" => &[("pool", 2, 2, Any, 1, 2), ("stalagmite", 1, 1, Any, 1, 3), ("moss", 1, 1, Any, 1, 3)],
        "ledge" => &[("stalagmite", 1, 1, Any, 0, 2), ("boulder", 2, 2, Any, 0, 1), ("campfire", 1, 1, Any, 0, 1), ("bedroll", 1, 2, Any, 0, 1)],
        // Lava tubes.
        "lava tube" | "tube gallery" => &[("basalt", 1, 1, Any, 1, 3), ("scorched", 1, 2, Any, 0, 1), ("obsidian", 1, 1, Any, 0, 2), ("rubble", 1, 1, Any, 0, 2)],
        "basalt shelf" => &[("basalt", 1, 1, Any, 1, 2), ("sulfur", 1, 1, Any, 0, 2)],
        "side pocket" | "obsidian gallery" => &[("obsidian", 1, 1, Any, 2, 4), ("basalt", 1, 1, Any, 0, 2), ("vent", 1, 1, Any, 0, 1), ("sulfur", 1, 1, Any, 1, 3), ("glass_pool", 2, 2, Any, 0, 1)],
        // Mines.
        "haulage way" => &[("ore_cart", 1, 1, Any, 1, 2), ("timber", 1, 1, Edge, 4, 10), ("lantern", 1, 1, Edge, 2, 4), ("tools", 1, 1, Edge, 0, 2)],
        "drift" => &[("timber", 1, 1, Edge, 0, 3), ("ore_vein", 1, 1, Edge, 1, 3), ("cave_in", 1, 1, Any, 0, 1), ("tools", 1, 1, Edge, 0, 1)],
        "stope" | "ore chamber" => &[("ore_vein", 1, 1, Edge, 2, 4), ("ore_cart", 1, 1, Any, 0, 1), ("rubble", 1, 1, Any, 1, 3), ("timber", 1, 1, Edge, 1, 3), ("tools", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        "collapsed stope" => &[("rubble", 1, 1, Any, 3, 6), ("boulder", 2, 2, Any, 0, 2), ("cave_in", 1, 1, Any, 1, 1), ("skeleton", 1, 2, Any, 0, 1)],
        "pump room" => &[("pool", 2, 2, Any, 1, 1), ("barrel", 1, 1, Edge, 1, 2), ("winch", 1, 1, Edge, 0, 1)],
        "shaft head" => &[("winch", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 0, 1)],
        // Prisons.
        "gatehouse" => &[("weapon_rack", 1, 1, Edge, 1, 2), ("table", 1, 1, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("chest", 1, 1, Edge, 0, 1)],
        "warden's office" => &[("desk", 2, 1, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 1), ("bookshelf", 1, 2, Edge, 0, 1), ("sconce", 1, 1, Edge, 1, 1)],
        "mess hall" => &[("long_table", 1, 3, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 2), ("sacks", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 1, 1)],
        "interrogation room" => &[("table", 1, 1, Any, 1, 1), ("chains", 1, 1, Edge, 1, 3), ("brazier", 1, 1, Edge, 1, 1), ("cage", 2, 2, Edge, 0, 1)],
        "guard post" => &[("desk", 2, 1, Edge, 1, 1), ("weapon_rack", 1, 1, Edge, 2, 3), ("brazier", 1, 1, Edge, 1, 2), ("chest", 1, 1, Edge, 0, 1)],
        "cell" => &[("chains", 1, 1, Edge, 0, 1), ("bedroll", 1, 2, Any, 0, 1), ("bones", 1, 1, Any, 0, 1)],
        "solitary cell" => &[("chains", 1, 1, Edge, 1, 1), ("bones", 1, 1, Any, 0, 1)],
        "cell row" => &[("sconce", 1, 1, Edge, 0, 2), ("chains", 1, 1, Edge, 0, 1)],
        "cell corridor" => &[("chains", 1, 1, Edge, 1, 3), ("sconce", 1, 1, Edge, 1, 3), ("trap", 1, 1, Any, 0, 1)],
        // Temples.
        "nave" => &[("sconce", 1, 1, Edge, 1, 3), ("banner", 1, 1, Edge, 0, 2), ("statue", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 0, 1)],
        "side chapel" => &[("altar", 2, 1, Edge, 1, 1), ("candles", 1, 1, Any, 1, 1), ("statue", 1, 1, Edge, 0, 1), ("offering", 1, 1, Edge, 0, 1)],
        "crossing" => &[("candles", 1, 1, Any, 0, 2), ("brazier", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 0, 2)],
        "narthex" => &[("statue", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 1), ("fountain", 2, 2, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 1)],
        "sanctum" => &[("altar", 2, 1, Edge, 1, 1), ("statue", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 3), ("brazier", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("pillar", 1, 1, Any, 0, 2)],
        "vestry" => &[("bookshelf", 1, 2, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 0, 1), ("table", 1, 2, Any, 0, 1)],
        "priests' cells" => &[("cot", 1, 2, Edge, 2, 4), ("chest", 1, 1, Edge, 0, 2), ("candles", 1, 1, Any, 0, 1)],
        "ritual pool" => &[("pool", 2, 2, Any, 1, 2), ("candles", 1, 1, Any, 1, 2), ("glyph", 2, 2, Any, 0, 1)],
        "processional way" => &[("statue", 1, 1, Edge, 0, 2), ("sconce", 1, 1, Edge, 1, 4), ("banner", 1, 1, Edge, 0, 2), ("trap", 1, 1, Any, 0, 1)],
        // Wizards' lairs.
        "foyer" => &[("statue", 1, 1, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("sconce", 1, 1, Edge, 1, 2), ("bookshelf", 1, 2, Edge, 0, 1)],
        "hall" => &[("sconce", 1, 1, Edge, 1, 3), ("banner", 1, 1, Edge, 0, 2), ("trap", 1, 1, Any, 0, 1)],
        "library" => &[("bookshelf", 1, 2, Edge, 3, 6), ("desk", 2, 1, Any, 0, 1), ("table", 1, 2, Any, 0, 1), ("candles", 1, 1, Any, 1, 2)],
        "laboratory" => &[("table", 1, 2, Any, 1, 2), ("bookshelf", 1, 2, Edge, 0, 2), ("vat", 1, 1, Edge, 0, 2), ("crystal", 1, 1, Any, 0, 1), ("glyph", 2, 2, Any, 0, 1), ("candles", 1, 1, Any, 1, 1)],
        "summoning circle" => &[("glyph", 2, 2, Any, 1, 1), ("candles", 1, 1, Any, 2, 4), ("brazier", 1, 1, Edge, 0, 2)],
        "specimen vault" => &[("cage", 2, 2, Edge, 1, 3), ("vat", 1, 1, Edge, 1, 2), ("bones", 1, 1, Any, 0, 2)],
        "study" => &[("desk", 2, 1, Edge, 1, 1), ("bookshelf", 1, 2, Edge, 1, 2), ("rug", 2, 3, Any, 0, 1), ("candles", 1, 1, Any, 1, 1)],
        "menagerie" => &[("cage", 2, 2, Edge, 2, 3), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 1)],
        "golem workshop" => &[("anvil", 1, 1, Any, 1, 1), ("forge", 1, 1, Edge, 0, 1), ("table", 1, 2, Any, 1, 1), ("statue", 1, 1, Edge, 1, 2), ("tools", 1, 1, Edge, 0, 1)],
        "scrying room" => &[("fountain", 2, 2, Any, 1, 1), ("candles", 1, 1, Any, 1, 2), ("crystal", 1, 1, Edge, 0, 2), ("rug", 2, 3, Any, 0, 1)],
        // Bandits.
        "hidden entry" => &[("crate", 1, 1, Edge, 1, 2), ("barrel", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 1, 1), ("trap", 1, 1, Any, 0, 1)],
        "common room" => &[("table", 1, 2, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 3), ("campfire", 1, 1, Any, 0, 1), ("bedroll", 1, 2, Any, 0, 2), ("sacks", 1, 1, Edge, 0, 2)],
        "bunk room" => &[("bedroll", 1, 2, Edge, 3, 6), ("chest", 1, 1, Edge, 1, 2), ("lantern", 1, 1, Edge, 0, 1)],
        "loot store" => &[("chest", 1, 1, Edge, 2, 4), ("crate", 1, 1, Edge, 2, 4), ("sacks", 1, 1, Edge, 1, 3), ("hoard", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        "kitchen" => &[("hearth", 1, 1, Edge, 1, 1), ("barrel", 1, 1, Edge, 1, 3), ("sacks", 1, 1, Edge, 1, 2), ("table", 1, 2, Any, 1, 1)],
        "captain's quarters" => &[("bed", 1, 2, Edge, 1, 1), ("chest", 1, 1, Edge, 1, 2), ("desk", 2, 1, Edge, 0, 1), ("rug", 2, 3, Any, 0, 1), ("weapon_rack", 1, 1, Edge, 0, 1)],
        "lookout" => &[("crate", 1, 1, Edge, 0, 1), ("lantern", 1, 1, Edge, 1, 1), ("bedroll", 1, 2, Any, 0, 1)],
        "tunnel" => &[("lantern", 1, 1, Edge, 0, 2), ("debris", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        // Dwarven halls.
        "gate hall" => &[("statue", 1, 1, Edge, 2, 4), ("pillar", 1, 1, Any, 2, 4), ("banner", 1, 1, Edge, 1, 2), ("sconce", 1, 1, Edge, 1, 3)],
        "great hall" => &[("long_table", 1, 3, Any, 2, 4), ("pillar", 1, 1, Any, 2, 6), ("hearth", 1, 1, Edge, 0, 1), ("banner", 1, 1, Edge, 1, 3), ("statue", 1, 1, Edge, 0, 2)],
        "forge" => &[("forge", 1, 1, Edge, 1, 2), ("anvil", 1, 1, Any, 1, 2), ("barrel", 1, 1, Edge, 1, 2), ("tools", 1, 1, Edge, 1, 2), ("crate", 1, 1, Edge, 0, 2)],
        "brewery" => &[("vat", 1, 1, Any, 2, 3), ("keg_rack", 1, 2, Edge, 1, 2), ("barrel", 1, 1, Edge, 2, 4)],
        "treasury" => &[("chest", 1, 1, Edge, 2, 5), ("hoard", 1, 1, Any, 1, 2), ("statue", 1, 1, Edge, 0, 1), ("trap", 1, 1, Any, 1, 2)],
        "ancestor hall" => &[("statue", 1, 1, Edge, 2, 5), ("sarcophagus", 1, 2, Any, 1, 3), ("candles", 1, 1, Any, 1, 2), ("brazier", 1, 1, Edge, 0, 2)],
        "mushroom farm" => &[("mushroom", 1, 1, Any, 2, 5), ("fungus", 1, 1, Any, 2, 4), ("barrel", 1, 1, Edge, 0, 1)],
        "gallery" => &[("sconce", 1, 1, Edge, 1, 3), ("statue", 1, 1, Edge, 0, 1), ("pillar", 1, 1, Any, 0, 2)],
        // Goblin warrens.
        "warren mouth" => &[("bones", 1, 1, Any, 1, 2), ("rubble", 1, 1, Any, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "den" => &[("bedroll", 1, 2, Any, 2, 4), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 2), ("campfire", 1, 1, Any, 0, 1)],
        "nest" => &[("nest", 1, 1, Any, 1, 2), ("bedroll", 1, 2, Any, 1, 3), ("bones", 1, 1, Any, 1, 2)],
        "cook pit" => &[("campfire", 1, 1, Any, 1, 1), ("barrel", 1, 1, Edge, 0, 2), ("bones", 1, 1, Any, 1, 3), ("sacks", 1, 1, Edge, 0, 1)],
        "refuse pit" => &[("pit", 2, 2, Any, 1, 1), ("debris", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 1, 3)],
        "wolf pen" => &[("nest", 1, 1, Any, 1, 2), ("bones", 1, 1, Any, 2, 4), ("chains", 1, 1, Edge, 1, 2), ("cage", 2, 2, Edge, 0, 1)],
        "shaman's hut" => &[("glyph", 2, 2, Any, 0, 1), ("skulls", 1, 1, Edge, 1, 2), ("candles", 1, 1, Any, 1, 2), ("bones", 1, 1, Any, 1, 1), ("offering", 1, 1, Edge, 1, 1)],
        "crawlway" => &[("rubble", 1, 1, Any, 0, 2), ("bones", 1, 1, Any, 0, 1), ("trap", 1, 1, Any, 0, 1)],
        // Flooded vaults.
        "flooded stair" => &[("pool", 2, 2, Any, 1, 2), ("debris", 1, 1, Any, 0, 2), ("moss", 1, 1, Any, 1, 2)],
        "sunken chapel" => &[("altar", 2, 1, Edge, 1, 1), ("pool", 2, 2, Any, 1, 2), ("statue", 1, 1, Edge, 0, 1), ("moss", 1, 1, Any, 1, 2)],
        "drowned barracks" => &[("cot", 1, 2, Edge, 2, 4), ("pool", 2, 2, Any, 1, 2), ("debris", 1, 1, Any, 1, 2), ("skeleton", 1, 2, Any, 0, 1)],
        "flooded passage" => &[("pool", 2, 2, Any, 0, 2), ("moss", 1, 1, Any, 1, 2), ("debris", 1, 1, Any, 0, 1)],
        // Tombs and ossuaries.
        "false tomb" => &[("sarcophagus", 1, 2, Any, 1, 1), ("trap", 1, 1, Any, 1, 3), ("pit", 2, 2, Any, 0, 1), ("cobweb", 1, 1, Edge, 1, 2)],
        "guardian hall" => &[("statue", 1, 1, Edge, 2, 4), ("pillar", 1, 1, Any, 2, 4), ("trap", 1, 1, Any, 0, 2), ("sconce", 1, 1, Edge, 0, 2)],
        "skull gallery" => &[("skulls", 1, 1, Edge, 3, 6), ("niche", 1, 1, Edge, 2, 5), ("candles", 1, 1, Any, 0, 1)],
        // Themed caves.
        "fungus forest" => &[("mushroom", 1, 1, Any, 3, 6), ("fungus", 1, 1, Any, 3, 6), ("moss", 1, 1, Any, 1, 2)],
        "spore grotto" => &[("fungus", 1, 1, Any, 2, 4), ("mushroom", 1, 1, Any, 1, 3), ("web", 1, 1, Edge, 0, 1)],
        "mycelium hall" => &[("fungus", 1, 1, Any, 4, 8), ("mushroom", 1, 1, Any, 1, 3), ("stalagmite", 1, 1, Any, 0, 2)],
        "crystal cavern" => &[("crystal", 1, 1, Any, 4, 8), ("stalagmite", 1, 1, Any, 1, 3), ("rock_column", 1, 1, Any, 0, 1)],
        "geode" => &[("crystal", 1, 1, Any, 5, 9), ("boulder", 2, 2, Any, 0, 1)],
        "crystal shelf" => &[("crystal", 1, 1, Any, 1, 3)],
        "ice cavern" => &[("ice", 1, 1, Any, 2, 5), ("ice_sheet", 2, 2, Any, 1, 3), ("stalagmite", 1, 1, Any, 0, 2)],
        "frozen pool" => &[("ice_sheet", 2, 2, Any, 2, 4), ("ice", 1, 1, Any, 1, 3)],
        "icicle gallery" => &[("ice", 1, 1, Any, 4, 7), ("ice_sheet", 2, 2, Any, 0, 2)],
        "ice shelf" => &[("ice", 1, 1, Any, 1, 2), ("ice_sheet", 2, 2, Any, 0, 1)],
        "flooded cavern" => &[("pool", 2, 2, Any, 2, 4), ("stalagmite", 1, 1, Any, 1, 3), ("moss", 1, 1, Any, 1, 3)],
        "sump" => &[("pool", 2, 2, Any, 2, 3), ("debris", 1, 1, Any, 0, 2), ("moss", 1, 1, Any, 1, 2)],
        "lair" => &[("bones", 1, 1, Any, 3, 6), ("skeleton", 1, 2, Any, 1, 2), ("nest", 1, 1, Any, 1, 2), ("boulder", 2, 2, Any, 0, 1)],
        "nest chamber" => &[("nest", 1, 1, Any, 2, 4), ("bones", 1, 1, Any, 2, 4), ("guano", 2, 2, Any, 0, 1)],
        "gnawing chamber" => &[("bones", 1, 1, Any, 3, 6), ("skeleton", 1, 2, Any, 0, 2), ("skulls", 1, 1, Any, 0, 1)],
        _ => &[],
    }
}

pub(crate) fn furnish(p: &mut Plan, nx: usize, ny: usize, kind: UnderKind, rng: &mut Pcg32) {
    p.reserve_doors();
    let up = p.up;
    p.taken[up] = true;
    if let Some(d) = p.down {
        p.taken[d] = true;
    }
    // Rails down the haulage way, before anything stands on it.
    if kind == UnderKind::Mine {
        let rails: Vec<usize> =
            (0..nx * ny).filter(|&k| p.cells[k] >= 0 && p.rooms[p.cells[k] as usize].kind == "haulage way" && k >= nx && p.cells[k - nx] == p.cells[k] && !p.taken[k]).collect();
        for k in rails {
            p.put(item("rail", k % nx, k / nx, 1, 1));
        }
    }
    let rooms: Vec<(i16, &'static str)> = p.rooms.iter().enumerate().map(|(i, r)| (i as i16, r.kind)).collect();
    for (ri, rkind) in rooms {
        furnish_room(p, nx, ny, ri, rkind, kind, rng);
    }
    if p.niches {
        line_niches(p, nx, ny);
    }
}

/// Burial niches down the narrow galleries (squares of the passage with walls on two opposite sides), on the two
/// walls by turns.
fn line_niches(p: &mut Plan, nx: usize, ny: usize) {
    let Some((passage, _)) = p.passage else { return };
    // (A lock in a gallery splits the passage: the part beyond is a room of its kind, `guard`.)
    let kind = p.rooms[passage as usize].kind;
    for k in 0..nx * ny {
        let r = p.cells[k];
        if r < 0 || p.rooms[r as usize].kind != kind || p.taken[k] {
            continue;
        }
        let (i, j) = (k % nx, k / nx);
        let wall = |di: isize, dj: isize| {
            let (x, y) = (i as isize + di, j as isize + dj);
            x < 0 || y < 0 || x >= nx as isize || y >= ny as isize || p.cells[y as usize * nx + x as usize] != r
        };
        // (Its back: 0 north, 1 east, 2 south, 3 west.)
        let sides = if wall(0, -1) && wall(0, 1) {
            [0, 2]
        } else if wall(-1, 0) && wall(1, 0) {
            [3, 1]
        } else {
            continue;
        };
        let mut it = item("niche", i, j, 1, 1);
        it.back = Some(sides[(i + j) % 2]);
        p.place(it, Some(r), ny);
    }
}

/// Room `ri`'s kit (a boss chamber's set piece), where it fits around what is there.
pub(crate) fn furnish_room(p: &mut Plan, nx: usize, ny: usize, ri: i16, rkind: &str, kind: UnderKind, rng: &mut Pcg32) {
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| p.cells[k] == ri).collect();
    if squares.is_empty() {
        return;
    }
    if rkind == BOSS {
        boss_chamber(p, nx, ny, ri, &squares, kind, rng);
        return;
    }
    let edge: Vec<usize> = squares.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] != ri) || k % nx == 0 || k / nx == 0).collect();
    for &(what, w, h, spot, lo, hi) in kit(rkind) {
        // Big rooms (a sewer's whole network, a great cavern) get a kit per 80 squares.
        let n = (lo + rng.below(hi - lo + 1)) * (squares.len() as u32 / 80).clamp(1, 12);
        let mut placed = 0;
        for _ in 0..n * 12 {
            if placed >= n {
                break;
            }
            let pool = match spot {
                Spot::Edge if !edge.is_empty() => &edge,
                _ => &squares,
            };
            let k = pool[rng.below(pool.len() as u32) as usize];
            let (w, h) = if rng.next_f64() < 0.5 { (w, h) } else { (h, w) };
            if p.place(item(what, k % nx, k / nx, w, h), Some(ri), ny) {
                placed += 1;
            }
        }
    }
}

/// Pillars in rows, a dais and throne at the far end from the way in, a hoard beside it,
/// braziers; caves get bones and a hoard instead of masonry.
pub(crate) fn boss_chamber(p: &mut Plan, nx: usize, ny: usize, ri: i16, squares: &[usize], kind: UnderKind, rng: &mut Pcg32) {
    let dist = p.distances(p.up, ny, None);
    let far = squares.iter().copied().max_by_key(|&k| dist[k]).unwrap_or(squares[0]);
    let (fx, fy) = (far % nx, far / nx);
    let built = !matches!(kind, UnderKind::Cave | UnderKind::LavaTube);
    if built {
        // The dais against the far end.
        for (dx, dy) in [(0isize, 0isize), (-1, 0), (0, -1), (-1, -1), (-2, 0), (0, -2), (-2, -1), (-1, -2), (1, 0), (0, 1)] {
            let (x, y) = (fx as isize + dx, fy as isize + dy);
            if x >= 0 && y >= 0 && p.place(item("dais", x as usize, y as usize, 3, 2), Some(ri), ny) {
                break;
            }
        }
        if kind != UnderKind::Mine {
            let (x0, y0) = (squares.iter().map(|k| k % nx).min().unwrap_or(0), squares.iter().map(|k| k / nx).min().unwrap_or(0));
            let (x1, y1) = (squares.iter().map(|k| k % nx).max().unwrap_or(0), squares.iter().map(|k| k / nx).max().unwrap_or(0));
            if x1 >= x0 + 5 && y1 >= y0 + 5 {
                let mut x = x0 + 2;
                while x + 2 <= x1 {
                    for y in [y0 + 1, y1 - 1] {
                        p.place(item("pillar", x, y, 1, 1), Some(ri), ny);
                    }
                    x += 3;
                }
            }
        }
        for _ in 0..30 {
            let k = squares[rng.below(squares.len() as u32) as usize];
            if p.place(item("brazier", k % nx, k / nx, 1, 1), Some(ri), ny) && rng.next_f64() < 0.4 {
                break;
            }
        }
        // Banners on the walls, a ritual circle on the floor.
        let edge: Vec<usize> = squares.iter().copied().filter(|&k| neighbours(k, nx, ny).any(|m| p.cells[m] != ri)).collect();
        let mut banners = 0;
        for _ in 0..20 {
            if banners >= 3 || edge.is_empty() {
                break;
            }
            let k = edge[rng.below(edge.len() as u32) as usize];
            if p.place(item("banner", k % nx, k / nx, 1, 1), Some(ri), ny) {
                banners += 1;
            }
        }
        if kind != UnderKind::Mine {
            for _ in 0..20 {
                let k = squares[rng.below(squares.len() as u32) as usize];
                if p.place(item("glyph", k % nx, k / nx, 2, 2), Some(ri), ny) {
                    break;
                }
            }
        }
    } else {
        for _ in 0..8 {
            let k = squares[rng.below(squares.len() as u32) as usize];
            let what = ["bones", "skeleton", "stalagmite", "boulder", if kind == UnderKind::LavaTube { "vent" } else { "pool" }][rng.below(5) as usize];
            let (w, h) = match what {
                "boulder" => (2, 2),
                "skeleton" => (1, 2),
                _ => (1, 1),
            };
            p.place(item(what, k % nx, k / nx, w, h), Some(ri), ny);
        }
    }
    // The hoard, as near the far end as fits.
    let mut near: Vec<usize> = squares.to_vec();
    near.sort_by_key(|&k| (k % nx).abs_diff(fx) + (k / nx).abs_diff(fy));
    let mut put = 0;
    for k in near {
        if put >= 2 {
            break;
        }
        if p.place(item(if put == 0 { "hoard" } else { "chest" }, k % nx, k / nx, 1, 1), Some(ri), ny) {
            put += 1;
        }
    }
}

/// A site's locks while its built levels are laid out, top down: how many it may have (by its
/// size: a small one none or one, a medium one or two, a big one up to three or four), how many
/// it has (the next one's colour is `made + 1`), and the rooms on the built levels above.
pub(crate) struct Locks {
    budget: usize,
    made: u8,
    rooms_above: usize,
}

impl Locks {
    /// `size`: the ground it covers against a medium site's.
    pub(crate) fn new(size: f64, rng: &mut Pcg32) -> Locks {
        let (lo, hi) = if size < 0.8 {
            (0, 1)
        } else if size < 1.3 {
            (1, 2)
        } else if size < 2.0 {
            (2, 3)
        } else {
            (3, 4)
        };
        Locks { budget: (lo + rng.below(hi - lo + 1)) as usize, made: 0, rooms_above: 0 }
    }
}

/// Floor squares reached from `start`, through no door that `shut` names (square pairs, smaller first).
fn reach(p: &Plan, ny: usize, start: usize, shut: &dyn Fn((usize, usize)) -> bool) -> Vec<bool> {
    let mut seen = vec![false; p.cells.len()];
    if p.cells[start] < 0 {
        return seen;
    }
    seen[start] = true;
    let mut stack = vec![start];
    while let Some(k) = stack.pop() {
        for m in neighbours(k, p.nx, ny) {
            if !seen[m] && p.passable(k, m) && !shut((k.min(m), k.max(m))) {
                seen[m] = true;
                stack.push(m);
            }
        }
    }
    seen
}

/// The floor's parts with the `shut` doors closed: a label per square (u32::MAX rock) and how many.
fn parts(p: &Plan, ny: usize, shut: &dyn Fn((usize, usize)) -> bool) -> (Vec<u32>, u32) {
    let mut label = vec![u32::MAX; p.cells.len()];
    let mut n = 0;
    for s in 0..p.cells.len() {
        if p.cells[s] < 0 || label[s] != u32::MAX {
            continue;
        }
        label[s] = n;
        let mut stack = vec![s];
        while let Some(k) = stack.pop() {
            for m in neighbours(k, p.nx, ny) {
                if label[m] == u32::MAX && p.passable(k, m) && !shut((k.min(m), k.max(m))) {
                    label[m] = n;
                    stack.push(m);
                }
            }
        }
        n += 1;
    }
    (label, n)
}

/// The steps (square pairs: one on the way in, one beyond) that are the only way between their
/// sides, from `start` (Tarjan's bridges), with each square's entry and exit times: square s
/// lies beyond step (_, c) when `tin[c] <= tin[s] < tout[c]`.
fn bridges(p: &Plan, ny: usize, start: usize) -> (Vec<(usize, usize)>, Vec<u32>, Vec<u32>) {
    let (nx, n) = (p.nx, p.cells.len());
    let (mut tin, mut low, mut tout) = (vec![u32::MAX; n], vec![0u32; n], vec![u32::MAX; n]);
    let mut cut = Vec::new();
    let mut timer = 0;
    tin[start] = 0;
    low[start] = 0;
    timer += 1;
    // (Square, the one it was reached from, the next side to look at.)
    let mut stack: Vec<(usize, usize, u8)> = vec![(start, usize::MAX, 0)];
    while let Some(top) = stack.last_mut() {
        let (v, from, side) = *top;
        if side < 4 {
            top.2 += 1;
            let (i, j) = (v % nx, v / nx);
            let w = match side {
                0 if i + 1 < nx => v + 1,
                1 if i > 0 => v - 1,
                2 if j + 1 < ny => v + nx,
                3 if j > 0 => v - nx,
                _ => continue,
            };
            if w == from || !p.passable(v, w) {
                continue;
            }
            if tin[w] == u32::MAX {
                tin[w] = timer;
                low[w] = timer;
                timer += 1;
                stack.push((w, v, 0));
            } else {
                low[v] = low[v].min(tin[w]);
            }
        } else {
            stack.pop();
            tout[v] = timer;
            if from != usize::MAX {
                low[from] = low[from].min(low[v]);
                if low[v] > tin[from] {
                    cut.push((from, v));
                }
            }
        }
    }
    (cut, tin, tout)
}

/// Whether two squares are within `limit` hops of each other on foot: a door crossed is a hop,
/// and so is every 8 squares walked.
fn near(p: &Plan, ny: usize, from: usize, to: usize, limit: u32) -> bool {
    walk(p, ny, from, to, limit * 8).is_some()
}

/// The cost on foot from one square to another (a square walked 1, a door crossed 8: 8 to a hop),
/// if under `limit`.
fn walk(p: &Plan, ny: usize, from: usize, to: usize, limit: u32) -> Option<u32> {
    let mut cost = vec![u32::MAX; p.cells.len()];
    let mut heap = std::collections::BinaryHeap::new();
    cost[from] = 0;
    heap.push(std::cmp::Reverse((0u32, from)));
    while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
        if k == to {
            return Some(c);
        }
        if c > cost[k] {
            continue;
        }
        for m in neighbours(k, p.nx, ny) {
            if p.passable(k, m) {
                let nc = c + if p.cells[k] == p.cells[m] { 1 } else { 8 };
                if nc < cost[m] && nc < limit {
                    cost[m] = nc;
                    heap.push(std::cmp::Reverse((nc, m)));
                }
            }
        }
    }
    None
}

/// The ways in and out on a level (squares of its `exit`, `up`, `down`, `tunnel` items).
fn ways(p: &Plan) -> Vec<usize> {
    p.items.iter().filter(|it| matches!(it.kind, "exit" | "up" | "down" | "tunnel")).map(|it| it.y as usize * p.nx + it.x as usize).collect()
}

/// Dead ends a key may lie in: rooms (not passages (nor corridors split off by a lock), the boss chamber, rooms with a way in or out
/// or a key already) with one door, wholly in `seen`, each with its free square farthest from the
/// way onto the level.
fn dead_ends(p: &Plan, ny: usize, seen: &[bool], has_key: &[bool]) -> Vec<(i16, usize)> {
    let passage = p.passage.map_or(-1, |q| q.0);
    let nr = p.rooms.len();
    let mut doors = vec![0u32; nr];
    let mut at_door = vec![false; p.cells.len()];
    for &(a, b) in &p.doors {
        at_door[a] = true;
        at_door[b] = true;
        for k in [a, b] {
            if p.cells[k] >= 0 {
                doors[p.cells[k] as usize] += 1;
            }
        }
    }
    let mut bad = vec![false; nr];
    for k in ways(p) {
        if p.cells[k] >= 0 {
            bad[p.cells[k] as usize] = true;
        }
    }
    let dist = p.distances(p.up, ny, None);
    let mut best: Vec<Option<usize>> = vec![None; nr];
    for k in 0..p.cells.len() {
        let r = p.cells[k];
        if r < 0 {
            continue;
        }
        let ri = r as usize;
        if !seen[k] {
            bad[ri] = true;
        }
        if !p.taken[k] && !p.blocked[k] && !at_door[k] && best[ri].is_none_or(|b| dist[k] > dist[b]) {
            best[ri] = Some(k);
        }
    }
    (0..nr)
        .filter(|&r| (passage < 0 || p.rooms[r].kind != p.rooms[passage as usize].kind) && p.rooms[r].kind != BOSS && doors[r] == 1 && !bad[r] && !has_key.get(r).copied().unwrap_or(false))
        .filter_map(|r| Some((r as i16, best[r]?)))
        .collect()
}

/// Locks, loops and keys on built level `d` of `depth` (laid out, its ways in and out put, not yet
/// furnished); `above`: the built levels over it, where its keys may lie too.
///
/// Locks go on doors that are the only way on to a big part of the site (deeper levels count as
/// beyond the way down), never on a secret door or the way up, never cutting off a way out. Then
/// loops, each kept only if it joins no two parts the locks keep apart: the planner's corridors
/// between near rooms, and doors between rooms that share a wall but lie 4 or more hops apart
/// (some secret). Then each lock's key, in a dead end reached without it or any later lock: on
/// this level or one above. A lock with nowhere for its key is dropped.
#[allow(clippy::too_many_arguments)]
pub(crate) fn guard(p: &mut Plan, above: &mut [Plan], nx: usize, ny: usize, d: usize, depth: usize, locks: &mut Locks, rng: &mut Pcg32) {
    let n = nx * ny;
    let passage = p.passage.map_or(-1, |q| q.0);
    let nr = p.rooms.len();
    let mut used = vec![false; nr];
    for k in 0..n {
        if p.cells[k] >= 0 {
            used[p.cells[k] as usize] = true;
        }
    }
    let level_rooms = (0..nr).filter(|&r| used[r] && r as i16 != passage).count();
    let below = (level_rooms * (depth - d - 1)) as f64;
    let total = (locks.rooms_above + level_rooms * (depth - d)).max(1) as f64;
    locks.rooms_above += level_rooms;
    // The locks: doors beyond which a big part lies.
    let left = locks.budget.saturating_sub(locks.made as usize);
    let cap = left.div_ceil(depth - d);
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    if cap > 0 {
        // Steps that are the only way on (a door, or a corridor one square wide, where a door is
        // put): (door, rooms beyond, the way down beyond, share of the site).
        let outs = ways(p);
        let (cut, tin, tout) = bridges(p, ny, p.up);
        let beyond = |c: usize, k: usize| tin[c] <= tin[k] && tin[k] < tout[c];
        let mut rep: Vec<Option<usize>> = vec![None; nr];
        for k in 0..n {
            if p.cells[k] >= 0 && p.cells[k] != passage && rep[p.cells[k] as usize].is_none() {
                rep[p.cells[k] as usize] = Some(k);
            }
        }
        let narrow = |k: usize, m: usize| {
            let across: [Option<usize>; 2] = if m.abs_diff(k) == 1 { [k.checked_sub(nx), (k + nx < n).then_some(k + nx)] } else { [(k % nx > 0).then(|| k - 1), (k % nx + 1 < nx).then_some(k + 1)] };
            across.into_iter().flatten().all(|q| p.cells[q] != p.cells[k])
        };
        // (A corridor's lock not on a square beside a door: that door would do.)
        let mut at_door = vec![false; n];
        for &(a, b) in &p.doors {
            at_door[a] = true;
            at_door[b] = true;
        }
        let mut cands: Vec<((usize, usize), Vec<bool>, bool, f64)> = Vec::new();
        for (a, c) in cut {
            let e = (a.min(c), a.max(c));
            let (k, m) = e;
            let door = p.cells[k] != p.cells[m];
            if p.secret.contains(&e) || k == p.up || m == p.up || p.taken[k] || p.taken[m] || (!door && (p.cells[k] != passage || at_door[k] || at_door[m] || !narrow(k, m) || !narrow(m, k))) {
                continue;
            }
            if outs.iter().any(|&w| beyond(c, w) && p.down != Some(w)) {
                continue;
            }
            let rooms: Vec<bool> = rep.iter().map(|r| r.is_some_and(|k| beyond(c, k))).collect();
            let deeper = p.down.is_some_and(|w| beyond(c, w));
            let weight = rooms.iter().filter(|&&r| r).count() as f64 + if deeper { below } else { 0.0 };
            if weight >= 2.0 && weight >= 0.15 * total {
                cands.push((e, rooms, deeper, weight / total));
            }
        }
        let mut picked: Vec<usize> = Vec::new();
        while picked.len() < cap {
            // (Two locks guard parts at least two rooms apart.)
            let apart = |i: usize, j: usize| {
                let (a, b) = (&cands[i], &cands[j]);
                a.2 != b.2 || a.1.iter().zip(&b.1).filter(|(x, y)| x != y).count() >= 2
            };
            let mut best: Option<(f64, usize)> = None;
            for i in 0..cands.len() {
                if picked.contains(&i) || !picked.iter().all(|&j| apart(i, j)) {
                    continue;
                }
                let score = (cands[i].3 - 0.4).abs() + 0.2 * rng.next_f64();
                if best.is_none_or(|(b, _)| score < b) {
                    best = Some((score, i));
                }
            }
            let Some((_, i)) = best else { break };
            picked.push(i);
        }
        chosen = picked.into_iter().map(|i| cands[i].0).collect();
        // Nearer the way onto the level first.
        let dist = p.distances(p.up, ny, None);
        chosen.sort_by_key(|&(a, b)| (dist[a].min(dist[b]), a, b));
        // A lock in a corridor: the corridor beyond it becomes a passage of its own, a door between.
        for &(k, m) in &chosen {
            if p.cells[k] == p.cells[m] {
                let seen = reach(p, ny, p.up, &|q| q == (k, m));
                let (old, new) = (p.cells[k], p.room(p.rooms[passage as usize].kind, 0.0));
                for q in 0..n {
                    if !seen[q] && p.cells[q] == old {
                        p.cells[q] = new;
                    }
                }
                p.door(k, m);
            }
        }
    }
    // Loops: the planner's corridors between near rooms, then doors through shared walls.
    let shut = |q: (usize, usize)| chosen.contains(&q);
    let (_, kept) = parts(p, ny, &shut);
    let mut dug = 0;
    if let Some((passage, wide)) = p.passage {
        for (a, b) in std::mem::take(&mut p.shortcuts) {
            let (cells, doors) = (p.cells.clone(), p.doors.clone());
            let made = dig(p, nx, ny, a, b, passage, wide);
            if made.is_empty() {
                continue;
            }
            if !chosen.is_empty() && parts(p, ny, &shut).1 != kept {
                p.cells = cells;
                p.doors = doors;
                continue;
            }
            dug += 1;
            // Some walled off by secret doors.
            let new: Vec<(usize, usize)> = made.into_iter().filter(|e| !doors.contains(e)).collect();
            if !new.is_empty() && rng.next_f64() < 0.5 && p.secret.len() + new.len() <= p.secrets {
                p.secret.extend(new);
            }
        }
    }
    let (zone, _) = parts(p, ny, &shut);
    let mut walls: Vec<(usize, usize)> = Vec::new();
    for k in 0..n {
        for m in [(k % nx + 1 < nx).then(|| k + 1), (k + nx < n).then(|| k + nx)].into_iter().flatten() {
            let (ra, rb) = (p.cells[k], p.cells[m]);
            if ra >= 0
                && rb >= 0
                && ra != rb
                && zone[k] == zone[m]
                && !p.taken[k]
                && !p.taken[m]
                && !p.doors.contains(&(k, m))
                && p.rooms[ra as usize].kind != BOSS
                && p.rooms[rb as usize].kind != BOSS
            {
                walls.push((k, m));
            }
        }
    }
    let mut most = (1 + level_rooms / 8).saturating_sub(dug / 2);
    if p.loops < 1.0 {
        most = (most as f64 * p.loops).round() as usize;
    }
    let mut made = 0;
    for _ in 0..24 {
        if made >= most || walls.is_empty() {
            break;
        }
        let (a, b) = walls.swap_remove(rng.below(walls.len() as u32) as usize);
        if !near(p, ny, a, b, 4) {
            p.door(a, b);
            if rng.next_f64() < 0.3 && p.secret.len() < p.secrets {
                p.secret.insert((a, b));
            }
            made += 1;
        }
    }
    // The keys, each lock's in turn.
    let mut keyed_here = vec![false; nr];
    let mut keyed_above: Vec<Vec<bool>> = above.iter().map(|q| vec![false; q.rooms.len()]).collect();
    for (li, q) in above.iter().enumerate() {
        for it in q.items.iter().filter(|it| it.kind == "key") {
            let r = q.cells[it.y as usize * nx + it.x as usize];
            if r >= 0 {
                keyed_above[li][r as usize] = true;
            }
        }
    }
    for i in 0..chosen.len() {
        let later = &chosen[i..];
        let seen = reach(p, ny, p.up, &|q| later.contains(&q));
        let here = dead_ends(p, ny, &seen, &keyed_here);
        let mut there: Vec<(usize, i16, usize)> = Vec::new();
        for (li, q) in above.iter().enumerate() {
            let all = vec![true; q.cells.len()];
            there.extend(dead_ends(q, ny, &all, &keyed_above[li]).into_iter().map(|(r, k)| (li, r, k)));
        }
        let colour = locks.made + 1;
        let mut key = item("key", 0, 0, 1, 1);
        key.variant = Some(colour);
        if !here.is_empty() && (there.is_empty() || rng.next_f64() < 0.6) {
            let (r, k) = here[rng.below(here.len() as u32) as usize];
            (key.x, key.y) = ((k % nx) as u16, (k / nx) as u16);
            p.put(key);
            keyed_here[r as usize] = true;
        } else if !there.is_empty() {
            let (li, r, k) = there[rng.below(there.len() as u32) as usize];
            (key.x, key.y) = ((k % nx) as u16, (k / nx) as u16);
            above[li].put(key);
            keyed_above[li][r as usize] = true;
        } else {
            continue;
        }
        p.locks.insert(chosen[i], colour);
        locks.made += 1;
    }
    if let Some((passage, wide)) = p.passage {
        shortcuts(p, nx, ny, passage, wide, level_rooms, rng);
    }
}

/// The most rock a shortcut digs through (squares).
const SHORTCUT_ROCK: usize = 8;

/// Shortcuts on a built level (its locks and keys put): corridors through a little rock
/// (`SHORTCUT_ROCK` squares or less) between rooms near each other that lie 4 or more hops apart
/// on foot (`walk`), a few by the level's size, some behind a secret door at one end. None
/// through another room (`dig`), by a way in or out, or joining two parts the locks keep apart.
fn shortcuts(p: &mut Plan, nx: usize, ny: usize, passage: i16, wide: bool, rooms: usize, rng: &mut Pcg32) {
    let n = nx * ny;
    let mut most = (1 + rooms / 6).min(4);
    if p.loops < 1.0 {
        most = (most as f64 * p.loops).round() as usize;
    }
    let locked: Vec<(usize, usize)> = p.locks.keys().copied().collect();
    let shut = |q: (usize, usize)| locked.contains(&q);
    let kept = parts(p, ny, &shut).1;
    let outs = ways(p);
    let by_way = |k: usize| outs.iter().any(|&w| w == k || neighbours(w, nx, ny).any(|m| m == k));
    let kind = p.rooms[passage as usize].kind;
    let joins = |c: i16| c >= 0 && p.rooms[c as usize].kind != kind && p.rooms[c as usize].kind != BOSS;
    // For each room, the rooms within a little rock of it: (rock squares, a, b, a's square, b's square).
    let mut cands: Vec<(usize, i16, i16, usize, usize)> = Vec::new();
    let inner = |k: usize| k % nx >= 1 && k % nx + 1 < nx && k / nx >= 1 && k / nx + 1 < ny;
    for a in 0..p.rooms.len() as i16 {
        if !joins(a) {
            continue;
        }
        // Breadth first through rock from the room's walls: depth and the room square it set out from.
        let mut depth = vec![u32::MAX; n];
        let mut from = vec![usize::MAX; n];
        let mut queue = std::collections::VecDeque::new();
        for k in (0..n).filter(|&k| p.cells[k] == a && !p.taken[k]) {
            for m in neighbours(k, nx, ny) {
                if p.cells[m] < 0 && inner(m) && depth[m] == u32::MAX {
                    depth[m] = 1;
                    from[m] = k;
                    queue.push_back(m);
                }
            }
        }
        let mut best: Vec<Option<(usize, usize, usize)>> = vec![None; p.rooms.len()];
        while let Some(k) = queue.pop_front() {
            for m in neighbours(k, nx, ny) {
                let c = p.cells[m];
                if c < 0 {
                    if inner(m) && depth[m] == u32::MAX && (depth[k] as usize) < SHORTCUT_ROCK {
                        depth[m] = depth[k] + 1;
                        from[m] = from[k];
                        queue.push_back(m);
                    }
                } else if c > a && joins(c) && !p.taken[m] && best[c as usize].is_none() {
                    best[c as usize] = Some((depth[k] as usize, from[k], m));
                }
            }
        }
        cands.extend(best.into_iter().enumerate().filter_map(|(b, q)| q.map(|(d, ka, kb)| (d, a, b as i16, ka, kb))));
    }
    // Shorter first, with a little chance.
    let mut order: Vec<(f64, usize)> = cands.iter().enumerate().map(|(i, c)| (c.0 as f64 + 4.0 * rng.next_f64(), i)).collect();
    order.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let mut used: Vec<i16> = Vec::new();
    let mut made = 0;
    for (_, i) in order {
        if made >= most {
            break;
        }
        let (_, a, b, ka, kb) = cands[i];
        if used.contains(&a) || used.contains(&b) || near(p, ny, ka, kb, 4) {
            continue;
        }
        let before = walk(p, ny, ka, kb, u32::MAX).unwrap_or(u32::MAX);
        let (cells, doors) = (p.cells.clone(), p.doors.clone());
        let dug = dig(p, nx, ny, a, b, passage, wide);
        let fresh: Vec<usize> = (0..n).filter(|&k| cells[k] < 0 && p.cells[k] >= 0).collect();
        let new: Vec<(usize, usize)> = dug.into_iter().filter(|e| !doors.contains(e)).collect();
        let saved = before.saturating_sub(walk(p, ny, ka, kb, u32::MAX).unwrap_or(u32::MAX));
        let fine = !new.is_empty()
            && fresh.len() <= SHORTCUT_ROCK * if wide { 2 } else { 1 }
            && saved >= 16
            && !fresh.iter().chain(new.iter().flat_map(|e| [&e.0, &e.1])).any(|&k| by_way(k) || p.taken[k])
            && parts(p, ny, &shut).1 == kept;
        if !fine {
            p.cells = cells;
            p.doors = doors;
            continue;
        }
        used.extend([a, b]);
        made += 1;
        if rng.next_f64() < 0.4 && p.secret.len() < p.secrets {
            p.secret.insert(new[rng.below(new.len() as u32) as usize]);
        }
    }
}

// Terrain: water, chasms and rubble slopes (tools for the themed planners: temples flood with `water_at`).

/// A level as the terrain tools see it: its floor, the steps between squares, what is already there.
pub struct Ground {
    pub nx: usize,
    pub ny: usize,
    /// Room per square, -1 rock.
    pub cells: Vec<i16>,
    /// Where one arrives (the way in, else the way up).
    pub start: usize,
    /// Squares terrain may cover: floor with nothing on it, off doorways (so off the ways, keys and doors).
    pub free: Vec<bool>,
    /// Squares the items already there block.
    pub blocked: Vec<bool>,
    /// Steps east (k to k + 1) and south (k to k + nx), items aside: 0 none, 1 open, 2 through a locked door.
    east: Vec<u8>,
    south: Vec<u8>,
    /// A locked door's squares: each a repair source besides `start`.
    locked: Vec<usize>,
}

impl Ground {
    fn new(nx: usize, cells: Vec<i16>, start: usize, free: Vec<bool>, blocked: Vec<bool>, step: &dyn Fn(usize, usize) -> u8, locked: Vec<usize>) -> Ground {
        let n = cells.len();
        let east = (0..n).map(|k| if k % nx + 1 < nx { step(k, k + 1) } else { 0 }).collect();
        let south = (0..n).map(|k| if k + nx < n { step(k, k + nx) } else { 0 }).collect();
        Ground { nx, ny: n / nx, cells, start, free, blocked, east, south, locked }
    }

    /// A level being laid out.
    pub(crate) fn of_plan(p: &Plan) -> Ground {
        let step = |a: usize, b: usize| if !p.passable(a, b) { 0 } else if p.locks.contains_key(&(a, b)) { 2 } else { 1 };
        let free = (0..p.cells.len()).map(|k| p.cells[k] >= 0 && !p.taken[k]).collect();
        let mut locked: Vec<usize> = p.locks.keys().flat_map(|&(a, b)| [a, b]).collect();
        locked.sort_unstable();
        Ground::new(p.nx, p.cells.clone(), p.up, free, p.blocked.clone(), &step, locked)
    }

    /// A built level (`nx` × `ny`).
    pub fn of_level(lv: &Level, nx: usize, ny: usize) -> Ground {
        let doors: crate::core::hash::FastMap<(usize, usize), u8> = lv.doors.iter().map(|d| (super::design::door_pair(d, nx), d.lock)).collect();
        let step = |a: usize, b: usize| {
            let (ra, rb) = (lv.cells[a], lv.cells[b]);
            match doors.get(&(a, b)) {
                _ if ra < 0 || rb < 0 => 0,
                Some(&l) if l != 0 => 2,
                Some(_) => 1,
                None => (ra == rb || lv.natural) as u8,
            }
        };
        let mut free: Vec<bool> = lv.cells.iter().map(|&c| c >= 0).collect();
        for f in &lv.furniture {
            for j in f.y as usize..((f.y + f.h) as usize).min(ny) {
                for i in f.x as usize..((f.x + f.w) as usize).min(nx) {
                    free[j * nx + i] = false;
                }
            }
        }
        for &(a, b) in doors.keys() {
            free[a] = false;
            free[b] = false;
        }
        let mut locked: Vec<usize> = doors.iter().filter(|d| *d.1 != 0).flat_map(|(&(a, b), _)| [a, b]).collect();
        locked.sort_unstable();
        let start = lv.furniture.iter().find(|f| f.kind == "exit").or_else(|| lv.furniture.iter().find(|f| f.kind == "up")).map_or(0, |f| f.y as usize * nx + f.x as usize);
        Ground::new(nx, lv.cells.clone(), start, free, blocked_squares(&lv.furniture, nx, ny), &step, locked)
    }

    /// The squares one steps to from `k` (through locked doors if `open`).
    fn steps(&self, k: usize, open: bool) -> impl Iterator<Item = usize> + '_ {
        let (nx, ok) = (self.nx, move |s: u8| s == 1 || open && s == 2);
        [(!k.is_multiple_of(nx) && ok(self.east[k - 1])).then(|| k - 1), ok(self.east[k]).then(|| k + 1), (k >= nx && ok(self.south[k - nx])).then(|| k - nx), ok(self.south[k]).then(|| k + nx)]
            .into_iter()
            .flatten()
    }

    /// Squares reached from `from`, never onto `stop` ones.
    fn reach(&self, from: usize, open: bool, stop: &dyn Fn(usize) -> bool) -> Vec<bool> {
        let mut seen = vec![false; self.cells.len()];
        if self.cells[from] < 0 {
            return seen;
        }
        seen[from] = true;
        let mut todo = vec![from];
        while let Some(k) = todo.pop() {
            for q in self.steps(k, open) {
                if !seen[q] && !stop(q) {
                    seen[q] = true;
                    todo.push(q);
                }
            }
        }
        seen
    }

    /// Floor squares nothing blocks that can't be reached from the way onto the level, every door open (none once the
    /// tools have laid their terrain on a level that had none).
    pub fn unreached(&self) -> usize {
        let seen = self.reach(self.start, true, &|k| self.blocked[k]);
        (0..self.cells.len()).filter(|&k| self.cells[k] >= 0 && !self.blocked[k] && !seen[k]).count()
    }
}

/// How much water a theme has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wetness {
    Dry,
    Wet,
    Flooded,
}

impl Wetness {
    /// The noise heights over which a square is shallow and deep water (`water_at`; over 1: never).
    pub fn bands(self) -> (f64, f64) {
        match self {
            Wetness::Dry => (2.0, 2.0),
            Wetness::Wet => (0.62, 0.72),
            Wetness::Flooded => (0.42, 0.56),
        }
    }
}

/// What carries one across a chasm: a rope bridge (straight, one square wide) or stepping stones.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    Bridge,
    Stones,
}

/// Terrain per square while it is laid: `t` what lies there (`LAID`), `span` what lies over it (`SPANNED`).
struct Layer {
    t: Vec<u8>,
    span: Vec<u8>,
}
const LAID: [&str; 5] = ["", "water_shallow", "water_deep", "chasm", "rubble_slope"];
const SPANNED: [&str; 3] = ["", "rope_bridge", "stepping_stone"];
const SHALLOW: u8 = 1;
const DEEP: u8 = 2;
const CHASM: u8 = 3;
const SLOPE: u8 = 4;
const BRIDGE: u8 = 1;
const STONES: u8 = 2;

impl Layer {
    fn new(n: usize) -> Layer {
        Layer { t: vec![0; n], span: vec![0; n] }
    }

    /// Whether square `k` is deep water or chasm with nothing over it.
    fn gap(&self, k: usize) -> bool {
        (self.t[k] == DEEP || self.t[k] == CHASM) && self.span[k] == 0
    }

    /// Make every square reached before the terrain reachable again, from the way onto the level and from each side of
    /// each locked door, locks shut (so each stretch a lock closes off stays whole and the keys' order holds): on the
    /// crossing over the fewest gap squares, then the fewest steps, deep water turns shallow (two squares or fewer) or
    /// gets stepping stones; deep water (wider) and a chasm get a rope bridge where the crossing runs straight and
    /// `bridge`, else stones.
    fn repair(&mut self, g: &Ground, bridge: bool) {
        let n = g.cells.len();
        for from in std::iter::once(g.start).chain(g.locked.iter().copied()) {
            if g.cells[from] < 0 || g.blocked[from] {
                continue;
            }
            let base = g.reach(from, false, &|k| g.blocked[k]);
            for _ in 0..n {
                let seen = g.reach(from, false, &|k| g.blocked[k] || self.gap(k));
                if (0..n).all(|k| !base[k] || seen[k] || self.gap(k)) {
                    break;
                }
                // (Cost: 1024 a gap square, 1 a step.)
                let mut cost = vec![u32::MAX; n];
                let mut came = vec![usize::MAX; n];
                let mut heap = std::collections::BinaryHeap::new();
                for k in (0..n).filter(|&k| seen[k]) {
                    cost[k] = 0;
                    heap.push(std::cmp::Reverse((0u32, k)));
                }
                let mut end = None;
                while let Some(std::cmp::Reverse((c, k))) = heap.pop() {
                    if c > cost[k] {
                        continue;
                    }
                    if !seen[k] && !self.gap(k) {
                        end = Some(k);
                        break;
                    }
                    for q in g.steps(k, false) {
                        let c = c + if self.gap(q) { 1024 } else { 1 };
                        if base[q] && c < cost[q] {
                            cost[q] = c;
                            came[q] = k;
                            heap.push(std::cmp::Reverse((c, q)));
                        }
                    }
                }
                let Some(mut k) = end else { break };
                let mut path = Vec::new();
                while !seen[k] {
                    if self.gap(k) {
                        path.push(k);
                    }
                    k = came[k];
                }
                let straight = path.iter().all(|&q| q % g.nx == path[0] % g.nx) || path.iter().all(|&q| q / g.nx == path[0] / g.nx);
                for &q in &path {
                    match self.t[q] {
                        DEEP if path.len() <= 2 => self.t[q] = SHALLOW,
                        DEEP | CHASM if bridge && straight => self.span[q] = BRIDGE,
                        _ => self.span[q] = STONES,
                    }
                }
            }
        }
    }

    /// The terrain as items, each kind merged into rectangles; spans after what they lie over.
    fn items(&self, nx: usize) -> Vec<Item> {
        let mut out = Vec::new();
        for (grid, names) in [(&self.t, &LAID[..]), (&self.span, &SPANNED[..])] {
            for (code, kind) in names.iter().enumerate().skip(1) {
                for (x, y, w, h) in rects(grid, code as u8, nx) {
                    out.push(item(kind, x, y, w, h));
                }
            }
        }
        out
    }
}

/// Rectangles covering the squares of `grid` (rows of `nx`) that hold `code`, greedily: a row's run, then down while
/// the rows below hold it whole.
fn rects(grid: &[u8], code: u8, nx: usize) -> Vec<(usize, usize, usize, usize)> {
    let ny = grid.len() / nx;
    let mut used = vec![false; grid.len()];
    let mut out = Vec::new();
    for j in 0..ny {
        for i in 0..nx {
            if grid[j * nx + i] != code || used[j * nx + i] {
                continue;
            }
            let w = (i..nx).take_while(|&x| grid[j * nx + x] == code && !used[j * nx + x]).count();
            let h = (j..ny).take_while(|&y| (i..i + w).all(|x| grid[y * nx + x] == code && !used[y * nx + x])).count();
            for y in j..j + h {
                for x in i..i + w {
                    used[y * nx + x] = true;
                }
            }
            out.push((i, j, w, h));
        }
    }
    out
}

/// Value noise in 0..1 at square (i, j): lattice hashes `cell` squares apart, smoothed, and an octave at half that.
fn noise(seed: u64, i: usize, j: usize, cell: f64) -> f64 {
    let (mut sum, mut amp, mut c) = (0.0, 1.0, cell);
    for o in 0..2 {
        let (x, y) = ((i as f64 + 0.5) / c, (j as f64 + 0.5) / c);
        let (xi, yi) = (x as i64, y as i64);
        let s = |t: f64| t * t * (3.0 - 2.0 * t);
        let (sx, sy) = (s(x - xi as f64), s(y - yi as f64));
        let v = |a: i64, b: i64| crate::core::rng::unit(hash3(seed, a, b, o));
        let top = v(xi, yi) + (v(xi + 1, yi) - v(xi, yi)) * sx;
        let bottom = v(xi, yi + 1) + (v(xi + 1, yi + 1) - v(xi, yi + 1)) * sx;
        sum += (top + (bottom - top) * sy) * amp;
        amp *= 0.5;
        c *= 0.5;
    }
    sum / 1.5
}

/// Water over a level: a noise height (`seed`, the square's coordinates) over the free floor, shallow above one level
/// and deep above another by `wet` (none if dry); then one repair (`Layer::repair`) so deep water cuts nothing off.
/// The items (shallow and deep water, stepping stones) are merged into rectangles, to be put down in bulk
/// (`Plan::put_all`, or onto a level's furniture).
pub fn water(g: &Ground, wet: Wetness, seed: u64) -> Vec<Item> {
    let bands = wet.bands();
    if bands.0 > 1.0 {
        return Vec::new();
    }
    water_at(g, seed, Span::Stones, &|_| bands)
}

/// `water` with bands of its own per square: `at(k)` the noise heights (0..1) over which square k is shallow and deep
/// water (over 1: never), so a planner floods its big halls deep and the rest shallow, keeps ledges dry along walls
/// (`grow::flood_room`). The repair lays a rope bridge over deep water where the crossing runs straight and `span`
/// says bridge, else stepping stones (or wades two squares or fewer).
pub fn water_at(g: &Ground, seed: u64, span: Span, at: &dyn Fn(usize) -> (f64, f64)) -> Vec<Item> {
    let mut l = Layer::new(g.cells.len());
    for k in (0..g.cells.len()).filter(|&k| g.free[k]) {
        let (shallow, deep) = at(k);
        if shallow > 1.0 {
            continue;
        }
        let h = noise(seed, k % g.nx, k / g.nx, 6.0);
        l.t[k] = if h >= deep { DEEP } else if h >= shallow { SHALLOW } else { 0 };
    }
    l.repair(g, span == Span::Bridge);
    l.items(g.nx)
}

/// A chasm across the floor of the level (or of room `room`): a band 1-3 squares wide wobbling across its longer
/// side, crossed straight by rope bridges (one per 14 squares of its length, 3 or more apart) or stepping stones as
/// `span` says; then one repair (`Layer::repair`) so it cuts nothing off.
pub fn chasm(g: &Ground, room: Option<i16>, span: Span, seed: u64) -> Vec<Item> {
    chasm_in(g, room, span, seed, true)
}

/// A chasm as `chasm` lays it; without `crossings` none are picked (the planner kept a way over it off the free
/// squares: a stone bridge), the repair alone bridging whatever it still cuts off.
pub(crate) fn chasm_in(g: &Ground, room: Option<i16>, span: Span, seed: u64, crossings: bool) -> Vec<Item> {
    let (nx, ny) = (g.nx, g.ny);
    let inside = |k: usize| g.cells[k] >= 0 && room.is_none_or(|r| g.cells[k] == r);
    let squares: Vec<usize> = (0..nx * ny).filter(|&k| inside(k)).collect();
    if squares.is_empty() {
        return Vec::new();
    }
    let (x0, x1) = (squares.iter().map(|k| k % nx).min().unwrap(), squares.iter().map(|k| k % nx).max().unwrap());
    let (y0, y1) = (squares.iter().map(|k| k / nx).min().unwrap(), squares.iter().map(|k| k / nx).max().unwrap());
    // The band runs along `t` (y, if the floor is wider than deep), across it at `a`.
    let along_y = x1 - x0 >= y1 - y0;
    let ((t0, t1), (a0, a1)) = if along_y { ((y0, y1), (x0, x1)) } else { ((x0, x1), (y0, y1)) };
    let at = |t: usize, a: usize| if along_y { t * nx + a } else { a * nx + t };
    let mid = (a0 + a1) as f64 * 0.5 + (crate::core::rng::unit(hash3(seed, 0, 0, 7)) - 0.5) * (a1 - a0) as f64 / 3.0;
    let mut l = Layer::new(nx * ny);
    let mut runs = Vec::new();
    for t in t0..=t1 {
        let c = mid + (noise(seed, t, 0, 5.0) - 0.5) * ((a1 - a0) as f64 / 3.0).min(6.0);
        let w = 1 + ((noise(seed ^ 0x5bd1_e995, t, 1, 4.0) * 3.0) as usize).min(2);
        let lo = (c - (w as f64 - 1.0) * 0.5 + 0.5).max(0.0) as usize;
        for a in lo.max(a0)..(lo + w).min(a1 + 1) {
            if g.free[at(t, a)] && inside(at(t, a)) {
                l.t[at(t, a)] = CHASM;
            }
        }
        // A straight crossing: one run of chasm with open floor at each end, stepped across.
        let laid: Vec<usize> = (a0..=a1).filter(|&a| l.t[at(t, a)] == CHASM).collect();
        let (Some(&s), Some(&e)) = (laid.first(), laid.last()) else { continue };
        let ok = |k: usize| g.cells[k] >= 0 && !g.blocked[k] && l.t[k] != CHASM;
        let step = |a: usize, b: usize| g.steps(at(t, a), false).any(|q| q == at(t, b));
        if e + 1 - s == laid.len() && s > a0 && e < a1 && ok(at(t, s - 1)) && ok(at(t, e + 1)) && (s - 1..=e).all(|a| step(a, a + 1)) {
            runs.push((t, s, e));
        }
    }
    let count = if crossings { 1 + (t1 - t0) / 14 } else { 0 };
    let mut picked: Vec<usize> = Vec::new();
    for b in 0..count {
        let want = t0 as f64 + (b as f64 + 0.5) * (t1 - t0 + 1) as f64 / count as f64;
        let best = runs.iter().filter(|r| picked.iter().all(|&p| r.0.abs_diff(p) >= 3)).min_by(|p, q| (p.0 as f64 - want).abs().total_cmp(&(q.0 as f64 - want).abs()));
        if let Some(&(t, s, e)) = best {
            picked.push(t);
            for a in s..=e {
                l.span[at(t, a)] = if span == Span::Bridge { BRIDGE } else { STONES };
            }
        }
    }
    l.repair(g, span == Span::Bridge);
    l.items(nx)
}

/// A rubble slope over the free squares of the box (x, y, w, h) inside the ellipse it holds (a breach's spill),
/// merged into rectangles. It never blocks.
pub fn rubble_slope(g: &Ground, x: usize, y: usize, w: usize, h: usize) -> Vec<Item> {
    let mut l = Layer::new(g.cells.len());
    let (cx, cy, rx, ry) = (x as f64 + w as f64 * 0.5, y as f64 + h as f64 * 0.5, w as f64 * 0.5, h as f64 * 0.5);
    for j in y..(y + h).min(g.ny) {
        for i in x..(x + w).min(g.nx) {
            let (dx, dy) = ((i as f64 + 0.5 - cx) / rx, (j as f64 + 0.5 - cy) / ry);
            if g.free[j * g.nx + i] && dx * dx + dy * dy <= 1.0 {
                l.t[j * g.nx + i] = SLOPE;
            }
        }
    }
    l.items(g.nx)
}
