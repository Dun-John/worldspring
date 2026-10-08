//! Names. Regions of the map belong to a handful of invented phonetic "cultures" (syllable
//! inventories, not real languages), so nearby places sound related. Natural features mix
//! culture names with descriptive English ("the Greyspine Mountains"). Names are unique
//! across the world.

use std::collections::BTreeSet;

use crate::core::rng::Pcg32;

pub struct Culture {
    onsets: &'static [&'static str],
    vowels: &'static [&'static str],
    codas: &'static [&'static str],
}

/// Cold north, temperate west, warm south, dry lands, open steppe.
pub const CULTURES: [Culture; 5] = [
    Culture {
        onsets: &["k", "sk", "th", "v", "h", "gr", "br", "fj", "st", "r", "b", "t", "j", "dr", "s", "hr"],
        vowels: &["a", "e", "i", "o", "u", "y", "ei", "au", "ae"],
        codas: &["r", "n", "rd", "lk", "nd", "ll", "st", "g", "rn", "k", "", ""],
    },
    Culture {
        onsets: &["b", "br", "c", "d", "f", "g", "gl", "l", "m", "p", "r", "s", "t", "tr", "w", "wh", "ll", "h"],
        vowels: &["a", "e", "i", "o", "ae", "ea", "ow", "y", "ai"],
        codas: &["n", "th", "ck", "r", "ll", "nd", "m", "w", "rn", "", ""],
    },
    Culture {
        onsets: &["v", "c", "t", "s", "m", "l", "p", "r", "qu", "br", "fl", "d", "n", "c", "val"],
        vowels: &["a", "e", "i", "o", "u", "ia", "io", "ae"],
        codas: &["r", "n", "s", "l", "nt", "x", "", "", ""],
    },
    Culture {
        onsets: &["z", "kh", "s", "sh", "r", "m", "q", "t", "d", "h", "j", "b", "n"],
        vowels: &["a", "i", "u", "aa", "ai", "e", "ou"],
        codas: &["r", "n", "m", "h", "d", "sh", "z", "l", "", ""],
    },
    Culture {
        onsets: &["t", "k", "b", "ch", "s", "m", "n", "g", "ts", "zh", "y", "d", "l"],
        vowels: &["a", "o", "u", "e", "i", "ao", "ei"],
        codas: &["n", "ng", "r", "k", "l", "", ""],
    },
];

pub const ADJ: [&str; 32] = [
    "Grey", "Iron", "Frost", "Ash", "Storm", "Silver", "Black", "Red", "White", "Golden", "Shadow", "Thunder", "Wind",
    "Stone", "Bleak", "Moon", "Sun", "Dragon", "Raven", "Wolf", "Ember", "Mist", "Thorn", "Elder", "Green", "Deep",
    "High", "Broken", "Sorrow", "Whisper", "Amber", "Hollow",
];

const RANGE_NOUN: [&str; 9] = ["spine", "teeth", "crown", "wall", "fang", "horn", "reach", "spire", "back"];

/// Place-name endings per culture (same order as `CULTURES`).
const ENDINGS: [&[&str]; 5] = [
    &["heim", "vik", "gard", "fell", "by", "stad", "holm", "dal"],
    &["ton", "wick", "ford", "ham", "bury", "dale", "mere", "field", "bridge", "stead"],
    &["ia", "um", "ona", "ara", "ium", "essa", "ano", "ella"],
    &["abad", "ar", "im", "esh", "ra", "an", "oum"],
    &["an", "ur", "gai", "tan", "ol", "khan", "su"],
];

const PLACE_ADJ: [&str; 16] = ["Oak", "Raven", "Stone", "Mill", "Wolf", "Ash", "Iron", "Salt", "Thorn", "Elm", "Bram", "Gold", "Frost", "Black", "Red", "High"];
const PLACE_NOUN: [&str; 14] = ["ford", "hollow", "bridge", "wick", "watch", "haven", "brook", "gate", "moor", "fall", "barrow", "cross", "stead", "keep"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    Continent,
    Ocean,
    Sea,
    Bay,
    Island,
    Range,
    Peak,
    Pass,
    Volcano,
    Lake,
    SaltLake,
    SaltFlat,
    River,
    Waterfall,
    Forest,
    Jungle,
    Taiga,
    Desert,
    Swamp,
    Plains,
    Tundra,
    Glacier,
    Blight,
    Ashlands,
    /// A region drawn and named in the sketch (always named, so rarely asked for).
    Region,
    Settlement,
    Ruin,
    Tower,
    Camp,
    Cave,
    Mine,
    LavaTube,
}

