//! Room programs per level: arches, level programs, battlements, terraces, corner towers, big rooms.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Arch {
    House,
    Tavern,
    Inn,
    Shop,
    Forge,
    Warehouse,
    Temple,
    Hall,
    Mansion,
    Keep,
    Arena,
    Tower,
    /// Breweries, wineries, distilleries: a brewhouse and a taproom.
    Brewery,
    /// A gaol: cell blocks on every level.
    Prison,
}

pub(super) fn arch_of(b: &town::Building) -> Arch {
    // (R5 towns: a steppe town's round felt hall laid out as a yurt is, `felt`.)
    if town::roof::felt_built(b).is_some() && round_ish(b) {
        return Arch::House;
    }
    let key = func_key(b);
    match key {
        Some("inn" | "caravanserai") => Arch::Inn,
        Some("tavern" | "alehouse" | "fine_dining" | "gambling_den" | "smugglers_den") => Arch::Tavern,
        Some("blacksmith" | "armorer" | "weaponsmith" | "glassblower" | "potter" | "tinker") => Arch::Forge,
        Some("brewery" | "winery" | "distillery") => Arch::Brewery,
        // A tower mill: round floors up a round tower, its own on each (`program`), under a cap.
        Some("windmill") if round_ish(b) => Arch::Tower,
        // R5 M20: an islet's watchtower: a stair up a round tower to its top room.
        Some("watchtower") => Arch::Tower,
        // R5 M20: a breakwater's lighthouse: the keeper below, the stair up to the lamp room.
        Some("lighthouse") => Arch::Tower,
        // (Roadmap 7 P8: its keeper's cottage a home of one storey, its oil store a shed.)
        Some("keepers_cottage") => Arch::House,
        Some("oil_store") => Arch::Shop,
        Some("warehouse" | "stables" | "shipwright" | "cartwright" | "mill" | "windmill" | "customs_house" | "carpenter" | "mason" | "cooper" | "black_market") => Arch::Warehouse,
        Some("temple" | "shrine" | "monastery" | "mausoleum" | "undertaker" | "catacombs" | "tomb") => Arch::Temple,
        Some("prison") => Arch::Prison,
        Some("castle" | "barracks" | "gatehouse") => Arch::Keep,
        Some("palace") => Arch::Mansion,
        Some("arena" | "fighting_pit" | "theater") => Arch::Arena,
        Some("observatory" | "arcane_academy" | "mages_guild") if round_ish(b) => Arch::Tower,
        Some(
            "town_hall" | "courthouse" | "guard_post" | "harbor_master" | "merchant_guild" | "craft_guild" | "adventurers_guild" | "mages_guild" | "thieves_guild" | "library"
            | "arcane_academy" | "observatory" | "bank" | "auction_house" | "bathhouse" | "orphanage" | "healer" | "bards_college" | "menagerie",
        ) => Arch::Hall,
        Some(_) => Arch::Shop,
        None => match town::catalog::RESIDENTIAL[b.residential as usize] {
            "noble estate" => Arch::Mansion,
            "wizard's tower" => Arch::Tower,
            _ => Arch::House,
        },
    }
}

/// A round footprint: convex, with many sides (the POI towers are 14-gons), its corners all about
/// as far from its middle (within 15%).
pub(super) fn round_ish(b: &town::Building) -> bool {
    let p = &b.poly;
    let n = p.len();
    if n < 8 {
        return false;
    }
    let turns: Vec<f64> = (0..n).map(|k| geom::cross(sub(p[(k + 1) % n], p[k]), sub(p[(k + 2) % n], p[(k + 1) % n]))).collect();
    if !(turns.iter().all(|&t| t >= 0.0) || turns.iter().all(|&t| t <= 0.0)) {
        return false;
    }
    // (Measured about the corners' mean, from the first corner: world feet are millions, and an
    // area centroid's sums lose the footprint to rounding.)
    let rel: Vec<P> = p.iter().map(|q| sub(*q, p[0])).collect();
    let c = mul(rel.iter().fold([0.0, 0.0], |a, q| add(a, *q)), 1.0 / n as f64);
    let (lo, hi) = rel.iter().map(|q| geom::dist(*q, c)).fold((f64::MAX, 0.0f64), |(a, b), r| (a.min(r), b.max(r)));
    hi > 0.0 && (hi - lo) <= 0.15 * hi
}

