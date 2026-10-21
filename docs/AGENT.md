# Agents: mapd and its MCP tools

`mapd` is the map's local server. It lets an agent (any MCP client) read the open world, look at it, and
edit it while you watch. Every change shows up live in the map app.

It listens on **127.0.0.1 only**. Never put it behind the reverse proxy. Pages from other hosts are refused:
`/ws` and `/mcp` check the `Origin` header, so a web page elsewhere cannot reach it through DNS rebinding.

## Setup

1. Start the server: `npm run wasm` (once), then `npm run mapd`. This is `cargo run --release -p mapd` with:
   - `--port 7777`: where it listens.
   - `--dir worlds`: worlds on disk, gitignored.
   - `--app app/dist`: serves the built app when `npm run build` has made one.
2. Open the map:
   - With the dev server (`npm run dev`, http://localhost:5173), the app connects to `ws://127.0.0.1:7777/ws`
     by itself.
   - Or open mapd's own address, http://127.0.0.1:7777, when the app is built.
   - `?mapd=PORT` points at another port; `?mapd=0` turns the link off.
   - Pages not served from this machine never connect.
   - A notice says "Connected to mapd" when the link is up.
3. Connect your MCP client (once) to `http://127.0.0.1:7777/mcp`, for example:
   `claude mcp add --transport http worldspring http://127.0.0.1:7777/mcp`

The app tells mapd which world it has open (seed and parameters).

- **A world mapd has seen:** mapd keeps its edits, including any agents made while the app was closed, and the
  app takes them.
- **A world mapd hasn't seen:** mapd takes it as the app has it.
- **Changes replaced by the user:** opening a world file, a saved world or a link that brings its own changes, the
  user chooses between the changes kept and the ones it brings (or none). Chosen over the kept ones, or restored
  from a backup of the app's library, they replace mapd's copy whole: sites created before stay in their places
  (their ids don't move) marked removed, and other changes not among them are gone. The log shows `replace_edits`.

mapd generates the world natively (a few seconds) and reopens the last one when it restarts.

**Several tabs:** mapd follows one world at a time. A tab opening another world while a tab shows mapd's world
doesn't switch it: that tab says "Live sync is following another tab's world", keeps its changes in the browser,
and offers **Follow this tab** to switch. When the last tab showing mapd's world closes and none comes back within
20 seconds (a reload does), mapd follows another open tab. Once mapd follows a tab, the changes made there
meanwhile (or while mapd was away) go on top of mapd's copy. Screenshots and view moves go to a tab showing mapd's world, and edits reach only those tabs. Every tool takes
an optional `world` (the hash `world_overview` gives) and refuses to run if mapd has another world open.

**Saving:** each change is written to a `.tmp` file and moved over `world.json`. If that fails (the file is held
open elsewhere, a network share hiccups), mapd retries, keeps the change in memory, reports the error to the tool
and the app, and saves again every few seconds until it works. A `world.json.tmp` newer than `world.json` (a save
that never finished) is taken at startup.

## Places and ids

Positions are feet from the map's top-left corner: x to the east, y to the south. 5-ft squares; D&D 5e travel
pace.

| Id | What |
|---|---|
| `city:1a2b…`, `range:…`, `river:…`, … | Named features. Settlements and sites keep these ids. |
| `b:<layout>:<i>` | A building. |
| `d:<layout>:<q>` | A district. |
| `t:<layout>:<k>` | A wall tower. |
| `u:<layout>:<k>`, `w:…`, `k:…` | Ways underground: a dungeon, crypt, cave, mine or lava tube; sewer sections; a keep's dungeon. |
| `c:<n>` | A created site (a building drawn by hand too; its building is `b:<layout>:0`: one place, its notes and hidden mark kept under `c:`, NPCs and plots found under either id). Generated from its id, so a remade world lays it out the same, though its layout number (and the ids inside it) can move; the app moves the changes stored under them along. Changes on places a remade world lays out differently are set aside at layout numbers from 1,000,000 up: no such place to the tools. |
| `gone:<n>:<id>` | A settlement taken back or removed in the app: its notes, NPCs' and plot points' places and its name are kept under this id, which names nothing (`world_overview`'s `places_removed`). |
| `was:<version>:<id>` | A settlement or site of a world upgraded to a generator that makes every town and site anew: what was made on it is kept under this id, which names nothing (`places_removed` too). |
| `n:<id>`, `p:<id>` | An NPC, a plot point (the DM's notebook). |
| `o:<id>`, `x:<id>`, `s:<asset>` | A battlemap object put down by hand; a clear (generated objects taken away); an uploaded sprite (an object kind). |
| `v:<id>` | A crossing put down by hand: a bridge, ford or ferry. |
| `h:<id>`, `g:<x>:<y>` | A plateau or pit; a block of ground brushed by hand (80 ft square, block indices). |

A tool that takes a place accepts either an `id` or `x_ft` and `y_ft`.

## Tools

Full reference with every parameter, example calls and prompts: [MCP.md](MCP.md).

| Tool | Does |
|---|---|
| `world_overview` | Size, land masses, how many features of each kind, the largest settlements; what is set aside, and settlements removed (`places_removed`). |
| `search_features` | Features by current name or kind, each with its extent (bounding box and area, or a river's length). From three letters it also finds districts and businesses (inns, temples, …), renamed ones by their new names. |
| `get_feature` | Details and relations: what it is, its extent, the areas it lies in, nearest settlements, roads, districts, notable buildings, ways underground, a site's levels and rooms; a settlement's sites and flights of steps (as generated), and for anything in a layout the settlement (and site) it is in. |
| `list_children` | What a feature contains: districts and businesses, levels and rooms, a settlement's sites (drawn businesses among its businesses), a region's settlements and sites. |
| `list_names` | Everything that can be renamed, with current and generated names: named features; a settlement's districts, businesses, towers, underground sites and the sites in it; a building's or site's levels (`l:`) and rooms (`r:`). |
| `describe_location` | What is at a place: elevation, biome, the areas it lies in, the building, district or site there, named features nearby (measured to their nearest edge or course). |
| `features_near` | Everything named within a radius of a place, nearest first, with distance, direction and extent; `kinds` filters (districts and businesses too). |
| `route` | Road or overland distance between two places, with travel days at normal, fast and slow pace. |
| `list_roads` | The road network: named roads drawn in the sketch with the settlements along them, then roads between settlements and junctions (class, length, ends); filter by a place and radius, a settlement, a class, or drawn roads only. |
| `get_battlemap` | The 640-ft battlemap chunk at a place: surfaces (`stairs` on steep city streets), buildings, objects with their tactical rules, and what lies near the place by square. |
| `render_view` | A screenshot (PNG, 1536×1024) from the open app. `size_ft` is the ground across the image. About 450 shows the painted battlemap; 3000 a village; 20000 a city; 200000 a region. |
| `focus_view` | Fly the user's view to a place. |
| `rename_feature` | Rename anything `list_names` lists, down to a dungeon's levels and rooms. An empty name restores the original. |
| `annotate_feature` | Notes (lore, hooks, DM secrets) with tags. The app shows them in the info panel. |
| `create_feature` | A new site, generated like the world's own (map, battlemap, interiors, underground). Kinds: ruin (over a `dungeon`, `crypt` or `catacombs`), tower, camp (tents round a fire), waystation (an inn by the road), cave, mine, lava_tube, entrance (a bare way down to any of those). Sites underground take `size` (small–huge), `levels` (1–6) and a `theme` of their kind (prison, temple, wizard_lair, dwarven_hall, tomb, ossuary, fungal, ice, beast_den…). It must be on dry land; it is stepped out of river channels. The same kind again within 300 ft returns the site already there (`existing: true`), so it can be run twice. |
| `update_feature` | Name and notes in one go. |
| `hide_feature` | Hide a feature from labels and search, or show it again. |
| `delete_feature` | Remove a created site. Generated features can only be hidden. |
| `list_npcs`, `get_npc` | The notebook's NPCs: filter by text, place (a settlement includes its buildings, levels and rooms, and the sites in it), tag or attitude; one in full. |
| `create_npc`, `update_npc`, `delete_npc` | NPCs: name, appearance, mannerisms, attitude toward the players (stance and why), goals, DM notes, tags, status, portrait. |
| `place_npc` | Put an NPC at a place (a building puts them inside, on a level, shown to the DM), at a point, or nowhere. |
| `list_plots`, `get_plot`, `create_plot`, `update_plot`, `delete_plot` | Plot points tied to places and NPCs, with a status (idea, active, resolved). |
| `list_sprites` | The object kinds for battlemaps: built-in (trees, rocks, props, hazards) and uploaded, with their rules. |
| `upload_sprite` | A picture (base64, or a file on this machine) as a new kind of object, with its size and rules (cover, blocks movement or sight, difficult, height). |
| `place_objects` | Put objects on the battlemap at world positions (up to 500 at once); `rot_deg` turns one (drawn turned for props that can turn and for uploaded sprites). With an object's `id`, move, turn or change one put down before (only the fields given change). |
| `remove_objects`, `restore_objects` | Take objects away (placed ones by id, generated ones by kind and place, or all in a circle); bring cleared ones back. |
| `create_feature` (castle, wall) | A castle drawn as an outline (`poly` or `rect`, convex, 80–600 ft across; a curtain on it, round or square towers, a gatehouse on side `gate` or facing the road, an inner curtain round the high ground in a big one, a keep named as the site on the high ground (a great tower, a hall, an L or a T with towers, or a shell keep on a motte), barracks, stables and a smithy against the curtain, a bailey of open grass set out sparingly (a garden yard in some); `keep`, `yard_buildings`, `structure: ruin`), or a wall drawn as a line (`pts`, up to 3,000 ft, `closed` for a ring; towers, gates on the corners in `gates` and where roads and main streets cross it). The world's own buildings in the way are named, or taken away with it with `remove_in_way`. |
| `update_fortification` | Change a castle or wall: its outline or line, gates, options, name. |
| `create_building` | Draw a building: a footprint (`poly` corners, a `rect` or a round tower `circle`, snapped to the 5-ft grid), or `near` a place with `width_ft` and `depth_ft` (the nearest clear lot on a street), on dry land clear of buildings, roads, walls and squares; what it is (`func`: a business such as inn, blacksmith, temple, castle, or a home), `floors`, `roof` (hip, battlements, cone), `tint`, `structure` (ruin). It gets an interior, a roof and walls on the battlemap. Returns its `c:` id and building id (the same footprint again returns the building already there). |
| `update_building` | Change a building's options, or move or reshape it with a new footprint: a drawn one (`c:` id) or one of the world's own (`b:<layout>:<id>`; `auto` puts an option back as generated). |
| `check_building_spot` | Whether a footprint is clear for a building (read-only): its point and name, or why not. |
| `remove_buildings` | Take away buildings of the world's own, by ids or every one with its middle inside a polygon (a ward to clear); every other building keeps its id. |
| `restore_building` | Put one of the world's own buildings back as generated. |
| `place_crossing` | Put a bridge, ford or ferry down `from` one bank `to` the other (`[x_ft, y_ft]` each; 10 to 2000 ft, a ferry at least 68; `width_ft` 5 to 40, default 12; a bridge's `material` `timber` (default) or `stone`): a bridge's deck clear of the water (timber planks, or old stone on piers), a ford's bed at wading depth under stepping stones, a ferry's jetties with a raft on a rope between. With `id`, changes that one. Returns its `v:` id. |
| `list_crossings`, `remove_crossings` | The crossings put down by hand; take some away by id. |
| `shape_ground` | Shape a battlemap's ground with a round brush, a dab (`at`, `radius_ft`) or a stroke (`path`, `width_ft`): `raise` or `lower` (`height_ft`, `hardness`), `smooth` or `restore` (`strength`). Dry ground lowered never sinks below the water beside it. Returns the blocks changed and the lowest and highest ground under it before and after. |
| `set_plateau`, `delete_plateau` | A plateau or pit: an `outline`, or `around` a building, tower or site (its footprint grown by `margin_ft`), at `level_ft` above the sea or `rise_ft` above the ground there, with an `edge_ft` slope (0: a sheer cliff). With `id`, changes one; buildings on it stand at its level. |
| `ground_heights` | The ground's height on every square of a box round a place, as generated and as shaped, with the plateaus and brushed blocks there. |
| `get_site_design` | An underground site (`u:`) or a building's interior (`b:`) as a text plan: rooms by symbol (with a shape drawn: round, octagon, apse, chamfered, cavern), one character per 5-ft square, doors, items and the ways between levels (a building's: its stair block, levels by storey, cellars, doors to the outside). Rooms of kind `open to below` (a hall open two storeys, a keep's yard) and `roof` (a keep's roof inside its walk) have no floor: no doors, no items. |
| `set_site_design` | Change a site or a building's interior from a plan (any part of it; `doors: auto`; `furnish`; a site's levels added below). Refused when it breaks a rule play mode needs (a site's one way in, ways down over ways up, every square reachable; a building's one front door onto open ground, the stairs on floor, every room reached). A building's design is set aside when its footprint changes, and follows a change of storeys; `cellars <n>` digs or fills in cellars (0 to 3; names and NPCs on its levels follow). Items: furniture, indoor props (`sacks`, `brazier`, `bones`…) and uploaded sprites (`s:<asset id>`, sites underground too); `back n|e|s|w` sets the side one stands against (left out: it finds its wall), `variant 0-3` one of its looks (left out: picked by position; a throne dais has 0-5, wall and standing banners 0-7, a house's colours, a temple's pieces 0-7, its god's kind). Great hall pieces: `throne_dais`, `low_dais` (variant 0 stone, 1 timber; a platform: a piece another may stand on, as is a court's `garden_path`), `chair_of_office`, `high_table`, `carpet_runner`, `tapestry`, `wall_banner`, `standing_banner`, `font`, `cloister_well`, `dance_floor`, `harpsichord`, `musicians_corner`, `refreshment_table`; a temple's: `god_statue`, `candle_stand`, `carved_screen`, `screen_gate`, `side_altar` (a look per kind of god). Generated great halls are furnished about their axis (a throne dais only in castles and palaces; a low dais with a chair of office in town and guild halls and noble estates; a castle's banners all of one house), and so are a palace's throne room, banqueting hall, ballroom, audience chamber, long gallery and chapel, a temple's nave, sanctuary and side chapels (the god's statue toward the apse, the altar before it, candle stands, a big nave's arcades and carved screen across it before the crossing), a shrine and a monastery's church and refectory. Temples are laid out as a cross with a round apse (a big one) or a hall ending in one, monasteries round a cloister garth on lots merged for them. |
| `reset_site_design` | The site or the building's interior as generated again. |
| `get_town_plan` | A town's or city's patches (wards cut into lots) and their corners, as the ward editor sees them: each patch's ward (as generated and as set), district, neighbours and lot size, a park's garden (formal or common); each corner's place, and whether it is pinned (water, a river), a gate or on the wall, a junction where a square can open, and the square there; how far a corner may move. |
| `edit_town` | Lay out a town anew: move corners (`moves`, `by` or `to`), pull patches toward regular shapes (`equalize`), smooth corners round a point (`relax`), set patches (`ward`, including `empty` open ground; `lots`: small, medium, large, huge; `merge_with` a neighbour; `reroll`; `close`: open ground round a temple's patch's temple and its churchyard; `park`: a park's patch laid out as a formal garden or a common), squares where streets meet (`squares`: crossroads, well, fountain, green or none), walls up or down, or `reset`. Corners go only as far as keeps every patch convex (the reply says how far each went). Only the patches touched are built again: buildings that come out as they were keep their ids (a palace keeps the footprint it grew to where the homes under it come out again; a monastery, as the capital's palace, whatever its patch is laid out as; a temple is shaped again from its lot); a business the town lost goes to a new building where one fits, else the nearest home about a patch away (`functions_lost` lists the rest). `dry_run` reports first. |
| `batch` | Many edit tools as one change (`steps`: `{tool, arguments}` each): later steps see earlier ones (a site created in step 3 can be renamed in step 4), one save, one log entry, one notice in the app. If a step fails, nothing changes. |

`render_view` and `focus_view` need the app open. The other tools work without it. `get_feature` and
`describe_location` include the NPCs and plot points at a place.

## Edits

Edits are layered over the generated world. The world file is seed + parameters + `edits`:

| Field | Holds |
|---|---|
| `renames` | New names. |
| `notes` | Notes and tags. |
| `hidden` | Hidden feature ids. |
| `created` | Created sites (buildings drawn by hand carry `poly`, `floors`, `func`, `roof`, `tint`, `structure`); deleting one marks it `removed`, so `c:<n>` ids never shift. |
| `npcs` | NPCs by `n:<id>`. |
| `plots` | Plot points by `p:<id>`. |
| `objects` | Battlemap objects put down by hand, by `o:<id>`: kind, x, y (ft), rot, scale, variant, `turned` (drawn at `rot`; absent: as before). |
| `cleared` | Generated objects taken away, by `x:<id>`: one by kind and place, or all within `r` ft. |
| `sprites` | Uploaded sprites by asset id: name, size, cover, blocks_move, blocks_sight, difficult, height_ft. |
| `designs` | Underground sites and building interiors designed by hand, by site or building id: the grid's place, and per level its squares (run-length encoded), rooms, doors and items (a building's also its stair block and what it was made for); items include uploaded sprites as `s:<asset id>` and, when set by hand, `back` (the side one stands against, 0 north to 3 west) and `variant` (0-3; a throne dais 0-5, banners 0-7, a banner with none flying its building's house; a temple's pieces 0-7, a god's kind). Built instead of the generated one. |
| `crossings` | Crossings put down by hand, by `v:<id>`: kind (bridge, ford, ferry), ends `a` and `b` (ft), width; a bridge of old stone has `material: "stone"` (none: timber). |
| `buildings` | The world's own buildings changed or taken away, by `b:<layout>:<id>`: `at` (its middle as generated; the change is set aside if another building stands at that id after the town is laid out anew), `removed`, `func`, `floors`, `poly`, `roof`, `tint`, `structure`. |
| `ground` | Ground brushed by hand, by block (`g:XXXXX:YYYYY`, block indices zero-padded, x first): one string per 16 × 16 squares of height change in tenths of a foot. |
| `plateaus` | Plateaus and pits, by `h:<id>`: `pts` (the outline, ft), `level` (ft, the world's datum: above the sea plus the sea's level), `edge` (ft, absent: sheer). |
| `towns` | Towns laid out anew, by layout number: `at` (the town's centre; the edit is set aside if the town is laid out elsewhere), `corners` (`{v, from, to}`: a patch corner moved from where it was planned), `patches` (`{p, at, ward, lots, merge_with, reroll, close}`), `walls`, `squares` (`{v, at, kind}`: a square set where streets meet at corner `v`, planned at `at`; `kind` crossroads, well, fountain, green or none). Building changes (`buildings`) apply on top of it. |

Edits don't change the world's hash, and created sites use the same generators as the world's own. Objects
and clears change only the battlemap chunks holding them; crossings, the battlemaps and town-zoom tiles along them;
shaped ground, the battlemaps (and the buildings, interiors and steps on them) and the three finest zooms over it.

How edits are stored and shared:

- **On disk:** `worlds/<hash>/world.json` holds the world with its edits. `edits.jsonl` logs every change: when,
  who (`agent` or `user`), what, and the ops it made (a batch is one entry). Uploaded pictures (sprites, portraits) are kept by content
  hash in `worlds/assets/<id>` (`GET`/`PUT /assets/<id>`; pictures only).
- **Live:** changes travel as ops, each setting or removing one entry of one edits field (`{op: "set", field,
  key, value}` or `{op: "unset", field, key}`), so the app's changes and agents' changes made at the same time
  to different entries all stay. mapd sends every change to the open app. The app updates labels, search and the info panel, and
  regenerates the tiles and battlemaps around a created site.
- **From the app:** what you change (for example a rename in the info panel) goes back to mapd, so agents see it.
- **Undo:** each change gets a notice with **Undo**. Ctrl+Z and Ctrl+Y undo and redo. Undoing a step puts back
  only what that step changed, and the result syncs back to mapd like any other edit.
- **Share links:** the world is in the page's share link (`#w=…`), with its edits while they are short (up to
  about 4,000 characters). The browser keeps every world's edits (IndexedDB), and exported world files carry
  them, with the pictures they refer to (NPC portraits and sprites, as `assets`: id → data URL; importing keeps them).

## Example session

> Rename the capital to Agentholm, then put a ruined watchtower with a crypt beneath it a few miles east of
> it, give it a hook, and show me its battlemap.

A typical sequence of tool calls:

1. `world_overview`: the capital's id.
2. `rename_feature`.
3. `create_feature` with `kind: "ruin"` and `under: "crypt"`: returns `c:<n>` and the crypt's `u:` id.
4. `annotate_feature`.
5. `render_view` with `size_ft: 400`.
6. `focus_view`.
