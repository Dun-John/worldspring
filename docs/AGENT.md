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
| `c:<n>` | A created site (a building drawn by hand too; its building is `b:<layout>:0`). |
| `n:<id>`, `p:<id>` | An NPC, a plot point (the DM's notebook). |
| `o:<id>`, `x:<id>`, `s:<asset>` | A battlemap object put down by hand; a clear (generated objects taken away); an uploaded sprite (an object kind). |
| `v:<id>` | A crossing put down by hand: a bridge, ford or ferry. |

A tool that takes a place accepts either an `id` or `x_ft` and `y_ft`.

## Tools

Full reference with every parameter, example calls and prompts: [MCP.md](MCP.md).

| Tool | Does |
|---|---|
| `world_overview` | Size, land masses, how many features of each kind, the largest settlements. |
| `search_features` | Features by current name or kind, each with its extent (bounding box and area, or a river's length). From three letters it also finds districts and businesses (inns, temples, …), renamed ones by their new names. |
| `get_feature` | Details and relations: what it is, its extent, the areas it lies in, nearest settlements, roads, districts, notable buildings, ways underground, a site's levels and rooms. |
| `list_children` | What a feature contains: districts and businesses, levels and rooms, a region's settlements and sites. |
| `list_names` | Everything that can be renamed, with current and generated names: named features; a settlement's districts, businesses, towers and underground sites; a building's or site's levels (`l:`) and rooms (`r:`). |
| `describe_location` | What is at a place: elevation, biome, the areas it lies in, the building, district or site there, named features nearby (measured to their nearest edge or course). |
| `features_near` | Everything named within a radius of a place, nearest first, with distance, direction and extent; `kinds` filters (districts and businesses too). |
| `route` | Road or overland distance between two places, with travel days at normal, fast and slow pace. |
| `list_roads` | The road network: named roads drawn in the sketch with the settlements along them, then roads between settlements and junctions (class, length, ends); filter by a place and radius, a settlement, a class, or drawn roads only. |
| `get_battlemap` | The 640-ft battlemap chunk at a place: surfaces, buildings, objects with their tactical rules, and what lies near the place by square. |
| `render_view` | A screenshot (PNG, 1536×1024) from the open app. `size_ft` is the ground across the image. About 450 shows the painted battlemap; 3000 a village; 20000 a city; 200000 a region. |
| `focus_view` | Fly the user's view to a place. |
| `rename_feature` | Rename anything `list_names` lists, down to a dungeon's levels and rooms. An empty name restores the original. |
| `annotate_feature` | Notes (lore, hooks, DM secrets) with tags. The app shows them in the info panel. |
| `create_feature` | A new site, generated like the world's own (map, battlemap, interiors, underground). Kinds: ruin (over a `dungeon`, `crypt` or `catacombs`), tower, camp (tents round a fire), waystation (an inn by the road), cave, mine, lava_tube, entrance (a bare way down to any of those). Sites underground take `size` (small–huge), `levels` (1–6) and a `theme` of their kind (prison, temple, wizard_lair, dwarven_hall, tomb, ossuary, fungal, ice, beast_den…). It must be on dry land; it is stepped out of river channels. The same kind again within 300 ft returns the site already there (`existing: true`), so it can be run twice. |
| `update_feature` | Name and notes in one go. |
| `hide_feature` | Hide a feature from labels and search, or show it again. |
| `delete_feature` | Remove a created site. Generated features can only be hidden. |
| `list_npcs`, `get_npc` | The notebook's NPCs: filter by text, place (a settlement includes its buildings), tag or attitude; one in full. |
| `create_npc`, `update_npc`, `delete_npc` | NPCs: name, appearance, mannerisms, attitude toward the players (stance and why), goals, DM notes, tags, status, portrait. |
| `place_npc` | Put an NPC at a place (a building puts them inside, on a level, shown to the DM), at a point, or nowhere. |
| `list_plots`, `get_plot`, `create_plot`, `update_plot`, `delete_plot` | Plot points tied to places and NPCs, with a status (idea, active, resolved). |
| `list_sprites` | The object kinds for battlemaps: built-in (trees, rocks, props, hazards) and uploaded, with their rules. |
| `upload_sprite` | A picture (base64, or a file on this machine) as a new kind of object, with its size and rules (cover, blocks movement or sight, difficult, height). |
| `place_objects` | Put objects on the battlemap at world positions (up to 500 at once). |
| `remove_objects`, `restore_objects` | Take objects away (placed ones by id, generated ones by kind and place, or all in a circle); bring cleared ones back. |
| `create_building` | Draw a building: a footprint (`poly` corners, a `rect` or a round tower `circle`, snapped to the 5-ft grid) on dry land clear of buildings, roads and walls; what it is (`func`: a business such as inn, blacksmith, temple, castle, or a home), `floors`, `roof` (hip, battlements, cone), `tint`, `structure` (ruin). It gets an interior, a roof and walls on the battlemap. Returns its `c:` id and building id (the same footprint again returns the building already there). |
| `update_building` | Change a drawn building's options, or move or reshape it with a new footprint. |
| `place_crossing` | Put a bridge, ford or ferry down `from` one bank `to` the other (`[x_ft, y_ft]` each; 10 to 2000 ft, a ferry at least 68; `width_ft` 5 to 40, default 12): a bridge's plank deck clear of the water, a ford's bed at wading depth under stepping stones, a ferry's jetties with a raft on a rope between. With `id`, changes that one. Returns its `v:` id. |
| `list_crossings`, `remove_crossings` | The crossings put down by hand; take some away by id. |
| `get_site_design` | An underground site (`u:`) as a text plan: rooms by symbol, one character per 5-ft square, doors, items and the ways between levels. |
| `set_site_design` | Change a site from a plan (any part of it; `doors: auto`; `furnish`; levels added below). Refused when it breaks a rule play mode needs (one way in, ways down over ways up, every square reachable). |
| `reset_site_design` | The site as generated again. |
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
| `objects` | Battlemap objects put down by hand, by `o:<id>`: kind, x, y (ft), rot, scale, variant. |
| `cleared` | Generated objects taken away, by `x:<id>`: one by kind and place, or all within `r` ft. |
| `sprites` | Uploaded sprites by asset id: name, size, cover, blocks_move, blocks_sight, difficult, height_ft. |
| `designs` | Underground sites designed by hand, by site id: the grid's place, and per level its squares (run-length encoded), rooms, doors and items. Built instead of the generated site. |
| `crossings` | Crossings put down by hand, by `v:<id>`: kind (bridge, ford, ferry), ends `a` and `b` (ft), width. |

Edits don't change the world's hash, and created sites use the same generators as the world's own. Objects
and clears change only the battlemap chunks holding them; crossings, the battlemaps and town-zoom tiles along them.

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
