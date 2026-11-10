//! Underground sites in search (item 116): an index of every site below ground world-wide
//! (ways down in towns and ruins `u:`, created sites' `u:`, keeps' deep dungeons `k:`, each
//! city's sewers as one site `w:<layout>`), built without laying any site out, and a search
//! over it by name, renamed levels and rooms, kind and theme words, and what the DM attached.
//!
//! The index is the generated part only (kind, theme, levels, size, entrance), cached per
//! layout while that layout is the same one (`town::layout` hands back the same `Rc`); names,
//! designs, notes, NPCs and plots are read from the edits on every search.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};

use crate::World;
use crate::core::hash::FastMap;
use crate::core::rng::{Pcg32, hash3, unit};
use crate::geoquery::Region;
use crate::t0::T0;
use crate::town::{self, geom};
use crate::under::{self, SiteSize, SiteSpec, UnderKind};

/// One site below ground, as generated.
#[derive(Clone, Debug)]
pub struct Site {
    /// `u:<layout>:<k>`, `k:<layout>:<building>` or `w:<layout>` (a city's sewers).
    pub id: String,
    pub kind: UnderKind,
    /// Theme key (`under::Theme::key`; `keep_dungeon`, `sewer` for those two).
    pub theme: &'static str,
    /// Levels below ground: as `under::generate` resolves them (exact); a keep's deep dungeons
    /// from the keep's shell (as `under::keep_dungeon` sizes them, a designed keep aside); sewers 2.
    pub levels: u8,
    /// small/medium/large/huge: a `u:` site's grid size as resolved; deep dungeons medium (2
    /// levels) or large (3); sewers by the sections with a street grate (`sections`).
    pub size: &'static str,
    pub layout: usize,
    /// The way in (a sewer: the grate nearest the town's middle).
    pub at: [f64; 2],
    /// Sewers: the 480-ft sections with a street grate (else 0).
    pub sections: u32,
    /// Sewers: every street grate (a sewer is in an area or near a point by any of them).
    pub grates: Vec<[f64; 2]>,
}

struct Entry {
    layout: Rc<town::Layout>,
    spec: Option<SiteSpec>,
    sites: Vec<Site>,
}

thread_local! {
    /// World hash and, per layout, the sites found in it (None: not built yet or deleted).
    static INDEX: RefCell<(u64, Vec<Option<Entry>>)> = const { RefCell::new((0, Vec::new())) };
}

/// The site kinds as search and its filters name them.
pub fn kind_name(k: UnderKind) -> &'static str {
    match k {
        UnderKind::KeepDungeon => "keep dungeon",
        k => k.name(),
    }
}

/// A kind by its name or key (`keep dungeon`, `keep_dungeon`, `lava tube`, plurals).
pub fn parse_kind(s: &str) -> Option<UnderKind> {
    let w = norm(s);
    KINDS.iter().find(|k| norm(kind_name(**k)) == w || norm(k.key()) == w || (**k == UnderKind::KeepDungeon && w == "deep dungeon")).copied()
}

const KINDS: [UnderKind; 8] = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs, UnderKind::Cave, UnderKind::Mine, UnderKind::LavaTube, UnderKind::Sewer, UnderKind::KeepDungeon];

/// A theme's name in words ("ice cave", "wizard lair", "keep dungeon").
pub fn theme_name(key: &str) -> String {
    match key {
        "fungal" | "crystal" | "ice" | "flooded" => format!("{key} cave"),
        k => k.replace('_', " "),
    }
}

/// Lower case, `_`/`-` as spaces, single spaces, each word singular (caves, ossuaries, catacombs).
fn norm(s: &str) -> String {
    let s = s.to_lowercase().replace(['_', '-'], " ");
    let one = |w: &str| -> String {
        if w.len() > 4 && w.ends_with("ies") {
            format!("{}y", &w[..w.len() - 3])
        } else if w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") {
            w[..w.len() - 1].to_string()
        } else {
            w.to_string()
        }
    };
    s.split_whitespace().map(one).collect::<Vec<_>>().join(" ")
}

