//! MCP tools for underground sites and building interiors designed by hand (`Edits::designs`,
//! `under::design`, `interior::design`): read one as a text plan, write one back (checked by the
//! same rules every generated one keeps), or go back to the generated one.

use serde_json::{Value, json};
use worldgen::core::hash::fnv64;
use worldgen::interior::design as building;
use worldgen::under::design::{self, SiteDesign};

use crate::Shared;
use crate::tools::{arg_str, schema, text};

const BUILDING_FORMAT: &str = "A building's plan (b:<layout>:<id>, any roofed building): a header 'building · <w> x <h> squares · <n> levels · stairs x,y wxh · cellars <n>' (the stair block, the same squares on every level it reaches: move or resize it here; cellars: how many levels below ground, 0 to 3, each new one a storeroom under the whole building that the stairs reach, the deepest filled in first; a 'level -2' block digs down to it too), then its levels by storey, 'level -2: Lower cellar', 'level -1: Cellar', 'level 0: Ground floor', 'level 1: …' (an open roof and a keep's tower tops are the storeys above), each with 'rooms:', 'grid:' ('.' is outside the walls), 'doors: x,y e|s|w|n [secret|portcullis|front|back|arch]' (on the edge of square x,y east, south, west or north; portcullis: an iron grate between two rooms, shut it stops a step but not the eye; one front door on the ground floor, onto the street side; back doors to the outside on the ground floor only; arch: an open archway with no leaves, a coaching inn's carriage arch from the street to its yard; 'auto' keeps the doors to the outside and adds inner doors wherever a room is shut off) and 'items: <furniture> x,y [wxh] [back n|e|s|w] [variant n]' (back: the side it stands against, for furniture that faces one side such as shelves, beds, hearths, altars, tapestries, left out to let it find its wall; variant: one of its looks, 0-3 (a throne_dais 0-5: one throne, a pair, a throne and a lesser chair, on stone steps with gilt thrones, then 3-5 on a timber platform; a wall_banner or standing_banner 0-7: a house's colours, the same number on both; left out, a banner flies its building's own house; a low_dais 0 stone, 1 timber), left out to pick by position; items stand apart, but a low_dais is a platform: one other piece may stand on each of its squares (a chair_of_office, a high_table); +items:/-items: add to or take from the list (see the site format) instead of replacing it; furniture kinds: bed, table, chair, bar, hearth, shelf, bookcase, chest, barrel, crate, workbench, spiral_stair, …; great halls and ballrooms: throne_dais, low_dais, chair_of_office, high_table, carpet_runner, tapestry, wall_banner, standing_banner, font, cloister_well, dance_floor, harpsichord, musicians_corner, refreshment_table; guild and civic: council_table, counting_desk, strongbox, job_board, guild_arms (variant 0-4: the merchants', the craftsmen's, the adventurers', the mages', the town's), stable_stall; roof terraces: roof_dome (a dome over the terrace, blocks), water_jar, planter, washing_line; indoor props: sacks, rubble, timber, bones, skeleton, skulls, urn, coffin, effigy, brazier, candles, sconce, lantern, banner, chains, cobweb, bedroll, tools, powder, hoard, offering, glyph, dais, well, fountain, iron_maiden, stocks, rat_nest, trap, collapsed_wall (tumbled blocks from a broken wall: difficult terrain, half cover, seen across; 1x1 to 2x2, its back the broken wall); an uploaded sprite as s:<asset id>, with the rules list_sprites gives it). The levels above ground follow the building's storeys: they can't be added or taken away here (change its storeys with update_building, which fits a saved design to them: the floors it keeps as designed, new ones as generated). A building with a trapdoor to the sewers or stairs down to a keep's deep dungeons keeps at least one cellar: those ways are on its deepest. Names, notes and NPCs on its levels follow them when cellars are dug or filled in. Walls are where rooms meet: a room split in two by a line of squares of another room, or two rooms made one, changes the walls. A building's design is set aside (the generated interior stands) when its footprint or storeys change.";

