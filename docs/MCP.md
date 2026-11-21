# Worldspring MCP: full reference

`mapd` exposes the open world to agents (any MCP client) through 37 tools. With them an agent can:
- read and search the world: features, buildings, districts, underground sites, roads, routes, battlemaps;
- look at the map as the user sees it, or move the user's view;
- edit the world: rename anything (down to a dungeon's levels and rooms), write notes, hide features, and make
  sites, settlements and buildings that are as playable as generated ones;
- keep the DM's notebook: NPCs, plot points and notes at points inside levels;
- put objects on battlemaps (and take generated ones away), with uploaded sprites;
- shape a battlemap's ground (brush, plateaus and pits, hills, islets), draw water, roads, fences, crossings and
  wharves;
- set the overland hex grid, draw in the world's sketch and regenerate from it;
- lay a town out anew (wards and lots, streets) or design a dungeon's rooms and a building's interior by hand;
- do many of these at once, as one change, with `batch`.

Every edit is saved and appears live in the open map app. For setup, storage and undo, see [AGENT.md](AGENT.md).
The tools changed names in v66: see [Changes in v66](#changes-in-v66) at the end.

## Quick start

```sh
npm run wasm          # once
npm run mapd          # the server, on 127.0.0.1:7777
npm run dev           # the map app, on http://localhost:5173 (it connects to mapd by itself)
claude mcp add --transport http worldspring http://127.0.0.1:7777/mcp   # once: connect your MCP client, e.g. this
```

Then ask your agent in plain language, for example: *"What's the biggest city on the map?"*

mapd only listens on 127.0.0.1. Keep it off the reverse proxy.

## Conventions

- **Verbs.** Reads: `get` (one thing by id), `list` (many of a kind: `what`), `search` (by name, kind, tag or area),
  `inspect` (a place), with `world_overview`, `route`, `nearest` and `road_point`. Views: `render_view`,
  `focus_view`. Writes: `set_<thing>` makes one when called without `id` and changes it when given one (only the
  fields given change; the reply has its id); `remove` and `restore` take ids of any family; `edit_town` and
  `edit_streets` apply ops to a plan; `batch` runs many writes as one change; `regenerate` remakes the world from its
  sketch.
- **One vocabulary.** The same thing has the same name in every tool, and only that name: any other is an unknown
  argument (with a did-you-mean). A tool that is gone answers with what does its work now
  (`unknown tool 'list_roads': now list {what: "roads"}`).
- **Kinds and views.** Where one tool does several things, a discriminator picks one: `get` `view` (`details`, the
  default, `children`, `design`, `town_plan`, `street_plan`), `list` `what` (required), `inspect` `what`
  (`location`, the default, `battlemap`, `heights`, `hex`), `set_ground` `kind` (`brush`, `plateau`, `hill`,
  `islet`; with `id`, the id's own), `remove` / `restore` `what` (by place). Each choice checks the arguments against
  its own: an argument of another choice is refused naming it (`what 'sprites' takes limit; not settlement`,
  `kind 'islet' takes at, id, radius_ft, rise_ft; not peak_ft`). The schema's description of the discriminator lists
  which arguments go with which.
- **Coordinates** are in feet from the map's top-left corner: `x_ft` runs east and `y_ft` runs south. One mile is
  5,280 ft; one battlemap square is 5 ft. Every length in an argument's name says its unit: `_ft`, `_mi`,
  `_squares`, `_deg`.
- **Places.** `at` is a place: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`; `from` and `to` take the same forms. A
  point off the map (0 <= x < width, 0 <= y < height; the message gives the size) or an id that names nothing is
  refused. **Areas**: `within`, a box `[x0, y0, x1, y1]`, a polygon `[[x, y], ...]` (3+ points) or a circle
  `{at, radius_ft}`. **Lines**: `path` `[[x, y], ...]` (roads, fences, wharves, walls, brush strokes, sketch strokes,
  building rows). **Outlines**: `poly`, or `rect {x_ft, y_ft, width_ft, depth_ft, angle_deg}` and
  `circle {x_ft, y_ft, radius_ft}` where a tool takes them (`set_building`, `set_site` castles).
- **Text.** `notes` are annotations on a thing (a feature, an NPC); `text` is the body of a plot point or a point
  note, and the plan `set_design` takes. `tags` / `remove_tags` work the same everywhere.
- **Arguments are checked before anything runs**, for every tool and every `batch` step: unknown arguments, types
  (an integer is a whole number; `"10"` is not a number), `enum` values, `minimum`/`maximum`, list sizes and required
  fields. `null` for an optional argument counts as not given (except `set_site {id, in_building: null}`, which takes
  an entrance out of its building, and `set_npc {id, at: null}`, which takes an NPC off the map). A number out of
  range is refused, never clamped.
- **Errors** are one string, worded the same way everywhere: `field: rule (got X)` for a bad argument
  (`radius_squares: 1 to 64, got 200`), `no such <id> (what its prefix means)`, `unknown tool 'x' (did you mean y?)`
  or `unknown tool 'x': now <call>`, `<id> is a <family>: use <tool>`, `<choice> takes a, b; not c`. Up to five
  problems at once, with an item's index in lists. **An error means nothing changed.**
- **dry_run.** `set_building` (does a footprint fit, where a stamp would land), `set_building_row` (the lots),
  `set_design`, `edit_town` and `edit_streets` take `dry_run: true`: the reply is what the call would give, marked
  `dry_run: true`, and nothing changes. Elsewhere `dry_run` is an unknown argument.
- **batch.** `batch {steps: [{tool, arguments}, ...]}` runs writes as one change: each step sees those before it,
  the world is saved and shown once, and the user's Undo takes it back in one step. If a step fails, nothing changes
  and the error names the step; the reply lists each step's result in order. Reads run in a
  batch too; `batch`, `render_view` and `focus_view` can't be steps.
- **Ids of any family** go to `remove` and `restore` (`id` or `ids`): a call with ids of several families is one
  change, all or none. What can come back, `restore` brings back (created sites, drawn and world buildings, cleared
  objects, islets, roads, fences); water, links, notebook entries, wharves, crossings, sketch strokes and ground
  shaped by hand are deleted for good (`restore` says so; the user's Undo in the app brings them back). `remove`
  retires a created site but keeps what pointed at it (NPCs, plots, notes, a hand design, renames:
  `left_pointing`).
- **Safe to repeat.** A make asked for again with the same arguments adds nothing and returns what is there, with
  `existing: true`: `set_site` (the same kind within 300 ft), `set_building` (the same footprint), `set_npc` (the same
  name and place), `set_plot` (the same title), `set_objects` (the same kind within 0.5 ft, same turn and scale),
  `set_crossing` (both ends within 5 ft), `set_ground` islets (the same radius within 5 ft) and plateaus (an outline
  within 1 ft at every corner), `set_wharf` (every point within 5 ft), `set_sprite` (the same picture),
  `set_sketch_stroke` (the same stroke). A different level, width or material over an existing one is refused,
  naming it.
- **Lists** come back as `{items, found}`: `found` counts them before `limit` cut them, `truncated: true` says it
  did. Every list has a `limit` (the schema gives its default and maximum).
- **Moves are reported:** a write that puts something more than 5 ft from where it was asked (a site pushed off a
  river, a footprint snapped to the grid) says so: `moved: {x_ft, y_ft, ft, why}`.
- **Saves.** A change made but not saved to disk is not an error: its reply carries `saved: false` and a `warning`
  (don't repeat the call; mapd saves again every few seconds).
- **Which world.** Every tool takes an optional `world` (the hash `world_overview` returns); if another world is open
  in the app, the call is refused and nothing happens. Pass it in long scripted sessions.
- **Hints.** Each tool's `annotations` say whether it is read-only, destructive or idempotent; none reaches outside
  the world (`openWorldHint: false`).
- **Names** in results include renames, and search matches them; hidden features are left out of search unless
  asked for. **Extents**: `get` and `search` give a place's `extent` (`bbox_ft` with `area_sq_mi`, or a river's
  `length_mi`).
- **Sites in settlements:** a site is *in* a settlement when its point stands within the settlement's holding radius
  (its built-up area and outer districts: 1.4 built-up radii for a village, 2.15 for a town and up; the nearest for
  its size wins). `get` on anything inside a settlement's or site's layout gives `settlement` (`{id, name}` or
  `null`) and, inside a site's layout, `site`.

### Ids

| Pattern | Example | What |
|---|---|---|
| `<kind>:<hash>` | `city:bab44567`, `range:e48b3d59`, `river:…` | Named features: continents, seas, ranges, peaks, forests, rivers, lakes, settlements, ruins, caves… |
| `b:<layout>:<i>` | `b:0:57` | A building in a settlement or site |
| `d:<layout>:<q>` | `d:0:3` | A district |
| `t:<layout>:<k>` | `t:0:2` | A wall tower |
| `rd:<8 hex>` | `rd:c3517b52` | A road stretch (between settlements and junctions, as `list {what: "roads"}` lists them). Made from its two ends (rounded to 10 ft, either way round; stretches with the same ends told apart by length, so two such twins may swap ids if a sketch change alters one's length), so it stays the same across every edit that leaves the roads as generated (renames, notes, sites, buildings, objects, crossings, town edits); a remake of the world, or a sketch change that moves one of its ends, gives it a new id. Renamed and annotated like a feature |
| `v:<id>` | `v:zbfopudy` | A crossing put down by hand (bridge, ford, ferry): renamed and annotated like a feature |
| `j:<id>` | `j:ohffcybs` | A link put down by hand: a way from a square below ground to another (`set_link`); `get` gives its ends and `problems` |
| `sq:<layout>:…` | `sq:0:p0`, `sq:0:20` | A town's square or park: `p<patch>` on a patch, `<corner>` where streets meet, `g<building>` a palace's garden, `r<road>` a cemetery by a road out |
| `gt:<layout>:<corner>` | `gt:0:243` | A gate in a town's wall (while the town is walled) |
| `<layout>` | `0` | A town's entry in `towns` (the ward editor): its layout number; its patches and corners are numbered by `get {view: "town_plan"}` |
| `u:<layout>:<k>` | `u:312:0` | An underground site: dungeon, crypt, cave, mine or lava tube |
| `w:<layout>:<sx>:<sy>` | `w:0:2:1` | A sewer section |
| `k:<layout>:…` | | A keep's dungeon |
| `c:<n>` | `c:2` | A site created by you or an agent (a building drawn by hand too: one place with its building id `b:<layout>:0`; its notes and hidden mark are kept under `c:`, either id reads them, and NPCs and plots at either id are found under both) |
| `l:<site>:<level>` | `l:u:312:0:1` | A building's or site's level (levels count from the bottom), for renaming |
| `r:<site>:<level>:<room>` | `r:u:312:0:1:4` | A room on a level, for renaming |
| `n:<id>` | `n:k3x9q0ab` | An NPC (the notebook) |
| `p:<id>` | `p:0c7rtw2m` | A plot point (the notebook) |
| `o:<id>` | `o:4f0x2kq9` | A battlemap object put down by hand |
| `x:<id>` | `x:9a1b7c3d` | Generated battlemap objects taken away (a clear) |
| `s:<asset>` | `s:0123…cdef` | An uploaded sprite, as an object kind |
| `h:<id>` | `h:d470a78s` | A plateau, pit or rock islet (ground shaped by hand; an islet says `what: "islet"`) |
| `e:<id>` | `e:k2v9x0qa` | Water drawn by hand: a polygon or a brushed line (`set_water`) |
| `rw:<id>` | `rw:3m8q1zte` | A road drawn by hand: a footpath, track, gravel road, paved road or cobbled street (`set_road`) |
| `q:<id>` | `q:7d0kx3ab` | A quay, pier or breakwater put down by hand (`set_wharf`); a lighthouse or shops stand on it as drawn buildings (`c:`) |
| `g:<x>:<y>` | `g:16033:27870` | A block of ground brushed by hand: 16 × 16 squares (80 ft), block indices zero-padded to five digits |
| `gone:<n>:<id>` | `gone:1:village:d63861ee` | A settlement taken back or removed in the app: the notes, NPCs' places, plot points' places and its name (in `renames`) made on it are kept here; it names nothing |
| `was:<version>:<id>` | `was:57:village:5fde5f79` | A settlement or site of a world upgraded from that generator version to one that makes every town and site anew (58): its name, note, hidden mark, NPCs' and plot points' places are kept here; it names nothing |

Layout numbers count the settlements, then the world's own sites (those drawn in the sketch last), then the created
sites in the order they were made. No place is generated from its number: a created site from its `c:<n>` id, a
settlement from where it stands (its cell, or where its pin was drawn), one of the world's own sites from its
place. When the world is remade (its sketch redrawn), their buildings, interiors, sewers and sites underground are
generated the same as long as their roads and rivers are, though their layout numbers move when the world has
more or fewer places before them, and the ids inside them with them (`b:`, `u:`, `w:`, `l:`, `r:`). Look a place up
again with `get`.

When the app remakes a world, the changes stored under layout-local ids (names, notes and hidden marks on
buildings, districts, rooms and levels; designed dungeons and interiors; building changes; towns laid out anew;
where NPCs are; plot points' places) follow their places to their new numbers: a created site's always, one of the
world's own when the remade world lays it out alike (the same feature in the same place, drawn from the same
seed, laid out the same). Every other place's changes are set aside: moved to layout numbers from 1,000,000 up, one for
each place, where they stay in the world file but name nothing. Tools treat those ids as no such place (renaming,
hiding or annotating them is refused); `world_overview` lists their building changes, interiors and towns as
"set aside when the world was remade or upgraded". A place drawn from the same seed but laid out anew (a road or river
redrawn past it) has its changes set aside too. A settlement pinned in the sketch comes first in its tier and
moves the later ones along a number; they keep their changes. It counts as one of its tier: one the world would
have had is gone (one near it, within its tier's spacing, else the last), with its roads, and the pin keeps its
tier's spacing round it. One added in the app's Edit › Settlements › Places (a pin marked `added`) stands beside the world's
own instead: after them in its tier, pushing none of its tier out; smaller places within 10 miles of it move
away, and none is placed within 3 miles of another settlement. A city or metropolis brings king's roads either
way, which draw other towns and villages to new places: those come out different. A settlement taken back or
removed in the app doesn't hand what was made on it to a neighbour: the notes, NPCs and plot points on its own id
move to `gone:<n>:<id>` (its name kept under that id in `renames`), and its layout's changes are set aside as above.

### Feature kinds

| Group | Kinds |
|---|---|
| Settlements | `metropolis`, `city`, `town`, `village` |
| Sites | `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube`; created or drawn in the sketch only: `entrance`; created only: `building`, `castle`, `wall` |
| Nature | `continent`, `island`, `ocean`, `sea`, `bay`, `range`, `peak`, `pass`, `volcano`, `river`, `lake`, `waterfall` |
| Roads | drawn and named in the sketch only: `road` (its `detail` is its class: king's road, road or track) |
| Regions | `forest`, `jungle`, `taiga`, `desert`, `swamp`, `plains`, `tundra`, `glacier`, `salt_flat`; painted in the sketch only: `blight` (blighted woods), `ashlands`; drawn in the sketch only: `region` |
| Inside settlements | `district`, `building`, `tower`, `underground`, and the sites standing in it as `site` (in `list {what: "names"}`) |
| Inside buildings and sites | `level`, `room` (in `list {what: "names"}`) |

---

## Tools

One section per tool, in the order `tools/list` gives them. Each lists the arguments (from the schema: `*` required; every tool also takes `world`), an example call and the errors you are likely to meet. The guides further down explain each family in depth.

### world_overview

*World overview* (read). The world's size, land masses, counts of named features and its largest settlements.

No arguments (besides `world`).

Example:
```json
{"tool": "world_overview", "arguments": {}}
```
Errors: `world: a hash as world_overview gives it, not …` (a bad `world`).

### search

*Search* (read). Named features: by query (towns, ranges, rivers, ruins...; from three letters also districts, businesses and sites underground by name, kind or theme), kind, tag or within. With at alone (and kinds), everything named within radius_ft of it, nearest first, with distance, direction and extent. Returns {items, found}; limit cuts it and sets truncated.

| Argument | Type | Description |
|---|---|---|
| `at` | string or array or object | Only within radius_ft of this place, nearest first |
| `in_notes` | boolean, default `false` | The query also matches notes |
| `include_hidden` | boolean, default `false` |  |
| `kind` | string | Only this kind (city, ruin, river, building, underground...) |
| `kinds` | array of string | With at alone: only these kinds |
| `levels_max` | integer |  |
| `levels_min` | integer |  |
| `limit` | integer 1 to 500 | Default 25 (50 with at alone); at most 200 with a query, kind, tag or within |
| `query` | string, default `""` |  |
| `radius_ft` | number 50 to 2640000, default `26400` |  |
| `tag` | string | Only features whose notes carry this tag |
| `underground_kinds` | array of string | dungeon, crypt, catacombs, cave, mine, lava tube, sewer, keep dungeon |
| `within` | array or object | Only features inside (a point) or overlapping: a box, polygon or circle {at, radius_ft} |

Example:
```json
{"tool": "search", "arguments": {"query": "holt", "kind": "city", "limit": 5}}
```
Errors: `kinds: only with at alone`; `radius: unknown argument (did you mean radius_ft?)`.
More: `search` round a place below.

### get

*One thing* (read). One thing by id: a feature's details and relations (an NPC or plot point in full by its n: or p: id), what it contains, a site's or interior's text plan, a settlement's ward plan or its hand-laid streets.

| Argument | Type | Description |
|---|---|---|
| `format` | boolean, default `false` | Also give the full plan-format text |
| `id`* | string |  |
| `limit` | integer 1 to 2000, default `200` |  |
| `summary` | boolean, default `false` | Levels with room counts and names only, no plan text |
| `view` | `details` \| `children` \| `design` \| `town_plan` \| `street_plan` | What to give. Arguments by view: details (default): id; children: id, limit; design: format, id, summary; town_plan: id, within; street_plan: id |
| `within` | array of number | [x0_ft, y0_ft, x1_ft, y1_ft] |

Example:
```json
{"tool": "get", "arguments": {"id": "city:54167bdf", "view": "children", "limit": 20}}
```
Errors: `no such b:1:999 (b: a building, b:<layout>:<n>)`; `view 'children' takes id, limit; not summary`.
More: `get` below.

### list

*Lists* (read). Many things of one kind as {items, found}, cut by limit (truncated: true): names to rename, roads, drawn roads, streets in an area, crossings, wharves, fences, objects and clears, sprites, ground (plateaus, brushed ground, islets), waters, designs, links, NPCs, plot points, notes, the sketch's strokes, the hex grid.

| Argument | Type | Description |
|---|---|---|
| `anchor` | string |  |
| `at` | string or object or array | A place |
| `check` | boolean, default `false` |  |
| `class` | string | king's road (also kings road, kings_road), road or track; results say "king's road" |
| `drawn` | boolean, default `false` | Only roads drawn in the sketch |
| `function` | string | Only businesses of this function (inn, blacksmith...), in a settlement or site |
| `generated` | boolean, default `true` | false leaves out the generated islets |
| `inside` | string |  |
| `kind` | string | Only this kind (river, city, building, site, room...) |
| `kinds` | array | Only these kinds |
| `level` | integer 0 to … |  |
| `limit` | integer 1 to 2000, default `200` |  |
| `npc` | string |  |
| `of` | string | A site (u:, k:, w:, b:) or one of its levels (l:) |
| `points` | boolean, default `true` | false: each stroke's point count and bounding box instead of its points |
| `query` | string |  |
| `settlement` | string |  |
| `site` | string |  |
| `stance` | `hostile` \| `unfriendly` \| `neutral` \| `friendly` \| `allied` |  |
| `status` | `idea` \| `active` \| `resolved` |  |
| `tag` | string |  |
| `tool` | `land` \| `sea` \| `range` \| `river` \| `biome` \| `pin` \| `road` \| `massif` \| `elevation` \| `lake` \| `volcano` \| `site` … | Only strokes of this tool |
| `what`* | `names` \| `roads` \| `drawn_roads` \| `streets` \| `crossings` \| `wharves` \| `fences` \| `objects` \| `sprites` \| `ground` \| `waters` \| `designs` … | What to list. Arguments by what: names: function, inside, kind, limit, settlement; roads: class, drawn, limit, settlement, within; drawn_roads: limit; streets: kind, limit, within; crossings: limit; wharves: limit, within; fences: limit, within; objects: kinds, limit, which, within; sprites: limit; ground: generated, limit, within; waters: limit; designs: limit, within; links: at, check, limit, of; npcs: at, limit, query, stance, tag; plots: anchor, limit, npc, query, status, tag; notes: kind, level, limit, query, site, tag; sketch: limit, points, tool; hex_grid: - |
| `which` | `objects` \| `clears` \| `both` |  |
| `within` | object | Only near: a circle {at, radius_ft} |

Example:
```json
{"tool": "list", "arguments": {"what": "roads", "within": {"at": "city:54167bdf", "radius_ft": 5280}}}
```
Errors: `what: required`; `what 'sprites' takes limit; not settlement`; `what: one of names, roads, …, got "road" (did you mean roads?)`.
More: `list {what: "roads"}` below.

### inspect

*What is here* (read). A place looked at: what is there (elevation, biome, water, coast, the building or site, things within radius_ft), its battlemap (surfaces, buildings, objects by square), the ground's heights per square, or its overland hex.

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | An id, a name, {x_ft, y_ft} or [x_ft, y_ft] |
| `hex` | string | A hex number (CCRR) or "col,row" |
| `radius_ft` | number 0 to 500, default `15` |  |
| `radius_squares` | number 1 to 64, default `12` |  |
| `what` | `location` \| `battlemap` \| `heights` \| `hex` | What to look at. Arguments by what: location (default): at, radius_ft; battlemap: at, radius_squares; heights: at, radius_ft; hex: at, hex |

Example:
```json
{"tool": "inspect", "arguments": {"at": "city:54167bdf", "what": "battlemap"}}
```
Errors: `what 'location' takes at, radius_ft; not radius_squares`; `at: off the map (it is W x H ft)`.
More: `inspect` below.

### route

*Route* (read). Distance between two places by road (where roads join them) or overland, with D&D 5e travel days (normal, fast, slow pace).

| Argument | Type | Description |
|---|---|---|
| `from`* | string or object or array | Where from: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft] |
| `to`* | string or object or array | Where to: an id, a place's name, {x_ft, y_ft} or [x_ft, y_ft] |

Example:
```json
{"tool": "route", "arguments": {"from": "city:54167bdf", "to": "city:1fce50f3"}}
```
Errors: `from: no such …` (an id that names nothing); `from: off the map …`.
More: `route` below.

### nearest

*Nearest coast, cliff, river, road, flat or highest* (read). The nearest coast, cliff, river, road or flat ground to a place, or the highest ground, within radius_ft (default 26,400 = 5 mi, at most 132,000 = 25 mi): its point, distance, direction and what it is. found is false where there is none; cliff and flat searches are not exhaustive. Thresholds: MCP.md.

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | An id, a name, {x_ft, y_ft} or [x_ft, y_ft] |
| `radius_ft` | number 528.0 to 132000.0, default `26400.0` |  |
| `what`* | `coast` \| `cliff` \| `river` \| `road` \| `flat` \| `highest` |  |

Example:
```json
{"tool": "nearest", "arguments": {"at": "city:54167bdf", "what": "river", "radius_ft": 20000}}
```
Errors: `within_mi: unknown argument (did you mean …)`; nothing in range is a result, `found: false`, not an error.
More: `nearest` below.

### road_point

*A point along a road* (read). A point at_mi or fraction along a road: road (an rd: id or a road's name) from one end (from: a place near it; default the first), or from and to along the road route takes. Returns x_ft, y_ft, heading_deg and the stretch there; past the length is refused with it.

| Argument | Type | Description |
|---|---|---|
| `at_mi` | number 0 to … |  |
| `fraction` | number 0 to 1 |  |
| `from` | string or object or array | An end of the road (a place near it), or where a route starts |
| `road` | string | An rd: id, or the name of a drawn or renamed road |
| `to` | string or object or array | Where a route ends (instead of road) |

Example:
```json
{"tool": "road_point", "arguments": {"road": "rd:e3672989", "at_mi": 1.5}}
```
Errors: `no road rd:…`; `at_mi: …, got …` (out of range).
More: `road_point` below.

### render_view

*Look at the map* (view). A screenshot of the open map app at a place (JPEG 1024x683; width_px 256-1536, format png); needs the app in a visible tab. size_ft is the ground across: ~450 a battlemap, 3000 a village, 20000 a city. An l: or r: id in at shows that level inside (plan, doors, locks, furniture), as does a b:, u: or k: id with level; the map goes back after.

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | An id, a name, {x_ft, y_ft} or [x_ft, y_ft] |
| `format` | `jpeg` \| `png` |  |
| `level` | integer 0 to … | With at b:, u: or k:: show this level inside, 0 the lowest |
| `size_ft` | number 50 to 5000000, default `3000` |  |
| `width_px` | integer 256 to 1536, default `1024` | Pixels across (height 2/3 of it) |

Example:
```json
{"tool": "render_view", "arguments": {"at": "city:54167bdf", "size_ft": 2000}}
```
Errors: `the map app is not open: open it (it connects to mapd) and try again`; `…; render_view needs the map app open in a visible tab, with the place generated …`; `render_view can't be used in a batch`.
More: `render_view` below.

### focus_view

*Show the user* (view). Move the open map app's view to a place (the user sees it).

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | An id, a name, {x_ft, y_ft} or [x_ft, y_ft] |
| `size_ft` | number 50 to 5000000, default `3000` |  |

Example:
```json
{"tool": "focus_view", "arguments": {"at": "city:54167bdf", "size_ft": 3000}}
```
Errors: `the map app is not open: open it (it connects to mapd) and try again`; `focus_view can't be used in a batch`.
More: `focus_view` below.

### set_feature

*Name, notes, hide* (edit). Rename anything named (a feature, building, district, tower, site, level l:, room r:, road stretch rd:, crossing v:; empty name restores the original), set or append its notes and tags (remove_tags takes some off), or hide it from labels and search (hidden). A building drawn by hand is one place: either of its ids.

| Argument | Type | Description |
|---|---|---|
| `append` | boolean, default `false` |  |
| `hidden` | boolean, default `true` |  |
| `id`* | string |  |
| `name` | string | Empty restores the original |
| `notes` | string |  |
| `remove_tags` | array of string | Tags to take off (notes may be left out when tags or remove_tags is given) |
| `tags` | array of string |  |

Example:
```json
{"tool": "set_feature", "arguments": {"id": "city:54167bdf", "notes": "The bell tower is cracked.", "tags": ["hook"]}}
```
Errors: `set_feature: give name, notes, tags, remove_tags or hidden`; `no such …`.
More: `set_feature {id, name}` below.

### set_site

*Make or change a site* (edit). Without id, make one (kind): ruin (under: dungeon, crypt, catacombs), tower, camp, waystation, cave, mine, lava_tube, entrance (in_building, way); underground ones take size, levels, theme; castle (poly or rect) or wall (path or around_patches), with gates, keep, moat... (remove_in_way removes buildings in the way). With id: a castle's or wall's outline and parts, an entrance's in_building and way, any site's name.

| Argument | Type | Description |
|---|---|---|
| `around_patches` | array of integer | Instead of pts: a closed wall round these plan patches (get (view town_plan)); give again after moving corners |
| `at` | string or object or array | An id, a name, {x_ft, y_ft} or [x_ft, y_ft] |
| `closed` | boolean, default `false` | A wall's last corner joins its first (a ring) |
| `gate` | integer 0 to … | A castle's gate: its side (corner gate to the next); else facing a road (several: gates) |
| `gates` | array of integer | A wall's corners (indices into its line) that are gates; a castle's sides (corner g to the next) gated in their middle, the first the main one, 8 at most (replaces gate) |
| `id` | string | A site to change (castle or wall: its outline and parts; an entrance: in_building, way; any: name) |
| `in_building` | string or null | An entrance in this building's cellar (b:<layout>:<n> or a drawn building's c: id); with id, null takes it out |
| `keep` | boolean, default `true` | A castle's keep in the yard |
| `keep_func` | string | The keep's business (a list (what names) key) or home; auto: the castle itself |
| `keep_name` | string | The keep's own name; empty: the castle's |
| `keep_poly` | array | A castle's keep drawn by hand: its outline, 12 ft or more inside the castle's |
| `keep_shape` | `square` \| `round` \| `drawn` \| `auto` | A castle's keep: square (of pieces, never a shell keep), round (a great tower), drawn (keep_poly); auto: as rolled |
| `kind` | `ruin` \| `tower` \| `camp` \| `waystation` \| `cave` \| `mine` \| `lava_tube` \| `entrance` \| `castle` \| `wall` \| `plot` |  |
| `levels` | integer 1 to 6 |  |
| `moat` | `none` \| `dry` \| `wet` \| `auto` | A castle's moat: none, dry (a ditch) or wet; auto: by roll, wet by water on level ground (as shaped) |
| `name` | string |  |
| `path` | array | A wall's line: corners in order, sides 10 ft or more, 3000 ft in all (24000 closed) |
| `poly` | array | A castle's outline: corners in order (it may turn inward, not cross itself), 80 to 1500 ft across |
| `raise_hill` | boolean, default `false` | A castle on a hill: adds a round hill (as set_ground kind hill; h:...) centred on it, 60 to 120 ft high with a flat crown wider than the castle, unless a hill already stands under its middle. remove takes the hill away. |
| `rect` | object | Instead of poly: a rectangle about its middle (angle_deg clockwise from east) |
| `remove_in_way` | boolean, default `false` | Take away the world's buildings in its way (else refused) |
| `settlement` | string | With around_patches: the town (its id) |
| `size` | `small` \| `medium` \| `large` \| `huge` | The site underground |
| `snap` | boolean, default `true` | Snap to the 5-ft grid |
| `street_gates` | boolean, default `false` | A gate on every street crossing the wall (else only roads and main streets) |
| `structure` | `roofed` \| `ruin` | Roofed (default) or in ruins |
| `theme` | `dungeon` \| `prison` \| `temple` \| `wizard_lair` \| `bandit_hideout` \| `dwarven_hall` \| `goblin_warren` \| `flooded_vault` \| `cult_sanctum` \| `labyrinth` \| `collapsed_fortress` \| `crypt` … | Themes by kind: MCP.md |
| `under` | `dungeon` \| `crypt` \| `catacombs` \| `cave` \| `mine` \| `lava_tube` | Beneath a ruin (dungeon, crypt, catacombs) or an entrance (any) |
| `way` | `stairs` \| `trapdoor` |  |
| `yard_buildings` | boolean, default `true` | Barracks, stables, a smithy... along the inside of the walls |

Example:
```json
{"tool": "set_site", "arguments": {"kind": "ruin", "under": "dungeon", "at": [3539300, 1032200], "levels": 2}}
```
Errors: `kind: only when making a site (no id); a site keeps its kind`; `set_site on c:3 (a ruin) takes name, in_building, way (an entrance's); not poly`; a spot in water or a building in the way is refused, naming it.
More: `set_site` (a new site) below.

### set_settlement

*Make or change a settlement* (edit). Without id: add a village or town at a point (at, tier; name, culture, lives_by, link, blank). With id: change a settlement's tier, culture, lives_by, capital, name (regenerate remakes its layout).

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | Where its centre goes (a place) |
| `blank` | boolean, default `false` | true: laid out by hand from nothing: roads reaching it run in as streets to one junction at its centre, nothing else (no blocks or buildings) until streets are drawn. Its size class follows the people its homes house |
| `capital` | boolean | true: the realm's capital (the world's own settlements only) |
| `culture` | integer 0 to 4 | Naming culture: 0 cold north, 1 temperate west, 2 warm south, 3 dry lands, 4 steppe (default: nearest settlement's) |
| `id` | string | A settlement to change |
| `link` | array | A lane of its own along these points (world ft) out to the world's roads (its far end within 60 ft of a road joins it); kept as world lanes (lanes/...) |
| `lives_by` | `river` \| `mining` \| `fortress` \| `market` \| `farming` \| `lumber` \| `herding` \| `oasis` | What it lives by (default: from its ground) |
| `name` | string | If left out, named in its culture's style |
| `regenerate` | boolean, default `false` | One of the world's own: remake the world at once (as regenerate) |
| `tier` | `village` \| `town` |  |

Example:
```json
{"tool": "set_settlement", "arguments": {"at": [3536000, 1030000], "tier": "village", "name": "Ashby"}}
```
Errors: `set_settlement without id (a new one) takes …; not regenerate`; a spot that is not dry land, or too near another settlement: refused, saying why.
More: `set_settlement` (a new village or town) below.

### set_building

*Make or change a building* (edit). Without id, draw one: poly, rect or circle (or at with width_ft x depth_ft: the nearest clear spot on a street), or a stamp (house, inn, smithy...) at at fronting the nearest street (size, facing). With id, change any building (shape, func, floors, roof, material, shade, tint, name). dry_run: true says whether a footprint fits (its blockers) or where a stamp would land, changing nothing.

| Argument | Type | Description |
|---|---|---|
| `at` | string or object or array | Instead of a footprint: the nearest clear spot for width_ft x depth_ft on a street |
| `circle` | object | Or a round tower |
| `clear` | boolean, default `false` | Take away the world's own buildings in the way |
| `depth_ft` | number 10 to 400 |  |
| `dry_run` | boolean, default `false` | Only say what it would do (nothing changes) |
| `facing` | number 0 to 360 | Its front's direction, degrees (0 = +x, 90 = +y); left out, it fronts the nearest street |
| `floors` | integer 1 to 8 |  |
| `func` | string | A home or business key (docs/MCP.md); default house |
| `id` | string | A building to change (b:<layout>:<n>, or a drawn one's c: id) |
| `material` | `thatch` \| `slate` \| `clay_tile` \| `shingle` \| `turf` \| `lead` \| `copper` \| `glazed_tile` \| `flat_clay` \| `hide` \| `plank` \| `whitewash` … |  |
| `name` | string |  |
| `poly` | array | Corners [x_ft, y_ft] in order |
| `rect` | object | A rectangle about its middle |
| `roof` | `hip` \| `battlements` \| `cone` \| `auto` |  |
| `shade` | `auto` or integer 0 to 4 | Colour variant 0-4 (2 the usual); auto: picked |
| `shape` | `hip` \| `gable` \| `cone` \| `dome` \| `flat` \| `round` \| `auto` |  |
| `size` | `small` \| `usual` \| `large` or object | Default usual |
| `snap` | boolean, default `true` | Snap to the 5-ft grid |
| `stamp` | string | A building of this kind fronting the nearest street at at (with size, facing) |
| `structure` | `roofed` \| `ruin` |  |
| `tint` | `terracotta` \| `slate` \| `thatch` \| `shingle` \| `moss` \| `auto` | Roof colour (auto: picked) |
| `width_ft` | number 10 to 400 | With near: along the street |

Example:
```json
{"tool": "set_building", "arguments": {"stamp": "inn", "at": "city:54167bdf", "size": "large", "dry_run": true}}
```
Errors: `set_building with stamp takes …; not poly`; `dry_run checks a footprint (poly, rect or circle) or a stamp; …`; a footprint over a street or building: its `blockers`.
More: `set_building` (a new building) below.

### set_building_row

*A row of buildings* (edit). A row of lots along a path (or a road) on one side, with kinds, depth and gaps; clear: true takes the world's own buildings away. dry_run: true only lists the lots.

| Argument | Type | Description |
|---|---|---|
| `clear` | boolean, default `false` | Take away the world's own buildings in the way |
| `depth` | number 15.0 to 120.0 | Lot depth ft (default by the ward) |
| `dry_run` | boolean, default `false` | Only list the lots |
| `gaps` | number 0 to 0.5, default `0.15` | Share of slots left empty |
| `kinds` | array of string | Business or home keys to mix (default the ward's homes, now and then a business) |
| `path`* | array or string | Points or a road stretch id |
| `road` | object | Also a road along it, the houses set back half its width: in a town a street of it, else a drawn road. Not with rd: along |
| `seed` | integer 0 to … | Another mix for the same path |
| `side` | `left` \| `right` \| `both` | Left is [-dy, dx] |
| `ward` | `plaza` \| `castle` \| `temple` \| `merchant` \| `craft` \| `noble` \| `common` \| `slum` \| `docks` \| `military` \| `farm` \| `park` … | Sets lot widths, storeys and the mix |

Example:
```json
{"tool": "set_building_row", "arguments": {"path": [[3543900, 1032900], [3544200, 1032900]], "side": "left", "kinds": ["house", "shop"], "dry_run": true}}
```
Errors: `path: required`; no room along the line: the lots that fit, or a refusal saying why.
More: `set_building_row` below.

### set_parcel

*Mark or change a plot of ground* (edit). Without id: a free plot of ground (poly) with a fill: lots left empty for buildings by hand, or open ground. With id (c:<n>): its outline, fill or name. Returns its id. (set_plot is a notebook plot point.)

| Argument | Type | Description |
|---|---|---|
| `fill` | object | What the plot holds, as a hand block's fill (default generated, common) |
| `id` | string | A parcel (c:<n>) |
| `name` | string |  |
| `poly` | array | Corners [x_ft, y_ft] in order, within 750 ft of their middle |
| `snap` | boolean, default `true` | Snap to the 5-ft grid |

Example:
```json
{"tool": "set_parcel", "arguments": {"poly": [[3539700, 1032600], [3539800, 1032600], [3539800, 1032700], [3539700, 1032700]], "fill": "garden"}}
```
Errors: `poly: crosses itself`.
More: `set_parcel` (a new parcel) below.

### set_npc

*Make or change an NPC* (edit). Without id: add an NPC (name; appearance, mannerisms, attitude, goals, notes, portrait, stance, status, tags). With id: change the fields given. at puts them somewhere (a place id, and level inside it; or a point), null takes them off the map. Returns the NPC with its id.

| Argument | Type | Description |
|---|---|---|
| `appearance` | string | How they look and sound |
| `at` | string or object or array or null | Where they are: a place id (with level inside), {x_ft, y_ft} or [x_ft, y_ft]; null takes them off the map |
| `attitude` | string |  |
| `goals` | string |  |
| `id` | string | An NPC to change (n:...) |
| `level` | integer 0 to … | Level inside a building or site (0 lowest) |
| `mannerisms` | string | Habits, speech, quirks |
| `name` | string |  |
| `notes` | string |  |
| `portrait` | string | An asset id |
| `stance` | `hostile` \| `unfriendly` \| `neutral` \| `friendly` \| `allied` | Attitude toward the players |
| `status` | string |  |
| `tags` | array of string |  |

Example:
```json
{"tool": "set_npc", "arguments": {"name": "Mira Vell", "at": "city:54167bdf", "attitude": "wary", "tags": ["guide"]}}
```
Errors: `name: required (to make an NPC; give id to change one)`; `p:1 is a plot point (p:): use set_plot`.
More: `set_npc` (a new NPC) below.

### set_plot

*Make or change a plot point* (edit). Without id: add a plot point (title; text, status, anchors: places, npcs, tags). With id: change the fields given (lists are replaced). Returns it with its id.

| Argument | Type | Description |
|---|---|---|
| `anchors` | array of string |  |
| `id` | string |  |
| `npcs` | array of string |  |
| `status` | `idea` \| `active` \| `resolved` |  |
| `tags` | array of string |  |
| `text` | string |  |
| `title` | string |  |

Example:
```json
{"tool": "set_plot", "arguments": {"title": "The lost bell", "anchors": ["city:54167bdf"], "status": "open"}}
```
Errors: `n:… is an NPC (n:): use set_npc`.
More: `set_plot` (a new plot point) below.

### set_note

*Make or change a point note* (edit). Without id: a note at a point in a building, dungeon or sewer level (site, level, at as [x, y] squares; or room) with text and tags; returns its m: id. With id: move it (at, or room on its level) and change its text and tags. Notes on places are set_feature's.

| Argument | Type | Description |
|---|---|---|
| `at` | array of number | [x, y] grid squares |
| `id` | string | A point note (m:...) to change or move |
| `level` | integer 0 to … | Level index |
| `room` | string | r: id (its centre) |
| `site` | string | Building, dungeon or sewer section id |
| `tags` | array of string |  |
| `text` | string |  |

Example:
```json
{"tool": "set_note", "arguments": {"site": "b:1:3", "level": 0, "at": [4, 4], "text": "A loose flagstone."}}
```
Errors: `site: only for a new note (no id); a note moves on its level (at, room)`; `set_note with id: give at, room, text or tags`.
More: `set_note` (a new point note) below.

### set_objects

*Put objects down* (edit). Put objects on the battlemap at world positions (ft): built-in kinds or uploaded sprites (list what sprites), up to 500 at once; each one with an id (o:...) changes one put down before. They block, cover and hinder by their kind's rules. Returns their ids.

| Argument | Type | Description |
|---|---|---|
| `objects`* | array |  |

Example:
```json
{"tool": "set_objects", "arguments": {"objects": [{"kind": "barrel", "x_ft": 3539650, "y_ft": 1032850}, {"kind": "crate", "x_ft": 3539660, "y_ft": 1032850, "rot_deg": 30}]}}
```
Errors: `objects[0].kind: one of …`; `no uploaded sprite s:… (set_sprite first)`.
More: `set_objects` below.

### set_sprite

*Add or change a sprite* (edit). Add a picture as an object kind for battlemaps (64 px per 5-ft square is plenty): name, data_base64 or path, size_squares, height_ft and its rules (cover, blocks_movement, blocks_sight, difficult); or asset to change an uploaded one's rules. Returns its kind id (s:<asset>).

| Argument | Type | Description |
|---|---|---|
| `asset` | string | An uploaded asset to change |
| `blocks_movement` | boolean |  |
| `blocks_sight` | boolean |  |
| `cover` | `none` \| `half` \| `three-quarters` \| `full` or integer 0 to 3 | none, half, three-quarters, full (or 0 to 3) |
| `data_base64` | string | The picture (png, jpeg, webp, gif or svg), base64, top-down |
| `difficult` | boolean |  |
| `height_ft` | number 0 to 500, default `3` |  |
| `name` | string |  |
| `path` | string |  |
| `size_squares` | number 0.2 to 40, default `1` | Squares across at scale 1 |

Example:
```json
{"tool": "set_sprite", "arguments": {"name": "Wagon", "path": "/home/me/wagon.png", "size_squares": 2, "cover": "half", "blocks_movement": true}}
```
Errors: none of data_base64, path or asset: refused; a picture that is not png, jpeg, webp, gif or svg: refused.
More: `set_sprite` below.

### set_ground

*Shape the ground* (edit). The battlemap's ground by kind: brush (raise, lower, smooth or restore with a round brush: a dab at at or a stroke along path), plateau (an outline levelled at one height; a pit when it digs), hill (round, ridge, knoll or mountain), islet (a rock islet in water). With id (h:...), changes that plateau, hill or islet. Returns its id (h:...) or, for a brush, the blocks changed.

| Argument | Type | Description |
|---|---|---|
| `angle_deg` | number, default `0` | rx_ft's direction, clockwise from east |
| `around` | string | A building, tower or site (its footprint) |
| `at` | string or object or array | A place (id, point or [x, y] in ft) |
| `crown_ft` | number 0 to 512, default `0` | A flat crown's radius (at most 0.8 of the narrower half-axis) |
| `edge_ft` | number 0 to 60.0, default `0` |  |
| `floor` | `auto` \| `rock` \| `ground` | What its top or floor draws as (kept when changing one; default auto) |
| `from` | string or object or array | A place (id, point or [x, y] in ft) |
| `hardness` | number 0 to 1, default `0.5` | 0 soft to 1 hard |
| `height_ft` | number 0 to 50, default `5` | Raise and lower by up to this |
| `id` | string | A plateau, pit, hill or islet to change (h:...) |
| `kind` | `brush` \| `plateau` \| `hill` \| `islet` | brush: mode, at or path, radius_ft, width_ft, strength, hardness, height_ft. plateau (a pit when it digs): poly or around, level_ft or rise_ft, edge_ft, floor, margin_ft. hill: at or from+to or around, peak_ft, crown_ft, radius_ft, rx_ft, ry_ft, angle_deg, width_ft, profile. islet: at, radius_ft, rise_ft. With id it is the id's own (leave it out) |
| `level_ft` | number -20000 to 20000 | Its level, ft above the sea |
| `margin_ft` | number 0 to 200, default `10` | How far round the footprint |
| `mode` | `raise` \| `lower` \| `smooth` \| `restore` |  |
| `path` | array | A stroke's points (world ft) |
| `peak_ft` | number 1 to 300.0, default `60` | Its peak over the ground |
| `poly` | array |  |
| `profile` | `round` \| `ridge` \| `knoll` \| `mountain` |  |
| `radius_ft` | number 2.5 to 200, default `15` | A dab's radius |
| `rise_ft` | number -300.0 to 300.0, default `10` |  |
| `rx_ft` | number 10.0 to 640.0 | Half-axis along angle_deg |
| `ry_ft` | number 10.0 to 640.0 | Half-axis across (default rx_ft) |
| `strength` | number 0 to 1, default `0.5` |  |
| `to` | string or object or array | A place (id, point or [x, y] in ft) |
| `width_ft` | number 5 to 400, default `30` | A stroke's width |

Example:
```json
{"tool": "set_ground", "arguments": {"kind": "hill", "at": [3538600, 1033600], "radius_ft": 80, "peak_ft": 30, "profile": "knoll"}}
```
Errors: `kind: required without id (brush, plateau, hill, islet)`; `kind 'islet' takes at, id, radius_ft, rise_ft; not peak_ft`; `h:… is a hill: kind hill (or leave kind out), not plateau`.
More: Shaping the ground below.

### set_water

*Draw or change water* (edit). Without id: water drawn by hand, a polygon (poly) or brushed band; its look follows it (the sea's at sea level, dug: a quay; an open band a stream; a ring a moat; else a pond or lake by depth). With id (e:...): change it; what is not given stays. Returns its id and look.

| Argument | Type | Description |
|---|---|---|
| `closed` | boolean | Brush comes round to its start: a ring |
| `depth_ft` | number 1.0 to 20.0 | Default 4 |
| `dug` | boolean | Straight sides, stone edge |
| `id` | string | Water drawn by hand to change (e:...) |
| `kind` | `inlet` \| `basin` \| `lake` \| `pond` \| `moat` \| `stream` | Legacy fixed look; leave out |
| `level_ft` | number -20000 to 20000 | Surface, ft above the sea (default: the sea's or a lake's it reaches, else under its shore) |
| `name` | string | Its name on the map (empty: none) |
| `poly` | array | Points (world ft): polygon 3+, brush 2+ |
| `shape` | `polygon` \| `brush` | brush: a band width_ft wide along poly (default polygon) |
| `width_ft` | number 3.0 to 80.0 | Brush width (default 10) |

Example:
```json
{"tool": "set_water", "arguments": {"poly": [[3539000, 1032400], [3539150, 1032400], [3539150, 1032550], [3539000, 1032550]], "depth_ft": 8, "name": "Mill pond"}}
```
Errors: `poly: required` (no id); `e:… has this outline: give its id to change it (set_water {id})`.
More: `set_water` below.

### set_road

*Draw or change a road* (edit). A road on the ground along path: on the map and battlemap, walked like a road, a bridge over water; scatter and the world's own town buildings keep off it. With id (rw:...), changes one. Returns its rw: id and crossings.

| Argument | Type | Description |
|---|---|---|
| `id` | string | A road to change (rw:...); what is not given stays |
| `name` | string |  |
| `path` | array | Its line (world ft): a smooth curve through them |
| `type` | `footpath` \| `track` \| `gravel` \| `paved` \| `cobbled` | Width 4, 10, 14, 18, 20 ft; default track |
| `width_ft` | number 3.0 to 40.0 |  |

Example:
```json
{"tool": "set_road", "arguments": {"path": [[3540000, 1032400], [3540000, 1033200]], "type": "track", "width_ft": 10}}
```
Errors: `pts: unknown argument (did you mean …)`: the line is `path`.
More: `set_road` below.

### set_fence

*Draw or change a fence* (edit). A fence or hedge along path (over roads, not water): kind, closed, gates. With id, changes one. gaps alone cuts gaps into the fences there (generated ones too). Returns its id.

| Argument | Type | Description |
|---|---|---|
| `closed` | boolean |  |
| `gaps` | array | Instead of a fence: cut gaps into fences, [{points, width_ft?: 5}] |
| `gates` | array | Points for 5-ft gates |
| `id` | string | To change: f:, or fr:/fh: (kind, gates) |
| `kind` | `hedgerow` \| `hedge` \| `dry_stone_wall` \| `post_and_rail` \| `wattle` \| `low_wall` \| `picket` \| `iron_railing` \| `stockade` \| `high_wall` | hedge blocks the way; stockade, high_wall block sight; post_and_rail no cover; the rest hard going, half cover |
| `path` | array |  |

Example:
```json
{"tool": "set_fence", "arguments": {"kind": "hedge", "path": [[3539900, 1032600], [3539900, 1032900]], "gates": [100]}}
```
Errors: gaps with other arguments: refused (gaps alone cuts gaps); a line over water: refused.
More: `set_fence` below.

### set_crossing

*Put a crossing down* (edit). A bridge, ford or ferry from one bank to the other (from, to: world ft), 10-2000 ft long (a ferry 68+), 5-120 ft wide (a ford or ferry 40 at most); a bridge may carry shops. With id (v:...), changes that crossing. Returns its id, shops and the river's banks; an end in the water is refused.

| Argument | Type | Description |
|---|---|---|
| `from` | string or object or array | One bank (a place) |
| `id` | string | A crossing to change |
| `kind` | `bridge` \| `ford` \| `ferry` |  |
| `material` | `timber` \| `stone` |  |
| `shops` | boolean or object | A bridge's shops: true or {depth_ft (the deepest row), frontage_ft, lane_ft, vary}; false takes them off |
| `to` | string or object or array | The other bank (a place); the crossing |
| `width_ft` | number 5 to 120, default `12` | Deck, ford or raft width (a ford or ferry at most 40) |

Example:
```json
{"tool": "set_crossing", "arguments": {"from": [3542640, 1034323], "to": [3542780, 1034323], "kind": "bridge", "shops": true}}
```
Errors: an end in the water: refused, giving the banks; another kind or width over an existing crossing: refused, naming it.
More: `set_crossing` below.

### set_wharf

*Put a quay, pier or breakwater down* (edit). kind quay (paving along a shore; shore: true follows the water's edge near path), pier (a deck) or breakwater (a stone mole; lighthouse: true builds one at its tip) along path, a pier's or breakwater's first point on the bank. With id (q:...), changes one. Returns its id.

| Argument | Type | Description |
|---|---|---|
| `id` | string | A wharf to change |
| `kind` | `quay` \| `pier` \| `breakwater` |  |
| `lighthouse` | boolean |  |
| `material` | `timber` \| `stone` |  |
| `path` | array |  |
| `pier_spacing_ft` | number 0 to 400 | A shore quay's piers, this far apart (0: none; else 30 to 400) |
| `shore` | boolean | A quay only: snap path to the water's edge within 60 ft of it and follow it smoothly |
| `width_ft` | number 6 to 60 | By kind: MCP.md |

Example:
```json
{"tool": "set_wharf", "arguments": {"kind": "pier", "path": [[3542780, 1034330], [3542725, 1034330]]}}
```
Errors: a pier's first point not on the bank: refused.
More: `set_wharf` below.

### set_link

*Link two places* (edit). Join a square of one place below ground (a) to another's (b): a site underground (a level or room of u:/k:), a sewer section (w:) or a building's cellar (b:). Each end shows on its map as a way across (way: stairs, ladder, passage, trapdoor, rope, portal; default stairs) naming the far end. Returns its id (j:...).

| Argument | Type | Description |
|---|---|---|
| `a` | object | One end |
| `b` | object | The other end |
| `name` | string | Its own name (shown with the far end's) |
| `way` | `stairs` \| `ladder` \| `passage` \| `trapdoor` \| `rope` \| `portal` |  |

Example:
```json
{"tool": "set_link", "arguments": {"a": {"at": "b:1:3"}, "b": {"at": "w:1"}, "way": "ladder"}}
```
Errors: an end not on free floor: refused, naming the end.
More: `set_link` below.

### set_hex_grid

*Set the hex grid* (edit). Change the overland hex grid: only the fields given change; reset goes back to the defaults. Returns the grid.

| Argument | Type | Description |
|---|---|---|
| `color` | string |  |
| `flat` | boolean | Flat-topped (default) or pointy |
| `numbers` | boolean |  |
| `opacity` | number 0 to 1 |  |
| `ox_mi` | number -10000 to 10000 | Move the grid east (mi) |
| `oy_mi` | number -10000 to 10000 | Move the grid south (mi) |
| `reset` | boolean | Defaults first, then the fields given |
| `size_mi` | number 0.5 to 60.0 | Miles across the flats; default 6 |
| `width` | number 0.5 to 6.0 |  |

Example:
```json
{"tool": "set_hex_grid", "arguments": {"size_mi": 6, "numbers": true, "opacity": 0.5}}
```
Errors: a value out of range (`size_mi`, `opacity`, `width`): refused, never clamped.
More: `set_hex_grid` below.

### set_sketch_stroke

*Draw or change a sketch stroke* (edit). Without id: add a stroke to the world's sketch (tool: land, sea, range, river, forest, biome, road, settlement...: an outline or path, or a pin at at). With id (sk:...): change its points and options; what is not given stays. Stored at once; regenerate remakes the world from the sketch. Returns its id.

| Argument | Type | Description |
|---|---|---|
| `added` | boolean | true (default): beside the world's settlements; false: in place of one |
| `at` | string or object or array | A pin's place: {x_ft, y_ft}, [x_ft, y_ft] or a feature's id |
| `biome` | `ocean` \| `lake` \| `ice` \| `tundra` \| `alpine` \| `taiga` \| `temperate_forest` \| `temperate_rainforest` \| `grassland` \| `steppe` \| `cold_desert` \| `hot_desert` … | The biome painted (forest: temperate_forest unless given) |
| `capital` | boolean | The realm's capital |
| `closed` | boolean | path is an outline, filled (default true); false: a band radius_mi each side |
| `hard` | boolean | Edges exactly as drawn, not natural ones |
| `id` | string | A stroke to change (sk:...) |
| `kind` | `kings_road` \| `road` \| `track` \| `none` | Road class (default road); none: a line no planned road crosses |
| `lives_by` | `port` \| `river` \| `mining` \| `fortress` \| `market` \| `farming` \| `fishing` \| `lumber` \| `herding` \| `oasis` | What the pin lives by (default: from its land) |
| `name` | string | Name of what it makes (a road's: else none); empty: none |
| `on_water` | boolean | true (default): a port, fishing village or river town moves onto its shore or river as the world's own do; false: it stays at the pin (on heights above its river, say) |
| `path` | array | The line or outline [[x_ft, y_ft], ...] (not for a pin): 2+ points, 3+ for an outline |
| `radius_mi` | number 0.1 to 200 | Half-width of a band, range or valley (defaults: land 15, sea 10, range 15, river 3, forest/biome 20) |
| `replaces` | `metropolis` \| `city` \| `town` \| `village` | With added false: tier of the world's settlement (within 3 mi) it replaces |
| `strength` | number 0 to 1 | range: height 0.2 hills to 1 peaks; river: size (default 0.7) |
| `tier` | `metropolis` \| `city` \| `town` \| `village` | The settlement's size |
| `tool` | `land` \| `sea` \| `range` \| `river` \| `forest` \| `biome` \| `road` \| `pin` | What the stroke makes (sea is cut out of land; range runs along the line, river down from its first point) |
| `wards` | array of string | District names, central first |

Example:
```json
{"tool": "set_sketch_stroke", "arguments": {"tool": "pin", "at": [3539000, 1031000], "tier": "village", "name": "Testford"}}
```
Errors: `tool: required` (no id); a path with too few points for an outline: refused.
More: `set_sketch_stroke` below.

### set_design

*Design a site* (edit). Change an underground site or a building's interior from a text plan: read it with get (view design; format: true gives the full format), change it, send it back (text). Only what the text gives changes. dry_run: true says what would change. reset: true drops the design (generated again as it was).

| Argument | Type | Description |
|---|---|---|
| `dry_run` | boolean, default `false` | Check it without saving |
| `fit_shapes` | boolean, default `false` | Fit shaped rooms to their shape; one that would break a rule stays square (the reply's fit_shapes says which) |
| `furnish` | boolean, default `false` | Fill rooms with no props from their kind's kit |
| `id`* | string |  |
| `reset` | boolean, default `false` | Drop the design: generated again as it was (id alone) |
| `text` | string | The text plan (get with view design gives it); not with reset |

Example:
```json
{"tool": "set_design", "arguments": {"id": "b:1:5", "text": "level 0: Ground floor\nitems:", "dry_run": true}}
```
Errors: `level 1: the building has no such level (its levels: -1, 0)`; `b:1:5 has no design: it is as generated` (reset); `set_design with reset takes id; not text`.
More: `get {view: "design"}` below.

### edit_town

*Lay out a town anew* (edit). Change a town's or city's layout as the ward editor does; give any of: moves (corners to or by a place), equalize (patch numbers), relax (corners round a point), patches (ward, lots, merge_with, reroll, close, park, district; "auto" puts a field back), squares (where streets meet), walls, reset, members. Corners go only as far as keeps every patch convex. dry_run reports only. See MCP.md > Towns.

| Argument | Type | Description |
|---|---|---|
| `dry_run` | boolean, default `false` |  |
| `equalize` | array of integer |  |
| `id`* | string | The town or city (its id; villages have no wards) |
| `members` | object | By hand: c:<n> drawn things (add joins, remove leaves); b:<layout>:<id> outskirt buildings (remove pushes out into a new place, add takes back) |
| `moves` | array |  |
| `patches` | array |  |
| `pattern` | `auto` \| `grid` \| `rings` \| `radial` \| `linear` | The street pattern of its planned part (see planning): a regular grid, concentric rings, streets radiating from the centre, or one long street with cross streets; "auto" as picked from its land and culture. Alone or with planning; lays the town out anew like planning does, setting aside the changes made inside it. Not for villages, nor towns laid out by hand |
| `planning` | `auto` or number 0 to 1 | How planned its streets are, given alone: 0-0.3 grown, 0.3-0.7 a planned quarter beside a grown core, 0.7-1 planned throughout (a regular grid); "auto" as rolled. Lays the town out anew: the changes made inside it are set aside |
| `relax` | object |  |
| `reset` | `all` \| `corners` \| `patches` \| `squares` \| `walls` |  |
| `squares` | array |  |
| `walls` | `auto` or boolean or object | true, false or "auto" (as generated), or {built, gates_add, gates_remove (corners), towers_add ([corner, next corner, f 0.05-0.95] or a corner), towers_remove (corner or t:<layout>:<n>)} |

Example:
```json
{"tool": "edit_town", "arguments": {"id": "city:54167bdf", "relax": true, "dry_run": true}}
```
Errors: a town not laid out by hand yet: the first edit lays it out; problems come back as `problems`.
More: `edit_town` below.

### edit_streets

*Lay out streets by hand* (edit). Lay out a town's streets by hand: ops in order (or one as op), ids from get (view street_plan), world ft: lay_out_by_hand, back_to_generated, add_street {points, width?, set_l?, set_r?, class?}, delete_street {street}, set_street {street, width?, set_l?, set_r?, class? main|street|quay|joint, bridge?}, move_junctions {moves: [{junction, to|by}]}, set_square {junction, square|none}, set_block {block, fill? {kind generated, ward, lots?, height? low|usual|tall, reroll?, close?} | {kind hand} | {kind open, open, style?} | {kind castle, gates? [[x, y]], keep_shape? square|round, moat? none|dry|wet}, district?, seed?, reroll?}, add_row {street, side l|r, depth, ward, lots?, gaps?}, remove_row {row}, add_area {poly, open}, remove_area {area}, add_wall {pts, closed?, spacing?, towers?, gates?, name?}, remove_wall {wall}, set_area {area, open}, set_wall {wall, gates? [[x, y]] (replaces the hand gates, each within 30 ft of the line), name? (empty: none), spacing?}. A refusal ends the list. dry_run: first op only. See MCP.md.

| Argument | Type | Description |
|---|---|---|
| `dry_run` | boolean, default `false` |  |
| `id`* | string | The town or city (its id) |
| `op` | object | {op, ...}: see the tool's description and MCP.md |
| `ops` | array |  |

Example:
```json
{"tool": "edit_streets", "arguments": {"id": "city:54167bdf", "op": {"op": "add_street", "points": [[3543900, 1032900], [3544200, 1032900]]}, "dry_run": true}}
```
Errors: an op the plan cannot take: refused, naming the op (with `dry_run`, the problems come back and nothing changes).
More: `edit_streets` below.

### remove

*Take away* (edit). Take things away by id (id or ids, any families, all or none, one change): created sites and drawn buildings (c:, b:), the world's own buildings (b:), NPCs (n:), plot points (p:), notes (m:, r:), objects (o:), ground (h:, i:), water (e:), roads (rd: mt: tr: pp: rw: ts:), fences (fr: fh: f: fg:), wharves (q:), crossings (v:), links (j:), sketch strokes (sk:). what: buildings / objects take within (objects also points, kinds); notes the notes on places. Generated features can only be hidden (set_feature).

| Argument | Type | Description |
|---|---|---|
| `id` | string |  |
| `ids` | array of string | Ids of any family (all or none) |
| `kinds` | array | Only these kinds |
| `points` | array | Generated objects, as inspect (what battlemap) lists them |
| `what` | `buildings` \| `objects` \| `notes` | Instead of ids alone: buildings (the world's own inside within, with ids), objects (generated ones: within, points, kinds; with ids), notes (the notes on these places' ids) |
| `within` | array or object | [x0, y0, x1, y1], [[x, y], ...] or {at, radius_ft}; only a circle takes generated objects |

Example:
```json
{"tool": "remove", "arguments": {"ids": ["n:ab12cd34", "o:9f8e7d6c", "rw:k2j3h4g5"]}}
```
Errors: `Nothing was changed`, naming the id refused (all or none); `x:… is a clear (generated objects taken away): restore brings them back`; `what 'buildings' takes …; not kinds`.
More: `remove` and `restore` below.

### restore

*Bring back* (edit). Bring back what remove took (id or ids, all or none, one change): created sites and drawn buildings (c:, b:), the world's own buildings (b:, as generated), clears (x:), generated islets (i:), roads and fences by their ids; or what: objects with within (every clear touching it). NPCs, notes, water, wharves, crossings, links, strokes and ground shaped by hand are deleted for good.

| Argument | Type | Description |
|---|---|---|
| `id` | string |  |
| `ids` | array of string | Ids of any family (all or none) |
| `what` | `objects` | objects: every clear touching within |
| `within` | array or object | Clears touching a box, polygon or circle |

Example:
```json
{"tool": "restore", "arguments": {"ids": ["b:1:7", "rd:e3672989"]}}
```
Errors: `e:1 can't be restored: water drawn by hand is deleted for good (the app's Undo can bring it back)`; `… can't be restored, its place is taken: …`.
More: `remove` and `restore` below.

### batch

*Many edits at once* (edit). Run many edit tools (set_feature, set_site, set_npc, remove, set_objects...) as one change: each step sees those before it; saved and shown once. If a step fails nothing changes and the error names it. Steps are {tool, arguments}; the result lists their results in order.

| Argument | Type | Description |
|---|---|---|
| `steps`* | array | The tools to run, in order (up to 5,000) |

Example:
```json
{"tool": "batch", "arguments": {"steps": [{"tool": "set_site", "arguments": {"kind": "tower", "at": [3539300, 1032500], "name": "The Needle"}}, {"tool": "set_npc", "arguments": {"name": "Old Hesk", "at": [3539300, 1032500], "attitude": "gruff"}}]}}
```
Errors: `focus_view can't be used in a batch`; a step that fails: nothing changes and the error names the step.
More: `batch` below.

### regenerate

*Remake the world* (edit). Remake the world with the sketch's pending changes (a few seconds). Edits on places that come out the same stay; the rest are set aside (`vanished` says what they said; later in world_overview's places_removed). The world gets a new hash; an open app loads it (asking first about unsaved work). Returns the changes kept, set aside and moved, settlements before and after, the time.

No arguments (besides `world`).

Example:
```json
{"tool": "regenerate", "arguments": {}}
```
Errors: `The sketch is as the world was made: nothing to regenerate`.
More: `regenerate` below.

## Guides by family

The sections below explain each family in depth: what the generator does with what an agent draws, the rules
that are not in a schema, and longer examples.

## Read tools

These never change anything, and work whether or not the map app is open.

### `world_overview`
A summary of the world:
- `size_mi`: the map's `[width, height]` in miles (the app's `width_ft` and `height_ft` are 5,280 times that);
- its land masses;
- how many features of each kind it has;
- its largest settlements, with population and position;
- how many sites have been created;
- `roads`: `generated and drawn`, or `only drawn` (see "Worlds with only drawn roads");
- `buildings_changed`, and `buildings_set_aside`: changes to the world's own buildings that no longer apply
  because the town was laid out anew (each with why);
- `designs_set_aside`: interiors and sites underground designed by hand that are not built (the building's
  footprint or storeys changed, or it is gone, or the design breaks a rule now; a site's design set aside with its
  place, or made for a site that stood elsewhere: the site's way in is not under it), each with why;
- `towns_set_aside`: towns laid out anew by hand whose changes no longer apply, whole or in part (the town was
  laid out elsewhere after a sketch change, a corner or patch is not where it was planned), each with why;
- `places_removed`: settlements taken back or removed in the app (`gone:<n>:<id>`), and settlements and sites of a
  world upgraded to a generator that made them anew (`was:<version>:<id>`), each with its `name` then,
  whether it has a `note`, and how many `npcs` are there and `plots` name it;
- `world`: its hash, for the `world` parameter of other tools.

| Parameter | Type | Default | |
|---|---|---|---|
| none | | | |

```json
{ "name": "world_overview", "arguments": {} }
```
> *"Give me an overview of this world."* · *"Which are the five biggest cities?"*

### `search`
Searches by name, as the places are named now (renames included). From three letters, it also finds districts and
named businesses (inns, temples, smithies…) and wall or gate towers that have been renamed; road stretches that have been
renamed (kind `road`, as the roads drawn in the sketch) and crossings put down by hand that have a name (kind `crossing`).
One of `query`, `kind`, `tag` or `within` must be given (nothing at all is refused); a `kind` nothing has is refused with a "did
you mean" and the kinds there are. A district, building or tower says its `settlement` as `{id, name}` (and its
`site`, in a site's layout), as `get` does. A building drawn by hand shows the ward of its own little layout
(`village`), not the one it stands in. Returns `{items, found}` (`truncated: true` when `limit` cut the list); each item has its `extent`.

**Sites underground** (kind `underground`, after the places): ways down in towns and ruins and under created sites (`u:`),
keeps' deep dungeons (`k:`) and each city's sewers as one site (`w:<layout>`, named "Sewers of …"; its sections
`w:<layout>:<sx>:<sy>` are not listed apart). Found by their own name (a rename, a city's sewers' name, or the name of the
place above them), by a **renamed** level or room (generated room names are not searched), by a kind word (`crypt`,
`crypts`, `mine`, `sewer`, `lava tube`, `keep dungeon`…: every site of that kind) or a theme word (`ice cave`, `temple`,
`wizard lair`, `tomb`…), and by the names of the NPCs and plots and the words of the notes on them, their levels or rooms.
Each hit gives `site_kind`, `theme` (key) and `theme_name`, `levels`, `size`, the entrance as `x_ft`/`y_ft`, `settlement`
and `site` (`{id, name}`), `match: {what, id, text}` (`what`: name, level, room, npc, plot, note, kind or theme; `id`
the `l:`/`r:` to open at; in the sewers also the section's middle `x_ft`/`y_ft`), `matches` when more than one did, and
`attached` (every note, NPC and plot on the site: `{what, id, at, name|text}`). Order: an exact name first, then by what
matched (as listed), then nearest `at`. `levels` is the site's depth as generated (a designed site's as drawn; a keep's deep
dungeons from the keep's footprint, a designed keep's may differ; sewers 2). `size`: a site's grid size (small to huge,
as generated); deep dungeons medium (2 levels) or large (3); sewers by the 480-ft sections with a street grate (under 10
small, under 40 medium, under 120 large). The first such search lays out every settlement (a few seconds on a large world;
later searches reuse the index).

| Parameter | Type | Default | |
|---|---|---|---|
| `query` | string | `""` | Part of a name. Empty, together with `kind` or `tag`, lists every feature of that kind or tag. |
| `kind` | string | none | Only this kind: `city`, `ruin`, `river`, `building`, `district`, `tower`, … |
| `tag` | string | none | Only features whose notes carry this tag (any case): the DM's `hook`, `secret`… |
| `in_notes` | boolean | false | The query also matches the words of a feature's notes (not only its name); needs a `query`. |
| `within` | box, polygon or circle | none | Only features with a point inside the area (`inside: "point"`) or an extent overlapping it (`inside: "extent"`: a river or road that runs through it, a region that covers part of it; area extents are tested at up to 400 sample points, so a sliver narrower than a map cell can be missed). A box is `[x0, y0, x1, y1]`, a polygon `[[x, y], ...]`, a circle `{at, radius_ft}` (as `list {what: "objects"}`). Hits come nearest the area's middle first. Combines with `query`, `kind` and `tag`; districts and businesses still need a query of three letters (an empty `kind: building` or `district` search without one says so in `note`). A circle's `radius_ft` must be more than 0. |
| `at` | place | none | Only features within `radius_ft` of this place (an id, `{x_ft, y_ft}` or `[x, y]`), nearest first: a circle `within`. Not with `within`. |
| `radius_ft` | number | 26400 | With `at` (5 mi); more than 0. |
| `underground_kinds` | array of strings | none | Only sites underground of these kinds: `dungeon`, `crypt`, `catacombs`, `cave`, `mine`, `lava tube`, `sewer`, `keep dungeon` (keys with `_` too). Lists only sites underground. |
| `levels_min` | integer | none | Only sites underground at least this many levels deep (lists only sites underground). |
| `levels_max` | integer | none | At most this many levels; not less than `levels_min`. |
| `limit` | integer | 25 | 1 to 200 |
| `include_hidden` | boolean | false | Also return hidden features. |

```json
{ "name": "search", "arguments": { "query": "crypts", "at": "city:bab44567", "radius_ft": 100000 } }
{ "name": "search", "arguments": { "underground_kinds": ["cave", "mine"], "levels_min": 3 } }
{ "name": "search", "arguments": { "query": "gate" } }
{ "name": "search", "arguments": { "query": "", "kind": "volcano" } }
{ "name": "search", "arguments": { "query": "Prancing", "kind": "building" } }
{ "name": "search", "arguments": { "tag": "hook" } }
{ "name": "search", "arguments": { "query": "cult", "in_notes": true } }
{ "name": "search", "arguments": { "kind": "ruin", "within": { "at": "city:bab44567", "radius_ft": 150000 } } }
```
> *"Find every inn called something with 'Pony'."* · *"List all the volcanoes."* · *"Is there a town named Wolfbarrow?"*

### `get`
One feature in detail. What comes back depends on the feature:

| For | Fields |
|---|---|
| Any feature | `name`, `kind`, `detail`, position, elevation, `notes`, `hidden` |
| Any place | `in`: the regions it lies in (continent, range, forest…), and `nearest_settlements` |
| Settlements | `members` (drawn things that belong to the town: `id`, `kind`, `name`, `why`: `gap` or `hand`) and `pushed_out` (`place`, `name`, `buildings`) when there are any; roads to other settlements, with road miles; districts; notable buildings; ways underground; flights of steps on steep streets as generated (`stairs`: each flight's middle `x_ft`/`y_ft`, `length_ft`, `rise_ft`, the highest first, up to 40; cities and up); `sites`: the sites in it (`id`, `kind`, `name`, and a drawn building's `building` id) |
| Sites | The layout: buildings, and ways underground with their `u:` ids; `settlement`: the town it stands in, or `null` |
| Buildings and underground sites | Levels and rooms |
| Buildings | `roof_material`, `roof_shape` and `roof_shade` (as `set_building` takes them; none on battlements or ruins), `footprint`: `poly` (corners in order, ft), `angle_deg` (its long side, clockwise from east, 0–180), `length_ft`, `width_ft`, `area_sq_ft`, and a castle's keep its `pieces` (`kind`: body, round tower, square tower, forebuilding, wing; `poly`), a temple its own (body, transept, apse), whose union `poly` is (drawing a new outline makes it one piece); `structure`; `edited` when changed by hand; a building taken away (`remove`) comes back as `removed` with what it was |
| Anything in a layout | `settlement` (and in a site's layout `site`): see Conventions. A building drawn by hand (`b:<layout>:0`) used to give its own site as `settlement`; it now gives the town it stands in (or `null`), and its site as `site` |
| Places with NPCs or plots | `npcs`, `plots`: those at the place, at anything in its layout, at their levels and rooms; for a settlement also at its sites |
| A building drawn by hand | `notes`, `hidden`, `npcs` and `plots` are the same under its `c:` id and its building id |
| A road stretch (`rd:`) | `kind: "road"`, `class`, `length_mi`, `from`/`to` (as `list {what: "roads"}`), `name` (renamed, or the drawn road's; else `null`), `renamed: true`, `drawn_road` `{id, name}` when it is a piece of a road drawn in the sketch, `line` (at most 40 points, ft), position halfway along, `notes`, `hidden`, `nearest_settlements` |
| A crossing (`v:`) | `kind: "crossing"`, `crossing` (bridge, ford, ferry), ends, width, length, `material`, `name` (as given, else `Timber bridge`, `Stone bridge`, `Ford`, `Ferry`), `generated`, `notes`, `river` (as `set_crossing` gives it) |

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |

```json
{ "name": "get", "arguments": { "id": "metropolis:6eee9055" } }
{ "name": "get", "arguments": { "id": "c:2" } }
```
> *"Tell me everything about Agentholm."* · *"Which roads leave Gatewatch, and how long are they?"*

### `get {view: "children"}`
What a feature contains:

| Feature | Children |
|---|---|
| Settlement | Its districts, businesses (buildings drawn in it with a trade too, as `b:<layout>:0`) and ways underground; `sites`: the sites in it (`id`, `kind`, `name`, a drawn building's `building` id, and `underground`: their ways underground) |
| Site | Its businesses and ways underground |
| District | Its businesses |
| Building or underground site | Its levels, with rooms and hazards |
| Region | Its settlements and sites |

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `limit` | integer | 200 | 1 to 2000; each list is cut to it (`found` has the full counts of those cut, `truncated: true`). |

```json
{ "name": "get", "arguments": { "view": "children", "id": "city:bab44567" } }
{ "name": "get", "arguments": { "view": "children", "id": "u:312:0" } }
```
> *"What shops are in the market district?"* · *"Walk me through the dungeon under the Sunken Bastion, level by level."*

### `list {what: "names"}`
Everything that can be renamed, with ids to pass to `set_feature`. Each entry has `id`, `kind`, `name` (as
renamed), `generated` (the name before renames), `x_ft`/`y_ft` where it has a spot, `enter: true` for buildings and
sites with levels, and `hidden: true` for hidden features. Returns `{items, found}`. A `kind` nothing has is refused
with a "did you mean" and the kinds there are (`kind: no names of kind "sitee"; did you mean "site"?`). Road stretches
are listed only once renamed (kind `road`; `list {what: "roads"}` gives every stretch's id); crossings put down by hand are all
listed (kind `crossing`).

| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | optional | Only this kind: `river`, `range`, `city`… or `district`, `building`, `tower`, `underground`, `site`, `level`, `room`. |
| `settlement` | string | optional | A settlement's or site's id: its districts, businesses, wall and gate towers and underground sites (sewer grates aside). For a settlement, also the buildings drawn in it (as `b:<layout>:0`: businesses with their `function`, homes as `house`) and every site in it (kind `site`, by the site's own id). |
| `inside` | string | optional | A building's or underground site's id: its levels (`l:`) and their rooms (`r:`). |
| `function` | string | optional | Only businesses of this function, by `set_building`'s `func` keys (`inn`, `blacksmith`, `temple`…); needs `settlement`. Each building entry also says its `function`. An unknown one is refused with a "did you mean". |
| `limit` | integer | 200 | 1 to 2000 (`truncated: true` when it cut the list). |

Without `settlement` or `inside` it lists every named feature: oceans, seas, rivers, ranges, settlements, sites
and created sites.

```json
{ "name": "list", "arguments": { "what": "names", "kind": "river" } }
{ "name": "list", "arguments": { "what": "names", "settlement": "city:bab44567", "kind": "district" } }
{ "name": "list", "arguments": { "what": "names", "inside": "u:312:0" } }
```
> *"Give every room in the crypt under the Sunken Bastion a proper name."* · *"Which rivers have I renamed?"*

### `inspect`
What is at a point:
- `elevation_ft` and `under_water`: the 5-ft square at the point as the battlemap draws it (shaped by hand included; `inspect {what: "heights"}` has the squares round it), and the biome. `biome` is the coarse map's and `elevation_ft` / `under_water` the battlemap's, so a dry point near the sea can read `biome: ocean`;
- `in`: the areas it lies in, largest first (continent or island, sea, range, forest, lake…);
- the building, district or site there (a building's `district` is its name as renamed, `district_id` its id; a
  building of a site standing in a settlement, such as one drawn by hand, has the settlement's district there);
- `nearby`: named features within a tenth of their size (2 to 10 miles), nearest first, measured to their nearest
  edge or course, with `distance_mi` and `direction`;
- `slope_deg`: the slope on the battlemap's squares over 40 ft (20 ft either side);
- `water`: a river within 1,000 ft (`{kind: "river", id, name, width_ft, banks: [[x, y], [x, y]] (the bank on the point's side first), distance_ft (to the water's edge; 0 in it), in_water}`), or a lake or the sea (`{kind: "lake" | "sea", id?, name? (the smallest named body there), in_water, distance_ft, shore? ({x_ft, y_ft}; in the water, `shore_ft` is how far it is)}`), whichever is nearer; null where there is none. `in_water` is the battlemap's square at the point (a bank point from `nearest river` or `banks` is on the water's edge, wet or dry by a foot or two); a lake's or the sea's edge is found on samples 20 ft apart and placed on the 5-ft squares;
- `coast`: the nearest coast of the sea within 5 miles, found on the battlemap's ground as `nearest` `coast` (`{distance_ft, x_ft, y_ft, shore: "cliff" | "beach", rise_ft, inland_ft, in_sea}`); null where there is none;
- the nearest settlements;
- `things`: everything within `radius_ft` of the point, drawn by hand or generated, topmost first (what lies under
  the point before what is only near it; a way underground, then a crossing or harbour piece, a building or tower, a
  wall or fence, a road, street or lane, a hedge, water, a castle or plot, a district, a settlement or site), then
  nearest. Each is `{kind, id, name, town, removable, bbox}`: `town` the feature id of the settlement it belongs to
  (null where none), `bbox` its extent `[x0, y0, x1, y1]` in world ft, `removable` true where `remove` takes it.
  Kinds and ids: `entrance` (`u:`, a sewer grate `w:`), `building` (`b:`; removable when it is the world's own,
  not a castle's or a plot's), `tower` (`t:`), `drawn_building`, `castle`, `plot`, `wall` (`c:`), `settlement`,
  `site` (their feature ids; removable when created), `district` (`d:`), `road` (a road stretch `rd:`, a track to
  a settlement added by hand `mt:`; removable), `street` / `lane` (a generated town's or village's `tr:`, one added
  by hand `ts:`; removable), `path` (a park's, garden's or cemetery's walk `pp:`; removable), `drawn_road` (`rw:`),
  `crossing` (`v:`), `water` (`e:`), `harbour` (`q:`), `fence` (a generated settlement's fence, hedge or wall run
  `fr:`, within 1,500 ft; a park's hedge or row of topiary `fh:`, within 400 ft; removable), `drawn_fence` (`f:`)
  and `fence_gap` (`fg:`); a fence's `fence` names its kind (`hedgerow`, `picket`…; none for topiary or a gap).
  With `id: null`: a hand town's streets, a place's lanes, the fences of a hand town or a site. Roads, streets and
  paths taken away with `remove`, and fences with `remove`, are left out.

| Parameter | Type | | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `radius_ft` | number, 0 to 500 | default 15 | how far `things` reaches |

```json
{ "name": "inspect", "arguments": { "at": [2310194, 4127619] } }
```
> *"What's at 437 miles east, 781 miles south?"* · *"What's right around the Ruins of Neling?"*

### `search` round a place
With `at` and none of `query`, `kind`, `tag`, `in_notes`, `within`, `underground_kinds`, `levels_min` or `levels_max`:
everything named within `radius_ft` of a place, nearest first. Distances are to each feature's nearest edge or
course (0, direction `here`, inside an area). Returns `{items, found}` (`truncated: true` when `limit` cut the list); each item has `distance_mi`, `direction` and `extent`.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `radius_ft` | number | 26,400 (5 mi) | 50 to 2,640,000 |
| `kinds` | string array | all | Only these kinds (only here: a search by query takes `kind`). Districts and businesses come when `kinds` names `district` or `building`, or with no `kinds` within a mile. |
| `limit` | integer | 50 | 1 to 500 |
| `include_hidden` | boolean | false | |

```json
{ "name": "search", "arguments": { "at": [2310194, 4127619], "radius_ft": 79200, "kinds": ["river", "lake"] } }
{ "name": "search", "arguments": { "at": "city:bab44567", "radius_ft": 2640, "kinds": ["building"] } }
```
> *"Which rivers and lakes are within 15 miles of here?"* · *"What's the lake just north of the capital called?"*

### `route`
The distance between two places:
- `by_road`: road miles (plus any stretch off the road), the settlements passed on the way, the `stretches` taken in
  order (`{id, class, length_mi, name?}`: `rd:` ids, a name where one was given or the stretch is a drawn road's) and
  `follows`: the named roads followed, runs of stretches of one name joined (`{name, length_mi}`); a long way lists its
  first 50 `stretches` with `stretches_found` and `truncated: true` (`follows` stays whole);
  a road way over 1.5 times the straight line has a `note` saying how many times;
- `straight_mi`: the distance as the crow flies, and its `direction`;
- `travel`: D&D 5e travel days at normal (24 mi/day), fast (30) and slow (18) pace.

| Parameter | Type | | |
|---|---|---|---|
| `from` | id, place name, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | A name matches a place's current name, whole and in any case (features, districts, businesses, renamed roads and crossings). One place of that name is used, and the reply says which (`from_place: {id, name, kind}`); several are refused, listing up to 5 with their ids and kinds; none is refused with a did-you-mean. |
| `to` | id, place name, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | As `from` (`to_place`). |

```json
{ "name": "route", "arguments": { "from": "metropolis:6eee9055", "to": "village:6db3886e" } }
{ "name": "route", "arguments": { "from": "city:bab44567", "to": { "x_ft": 1200000, "y_ft": 2500000 } } }
```
> *"How many days does it take to ride from Agentholm to Geirgai at a fast pace?"* · *"What's the route from Gatewatch to the capital, and which towns does it pass?"*

### `nearest`
The nearest coast, cliff, river, road or flat ground, or the highest ground, to a place, within `radius_ft` (default 26,400 = 5 mi, at most 132,000 = 25 mi): a
bounded search, never a scan of the whole map. Returns `{what, found, x_ft, y_ft, distance_ft, distance_mi, direction}`
and, by what, one more; `found: false` (not an error) where there is none within the distance. Coast, cliff and flat
are answered on the battlemap's own ground (the refined terrain tiles; 5-ft squares as `inspect {what: "heights"}` reads them,
shaped by hand included): the world's coarse grid (one cell is about 1.2 miles on a 1,200-mile map) only says where to
look. A first call in a new area takes up to about 0.3 s (the tiles are made), a second one there a few ms. Rivers and
roads are found on their own lines. Definitions (defined once, `worldgen/src/geoquery.rs`):
- `coast`: the sea's shore nearest the point (a point at sea gets the nearest land's edge, `in_sea: true`): searched on
  samples 80 ft apart about the point and about the coarse coast (out to 3 cells from it, as far as the coarse coast can
  be off on a steep coast), then placed on the squares. The point is the last dry square, with the sea's squares beside
  it (a river or lake square is never the coast point). `shore` is `cliff` where the land stands 15 ft or more above the water within 20 ft of it, else `beach`; `rise_ft`
  is that height, `inland_ft` the highest within 100 ft. Lakes are not coasts (`inspect`'s `water` has them).
- `cliff`: a drop of 15 ft or more within 20 ft on the squares (about 37 degrees or steeper; the top dry, the foot at its
  ground or at the water over it). Looked for in the coarse cells steeper than 12 degrees within reach (the two nearest
  and the two steepest, scanned on samples 40 ft apart) and along hand-made plateaus (`set_ground {kind: "plateau"}`), then measured on
  the squares at the steepest few spots, and at hand-brushed blocks (`set_ground {kind: "brush"}`): so the nearest of those, not every cliff. The search is not exhaustive: where none is found the `note` says how many steep spots were measured. Returns the top (`x_ft`, `y_ft`,
  `top_ft` its height above the sea), `foot`, `drop_ft`, `run_ft` and `slope_deg`. Where none is found, `note` says so
  and `steepest` gives the steepest ground measured (`{x_ft, y_ft, rise_ft, distance_mi}`, rise within 20 ft). Natural
  cliffs are rare (mountain faces); for a sheer one, make it with `set_ground {kind: "plateau"}` (`edge_ft: 0`) or `set_ground {kind: "brush"}`.
- `river`: the nearest river's water: `id`, `name`, `width_ft`, `banks` (the one on the point's side first), the point
  (`x_ft`, `y_ft`) on that bank (the point itself when it is in the water, `in_water`), `distance_ft` to the water.
- `road`: the nearest point of any road: the stretch `id` (`rd:`), `name` if it has one, `class`, `heading_deg`
  (clockwise from north, as in `road_point`).
- `flat`: a 200-ft square patch (`patch_ft`) of dry squares whose heights differ by 10 ft or less (`range_ft`), its
  middle the point. Candidates are the gentle points (under 5 degrees on the coarse grid) in and round the nearest coarse
  cells gentle with their four neighbours, nearest first, after the tops of hand-made plateaus and the middles of hand-brushed blocks; at most 12 are measured on the squares (the search is not exhaustive: the `note` of a `found: false` says how many were, or that no gentle ground was in reach to measure). The whole patch must be on the map. `slope_deg` is the
  patch's mean slope, `elevation_ft` its middle's. Distance 0 where the point is on one; where the candidates measured
  are all rougher, `found: false` with a `note` saying how rough.
- `highest`: the highest dry ground within `radius_ft` of the place: the ten highest coarse cells in reach are scanned on samples
  80 ft apart, the highest few (and the point itself, hand-made plateaus and brushed blocks) measured on the squares. Returns the
  top's point, `elevation_ft` above the sea and `rise_ft` above the place (0 where the place is the highest). A first call in a
  new area is a fraction of a second, a second one there a few ms.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `what` | string | required | `coast`, `cliff`, `river`, `road`, `flat` or `highest` |
| `radius_ft` | number | 26,400 | 528 to 132,000 |

```json
{ "name": "nearest", "arguments": { "at": "city:bab44567", "what": "coast", "radius_ft": 132000 } }
{ "name": "nearest", "arguments": { "at": [2310194, 4127619], "what": "flat" } }
{ "name": "nearest", "arguments": { "at": "city:bab44567", "what": "highest", "radius_ft": 10560 } }
```
> *"How far is the nearest cliff from the keep?"* · *"Where is the closest flat ground to build a camp?"*

### `road_point`
A point along a road, for placing things "on the Trade Way" or a day's ride down a route. Give `road` (an `rd:` id, or a
road's name: a drawn road, or stretches renamed alike and joined end to end; names match whole and in any case) and
optionally `from`, or `from` and `to` for the road path `route` takes between them. Then `at_mi` (miles along it from
the start) or `fraction` (0 to 1), one of the two. With `road`, the start is the road's first end, or the end nearer
`from` (a place near one end; a name works as in `route`, and is echoed as `from_place`). Returns `x_ft`, `y_ft`,
`heading_deg` (the way it runs there, clockwise from north: 90 is east), `at_mi`, `fraction`, `stretch` (`{id, class, name?,
from, to, length_mi}`: the stretch there with its ends, as `list {what: "roads"}`) and `road` (`{name?, length_mi, stretches, from,
to}`). Lengths are the stretches' own (`list {what: "roads"}`' `length_mi`, chord lengths: a long stretch's line drifts some way
off the drawn road); the point is then moved onto the road as the battlemap carves it, within 60 ft
(`moved_onto_road_ft` says how far), or carries a `note` where no carved road is that near. A point
past the road's length is refused, saying the length. A name for several separate roads is refused, listing a stretch of
each; no road by that name is refused with a did-you-mean; a `route` with no road between its places is refused
(its `by_road` is null). Several roads share a name only if you named them so.

| Parameter | Type | | |
|---|---|---|---|
| `road` | string | | An `rd:` id or a road's name (not with `to`) |
| `from` | id, place name, `{x_ft, y_ft}` or `[x_ft, y_ft]` | | With `road`: the end to start from (default the first); without: where the route starts |
| `to` | id, place name, `{x_ft, y_ft}` or `[x_ft, y_ft]` | | Where the route ends (instead of `road`) |
| `at_mi` | number | | Miles from the start; 0 or more |
| `fraction` | number | | 0 to 1 of the length |

```json
{ "name": "road_point", "arguments": { "road": "The Trade Way", "from": "city:bab44567", "at_mi": 12 } }
{ "name": "road_point", "arguments": { "from": "city:bab44567", "to": "village:6db3886e", "fraction": 0.5 } }
```
> *"Put a waystation 12 miles down the Trade Way from the capital."* · *"Where is the midpoint of the road between these towns?"*

### `list {what: "roads"}`
The road network:
- `named`: the roads drawn and named in the world's sketch, each with its id, name, class, length, the
  settlements along it, in order, and its `stretches` (`rd:` ids); then the stretches renamed alike, one entry per
  name: `{name, stretches (in order along it), class, length_mi, settlements}`;
- `items`: the roads between settlements and junctions (stretches), longest first, each with its `id` (`rd:`),
  `class` (king's road, road, track), `length_mi`, its ends `from` and `to` (a settlement's `id` and `name`, or a
  junction's `x_ft`/`y_ft`) and `name` (renamed, else the drawn road's); a drawn road's pieces say which (`road`:
  its name, and `road_id`; or `drawn: true`); `removed: true` on a stretch taken off the map (`remove`; routes
  still use it and mark it so);
- `found`: how many roads matched (`truncated: true` when `limit` cut the list).

| Parameter | Type | Default | |
|---|---|---|---|
| `within` | a circle `{at, radius_ft}` | none | Only roads near it (`radius_ft` up to 15,840,000). |
| `settlement` | id string | none | Only roads ending at this settlement. |
| `class` | `king's road` (also `kings road`, `kings_road`), `road` or `track` | all | Results always say `king's road`. |
| `drawn` | boolean | false | Only roads drawn in the sketch. |
| `limit` | integer | 50 | 1 to 1000 |

```json
{ "name": "list", "arguments": { "what": "roads", "settlement": "city:bab44567" } }
{ "name": "list", "arguments": { "what": "roads", "drawn": true } }
{ "name": "list", "arguments": { "what": "roads", "within": { "at": [2310194, 4127619], "radius_ft": 211200 }, "class": "kings_road" } }
```
> *"Which roads leave Bladegarden, and where do they go?"* · *"Which towns does the Gilded Roadway pass through?"*

**Naming a road.** A road across country is several stretches. To name the High Road from one city to another:
`route` between them, then one `set_feature` per `rd:` id in `by_road.stretches` (in one `batch`), all with the
same name. `list {what: "roads"}` then gives it as one entry in `named`, `route` says it `follows` it, and search finds its
stretches. `name: ""` takes a stretch's name off again. Renaming one stretch of a named road over MCP splits it
(the app renames all of its stretches together): to rename a whole road, use the batch above with every stretch. Only roads someone named carry a name; nothing is generated.

```json
{ "name": "batch", "arguments": { "steps": [
  { "tool": "set_feature", "arguments": { "id": "rd:6eaa3b7c", "name": "The High Road" } },
  { "tool": "set_feature", "arguments": { "id": "rd:28a39f0b", "name": "The High Road" } } ] } }
```

### `inspect {what: "battlemap"}`
The 640-ft battlemap chunk (128 × 128 five-foot squares) that contains a place, for running a fight there. It
lists:
- the elevation range, surfaces and buildings (`stairs`: a flight of steps on a steep city street, paved, walkable,
  no movement cost; also where ground shaped by hand makes a city street climb steeply; `gravel`: a park's paths);
- each kind of object in the chunk, with its 5e rules: cover, whether it blocks movement or sight, difficult
  terrain, hazards and their effects;
- the objects within `radius_squares` of the place (placed objects are read within the one chunk only; `list {what: "objects"}`
  lists every one in the world), by grid square (`near_focus`), each with its centre
  (`x_ft`, `y_ft`) and either its `id` (put down by hand: `set_objects` moves or turns it, `remove`
  takes it; `rot_deg` when it was given a turn) or its built-in `kind` (generated: `remove` takes it by
  `at`);
- the clears in the chunk (`cleared`: id, centre, kind or radius; `restore` takes them).

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `radius_squares` | number | 12 | 1 to 64 |

```json
{ "name": "inspect", "arguments": { "what": "battlemap", "at": "c:2", "radius_squares": 20 } }
```
> *"Set up an ambush at the Sunken Bastion: where's the cover, and are there any hazards?"* · *"What terrain is on the bridge outside Gatewatch?"*

---

## View tools

These need the map app open and connected. If it isn't, the tool says so.

### `render_view`
A screenshot of the map, as the app draws it: a JPEG 1,024 px across (683 high) by default, a fifth of the size of the
old 1,536 px PNG. The app loads everything at that place first, then puts your view back where it was. Waits up to 45 s;
it needs the app open in a visible tab (a hidden one does not draw) and the place generated, so if it times out, wait a
few seconds and ask again, or try a larger `size_ft`.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `size_ft` | number | 3000 | The ground across the image (50 to 5,000,000). |
| `level` | integer | none | With `at` a building or site id (`b:`, `u:`, `k:`): its inside on this level, counted from the bottom. An `l:` or `r:` id in `at` shows that level of its site (floor plan, doors, locks, furniture) with no `level` needed; the map goes back as it was. |
| `width_px` | integer | 1024 | Pixels across, 256 to 1536; the height is two thirds of it. |
| `format` | string | `jpeg` | `jpeg` (quality 0.85) or `png` (exact lines; larger). The reply's `mimeType` follows it. |

What each `size_ft` shows:

| `size_ft` | Shows |
|---|---|
| ≤ 450 | The painted battlemap with its 5-ft grid |
| ~3,000 | A village, or a town's streets |
| ~20,000 | A city and its fields |
| ~200,000 | A region |
| ~5,000,000 | The whole continent |

```json
{ "name": "render_view", "arguments": { "at": "c:2", "size_ft": 400 } }
{ "name": "render_view", "arguments": { "at": "range:e48b3d59", "size_ft": 200000 } }
```
> *"Show me the battlemap at the ruin."* · *"What does the land around the Katsei Mountains look like?"*

### `focus_view`
Flies your map view to a place. You see it happen.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `size_ft` | number | 3000 | How much ground fits on screen (50 to 5,000,000). |

```json
{ "name": "focus_view", "arguments": { "at": "city:bab44567", "size_ft": 20000 } }
```
> *"Take me to Gatewatch."* · *"Show me where the party is: the crossroads south of Bulol."*

---

## Edit tools

Each edit is:
- saved to `worlds/<hash>/world.json`;
- logged in `edits.jsonl`, as made by `agent`;
- sent to the open app, where it shows a notice with **Undo**.

Edits are layered over the generated world; the generated world itself never changes. Tools that take an `id`
check that it exists first, so a typo can't leave a stray edit.

If saving fails (the file is held open elsewhere), the tool reports it, but the change stands: mapd keeps it in
memory and saves it again every few seconds.

### `batch`
Many edits as one change: `steps` is a list of `{ "tool": …, "arguments": { … } }`, run in order (up to 5,000). Any
edit tool can be a step (and read tools too); `batch`, `render_view` and `focus_view` can't. Each step sees what
the steps before it did: a site created in one step can be renamed or annotated in the next (`c:<n>` ids count up
from the highest ever made. Deleted sites keep their number, so `world_overview`'s `created_sites`, which counts the
live ones, can be too low: when unsure, create in one call and read the id it returns). The whole batch is saved once, logged as one entry and shown in the
app as one notice. If a step fails, nothing is changed, and the error names the step. Returns each step's result.

```json
{ "name": "batch", "arguments": { "world": "b3a8412f4fa3a3b1", "steps": [
  { "tool": "set_feature", "arguments": { "id": "d:4:2", "name": "Tri-Spire Ward" } },
  { "tool": "set_feature", "arguments": { "id": "d:4:2", "notes": "Three wizard towers lean over its lanes." } },
  { "tool": "set_site", "arguments": { "kind": "ruin", "under": "crypt", "at": [2325000, 4127000] } },
  { "tool": "set_feature", "arguments": { "id": "c:0", "name": "The Evening Nip" } }
] } }
```
> *"Name every district in the capital and give each a line of lore, all in one go."*

### `set_feature {id, name}`
Renames anything with a name: a feature, building, district, tower, underground site, a site's level (`l:`) or
room (`r:`), a road stretch (`rd:`, ids from `list {what: "roads"}` or `route`), a crossing put down by hand (`v:`), a city's whole sewers (`w:<layout>`) or one 480-ft section of them (`w:<layout>:<sx>:<sy>`), or a plateau (`h:`; its name shows in `list {what: "ground"}`). Use
`list {what: "names"}` for the ids. The app shows the new names everywhere, including the level list and
the room labels inside. A building drawn by hand is one place, whether it is named by its `c:` id or its
building id (`b:<layout>:0`): one name, one note (kept under `c:`) and one hidden mark.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | required | An empty string removes a rename and restores the generated name. |

```json
{ "name": "set_feature", "arguments": { "id": "city:bab44567", "name": "Gatewatch" } }
{ "name": "set_feature", "arguments": { "id": "b:0:57", "name": "The Gilded Griffon" } }
{ "name": "set_feature", "arguments": { "id": "r:u:312:0:1:4", "name": "Hall of the Drowned King" } }
{ "name": "set_feature", "arguments": { "id": "city:bab44567", "name": "" } }
```
> *"Rename the capital to Agentholm."* · *"Call the biggest tavern in Bulol 'The Drunken Kraken'."* · *"Put Fyckmere's old name back."*

### `set_feature {id, notes, tags}`
Sets a feature's notes: lore, adventure hooks, secrets for the DM. The app shows them in the info panel. A
building drawn by hand keeps its note under its `c:` id, whichever id is given (the reply's `id`); a note left
under its building id is joined onto it. A city's sewers take notes too, as a whole (`w:<layout>`) or by
section (`w:<layout>:<sx>:<sy>`); `set_feature` refuses them.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | |
| `notes` | string | one of `notes`, `tags`, `remove_tags` | Left out, the notes stay as they are; the reply echoes `notes`. |
| `tags` | string[] | none | Added to the existing tags. `[]` alone (no notes, no non-empty `remove_tags`) is refused: "at least 1 item". |
| `remove_tags` | string[] | none | Taken off; one that is not there is fine. The reply's `removed_tags` says which were, and `tags` what is left. (`set_feature` with `tags: []` clears them all.) |
| `append` | boolean | false | Add to the existing notes instead of replacing them. |

```json
{ "name": "set_feature", "arguments": { "id": "c:2", "notes": "The night watch vanishes one by one; the trail leads here.", "tags": ["hook"] } }
{ "name": "set_feature", "arguments": { "id": "c:2", "notes": "Secret: the baron funds the cult.", "tags": ["secret"], "append": true } }
```
> *"Write a short history for every ruin in the Emberwood."* · *"Add a rumour to the Gilded Griffon that the innkeeper is a retired assassin."*

### `set_site` (a new site)
Makes sites, castles and walls (not buildings: `set_building`; not towns or settlements: see "What agents can't do").
Adds a new site, built by the same generators as the world's own: map symbol, buildings, battlemap, interiors,
and a full multi-level site underground (a ruin's dungeon, crypt or catacombs; a cave, mine or lava tube; or an
`entrance`, a bare way down with nothing built above it). The result includes the new `c:<n>` id and its
underground ids.

Creating the same site again adds nothing: if a live site of the same `kind` (and the same `under`) stands within
300 ft of the place asked for, or of the spot the new one would take, the result is that site, with
`"existing": true` (and `note` says it was already there, not created). A script that creates sites can be run again
safely; for a second one on purpose, ask for it somewhere else.

A site does not go on top of what stands: each kind keeps a disc round its point clear of buildings (the world's, a castle's, drawn ones, other sites'), walls
and, but for an `entrance`, streets and squares (a bare `entrance` may sit in a street or a square; one `in_building` skips the check). The disc is
65 ft for a `tower`, 55 for a `camp` and a `mine`, 120 for a `waystation`, 115 for a `ruin`, 12 for a `cave` and a `lava_tube`, 10 for an `entrance`.
The refusal names what is in the way and where, and the nearest clear spot: `that overlaps Dragonspear Castle (c:5), at (x, y); the nearest clear spot
is (x, y), 140 ft away`. In a built-up area with none within 400 ft, mapd looks on to 1,500 ft (a few seconds) and says where the nearest is, or that there is none (put it outside the built-up area); for a `tower` it adds that `set_building` with `func: wizard_tower` and `clear: true` makes one there. The same-site rule above comes first: asking again for a site that stands is that site, not a refusal.

On a rock islet (`set_ground {kind: "islet"}`) a `tower`, `castle` or `wall` stays where it was asked, with no `moved`.

A site asked for in a river or within 150 ft of one is put on the bank (a camp's clearing is also kept inside its
battlemap chunk). When that moves it more than 5 ft the result says so: `moved: {x_ft, y_ft, ft, why}`.

| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | required | `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube`, `entrance`; `castle`, `wall` (drawn by their shape: see *Castles and walls* below) |
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | Where to put it. With an `id`, the site goes at that feature's position. |
| `name` | string | optional | If left out, it is named in the local culture's style. |
| `under` | string | optional | What lies beneath: for a ruin `dungeon`, `crypt` or `catacombs`; for an entrance any of those or `cave`, `mine`, `lava_tube` (default `dungeon`). |
| `size` | string | optional | The site underground: `small`, `medium`, `large`, `huge`. |
| `levels` | integer | optional | Levels underground, 1 to 6. |
| `theme` | string | optional | The site's theme, one of its kind's (below). |
| `in_building` | string | optional | An `entrance` only: a building's id (`b:<layout>:<n>`, or the `c:` id of a drawn one). The way down is in that building's cellar and `at` is not needed. |
| `way` | string | optional | With `in_building`: `stairs` (default) or `trapdoor`. |

An entrance with `in_building` goes down from the cellar of a roofed building that has one (not a ruin, an open structure or a yurt): the cellar gets the stairs or trapdoor, the dungeon below them the way back up, and the surface shows no opening. The dungeon takes the entrance's name. One building has one entrance of each kind beneath. A building with a hand-drawn design that has no cellar level or no free square on it is refused (`set_design`: add a cellar or free a square), nothing changed. The reply's `in_building` has `cellar_level` (the id of the level with the stairs), and `moved` when `at` was given and is not the building's middle. A name another feature already has is made anyway, with a `note` (`another "X" exists: c:14`). Asking again for the same building and kind is that entrance (`existing: true`). `set_feature` moves an
entrance into a building, or out, keeping its ids.
`get` of a created site lists its `underground` ids; of an entrance in a building, `in_building` (`{id, name}` (the name as `get` gives the building, kept after it is taken away), with `standing: false` and a note
when the building is gone, ruined or has no cellar: the entrance then opens on the ground) and `way`; of a building, its `underground` ids. A change that
takes a building away or leaves it without a cellar (`remove`, of a world or a drawn one, `set_building` with `structure: ruin`) lists
the entrances that stood in it as `entrances_out: [{id, name, now}]`; restoring the building brings the way down back.

A `camp` is tents round a campfire in a levelled clearing (bedrolls, firewood, a cart with its stores), anywhere;
a `waystation` is an inn with stables set back from the nearest road. A camp is nudged up to 70 ft so its clearing
lies within one battlemap chunk.

Anything left out of `size`, `levels` and `theme` is chosen as for the world's own sites. A theme left out is never `ice` except on snowy ground (Ice, Alpine, Tundra and high Taiga), where a cave in the Ice biome is always ice; asking for `ice` works anywhere. Towers, camps and inns
have nothing underground, so they take none of them. Themes by kind of site:

| Kind | Themes (the first is the usual one) |
|---|---|
| dungeon | `dungeon`, `prison`, `temple`, `wizard_lair`, `bandit_hideout`, `dwarven_hall`, `goblin_warren`, `flooded_vault`, `cult_sanctum`, `labyrinth`, `collapsed_fortress`, `sunken_temple` |
| crypt | `crypt`, `tomb`, `ossuary` |
| catacombs | `catacombs` |
| cave | `cave`, `fungal`, `crystal`, `ice`, `flooded`, `beast_den` |
| mine | `mine` |
| lava_tube | `lava_tube` |

A theme sets the rooms (a prison's guard ring, cell blocks and warden's office, a dwarven hall's forges and great hall), how many
there are, corridor width and the floor. Every site, whatever its theme, size and depth, ends in one boss chamber on
its deepest level, and every square is reachable. A `labyrinth` is a maze of 5-ft passages with the stairs down
(or the boss) in its heart room; a `collapsed_fortress` is a fort's halls with caverns breaking in, more the deeper (the
world's own sites take it only under ruins). Old sites cave in: under ruins about half the levels, under living towns,
keeps and cities about one site in ten, a cavern breaks across the rooms, joined to them by open gaps (`arch` in a
plan's doors) with rubble; created and designed sites never do, nor labyrinths, caves, mines and lava tubes.

Rules:
- the place must be on the map and on dry land;
- a spot in or on the banks of a river channel is moved just clear of it.

```json
{ "name": "set_site", "arguments": { "kind": "ruin", "under": "crypt", "at": [1158199, 2720810], "name": "The Sunken Bastion" } }
{ "name": "set_site", "arguments": { "kind": "waystation", "at": [3700000, 1650000] } }
{ "name": "set_site", "arguments": { "kind": "entrance", "under": "dungeon", "theme": "dwarven_hall", "size": "huge", "levels": 5, "at": [2400000, 1900000] } }
{ "name": "set_site", "arguments": { "kind": "tower", "at": [4600000, 2050000], "name": "Spire of Vael" } }
```
> *"Put a ruined watchtower with a crypt under it two miles north of Gatewatch."* · *"Hide a small two-level goblin warren in the hills east of Bulol."* · *"Add a roadside inn halfway between Agentholm and Bulol."* · *"Make a bandit camp in the Emberwood."*

### Castles and walls
`set_site` with `kind: "castle"` or `"wall"` takes a shape instead of a place. A **castle** is an outline
(`poly`, 3–32 corners at least 10 ft apart, turning inward or not but never crossing itself, 80 to 1,500 ft across,
measured as the long side of the box that fits it closest: a `rect` of 1,400 × 1,000 is 1,400 across; no neck under
30 ft across, where its curtain would come apart; or `rect` about its middle):
a curtain wall just inside it, a tower on every corner (those turning inward too) and along long runs (round, or
square in some castles), a gatehouse on one side (`gate`: the side from corner `gate` to the next; else the side
facing the nearest road) or on several (`gates`: the sides, up to 8, a gatehouse in the middle of each, the first the
main one; `gates` replaces `gate`; a gate whose spot won't do (water or a building before it, too near another
gate, its towers against others) moves along its side, else is not built and the reply lists it in
`gates_not_built`, `[{side, why}]`), in a big one (80,000 sq ft or more and
200 ft across) an inner curtain with its own gatehouse round the keep's high ground, leaving an outer bailey at
least 48 ft wide all round and the inner curtain at least 80 ft from the outer gate, its gate at least 90 ft from
that one; the keep (the castle itself, named as the site;
`keep: false` for none) on the highest ground that holds it, made of pieces: a great tower with corner towers, a hall
with a forebuilding, an L or a T with towers at its outer corners, or a round shell keep on a motte (a mound eased up
from the ground, a gravel path up it to the shell's gate; a tower keep alone where nothing else fits); `keep_shape`
chooses: `square` (those of pieces, never a shell keep), `round` (a round great tower, as big as the court holds),
`drawn` with `keep_poly` (its outline, 3–32 corners, all of it 12 ft or more inside the castle's, off the curtain's
towers, its gates and the buildings drawn in the yard; `keep_poly` alone means drawn), `auto` back to the roll; its flat tops
drawn as open battlements (a wall walk of stone flags in the merlons, a broad one's middle dark lead) or, every top of
1,100 sq ft or more and in some keeps the smaller ones too, hipped roofs of lead or slate; and ranges of
stone buildings under slate against the inside of the curtain, clear of the way from gate to gate (the biggest the
barracks, then stables and a smithy; `yard_buildings: false` for none). A moat round the curtain comes by roll (keyed
to the outline, so the same castle drawn again under another id keeps it), wet where water lies near and the ground,
as shaped by hand (plateaus, the brush), is level, else a dry ditch; `moat` chooses: `none`, `dry`, `wet`, `auto`
back to the roll; a drawbridge spans it at each gate. The bailey inside is an open yard of grass, no
wild trees or rocks, set out sparingly: stores by the ranges, a cart, a trough or barrels inside each gate, a well in
the keep's court, a little cover out in the open yard (haystacks, carts, troughs, firewood; small stores by a wall or
a building: its curtains, towers and buildings are the yard's cover, so the ground near them needs none), nothing on a
motte or in the way from gate to gate and to the keep; in some castles a garden yard: a plot
by the keep (flowerbeds, topiary at its corners, benches, a short hedge, now and then a fountain) and a few planted
trees. Buildings can be drawn in it (`set_building`): the castle's own yard buildings make room (its keep and
walls do not). With
`structure: "ruin"` the walls are broken, some towers fallen and every building a roofless shell.

The keep and the yard's buildings are the castle's pieces (`b:<layout>:<id>`, as `get`, `search`
and `inspect {what: "battlemap"}` give them): `set_building` changes one as it changes a town's building (`func`, `name`,
`floors`, `roof`, `tint`, `material`, `shape`, `shade`, `structure`; not its footprint, which is the castle's),
`remove` takes a yard building away and `restore` puts a piece back as built. `keep_func` and
`keep_name` on `set_site` are the short form for the keep (`auto` and `""` put them
back: the castle itself, named as the site). The changes are kept with the castle (by its `c:` id), so they stay on it
when another site is deleted or the castle is redrawn; a yard building's change holds while a building stands where it
was changed (a redrawn castle lays out its yard anew), the keep's follows the keep. A ruined piece (the castle in
ruins, or the piece made `structure: "ruin"`) takes a function, a name and looks but no storeys; the keep is taken
away only with `keep: false`.

```json
{ "name": "set_site", "arguments": { "kind": "castle", "rect": { "x_ft": 770000, "y_ft": 2330000, "width_ft": 400, "depth_ft": 300 }, "name": "Candlekeep", "keep_func": "library" } }
{ "name": "set_building", "arguments": { "id": "b:322:18", "func": "library", "name": "The Great Library", "floors": 5 } }
```

A **wall** is a line (`path`, 2–64 corners, each side at least 10 ft, at most 3,000 ft in all; `closed: true` joins
the last corner to the first, and a closed wall may have 3–256 corners and run 24,000 ft round): towers on its
corners and along long runs, a gatehouse on each corner listed in `gates` (indices into its line) and wherever a
road, a town's approach or its main street crosses it (a closed wall: a lane through it, not one running along it).
Side streets and alleys are closed by it, unless `street_gates: true` (a gate on every street crossing it, a new
corner where none is within 20 ft); a closed wall that would have no gate gets one where its busiest side street
crosses (the one nearest the town's middle), so every ring has a way in. In or round a town it is built as that town
builds its own walls (stone, towers, gatehouses and merlons by its culture and wealth). Where it would stand in water
it is broken (a tower on each bank); an open wall in the water is refused, a closed one is broken there.

Or a ring round patches of a town's plan: `settlement` (the town's id) and `around_patches` (patch numbers from
`get {view: "town_plan"}`) in place of `path`. Its line is the outer edge of those patches, which is the middle of the street
round them: a lane inside the wall and one outside, the blocks untouched (where one district's lots run across a
patch edge with no street, those buildings are in its way: `sides_without_street` in the refusal). Patches the set
encloses are inside it (`enclosed_patches`). It is refused for patches in more than one piece (naming the pieces),
patches meeting only at a corner, a side on the town's standing wall (take the walls down with `edit_town`, or leave
out the patches on the edge), or more than 256 corners. Its gates: on every corner where a main street goes through;
with `street_gates`, on every corner where a side street leaves it outward (patch corners are where three patches
meet, so a street there ran on into the streets inside and round it); `gates` adds corners by hand. The line is
taken from the plan as it stands and kept in the world file: corners moved later with `edit_town` leave it where it
was, until `set_site` gets `around_patches` again.

Both must stand on dry land, off river channels, and not cross another wall (the refusal names it: `that crosses
wall c:4 at (x, y)` or `the town wall of city:…`), but an open wall may end on one: an end within 15 ft of another
wall's line or a castle's curtain (short of it or past it) is put on that line, a T-join with no gap (the result's
`path` gives the line as laid, `joined` says so); a castle also stays off squares, roads and main streets. Buildings drawn by hand in the way refuse it; the world's own buildings in the way are named in the
refusal, or taken away with it (one change) when `remove_in_way: true`. A castle with `raise_hill: true` stands on a
round hill of its own, added in the same change (`hill: {id, added}` in the result; none is added when a hill already
stands under its middle): 60 to 120 ft high, its flat crown a little wider than the castle, its slopes about three
times as long as it is high. It is an entry like any hill (`set_ground {kind: "hill"}`, `remove`). Corners snap to the 5-ft grid (`snap: false`
not). Their wall and gate towers have interiors (`t:<layout>:<k>`), and a building drawn on the wall is refused. The
result also gives `towers`, `gates`, `buildings` and the castle's `keep` (its building id); a wall's also `corners`,
`length_ft`, `closed`, `gate_list` (each gate as built: `corner` when on one, and `x_ft`, `y_ft`) and
`streets_closed` (crossings with no gate: `corner`, `x_ft`, `y_ft`, `street` main or side); a ring round patches
`around` (`settlement`, `patches`, `plan_corners`: the plan's corner for each corner of its line).

```json
{ "name": "set_site", "arguments": { "kind": "castle", "rect": { "x_ft": 4772760, "y_ft": 1498825, "width_ft": 300, "depth_ft": 200 }, "gate": 2, "name": "Highmoor Keep" } }
{ "name": "set_site", "arguments": { "kind": "wall", "path": [[4772500, 1499150], [4773050, 1499150], [4773200, 1498650]], "gates": [1] } }
{ "name": "set_site", "arguments": { "kind": "wall", "path": [[412000, 380000], [413200, 380000], [413200, 381000], [412000, 381000]], "closed": true, "remove_in_way": true } }
{ "name": "set_site", "arguments": { "kind": "wall", "settlement": "city:6c30c809", "around_patches": [0, 3, 1, 2, 40, 9, 6, 7, 5, 4], "name": "Upper City Wall" } }
```
The last answers (cut short) `{"id": "c:0", "closed": true, "corners": 19, "length_ft": 6005, "gates": 1, "gate_list":
[{"corner": 7, "x_ft": 2912602, "y_ft": 1052812}], "streets_closed": [{"corner": 2, "street": "side", ...}, ...],
"towers": 42, "around": {"settlement": "city:6c30c809", "patches": [...], "plan_corners": [0, 10, 13, ...]}}`.
> *"Build a castle on the hill west of Bulol with its gate toward the road."* · *"Wall off the north side of Agentholm."* · *"A ruined fort, keep only."*

### `set_site {id}` (a castle or wall)
`id` (a castle's or wall's `c:` id) and any of `set_site`'s castle or wall fields: a new outline or line
(`path`, or `settlement` and `around_patches`: the ring laid anew on the plan as it stands, its gates with it), its
gate or gates (a castle's: its sides), `street_gates`, `keep`, `keep_shape`, `keep_poly`, `yard_buildings`, `moat`,
`structure`, `closed`, `name`. Fields left out stay; the result is checked
like a new one (`remove_in_way` as there). `remove` takes it away.

### `set_feature` (several fields) and `set_site {id}` (an entrance)
Changes a name and the notes in one call. Unlike `set_feature`, `tags` here replace the existing tags. A
building drawn by hand takes either id, as in `set_feature`.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | optional | |
| `notes` | string | optional | Replaces the notes text. |
| `tags` | string[] | optional | Replaces the tags; `[]` clears them. |
| `in_building` | string or null | optional | An entrance made with `set_site` only: a building's id (`b:<layout>:<n>`, or the `c:` id of a drawn one) moves it into that building's cellar, its point the building's middle, by the checks and refusals of `set_site` `in_building`. `null` takes it back out, to `at` if given, else the nearest clear spot just outside the building. Its ids (`c:<n>`, `u:<layout>:0`) stay, so NPCs, plots, notes and renames still point at it. |
| `way` | string | optional | `stairs` or `trapdoor`: with `in_building`, or alone for an entrance already in a building. |
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | optional | Only with `in_building: null`: where outside (checked as a bare entrance is). |

Give at least one of the others; with none the call is refused. A move says where it went (`moved: {x_ft, y_ft, ft, why}`) and gives `in_building` and `way`.
`in_building` on anything but an entrance is refused (`in_building is for entrances`); no other feature moves, and `at` or `x_ft`/`y_ft` without `in_building`
are refused. A design drawn by hand for the site underground (`set_design`) is set aside when the entrance has moved away from it (a `note` says so).

```json
{ "name": "set_feature", "arguments": { "id": "c:2", "name": "Bastion of the Drowned", "notes": "Flooded at high tide.", "tags": ["dungeon", "undead"] } }
```

### `set_feature {id, hidden}`
Hides a feature from the map's labels and from search, or shows it again. It still exists, and
`search` with `include_hidden` finds it. A building drawn by hand is hidden by either id (the mark is
kept under its `c:` id), and its business pin is hidden with it.
A district the ward plan left with no patch can be hidden too (it sits at the town's middle).

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | |
| `hidden` | boolean | true | `false` shows it again. |

```json
{ "name": "set_feature", "arguments": { "id": "town:764adcb3" } }
{ "name": "set_feature", "arguments": { "id": "town:764adcb3", "hidden": false } }
```
> *"Hide the town of Zhuol; the players haven't discovered it yet."* · *"Reveal Zhuol."*

### `remove` and `restore`
`remove` takes things away by id, `restore` brings back what can come back. Both take `id` or `ids` of any families
(the prefix says which: the Ids table); a call is one change, saved once and undone at once in the app, and all or
none: if one id is refused nothing changes and the error names it.

| Prefix | `remove` | `restore` |
|---|---|---|
| `c:`, a drawn building's `b:<layout>:0` | deletes a created site or drawn building (below) | brings it back |
| the world's own `b:` | takes the building away | puts it back as generated |
| `n:`, `p:` | deletes an NPC (taken out of plots), a plot point | deleted for good |
| `m:`, `r:` | deletes a point note, a room's note | deleted for good |
| `o:` | takes a placed object away | deleted for good |
| `x:` | refused: a clear is brought back with `restore` | lifts the clear (the generated objects come back) |
| `h:`, `i:` | takes a plateau, pit or hill away; hides a generated islet (its watchtower goes too) | an islet comes back (`i:` only) |
| `e:`, `q:`, `v:`, `j:`, `sk:` | water, a wharf, a crossing, a link, a sketch stroke | deleted for good |
| `rd: mt: tr: pp: rw: ts:` | a road off the map (a stretch takes its towns' approaches: `also`) | the road back |
| `fr: fh: f: fg:` | a fence, hedge or gap off | back |

"Deleted for good" is refused (`e:1 can't be restored: water drawn by hand is deleted for good (the app's Undo can
bring it back)`). Generated features other than buildings can't be removed, only hidden (`set_feature {id, hidden}`).
Besides ids, `what` picks by place: `remove {what: "buildings", within}` (the world's own buildings whose middle is
inside), `remove {what: "objects", within, points, kinds}` (generated objects: below), `restore {what: "objects",
within}` (every clear touching it), and `remove {what: "notes", id}` (the note on a place). One family's reply is its
own (below); several give `{changes, results}`, one result per family in the order first named.

```json
{ "name": "remove", "arguments": { "ids": ["n:k2x9", "b:4:120", "rw:3f0a"] } }
{ "name": "restore", "arguments": { "ids": ["b:4:120"] } }
```
> *"Take the smith, his shop and the lane to it away."* · *"Put the shop back."*

### `remove` (created sites and drawn buildings)
Deletes created sites (`c:<n>`: ruins, towers, camps, castles, walls...) and buildings drawn by hand: `id` or `ids`, all or none in one change. A building drawn by hand goes by either of
its ids (`c:<n>` or `b:<layout>:0`). Generated features can't be deleted, only hidden; the world's own buildings go
with `remove`. The ids are retired and never reused: the site's `b:`, `d:`, `u:` and other layout ids stop
answering (`get` says `b:320:0 was deleted with c:2 (restore brings it back)`), and searches, lists and
`inspect` skip it. What pointed at it is kept as it is, and listed per site in `left_pointing` (only what
applies): `npcs`, `plots` (ids), `notes: true`, `design: true`, `renames` (a count). Deleting a deleted site is
refused (`c:11 is already deleted (restore brings it back)`).

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | one of | A `c:` id, or a drawn building's `b:<layout>:0`. |
| `ids` | string[] | one of | Several; all or none. |

```json
{ "name": "remove", "arguments": { "id": "c:0" } }
```
> *"Delete the ruin we made earlier."*

### `restore` (created sites and drawn buildings)
Brings back sites deleted with `remove`: `id` or `ids` (`c:<n>`, or a drawn building's `b:<layout>:0`; all or
none, one change). The ids answer again, with the NPCs, plots, notes, design and renames that still pointed at them.
Refused when the spot is taken now, by the checks a new one of that kind meets (`b:… can't be restored, its place is
taken: that overlaps …`; for a castle or wall, the world's own buildings in its way are named), or when the site is
not deleted. Returns the site as `get` does (a building with its `building` id); several return
`{ids, restored: [sites]}`.

```json
{ "name": "restore", "arguments": { "id": "c:0" } }
```
> *"Bring the ruin back."*

### `set_settlement` (a new village or town)
Adds a village or town without remaking the world. It is laid out by the world's own village and town generators
(streets, buildings, businesses, districts, battlemaps, interiors), its size drawn from its id, and joined to the
nearest road as the world's own are (a village by a track running in through its middle, a town by a road to its
edge). Nothing else in the world changes: other settlements, sites, roads and terrain stay as they were. The result
is `get`'s details with the new `c:<n>` id, plus `culture`, `lives_by`, `population` and `track_ft` (the
link's length). Its buildings take the world's building tools (`b:<layout>:<id>`), and `get {view: "town_plan"}`/`edit_town` take a
town.

By a sea or lake a village or town stands back from the shore, its centre about a third of its built-up radius
inland (as a port stands back from its harbour), and its patches are shaped by the water as a coastal town's are; it
still lives by its land (ports and fishing villages need the world remade). The call is refused, with the reason, on
the edge of the map, in water or a river, on the water's edge (nearer than that), too near another settlement (half a
mile of open ground between them) or site, with a road running through it, on steep ground, more than 10 miles
from a road (the refusal says how far the nearest is, which way, and where), or where water keeps its link from reaching one. Asking again for the
same settlement at the same place returns that one, with `"existing": true`.

`remove` takes it away. The changes made in it (its buildings' edits, its town laid out anew, names, notes,
hidden marks, NPCs' places and plot points in it) are set aside, as when the world is remade: they stay in the file
and resolve nowhere. The result's `set_aside` counts them. `set_settlement` changes its tier, what it lives by,
its culture or its name at once. A city, a capital, a port or a fishing village needs the world remade: a settlement pin
(`set_sketch_stroke`) and `regenerate`.

| Parameter | Type | | |
|---|---|---|---|
| `tier` | string | required | `village` or `town`. |
| `at` | a place | required | Where its centre goes: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`. |
| `name` | string | optional | If left out, it is named in its culture's style. |
| `culture` | integer | optional | Naming culture: 0 cold north, 1 temperate west, 2 warm south, 3 dry lands, 4 open steppe. Default: the nearest settlement's. |
| `lives_by` | string | optional | `river`, `mining`, `fortress`, `market`, `farming`, `lumber`, `herding` or `oasis`. Default: from its ground. |
| `blank` | boolean | optional | `true`: laid out by hand from nothing. The roads that reach it run in as streets to one junction at its centre, and there is nothing else (no blocks, no buildings) until streets are drawn round it; each block they make takes common lots. Its size class (label, icon, lists) and `population` follow the people its homes house, at once; its roads and trade catch up when the world is next remade. The result adds `blank: {layout, streets}`. Default `false`. |
| `link` | points | optional | A lane of its own: 2 to 64 points (world ft, `[x_ft, y_ft]` or `{x_ft, y_ft}`) from the settlement out to the world's roads; a free end within 60 ft of a road runs on to it and meets it at its level. It is kept as a world lane (`lanes/…` records: two junctions and the lane between them), which battlemaps draw with its bridges and fords. The result adds `lane` (its key). |

```json
{ "name": "set_settlement", "arguments": { "tier": "village", "at": [2318926, 1121032], "lives_by": "lumber" } }
{ "name": "set_settlement", "arguments": { "tier": "town", "at": { "x_ft": 2318926, "y_ft": 1121032 }, "name": "Pelbridge", "culture": 1 } }
{ "name": "set_settlement", "arguments": { "tier": "town", "at": [2318926, 1121032], "blank": true } }
```
> *"Add a logging village up in those hills."* · *"Put a market town between Bornworth and the lake."*

### `set_settlement {id}`
Changes a settlement's tier, what it lives by, whether it is the realm's capital, its name or its culture.

- **Culture**, for every settlement (the world's own and those added by hand), changes at once with no remake: it
  is laid out anew in that culture, as if generated so (its streets, buildings, roofs, walls, yurts and felt halls,
  parks, cemeteries and generated names of districts, squares, gates and buildings), in the same place: no road,
  neighbour or other settlement moves. Its own name, notes, NPCs and plot points stay on it; the changes made inside
  it (buildings, its town plan, designs underground, names, notes and marks on what is in it, NPCs and plot points
  placed there) are set aside: they stay in the file and resolve nowhere (`set_aside` counts them, `set_aside_by` by
  field). Changed back, it is laid out as generated again (what was set aside stays aside). For one of the world's
  own, give `culture` on its own (tier, lives_by, capital and name go through a pin and `regenerate`); for one added
  by hand, with any of the others. The result gives `culture` and `from` (names), `feature` (`get`'s details:
  `layout.culture`), `pending: false`; asked again, `unchanged: true`.

- **A village or town added by hand** (`c:...`) changes at once, with no remake: a village becomes a town or the
  other way, or it lives by something else (inland kinds only). It is laid out anew, so the changes made inside it
  (buildings, its town plan) are set aside (`set_aside` counts them), as `remove` sets them aside; its name,
  notes, NPCs and plot points stay on it. A city or metropolis, the capital, a port or a fishing village is refused,
  saying how: `remove` it, put a settlement pin there and `regenerate`. The result is `get`'s details
  as `set_settlement` gives them.
- **One of the world's own** changes through a settlement pin in its place in the sketch (`replaces`: the tier it
  was placed at). The pin is stored at once and the result says `pending: true`; `regenerate` (or `regenerate: true`
  here) remakes the world. The tier is placed as it would be without the pin, then the pin's settlement takes that
  one's place and its layout number, so no other settlement of the world moves, though its new size changes what
  lies round it (its roads, the trade that sizes the others). Its names, notes, NPCs and plot points follow it (to
  its new id when its tier changes: `now` with `regenerate: true`, or `made` in `regenerate`'s result). It is laid out
  anew, so the changes made inside it are set aside. Its population is drawn in its tier's range. Its name stays as it
  is unless `name` is given, and a name given in a pin wins over a rename. A settlement made by a pin already has
  that pin changed. Asking again changes the same pin. `list {what: "sketch"}` lists the pin with the settlement's id
  (`settlement`).
- A **port** or **fishing village** only on a sea or lake shore (fishing also on a river): refused inland, with the
  reason. When the remade world can't follow it, `regenerate`'s `conflicts` say so.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | The settlement. |
| `tier` | string | optional | `metropolis`, `city`, `town` or `village` (added by hand: `village` or `town`). |
| `lives_by` | string | optional | As a pin's. Default: as it is when its tier stays or still fits the new one (a port is no village's, a fishing village no town's), else from its land; `economy` in the result says which. |
| `capital` | boolean | optional | `true`: the realm's capital (the world's own only). |
| `name` | string | optional | Its new name. |
| `culture` | string or integer | optional | `north` (0), `west` (1), `south` (2), `dry` (3) or `steppe` (4): at once, as above. |
| `regenerate` | boolean | optional | The world's own: remake the world at once (default false). |

```json
{ "name": "set_settlement", "arguments": { "id": "town:1a2b3c4d", "tier": "village", "lives_by": "fishing" } }
{ "name": "set_settlement", "arguments": { "id": "village:5e6f7a8b", "tier": "town", "name": "Neverwinter", "regenerate": true } }
{ "name": "set_settlement", "arguments": { "id": "c:2", "tier": "town" } }
{ "name": "set_settlement", "arguments": { "id": "city:54167bdf", "culture": "steppe" } }
```
> *"Luskan is only a fishing village."* · *"Make Neverwinter a town of two thousand."* · *"Lay Hollywell out as a
> steppe city."*

## Hex grid tools

The overland hex grid the map app can draw over the surface (one per world, saved with the edits; it does not change
generation). Hex columns and rows count from 1: hex `0101` is the first whole hex at the map's top-left corner (world
0,0), the whole lattice moved by the origin offset. A number is `CCRR`, zero-padded (`CCCRRR` on a map with more than 99
columns or rows); a key is `"col,row"`. Which hexes the players have seen is play state, kept in the browser: these
tools never see it.

### `list {what: "hex_grid"}`
No parameters. Returns `{grid, set}`: `grid` is `{size_mi, flat, color, width, opacity, ox_mi, oy_mi, numbers}` (the defaults where nothing is set: 6 mi, flat-topped, `#2b241d` at 0.35, 1.5 px, no offset, numbers on) and `set` says whether any setting has been changed.

### `set_hex_grid`
| Parameter | Type | | |
|---|---|---|---|
| `size_mi` | number | optional | Miles across the flats (centre to neighbouring centre), 0.5–60. |
| `flat` | boolean | optional | Flat-topped (`true`) or pointy-topped hexes. Flat grids shove odd columns down half a hex, pointy ones odd rows right. |
| `color` | string | optional | Line colour, `#rrggbb`. |
| `width` | number | optional | Line width in px, 0.5–6. |
| `opacity` | number | optional | 0–1. |
| `ox_mi`, `oy_mi` | number | optional | The lattice moved east and south, in miles (±10,000). |
| `numbers` | boolean | optional | Hex numbers shown on the map. |
| `reset` | boolean | optional | Back to the defaults first; fields given are then applied. |

Only the fields given change. Returns `{grid, set}` as `list {what: "hex_grid"}` does. Changing the grid keeps revealed hexes by number: old keys then name other hexes.

### `inspect {what: "hex"}`
| Parameter | Type | | |
|---|---|---|---|
| `at` | id, name, `{x_ft, y_ft}` or `[x_ft, y_ft]` | one of `at`, `hex` | The hex holding that place (on the map). |
| `hex` | string | one of `at`, `hex` | A hex by number (`0412`) or key (`"4,12"`); it may lie off the map. |

Returns `label` (the number), `key`, `col`, `row`, `centre` and `corners` (six, world ft: clockwise from the east corner of a flat hex or the north-east one of a pointy hex), `on_map` (the centre is on the map), `around` (the six neighbours, `{key, label}`) and the `grid` used. Edge hexes may have column or row 0 or beyond the map's last.

## The sketch and `regenerate`

The sketch is the set of strokes that steer generation: land, sea, mountain ranges and massifs, rivers, lakes, painted
biomes, volcanoes, settlement pins, sites, regions and roads (World › Sketch in the app). It is part of the world, so
changing it makes another world. The sketch tools store their changes at once (in `<hash>/sketch.json` beside the
world), but the world is not remade until `regenerate`; their results say `pending: true`, and `list {what: "sketch"}` says
whether changes wait. They can't go in a `batch`.

A stroke's id is `sk:` and a hash of the stroke: it stays while other strokes come and go, and a changed stroke gets a
new one (`set_sketch_stroke {id}` returns it). `index` is its place in the sketch, for reading only.

### `list {what: "sketch"}`
The strokes (as the pending changes leave them), each with `id`, `index`, `tool` and its points (`path`, or `at` for a
pin, volcano, site or region named at a point), and its options: `closed`, `radius_mi`, `strength`, `hard`, `biome`,
`name`, a road's `kind`, a pin's `tier`, `lives_by`, `capital`, `wards`, `added`, `replaces`, `on_water` and `settlement` (the
settlement it made, or for a pin not regenerated yet that replaces one, that one's id; `replaces` is the tier the
generator placed that one at, which can be above the tier it shows: a city placed that grew only to a town says `city`). A stroke changed or added since the
world was made says so (`pending: "changed"` or `"added"`), and `changes` lists what changed (`added`, `changed`, and
the ids `removed`). `tool` filters, `points: false` gives `point_count` and `bbox` instead of points, `limit`
(default 100) cuts the list (`{items, found}`).

### `set_sketch_stroke`
| Parameter | Type | | |
|---|---|---|---|
| `tool` | string | required | `land`, `sea` (cut out of land), `range`, `river` (from its first point down), `forest` (a temperate forest painted), `biome`, `road`, `pin` (a settlement). |
| `path` | `[[x_ft, y_ft], ...]` | all but pins | 2 or more points, 3 or more for an outline; at most 5,000, all on the map. |
| `at` | a place | pins | `{x_ft, y_ft}`, `[x_ft, y_ft]` or a feature's id. |
| `closed` | boolean | land, sea, forest, biome | The path is an outline, filled (default true); false: a band brushed along it. |
| `radius_mi` | number | not pins, roads | Brush half-width, 0.1 to 200 (defaults: land 15, sea 10, range 15, river 3, forest and biome 20). |
| `strength` | number | range, river | 0 to 1 (default 0.7): a range's height (0.2 hills to 1 high peaks), a river's size. |
| `hard` | boolean | land, sea, forest, biome | Edges exactly as drawn. |
| `biome` | string | forest, biome | The biome painted (a biome stroke needs it). |
| `name` | string | optional | What it makes is called this (up to 80 characters); a road's name (else it has none). |
| `kind` | string | road | `kings_road`, `road` (default), `track`, or `none` (a line no planned road crosses; no name). |
| `tier` | string | pins, required | `metropolis`, `city`, `town` or `village`. |
| `lives_by` | string | pins | `port`, `river`, `mining`, `fortress`, `market`, `farming`, `fishing`, `lumber`, `herding`, `oasis`. Default: from its land. |
| `capital` | boolean | pins | The realm's capital. |
| `wards` | `[string]` | pins | Its districts' names, the central one first (up to 24). |
| `added` | boolean | pins | Beside the world's own settlements (default true, as Edit › Settlements › Places adds them); false: in place of one of its tier. |
| `replaces` | string | pins | In place of the world's own settlement at its spot (within 3 mi), changed: the tier the generator placed it at (`list {what: "sketch"}`'s `replaces`; a city that grew only to a town is placed as a city). Nothing else moves. `set_settlement` makes these pins. |
| `on_water` | boolean | pins | `false`: a port, fishing village or river town stays where the pin stands (on heights above its river, say) instead of being moved onto its shore or beside its river as the world's own are. Left out: moved. |

An option for another tool is refused (`tier: only for pin, got "town"`), as is a point off the map (the message gives
the map's size). The same stroke again adds nothing and returns `existing: true`. A pin far out at sea (no land within
25 mi in the world as it is) is stored with a `warning`: `regenerate` would place no settlement there. So is the
world's first `range` or massif: drawn ranges replace the generated mountains across the whole world, so the remake
changes features, names and settlements far from it.

### `set_sketch_stroke {id}`
`id` and any of `path`, `at`, `closed`, `radius_mi`, `strength`, `hard`, `biome`, `name`, `kind`, `tier`, `lives_by`,
`capital`, `wards`, `added`, `replaces` and `on_water` (as above, not `tool`); what is not given stays. Returns the new `id` (and `was`).

### `remove` (sketch strokes)
`ids` (or `id`): all or none. A pin taken away takes its settlement with it at `regenerate`: what was made on it is set
aside (`gone:...`, as when the app removes a settlement), never handed to a neighbour.

### `regenerate`
Remakes the world with the pending changes, natively in mapd (a few seconds), with or without the app open. The world's
changes follow their places, as when the app remakes it: names and notes follow features that moved, changes inside
settlements and sites follow those laid out the same, and the rest are set aside under ids that name nothing. Names,
notes, hidden marks, NPCs and plot points on a feature that is gone with nothing in its place are set aside under
`gone:<n>:<id>` (as a settlement taken away is: `world_overview`'s `places_removed` lists them), and come back to it if a
later remake brings that feature back (the stroke taken away again). Set-aside changes can be read later in the app
(Notes › Gazetteer, Set aside) and counted by `places_removed` (name, whether it has a note, NPCs, plot points); no MCP
tool reads their text, so `vanished` is the place to copy it from. A
settlement a pin made takes the changes of a vanished one only at its own spot (a quarter of that one's label extent:
about a mile for a village, 3 for a town), never of one farther off, and a name given in the pin wins over a rename. Tool
calls made meanwhile wait, and see the world before or the world remade, never a mix. The remade world is saved under
its new hash (`world`; `was` is the one before). An open map app loads it, asking first when it has work not saved
(sketch strokes not generated, a site's design, a session being played); an app opened later is told to load it.

Returns `kept` (changes that followed their places to new numbers, or came back to them; the others stayed as they
were), `set_aside` (of them `removed`: on settlements taken away), `vanished` (features gone with nothing in their
place that had changes: `{id, kind, name, kept_as, what}`, `what` e.g. `["renamed", "note", "1 NPC"]`, with the changes
themselves: `renamed_to`, `note` (its text), `hidden`, `other` such as `a building edit`), `moved`
(features whose changes moved to another), `settlements` and `settlements_before`, `changed` (how much of the map came
out otherwise: `features_before`, `features_after`, `features_same_id`, `features_renamed` of those, and
`settlements_in_place`), `gone` (settlements taken away), `made` (the settlement each new or changed pin made:
`{stroke, id, name}`, `id` null if none was placed), `conflicts` (what the sketch could not follow: `{stroke, message,
x_ft, y_ft}`) and `seconds`. With nothing pending it does nothing (`regenerated: false`).

A stroke can change the map far from where it is drawn, as in the app: the generator works the whole map at once, so
heights, rivers, coasts and climate everywhere can come out otherwise, and with them features' ids and names and the
world's own settlements. The first range (or massif) drawn does most: drawn ranges become the main ones and the
plates' own are kept at half their height, all over the map. Changes on the places that moved follow them or are set
aside (`changed` and `vanished` say how much); a pin far from them leaves them as they are.

```json
{ "name": "set_sketch_stroke", "arguments": { "tool": "range", "path": [[4682158, 2536577], [4708558, 2642177], [4682158, 2747777]], "strength": 0.6, "name": "Greyspine" } }
{ "name": "set_sketch_stroke", "arguments": { "tool": "pin", "at": [4391758, 2694977], "tier": "city", "capital": true, "name": "Newhold" } }
{ "name": "set_sketch_stroke", "arguments": { "tool": "road", "path": [[2300000, 1100000], [2500000, 1180000]], "kind": "kings_road", "name": "The High Road" } }
{ "name": "regenerate", "arguments": {} }
```
> *"Make Newhold a city and the capital."* · *"Draw a mountain range along the east coast, then remake the world."*

`set_settlement` adds a village or town with no remake, and `set_settlement` changes one; to change one of the
world's own, `set_settlement` stores a pin, and `regenerate` makes it.

## Notebook tools: NPCs and plot points

The DM's notebook holds NPCs (`n:<id>`) and plot points (`p:<id>`). They are written by the user (the app's
**Notebook**) or by agents, never generated. Edits to them are saved, logged and shown live like the other edit
tools. Ids are random (`n:k3x9q0ab`), so the app and an agent can create at the same time.

- An NPC can be **placed** at any place id. Placed in a building or site (`b:`, `t:`, `u:`, `k:`, `w:`), they
  show inside it on the DM's map as a marker, on their `level` (default: the way in), at their point if given,
  else in that level's biggest room. The DM can drop them into play as a token.
- A plot point is tied to places (`anchors`) and NPCs (`npcs`).
- `get` and `inspect` include the NPCs and plot points there (`npcs`, `plots`). For a
  settlement or site that includes those in its buildings, districts, towers and ways underground, and on their
  levels and rooms; for a settlement, also those at the sites in it (see Conventions).
- `focus_view` and `route` take an NPC's id (where they are) or a plot's (its first place).
- The players' window never gets NPCs, plots or notes.

### `list {what: "npcs"}`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches name, appearance, mannerisms, goals, notes, attitude. |
| `at` | string | optional | A place id. Also finds NPCs on its levels and in its rooms; a settlement's or site's id, NPCs in its buildings, districts, towers and ways underground; a settlement's, NPCs at the sites in it too. A building drawn by hand by either of its ids. |
| `tag` | string | optional | |
| `stance` | string | optional | `hostile`, `unfriendly`, `neutral`, `friendly` or `allied`. |
| `limit` | integer | 100 | 1 to 1000 (`truncated: true` when it cut the list). |

Returns `{items: [{id, name, stance, status, location: {id, name, level?, x_ft?, y_ft?}, tags}], found}`.

### `get`
An NPC in full: `name`, `appearance`, `mannerisms`, `attitude: {stance, text}`, `goals`, `notes`, `tags`,
`portrait` (an asset id), `status`, `location` (where they are), and the `plots` they are in.

### `set_npc` (a new NPC)
| Parameter | Type | | |
|---|---|---|---|
| `name` | string | required | |
| `appearance`, `mannerisms`, `goals`, `notes` | string | optional | `notes` are the DM's (secrets). |
| `stance` | string | optional | Attitude toward the players (default `neutral`). |
| `attitude` | string | optional | Why they feel that way, what would change it. |
| `tags` | string[] | optional | |
| `status` | string | optional | Free text: alive, dead, missing, imprisoned… |
| `portrait` | string | optional | An asset id (`PUT /assets/<id>`, or a picture uploaded in the app). |
| `level` | integer | optional | Inside a building or site: 0 (the lowest level) or more. |
| `at` | id, point, null | optional | Where they are: a place id, or a point `{x_ft, y_ft}` / `[x_ft, y_ft]` (on the map: the building, district or settlement there, else the nearest named place, is the place). `null` (with `id`) takes them off the map. |

```json
{ "name": "set_npc", "arguments": { "name": "Old Brannoc", "appearance": "One eye, a scarred chin", "stance": "unfriendly", "attitude": "Distrusts adventurers since the fire", "goals": "Sell the inn and retire", "at": "b:0:620", "tags": ["innkeeper"] } }
```
> *"Give the Jolly Lantern an innkeeper with a secret, and put him behind the bar."*

Safe to repeat: an NPC with the same name (case and surrounding spaces ignored) and the same place, or both without
one, is that NPC: the result is it, with `"existing": true`, and nothing is added. Another name or place makes a second.

### `set_npc {id}`
`id` plus any of `set_npc`'s fields (`at` and `level` too: see below; `name`, `appearance`, `mannerisms`, `attitude`, `goals`, `notes`, `stance`, `status`, `tags`, `portrait`); only the fields given change.

### `set_npc {id, at}`
`id` plus `at` (a place id, and `level` inside it; or a point). `at: null` takes them off the map. An id that is an NPC with no place, or a plot point, is not a place: the refusal says so.

### `remove` (NPCs)
`id` or `ids` (all or none, in one edit). They are also taken out of every plot point.

### `list {what: "plots"}`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches title and text. |
| `status` | string | optional | `idea`, `active` or `resolved`. |
| `anchor` | string | optional | A place id. |
| `npc` | string | optional | An NPC id. |
| `tag` | string | optional | |
| `limit` | integer | 100 | 1 to 1000 (`truncated: true` when it cut the list). |

Returns `{items, found}`.

### `get`
`id`: a plot point in full: `title`, `text`, `status`, `anchors` and `npcs` (each with its name), `tags`.

### `set_plot` (a new plot point)
| Parameter | Type | | |
|---|---|---|---|
| `title` | string | required | |
| `text` | string | optional | What is going on, hooks, how it may play out. |
| `status` | string | optional | `idea` (default), `active` or `resolved`. |
| `anchors` | string[] | optional | Place ids (not NPCs). |
| `npcs` | string[] | optional | NPC ids; they must exist. |
| `tags` | string[] | optional | |

```json
{ "name": "set_plot", "arguments": { "title": "Smugglers in the cellar", "text": "Barrels move at night...", "status": "active", "anchors": ["b:0:620"], "npcs": ["n:k3x9q0ab"] } }
```
> *"Make a plot point for the smuggling ring, tie it to the inn and the harbour district, and involve Brannoc."*

Safe to repeat: a plot point with the same title (case and surrounding spaces ignored) is that plot point, with
`"existing": true`; nothing is added. Another title makes a second.

### `set_plot {id}`
`id` plus any of `set_plot`'s fields (`title`, `text`, `status`, `anchors`, `npcs`, `tags`); lists given replace the old ones.

### `remove` (plot points)
`id` or `ids` (all or none).

### `set_note` (a new point note)
A note at a point inside a building, dungeon or sewer level (the DM sees it as a marker in the room; the players never do).
| Parameter | Type | | |
|---|---|---|---|
| `site` | string | with `level` and `at` | The building, dungeon or sewer section id (`b:`, `u:`, `k:`, `t:`, `w:`). A place on the surface is refused: use `set_feature`. |
| `level` | integer | with `site` | 0 is the lowest level. |
| `at` | [x, y] | or `room` | Grid squares of that level (decimals kept to 0.01). Must lie inside the level's grid; a point outside the rooms is kept and the reply's `on` says so. |
| `room` | string | or `at` | An `r:` id: the note goes at its centre (`site` and `level`, if also given, must agree). |
| `text` | string | required | The note. |
| `tags` | array | optional | Stored in one form: the quick picks (`treasure`, `read-aloud`, `trap`, `secret`, `monster`) lower-case, the rest deduplicated ignoring case. `set_feature` does the same. |

A bare `w:<layout>` (a whole sewer network) is refused with the section form `w:<layout>:<sx>:<sy>`.

Returns the note: `id` (`m:<site>:<level>:<own id>`), `kind` "point", `place` (site › level › room names), `room` (the `r:` id it lies in, or null), `text`, `tags`, `at`, `site`, `level`, `on`.

### `set_note {id}`
`id` (an `m:` id) and `at` or `room`, as `set_note`. A note stays on its level: to put it on another, delete it and add one there. `set_feature` with `at` on an `m:` id does the same. Returns the note.

### `remove` (notes)
`id`: the note on a place, a room (`r:`) or a point (`m:`). A building drawn by hand's note is found under either of its ids. Returns the text it had.

### `list {what: "notes"}`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches the text. |
| `tag` | string | optional | |
| `site` | string | optional | A place id; also what is in it (a settlement: its buildings, sites, levels, rooms, points). |
| `level` | integer | optional | Level of a room or point note. |
| `kind` | string | optional | `place`, `room` or `point`. |
| `limit` | integer | 100 | 1 to 1000 (`truncated: true` when it cut the list). |

Returns `{items, found}`: each has `id`, `kind`, `place` (path of names), `room` (points), `text`, `tags`, and `at` (points). A point note whose site has gone (a world remade) is listed with `place: null` and a `missing` line, so it can still be deleted.
`get` also takes `r:` and `m:` ids (an `m:` gives its site, level, room, spot and note); `set_feature` adds to or changes the note on an `r:` room or an existing `m:` point (an `m:` has no name: `set_feature` refuses `name` for it).

---

## Battlemap objects: scatter and sprites

Battlemaps are generated with their trees, rocks, props and hazards. On top of that, objects can be **put down**
by hand (`o:<id>` entries) and generated ones **taken away** (clears, `x:<id>` entries), by the user (the app's
**Edit › Land › Props** tool) or by agents. Pictures can be **uploaded** as new kinds of object, with their own tactical
rules (`s:<asset>` kinds). Objects block, cover and hinder in play mode by their kind's rules, and
`inspect {what: "battlemap"}` lists them. Like the other edits, they are saved, logged, shown live, and leave the world hash
alone; only the battlemap chunk holding each object (by its centre) is made again.

- A **built-in kind** is named by its id or name (`12` or `"boulder"`); `list {what: "sprites"}` lists them all with their
  rules.
- An **uploaded sprite** is named `s:<asset>` (or by its name). Its picture is in the asset store
  (`/assets/<id>`; the id is the first 16 bytes of the picture's SHA-256 in hex, as the app makes it). A top-down
  picture with a transparent background works best, at about 64 px a square.

### `list {what: "sprites"}`
Parameter: `limit` (1 to 1000, default 200; `truncated: true` when it cut the list). Returns `{items, found}`: built-in kinds and uploaded sprites in one list, each
`{kind, name, uploaded, rules}` (an uploaded one also `picture` and `placed`, how many are put down); `rules` are cover (a
word), blocks_movement, blocks_sight, difficult, height_ft, size_squares and any hazard.

### `list {what: "objects"}`
Every object put down by hand (`o:`) and every clear (`x:`, generated objects taken away), world-wide, in one `{items, found}`
list (objects first; `objects` and `clears` beside say how many of each), so an agent that lost its ids can audit or undo.

| Parameter | Type | Default | |
|---|---|---|---|
| `within` | box `[x0, y0, x1, y1]`, polygon `[[x, y], ...]` or circle `{at, radius_ft}` | whole world | Only those with their place inside: an object belongs to an area by its middle, and a box is half-open (`x0 <= x < x1`, `y0 <= y < y1`), so boxes that tile a region list each object once. |
| `kinds` | (id or name)[] | all | Only these kinds (built-in id or name, or `s:<asset>`); a clear of all kinds matches any. |
| `which` | `objects`, `clears` or `both` | both | |
| `limit` | integer | 200 | 1 to 2000 |

An object item is `{id, what: "object", kind, x_ft, y_ft, variant}` and, when set, `rot_deg`, `scale` and `sprite`
(`s:<asset>`); a clear is `{id, what: "clear", x_ft, y_ft}` with `kind` (one object taken by kind and place) or `radius_ft`
and `kinds` (a circle). `remove` takes objects by id; `restore` brings clears back.

```json
{ "name": "list", "arguments": { "what": "objects", "within": { "at": "c:2", "radius_ft": 300 } } }
{ "name": "list", "arguments": { "what": "objects", "which": "clears", "kinds": ["boulder"] } }
```
> *"What did I put on the bridge map? Take the barrels away."*

### `set_sprite`
| Parameter | Type | | |
|---|---|---|---|
| `data_base64` | string | one of these | The picture (png, jpeg, webp, gif or svg), base64 (a data URL works too). |
| `path` | string | | A picture file on the machine mapd runs on. |
| `asset` | string | | A picture already uploaded: change its name or rules. |
| `name` | string | optional | |
| `size_squares` | number | optional | Squares across at scale 1 (default 1; 0.2–40). Also `size`. |
| `cover` | string or integer | optional | `none` (default), `half`, `three-quarters` or `full`, as the rules are returned; or 0 to 3. |
| `blocks_movement`, `blocks_sight`, `difficult` | boolean | optional | Default false. |
| `height_ft` | number | optional | 0–500 (default 3). Objects 5 ft and taller draw above tokens' shadows. |

Returns `{kind: "s:<asset>", asset}`. A picture already uploaded (same bytes) under another name or rules adds `existing: true` and `replaced` (its old name and the rules that changed): a picture has one name and one set of rules, the last upload's.

```json
{ "name": "set_sprite", "arguments": { "path": "C:/art/altar.png", "name": "Blood altar", "size_squares": 2, "cover": "three-quarters", "blocks_movement": true, "height_ft": 4 } }
```

### `set_objects`
| Parameter | Type | | |
|---|---|---|---|
| `objects` | array | required | 1–500 of `{id?, kind, x_ft, y_ft, rot_deg?, scale?, variant?}`. |

A mistake in the list is refused with the item's place in it (`objects[17].scale: 0.2 to 5, got 9`, `objects[3].kind: no object kind 'boulderr'`); nothing is put down then. `kind` is a built-in kind (id or name) or an uploaded sprite. `rot_deg` turns it clockwise: it is drawn turned
for props that can turn (not trees, plants or pools) and for uploaded sprites; without it the object keeps the
way it is drawn by default. `scale` is 0.2–5 (default 1). `variant` (0–15) picks a built-in kind's look: most
have four (taken modulo 4), a market stall sixteen (its awning and its goods); default by chance. Positions must be on the map. Objects stand on the outdoor battlemap: one put down where a building
stands is under its roof. Furniture, props and traps in a building's rooms or underground go in its design
(`set_design`).

With `id` (`o:…`, an object put down before) the object is changed in place and keeps its id: only the fields
given change, so `{id, x_ft, y_ft}` moves it and `{id, rot_deg}` turns it (`kind` is then optional). A
generated object is moved by taking it away (`remove` with `points`) and putting one down. Returns
`{tool, ids, changed}` (`changed`: the ids that were given, changed in place).

Safe to repeat: a new object of the same kind within 0.5 ft of one put down before, with the same turn and scale (and
the same `variant` when one is given), is that object: its id is in `ids` and also in `existing`, and nothing is added.
Give another place, turn or scale for a second one on purpose.

```json
{ "name": "set_objects", "arguments": { "objects": [ { "kind": "boulder", "x_ft": 412500, "y_ft": 380120 }, { "kind": "s:0123456789abcdef0123456789abcdef", "x_ft": 412520, "y_ft": 380120, "rot_deg": 90 } ] } }
```
```json
{ "name": "set_objects", "arguments": { "objects": [ { "id": "o:k2x9qa", "x_ft": 412540, "y_ft": 380125, "rot_deg": 45 } ] } }
```
> *"Put a ring of standing stones (boulders) round the hilltop east of Bulol, with my altar sprite in the middle."*

### `remove` (objects)
| Parameter | Type | | |
|---|---|---|---|
| `id`, `ids` | string / string[] | optional | Objects put down by hand (`o:…`); all or none. |
| `points` | array | optional | Generated objects: `{kind, x_ft, y_ft}` each, as `inspect {what: "battlemap"}` lists them (matched within a quarter square; a point already cleared there adds nothing and its clear is listed under `existing`). |
| `within` | area | optional | Everything inside: a box `[x0, y0, x1, y1]`, a polygon `[[x, y], ...]` or a circle `{at, radius_ft}` (or `{x_ft, y_ft, radius_ft}`; radius up to 640 ft). A circle takes away placed and generated objects; a box or polygon only those put down by hand (the reply says so). Also `area`. |
| `kinds` | array | optional | With `within`: only these built-in kinds (ids or names). A `kinds` inside a circle works too. |

Returns `{tool, removed: [o:…], clears: [x:…]}` (+ `existing` for points that were already cleared).

```json
{ "name": "remove", "arguments": { "what": "objects", "within": { "x_ft": 412500, "y_ft": 380120, "radius_ft": 30 }, "kinds": ["deciduous tree", "conifer"] } }
```
> *"Clear the trees from a 60-ft circle in the woods north of the ruin so there's room for a fight."*

### `restore` (objects)
`id` or `ids` (clears, `x:…`; all or none) and/or `within` (a box, a polygon or a circle `{at, radius_ft}`:
every clear touching it: a clear's circle meeting the circle, box or polygon, or its middle inside). The generated objects come back.

## Buildings drawn by hand

Buildings and towers can be **drawn**, by the user (the app's **Build** menu) or by agents: a footprint on the map
and what the building is. Each one is a created site (`c:<n>`, kind `building`) with a layout of its own, so it
gets everything a generated building has: a roof and walls on the battlemap, an interior with its rooms and
furniture on every storey (a cellar too), its name on the map and in search. Its building id is `b:<layout>:0`
(returned as `building`); use it with `get`, `set_npc` and interiors, as for any building. One drawn in
a settlement is listed under it: among its sites and, with a trade, its businesses (`settlement` on its ids names
the town; see Conventions). Like the other edits, buildings are saved, logged, shown live and leave the world hash
alone.

- **Where:** on dry land, out of rivers, clear of other buildings (walls may touch), roads, streets, town
  walls and squares (market squares, quays, village greens, a site's yard). Anything else is refused, saying what
  is in the way and where (`that overlaps Noble estate (b:12:345) in Waterdeep, at (412530, 380140)`, `that is on a
  street (20 ft wide) of Waterdeep at (412510, 380130)`, `that is on Waterdeep's wall at (…)`, `that is on a square
  (market) of Waterdeep at (…)`, `that is on the King's road rd:1a2b3c4d at (…)`; points in whole feet).
  `set_building {dry_run: true}` asks without building and lists every blocker; `clear: true` takes the world's own
  buildings away with the new one; `list {what: "streets"}` shows the streets, walls and squares of an area.
- **Footprint:** any simple shape of 3–64 corners, at least 10 ft across, every corner within 200 ft of its
  middle. L-shapes and other concave plans are fine (their roofs are drawn as several hips).
- **What it is (`func`):** a business by its catalog key (`inn`, `tavern`, `blacksmith`, `temple`, `castle`,
  `observatory`, `library`… as `list {what: "names"}` shows them; the tool's `func` description lists every key) or a home: `hovel`, `house` (default), `townhouse`,
  `tenement`, `noble_estate`, `farmhouse`, `wizard_tower`. It decides the rooms inside: a taproom and guest
  rooms for an inn, a forge for a smithy, cells for a prison, a round stair and study floors for a wizard's tower
  (when the footprint is round). A `func` the code does not know is refused with the nearest key
  (`func: a business or home such as inn, tavern, ..., got "tavrn" (did you mean tavern?)`).
- **`hall` and `chapel`** are funcs too (insides of a town hall and of a shrine: great room, nave and crypt). `material` takes
  `stone` (flagstone slabs) as well as the other roof materials.
- **Roof:** `hip` (pitched; L-shapes get one per wing), `battlements` (a walkable roof with a parapet: the
  interior gains a roof level) or `cone`; left out, as its kind has it (battlements on castles, barracks and
  towers). **Tint:** `terracotta`, `slate`, `thatch`, `shingle` or `moss` (left out: picked).
- **Structure:** `ruin` leaves broken walls with no roof and no way in.
- Delete one with `remove`, hide its label with `set_feature`. It is one place under its `c:` id and
  its building id: notes and the hidden mark are kept under `c:` (either id reads and writes them), and NPCs and
  plots at either id are found under both.

### `set_building` (a new building)
A building with the same footprint as one already drawn (every corner within 2 ft), the same `func` and (if
given) the same name is not drawn twice: the result is that building, with `"existing": true`. A different
building on that footprint is refused (`that overlaps …`). With `at` (no footprint), mapd remembers this session's calls (the world and every argument): an identical call whose building
still stands returns it (`existing: true`) and draws nothing, until mapd restarts. Any difference (another name, place, size
or `func`) makes a new one, so a row of unnamed houses along a street is a row of buildings. `func` is matched
without regard to case and spaces (`"Inn"` is `inn`).

Instead of a footprint, `at` (an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`) with `width_ft` and `depth_ft` finds the nearest clear
lot: tried ring by ring out to 400 ft, turned square to the nearest street (or road), its front 2–12 ft from it
where a street is within 300 ft, on the grid when unturned, off squares.

| Parameter | Type | | |
|---|---|---|---|
| `poly` | `[[x_ft, y_ft], …]` | one of these | The footprint's corners in order (3–64). |
| `rect` | object | | `{x_ft, y_ft, width_ft, depth_ft, angle_deg?}`: a rectangle about its middle, `width` along `angle_deg` (clockwise from east). |
| `circle` | object | | `{x_ft, y_ft, radius_ft, sides?}`: a round tower (8–32 sides, default 16). |
| `snap` | boolean | optional | Default true: corners of a `poly` or unturned `rect`, and a circle's middle, snap to the 5-ft grid. |
| `name` | string | optional | Else named for its trade in the local style ("The Drowned Lantern", "Harlan's Smithy"), or what it is ("House"). |
| `func` | string | optional | What it is (see above). |
| `floors` | integer | optional | Storeys above ground, 1–8 (default 1; a wizard's tower 4). |
| `roof` | string | optional | `hip`, `battlements`, `cone` or `auto`. |
| `tint` | string | optional | `terracotta`, `slate`, `thatch`, `shingle`, `moss` or `auto`. |
| `material` | string | optional | Roof material: `thatch`, `slate`, `clay_tile`, `shingle`, `turf`, `lead`, `copper`, `glazed_tile`, `flat_clay`, `hide`, `plank`, `whitewash` or `auto` (picked for the town's culture and the building's kind). Wins over `tint`. |
| `shape` | string | optional | Roof shape: `hip`, `gable`, `cone`, `dome`, `flat`, `round` or `auto`. A flat roof (or a dome on a plain building) has a walkable terrace. A material that cannot take the shape is eased to one it can. |
| `shade` | integer or string | optional | Roof colour: one of the material's five variants, 0–4 (2 the usual; e.g. slate 0 blue-grey, 1 green-grey, 3 purple-grey, 4 dark; clay tile 0 brick, 1 orange, 3 brown, 4 salmon), or `auto` (picked: mostly the usual one). |
| `structure` | string | optional | `roofed` (default) or `ruin`. |
| `at` | id, object or `[x_ft, y_ft]` | instead of a footprint | Find the nearest clear lot near this place (see above). |
| `width_ft`, `depth_ft` | number | with `at` | The lot along the street and back from it, 10–400 ft. |
| `clear` | boolean | optional | Default false. Take away the world's own (generated) buildings that overlap the footprint, in the same change (`remove` does the same by id; `restore` brings them back, unless a building drawn by hand now stands over one: it is refused then, naming both). Buildings drawn by hand in the way are still refused, naming them (`remove` one first); streets, walls, squares, roads and water refuse as ever. With `near`: the nearest lot clear of everything but the world's own buildings, fronting a street as without it. |

A footprint wholly on a hand bridge's deck (off its lane), a pier or a breakwater stands on it: the reply says `on: "v:…"` or `on: "q:…"`, its floor is the deck's level and it has no cellar. Over a bridge's lane it is refused (`that is on the lane of bridge v:… (keep 24 ft clear down its middle)`). On a rock islet it is an ordinary building.

Returns the site as `get` does, plus `building` (its building id), `on`, `moved: {x_ft, y_ft, ft, why}` when
the footprint snapped to the grid (or its point set where it fits) put it more than 5 ft from where it was asked
(not with `at`, which picks a lot by design), and with `clear` `removed: [{id, name, function}]` (the buildings
taken away, with their names and trades; absent when none stood there). The same call again returns the
building and removes nothing.

```json
{ "name": "set_building", "arguments": { "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 50, "depth_ft": 30 }, "func": "inn", "floors": 2, "tint": "thatch" } }
{ "name": "set_building", "arguments": { "circle": { "x_ft": 412600, "y_ft": 380200, "radius_ft": 15 }, "func": "wizard_tower", "floors": 5, "roof": "cone", "tint": "slate", "name": "The Needle" } }
{ "name": "set_building", "arguments": { "poly": [[412700, 380100], [412750, 380100], [412750, 380120], [412720, 380120], [412720, 380145], [412700, 380145]], "func": "blacksmith" } }
{ "name": "set_building", "arguments": { "at": "b:12:340", "width_ft": 60, "depth_ft": 45, "func": "tavern", "floors": 2, "name": "The Leaky Tap" } }
```
> *"Build a two-storey inn with a thatched roof at the crossroads south of Bulol, and an L-shaped smithy beside it."*

### `set_building {id}`
`id` (a drawn building's `c:` id, or one of the world's own: `b:<layout>:<id>`) and any of the parameters of
`set_building`; those left out stay. A new footprint (`poly`, `rect` or `circle`) moves or reshapes it and is
checked like a new one (it never collides with itself). The entrances made in the building follow its new middle in the same change, listed as `entrances_moved: [{id, name, ft}]` (`edit_town` does not move them).

The world's own buildings take the same changes (`edits.buildings`): what it is (`func`), `floors`, `roof`,
`tint`, `material`, `shape`, `shade`, `structure`, a new footprint (`snap` as in `set_building`), and `name` (a rename). `auto` (or an empty `func`) puts one option back as
generated. A new trade with no name of its own is named as a drawn building's would be. Every other building
keeps its id and footprint. If the world is generated again from a changed sketch and the town is laid out anew,
a change whose building no longer stands at its id is set aside (not applied to another building).

A castle's keep and yard buildings (a castle made with `set_site`) take the same changes but a footprint;
they are kept with the castle (see "Castles and walls") and the reply names it (`castle: {id, name}`, `piece`). An id
that is neither says what it is: a building drawn by hand (it goes by its `c:` id), a building of another site made
by hand, a castle's building that isn't there, or no such place.

```json
{ "name": "set_building", "arguments": { "id": "c:4", "floors": 3, "roof": "battlements" } }
{ "name": "set_building", "arguments": { "id": "b:12:340", "func": "tavern", "floors": 2, "name": "The Leaky Tap" } }
```

### `set_building {stamp}`
One building of a kind and a size set down at a point. `stamp` (required): a business or home key, as `set_building`'s `func`.
`at` (required): a place. `size`: `small`, `usual` (default) or `large` (0.75, 1 and 1.4 times the kind's footprint, on the 5-ft grid;
a house is 25 x 25 ft, an inn 50 x 40, a temple 35 x 40 usual) or `{width_ft, depth_ft}` (10 to 400; width runs along the
street). Without `facing` it goes to the nearest clear spot to `at` that fronts the nearest street (within 300 ft), turned square to
it and 2 to 12 ft from its bed (as `set_building at`). With `facing` (degrees, 0 = +x, 90 = +y, the way its front
looks) it stands at `at` turned that way, or on the nearest clear spot (moved, not turned) when `at` is not clear. `clear: true` takes
away the world's own buildings it stands on (`removed`). `dry_run: true` makes nothing and returns `{preview, dry_run, kind, name, x_ft, y_ft,
poly, width_ft, depth_ft, facing_deg?, would_remove?}`. Otherwise the reply is `set_building`'s with `width_ft`, `depth_ft`,
`facing_deg` (toward the nearest street, if there is one) and `moved_ft` when more than 5 ft from `at`. It also takes `name`,
`floors`, `roof`, `tint`, `material`, `shape`, `shade`, `structure` as `set_building` does. The result is an ordinary drawn building.

### `set_building_row`
Drawn buildings lining a path, in one edit. `path` (required): `[[x, y], ...]` (2 to 200 points) or a road stretch id (`rd:`).
`side`: `left` (the normal `[-dy, dx]` of the direction of travel), `right` or `both` (default). `ward` (default `common`) sets the lot
widths (from the ward's lot area), storeys and mix; `kinds` (up to 24 keys) mixes only those, else homes of the ward with now and then
a business of it; `depth` (15 to 120 ft; else the lot area over its width, 22 to 60); `gaps` (0 to 0.5, default 0.15) the share of
slots left empty; `seed` another mix for the same path; `clear` takes away the world's own buildings in the way. Each lot is a rectangle
square to its chord with its front 7, 12, 18 or 26 ft off the line (the first that is clear); a slot that is not clear, or meets
another of the row, is skipped and listed. Up to 100 buildings (`note` says it stopped). Returns `{count, ids, lots: [{id, kind,
floors, x_ft, y_ft, poly}], refused_count, refused?: [{x_ft, y_ft, why}] (12 at most), removed?, length_ft}`; the same call again returns
the same ids and adds nothing. `dry_run: true` returns the lots without ids and makes nothing. Each building is `c:<n>` (remove
takes one away; one change per call). Each building of a row faces the line it was laid along (its door side).

`road: {kind, width?}` (a path kind: footpath, track, gravel, paved, cobbled; `width` 3 to 40 ft, else the kind's) also lays a road
along the row, in the same change: the line is the smooth curve through the points, the lots are set back half the road's width
(then 4, 10, 18 ft more) and kept off its band. Where the lots would be part of a town (the membership rule of `edit_town` members:
within 300 ft of its edge or of one of its members), the road is one of the town's streets: in a generated town or village an added
street (`ts:<layout>:<id>`, listed by `list {what: "streets"}` with that id; its ends joined to the nearest street within 100 ft unless a
wall is in the way; the town's own buildings on its band give way; it takes the town's street look, the kind setting its width
only); in a town laid out by hand, streets of its plan (joined at their ends, split where they cross others). Elsewhere it is a
road drawn by hand (`rw:`, as `set_road` makes). Over water either gets a bridge (a town street a timber deck a little wider than it). Refused with an `rd:` along, and for a town street through the town
wall (open a gate first) or past 64 added streets in one town. The reply's `road` is `{as: road|street, id, town?, line_pts,
removed?}` (`id`: the `rw:` or `ts:` id, or a hand town's new street numbers; `removed`: the town's buildings taken off; `line` with
`dry_run`). Laying the town out by hand (`edit_streets` `lay_out_by_hand`) turns added streets into streets of its plan.

### `set_parcel` (a new parcel)
A free plot of ground drawn as an outline (`Created` kind `plot`, built like a hand block). `poly` (required, 3 to 64 corners, simple, at
least 20 ft across, within 750 ft of their middle; `snap` as for buildings). `fill`: `{kind: generated, ward?, lots?, reroll?, close?}`
(lots of a ward, default `common`; `lots` small to huge), `{kind: hand}` (left empty) or `{kind: open, open, style?}` (plaza, lawn, park,
garden, market, cemetery, farmland); default generated common. `name`. A plot may not meet another one (refused naming it); the same
outline and name again returns that plot (`existing`: true; a new one has no such field). An outline with water, a river, a road or street, or a
building drawn by hand in it is refused with the reason. Plots are listed by `search`/`list {what: "names"}` (kind `plot`, the fill in `detail`). Returns the plot's feature (`c:<n>`, kind `plot`). `remove` and
`restore` work on it. (`set_plot` is the notebook's plot point.)

### `set_parcel {id}`
`id` (a plot, `c:<n>`) with any of `poly` (`snap` as in `set_parcel`), `fill`, `name`; fields left out stay. Checked as `set_parcel` is.

### `set_building {dry_run: true}`
Read-only: whether a footprint (`poly`, `rect` or `circle`, with `snap`) is clear for a building, as
`set_building` checks it. `id` is the building being reshaped (it does not count against itself). `clear: true`
counts the world's own buildings out, as `set_building clear` does. Returns
`{ok, blockers, ...}`: with `ok: true` its point `x_ft`, `y_ft` and the `name` it would get for `func`; with `ok: false`
a `reason` (the first blocker that stays, worded as `set_building`'s refusal) and `nearest_clear`; `would_remove` only beside `ok: true`.

- `blockers`: `{items, found, truncated?}` (50 at most), each `{what, id?, name?, function?, settlement?: {id, name},
  at: [x, y], width_ft?, kind?}`. `what` is `building` (`id` `b:<layout>:<id>`, or `c:<n>` when drawn by hand, with
  `name` and `function`), `square` (`kind`: market, civic, quay, green, yard, ...), `street` (`width_ft`, `kind`:
  street, main street or approach road), `wall` (a hand-drawn one with its `c:` id), `road` (`id` `rd:`, `kind`:
  the class, `width_ft`), `water` (`kind` sea or lake) or `river` (its `id` and `name` when it has them). `at` is a
  building's middle, else the point of the square, street, wall or road nearest the footprint's middle, in whole
  feet. With `clear` the world's own buildings are not blockers (a building the ward editor added is taken away too, as
  by `remove`). `truncated` is also true when more than 300 things were in the way and the scan stopped.
- `nearest_clear` (not when `ok`): the same footprint, not turned, moved on 5-ft steps to the nearest spot that is
  clear, up to 400 ft away: `{x_ft, y_ft, ft, poly}` (`x_ft`, `y_ft` its new middle, `ft` how far that is from the
  middle asked about, `poly` the moved corners: pass them back as `poly` with `snap: false`), plus its own
  `would_remove` with `clear`. A suggestion, not a move: nothing is changed. It is `null` when there is none (or the
  footprint is in the sea), with a `note`.
- `would_remove` (with `clear`, beside `ok: true`): `[{id, name, function}]`, the world's own buildings
  `set_building clear` would take away. With `clear` over more than 300 buildings the check is refused. A town laid
  out by hand (in Streets) is not listed: it lays its lots out anew round the new building, so its buildings in the way
  give way without a removal (and with new ids round it).

```json
{ "name": "set_building", "arguments": { "dry_run": true, "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 50, "depth_ft": 30 }, "func": "inn" } }
{ "name": "set_building", "arguments": { "dry_run": true, "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 170, "depth_ft": 130 }, "clear": true } }
```

### `list {what: "streets"}`
Read-only: the streets, walls, squares and roads inside an area, as the map draws them (`set_building` and
`set_building {dry_run: true}` refuse a footprint on any of them). `within` (required): a box, polygon or circle whose bounding box is at most
3,000 ft across (a larger one is refused). `kind` (optional): `street`, `wall`, `square` or `road`. `limit` (default
100, at most 1,000). Returns `{items, found, truncated?}`, settlement by settlement (streets, walls, squares), then the
roads; points are whole feet.

- `street`: `{settlement: {id, name}, site?, width_ft, kind, pts}`, `kind` street, main street or approach road
  (a road from a gate in); a street running through the area is cut to it, one item for each stretch inside.
- `wall`: `{settlement, width_ft: 9, pts, id?, name?}`, town and castle walls; one drawn by hand has its `c:` id and name.
- `square`: `{settlement, kind, pts}`, the whole outline of each square, quay, green or yard that touches the area.
- `road`: `{id: "rd:...", class, width_ft, pts}`, the world's roads outside and between the settlements, cut to the area.

```json
{ "name": "list", "arguments": { "what": "streets", "within": { "at": [412500, 380120], "radius_ft": 300 }, "kind": "street" } }
```

### `remove` (the world's own buildings)
Take away buildings of the world's own (drawn ones go with `remove`; a drawn building's id is refused, saying so): `id` or `ids` (`b:<layout>:<id>`),
and/or every one whose middle lies inside `within`: a polygon `[[x_ft, y_ft], …]`, a box `[x0, y0, x1, y1]` or a
circle `{at, radius_ft}`, such as a ward to clear for buildings of your own. Their ground is free to
build on; every other building keeps its id. Returns `removed` (the ids) and `count`. (`set_building` with
`clear: true` does it for the buildings under a new one, and lists them with names and trades.)

```json
{ "name": "remove", "arguments": { "what": "buildings", "within": [[412400, 380000], [412800, 380000], [412800, 380300], [412400, 380300]] } }
```

### `restore` (the world's own buildings)
`id` or `ids` (`b:<layout>:<id>`; all or none): the buildings back as generated, whether taken away or changed (a
rename stays). One id returns that building; several return `{ids, restored: [buildings]}`. Refused, nothing
changing, when one would come back under a building drawn by hand (after `set_building clear`):
`b:3:1186 would come back under c:21 (drawn by hand): remove c:21 first`.

### Worlds with only drawn roads
With the world parameter `generated_roads: false` (the app's World › Generate › People & places › Roads: Only
drawn), the world has only the roads drawn in its sketch, with short spurs to the settlements beside them, and no
roadside inns; settlements are placed without regard to roads, so drawing a road never moves one. Drawing a road
into a town lays the town out anew (its gates follow its roads): draw roads before changing a town's buildings.

## Crossings (bridges, fords and ferries)

Crossings put down by hand (`v:<id>`) join one bank to the other, drawn on the battlemap over what is there. A
bridge is a walkable deck clear of the water (timber, or old stone), a ford a bed brought up to wading depth under
stepping stones, a ferry jetties out from each end with a raft between. The ends are exactly where they are given
(never moved), but an end in a river's water is refused, naming the bank point on that side to use. An end on a rock islet (hand-made or the world's) or on a plateau over the water is dry land and passes. A hand bridge's deck climbs straight from the ground (or the water's clearance) at one end to the other, at most 1 in 8, so a long bridge can reach a cliff top (it runs into the cliff face where the climb is capped); shops on it stand at its level where they are. A pier may start on an islet. `set_ground {kind: "islet"}` reads the water as the battlemap draws it (a spot T0 calls dry but the fine shore floods takes an islet; one it floods but the fine shore dries is dry land). A crossing takes a
name and notes like a feature (`set_feature`, `get` with its `v:` id); a named one is
found by `search` (kind `crossing`) and listed in Notes › Gazetteer (nothing is lettered on the map).

A bridge up to 120 ft wide may carry **shops**: rows of houses and shops (drawn buildings, `c:<n>`, standing `on` the
bridge) along both sides of its deck, a lane left clear down the middle and 20 ft clear at each end. They are made with
the bridge (`shops`), follow it when its ends or width change, and are retired when it is removed. See *Shops on Wyrm's
Crossing* under Recipes.

### `list {what: "crossings"}`
Parameter: `limit` (1 to 2000, default 200; `truncated: true` when it cut the list). Returns `{items, found}`: `{id, kind, from, to, width_ft, length_ft, material?, name, notes?}` (`name`: as given, else
`Timber bridge`, `Stone bridge`, `Ford` or `Ferry`).

### `set_crossing`
| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | required for a new one | `bridge`, `ford` or `ferry`. |
| `from`, `to` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required for a new one | The banks (on the map). The length between them is 10–2,000 ft (a ferry at least 68). |
| `width_ft` | number | optional | Deck, ford or raft width, 5–120 (a ford or ferry at most 40; default 12; with `shops`: rows and lane, 50). |
| `material` | string | optional | A bridge's: `timber` (default) or `stone`. |
| `shops` | boolean or object | optional | A bridge's shops: `true`, or `{depth_ft, frontage_ft, lane_ft, vary}`: rows up to `depth_ft` deep (10–40, default 18) of lots along the deck (`frontage_ft` 12–40, default 20, for even rows) either side of a lane `lane_ft` wide (10–40, 14, or what the width leaves). `vary` (default true) makes each shop its own size: 14–32 ft along the deck, 60–100% of `depth_ft` deep (the first is the full depth, so the back line is ragged), 1–3 storeys, an open landing now and then; the same bridge always gets the same shops. `vary: false` gives even rows of `frontage_ft`, two storeys. A new bridge's width then defaults to `lane + 2 · depth` (50); a narrower one is refused, and so is a lane narrower than the deck keeps clear (`width_ft` less 80, at least 10). `false` takes the shops off. Only a bridge has them (a ford or ferry is refused), and one long enough for a row (`frontage + 40` ft). |
| `id` | string | optional | A crossing to change (`v:…`): only the fields given change. Without it, a crossing with both ends within 5 ft of an existing one's (either way round) is that one: `existing: true`; a different kind, material or width is refused (give its id to change it). |

Returns `{tool, id, kind, changed, crossing}` (`changed: true` when an `id` was given; `existing: true` for the same
crossing again), `shops` (the ids of the shops made; `shops_skipped` `[{id, why}]` lists a lot a building cannot stand on, such as one over another building, left out) and where the banks are: `river` `{id, name, width_ft, banks}` (the river it crosses, its width there,
and the two bank points across it there, the one on `from`'s side first; `id`/`name` null for a stream too small to be
named; `river: null` when none is within 1,000 ft), `from_bank_ft` and `to_bank_ft` (how far each end lies beyond its
bank), and `note` when both ends are on the same side. An end more than 5 ft into the water is refused (a fixed tolerance, whatever the width):
`from: (x, y) is in the water of The Red River (river:f32acc5b), 419 ft wide there: its bank on that side is at about (x, y)`.

A bridge's shops follow it. Changing a bridge's ends or width (`id`) lays its shops out again on the new deck, each side in order, keeping their ids, as deep and as long as they were: `shops_relaid` lists them, `shops_retired` those left without a lot. `shops` with an `id` replaces the shops: the old ones are in `shops_retired`, the new ones in `shops`.

### `remove` (crossings)
`id` or `ids` (`v:…`; all or none); a bridge's shops are retired with it (`shops_retired`). A mistake names the id's place in the list (`ids[1]: no crossing v:zz`; a single `id` is `id: …`).

## Quays, piers and breakwaters

Water's-edge works put down by hand (`q:<id>`), drawn on the battlemap and the three finest zooms. A **quay** is paved ground along a shore: its line is the water's edge and the paving reaches `width_ft` to the dry side (found once, from the ground at the middle of each side), the water inside it raised above the water. A **pier** is a deck over the water, lifted flat like a bridge's, with bollards along it. A **breakwater** is a raised strip of rubble (always stone), armoured along its wet sides. A pier or breakwater starts on the bank. A building may stand on a pier's deck or a breakwater (`set_building` says `on: "q:…"`), and a breakwater may carry a lighthouse at its tip.

### `set_wharf`
| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | required for a new one | `quay`, `pier` or `breakwater`. |
| `path` | `[[x_ft, y_ft], …]` | required for a new one | 2 to 16 points (each an id, point or `[x, y]`), each side at least 10 ft, 2,000 ft in all. A pier's or breakwater's first point is on the bank: more than 5 ft into the water is refused. With `shore`, a line drawn along the shore (up to 512 points, at least 20 ft long). |
| `shore` | boolean | optional | A quay's: follow the shore. `path` is snapped to the water's edge within 60 ft of it, smoothed, and the quay's face set 3 ft out over the water, with its paving `width_ft` to the dry side in a band of even width and round ends; the stored `path` is that face (points about 5 ft apart). No shore near `path` is refused (`path: no shore within 60 ft of the stroke: …`). A shore quay keeps `shore` when changed; a new `path` is snapped again. |
| `pier_spacing_ft` | number | optional | A shore quay's piers: 0 (default, none) or 30–400 ft apart along it, none within half that of its ends; each 40 ft straight out from its face, 10 ft wide, of the quay's material, where there is water under it. |
| `width_ft` | number | optional | A quay 10–60 (default 20), a pier 6–30 (10), a breakwater 12–60 (24). |
| `material` | string | optional | A pier's or quay's: `timber` (default) or `stone`. A breakwater is stone. A shore quay of timber is planked over. |
| `lighthouse` | boolean | optional | A breakwater's: a 3-storey `lighthouse` building at its tip, `on` it (a round tower at most 9 ft in radius, set back from the last point). `false` retires it. |
| `id` | string | optional | A wharf to change (`q:…`): only the fields given change; its kind cannot. A new `path` retires the buildings on it (a lighthouse is built again at the new tip). Without it, a wharf of the same kind with every point within 5 ft of an existing one's (a quay either way round) is that one: `existing: true`; another width or material is refused, naming it. |

Returns `{tool, id, kind, changed, wharf, river | water, bank_ft?}`: `wharf` is `{id, kind, path, width_ft, length_ft, shore?, pier_spacing_ft?, material?, level_ft?, buildings?}` (`shore` and `pier_spacing_ft` on a shore quay, which also gives its `material`) (`level_ft`: the top, above the sea, where there is water under it); `river` `{id, name, width_ft, banks}` and `bank_ft` (how far `path[0]` is from the bank) when a river is within 300 ft, else `water` `{kind, id, name}` (`null`: no water under it); `buildings` the lighthouse made, `retired` and `left_pointing` what was retired. A pier starting in the water is refused, naming the bank: `path[0]: (x, y) is in the water of Shadow Lake (lake:b4da59db): a pier starts on the bank: the nearest dry ground is at about (x, y)` (in a river: `… 419 ft wide there: a pier starts on the bank, at about (x, y)`). A lighthouse that cannot stand (something in the way) refuses a new breakwater, naming what; nothing is put down.

### `list {what: "wharves"}`
`within` (a box, a polygon or a circle) and `limit` (1 to 2000, default 200). Returns `{items, found}`: `{id, kind, path, width_ft, length_ft, material?, level_ft?, buildings?}`. The order is the geometry's: by path (point by point, to half a foot), then kind and width, the id last, so quays sharing a first point keep their order from build to build.

### `remove` (wharves)
`id` or `ids` (`q:…`; all or none). The buildings on them are retired in the same change: `buildings` `[{id, name}]`, and `left_pointing` says what still points at them (NPCs, plots, notes), as `remove` does.

## Underground sites designed by hand

An underground site (`u:<layout>:<k>`: a dungeon, crypt, catacombs, cave, mine or lava tube; not a city's sewers)
can be **redesigned**, by the user (the app's designer: *Edit › Rooms* inside it) or by agents, as a text
plan. The first change copies the generated site; from then on the world builds the design instead (walls are
always worked out from the squares, as for a generated site). Like the other edits, designs are saved, logged,
shown live (to the players' window too) and leave the world hash alone; `set_design {id, reset: true}` brings back the
generated site.

A design must keep the rules play mode relies on, the same ones every generated site is tested against:

- one way in (`exit`), on the top level, on the square under the entrance (`entry` in the header);
- a way `down` on every level but the deepest, and a way `up` on the level below, on the same square;
- every square of floor reachable from the way onto its level: through doors between rooms (natural levels:
  caves, mines, lava tubes open into each other), around props that block movement;
- props on floor, inside the grid, none on another.

A plan that breaks one is refused with the first five reasons and a count (`not saved: level 2: 12 squares of the tomb at 30,4 can't be
reached; … (and 6 more)`). No boss chamber, or one not on the deepest level, is only noted.

**The plan** (`get {view: "design"}` gives it; `set_design` takes it, or any part of it):

```text
site crypt · theme tomb · 36 x 30 squares · 2 levels · entry 2,15

level 1: Level 1 · 20 ft down
rooms: a=antechamber; b=embalming room; c=guardian hall "The Hall of Oaths"; d=ledge+5; f=passage
grid:
....................................
..aaaafbbbb........cccc.............
..aaaa.....fffffffccccc.............
doors: 5,1 e; 6,1 e secret; 17,2 e
items: exit 2,15; down 21,16; statue 20,19 back w; sarcophagus 8,21 1x2 variant 2; trap 14,18

level 2: Level 2 · the deep
…
```

- Levels are numbered from **1 at the top**. (Their rename ids, `l:<site>:<i>`, count from the bottom.)
- `rooms:` a symbol per room (`a`–`z`, `A`–`Z`, `0`–`9`, then `#$%&+?@<>~^*!`): its kind (as the generator names
  rooms: `guard room`, `crypt of the old lords`, `boss chamber`, `cavern`…), `+5`/`+10` for a raised floor,
  a shape in brackets, and a name in quotes (it becomes the room's rename): `c=sanctum+5 (round) "The Well of
  Stars"`. A kind the generator doesn't know becomes a chamber named as written (brackets that name no shape
  are part of it: `a=hall (east)`).
- Shapes are drawn only: `round` (a circle, or round-ended in a longer room), `octagon`, `apse n`/`e`/`s`/`w` (a
  round end on that side of the grid: `n` toward y 0), `chamfered` (corners cut), and `cavern` (rough rock
  breaking into a built level; underground only). Everything played stays on the squares, so a shaped room's
  squares must be the shape's: those whose middle lies inside it, in the box round the room's squares (a round
  room 7 squares across leaves out each corner square and the two beside it). A room whose squares don't fit
  its shape is drawn square, and the reply notes it. In the app's designer, choosing a shape fits the squares to
  it; `set_design` with `fit_shapes` does the same.
- `grid:` one row per line, one character per 5-ft square (`.` rock); `x` counts across from 0, `y` down from 0.
  Rows may be short (the rest is rock).
- `doors:` `x,y e` is a door between square x,y and the one east of it, `x,y s` the one south; `secret` makes it
  a secret door. `doors: auto` puts doors wherever a room would be shut off.
- `items:` `kind x,y [wxh] [back n|e|s|w] [variant 0-3]`: props by kind or name (`chest`, `sarcophagus`,
  `pillar`, `pit`, `trap`, `altar`, `brazier`, `well`, `cage`, `bookshelf`, `statue`, `mushroom`, `web`, `lava`…
  every prop underground; the designer's prop list shows them with their rules), and the ways `exit`, `up`,
  `down`. `back` is the side the prop stands against (north, east, south or west), for props that face one side
  (shelves, beds, hearths, statues, altars, things hung on a wall; others ignore it); left out, it finds its
  wall. `variant` picks one of its looks; left out, it is picked by position. The options come after the square,
  in any order.
- `+items:` and `-items:` change the list without replacing it (underground sites and building interiors alike): `+items:`
  adds the pieces given; `-items:` removes the piece of that kind whose top-left square is x,y (a size, if written, must
  match as well). The match is by kind and top-left square only: `back` and `variant` written in a `-items:` piece don't
  count. A `-items:` piece that matches nothing is an error naming it, with the nearest pieces of its kind. An empty
  `+items:`/`-items:` (or an empty piece between `;`) is refused. Underground, the way in (`exit`) and a way `up` under a
  `down` can't be removed (they would be put back, so the edit is refused); the way in always stays at the entry, and a
  way up follows the way down above it.
  Same entries as `items:`; several `+`/`-` lines (and an `items:`) in one block apply in the order written:
  `+items: well 3,0 2x2` / `-items: barrel 9,4; table 4,0`. `items:` alone still replaces the whole level's list.
- Where a level's floor isn't flat, `get {view: "design"}` adds under its `grid:` a `heights:` legend (`1=5 ft; 2=10 ft; /=ramp`)
  and a `height grid:` (one character per square, `.` at floor level or rock, `/` a ramp). It is a read-out:
  `set_design` ignores it. Heights come from a room's `+5`/`+10` and the raising items `platform` (5 ft),
  `balcony` and `scaffold` (10 ft) and `dais`; `steps`, `climb_ladder` and `rubble_slope` are the ramps that join two
  heights (a rise of more than 3 ft between squares with no ramp is a sheer edge: a climb up). A design with floor
  reachable only by such a climb is refused, naming the square and its height (`level 1: 9 squares of the junction
  at 2,7 (10 ft up) can be reached only by climbing a sheer edge`). All the new kinds, `back` included, read back as written.
- Only what the text gives changes: a level block with only `items:` replaces only that level's items.
- The header's `<n> levels` adds levels below the deepest (new ones start as rock: give their grids) or fills
  in the deepest ones. A missing `up` under a level's `down` and a missing `exit` at the entry are filled in.

### `get {view: "design"}`
| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | The site (`u:<layout>:<k>`) or a building (`b:<layout>:<id>`). |
| `summary` | boolean | optional | Only the levels, top first, each with its room count and room names (renamed or by kind); no plan text. |
| `format` | boolean | optional | Also give the full plan-format text (every item kind, door flag and rule) as a last content block. The tool descriptions carry only a short form of it, so `set_design` and this tool don't repeat it. |

Returns a summary (`designed`, kind, theme, grid size, entry, levels, `problems`) as the first content block (JSON)
and the plan as text in a second content block (a client that reads only the first sees no plan). A site nobody has
designed gives the plan of the generated site there (`designed: false`), to start from. `problems` lists the first
ten, then "and N more". A site's saved design made for a site that stood elsewhere (its way in is not under the
design's grid: edits kept for another sketch or generator) is set aside: the generated site stands, and the reply
says so (`set_aside`).

### `set_design`
| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | The site. |
| `text` | string | required | The plan, or the parts to change. |
| `furnish` | boolean | optional | Fill rooms with no props from their kind's kit (a boss chamber gets its dais, pillars and hoard). |
| `fit_shapes` | boolean | optional | Fit the squares of rooms given a shape to it, as the designer does (squares outside it go to rock, a building's to the room beside it); one that can't be fitted without breaking a rule stays as written and is drawn square. The reply's `fit_shapes` lists those, with why, and any items fitting took away (on a square left empty, in the rock underground, in a doorway moved, or across two rooms). |
| `dry_run` | boolean | optional | Check it and return what would change (the diff and the check), without saving. |

```json
{ "name": "set_design", "arguments": { "id": "u:168:0", "text": "site · 3 levels\nlevel 2\nitems: up 12,9; down 30,8\nlevel 3: Level 3 · the vault\nrooms: a=landing; b=boss chamber\ngrid:\n…\ndoors: auto", "furnish": true } }
```
> *"Dig a third level under the Fallen Shrine's crypt: a landing under a new stair from level 2, a flooded
> vault beyond it as the boss chamber, furnished."*

**The reply says what changed, not the whole plan** (`get {view: "design"}` has that): `changes`, by level, with only what is
there: `items_added`, `items_removed`, `squares_changed` (count and the rooms' letters), `rooms_changed` (a room's kind,
raise or shape), `doors_added`, `doors_removed`, `heights_changed` (count and the first squares, `2,7: 0 -> 10 ft`),
`renamed`, a building's `stairs`; `"no changes"` if none; and the check's `verdict` and `problems`. The same reply, brief,
when saved. With `dry_run` nothing is saved and the same diff comes wrapped: `{"dry_run": true, "would_save": {"changes": {…},
"verdict": …, "problems": […]}}` (a saved reply is the bare object). A refused plan is an error with the reasons.

### `set_design {id, reset: true}`
`id`: the site (or a building's interior) as generated again.

### `list {what: "designs"}`
The underground sites (`u:`) and buildings (`b:`) that have a design made by hand, in one `{items, found}` list (`limit` 1 to
1000, default 100): `{id, name, what: "site" or "building", kind, levels, x_ft, y_ft}` (`x_ft`/`y_ft` only while the site or building stands; one that is gone has `gone: true` instead; `world_overview`'s `designs_set_aside` also lists building designs that no longer fit, and a gone site only here). `within` (a box, a polygon or a circle
`{at, radius_ft}`) keeps those whose place is inside. `get` of one of them says `designed: true`; `set_design {id, reset: true}`
drops a design.

## Links below ground (paired exits)

A link put down by hand (`j:<id>`) joins a square of one place below ground to a square of another: a level of a
site underground (`u:`, a keep's deep dungeons `k:`), a sewer section (`w:<layout>:<sx>:<sy>`) or a building's
cellar (`b:`, a level below ground). Each end shows on its own map as a way across (item kind `link`, named by its
way) that names the far end, so both maps have it; furniture on the square gives way. Ways (`way`): `stairs`
(default), `ladder`, `passage`, `trapdoor`, `rope`, `portal`. An end is a level and a square: levels are counted from
the bottom (`l:<site>:<level>`, as level and room ids are), squares as `get {view: "design"}`'s plan numbers them (`x`
across, `y` down, from 0). Places a link names change only by its ends; `get` of a site or level lists its
`links`, `inspect` those with an end below the point. A site designed by hand (`set_design`) keeps
its links: they are put on its levels after the design is built, so a designed site can have as many extra ways out
(to another site, a cellar or the sewers) as it has links; an end whose square a new design fills with rock stops
showing (`list {what: "links"}` `check` says so) until it is moved. A remade world moves the ends with their places. In play
mode a token on a link's square steps across to the far end's square (the sewers: on the city's one sewer level).

### `set_link`
| Parameter | Type | | |
|---|---|---|---|
| `a`, `b` | `{at, x, y}` | required | Each end. `at`: a level (`l:<site>:<level>`), a room (`r:<site>:<level>:<room>`; with no `x`/`y`, its free square nearest its middle), or a site: `u:`/`k:` (its top level), `w:` (a sewer section's walkway), `b:` (its cellar); a site with no `x`/`y` takes the free square nearest the middle of that level. A city's whole sewers (`w:<layout>`, no `x`/`y`, one end only) take the free walkway square nearest the other end (within 3 sections, 1440 ft), so a site can be joined to its city's sewers without knowing the section. `x`, `y`: the square. |
| `way` | string | optional | How it goes: `stairs` (default), `ladder`, `passage`, `trapdoor`, `rope`, `portal`. |
| `name` | string | optional | Its own name, shown with the far end's. |

Both ends must be floor, off a building's stairs and off a way, key, hazard or picture already there, and not on
another link's square; a building's end must be in a cellar. Refused with the reason (`a: 0,0 on level 1 of u:0:400
is not floor`). Returns `{id, way, name?, a, b}`, each end `{level, site, x, y, name}` (`name`: the place's, as the
far end's way names it).

### `list {what: "links"}`
| Parameter | Type | | |
|---|---|---|---|
| `of` | string | optional | Only links with an end in this site (`u:`, `k:`, `w:`, `b:`) or level (`l:`). |
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | optional | Only links with an end below this place: in the sewer section under it, or in a site or building whose place is within 100 ft. |
| `check` | boolean | optional | Each with `problems`: what keeps an end from showing (its square no longer floor, a way now there, the place gone); empty when both show. |
| `limit` | integer | optional | 1 to 2000, default 200 (`truncated: true` when it cut the list). |

Returns `{items, found}` of links as `set_link` gives them.

### `remove` (links)
`id` or `ids` (`j:…`; all or none): both ends go. A mistake names the id's place in the list (`ids[1]: no link j:zz`).

## Building interiors designed by hand

Any roofed building (`b:<layout>:<id>`: the world's own and those drawn by hand) can have its interior
**redesigned** the same way, by the user (the app's designer: *Edit › Rooms* inside it) or by agents,
with the same three tools. The first change copies the generated interior; from then on the world builds the
design. Walls are where rooms meet: a room is split by giving a line of its squares to a new room (the wall
runs between them), and two rooms become one by giving one's squares to the other. The outer walls follow the
footprint, and windows are worked out as for a generated interior. The cellar's ways to other sites (a trapdoor
to the sewers, stairs down to a keep's deep dungeons) are put back on its free floor when it is built.

A design may have **no cellar, or up to three levels below ground** (generated buildings have one): the header's
`cellars <n>` digs new ones below the deepest (each a storeroom under the whole building, the stair block going
on down to it) or fills in the deepest, and a `level -2` block digs down to it. The ways to other sites go on the
deepest; a building that has one (a cellar on the sewers, a large keep over its deep dungeons) keeps at least one
cellar. Levels count from the bottom in level and room ids (`l:`, `r:`), so when cellars are dug or filled in
(here, in the app, or by going back to the generated interior) the names, notes and hidden marks of the other
levels and rooms, plots' anchors on them and NPCs inside move with them; those on a level filled in go.

A design is made for the building as it is: its grid, footprint and storeys. When the footprint changes (or the
building is made a ruin) the design is **set aside**: the generated interior stands, `get {view: "design"}` says so
(`set_aside`), `get` gives `interior_designed`, and `world_overview` lists it in `designs_set_aside`;
put the footprint back and it applies again. When only the storeys change (`set_building` with `floors`, on
the world's own buildings and those drawn by hand), the design follows in the same change: the floors both have
stay as designed, new storeys come as generated. The app's designer does the same (*Add a floor on top*, *Take
the top floor away*): the building takes the new storeys when the design is saved.

The rules (the ones every generated interior is tested against):

- one ground floor (`level 0`) with one front door, onto open ground (not into the building next door); doors
  to the outside (front and back doors) only on the ground floor, each onto open ground;
- the stair block (1 to 3 squares each way) on floor on every level it reaches;
- squares only inside the footprint;
- no furniture off the floor, on another piece, on the stairs or in a doorway (either side of a door);
- every room and every door reached from the stairs (on a keep's tower tops, from its spiral stairs), through
  doors between rooms, around furniture that blocks movement; rooms with no floor, `open to below` (a hall's
  height left open over it: a hall open two storeys, a shell keep's yard) and `roof` (a roof's slopes inside a
  keep's parapet walk), have no door and nothing on them, and aren't reached;
- the deepest cellar has free floor for its ways to other sites, and a building with such a way has a cellar;
- levels one above another from at most three below ground (a one-storey building without a cellar has no
  stairs: its ground floor is reached from its doors).

**The plan:**

```text
building · 7 x 6 squares · 3 levels · stairs 1,1 2x1

level 1: Upper floor
rooms: a=bedroom; b=bedroom
grid:
aaaaabb
…

level 0: Ground floor
rooms: a=main room; b=kitchen
grid:
aaaaabb
aaaaabb
aaaaabb
aaaaabb
.aaaabb
.....bb
doors: 4,2 e; 2,4 s front; 1,4 s back
items: hearth 1,0 2x1; table 2,3 2x1; chest 0,3; bench 3,0 2x1; oven 6,0 1x2; shelf 5,5 2x1 back s; barrel 6,4 variant 1

level -1: Cellar
…
```

- The header gives the grid, how many levels and the stair block (`stairs x,y wxh`: change it there to move or
  resize the stairs); `cellars <n>` (0 to 3) when it has other than one cellar, to dig or fill in cellars.
- Levels go by storey: `level -2` a lower cellar, `level -1` the cellar, `level 0` the ground floor, `level 1`
  and up the floors above (an open roof and a keep's tower tops are the storeys above its top floor; a keep
  built of pieces has its top, open battlements or a wall walk round its roof, as the storey above its top
  floor, each piece only as high as it rises: a forebuilding two storeys and a platform, wings a storey below
  the body, towers a storey above the piece they stand on). Those above ground follow the building's storeys:
  a plan can't add or take away floors (`set_building` does).
- A castle's keep is laid out as real ones were: a hall keep, L or T keep entered through its screens passage
  (a forebuilding's entrance hall before it), service rooms at that end, the great hall, the main stair in the
  landing behind its high end; a great tower its hall on the first floor over stores; about half the halls rise
  two storeys, the floor above them `open to below` with a `minstrels' gallery` over the screens end, reached
  along a mural passage. Upper floors have a mural passage along one long wall from the landing and the rest
  divided by cross walls into rooms no longer than 2.5 times their width (40 squares at most), every room on a
  passage or beside one that is; the great chamber (about twice as wide as deep) by the main stair, the solar
  beside the landing or beyond the great chamber's cross wall; the chapel, a treasury, an armory and a
  garrison on the floors above; no storey on the squares of the one below; tower rooms of their own kinds (an
  `oratory` among them). A shell keep's yard (and what is open over it) is drawn round, and so is the gallery
  round it on the floor above. Where `open to below` meets a gallery, passage or landing the wall is a
  railing: it stops a step, not the eye. A small tower keep (a body under 7 squares long or 5 across) has its
  stair against the end wall in a landing across it, an entrance hall over the stores, the great hall above
  and a room or two by the landing on each floor over it.
- Palaces and noble estates are laid out by zones too. The capital's palace is a grand palace symmetric about
  its axis, its front door on it: round a `courtyard` (open to the sky: a lawn, a fountain in its middle,
  `garden_path` gravel walks along the axis and across it, benches beside them; the battlemap draws the court
  the same; 7 squares across or more each way) where it is 17 squares across and 20 deep or more (always, as
  generated, whatever ward its patch is set to), else a front range and a back range; before its front a
  forecourt (2 to 6 squares deep) with a low garden wall round it and a gate on the axis between two piers,
  3 squares wide, its iron or timber leaves standing open, and a paved way as wide as the gate on to the
  nearest street where the gate doesn't open onto one. On the ground floor the
  `entrance hall` with the grand stair on the axis, a `guardroom` either side and a room in each corner, the
  service wing (`servants' hall`, pantry, kitchen) down one side of the court and the `royal chapel` down the
  other, the `banqueting hall` across the back between the buttery and a vestry; on the state floor the
  stair's landing, a `salon` over the entrance, guard rooms and `antechamber`s, the `audience chamber` and the
  `ballroom` down the sides and the `throne room` across the back (a guard room before it), the state rooms
  opening one into the next from the landing round to the throne room both ways; above, the `long gallery` (3
  squares across or more) across the front and the royal apartments round a corridor that rings the court (the
  great chamber and the bedchamber the floor's biggest rooms but the gallery, at the side ranges' front ends,
  across the back where the side ranges are narrow; small wardrobes beside them, studies, guest chambers, a
  library),
  servants' quarters and stores in the attic. Other palaces (a metropolis's that is not the capital's, and
  about two cities in five have one: the biggest noble home, three storeys; always smaller than the least a
  capital's is) are older ones round their great hall, a big keep without the defences: its great hall their
  banqueting hall too, the throne dais at its high end and the high table before it; a guard room by the entrance, a guard room and a chapel
  off the dais end, the audience chamber and the solar by the stair, a long gallery down the top storey, a
  treasury. A
  noble estate of 40 squares or more (8 squares long, on a lot near enough a rectangle; other estates are laid
  out as other houses are) is a hall house (the hall between a small service end and a parlour by
  the stair, chambers and the solar above) or, from 130 squares, a manor (a service wing at the low end deep
  enough for a kitchen, a buttery and a pantry, the hall 30% of the ground floor at most, a cross wing for the
  family at the high end with a parlour, a `stair hall` and a study, a chapel from 260 squares; the great
  chamber and the solar above, chambers, guest chambers, a library; the map and the battlemap roof its cross
  wing apart, its ridge across the main range's); passages in an estate are corridors.
- Temples, shrines and monasteries are laid out by zones too. A temple stands on the largest rectangle its lot
  holds, its door at the end whose way out reaches a street, the end nearer one first (else its apse toward the
  east), the lot's ground round it open (its own: no neighbour's door opens onto it). The front door of every
  temple, shrine and monastery opens onto ground that reaches a street (with no street near, as a hamlet's
  shrine, onto open ground clear of its neighbours, on a side no house stands against if it has one); a shrine
  shut in a block's middle changes places with the nearest home that has a way out.
  On one 13 squares by 9 or more it is a cross: the `nave` (5 squares wide or more, an aisle each side of its
  arcades) from the door through the crossing to the `sanctuary`, the transept's arms two squares out each way
  (three from 13 across), a `side chapel` in each arm's end toward the apse, a `vestry` in the other end of
  one arm and the stairs down to the crypt in the other's; the sanctuary across the body's end, rounded by the
  apse (`apse` shape). An altar rail parts the sanctuary and each side chapel from the nave (a railing: it
  stops a step, not the eye), its gate on the axis. On a smaller one (8 by 5 or more) it is a single hall
  ending in its apse, the stairs down along a wall by the door; a smaller lot keeps its rectangle, laid out as
  other buildings are. The map and the battlemap draw a cross's nave under one ridge to the apse's half cone,
  the transept across it and a squat tower's spire over the crossing; a hall under its ridge and the apse's
  half cone. A shrine is one `shrine` room, its stairs from a corner by the front wall; in a town or a city, on
  a lot 8 squares by 5 or more, a small hall ending in its apse (5 squares wide, 10 long at most), drawn as a
  small temple's. A monastery takes in
  the homes beside it for a footprint round a cloister (as a palace does; else clearing more of its block, its
  alleys too, on ground fit for a terrace), up to 8,000 sq ft, 12 squares by 12 or more; a town laid out anew
  keeps it whatever its patch is laid out as. Its church is a single hall along one long side; across the
  cloister walk (a way about the floor) from it the open `garth` (gravel walks across it to the
  `cloister_well` in its middle; the battlemap draws it the same, nothing else on it), an arcade between them;
  round the walk the `entrance hall` from the front door, the `scriptorium`, the `kitchen` by the
  `refectory`, the `chapter house` (drawn as an octagon) beside the church and the `dorter`; above, the
  church's height open, nothing over the cloister, a library, an infirmary, chambers and the novices' dorter
  one into the next from the landing; below, the crypt under the church only, its stairs down from the entrance
  hall. The map and the battlemap draw the church a storey higher than its ranges, a bell-cote on its west
  end. Every room of a temple, shrine or monastery is furnished so that each square of it nothing stands on is
  reached from its doors. A monastery with no room for a cloister is laid out as other buildings
  are.
- A palace (and a monastery, for its cloister) takes in the homes of its ward (a monastery's, any homes) beside
  it on its block for a bigger footprint, as long as the whole crosses no street or alley: it stands on the largest rectangle they hold, square to its
  own lot. Where that gives the capital no footprint round a court it clears more of its block: the homes and
  the open ground there beside its lot. The capital's front is the side it stands round its court from, an odd
  number of squares across (an even side loses a square), with room for its forecourt (drawn back from it by
  up to 4 squares for one 4 deep), with its way on to a street, nearest a street; the end of a street that
  runs up to the front stops at the forecourt's outer wall. The lots taken in are gone with their ids; every
  other building keeps its own, and the palace keeps its id and name; nothing overlaps it. A town laid out
  anew keeps its palace's footprint where the lots under it are homes of its ward again (any homes, the
  capital's, whatever its patch is laid out as: its patch's streets stop at it and at its forecourt, the homes
  on its forecourt and its way gone); a monastery's as the capital's. A temple is shaped again from its lot.
- Great halls are furnished as one composition about the axis from the end they are entered at to the far
  end: a castle's (its throne dais of stone steps and gilt thrones in a capital, a pair, and in rich cities,
  else of timber, now and then with a lesser chair), a town hall's, a guild's, a harbor master's or an
  auction house's (a `low_dais` with a `chair_of_office` at its middle), a noble estate's (a timber
  `low_dais` carrying the `high_table`, the chair of office behind its middle), a barracks' mess hall and
  any other hall (a guard post's, an orphanage's...: tables only); a palace's throne room (the capital's pair of
  thrones on a stone and gilt dais, standing banners and braziers beside it, the runner up to it, pillars,
  banners and tapestries, no tables), its banqueting hall (a stone `low_dais` carrying the `high_table`, long
  tables both sides) and its ballroom (the `dance_floor` in the middle, the `musicians_corner` on the axis at
  the far end with a `harpsichord` each side, `refreshment_table`s by the long walls at the door end), its
  audience chamber (the chair of state on a stone `low_dais` centred on the far wall, standing banners either
  side, the runner up to it), its long gallery (statues, benches and tapestries along its long walls only, the
  walk down its middle clear) and its chapel (the altar on a stone step against the far wall, east where it
  can be, pews in two blocks facing it either side of the aisle, the font by the way in); an older
  palace's great hall has a throne dais as a castle's. A temple's rooms are furnished so too, every piece of
  its god's kind (`variant`, by the god in its name): its nave (a big one's arcades a pillar pair every second
  row, the crossing's four piers, the `carved_screen` across the nave on the row before the crossing, from the
  piers' line to its open `screen_gate` on the axis (a thin partition drawn along its edge toward the
  crossing), `candle_stand`s either side of the gate beyond it; pews either side of the aisle up the middle
  facing the altar, the runner up the axis through the gate, the font by the door), its sanctuary (the `god_statue` on the axis toward the apse,
  the altar two rows before it on a stone step, candle stands either side), each side chapel (its `side_altar`
  against the far wall, a candle stand by it), a small temple's hall (its far end as a sanctuary, then the
  pews), a shrine (as a sanctuary, a pew or two each side) and a monastery's church (as a small temple's hall) and
  refectory (a timber `low_dais` across its far end carrying the high table, the abbot's chair behind it, long
  tables and benches down its axis and both sides). The dais stands on the axis against the
  far wall, a low dais as wide as the tables' lanes (a door on the far wall may open onto it: it is walked
  over), a throne dais a row in from the wall where a door opens there behind it (the way round it); a
  `carpet_runner` up the axis to it; long tables with benches both sides, pillar pairs where a column is free
  by the long walls (off the rows of the hearth, of what hangs on the walls and of the doors there, out of
  every door's way, the way by the wall kept clear), standing banners and braziers either side of a throne,
  a castle's high table before the dais or beside it, wall banners and tapestries along the long walls, all mirrored across the axis; the hearth on a
  long wall. Every banner in a castle's buildings flies the castle's one house; an open hall's minstrels'
  gallery holds a `musicians_corner`. Thrones stand only in castles and palaces; `furnish` in a design
  furnishes a room as the generator would in that building (its great hall as a whole, in its style, in the
  frame its zones gave it where the room is as generated).
- `grid:` as for a site, `.` outside the walls.
- `doors:` `x,y e|s|w|n` is a door on the east, south, west or north edge of square x,y; `secret` makes a door
  between rooms a secret door; on an outside wall `front` (one, on the ground floor) or `back`. `doors: auto`
  keeps the level's doors to the outside and adds inner doors wherever a room would be shut off.
- `items:` furniture by kind or name: `bed`, `table`, `chair`, `long_table`, `bar`, `counter`, `hearth`,
  `oven`, `shelf`, `bookcase`, `chest`, `wardrobe`, `barrel`, `crate`, `workbench`, `forge`, `anvil`, `altar`,
  `pew`, `statue`, `rug`, `couch`, `desk`, `cage`, `spiral_stair`… (the designer's furniture list shows them
  all, with cover and whether they block movement); for great halls and ballrooms (the designer's "Great hall"
  group): `throne_dais` (w×2 against a wall, difficult ground, half cover), `low_dais` (a platform 1 ft
  high, difficult ground: the one piece another may stand on, a chair of office or a high table on it; it
  may lie in a doorway),
  `chair_of_office`, `high_table`, `carpet_runner` and `dance_floor` (walked over), `tapestry` and
  `wall_banner` (hung on the wall they stand against, walked under), `standing_banner` (blocks movement, not
  sight), `font`, `cloister_well`, `harpsichord`, `musicians_corner` (difficult ground), `refreshment_table`;
  a temple's (the designer's "Worship & tombs" group): `god_statue` (total cover, blocks movement and sight),
  `candle_stand` (blocks movement, not sight), `carved_screen` (three-quarters cover, blocks movement and
  sight), `screen_gate` (the screen's open gate, walked through as open floor), `side_altar`, each with a look per kind of god (`variant` 0 the Sun and the Morning Lord, 1 the
  Dawn, 2 the Moon and the Veiled Lady, 3 the Deep Sea, 4 the Storm, 5 the Forge, 6 the Harvest and the
  Hearth, 7 the Seven and the Watcher); indoor props, with the rules they have underground:
  `sacks`, `rubble`, `timber`, `bones`, `skeleton`, `skulls`, `urn`, `coffin`, `effigy`, `brazier`,
  `candles`, `sconce`, `lantern`, `banner`, `chains`, `cobweb`, `bedroll`, `tools`, `powder`, `hoard`,
  `offering`, `glyph`, `dais`, `well`, `fountain`, `iron_maiden`, `stocks`, `rat_nest`, `trap`; and uploaded
  sprites as `s:<asset id>` (as `list {what: "sprites"}` gives them; underground sites take them too). A sprite keeps its
  rules: cover, blocking movement; one that blocks movement blocks sight from 6 ft up (a sprite that blocks
  sight but not movement doesn't indoors), one that doesn't is difficult ground when marked so. `back` and
  `variant` as for a site: `shelf 5,5 2x1 back s` stands with its back to the south wall; a sprite's picture is
  turned to put its top (north) edge against that side. A few pieces have more than four looks: a
  `throne_dais` 0-5 (one throne, a pair, a throne and a lesser chair: 0-2 on two stone steps under a carpet with
  gilt thrones and a cloth of estate on the wall, 3-5 on a plain timber platform with oak thrones); a
  `wall_banner` and a `standing_banner` 0-7, a house's colours (the same number on both, so a hall's banners
  can share one house; a banner with no variant flies its building's own house, so every banner left to
  itself in one building matches); a `low_dais` 0 stone, 1 timber. A look past a piece's last is refused.
- Room kinds as the generator names rooms in buildings: `main room`, `kitchen`, `bedroom`, `common room`,
  `storeroom`, `shop`, `workshop`, `forge`, `nave`, `great hall`, `guardroom`, `study`, `library`… A kind it
  doesn't know becomes a chamber named as written.

```json
{ "name": "set_design", "arguments": { "id": "b:23:70", "text": "level 0\ndoors: 4,2 e; 2,4 s front; 1,4 s back" } }
```
> *"Give the house by the well a back door out of its main room."*

A building's `a=room+5` raises have no `heights:` read-out and no climb check (those are underground only); the diff
shows them only as `rooms_changed`.

## Towns laid out anew (the ward editor)

A town or city is laid out on patches, as in watabou's city generator: Voronoi cells that share their corners,
each a ward (merchant, craft, common, noble, slum, docks, military, temple, castle, the market square, parks,
farms outside the walls) cut into lots. The ward editor changes that layout without starting it again:

- **Corners move** one at a time or by brushes, as the generator's Warp tools move them: every patch that meets a
  corner bends with it. A corner goes only as far as keeps every patch round it convex (all the way, half, a
  quarter, or not at all), and at most `max_move_ft` (one patch's spacing) from where it was laid out. Corners on
  the water or a river are pinned. Gates move with their corner, and the roads come in to them.
- **Patches** take another ward (`empty` leaves the ground open, for buildings of your own), a lot size (`small`,
  `medium`, `large`, `huge`: the target lot area, times the ward's usual), a merge with a neighbour (it joins
  that district: same ward and lots, no street between; not across a main street), or a new roll (`reroll`: its
  lots and buildings drawn again). A `castle` ward builds a castle as one drawn by hand is built (see *Castles and
  walls*), but its curtain follows the brow of the ground (where it falls away from the top, keeping at least half
  the block, and 16 ft back from a river; a corner sharper than 40 degrees cut back where it is 60 ft across), the
  town wall is its wall on a side where that wall runs the whole way as built (a stream's water gap in it too), with
  no main street running between
  (a main street on the wall's own line with no paved strip inside it doesn't count) and no town gate on it but at
  its end (the castle's corner cut back from that gate, which opens onto the town), and ranges stand against it there
  too; elsewhere its curtain stands back with the wall's street between, never within 30 ft of the town wall
  alongside it; it keeps clear of the roads through its patch (a road found running through its ring cuts the patch
  back, its larger side kept); its towers stand 12 ft clear of the town's (a town tower crowding one of a castle the
  town planned goes, else the castle's); its gatehouse is on a side by a street outside it, at least 90 ft from a
  town gate where one fits (facing the town's middle among those; a way to the nearest street where the curtain stands
  back up a hill, out from the gate first where the straight way is blocked, else round the buildings in the way, the
  gate moved where no way can be laid, and
  nothing built on it later), and the inner curtain is a capital's wherever one fits, or a big castle's (170,000 sq ft
  or more: about half). Taking the town wall down changes only the castle's own buildings: what the patch's own
  buildings did (a smithy, an inn) moves to the town's buildings near it, never to a castle's range. A `park` ward is a whole block laid out as a garden, no lots on it: a
  **formal** garden (cross walks from street to street and a walk round it, a fountain where they meet, hedged
  parterres with beds, topiary, raised planters or bosquets of trees, statues in pairs at the walks' ends, benches)
  or a **common** (winding gravel paths from its streets to a hub with a fountain or statue, a pond on its low
  ground with reeds and a walk round it, sometimes a stream into it with a footbridge, a gazebo or bandstand on a
  rise, clumps of trees, benches); `park` sets which (left out, `auto`: formal by the castle and the nobles, now
  and then in a capital or a metropolis, where the ground is level enough for a terrace, a common elsewhere), and
  `reroll` lays it out another way (moving its corners keeps its looks). A formal garden whose ground varies 8 ft
  or less is laid level, eased to its streets (cut or filled no more than a quarter of the way in, to 8 ft); one
  set by hand on ground that varies more follows the ground. Its ways in are on its sides along a street. A park
  goes only on a patch inside the town, its block 9,000 sq ft or more and 60 ft across, no cliff across it, 15% of
  it on the water at most, with a street along one side at least (`park_refused` says why not). Each town is
  generated with its parks: a metropolis one to three, the first on the patch it always kept open and its palace
  garden, as a capital city's, formal where the ground is level enough; a city most often one; a town about one in
  four, a small common. A patch too steep to build on inside the town (its grade 0.18 to 0.4) is a common on the
  slope (no pond or rise: a gazebo on its top with a bench looking down), planned as a `park` ward and counted
  among them, two at most, none beside another; any other steep block is a terraced garden with a house here and
  there, as before.
- **Squares** open where three streets or more meet between built-up patches (a corner's `junction`, inside the
  walls and off the water): the lots at the corner of every block round it are trimmed back, and the ground they
  leave is a `crossroads` (widened, with a signpost, lamps, a trough), a `well` square (the well on a plinth), a
  `fountain` square or a small `green` (the biome's trees, a path before the houses; its lanes stop at its edge). A
  temple's patch can have a **close**: open ground in the middle of its block with the temple, a standing cross
  before it and a hedged churchyard behind it, inside a ring of houses a lot deep along its streets, cut back to face
  it (only where the town's temple stands on that patch, the temple and churchyard fit inside the ring, houses stand
  along at least 65% of its street fronts, and no square opens at its corners). The close's temple keeps the old
  temple's id, name, notes and NPCs; changes made to the old building (storeys, footprint, roof…) are set aside, as it
  no longer stands where it did. Each town is generated with a few (a town sometimes one, a city one or two, a
  metropolis two or three, each a different kind); set others or take them away by hand.
- **Walls** go up round a town that had none, or come down (the strip they stood on is a street round the town;
  the gates stay where the roads come in).
- **What stays:** the town's plan (gates, main streets, districts and their names, the towers along each wall).
  Only the patches touched are built again. A building that comes out as it was generated keeps its id
  (`b:<layout>:<id>`) and everything decided about it; new ones get ids of 1,000,000 and up (by patch; in
  a patch rolled again, 100,000,000 and up, by patch and roll), so a change to one ward never renumbers another's
  and a roll's buildings never take the last roll's ids (except a business, which keeps its id and name). A lot
  trimmed back by a square or a close keeps its building's id and the changes made to it (storeys, roof, function);
  an outline drawn for it by hand is set aside (`buildings_set_aside` says so). A business the town lost with the
  buildings taken away goes to a new building about a patch from where it stood, else to the nearest home there,
  which takes the business's id and name (a home's own id goes, as a building taken away does); `functions_lost`
  lists the ones that found none. Renames, notes, NPCs and building changes on buildings that keep their ids stay
  with them.
- Villages have no wards (they grow along their roads). A town's first edit keeps the roads it was laid out from
  (`roads` in its edit): while the edit stands the town is planned from them, so a road drawn into it later (a sketch
  road, then `regenerate`) leaves its patches, wards, gates and edits as they were. The new road comes in by the
  nearest gate it can reach round the town (never through it); where none can be reached it stops at the outskirts:
  put a gate in where it comes in (`walls.gates_add`), and the road comes in by that gate (no way out of its own,
  `way_out_ft` 0). A road that moved a little (250 ft or less at the town's edge) keeps its gate. `reset: "all"`
  drops the edit, and the town is laid out anew from the roads as they are. Edits made before these were kept lay
  the town out anew as before.

### `get {view: "town_plan"}`
A town's plan: `patches` (`patch`, `at`, `corners`, `ward`, `ward_generated` when set by hand, `in_town`,
`district` with its id, `neighbours`, `lots`, `merged_with`, `reroll`, `lots_like` for a patch sharing another's
lots, `close` for a temple's with a close as built, `close_generated` when set by hand, `close_refused`: why one
can't be set there, or why the one set isn't built, `park` for a park's patch: `formal` or `common` as laid out,
`park_by_hand` when set by hand, `park_refused`: why the patch can't be made a park, `ward_planned` for a park's
patch built on as a common ward's, too small once its streets were cut), `corners` (`corner`, `at`,
`planned` when moved, `pinned`, `gate`, `wall` (with `wall_next`: its neighbours along the wall `[prev, next]`, null at an open wall's end; `tower_removed` where the tower standing at that corner was taken down by hand), `junction` where a square can open, `square_refused` where streets
meet but none can (a close's corner), `square` and `square_ft` (how far out from the corner its chords are cut,
on average) for the one there, `square_generated` when set by hand: a kind or `none`), `districts` (`id`, `name`,
`kind`, `patches`; `by_hand` for one made by hand, `changed` when its patches or their wards were changed by hand),
`max_move_ft`, `planning` (how planned its streets are: `value` 0-1, `class` `grown`, `mixed` or `planned`,
`rolled`, `set` when set with `edit_town`), `walls` (`built`, `generated`), `edited`, `set_aside`, `places`: its squares, parks and gates (`id`,
`kind`: `square`, `park` or `gate`; `sort`: a square's `market`, `civic`, `fountain`, `well`, `crossroads` or `green`,
a park's `formal`, `common`, `cemetery` or `horses`; `name`, `generated`, `x_ft`, `y_ft`, `size_ft`, `patch`, `district` with
its id, `main` for the square the main streets lead to). Their ids are kept by what they stand on (a patch, a corner,
a road, a palace), so an edit elsewhere in the town or the walls set on or off move none; a gate is there only
while the town is walled. `get`, `set_feature`, notes, `list {what: "names"}` (`kind` `square`, `park`, `gate`)
and `search` take them as other features.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | The town or city (its feature id) |
| `within` | `[x0_ft, y0_ft, x1_ft, y1_ft]` | | Only the patches whose middle is inside, and their corners |

```json
{ "name": "get", "arguments": { "view": "town_plan", "id": "city:bab44567", "within": [1288000, 2229000, 1289500, 2230500] } }
```

### `edit_town`
Change a town's layout. Every field is optional; they apply in this order: `reset`, `moves`, `equalize`,
`relax`, `patches`, `squares`, `walls`. `relax.at` is a place (an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`); the arguments
are checked against the schema as everywhere else (`walls` is a boolean, `"auto"` or an object of `built`, `gates_add`,
`gates_remove`, `towers_add`, `towers_remove`). The reply: `corners` (`asked`, `moved` all the way, `part_way`, `stayed`,
and the ones that fell `short`), `buildings` (`before`, `after`, `added`, `taken_away`), `walls` (`built`, `towers`,
`gates`), `squares` (the kinds of its open spaces, quays aside), `parks` (how each is laid out), `patches_changed` (`patch`, `what`: which of `ward`, `lots`, `merge`, `reroll`, `close`, `park`, `corners` changed on it, compared with before the call), `functions_lost`, `unmerged` (merges dropped because
a ward no longer allows them), `districts_changed` (`patch`, `from`, `to`: district ids), `renames_carried` (`from`,
`to`, `name`: a building renamed or noted that the change took away hands its name and note to the new building on
its patch with the same function, a castle's to the new keep), `renames_lost` (`id`, `name`: those with none; the
rename stays on the old id), `rects` (where the map is drawn again). A square or park whose patch or corner the
change takes away goes with its id (and its name and note stay on that id); the rest keep theirs. A square set by hand where the wards no
longer leave three streets goes with the change that took them. `added` and `taken_away` count buildings by id
and footprint (a patch laid out again keeps reusing its new buildings' ids). A square set or taken away changes only
the lots at its corner: they are trimmed back to it, or come back whole.

Each town and city rolls how planned its streets are (`planning`, 0-1; `get {view: "town_plan"}` and `get` report
it): most are grown (under 0.3: winding streets, irregular blocks), some have a planned quarter beside their grown
core (0.3-0.7: a side of the town laid out as a grid, larger the higher the value) and a few are planned throughout
(0.7-1: a regular grid of streets, square blocks or long ones between avenues, the market square its middle block).
Rivers and shores still shape the blocks along them, and the main streets still come straight in from the gates.
`planning` sets it (`"auto"`: back to as rolled); it lays the town out anew, so it is given alone, and the changes
made inside the town are set aside as a culture change sets them aside. The reply: `planning` (`from`, `to`,
`class`, `unchanged`, `set_aside`, `set_aside_by`). Villages and towns laid out by hand (`edit_streets`) have none.

`pattern` sets the street pattern of the planned part: `grid`, `rings` (concentric rings; round a bay, arcs
round the harbour with streets down to the quays), `radial` (streets radiating from the centre) or `linear` (one
long high street along the valley, ridge, hillside or river, back lanes behind its long narrow plots, short cross
lanes); `"auto"` (the default) picks one from its land and culture (a hill top or a northern city rings, a bay
town harbour arcs, a valley or hillside town or some river towns linear, a southern city radial, most towns a
grid; harbour arcs only where the shore curves round the harbour, not on a straight coast). Patterns show
only in a town's planned part: a pattern given by hand is followed at least as strictly as planning 0.6 (a
small grown core), more if `planning` is higher. It is given alone or with `planning`, with the same effect: the town is laid out
anew and the changes made inside it are set aside. The reply carries `pattern` (`from`, `to`, `unchanged`,
`set_aside`, `set_aside_by`) beside `planning`, each only when given; a `dry_run` reply is the same (with
`dry_run: true`, nothing saved). Once the ward editor (or `edit_town`'s other fields) has changed a town on
`"auto"`, it keeps the pattern it was laid out on while that edit stands, so a change to the land round it
doesn't lay its streets out anew. Villages and towns laid out by hand have none.

Gates and towers go in and out of the town's own wall by its corners (`get {view: "town_plan"}`: corners with `wall: true`;
`gate_by_hand`, `gate_removed`, and `walls.by_hand` for what was set). A gate put in takes the town's gatehouse
(its culture's form: square tower, twin drums, arched, bent, log), the id `gt:<layout>:<corner>` and a name like
the others, and a track (12 ft) out through it to the nearest approach or road, or across the outskirts to the
open country; it is refused (in `wall.refused`, with why) where there is none, at a corner on the water, at the
wall's end, on a castle's curtain, under 80 ft from another gate, or where the wall's sides there are under 56 ft.
A gate taken out closes the wall: its road stops 60 ft or more outside (or at a junction just past that), its main
street inside, and its ground keeps clear as with the gate (nothing built nearer the wall). A tower goes up `f` of the way
along a side (refused within 20 ft of another tower or a gate, over a building, or over a street crossing the
wall) and comes down by its corner or its id (a gate's towers go with the gate); a corner's tower taken down goes
back up by its corner. Every other gate and tower keeps its id, renames and notes (a tower put up by hand is
`t:<layout>:1000000` and on; a hand gate's two `t:<layout>:2000000 + 2 × corner` and `+ 1`), and walls taken down
and put up again keep them. The reply's `wall`: `gates_added` (`corner`, `id`, `name`, `way_out_ft`, `towers`),
`gates_removed` (`corner`, `id`, `name`), `towers_added` (`id`, `x_ft`, `y_ft`), `towers_removed` (ids), `refused`
(what, `why`).

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | The town or city |
| `moves` | `[{corner, to \| by \| as_generated}]` | | `to: [x_ft, y_ft]`, or `by: [dx_ft, dy_ft]` from where it stands now; `as_generated` puts it back |
| `equalize` | `[patch]` | | Pull each patch toward a regular polygon (its corners move) |
| `relax` | `{at, radius_ft, amount}` | | Each corner within `radius_ft` of `at` toward the middle of its neighbours, by `amount` (0–1, default 0.5) at the centre, less toward the edge |
| `patches` | `[{patch, ward, lots, merge_with, reroll, close, park, district, as_generated}]` | | `district`: a district's id (`d:<layout>:<n>`: any patch to any district, apart or not; a patch outside the walls once it has a ward built on, such as open country made `slum` or `common`, which also joins its neighbour's district by itself), a name (the district of that name now, any case, else a new district so named: new ones follow the generated ones, so every district keeps its id; one emptied keeps its id with no patches or label) or `"auto"`; a district whose patches change takes its kind from its wards by area (the centre stays the centre) and its label at its largest piece; `set_feature` renames one; `ward`: plaza, castle, temple, merchant, craft, noble, common, slum, docks, military, farm, park, empty; `lots`: small, medium, large, huge; `merge_with`: a neighbouring patch or `"none"`; `reroll: true` rolls it again; `close`: a close round a temple's patch's temple (`true`) or none (`false`); `park`: `formal` or `common`, a park's patch only; `"auto"` puts a field back |
| `squares` | `[{corner, kind}]` | | `kind`: crossroads, well, fountain, green, `"none"` (no square there) or `"auto"` (as generated); a square opens only at a `junction` |
| `walls` | `true` \| `false` \| `"auto"` \| `{built, gates_add, gates_remove, towers_add, towers_remove}` | | Walls up, down, or as generated; or by corner: `built` as the plain value, `gates_add`/`gates_remove`: `[corner]`, `towers_add`: `[[corner, next corner, f]]` (`f` 0.05–0.95, default 0.5) or `[corner]` (a tower taken down there, back up), `towers_remove`: `[corner \| "t:<layout>:<n>"]` |
| `members` | `{add, remove}` (lists of ids) | | What belongs to the town, by hand (see below): `c:<n>` drawn things (`add` joins, `remove` leaves), `b:<layout>:<id>` generated outskirt buildings (`remove` pushes them out into a new place, `add` takes them back). May be given alone |
| `reset` | `"all"` \| `"corners"` \| `"patches"` \| `"squares"` \| `"walls"` | | Back to as generated (`walls`: up or down and every gate and tower set by hand) |
| `planning` | number 0-1 or `"auto"` | | How planned its streets are, given alone (with `id`, `dry_run`): see below |
| `pattern` | `"auto"` \| `"grid"` \| `"rings"` \| `"radial"` \| `"linear"` | | The street pattern of its planned part, alone or with `planning` (see above) |
| `dry_run` | boolean | `false` | Report without changing anything |

```json
{ "name": "edit_town", "arguments": { "id": "city:bab44567", "moves": [{ "corner": 41, "by": [40, 25] }], "patches": [{ "patch": 2, "ward": "castle" }] } }
```
> *"Put a castle on the ward east of Agentholm's market, and take its walls down."* · *"Make the docks' lots
> bigger and round off the square by the north gate."*

Membership: a thing drawn by hand (a building, a site) belongs to a town when it stands within 300 ft of the
town's built edge or of another of its members, so a row of houses drawn outward chains on. It then takes the
town's look and culture and counts in its lists (sites, shops, people), and a cellar of its own links to the
city's sewers where it is in reach of the paved streets. `members` is the hand choice, and it beats distance:
`add` joins a thing however far, `remove` keeps it out however near. A generated building outside the walls
(sprawl, farm, rural, or in no quarter) can be pushed out with `remove`: the call makes one new place (kind
`place`, named in the town's culture, shown as a village) that takes them as its own, and the reply says so.
Buildings inside the wall ring or a quarter are refused, each in `members_refused` (`id`, `why`). The reply's
`members` says what joined, left or moved out; `dry_run` reports without changing.
`members` alone also works for a town laid out by hand and for a place (`id` a place's id), which is how `add`
takes buildings into a place; a generated building already in a place stays there (refused as already out).
With other edits in the call the town must be a generated one.

```json
{ "name": "edit_town", "arguments": { "id": "city:6c30c809", "members": { "add": ["c:12"], "remove": ["b:5:340"] } } }
```

```json
{ "name": "edit_town", "arguments": { "id": "city:6c30c809", "walls": { "gates_add": [42], "towers_add": [[92, 93, 0.5]] } } }
```
→ `"wall": { "gates_added": [{ "corner": 42, "id": "gt:5:42", "name": "Harys Gate", "towers": ["t:5:2000084",
"t:5:2000085"], "way_out_ft": 1221 }], "towers_added": [{ "id": "t:5:1000000", "x_ft": 2909301, "y_ft": 1052522 }] }`

| Error | Why |
|---|---|
| `a village has no wards` | Villages grow along their roads |
| `it is on the water or a river: it stays put` | A pinned corner |
| `that is N ft from where it was laid out: at most M` | Past one patch's spacing |
| `patch q is not next to patch p` / `a main street runs between them` / `only wards built on lots merge` | A merge refused |
| `no square opens there: …` | A square asked for at a corner that is not a junction (fewer than three streets, the wall, a gate, the water, or an unbuilt patch round it) |
| `only a temple's patch has a close` / `no temple stands on that patch…` / `a square opens at its corner` / `no room for a close there…` (the temple and churchyard don't fit, or houses would line under 65% of the block's streets) | A close refused |
| `a temple's close is at that corner` | A square refused at a close's corner |
| `only a park's patch is laid out as a garden…` | `park` asked of a patch whose ward isn't `park` |
| `too much of that patch is on the water for a park…` / `no street comes to that patch…` / `parks are laid out inside the town` / `that patch is too small for a park…` / `a cliff crosses that patch…` | The `park` ward refused on that patch (`park_refused`) |
| `the town was laid out anew: …` | The edit was made before a sketch change moved the town or its patches |
| `corner N is not on the town's wall …` / `corners N and M are not next to each other on the wall` | A gate or tower asked off the wall's line |
| `towers_remove: … is not one of this town's towers` | A tower id of another layout |

## Streets laid out by hand

`edit_streets` with `lay_out_by_hand` turns a town's or city's plan (as generated, or as the ward editor left it) into
records you change one by one: junctions, streets between them (straight or with bends), blocks the streets enclose
(each with a fill), rows of lots along a street, painted open areas and wall lines. Villages have no street plan (refused).
Positions are world feet. A town laid out by hand is refused by `edit_town` (the message points here); `back_to_generated`
drops the records and the town is the ward editor's again, as it was before.

### `get {view: "street_plan"}`

`{id}`: the town as its records stand. Returns `{layout, origin, tier, fill (the default for a face with no block), snap_ft (3),
junctions [{id, p, square, streets}], streets [{id, a, b, bends, width, set_l, set_r, class, bridge}], blocks [{id, at, fill,
district, seed, gen, kept, pins, streets (the ones round it)}], rows [{id, street, side, depth, ward, lots, gaps}], areas [{id,
poly, open}], walls [{id, pts, closed, spacing, gates, name}], problems [{key, why}]}`. A town not laid out by hand is refused.

### `edit_streets`

`{id, ops: [{op, …}, …]}` (or one op as `op`), `dry_run` (one op, nothing changed). The ops are applied in order, each on the
result of the last; one that is refused ends the list (the ones before it stay, the message says which). Inside `batch`, one op
per call. Ids are the numbers `get {view: "street_plan"}` shows; unknown fields are refused.

| op | fields |
|---|---|
| `lay_out_by_hand` | none |
| `back_to_generated` | none |
| `add_street` | `points` [[x, y], …], `width`, `set_l`, `set_r` (setbacks), `class` main, street, quay or joint |
| `delete_street` | `street` (its rows go, so do junctions left with no street) |
| `set_street` | `street`, `width`, `set_l`, `set_r`, `class`, `bridge` |
| `move_junctions` | `moves` [{`junction`, `to` or `by`}], all at once (if that breaks the streets, each that does not) |
| `set_square` | `junction`, `square` (a square kind, or none) |
| `set_block` | `block`, `fill` ({kind: generated, ward, lots, height, reroll, close} / {kind: hand} / {kind: open, open, style} / {kind: castle, gates, keep_shape, moat}: a castle built from the block's outline, its curtain, towers, keep and yard as a castle drawn by hand (`gates`: up to 8 points in world ft, each a gate on the side nearest it, the main one first, none: facing the town's middle; `keep_shape` `square` or `round`, none rolled; `moat` `none`, `dry` or `wet`, none rolled); a converted castle keeps its buildings and works until its fill is set again; `height`: `low` one storey each, `usual` (or none) as its ward has them, `tall` one storey more, up to 4), `district` (empty: none), `seed`, `reroll` |
| `add_row` / `remove_row` | `street`, `side` l or r, `depth`, `ward`, `lots`, `gaps`, `seed` / `row` |
| `add_area` / `remove_area` | `poly`, `open` (plaza, lawn, park, garden, market, cemetery, farmland) / `area` |
| `add_wall` / `remove_wall` | `pts`, `closed`, `spacing`, `towers`, `gates` [[x, y]], `name` / `wall` |
| `set_area` | `area`, `open` (as add_area) |
| `set_wall` | `wall`, `gates` [[x, y]] (replaces the hand gates: up to 16, each snapped to the line, refused if over 30 ft from it; streets crossing the wall still make gates), `name` (empty: none), `spacing` (towers apart, 40 to 1000 ft) |

A street's ends and bends within 3 ft of a junction meet it, else within 3 ft of a street split it; where a new street
crosses another both get a junction. Free ends make new junctions (a dead end is allowed and listed in `problems` until
joined). A change that would make streets cross or break the graph is refused ("that would break the streets"). Blocks follow
the streets: a block whose shape is unchanged is kept untouched, others are laid out again (`blocks.relaid`), buildings that
carried a name, note, business or NPC keep their ids where a building stands there again, else they are listed in `lost`.
The reply of each op: `{op, records {set, removed}, blocks {relaid, added, removed}, buildings {before, after}, rects, lost,
problems}` and per op `streets_added`, `row`, `area`, `wall`, `stayed`, `size`. `regenerate` keeps a hand town (its records
follow the world as other edits do).

## Shaping the ground

A battlemap's ground can be shaped by hand, as Edit › Land › Ground shapes it: brushed up or down, smoothed, put back,
levelled as plateaus and pits, or raised as hills. Two edits fields hold it:

- **`ground`**: brushed blocks, by `g:XXXXX:YYYYY` (block indices, zero-padded, x first; block `(bx, by)` holds
  squares `16·bx … 16·bx + 15` east and `16·by …` south). Each is one string: 256 height changes in tenths of a
  foot, row by row, written as zigzag-delta varints (a 0 delta is followed by a byte counting the further zeros)
  in base64url without padding. A block that comes out all zero is removed. Use `set_ground {kind: "brush"}` rather than
  writing them.
- **`plateaus`**: by `h:<id>`: `pts` (the outline, 3 to 48 points, rounded to 0.5 ft, not crossing itself,
  spanning at most 1,280 ft), `level` (ft on the world's datum: above the sea plus the sea's level), `edge` (0 to
  60 ft; absent: 0, a sheer cliff at the outline). A hill is an entry with `kind: "hill"` and a `hill` form
  `{profile, at: [x, y], rx, ry, angle?, crown?, seed}` (profile `round`, `ridge`, `knoll` or `mountain`; half-axes
  10 to 640 ft, `rx` along `angle` degrees clockwise from east; `crown` a flat top's radius); its `level` is its
  peak in ft over the ground (1 to 300) and `pts` its footprint's outline. Use `set_ground {kind: "hill"}` rather than writing them.

How it applies:

- Plateaus first, largest first (a terrace or pit inside a plateau goes on top of it): inside the outline the
  ground is at the level; outside it eases back to the ground over `edge` ft (a sheer one over a square). Then the
  brushed change and the hills together, taken as they are up to 220 ft and easing smoothly toward 300 ft either
  way beyond (plateaus and pits have no limit: they stand at their level). Ground that was dry never sinks below
  the water beside it; raising ground under water makes dry land.
- A hill rises from the ground at its footprint's edge to its peak, with a little shape noise of its own (keyed by
  its `seed`): `round` smooth all round, `ridge` a crest along `rx`, `knoll` steeper with rocky outcrops, `mountain`
  concave slopes and bands of crag. Its slopes steeper than about 50 degrees are cliff faces on the battlemap,
  hard going, with scree below.
- Everything on the ground follows it: buildings stand on it (on a plateau, at its level; a building over a sheer
  edge takes the mean, so draw round it), with their interiors and cellars; streets, roads, walls, towers and camp
  yards. Plateaus are flat: no generated knolls on them. Steep shaped ground draws as rock, with its cliffs.
- In cities and metropolises a street square that shaping makes climb at least 0.15 along the street, and at
  least 0.03 more than it did (but less than 1: steeper is a cliff), is drawn as steps, its treads across the
  street. A generated flight's square goes back to paving where shaping leaves it gentler than 0.12 and 0.03
  gentler than it was, or puts it on a plateau.
- Battlemaps and the three finest zooms draw it. The world's own heights (`inspect`, routes, where
  settlements stand) don't change, nor does `get`'s `stairs` (the flights as generated); `inspect {what: "battlemap"}`
  has the steps as drawn.
- Limits: a stroke reaches at most 48 blocks; a world holds 1,500 blocks and 256 plateaus.
- **Rock islets** are plateaus of kind `islet` (`set_ground {kind: "islet"}`): a rough ring of 12 to 20 corners, its top a few feet over the water of a river, lake or the sea, easing down over 6 ft, drawn as bare rock with a gravel rim. A building, tower or castle may stand on one (`set_site` and `set_building` accept it, with no `moved`), as on the world's own islets (cities and metropolises on rivers and coasts). `remove` takes one away, with its name, note and hidden mark.

### `set_ground {kind: "brush"}`
A round brush, or a stroke along `path` (not both: `at` and `path` together are refused). `raise` and `lower` move each square by up to `height_ft` (full out to `hardness` of the radius,
fading to the rim), `smooth` eases each square toward the mean of the stroke's own squares round it (within half the radius, at least 10 ft; the ground beyond the rim doesn't pull) by `strength`, so a repeated smooth settles instead of piling up,
`restore` takes brushed change back toward the ground as generated by `strength` (plateaus stay). Each square
takes the stroke's strongest touch, so going over a place twice in one stroke doesn't pile up (a second call
does). The reply: `blocks` changed, `squares` under the brush, the lowest and highest ground there `before` and
`after` (ft above the sea).

| Parameter | Type | Default | |
|---|---|---|---|
| `mode` | string | required | `raise`, `lower`, `smooth`, `restore` |
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | | A dab (or `x_ft` + `y_ft`, or `id`, beside the other arguments) |
| `radius_ft` | number | 15 | A dab's radius, 2.5–200 |
| `path` | `[[x_ft, y_ft], …]` | | Or a stroke along a line (each point may be an id or `{x_ft, y_ft}` too) |
| `width_ft` | number | 30 | A stroke's width, 5–400 |
| `height_ft` | number | 5 | Raise, lower: 0–50 (one tier is 5) |
| `hardness` | number | 0.5 | 0 soft to 1 hard |
| `strength` | number | 0.5 | Smooth, restore: 0–1 |

```json
{ "name": "set_ground", "arguments": { "mode": "raise", "at": "b:0:2", "radius_ft": 25, "height_ft": 8 } }
```

### `set_ground {kind: "islet"}`
A rock islet standing in water. `at` (an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`: in a river's channel, a lake or the sea), `radius_ft` (10–200, default 30), `rise_ft` (1–30, default 5: its top over the water there) and `id` (`h:…`, to change one: what is not given stays, the ring following the id). The outline is made from the id, so the same radius and place give the same ring. Without `id`, an islet whose ring for this place and radius lies within 5 ft at every corner of an existing one's is that one: `existing: true`; another rise is refused, naming it.

Returns `{tool, id, changed, level_ft, water: {kind, id, name, depth_ft}, poly, bbox}` (`level_ft` above the sea; `water.kind` `river`, `lake` or `sea`). Refused: `at: (x, y) is on dry land: an islet stands in a river, lake or the sea`; `at: the water there is 172 ft deep: at most 120`; the plateau limit (256 in a world). `list {what: "ground"}` lists islets (`what: "islet"`).

```json
{ "name": "set_ground", "arguments": { "at": [3124704, 3868520], "radius_ft": 130, "rise_ft": 6 } }
```

### `set_ground {kind: "plateau"}`
A plateau or pit, or a change to one. The outline is `poly`, or `around` a building (`b:`), a keep, a wall
tower (`t:`) or a site (its buildings, towers and walls), its footprint's convex outline grown by `margin_ft`;
outlines are simplified to at most 48 points. The level is `level_ft` above the sea, or `rise_ft` above the
ground (as shaped, this plateau left out) at the outline's middle. The reply: `id`, `changed`, `level_ft`,
`ground_ft` (the ground at the middle), `edge_ft`, `points`, `area_sq_ft`, `bbox`, `floor`. Without `id`, an outline that matches an existing plateau's (the same number of corners, each within 1 ft) is that one: `existing: true`, nothing added; a different `level_ft`, `rise_ft`, `edge_ft` or `floor` is refused (give its id to change it).

Plateaus stack, whatever order they were made in: where they overlap the highest holds (a lower one inside a
higher one changes nothing), and a pit (below the ground there) digs through the plateaus round it, the deepest pit
holding; a plateau inside a pit stands up out of it. Pieces of one level join as one ground (no seam where they
meet), and each edge eases toward the ground round the whole stack. Walls, castles and buildings may stand where a
plateau holds the ground above the sea or a lake. The floor: `rock` draws the top as bare rock (scree at its foot) at
every zoom, `ground` as the land round it; `auto` (the default, not written) is rock on a pit's floor 15 ft or more
below the ground, the land round it otherwise.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | | A plateau to change (`h:…`): only the fields given change |
| `poly` | `[[x_ft, y_ft], …]` | | The outline (world ft, 3 or more corners) |
| `around` | string | | Or a building, tower or site to raise it round |
| `margin_ft` | number | 10 | Round the footprint, 0–200 |
| `level_ft` | number | | Its level above the sea, within 20,000 ft |
| `rise_ft` | number | 10 | Or its level above the ground at the middle (below zero: a pit); for a brush stroke, within 300 ft; with `id`, only when given |
| `edge_ft` | number | 0 | The slope round it, 0 (sheer) to 60 |
| `floor` | string | `auto` | `auto`, `rock` or `ground`: what its top (a pit's floor) draws as; with `id`, kept unless given |

```json
{ "name": "set_ground", "arguments": { "around": "c:3", "margin_ft": 15, "rise_ft": 25, "edge_ft": 0 } }
```

A hill (`set_ground {kind: "hill"}` changes it; `set_ground {kind: "plateau"}` refuses one).

### `set_ground {kind: "hill"}`
Raise a hill, or change one by `id` (fields left out stay; its noise stays its own). Returns `{tool, id, changed,
profile, peak_ft, x_ft, y_ft, rx_ft, ry_ft, angle_deg, crown_ft, bbox}`. `remove` takes it away; `list {what: "ground"}`
lists it (`what: "hill"`, with its form).

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | | A hill to change (`h:…`) |
| `profile` | string | `round` | `round`, `ridge`, `knoll` or `mountain` |
| `at` | place | | Its middle, with `radius_ft` (or `rx_ft` and `ry_ft`) |
| `radius_ft` | number | | The footprint's radius, 10–640 |
| `rx_ft`, `ry_ft` | number | | Or half-axes along and across `angle_deg` (`ry_ft` defaults to `rx_ft`) |
| `angle_deg` | number | 0 | `rx_ft`'s direction, clockwise from east |
| `from`, `to` | place | | Or a ridge's crest, end to end, with `width_ft` its half-width (default 80) |
| `around` | string | | Or a building, tower or site: a round hill centred on it, its crown wider than it |
| `peak_ft` | number | 60 | Its peak over the ground, 1–300 |
| `crown_ft` | number | 0 | A flat top's radius, at most 0.8 of the narrower half-axis |

```json
{ "name": "set_ground", "arguments": { "profile": "ridge", "from": [412000, 380000], "to": [412600, 380200], "width_ft": 90, "peak_ft": 120 } }
```

### `list {what: "ground"}`
The ground shaped by hand, world-wide, in one `{items, found}` list (`limit` 1 to 1000, default 100): plateaus and pits
`{id: "h:...", what: "plateau" (or "islet"), name? (if renamed), level_ft, edge_ft, floor? (rock or ground, if set), x_ft, y_ft, bbox, poly}` (`level_ft` above the sea, `poly` the outline),
hills `{id: "h:...", what: "hill", profile, peak_ft, x_ft, y_ft, rx_ft, ry_ft, angle_deg, crown_ft, bbox, poly}` (`peak_ft` over the ground), then
brushed blocks `{id: "g:...", what: "ground", x_ft, y_ft, size_ft: 80, squares_shaped, raised_ft, lowered_ft}` (`x_ft`/`y_ft` the
block's north-west corner), and (unless `generated: false`) the world's own islets in rivers and off coasts
`{id: "i:<8 hex>", what: "islet", generated: true, level_ft, water_ft, x_ft, y_ft, radius_ft, in: "river" or "sea or lake", tower? (a watchtower stands on it), removed? (taken away), bbox, poly}`.
`within` (a box, a polygon or a circle `{at, radius_ft}`) keeps those with their middle or any outline point (a ground block: its middle or a corner) inside it, or whose outline holds the area's middle (a small area inside a big plateau). `remove` takes plateaus away; `inspect {what: "heights"}` reads the heights.

### `remove` (ground)
Take plateaus or pits away by `id` or `ids` (all or none); the ground goes back as it was (brushed changes stay).
Its name (`set_feature`), note and hidden mark go with it; there is no restore (the user's Undo in the app
brings all of them back).

Also takes the world's own islets (`i:...`, from `list {what: "ground"}`): a hidden mark takes one off the ground, the shore and the
map (a bridge deck or pier no longer keeps clear of it only where the town's plan already did), and a watchtower on it is
removed as a building (`removed_towers`, `b:...`). `restore` undoes it.

### `restore` (islets)
Bring the world's own islets (`i:...`) back by `id` or `ids` (all or none) after `remove` took them away; one not
taken away is refused. A watchtower taken away with one stays removed (`restore`). Returns `{tool, ids}`.

### `inspect {what: "heights"}`
Every square of the box reaching `radius_ft` (default 25, at most 200) east, west, north and south of a place:
`origin` (the first square's middle),
`columns`, `rows`, `heights_generated` and `heights_shaped` (rows north to south, ft above the sea), their
`generated` and `shaped` ranges, `squares_shaped`, `wet` (`[column, row]` of squares under water), and the
`plateaus` (id, level, edge, middle, bbox) and `brushed_blocks` there. The battlemap draws a little relief on
top (knolls, hollows) and levels road beds under roads.

```json
{ "name": "inspect", "arguments": { "what": "heights", "at": [1288677, 2229693], "radius_ft": 60 } }
```
> *"Put the Sunken Bastion on a cliff 30 ft above the plain, sheer on every side."* · *"Dig a 10-ft pit with
> sloping sides in the middle of the market."* · *"Raise a low ridge along the road north of Gatewatch for an
> ambush."*

| Error | Why |
|---|---|
| `a stroke reaches at most 48 blocks of 80 ft` | Shorten the path or narrow it; or several calls |
| `a world holds at most 1500 blocks of brushed ground` / `at most 256 plateaus` | The world's limits |
| `radius_ft: 2.5 to 200, got 500` (and the like for `width_ft`, `height_ft`, `hardness`, `edge_ft`, `margin_ft`) | The numbers' ranges are in the schema and refused, never clamped |
| `an outline spans at most 1280 ft` / `the outline crosses itself` / `the outline holds no ground` | A plateau's outline |
| `… is a settlement: a plateau spans at most 1280 ft` | `around` a whole town: give a building or an outline |

---

## Water drawn by hand

Water drawn by hand, as Edit › Roads & water › Water draws it: a polygon, or a band brushed along a line, with a depth and a
shore that is natural or dug. Its look follows the water (there is no kind to pick). The edits field **`waters`**
holds it, by `e:<id>`: `pts` (a polygon's points, 3 to 128, rounded to 0.5 ft, not crossing itself, spanning at
most 5 miles; or a line's, 2 or more, with `line: true`, `width` and `closed`), `depth` (ft), `dug` and `level`
(its surface, ft on the world's datum). Water made by older callers may carry a `kind` instead (see below).

How it applies:

- Inside it the water stands at its level and the ground falls from the shore to its depth below it (1 to 20 ft,
  default 4) over a bank that widens gently with depth (3 ft at 2 ft deep or less, up to 15 ft at 20 ft; dug water
  and rings about 4 ft, steep), to any depth under the ground as generated. Polygons that overlap are one water
  with one shore (the lower level, the deeper bed); one that reaches the sea or a lake at its level joins it.
  Plateaus and islets go on top: one drawn over the water stands dry.
- Its look, the same on the map and the battlemap: at the sea's level (within a quarter foot) it is the sea's,
  its colour and a sand beach, or, dug, a paved quay about 6 ft wide with bollards; an open brushed line is a
  stream, shallow-looking, its level falling along it, gravel banks; a ring (a brushed line closed) is a moat,
  still and dark, earth banks (stone within 60 ft of a wall or castle); anything else is still water: a pond
  (greener, reeds, mud) shallower than 4 ft, a lake (blue, darker toward its middle where it is wide enough,
  pebbles) 4 ft or deeper. Dug still water has straight sides and a stone edge.
- Battlemaps draw it as water (and its shore); the map draws it at every zoom; a name given to it
  (`set_feature`, or `name`) shows on the map. Buildings, walls and castles keep out of it, and a crossing's end
  can't be in it. The world's own towns as generated aren't laid out again round it.
- Limits: a world holds 256 waters.

### `set_water`

`poly` and optionally `shape` (`polygon`, the default, or `brush`), `width_ft` (a brushed line's,
3 to 80, default 10), `closed` (a brushed line brought round to its first point: a ring), `depth_ft` (1 to 20,
default 4), `dug` (default false), `level_ft` (above the sea) and `name`. Without a level it takes the sea's or a
lake's where it reaches one within 4 ft of its shore (the highest; the reply's `joins` says which), else 1 ft
under the lowest ground on its shore (half a foot over the sea's at least: only water that reaches the sea looks
the sea's); an open brushed line's
level follows the ground down the line from its higher end, 1 ft under it and never rising, meeting water it runs
into at that water's level (with `level_ft` it is flat). The same points again return the water there
(`existing: true`). Returns the water's entry: `id`, `kind` (its look: `inlet`, `basin`, `lake`, `pond`, `moat`
or `stream`), `shape`, `depth_ft`, `dug`, `level_ft`, `x_ft`, `y_ft`, `bbox`, `area_sq_ft`, `points`, `poly`,
`name`.

`poly` is kept as given; the shore is shaped from it. A natural polygon gets a smooth curve round the points
(passing a little inside sharp corners, a little outside straight sides) with a gentle irregular wobble along it
(a few feet, more on bigger water); a dug one keeps straight sides with its corners slightly rounded. `x_ft`,
`y_ft`, `bbox` and `area_sq_ft` are the shaped shore's; `points` and `poly` the points as given. Four points make
a natural pond; more points steer the shape.

A brushed line's water is the band `width_ft` wide along it (an open line's smoothed), round at the bends'
outsides and at an open line's ends. A band that would fold over itself (a bend too sharp for its width) is
refused. Its entry adds `line`, `width_ft`, `closed`, and an open line's `level_ft` is its top and `falls_to_ft`
its lowest.

Legacy: `kind` (`inlet`, `basin`, `lake`, `pond`, `moat`, `stream`) and `line` are still taken from older callers.
Given a kind, the water keeps it and looks, digs and finds its level as that kind did: inlet 12 ft deep and basin
10 (the sea's level, a basin dug), lake 8, moat 6 (dug; may be a line 8 to 60 ft wide, default 20), pond 3, stream
1.5 (always an open line, 3 to 12 ft wide, default 5); `line: true` is `shape: brush`. Its entry adds
`legacy_kind`. Water with neither a kind nor a depth is a lake.

### `set_water {id}`

`id` (`e:…`) and any of `poly`, `shape`, `width_ft`, `closed`, `depth_ft`, `dug`, `level_ft`, `name` (empty:
none), or a legacy `kind`. What is not given stays; water with a legacy kind keeps it unless `shape`, `depth_ft` or
`dug` is given (then it has none, keeping its depth and shore). New points, a new shape or width, or a change
between an open line and anything else finds its level again unless `level_ft` is given.

### `remove` (water)

`id` or `ids` (all or none): the water goes with its name, note and hidden mark (no restore; the user's Undo in
the app brings it back).

### `list {what: "waters"}`

Every water drawn by hand, as `set_water` returns it (`{items, found}`, `limit` 100 by default).

## Roads drawn by hand

Roads drawn on the ground, as Edit › Roads & water › Road draws them: a footpath, track, gravel road, paved road or cobbled street
along a line of points. The edits field **`paths`** holds them, by `rw:<id>`: `pts` (2 to 256 points, rounded to
0.5 ft, no two in a row the same, spanning at most 5 miles), `type` (`footpath`, `track`, `gravel`, `paved`,
`cobbled`), `width` (ft, 3 to 40; absent: the type's own, footpath 4, track 10, gravel 14, paved 18, cobbled 20) and
`name`.

How it applies:

- The road runs on a smooth curve through its points (a centripetal Catmull-Rom spline), a band of its width along
  it. The battlemap draws the band as road squares: a footpath or track packed earth (a footpath at least a square
  wide), gravel as gravel, a paved road or cobbled street as cobbles; it is walked like a road.
- Where its line runs over water (a river, a lake, the sea, water drawn by hand), it crosses on a
  bridge (timber; paved and cobbled on stone), from a few feet before the water to as far past it; a crossing longer than 300 ft is built in
  pieces end to end.
- Trees, rocks and bushes keep off the band; the world's own buildings in villages, towns and cities that the band
  meets are taken away (not those of a town laid out by hand, nor buildings drawn by hand). The world's road
  network, its towns' plans and streets and the ground's shape don't change.
- The map draws it at every zoom as a road of its look (a line, then a bed of its width close in).
- Limits: a world holds 256 roads drawn by hand.

### `set_road`

`path` (the line) and optionally `type` (default `track`), `width_ft` and `name`; with `id` (`rw:…`) it changes that road, and
what is not given stays (a new type without `width_ft` takes its own width unless the road had a width of its own;
`name` empty: none). The same line again returns the road there (`existing: true`). Returns its entry: `id`,
`type`, `what`, `width_ft`, `name`, `length_ft`, `middle`, `bbox`, `pts`, and `crossings` (`{kind: bridge | ford,
material, a, b, length_ft}` each).

### `remove` and `restore` (roads)

`ids`: roads, streets and paths taken off the map, one by one (each id done or refused alone); `restore: true`
brings them back. What's left is ground: the ground beneath shows, a park's lawn closes over its path, a bridge
deck on a removed town street goes with it; buildings, lots, a town's gates, traffic and routes stay as they were.
- `rd:` a world road's stretch (`list {what: "roads"}`, `inspect`) and `mt:` the track to a settlement added by
  hand. A stretch takes the approach street of each generated town it ends at with it, and brings it back with it:
  those are listed in `also`.
- `tr:<layout>:<8 hex>` a generated town's or village's own street or lane, `pp:<layout>:<spot>:<j>` a path in one
  of its parks, gardens or cemeteries (both from `inspect`'s `things`). Kept while the town is laid out
  where it was: a ward edit (`edit_town`) that moves a street's ends gives it a new id, and the removal lapses.
  Laying the town out by hand (`edit_streets` `lay_out_by_hand`) brings them back (its report's `removed_back`
  says how many), `back_to_generated` takes them away again. A hand town's streets are refused (`edit_streets`
  changes them).
- `rw:` a road drawn by hand and `ts:` a street added to a generated town: deleted (with a drawn road's notes);
  not brought back by `restore` (the user's Undo in the app does).

Returns `{removed | restored, also, refused: [{id, why}]}`; an error when nothing was done.

### `list {what: "drawn_roads"}`

Every road drawn by hand, as `set_road` returns it (`{items, found}`, `limit` 100 by default).

### Fences and hedges

Every fence, hedge and garden wall a settlement generates (round fields, crofts, yards, the churchyard and
cemetery, corrals, pens, manor yards) is a run with an id, `fr:<layout>:<8 hex>`, from its source and its two ends;
a park's clipped hedge or row of topiary is `fh:<layout>:<spot>:<j>`. A ward edit that moves a run's ends gives it
a new id (its change lapses). Fences drawn by hand are `f:<n>`, the eraser's gaps `fg:<n>`. Only the battlemap
draws them (and the plan's corral, pen and churchyard marks): fields, plots, gates, haystacks and animals stay as
generated. A palace forecourt's walls, a castle kitchen's wattle, lone topiary and a temple close's bushes have no
id: cut them with a gap (`remove` takes any of them).

Kinds and their rules in play: `hedgerow`, `dry_stone_wall`, `wattle`, `low_wall`, `picket`, `iron_railing`: hard
going, half cover, seen over; `hedge` (clipped): blocks the way, three-quarters cover, seen over; `post_and_rail`:
hard going, no cover; `stockade`, `high_wall`: block the way and sight. `garden_wall` (a manor's yard, a
forecourt) is generated only: it can be removed or gapped, not changed in kind, and no tool accepts it as a `kind`. A
gate is an opening of one battlemap square or a little more (5 to 7 ft, its middle moved to the square's) with an open
gate leaf: open passage for every kind, never shut in play.

### `set_fence`

Draws a fence or hedge along `path` (2 to 400, world ft; `closed: true` back round to the first), of `kind`
(required for a new one), with `gates` (points: each a gate where the line passes nearest; a point over 10 ft from the line is refused). It crosses roads
and roofs and stops only at water. With `id`:
- `f:` a drawn fence changed: what is given replaces it (new `path` or `closed` drop its gates unless given
  again; `kind` or `gates` alone keep the line and add the gates);
- `fr:` / `fh:` a generated run: `kind` (back to its own kind: as generated) and/or `gates` (added; `path` is
  refused). A garden wall keeps its kind; a hand town's runs are refused (`edit_streets` changes the town).

Returns a drawn fence's `{id, kind, length_ft, pts, closed?, gates? (ft along the line)}`, or a run's `{id, kind,
gates}`. A gate within 5 ft of another is refused.

### `remove` and `restore` (fences)

`ids` (at most 500 a call; more is refused whole) taken away one by one, each done or refused alone: `fr:` /
`fh:` a generated run gone whole (`restore: true` brings it back as generated, dropping its kind and gates too),
`f:` a drawn fence deleted, `fg:` a gap dropped (neither brought back by `restore`: the user's Undo does). `gaps`:
`[{points, width_ft? (1 to 20, default 5)}]` cut every fence and hedge under the stroke, generated or drawn, and
stay through remakes. Returns `{removed | restored, gaps (the new fg: ids), refused: [{id, why}]}`; an error when
nothing was done.

### `list {what: "fences"}`

Without `at`: every drawn fence, gap and changed run (`{id, town, found, gone?, kind?, gates?}`; `found` false when
its town's edits are set aside). With `at` (and `radius_ft`, default 200, at most 2,000): every fence and hedge
near, nearest first: runs `{id, kind, length_ft, ends, town, gone?, gates?, changed_from?}` (parks' hedges within
400 ft), drawn fences and gaps. `{items, found}`, `limit` 100 by default.

## What agents can't do

Asked for any of these, say so rather than looking for a tool:
- **Reshape the world.** The sketch (coastlines, mountains, rivers and roads drawn in the app's Sketch mode) is the
  user's to draw; no tool draws, changes or erases a stroke, or generates the world again. Sea and land stay as they
  are: no sea, lake or river can be added, moved or removed, and `set_site` refuses water.
- **Draw political borders.** There are no realms, provinces or border lines to read or make.
- **Make or remove settlements.** Cities, towns and villages are generated; an agent can rename, annotate and hide
  them, lay a town out anew (`edit_town`), add and take away buildings, but not found a new city or delete one.
- **Move a feature.** Names, notes, tags and hiding change; a site's, a river's or a settlement's place does not
  (delete and create a site again to put it elsewhere; a building: `set_building`). The one move is an entrance
  into a building's cellar and out again (`set_feature` `in_building`).
- **See what the user sees without the app.** `render_view` and `focus_view` need the map app open and connected.

---

## Recipes (prompts for an agent)

**Session prep**
> *"The party is heading from Agentholm to Gatewatch. Plan the route, tell me how many days it takes at a normal
> pace, and put a waystation and a bandit camp along the way, each with a hook in its notes. Show me the camp's
> battlemap."*

**A dungeon from scratch**
> *"Create a ruin over a dungeon in the hills east of Bulol, name it something ominous, then list its levels
> and rooms and write a one-line description for the boss chamber in its notes."*

**Worldbuilding pass**
> *"For every city, write two sentences of lore and a rumour, and tag each 'lore'."*

**A cast for a town**
> *"Write five NPCs for Bulol (an innkeeper, a priest, a smuggler, a guard captain, a noble), each with
> appearance, mannerisms, attitude toward the party and goals, place each in a fitting building, and add a plot
> point that ties three of them together."*

**Fog of knowledge**
> *"Hide every settlement more than 100 miles from Gatewatch; we'll reveal them as the players explore."*

**Encounter design**
> *"Get the battlemap at the bridge south of Gatewatch, describe the cover and elevation for an ambush by six
> bandits, and show it to me."*

**Wyrm's Rock: a castle on an islet**
> *"In the Red River, raise an islet 130 ft across and 6 ft over the water, put a castle on it and name it Wyrm's Rock. Put a pier out from the south bank and a quay in front of the castle's gate."*
> Steps: `set_ground {kind: "islet"}` (`at` a point in the channel, `radius_ft` 130), `set_site` (`kind: "castle"`, a `rect` of at least 80 ft across on the islet, `name`), `set_wharf` (`kind: "pier"`, `path` from the bank), `set_wharf` (`kind: "quay"`).

**Shops on Wyrm's Crossing**
> *"Build a stone bridge over the Red River, bank to bank, with a row of shops down each side, then widen it to 60 ft."*
> Steps: `set_crossing` (`kind: "bridge"`, `material: "stone"`, `from` and `to` 20 ft beyond the banks, `shops: true`: the reply lists the shops); then `set_crossing` with that `id` and `width_ft: 60` (`shops_relaid`). The shops are drawn buildings (`c:<n>`): `set_building` and `set_feature` change them like any other.

**Renaming a region**
> *"Rename the Katsei Mountains to the Ashen Teeth and rename every village inside them to fit."*

## Calling it without an MCP client

mapd speaks MCP over streamable HTTP with JSON responses (JSON-RPC 2.0, protocol versions 2025-06-18,
2025-03-26 and 2024-11-05). With curl:

```sh
curl -s -X POST http://127.0.0.1:7777/mcp \
  -H 'content-type: application/json' -H 'accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"route","arguments":{"from":"city:bab44567","to":"metropolis:6eee9055"}}}'
```

It also answers `initialize`, `ping` and `tools/list`.

## When things go wrong

| Message | Meaning |
|---|---|
| `no world is open: open the map app first` | mapd hasn't seen a world yet. Open the app once; after that mapd remembers it. |
| `the map app is not open` / `did not answer in time` | `render_view` and `focus_view` need the app open and connected. |
| `no open map app shows mapd's world` | The open tabs show other worlds: open mapd's world in a tab, or choose **Follow this tab** in one. |
| `mapd has another world open (…), not …: nothing was done` | The `world` given isn't the one open: the user switched worlds. |
| `saved: false`, `warning: made, but not yet saved to disk (…)` | Not an error: the change stands in memory and is saved again every few seconds. Don't repeat the call. |
| `step N (tool) failed: … Nothing was changed.` | A `batch` step failed; the batch was not applied. |
| `no such feature: …` | A bad id: check it with `search`. |
| `that place is under water: pick dry land` / `off the map` | `set_site` refused the position. |
| `… is not a created site` | `remove` only removes `c:` sites. Use `set_feature` for generated ones. |
| `… was deleted with c:<n> (restore brings it back)` / `… is already deleted` | A deleted site's ids are retired; `restore` undoes the delete. |
| `under a ruin: …` / `under an entrance: …` / `'under' is for ruins and entrances` | Only ruins and entrances choose what lies beneath them. |
| `themes for a …: …` / `size must be …` / `levels: 1 to 6` / `… has nothing underground` | `set_site` options that don't fit the site. |
| `not saved: …` | `set_design`: the plan breaks a rule play mode needs (the reasons follow). |
| `only underground sites (u:<layout>:<k>) can be designed` / `a city's sewers can't be designed` | `get {view: "design"}` / `set_design` take `u:` sites and `b:` buildings, not sewers or keeps' deep dungeons. |
| `a ruin or a yard has no inside to design` | Only roofed buildings have interiors. |
| `a building's levels are its cellars and storeys` | A building's plan can't add or take away floors: change its storeys with `set_building`; cellars with `cellars <n>`. |
| `a building has 0 to 3 levels below ground` | `cellars <n>` (or a `level -4` block) asks for too many. |
| `it needs a cellar, for the trapdoor to the sewers` (or `the stairs down to the deep dungeons`) | A building with a way to another site keeps at least one cellar. |
| `'x' at 4,7 is not in its rooms list` / `level 3 is new: give its grid` | A plan that can't be read. |

## Changes in v66

The tools were merged into one vocabulary (above). There are no aliases: an old name is refused and the error says
what does its work now, so a saved prompt or script can be fixed from it. The change labels the app keeps in its
history are unchanged. Argument renames that go with the merge: lines are `path` (was `pts`, `points`, `along`);
places are `at` (was `location`, `near`, top-level `x_ft`/`y_ft`); dry runs are `dry_run` (was `preview`);
`nearest` takes `radius_ft` (was `within_mi`); list filters round a place take `within` (a circle).

Kept as they were: `world_overview`, `route`, `render_view`, `focus_view`, `set_hex_grid`, `nearest`, `road_point`, `edit_town`, `edit_streets`, `regenerate`, `batch`.

| Old tool | Now |
|---|---|
| `search_features` | `search` |
| `get_feature` | `get {view: "details"}` |
| `list_children` | `get {view: "children"}` |
| `list_names` | `list {what: "names"}` |
| `describe_location` | `inspect {what: "location"}` |
| `features_near` | `search {at, radius_ft}` |
| `list_roads` | `list {what: "roads"}` |
| `get_battlemap` | `inspect {what: "battlemap"}` |
| `rename_feature` | `set_feature {id, name}` |
| `annotate_feature` | `set_feature {id, notes, tags}` |
| `create_feature` | `set_site {kind, at}` |
| `update_feature` | `set_feature {id, name, notes, tags} (an entrance's in_building: set_site; an m: note's at: set_note)` |
| `hide_feature` | `set_feature {id, hidden}` |
| `delete_feature` | `remove {ids}` |
| `list_npcs` | `list {what: "npcs"}` |
| `get_npc` | `get {id}` |
| `create_npc` | `set_npc {name}` |
| `update_npc` | `set_npc {id}` |
| `delete_npc` | `remove {ids}` |
| `place_npc` | `set_npc {id, at}` |
| `list_plots` | `list {what: "plots"}` |
| `get_plot` | `get {id}` |
| `create_plot` | `set_plot {title}` |
| `update_plot` | `set_plot {id}` |
| `add_note_at` | `set_note {site, level, text}` |
| `move_note` | `set_note {id, at}` |
| `delete_note` | `remove {ids} (a place's note: remove {what: "notes", id})` |
| `list_notes` | `list {what: "notes"}` |
| `delete_plot` | `remove {ids}` |
| `restore_feature` | `restore {ids}` |
| `list_sprites` | `list {what: "sprites"}` |
| `upload_sprite` | `set_sprite {name, data_base64}` |
| `place_objects` | `set_objects {objects}` |
| `remove_objects` | `remove {ids} (or {what: "objects", within, points, kinds})` |
| `restore_objects` | `restore {ids} (or {what: "objects", within})` |
| `create_building` | `set_building` |
| `update_building` | `set_building {id}` |
| `check_building_spot` | `set_building {dry_run: true}` |
| `remove_buildings` | `remove {ids} (or {what: "buildings", within})` |
| `restore_building` | `restore {ids}` |
| `place_stamp` | `set_building {at, stamp}` |
| `create_building_row` | `set_building_row {path}` |
| `create_parcel` | `set_parcel {poly}` |
| `update_parcel` | `set_parcel {id}` |
| `update_fortification` | `set_site {id}` |
| `list_crossings` | `list {what: "crossings"}` |
| `place_crossing` | `set_crossing {from, to}` |
| `remove_crossings` | `remove {ids}` |
| `get_hex_grid` | `list {what: "hex_grid"}` |
| `hex_at` | `inspect {what: "hex"}` |
| `create_link` | `set_link {a, b}` |
| `list_links` | `list {what: "links"}` |
| `delete_link` | `remove {ids}` |
| `shape_ground` | `set_ground {kind: "brush", mode}` |
| `set_plateau` | `set_ground {kind: "plateau"} (with id: set_ground {id})` |
| `set_hill` | `set_ground {kind: "hill"} (with id: set_ground {id})` |
| `delete_plateau` | `remove {ids}` |
| `restore_islets` | `restore {ids}` |
| `ground_heights` | `inspect {what: "heights"}` |
| `create_water` | `set_water {poly}` |
| `update_water` | `set_water {id}` |
| `delete_water` | `remove {ids}` |
| `list_waters` | `list {what: "waters"}` |
| `place_path` | `set_road {path}` |
| `remove_roads` | `remove {ids} (restore {ids} brings them back)` |
| `list_paths` | `list {what: "drawn_roads"}` |
| `place_fence` | `set_fence {path}` |
| `remove_fences` | `remove {ids} (restore {ids} brings them back; gaps: set_fence {gaps})` |
| `list_fences` | `list {what: "fences"}` |
| `raise_islet` | `set_ground {kind: "islet", at}` |
| `place_wharf` | `set_wharf {kind, path}` |
| `list_wharves` | `list {what: "wharves"}` |
| `remove_wharves` | `remove {ids}` |
| `get_site_design` | `get {view: "design"}` |
| `set_site_design` | `set_design {id, text}` |
| `reset_site_design` | `set_design {id, reset: true}` |
| `list_objects` | `list {what: "objects"}` |
| `list_plateaus` | `list {what: "ground"}` |
| `list_designs` | `list {what: "designs"}` |
| `list_streets` | `list {what: "streets"}` |
| `get_town_plan` | `get {view: "town_plan"}` |
| `get_street_plan` | `get {view: "street_plan"}` |
| `create_settlement` | `set_settlement {at, tier}` |
| `update_settlement` | `set_settlement {id}` |
| `get_sketch` | `list {what: "sketch"}` |
| `create_sketch_stroke` | `set_sketch_stroke {tool}` |
| `update_sketch_stroke` | `set_sketch_stroke {id}` |
| `delete_sketch_stroke` | `remove {ids}` |
| `remove_paths` | `remove {ids}` |
