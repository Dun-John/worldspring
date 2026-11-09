//! Names QA: samples the namer (`t0::names`) and the business namer (`town::business_name`) for
//! seeds 1-8 the way a world draws them (one namer a world, its sites at their own spots; one a
//! town for its businesses), and prints ~25 names per culture and kind group (settlements, land
//! features, sites, inns and shops), then the counts: filter faults (must be 0), training words
//! given out as they are (must be 0), businesses in one town sharing a key word (must be 0), and
//! repeats (the same inn or shop name in two towns of a world, temples aside; a word reused
//! across a world's settlement names); per culture, the share of settlements named in English
//! and of the commonest ending (last two letters); the most businesses in a town on one head
//! noun (`town::head_noun`, temples aside).
//! Also: the share of culture words drawn from the letter model, culture words hitting the
//! later bans (`names::ban`, must be 0), the most sites of a world on one head noun
//! (`names::site_noun`) and the most filler library and bank names in a world (an adjective or
//! a made-up word before the trade: must be 0).
//! `--world <seed>` instead: that world's caves, mines and lava tubes, each with the theme its
//! site is built in (a themed name must match it).
//! `--hand <seed> [tier]`: that world generated twice, the second time with a settlement (a
//! village, else `tier`) added by hand
//! (Edit › Places) and a site drawn in the sketch; how many of the first's names changed (must
//! be 0) and what the added ones were named.
//! `--ice`: ice caves and the cold ground (ice caves only on snowy ground, `under::cold_of`), seeds 1-8: per biome the
//! generated caves' themes; ice outside Ice/Alpine/Tundra/high Taiga (must be 0); glacier caves; the share of Taiga land
//! the snowy rule admits; and up to 12 caves whose theme changed (seed, `u:<layout>:0`, world ft x,y, biome, old -> new).
//! `cargo run --profile fast -p worldgen --example namecheck -- [--summary | --world <seed> | --hand <seed> | --ice]`
use std::collections::BTreeMap;

use worldgen::core::rng::{Pcg32, hash2};
use worldgen::t0::names::{self, NameKind, Namer};
use worldgen::town::catalog::{self, Ward};
use worldgen::town::{Locale, business_name, head_noun};

const CULTURES: [&str; 5] = ["north", "west", "south", "dry", "steppe"];
const GROUPS: [&str; 4] = ["settlements", "land", "sites", "inns+shops"];
const LAND: [NameKind; 21] = [
    NameKind::Ocean,
    NameKind::Sea,
    NameKind::Bay,
    NameKind::Island,
    NameKind::Range,
    NameKind::Peak,
    NameKind::Pass,
    NameKind::Volcano,
    NameKind::Lake,
    NameKind::River,
    NameKind::Waterfall,
    NameKind::Forest,
    NameKind::Jungle,
    NameKind::Taiga,
    NameKind::Desert,
    NameKind::Swamp,
    NameKind::Plains,
    NameKind::Tundra,
    NameKind::Glacier,
    NameKind::Blight,
    NameKind::Ashlands,
];
const CAVE_THEMES: [&str; 6] = ["cave", "fungal", "crystal", "ice", "flooded", "beast_den"];
const DEEP_THEMES: [&str; 14] = [
    "dungeon", "prison", "temple", "wizard_lair", "bandit_hideout", "dwarven_hall", "goblin_warren", "flooded_vault", "cult_sanctum", "labyrinth", "crypt", "tomb", "ossuary",
    "catacombs",
];
const SHOPS: [&str; 16] = [
    "inn", "inn", "tavern", "tavern", "alehouse", "fine_dining", "bakery", "blacksmith", "tailor", "apothecary", "general_store", "temple", "temple", "shrine", "library", "bank",
];
const LANDMARKS: [&str; 10] = ["Bridge", "Ferry", "Riverside", "Harbour", "Quay", "Summit", "Crossroads", "Pithead", "Granary", "Two Rivers"];
const WARDS: [Ward; 6] = [Ward::Plaza, Ward::Merchant, Ward::Common, Ward::Docks, Ward::Temple, Ward::Craft];

