//! Names. Regions of the map belong to a handful of invented "cultures" (cold north, temperate
//! west, warm south, dry lands, open steppe), so nearby places sound related. A culture word comes
//! either from a small letter model (order-2 Markov, learnt from a short approved list of words per
//! culture) or from a shape template on the culture's onsets, vowels and codas; every one passes a
//! filter of sayable letter runs (`fault`) and is never a word of the lists itself. Places mix
//! culture words with English in each culture's measure (the west mostly English compounds, the
//! others mostly their own words), with "X of Y" forms, saints and kings, and grim names mostly on
//! dangerous ground. Names are unique across the world.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::core::rng::Pcg32;

pub struct Culture {
    onsets: &'static [&'static str],
    vowels: &'static [&'static str],
    codas: &'static [&'static str],
    /// The approved words its letter model learns from (never given out as they are).
    train: &'static [&'static str],
    /// Endings put on a culture word for a place.
    endings: &'static [&'static str],
    /// English place-name pieces in its flavour: first parts and last parts ("Raven" + "holm").
    pre: &'static [&'static str],
    suf: &'static [&'static str],
    /// Its own words for describing land, beside the shared `ADJ`.
    adj: &'static [&'static str],
    /// The share of its settlements named in English, and of its land features.
    english: f64,
    nature: f64,
}

