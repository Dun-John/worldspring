//! Buildings' functions (`assign_functions`) and names: locales, owners and businesses.

use super::*;

pub(super) fn assign_functions(site: &Site, s: &Settlement, on_water: bool, rng: &mut Pcg32, l: &mut Layout) {
    let tier = s.tier;
    let mut needs: Vec<usize> = Vec::new();
    for (fi, f) in CATALOG.iter().enumerate() {
        let n = catalog::required_count(f, tier, on_water);
        needs.extend(std::iter::repeat_n(fi, n));
        if n == 0 && tier >= f.min_tier && (!f.water || on_water) && tier != Tier::Metropolis && rng.next_f64() < 0.3 {
            needs.push(fi);
        }
    }
    // Enough buildings for the guarantees, even on a cramped or flooded site.
    let mut k = 0;
    while l.buildings.len() < needs.len() && k < 400 {
        k += 1;
        let a = rng.range(0.0, std::f64::consts::TAU);
        let rad = rng.range(30.0, l.radius.max(80.0));
        let c = [rad * libm::cos(a), rad * libm::sin(a)];
        // (In a village, its long side along the nearest lane.)
        let house = rect(c, village::spare_dir(l, c).unwrap_or([-libm::sin(a), libm::cos(a)]), 30.0, 20.0);
        let clear = l.buildings.iter().all(|b| dist(centroid(&b.poly), c) > 28.0) && village::spare_ok(site, l, &house);
        if clear && (site.buildable(&house) || (k > 300 && !site.wet(c) && site.dry_outline(&house, 10.0))) {
            let pad = site.pad(&house);
            let ward = if tier == Tier::Village { Ward::Rural } else { Ward::Common };
            l.buildings.push(Building { poly: house, ward, func: None, residential: 1, name: None, floors: 1, pad_ft: pad, structure: Structure::Roofed, roof: None, tint: None, material: None, shape: None, shade: None, id: 0, parts: Vec::new(), keep: None, deck: None });
        }
    }
    // Big functions first so they get the big lots.
    needs.sort_by_key(|&fi| (!CATALOG[fi].big, fi));
    let areas: Vec<f64> = l.buildings.iter().map(|b| area(&b.poly).abs()).collect();
    let max_area = areas.iter().cloned().fold(1.0, f64::max);
    let mut taken = vec![false; l.buildings.len()];
    // The keep at the heart of a castle ward (4 storeys, raised by the ward layout) is the
    // castle.
    if let Some(castle) = CATALOG.iter().position(|f| f.key == "castle")
        && let Some(keep) = l.buildings.iter().position(|b| b.keep.is_some() && b.func.is_none())
    {
        l.buildings[keep].func = Some(castle as u16);
        taken[keep] = true;
        if let Some(k) = needs.iter().position(|&fi| fi == castle) {
            needs.remove(k);
        }
    }
    // What the layout built for a purpose (a temple on its close) is that already.
    for (bi, b) in l.buildings.iter().enumerate() {
        if let Some(f) = b.func.filter(|_| !taken[bi]) {
            taken[bi] = true;
            if let Some(k) = needs.iter().position(|&fi| fi == f as usize) {
                needs.remove(k);
            }
        }
    }
    if tier == Tier::Village {
        village::reserve(l, &mut needs, &mut taken);
    }
    for fi in needs {
        let f = &CATALOG[fi];
        let mut best: Option<(f64, usize)> = None;
        for (bi, b) in l.buildings.iter().enumerate() {
            if taken[bi] {
                continue;
            }
            let ward_score = f.wards.iter().position(|w| *w == b.ward).map_or(0.0, |p| 10.0 - 2.0 * p as f64);
            let size = areas[bi] / max_area;
            let size_score = if f.big { 6.0 * size } else { -2.0 * size };
            // A business needs room for its fittings (a bar and tables, a counter): a shed-sized
            // lot only when nothing else is left.
            let cramped = if areas[bi] < 500.0 { 100.0 } else { 0.0 };
            let score = ward_score + size_score - cramped + village::green_bonus(l, &b.poly, f.key) + rng.next_f64() * 1.5;
            if best.is_none_or(|(s, _)| score > s) {
                best = Some((score, bi));
            }
        }
        let Some((_, bi)) = best else { break };
        taken[bi] = true;
        l.buildings[bi].func = Some(fi as u16);
        if f.key == "graveyard" {
            l.buildings[bi].structure = Structure::Open;
        }
    }
    // Names and floors.
    let mut namer = Namer::new(hash2(s.seed, 0x7a3e, 1));
    let mut used: std::collections::BTreeSet<String> = Default::default();
    let mut loc = Locale::of(s, s.river || !site.river.is_empty(), &mut namer);
    for b in &mut l.buildings {
        let big = area(&b.poly).abs() > 2500.0;
        match b.func {
            Some(fi) => {
                let f = &CATALOG[fi as usize];
                for _ in 0..12 {
                    match business_name(f, &mut namer, &mut loc, Some(b.ward)) {
                        Some(n) if used.insert(n.clone()) => {
                            b.name = Some(n);
                            break;
                        }
                        None => break,
                        _ => {}
                    }
                }
                b.floors = match f.key {
                    "castle" | "palace" => 4,
                    "inn" | "tavern" | "town_hall" | "library" | "arcane_academy" | "guildhall" => 2 + (rng.next_f64() < 0.5) as u8,
                    "temple" | "warehouse" | "arena" | "graveyard" | "shrine" => 1,
                    _ => 1 + (tier >= Tier::Town && rng.next_f64() < 0.6) as u8,
                };
            }
            None => {
                b.residential = catalog::residential_for(b.ward, big) as u8;
                b.floors = match b.ward {
                    Ward::Noble | Ward::Merchant | Ward::Castle => 2 + (rng.next_f64() < 0.4) as u8,
                    Ward::Slum | Ward::Common | Ward::Craft | Ward::Docks => 1 + (tier >= Tier::City && rng.next_f64() < 0.5) as u8,
                    _ => 1,
                };
            }
        }
    }
    // A city's palace (about two in five; a metropolis has its own, `CATALOG`): the biggest home of
    // its noble quarter, 3 storeys, an older palace round its great hall (`interior::hub`). Its own
    // stream, so that nothing else in the town is named or built otherwise.
    if let Some(fi) = catalog::index_of("palace")
        && tier == Tier::City
        && unit(hash2(s.seed, 0x9a1a, 1)) < CITY_PALACES
        && !l.buildings.iter().any(|b| b.func == Some(fi as u16))
    {
        let size = |b: &Building| area(&b.poly).abs();
        let home = |b: &Building| b.func.is_none() && b.keep.is_none() && b.ward == Ward::Noble && b.structure == Structure::Roofed && size(b) >= 2500.0;
        if let Some(i) = (0..l.buildings.len()).filter(|&i| home(&l.buildings[i])).max_by(|&a, &b| size(&l.buildings[a]).total_cmp(&size(&l.buildings[b])).then(b.cmp(&a))) {
            let b = &mut l.buildings[i];
            (b.func, b.floors, b.name) = (Some(fi as u16), 3, None);
            for _ in 0..12 {
                match business_name(&CATALOG[fi], &mut namer, &mut loc, Some(b.ward)) {
                    Some(n) if used.insert(n.clone()) => {
                        b.name = Some(n);
                        break;
                    }
                    None => break,
                    _ => {}
                }
            }
        }
    }
}

