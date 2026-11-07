//! Building functions (Appendix B of the plan) with the tier each first appears at, the tier
//! from which it is guaranteed, how many a settlement gets, ward preferences and name style.
//! A metropolis gets every entry (water trades only when it is on water).

use serde::Serialize;

use crate::t0::settle::Tier;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum Ward {
    Plaza = 0,
    Castle,
    Temple,
    Merchant,
    Craft,
    Noble,
    Common,
    Slum,
    Docks,
    Military,
    Farm,
    /// Village houses.
    Rural,
    Park,
}

impl Ward {
    pub fn name(self) -> &'static str {
        ["plaza", "castle", "temple", "merchant", "craft", "noble", "common", "slum", "docks", "military", "farm", "village", "park"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Naming {
    /// "The Prancing Pony".
    Sign,
    /// "Harlan's Smithy".
    Owner,
    /// "Temple of the Dawn", "The Grey Academy".
    Institution,
    /// No business name (warehouses, graveyards, hidden places).
    Plain,
}

pub struct Func {
    pub key: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    /// Can appear from this tier.
    pub min_tier: Tier,
    /// Guaranteed from this tier.
    pub required: Option<Tier>,
    /// Copies guaranteed at village / town / city / metropolis (0 = just one if required).
    pub counts: [u8; 4],
    pub wards: &'static [Ward],
    pub naming: Naming,
    /// Needs the settlement to be on water (sea, lake or river).
    pub water: bool,
    /// Wants a big footprint (the largest lots in its ward).
    pub big: bool,
    /// Trade noun for owner-style names.
    pub trade: &'static str,
}

use Naming::*;
use Tier::*;
use Ward::*;

#[allow(clippy::too_many_arguments)]
const fn f(
    key: &'static str,
    name: &'static str,
    category: &'static str,
    min_tier: Tier,
    required: Option<Tier>,
    counts: [u8; 4],
    wards: &'static [Ward],
    naming: Naming,
    water: bool,
    big: bool,
    trade: &'static str,
) -> Func {
    Func { key, name, category, min_tier, required, counts, wards, naming, water, big, trade }
}

const FOOD: &str = "food & lodging";
const ARMS: &str = "arms & gear";
const MAGIC: &str = "magic & knowledge";
const LUX: &str = "luxury & finance";
const CRAFT: &str = "crafts & transport";
const FAITH: &str = "faith & death";
const CIVIC: &str = "civic & military";
const GUILD: &str = "guilds & underworld";
const FUN: &str = "entertainment & oddities";

pub const CATALOG: &[Func] = &[
    // Food & lodging.
    f("inn", "Inn", FOOD, Village, Some(Village), [1, 2, 4, 8], &[Plaza, Merchant, Common, Rural], Sign, false, true, "Inn"),
    f("tavern", "Tavern", FOOD, Village, Some(Town), [0, 1, 3, 6], &[Common, Merchant, Docks, Rural], Sign, false, false, "Tavern"),
    f("alehouse", "Alehouse", FOOD, Town, Some(City), [0, 0, 2, 4], &[Slum, Common, Docks], Sign, false, false, "Alehouse"),
    f("fine_dining", "Fine dining", FOOD, City, Some(City), [0, 0, 1, 2], &[Noble, Merchant], Sign, false, false, "Table"),
    f("bakery", "Bakery", FOOD, Town, Some(Town), [0, 1, 2, 4], &[Common, Merchant], Owner, false, false, "Bakery"),
    f("butcher", "Butcher", FOOD, Town, Some(Town), [0, 1, 2, 3], &[Common, Craft], Owner, false, false, "Butchery"),
    f("fishmonger", "Fishmonger", FOOD, Village, Some(City), [0, 0, 1, 2], &[Docks, Merchant], Owner, true, false, "Fish Stall"),
    f("grocer", "Grocer", FOOD, Town, Some(Town), [0, 1, 2, 3], &[Merchant, Common], Owner, false, false, "Greengrocer"),
    f("brewery", "Brewery", FOOD, Town, Some(City), [0, 0, 1, 2], &[Craft], Owner, false, true, "Brewery"),
    f("winery", "Winery", FOOD, Town, Some(City), [0, 0, 1, 1], &[Craft, Noble], Owner, false, true, "Winery"),
    f("distillery", "Distillery", FOOD, City, Some(Metropolis), [0, 0, 0, 1], &[Craft], Owner, false, true, "Distillery"),
    // Arms & gear.
    f("blacksmith", "Blacksmith", ARMS, Village, Some(Town), [0, 1, 2, 3], &[Craft, Rural, Common], Owner, false, false, "Smithy"),
    f("armorer", "Armorer", ARMS, City, Some(City), [0, 0, 1, 2], &[Craft, Military], Owner, false, false, "Armory"),
    f("weaponsmith", "Weaponsmith", ARMS, City, Some(City), [0, 0, 1, 2], &[Craft, Military], Owner, false, false, "Blades"),
    f("bowyer", "Bowyer & fletcher", ARMS, Town, Some(Town), [0, 1, 1, 2], &[Craft], Owner, false, false, "Bows"),
    f("leatherworker", "Leatherworker & tanner", ARMS, Town, Some(Town), [0, 1, 1, 2], &[Craft, Slum], Owner, false, false, "Tannery"),
    f("outfitter", "Outfitter", ARMS, Town, Some(City), [0, 0, 1, 2], &[Merchant], Owner, false, false, "Outfitters"),
    f("tailor", "Tailor", ARMS, Town, Some(Town), [0, 1, 2, 3], &[Merchant, Common], Owner, false, false, "Tailoring"),
    f("cobbler", "Cobbler", ARMS, Town, Some(Town), [0, 1, 1, 2], &[Common], Owner, false, false, "Boots"),
    // Magic & knowledge.
    f("alchemist", "Alchemist", MAGIC, Town, Some(City), [0, 0, 1, 2], &[Merchant, Craft], Owner, false, false, "Alchemy"),
    f("apothecary", "Apothecary & herbalist", MAGIC, Village, Some(Town), [0, 1, 2, 3], &[Common, Merchant, Rural], Owner, false, false, "Remedies"),
    f("magic_shop", "Magic shop", MAGIC, City, Some(City), [0, 0, 1, 2], &[Merchant, Noble], Owner, false, false, "Curios Arcane"),
    f("scribe", "Scribe", MAGIC, Town, Some(City), [0, 0, 1, 2], &[Merchant, Temple], Owner, false, false, "Scriptorium"),
    f("bookseller", "Bookseller", MAGIC, City, Some(City), [0, 0, 1, 2], &[Merchant, Noble], Owner, false, false, "Books"),
    f("cartographer", "Cartographer", MAGIC, City, Some(City), [0, 0, 1, 1], &[Merchant, Docks], Owner, false, false, "Maps & Charts"),
    f("library", "Library", MAGIC, City, Some(City), [0, 0, 1, 2], &[Temple, Noble, Plaza], Institution, false, true, "Library"),
    f("arcane_academy", "Arcane academy", MAGIC, Metropolis, Some(Metropolis), [0, 0, 0, 1], &[Noble, Temple], Institution, false, true, "Academy"),
    f("observatory", "Observatory", MAGIC, City, Some(Metropolis), [0, 0, 0, 1], &[Noble, Temple], Institution, false, true, "Observatory"),
    f("fortune_teller", "Fortune teller", MAGIC, Town, Some(City), [0, 0, 1, 2], &[Slum, Common], Sign, false, false, "Tent"),
    // Luxury & finance.
    f("jeweler", "Jeweler", LUX, City, Some(City), [0, 0, 1, 2], &[Merchant, Noble], Owner, false, false, "Jewels"),
    f("bank", "Bank & moneychanger", LUX, City, Some(City), [0, 0, 1, 2], &[Merchant, Plaza], Institution, false, true, "Counting House"),
    f("pawnbroker", "Pawnbroker", LUX, Town, Some(City), [0, 0, 1, 2], &[Slum, Common], Owner, false, false, "Pawn"),
    f("auction_house", "Auction house", LUX, City, Some(Metropolis), [0, 0, 0, 1], &[Merchant, Plaza], Institution, false, true, "Auction House"),
    f("perfumer", "Perfumer", LUX, City, Some(City), [0, 0, 1, 1], &[Merchant, Noble], Owner, false, false, "Perfumery"),
    // Crafts & transport.
    f("carpenter", "Carpenter", CRAFT, Village, Some(Town), [0, 1, 2, 3], &[Craft, Rural], Owner, false, false, "Joinery"),
    f("mason", "Mason", CRAFT, Town, Some(Town), [0, 1, 1, 2], &[Craft], Owner, false, false, "Stoneworks"),
    f("cooper", "Cooper", CRAFT, Town, Some(Town), [0, 1, 1, 2], &[Craft, Docks], Owner, false, false, "Cooperage"),
    f("glassblower", "Glassblower", CRAFT, City, Some(City), [0, 0, 1, 1], &[Craft], Owner, false, false, "Glassworks"),
    f("potter", "Potter", CRAFT, Village, Some(Town), [0, 1, 1, 2], &[Craft, Rural], Owner, false, false, "Pottery"),
    f("weaver", "Weaver & dyer", CRAFT, Town, Some(Town), [0, 1, 1, 2], &[Craft, Slum], Owner, false, false, "Looms"),
    f("tinker", "Tinker", CRAFT, Town, Some(City), [0, 0, 1, 2], &[Craft, Common], Owner, false, false, "Tinkery"),
    f("shipwright", "Shipwright", CRAFT, Town, Some(City), [0, 0, 1, 2], &[Docks], Owner, true, true, "Shipyard"),
    f("stables", "Stables", CRAFT, Village, Some(Town), [0, 1, 2, 3], &[Common, Military, Rural], Owner, false, true, "Stables"),
    f("cartwright", "Cartwright", CRAFT, Town, Some(Town), [0, 1, 1, 2], &[Craft], Owner, false, false, "Wheelwrights"),
    f("warehouse", "Warehouse", CRAFT, Town, Some(City), [0, 0, 3, 6], &[Docks, Craft], Plain, false, true, ""),
    f("caravanserai", "Caravanserai", CRAFT, City, Some(Metropolis), [0, 0, 0, 1], &[Merchant, Common], Sign, false, true, "Caravanserai"),
    f("general_store", "General store", CRAFT, Village, Some(Town), [0, 1, 1, 2], &[Merchant, Common, Rural], Owner, false, false, "Goods"),
    f("mill", "Mill", CRAFT, Village, Some(Village), [1, 1, 1, 2], &[Rural, Craft, Farm], Owner, false, true, "Mill"),
    // Faith & death.
    f("temple", "Temple", FAITH, Town, Some(Town), [0, 1, 3, 6], &[Temple, Plaza], Institution, false, true, "Temple"),
    f("shrine", "Shrine", FAITH, Village, Some(Village), [1, 1, 2, 4], &[Rural, Temple, Common], Institution, false, false, "Shrine"),
    f("monastery", "Monastery", FAITH, Town, Some(City), [0, 0, 1, 1], &[Temple, Farm], Institution, false, true, "Monastery"),
    f("undertaker", "Undertaker", FAITH, Town, Some(City), [0, 0, 1, 2], &[Temple, Common], Owner, false, false, "Undertakers"),
    f("graveyard", "Graveyard", FAITH, Village, Some(Village), [1, 1, 1, 2], &[Temple, Rural, Farm], Plain, false, true, ""),
    f("mausoleum", "Mausoleum", FAITH, City, Some(City), [0, 0, 1, 2], &[Temple, Noble], Institution, false, false, "Mausoleum"),
    f("catacombs", "Catacombs entrance", FAITH, City, Some(City), [0, 0, 1, 1], &[Temple], Institution, false, false, "Catacombs"),
    // Civic & military.
    f("town_hall", "Town hall", CIVIC, Town, Some(Town), [0, 1, 1, 1], &[Plaza, Merchant], Institution, false, true, "Hall"),
    f("courthouse", "Courthouse", CIVIC, City, Some(City), [0, 0, 1, 1], &[Plaza, Noble], Institution, false, true, "Court"),
    f("prison", "Prison", CIVIC, City, Some(City), [0, 0, 1, 1], &[Military, Castle], Institution, false, true, "Gaol"),
    f("barracks", "Barracks", CIVIC, City, Some(City), [0, 0, 1, 2], &[Military, Castle], Institution, false, true, "Barracks"),
    f("guard_post", "Guard post", CIVIC, Town, Some(Town), [0, 1, 2, 4], &[Military, Common, Plaza], Institution, false, false, "Watch House"),
    f("gatehouse", "Gatehouse", CIVIC, City, Some(City), [0, 0, 1, 2], &[Military], Institution, false, false, "Gatehouse"),
    f("castle", "Castle", CIVIC, City, Some(City), [0, 0, 1, 1], &[Castle], Institution, false, true, "Keep"),
    f("palace", "Palace", CIVIC, Metropolis, Some(Metropolis), [0, 0, 0, 1], &[Castle, Noble], Institution, false, true, "Palace"),
    f("harbor_master", "Harbor master", CIVIC, Town, Some(City), [0, 0, 1, 1], &[Docks], Institution, true, false, "Harbor Office"),
    f("customs_house", "Customs house", CIVIC, City, Some(City), [0, 0, 1, 1], &[Docks], Institution, true, false, "Customs"),
    // Guilds & underworld.
    f("merchant_guild", "Merchants' guild", GUILD, City, Some(City), [0, 0, 1, 1], &[Merchant, Plaza], Institution, false, true, "Guildhall"),
    f("craft_guild", "Craft guild", GUILD, City, Some(City), [0, 0, 1, 2], &[Craft], Institution, false, true, "Guildhall"),
    f("adventurers_guild", "Adventurers' guild", GUILD, City, Some(City), [0, 0, 1, 1], &[Common, Merchant], Institution, false, true, "Lodge"),
    f("mages_guild", "Mages' guild", GUILD, City, Some(City), [0, 0, 1, 1], &[Noble, Merchant], Institution, false, true, "Conclave"),
    f("thieves_guild", "Thieves' guild (hidden)", GUILD, City, Some(City), [0, 0, 1, 1], &[Slum, Docks], Plain, false, false, ""),
    f("black_market", "Black market", GUILD, Metropolis, Some(Metropolis), [0, 0, 0, 1], &[Slum, Docks], Plain, false, false, ""),
    f("smugglers_den", "Smugglers' den", GUILD, City, Some(City), [0, 0, 1, 1], &[Docks, Slum], Sign, false, false, "Den"),
    f("gambling_den", "Gambling den", GUILD, Town, Some(City), [0, 0, 1, 2], &[Slum, Common], Sign, false, false, "Dice House"),
    // Entertainment & oddities.
    f("arena", "Arena", FUN, City, Some(City), [0, 0, 1, 1], &[Common, Military], Institution, false, true, "Arena"),
    f("fighting_pit", "Fighting pit", FUN, Town, Some(City), [0, 0, 1, 1], &[Slum], Sign, false, false, "Pit"),
    f("theater", "Theater", FUN, City, Some(City), [0, 0, 1, 2], &[Merchant, Noble, Plaza], Institution, false, true, "Playhouse"),
    f("bathhouse", "Bathhouse", FUN, Town, Some(City), [0, 0, 1, 2], &[Common, Merchant], Sign, false, true, "Baths"),
    f("menagerie", "Menagerie", FUN, City, Some(Metropolis), [0, 0, 0, 1], &[Noble, Park], Institution, false, true, "Menagerie"),
    f("gardens", "Gardens", FUN, City, Some(Metropolis), [0, 0, 0, 1], &[Noble, Park], Institution, false, true, "Gardens"),
    f("curiosities", "Curiosities shop", FUN, Town, Some(City), [0, 0, 1, 1], &[Merchant, Slum], Owner, false, false, "Curiosities"),
    f("healer", "Healer", FUN, Village, Some(Town), [0, 1, 1, 2], &[Temple, Common], Owner, false, false, "Infirmary"),
    f("orphanage", "Orphanage", FUN, City, Some(City), [0, 0, 1, 1], &[Temple, Slum], Institution, false, false, "Orphanage"),
    f("bards_college", "Bards' college", FUN, City, Some(Metropolis), [0, 0, 0, 1], &[Noble, Merchant], Institution, false, true, "College"),
    // v63 countryside: set only by the layout (`rural::place`), on a rise among the fields: never on a
    // lot (`required_count` 0, and no tier draws it at random).
    f("windmill", "Windmill", CRAFT, Metropolis, None, [0, 0, 0, 0], &[Rural, Farm], Owner, false, false, "Windmill"),
];

/// Residential building kinds by ward (index into `RESIDENTIAL`).
pub const RESIDENTIAL: [&str; 8] = ["hovel", "house", "townhouse", "tenement", "noble estate", "farmhouse", "wizard's tower", "ruined building"];
pub const WIZARD_TOWER: usize = 6;
pub const RUINED: usize = 7;

pub fn residential_for(ward: Ward, big: bool) -> usize {
    match ward {
        Ward::Slum => if big { 3 } else { 0 },
        Ward::Noble | Ward::Castle => 4,
        Ward::Merchant | Ward::Plaza => 2,
        Ward::Farm => 5,
        Ward::Rural => 1,
        _ => if big { 3 } else { 1 },
    }
}

/// Copies of `func` this settlement is guaranteed.
pub fn required_count(func: &Func, tier: Tier, on_water: bool) -> usize {
    if func.key == "windmill" {
        return 0;
    }
    if func.water && !on_water {
        return 0;
    }
    if tier == Tier::Metropolis {
        return (func.counts[3] as usize).max(1);
    }
    match func.required {
        Some(t) if tier >= t => (func.counts[tier as usize] as usize).max(1),
        _ => 0,
    }
}

pub fn index_of(key: &str) -> Option<usize> {
    CATALOG.iter().position(|f| f.key == key)
}

// Business-name grammar.
/// A culture's words for its businesses' names (`town::business_name`), in `t0::names::CULTURES`
/// order.
pub struct Signs {
    /// "The Laughing ___".
    pub adj: &'static [&'static str],
    /// What hangs on an inn's sign.
    pub noun: &'static [&'static str],
    /// "Hrolf's ___": an inn kept by its owner.
    pub rest: &'static [&'static str],
    /// "The ___ Library".
    pub inst: &'static [&'static str],
    /// The gods its temples and shrines are for (each one of `DEITY`).
    pub gods: &'static [&'static str],
}