#[derive(Default)]
struct Counts {
    names: usize,
    faults: Vec<String>,
    training: Vec<String>,
    town_keys: usize,
    shop_repeats: usize,
    word_repeats: usize,
    /// Repeated inn and shop names, with how often (all seeds).
    repeated: BTreeMap<String, usize>,
    words_strict: usize,
    /// Per culture: settlements, in English, per ending.
    settled: [usize; 5],
    english: [usize; 5],
    ends: [BTreeMap<String, usize>; 5],
    /// The most businesses of a town on one head noun.
    heads: usize,
    /// Culture words made, and from the letter model; culture words given out hitting a ban.
    made: u64,
    from_model: u64,
    bans: Vec<String>,
    /// The most sites of a world on one head noun (with it), filler library and bank names.
    site_noun: (usize, String),
    filler: usize,
    /// The most Counting House, Exchange, Treasury or Bank names in one town.
    money: usize,
}

/// The world `seed`'s features as (id, name), generated with the sketch strokes `strokes`
/// (JSON) added.
fn features(seed: u32, strokes: &str) -> Vec<(String, String, &'static str, [i64; 2])> {
    let mut file = worldgen::WorldFile { seed, ..Default::default() };
    file.sketch.strokes = serde_json::from_str(strokes).expect("strokes");
    let world = worldgen::World::new(file).expect("a world");
    let t0 = worldgen::t0::T0::generate(&world);
    t0.extra.expect("generated").overlay.features.into_iter().map(|f| (f.id, f.name, f.kind, [f.x as i64, f.y as i64])).collect()
}

/// A world's names, then the same world with a settlement added by hand and a site drawn in the
/// sketch (beside a generated town): the generated names must not change.
fn hand(seed: u32, tier: &str) {
    let before = features(seed, "[]");
    let (fx, fy) = {
        let world = worldgen::World::new(worldgen::WorldFile { seed, ..Default::default() }).expect("a world");
        let t0 = worldgen::t0::T0::generate(&world);
        let s = &t0.settlements[t0.settlements.len() / 2];
        (s.x, s.y)
    };
    let (px, py, sx, sy) = (fx + 6.0 * 5280.0, fy + 2.0 * 5280.0, fx - 3.0 * 5280.0, fy - 4.0 * 5280.0);
    let strokes = format!(
        r#"[{{"tool":"pin","pts":[[{px},{py}]],"tier":"{tier}","added":true}},{{"tool":"site","pts":[[{sx},{sy}]],"kind":"tower"}}]"#
    );
    let after = features(seed, &strokes);
    let was: BTreeMap<&str, (&str, [i64; 2])> = before.iter().map(|(id, n, _, at)| (id.as_str(), (n.as_str(), *at))).collect();
    let mut changed = Vec::new();
    for (id, n, _, at) in &after {
        if let Some(&(o, at0)) = was.get(id.as_str())
            && o != n
        {
            let moved = if at0 != *at { format!(" (moved {at0:?} -> {at:?})") } else { String::new() };
            changed.push(format!("{id}: {o} -> {n}{moved}"));
        }
    }
    let new: Vec<String> = after.iter().filter(|(id, ..)| !was.contains_key(id.as_str())).map(|(_, n, k, _)| format!("{n} ({k})")).collect();
    let gone = before.iter().filter(|(id, ..)| !after.iter().any(|a| &a.0 == id)).count();
    for c in changed.iter().take(12) {
        println!("changed: {c}");
    }
    println!("world {seed} with a {tier} added by hand and a site drawn in the sketch: {} names, {} changed, {} gone, added: {}", before.len(), changed.len(), gone, new.join(" · "));
}

