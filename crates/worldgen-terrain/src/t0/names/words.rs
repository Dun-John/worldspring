//! The culture tables and word lists the namer draws from.

pub struct Culture {
    pub(super) onsets: &'static [&'static str],
    pub(super) vowels: &'static [&'static str],
    pub(super) codas: &'static [&'static str],
    /// The approved words its letter model learns from (never given out as they are).
    pub(super) train: &'static [&'static str],
    /// Curated roots: first parts and last parts run together ("Thor" + "geir").
    pub(super) roots: (&'static [&'static str], &'static [&'static str]),
    /// Endings put on a culture word for a place.
    pub(super) endings: &'static [&'static str],
    /// English place-name pieces in its flavour: first parts and last parts ("Raven" + "holm").
    pub(super) pre: &'static [&'static str],
    pub(super) suf: &'static [&'static str],
    /// Its own words for describing land, beside the shared `ADJ`.
    pub(super) adj: &'static [&'static str],
    /// The share of its settlements named in English, and of its land features.
    pub(super) english: f64,
    pub(super) nature: f64,
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

pub(super) const RANGE_NOUN: [&str; 9] = ["spine", "teeth", "crown", "wall", "fang", "horn", "reach", "spire", "back"];

/// "The Vale of ___".
pub(super) const OF_Y: [&str; 26] = [
    "Storms", "Whispers", "Echoes", "Mists", "Thorns", "Kings", "Giants", "Ravens", "Winds", "Stars", "Glass", "Lanterns", "Crowns", "Wolves", "Tides", "Dawn", "Silence",
    "Bells", "Spires", "Eagles", "Serpents", "Sorrows", "Thunder", "Mirrors", "Dreams", "Embers",
];

/// Grim words: "The Drowned Mire", "Widow's Pass", "The Marsh of Tears", "Gallowsford".
pub(super) const GRIM_ADJ: [&str; 25] = [
    "Dead", "Drowned", "Bleak", "Murk", "Grim", "Dread", "Gloom", "Wraith", "Carrion", "Rotting", "Blood", "Bone", "Shrieking", "Weeping", "Cursed", "Forsaken", "Lost",
    "Hanging", "Mourning", "Corpse", "Ghoul", "Gallows", "Raven", "Witching", "Sunless",
];
pub(super) const GRIM_POSS: [&str; 10] = ["Deadman's", "Widow's", "Hangman's", "Devil's", "Beggar's", "Crow's", "Wight's", "Witch's", "Traitor's", "Ghoul's"];
pub(super) const GRIM_OF: [&str; 14] = ["Sorrows", "Bones", "Tears", "Woe", "the Dead", "Ashes", "Despair", "the Lost", "Shadows", "Graves", "Whispers", "Ruin", "Lament", "the Hanged"];
pub(super) const GRIM_PRE: [&str; 12] = ["Gallows", "Dead", "Murk", "Gloom", "Crow", "Grave", "Bone", "Mourn", "Wither", "Black", "Rot", "Grim"];

/// Kings and saints (the north and the west): "Kingsbridge", "Queensmere".
pub(super) const ROYAL: [&str; 10] = ["Kings", "Queens", "Princes", "Earls", "Bishops", "Abbots", "Monks", "Friars", "Knights", "Lords"];
pub(super) const ROYAL_SUF: [&str; 15] = ["bridge", "mere", "ford", "wood", "well", "gate", "bury", "ton", "haven", "holm", "field", "cross", "barrow", "stow", "hold"];