pub struct Namer {
    rng: Pcg32,
    /// Settlements' and sites' own stream (when they have one), so the land's names (more or
    /// fewer regions, say) don't change theirs.
    sites: Option<Pcg32>,
    used: BTreeSet<String>,
}

impl Namer {
    pub fn new(seed: u64) -> Self {
        Self { rng: Pcg32::new(seed, 11), sites: None, used: BTreeSet::new() }
    }

    /// A namer whose settlements and sites draw from their own stream (`site_seed`).
    pub fn with_sites(seed: u64, site_seed: u64) -> Self {
        Self { rng: Pcg32::new(seed, 11), sites: Some(Pcg32::new(site_seed, 13)), used: BTreeSet::new() }
    }

    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.rng.below(xs.len() as u32) as usize]
    }

    /// A culture word of 1–3 syllables, capitalized.
    pub fn word(&mut self, culture: usize, min_syl: u32, max_syl: u32) -> String {
        let c = &CULTURES[culture % CULTURES.len()];
        let syl = min_syl + self.rng.below(max_syl - min_syl + 1);
        let mut s = String::new();
        for k in 0..syl {
            if k == 0 || self.rng.below(3) > 0 {
                s.push_str(self.pick(c.onsets));
            }
            s.push_str(self.pick(c.vowels));
            if k + 1 == syl || self.rng.below(3) == 0 {
                s.push_str(self.pick(c.codas));
            }
        }
        capitalize(&s)
    }

    /// Unique name for a feature of `kind` in the given culture region.
    pub fn name(&mut self, kind: NameKind, culture: usize) -> String {
        use NameKind::*;
        let site = matches!(kind, Settlement | Ruin | Tower | Camp | Cave | Mine | LavaTube);
        if site && let Some(own) = &mut self.sites {
            std::mem::swap(&mut self.rng, own);
            let n = self.unique(kind, culture);
            std::mem::swap(&mut self.rng, self.sites.as_mut().expect("there"));
            return n;
        }
        self.unique(kind, culture)
    }

    fn unique(&mut self, kind: NameKind, culture: usize) -> String {
        for _ in 0..40 {
            let n = self.candidate(kind, culture);
            if self.used.insert(n.clone()) {
                return n;
            }
        }
        // Practically unreachable; lengthen the word until unique.
        loop {
            let n = format!("{} {}", self.candidate(kind, culture), self.word(culture, 1, 1));
            if self.used.insert(n.clone()) {
                return n;
            }
        }
    }

    fn candidate(&mut self, kind: NameKind, c: usize) -> String {
        let r = self.rng.below(4);
        let w = self.word(c, 2, 3);
        let short = self.word(c, 1, 2);
        let adj = self.pick(&ADJ);
        use NameKind::*;
        match (kind, r) {
            (Continent, _) => w,
            (Ocean, 0 | 1) => format!("The {w} Ocean"),
            (Ocean, _) => format!("The {adj} Ocean"),
            (Sea, 0 | 1) => format!("The {w} Sea"),
            (Sea, _) => format!("The {adj} Sea"),
            (Bay, 0) => format!("Gulf of {w}"),
            (Bay, 1) => format!("The {adj} Bay"),
            (Bay, _) => format!("{w} Bay"),
            (Island, 0) => format!("Isle of {w}"),
            (Island, 1) => format!("{adj} Isle"),
            (Island, _) => format!("{w}"),
            (Range, 0) => format!("The {w} Mountains"),
            (Range, 1) => format!("The {}{} Mountains", adj, self.pick(&RANGE_NOUN)),
            (Range, 2) => format!("The {adj} Peaks"),
            (Range, _) => format!("The {w} Range"),
            (Peak, 0 | 1) => format!("Mount {w}"),
            (Peak, 2) => format!("{w} Peak"),
            (Peak, _) => format!("The {adj} Horn"),
            (Pass, 0 | 1) => format!("{short} Pass"),
            (Pass, _) => format!("The {adj} Gap"),
            (Volcano, 0 | 1) => format!("Mount {w}"),
            (Volcano, _) => format!("{adj}fire"),
            (Lake, 0 | 1) => format!("Lake {w}"),
            (Lake, 2) => format!("{w} Mere"),
            (Lake, _) => format!("{adj}water"),
            (SaltLake, _) => format!("The {w} Salt Lake"),
            (SaltFlat, _) => format!("The {w} Salt Flats"),
            (River, 0 | 1) => format!("River {w}"),
            (River, 2) => format!("The {w}"),
            (River, _) => format!("{adj}water"),
            (Waterfall, 0 | 1) => format!("{short} Falls"),
            (Waterfall, _) => format!("The {adj} Falls"),
            (Forest, 0) => format!("{w} Forest"),
            (Forest, 1) => format!("The {adj}wood"),
            (Forest, 2) => format!("The {adj} Wood"),
            (Forest, _) => format!("The Forest of {w}"),
            (Jungle, 0 | 1) => format!("The {w} Jungle"),
            (Jungle, _) => format!("The {adj} Wilds"),
            (Taiga, 0 | 1) => format!("The {w} Pinewood"),
            (Taiga, _) => format!("The {adj} Taiga"),
            (Desert, 0) => format!("The {w} Desert"),
            (Desert, 1) => format!("The {adj} Sands"),
            (Desert, _) => format!("The {adj} Waste"),
            (Swamp, 0) => format!("The {w} Marsh"),
            (Swamp, 1) => format!("The {adj}fen"),
            (Swamp, _) => format!("{w} Bog"),
            (Plains, 0) => format!("The {w} Plains"),
            (Plains, 1) => format!("The {adj} Steppe"),
            (Plains, _) => format!("The {w} Downs"),
            (Tundra, _) => format!("The {adj} Barrens"),
            (Blight, 0) => format!("The Blighted Wood of {w}"),
            (Blight, 1) => format!("The {adj} Blight"),
            (Blight, _) => format!("The Withered {}", ["Wood", "Weald", "Grove", "Holt"][r as usize % 4]),
            (Ashlands, 0 | 1) => format!("The {w} Ashlands"),
            (Ashlands, _) => format!("The {adj} Cinders"),
            (Region, 0 | 1) => format!("The {w} Lands"),
            (Region, _) => format!("{w}"),
            (Settlement, 0) => format!("{}{}", self.pick(&PLACE_ADJ), self.pick(&PLACE_NOUN)),
            (Settlement, _) => {
                let stem = self.word(c, 1, 2);
                let end = self.pick(ENDINGS[c % ENDINGS.len()]);
                format!("{stem}{end}")
            }
            (Ruin, 0) => format!("The Ruins of {w}"),
            (Ruin, 1) => format!("Old {w}"),
            (Ruin, _) => format!("The Fallen {}", ["Temple", "Keep", "Citadel", "Shrine", "Hall"][r as usize % 5]),
            (Tower, 0 | 1) => format!("{w}'s Tower"),
            (Tower, _) => format!("The {adj} Spire"),
            (Camp, 0) => format!("{w}'s Camp"),
            (Camp, _) => format!("The {adj} Camp"),
            (Cave, 0) => format!("The {w} Caves"),
            (Cave, 1) => format!("The Caves of {w}"),
            (Cave, _) => format!("The {adj} Hollow"),
            (Mine, 0 | 1) => format!("{w} Mine"),
            (Mine, _) => format!("The {adj} Delve"),
            (LavaTube, 0 | 1) => format!("The {w} Tubes"),
            (LavaTube, _) => format!("The {adj} Lava Tubes"),
            (Glacier, _) => format!("The {w} Icefield"),
        }
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