/// Every underground site of the world, deleted ones left out. Built on first use and kept: a
/// layout's sites are found again only when its layout changed (or a created site's spec did).
pub fn sites(world: &World, t0: &T0) -> Vec<Site> {
    let n = t0.settlements.len();
    let count = town::layout_count(t0);
    INDEX.with(|ix| {
        let mut ix = ix.borrow_mut();
        if ix.0 != world.hash {
            *ix = (world.hash, Vec::new());
        }
        ix.1.resize_with(count, || None);
        ix.1.truncate(count);
        let mut out = Vec::new();
        for li in 0..count {
            let made = li.checked_sub(n + t0.base_pois).and_then(|k| world.file.edits.created.get(k));
            if made.is_some_and(|c| c.removed || c.kind == "building") {
                ix.1[li] = None;
                continue;
            }
            let spec = li.checked_sub(n).and_then(|p| t0.created_site(p)).map(|c| c.spec);
            let l = town::layout(world, t0, li);
            let fresh = ix.1[li].as_ref().is_some_and(|e| Rc::ptr_eq(&e.layout, &l) && e.spec == spec);
            if !fresh {
                let sites = layout_sites(world, t0, &l, li, spec);
                ix.1[li] = Some(Entry { layout: l, spec, sites });
            }
            out.extend(ix.1[li].as_ref().map(|e| e.sites.iter().cloned()).into_iter().flatten());
        }
        out
    })
}

/// The sites in one layout, as `under::generate`, `keep_dungeon` and `sewer_site` would make them.
fn layout_sites(world: &World, t0: &T0, l: &town::Layout, li: usize, spec: Option<SiteSpec>) -> Vec<Site> {
    let mut out = Vec::new();
    let n = t0.settlements.len();
    let ruin = li >= n && l.buildings.iter().all(|b| b.structure == town::Structure::Ruin);
    let cold = t0.cold_of_layout(world, li);
    let key = town::rng_key(t0, li);
    for e in l.entrances.iter().filter(|e| e.kind != UnderKind::Sewer) {
        let k = e.id as i64;
        // (The same rng and first draws as `under::generate`, and its sunken temple.)
        let mut rng = Pcg32::new(hash3(world.stream("under"), key, k, e.kind as i64), 91);
        let sp = spec.unwrap_or_default();
        let (size, depth, theme) = under::resolve(e.kind, sp, ruin, cold, &mut rng);
        let share = unit(hash3(world.stream("under"), key, k, 0x5e4c)) < 0.25;
        let theme = if sp.theme.is_none() && theme.key == "temple" && (share || under::wet_ground(t0, e.at)) { under::SUNKEN_TEMPLE } else { theme.key };
        out.push(Site { id: format!("u:{li}:{}", e.id), kind: e.kind, theme, levels: depth as u8, size: size.name(), layout: li, at: e.at, sections: 0, grates: Vec::new() });
    }
    for b in l.buildings.iter().filter(|b| crate::interior::has_deep_dungeon(b)) {
        let sh = crate::interior::Shell::of(l, b);
        let depth = if (sh.nx + 28) * (sh.ny + 28) > 6000 { 3 } else { 2 };
        let size = if depth >= 3 { "large" } else { "medium" };
        out.push(Site { id: format!("k:{li}:{}", b.id), kind: UnderKind::KeepDungeon, theme: "keep_dungeon", levels: depth, size, layout: li, at: geom::centroid(&b.poly), sections: 0, grates: Vec::new() });
    }
    let grates: Vec<[f64; 2]> = l.entrances.iter().filter(|e| e.kind == UnderKind::Sewer).map(|e| e.at).collect();
    if !grates.is_empty() {
        let mut secs: Vec<[i64; 2]> = grates.iter().map(|p| under::sewer_section(*p)).map(|o| [(o[0] / under::SEWER_SECTION_FT) as i64, (o[1] / under::SEWER_SECTION_FT) as i64]).collect();
        secs.sort_unstable();
        secs.dedup();
        let mid = t0.settlements.get(li).map_or(grates[0], |s| [s.x, s.y]);
        let at = grates.iter().copied().min_by(|a, b| geom::dist(*a, mid).total_cmp(&geom::dist(*b, mid))).unwrap_or(mid);
        let size = match secs.len() {
            0..10 => "small",
            10..40 => "medium",
            40..120 => "large",
            _ => "huge",
        };
        out.push(Site { id: format!("w:{li}"), kind: UnderKind::Sewer, theme: "sewer", levels: 2, size, layout: li, at, sections: secs.len() as u32, grates });
    }
    out
}

/// What `search_underground` keeps: site kinds (none: all), levels, sizes, an area, a point
/// and radius (ft; hits then sorted by distance to it), hidden sites too.
#[derive(Clone, Debug, Default)]
pub struct UnderOpts {
    pub kinds: Vec<UnderKind>,
    pub levels_min: Option<usize>,
    pub levels_max: Option<usize>,
    pub sizes: Vec<String>,
    pub within: Option<Region>,
    pub near: Option<[f64; 2]>,
    pub radius_ft: Option<f64>,
    pub hidden: bool,
}