/// A world's caves, mines and lava tubes: name, kind and the theme their site is built in.
fn world_sites(seed: &str) {
    let world = worldgen::World::new(worldgen::WorldFile { seed: seed.parse().expect("a seed"), ..Default::default() }).expect("a world");
    let t0 = worldgen::t0::T0::generate(&world);
    let feats = &t0.extra.as_ref().expect("generated").overlay.features;
    for (i, p) in t0.pois.iter().enumerate().filter(|(_, p)| matches!(p.kind, worldgen::t0::settle::PoiKind::Cave | worldgen::t0::settle::PoiKind::Mine | worldgen::t0::settle::PoiKind::LavaTube)).take(24) {
        let name = feats.iter().find(|f| (f.x - p.x).abs() < 1.0 && (f.y - p.y).abs() < 1.0).map_or("?", |f| f.name.as_str());
        let theme = worldgen::under::generate(&world, &t0, t0.settlements.len() + i, 0).and_then(|it| it.theme).unwrap_or("-");
        println!("{name:40} {:?} {theme}", p.kind);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--world") {
        return world_sites(args.get(i + 1).map_or("1", |s| s.as_str()));
    }
    if let Some(i) = args.iter().position(|a| a == "--hand") {
        return hand(args.get(i + 1).map_or(3, |s| s.parse().expect("a seed")), args.get(i + 2).map_or("village", |s| s.as_str()));
    }
    if args.iter().any(|a| a == "--ice") {
        return ice();
    }
    let summary = args.iter().any(|a| a == "--summary");
    // samples[culture][group]: names from every seed.
    let mut samples: Vec<Vec<Vec<String>>> = vec![vec![Vec::new(); GROUPS.len()]; CULTURES.len()];
    let mut n = Counts::default();
    for seed in 1..=8u64 {
        let mut rng = Pcg32::new(seed, 5);
        let mut namer = Namer::with_sites(hash2(seed, 1, 0), hash2(seed, 2, 0));
        namer.log = Some(Vec::new());
        let mut site_nouns: BTreeMap<String, usize> = BTreeMap::new();
        let mut filler = 0;
        let mut shops: BTreeMap<String, usize> = BTreeMap::new();
        let mut settle_words: BTreeMap<String, usize> = BTreeMap::new();
        for c in 0..CULTURES.len() {
            let mut at = || [rng.below(1 << 20) as i64 * 5, rng.below(1 << 20) as i64 * 5];
            let mut out: Vec<Vec<String>> = vec![Vec::new(); GROUPS.len()];
            for _ in 0..60 {
                let s = namer.name_at(NameKind::Settlement, c, at());
                n.settled[c] += 1;
                n.english[c] += names::is_english(&s) as usize;
                let low = s.to_lowercase();
                *n.ends[c].entry(low[low.len() - 2..].to_string()).or_default() += 1;
                for w in names::words(&s).filter(|w| w.len() > 3) {
                    *settle_words.entry(w).or_default() += 1;
                }
                out[0].push(s);
            }
            // (Kinds of land a culture's region all but never has left out.)
            for k in LAND.into_iter().filter(|k| !matches!((c, k), (0, NameKind::Jungle | NameKind::Desert) | (2 | 3, NameKind::Glacier | NameKind::Tundra | NameKind::Taiga))) {
                for _ in 0..3 {
                    out[1].push(namer.name(k, c));
                }
            }
            for k in 0..60 {
                let (kind, theme) = match k % 6 {
                    0 => (NameKind::Ruin, None),
                    1 => (NameKind::Tower, None),
                    2 => (NameKind::Camp, None),
                    3 => (NameKind::Cave, Some(CAVE_THEMES[(k / 6) % CAVE_THEMES.len()])),
                    4 => (if k % 12 == 4 { NameKind::Mine } else { NameKind::LavaTube }, Some(if k % 12 == 4 { "mine" } else { "lava_tube" })),
                    _ => (NameKind::Ruin, Some(DEEP_THEMES[(k / 6) % DEEP_THEMES.len()])),
                };
                namer.theme = theme;
                let s = namer.name_at(kind, c, at());
                *site_nouns.entry(names::site_noun(&s).to_string()).or_default() += 1;
                out[2].push(s);
                namer.theme = None;
            }
            // Towns: one namer each, as `assign_functions`.
            for t in 0..12u64 {
                let mut tn = Namer::new(hash2(seed, c as i64, t as i64));
                let mut loc = Locale::new(c);
                for (i, m) in LANDMARKS.iter().enumerate() {
                    if hash2(seed ^ t, c as i64, i as i64) % 4 == 0 {
                        loc.landmarks.push(m);
                    }
                }
                loc.lord = Some(tn.person(c));
                let mut used = std::collections::BTreeSet::new();
                let mut heads: BTreeMap<String, usize> = BTreeMap::new();
                let mut money = 0;
                for (i, key) in SHOPS.iter().enumerate() {
                    let f = &catalog::CATALOG[catalog::index_of(key).expect("a catalog key")];
                    let before = loc.keys.len();
                    let Some(name) = business_name(f, &mut tn, &mut loc, Some(WARDS[(hash2(seed, t as i64, i as i64) % WARDS.len() as u64) as usize])) else { continue };
                    if loc.keys.len() == before {
                        n.town_keys += 1;
                    }
                    if f.naming != catalog::Naming::Institution && used.insert(name.clone()) {
                        *shops.entry(name.clone()).or_default() += 1;
                    }
                    // (Filler: "The Grey Library", "The Kolvar Exchange"; not "The Harbour Exchange".)
                    let second = name.split(' ').nth(1).unwrap_or("");
                    if matches!(*key, "library" | "bank") && name.starts_with("The ") && !name.contains(" of ") && !loc.landmarks.iter().any(|m| m.starts_with(second)) && !second.starts_with("Wharf") && !second.starts_with("Market") && !second.starts_with("Gatehouse") && !second.starts_with("Pilgrims") {
                        filler += 1;
                    }
                    if ["Counting House", "Exchange", "Treasury", "Bank"].iter().any(|m| name.contains(m)) {
                        money += 1;
                        n.money = n.money.max(money);
                    }
                    if !matches!(*key, "temple" | "shrine") {
                        let h = heads.entry(head_noun(&name).to_string()).or_default();
                        *h += 1;
                        n.heads = n.heads.max(*h);
                    }
                    out[3].push(name);
                }
                n.made += tn.made as u64;
                n.from_model += tn.from_model as u64;
            }
            // Bare culture words and people's names, against the strict rules.
            for _ in 0..200 {
                let w = namer.word(c, 1, 3);
                let p = namer.person(c);
                for w in [w, p] {
                    if let Some(f) = names::fault(&w, Some(c)) {
                        n.faults.push(format!("{w} ({}, word: {f})", CULTURES[c]));
                    }
                    if names::is_training(&w) {
                        n.training.push(w);
                    }
                    n.words_strict += 1;
                }
            }
            for (g, names_) in out.into_iter().enumerate() {
                for s in &names_ {
                    n.names += 1;
                    if let Some((w, f)) = names::name_fault(s) {
                        n.faults.push(format!("{s} ({}, \"{w}\": {f})", CULTURES[c]));
                    }
                    if names::words(s).any(|w| names::is_training(&w)) {
                        n.training.push(s.clone());
                    }
                }
                // Three or four a seed, spread through the draws.
                let take = if seed <= 1 { 4 } else { 3 };
                samples[c][g].extend((0..take).map(|j| names_[(seed as usize * 37 + j * 53) % names_.len()].clone()));
            }
        }
        n.shop_repeats += shops.values().filter(|&&k| k > 1).map(|k| k - 1).sum::<usize>();
        for (s, k) in shops.into_iter().filter(|e| e.1 > 1) {
            *n.repeated.entry(s).or_default() += k - 1;
        }
        n.word_repeats += settle_words.values().filter(|&&k| k > 1).map(|k| k - 1).sum::<usize>();
        n.made += namer.made as u64;
        n.from_model += namer.from_model as u64;
        for w in namer.log.take().unwrap_or_default() {
            if let Some(f) = names::ban(&w) {
                n.bans.push(format!("{w} ({f})"));
            }
        }
        if let Some((s, &k)) = site_nouns.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
            && k > n.site_noun.0
        {
            n.site_noun = (k, s.clone());
        }
        n.filler = n.filler.max(filler);
    }
    if !summary {
        for (c, groups) in samples.iter().enumerate() {
            for (g, names_) in groups.iter().enumerate() {
                println!("== {} {}", CULTURES[c], GROUPS[g]);
                let mut line = String::new();
                for s in names_ {
                    if !line.is_empty() && line.len() + s.len() > 150 {
                        println!("  {line}");
                        line.clear();
                    }
                    if !line.is_empty() {
                        line.push_str(" · ");
                    }
                    line.push_str(s);
                }
                if !line.is_empty() {
                    println!("  {line}");
                }
            }
        }
        for f in n.faults.iter().take(20) {
            println!("fault: {f}");
        }
        let mut rep: Vec<(&String, &usize)> = n.repeated.iter().collect();
        rep.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        println!("most repeated inns and shops: {}", rep.iter().take(if summary { 12 } else { 100 }).map(|(s, k)| format!("{s} +{k}")).collect::<Vec<_>>().join(" · "));
        for t in n.training.iter().take(20) {
            println!("training word: {t}");
        }
        for b in n.bans.iter().take(20) {
            println!("ban: {b}");
        }
    }
    let mut per = Vec::new();
    for c in 0..CULTURES.len() {
        let (top, k) = n.ends[c].iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))).map_or(("", 0), |(e, k)| (e.as_str(), *k));
        let share = |k: usize| 100 * k / n.settled[c].max(1);
        per.push(format!("{} {}% English, -{top} {}%", CULTURES[c], share(n.english[c]), share(k)));
    }
    println!("settlements: {}", per.join(" · "));
    println!("most businesses in a town on one head noun: {}", n.heads);
    println!(
        "culture words from the letter model: {}% · culture words hitting the later bans: {} · most sites of a world on one noun: {} ({}) · most filler library and bank names in a world: {} · most Counting House/Exchange/Treasury/Bank in a town: {}",
        100 * n.from_model / n.made.max(1),
        n.bans.len(),
        n.site_noun.0,
        n.site_noun.1,
        n.filler,
        n.money
    );
    println!(
        "{} names, {} culture words: {} faults, {} training words, {} town key-word clashes, {} inn and shop names repeated across a world's towns, {} settlement words reused in a world",
        n.names,
        n.words_strict,
        n.faults.len(),
        n.training.len(),
        n.town_keys,
        n.shop_repeats,
        n.word_repeats
    );
}