/// The share of cities with a palace of their own.
const CITY_PALACES: f64 = 0.4;
/// The share of town castles (but a capital's, which always has one) sized for an inner bailey.
pub(super) const CASTLE_INNER: f64 = 0.45;

/// A bank is named with one of these; a town has at most one (`MONEY_KEY` in its key words).
const MONEY: [&str; 4] = ["Counting House", "Exchange", "Treasury", "Bank"];
const MONEY_KEY: &str = "$bank";

/// What a town's businesses are named for (`business_name`): its culture, its landmarks (a river,
/// the sea, what the town is there for), its lord, and the key words its businesses' names have
/// taken (no two share one: one Fox, one Harbour, one Aldric in a town).
pub struct Locale {
    pub culture: usize,
    pub landmarks: Vec<&'static str>,
    pub lord: Option<String>,
    pub keys: std::collections::BTreeSet<String>,
    /// How many businesses have been named, and how many of them per head noun (`head_noun`):
    /// no noun on more than about one in eight ("The Grey Library", "The Old Library").
    pub named: u32,
    pub heads: std::collections::BTreeMap<String, u32>,
}

impl Locale {
    pub fn new(culture: usize) -> Self {
        Self { culture: culture % catalog::SIGNS.len(), landmarks: Vec::new(), lord: None, keys: Default::default(), named: 0, heads: Default::default() }
    }

