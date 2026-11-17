//! Placement checks: detail of created places, spots for new places and buildings,
//! blockers and clear-spot searches.

use super::*;

/// What a created site is: "created ruin over a crypt (tomb, large, 3 levels)".
pub fn created_detail(c: &crate::world::Created) -> String {
    if c.kind == "building" {
        // "created building (inn, 2 floors, battlements, slate roof)"
        let what = c.func.as_deref().map(|f| town::catalog::index_of(f).map_or(f.replace('_', " "), |i| town::catalog::CATALOG[i].name.to_string()));
        let floors = c.floors.map(|f| format!("{f} floor{}", if f == 1 { "" } else { "s" }));
        let roof = c.roof.as_ref().map(|r| if r == "hip" { "hip roof".to_string() } else { r.clone() });
        let tint = c.tint.as_ref().map(|t| format!("{t} roof"));
        let material = c.material.as_ref().map(|m| format!("{} roof", m.replace('_', " ")));
        let shape = c.shape.as_ref().map(|r| format!("{r} roof shape"));
        let shape = match (shape, c.shade) {
            (s, Some(n)) => Some(format!("{}colour {n}", s.map_or(String::new(), |s| format!("{s}, ")))),
            (s, None) => s,
        };
        let ruin = (c.structure.as_deref() == Some("ruin")).then(|| "ruined".to_string());
        let opts: Vec<String> = [what, floors, ruin, roof, tint, material, shape].into_iter().flatten().collect();
        return if opts.is_empty() { "created building".into() } else { format!("created building ({})", opts.join(", ")) };
    }
    if c.kind == "castle" {
        // "created castle (ruined, no keep, gate on side 2)"
        let opts: Vec<String> = [
            (c.structure.as_deref() == Some("ruin")).then(|| "ruined".to_string()),
            (c.keep == Some(false)).then(|| "no keep".to_string()),
            (c.yard_buildings == Some(false)).then(|| "an empty yard".to_string()),
            match c.castle_gates().as_slice() {
                [] => None,
                [g] => Some(format!("gate on side {g}")),
                gs => Some(format!("gates on sides {}", gs.iter().map(|g| g.to_string()).collect::<Vec<_>>().join(", "))),
            },
            c.keep_shape.as_deref().or((!c.keep_poly.is_empty()).then_some("drawn")).map(|k| format!("a {k} keep")),
            c.moat.as_deref().map(|m| if m == "none" { "no moat".to_string() } else { format!("a {m} moat") }),
        ]
        .into_iter()
        .flatten()
        .collect();
        return if opts.is_empty() { "created castle".into() } else { format!("created castle ({})", opts.join(", ")) };
    }
    if c.kind == "wall" {
        // "created wall (a ring, 1200 ft, 2 gates)"
        let n = c.pts.len();
        let sides = if c.closed { n } else { n.saturating_sub(1) };
        let length: f64 = (0..sides).map(|i| geom::dist(c.pts[i], c.pts[(i + 1) % n])).sum();
        let gates = match c.gates.len() {
            0 => String::new(),
            1 => ", a gate".to_string(),
            g => format!(", {g} gates"),
        };
        return format!("created wall ({}{} ft{gates})", if c.closed { "a ring, " } else { "" }, length.round());
    }
    let mut s = if c.kind == "village" || c.kind == "town" { format!("{} (added by hand)", c.kind) } else { format!("created {}", c.kind.replace('_', " ")) };
    if let Some(u) = &c.under {
        s += &format!(" over a {}", u.replace('_', " "));
    }
    if let Some(f) = c.fill.as_ref().filter(|_| c.kind == "plot") {
        use crate::town::hand::BlockFill;
        let open = |o: &crate::town::hand::OpenKind| serde_json::to_value(o).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
        s += &match f {
            BlockFill::Generated { ward, lots, .. } => format!(" (generated lots, {ward} ward{})", lots.as_ref().map(|l| format!(", {l} lots")).unwrap_or_default()),
            BlockFill::Hand => " (left for buildings by hand)".to_string(),
            BlockFill::Open { open: o, .. } => format!(" (open ground: {})", open(o)),
            BlockFill::Castle { .. } => " (castle)".to_string(),
        };
    }
    let opts: Vec<String> = [c.theme.as_ref().map(|t| t.replace('_', " ")), c.size.clone(), c.levels.map(|l| format!("{l} level{}", if l == 1 { "" } else { "s" }))].into_iter().flatten().collect();
    if !opts.is_empty() {
        s += &format!(" ({})", opts.join(", "));
    }
    s
}

/// Where a site of `kind` can be created near `p`: on dry land (as the map draws the shore)
/// inside the map, stepped out of river channels (as generated sites are); with a name for it
/// (the nearest settlement's naming culture's), seeded by its id. The app and mapd both place sites through this.
pub fn creation_spot(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2]) -> Result<([f64; 2], String), String> {
    creation_spot_with(world, t0, kind, under, id, p, Overlap::Refuse)
}

/// What `creation_spot_with` does about what stands where the site would.
#[derive(Clone, Copy, PartialEq)]
pub enum Overlap {
    /// Refuse, naming the blocker with its id and where it is, and the nearest clear spot.
    Refuse,
    /// Refuse in a person's words: no ids, no points in world ft (the app).
    RefuseShort,
    /// No check (an entrance in a building).
    Skip,
}

/// How far a point site of `kind` reaches from its point (ft): the largest extent of what `town::sites::generate` lays out
/// (tower and its annex, camp ring and cart, the inn, stables and yard of a waystation, a ruin's shells, a mine's shed and yard).
pub fn site_reach(kind: &str) -> f64 {
    match kind {
        "tower" => 65.0,
        "camp" => 55.0,
        "waystation" => 120.0,
        "ruin" => 115.0,
        "mine" => 55.0,
        "cave" | "lava_tube" => 12.0,
        _ => 10.0,
    }
}