impl UnderOpts {
    /// A filter only underground sites can meet (kinds, levels, size).
    pub fn narrows(&self) -> bool {
        !self.kinds.is_empty() || self.levels_min.is_some() || self.levels_max.is_some() || !self.sizes.is_empty()
    }

    /// From JSON: `{kinds: [..], levels_min, levels_max, sizes: [..], rect: [x0, y0, x1, y1],
    /// near: [x, y], radius_ft, hidden}` (anything left out not filtered on). Unknown kinds and
    /// sizes are refused.
    pub fn from_json(v: &Value) -> Result<UnderOpts, String> {
        let mut o = UnderOpts::default();
        for k in v["kinds"].as_array().into_iter().flatten() {
            let s = k.as_str().unwrap_or("");
            o.kinds.push(parse_kind(s).ok_or_else(|| format!("kinds: no site kind \"{s}\" ({})", KINDS.map(kind_name).join(", ")))?);
        }
        for s in v["sizes"].as_array().into_iter().flatten() {
            let s = s.as_str().unwrap_or("").to_lowercase();
            if SiteSize::parse(&s).is_none() {
                return Err(format!("sizes: no size \"{s}\" ({})", SiteSize::NAMES.join(", ")));
            }
            o.sizes.push(s);
        }
        o.levels_min = v["levels_min"].as_u64().map(|x| x as usize);
        o.levels_max = v["levels_max"].as_u64().map(|x| x as usize);
        let nums = |k: &str| v[k].as_array().map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<f64>>());
        o.within = nums("rect").filter(|r| r.len() == 4).map(|r| Region::Box([r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])]));
        o.near = nums("near").filter(|p| p.len() == 2).map(|p| [p[0], p[1]]);
        o.radius_ft = v["radius_ft"].as_f64();
        o.hidden = v["hidden"].as_bool().unwrap_or(false);
        Ok(o)
    }
}

/// Something the DM put on a site, its levels or rooms.
struct Attached {
    what: &'static str,
    /// The note's place, or the NPC's / plot's own id.
    id: String,
    /// The place it is at (the site, `l:` or `r:`).
    at: String,
    text: String,
}

impl Attached {
    fn json(&self) -> Value {
        let mut v = json!({ "what": self.what, "id": self.id, "at": self.at });
        v[if self.what == "note" { "text" } else { "name" }] = json!(self.text);
        v
    }
}

/// The site an id belongs to (`u:`/`k:` itself, a sewer section's city `w:<layout>`), for a
/// site's id or one of its levels' or rooms'.
fn site_key(id: &str) -> Option<String> {
    let base = crate::agent::split_room_id(id).map_or(id, |s| s.0);
    if let Some(rest) = base.strip_prefix("w:") {
        return Some(format!("w:{}", rest.split(':').next()?));
    }
    (base.starts_with("u:") || base.starts_with("k:")).then(|| base.to_string())
}

/// The middle of the sewer section an id names (`w:<layout>:<sx>:<sy>` or a level or room of it).
fn section_middle(id: &str) -> Option<[f64; 2]> {
    let base = crate::agent::split_room_id(id).map_or(id, |s| s.0);
    let n: Vec<f64> = base.strip_prefix("w:")?.split(':').skip(1).filter_map(|s| s.parse().ok()).collect();
    let s = under::SEWER_SECTION_FT;
    (n.len() == 2).then(|| [(n[0] + 0.5) * s, (n[1] + 0.5) * s])
}

/// Everything attached under ground, by site (renamed levels and rooms apart, as `(id, name)`).
fn attachments(world: &World) -> (FastMap<String, Vec<Attached>>, FastMap<String, Vec<(String, String)>>) {
    let e = &world.file.edits;
    let mut att: FastMap<String, Vec<Attached>> = FastMap::default();
    let mut put = |at: &str, a: Attached| {
        if let Some(k) = site_key(at) {
            att.entry(k).or_default().push(a);
        }
    };
    for (id, n) in &e.notes {
        put(id, Attached { what: "note", id: id.clone(), at: id.clone(), text: n.text.clone() });
    }
    for (id, n) in &e.npcs {
        if let Some(l) = &n.location {
            let at = match l.level {
                Some(lv) if !l.id.starts_with("l:") && !l.id.starts_with("r:") => format!("l:{}:{lv}", l.id),
                _ => l.id.clone(),
            };
            put(&l.id, Attached { what: "npc", id: id.clone(), at, text: n.name.clone() });
        }
    }
    for (id, p) in &e.plots {
        let mut seen: Vec<String> = Vec::new();
        for a in &p.anchors {
            if let Some(k) = site_key(a).filter(|k| !seen.contains(k)) {
                seen.push(k);
                put(a, Attached { what: "plot", id: id.clone(), at: a.clone(), text: p.title.clone() });
            }
        }
    }
    let mut renamed: FastMap<String, Vec<(String, String)>> = FastMap::default();
    for (id, name) in e.renames.iter().filter(|(k, _)| k.starts_with("l:") || k.starts_with("r:")) {
        if let Some(k) = site_key(id) {
            renamed.entry(k).or_default().push((id.clone(), name.clone()));
        }
    }
    (att, renamed)
}

