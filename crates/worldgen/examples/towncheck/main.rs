//! QA: castles, play reach, interiors and the street editor, swept over whole worlds. Counts what
//! should never happen, with the worst places (world ft) to go and look (`TOP=n` lists more):
//! - `castles`: every town's castle as generated. Town gates opening into a bailey; streets and
//!   roads inside a bailey (the castle's own way to its gate excepted) or across a curtain; dry
//!   stretches of a bailey's ring with no wall, tower or gate on them; curtains doubling the town
//!   wall (3 to 30 ft from it along 30 ft or more) and 30 to 45 ft strips between them with no
//!   street in them; towers overlapping, castle towers within 12 ft of town towers, castle
//!   buildings on towers, walls, streets or other buildings; gate ways blocked or laid through
//!   buildings; the outer gate's distance to the nearest street or way; the outer bailey's width
//!   and the inner curtain's distance to the outer gate; castles with no keep; mottes with
//!   objects on them (from the battlemap); and a few more (spikes, keep tops left open, ids twice).
//!   Every town's quays with no water beyond them, walled towns' rings with no wall on dry ground,
//!   buildings over a river's channel and built blocks centred on one.
//! - `reach`: each castle's battlemap round it, flooded as play mode blocks movement (buildings,
//!   walls, towers, blocking objects, a hedge every square it is drawn across) from the window's
//!   edge and its streets, so from outside the outer gate: castle buildings cut off, outer gates
//!   not reached, bailey squares sealed (and the largest pocket); the same for every park.
//! - `slums`: out-of-town slum patches as generated, their buildings, and those with none (`SLUMS_LIST=1`: each).
//! - `yurts`: round homes' doors (`town::round_off`): how many, the share facing their lane (within 70 ft)
//!   or the centre within 30 degrees, the share still facing south, interior door squares more than 1.5
//!   square from the ring's door side, and up to 8 listed per world with seed, layout, id, x,y and angle.
//!   `cargo run --profile fast -p worldgen --example towncheck -- yurts [seeds]` (default seeds 1-8; `TOP=8`); a door over 1.5 squares off is the interior finding the ring's side shut.
//! - `interiors`: vital's `check_interior` over every enterable building and wall or gate tower,
//!   failures grouped by function; and the corridor rule on its own (no room more than two rooms
//!   from a corridor), with the stricter old reading (a hall or corridor only, not the front room).
//!   `--keeps`: keeps only.
//! - `hub`: castle keeps and barracks of 150 squares or more, laid out by zones: floors the zones
//!   fell back on, the hall's floor, the front door's room, the main stair at the dais end, the great
//!   chamber and solar by it (a solar's middle nearer the stair than the hall's low end), the
//!   minstrels' gallery reached along passages, windows and furniture on open tops, thrones in mess
//!   halls, room sizes and proportions (the box and how much of its floor a room fills), storeys
//!   alike, tower rooms, landings, the screens passage, the service and officers' ends, rooms a square
//!   wide, a shell keep's round yard (`hub_rules`).
//! - `halls`: great halls furnished as wholes (`interior::hall_plans`: castles', town and guild
//!   halls', noble estates', mess halls): the dais at the far end on the axis (a throne dais in a
//!   castle, a low dais with a chair of office elsewhere, none in a mess hall), the runner on the
//!   axis, the tables, benches, pillars, banners, tapestries, braziers and high tables mirrored
//!   across it, nothing shut off, an open hall's musicians on its gallery; no throne outside a
//!   castle or a palace; every banner in a castle's buildings of one house (`hall_rules`).
//! - `palaces`: palaces and noble estates laid out by zones (`interior::hub`: the capital's palace
//!   round its court, other cities' older palaces round their great halls, hall houses and manors):
//!   floors the zones fell back on, vital's `check_interior` (every room reached, the corridor rule),
//!   the rooms of each one's programme, rooms of sound proportions (no longer than 2.5:1 but a long
//!   gallery, filling 60% of their box, a hall, a court or a chapel the only rooms over 60 squares,
//!   none a square wide); the capital's palace laid out as one, its front door on its axis, its front
//!   range (the ground floor's) and every floor above mirrored across its axis, its state rooms one into
//!   the next from the landing round to the throne room both ways (the audience chamber's side and the
//!   ballroom's) through doors mirrored across the axis, its throne dais on the axis, its forecourt and
//!   its paved way off the water (`palace_rules`); a chapel's altar at the end away from its doors.
//! - `temples`: temples, shrines and monasteries (`interior::hub`'s `temple` and `monastery`,
//!   `town::temple`): floors fallen back on; a big temple's footprint a cross ending in a round apse,
//!   a small one's a hall ending in one; each programme's rooms (a big one's nave, sanctuary in its
//!   apse, two side chapels, a vestry; a small one's single hall), the front door into the nave, the
//!   altar rails; the god's statue on the axis at the far end, the altar before it, candle stands
//!   mirrored; a big nave's arcades, the crossing's piers, the carved screen with its gate clear; pews
//!   mirrored facing the altar; the font; side altars; the god's look on every piece; a shrine of one
//!   room; a monastery round its cloister on its merged footprint: its church along a long side, the
//!   chapter house (an octagon), refectory, dorter, scriptorium and kitchen round the walk, the garth open
//!   on the map with nothing scattered in it, its well and walks, the arcade, the refectory furnished
//!   (`temple_rules`). Temples too small for a hall with an apse and monasteries with no cloister listed.
//! - `editor`: castles set in the street editor (`wards::change`) on the patches nearest the
//!   town wall (up to 3 a town, `PLACES=n` a world, default 20), with the walls on and off, each
//!   run through the castle checks, and its keep and barracks through the `hub` and `halls` rules;
//!   every non-castle building's id, outline, ward, function, kind, name and floors the same with
//!   the walls off as on; and every town castle's town with its walls taken down. Every monastery's
//!   town edited round it (its patch's lots made small, rerolled; the walls taken down, else a far
//!   patch rerolled): no building on another's ground (`Layout::grounds`), as generated or after the
//!   edit, and a front door on every building within 150 ft of it.
//!   Then districts drawn by hand (`district_edits`; also `districts` alone): a patch into a
//!   neighbour's district, a far one, two into a new named one, a small district emptied, walls
//!   as generated and toggled; renames and notes kept, carried or reported when a patch is laid
//!   out anew.
//! - `inns`: inns and caravanserais of `hub::inn::LEAST` squares or more laid out by zones: floors
//!   fallen back on, `check_interior` (the drinking rules: the bar and tables in the taproom, the front
//!   door into it, the stairs in it; every room reached), the programme (taproom, snug, private dining
//!   room, kitchen, scullery, storeroom; above, best chamber, guest rooms, dormitory, innkeeper's
//!   quarters: on one storey reasoned absent; the snug, the scullery and the private dining room on a
//!   service range too narrow for them listed apart), rooms no longer than 2.5:1, rooms per kind;
//!   those under the threshold and those over it no frame fits listed (`PLAN=b:L:id` prints a plan).
//!   Coaching inns (`town::palace::is_coaching`) round their yard: the yard reached from the street
//!   through an arch 2 squares wide, every stables onto it, every room upstairs off the gallery or a
//!   corridor, no other building on their ground, the front door and the arch within 20 ft of a
//!   street; those not round a yard, and every yard listed with where it is (`editor` edits each
//!   one's patch as a monastery's: still round a yard after).
//! - `walls` (not in `all`): walls by culture group (item 104, `town::wallstyle`): walled places and
//!   their wall, castle and corner towers by shape; square towers where the style has none; towers whose
//!   battlemap squares don't follow their shape (ring blocked, floor open, door reached); up to 3 places
//!   a group listed with cameras (`fx, fy, zoom`) on the whole wall and on a tower by a gate.
//! - `cemeteries` (not in `all`; R5 towns): cemeteries per tier and culture group, what they hold,
//!   the rules (props or tombs on a walk, over the boundary or outside the plot, tomb doors not
//!   reached from the way in, kurgans overlapping, a lychgate off the way in) and cameras; a park set
//!   to a cemetery by hand, walls on and off.
//! - `hash` (not in `all`): a hash per settlement layout (`L:`), per building of it (`B:`, with what it
//!   is) and per building or tower interior (`b:`/`t:`), marked `C` in a settlement with a castle, one
//!   per line on stdout (everything else goes to stderr): run it on two trees and diff to see what
//!   changed.
//!
//! - `under` (not in `all`): every underground site of every layout (`u:` ways down, up to 4 sewer sections a layout,
//!   keep dungeons `k:`): a hash per site on stdout (`U <seed> <id> <hash> <theme> <levels>`; everything else goes
//!   with the other rows) and totals of sites, levels, rooms, doors, secret doors, loops (doors beyond a spanning
//!   tree of a level's rooms), shortcut loops (each saving 4+ hops) and the hops they save, and `u:` sites breaking `under::design::check`;
//!   portcullises and the water squares generators laid. THEME=temple,sunken_temple keeps only sites of those themes (`keep`, `undercroft`: keep dungeons, sewer sections). TERRAIN=1 also lays the terrain tools
//!   (`under::chasm`, `under::water`) on a copy of each site and counts what they laid and what they left unreached.
//!
//! - `castlepieces` (not in `all`): a castle drawn by hand, its keep and yard buildings changed (`Created::pieces`),
//!   an earlier site deleted and taken out of the list, the castle in ruins; keeps of generated castles changed.
//!
//! - `same` (not in `all`): every settlement layout's hash (`{:?}` of the whole layout, `L:`), and every town's
//!   with one scripted street-editor edit (`E:`: three corners moved, a patch's ward, lots and reroll set, the
//!   walls toggled), checked against a baseline (`SAME_DIR`, default `target/towncheck-same`, a file per
//!   world): the first run, or `SAME=write`, writes it. Layouts that differ are listed (`TOP=n`). Refactors
//!   that must not change output run it before and after (Roadmap 6 P2: seeds 1-24 and the MCP fixture).
//!   `SAME_EDITS=n`: towns edited per world (default 8).
//!
//! - `fills` (not in `all`; default seeds 1-8; Roadmap 6 P2): hand-laid blocks filled with lots (`town::qa_fill`,
//!   `Block::Region`). Each town's blocks (`Layout::blocks`, the ward of a building in it) made concave: a corner cut
//!   out with its new sides fronting a street, the same with them not fronting, a courtyard hole, two neighbours
//!   joined across their street. Counts: buildings outside their region or over a hole, overlapping buildings,
//!   buildings with no front (an edge on a street's or alley's line, up to the 16 ft a house stands back), fills
//!   not the same twice or with their rings rotated; built area per area against the convex blocks filled the same
//!   way. Up to 6 SVGs to `SVG_DIR` (default `target/towncheck-fills`), and those whose listing holds `FILL_SVG`.
//! - `rows` (not in `all`; default seeds 1-8): both sides of each town's streets (`Layout::roads`
//!   but class 3, the alleys and paved strips) lined with a row of lots (`town::qa_rows`, P3's path fill)
//!   round the town's buildings, walls, other streets and their bands. Counts lots off their band,
//!   overlapping (on one side, at bends; and another row's), over the region avoided, with no front on
//!   their street, on the wrong side, not convex, under `build_block`'s rules, and rows not repeating
//!   (the first two towns, twice); lots per 100 ft of a street's side. `ROWS=n`: towns a world (12);
//!   `SVG_DIR`: a picture a world, up to 6.
//!
//! - `streetsin` (not in `all`; Roadmap 6 R7): walled towns' main streets from their gates: length within 30 ft of
//!   the town wall's inner face past the first 40 ft (and the part running along it), the straight run from the
//!   gate before the first piece turning over 20 degrees off the wall's inward normal, the first 100 ft's heading
//!   off that normal (median, over 20 and 30 degrees); a summary line over every
//!   world with the worst 5 towns (seeds 1-24 to compare); `STREETS_OFF=1`: every street over 20 degrees off square,
//!   to stderr. `STREETS_DUMP="L<i> <name>"` (or `all`): that town's streets
//!   piece by piece (heading against the gate's inward normal, distance to the wall).
//!
//! Usage: cargo run --release -p worldgen --example towncheck -- [streetsin|same|fills|rows|castlepieces|castles|reach|interiors|hub|halls|palaces|temples|editor|under|hash|all]... [seed|world.json...] [--keeps]
//! Default: `all` (every mode but `hash`), seeds 1 2 3: 30 to 50 s a seed under `--profile fast`, half of
//! it `interiors` (`--keeps` takes a second).

