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
- **Changes only the browser has:** the app notes which edits mapd last saved to disk for it (mapd says so per
  change, and names its world store), so the next time the world opens with edits the browser kept, mapd's copy
  wins without a question only when mapd saved those edits and its copy lacks none of them; otherwise (a change
  that never reached mapd's disk, another worlds folder at the same address, a copy that lost something) the
  user chooses. When mapd's copy replaces the kept edits it is one step that Undo takes back.
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
| `c:<n>` | A created site (a building drawn by hand too; its building is `b:<layout>:0`: one place, its notes and hidden mark kept under `c:`, NPCs and plots found under either id). Generated from its id, so a remade world lays it out the same, though its layout number (and the ids inside it) can move; the remake moves the changes stored under them along (the app's, or mapd's own `regenerate`, which remakes the world natively, saves it and has the open app load it). Changes on places a remade world lays out differently are set aside at layout numbers from 1,000,000 up: no such place to the tools. |
| `gone:<n>:<id>` | A settlement taken back or removed in the app: its notes, NPCs' and plot points' places and its name are kept under this id, which names nothing (`world_overview`'s `places_removed`). |
| `was:<version>:<id>` | A settlement or site of a world upgraded to a generator that makes every town and site anew: what was made on it is kept under this id, which names nothing (`places_removed` too). |
| `n:<id>`, `p:<id>` | An NPC, a plot point (the DM's notebook). |
| `o:<id>`, `x:<id>`, `s:<asset>` | A battlemap object put down by hand; a clear (generated objects taken away); an uploaded sprite (an object kind). |
| `v:<id>` | A crossing put down by hand: a bridge, ford or ferry; takes a name and notes like a feature. |
| `j:<id>` | A link put down by hand: a way from a square below ground (a site, sewer section or cellar) to another. |
| `q:<id>` | A quay, pier or breakwater put down by hand. |
| `rw:<id>` | A road drawn by hand on the ground: a footpath, track, gravel road, paved road or cobbled street. |
| `rd:<8 hex>` | A road stretch, made from its two ends: the same across every edit that leaves the roads as generated; renamed and annotated like a feature. |
| `mt:<8 hex>` | The track from a settlement added by hand to the roads (from its two ends). |
| `tr:<layout>:<8 hex>`, `pp:<layout>:<spot>:<j>` | A generated town's or village's own street or lane (from its ends); a path in one of its parks, gardens or cemeteries. |
| `ts:<layout>:<id>` | A street added to a generated town by hand. |
| `h:<id>`, `g:<x>:<y>` | A plateau or pit; a block of ground brushed by hand (80 ft square, block indices). |
| `e:<id>` | Water drawn by hand (inlet, basin, lake, pond, moat). |

A place (`at`) is an id, a place's name, `{x_ft, y_ft}` or `[x_ft, y_ft]`.

## Tools

Full reference with every parameter, example calls and prompts: [MCP.md](MCP.md).
Every tool call is checked against the tool's schema before it runs (unknown arguments, wrong types and out-of-range
numbers are refused with the field and the rule), and the same thing has the same name in every tool (places are `at`,
lists come back as `{items, found}`): see [Conventions](MCP.md#conventions). The reads are `search`, `get` (by id; `view` children, design, town_plan, street_plan), `list` (`what`: names, roads, drawn_roads, streets, crossings, wharves, fences, objects, sprites, ground, waters, designs, links, npcs, plots, notes, sketch, hex_grid) and `inspect` (a place; `what` location, battlemap, heights, hex); each view takes only its own arguments.

| Tool | Does |
|---|---|
| `world_overview` | Size, land masses, counts of features by kind, the largest settlements, work set aside; the world hash. |
| `search` | Features by name, kind, tag or area (`within`), each with its extent; with `at` alone, everything within `radius_ft`, nearest first. |
| `get` | One thing by id: details and relations (`view` `details`), or `children`, `design` (a site's or interior's text plan), `town_plan`, `street_plan`; NPCs (`n:`) and plot points (`p:`) too. |
| `list` | Many of a kind, `what`: names, roads, drawn_roads, streets, crossings, wharves, fences, objects, sprites, ground, waters, designs, links, npcs, plots, notes, sketch, hex_grid (`{items, found}`). |
| `inspect` | A place, `what`: location (elevation, biome, water, what is near), battlemap (the 640-ft chunk), heights, hex. |
| `route` | Road or overland distance between two places, with 5e travel days. |
| `nearest` | The nearest coast, cliff, river, road, flat or highest ground to a place, within `radius_ft`. |
| `road_point` | A point `at_mi` or `fraction` along a road. |
| `render_view` | A screenshot of the open app at a place (needs the app in a visible tab). |
| `focus_view` | Fly the user's view to a place. |
| `set_feature` | Rename anything named, set or append notes and tags, hide or show it. |
| `set_site` | Without `id`, a new site (`kind`: ruin, tower, camp, waystation, cave, mine, lava_tube, entrance, castle, wall); with `id`, a castle's or wall's outline and parts, an entrance's building and way, a name. |
| `set_settlement` | Add a village or town (`at`, `tier`), or change one's tier, culture, trade, capital, name. |
| `set_building` | Draw a building (`poly`, `rect`, `circle`, or `at` with a size), a `stamp` (house, inn, smithy...) fronting a street, or change one (`id`); `dry_run` checks the spot. |
| `set_building_row` | A row of lots along a `path` or road, on one `side`; `dry_run` lists the lots. |
| `set_parcel` | A plot of ground (`poly`) with a `fill`, or change one. |
| `set_npc` | Add or change an NPC; `at` puts them somewhere (`null`: off the map). |
| `set_plot` | Add or change a plot point tied to places and NPCs. |
| `set_note` | A note at a point inside a building, dungeon or sewer level (`m:`), or move and change one. |
| `set_objects` | Put objects on the battlemap (up to 500), or change ones put down before (`o:` ids). |
| `set_sprite` | A picture as a new object kind, with its size and rules; or change an uploaded one's rules. |
| `set_ground` | Shape a battlemap's ground, `kind`: brush, plateau (or pit), hill, islet; with `id` (`h:`), change one. |
| `set_water` | Water drawn by hand (`poly` or a brushed band), or change it (`e:`). |
| `set_road` | A road drawn on the ground along a `path`, or change one (`rw:`). |
| `set_fence` | A fence or hedge along a `path`, change one or a generated run; `gaps` alone cuts gaps. |
| `set_crossing` | A bridge, ford or ferry `from` one bank `to` the other, or change one (`v:`). |
| `set_wharf` | A quay, pier or breakwater along a `path`, or change one (`q:`). |
| `set_link` | Paired exits below ground between two places (`a`, `b`). |
| `set_hex_grid` | The overland hex grid: size, orientation, offset, numbers, look (`reset` for the default). |
| `set_sketch_stroke` | Draw in the world's sketch, or change a stroke (`sk:`); `regenerate` applies it. |
| `set_design` | Change a site or a building's interior from its text plan (`dry_run`), or `reset` it to as generated. |
| `edit_town` | Lay a town out anew: move corners, relax, equalize, members, patterns, squares, walls (`dry_run`). |
| `edit_streets` | Lay a town's streets out by hand with ops (`dry_run`). |
| `remove` | Take things away by `id`/`ids` of any families (one change, all or none), or the world's own buildings and generated objects by place (`what`). |
| `restore` | Bring back what can come back (created sites, buildings, cleared objects, islets, roads, fences). |
| `batch` | Many edit steps as one change (`steps`: `{tool, arguments}`). |
| `regenerate` | Remake the world from its sketch with the strokes drawn since. |

`render_view` and `focus_view` need the app open. The other tools work without it. `get` and
`inspect` include the NPCs and plot points at a place.

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
| `plateaus` | Plateaus and pits, by `h:<id>`: `pts` (the outline, ft), `level` (ft, the world's datum: above the sea plus the sea's level), `edge` (ft, absent: sheer), `kind` (`islet` for a rock islet; absent: a plateau). |
| `wharves` | Quays, piers and breakwaters, by `q:<id>`: `kind`, `pts` (ft), `width`, `material` (absent: timber). |
| `paths` | Roads drawn by hand, by `rw:<id>`: `pts` (ft), `type` (footpath, track, gravel, paved, cobbled), `width` (ft; absent: the type's), `name`. |
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
- **Undo:** what an agent's `remove` takes, `restore` brings back where it can (it says when not); the creates are safe to repeat (the same one again returns what is there); an error means nothing changed, except a save warning (`saved: false`: don't repeat the call). The user's side: each change gets a notice with **Undo**. Ctrl+Z and Ctrl+Y undo and redo. Undoing a step puts back
  only what that step changed, and the result syncs back to mapd like any other edit.
- **Share links:** the world is in the page's share link (`#w=…`), with its edits while they are short (up to
  about 4,000 characters). The browser keeps every world's edits (IndexedDB), and exported world files carry
  them, with the pictures they refer to (NPC portraits and sprites, as `assets`: id → data URL; importing keeps them).

## Example session

> Rename the capital to Agentholm, then put a ruined watchtower with a crypt beneath it a few miles east of
> it, give it a hook, and show me its battlemap.

A typical sequence of tool calls:

1. `world_overview`: the capital's id.
2. `set_feature` with `name`.
3. `set_site` with `kind: "ruin"` and `under: "crypt"`: returns `c:<n>` and the crypt's `u:` id.
4. `set_feature` with `notes`.
5. `render_view` with `size_ft: 400`.
6. `focus_view`.

> Put the inn on a low hill by the river, with a jetty, and set a few barrels on it.

1. `nearest` with `what: "river"`: a point on the bank and the river's width.
2. `set_building` with `stamp: "inn"`, `at` near the bank and `dry_run: true`: where it would land; then again
   without `dry_run`.
3. `set_ground` with `kind: "hill"`, `around` the inn's id and `peak_ft: 15`: returns `h:<id>`.
4. `set_wharf` with `kind: "pier"` and a `path` from the bank out over the water.
5. `set_objects` with three `barrel`s on the pier.
6. If it looks wrong: `remove` with the ids made (one change), or `set_ground {id, peak_ft}` to change the hill.