/// Order of what matched (lower first).
const WHAT: [&str; 8] = ["name", "level", "room", "npc", "plot", "note", "kind", "theme"];

/// Underground sites matching `query` and the filters, best first: an exact name, then by what
/// matched (name, renamed level, renamed room, NPC, plot, note, kind word, theme word), then
/// nearest `near`. An empty query lists every site the filters keep. Each hit as JSON (kind
/// `underground`, `site_kind`, `theme`, `levels`, `size`, `match`, `attached`...).
pub fn search_underground(world: &World, t0: &T0, query: &str, o: &UnderOpts) -> Vec<Value> {
    let q = query.trim().to_lowercase();
    let qn = norm(&q);
    let edits = &world.file.edits;
    // Kind and theme words: the whole query names a kind ("crypts", "lava tube"), or is a run
    // of words of a theme's name ("temple" in "sunken temple", "ice cave").
    let word_kind = parse_kind(&q).or_else(|| (qn == "cavern").then_some(UnderKind::Cave));
    let theme_word = |key: &str| -> bool {
        if qn.is_empty() {
            return false;
        }
        let t = norm(&theme_name(key));
        t == qn || format!(" {t} ").contains(&format!(" {qn} "))
    };
    let (att, renamed) = attachments(world);
    let none = Vec::new();
    let mut hits: Vec<(u8, usize, f64, usize, Value)> = Vec::new();
    for (i, s) in sites(world, t0).into_iter().enumerate() {
        if crate::agent::deleted_with(world, t0, &s.id).is_some() {
            continue;
        }
        let (settlement, site) = crate::agent::whereabouts(world, t0, s.layout);
        // (Hidden with the site or the settlement it is under: a hidden city's sewers and sites too.)
        let hides = |v: &Value| v["id"].as_str().is_some_and(|id| edits.hidden.contains(id));
        if !o.hidden && (edits.hidden.contains(&s.id) || site.as_ref().is_some_and(hides) || hides(&settlement)) {
            continue;
        }
        // A designed site: its levels and theme as drawn.
        let (mut kind, mut theme, mut levels) = (s.kind, s.theme, s.levels as usize);
        if let Some(d) = edits.designs.get(&s.id) {
            levels = d.levels.len();
            kind = UnderKind::parse(&d.kind).unwrap_or(kind);
            theme = d.theme.as_deref().and_then(|t| under::THEMES.iter().find(|x| x.key == t)).map_or(theme, |x| x.key);
        }
        if (!o.kinds.is_empty() && !o.kinds.contains(&kind)) || o.levels_min.is_some_and(|m| levels < m) || o.levels_max.is_some_and(|m| levels > m) || (!o.sizes.is_empty() && !o.sizes.iter().any(|z| z == s.size)) {
            continue;
        }
        // A sewer is where any of its grates is.
        let spots: &[[f64; 2]] = if s.grates.is_empty() { std::slice::from_ref(&s.at) } else { &s.grates };
        if o.within.as_ref().is_some_and(|r| !spots.iter().any(|p| r.contains(*p))) {
            continue;
        }
        let dist = o.near.map_or(0.0, |p| spots.iter().map(|q| geom::dist(p, *q)).fold(f64::INFINITY, f64::min));
        if o.near.is_some() && o.radius_ft.is_some_and(|r| dist > r) {
            continue;
        }
        let generated = match kind {
            UnderKind::Sewer => format!("Sewers of {}", settlement["name"].as_str().unwrap_or("the city")),
            UnderKind::KeepDungeon => "Deep dungeons".to_string(),
            k => crate::sitelinks::dungeon_name(world, t0, s.layout).unwrap_or_else(|| cap(k.name())),
        };
        let own = edits.renames.get(&s.id);
        let name = own.cloned().unwrap_or_else(|| generated.clone());
        let attached = att.get(&s.id).unwrap_or(&none);
        let levels_renamed = renamed.get(&s.id).map(Vec::as_slice).unwrap_or_default();
        // What matched: (rank, what, id, text).
        let mut m: Vec<(usize, &str, String, String)> = Vec::new();
        let mut exact = false;
        if !q.is_empty() {
            // Its own name (renamed, or a city's sewers), and the site it lies under.
            let site_name = site.as_ref().and_then(|v| v["name"].as_str()).map(str::to_string);
            for (nm, id) in [(own.cloned().or_else(|| (kind == UnderKind::Sewer && q.len() >= 3).then(|| name.clone())), s.id.clone()), (site_name, site.as_ref().and_then(|v| v["id"].as_str()).unwrap_or("").to_string())] {
                if let Some(nm) = nm.filter(|nm| q.len() >= 2 && nm.to_lowercase().contains(&q)) {
                    exact |= nm.to_lowercase() == q;
                    m.push((0, "name", id, nm));
                }
            }
            // (A level a design has cut is no longer there.)
            let cut = |id: &str| crate::agent::split_room_id(id).is_some_and(|(_, lv, _)| lv >= levels);
            for (id, nm) in levels_renamed.iter().filter(|(id, nm)| q.len() >= 2 && nm.to_lowercase().contains(&q) && !cut(id)) {
                exact |= nm.to_lowercase() == q;
                m.push((if id.starts_with("l:") { 1 } else { 2 }, if id.starts_with("l:") { "level" } else { "room" }, id.clone(), nm.clone()));
            }
            for a in attached.iter().filter(|a| a.text.to_lowercase().contains(&q) && (a.what != "note" || q.len() >= 3)) {
                let id = if a.what == "note" { a.at.clone() } else { a.id.clone() };
                m.push((WHAT.iter().position(|w| *w == a.what).unwrap_or(5), a.what, id, a.text.chars().take(120).collect()));
            }
            if theme_word(theme) {
                m.push((7, "theme", s.id.clone(), theme_name(theme)));
            }
            if word_kind == Some(kind) {
                m.push((6, "kind", s.id.clone(), kind_name(kind).to_string()));
            }
            if m.is_empty() {
                continue;
            }
            m.sort_by_key(|x| x.0);
        }
        let mj = |x: &(usize, &str, String, String)| {
            let mut v = json!({ "what": x.1, "id": x.2, "text": x.3 });
            if let Some(p) = (kind == UnderKind::Sewer).then(|| section_middle(&x.2)).flatten() {
                v["x_ft"] = json!(p[0]);
                v["y_ft"] = json!(p[1]);
            }
            v
        };
        let mut v = json!({
            "id": s.id, "kind": "underground", "site_kind": kind_name(kind), "theme": theme, "theme_name": theme_name(theme), "name": name, "generated": generated,
            "levels": levels, "size": s.size, "x_ft": s.at[0].round(), "y_ft": s.at[1].round(), "settlement": settlement,
            "match": m.first().map(mj), "attached": attached.iter().map(Attached::json).collect::<Vec<_>>(),
        });
        if m.len() > 1 {
            v["matches"] = json!(m.iter().map(mj).collect::<Vec<_>>());
        }
        if let Some(site) = site {
            v["site"] = site;
        }
        // Under an entrance made in a building's cellar: that building (the title and sub line say "under The Lucky Shield").
        if let Some((li, bi)) = crate::sitelinks::building_of(world, t0, s.layout) {
            let id = format!("b:{li}:{bi}");
            v["in_building"] = json!({ "name": crate::agent::building_label(world, t0, &id), "id": id });
        }
        if kind == UnderKind::Sewer {
            v["sections"] = json!(s.sections);
        }
        if edits.hidden.contains(&s.id) {
            v["hidden"] = json!(true);
        }
        if o.near.is_some() {
            v["distance_ft"] = json!(dist.round());
        }
        hits.push((u8::from(!exact), m.first().map_or(WHAT.len(), |x| x.0), dist, i, v));
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)).then(a.3.cmp(&b.3)));
    hits.into_iter().map(|h| h.4).collect()
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// `search_underground` for the app: `filters` as `UnderOpts::from_json` reads them, plus
/// `limit` (default 50). `{items, found, truncated}` or `{error}`.
pub fn search_json(world: &World, t0: &T0, query: &str, filters: &str) -> String {
    let v: Value = serde_json::from_str(filters).unwrap_or(Value::Null);
    let out = match UnderOpts::from_json(&v) {
        Ok(o) => {
            let all = search_underground(world, t0, query, &o);
            let limit = v["limit"].as_u64().unwrap_or(50) as usize;
            let found = all.len();
            json!({ "items": all.into_iter().take(limit).collect::<Vec<_>>(), "found": found, "truncated": found > limit })
        }
        Err(e) => json!({ "error": e }),
    };
    out.to_string()
}