pub(super) fn program(arch: Arch, z: i8, top: bool, key: Option<&str>) -> LevelProgram {
    let p = |name: &'static str, rooms: Prog, filler: Option<&'static str>, corridor: bool| LevelProgram { name, rooms: rooms.to_vec(), filler, corridor, cells: false, by_size: false };
    let cells = |name: &'static str, strip: &'static str| LevelProgram { name, rooms: vec![(strip, 1.0)], filler: None, corridor: false, cells: true, by_size: false };
    let sized = |name: &'static str, rooms: Prog, filler: Option<&'static str>| LevelProgram { by_size: true, ..p(name, rooms, filler, false) };
    // (The trades in workshops: a carpenter's, a mason's, a cooper's, a cartwright's.)
    let trade = matches!(key, Some("carpenter" | "mason" | "cooper" | "cartwright"));
    use Arch::*;
    match (arch, z) {
        (Tower, z) if z == ROOF && key == Some("lighthouse") => p("Lamp room", &[("lamp room", 1.0)], None, false),
        (_, z) if z == ROOF => p("Battlements", &[("battlements", 1.0)], None, false),
        (_, -1) => match arch {
            Temple => p("Crypt", &[("crypt", 3.0), ("ossuary", 1.0)], Some("crypt"), false),
            Keep => cells("Dungeon", "guardroom"),
            Prison => cells("Oubliettes", "torture chamber"),
            Arena => p("Undercroft", &[("holding pen", 2.0), ("armory", 1.0), ("beast pen", 2.0)], Some("holding pen"), true),
            Tavern | Inn | Brewery => p("Cellar", &[("keg cellar", 2.0), ("storeroom", 1.0)], None, false),
            Mansion => p("Cellar", &[("wine cellar", 2.0), ("storeroom", 1.0)], Some("storeroom"), false),
            Hall if matches!(key, Some("bank" | "thieves_guild")) => p("Vault", &[("vault", 2.0), ("storeroom", 1.0)], None, false),
            Hall if matches!(key, Some("courthouse" | "guard_post")) => cells("Cells", "guardroom"),
            Tower if key == Some("windmill") => p("Cellar", &[("storeroom", 1.0)], None, false),
            Tower => p("Vault", &[("vault", 1.0)], None, false),
            Warehouse => p("Cellar", &[("storeroom", 3.0)], Some("storeroom"), false),
            _ => p("Cellar", &[("storeroom", 1.0)], None, false),
        },
        // A tower mill, from the ground up: the meal floor (the flour bagged off the stones), the
        // stone floor, the bin floor (the grain, the sack hoist's trap), the dust floor under the cap
        // (the gearing).
        (Tower, 0) if key == Some("windmill") => p("Meal floor", &[("meal floor", 1.0)], None, false),
        (Tower, _) if key == Some("windmill") && top => p("Dust floor", &[("dust floor", 1.0)], None, false),
        (Tower, 1) if key == Some("windmill") => p("Stone floor", &[("stone floor", 1.0)], None, false),
        (Tower, _) if key == Some("windmill") => p("Bin floor", &[("bin floor", 1.0)], None, false),
        // R5 M20: an islet's watchtower: the guard below, stores between, the lookout at the top.
        (Tower, 0) if key == Some("watchtower") => p("Guardroom", &[("guardroom", 1.0)], None, false),
        (Tower, _) if key == Some("watchtower") && top => p("Lookout", &[("guardroom", 1.0)], None, false),
        (Tower, _) if key == Some("watchtower") => p("Store", &[("storeroom", 1.0)], None, false),
        // A lighthouse (Roadmap 7 P8): its spiral stair up from the stair foot through the store
        // floors to the watch room, the lamp room on the roof under the lantern with its gallery
        // round it (`frames::lamp_top`; above). Its keeper lives in his cottage beside it.
        (Tower, 0) if key == Some("lighthouse") => p("Stair foot", &[("storeroom", 1.0)], None, false),
        (Tower, _) if key == Some("lighthouse") && top => p("Watch room", &[("watch room", 1.0)], None, false),
        (Tower, _) if key == Some("lighthouse") => p("Store", &[("wick store", 1.0)], None, false),
        (House, 0) if key == Some("keepers_cottage") => p("Keeper's cottage", &[("keeper's room", 1.0)], None, false),
        (_, 0) if key == Some("oil_store") => p("Oil store", &[("oil store", 1.0)], None, false),
        (Tower, 0) => p("Entry", &[("entry hall", 1.0)], None, false),
        (Tower, _) => {
            let rooms = ["library", "laboratory", "study", "bedchamber"];
            if top {
                p("Observatory", &[("observatory", 1.0)], None, false)
            } else {
                LevelProgram { name: "Floor", rooms: vec![(rooms[(z as usize - 1) % 4], 1.0)], filler: None, corridor: false, cells: false, by_size: false }
            }
        }
        // One storey: a hovel one room with its bed by the hearth, other homes by their size.
        (House, 0) if top && key == Some("hovel") => p("Ground floor", &[("hearth room", 1.0)], None, false),
        // A home on a round footprint (under a round roof: `town::round_off`): a yurt, one round room.
        (House, 0) if top && key == Some("yurt") => p("Yurt", &[("yurt", 1.0)], None, false),
        // (R5 towns: a felt hall's one round room, `felt`.)
        (House, 0) if top && key == Some(felt::ROOMS[0]) => p("Felt hall", &[(felt::ROOMS[0], 1.0)], None, false),
        (House, 0) if top && key == Some(felt::ROOMS[1]) => p("Temple", &[(felt::ROOMS[1], 1.0)], None, false),
        (House, 0) if top && key == Some(felt::ROOMS[2]) => p("Inn", &[(felt::ROOMS[2], 1.0)], None, false),
        (House, 0) if top => sized("Ground floor", &[("main room", 3.0), ("kitchen", 1.5), ("bedroom", 1.5)], Some("bedroom")),
        (House, 0) => p("Ground floor", &[("main room", 3.0), ("kitchen", 1.5)], Some("bedroom"), false),
        (House, _) => p("Upper floor", &[("bedroom", 2.0), ("bedroom", 1.5)], Some("bedroom"), false),
        (Tavern, 0) => p("Taproom", &[("common room", 5.0), ("kitchen", 1.5), ("storeroom", 1.0)], None, false),
        (Inn, 0) => p("Taproom", &[("common room", 5.0), ("kitchen", 1.5), ("storeroom", 1.0)], None, false),
        (Inn, _) => p("Guest rooms", &[("guest room", 1.0), ("guest room", 1.0)], Some("guest room"), true),
        (Tavern, _) => p("Upstairs", &[("owner's quarters", 2.0), ("storeroom", 1.0)], Some("guest room"), false),
        // One storey: a bedroom behind the work room.
        (Shop, 0) if top => sized("Shop floor", &[("shop", 3.0), ("workshop", 2.0), ("bedroom", 1.5), ("storeroom", 1.0)], None),
        (Shop, 0) => p("Shop floor", &[("shop", 3.0), ("workshop", 2.0), ("storeroom", 1.0)], None, false),
        (Shop, _) => p("Living quarters", &[("living room", 1.5), ("bedroom", 1.5)], Some("bedroom"), false),
        (Forge, 0) if top => sized("Forge", &[("forge", 4.0), ("bedroom", 1.5), ("storeroom", 1.0)], None),
        (Forge, 0) => p("Forge", &[("forge", 4.0), ("storeroom", 1.0)], None, false),
        (Forge, _) => p("Living quarters", &[("living room", 1.5), ("bedroom", 1.5)], Some("bedroom"), false),
        (Brewery, 0) => p("Brewhouse", &[("taproom", 3.0), ("brewhouse", 4.0), ("storeroom", 1.0)], None, false),
        (Brewery, _) => p("Loft", &[("loft storage", 3.0), ("office", 1.0)], Some("loft storage"), false),
        (Warehouse, 0) if key == Some("stables") => p("Stables", &[("stables", 6.0), ("tack room", 1.0)], None, false),
        (Warehouse, 0) if top && trade => sized("Floor", &[("warehouse floor", 6.0), ("bedroom", 1.5), ("office", 1.0)], None),
        (Warehouse, 0) => p("Floor", &[("warehouse floor", 6.0), ("office", 1.0)], None, false),
        (Warehouse, _) if trade => p("Living quarters", &[("living room", 1.5), ("bedroom", 1.5)], Some("bedroom"), false),
        (Warehouse, _) => p("Loft", &[("loft storage", 4.0), ("office", 1.0)], Some("loft storage"), false),
        (Temple, 0) if key == Some("shrine") => p("Shrine", &[("shrine", 1.0)], None, false),
        // R5 towns: a cemetery's tomb (`town::cemetery`), one chamber.
        (Temple, 0) if key == Some("tomb") => p("Tomb", &[("tomb chamber", 1.0)], None, false),
        (Temple, 0) => p("Sanctuary", &[("nave", 5.0), ("sanctuary", 2.0), ("vestry", 1.0)], None, false),
        (Temple, _) => p("Bell loft", &[("bell loft", 2.0), ("monk's cell", 1.0)], Some("monk's cell"), true),
        (Hall, 0) => {
            let main: &'static str = match key {
                Some("library") => "reading room",
                Some("bank") => "counting house",
                Some("bathhouse") => "baths",
                Some("healer") => "infirmary",
                Some("courthouse") => "courtroom",
                _ => "great hall",
            };
            LevelProgram { name: "Ground floor", rooms: vec![(main, 4.0), ("office", 1.0), ("office", 1.0)], filler: Some("office"), corridor: false, cells: false, by_size: false }
        }
        (Hall, _) => {
            let filler: &'static str = match key {
                Some("library" | "arcane_academy" | "bards_college") => "library",
                Some("orphanage" | "adventurers_guild") => "dormitory",
                _ => "office",
            };
            LevelProgram { name: "Upper floor", rooms: vec![("meeting room", 2.0), ("archive", 1.0), (filler, 1.0)], filler: Some(filler), corridor: true, cells: false, by_size: false }
        }
        (Mansion, 0) => p("Ground floor", &[("great hall", 3.0), ("dining room", 2.0), ("foyer", 1.5), ("kitchen", 1.5), ("parlor", 1.5)], Some("parlor"), false),
        (Mansion, _) => p("Upper floor", &[("master bedroom", 2.0), ("study", 1.2), ("bedroom", 1.5), ("gallery", 1.5)], Some("bedroom"), true),
        (Prison, 0) => cells("Gaol", "guardroom"),
        (Prison, 1) => cells("Cell block", "warden's office"),
        (Prison, _) => cells("Cell block", "guardroom"),
        (Keep, 0) => {
            if key == Some("barracks") {
                p("Hall", &[("mess hall", 5.0), ("guardroom", 1.5), ("kitchen", 1.5), ("pantry", 1.0)], Some("guardroom"), false)
            } else {
                p("Hall", &[("great hall", 5.0), ("guardroom", 1.5), ("kitchen", 1.5), ("pantry", 1.0), ("buttery", 1.0)], Some("guardroom"), false)
            }
        }
        (Keep, _) if key == Some("barracks") => p("Upper floor", &[("barrack room", 2.0), ("armory", 1.0), ("officers' quarters", 1.0)], Some("barrack room"), true),
        // Each floor of a keep has its own rooms: one chapel, one lord's chamber.
        (Keep, 1) => p("Lord's floor", &[("lord's chamber", 2.0), ("solar", 1.5), ("chapel", 1.0)], Some("chamber"), true),
        (Keep, 2) => p("Upper floor", &[("armory", 1.5), ("map room", 1.5), ("treasury", 1.0)], Some("guest chamber"), true),
        (Keep, 3) => p("Upper floor", &[("library", 1.5), ("study", 1.0), ("guest chamber", 1.5)], Some("guest chamber"), true),
        (Keep, _) => p("Upper floor", &[("servants' quarters", 1.5), ("wardrobe", 1.0), ("storeroom", 1.0)], Some("servants' quarters"), true),
        (Arena, 0) => p("Arena", &[("arena floor", 1.0)], None, false),
        (Arena, _) => p("Gallery", &[("gallery", 1.0)], None, false),
    }
}

