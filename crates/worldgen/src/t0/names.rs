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
        onsets: &["k", "sk", "th", "v", "h", "gr", "br", "st", "r", "b", "t", "j", "dr", "s", "hr", "sv"],
        vowels: &["a", "e", "i", "o", "u", "y", "ei", "au"],
        codas: &["r", "n", "rd", "nd", "ll", "st", "g", "rn", "k", "", ""],
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
        adj: &["Rime", "Frost", "Grey", "Raven", "Wolf", "Bleak", "Iron", "Storm", "Ice", "Cold", "White", "Troll", "Hoar", "Skerry"],
        english: 0.04,
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
        suf: &["haven", "bay", "cliff", "vale", "grove", "hill", "watch", "mere", "shore", "crest", "bridge"],
        adj: &["Sun", "Amber", "Gold", "Azure", "Coral", "Laurel", "Olive", "Crimson", "Bright", "Dawn", "Vine", "Myrtle", "Marble", "Sapphire"],
        english: 0.05,
        nature: 0.5,
    },
    Culture {
        onsets: &["z", "kh", "s", "sh", "r", "m", "q", "t", "d", "h", "j", "b", "n"],
        vowels: &["a", "i", "u", "ai", "e"],
        codas: &["r", "n", "m", "h", "d", "sh", "z", "l", "", ""],
        train: &[
            "Zahirah", "Qasim", "Sharaz", "Kharim", "Darjan", "Hamadi", "Mirzan", "Rashad", "Tabriq", "Baharun", "Nasira", "Jafira", "Sulaim", "Qarra", "Zaroun", "Khadesh",
            "Sahrin", "Dumaz", "Hirun", "Basrim", "Yazran", "Tamuz", "Ishara", "Nadrim", "Ruzhan", "Shiraqa", "Zafir", "Amunet", "Kesra", "Halabi",
        ],
        endings: &["abad", "ar", "im", "esh", "ra", "an", "un", "iya", "una", "ash", "is", "ira"],
        pre: &["Sand", "Sun", "Salt", "Dust", "Red", "Gold", "Palm", "Brass", "Copper", "Dune", "Amber", "Jackal", "Falcon", "Ochre", "Saffron"],
        suf: &["well", "gate", "wall", "spring", "rock", "tower", "haven", "cross", "hold", "reach", "stone", "pool"],
        adj: &["Red", "Amber", "Dust", "Brass", "Sun", "Scorched", "Golden", "Burning", "Copper", "Ochre", "Saffron", "Bone", "Glass", "Shimmering"],
        english: 0.04,
        nature: 0.5,
    },
    Culture {
        onsets: &["t", "k", "b", "ch", "s", "m", "n", "g", "y", "d", "l", "kh", "j"],
        vowels: &["a", "o", "u", "e", "i"],
        codas: &["n", "ng", "r", "k", "l", "", ""],
        train: &[
            "Temur", "Bayan", "Khulan", "Tsogt", "Batu", "Orkhon", "Yesun", "Altan", "Chaghan", "Dorgai", "Kerul", "Mingan", "Naran", "Sukhe", "Toghan", "Ulaan", "Bortai",
            "Ergen", "Khasar", "Ordu", "Tengis", "Burqan", "Selenge", "Yisu", "Chono", "Daran", "Qara", "Jebei", "Munkh", "Sartaq",
        ],
        endings: &["ur", "gol", "su", "dar", "in", "ol", "hot", "bulag", "tei", "khan", "del", "tu"],
        pre: &["Horse", "Wind", "Grass", "Sky", "Eagle", "Wolf", "Red", "White", "Blue", "Hawk", "Mare", "Arrow", "Yak", "Felt", "Banner", "Kite"],
        suf: &["ford", "well", "camp", "stone", "hill", "field", "cairn", "moot", "fold", "ring", "mound"],
        adj: &["Blue", "Sky", "Wind", "Grass", "Horse", "Eagle", "Endless", "Wide", "Thunder", "Golden", "White", "Wild", "Whistling", "Silver"],
        english: 0.04,
        nature: 0.5,
    },
];