/// What a site of `kind` at `p` stands on that it must not: buildings, castles and walls, and (all but a bare entrance, which may sit in a
/// street or a square) streets and squares; the one nearest `p`, with the nearest clear spot's move.
fn site_overlap(world: &World, t0: &T0, kind: &str, p: P) -> Option<(Blocker, Option<P>)> {
    let disc = geom::circle(p, site_reach(kind), 16);
    let free = kind == "entrance";
    let (found, _) = spot_blockers(world, t0, &disc, None, false).ok()?;
    let b = found.into_iter().filter(|b| matches!(b.what, "building" | "wall") || (!free && matches!(b.what, "street" | "square"))).min_by(|a, b| geom::dist(a.at, p).total_cmp(&geom::dist(b.at, p)))?;
    Some((b, nearest_clear(world, t0, &disc, None, "", None, false).map(|r| r.1)))
}

/// Where an entrance taken out of its building may stand: the spot nearest `from` (the building's middle) that a free-standing entrance
/// may take (`creation_spot`: dry, clear of buildings, castles and walls; a street or a square is fine), on 5-ft steps to 100 ft.
pub fn entrance_out_spot(world: &World, t0: &T0, from: P) -> Option<P> {
    let mut offsets: Vec<(i32, i32)> = (-20..=20).flat_map(|i| (-20..=20).map(move |j| (i, j))).filter(|&(i, j)| i * i + j * j <= 400).collect();
    offsets.sort_by_key(|&(i, j)| (i * i + j * j, i, j));
    offsets.into_iter().find_map(|(i, j)| {
        let q = [from[0] + 5.0 * i as f64, from[1] + 5.0 * j as f64];
        creation_spot_with(world, t0, "entrance", None, "", q, Overlap::Refuse).ok().filter(|(r, _)| geom::dist(*r, q) < 1.0).map(|(r, _)| r)
    })
}

/// `creation_spot`, with `overlap` saying what to do about what stands there.
pub fn creation_spot_with(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2], overlap: Overlap) -> Result<([f64; 2], String), String> {
    creation_spot_inner(world, t0, kind, under, id, p, overlap, true)
}

/// The nearest spot beyond the 400 ft `nearest_clear` covers where a site of `kind` may stand (rings 150 ft apart out to 1,500 ft, points 150 ft apart round
/// each): the move from `p`. What stands in the way (buildings, walls; all but an entrance also streets and squares) is gathered once with
/// bounding boxes, so only the points clear of it go through the full check.
fn farther_clear(world: &World, t0: &T0, kind: &str, p: P) -> Option<P> {
    let reach = site_reach(kind) + 2.0;
    let region = [p[0] - 1500.0 - reach, p[1] - 1500.0 - reach, p[0] + 1500.0 + reach, p[1] + 1500.0 + reach];
    let near = |b: [f64; 4]| b[2] >= region[0] && b[0] <= region[2] && b[3] >= region[1] && b[1] <= region[3];
    let free = kind == "entrance";
    let mut areas: Vec<([f64; 4], Vec<P>)> = Vec::new();
    let mut lines: Vec<([f64; 4], P, P, f64)> = Vec::new();
    for l in town::layouts_near(world, t0, region) {
        let plazas = l.plazas.iter().filter(|q| !free && q.kind != town::PlazaKind::Bailey).map(|q| &q.pts);
        for pts in l.buildings.iter().map(|b| &b.poly).chain(plazas) {
            let bb = bbox(pts);
            if near(bb) {
                areas.push((bb, pts.clone()));
            }
        }
        let streets = l.roads.iter().filter(|_| !free).map(|(pts, _, w)| (pts, 0.5 * w));
        for (pts, half) in streets.chain(l.walls.iter().map(|w| (w, 4.5))) {
            for s in pts.windows(2) {
                let b = bbox(s);
                let bb = [b[0] - half, b[1] - half, b[2] + half, b[3] + half];
                if near(bb) {
                    lines.push((bb, s[0], s[1], half));
                }
            }
        }
    }
    let hits = |a: [f64; 4], b: [f64; 4]| a[2] >= b[0] && a[0] <= b[2] && a[3] >= b[1] && a[1] <= b[3];
    let mut r = 450.0;
    while r <= 1500.0 {
        let n = ((std::f64::consts::TAU * r / 150.0) as usize).max(12);
        for k in 0..n {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            let q = [crate::core::round(p[0] + r * libm::cos(a)), crate::core::round(p[1] + r * libm::sin(a))];
            let disc = inner_of(&geom::circle(q, site_reach(kind), 16));
            let db = bbox(&disc);
            let blocked = areas.iter().any(|(b, pts)| hits(*b, db) && polys_overlap(&disc, pts)) || lines.iter().any(|(b, s0, s1, half)| hits(*b, db) && poly_seg_dist(&disc, *s0, *s1) < *half);
            if blocked {
                continue;
            }
            if let Ok((s, _)) = creation_spot_inner(world, t0, kind, None, "", q, Overlap::Refuse, false)
                && geom::dist(s, q) < 1.0
            {
                return Some([q[0] - p[0], q[1] - p[1]]);
            }
        }
        r += 150.0;
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn creation_spot_inner(world: &World, t0: &T0, kind: &str, under: Option<&str>, id: &str, p: [f64; 2], overlap: Overlap, farther: bool) -> Result<([f64; 2], String), String> {
    use crate::t0::names::{NameKind, Namer};
    use crate::under::UnderKind;
    if p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft {
        return Err("that place is off the map".to_string());
    }
    // (On an islet or a plateau over the water it stays where it was put: that is its ground.)
    let on_islet = !world.file.edits.plateaus.is_empty() && {
        let sh = town::Shore::new(world, t0, [p[0] - 1.0, p[1] - 1.0, p[0] + 1.0, p[1] + 1.0]);
        sh.on_islet(p) || sh.raised(p)
    };
    let (x, y) = if on_islet { (p[0], p[1]) } else { crate::lod::rivers::clear_of_rivers(&t0.rivers, p[0], p[1], 150.0, t0.cell_ft) };
    let mut p = [x, y];
    if kind == "camp" {
        // A camp's clearing lies within one battlemap chunk (so it is levelled to the ground
        // there): kept 70 ft from the chunk's edges.
        let size = world.geom.tile_size_ft(world.geom.max_level);
        p = p.map(|v| crate::core::floor(v / size) * size + (v - crate::core::floor(v / size) * size).clamp(70.0, size - 70.0));
    }
    // Sea or lake as the map draws the shore (`town::Shore`, as the site is then built).
    if town::Shore::new(world, t0, [p[0] - 1.0, p[1] - 1.0, p[0] + 1.0, p[1] + 1.0]).wet(p) {
        return Err("that place is under water: pick dry land".to_string());
    }
    if overlap != Overlap::Skip {
        if let Some((b, near)) = site_overlap(world, t0, kind, p) {
            let short = overlap == Overlap::RefuseShort;
            let what = if short { b.short() } else { b.message(false) };
            let ft = |d: P| crate::core::round(geom::len(d));
            return Err(match near {
                Some(d) if short => format!("{what}; the nearest clear spot is {} ft away", ft(d)),
                Some(d) => format!("{what}; the nearest clear spot is ({}, {}), {} ft away", crate::core::round(p[0] + d[0]), crate::core::round(p[1] + d[1]), ft(d)),
                None => {
                    // Built over: a tower can be a building (its ground cleared), anything else goes outside the built-up area.
                    let far = if farther && !short { farther_clear(world, t0, kind, p) } else { None };
                    let tower = if kind == "tower" { "; for a tower here, create_building with func wizard_tower and clear: true takes away what is in the way" } else { "" };
                    match far {
                        Some(d) if short => format!("{what}; no clear spot within 400 ft, the nearest is {} ft away{tower}", ft(d)),
                        Some(d) => format!("{what}; no clear spot within 400 ft, the nearest is ({}, {}), {} ft away{tower}", crate::core::round(p[0] + d[0]), crate::core::round(p[1] + d[1]), ft(d)),
                        None if farther && !short => format!("{what}; no clear spot within 1,500 ft (the area is built up): put it outside the built-up area{tower}"),
                        None => format!("{what}; no clear spot within 400 ft"),
                    }
                }
            });
        }
    }
    let culture = t0.settlements.iter().min_by(|a, b| (a.x - p[0]).hypot(a.y - p[1]).total_cmp(&(b.x - p[0]).hypot(b.y - p[1]))).map(|s| s.culture as usize).unwrap_or(0);
    let nk = match (kind, under.and_then(UnderKind::parse)) {
        ("tower", _) => NameKind::Tower,
        ("camp", _) => NameKind::Camp,
        ("waystation", _) => NameKind::Settlement,
        ("cave", _) | ("entrance", Some(UnderKind::Cave)) => NameKind::Cave,
        ("mine", _) | ("entrance", Some(UnderKind::Mine)) => NameKind::Mine,
        ("lava_tube", _) | ("entrance", Some(UnderKind::LavaTube)) => NameKind::LavaTube,
        _ => NameKind::Ruin,
    };
    let mut namer = Namer::new(crate::core::hash::fnv64(id.as_bytes()) ^ world.seed);
    // (A mine or a lava tube has the one theme.)
    namer.theme = match nk {
        NameKind::Mine => Some("mine"),
        NameKind::LavaTube => Some("lava_tube"),
        _ => None,
    };
    // (Named apart from the world's features, which it never changes: `Namer` is its own.)
    let mut n = namer.name(nk, culture);
    for _ in 0..8 {
        if !named(t0, &n) {
            break;
        }
        n = namer.name(nk, culture);
    }
    Ok((p, if kind == "waystation" { format!("{n} Inn") } else { n }))
}

/// Whether one of the world's features (as generated) is named `n`.
pub(super) fn named(t0: &T0, n: &str) -> bool {
    t0.extra.as_ref().is_some_and(|e| e.overlay.features.iter().any(|f| f.name == n))
}

/// The bounding box (x0, y0, x1, y1) of `pts`.
pub(super) fn bbox(pts: &[P]) -> [f64; 4] {
    pts.iter().fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])])
}