/// Cold north, temperate west, warm south, dry lands, open steppe.
pub const CULTURES: [Culture; 5] = [
    Culture {
        onsets: &["k", "sk", "th", "v", "h", "gr", "br", "fj", "st", "r", "b", "t", "j", "dr", "s", "hr", "sv", "bj"],
        vowels: &["a", "e", "i", "o", "u", "y", "ei", "au"],
        codas: &["r", "n", "rd", "lk", "nd", "ll", "st", "g", "rn", "k", "", ""],
        train: &[
            "Skeld", "Thorvald", "Hrossey", "Brennvik", "Fjalsted", "Grimmar", "Kaldur", "Vestrey", "Haukrun", "Sigvald", "Torsund", "Brynjar", "Eskil", "Hjaldur", "Skarna",
            "Vidrun", "Jorund", "Starkad", "Halvard", "Dravik", "Sveinar", "Ulfrik", "Kolbrun", "Asmund", "Thyra", "Bjarke", "Folkvar", "Vargrun", "Haldis", "Ragnvald",
        ],
        endings: &["heim", "vik", "gard", "fell", "by", "stad", "holm", "dal", "sund", "ness", "vang", "rud"],
        pre: &[
            "Raven", "Wolf", "Bear", "Frost", "Elk", "Storm", "Rime", "Grey", "Salt", "Iron", "Black", "White", "Ash", "Thorn", "Whale", "Seal", "Crow", "Hawk", "Stone", "Birch",
            "Pine", "Skerry", "Troll", "Gull", "Horn", "Snow", "Cold", "Fir", "Wind", "Hammer", "Eagle", "Otter", "Shield", "Ember", "Mist",
        ],
        suf: &["holm", "fell", "vik", "wick", "ness", "by", "thorpe", "gard", "stead", "dale", "scar", "garth", "sund", "haven", "howe", "beck", "mere", "hold", "watch", "gate"],
        adj: &["Rime", "Frost", "Grey", "Raven", "Wolf", "Bleak", "Iron", "Storm", "Ice", "Cold", "White", "Troll", "Giant", "Hoar"],
        english: 0.25,
        nature: 0.5,
    },
    Culture {
        onsets: &["b", "br", "c", "d", "f", "g", "gl", "l", "m", "p", "r", "s", "t", "tr", "w", "wh", "ll", "h"],
        vowels: &["a", "e", "i", "o", "ea", "ow", "y", "ai"],
        codas: &["n", "th", "ck", "r", "ll", "nd", "m", "w", "rn", "", ""],
        train: &[
            "Aldric", "Brannoc", "Caerwen", "Dunmar", "Elwen", "Gwyllan", "Haddow", "Llandry", "Tregarth", "Wendell", "Whitlow", "Glenna", "Corwen", "Rhydder", "Maelor",
            "Tamsin", "Edric", "Carrow", "Haverel", "Lyndon", "Merrick", "Odwin", "Pellam", "Ravell", "Talwyn", "Cador", "Fennick", "Hollis", "Kendal", "Brython",
        ],
        endings: &["ton", "wick", "ford", "ham", "bury", "dale", "mere", "field", "bridge", "stead", "worth", "ley", "combe", "more", "by", "well"],
        pre: &[
            "Oak", "Ash", "Elm", "Bram", "Thorn", "Mill", "Black", "Red", "White", "Green", "Hart", "Wolf", "Raven", "Stone", "Salt", "Iron", "Gold", "High", "Long", "Broad",
            "Nether", "Fern", "Hazel", "Willow", "Bracken", "Heather", "Cold", "Wind", "Barrow", "Hay", "Swan", "Crane", "Otter", "Fox", "Badger", "Wren", "Lark", "Honey",
            "Apple", "Cherry", "Rose", "Bell", "Chapel", "Holly", "Alder", "Rowan", "Beech", "Marsh", "Moor", "Sheep", "Wool", "Mead", "Elder", "Linden", "Bright", "Tinker",
            "Copper", "Frost",
        ],
        suf: &[
            "ford", "ton", "bury", "ham", "wick", "field", "bridge", "stead", "dale", "mere", "brook", "combe", "ley", "worth", "minster", "cott", "hurst", "well", "wood", "gate",
            "moor", "barrow", "cross", "haven", "stow", "hollow", "watch", "keep", "hythe", "thorpe", "borough", "wold", "cliffe", "den", "holt",
        ],
        adj: &["Green", "Elder", "Hollow", "Whisper", "Mist", "Silver", "Hart", "Thorn", "Oak", "Willow", "Bramble", "Amber", "Grey", "Bright"],
        english: 0.8,
        nature: 0.65,
    },
    Culture {
        onsets: &["v", "c", "t", "s", "m", "l", "p", "r", "br", "fl", "d", "n", "c", "val", "f", "g", "b"],
        vowels: &["a", "e", "i", "o", "u", "ia", "io"],
        codas: &["r", "n", "s", "l", "nt", "", "", ""],
        train: &[
            "Valeria", "Castor", "Mirella", "Avento", "Corvina", "Lucano", "Serrano", "Tavira", "Ostra", "Benevar", "Calvessa", "Dorano", "Marcellin", "Peretta", "Quintar",
            "Rovesa", "Teronna", "Varenza", "Aurelo", "Brisano", "Celano", "Fiora", "Lorenza", "Montaro", "Neviano", "Palestro", "Riona", "Solvena", "Ternio", "Vasca",
        ],
        endings: &["ia", "um", "ona", "ara", "essa", "ano", "ella", "ino", "era", "enza", "ora", "eto", "ani", "esca"],
        pre: &["Sun", "Gold", "Olive", "Vine", "Lemon", "Salt", "Rose", "Bright", "White", "Amber", "Coral", "Cypress", "Myrtle", "Laurel", "Fig", "Dove"],
        suf: &["haven", "port", "bay", "cliff", "vale", "grove", "hill", "watch", "mere", "shore", "crest", "bridge"],
        adj: &["Sun", "Amber", "Gold", "Azure", "Coral", "Laurel", "Olive", "Crimson", "Bright", "Dawn", "Vine", "Myrtle", "Marble", "Sapphire"],
        english: 0.2,
        nature: 0.5,
    },
    Culture {
        onsets: &["z", "kh", "s", "sh", "r", "m", "q", "t", "d", "h", "j", "b", "n"],
        vowels: &["a", "i", "u", "ai", "e", "ou"],
        codas: &["r", "n", "m", "h", "d", "sh", "z", "l", "", ""],
        train: &[
            "Zahirah", "Qasim", "Sharaz", "Kharim", "Darjan", "Hamadi", "Mirzan", "Rashad", "Tabriq", "Baharun", "Nasira", "Jafira", "Sulaim", "Qarra", "Zaroun", "Khadesh",
            "Sahrin", "Dumaz", "Hirun", "Basrim", "Yazran", "Tamuz", "Ishara", "Nadrim", "Ruzhan", "Shiraqa", "Zafir", "Amunet", "Kesra", "Halabi",
        ],
        endings: &["abad", "ar", "im", "esh", "ra", "an", "oum", "iya", "una", "ash", "is", "ira"],
        pre: &["Sand", "Sun", "Salt", "Dust", "Red", "Gold", "Palm", "Brass", "Copper", "Dune", "Amber", "Jackal", "Falcon", "Ochre", "Saffron"],
        suf: &["well", "gate", "wall", "spring", "rock", "tower", "haven", "cross", "hold", "reach", "stone", "pool"],
        adj: &["Red", "Amber", "Dust", "Brass", "Sun", "Scorched", "Golden", "Burning", "Copper", "Ochre", "Saffron", "Bone", "Glass", "Shimmering"],
        english: 0.15,
        nature: 0.5,
    },
    Culture {
        onsets: &["t", "k", "b", "ch", "s", "m", "n", "g", "ts", "zh", "y", "d", "l"],
        vowels: &["a", "o", "u", "e", "i"],
        codas: &["n", "ng", "r", "k", "l", "", ""],
        train: &[
            "Temur", "Bayan", "Khulan", "Tsogt", "Batu", "Orkhon", "Yesun", "Altan", "Chaghan", "Dorgai", "Kerul", "Mingan", "Naran", "Sukhe", "Toghan", "Ulaan", "Bortai",
            "Ergen", "Khasar", "Ordu", "Tengis", "Burqan", "Selenge", "Yisu", "Chono", "Daran", "Qara", "Jebei", "Munkh", "Sartaq",
        ],
        endings: &["an", "ur", "gai", "tan", "ol", "khan", "su", "tai", "gol", "dar", "in", "ai"],
        pre: &["Horse", "Wind", "Grass", "Sky", "Eagle", "Wolf", "Red", "White", "Blue", "Hawk", "Mare", "Arrow", "Yak", "Felt", "Banner", "Kite"],
        suf: &["ford", "well", "camp", "stone", "hill", "field", "cairn", "moot", "fold", "ring", "mound"],
        adj: &["Blue", "Sky", "Wind", "Grass", "Horse", "Eagle", "Endless", "Wide", "Thunder", "Golden", "White", "Wild", "Whistling", "Silver"],
        english: 0.15,
        nature: 0.5,
    },
];

pub const ADJ: [&str; 44] = [
    "Grey", "Iron", "Frost", "Ash", "Storm", "Silver", "Black", "Red", "White", "Golden", "Shadow", "Thunder", "Wind", "Stone", "Bleak", "Moon", "Sun", "Dragon", "Raven",
    "Wolf", "Ember", "Mist", "Thorn", "Elder", "Green", "Deep", "High", "Broken", "Sorrow", "Whisper", "Amber", "Hollow", "Bright", "Cold", "Long", "Far", "Dawn", "Dusk",
    "Star", "Copper", "Wild", "Pale", "Twin", "Giant",
];