pub const ADJ: [&str; 43] = [
    "Grey", "Iron", "Frost", "Ash", "Storm", "Silver", "Black", "Red", "White", "Golden", "Shadow", "Thunder", "Wind", "Stone", "Bleak", "Moon", "Sun", "Dragon", "Raven",
    "Wolf", "Ember", "Mist", "Thorn", "Elder", "Green", "Deep", "High", "Broken", "Sorrow", "Whisper", "Amber", "Hollow", "Bright", "Cold", "Long", "Far", "Dawn", "Dusk",
    "Star", "Copper", "Wild", "Pale", "Twin",
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
    ("mine", &["Copper", "Iron", "Silver", "Tin", "Lode", "Deep", "Pick", "Lantern", "Coal"], &["Mine", "Delve", "Workings", "Diggings"]),
    ("lava_tube", &["Ember", "Cinder", "Smoke", "Burnt", "Fire", "Scorch", "Black", "Glass"], &["Tubes", "Lava Tubes", "Burrows", "Vents"]),
    ("dungeon", &["Iron", "Black", "Grim", "Forgotten", "Deep", "Lost"], &["Dungeon", "Vaults", "Halls", "Depths"]),
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
    ("catacombs", &["Bone", "Deep", "Candle", "Silent", "Winding"], &["Catacombs", "Tunnels", "Vaults"]),
    ("undercroft", &["Mouldering", "Vaulted", "Abbey", "Cellar"], &["Undercroft", "Cellars", "Vaults"]),
    ("collapsed_fortress", &["Broken", "Fallen", "War", "Iron", "Shattered"], &["Fortress", "Bastion", "Keep", "Halls"]),
];