/// The most blockers `spot_blockers` lists (a longer scan is cut and says so).
const BLOCKER_CAP: usize = 300;

/// One thing in the way of a footprint (`spot_blockers`).
#[derive(Clone, Debug)]
pub struct Blocker {
    /// `building`, `square`, `street`, `wall`, `road`, `water` or `river`.
    pub what: &'static str,
    /// A building's (`b:<layout>:<id>`, or `c:<n>` when drawn by hand), a hand-drawn wall's `c:` id, a road's `rd:` id, a river's.
    pub id: Option<String>,
    pub name: Option<String>,
    /// A building's trade or home.
    pub function: Option<String>,
    /// `{id, name}` of the settlement it lies in, or null.
    pub settlement: Value,
    /// The name of the layout it lies in (a settlement or a site), for the messages.
    pub place: Option<String>,
    /// Where it is: for a building its middle, for a square, street, wall or road the point of it nearest the footprint's middle.
    pub at: P,
    /// A street's, wall's or road's width, ft.
    pub width: Option<f64>,
    /// A square's, street's or road's kind; water's: `sea` or `lake`.
    pub kind: Option<&'static str>,
    /// One of the world's own buildings (`clear` takes it away).
    pub removable: bool,
}

impl Blocker {
    fn plain(what: &'static str, at: P) -> Blocker {
        Blocker { what, id: None, name: None, function: None, settlement: Value::Null, place: None, at, width: None, kind: None, removable: false }
    }

    /// The refusal for it. With `clear`: what a building that stays is told.
    pub fn message(&self, clear: bool) -> String {
        self.text(clear, false)
    }

    /// The refusal for a person: no ids, no world-ft points.
    pub fn short(&self) -> String {
        let t = self.text(false, true);
        let mut cs = t.chars();
        cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or(t)
    }