const RANGE_NOUN: [&str; 9] = ["spine", "teeth", "crown", "wall", "fang", "horn", "reach", "spire", "back"];

/// "The Vale of ___".
const OF_Y: [&str; 26] = [
    "Storms", "Whispers", "Echoes", "Mists", "Thorns", "Kings", "Giants", "Ravens", "Winds", "Stars", "Glass", "Lanterns", "Crowns", "Wolves", "Tides", "Dawn", "Silence",
    "Bells", "Spires", "Eagles", "Serpents", "Sorrows", "Thunder", "Mirrors", "Dreams", "Embers",
];

/// Grim words: "The Drowned Mire", "Widow's Pass", "The Marsh of Tears", "Gallowsford".
const GRIM_ADJ: [&str; 25] = [
    "Dead", "Drowned", "Bleak", "Murk", "Grim", "Dread", "Gloom", "Wraith", "Carrion", "Rotting", "Blood", "Bone", "Shrieking", "Weeping", "Cursed", "Forsaken", "Lost",
    "Hanging", "Mourning", "Corpse", "Ghoul", "Gallows", "Raven", "Witching", "Sunless",
];
const GRIM_POSS: [&str; 10] = ["Deadman's", "Widow's", "Hangman's", "Devil's", "Beggar's", "Crow's", "Wight's", "Witch's", "Traitor's", "Ghoul's"];
const GRIM_OF: [&str; 14] = ["Sorrows", "Bones", "Tears", "Woe", "the Dead", "Ashes", "Despair", "the Lost", "Shadows", "Graves", "Whispers", "Ruin", "Lament", "the Hanged"];
const GRIM_PRE: [&str; 12] = ["Gallows", "Dead", "Murk", "Gloom", "Crow", "Grave", "Bone", "Mourn", "Wither", "Black", "Rot", "Grim"];

/// Kings and saints (the north and the west): "Kingsbridge", "Queensmere".
const ROYAL: [&str; 10] = ["Kings", "Queens", "Princes", "Earls", "Bishops", "Abbots", "Monks", "Friars", "Knights", "Lords"];
const ROYAL_SUF: [&str; 15] = ["bridge", "mere", "ford", "wood", "well", "gate", "bury", "ton", "haven", "holm", "field", "cross", "barrow", "stow", "hold"];

/// Words for an underground site by its theme (`under::THEMES` keys): its describing words and
/// its nouns ("The Rime Caves", "Glimmer Grotto").
const THEME_WORDS: [(&str, &[&str], &[&str]); 25] = [
    ("cave", &["Echo", "Deep", "Dark", "Drip", "Whispering", "Bat", "Long", "Hollow"], &["Caves", "Caverns", "Hollow", "Grotto"]),
    ("fungal", &["Mould", "Spore", "Toadstool", "Pale", "Puffball", "Mushroom", "Glowcap"], &["Caves", "Grotto", "Hollow", "Caverns"]),
    ("crystal", &["Glimmer", "Shard", "Crystal", "Gleam", "Prism", "Glitter"], &["Caves", "Grotto", "Caverns", "Hollow"]),
    ("ice", &["Rime", "Frost", "White", "Ice", "Hoar", "Frozen"], &["Caves", "Caverns", "Hollow", "Grotto"]),
    ("flooded", &["Drowned", "Sunken", "Weeping", "Flooded", "Dripping", "Still Water"], &["Caves", "Caverns", "Grotto", "Pools"]),
    ("beast_den", &["Wolf", "Bear", "Fang", "Claw", "Howling", "Gnawbone"], &["Den", "Caves", "Lair", "Hollow"]),
    ("mine", &["Copper", "Iron", "Silver", "Tin", "Lode", "Deep", "Old", "Lantern", "Coal"], &["Mine", "Delve", "Workings", "Diggings"]),
    ("lava_tube", &["Ember", "Cinder", "Smoke", "Burnt", "Fire", "Scorch", "Black", "Glass"], &["Tubes", "Lava Tubes", "Burrows", "Vents"]),
    ("dungeon", &["Iron", "Black", "Grim", "Old", "Deep", "Lost"], &["Dungeon", "Vaults", "Halls", "Depths"]),
    ("prison", &["Chain", "Shackle", "Iron", "Dread", "Lock", "Weeping"], &["Pits", "Cells", "Gaol", "Oubliette"]),
    ("temple", &["Hallowed", "Holy", "Sun", "Moon", "Silent", "Gilded"], &["Temple", "Sanctum", "Halls"]),
    ("sunken_temple", &["Sunken", "Drowned", "Silted", "Weeping"], &["Temple", "Sanctum", "Shrine"]),
    ("wizard_lair", &["Star", "Runed", "Moon", "Sorcerer's", "Mage's", "Arcane"], &["Vaults", "Sanctum", "Halls", "Lair"]),
    ("bandit_hideout", &["Robbers'", "Thieves'", "Cutthroat", "Smugglers'", "Outlaws'", "Brigands'"], &["Hole", "Den", "Hideout", "Cellars"]),
    ("dwarven_hall", &["Anvil", "Deep", "Stone", "Hammer", "Forge", "Rune"], &["Halls", "Hold", "Delve", "Deeps"]),
    ("goblin_warren", &["Goblin", "Gnasher", "Muck", "Rat", "Snaggle", "Grub"], &["Warren", "Holes", "Burrows", "Tunnels"]),
    ("flooded_vault", &["Sunken", "Drowned", "Brine", "Flooded"], &["Vaults", "Cisterns", "Halls"]),
    ("cult_sanctum", &["Whispering", "Veiled", "Hooded", "Blood", "Shadow", "Black"], &["Sanctum", "Chapel", "Halls", "Altar"]),
    ("labyrinth", &["Winding", "Twisting", "Maze", "Lost", "Endless", "Coiled"], &["Labyrinth", "Maze", "Ways", "Halls"]),
    ("crypt", &["Bone", "Silent", "Grave", "Pale", "Still", "Hushed"], &["Crypt", "Tombs", "Vaults"]),
    ("tomb", &["King's", "Sealed", "Gilded", "Dust", "Barrow", "Queen's"], &["Tomb", "Barrow", "Vault"]),
    ("ossuary", &["Bone", "Skull", "Charnel", "Rattling"], &["Ossuary", "Vaults", "Pits"]),
    ("catacombs", &["Bone", "Deep", "Candle", "Silent", "Old"], &["Catacombs", "Tunnels", "Vaults"]),
    ("undercroft", &["Old", "Vaulted", "Abbey", "Cellar"], &["Undercroft", "Cellars", "Vaults"]),
    ("collapsed_fortress", &["Broken", "Fallen", "War", "Iron", "Shattered"], &["Fortress", "Bastion", "Keep", "Halls"]),
];

