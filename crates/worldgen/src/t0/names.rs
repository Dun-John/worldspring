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
    /// Curated roots: first parts and last parts run together ("Thor" + "geir").
    roots: (&'static [&'static str], &'static [&'static str]),
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
        onsets: &["k", "sk", "th", "v", "h", "gr", "br", "st", "r", "b", "t", "j", "dr", "s", "sv"],
        vowels: &["a", "e", "i", "o", "u", "y", "ei", "au"],
        codas: &["r", "n", "rd", "nd", "ll", "rn", "k", "", ""],
        train: &[
            "Skeld", "Thorvald", "Hrossey", "Brennvik", "Fjalsted", "Grimmar", "Kaldur", "Vestrey", "Haukrun", "Sigvald", "Torsund", "Brynjar", "Eskil", "Hjaldur", "Skarna",
            "Vidrun", "Jorund", "Starkad", "Halvard", "Dravik", "Sveinar", "Ulfrik", "Kolbrun", "Asmund", "Thyra", "Bjarke", "Folkvar", "Vargrun", "Haldis", "Ragnvald",
        ],
        roots: (
            &["Thor", "Ulf", "Sig", "Rag", "Hal", "Kol", "Sven", "Ask", "Grim", "Vig", "Ein", "Arn", "Bor", "Frey", "Gunn", "Orm", "Tor", "Var", "Skar", "Brand", "Hak", "Leif", "Hel", "Dag", "Os", "Ket", "Sol", "Eld"],
            &["mund", "rik", "vald", "dis", "geir", "stein", "ulf", "run", "ar", "ald", "olf", "ard", "vor", "heid", "borg", "dur", "mar", "ved", "ka", "va", "is", "ny"],
        ),
        endings: &["heim", "vik", "gard", "fell", "by", "stad", "holm", "dal", "sund", "ness", "vang", "berg", "lund", "nes", "tun", "sted", "voll", "ey"],
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
        roots: (
            &["Al", "Bran", "Cad", "Dun", "El", "Gar", "Hal", "Mer", "Pen", "Tal", "Tre", "Gwen", "Ros", "Car", "Mor", "Bel", "Cor", "Lan", "Od", "Ber", "Wil", "Gil", "Har", "Ced"],
            &["wen", "ric", "oc", "mar", "wyn", "dor", "ell", "an", "ith", "ard", "in", "el", "ow", "ys", "wald", "ry"],
        ),
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
        roots: (
            &["Val", "Cas", "Mar", "Lu", "Ser", "Tav", "Ben", "Cal", "Dor", "Mon", "Ner", "Pal", "Sol", "Ver", "Ar", "Fab", "Lor", "Ren", "Tor", "Cor", "Ost", "Sal", "Vit", "Lev"],
            &["ano", "era", "ino", "etta", "ona", "ello", "ara", "esso", "oro", "enzo", "ina", "ato", "ola", "ezzo", "ita"],
        ),
        endings: &["ia", "um", "ona", "ara", "essa", "ano", "ella", "ino", "era", "enza", "ora", "eto", "esca", "ata", "ola", "ezza", "ina", "ello"],
        pre: &["Sun", "Gold", "Olive", "Vine", "Lemon", "Salt", "Rose", "Bright", "White", "Amber", "Coral", "Cypress", "Myrtle", "Laurel", "Fig", "Dove"],
        suf: &["haven", "bay", "cliff", "vale", "grove", "hill", "watch", "mere", "shore", "crest", "bridge"],
        adj: &["Sun", "Amber", "Gold", "Azure", "Coral", "Laurel", "Olive", "Crimson", "Bright", "Dawn", "Vine", "Myrtle", "Marble", "Sapphire"],
        english: 0.0,
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
        roots: (
            &["Zah", "Qas", "Shar", "Khar", "Dar", "Ham", "Mir", "Rash", "Tab", "Bah", "Nas", "Jaf", "Sul", "Kes", "Sah", "Dum", "Hir", "Bas", "Yaz", "Tam", "Nad", "Zaf", "Hal", "Sab"],
            &["im", "ar", "an", "ad", "ira", "un", "aya", "ud", "il", "ab", "at", "ir", "ul", "ida"],
        ),
        endings: &["abad", "ar", "im", "esh", "ra", "an", "un", "iya", "una", "ash", "is", "ira", "sar", "at", "ul", "ana", "il", "ad"],
        pre: &["Sand", "Sun", "Salt", "Dust", "Red", "Gold", "Palm", "Brass", "Copper", "Dune", "Amber", "Jackal", "Falcon", "Ochre", "Saffron"],
        suf: &["well", "gate", "wall", "spring", "rock", "tower", "haven", "cross", "hold", "reach", "stone", "pool"],
        adj: &["Red", "Amber", "Dust", "Brass", "Sun", "Scorched", "Golden", "Burning", "Copper", "Ochre", "Saffron", "Bone", "Glass", "Shimmering"],
        english: 0.0,
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
        roots: (
            &["Tem", "Bay", "Khul", "Bat", "Or", "Yes", "Alt", "Chag", "Dor", "Ker", "Min", "Nar", "Suk", "Tog", "Ul", "Bor", "Er", "Kha", "Ten", "Bur", "Sel", "Jeb", "Mun", "Sar"],
            &["ur", "an", "gai", "tai", "ul", "un", "ar", "en", "dai", "mur", "gen", "jin", "lan", "mai", "sun", "ei"],
        ),
        endings: &["ur", "gol", "su", "dar", "in", "nur", "tei", "bulag", "dal", "del", "tu", "ge", "tai"],
        pre: &["Horse", "Wind", "Grass", "Sky", "Eagle", "Wolf", "Red", "White", "Blue", "Hawk", "Mare", "Arrow", "Yak", "Felt", "Banner", "Kite"],
        suf: &["ford", "well", "camp", "stone", "hill", "field", "cairn", "moot", "fold", "ring", "mound"],
        adj: &["Blue", "Sky", "Wind", "Grass", "Horse", "Eagle", "Endless", "Wide", "Thunder", "Golden", "White", "Wild", "Whistling", "Silver"],
        english: 0.0,
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
    ("lava_tube", &["Ember", "Cinder", "Smoke", "Burnt", "Fire", "Scorch", "Black", "Glass"], &["Tubes", "Lava Tubes", "Burrows", "Vents", "Flues", "Tunnels"]),
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