    fn text(&self, clear: bool, short: bool) -> String {
        let at = if short { String::new() } else { format!(" at ({}, {})", crate::core::round(self.at[0]), crate::core::round(self.at[1])) };
        let of = self.place.as_deref().map(|p| format!(" of {p}")).unwrap_or_default();
        let id = self.id.as_deref().unwrap_or_default();
        let with_id = if short || id.is_empty() { String::new() } else { format!(" ({id})") };
        match self.what {
            "water" => format!("that is in {}: draw it on dry land", if self.kind == Some("lake") { "a lake" } else { "the sea" }),
            "river" => match &self.name {
                Some(n) => format!("that is in the river {n}{with_id}: draw it on the bank"),
                None => "that is in a river: draw it on the bank".to_string(),
            },
            "building" => {
                let inn = self.place.as_deref().map(|p| format!(" in {p}")).unwrap_or_default();
                let mut m = format!("that overlaps {}{with_id}{inn}{}", self.name.as_deref().unwrap_or("a building"), at.replacen(" at", ", at", 1));
                if clear && !self.removable {
                    m += &format!("; it is drawn by hand: delete_feature {id} first");
                }
                m
            }
            // (R5 M20: a footprint on a hand bridge's deck over its lane.)
            "lane" if short => "that is on the bridge's lane: keep its middle clear".to_string(),
            "lane" => format!("that is on the lane of bridge {id} (keep {:.0} ft clear down its middle){at}", self.width.unwrap_or(0.0)),
            "square" if self.kind == Some("yard") => format!("that is on the yard{of}{at}"),
            "square" => format!("that is on a square ({}){of}{at}", self.kind.unwrap_or("square")),
            "street" => {
                let k = self.kind.unwrap_or("street");
                let w = crate::core::round(self.width.unwrap_or(0.0));
                let what = match k {
                    "main street" => "the main street".to_string(),
                    "approach road" => "an approach road".to_string(),
                    _ => "a street".to_string(),
                };
                format!("that is on {what} ({w} ft wide){of}{at}")
            }
            "wall" if self.id.is_some() => format!("that is on the wall {}{with_id}{at}", self.name.as_deref().unwrap_or("")),
            "wall" => format!(
                "that is on {}wall{}{at}",
                self.place.as_deref().map(|p| format!("{p}'s ")).unwrap_or_else(|| "a ".to_string()),
                if self.place.is_some() { "" } else { " of a settlement" }
            ),
            _ => {
                let k = self.kind.unwrap_or("road");
                let k = if k.starts_with("king") { "King's road" } else { k };
                format!("that is on the {k}{}{at}", if id.is_empty() || short { String::new() } else { format!(" {id}") })
            }
        }
    }

    /// `{what, id?, name?, function?, settlement?, at: [x, y], width_ft?, kind?}`, points in whole ft.
    pub fn to_json(&self) -> Value {
        let mut v = json!({ "what": self.what, "at": [crate::core::round(self.at[0]), crate::core::round(self.at[1])] });
        for (k, o) in [("id", &self.id), ("name", &self.name), ("function", &self.function)] {
            if let Some(s) = o {
                v[k] = json!(s);
            }
        }
        if !self.settlement.is_null() {
            v["settlement"] = self.settlement.clone();
        }
        if let Some(w) = self.width {
            v["width_ft"] = json!(crate::core::round(w));
        }
        if let Some(k) = self.kind {
            v["kind"] = json!(k);
        }
        v
    }
}

/// One of the world's own buildings taken away by `clear` (as `remove_buildings` does): its id, name, trade and where its edit is.
#[derive(Clone, Debug)]
pub struct Removal {
    pub id: String,
    pub name: String,
    pub function: String,
    pub at: P,
}

impl Removal {
    /// `{id, name, function}` (the one shape of a removal in every reply).
    pub fn to_json(&self) -> Value {
        json!({ "id": self.id, "name": self.name, "function": self.function })
    }
}

/// The removals the buildings in `found` that `clear` takes away come to (each with where its edit is: the building as generated).
pub fn removals(world: &World, t0: &T0, found: &[Blocker]) -> Vec<Removal> {
    let mut bases: FastMap<usize, std::rc::Rc<town::Layout>> = FastMap::default();
    let mut out = Vec::new();
    for b in found.iter().filter(|b| b.removable) {
        let Some(id) = b.id.clone() else { continue };
        let Some((li, bi)) = generated_building(t0, &id) else { continue };
        let base = bases.entry(li).or_insert_with(|| town::base_layout(world, t0, li));
        if let Some(g) = base.building(bi as usize) {
            out.push(Removal { id, name: b.name.clone().unwrap_or_default(), function: b.function.clone().unwrap_or_default(), at: geom::centroid(&g.poly) });
        }
    }
    out
}

/// The layout `li`'s settlement (`{id, name}` or null) and its own name for a message (a building drawn by hand is named by its settlement).
pub(super) fn place_of(world: &World, t0: &T0, li: usize) -> (Value, Option<String>) {
    let (s, site) = whereabouts(world, t0, li);
    let generated = t0.settlements.len() + t0.base_pois;
    let drawn = li >= generated && world.file.edits.created.get(li - generated).is_some_and(|c| c.kind == "building");
    let own = if town::is_settlement(t0, li) || drawn { s.clone() } else { site.unwrap_or(Value::Null) };
    let name = own["name"].as_str().or_else(|| s["name"].as_str()).map(str::to_string);
    (s, name)
}

fn building_blocker(world: &World, t0: &T0, l: &town::Layout, b: &town::Building, full: bool) -> Blocker {
    let li = l.index as usize;
    let generated = t0.settlements.len() + t0.base_pois;
    let at = geom::centroid(&b.poly);
    let drawn = if li >= generated { world.file.edits.created.get(li - generated).filter(|c| c.kind == "building").map(|c| c.id.clone()) } else { None };
    // (A building a town edit added, id from `NEW_ID` up, is taken away like the others: `remove_buildings` names it too.
    // So is one a plot's fill laid out: the plot's outline can move over its own.)
    let removable = li < generated || town::is_settlement(t0, li) || world.file.edits.created.get(li - generated).is_some_and(|c| c.kind == "plot");
    let (settlement, place) = if full { place_of(world, t0, li) } else { (Value::Null, None) };
    let id = Some(drawn.unwrap_or_else(|| format!("b:{li}:{}", b.id)));
    Blocker { id, name: Some(building_name(world, l, b)), function: Some(b.label().to_string()), settlement, place, removable, ..Blocker::plain("building", at) }
}

/// The point on the segments nearest `c`.
fn near_on(segs: &[(P, P)], c: P) -> P {
    let mut best = (f64::MAX, c);
    for &(a, b) in segs {
        let u = geom::sub(b, a);
        let t = (geom::dot(geom::sub(c, a), u) / geom::dot(u, u).max(1e-9)).clamp(0.0, 1.0);
        let p = geom::add(a, geom::mul(u, t));
        let d = geom::dist(p, c);
        if d < best.0 {
            best = (d, p);
        }
    }
    best.1
}