/// Corner towers of a keep's grid: their side in squares (0 when the keep is too small).
pub(super) fn keep_tower_squares(nx: usize, ny: usize) -> usize {
    if nx.min(ny) >= 8 { 3 } else { 0 }
}

/// Keeps and towers have a walkable roof: battlements instead of a pitched roof. For a keep,
/// the side of its corner towers (ft; 0 for none), as the battlemap draws them.
pub fn battlements(b: &town::Building) -> Option<f64> {
    // A roof chosen by hand: battlements on anything (a keep keeps its corner towers).
    match b.roof {
        Some(town::RoofStyle::Battlements) if arch_of(b) != Arch::Keep => return Some(0.0),
        Some(town::RoofStyle::Hip | town::RoofStyle::Cone) => return None,
        _ => {}
    }
    match arch_of(b) {
        // (A keep built of pieces: its towers are pieces of its own, not the grid's corners.)
        Arch::Keep if !b.parts.is_empty() || !corner_towers(b) => Some(0.0),
        Arch::Keep => {
            let o = geom::obb(&b.poly);
            let (nx, ny) = (crate::core::round(o.long / SQUARE_FT) as usize, crate::core::round(o.short / SQUARE_FT) as usize);
            Some(keep_tower_squares(nx, ny) as f64 * SQUARE_FT)
        }
        // (A tower mill under its cap.)
        Arch::Tower if b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "windmill") => None,
        Arch::Tower => Some(0.0),
        _ => None,
    }
}