pub const FORMAT: &str = "The plan, levels numbered from 1 at the top: 'level <n>: <name>', then 'rooms: a=<kind>; b=<kind>+5 (round) \"<name>\"' (symbol = room; +5 a floor raised 5 ft; a shape in brackets, drawn only: round, octagon, apse n|e|s|w, chamfered, cavern (underground), its squares those whose middles lie inside the shape in the box round them, else drawn square (fit_shapes fits them); a name in quotes), 'grid:' and one row per line (one character per 5-ft square, '.' rock, x across from 0, y down from 0), 'doors: x,y e; x,y s secret; x,y e locked 2; x,y s secret locked 5; x,y e portcullis; x,y s portcullis locked 3; x,y e arch' (between square x,y and the one east or south of it; 'portcullis' an iron grate in the doorway: shut it stops a step but not the eye, and in play the DM raises and lowers it with a click; 'arch' an open gap with no door, never shut nor locked (where a cave-in broke through a wall); 'auto' adds doors wherever a room is shut off; 'locked N', N 1-7, is a lock of colour N: 1 red, 2 orange, 3 yellow, 4 green, 5 blue, 6 purple, 7 white; a locked door is a wall until its key is found, and in play the DM unlocks it with one click and opens it with the next), 'items: <kind> x,y [wxh] [back n|e|s|w] [variant 0-3]; …' (the whole list; '+items:' and '-items:' add and remove, see below; back: the side it stands against, for props that face one side such as statues, altars, things hung on a wall, left out to let it find its wall; variant: one of its looks, left out to pick by position; props, an uploaded sprite as s:<asset id>, and the ways: exit on the top level at the entry, down on every level but the deepest, up below each down on the same square). '+items: <pieces>' adds pieces to the level's list and '-items: <pieces>' removes the pieces of that kind at those top-left squares (a size, if written, must match too; one that matches nothing is an error naming it with the nearest of its kind): they leave the rest, where 'items:' alone replaces the whole list, and several lines of the three apply in the order written, e.g. '+items: well 3,0 2x2; -items: barrel 9,4'. The plan from get_site_design shows, under a level that isn't flat, a 'heights:' legend and a 'height grid:' (the floor's height per square, '.' 0 ft, '/' a ramp), a read-out that set_site_design ignores: raise floors with '+5'/'+10' on a room and the items platform (5 ft), balcony (10 ft), scaffold (10 ft) and dais, join them with steps, climb_ladder or rubble_slope (a floor reachable only by climbing a sheer edge of more than 3 ft is refused, naming the square and height). set_site_design answers with what changed (items and doors added and removed, squares and rooms changed, floor heights) and the check's verdict, not the whole plan; dry_run answers the same without saving. A 'collapsed_wall x,y [wxh] [back n|e|s|w]' is tumbled blocks spilling from a broken wall into the room (difficult terrain, half cover, seen across; 1x1 to 2x2, its back the wall it fell from). Keys: 'key x,y variant N' opens the lock of colour N (variant 1-7; the colour is its look; DM-only, never blocks); each lock has exactly one key, reached without crossing that lock (on the lock's level or a level above), and the plan's problems list notes a lock with no key, a second key, a key that opens nothing, and squares only reached after their own key. Terrain, each a full w x h rectangle on bare floor (a square holds one item, or one terrain and one span over it): 'water_shallow' (wades, difficult terrain, never blocks), 'water_deep' (blocks movement, not sight), 'chasm' (blocks movement, not sight), 'rubble_slope' (difficult terrain, never blocks), 'stone_bridge' (a stone bridge's deck of dressed slabs, walked like floor; lay the chasm on either side of it), and the spans 'rope_bridge' and 'stepping_stone' laid over water or a chasm, which make the squares under them walkable; every square must stay reachable, so cross deep water and chasms with a span, e.g. 'chasm 8,4 1x5; rope_bridge 8,4 1x5'.";

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "get_site_design", "title": "Site plan", "description": "An underground site (u:<layout>:<k>; not a city's sewers) or a building's interior (b:<layout>:<id>) as a text plan, with its problems: a JSON summary, then the plan as a second text part (a site nobody designed gives its generated plan, to start from). Change it with set_site_design. summary: true gives only the levels with room counts and names; format: true adds the full plan-format text.", "inputSchema": schema(json!({ "id": { "type": "string", "description": "The site's id (u:<layout>:<k>) or the building's (b:<layout>:<id>)" }, "summary": { "type": "boolean", "default": false, "description": "Levels with room counts and names only, no plan text" }, "format": { "type": "boolean", "default": false, "description": "Also give the full plan-format text" } }), &["id"]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "set_site_design", "title": "Design a site", "description": "Change an underground site or a building's interior from a text plan: read it with get_site_design (format: true gives the full format: item kinds, door flags, rules), change what you want, send it back. Only what the text gives changes: a level block with just 'items:' changes only that level; '+items:' and '-items:' add to or take from a list. Levels number from 1 at the top. Per level: 'level <n>: <name>'; 'rooms: a=<kind>[+5 raised ft][ (round|octagon|apse n|chamfered|cavern)][ \"name\"]; b=...'; 'grid:' one row per line, a letter per 5-ft square, '.' rock; 'doors: x,y e|s [secret|locked 1-7|portcullis|arch]; ...' ('auto' adds doors where a room is shut off); 'items: <kind> x,y [wxh] [back n|e|s|w] [variant n]; ...' (the ways exit, down, up too). Headers: a site's 'site · <n> levels' adds levels below; a building's 'building · <w> x <h> squares · <n> levels · stairs x,y wxh · cellars <n>' with levels 'level -1: Cellar', 'level 0: Ground floor' (storeys above follow the building: update_building). Refused, with the first reasons and a count, when it breaks a rule play needs (a site: one way in at the entry, every way down above a way up, every square reachable; a building: one front door, stairs on every level, every room reached). See MCP.md > Underground sites designed by hand, Building interiors designed by hand.", "inputSchema": schema(json!({
            "id": { "type": "string" },
            "text": { "type": "string", "description": "The plan, or the parts of it to change" },
            "furnish": { "type": "boolean", "default": false, "description": "Fill rooms with no props from their kind's kit" },
            "fit_shapes": { "type": "boolean", "default": false, "description": "Fit rooms given a shape to it; one that can't be fitted without breaking a rule stays square (the reply's fit_shapes says which)" },
            "dry_run": { "type": "boolean", "default": false, "description": "Check it without saving" },
        }), &["id", "text"]) }),
        json!({ "name": "reset_site_design", "title": "Site as generated", "description": "Drop a site's or building's design: it is generated again as it was.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "get_site_design" => get(app, a).await,
        "set_site_design" => set(app, a).await,
        "reset_site_design" => reset(app, a).await,
        _ => return None,
    })
}