pub(super) fn plaza_kind(k: town::PlazaKind) -> &'static str {
    use town::PlazaKind as K;
    match k {
        K::Market | K::MarketSparse | K::MarketDay => "market",
        K::Civic => "civic",
        K::Quay => "quay",
        K::Green => "green",
        K::Yard => "yard",
        K::TempleClose => "temple close",
        K::WellSquare => "well",
        K::FountainSquare => "fountain",
        K::Crossroads => "crossroads",
        K::Bailey => "bailey",
    }
}

pub(super) fn street_kind(class: u8) -> &'static str {
    match class {
        0..=2 => "approach road",
        3 => "street",
        _ => "main street",
    }
}

/// A footprint shrunk a foot toward its middle, so walls may touch.
pub(super) fn inner_of(poly: &[P]) -> Vec<P> {
    let c = geom::centroid(poly);
    poly.iter().map(|p| geom::lerp(*p, c, 1.0 / geom::dist(*p, c).max(1.0))).collect()
}

/// Whether the scan may stop: not collecting, and the last thing found is one that stays in the way.
pub(super) fn enough(all: bool, clear: bool, out: &[Blocker]) -> bool {
    out.len() > BLOCKER_CAP || (!all && out.last().is_some_and(|b| !(clear && b.removable)))
}

/// `scan_raw` cut to `BLOCKER_CAP`: the blockers and whether there were more (a scan that stops at the first blocker has more only when
/// `clear` is on and over the cap of the world's own buildings stand in the way).
fn scan_spot(world: &World, t0: &T0, shore: &town::Shore, poly: &[P], skip: Option<(usize, Option<u32>)>, all: bool, clear: bool, full: bool) -> (Vec<Blocker>, bool) {
    let mut out = scan_raw(world, t0, shore, poly, skip, all, clear, full);
    let more = out.len() > BLOCKER_CAP;
    out.truncate(BLOCKER_CAP);
    (out, more)
}

/// What is in the way of `poly` (a footprint, world ft, 3 corners or more, on the map). `all`: every blocker, else the first one (with
/// `clear`, the world's own buildings before it are listed too). `full`: ids, names of places and settlements (slower; the hunt for a
/// spot leaves them out). Stops a blocker past `BLOCKER_CAP`.
fn scan_raw(world: &World, t0: &T0, shore: &town::Shore, poly: &[P], skip: Option<(usize, Option<u32>)>, all: bool, clear: bool, full: bool) -> Vec<Blocker> {
    let mut out: Vec<Blocker> = Vec::new();
    let c = geom::centroid(poly);
    // Corners, the middle and every 10 ft along the walls must be dry: sea or lake as the map
    // draws the shore (`town::Shore`; not the ground under sea level: a beach just inside the
    // shoreline can dip a few feet below it).
    let mut probes = vec![c];
    for (k, &a) in poly.iter().enumerate() {
        let b = poly[(k + 1) % poly.len()];
        let n = (geom::dist(a, b) / 10.0).ceil().max(1.0) as usize;
        probes.extend((0..n).map(|s| geom::lerp(a, b, s as f64 / n as f64)));
    }
    // R5 M20: wholly on a hand bridge's deck, it stands over the water (clear of the lane).
    let deck = crate::wharf::deck_under(world, poly);
    if let Some((id, Some(lane))) = &deck {
        out.push(Blocker { id: Some(id.clone()), width: Some(*lane), ..Blocker::plain("lane", c) });
        if !all {
            return out;
        }
    }
    let on_deck = deck.is_some();
    let (mut wet, mut river) = (on_deck, on_deck);
    for p in &probes {
        if !wet && shore.wet(*p) {
            wet = true;
            let sea = world.params().sea_level_ft + 1.0;
            let held = crate::waters::holding(&world.file.edits, *p).map(|k| world.file.edits.waters[k].look(sea - 1.0).kind);
            let lake = held.map_or((t0.sample_water(p[0], p[1]) as f64) > sea, |k| !k.of_sea());
            out.push(Blocker { kind: Some(if lake { "lake" } else { "sea" }), ..Blocker::plain("water", *p) });
            if !all {
                return out;
            }
        }
        if !river {
            if shore.in_river(*p) {
                river = true;
                let named = if full { river_at(world, t0, *p, 8.0) } else { None };
                out.push(Blocker { id: named.as_ref().and_then(|r| r.id.clone()), name: named.and_then(|r| r.name), ..Blocker::plain("river", *p) });
                if !all {
                    return out;
                }
            }
        }
    }
    let [x0, y0, x1, y1] = bbox(poly);
    let inner = inner_of(poly);
    for l in town::layouts_near(world, t0, [x0, y0, x1, y1]) {
        let li = l.index as usize;
        let own = skip.filter(|s| s.0 == li).map(|s| s.1);
        if own == Some(None) {
            continue;
        }
        let place = |world: &World| if full { place_of(world, t0, li) } else { (Value::Null, None) };
        // (A castle drawn by hand: its yard's buildings make room for one drawn there, `town::sites`.)
        let yard = hand_castle(world, t0, li);
        for b in l.buildings.iter().filter(|b| Some(Some(b.id)) != own && !(yard && b.keep.is_none()) && polys_overlap(&inner, &b.poly)) {
            out.push(building_blocker(world, t0, &l, b, full));
            if enough(all, clear, &out) {
                return out;
            }
        }
        // (A castle's bailey may be built on.)
        for pl in l.plazas.iter().filter(|p| p.kind != town::PlazaKind::Bailey && polys_overlap(&inner, &p.pts)) {
            let (settlement, place) = place(world);
            // (The point of the square nearest the footprint's middle: that, when it is inside.)
            let edges: Vec<(P, P)> = (0..pl.pts.len()).map(|i| (pl.pts[i], pl.pts[(i + 1) % pl.pts.len()])).collect();
            let at = if geom::contains(&pl.pts, c) { c } else { near_on(&edges, c) };
            out.push(Blocker { settlement, place, kind: Some(plaza_kind(pl.kind)), ..Blocker::plain("square", at) });
            if enough(all, clear, &out) {
                return out;
            }
        }
        let streets = l.roads.iter().map(|(pts, class, w)| (pts, 0.5 * w, Some(*class))).chain(l.walls.iter().map(|w| (w, 4.5, None)));
        for (pts, half, class) in streets {
            if !pts.windows(2).any(|s| poly_seg_dist(&inner, s[0], s[1]) < half) {
                continue;
            }
            let hit: Vec<(P, P)> = pts.windows(2).filter(|s| poly_seg_dist(&inner, s[0], s[1]) < half).map(|s| (s[0], s[1])).collect();
            let (settlement, place) = place(world);
            let at = near_on(&hit, c);
            let created = if class.is_none() && li >= t0.settlements.len() + t0.base_pois { world.file.edits.created.get(li - t0.settlements.len() - t0.base_pois) } else { None };
            out.push(match class {
                Some(k) => Blocker { settlement, place, width: Some(2.0 * half), kind: Some(street_kind(k)), ..Blocker::plain("street", at) },
                None => {
                    let (id, name) = (created.map(|c| c.id.clone()), created.map(|c| c.name.clone()));
                    Blocker { settlement, place, width: Some(2.0 * half), id, name, ..Blocker::plain("wall", at) }
                }
            });
            if enough(all, clear, &out) {
                return out;
            }
        }
    }
    let mut roads: Vec<(u32, f64, P)> = Vec::new();
    for (ri, k) in t0.roads.segments_near([x0, y0, x1, y1], 40.0) {
        let rc = &t0.roads.roads[ri as usize];
        let half = 0.5 * rc.class.width_ft();
        let pts: Vec<P> = (0..=16).map(|j| rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p).collect();
        let hit: Vec<(P, P)> = pts.windows(2).filter(|s| poly_seg_dist(&inner, s[0], s[1]) < half).map(|s| (s[0], s[1])).collect();
        if hit.is_empty() {
            continue;
        }
        let at = near_on(&hit, c);
        let d = geom::dist(at, c);
        match roads.iter_mut().find(|r| r.0 == ri) {
            Some(r) if r.1 <= d => {}
            Some(r) => (r.1, r.2) = (d, at),
            None => roads.push((ri, d, at)),
        }
        if !all {
            break;
        }
    }
    if !roads.is_empty() {
        let ids = if full { crate::roadids::road_ids(t0) } else { Vec::new() };
        for (ri, _, at) in roads {
            let rc = &t0.roads.roads[ri as usize];
            let id = ids.get(ri as usize).cloned();
            out.push(Blocker { id, kind: Some(crate::roadids::class_name(rc.class)), width: Some(rc.class.width_ft()), ..Blocker::plain("road", at) });
            if enough(all, clear, &out) {
                return out;
            }
        }
    }
    out
}