/// Runs read badly or as English in a culture word ("Zaztiqai", "Jauthest", "Glaock", "Whock",
/// "Hushuzair", "Hauskall", "Porrol", "Chukhot").
const NEW_RUNS: [&str; 20] = ["zt", "zz", "hz", "rgr", "auth", "auj", "aust", "aock", "aime", "orr", "who", "whu", "hush", "haus", "bantam", "otic", "crud", "krud", "hot", "qang"];
/// Starts that read as English ("Hitisad", "Hidun", "Sirano").
const JOKE_START: [&str; 6] = ["hit", "hid", "sir", "his", "her", "hum"];
/// Short English words and names a culture word may be ("The Both Archive").
const SHORT_WORDS: &str = "both bath moth hath then than them this that with were here rain main more sore bore bone done gone none hide side tide ride road toad load \
    sand band land hand wand bend lend mend send tend rent tent dent bent sent mist list fist hint mint tint sink link dark bark park mark tusk dusk husk tomb bomb limb lamb \
    dumb numb bint third fiona tina nina lena anna emma ella sara maria rosa eric ivan omar lisa mona dana nora vera hana tara kara mara rita gina dina lola tony toby mary";

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
    // w after a vowel only at the end ("Haddow", not "Cygeawfowm", "Tewom").
    for i in 1..b.len() {
        if b[i] == b'w' && plain(b[i - 1]) && i + 1 < b.len() {
            return Some("w after a vowel inside a word");
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
    if c != 1 && b.windows(2).any(|p| p == b"ou") {
        return Some("ou outside the west tongue");
    }
    // Vowel pairs: only the culture's own ("ei", "au" in the north; "ia", "io" in the south),
    // one a word, and in a word of three syllables only in the last ("Valeria", not
    // "Siopenus", "Nugaent", "Muek").
    let pairs: &[&[u8]] = match c {
        0 => &[b"au", b"ei"],
        1 => &[b"ae", b"ea", b"ai"],
        2 => &[b"ia", b"io", b"au"],
        3 => &[b"ai"],
        _ => &[b"ai", b"ei", b"aa"],
    };
    let at: Vec<usize> = (0..b.len().saturating_sub(1)).filter(|&i| plain(b[i]) && plain(b[i + 1])).collect();
    if at.iter().any(|&i| !pairs.contains(&&b[i..i + 2])) || at.len() > 1 {
        return Some("a vowel pair not of the tongue");
    }
    if let Some(&i) = at.first() {
        if syllables(word) == 3 && b[i + 2..].iter().any(|&x| plain(x)) {
            return Some("a vowel pair early in a long word");
        }
        // ("Fusmia": ia and io after one consonant.)
        if c == 2 && i >= 2 && !plain(b[i - 1]) && !plain(b[i - 2]) {
            return Some("a vowel pair after two consonants");
        }
        // ("Brucugiar", "Bonruvial": the south's pairs end a word or come before a vowel.)
        if c == 2 && i + 3 == b.len() {
            return Some("a vowel pair before a last consonant");
        }
    }
    if let Some(f) = ban(word) {
        return Some(f);
    }
    // Endings of no tongue here ("Mijangek", "Yichik", "Tikege"; -ik is the north's, "Dravik").
    if b.ends_with(b"ek") || b.ends_with(b"ege") || (c != 0 && b.ends_with(b"ik")) {
        return Some("-ek, -ege or -ik outside its tongue");
    }
    // The steppe's long words do not end in two closed syllables running ("Chonsoknur").
    if c == 4 && syllables(word) >= 3 && !vowel_at(&b, b.len() - 1) {
        let last = (0..b.len()).rev().find(|&i| plain(b[i]));
        let before = last.and_then(|l| (0..l).rev().find(|&i| plain(b[i])));
        if let (Some(l), Some(v)) = (last, before) {
            if l - v > 2 {
                return Some("two closed syllables ending a steppe word");
            }
        }
    }
    if (c == 0 || c == 1) && b.ends_with(b"i") {
        return Some("-i in the north or west tongue");
    }
    if COMIC.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes())) || REAL_NAMES.iter().any(|x| x.as_bytes() == b) {
        return Some("a real word");
    }
    None
}