use prelude::*;

pub type P = [f64; 2];

/// `hash`: only the hashes go to stdout.
pub static QUIET: AtomicBool = AtomicBool::new(false);
macro_rules! say {
    ($($t:tt)*) => {
        if QUIET.load(Ordering::Relaxed) { eprintln!($($t)*) } else { println!($($t)*) }
    };
}

pub fn top() -> usize {
    std::env::var("TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(4)
}

#[path = "../common/mod.rs"]
mod common;
mod prelude;
mod shared;
mod castles;
mod interiors;
mod temples;
mod villages;
mod walls;
mod cemeteries;
mod handwalls;
mod steppe;
mod planning;
mod wedges;

fn main() {
    const MODES: [&str; 34] = ["wedges", "planning", "streetsin", "slums", "hand", "same", "fills", "harbours", "horseparks", "steppe", "castlepieces", "wallgates", "handwalls", "districts", "places", "walls", "yurts", "villages", "castles", "reach", "interiors", "hub", "halls", "palaces", "temples", "civic", "inns", "editor", "under", "hash", "piers", "cemeteries", "all", "rows"];
    let args = common::args();
    let keeps = args.pos.iter().any(|a| a == "--keeps");
    let mut modes: Vec<&str> = args.pos.iter().filter_map(|a| MODES.iter().find(|m| **m == a.as_str()).copied()).collect();
    for a in args.pos.iter().filter(|a| !MODES.contains(&a.as_str()) && *a != "--keeps" && !common::is_world(a)) {
        eprintln!("{a}: not a mode, a seed or a world file, ignored");
    }
    if modes.is_empty() || modes.contains(&"all") {
        modes.retain(|m| *m != "all");
        for m in ["castles", "reach", "interiors", "hub", "halls", "palaces", "temples", "civic", "inns", "editor"] {
            if !modes.contains(&m) {
                modes.push(m);
            }
        }
    }
    QUIET.store(modes.contains(&"hash"), Ordering::Relaxed);
    let worlds: Vec<String> = args.pos.iter().filter(|a| common::is_world(a)).cloned().collect();
    let worlds = if worlds.is_empty() && (modes == ["yurts"] || modes == ["slums"] || modes == ["fills"] || modes == ["rows"] || modes == ["hand"] || modes == ["steppe"] || modes == ["horseparks"] || modes == ["harbours"] || modes == ["planning"] || modes == ["wedges"]) {
        (1..=8).map(|s| s.to_string()).collect()
    } else if worlds.is_empty() {
        vec!["1".into(), "2".into(), "3".into()]
    } else {
        worlds
    };
    let mut all: BTreeMap<&str, Report> = BTreeMap::new();
    let start = Instant::now();
    for arg in &worlds {
        let t = Instant::now();
        let mut world = common::world(arg);
        // (Dev: `TC_PLANNING=<0-100>` sets every town's planning, to tell what planned streets do.)
        if let Some(v) = std::env::var("TC_PLANNING").ok().and_then(|v| v.parse::<u8>().ok()) {
            let t0 = T0::generate(&world);
            let mut file = world.file.clone();
            for li in (0..t0.settlements.len()).filter(|&li| t0.settlements[li].tier >= worldgen::t0::settle::Tier::Town) {
                file.edits.planning.insert(li.to_string(), v);
            }
            world = World::new(file).unwrap();
        }
        let mut ctx = Ctx { ex: Executor::new(world), chunks: HashMap::new() };
        let label = common::label(arg);
        say!("{label}: {} settlements, T0 {:.1} s", ctx.ex.t0.settlements.len(), t.elapsed().as_secs_f64());
        for &mode in &modes {
            let t = Instant::now();
            let mut r = Report::default();
            match mode {
                "castles" => castles(&mut ctx, &mut r),
                "reach" => reach(&mut ctx, &mut r),
                "interiors" => interiors(&mut ctx, keeps, &mut r),
                "hub" => hub(&mut ctx, &mut r),
                "halls" => halls(&mut ctx, &mut r),
                "palaces" => palaces(&mut ctx, &mut r),
                "temples" => temples(&mut ctx, &mut r),
                "civic" => civic(&mut ctx, &mut r),
                "inns" => inns(&mut ctx, &mut r),
                "piers" => piers(&mut ctx, &mut r),
                "harbours" => harbours(&mut ctx, &mut r),
                "yurts" => yurts(&mut ctx, &mut r),
                "villages" => villages(&mut ctx, &mut r),
                "walls" => walls(&mut ctx, &mut r),
                "steppe" => steppe(&mut ctx, &mut r),
                "horseparks" => horseparks(&mut ctx, &mut r),
                "cemeteries" => cemeteries(&mut ctx, &mut r),
                "handwalls" => handwalls(&mut ctx, &mut r),
                "wallgates" => wallgates(&mut ctx, &mut r),
                "streetsin" => streets_in(&mut ctx, &label, &mut r),
                "planning" => planning::planning(&mut ctx, &label, &mut r),
                "wedges" => wedges::wedges(&ctx.ex.world, &ctx.ex.t0, &label, &mut r),
                "castlepieces" => castle_pieces(&mut ctx, &mut r),
                "editor" => editor(&mut ctx, &mut r),
                "districts" => district_edits(&ctx.ex.world, &ctx.ex.t0, &mut r),
                "places" => places(&ctx.ex.world, &ctx.ex.t0, &mut r),
                "under" => under(&mut ctx, &mut r, &label.replace(' ', "")),
                "same" => same(&mut ctx, &label.replace(' ', ""), &mut r),
                "fills" => fills(&mut ctx, &label.replace(' ', ""), &mut r),
                "rows" => rows(&ctx.ex.world, &ctx.ex.t0, &label.replace(' ', ""), &mut r),
                "slums" => slums(&ctx.ex.world, &ctx.ex.t0, &label.replace(' ', ""), &mut r),
                "hand" => hand_towns(&ctx.ex.world, &ctx.ex.t0, &label.replace(' ', ""), &mut r),
                _ => hashes(&mut ctx, &label.replace(' ', ""), &mut r),
            }
            say!("  {mode} ({:.1} s)", t.elapsed().as_secs_f64());
            r.print();
            r.merge_into(all.entry(mode).or_default(), &label);
        }
    }
    if worlds.len() > 1 {
        say!("all {} worlds ({:.1} s)", worlds.len(), start.elapsed().as_secs_f64());
        for &mode in &modes {
            if mode != "hash" {
                say!("  {mode}");
                all.get_mut(mode).unwrap().print();
            }
        }
    }
    if let Some(a) = all.get("streetsin") {
        streets_in_summary(a);
    }
}