/// Three consonants that may stand together in a culture word ("Skjald", "Halstra"); any other run
/// of three, and every longer run, is refused.
const CLUSTERS: [&str; 38] = [
    "skj", "str", "ngr", "ndr", "rst", "nst", "ldr", "rth", "lth", "nth", "thr", "shr", "chr", "spr", "scr", "skr", "ght", "nch", "rch", "lch", "nkh", "rkh", "lkh", "rsh",
    "lsh", "nsh", "ngh", "nds", "rds", "lds", "lst", "nsk", "rsk", "mbr", "mbl", "ngl", "rdh", "ldh",
];
/// Letter pairs no name has.
const BANNED: [&str; 18] = ["jk", "qk", "vj", "gk", "kg", "tl", "dt", "hh", "yy", "wq", "xz", "ii", "uu", "jj", "vv", "ww", "qq", "xx"];

/// The kinds of place where grim names belong.
fn dangerous(kind: NameKind) -> bool {
    use NameKind::*;
    matches!(kind, Swamp | Blight | Ashlands | Ruin | Cave | Mine | LavaTube | Pass)
}

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

/// A culture's letter model: how often each letter (1–26; 0 the word's edge) follows each pair.
struct Markov {
    /// (a, b, c) sorted, each with how often c follows a, b.
    next: Vec<([u8; 3], u32)>,
}

impl Markov {
    fn learn(words: &[&str]) -> Self {
        let mut counts: BTreeMap<[u8; 3], u32> = BTreeMap::new();
        for w in words {
            let mut s = vec![0u8, 0];
            s.extend(w.bytes().filter(u8::is_ascii_alphabetic).map(|b| b.to_ascii_lowercase() - b'a' + 1));
            s.push(0);
            for t in s.windows(3) {
                *counts.entry([t[0], t[1], t[2]]).or_default() += 1;
            }
        }
        Self { next: counts.into_iter().collect() }
    }

    /// A word of at most `max` letters, lower case (None: the walk ran long).
    fn walk(&self, rng: &mut Pcg32, max: usize) -> Option<String> {
        let (mut a, mut b) = (0u8, 0u8);
        let mut s = String::new();
        loop {
            let lo = self.next.partition_point(|(t, _)| (t[0], t[1]) < (a, b));
            let hi = self.next.partition_point(|(t, _)| (t[0], t[1]) <= (a, b));
            let total: u32 = self.next[lo..hi].iter().map(|e| e.1).sum();
            if total == 0 {
                return None;
            }
            let mut r = rng.below(total);
            let mut c = 0;
            for (t, n) in &self.next[lo..hi] {
                if r < *n {
                    c = t[2];
                    break;
                }
                r -= n;
            }
            if c == 0 {
                return Some(s);
            }
            s.push((b'a' + c - 1) as char);
            if s.len() > max {
                return None;
            }
            (a, b) = (b, c);
        }
    }
}

fn markov(culture: usize) -> &'static Markov {
    static MODELS: OnceLock<Vec<Markov>> = OnceLock::new();
    &MODELS.get_or_init(|| CULTURES.iter().map(|c| Markov::learn(c.train)).collect())[culture]
}

/// A word of the approved lists (any culture's), as it stands.
pub fn is_training(word: &str) -> bool {
    CULTURES.iter().any(|c| c.train.iter().any(|t| t.eq_ignore_ascii_case(word)))
}

fn vowel_at(b: &[u8], i: usize) -> bool {
    let plain = |c: u8| matches!(c, b'a' | b'e' | b'i' | b'o' | b'u');
    match b[i] {
        c if plain(c) => true,
        // y is a vowel between consonants (or at an end beside one).
        b'y' => !(i > 0 && plain(b[i - 1])) && !(i + 1 < b.len() && plain(b[i + 1])),
        _ => false,
    }
}