/// How many problems a reply lists (and a refusal names): the first few, then a count.
const SHOWN: usize = 5;

/// "first; second; ... (and N more)".
fn few(items: Vec<String>) -> String {
    let more = items.len().saturating_sub(SHOWN);
    let mut s = items.into_iter().take(SHOWN).collect::<Vec<_>>().join("; ");
    if more > 0 {
        s.push_str(&format!(" (and {more} more)"));
    }
    s
}

/// The problems as the replies list them: the first few and a count of the rest.
fn problem_list(problems: &[design::Problem]) -> Value {
    let mut v: Vec<Value> = problems.iter().take(SHOWN * 2).map(|p| json!({ "text": p.text, "at": p.at, "refused": p.blocking })).collect();
    if problems.len() > SHOWN * 2 {
        v.push(json!({ "text": format!("and {} more", problems.len() - SHOWN * 2) }));
    }
    json!(v)
}

/// Levels as the summary option gives them (top first for a site, as the plan numbers them): room counts and names.
fn level_rooms(d: &SiteDesign, names: &std::collections::BTreeMap<String, String>, id: &str) -> Vec<Value> {
    let n = d.levels.len();
    let squares = (d.nx as usize) * (d.ny as usize);
    let building = d.is_building();
    let order: Vec<usize> = if building { (0..n).collect() } else { (0..n).rev().collect() };
    order
        .into_iter()
        .map(|li| {
            let l = &d.levels[li];
            let cells = design::decode(&l.cells, squares).unwrap_or_default();
            let rooms: Vec<String> = l
                .rooms
                .iter()
                .enumerate()
                .filter(|(ri, _)| cells.iter().any(|&c| c == *ri as i16))
                .map(|(ri, r)| names.get(&format!("r:{id}:{li}:{ri}")).cloned().unwrap_or_else(|| r.kind.clone()))
                .collect();
            let level = if building { json!(l.z) } else { json!(n - li) };
            json!({ "level": level, "name": l.name, "rooms": rooms.len(), "room_names": rooms })
        })
        .collect()
}