/// A flat roof behind its parapet (`town::roof::look`'s shape flat, picked or set by hand), or the
/// flat roof round a dome's drum on a building of one piece: a terrace one walks on, as battlements
/// are. Its domes (world centre, the drum's radius ft) as the battlemap draws them (`domeRoof` in
/// `battlePrep.ts`: one over the middle, two along a long footprint); None for any other roof.
pub fn terrace(l: &Layout, b: &town::Building) -> Option<Vec<(P, f64)>> {
    use town::roof::Shape;
    if b.structure != Structure::Roofed || battlements(b).is_some() || b.roof == Some(town::RoofStyle::Cone) || b.poly.len() < 3 {
        return None;
    }
    let look = town::roof::look(l, b);
    match look.shape {
        // (R5 towns, 114: a steppe home's low felt or turf roof is no terrace.)
        Shape::Flat if l.culture % 5 == 4 && matches!(look.material, town::roof::Material::Hide | town::roof::Material::Turf) => None,
        Shape::Flat => Some(Vec::new()),
        // (The battlemap's domes, `town::roof::domes`; none where the roof is too small for one.)
        Shape::Dome if b.parts.is_empty() => Some(town::roof::domes(&b.poly)),
        _ => None,
    }
}

/// A keep with no pieces of its own has a tower on each corner of its grid, but a castle's keep
/// drawn round its towers by hand (its pieces gone: a footprint far from its grid's rectangle),
/// whose corners those don't fit.
pub(super) fn corner_towers(b: &town::Building) -> bool {
    let castle = b.func.is_some_and(|f| town::catalog::CATALOG[f as usize].key == "castle");
    let o = geom::obb(&b.poly);
    !castle || geom::area(&b.poly).abs() >= 0.95 * o.long * o.short
}

/// The level index of an open roof (battlements) on keeps and towers.
pub(super) const ROOF: i8 = 100;

/// The level index of a keep's tower tops (above its battlements).
pub(super) const TOWER_TOP: i8 = 101;

/// Rooms that stay big (they are the point of the building).
pub(super) fn big_room(kind: &str) -> bool {
    matches!(
        kind,
        "common room" | "taproom" | "brewhouse" | "battlements" | "roof terrace" | "warehouse floor" | "loft storage" | "nave" | "great hall" | "mess hall" | "forge" | "reading room" | "counting house" | "baths" | "infirmary" | "courtroom" | "arena floor" | "stands" | "crypt" | "dungeon"
    )
}

// ---------------------------------------------------------------------------------------
// Furniture.