pub const SIGNS: [Signs; 5] = [
    Signs {
        adj: &[
            "Drunken", "Grey", "Salty", "Black", "Howling", "Laughing", "Iron", "Sleeping", "Tarred", "Bearded", "Horned", "Wandering", "Thirsty", "Stout", "Red", "Frozen",
            "Snoring", "Golden", "Weary", "Merry", "Rusty", "One-Eyed", "Shaggy", "Brawling", "Singing", "Silver", "White", "Bold", "Grumbling", "Lucky", "Blind", "Sea-Worn",
        ],
        noun: &[
            "Bear", "Wolf", "Raven", "Longship", "Elk", "Seal", "Whale", "Hammer", "Axe", "Boar", "Skald", "Troll", "Walrus", "Herring", "Cauldron", "Horn", "Bearskin", "Crow", "Oar",
            "Anchor", "Hound", "Goat", "Reindeer", "Mead Hall", "Shield", "Otter", "Eagle", "Hearth", "Sledge", "Lynx", "Puffin", "Ram", "Salmon", "Drinking Horn", "Fox",
            "Badger", "Owl", "Stag", "Gull", "Harpoon", "Rune Stone",
        ],
        rest: &["Rest", "Hearth", "Hall", "Longhouse", "Board", "Fire"],
        inst: &["Grey", "High", "Iron", "Frost", "Raven", "Stone", "Storm", "Rune", "Silver", "Northern", "Winter", "Black"],
        gods: &["the All-Father", "the White Lady", "the Wolf Mother", "the Hammer", "the Thunderer", "the Storm", "the Hearth", "the Deep Sea", "the Watcher", "the Moon"],
    },
    Signs {
        adj: &[
            "Prancing", "Drunken", "Gilded", "Rusty", "Laughing", "Sleeping", "Golden", "Silver", "Crooked", "Wandering", "Jolly", "Salty", "Broken", "Hungry", "Red", "Green",
            "Blind", "Black", "Merry", "Weary", "Painted", "Howling", "Copper", "Lucky", "Grinning", "Stout", "Dancing", "Speckled", "Royal", "Bashful", "Nodding", "Tipsy",
            "Wise", "Proud", "Hooded", "Fighting",
        ],
        noun: &[
            "Pony", "Dragon", "Griffin", "Stag", "Boar", "Goose", "Tankard", "Lantern", "Crown", "Barrel", "Hound", "Owl", "Flagon", "Wyvern", "Rooster", "Wheel", "Hart", "Fox",
            "Badger", "Maiden", "Giant", "Unicorn", "Candle", "Shield", "Plough", "Swan", "Bell", "Rose", "Lamb", "Bull", "Pheasant", "Harp", "Mitre", "Wheatsheaf", "Magpie",
            "Hare", "Otter", "Heron", "Falcon", "Pilgrim", "Ram", "Primrose", "Acorn", "Feathers", "Chequers", "Woolpack", "Cauldron", "Weasel", "Jester", "Bishop",
            "Kingfisher",
        ],
        rest: &["Rest", "Hearth", "Lodge", "Table", "House", "Cellar", "Board"],
        inst: &["Grey", "High", "Silver", "Dawn", "Iron", "Amber", "Twilight", "Azure", "Radiant", "Ashen", "Verdant", "Stone", "Starlit", "Royal", "Elder", "Hallowed"],
        gods: &["the Dawn", "the Morning Lord", "the Harvest", "the Seven", "the Green Man", "the Lady of the Lake", "the Shepherd", "the Lantern Bearer", "the Hearth", "the Forge", "the Moon", "the Watcher"],
    },
    Signs {
        adj: &[
            "Golden", "Gilded", "Laughing", "Merry", "Painted", "Velvet", "Singing", "Amber", "Crimson", "Silver", "Blue", "Wandering", "Sleeping", "Drunken", "Lucky", "Jolly",
            "Dancing", "Smiling", "Masked", "Sunlit", "Purple", "Weeping", "Sweet", "Idle", "Proud", "Winking", "Fat", "Noble",
        ],
        noun: &[
            "Grape", "Olive", "Dolphin", "Lion", "Peacock", "Galley", "Amphora", "Gondola", "Lute", "Mask", "Fig", "Lemon", "Sun", "Mermaid", "Goat", "Bull", "Sparrow", "Cypress",
            "Fountain", "Pearl", "Anchor", "Orange", "Swallow", "Vine", "Octopus", "Harp", "Laurel", "Tuna", "Pomegranate", "Wine Jar", "Heron", "Tortoise", "Seashell", "Crab",
            "Gull", "Nightingale", "Donkey", "Cockerel", "Mandolin",
        ],
        rest: &["Table", "House", "Terrace", "Cellar", "Courtyard", "Rest", "Loggia"],
        inst: &["Golden", "Marble", "Azure", "Radiant", "Laurel", "High", "Sunlit", "Crimson", "Silver", "Ivory", "Gilded", "Coral"],
        gods: &["the Sun", "the Sea Mother", "the Vintner", "the Unconquered Sun", "the Twin Stars", "the Harvest", "the Veiled Lady", "the Dawn", "the Deep Sea", "the Forge"],
    },
    Signs {
        adj: &[
            "Golden", "Silver", "Thirsty", "Wandering", "Patient", "Sleeping", "Blue", "Painted", "Copper", "Veiled", "Laughing", "Singing", "Jewelled", "Brass", "Dusty", "Seven",
            "Hidden", "Shaded", "Sun-Kissed", "Crimson", "Proud", "Clever", "Weary", "Gleaming", "Lucky", "Spotted",
        ],
        noun: &[
            "Camel", "Oasis", "Lamp", "Palm", "Scorpion", "Falcon", "Jackal", "Carpet", "Caravan", "Gazelle", "Saddle", "Well", "Pomegranate", "Dune", "Tent", "Crescent", "Star",
            "Cistern", "Fig", "Ibis", "Date Palm", "Fountain", "Hawk", "Tiger", "Water Jar", "Peacock", "Lizard", "Oryx", "Hoopoe", "Coffee Pot", "Rose", "Lute", "Serpent", "Ostrich",
            "Leopard", "Sandal",
        ],
        rest: &["Rest", "House", "Courtyard", "Tent", "Well"],
        inst: &["Golden", "Brass", "High", "Sun", "Amber", "Copper", "Moon", "Starlit", "Silver", "Saffron", "Indigo", "Crescent"],
        gods: &["the Sun", "the Veiled Lady", "the Well Keeper", "the Night Sky", "the Thousand Eyes", "the Moon", "the Watcher", "the Morning Lord", "the Forge"],
    },
    Signs {
        adj: &[
            "Swift", "Red", "White", "Black", "Grey", "Wandering", "Thirsty", "Laughing", "Wild", "Bold", "Blue", "Painted", "Golden", "Howling", "Sleeping", "Lucky", "Spotted",
            "Dappled", "Restless", "Brave", "Shaggy", "Galloping", "Proud", "Whistling", "Silver", "Hungry",
        ],
        noun: &[
            "Horse", "Mare", "Yurt", "Bow", "Eagle", "Wolf", "Saddle", "Yak", "Hawk", "Stallion", "Arrow", "Camel", "Cookpot", "Sky", "Banner", "Drum", "Ox", "Marmot", "Antelope",
            "Felt Tent", "Bridle", "Quiver", "Crane", "Foal", "Fox", "Bustard", "Kumis Jug", "Horsehead", "Snow Leopard", "Falcon", "Steppe Wind", "Lasso", "Fiddle", "Goat", "Bear",
        ],
        rest: &["Rest", "Fire", "Yurt", "Tent", "Camp", "Hearth"],
        inst: &["Blue", "Sky", "Golden", "High", "White", "Eternal", "Silver", "Wind", "Grey", "Great", "Felt", "Horse"],
        gods: &["the Eternal Sky", "the Earth Mother", "the Wind Rider", "the Ancestors", "the Storm", "the Moon", "the Sun", "the Hearth", "the Watcher"],
    },
];
/// Every god a temple may be for, in `temple::god_look`'s order (the first twelve as they were).
pub const DEITY: &[&str] = &[
    "the Dawn", "the Morning Lord", "the Harvest", "the Deep Sea", "the Forge", "the Moon", "the Seven", "the Watcher", "the Hearth", "the Storm", "the Veiled Lady", "the Sun",
    "the All-Father", "the White Lady", "the Wolf Mother", "the Hammer", "the Thunderer", "the Green Man", "the Lady of the Lake", "the Shepherd", "the Lantern Bearer",
    "the Sea Mother", "the Vintner", "the Unconquered Sun", "the Twin Stars", "the Well Keeper", "the Night Sky", "the Thousand Eyes", "the Eternal Sky", "the Earth Mother",
    "the Wind Rider", "the Ancestors",
];