/// A summary and the plan, as two texts (the plan reads as it is written).
fn reply(summary: Value, plan: String) -> Vec<Value> {
    let mut out = text(summary);
    out.push(json!({ "type": "text", "text": plan }));
    out
}

fn summary(id: &str, d: &SiteDesign, designed: bool, problems: &[design::Problem]) -> Value {
    let problems = problem_list(problems);
    if d.is_building() {
        let levels: Vec<Value> = d.levels.iter().map(|l| json!({ "level": l.z, "name": l.name, "roof": l.roof, "stairs": l.has_stairs })).collect();
        return json!({ "id": id, "designed": designed, "kind": d.kind, "grid": [d.nx, d.ny], "stairs": building::stairs_of(d), "levels": levels, "problems": problems });
    }
    json!({
        "id": id,
        "designed": designed,
        "kind": d.kind,
        "theme": d.theme,
        "grid": [d.nx, d.ny],
        "entry": d.entry,
        "levels": d.levels.len(),
        "problems": problems,
    })
}

async fn get(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    let (brief, with_format) = (a["summary"].as_bool().unwrap_or(false), a["format"].as_bool().unwrap_or(false));
    app.worker
        .with(move |ex| {
            let (d, set_aside) = if id.starts_with("b:") { building::design_of(&ex.world, &ex.t0, &id, false)? } else { (design::design_of(&ex.world, &ex.t0, &id, false)?, design::misplaced(&ex.world, &ex.t0, &id)) };
            let names = &ex.world.file.edits.renames;
            let (built, problems) = d.problems(&ex.world, &ex.t0, &id)?;
            let plan = design::to_text_with(&d, &|li, ri| names.get(&format!("r:{id}:{li}:{ri}")).cloned(), Some(&built))?;
            let mut out = summary(&id, &d, ex.world.file.edits.designs.contains_key(&id) && !set_aside, &problems);
            if set_aside {
                out["set_aside"] = json!(if id.starts_with("b:") {
                    "its saved design was made for the building as it was (footprint or storeys since changed): this is the generated interior; saving a new design replaces the old one"
                } else {
                    "its saved design was made for a site that stood elsewhere (its way in is not under it): this is the generated site; saving a new design replaces the old one"
                });
            }
            let mut parts = if brief {
                out["levels"] = json!(level_rooms(&d, names, &id));
                text(out)
            } else {
                reply(out, plan)
            };
            if with_format {
                let f = if id.starts_with("b:") { format!("{BUILDING_FORMAT}\n\nA building's items: lines also take the site format's '+items:'/'-items:'. The site format:\n{FORMAT}") } else { FORMAT.to_string() };
                parts.push(json!({ "type": "text", "text": f }));
            }
            Ok(parts)
        })
        .await
}