/// The later bans on a culture word (with an ending on or not): no Hr- or Bj-, Ll only before a
/// vowel, no -au, -dry, -ian nor hard final pair (-gt, -st), `q` only before a or i, no
/// `NEW_RUNS`, no English start or word, no two r-clusters a vowel apart ("Trobr-") nor three
/// r's, no first syllable said back ("Sist", "Rers"), no hiss twice ("Shalaimesh", "Jezzaz").
pub fn ban(word: &str) -> Option<&'static str> {
    let b = letters(word);
    let plain = |x: u8| matches!(x, b'a' | b'e' | b'i' | b'o' | b'u');
    let cons = |x: u8| !plain(x) && x != b'y';
    let stop = |x: u8| matches!(x, b'b' | b'c' | b'd' | b'f' | b'g' | b'k' | b'p' | b't');
    let n = b.len();
    if b.starts_with(b"hr") || b.starts_with(b"bj") {
        return Some("Hr- or Bj-");
    }
    // ("Brytathy", "Yukhinun", "Yichik": no y right after a starting cluster, nor yu-, yi-.)
    if (n >= 3 && cons(b[0]) && cons(b[1]) && b[2] == b'y') || b.starts_with(b"yu") || b.starts_with(b"yi") {
        return Some("a clumsy y at the start");
    }
    if b.starts_with(b"ll") && !b.get(2).is_some_and(|&x| plain(x)) {
        return Some("Ll before a consonant");
    }
    if b.ends_with(b"au") || b.ends_with(b"dry") || b.ends_with(b"try") || b.ends_with(b"ian") {
        return Some("a bad ending");
    }
    if n >= 3 && matches!(&b[n - 2..], b"gt" | b"kt" | b"pt" | b"bt" | b"dt" | b"zt" | b"st" | b"ht" | b"gd" | b"kd") {
        return Some("a hard final pair");
    }
    for i in 0..n {
        if b[i] == b'q' && !matches!(b.get(i + 1), Some(b'a' | b'i')) {
            return Some("q not before a or i");
        }
    }
    if NEW_RUNS.iter().any(|x| b.windows(x.len()).any(|w| w == x.as_bytes())) {
        return Some("bad letter run");
    }
    if JOKE_START.iter().any(|x| b.starts_with(x.as_bytes())) || SHORT_WORDS.split_whitespace().any(|x| x.as_bytes() == b) {
        return Some("reads as English");
    }
    if b.windows(5).any(|w| stop(w[0]) && w[1] == b'r' && plain(w[2]) && stop(w[3]) && w[4] == b'r') || b.iter().filter(|&&x| x == b'r').count() > 2 {
        return Some("r-clusters piled up");
    }
    if n >= 3 && cons(b[0]) && b[0] == b[2] && plain(b[1]) && b.get(3).is_none_or(|&x| cons(x)) {
        return Some("first syllable said back");
    }
    if b.windows(2).filter(|p| p == b"sh").count() >= 2 || b.iter().filter(|&&x| x == b'z').count() >= 2 {
        return Some("a hiss twice");
    }
    // ("Estedsted", "Ulfulf", "Gwenwen".)
    if (0..n.saturating_sub(5)).any(|i| b[i + 3..].windows(3).any(|w| w == &b[i..i + 3])) {
        return Some("a run said twice");
    }
    None
}