fn spot_precheck(world: &World, poly: &[P]) -> Result<(), String> {
    if poly.len() < 3 {
        return Err("a building needs at least 3 corners".to_string());
    }
    if poly.iter().any(|p| p[0] < 0.0 || p[1] < 0.0 || p[0] > world.geom.map_w_ft || p[1] > world.geom.map_h_ft) {
        return Err("that place is off the map".to_string());
    }
    Ok(())
}

/// Everything in the way of a footprint: water, rivers, buildings, squares, streets, walls and the world's roads, each with what it is,
/// its id, name and trade (a building's), its settlement and where it is (`Blocker`), in the order `building_spot` finds them (the
/// first one is its refusal). `skip`: as for `building_spot`. `clear`: the world's own buildings (`removable` set) are listed too, to be
/// left out by the caller. At most 300, the flag saying there were more; `Err` for a footprint with fewer than 3 corners or off the map.
/// Public so that other tools (sites) can name what stands where they are asked to go.
pub fn spot_blockers(world: &World, t0: &T0, poly: &[P], skip: Option<(usize, Option<u32>)>, clear: bool) -> Result<(Vec<Blocker>, bool), String> {
    spot_precheck(world, poly)?;
    Ok(scan_spot(world, t0, &town::Shore::new(world, t0, bbox(poly)), poly, skip, true, clear, true))
}

/// Where a building drawn by hand may stand: on the map, on dry land out of river channels (or
/// R5 M20 wholly on a hand bridge's deck, off its lane: `wharf::deck_under`, then `on` it),
/// clear of other buildings (sharing a wall is fine), roads, streets, town walls and squares
/// (market squares, quays, greens, yards). `skip`: the building being reshaped: (its layout,
/// and in a settlement its id; a drawn building's own layout is all of it). With its point
/// (the footprint's middle) and a name for it: its trade's in the nearest settlement's
/// culture, seeded by its id (else what it is: "House"). A refusal names what is in the way,
/// where (`spot_blockers` lists them all).
/// Whether layout `li` is a castle drawn by hand (`Created` kind `castle`).
fn hand_castle(world: &World, t0: &T0, li: usize) -> bool {
    li.checked_sub(t0.settlements.len() + t0.base_pois).and_then(|k| world.file.edits.created.get(k)).is_some_and(|c| c.kind == "castle")
}

pub fn building_spot(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str, skip: Option<(usize, Option<u32>)>) -> Result<(P, String), String> {
    spot_check(world, t0, &town::Shore::new(world, t0, bbox(poly)), poly, func, id, skip, false, true).map(|s| (s.0, s.1))
}

/// `building_spot` with the world's own buildings in the way taken to be taken away (`create_building clear`): also the ones that are.
pub fn building_spot_clearing(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str, skip: Option<(usize, Option<u32>)>) -> Result<(P, String, Vec<Removal>), String> {
    spot_check(world, t0, &town::Shore::new(world, t0, bbox(poly)), poly, func, id, skip, true, true)
}