async fn set(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    let src = arg_str(a, "text")?;
    let (furnish, dry, fit) = (a["furnish"].as_bool().unwrap_or(false), a["dry_run"].as_bool().unwrap_or(false), a["fit_shapes"].as_bool().unwrap_or(false));
    let id2 = id.clone();
    let (d, names, problems, shift, fitted, changes) = app
        .worker
        .with(move |ex| {
            let base = design::design_of(&ex.world, &ex.t0, &id2, false)?;
            let (mut d, names) = design::from_text(&src, &base)?;
            // What fitting the shapes did: rooms refused (left as written, drawn square) and why,
            // and the items it took away.
            let mut fitted: Vec<String> = Vec::new();
            if fit {
                let n = d.levels.len();
                for li in 0..n {
                    let tag = if d.is_building() { format!("level {}", d.levels[li].z) } else { format!("level {}", n - li) };
                    for ri in 0..d.levels[li].rooms.len() {
                        let shape = d.levels[li].rooms[ri].shape;
                        if !worldgen::interior::shape::geometric(shape) {
                            continue;
                        }
                        let had = d.levels[li].items.clone();
                        match d.set_shape_checked(&ex.world, &ex.t0, &id2, li, ri, shape) {
                            // (A refusal may name the level again, from the problem it found.)
                            Err(e) => fitted.push(format!("{tag}: not fitted, drawn square: {}", e.replacen(&format!("{tag}: "), "", 1))),
                            Ok(()) => {
                                let now = &d.levels[li].items;
                                let nx = d.nx as usize;
                                let cells = design::decode(&d.levels[li].cells, nx * d.ny as usize).unwrap_or_default();
                                // (Why: a square of it left empty (underground, rock), else a doorway or two rooms.)
                                let emptied = |f: &design::DesignItem| (f.y..f.y + f.h.max(1)).any(|j| (f.x..f.x + f.w.max(1)).any(|i| cells.get(j as usize * nx + i as usize).is_none_or(|&c| c < 0)));
                                for f in had.iter().filter(|f| !now.contains(f)) {
                                    let why = match (emptied(f), d.is_building()) {
                                        (true, true) => "on a square left empty",
                                        (true, false) => "in the rock",
                                        (false, _) => "in a doorway moved or across two rooms",
                                    };
                                    fitted.push(format!("{tag}: the {} at {},{} went ({why} once the {} was fitted)", f.kind, f.x, f.y, d.levels[li].rooms[ri].kind));
                                }
                            }
                        }
                    }
                }
            }
            if furnish {
                let (it, _) = d.problems(&ex.world, &ex.t0, &id2)?;
                for (li, lv) in it.levels.iter().enumerate() {
                    for (ri, r) in lv.rooms.iter().enumerate() {
                        let bare = !lv
                            .furniture
                            .iter()
                            .any(|f| !design::WAYS.contains(&f.kind) && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16);
                        if r.squares > 0 && bare {
                            d.furnish(&ex.world, &ex.t0, &id2, li, ri, fnv64(id2.as_bytes()));
                        }
                    }
                }
            }
            let (built, problems) = d.problems(&ex.world, &ex.t0, &id2)?;
            let shift = d.below_ground() as isize - base.below_ground() as isize;
            // (What changed, as the reply says it; only worked out for a design that will be kept.)
            let changes = if problems.iter().any(|p| p.blocking) { Value::Null } else { design::diff(&base, &base.problems(&ex.world, &ex.t0, &id2)?.0, &d, &built) };
            Ok((d, names, problems, shift, fitted, changes))
        })
        .await?;
    let refused: Vec<&str> = problems.iter().filter(|p| p.blocking).map(|p| p.text.as_str()).collect();
    if !refused.is_empty() {
        return Err(format!("not saved: {}", few(refused.iter().map(|r| r.to_string()).collect())));
    }
    // (Brief: the id, what changed, the check; get_site_design has the rest.)
    let mut out = json!({ "id": id, "problems": problem_list(&problems) });
    if fit {
        out["fit_shapes"] = json!(fitted);
    }
    // What changed, not the whole plan (get_site_design gives that), and the check's verdict.
    out["changes"] = if changes.as_object().is_some_and(|c| c.is_empty()) { json!("no changes") } else { changes };
    out["verdict"] = json!(if problems.is_empty() { "ok" } else { "ok, with notes (see problems)" });
    if dry {
        return Ok(text(json!({ "dry_run": true, "would_save": out })));
    }
    let id3 = id.clone();
    app.edit("agent", 0, move |e| {
        // (Levels dug or filled in below: names and places on the others follow them.)
        e.shift_levels(&id3, shift);
        for (li, ri, name) in names {
            e.renames.insert(format!("r:{id3}:{li}:{ri}"), name);
        }
        e.designs.insert(id3.clone(), d);
        Ok(json!({ "tool": "set_site_design", "id": id3 }))
    })
    .await?;
    Ok(text(out))
}

async fn reset(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    let id2 = id.clone();
    // (As generated, it may have more or fewer levels below ground than as designed.)
    let shift = app
        .worker
        .with(move |ex| {
            let now = design::design_of(&ex.world, &ex.t0, &id2, false)?;
            let generated = design::design_of(&ex.world, &ex.t0, &id2, true)?;
            Ok::<_, String>(generated.below_ground() as isize - now.below_ground() as isize)
        })
        .await
        .unwrap_or(0);
    app.edit("agent", 0, move |e| {
        e.designs.remove(&id).ok_or_else(|| format!("{id} has no design: it is as generated"))?;
        e.shift_levels(&id, shift);
        Ok(json!({ "tool": "reset_site_design", "id": id }))
    })
    .await
    .map(text)
}