/// `--ice`: see the header.
fn ice() {
    use worldgen::t0::biome::Biome;
    use worldgen::t0::settle::PoiKind;
    use worldgen::under::{Cold, UnderKind, surface::theme_in};
    let mut per: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    let (mut outside, mut glacier, mut changed, mut caves) = (0, 0, 0, 0);
    let (mut taiga, mut taiga_snowy) = (0usize, 0usize);
    let mut tall: Vec<f32> = Vec::new();
    let mut list = Vec::new();
    for seed in 1..=8u32 {
        let world = worldgen::World::new(worldgen::WorldFile { seed, ..Default::default() }).expect("a world");
        let t0 = worldgen::t0::T0::generate(&world);
        let sea = world.params().sea_level_ft;
        for k in 0..t0.height.data.len() {
            if Biome::from_u8((t0.biome.data[k] & 0xff) as u8) == Biome::Taiga && t0.coast.data[k] == 0.0 {
                taiga += 1;
                tall.push(t0.height.data[k] - sea as f32);
                taiga_snowy += (worldgen::under::cold_of(t0.biome.data[k], t0.height.data[k], sea) == Cold::Snowy) as usize;
            }
        }
        for (p, poi) in t0.pois.iter().enumerate() {
            if poi.kind != PoiKind::Cave {
                continue;
            }
            let layout = t0.settlements.len() + p;
            let (i, j) = ((poi.x / t0.cell_ft).round() as usize, (poi.y / t0.cell_ft).round() as usize);
            let b = Biome::from_u8((t0.biome.get(i.min(t0.biome.w - 1), j.min(t0.biome.h - 1)) & 0xff) as u8);
            let cold = t0.cold_at(&world, poi.x, poi.y);
            let new = theme_in(&world, &t0, layout, 0, UnderKind::Cave, true, cold).key;
            let old = theme_in(&world, &t0, layout, 0, UnderKind::Cave, true, Cold::Snowy).key;
            caves += 1;
            *per.entry((b.name(), new)).or_default() += 1;
            outside += (new == "ice" && cold == Cold::No) as usize;
            glacier += (new == "ice" && cold == Cold::Glacier) as usize;
            if new != old {
                changed += 1;
                if list.len() < 12 {
                    list.push(format!("seed {seed} u:{layout}:0 ({:.0},{:.0}) {} {old} -> {new}", poi.x, poi.y, b.name()));
                }
            }
        }
    }
    println!("caves {caves}; ice outside snowy ground {outside}; glacier caves {glacier}; changed {changed}");
    println!("taiga land snowy: {taiga_snowy}/{taiga} = {:.1}%", 100.0 * taiga_snowy as f64 / taiga.max(1) as f64);
    tall.sort_by(|a, b| a.total_cmp(b));
    println!("taiga height above sea, ft: p33 {:.0} p50 {:.0} p67 {:.0} p90 {:.0}", tall[tall.len() / 3], tall[tall.len() / 2], tall[tall.len() * 2 / 3], tall[tall.len() * 9 / 10]);
    for ((b, t), n) in per {
        println!("{b:<12} {t:<10} {n}");
    }
    for l in list {
        println!("{l}");
    }
}