/// Words for an underground site by its theme (`under::THEMES` keys): its describing words and
/// its nouns ("The Rime Caves", "Glimmer Grotto").
pub(super) const THEME_WORDS: [(&str, &[&str], &[&str]); 25] = [
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
pub(super) const CLUSTERS: [&str; 33] = [
    "skj", "str", "ngr", "ndr", "rst", "nst", "ldr", "rth", "lth", "nth", "thr", "shr", "chr", "spr", "scr", "skr", "ght", "nch", "rch", "nkh", "rsh", "lsh", "nsh", "ngh",
    "nds", "rds", "lds", "lst", "nsk", "rsk", "mbr", "mbl", "ngl",
];
/// Letter pairs no name has.
pub(super) const BANNED: [&str; 19] = ["jk", "qk", "vj", "gk", "kg", "tl", "dt", "hh", "yy", "wq", "xz", "ii", "uu", "jj", "vv", "ww", "qq", "xx", "zh"];
/// Letter runs that read badly anywhere ("Gochiuk", "Brytraill", "Grirddal", "Buduor", "Moel").
pub(super) const BAD_RUNS: [&str; 13] = ["iuk", "ytr", "eack", "eamy", "yll", "rdd", "rkh", "lkh", "oolk", "uo", "urr", "oe", "ieh"];
/// Bits of real words that read as a joke, rude or foreign to these tongues in a made-up word
/// ("Orge", "Bungi", "Nemum", "Yeting").
pub(super) const COMIC: [&str; 34] = [
    "ogre", "orge", "bung", "dung", "mum", "bum", "poo", "fart", "piss", "shit", "fuk", "cum", "tit", "dick", "cock", "turd", "butt", "puke", "snot", "anus", "boob", "wank",
    "porn", "nazi", "ing", "yin", "khan", "dork", "nerd", "slut", "whor", "organ", "sahara", "chan",
];
/// Real given names a made-up word may land on (owners come from their culture's words).
pub(super) const REAL_NAMES: [&str; 40] = [
    "brian", "alan", "kevin", "gary", "barry", "dave", "steve", "nigel", "derek", "karen", "doris", "norman", "gordon", "dennis", "harry", "terry", "larry", "colin",
    "keith", "brenda", "linda", "sandra", "susan", "donna", "tracy", "carol", "bob", "tom", "tim", "ben", "dan", "sam", "ted", "ian", "jim", "joe", "ken", "ron", "sue",
    "mike",
];

/// Runs read badly or as English in a culture word ("Zaztiqai", "Jauthest", "Glaock", "Whock",
/// "Hushuzair", "Hauskall", "Porrol", "Chukhot").
pub(super) const NEW_RUNS: [&str; 20] = ["zt", "zz", "hz", "rgr", "auth", "auj", "aust", "aock", "aime", "orr", "who", "whu", "hush", "haus", "bantam", "otic", "crud", "krud", "hot", "qang"];
/// Starts that read as English ("Hitisad", "Hidun", "Sirano").
pub(super) const JOKE_START: [&str; 6] = ["hit", "hid", "sir", "his", "her", "hum"];
/// Short English words and names a culture word may be ("The Both Archive").
pub(super) const SHORT_WORDS: &str = "both bath moth hath then than them this that with were here rain main more sore bore bone done gone none hide side tide ride road toad load \
    sand band land hand wand bend lend mend send tend rent tent dent bent sent mist list fist hint mint tint sink link dark bark park mark tusk dusk husk tomb bomb limb lamb \
    dumb numb bint third fiona tina nina lena anna emma ella sara maria rosa eric ivan omar lisa mona dana nora vera hana tara kara mara rita gina dina lola tony toby mary";

/// Site nouns no world has more than two of ("The Glimmer Grotto", "The Weeping Hollow").
pub(super) const RARE_NOUNS: [&str; 12] = ["Delve", "Grotto", "Hollow", "Lair", "Diggings", "Workings", "Spire", "Burrows", "Vents", "Deeps", "Oubliette", "Stockade"];

/// Words that say what a cave does ("The Weeping Hollow").
pub(super) const CAVE_DESC: [&str; 16] = [
    "Weeping", "Whispering", "Howling", "Echoing", "Dripping", "Sighing", "Hungry", "Sleeping", "Moaning", "Singing", "Yawning", "Gaping", "Mourning", "Murmuring", "Wailing",
    "Grinning",
];

/// Words for a wide sea ("The Sundering Ocean", "The Restless Deep").
pub(super) const VAST: [&str; 12] = ["Endless", "Sundering", "Restless", "Boundless", "Wailing", "Shattered", "Sullen", "Wine-Dark", "Stormy", "Sleepless", "Silent", "Glittering"];

/// Each culture's words for a ruin, a tower and a camp ("Ulfrun Howe", "The Grey Minaret",
/// "Temur's Yurts"), in `CULTURES` order.
pub(super) const RUIN_NOUNS: [&[&str]; 5] = [
    &["Howe", "Barrow", "Hall", "Keep", "Stones", "Steading"],
    &["Keep", "Abbey", "Hall", "Stones", "Motte", "Priory"],
    &["Villa", "Basilica", "Forum", "Colonnade", "Baths", "Keep"],
    &["Qasr", "Kasbah", "Ribat", "Ziggurat", "Palace", "Walls"],
    &["Kurgan", "Ovoo", "Stones", "Barrow", "Mound", "Cairn"],
];
pub(super) const TOWER_NOUNS: [&[&str]; 5] = [
    &["Tower", "Watch", "Beacon", "Broch"],
    &["Tower", "Spire", "Folly", "Turret"],
    &["Tower", "Campanile", "Lighthouse", "Belvedere"],
    &["Tower", "Minaret", "Watchtower", "Burj"],
    &["Tower", "Lookout", "Watchpost", "Pillar"],
];
pub(super) const CAMP_NOUNS: [&[&str]; 5] = [
    &["Camp", "Stockade", "Shieling", "Outpost", "Bothy"],
    &["Camp", "Stockade", "Palisade", "Outpost", "Bivouac"],
    &["Camp", "Castra", "Bivouac", "Fort", "Outpost"],
    &["Camp", "Encampment", "Tents", "Fort", "Outpost"],
    &["Camp", "Yurts", "Encampment", "Tents", "Horse Lines"],
];
/// Words for a cave and a lava tube.
pub(super) const CAVE_NOUNS: [&str; 10] = ["Caves", "Hollow", "Caverns", "Grotto", "Mouth", "Maw", "Throat", "Galleries", "Chambers", "Hole"];
pub(super) const TUBE_NOUNS: [&str; 6] = ["Tubes", "Tunnels", "Flues", "Runs", "Chimneys", "Galleries"];
/// What a volcano does ("The Smoking Crown").
pub(super) const FUME: [&str; 8] = ["Smoking", "Fuming", "Grumbling", "Sleeping", "Burning", "Ashen", "Molten", "Brooding"];