/// What is wrong with one word of a name, if anything. `culture`: it is a culture word of that
/// culture (the strict rules: 10 letters at most, only the allowed consonant clusters, `q` only in
/// the dry and steppe tongues); otherwise a compound or English word (14 letters, runs of up to 5
/// consonants: "Kingsbridge").
pub fn fault(word: &str, culture: Option<usize>) -> Option<&'static str> {
    let b: Vec<u8> = word.bytes().filter(u8::is_ascii_alphabetic).map(|c| c.to_ascii_lowercase()).collect();
    if b.len() > if culture.is_some() { 10 } else { 14 } {
        return Some("too long");
    }
    for p in b.windows(2) {
        if BANNED.iter().any(|x| x.as_bytes() == p) {
            return Some("banned pair");
        }
    }
    for (i, &c) in b.iter().enumerate() {
        if c == b'q' && !matches!(b.get(i + 1), Some(b'u' | b'a')) {
            return Some("q not before u or a");
        }
    }
    if culture.is_some_and(|c| c != 3 && c != 4) && b.contains(&b'q') {
        return Some("q outside the dry and steppe tongues");
    }
    if b.windows(3).any(|t| t[0] == t[1] && t[1] == t[2]) {
        return Some("tripled letter");
    }
    let (mut vs, mut cs) = (0, 0);
    for i in 0..b.len() {
        if vowel_at(&b, i) {
            (vs, cs) = (vs + 1, 0);
        } else {
            (vs, cs) = (0, cs + 1);
        }
        if vs >= 3 {
            return Some("three vowels");
        }
        match culture {
            Some(_) if cs == 3 && !CLUSTERS.iter().any(|x| x.as_bytes() == &b[i - 2..=i]) => return Some("consonant cluster"),
            Some(_) if cs >= 4 => return Some("four consonants"),
            None if cs >= 6 => return Some("six consonants"),
            _ => {}
        }
    }
    None
}

/// The first fault in a whole name (each of its words as a compound), with the word.
pub fn name_fault(name: &str) -> Option<(String, &'static str)> {
    words(name).find_map(|w| fault(&w, None).map(|f| (w, f)))
}

/// A name's words, without possessives and punctuation ("St. Aldric's Cross": St, Aldric, Cross).
pub fn words(name: &str) -> impl Iterator<Item = String> + '_ {
    name.split([' ', '-']).map(|w| w.trim_end_matches("'s").trim_end_matches('\'').chars().filter(char::is_ascii_alphabetic).collect::<String>()).filter(|w| !w.is_empty())
}

/// `a` and `b` run together ("Raven" + "holm"), if they meet well: no banned pair or tripled
/// letter where they join, no three vowels, at most `run` consonants across the join.
fn join(a: &str, b: &str, run: usize) -> Option<String> {
    let s = format!("{a}{}", b.to_lowercase());
    let bytes: Vec<u8> = s.bytes().map(|c| c.to_ascii_lowercase()).collect();
    let k = a.len();
    if k == 0 || k >= bytes.len() {
        return Some(s);
    }
    if BANNED.iter().any(|x| x.as_bytes() == &bytes[k - 1..=k]) || bytes.windows(3).any(|t| t[0] == t[1] && t[1] == t[2]) {
        return None;
    }
    let v = |i: usize| vowel_at(&bytes, i);
    let kind = v(k);
    if v(k - 1) != kind {
        return Some(s);
    }
    let (mut lo, mut hi) = (k - 1, k + 1);
    while lo > 0 && v(lo - 1) == kind {
        lo -= 1;
    }
    while hi < bytes.len() && v(hi) == kind {
        hi += 1;
    }
    if hi - lo > if kind { 2 } else { run } { None } else { Some(s) }
}

/// `a` + `b` run together, or apart when they meet badly ("Ash Haven").
fn compound(a: &str, b: &str, run: usize) -> String {
    join(a, b, run).unwrap_or_else(|| format!("{a} {}", capitalize(b)))
}

pub struct Namer {
    rng: Pcg32,
    /// Settlements' and sites' own seed (when they have one): each draws from a stream of its
    /// own, keyed by where it stands, so neither the land's names (more or fewer regions, say)
    /// nor a settlement or site added or taken away elsewhere change theirs.
    sites: Option<u64>,
    used: BTreeSet<String>,
    /// The underground theme (`under::THEMES` key) of the next site named, when known: its
    /// words ("Rime", "Glimmer") go into the name.
    pub theme: Option<&'static str>,
}

impl Namer {
    pub fn new(seed: u64) -> Self {
        Self { rng: Pcg32::new(seed, 11), sites: None, used: BTreeSet::new(), theme: None }
    }

    /// A namer whose settlements and sites draw from streams of their own (`site_seed` and where
    /// they stand: `name_at`).
    pub fn with_sites(seed: u64, site_seed: u64) -> Self {
        Self { rng: Pcg32::new(seed, 11), sites: Some(site_seed), used: BTreeSet::new(), theme: None }
    }