/// `building_spot` with the shore over an area that holds `poly`; `clear`: the world's own buildings do not block (they are returned);
/// `full`: refusals with ids and places (the hunt for a spot leaves them out).
fn spot_check(
    world: &World, t0: &T0, shore: &town::Shore, poly: &[P], func: Option<&str>, id: &str, skip: Option<(usize, Option<u32>)>, clear: bool, full: bool,
) -> Result<(P, String, Vec<Removal>), String> {
    spot_precheck(world, poly)?;
    let (found, more) = scan_spot(world, t0, shore, poly, skip, false, clear, full);
    if let Some(b) = found.iter().find(|b| !(clear && b.removable)) {
        return Err(b.message(clear));
    }
    if more {
        return Err(too_many());
    }
    Ok((geom::centroid(poly), spot_name(world, t0, poly, func, id), removals(world, t0, &found)))
}

/// What `clear` says to a footprint over more of the world's buildings than a scan holds.
pub fn too_many() -> String {
    format!("that would take away more than {BLOCKER_CAP} of the world's buildings: ask for a smaller footprint")
}

/// The name a building put on `poly` gets: its trade's in the nearest settlement's culture, seeded by its id (else what it is: "House").
pub fn spot_name(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str) -> String {
    use crate::core::rng::Pcg32;
    let c = geom::centroid(poly);
    let name = match func.and_then(town::catalog::index_of) {
        Some(fi) => {
            let f = &town::catalog::CATALOG[fi];
            let culture = t0.settlements.iter().min_by(|a, b| (a.x - c[0]).hypot(a.y - c[1]).total_cmp(&(b.x - c[0]).hypot(b.y - c[1]))).map_or(0, |s| s.culture as usize);
            let mut rng = Pcg32::new(crate::core::hash::fnv64(id.as_bytes()) ^ world.seed, 43);
            let mut namer = crate::t0::names::Namer::new(rng.next_u32() as u64);
            town::business_name(f, &mut namer, &mut town::Locale::new(culture), None).unwrap_or_else(|| f.name.to_string())
        }
        None => {
            let home = func.and_then(|f| crate::world::BUILDING_HOMES.iter().position(|h| *h == f)).unwrap_or(1);
            town::catalog::RESIDENTIAL[home].to_string()
        }
    };
    let mut cs = name.chars();
    cs.next().map(|f| f.to_uppercase().chain(cs).collect()).unwrap_or(name)
}

/// The footprint `poly` moved (not turned) to the nearest clear spot: tried on 5-ft steps outward by distance to 400 ft. Returns the moved
/// corners, the move `[dx, dy]` and (with `clear`) the world's own buildings it then stands on; `None` when there is none.
pub fn nearest_clear(world: &World, t0: &T0, poly: &[P], func: Option<&str>, id: &str, skip: Option<(usize, Option<u32>)>, clear: bool) -> Option<(Vec<P>, P, Vec<Removal>)> {
    const REACH: f64 = 400.0;
    spot_precheck(world, poly).ok()?;
    enum Obst {
        Area([f64; 4], Vec<P>),
        Line([f64; 4], P, P, f64),
    }
    let bb = bbox(poly);
    let region = [bb[0] - REACH - 2.0, bb[1] - REACH - 2.0, bb[2] + REACH + 2.0, bb[3] + REACH + 2.0];
    let generated = t0.settlements.len() + t0.base_pois;
    let mut obst: Vec<Obst> = Vec::new();
    let near = |b: [f64; 4]| b[2] >= region[0] && b[0] <= region[2] && b[3] >= region[1] && b[1] <= region[3];
    let pad = |b: [f64; 4], h: f64| [b[0] - h, b[1] - h, b[2] + h, b[3] + h];
    for l in town::layouts_near(world, t0, region) {
        let li = l.index as usize;
        let own = skip.filter(|s| s.0 == li).map(|s| s.1);
        if own == Some(None) {
            continue;
        }
        let yard = hand_castle(world, t0, li);
        for b in l.buildings.iter().filter(|b| Some(Some(b.id)) != own && !(clear && li < generated) && !(yard && b.keep.is_none())) {
            let bb = bbox(&b.poly);
            if near(bb) {
                obst.push(Obst::Area(bb, b.poly.clone()));
            }
        }
        for p in l.plazas.iter().filter(|p| p.kind != town::PlazaKind::Bailey) {
            let bb = bbox(&p.pts);
            if near(bb) {
                obst.push(Obst::Area(bb, p.pts.clone()));
            }
        }
        let lines = l.roads.iter().map(|(pts, _, w)| (pts, 0.5 * w)).chain(l.walls.iter().map(|w| (w, 4.5)));
        for (pts, half) in lines {
            for s in pts.windows(2) {
                let bb = pad(bbox(s), half);
                if near(bb) {
                    obst.push(Obst::Line(bb, s[0], s[1], half));
                }
            }
        }
    }
    for (ri, k) in t0.roads.segments_near(region, 40.0) {
        let rc = &t0.roads.roads[ri as usize];
        let half = 0.5 * rc.class.width_ft();
        let pts: Vec<P> = (0..=16).map(|j| rc.eval(k as usize, j as f64 / 16.0, 5.0, t0.cell_ft).p).collect();
        for s in pts.windows(2) {
            obst.push(Obst::Line(pad(bbox(s), half), s[0], s[1], half));
        }
    }
    let inner = inner_of(poly);
    let steps = (REACH / 5.0) as i32;
    let mut offsets: Vec<(i32, i32)> = (-steps..=steps)
        .flat_map(|i| (-steps..=steps).map(move |j| (i, j)))
        .filter(|&(i, j)| (i, j) != (0, 0) && i * i + j * j <= steps * steps)
        .collect();
    offsets.sort_by_key(|&(i, j)| (i * i + j * j, i, j));
    let shore = town::Shore::new(world, t0, region);
    let mid = geom::centroid(&inner);
    for (i, j) in offsets {
        let d = [i as f64 * 5.0, j as f64 * 5.0];
        let shifted: Vec<P> = inner.iter().map(|p| geom::add(*p, d)).collect();
        let sb = bbox(&shifted);
        let hit = obst.iter().any(|o| match o {
            Obst::Area(b, q) => b[2] >= sb[0] && b[0] <= sb[2] && b[3] >= sb[1] && b[1] <= sb[3] && polys_overlap(&shifted, q),
            Obst::Line(b, p, q, half) => b[2] >= sb[0] && b[0] <= sb[2] && b[3] >= sb[1] && b[1] <= sb[3] && poly_seg_dist(&shifted, *p, *q) < *half,
        });
        // Water and rivers: the middle first, so that a footprint at sea or in a river does not run every spot through the full check.
        let m = geom::add(mid, d);
        if hit || shore.wet(m) || shore.in_river(m) {
            continue;
        }
        let moved: Vec<P> = poly.iter().map(|p| geom::add(*p, d)).collect();
        if let Ok((_, _, gone)) = spot_check(world, t0, &shore, &moved, func, id, skip, clear, false) {
            return Some((moved, d, gone));
        }
    }
    None
}

