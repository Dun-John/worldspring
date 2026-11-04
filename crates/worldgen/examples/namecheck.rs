//! Names QA: samples the namer (`t0::names`) and the business namer (`town::business_name`) for
//! seeds 1-8 the way a world draws them (one namer a world, its sites at their own spots; one a
//! town for its businesses), and prints ~25 names per culture and kind group (settlements, land
//! features, sites, inns and shops), then the counts: filter faults (must be 0), training words
//! given out as they are (must be 0), businesses in one town sharing a key word (must be 0), and
//! repeats (the same inn or shop name in two towns of a world, temples aside; a word reused
//! across a world's settlement names).
//! `--world <seed>` instead: that world's caves, mines and lava tubes, each with the theme its
//! site is built in (a themed name must match it).
//! `cargo run --profile fast -p worldgen --example namecheck -- [--summary | --world <seed>]`
use std::collections::BTreeMap;

use worldgen::core::rng::{Pcg32, hash2};
use worldgen::t0::names::{self, NameKind, Namer};
use worldgen::town::catalog::{self, Ward};
use worldgen::town::{Locale, business_name};

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
    let summary = args.iter().any(|a| a == "--summary");
    // samples[culture][group]: names from every seed.
    let mut samples: Vec<Vec<Vec<String>>> = vec![vec![Vec::new(); GROUPS.len()]; CULTURES.len()];
    let mut n = Counts::default();
    for seed in 1..=8u64 {
        let mut rng = Pcg32::new(seed, 5);
        let mut namer = Namer::with_sites(hash2(seed, 1, 0), hash2(seed, 2, 0));
        let mut shops: BTreeMap<String, usize> = BTreeMap::new();
        let mut settle_words: BTreeMap<String, usize> = BTreeMap::new();
        for c in 0..CULTURES.len() {
            let mut at = || [rng.below(1 << 20) as i64 * 5, rng.below(1 << 20) as i64 * 5];
            let mut out: Vec<Vec<String>> = vec![Vec::new(); GROUPS.len()];
            for _ in 0..60 {
                let s = namer.name_at(NameKind::Settlement, c, at());
                for w in names::words(&s).filter(|w| w.len() > 3) {
                    *settle_words.entry(w).or_default() += 1;
                }
                out[0].push(s);
            }
            for k in LAND {
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
                out[2].push(namer.name_at(kind, c, at()));
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
                    out[3].push(name);
                }
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
        println!("most repeated inns and shops: {}", rep.iter().take(12).map(|(s, k)| format!("{s} +{k}")).collect::<Vec<_>>().join(" · "));
        for t in n.training.iter().take(20) {
            println!("training word: {t}");
        }
    }
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