/// One letter put in, left out or changed in a training word ("Hadow", "Fiona").
fn near_training(w: &str) -> bool {
    let a = letters(w);
    let near = |t: &[u8]| {
        if a.len().abs_diff(t.len()) > 1 {
            return false;
        }
        let (mut i, mut j, mut k) = (0usize, a.len(), t.len());
        while i < j.min(k) && a[i] == t[i] {
            i += 1;
        }
        while j > i && k > i && a[j - 1] == t[k - 1] {
            j -= 1;
            k -= 1;
        }
        j - i <= 1 && k - i <= 1
    };
    a.len() >= 4 && CULTURES.iter().any(|c| c.train.iter().any(|t| near(&letters(t))))
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
    if let Some(f) = ban(word) {
        return Some(f);
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

/// A name's head noun, what the place is called: "Camp" of "Hrolf's Camp", "Tower" of "The
/// Tower of Kald", "Sands" of "The Sands of Zahir".
pub fn site_noun(n: &str) -> &str {
    let head = n.split(" of ").next().unwrap_or(n);
    head.rsplit(' ').next().unwrap_or(head).trim_end_matches("'s")
}

/// How many features of `kind` a world names with head noun `h`: the rare ones twice, a few
/// overused ones a little more, any other a site's six times (land's as often as it comes).
fn noun_cap(kind: NameKind, h: &str) -> Option<u32> {
    use NameKind::*;
    const CAPPED: [(&str, u32); 9] = [("Cauldron", 2), ("Wilds", 2), ("Deep", 2), ("Ocean", 3), ("Sands", 3), ("Halls", 4), ("Camp", 5), ("Tower", 5), ("Mine", 5)];
    if RARE_NOUNS.contains(&h) {
        return Some(2);
    }
    if let Some(&(_, k)) = CAPPED.iter().find(|x| x.0 == h) {
        return Some(k);
    }
    matches!(kind, Ruin | Tower | Camp | Cave | Mine | LavaTube).then_some(6)
}

/// Site nouns no world has more than two of ("The Glimmer Grotto", "The Weeping Hollow").
const RARE_NOUNS: [&str; 12] = ["Delve", "Grotto", "Hollow", "Lair", "Diggings", "Workings", "Spire", "Burrows", "Vents", "Deeps", "Oubliette", "Stockade"];

/// Words that say what a cave does ("The Weeping Hollow").
const CAVE_DESC: [&str; 16] = [
    "Weeping", "Whispering", "Howling", "Echoing", "Dripping", "Sighing", "Hungry", "Sleeping", "Moaning", "Singing", "Yawning", "Gaping", "Mourning", "Murmuring", "Wailing",
    "Grinning",
];

/// Words for a wide sea ("The Sundering Ocean", "The Restless Deep").
const VAST: [&str; 12] = ["Endless", "Sundering", "Restless", "Boundless", "Wailing", "Shattered", "Sullen", "Wine-Dark", "Stormy", "Sleepless", "Silent", "Glittering"];

/// Each culture's words for a ruin, a tower and a camp ("Ulfrun Howe", "The Grey Minaret",
/// "Temur's Yurts"), in `CULTURES` order.
const RUIN_NOUNS: [&[&str]; 5] = [
    &["Howe", "Barrow", "Hall", "Keep", "Stones", "Steading"],
    &["Keep", "Abbey", "Hall", "Stones", "Motte", "Priory"],
    &["Villa", "Basilica", "Forum", "Colonnade", "Baths", "Keep"],
    &["Qasr", "Kasbah", "Ribat", "Ziggurat", "Palace", "Walls"],
    &["Kurgan", "Ovoo", "Stones", "Barrow", "Mound", "Cairn"],
];
const TOWER_NOUNS: [&[&str]; 5] = [
    &["Tower", "Watch", "Beacon", "Broch"],
    &["Tower", "Spire", "Folly", "Turret"],
    &["Tower", "Campanile", "Lighthouse", "Belvedere"],
    &["Tower", "Minaret", "Watchtower", "Burj"],
    &["Tower", "Lookout", "Watchpost", "Pillar"],
];
const CAMP_NOUNS: [&[&str]; 5] = [
    &["Camp", "Stockade", "Shieling", "Outpost", "Bothy"],
    &["Camp", "Stockade", "Palisade", "Outpost", "Bivouac"],
    &["Camp", "Castra", "Bivouac", "Fort", "Outpost"],
    &["Camp", "Encampment", "Tents", "Fort", "Outpost"],
    &["Camp", "Yurts", "Encampment", "Tents", "Horse Lines"],
];
/// Words for a cave and a lava tube.
const CAVE_NOUNS: [&str; 10] = ["Caves", "Hollow", "Caverns", "Grotto", "Mouth", "Maw", "Throat", "Galleries", "Chambers", "Hole"];
const TUBE_NOUNS: [&str; 6] = ["Tubes", "Tunnels", "Flues", "Runs", "Chimneys", "Galleries"];
/// What a volcano does ("The Smoking Crown").
const FUME: [&str; 8] = ["Smoking", "Fuming", "Grumbling", "Sleeping", "Burning", "Ashen", "Molten", "Brooding"];

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
    stems3: BTreeMap<String, u32>,
    own: BTreeSet<String>,
    settled: [u32; 5],
    ends: BTreeMap<(usize, [u8; 2]), u32>,
    /// Per culture, how often each of its endings ("-heim") has gone on a settlement; and how
    /// often each head noun (`site_noun`: "Camp", "Halls") has named a feature.
    endings: BTreeMap<(usize, &'static str), u32>,
    heads: BTreeMap<String, u32>,
    /// The culture words drawn for the name being made, and the ending put on one.
    drawn: Vec<String>,
    ending: Option<&'static str>,
    /// Culture words made, and how many of them came from the letter model.
    pub made: u32,
    pub from_model: u32,
    /// When set, the words of each name given out made from culture words (namecheck's).
    pub log: Option<Vec<String>>,
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
            stems3: BTreeMap::new(),
            own: BTreeSet::new(),
            settled: [0; 5],
            ends: BTreeMap::new(),
            endings: BTreeMap::new(),
            heads: BTreeMap::new(),
            drawn: Vec::new(),
            ending: None,
            made: 0,
            from_model: 0,
            log: None,
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

    /// Two of the culture's curated roots run together ("Thor" + "geir"), lower case.
    fn rooted(&mut self, c: &Culture) -> Option<String> {
        let (a, b) = (self.pick(c.roots.0), self.pick(c.roots.1));
        let b = if a.ends_with(['a', 'e', 'i', 'o', 'u']) && b.starts_with(['a', 'e', 'i', 'o', 'u']) { &b[1..] } else { b };
        join(a, b, 3).map(|w| w.to_lowercase())
    }

    /// A culture word of 1–3 syllables (by length: about 3–6, 4–8, 6–10 letters), capitalized:
    /// from the letter model (two in five), a template or two curated roots, sayable
    /// (`fault`), never a training word nor a letter off one, a model's word not starting as
    /// one does (its first four letters), and only now and then Bj-, Fj- or Hj-.
    pub fn word(&mut self, culture: usize, min_syl: u32, max_syl: u32) -> String {
        let ci = culture % CULTURES.len();
        let c = &CULTURES[ci];
        let lo = [3, 4, 6][(min_syl.clamp(1, 3) - 1) as usize];
        let hi = [6, 8, 10][(max_syl.clamp(1, 3) - 1) as usize];
        let like_training = |w: &str| w.len() >= 4 && CULTURES.iter().any(|c| c.train.iter().any(|t| t.len() >= 4 && t[..4].eq_ignore_ascii_case(&w[..4])));
        for _ in 0..40 {
            let r = self.rng.below(20);
            let w = if r < 13 {
                match markov(ci).walk(&mut self.rng, hi) {
                    Some(w) if !like_training(&w) => w,
                    _ => continue,
                }
            } else if r < 17 || max_syl < 2 {
                let syl = min_syl + self.rng.below(max_syl.max(min_syl) - min_syl + 1);
                self.template(c, syl)
            } else {
                match self.rooted(c) {
                    Some(w) => w,
                    None => continue,
                }
            };
            if (w.starts_with("bj") || w.starts_with("fj") || w.starts_with("hj")) && self.rng.below(10) > 0 {
                continue;
            }
            if (lo..=hi).contains(&w.len()) && fault(&w, Some(ci)).is_none() && !is_training(&w) && !near_training(&w) {
                if self.drawn.len() >= 64 {
                    self.drawn.clear();
                }
                self.drawn.push(w.clone());
                self.made += 1;
                self.from_model += (r < 13) as u32;
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
        self.made += 1;
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
            self.ending = None;
            let n = self.candidate(kind, culture);
            if ok(&n) && (i >= 40 || self.fresh(&n, kind, culture)) && !self.used.contains(&n) {
                self.note(&n, kind, culture);
                return n;
            }
        }
        // Practically unreachable; lengthen the name until unique.
        for k in 0.. {
            self.drawn.clear();
            self.ending = None;
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
    /// before, nor its stem (first four letters) twice or its first three letters three times;
    /// a head noun (`site_noun`) no more often than `noun_cap` says; a settlement's ending
    /// ("-heim") on at most ~10% of its culture's, its last two letters on ~15%.
    fn fresh(&self, n: &str, kind: NameKind, c: usize) -> bool {
        let stem3 = |w: &str| w[..w.len().min(3)].to_string();
        if self.own_words(n).iter().any(|(w, s)| self.own.contains(w) || self.stems.get(s).is_some_and(|&k| k >= 2) || (w.len() >= 5 && self.stems3.get(&stem3(w)).is_some_and(|&k| k >= 3))) {
            return false;
        }
        // ("The Wild Wilds".)
        let ws: Vec<String> = words(n).map(|w| w.to_lowercase()).filter(|w| w.len() >= 4).collect();
        if ws.iter().enumerate().any(|(i, a)| ws[i + 1..].iter().any(|b| a[..4] == b[..4])) {
            return false;
        }
        if kind != NameKind::Settlement {
            let h = site_noun(n);
            if noun_cap(kind, h).is_some_and(|cap| self.heads.get(h).is_some_and(|&k| k >= cap)) {
                return false;
            }
        }
        if kind == NameKind::Settlement {
            let share = |k: u32, p: f64| k >= 2 && (k + 1) as f64 > p * (self.settled[c] + 1) as f64;
            if share(self.ends.get(&(c, ending(n))).copied().unwrap_or(0), 0.15) || self.ending.is_some_and(|e| share(self.endings.get(&(c, e)).copied().unwrap_or(0), 0.1)) {
                return false;
            }
        }
        true
    }

    fn note(&mut self, n: &str, kind: NameKind, c: usize) {
        if let Some(log) = &mut self.log {
            let stems: Vec<String> = self.drawn.iter().map(|w| w[..w.len().min(4)].to_string()).collect();
            log.extend(words(n).filter(|w| stems.iter().any(|s| w.to_lowercase().contains(s.as_str()))));
        }
        for (w, s) in self.own_words(n) {
            *self.stems.entry(s).or_default() += 1;
            *self.stems3.entry(w[..w.len().min(3)].to_string()).or_default() += 1;
            self.own.insert(w);
        }
        if kind != NameKind::Settlement {
            *self.heads.entry(site_noun(n).to_string()).or_default() += 1;
        }
        for w in words(n) {
            *self.seen.entry(w.to_lowercase()).or_default() += 1;
        }
        if kind == NameKind::Settlement {
            self.settled[c] += 1;
            *self.ends.entry((c, ending(n))).or_default() += 1;
            if let Some(e) = self.ending {
                *self.endings.entry((c, e)).or_default() += 1;
            }
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

    /// One of `xs`, used at most twice in a world when it can be.
    fn rare_pick(&mut self, xs: &[&'static str]) -> &'static str {
        let mut a = "";
        for _ in 0..6 {
            a = self.pick(xs);
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
                self.ending = Some(end);
                return w;
            }
        }
        self.word(c, 2, 3)
    }

    fn settlement(&mut self, c: usize) -> String {
        let cu = &CULTURES[c];
        let r = self.rng.next_f64();
        // (Grim compounds are English: the west's and now and then the north's.)
        if r < [0.01, 0.03, 0.0, 0.0, 0.0][c] {
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
            match self.rng.below(20) {
                0 | 1 => {
                    // A one-syllable stem of plain vowels: "Corvamare", not "Ciasamare".
                    for _ in 0..6 {
                        let stem = self.word(c, 1, 1);
                        let stem = stem.trim_end_matches(['a', 'e', 'i', 'o', 'u']);
                        if syllables(stem) == 1 && !stem.contains("ia") && !stem.contains("io") && ban(&format!("{stem}amare")).is_none() {
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
                Cave => self.pick(&CAVE_NOUNS),
                Mine => self.pick(&["Mine", "Shaft", "Pits", "Workings", "Delvings", "Adit"]),
                LavaTube => self.pick(&TUBE_NOUNS),
                _ => self.pick(RUIN_NOUNS[c]),
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
        let vast = self.rare_pick(&VAST);
        let desc = self.rare_pick(&CAVE_DESC);
        let one = self.word(c, 1, 1);
        // English run together ("Grasswater", "Ironspine") is the west's and the north's; the
        // others say it in two words.
        let runs = c <= 1;
        let ruin = self.pick(RUIN_NOUNS[c]);
        let tower = self.pick(TOWER_NOUNS[c]);
        let camp = self.pick(CAMP_NOUNS[c]);
        let fume = self.rare_pick(&FUME);
        let cone = ["Mountain", "Cone", "Crown", "Throne"][self.rng.below(4) as usize];
        let wood = match c {
            3 => ["Grove", "Palms", "Thickets"][self.rng.below(3) as usize],
            2 => ["Wood", "Grove", "Groves"][self.rng.below(3) as usize],
            _ => "Wood",
        };
        match (kind, eng, r) {
            (Continent, _, _) => w,
            (Ocean, false, _) => format!("The {w} Ocean"),
            (Ocean, true, 0) => format!("The {adj} Ocean"),
            (Ocean, true, 1) => format!("The {vast} Ocean"),
            (Ocean, true, 2) => format!("The {vast} Sea"),
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
            (Range, true, 0 | 1) if runs => {
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
            (Volcano, true, 1) if runs => compound(adj, "fire", 4),
            (Volcano, true, 1) => format!("Mount {w}"),
            (Volcano, true, _) => format!("The {fume} {cone}"),
            (Lake, false, 0 | 1) => format!("Lake {w}"),
            (Lake, false, _) => format!("{w} Mere"),
            (Lake, true, 0) => format!("{adj} Lake"),
            (Lake, true, 1) => format!("The {adj} Mere"),
            (Lake, true, _) if c == 1 => compound(adj, "water", 4),
            (Lake, true, _) => format!("The {adj} Lake"),
            (SaltLake, false, _) => format!("The {w} Salt Lake"),
            (SaltLake, true, _) => format!("The {adj} Salt Lake"),
            (SaltFlat, false, _) => format!("The {w} Salt Flats"),
            (SaltFlat, true, _) => format!("The {adj} Flats"),
            (River, false, 0 | 1) => format!("River {w}"),
            (River, false, _) => format!("The {w}"),
            (River, true, 0) => format!("The {adj} River"),
            (River, true, 1) if c == 0 => compound(adj, "beck", 4),
            (River, true, _) if c == 1 => compound(adj, "water", 4),
            (River, true, _) => format!("The {adj} River"),
            (Waterfall, false, _) => format!("{short} Falls"),
            (Waterfall, true, 0) if c == 0 => compound(adj, "force", 4),
            (Waterfall, true, _) => format!("The {adj} Falls"),
            (Forest, false, 0 | 1) => format!("{w} Forest"),
            (Forest, false, _) => format!("The Forest of {w}"),
            (Forest, true, 0) if runs => format!("The {}", compound(adj, "wood", 4)),
            (Forest, true, 1) if c == 1 => format!("The {adj} Weald"),
            (Forest, true, _) => format!("The {adj} {wood}"),
            (Jungle, false, _) => format!("The {w} {jungle}"),
            (Jungle, true, 0) => format!("The {adj} Wilds"),
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
            (Swamp, true, 0 | 1) if runs => format!("The {}", compound(adj, "fen", 4)),
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
            (Ruin, _, 2) if c == 3 => format!("{ruin} {w}"),
            (Ruin, _, 2) => format!("{w} {ruin}"),
            (Ruin, _, _) => format!("The Fallen {ruin}"),
            (Tower, _, 0) => format!("{short}'s {tower}"),
            (Tower, _, 1) if self.rng.below(2) == 0 => format!("The {tower} of {short}"),
            (Tower, _, 1 | 2) => format!("The {adj} Spire"),
            (Tower, _, _) => format!("The {adj} {tower}"),
            (Camp, _, 0) => format!("{short}'s {camp}"),
            (Camp, _, 1) => format!("The {adj} {camp}"),
            (Camp, _, 2) => format!("{short} {camp}"),
            (Camp, _, _) => format!("The {adj} Stockade"),
            (Cave, _, 0) => format!("The {short} Caves"),
            (Cave, _, 1) => format!("The Caves of {one}"),
            (Cave, _, _) => format!("The {desc} {}", self.pick(&CAVE_NOUNS)),
            (Mine, _, 0 | 1) => format!("{short} Mine"),
            (Mine, _, 2) => format!("{short}'s Diggings"),
            (Mine, _, _) => format!("The {adj} Delve"),
            (LavaTube, _, 0) => format!("The {short} Tubes"),
            (LavaTube, _, 1) => format!("The {short} {}", self.pick(&TUBE_NOUNS)),
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