/// The nearest clear spot for a `width` × `depth` ft building near `near` (world ft): turned
/// square to the nearest street (or road), its front within a few feet of it where there is
/// one within 300 ft, on the 5-ft grid when unturned; tried outward ring by ring to 400 ft.
/// Returns the footprint, its point and its name (`building_spot`).
pub fn find_spot(world: &World, t0: &T0, near: P, width: f64, depth: f64, func: Option<&str>, id: &str) -> Result<(Vec<P>, P, String), String> {
    find_spot_clearing(world, t0, near, width, depth, func, id, false).map(|s| (s.0, s.1, s.2))
}

/// `find_spot`; with `clear`, the world's own buildings do not block (the ones it stands on are returned: take them away with it).
pub fn find_spot_clearing(world: &World, t0: &T0, near: P, width: f64, depth: f64, func: Option<&str>, id: &str, clear: bool) -> Result<(Vec<P>, P, String, Vec<Removal>), String> {
    if !(10.0..=400.0).contains(&width) || !(10.0..=400.0).contains(&depth) {
        return Err("width_ft and depth_ft: 10 to 400".into());
    }
    let pad = 300.0 + width.max(depth);
    let rect_q = [near[0] - pad, near[1] - pad, near[0] + pad, near[1] + pad];
    // Street pieces (edges of their beds) near the place, from the settlements' layouts.
    let mut streets: Vec<(P, P, f64)> = Vec::new();
    for l in town::layouts_near(world, t0, rect_q) {
        for (pts, _, w) in &l.roads {
            streets.extend(pts.windows(2).map(|s| (s[0], s[1], 0.5 * w)));
        }
    }
    if streets.is_empty() {
        for (ri, k) in t0.roads.segments_near(rect_q, 0.0) {
            let rc = &t0.roads.roads[ri as usize];
            let pts: Vec<P> = (0..=8).map(|j| rc.eval(k as usize, j as f64 / 8.0, 5.0, t0.cell_ft).p).collect();
            streets.extend(pts.windows(2).map(|s| (s[0], s[1], 0.5 * rc.class.width_ft())));
        }
    }
    let nearest = |p: P| streets.iter().map(|&(a, b, half)| (geom::seg_dist(p, a, b) - half, a, b)).min_by(|x, y| x.0.total_cmp(&y.0));
    let front = depth * 0.5;
    // (One shore for every footprint tried.)
    let r = 400.0 + width.max(depth);
    let shore = town::Shore::new(world, t0, [near[0] - r, near[1] - r, near[0] + r, near[1] + r]);
    let mut tried = 0;
    for ring in 0..=40 {
        let r = ring as f64 * 10.0;
        let n = if ring == 0 { 1 } else { (std::f64::consts::TAU * r / 10.0).ceil() as usize };
        for k in 0..n {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            let mut c = [near[0] + r * libm::cos(a), near[1] + r * libm::sin(a)];
            let (dir, gap) = match nearest(c) {
                Some((d, a, b)) if d < 300.0 => {
                    let u = geom::sub(b, a);
                    (geom::mul(u, 1.0 / geom::len(u).max(1e-9)), d - front)
                }
                _ => ([1.0, 0.0], 0.0),
            };
            // Its front on the street: within 2–12 ft of the bed.
            if !(2.0..=12.0).contains(&gap) && !streets.is_empty() && nearest(c).is_some_and(|s| s.0 < 300.0) {
                continue;
            }
            let square = dir[0].abs() > 0.999 || dir[1].abs() > 0.999;
            if square {
                c = c.map(|v| crate::core::round(v / 5.0) * 5.0);
            }
            let poly = town::rect(c, dir, width, depth);
            tried += 1;
            if let Ok((p, name, gone)) = spot_check(world, t0, &shore, &poly, func, id, None, clear, false) {
                return Ok((poly, p, name, gone));
            }
        }
    }
    Err(format!("no clear spot for a {width} × {depth} ft building within 400 ft of that place ({tried} tried)"))
}

/// The runs of `pts` that lie inside the area (`inside`), cut where the line crosses its edge (found by bisection), whole ft.
pub(super) fn clip_runs(pts: &[P], inside: &dyn Fn(P) -> bool) -> Vec<Vec<P>> {
    let rnd = |p: P| [crate::core::round(p[0]), crate::core::round(p[1])];
    let mut runs: Vec<Vec<P>> = Vec::new();
    let mut cur: Vec<P> = Vec::new();
    let Some(&first) = pts.first() else { return runs };
    let mut last = (first, inside(first));
    if last.1 {
        cur.push(rnd(first));
    }
    for &p in &pts[1..] {
        let n = (geom::dist(last.0, p) / 20.0).ceil().max(1.0) as usize;
        let from = last.0;
        for s in 1..=n {
            let m = if s == n { p } else { geom::lerp(from, p, s as f64 / n as f64) };
            let mi = inside(m);
            if mi != last.1 {
                let (mut a, mut b) = (last.0, m);
                for _ in 0..12 {
                    let mid = geom::lerp(a, b, 0.5);
                    if inside(mid) == last.1 {
                        a = mid;
                    } else {
                        b = mid;
                    }
                }
                if last.1 {
                    cur.push(rnd(a));
                    runs.push(std::mem::take(&mut cur));
                } else {
                    cur.push(rnd(b));
                }
            }
            if mi && s == n {
                cur.push(rnd(p));
            }
            last = (m, mi);
        }
    }
    runs.push(cur);
    runs.retain(|r| r.len() >= 2);
    for r in &mut runs {
        r.dedup();
    }
    runs.retain(|r| r.len() >= 2);
    runs
}