    /// A town's, for one more business beside those named `names`: their words taken as its
    /// key words, their head nouns counted.
    pub fn beside<'a>(culture: usize, names: impl IntoIterator<Item = &'a str>) -> Self {
        let mut l = Self::new(culture);
        for n in names {
            l.keys.extend(crate::t0::names::words(n));
            if MONEY.iter().any(|m| n.contains(m)) {
                l.keys.insert(MONEY_KEY.to_string());
            }
            *l.heads.entry(head_noun(n).to_string()).or_default() += 1;
            l.named += 1;
        }
        l
    }

    /// Settlement `s`'s (`river`: a river runs through it), its lord named by `namer`.
    pub fn of(s: &Settlement, river: bool, namer: &mut Namer) -> Self {
        use crate::t0::settle::Reason;
        let mut l = Self::new(s.culture as usize);
        if river {
            l.landmarks.extend(["Bridge", "Ferry", "Riverside", "Weir"]);
        }
        if s.coastal {
            l.landmarks.extend(["Harbour", "Quay", "Seawall", "Lighthouse"]);
        }
        l.landmarks.extend(match s.reason {
            Reason::Pass => &["Summit", "High Road"][..],
            Reason::Ore => &["Pithead", "Old Mine"],
            Reason::Crossroads => &["Crossroads", "Waystone"],
            Reason::Crossing => &["Ford", "Old Bridge"],
            Reason::Timber => &["Sawmill", "Woodyard"],
            Reason::Fishing => &["Slipway", "Net Loft"],
            Reason::Confluence => &["Two Rivers"],
            Reason::Food => &["Granary", "Haywain"],
            Reason::Spring => &["Spring", "Old Well"],
            Reason::Market => &["Market Cross", "Fair Green"],
            _ => &[],
        });
        l.landmarks.dedup();
        if s.tier >= Tier::Town {
            l.lord = Some(namer.person(l.culture));
        }
        l
    }
}

/// A business name's head noun, what the place is called: "Inn" of "The Bridge Inn", "Temple" of
/// "Temple of the Dawn", "Rest" of "Hrolf's Rest".
pub fn head_noun(name: &str) -> &str {
    let n = name.split(" of ").next().unwrap_or(name);
    n.rsplit(' ').next().unwrap_or(n)
}

/// A shop owner's (or a saint's) name: a culture word of four to seven letters and at most two
/// syllables ("Hrolf", "Maddo").
fn owner(namer: &mut Namer, c: usize) -> String {
    // (Two-syllable words: a far larger space than one-syllable ones, so towns rarely share one.)
    let mut p = namer.word(c, 2, 2);
    for _ in 0..12 {
        if p.len() <= 7 && crate::t0::names::syllables(&p) <= 2 {
            break;
        }
        p = namer.word(c, 2, 2);
    }
    p
}