/// Three consonants that may stand together in a culture word ("Skjald", "Halstra"); any other run
/// of three, and every longer run, is refused.
const CLUSTERS: [&str; 33] = [
    "skj", "str", "ngr", "ndr", "rst", "nst", "ldr", "rth", "lth", "nth", "thr", "shr", "chr", "spr", "scr", "skr", "ght", "nch", "rch", "nkh", "rsh", "lsh", "nsh", "ngh",
    "nds", "rds", "lds", "lst", "nsk", "rsk", "mbr", "mbl", "ngl",
];
/// Letter pairs no name has.
const BANNED: [&str; 19] = ["jk", "qk", "vj", "gk", "kg", "tl", "dt", "hh", "yy", "wq", "xz", "ii", "uu", "jj", "vv", "ww", "qq", "xx", "zh"];
/// Letter runs that read badly anywhere ("Gochiuk", "Brytraill", "Grirddal", "Buduor", "Moel").
const BAD_RUNS: [&str; 13] = ["iuk", "ytr", "eack", "eamy", "yll", "rdd", "rkh", "lkh", "oolk", "uo", "urr", "oe", "ieh"];
/// Bits of real words that read as a joke, rude or foreign to these tongues in a made-up word
/// ("Orge", "Bungi", "Nemum", "Yeting").
const COMIC: [&str; 34] = [
    "ogre", "orge", "bung", "dung", "mum", "bum", "poo", "fart", "piss", "shit", "fuk", "cum", "tit", "dick", "cock", "turd", "butt", "puke", "snot", "anus", "boob", "wank",
    "porn", "nazi", "ing", "yin", "khan", "dork", "nerd", "slut", "whor", "organ", "sahara", "chan",
];
/// Real given names a made-up word may land on (owners come from their culture's words).
const REAL_NAMES: [&str; 40] = [
    "brian", "alan", "kevin", "gary", "barry", "dave", "steve", "nigel", "derek", "karen", "doris", "norman", "gordon", "dennis", "harry", "terry", "larry", "colin",
    "keith", "brenda", "linda", "sandra", "susan", "donna", "tracy", "carol", "bob", "tom", "tim", "ben", "dan", "sam", "ted", "ian", "jim", "joe", "ken", "ron", "sue",
    "mike",
];

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
            let total: u32 = self.next[lo..hi].iter().map(|(t, n)| weight(t, *n)).sum();
            if total == 0 {
                return None;
            }
            let mut r = rng.below(total);
            let mut c = 0;
            for (t, n) in &self.next[lo..hi] {
                let n = weight(t, *n);
                if r < n {
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

/// How often a letter is drawn after a pair it followed in the lists: as often as it did, to
/// three times (so the lists' commonest runs don't come round in every word).
fn weight(_: &[u8; 3], n: u32) -> u32 {
    n.min(3)
}

fn letters(word: &str) -> Vec<u8> {
    word.bytes().filter(u8::is_ascii_alphabetic).map(|c| c.to_ascii_lowercase()).collect()
}

/// How many syllables (runs of vowels) a word has.
pub fn syllables(word: &str) -> usize {
    let b = letters(word);
    (0..b.len()).filter(|&i| vowel_at(&b, i) && (i == 0 || !vowel_at(&b, i - 1))).count()
}

/// What is wrong with one word of a name, if anything. `culture`: it is a culture word of that
/// culture (the strict rules: 10 letters and 3 syllables at most, only the allowed consonant
/// clusters, no y before two consonants, no doubled vowel, no syllable said twice running, no
/// comic bit of a real word, `q` only in the dry and steppe tongues, ...); otherwise a compound or
/// English word (14 letters, runs of up to 5 consonants: "Kingsbridge").
pub fn fault(word: &str, culture: Option<usize>) -> Option<&'static str> {
    let b = letters(word);
    if b.len() > if culture.is_some() { 10 } else { 14 } {
        return Some("too long");
    }
    // (English words have some: "Kettle", "Hoopoe", "Turquoise".)
    for p in b.windows(2) {
        if BANNED.iter().any(|x| x.as_bytes() == p && (culture.is_some() || p != b"tl")) {
            return Some("banned pair");
        }
    }
    if culture.is_some() && bad_run(&b) {
        return Some("bad letter run");
    }
    if b.starts_with(b"ts") || b.ends_with(b"eh") {
        return Some("ts- or -eh");
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
    let Some(c) = culture else { return None };
    if syllables(word) > 3 {
        return Some("four syllables");
    }
    for i in 0..b.len() {
        if b[i] == b'y' && vowel_at(&b, i) && i + 2 < b.len() && !vowel_at(&b, i + 1) && !vowel_at(&b, i + 2) {
            return Some("y before two consonants");
        }
    }
    let plain = |x: u8| matches!(x, b'a' | b'e' | b'i' | b'o' | b'u');
    if b.windows(2).any(|p| p[0] == p[1] && plain(p[0]) && !(p[0] == b'a' && c == 4)) {
        return Some("doubled vowel");
    }
    if b.windows(4).any(|w| w[0] == w[2] && w[1] == w[3] && plain(w[0]) != plain(w[1])) {
        return Some("syllable said twice");
    }
    // w after a vowel only at the end or before a vowel or n ("Haddow", not "Cygeawfowm").
    for i in 1..b.len() {
        if b[i] == b'w' && plain(b[i - 1]) && i + 1 < b.len() && !plain(b[i + 1]) && b[i + 1] != b'n' {
            return Some("w before a consonant");
        }
    }
    if b.windows(2).any(|p| p == b"iu" || p == b"ui") || (c == 4 && b.ends_with(b"ng")) {
        return Some("iu or ui, or -ng in the steppe tongue");
    }
    // y after a vowel only at the end or before a vowel ("Hrossey", not "Loyw", "Beaym").
    for i in 1..b.len() {
        if b[i] == b'y' && plain(b[i - 1]) && i + 1 < b.len() && !plain(b[i + 1]) {
            return Some("y after a vowel before a consonant");
        }
        if b[i] == b'y' && b.get(i + 1) == Some(&b'i') {
            return Some("yi in a word");
        }
    }
    if (c == 3 || c == 4) && b.windows(2).any(|p| p == b"ou") {
        return Some("ou in the dry or steppe tongue");
    }
    if (c == 0 || c == 1) && b.ends_with(b"i") {
        return Some("-i in the north or west tongue");
    }
    if COMIC.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes())) || REAL_NAMES.iter().any(|x| x.as_bytes() == b) {
        return Some("a real word");
    }
    None
}

fn bad_run(b: &[u8]) -> bool {
    BAD_RUNS.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes()))
}

/// What is wrong with a culture word with an ending on ("Haldurrud", "Grirddal"): a compound's
/// faults, a bad letter run or "tl", a doubled consonant in a run of three, more than three
/// syllables.
fn place_fault(word: &str) -> Option<&'static str> {
    if let Some(f) = fault(word, None) {
        return Some(f);
    }
    let b = letters(word);
    if bad_run(&b) || b.windows(2).any(|p| p == b"tl") {
        return Some("bad letter run");
    }
    if b.windows(3).any(|t| (t[0] == t[1] || t[1] == t[2]) && t.iter().all(|&x| !matches!(x, b'a' | b'e' | b'i' | b'o' | b'u' | b'y'))) {
        return Some("doubled consonant in a cluster");
    }
    if syllables(word) > 3 {
        return Some("four syllables");
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
    // (Long ones apart too: "Whistling Fire", not "Whistlingfire".)
    join(a, b, run).filter(|_| a.len() + b.len() <= 12).unwrap_or_else(|| format!("{a} {}", capitalize(b)))
}