    pub fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.rng.below(xs.len() as u32) as usize]
    }

    pub fn below(&mut self, n: u32) -> u32 {
        self.rng.below(n)
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.rng.next_f64() < p
    }

    /// A shape-template word of `syl` syllables, lower case.
    fn template(&mut self, c: &Culture, syl: u32) -> String {
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
        s
    }

    /// A culture word of 1–3 syllables (by length: about 3–6, 4–8, 6–10 letters), capitalized:
    /// from the letter model or a template, sayable (`fault`), never a training word.
    pub fn word(&mut self, culture: usize, min_syl: u32, max_syl: u32) -> String {
        let ci = culture % CULTURES.len();
        let c = &CULTURES[ci];
        let lo = [3, 4, 6][(min_syl.clamp(1, 3) - 1) as usize];
        let hi = [6, 8, 10][(max_syl.clamp(1, 3) - 1) as usize];
        for _ in 0..40 {
            let w = if self.rng.below(10) < 7 {
                match markov(ci).walk(&mut self.rng, hi) {
                    Some(w) => w,
                    None => continue,
                }
            } else {
                let syl = min_syl + self.rng.below(max_syl.max(min_syl) - min_syl + 1);
                self.template(c, syl)
            };
            if (lo..=hi).contains(&w.len()) && fault(&w, Some(ci)).is_none() && !is_training(&w) {
                return capitalize(&w);
            }
        }
        // Consonant, vowel by turns: always sayable.
        let cons: Vec<&str> = c.onsets.iter().copied().filter(|o| o.len() == 1 && *o != "q" && *o != "y").collect();
        let vow: Vec<&str> = c.vowels.iter().copied().filter(|v| v.len() == 1 && *v != "y").collect();
        let mut s = String::new();
        for k in 0..lo.max(4) {
            s.push_str(if k % 2 == 0 { self.pick(&cons) } else { self.pick(&vow) });
        }
        capitalize(&s)
    }

    /// A person's name in a culture ("Hrolf", "Maddo"): a short culture word.
    pub fn person(&mut self, culture: usize) -> String {
        let mut p = self.word(culture, 1, 2);
        for _ in 0..8 {
            if p.len() >= 4 {
                break;
            }
            p = self.word(culture, 1, 2);
        }
        p
    }

    /// Unique name for a feature of `kind` in the given culture region.
    pub fn name(&mut self, kind: NameKind, culture: usize) -> String {
        self.unique(kind, culture)
    }

    /// `name` for a feature standing at `at` (ft): a settlement or site draws from its own
    /// stream there, when this namer gives them one (`with_sites`).
    pub fn name_at(&mut self, kind: NameKind, culture: usize, at: [i64; 2]) -> String {
        use NameKind::*;
        let site = matches!(kind, Settlement | Ruin | Tower | Camp | Cave | Mine | LavaTube);
        if site && let Some(seed) = self.sites {
            let mut own = Pcg32::new(crate::core::rng::hash2(seed, at[0], at[1]), 13);
            std::mem::swap(&mut self.rng, &mut own);
            let n = self.unique(kind, culture);
            std::mem::swap(&mut self.rng, &mut own);
            return n;
        }
        self.unique(kind, culture)
    }

    fn unique(&mut self, kind: NameKind, culture: usize) -> String {
        let culture = culture % CULTURES.len();
        let ok = |n: &str| name_fault(n).is_none() && !words(n).any(|w| is_training(&w));
        for _ in 0..60 {
            let n = self.candidate(kind, culture);
            if ok(&n) && self.used.insert(n.clone()) {
                return n;
            }
        }
        // Practically unreachable; lengthen the name until unique.
        for k in 0.. {
            let n = format!("{} {}", self.candidate(kind, culture), self.word(culture, 1, 1));
            if (k > 200 || ok(&n)) && self.used.insert(n.clone()) {
                return n;
            }
        }
        unreachable!()
    }

    fn adj(&mut self, c: usize) -> &'static str {
        if self.rng.below(2) == 0 { self.pick(CULTURES[c].adj) } else { self.pick(&ADJ) }
    }

    fn grim(&mut self, nouns: &[&str]) -> String {
        let n = self.pick(nouns);
        match self.rng.below(3) {
            0 => format!("The {} {n}", self.pick(&GRIM_ADJ)),
            1 => format!("{} {n}", self.pick(&GRIM_POSS)),
            _ => format!("The {n} of {}", self.pick(&GRIM_OF)),
        }
    }

    fn themed(&mut self, theme: &str, plain: &str) -> Option<String> {
        let (_, adjs, nouns) = THEME_WORDS.iter().find(|t| t.0 == theme)?;
        let a = self.pick(adjs);
        let n = if self.rng.below(3) == 0 { plain } else { self.pick(nouns) };
        Some(if self.rng.below(4) == 0 && !a.ends_with('\'') && !a.ends_with('s') { format!("{a} {n}") } else { format!("The {a} {n}") })
    }

    fn settlement(&mut self, c: usize) -> String {
        let cu = &CULTURES[c];
        let r = self.rng.next_f64();
        if r < 0.03 {
            let pre = self.pick(&GRIM_PRE);
            let suf = self.pick(cu.suf);
            return compound(pre, suf, 4);
        }
        if r < 0.10 {
            let p = self.person(c);
            return match (c, self.rng.below(2)) {
                (0 | 1, 0) => format!("St. {p}'s"),
                (0 | 1, _) => {
                    let (a, b) = (self.pick(&ROYAL), self.pick(&ROYAL_SUF));
                    format!("{a}{b}")
                }
                (2, 0) => format!("{} {p}", if p.ends_with('a') { "Santa" } else { "San" }),
                (2, _) => format!("Porto {p}"),
                (3, 0) => format!("{p}'s Well"),
                (3, _) => format!("Qasr {p}"),
                (_, 0) => format!("{p}'s Cairn"),
                (_, _) => format!("{p}'s Ring"),
            };
        }
        if self.chance(cu.english) {
            if c == 2 && self.rng.below(3) == 0 {
                return format!("Port {}", self.word(c, 2, 3));
            }
            let (a, b) = (self.pick(cu.pre), self.pick(cu.suf));
            let n = compound(a, b, 4);
            if c == 1 && self.rng.below(20) == 0 {
                return format!("{} {n}", self.pick(&["Great", "Little", "Upper", "Lower", "Nether", "Long", "East", "West"]));
            }
            return n;
        }
        if c == 2 {
            match self.rng.below(12) {
                0 | 1 => {
                    let stem = self.word(c, 1, 2);
                    let stem = stem.trim_end_matches(['a', 'e', 'i', 'o', 'u']);
                    return compound(stem, "amare", 3);
                }
                2 => return format!("Monte {}", self.word(c, 2, 3)),
                3 => return format!("Castel {}", self.word(c, 2, 3)),
                _ => {}
            }
        }
        let bare = if c >= 2 { 45 } else { 25 };
        if self.rng.below(100) < bare {
            return self.word(c, 2, 3);
        }
        let end = self.pick(cu.endings);
        let stem = self.word(c, 1, if end.len() >= 4 { 1 } else { 2 });
        // A vowel meeting a vowel: the stem's gives way ("Corva" + "ona": "Corvona").
        let stem = if end.starts_with(['a', 'e', 'i', 'o', 'u']) { stem.trim_end_matches(['a', 'e', 'i', 'o', 'u', 'y']).to_string() } else { stem };
        join(&stem, end, 3).unwrap_or(stem)
    }

    fn candidate(&mut self, kind: NameKind, c: usize) -> String {
        use NameKind::*;
        if kind == Settlement {
            return self.settlement(c);
        }
        let eng = self.chance(CULTURES[c].nature);
        let grim = dangerous(kind) && self.chance(match kind {
            Blight | Swamp => 0.4,
            Ruin | Pass | Ashlands => 0.25,
            _ => 0.15,
        });
        let of_y = self.chance(0.08);
        let r = self.rng.below(4);
        // A site with a known theme takes its words, but for a grim name (or half the time in a
        // plain cave): a crystal cave is never "the Sun Hollow".
        if let (Some(t), Cave | Mine | LavaTube | Ruin) = (self.theme, kind)
            && !grim
            && (t != "cave" || self.rng.below(2) == 0)
        {
            let plain = match kind {
                Cave => "Caves",
                Mine => "Mine",
                LavaTube => "Tubes",
                _ => "Halls",
            };
            if let Some(n) = self.themed(t, plain) {
                return n;
            }
        }
        if grim {
            let nouns: &[&str] = match kind {
                Swamp => &["Mire", "Fen", "Marsh", "Bog", "Slough"],
                Blight => &["Wood", "Weald", "Waste", "Holt"],
                Ashlands => &["Cinders", "Ashes", "Barrens"],
                Ruin => &["Ruins", "Halls", "Keep", "Stones"],
                Cave => &["Caves", "Hollow", "Pit", "Grotto"],
                Mine => &["Mine", "Delve", "Pit"],
                LavaTube => &["Tubes", "Burrows", "Vents"],
                _ => &["Pass", "Gap", "Notch"],
            };
            return self.grim(nouns);
        }
        if of_y {
            let x = match kind {
                Ocean => Some("The Ocean"),
                Sea => Some("The Sea"),
                Bay => Some("The Bay"),
                Island => Some("The Isle"),
                Range => Some("The Mountains"),
                Lake => Some("The Lake"),
                Forest => Some("The Wood"),
                Plains => Some("The Vale"),
                Desert => Some("The Sands"),
                Tundra => Some("The Barrens"),
                Region => Some("The Land"),
                _ => None,
            };
            if let Some(x) = x {
                return format!("{x} of {}", self.pick(&OF_Y));
            }
        }
        let w = self.word(c, 2, 3);
        let short = self.word(c, 1, 2);
        let adj = self.adj(c);
        match (kind, eng, r) {
            (Continent, _, _) => w,
            (Ocean, false, _) => format!("The {w} Ocean"),
            (Ocean, true, _) => format!("The {adj} Ocean"),
            (Sea, false, _) => format!("The {w} Sea"),
            (Sea, true, _) => format!("The {adj} Sea"),
            (Bay, false, 0) => format!("Gulf of {w}"),
            (Bay, false, 1) => format!("Bay of {w}"),
            (Bay, false, _) => format!("{w} Bay"),
            (Bay, true, 0) => format!("{adj} Cove"),
            (Bay, true, _) => format!("The {adj} Bay"),
            (Island, false, 0 | 1) => format!("Isle of {w}"),
            (Island, false, _) => w,
            (Island, true, 0) if c == 0 => compound(adj, "holm", 4),
            (Island, true, _) => format!("{adj} Isle"),
            (Range, false, 0 | 1) => format!("The {w} Mountains"),
            (Range, false, _) => format!("The {w} Range"),
            (Range, true, 0 | 1) => {
                let n = self.pick(&RANGE_NOUN);
                format!("The {} Mountains", compound(adj, n, 4))
            }
            (Range, true, 2) => format!("The {adj} Peaks"),
            (Range, true, _) => format!("The {adj} Mountains"),
            (Peak, false, 0 | 1) => format!("Mount {w}"),
            (Peak, false, _) => format!("{w} Peak"),
            (Peak, true, 0) => format!("{adj} Peak"),
            (Peak, true, 1) => format!("The {adj} Tooth"),
            (Peak, true, _) => format!("The {adj} Horn"),
            (Pass, false, 0 | 1) => format!("{short} Pass"),
            (Pass, false, _) => format!("The Pass of {short}"),
            (Pass, true, 0) => format!("{adj} Pass"),
            (Pass, true, _) => format!("The {adj} Gap"),
            (Volcano, false, _) => format!("Mount {w}"),
            (Volcano, true, 0) => format!("The {adj} Cauldron"),
            (Volcano, true, _) => compound(adj, "fire", 4),
            (Lake, false, 0 | 1) => format!("Lake {w}"),
            (Lake, false, _) => format!("{w} Mere"),
            (Lake, true, 0) => format!("{adj} Lake"),
            (Lake, true, 1) => format!("The {adj} Mere"),
            (Lake, true, _) => compound(adj, "water", 4),
            (SaltLake, false, _) => format!("The {w} Salt Lake"),
            (SaltLake, true, _) => format!("The {adj} Salt Lake"),
            (SaltFlat, false, _) => format!("The {w} Salt Flats"),
            (SaltFlat, true, _) => format!("The {adj} Flats"),
            (River, false, 0 | 1) => format!("River {w}"),
            (River, false, _) => format!("The {w}"),
            (River, true, 0) => format!("The {adj} River"),
            (River, true, 1) if c == 0 => compound(adj, "beck", 4),
            (River, true, _) => compound(adj, "water", 4),
            (Waterfall, false, _) => format!("{short} Falls"),
            (Waterfall, true, 0) if c == 0 => compound(adj, "force", 4),
            (Waterfall, true, _) => format!("The {adj} Falls"),
            (Forest, false, 0 | 1) => format!("{w} Forest"),
            (Forest, false, _) => format!("The Forest of {w}"),
            (Forest, true, 0) => format!("The {}", compound(adj, "wood", 4)),
            (Forest, true, 1) => format!("The {adj} Weald"),
            (Forest, true, _) => format!("The {adj} Wood"),
            (Jungle, false, _) => format!("The {w} Jungle"),
            (Jungle, true, 0 | 1) => format!("The {adj} Wilds"),
            (Jungle, true, _) => format!("The {adj} Tangle"),
            (Taiga, false, 0 | 1) => format!("The {w} Pinewood"),
            (Taiga, false, _) => format!("The {w} Taiga"),
            (Taiga, true, 0 | 1) => format!("The {adj} Taiga"),
            (Taiga, true, _) => format!("The {adj} Pines"),
            (Desert, false, 0 | 1) => format!("The {w} Desert"),
            (Desert, false, _) => format!("The Sands of {w}"),
            (Desert, true, 0 | 1) => format!("The {adj} Sands"),
            (Desert, true, _) => format!("The {adj} Waste"),
            (Swamp, false, 0 | 1) => format!("The {w} Marsh"),
            (Swamp, false, _) => format!("{w} Bog"),
            (Swamp, true, 0 | 1) => format!("The {}", compound(adj, "fen", 4)),
            (Swamp, true, _) => format!("The {adj} Marsh"),
            (Plains, false, 0 | 1) => format!("The {w} Plains"),
            (Plains, false, _) if c == 1 => format!("The {w} Downs"),
            (Plains, false, _) => format!("The {w} Grasslands"),
            (Plains, true, _) if c == 4 => format!("The {adj} Steppe"),
            (Plains, true, 0 | 1) => format!("The {adj} Plains"),
            (Plains, true, _) if c == 1 => format!("The {adj} Downs"),
            (Plains, true, _) => format!("The {adj} Vale"),
            (Tundra, false, _) => format!("The {w} Barrens"),
            (Tundra, true, _) => format!("The {adj} Barrens"),
            (Glacier, false, _) => format!("The {w} Icefield"),
            (Glacier, true, 0 | 1) => format!("The {adj} Glacier"),
            (Glacier, true, _) => format!("The {adj} Icefield"),
            (Blight, _, 0) => format!("The Blighted Wood of {w}"),
            (Blight, _, 1) => format!("The {adj} Blight"),
            (Blight, _, _) => format!("The Withered {}", ["Wood", "Weald", "Grove", "Holt"][self.rng.below(4) as usize]),
            (Ashlands, false, _) => format!("The {w} Ashlands"),
            (Ashlands, true, _) => format!("The {adj} Cinders"),
            (Region, _, 0 | 1) => format!("The {w} Lands"),
            (Region, _, _) => w,
            (Settlement, _, _) => unreachable!(),
            (Ruin, _, 0) => format!("The Ruins of {w}"),
            (Ruin, _, 1) => format!("Old {w}"),
            (Ruin, _, 2) => format!("{w} Keep"),
            (Ruin, _, _) => format!("The Fallen {}", ["Temple", "Keep", "Citadel", "Shrine", "Hall"][self.rng.below(5) as usize]),
            (Tower, _, 0 | 1) => format!("{short}'s Tower"),
            (Tower, _, 2) => format!("The Tower of {w}"),
            (Tower, _, _) => format!("The {adj} Spire"),
            (Camp, _, 0 | 1) => format!("{short}'s Camp"),
            (Camp, _, _) => format!("The {adj} Camp"),
            (Cave, _, 0) => format!("The {w} Caves"),
            (Cave, _, 1) => format!("The Caves of {w}"),
            (Cave, _, _) => format!("The {adj} Hollow"),
            (Mine, _, 0 | 1) => format!("{w} Mine"),
            (Mine, _, 2) => format!("{short}'s Diggings"),
            (Mine, _, _) => format!("The {adj} Delve"),
            (LavaTube, _, 0 | 1) => format!("The {w} Tubes"),
            (LavaTube, _, _) => format!("The {adj} Lava Tubes"),
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