/// Arms an inn of the north or the west may bear ("The Masons' Arms").
pub const INN_ARMS: [&str; 16] = [
    "King's", "Masons'", "Carpenters'", "Bishop's", "Smiths'", "Coopers'", "Drovers'", "Ploughman's", "Shepherd's", "Farriers'", "Wheelwrights'", "Weavers'", "Bargemen's",
    "Archers'", "Huntsman's", "Saddlers'",
];

/// The words a business's trade goes by in its name ("Smithy", "Forge"), by catalog key: its
/// `trade` and others.
pub fn trades(f: &Func) -> &[&'static str] {
    match f.key {
        "inn" => &["Inn", "Lodge", "Hostelry", "Coaching Inn"],
        "tavern" => &["Tavern", "Taproom", "Alehouse"],
        "general_store" => &["Goods", "Provisions", "Trading Post", "Chandlery"],
        "apothecary" => &["Remedies", "Apothecary", "Herbs", "Tinctures", "Salves", "Physic"],
        "tailor" => &["Tailoring", "Clothiers", "Drapery", "Needlework", "Garments"],
        "bakery" => &["Bakery", "Bakehouse", "Ovens", "Breads"],
        "butcher" => &["Butchery", "Meats", "Shambles"],
        "blacksmith" => &["Smithy", "Forge", "Ironworks", "Anvil"],
        "leatherworker" => &["Tannery", "Leathers", "Tanner's Yard"],
        "cooper" => &["Cooperage", "Casks", "Cooper's Yard"],
        "bowyer" => &["Bows", "Fletchery", "Arrows"],
        "scribe" => &["Scriptorium", "Scribes"],
        "carpenter" => &["Joinery", "Woodworks", "Carpentry"],
        "cobbler" => &["Boots", "Shoes", "Cobblery"],
        "potter" => &["Pottery", "Kilns", "Crockery"],
        "library" => &["Library", "Archive", "Scriptorium"],
        "bank" => &["Counting House", "Exchange", "Treasury", "Bank"],
        "town_hall" => &["Hall", "Moot Hall", "Town House"],
        "guard_post" => &["Watch House", "Guardhouse", "Watch"],
        _ => std::slice::from_ref(&f.trade),
    }
}

/// What hangs over a shop's door ("The Golden Needle"), by catalog key.
pub fn shop_signs(key: &str) -> &'static [&'static str] {
    match key {
        "apothecary" => &["Mortar", "Phial", "Sprig", "Leech"],
        "tailor" => &["Needle", "Thimble", "Spool", "Thread"],
        "general_store" => &["Sack", "Barrel", "Basket", "Lamp"],
        "blacksmith" => &["Anvil", "Nail", "Bellows", "Tongs"],
        "bakery" => &["Loaf", "Oven", "Sheaf", "Bun"],
        "cobbler" => &["Boot", "Last", "Buckle"],
        "jeweler" => &["Ring", "Gem", "Locket"],
        "bookseller" => &["Quill", "Book", "Inkpot"],
        "pawnbroker" => &["Purse", "Strongbox"],
        "curiosities" => &["Imp", "Cat", "Box"],
        "alchemist" => &["Alembic", "Retort", "Phial"],
        "cooper" => &["Hoop", "Cask"],
        "potter" => &["Jug", "Wheel"],
        _ => &[],
    }
}