/// Whether a name is all English (each word an English place-name piece, or two run together:
/// "Ravenholm", "Kingsbridge"), not made with a culture word.
pub fn is_english(name: &str) -> bool {
    static PIECES: OnceLock<BTreeSet<String>> = OnceLock::new();
    let pieces = PIECES.get_or_init(|| {
        let mut p: BTreeSet<String> = BTreeSet::new();
        for c in &CULTURES {
            p.extend(c.pre.iter().chain(c.suf).chain(c.adj).map(|w| w.to_lowercase()));
        }
        let more = ADJ.iter().chain(&ROYAL).chain(&ROYAL_SUF).chain(&GRIM_PRE).chain(&GRIM_ADJ).chain(&["Great", "Little", "Upper", "Lower", "Nether", "Long", "East", "West"]);
        p.extend(more.map(|w| w.to_lowercase()));
        p
    });
    words(name).all(|w| {
        let w = w.to_lowercase();
        pieces.contains(&w) || (2..w.len().saturating_sub(1)).any(|k| pieces.contains(&w[..k]) && pieces.contains(&w[k..]))
    })
}

/// Site nouns no world has more than two of ("The Glimmer Grotto", "The Weeping Hollow").
const RARE_NOUNS: [&str; 12] = ["Delve", "Grotto", "Hollow", "Lair", "Diggings", "Workings", "Spire", "Burrows", "Vents", "Deeps", "Oubliette", "Stockade"];

/// Words that say what a cave does ("The Weeping Hollow").
const CAVE_DESC: [&str; 16] = [
    "Weeping", "Whispering", "Howling", "Echoing", "Dripping", "Sighing", "Hungry", "Sleeping", "Moaning", "Singing", "Yawning", "Gaping", "Mourning", "Murmuring", "Wailing",
    "Grinning",
];

/// Words for a wide sea ("The Sundering Ocean", "The Restless Deep").
const VAST: [&str; 12] = ["Endless", "Sundering", "Restless", "Boundless", "Wailing", "Shattered", "Western", "Eastern", "Northern", "Southern", "Silent", "Glittering"];

pub struct Namer {
    rng: Pcg32,
    /// Settlements' and sites' own seed (when they have one): each draws from a stream of its
    /// own, keyed by where it stands, so neither the land's names (more or fewer regions, say)
    /// nor a settlement or site added or taken away elsewhere change theirs (unless the one
    /// drawn first comes out too like a name given before: see `fresh`).
    sites: Option<u64>,
    used: BTreeSet<String>,
    /// How often each word (lower case) has gone into a name given out, and each culture word's
    /// first four letters; the culture words given out; and settlements named per culture, and
    /// per culture and last two letters: so no adjective, stem or ending runs through a world.
    seen: BTreeMap<String, u32>,
    stems: BTreeMap<String, u32>,
    own: BTreeSet<String>,
    settled: [u32; 5],
    ends: BTreeMap<(usize, [u8; 2]), u32>,
    /// The culture words drawn for the name being made.
    drawn: Vec<String>,
    /// The underground theme (`under::THEMES` key) of the next site named, when known: its
    /// words ("Rime", "Glimmer") go into the name.
    pub theme: Option<&'static str>,
}

impl Namer {
    pub fn new(seed: u64) -> Self {
        Self::make(seed, None)
    }

    /// A namer whose settlements and sites draw from streams of their own (`site_seed` and where
    /// they stand: `name_at`).
    pub fn with_sites(seed: u64, site_seed: u64) -> Self {
        Self::make(seed, Some(site_seed))
    }