/// A business's name in its function's style and its town's culture (`None` for plain ones:
/// warehouses, graveyards), drawn from `namer`; its key words are recorded in `loc`, and a name
/// whose key word the town has already used, or whose head noun (`head_noun`) is on more than
/// about one in eight of its businesses, is drawn again. `ward`: where it stands.
pub fn business_name(f: &catalog::Func, namer: &mut Namer, loc: &mut Locale, ward: Option<Ward>) -> Option<String> {
    if f.naming == Naming::Plain {
        return None;
    }
    if f.key == "bank" && loc.keys.contains(MONEY_KEY) {
        return None;
    }
    let c = loc.culture;
    let signs = &catalog::SIGNS[c];
    let mut marks = loc.landmarks.clone();
    // (No "The Pithead Archive": a pithead is no name for a library or a bank.)
    if matches!(f.key, "library" | "bank") {
        marks.retain(|m| *m != "Pithead");
    }
    marks.extend(match ward {
        Some(Ward::Docks) => &["Wharf"][..],
        Some(Ward::Plaza) => &["Market Cross"],
        Some(Ward::Castle | Ward::Military) => &["Gatehouse"],
        Some(Ward::Temple) => &["Pilgrims'"],
        _ => &[],
    });
    let sacred = matches!(f.key, "temple" | "shrine" | "monastery");
    let drinks = matches!(f.key, "inn" | "tavern" | "alehouse");
    // (Halls named for the town's landmarks: "The Harbour Exchange", "The Bridge Library".)
    let marked = matches!(f.key, "library" | "bank" | "town_hall" | "theater" | "merchant_guild" | "craft_guild" | "courthouse" | "auction_house" | "bathhouse");
    let trades = catalog::trades(f);
    let shop = catalog::shop_signs(f.key);
    for attempt in 0..16 {
        let r = namer.below(100);
        let trade = namer.pick(trades);
        // Late tries: a person's name, which is all but always fresh.
        let late = attempt >= 10;
        let (name, keys): (String, Vec<String>) = match f.naming {
            // An owner's own sign ("Hrolf's Anchor"): a far wider space than adjective and noun.
            Naming::Sign if !late && r < 20 => {
                let n = namer.pick(signs.noun);
                let p = owner(namer, c);
                (format!("{p}'s {n}"), vec![n.to_string(), p])
            }
            Naming::Sign if !late && r < 50 => {
                let n = namer.pick(signs.noun);
                (format!("The {} {n}", namer.pick(signs.adj)), vec![n.to_string()])
            }
            Naming::Sign if !late && r < 63 => {
                let single: Vec<&str> = signs.noun.iter().copied().filter(|n| !n.contains(' ')).collect();
                let (a, b) = (namer.pick(&single), namer.pick(&single));
                if a == b {
                    continue;
                }
                (format!("The {a} and {b}"), vec![a.to_string(), b.to_string()])
            }
            Naming::Sign if !late && r < 65 && drinks && !marks.is_empty() => {
                let m = namer.pick(&marks);
                (format!("The {m} {trade}"), vec![m.to_string()])
            }
            Naming::Sign if !late && r < 70 && drinks && loc.lord.is_some() => {
                let lord = loc.lord.clone().unwrap_or_default();
                (format!("The {lord} Arms"), vec![lord])
            }
            Naming::Sign if !late && r < 72 && drinks && c <= 1 => {
                let a = namer.pick(&catalog::INN_ARMS);
                (format!("The {a} Arms"), vec![a.to_string()])
            }
            Naming::Sign => {
                let p = owner(namer, c);
                let rest = namer.pick(signs.rest);
                let name = if namer.below(5) == 0 { format!("Old {p}'s {rest}") } else { format!("{p}'s {rest}") };
                (name, vec![p])
            }
            Naming::Owner if !late && r < 18 && !shop.is_empty() => {
                let n = namer.pick(shop);
                (format!("The {} {n}", namer.pick(signs.adj)), vec![n.to_string()])
            }
            Naming::Owner => {
                let p = owner(namer, c);
                let name = match namer.below(10) {
                    0 => format!("Old {p}'s {trade}"),
                    1 if matches!(c, 2 | 3) => format!("House of {p}"),
                    _ => format!("{p}'s {trade}"),
                };
                (name, vec![p])
            }
            Naming::Institution if sacred && (late || (matches!(c, 0 | 1) && r < 25)) => {
                let p = namer.person(c);
                let name = if matches!(c, 0 | 1) { format!("{} of St. {p}", f.trade) } else { format!("{} of {p}", f.trade) };
                (name, vec![p])
            }
            Naming::Institution if sacred => {
                let d = namer.pick(signs.gods);
                (format!("{} of {d}", f.trade), vec![d.to_string()])
            }
            // A library or a bank: named for a saint, the lord or a landmark, never filler ("The
            // Grey Library", "The Both Archive").
            Naming::Institution if f.key == "bank" && !late && r < 50 && loc.lord.is_some() => {
                let lord = loc.lord.clone().unwrap_or_default();
                let name = if r < 25 { format!("The Bank of {lord}") } else { format!("{lord}'s Counting House") };
                (name, vec![lord])
            }
            Naming::Institution if matches!(f.key, "library" | "bank") && !late && r < 75 && !marks.is_empty() => {
                let m = namer.pick(&marks);
                (format!("The {m} {trade}"), vec![m.to_string()])
            }
            Naming::Institution if matches!(f.key, "library" | "bank") => {
                let p = owner(namer, c);
                let name = match (f.key, c) {
                    ("library", 0 | 1) => format!("The {} of St. {p}", if r % 2 == 0 { "Library" } else { "Scriptorium" }),
                    _ => format!("The {trade} of {p}"),
                };
                (name, vec![p])
            }
            Naming::Institution if !late && r < 45 => {
                let a = namer.pick(signs.inst);
                (format!("The {a} {trade}"), vec![a.to_string()])
            }
            Naming::Institution if !late && r < 65 && marked && !marks.is_empty() => {
                let m = namer.pick(&marks);
                (format!("The {m} {trade}"), vec![m.to_string()])
            }
            Naming::Institution => {
                let p = owner(namer, c);
                (format!("The {trade} of {p}"), vec![p])
            }
            Naming::Plain => return None,
        };
        let head = head_noun(&name).to_string();
        let crowded = !sacred && loc.heads.get(&head).is_some_and(|&k| k > loc.named / 8);
        if (keys.iter().all(|k| !loc.keys.contains(k)) && !crowded) || attempt == 15 {
            loc.keys.extend(keys);
            if f.key == "bank" {
                loc.keys.insert(MONEY_KEY.to_string());
            }
            *loc.heads.entry(head).or_default() += 1;
            loc.named += 1;
            return Some(name);
        }
    }
    None
}