    fn make(seed: u64, sites: Option<u64>) -> Self {
        Self {
            rng: Pcg32::new(seed, 11),
            sites,
            used: BTreeSet::new(),
            seen: BTreeMap::new(),
            stems: BTreeMap::new(),
            own: BTreeSet::new(),
            settled: [0; 5],
            ends: BTreeMap::new(),
            drawn: Vec::new(),
            theme: None,
        }
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
    /// from the letter model or a template, sayable (`fault`), never a training word nor
    /// starting as one does (its first four letters), and only now and then Bj- or Fj-.
    pub fn word(&mut self, culture: usize, min_syl: u32, max_syl: u32) -> String {
        let ci = culture % CULTURES.len();
        let c = &CULTURES[ci];
        let lo = [3, 4, 6][(min_syl.clamp(1, 3) - 1) as usize];
        let hi = [6, 8, 10][(max_syl.clamp(1, 3) - 1) as usize];
        let like_training = |w: &str| w.len() >= 4 && CULTURES.iter().any(|c| c.train.iter().any(|t| t.len() >= 4 && t[..4].eq_ignore_ascii_case(&w[..4])));
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
            if (w.starts_with("bj") || w.starts_with("fj")) && self.rng.below(6) > 0 {
                continue;
            }
            if (lo..=hi).contains(&w.len()) && fault(&w, Some(ci)).is_none() && !is_training(&w) && !like_training(&w) {
                if self.drawn.len() >= 64 {
                    self.drawn.clear();
                }
                self.drawn.push(w.clone());
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
        self.drawn.push(s.clone());
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
        for i in 0..60 {
            self.drawn.clear();
            let n = self.candidate(kind, culture);
            if ok(&n) && (i >= 40 || self.fresh(&n, kind, culture)) && !self.used.contains(&n) {
                self.note(&n, kind, culture);
                return n;
            }
        }
        // Practically unreachable; lengthen the name until unique.
        for k in 0.. {
            self.drawn.clear();
            let n = format!("{} {}", self.candidate(kind, culture), self.word(culture, 1, 1));
            if (k > 200 || ok(&n)) && !self.used.contains(&n) {
                self.note(&n, kind, culture);
                return n;
            }
        }
        unreachable!()
    }

    /// The culture words drawn for name `n` that are in it, with their first four letters.
    fn own_words(&self, n: &str) -> Vec<(String, String)> {
        let low = n.to_lowercase();
        let stem = |w: &str| w[..w.len().min(4)].to_string();
        self.drawn.iter().filter(|w| low.contains(&stem(w))).map(|w| (w.clone(), stem(w))).collect()
    }

    /// Name `n` is not too like the names given before: none of its culture words given out
    /// before nor its stem twice; a rare site noun (`RARE_NOUNS`) at most twice in a world; a
    /// settlement's ending (its last two letters) on at most ~30% of its culture's.
    fn fresh(&self, n: &str, kind: NameKind, c: usize) -> bool {
        if self.own_words(n).iter().any(|(w, s)| self.own.contains(w) || self.stems.get(s).is_some_and(|&k| k >= 2)) {
            return false;
        }
        // ("The Wild Wilds".)
        let ws: Vec<String> = words(n).map(|w| w.to_lowercase()).filter(|w| w.len() >= 4).collect();
        if ws.iter().enumerate().any(|(i, a)| ws[i + 1..].iter().any(|b| a[..4] == b[..4])) {
            return false;
        }
        let last = words(n).last().unwrap_or_default();
        if RARE_NOUNS.contains(&last.as_str()) && self.seen.get(&last.to_lowercase()).is_some_and(|&k| k >= 2) {
            return false;
        }
        if kind == NameKind::Settlement {
            let k = self.ends.get(&(c, ending(n))).copied().unwrap_or(0);
            if k >= 2 && (k + 1) as f64 > 0.3 * (self.settled[c] + 1) as f64 {
                return false;
            }
        }
        true
    }

    fn note(&mut self, n: &str, kind: NameKind, c: usize) {
        for (w, s) in self.own_words(n) {
            *self.stems.entry(s).or_default() += 1;
            self.own.insert(w);
        }
        for w in words(n) {
            *self.seen.entry(w.to_lowercase()).or_default() += 1;
        }
        if kind == NameKind::Settlement {
            self.settled[c] += 1;
            *self.ends.entry((c, ending(n))).or_default() += 1;
        }
        self.used.insert(n.to_string());
    }

    /// A describing word, the culture's own or a shared one, used at most twice in a world when
    /// it can be.
    fn adj(&mut self, c: usize) -> &'static str {
        let mut a = "";
        for _ in 0..6 {
            a = if self.rng.below(2) == 0 { self.pick(CULTURES[c].adj) } else { self.pick(&ADJ) };
            if self.seen.get(&a.to_lowercase()).is_none_or(|&k| k < 2) {
                break;
            }
        }
        a
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
        // ("Glimmer Grotto", "Anvil Halls"; but "The Forgotten Dungeon", "The Sunken Temple".)
        let bare = !a.ends_with('\'') && !a.ends_with('s') && !a.ends_with("ed") && !a.ends_with("en") && !a.ends_with("ing");
        Some(if bare && self.rng.below(4) == 0 { format!("{a} {n}") } else { format!("The {a} {n}") })
    }

    /// A culture word with one of the culture's endings on, sayable as one (`place_fault`);
    /// else a bare culture word.
    fn with_ending(&mut self, c: usize) -> String {
        for _ in 0..8 {
            let end = self.pick(CULTURES[c].endings);
            let stem = self.word(c, 1, if end.len() >= 4 { 1 } else { 2 });
            // A stem in -ur takes no ending ("Valdurvik", "Haldurrud").
            if stem.ends_with("ur") {
                continue;
            }
            // A vowel meeting a vowel: the stem's gives way ("Corva" + "ona": "Corvona").
            let stem = if end.starts_with(['a', 'e', 'i', 'o', 'u']) { stem.trim_end_matches(['a', 'e', 'i', 'o', 'u', 'y']).to_string() } else { stem };
            if let Some(w) = join(&stem, end, 3)
                && place_fault(&w).is_none()
            {
                return w;
            }
        }
        self.word(c, 2, 3)
    }

    fn settlement(&mut self, c: usize) -> String {
        let cu = &CULTURES[c];
        let r = self.rng.next_f64();
        if r < if c == 1 { 0.03 } else { 0.01 } {
            let pre = self.pick(&GRIM_PRE);
            let suf = self.pick(cu.suf);
            return compound(pre, suf, 4);
        }
        if r < 0.08 {
            let p = self.person(c);
            return match (c, self.rng.below(3)) {
                (0, 0 | 1) | (1, 0) => format!("St. {p}'s"),
                (0 | 1, _) => {
                    let (a, b) = (self.pick(&ROYAL), self.pick(&ROYAL_SUF));
                    format!("{a}{b}")
                }
                (2, 0 | 1) => format!("{} {p}", if p.ends_with('a') { "Santa" } else { "San" }),
                (2, _) => format!("Porto {p}"),
                (3, 0) => format!("Bir {p}"),
                (3, 1) => format!("Qasr {p}"),
                (3, _) => format!("Ain {p}"),
                (_, 0) => format!("{p}'s Cairn"),
                (_, _) => format!("{p}'s Ring"),
            };
        }
        if self.chance(cu.english) {
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
                    // A one-syllable stem of plain vowels: "Corvamare", not "Ciasamare".
                    for _ in 0..6 {
                        let stem = self.word(c, 1, 1);
                        let stem = stem.trim_end_matches(['a', 'e', 'i', 'o', 'u']);
                        if syllables(stem) == 1 && !stem.contains("ia") && !stem.contains("io") {
                            return compound(stem, "amare", 3);
                        }
                    }
                }
                2 => return format!("Monte {}", self.word(c, 2, 3)),
                3 => return format!("Castel {}", self.word(c, 2, 3)),
                4 => return format!("Porto {}", self.word(c, 2, 3)),
                _ => {}
            }
        }
        let bare = if c >= 2 { 45 } else { 25 };
        if self.rng.below(100) < bare {
            // (Five letters or more: the short ones come round in every world: "Pero", "Cano".)
            let mut w = self.word(c, 2, 3);
            for _ in 0..4 {
                if w.len() >= 5 {
                    break;
                }
                w = self.word(c, 2, 3);
            }
            return w;
        }
        self.with_ending(c)
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
        // Words for land the culture may not know by its name: a taiga, a jungle or a glacier
        // far from where the word belongs.
        let pines = if c == 0 { "Taiga" } else { ["Pinewood", "Pines", "Firwood"][self.rng.below(3) as usize] };
        let jungle = if matches!(c, 2 | 3) { "Jungle" } else { ["Wilds", "Greenwood", "Thickets"][self.rng.below(3) as usize] };
        let ice = if matches!(c, 0 | 4) { "Glacier" } else { ["Icefield", "Snowfield"][self.rng.below(2) as usize] };
        let plains = match c {
            0 => ["Heath", "Moors", "Plains"][self.rng.below(3) as usize],
            1 => ["Downs", "Vale", "Lea", "Plains"][self.rng.below(4) as usize],
            2 => ["Fields", "Meadows", "Plains"][self.rng.below(3) as usize],
            3 => ["Plain", "Flats", "Reaches"][self.rng.below(3) as usize],
            _ => ["Steppe", "Grasslands", "Plains"][self.rng.below(3) as usize],
        };
        let vast = self.pick(&VAST);
        let desc = self.pick(&CAVE_DESC);
        let one = self.word(c, 1, 1);
        match (kind, eng, r) {
            (Continent, _, _) => w,
            (Ocean, false, _) => format!("The {w} Ocean"),
            (Ocean, true, 0) => format!("The {adj} Ocean"),
            (Ocean, true, 1 | 2) => format!("The {vast} Ocean"),
            (Ocean, true, _) => format!("The {vast} Deep"),
            (Sea, false, _) => format!("The {w} Sea"),
            (Sea, true, 0 | 1) => format!("The {adj} Sea"),
            (Sea, true, _) => format!("The {vast} Sea"),
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
            (Jungle, false, _) => format!("The {w} {jungle}"),
            (Jungle, true, 0 | 1) => format!("The {adj} Wilds"),
            (Jungle, true, _) => format!("The {adj} {jungle}"),
            (Taiga, false, 0 | 1) => format!("The {w} Pinewood"),
            (Taiga, false, _) => format!("The {w} {pines}"),
            (Taiga, true, 0 | 1) => format!("The {adj} {pines}"),
            (Taiga, true, _) => format!("The {adj} Pines"),
            (Desert, false, 0 | 1) => format!("The {w} Desert"),
            (Desert, false, _) => format!("The Sands of {w}"),
            (Desert, true, 0 | 1) => format!("The {adj} Sands"),
            (Desert, true, _) => format!("The {adj} Waste"),
            (Swamp, false, 0 | 1) => format!("The {w} Marsh"),
            (Swamp, false, _) => format!("{w} Bog"),
            (Swamp, true, 0 | 1) => format!("The {}", compound(adj, "fen", 4)),
            (Swamp, true, _) => format!("The {adj} Marsh"),
            (Plains, false, _) => format!("The {w} {plains}"),
            (Plains, true, _) => format!("The {adj} {plains}"),
            (Tundra, false, _) => format!("The {w} Barrens"),
            (Tundra, true, _) => format!("The {adj} Barrens"),
            (Glacier, false, _) => format!("The {w} {ice}"),
            (Glacier, true, 0 | 1) => format!("The {adj} {ice}"),
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
            (Tower, _, 0) => format!("{short}'s Tower"),
            (Tower, _, 1) => format!("The Tower of {short}"),
            (Tower, _, 2) => format!("The {adj} Spire"),
            (Tower, _, _) => format!("The {adj} Tower"),
            (Camp, _, 0) => format!("{short}'s Camp"),
            (Camp, _, 1) => format!("The {adj} Camp"),
            (Camp, _, 2) => format!("{short} Camp"),
            (Camp, _, _) => format!("The {adj} Stockade"),
            (Cave, _, 0) => format!("The {short} Caves"),
            (Cave, _, 1) => format!("The Caves of {one}"),
            (Cave, _, _) => format!("The {desc} {}", ["Caves", "Hollow", "Caverns", "Grotto"][self.rng.below(4) as usize]),
            (Mine, _, 0 | 1) => format!("{short} Mine"),
            (Mine, _, 2) => format!("{short}'s Diggings"),
            (Mine, _, _) => format!("The {adj} Delve"),
            (LavaTube, _, 0 | 1) => format!("The {short} Tubes"),
            (LavaTube, _, _) => format!("The {adj} Lava Tubes"),
        }
    }
}

/// A name's last two letters (its ending's family: "-an", "-ai").
fn ending(n: &str) -> [u8; 2] {
    let b = letters(n);
    match b.len() {
        0 => [0, 0],
        1 => [0, b[0]],
        k => [b[k - 2], b[k - 1]],
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
