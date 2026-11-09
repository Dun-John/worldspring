# Worldspring MCP: full reference

`mapd` exposes the open world to agents (any MCP client) through 57 tools. With them an agent can:
- read and search the world: features, buildings, districts, underground sites, routes, battlemaps;
- look at the map as you see it, or move your view;
- edit the world: rename anything (down to a dungeon's levels and rooms), write notes, hide features, and create
  sites that are as playable as generated ones;
- keep the DM's notebook: NPCs and plot points;
- put objects on battlemaps (and take generated ones away), with uploaded sprites;
- draw buildings and towers, with what they are, their storeys and roofs;
- shape a battlemap's ground: mounds, hollows, plateaus, cliffs and pits;
- put bridges, fords and ferries across rivers (crossings), and lay a town out anew (town plans: its patches, wards
  and lots) or design a dungeon's rooms and a building's interior by hand (designs);
- do many of these at once, as one change, with `batch`.

Every edit is saved and appears live in the open map app.

For setup details, storage and undo, see [AGENT.md](AGENT.md).

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

- **Coordinates** are in feet from the map's top-left corner: `x_ft` runs east and `y_ft` runs south. One mile is
  5,280 ft; one battlemap square is 5 ft.
- **One vocabulary:** the same thing has the same name in every tool (table below). Old names still work (they are
  accepted, never returned, and not in the tool schemas). Give a new name and its old one with different values and the
  call is refused (`blocks_move and blocks_movement disagree: give one`); the same value twice is fine.
- **Arguments are checked before anything runs**, for every tool and every `batch` step, against the tool's own
  schema: unknown arguments, types (an integer is a whole number; `"10"` is not a number), `enum` values,
  `minimum`/`maximum`, list sizes and required fields. `null` for an optional argument counts as not given. A
  refusal says which field and what was wrong, up to five at once, with the item's index in lists, and suggests a
  near name when there is one:
  `radius_squares: 1 to 64, got 200` · `hidden: true or false, got "yes"` ·
  `radius: unknown argument (did you mean radius_mi?)` · `kind: one of city, town, ..., got "citty" (did you mean city?)` ·
  `unknown tool 'serch_features' (did you mean search_features?)`.
  A number out of range is refused, never clamped; the ranges and defaults are in the schemas (`minimum`, `maximum`,
  `default`): every range and default below is the schema's. A tool's rules that a schema can't say (one of several fields needed, a polygon that crosses itself)
  are worded the same way: `field: rule, got X`.
- **Places:** `at` is a place: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`. Other points (`near`, `from`, `to`) take the
  same three forms. Refused: a point off the map (0 ≤ x < width and 0 ≤ y < height, the same for every tool; the message gives the map's size), `at` together with `id`,
  `x_ft` or `y_ft` (the same place twice is fine), only one of `x_ft`/`y_ft`, an id that names nothing. A tool whose
  `id` is not its subject also takes the old `id`, or `x_ft` and `y_ft`, beside its other arguments.
- **Lists** come back as `{items, found}`: `found` is how many there were before any `limit`, and `truncated: true`
  says a limit cut the list; a tool's other fields stay beside them (`named` in `list_roads`). Every list tool has a
  `limit` (defaults: `list_names`, `list_children`, `list_sprites`, `list_crossings` and `list_objects` 200, `list_npcs`,
  `list_plots`, `list_plateaus` and `list_designs` 100, `search_features` 25, `features_near` and `list_roads` 50; `list_children` cuts each of its lists
  and gives the full counts in `found`). A `limit` out of range is refused. Results are compact JSON (no
  indentation), and `render_view` sends a JPEG of 1,024 px unless asked for more.
- **Safe to repeat:** a create that is asked for again with the same arguments adds nothing and returns what is already
  there, with `"existing": true` and a `note` that says so (never "created"). Which tools, and what counts as the same:
  `create_feature` (the same kind of site within 300 ft), `create_building` (the same footprint; with `near`, an
  identical call: mapd remembers this session's `near` calls, last 500, until it restarts, and returns the building of
  an identical earlier call while it stands. Change anything, e.g. the name, to make a second), `create_npc` (the same
  name and place), `create_plot` (the same title), `place_objects` (new objects: the same kind within 0.5 ft with the
  same turn, scale and any variant given; those ids come back under `existing`), `remove_objects` with `points` (a
  point already cleared there: its clear comes back under `existing`), `place_crossing` (a crossing with both ends within
  5 ft of an existing one's, either way round; another kind, material or width is refused, naming it), `set_plateau`
  without `id` (an outline within 1 ft at every corner of an existing one; another level or edge is refused, naming it) and `upload_sprite` (the same picture again: `existing: true`, and
  `replaced` says what name and rules it had; a picture has one name and rules, the last upload's). To make a second
  one on purpose, give it another name, title or place. A change that was made but could not be saved to disk is not
  an error: its reply carries `saved: false` and a `warning` (don't repeat the call; mapd saves it again every few
  seconds); a call that changed nothing never carries it.
- **Deletes:** `delete_feature` retires a created site (a drawn building's `b:` ids with it), but keeps what pointed at
  it (NPCs, plots, notes, a hand design, renames) and says which in `left_pointing`; `restore_feature` brings the site
  back, and what pointed at it is as it was. Every tool says in its `annotations` whether it is read-only, destructive,
  idempotent; none reaches outside the world (`openWorldHint: false`).
- **Moves are reported:** a tool that puts something elsewhere than asked (a site pushed off a river onto dry land, a
  footprint snapped to the grid) says so when it is more than 5 ft: `moved: {x_ft, y_ft, ft, why}`.

| What | Name | Also accepted | Tools |
|---|---|---|---|
| A point | `at`: an id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | `id` (where it is not the tool's subject), `x_ft` + `y_ft` | `describe_location`, `features_near`, `list_roads`, `get_battlemap`, `render_view`, `focus_view`, `create_feature`, `ground_heights`, `shape_ground` |
| Other points | `near` (`create_building`), `from`/`to` (`route`, `place_crossing`): the same three forms | | |
| Generated objects to take away | `points` `[{kind, x_ft, y_ft}, ...]` | `at` (a list here only) | `remove_objects` |
| An NPC's place | `location` (an id) and/or a point | `at` (an id is the `location`, a point is `x_ft`/`y_ft`) | `create_npc`, `place_npc` |
| An area | `within`: a box `[x0, y0, x1, y1]`, a polygon `[[x, y], ...]` (3+ points) or a circle `{at, radius_ft}` (or `{x_ft, y_ft, radius_ft}`) | `area` | `remove_buildings`, `remove_objects`, `restore_objects` (`get_town_plan`: a box) |
| Inside a building or site | `inside`: its id | `within` given as an id | `list_names` |
| A footprint | `poly` `[[x, y], ...]`, `rect {x_ft, y_ft, width_ft, depth_ft, angle_deg}`, `circle {x_ft, y_ft, radius_ft}` | `outline` for `poly` | `create_building`, `update_building`, `check_building_spot` (all three forms); `create_feature` and `update_fortification` (castles: `poly` or `rect`); `set_plateau` (`poly` only, or `around`) |
| A line | `path` `[[x, y], ...]` | walls keep `pts` and also take `path` | `shape_ground`, `create_feature` (wall), `update_fortification` |
| One or many ids | `ids` (a list) or `id` (one), either; all or nothing | | every remover and restorer: `delete_feature`, `restore_feature`, `delete_npc`, `delete_plot`, `delete_plateau`, `remove_objects`, `restore_objects`, `remove_buildings`, `restore_building`, `remove_crossings` |
| Notes | `notes` on features and NPCs, `text` on plot points | the other one | `annotate_feature`, `update_feature`, `create_npc`, `update_npc`, `create_plot`, `update_plot` (not `set_site_design`: its `text` is the plan) |
| A sprite's rules | `blocks_movement`, `blocks_sight`, `cover` as a word (`none`, `half`, `three-quarters`, `full`), `size_squares` | `blocks_move`, `cover` 0 to 3, `size` | `upload_sprite` |
| Units | `_ft` feet, `_mi` miles, `_squares` squares, `_deg` degrees, in every name that carries one | | |
| Dry runs | `dry_run`, only where a tool lists it; elsewhere it is an unknown argument | | `set_site_design`, `edit_town` |
| Lists returned | `{items, found}` (+ `truncated`) | | `search_features`, `features_near`, `list_names`, `list_roads`, `list_npcs`, `list_plots`, `list_sprites` (each item says `uploaded`), `list_crossings`, `list_objects`, `list_plateaus`, `list_designs` |
| Moves reported | `moved: {x_ft, y_ft, ft, why}` | | `create_feature`, `create_building` (a crossing's ends are never moved) |

- **Names** in results already include renames, and search matches them. Hidden features are left out of search
  unless you ask for them.
- **Extents:** search results, `get_feature` and `features_near` give each place's `extent`: `bbox_ft` (`[x0, y0,
  x1, y1]`) with `area_sq_mi`, or a river's `length_mi`. Settlements' and sites' areas are their built-up circle.
- **Sites in settlements:** a site (created, drawn by hand or generated) is *in* a settlement when its point stands
  within the settlement's holding radius: its built-up area and its outer districts (1.4 built-up radii for a
  village, 2.15 for a town and up). When several hold it, the nearest for its size wins. A wall or castle has one
  settlement, from its point. Such a site is listed under the settlement (`get_feature` `sites`, `list_children`,
  `list_names`), and the settlement's NPCs and plots include those at the site.
- **`settlement` and `site`:** `get_feature` on a site, or on anything inside a settlement's or site's layout
  (building, district, tower, underground site, level, room), gives `settlement`: the town it is in, `{id, name}`,
  or `null` when it stands in none. Inside a site's layout it also gives `site`: `{id, name}` (for levels and rooms,
  `site` is the building's or site's id).
- **Which world:** every tool takes an optional `world` (a string: the hash `world_overview` returns). If mapd has
  another world open (the user opened a different one in the app), the call is refused and nothing happens. Pass it in
  long scripted sessions.
- **An error means nothing changed**, with one exception: a write that went through but could not be saved to disk is
  a success carrying `saved: false` and a `warning` (don't repeat it). Before a write, `search_features` finds the id
  of a name you were not sure of, `check_building_spot` and a `dry_run` (where a tool has one) try it without
  keeping it.
- **Undo:** the user has Undo in the app for each edit. For an agent, deletes come back with `restore_feature`
  (sites), `restore_objects` (cleared generated objects) and `restore_building` (the world's own buildings taken
  away or changed); `remove_crossings`, `delete_npc`, `delete_plot` and `delete_plateau` have no restore: make them
  again.

### Ids

| Pattern | Example | What |
|---|---|---|
| `<kind>:<hash>` | `city:bab44567`, `range:e48b3d59`, `river:…` | Named features: continents, seas, ranges, peaks, forests, rivers, lakes, settlements, ruins, caves… |
| `b:<layout>:<i>` | `b:0:57` | A building in a settlement or site |
| `d:<layout>:<q>` | `d:0:3` | A district |
| `t:<layout>:<k>` | `t:0:2` | A wall tower |
| `rd:<8 hex>` | `rd:c3517b52` | A road stretch (between settlements and junctions, as `list_roads` lists them). Made from its two ends (rounded to 10 ft, either way round; stretches with the same ends told apart by length, so two such twins may swap ids if a sketch change alters one's length), so it stays the same across every edit that leaves the roads as generated (renames, notes, sites, buildings, objects, crossings, town edits); a remake of the world, or a sketch change that moves one of its ends, gives it a new id. Renamed and annotated like a feature |
| `v:<id>` | `v:zbfopudy` | A crossing put down by hand (bridge, ford, ferry): renamed and annotated like a feature |
| `<layout>` | `0` | A town's entry in `towns` (the ward editor): its layout number; its patches and corners are numbered by `get_town_plan` |
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
| `h:<id>` | `h:d470a78s` | A plateau or pit (ground shaped by hand) |
| `g:<x>:<y>` | `g:16033:27870` | A block of ground brushed by hand: 16 × 16 squares (80 ft), block indices zero-padded to five digits |
| `gone:<n>:<id>` | `gone:1:village:d63861ee` | A settlement taken back or removed in the app: the notes, NPCs' places, plot points' places and its name (in `renames`) made on it are kept here; it names nothing |
| `was:<version>:<id>` | `was:57:village:5fde5f79` | A settlement or site of a world upgraded from that generator version to one that makes every town and site anew (58): its name, note, hidden mark, NPCs' and plot points' places are kept here; it names nothing |

Layout numbers count the settlements, then the world's own sites (those drawn in the sketch last), then the created
sites in the order they were made. No place is generated from its number: a created site from its `c:<n>` id, a
settlement from where it stands (its cell, or where its pin was drawn), one of the world's own sites from its
place. When the world is remade (its sketch redrawn), their buildings, interiors, sewers and sites underground are
generated the same as long as their roads and rivers are, though their layout numbers move when the world has
more or fewer places before them, and the ids inside them with them (`b:`, `u:`, `w:`, `l:`, `r:`). Look a place up
again with `get_feature`.

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
tier's spacing round it. One added in the app's Edit › Places (a pin marked `added`) stands beside the world's
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
| Inside settlements | `district`, `building`, `tower`, `underground`, and the sites standing in it as `site` (in `list_names`) |
| Inside buildings and sites | `level`, `room` (in `list_names`) |

---

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

### `search_features`
Searches by name, as the places are named now (renames included). From three letters, it also finds districts and
named businesses (inns, temples, smithies…) and wall or gate towers that have been renamed; road stretches that have been
renamed (kind `road`, as the roads drawn in the sketch) and crossings put down by hand that have a name (kind `crossing`).
One of `query`, `kind` or `tag` must be given (nothing at all is refused); a `kind` nothing has is refused with a "did
you mean" and the kinds there are. A district, building or tower says its `settlement` as `{id, name}` (and its
`site`, in a site's layout), as `get_feature` does. A building drawn by hand shows the ward of its own little layout
(`village`), not the one it stands in. Returns `{items, found}` (`truncated: true` when `limit` cut the list); each item has its `extent`.

| Parameter | Type | Default | |
|---|---|---|---|
| `query` | string | `""` | Part of a name. Empty, together with `kind` or `tag`, lists every feature of that kind or tag. |
| `kind` | string | none | Only this kind: `city`, `ruin`, `river`, `building`, `district`, `tower`, … |
| `tag` | string | none | Only features whose notes carry this tag (any case): the DM's `hook`, `secret`… |
| `in_notes` | boolean | false | The query also matches the words of a feature's notes (not only its name); needs a `query`. |
| `limit` | integer | 25 | 1 to 200 |
| `include_hidden` | boolean | false | Also return hidden features. |

```json
{ "name": "search_features", "arguments": { "query": "gate" } }
{ "name": "search_features", "arguments": { "query": "", "kind": "volcano" } }
{ "name": "search_features", "arguments": { "query": "Prancing", "kind": "building" } }
{ "name": "search_features", "arguments": { "tag": "hook" } }
{ "name": "search_features", "arguments": { "query": "cult", "in_notes": true } }
```
> *"Find every inn called something with 'Pony'."* · *"List all the volcanoes."* · *"Is there a town named Wolfbarrow?"*

### `get_feature`
One feature in detail. What comes back depends on the feature:

| For | Fields |
|---|---|
| Any feature | `name`, `kind`, `detail`, position, elevation, `notes`, `hidden` |
| Any place | `in`: the regions it lies in (continent, range, forest…), and `nearest_settlements` |
| Settlements | Roads to other settlements, with road miles; districts; notable buildings; ways underground; flights of steps on steep streets as generated (`stairs`: each flight's middle `x_ft`/`y_ft`, `length_ft`, `rise_ft`, the highest first, up to 40; cities and up); `sites`: the sites in it (`id`, `kind`, `name`, and a drawn building's `building` id) |
| Sites | The layout: buildings, and ways underground with their `u:` ids; `settlement`: the town it stands in, or `null` |
| Buildings and underground sites | Levels and rooms |
| Buildings | `roof_material`, `roof_shape` and `roof_shade` (as `create_building` takes them; none on battlements or ruins), `footprint`: `poly` (corners in order, ft), `angle_deg` (its long side, clockwise from east, 0–180), `length_ft`, `width_ft`, `area_sq_ft`, and a castle's keep its `pieces` (`kind`: body, round tower, square tower, forebuilding, wing; `poly`), a temple its own (body, transept, apse), whose union `poly` is (drawing a new outline makes it one piece); `structure`; `edited` when changed by hand; a building taken away (`remove_buildings`) comes back as `removed` with what it was |
| Anything in a layout | `settlement` (and in a site's layout `site`): see Conventions. A building drawn by hand (`b:<layout>:0`) used to give its own site as `settlement`; it now gives the town it stands in (or `null`), and its site as `site` |
| Places with NPCs or plots | `npcs`, `plots`: those at the place, at anything in its layout, at their levels and rooms; for a settlement also at its sites |
| A building drawn by hand | `notes`, `hidden`, `npcs` and `plots` are the same under its `c:` id and its building id |
| A road stretch (`rd:`) | `kind: "road"`, `class`, `length_mi`, `from`/`to` (as `list_roads`), `name` (renamed, or the drawn road's; else `null`), `renamed: true`, `drawn_road` `{id, name}` when it is a piece of a road drawn in the sketch, `line` (at most 40 points, ft), position halfway along, `notes`, `hidden`, `nearest_settlements` |
| A crossing (`v:`) | `kind: "crossing"`, `crossing` (bridge, ford, ferry), ends, width, length, `material`, `name` (as given, else `Timber bridge`, `Stone bridge`, `Ford`, `Ferry`), `generated`, `notes`, `river` (as `place_crossing` gives it) |

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |

```json
{ "name": "get_feature", "arguments": { "id": "metropolis:6eee9055" } }
{ "name": "get_feature", "arguments": { "id": "c:2" } }
```
> *"Tell me everything about Agentholm."* · *"Which roads leave Gatewatch, and how long are they?"*

### `list_children`
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
{ "name": "list_children", "arguments": { "id": "city:bab44567" } }
{ "name": "list_children", "arguments": { "id": "u:312:0" } }
```
> *"What shops are in the market district?"* · *"Walk me through the dungeon under the Sunken Bastion, level by level."*

### `list_names`
Everything that can be renamed, with ids to pass to `rename_feature`. Each entry has `id`, `kind`, `name` (as
renamed), `generated` (the name before renames), `x_ft`/`y_ft` where it has a spot, `enter: true` for buildings and
sites with levels, and `hidden: true` for hidden features. Returns `{items, found}`. A `kind` nothing has is refused
with a "did you mean" and the kinds there are (`kind: no names of kind "sitee"; did you mean "site"?`). Road stretches
are listed only once renamed (kind `road`; `list_roads` gives every stretch's id); crossings put down by hand are all
listed (kind `crossing`).

| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | optional | Only this kind: `river`, `range`, `city`… or `district`, `building`, `tower`, `underground`, `site`, `level`, `room`. |
| `settlement` | string | optional | A settlement's or site's id: its districts, businesses, wall and gate towers and underground sites (sewer grates aside). For a settlement, also the buildings drawn in it that have a trade (as businesses, `b:<layout>:0`) and every site in it (kind `site`, by the site's own id). |
| `inside` | string | optional | A building's or underground site's id: its levels (`l:`) and their rooms (`r:`). (`within` is accepted too.) |
| `function` | string | optional | Only businesses of this function, by `create_building`'s `func` keys (`inn`, `blacksmith`, `temple`…); needs `settlement`. Each building entry also says its `function`. An unknown one is refused with a "did you mean". |
| `limit` | integer | 200 | 1 to 2000 (`truncated: true` when it cut the list). |

Without `settlement` or `inside` it lists every named feature: oceans, seas, rivers, ranges, settlements, sites
and created sites.

```json
{ "name": "list_names", "arguments": { "kind": "river" } }
{ "name": "list_names", "arguments": { "settlement": "city:bab44567", "kind": "district" } }
{ "name": "list_names", "arguments": { "inside": "u:312:0" } }
```
> *"Give every room in the crypt under the Sunken Bastion a proper name."* · *"Which rivers have I renamed?"*

### `describe_location`
What is at a point:
- elevation (the world's coarse ground, before shaping by hand: `ground_heights` has the battlemap's) and biome;
- `in`: the areas it lies in, largest first (continent or island, sea, range, forest, lake…);
- the building, district or site there (a building's `district` is its name as renamed, `district_id` its id; a
  building of a site standing in a settlement, such as one drawn by hand, has the settlement's district there);
- `nearby`: named features within a tenth of their size (2 to 10 miles), nearest first, measured to their nearest
  edge or course, with `distance_mi` and `direction`;
- the nearest settlements.

| Parameter | Type | | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | (`id`, or `x_ft` + `y_ft`, still work) |

```json
{ "name": "describe_location", "arguments": { "x_ft": 2310194, "y_ft": 4127619 } }
```
> *"What's at 437 miles east, 781 miles south?"* · *"What's right around the Ruins of Neling?"*

### `features_near`
Everything named within a radius of a place, nearest first. Distances are to each feature's nearest edge or
course (0, direction `here`, inside an area). Returns `{items, found}` (`truncated: true` when `limit` cut the list); each item has `distance_mi`, `direction` and `extent`.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | (`id`, or `x_ft` + `y_ft`, still work) |
| `radius_mi` | number | 5 | 0.01 to 500 |
| `kinds` | string array | all | Only these kinds. Districts and businesses come when `kinds` names `district` or `building`, or with no `kinds` within a mile. |
| `limit` | integer | 50 | 1 to 500 |
| `include_hidden` | boolean | false | |

```json
{ "name": "features_near", "arguments": { "x_ft": 2310194, "y_ft": 4127619, "radius_mi": 15, "kinds": ["river", "lake"] } }
{ "name": "features_near", "arguments": { "id": "city:bab44567", "radius_mi": 0.5, "kinds": ["building"] } }
```
> *"Which rivers and lakes are within 15 miles of here?"* · *"What's the lake just north of the capital called?"*

### `route`
The distance between two places:
- `by_road`: road miles (plus any stretch off the road), the settlements passed on the way, the `stretches` taken in
  order (`{id, class, length_mi, name?}`: `rd:` ids, a name where one was given or the stretch is a drawn road's) and
  `follows`: the named roads followed, runs of stretches of one name joined (`{name, length_mi}`); a long way lists its
  first 50 `stretches` with `stretches_found` and `truncated: true` (`follows` stays whole);
- `straight_mi`: the distance as the crow flies, and its `direction`;
- `travel`: D&D 5e travel days at normal (24 mi/day), fast (30) and slow (18) pace.

| Parameter | Type | | |
|---|---|---|---|
| `from` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |
| `to` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | |

```json
{ "name": "route", "arguments": { "from": "metropolis:6eee9055", "to": "village:6db3886e" } }
{ "name": "route", "arguments": { "from": "city:bab44567", "to": { "x_ft": 1200000, "y_ft": 2500000 } } }
```
> *"How many days does it take to ride from Agentholm to Geirgai at a fast pace?"* · *"What's the route from Gatewatch to the capital, and which towns does it pass?"*

### `list_roads`
The road network:
- `named`: the roads drawn and named in the world's sketch, each with its id, name, class, length, the
  settlements along it, in order, and its `stretches` (`rd:` ids); then the stretches renamed alike, one entry per
  name: `{name, stretches (in order along it), class, length_mi, settlements}`;
- `items`: the roads between settlements and junctions (stretches), longest first, each with its `id` (`rd:`),
  `class` (king's road, road, track), `length_mi`, its ends `from` and `to` (a settlement's `id` and `name`, or a
  junction's `x_ft`/`y_ft`) and `name` (renamed, else the drawn road's); a drawn road's pieces say which (`road`:
  its name, and `road_id`; or `drawn: true`);
- `found`: how many roads matched (`truncated: true` when `limit` cut the list).

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | none | Only roads within `radius_mi` of this place. |
| `radius_mi` | number | 25 | 0.01 to 3000 |
| `settlement` | id string | none | Only roads ending at this settlement. |
| `class` | `king's road` (also `kings road`, `kings_road`), `road` or `track` | all | Results always say `king's road`. |
| `drawn` | boolean | false | Only roads drawn in the sketch. |
| `limit` | integer | 50 | 1 to 1000 |

```json
{ "name": "list_roads", "arguments": { "settlement": "city:bab44567" } }
{ "name": "list_roads", "arguments": { "drawn": true } }
{ "name": "list_roads", "arguments": { "x_ft": 2310194, "y_ft": 4127619, "radius_mi": 40, "class": "kings_road" } }
```
> *"Which roads leave Bladegarden, and where do they go?"* · *"Which towns does the Gilded Roadway pass through?"*

**Naming a road.** A road across country is several stretches. To name the High Road from one city to another:
`route` between them, then one `rename_feature` per `rd:` id in `by_road.stretches` (in one `batch`), all with the
same name. `list_roads` then gives it as one entry in `named`, `route` says it `follows` it, and search finds its
stretches. `name: ""` takes a stretch's name off again. Renaming one stretch of a named road over MCP splits it
(the app renames all of its stretches together): to rename a whole road, use the batch above with every stretch. Only roads someone named carry a name; nothing is generated.

```json
{ "name": "batch", "arguments": { "steps": [
  { "tool": "rename_feature", "arguments": { "id": "rd:6eaa3b7c", "name": "The High Road" } },
  { "tool": "rename_feature", "arguments": { "id": "rd:28a39f0b", "name": "The High Road" } } ] } }
```

### `get_battlemap`
The 640-ft battlemap chunk (128 × 128 five-foot squares) that contains a place, for running a fight there. It
lists:
- the elevation range, surfaces and buildings (`stairs`: a flight of steps on a steep city street, paved, walkable,
  no movement cost; also where ground shaped by hand makes a city street climb steeply; `gravel`: a park's paths);
- each kind of object in the chunk, with its 5e rules: cover, whether it blocks movement or sight, difficult
  terrain, hazards and their effects;
- the objects within `radius_squares` of the place (placed objects are read within the one chunk only; `list_objects`
  lists every one in the world), by grid square (`near_focus`), each with its centre
  (`x_ft`, `y_ft`) and either its `id` (put down by hand: `place_objects` moves or turns it, `remove_objects`
  takes it; `rot_deg` when it was given a turn) or its built-in `kind` (generated: `remove_objects` takes it by
  `at`);
- the clears in the chunk (`cleared`: id, centre, kind or radius; `restore_objects` takes them).

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | (`id`, or `x_ft` + `y_ft`, still work) |
| `radius_squares` | number | 12 | 1 to 64 |

```json
{ "name": "get_battlemap", "arguments": { "id": "c:2", "radius_squares": 20 } }
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
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | (`id`, or `x_ft` + `y_ft`, still work) |
| `size_ft` | number | 3000 | The ground across the image (50 to 5,000,000). |
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
{ "name": "render_view", "arguments": { "id": "c:2", "size_ft": 400 } }
{ "name": "render_view", "arguments": { "id": "range:e48b3d59", "size_ft": 200000 } }
```
> *"Show me the battlemap at the ruin."* · *"What does the land around the Katsei Mountains look like?"*

### `focus_view`
Flies your map view to a place. You see it happen.

| Parameter | Type | Default | |
|---|---|---|---|
| `at` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required | (`id`, or `x_ft` + `y_ft`, still work) |
| `size_ft` | number | 3000 | How much ground fits on screen (50 to 5,000,000). |

```json
{ "name": "focus_view", "arguments": { "id": "city:bab44567", "size_ft": 20000 } }
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
  { "tool": "rename_feature", "arguments": { "id": "d:4:2", "name": "Tri-Spire Ward" } },
  { "tool": "annotate_feature", "arguments": { "id": "d:4:2", "notes": "Three wizard towers lean over its lanes." } },
  { "tool": "create_feature", "arguments": { "kind": "ruin", "under": "crypt", "x_ft": 2325000, "y_ft": 4127000 } },
  { "tool": "rename_feature", "arguments": { "id": "c:0", "name": "The Evening Nip" } }
] } }
```
> *"Name every district in the capital and give each a line of lore, all in one go."*

### `rename_feature`
Renames anything with a name: a feature, building, district, tower, underground site, a site's level (`l:`) or
room (`r:`), a road stretch (`rd:`, ids from `list_roads` or `route`) or a crossing put down by hand (`v:`). Use
`list_names` for the ids. The app shows the new names everywhere, including the level list and
the room labels inside. A building drawn by hand is one place, whether it is named by its `c:` id or its
building id (`b:<layout>:0`): one name, one note (kept under `c:`) and one hidden mark.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | required | An empty string removes a rename and restores the generated name. |

```json
{ "name": "rename_feature", "arguments": { "id": "city:bab44567", "name": "Gatewatch" } }
{ "name": "rename_feature", "arguments": { "id": "b:0:57", "name": "The Gilded Griffon" } }
{ "name": "rename_feature", "arguments": { "id": "r:u:312:0:1:4", "name": "Hall of the Drowned King" } }
{ "name": "rename_feature", "arguments": { "id": "city:bab44567", "name": "" } }
```
> *"Rename the capital to Agentholm."* · *"Call the biggest tavern in Bulol 'The Drunken Kraken'."* · *"Put Fyckmere's old name back."*

### `annotate_feature`
Sets a feature's notes: lore, adventure hooks, secrets for the DM. The app shows them in the info panel. A
building drawn by hand keeps its note under its `c:` id, whichever id is given (the reply's `id`); a note left
under its building id is joined onto it.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | |
| `notes` | string | one of `notes`, `tags`, `remove_tags` | (`text` is accepted too.) Left out, the notes stay as they are; the reply echoes `notes`. |
| `tags` | string[] | none | Added to the existing tags. |
| `remove_tags` | string[] | none | Taken off; one that is not there is fine. The reply's `removed_tags` says which were, and `tags` what is left. (`update_feature` with `tags: []` clears them all.) |
| `append` | boolean | false | Add to the existing notes instead of replacing them. |

```json
{ "name": "annotate_feature", "arguments": { "id": "c:2", "notes": "The night watch vanishes one by one; the trail leads here.", "tags": ["hook"] } }
{ "name": "annotate_feature", "arguments": { "id": "c:2", "notes": "Secret: the baron funds the cult.", "tags": ["secret"], "append": true } }
```
> *"Write a short history for every ruin in the Emberwood."* · *"Add a rumour to the Gilded Griffon that the innkeeper is a retired assassin."*

### `create_feature`
Makes sites, castles and walls (not buildings: `create_building`; not towns or settlements: see "What agents can't do").
Adds a new site, built by the same generators as the world's own: map symbol, buildings, battlemap, interiors,
and a full multi-level site underground (a ruin's dungeon, crypt or catacombs; a cave, mine or lava tube; or an
`entrance`, a bare way down with nothing built above it). The result includes the new `c:<n>` id and its
underground ids.

Creating the same site again adds nothing: if a live site of the same `kind` (and the same `under`) stands within
300 ft of the place asked for, or of the spot the new one would take, the result is that site, with
`"existing": true` (and `note` says it was already there, not created). A script that creates sites can be run again
safely; for a second one on purpose, ask for it somewhere else.

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
{ "name": "create_feature", "arguments": { "kind": "ruin", "under": "crypt", "x_ft": 1158199, "y_ft": 2720810, "name": "The Sunken Bastion" } }
{ "name": "create_feature", "arguments": { "kind": "waystation", "x_ft": 3700000, "y_ft": 1650000 } }
{ "name": "create_feature", "arguments": { "kind": "entrance", "under": "dungeon", "theme": "dwarven_hall", "size": "huge", "levels": 5, "x_ft": 2400000, "y_ft": 1900000 } }
{ "name": "create_feature", "arguments": { "kind": "tower", "x_ft": 4600000, "y_ft": 2050000, "name": "Spire of Vael" } }
```
> *"Put a ruined watchtower with a crypt under it two miles north of Gatewatch."* · *"Hide a small two-level goblin warren in the hills east of Bulol."* · *"Add a roadside inn halfway between Agentholm and Bulol."* · *"Make a bandit camp in the Emberwood."*

### Castles and walls
`create_feature` with `kind: "castle"` or `"wall"` takes a shape instead of a place. A **castle** is an outline
(`poly`, convex, 3–32 corners, 80 to 600 ft across, measured along its longest diagonal: a `rect` of 500 × 350 is 610
across and refused; or `rect` about its middle): a curtain wall just inside it, a
tower on every corner and along long runs (round, or square in some castles), a gatehouse on one side (`gate`: the
side from corner `gate` to the next; else the side facing the nearest road), in a big one (80,000 sq ft or more and
200 ft across) an inner curtain with its own gatehouse round the keep's high ground, leaving an outer bailey at
least 48 ft wide all round and the inner curtain at least 80 ft from the outer gate, its gate at least 90 ft from
that one; the keep (the castle itself, named as the site;
`keep: false` for none) on the highest ground that holds it, made of pieces: a great tower with corner towers, a hall
with a forebuilding, an L or a T with towers at its outer corners, or a round shell keep on a motte (a mound eased up
from the ground, a gravel path up it to the shell's gate; a tower keep alone where nothing else fits), its flat tops
drawn as open battlements (a wall walk of stone flags in the merlons, a broad one's middle dark lead) or, every top of
1,100 sq ft or more and in some keeps the smaller ones too, hipped roofs of lead or slate; and ranges of
stone buildings under slate against the inside of the curtain, clear of the way from gate to gate (the biggest the
barracks, then stables and a smithy; `yard_buildings: false` for none). The bailey inside is an open yard of grass, no
wild trees or rocks, set out sparingly: stores by the ranges, a cart, a trough or barrels inside each gate, a well in
the keep's court, a little cover out in the open yard (haystacks, carts, troughs, firewood; small stores by a wall or
a building: its curtains, towers and buildings are the yard's cover, so the ground near them needs none), nothing on a
motte or in the way from gate to gate and to the keep; in some castles a garden yard: a plot
by the keep (flowerbeds, topiary at its corners, benches, a short hedge, now and then a fountain) and a few planted
trees. Buildings can be drawn in it. With
`structure: "ruin"` the walls are broken, some towers fallen and every building a roofless shell.

A **wall** is a line (`pts`, 2–64 corners, each side at least 10 ft, at most 3,000 ft in all; `closed: true` joins
the last corner to the first): towers on its corners and along long runs, a gatehouse on each corner listed in
`gates` (indices into `pts`) and wherever a road, a town's approach or its main street crosses it (side streets and
alleys are closed by it). Where it would stand in water it is broken (a tower on each bank).

Both must stand on dry land, off river channels, and not cross another wall; a castle also stays off squares, roads
and main streets. Buildings drawn by hand in the way refuse it; the world's own buildings in the way are named in the
refusal, or taken away with it (one change) when `remove_in_way: true`. Corners snap to the 5-ft grid (`snap: false`
not). Their wall and gate towers have interiors (`t:<layout>:<k>`), and a building drawn on the wall is refused. The
result also gives `towers`, `gates`, `buildings` and the castle's `keep` (its building id).

```json
{ "name": "create_feature", "arguments": { "kind": "castle", "rect": { "x_ft": 4772760, "y_ft": 1498825, "width_ft": 300, "depth_ft": 200 }, "gate": 2, "name": "Highmoor Keep" } }
{ "name": "create_feature", "arguments": { "kind": "wall", "pts": [[4772500, 1499150], [4773050, 1499150], [4773200, 1498650]], "gates": [1] } }
{ "name": "create_feature", "arguments": { "kind": "wall", "pts": [[412000, 380000], [413200, 380000], [413200, 381000], [412000, 381000]], "closed": true, "remove_in_way": true } }
```
> *"Build a castle on the hill west of Bulol with its gate toward the road."* · *"Wall off the north side of Agentholm."* · *"A ruined fort, keep only."*

### `update_fortification`
`id` (a castle's or wall's `c:` id) and any of `create_feature`'s castle or wall fields: a new outline or line, its
gate or gates, `keep`, `yard_buildings`, `structure`, `closed`, `name`. Fields left out stay; the result is checked
like a new one (`remove_in_way` as there). `delete_feature` takes it away.

### `update_feature`
Changes a name and the notes in one call. Unlike `annotate_feature`, `tags` here replace the existing tags. A
building drawn by hand takes either id, as in `rename_feature` and `annotate_feature`.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | optional | |
| `notes` | string | optional | Replaces the notes text. (`text` is accepted too.) |
| `tags` | string[] | optional | Replaces the tags; `[]` clears them. |

Give at least one of `name`, `notes` and `tags`; with none the call is refused.

```json
{ "name": "update_feature", "arguments": { "id": "c:2", "name": "Bastion of the Drowned", "notes": "Flooded at high tide.", "tags": ["dungeon", "undead"] } }
```

### `hide_feature`
Hides a feature from the map's labels and from search, or shows it again. It still exists, and
`search_features` with `include_hidden` finds it. A building drawn by hand is hidden by either id (the mark is
kept under its `c:` id), and its business pin is hidden with it.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | |
| `hidden` | boolean | true | `false` shows it again. |

```json
{ "name": "hide_feature", "arguments": { "id": "town:764adcb3" } }
{ "name": "hide_feature", "arguments": { "id": "town:764adcb3", "hidden": false } }
```
> *"Hide the town of Zhuol; the players haven't discovered it yet."* · *"Reveal Zhuol."*

### `delete_feature`
Deletes created sites (`c:<n>`: ruins, towers, camps, castles, walls...) and buildings drawn by hand: `id` or `ids`, all or none in one change. A building drawn by hand goes by either of
its ids (`c:<n>` or `b:<layout>:0`). Generated features can't be deleted, only hidden; the world's own buildings go
with `remove_buildings`. The ids are retired and never reused: the site's `b:`, `d:`, `u:` and other layout ids stop
answering (`get_feature` says `b:320:0 was deleted with c:2 (restore_feature brings it back)`), and searches, lists and
`describe_location` skip it. What pointed at it is kept as it is, and listed per site in `left_pointing` (only what
applies): `npcs`, `plots` (ids), `notes: true`, `design: true`, `renames` (a count). Deleting a deleted site is
refused (`c:11 is already deleted (restore_feature brings it back)`).

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | one of | A `c:` id, or a drawn building's `b:<layout>:0`. |
| `ids` | string[] | one of | Several; all or none. |

```json
{ "name": "delete_feature", "arguments": { "id": "c:0" } }
```
> *"Delete the ruin we made earlier."*

### `restore_feature`
Brings back sites deleted with `delete_feature`: `id` or `ids` (`c:<n>`, or a drawn building's `b:<layout>:0`; all or
none, one change). The ids answer again, with the NPCs, plots, notes, design and renames that still pointed at them.
Refused when the spot is taken now, by the checks a new one of that kind meets (`b:… can't be restored, its place is
taken: that overlaps …`; for a castle or wall, the world's own buildings in its way are named), or when the site is
not deleted. Returns the site as `get_feature` does (a building with its `building` id); several return
`{ids, restored: [sites]}`.

```json
{ "name": "restore_feature", "arguments": { "id": "c:0" } }
```
> *"Bring the ruin back."*

## Notebook tools: NPCs and plot points

The DM's notebook holds NPCs (`n:<id>`) and plot points (`p:<id>`). They are written by the user (the app's
**Notebook**) or by agents, never generated. Edits to them are saved, logged and shown live like the other edit
tools. Ids are random (`n:k3x9q0ab`), so the app and an agent can create at the same time.

- An NPC can be **placed** at any place id. Placed in a building or site (`b:`, `t:`, `u:`, `k:`, `w:`), they
  show inside it on the DM's map as a marker, on their `level` (default: the way in), at their point if given,
  else in that level's biggest room. The DM can drop them into play as a token.
- A plot point is tied to places (`anchors`) and NPCs (`npcs`).
- `get_feature` and `describe_location` include the NPCs and plot points there (`npcs`, `plots`). For a
  settlement or site that includes those in its buildings, districts, towers and ways underground, and on their
  levels and rooms; for a settlement, also those at the sites in it (see Conventions).
- `focus_view` and `route` take an NPC's id (where they are) or a plot's (its first place).
- The players' window never gets NPCs, plots or notes.

### `list_npcs`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches name, appearance, mannerisms, goals, notes, attitude. |
| `location` | string | optional | A place id. Also finds NPCs on its levels and in its rooms; a settlement's or site's id, NPCs in its buildings, districts, towers and ways underground; a settlement's, NPCs at the sites in it too. A building drawn by hand by either of its ids. |
| `tag` | string | optional | |
| `stance` | string | optional | `hostile`, `unfriendly`, `neutral`, `friendly` or `allied`. |
| `limit` | integer | 100 | 1 to 1000 (`truncated: true` when it cut the list). |

Returns `{items: [{id, name, stance, status, location: {id, name, level?, x_ft?, y_ft?}, tags}], found}`.

### `get_npc`
An NPC in full: `name`, `appearance`, `mannerisms`, `attitude: {stance, text}`, `goals`, `notes`, `tags`,
`portrait` (an asset id), `status`, `location`, and the `plots` they are in.

### `create_npc`
| Parameter | Type | | |
|---|---|---|---|
| `name` | string | required | |
| `appearance`, `mannerisms`, `goals`, `notes` | string | optional | `notes` are the DM's (secrets); `text` is accepted for `notes`. |
| `stance` | string | optional | Attitude toward the players (default `neutral`). |
| `attitude` | string | optional | Why they feel that way, what would change it. |
| `tags` | string[] | optional | |
| `status` | string | optional | Free text: alive, dead, missing, imprisoned… |
| `portrait` | string | optional | An asset id (`PUT /assets/<id>`, or a picture uploaded in the app). |
| `location` | string | optional | A place id. |
| `level` | integer | optional | Inside a building or site: 0 (the lowest level) or more. |
| `x_ft`, `y_ft` | number | optional | A point to stand at (both; on the map). Without `location`, the building, district or settlement there (else the nearest named place) is used. |
| `at` | id, point | optional | Instead of those: an id is the `location`, `{x_ft, y_ft}` or `[x_ft, y_ft]` is the point. Not together with `location`, `x_ft` or `y_ft` (refused). |

```json
{ "name": "create_npc", "arguments": { "name": "Old Brannoc", "appearance": "One eye, a scarred chin", "stance": "unfriendly", "attitude": "Distrusts adventurers since the fire", "goals": "Sell the inn and retire", "location": "b:0:620", "tags": ["innkeeper"] } }
```
> *"Give the Jolly Lantern an innkeeper with a secret, and put him behind the bar."*

Safe to repeat: an NPC with the same name (case and surrounding spaces ignored) and the same place, or both without
one, is that NPC: the result is it, with `"existing": true`, and nothing is added. Another name or place makes a second.

### `update_npc`
`id` plus any of `create_npc`'s fields except the place (`name`, `appearance`, `mannerisms`, `attitude`, `goals`, `notes`, `stance`, `status`, `tags`, `portrait`); only the fields given change.

### `place_npc`
`id` plus `location` (and `level`), `x_ft`/`y_ft` or `at`. An empty `location` takes them off the map. An id that is an NPC with no place, or a plot point, is not a place: the refusal says so.

### `delete_npc`
`id` or `ids` (all or none, in one edit). They are also taken out of every plot point.

### `list_plots`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches title and text. |
| `status` | string | optional | `idea`, `active` or `resolved`. |
| `anchor` | string | optional | A place id. |
| `npc` | string | optional | An NPC id. |
| `tag` | string | optional | |
| `limit` | integer | 100 | 1 to 1000 (`truncated: true` when it cut the list). |

Returns `{items, found}`.

### `get_plot`
`id`: a plot point in full: `title`, `text`, `status`, `anchors` and `npcs` (each with its name), `tags`.

### `create_plot`
| Parameter | Type | | |
|---|---|---|---|
| `title` | string | required | |
| `text` | string | optional | What is going on, hooks, how it may play out; `notes` is accepted for `text`. |
| `status` | string | optional | `idea` (default), `active` or `resolved`. |
| `anchors` | string[] | optional | Place ids (not NPCs). |
| `npcs` | string[] | optional | NPC ids; they must exist. |
| `tags` | string[] | optional | |

```json
{ "name": "create_plot", "arguments": { "title": "Smugglers in the cellar", "text": "Barrels move at night...", "status": "active", "anchors": ["b:0:620"], "npcs": ["n:k3x9q0ab"] } }
```
> *"Make a plot point for the smuggling ring, tie it to the inn and the harbour district, and involve Brannoc."*

Safe to repeat: a plot point with the same title (case and surrounding spaces ignored) is that plot point, with
`"existing": true`; nothing is added. Another title makes a second.

### `update_plot`
`id` plus any of `create_plot`'s fields (`title`, `text` or `notes`, `status`, `anchors`, `npcs`, `tags`); lists given replace the old ones.

### `delete_plot`
`id` or `ids` (all or none).

---

## Battlemap objects: scatter and sprites

Battlemaps are generated with their trees, rocks, props and hazards. On top of that, objects can be **put down**
by hand (`o:<id>` entries) and generated ones **taken away** (clears, `x:<id>` entries), by the user (the app's
**Edit › Objects** tab) or by agents. Pictures can be **uploaded** as new kinds of object, with their own tactical
rules (`s:<asset>` kinds). Objects block, cover and hinder in play mode by their kind's rules, and
`get_battlemap` lists them. Like the other edits, they are saved, logged, shown live, and leave the world hash
alone; only the battlemap chunk holding each object (by its centre) is made again.

- A **built-in kind** is named by its id or name (`12` or `"boulder"`); `list_sprites` lists them all with their
  rules.
- An **uploaded sprite** is named `s:<asset>` (or by its name). Its picture is in the asset store
  (`/assets/<id>`; the id is the first 16 bytes of the picture's SHA-256 in hex, as the app makes it). A top-down
  picture with a transparent background works best, at about 64 px a square.

### `list_sprites`
Parameter: `limit` (1 to 1000, default 200; `truncated: true` when it cut the list). Returns `{items, found}`: built-in kinds and uploaded sprites in one list, each
`{kind, name, uploaded, rules}` (an uploaded one also `picture` and `placed`, how many are put down); `rules` are cover (a
word), blocks_movement, blocks_sight, difficult, height_ft, size_squares and any hazard.

### `list_objects`
Every object put down by hand (`o:`) and every clear (`x:`, generated objects taken away), world-wide, in one `{items, found}`
list (objects first; `objects` and `clears` beside say how many of each), so an agent that lost its ids can audit or undo.

| Parameter | Type | Default | |
|---|---|---|---|
| `within` | box `[x0, y0, x1, y1]`, polygon `[[x, y], ...]` or circle `{at, radius_ft}` | whole world | Only those with their place inside. |
| `kinds` | (id or name)[] | all | Only these kinds (built-in id or name, or `s:<asset>`); a clear of all kinds matches any. |
| `which` | `objects`, `clears` or `both` | both | |
| `limit` | integer | 200 | 1 to 2000 |

An object item is `{id, what: "object", kind, x_ft, y_ft, variant}` and, when set, `rot_deg`, `scale` and `sprite`
(`s:<asset>`); a clear is `{id, what: "clear", x_ft, y_ft}` with `kind` (one object taken by kind and place) or `radius_ft`
and `kinds` (a circle). `remove_objects` takes objects by id; `restore_objects` brings clears back.

```json
{ "name": "list_objects", "arguments": { "within": { "at": "c:2", "radius_ft": 300 } } }
{ "name": "list_objects", "arguments": { "which": "clears", "kinds": ["boulder"] } }
```
> *"What did I put on the bridge map? Take the barrels away."*

### `upload_sprite`
| Parameter | Type | | |
|---|---|---|---|
| `data_base64` | string | one of these | The picture (png, jpeg, webp, gif or svg), base64 (a data URL works too). |
| `path` | string | | A picture file on the machine mapd runs on. |
| `asset` | string | | A picture already uploaded: change its name or rules. |
| `name` | string | optional | |
| `size_squares` | number | optional | Squares across at scale 1 (default 1; 0.2–40). Also `size`. |
| `cover` | string or integer | optional | `none` (default), `half`, `three-quarters` or `full`, as the rules are returned; or 0 to 3. |
| `blocks_movement`, `blocks_sight`, `difficult` | boolean | optional | Default false. `blocks_move` is accepted for `blocks_movement`. |
| `height_ft` | number | optional | 0–500 (default 3). Objects 5 ft and taller draw above tokens' shadows. |

Returns `{kind: "s:<asset>", asset}`. A picture already uploaded (same bytes) under another name or rules adds `existing: true` and `replaced` (its old name and the rules that changed): a picture has one name and one set of rules, the last upload's.

```json
{ "name": "upload_sprite", "arguments": { "path": "C:/art/altar.png", "name": "Blood altar", "size_squares": 2, "cover": "three-quarters", "blocks_movement": true, "height_ft": 4 } }
```

### `place_objects`
| Parameter | Type | | |
|---|---|---|---|
| `objects` | array | required | 1–500 of `{id?, kind, x_ft, y_ft, rot_deg?, scale?, variant?}`. |

A mistake in the list is refused with the item's place in it (`objects[17].scale: 0.2 to 5, got 9`, `objects[3].kind: no object kind 'boulderr'`); nothing is put down then. `kind` is a built-in kind (id or name) or an uploaded sprite. `rot_deg` turns it clockwise: it is drawn turned
for props that can turn (not trees, plants or pools) and for uploaded sprites; without it the object keeps the
way it is drawn by default. `scale` is 0.2–5 (default 1). `variant` (0–15) picks a built-in kind's look: most
have four (taken modulo 4), a market stall sixteen (its awning and its goods); default by chance. Positions must be on the map. Objects stand on the outdoor battlemap: one put down where a building
stands is under its roof. Furniture, props and traps in a building's rooms or underground go in its design
(`set_site_design`).

With `id` (`o:…`, an object put down before) the object is changed in place and keeps its id: only the fields
given change, so `{id, x_ft, y_ft}` moves it and `{id, rot_deg}` turns it (`kind` is then optional). A
generated object is moved by taking it away (`remove_objects` with `points`) and putting one down. Returns
`{tool, ids, changed}` (`changed`: the ids that were given, changed in place).

Safe to repeat: a new object of the same kind within 0.5 ft of one put down before, with the same turn and scale (and
the same `variant` when one is given), is that object: its id is in `ids` and also in `existing`, and nothing is added.
Give another place, turn or scale for a second one on purpose.

```json
{ "name": "place_objects", "arguments": { "objects": [ { "kind": "boulder", "x_ft": 412500, "y_ft": 380120 }, { "kind": "s:0123456789abcdef0123456789abcdef", "x_ft": 412520, "y_ft": 380120, "rot_deg": 90 } ] } }
```
```json
{ "name": "place_objects", "arguments": { "objects": [ { "id": "o:k2x9qa", "x_ft": 412540, "y_ft": 380125, "rot_deg": 45 } ] } }
```
> *"Put a ring of standing stones (boulders) round the hilltop east of Bulol, with my altar sprite in the middle."*

### `remove_objects`
| Parameter | Type | | |
|---|---|---|---|
| `id`, `ids` | string / string[] | optional | Objects put down by hand (`o:…`); all or none. |
| `points` | array | optional | Generated objects: `{kind, x_ft, y_ft}` each, as `get_battlemap` lists them (matched within a quarter square; a point already cleared there adds nothing and its clear is listed under `existing`). Before, this was called `at`, which still works here (`at` is one place everywhere else). |
| `within` | area | optional | Everything inside: a box `[x0, y0, x1, y1]`, a polygon `[[x, y], ...]` or a circle `{at, radius_ft}` (or `{x_ft, y_ft, radius_ft}`; radius up to 640 ft). A circle takes away placed and generated objects; a box or polygon only those put down by hand (the reply says so). Also `area`. |
| `kinds` | array | optional | With `within`: only these built-in kinds (ids or names). A `kinds` inside a circle works too. |

Returns `{tool, removed: [o:…], clears: [x:…]}` (+ `existing` for points that were already cleared).

```json
{ "name": "remove_objects", "arguments": { "within": { "x_ft": 412500, "y_ft": 380120, "radius_ft": 30 }, "kinds": ["deciduous tree", "conifer"] } }
```
> *"Clear the trees from a 60-ft circle in the woods north of the ruin so there's room for a fight."*

### `restore_objects`
`id` or `ids` (clears, `x:…`; all or none) and/or `within` (a box, a polygon or a circle `{at, radius_ft}`, also
`area`: every clear touching it: a clear's circle meeting the circle, box or polygon, or its middle inside). The generated objects come back.

## Buildings drawn by hand

Buildings and towers can be **drawn**, by the user (the app's **Build** menu) or by agents: a footprint on the map
and what the building is. Each one is a created site (`c:<n>`, kind `building`) with a layout of its own, so it
gets everything a generated building has: a roof and walls on the battlemap, an interior with its rooms and
furniture on every storey (a cellar too), its name on the map and in search. Its building id is `b:<layout>:0`
(returned as `building`); use it with `get_feature`, `place_npc` and interiors, as for any building. One drawn in
a settlement is listed under it: among its sites and, with a trade, its businesses (`settlement` on its ids names
the town; see Conventions). Like the other edits, buildings are saved, logged, shown live and leave the world hash
alone.

- **Where:** on dry land, out of rivers, clear of other buildings (walls may touch), roads, streets, town
  walls and squares (market squares, quays, village greens, a site's yard). Anything else is refused with the
  reason (`that overlaps The Gilded Goose`, `that is on a road`, `that is on a square (a market, quay or
  green)`). `check_building_spot` asks without building.
- **Footprint:** any simple shape of 3–64 corners, at least 10 ft across, every corner within 200 ft of its
  middle. L-shapes and other concave plans are fine (their roofs are drawn as several hips).
- **What it is (`func`):** a business by its catalog key (`inn`, `tavern`, `blacksmith`, `temple`, `castle`,
  `observatory`, `library`… as `list_names` shows them; the tool's `func` description lists every key) or a home: `hovel`, `house` (default), `townhouse`,
  `tenement`, `noble_estate`, `farmhouse`, `wizard_tower`. It decides the rooms inside: a taproom and guest
  rooms for an inn, a forge for a smithy, cells for a prison, a round stair and study floors for a wizard's tower
  (when the footprint is round). A `func` the code does not know is refused with the nearest key
  (`func: a business or home such as inn, tavern, ..., got "tavrn" (did you mean tavern?)`).
- **Roof:** `hip` (pitched; L-shapes get one per wing), `battlements` (a walkable roof with a parapet: the
  interior gains a roof level) or `cone`; left out, as its kind has it (battlements on castles, barracks and
  towers). **Tint:** `terracotta`, `slate`, `thatch`, `shingle` or `moss` (left out: picked).
- **Structure:** `ruin` leaves broken walls with no roof and no way in.
- Delete one with `delete_feature`, hide its label with `hide_feature`. It is one place under its `c:` id and
  its building id: notes and the hidden mark are kept under `c:` (either id reads and writes them), and NPCs and
  plots at either id are found under both.

### `create_building`
A building with the same footprint as one already drawn (every corner within 2 ft), the same `func` and (if
given) the same name is not drawn twice: the result is that building, with `"existing": true`. A different
building on that footprint is refused (`that overlaps …`). With `near`, mapd remembers this session's calls (the world and every argument): an identical call whose building
still stands returns it (`existing: true`) and draws nothing, until mapd restarts. Any difference (another name, place, size
or `func`) makes a new one, so a row of unnamed houses along a street is a row of buildings. `func` is matched
without regard to case and spaces (`"Inn"` is `inn`).

Instead of a footprint, `near` (an id, `{x_ft, y_ft}` or `[x_ft, y_ft]`) with `width_ft` and `depth_ft` finds the nearest clear
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
| `near` | id, object or `[x_ft, y_ft]` | instead of a footprint | Find the nearest clear lot near this place (see above). |
| `width_ft`, `depth_ft` | number | with `near` | The lot along the street and back from it, 10–400 ft. |

Returns the site as `get_feature` does, plus `building` (its building id), and `moved: {x_ft, y_ft, ft, why}` when
the footprint snapped to the grid (or its point set where it fits) put it more than 5 ft from where it was asked
(not with `near`, which picks a lot by design).

```json
{ "name": "create_building", "arguments": { "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 50, "depth_ft": 30 }, "func": "inn", "floors": 2, "tint": "thatch" } }
{ "name": "create_building", "arguments": { "circle": { "x_ft": 412600, "y_ft": 380200, "radius_ft": 15 }, "func": "wizard_tower", "floors": 5, "roof": "cone", "tint": "slate", "name": "The Needle" } }
{ "name": "create_building", "arguments": { "poly": [[412700, 380100], [412750, 380100], [412750, 380120], [412720, 380120], [412720, 380145], [412700, 380145]], "func": "blacksmith" } }
{ "name": "create_building", "arguments": { "near": "b:12:340", "width_ft": 60, "depth_ft": 45, "func": "tavern", "floors": 2, "name": "The Leaky Tap" } }
```
> *"Build a two-storey inn with a thatched roof at the crossroads south of Bulol, and an L-shaped smithy beside it."*

### `update_building`
`id` (a drawn building's `c:` id, or one of the world's own: `b:<layout>:<id>`) and any of the parameters of
`create_building`; those left out stay. A new footprint (`poly`, `rect` or `circle`) moves or reshapes it and is
checked like a new one (it never collides with itself).

The world's own buildings take the same changes (`edits.buildings`): what it is (`func`), `floors`, `roof`,
`tint`, `material`, `shape`, `shade`, `structure`, a new footprint (`snap` as in `create_building`), and `name` (a rename). `auto` (or an empty `func`) puts one option back as
generated. A new trade with no name of its own is named as a drawn building's would be. Every other building
keeps its id and footprint. If the world is generated again from a changed sketch and the town is laid out anew,
a change whose building no longer stands at its id is set aside (not applied to another building).

```json
{ "name": "update_building", "arguments": { "id": "c:4", "floors": 3, "roof": "battlements" } }
{ "name": "update_building", "arguments": { "id": "b:12:340", "func": "tavern", "floors": 2, "name": "The Leaky Tap" } }
```

### `check_building_spot`
Read-only: whether a footprint (`poly`, `rect` or `circle`, with `snap`) is clear for a building, as
`create_building` checks it. `id` is the building being reshaped (it does not count against itself). Returns
`{ok: true, x_ft, y_ft, name}` (its point and the name it would get for `func`) or `{ok: false, reason}`.

```json
{ "name": "check_building_spot", "arguments": { "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 50, "depth_ft": 30 }, "func": "inn" } }
```

### `remove_buildings`
Take away buildings of the world's own (drawn ones go with `delete_feature`; a drawn building's id is refused, saying so): `id` or `ids` (`b:<layout>:<id>`),
and/or every one whose middle lies inside `within`: a polygon `[[x_ft, y_ft], …]`, a box `[x0, y0, x1, y1]` or a
circle `{at, radius_ft}` (also `area`), such as a ward to clear for buildings of your own. Their ground is free to
build on; every other building keeps its id. Returns `removed` (the ids) and `count`.

```json
{ "name": "remove_buildings", "arguments": { "within": [[412400, 380000], [412800, 380000], [412800, 380300], [412400, 380300]] } }
```

### `restore_building`
`id` or `ids` (`b:<layout>:<id>`; all or none): the buildings back as generated, whether taken away or changed (a
rename stays). One id returns that building; several return `{ids, restored: [buildings]}`.

### Worlds with only drawn roads
With the world parameter `generated_roads: false` (the app's World › Generate › People & places › Roads: Only
drawn), the world has only the roads drawn in its sketch, with short spurs to the settlements beside them, and no
roadside inns; settlements are placed without regard to roads, so drawing a road never moves one. Drawing a road
into a town lays the town out anew (its gates follow its roads): draw roads before changing a town's buildings.

## Crossings (bridges, fords and ferries)

Crossings put down by hand (`v:<id>`) join one bank to the other, drawn on the battlemap over what is there. A
bridge is a walkable deck clear of the water (timber, or old stone), a ford a bed brought up to wading depth under
stepping stones, a ferry jetties out from each end with a raft between. The ends are exactly where they are given
(never moved), but an end in a river's water is refused, naming the bank point on that side to use. A crossing takes a
name and notes like a feature (`rename_feature`, `annotate_feature`, `get_feature` with its `v:` id); a named one is
found by `search_features` (kind `crossing`) and listed in Notes › Gazetteer (nothing is lettered on the map).

### `list_crossings`
Parameter: `limit` (1 to 2000, default 200; `truncated: true` when it cut the list). Returns `{items, found}`: `{id, kind, from, to, width_ft, length_ft, material?, name, notes?}` (`name`: as given, else
`Timber bridge`, `Stone bridge`, `Ford` or `Ferry`).

### `place_crossing`
| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | required for a new one | `bridge`, `ford` or `ferry`. |
| `from`, `to` | id, `{x_ft, y_ft}` or `[x_ft, y_ft]` | required for a new one | The banks (on the map). The length between them is 10–2,000 ft (a ferry at least 68). |
| `width_ft` | number | optional | Deck, ford or raft width, 5–40 (default 12). |
| `material` | string | optional | A bridge's: `timber` (default) or `stone`. |
| `id` | string | optional | A crossing to change (`v:…`): only the fields given change. Without it, a crossing with both ends within 5 ft of an existing one's (either way round) is that one: `existing: true`; a different kind, material or width is refused (give its id to change it). |

Returns `{tool, id, kind, changed, crossing}` (`changed: true` when an `id` was given; `existing: true` for the same
crossing again) and where the banks are: `river` `{id, name, width_ft, banks}` (the river it crosses, its width there,
and the two bank points across it there, the one on `from`'s side first; `id`/`name` null for a stream too small to be
named; `river: null` when none is within 1,000 ft), `from_bank_ft` and `to_bank_ft` (how far each end lies beyond its
bank), and `note` when both ends are on the same side. An end in the water is refused:
`from: (x, y) is in the water of The Red River (river:f32acc5b), 419 ft wide there: its bank on that side is at about (x, y)`.

### `remove_crossings`
`id` or `ids` (`v:…`; all or none). A mistake names the id's place in the list (`ids[1]: no crossing v:zz`; a single `id` is `id: …`).

## Underground sites designed by hand

An underground site (`u:<layout>:<k>`: a dungeon, crypt, catacombs, cave, mine or lava tube; not a city's sewers)
can be **redesigned**, by the user (the app's designer: *Edit › Rooms* inside it) or by agents, as a text
plan. The first change copies the generated site; from then on the world builds the design instead (walls are
always worked out from the squares, as for a generated site). Like the other edits, designs are saved, logged,
shown live (to the players' window too) and leave the world hash alone; `reset_site_design` brings back the
generated site.

A design must keep the rules play mode relies on, the same ones every generated site is tested against:

- one way in (`exit`), on the top level, on the square under the entrance (`entry` in the header);
- a way `down` on every level but the deepest, and a way `up` on the level below, on the same square;
- every square of floor reachable from the way onto its level: through doors between rooms (natural levels:
  caves, mines, lava tubes open into each other), around props that block movement;
- props on floor, inside the grid, none on another.

A plan that breaks one is refused with the reasons (`not saved: level 2: 12 squares of the tomb at 30,4 can't be
reached`). No boss chamber, or one not on the deepest level, is only noted.

**The plan** (`get_site_design` gives it; `set_site_design` takes it, or any part of it):

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
  it; `set_site_design` with `fit_shapes` does the same.
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
- Only what the text gives changes: a level block with only `items:` replaces only that level's items.
- The header's `<n> levels` adds levels below the deepest (new ones start as rock: give their grids) or fills
  in the deepest ones. A missing `up` under a level's `down` and a missing `exit` at the entry are filled in.

### `get_site_design`
`id`. Returns a summary (`designed`, kind, theme, grid size, entry, levels, `problems`) and the plan as text. A
site's saved design made for a site that stood elsewhere (its way in is not under the design's grid: edits kept for
another sketch or generator) is set aside: the generated site stands, and the reply says so (`set_aside`).

### `set_site_design`
| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | The site. |
| `text` | string | required | The plan, or the parts to change. |
| `furnish` | boolean | optional | Fill rooms with no props from their kind's kit (a boss chamber gets its dais, pillars and hoard). |
| `fit_shapes` | boolean | optional | Fit the squares of rooms given a shape to it, as the designer does (squares outside it go to rock, a building's to the room beside it); one that can't be fitted without breaking a rule stays as written and is drawn square. The reply's `fit_shapes` lists those, with why, and any items fitting took away (on a square left empty, in the rock underground, in a doorway moved, or across two rooms). |
| `dry_run` | boolean | optional | Check it and return what would be saved, without saving. |

```json
{ "name": "set_site_design", "arguments": { "id": "u:168:0", "text": "site · 3 levels\nlevel 2\nitems: up 12,9; down 30,8\nlevel 3: Level 3 · the vault\nrooms: a=landing; b=boss chamber\ngrid:\n…\ndoors: auto", "furnish": true } }
```
> *"Dig a third level under the Fallen Shrine's crypt: a landing under a new stair from level 2, a flooded
> vault beyond it as the boss chamber, furnished."*

### `reset_site_design`
`id`: the site (or a building's interior) as generated again.

### `list_designs`
The underground sites (`u:`) and buildings (`b:`) that have a design made by hand, in one `{items, found}` list (`limit` 1 to
1000, default 100): `{id, name, what: "site" or "building", kind, levels, x_ft, y_ft}` (`x_ft`/`y_ft` only while the site or building stands; one that is gone has `gone: true` instead; `world_overview`'s `designs_set_aside` also lists building designs that no longer fit, and a gone site only here). `within` (a box, a polygon or a circle
`{at, radius_ft}`) keeps those whose place is inside. `get_feature` of one of them says `designed: true`; `reset_site_design`
drops a design.

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
building is made a ruin) the design is **set aside**: the generated interior stands, `get_site_design` says so
(`set_aside`), `get_feature` gives `interior_designed`, and `world_overview` lists it in `designs_set_aside`;
put the footprint back and it applies again. When only the storeys change (`update_building` with `floors`, on
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
  a plan can't add or take away floors (`update_building` does).
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
  sprites as `s:<asset id>` (as `list_sprites` gives them; underground sites take them too). A sprite keeps its
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
{ "name": "set_site_design", "arguments": { "id": "b:23:70", "text": "level 0\ndoors: 4,2 e; 2,4 s front; 1,4 s back" } }
```
> *"Give the house by the well a back door out of its main room."*

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
- Villages have no wards (they grow along their roads). Draw roads before editing towns: a road drawn into a town
  lays it out anew (from the sketch) and sets its ward edits aside.

### `get_town_plan`
A town's plan: `patches` (`patch`, `at`, `corners`, `ward`, `ward_generated` when set by hand, `in_town`,
`district` with its id, `neighbours`, `lots`, `merged_with`, `reroll`, `lots_like` for a patch sharing another's
lots, `close` for a temple's with a close as built, `close_generated` when set by hand, `close_refused`: why one
can't be set there, or why the one set isn't built, `park` for a park's patch: `formal` or `common` as laid out,
`park_by_hand` when set by hand, `park_refused`: why the patch can't be made a park, `ward_planned` for a park's
patch built on as a common ward's, too small once its streets were cut), `corners` (`corner`, `at`,
`planned` when moved, `pinned`, `gate`, `wall`, `junction` where a square can open, `square_refused` where streets
meet but none can (a close's corner), `square` and `square_ft` (how far out from the corner its chords are cut,
on average) for the one there, `square_generated` when set by hand: a kind or `none`), `max_move_ft`, `walls`
(`built`, `generated`), `edited`, `set_aside`.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | The town or city (its feature id) |
| `within` | `[x0_ft, y0_ft, x1_ft, y1_ft]` | | Only the patches whose middle is inside, and their corners |

```json
{ "name": "get_town_plan", "arguments": { "id": "city:bab44567", "within": [1288000, 2229000, 1289500, 2230500] } }
```

### `edit_town`
Change a town's layout. Every field is optional; they apply in this order: `reset`, `moves`, `equalize`,
`relax`, `patches`, `squares`, `walls`. The reply: `corners` (`asked`, `moved` all the way, `part_way`, `stayed`,
and the ones that fell `short`), `buildings` (`before`, `after`, `added`, `taken_away`), `walls` (`built`, `towers`,
`gates`), `squares` (the kinds of its open spaces, quays aside), `parks` (how each is laid out), `functions_lost`, `unmerged` (merges dropped because
a ward no longer allows them), `rects` (where the map is drawn again). A square set by hand where the wards no
longer leave three streets goes with the change that took them. `added` and `taken_away` count buildings by id
and footprint (a patch laid out again keeps reusing its new buildings' ids). A square set or taken away changes only
the lots at its corner: they are trimmed back to it, or come back whole.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | The town or city |
| `moves` | `[{corner, to \| by \| as_generated}]` | | `to: [x_ft, y_ft]`, or `by: [dx_ft, dy_ft]` from where it stands now; `as_generated` puts it back |
| `equalize` | `[patch]` | | Pull each patch toward a regular polygon (its corners move) |
| `relax` | `{at, radius_ft, amount}` | | Each corner within `radius_ft` of `at` toward the middle of its neighbours, by `amount` (0–1, default 0.5) at the centre, less toward the edge |
| `patches` | `[{patch, ward, lots, merge_with, reroll, close, park, as_generated}]` | | `ward`: plaza, castle, temple, merchant, craft, noble, common, slum, docks, military, farm, park, empty; `lots`: small, medium, large, huge; `merge_with`: a neighbouring patch or `"none"`; `reroll: true` rolls it again; `close`: a close round a temple's patch's temple (`true`) or none (`false`); `park`: `formal` or `common`, a park's patch only; `"auto"` puts a field back |
| `squares` | `[{corner, kind}]` | | `kind`: crossroads, well, fountain, green, `"none"` (no square there) or `"auto"` (as generated); a square opens only at a `junction` |
| `walls` | `true` \| `false` \| `"auto"` | | Walls up, down, or as generated |
| `reset` | `"all"` \| `"corners"` \| `"patches"` \| `"squares"` | | Back to as generated |
| `dry_run` | boolean | `false` | Report without changing anything |

```json
{ "name": "edit_town", "arguments": { "id": "city:bab44567", "moves": [{ "corner": 41, "by": [40, 25] }], "patches": [{ "patch": 2, "ward": "castle" }] } }
```
> *"Put a castle on the ward east of Agentholm's market, and take its walls down."* · *"Make the docks' lots
> bigger and round off the square by the north gate."*

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

## Shaping the ground

A battlemap's ground can be shaped by hand, as the Ground tab shapes it: brushed up or down, smoothed, put back,
or levelled as plateaus and pits. Two edits fields hold it:

- **`ground`**: brushed blocks, by `g:XXXXX:YYYYY` (block indices, zero-padded, x first; block `(bx, by)` holds
  squares `16·bx … 16·bx + 15` east and `16·by …` south). Each is one string: 256 height changes in tenths of a
  foot, row by row, written as zigzag-delta varints (a 0 delta is followed by a byte counting the further zeros)
  in base64url without padding. A block that comes out all zero is removed. Use `shape_ground` rather than
  writing them.
- **`plateaus`**: by `h:<id>`: `pts` (the outline, 3 to 48 points, rounded to 0.5 ft, not crossing itself,
  spanning at most 1,280 ft), `level` (ft on the world's datum: above the sea plus the sea's level), `edge` (0 to
  60 ft; absent: 0, a sheer cliff at the outline).

How it applies:

- Plateaus first, largest first (a terrace or pit inside a plateau goes on top of it): inside the outline the
  ground is at the level; outside it eases back to the ground over `edge` ft (a sheer one over a square). Then the
  brushed change, at most 150 ft from the ground as generated either way. Ground that was dry never sinks below
  the water beside it; raising ground under water makes dry land.
- Everything on the ground follows it: buildings stand on it (on a plateau, at its level; a building over a sheer
  edge takes the mean, so draw round it), with their interiors and cellars; streets, roads, walls, towers and camp
  yards. Plateaus are flat: no generated knolls on them. Steep shaped ground draws as rock, with its cliffs.
- In cities and metropolises a street square that shaping makes climb at least 0.15 along the street, and at
  least 0.03 more than it did (but less than 1: steeper is a cliff), is drawn as steps, its treads across the
  street. A generated flight's square goes back to paving where shaping leaves it gentler than 0.12 and 0.03
  gentler than it was, or puts it on a plateau.
- Battlemaps and the three finest zooms draw it. The world's own heights (`describe_location`, routes, where
  settlements stand) don't change, nor does `get_feature`'s `stairs` (the flights as generated); `get_battlemap`
  has the steps as drawn.
- Limits: a stroke reaches at most 48 blocks; a world holds 1,500 blocks and 256 plateaus.

### `shape_ground`
A round brush, or a stroke along `path` (not both: `at` and `path` together are refused). `raise` and `lower` move each square by up to `height_ft` (full out to `hardness` of the radius,
fading to the rim), `smooth` eases each square toward the mean of its neighbours as drawn by `strength`,
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
{ "name": "shape_ground", "arguments": { "mode": "raise", "at": "b:0:2", "radius_ft": 25, "height_ft": 8 } }
```

### `set_plateau`
A plateau or pit, or a change to one. The outline is `poly` (also `outline`), or `around` a building (`b:`), a keep, a wall
tower (`t:`) or a site (its buildings, towers and walls), its footprint's convex outline grown by `margin_ft`;
outlines are simplified to at most 48 points. The level is `level_ft` above the sea, or `rise_ft` above the
ground (as shaped, this plateau left out) at the outline's middle. The reply: `id`, `changed`, `level_ft`,
`ground_ft` (the ground at the middle), `edge_ft`, `points`, `area_sq_ft`, `bbox`. Without `id`, an outline that matches an existing plateau's (the same number of corners, each within 1 ft) is that one: `existing: true`, nothing added; a different `level_ft`, `rise_ft` or `edge_ft` is refused (give its id to change it).

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | | A plateau to change (`h:…`): only the fields given change |
| `poly` | `[[x_ft, y_ft], …]` | | The outline (world ft, 3 or more corners; also called `outline`) |
| `around` | string | | Or a building, tower or site to raise it round |
| `margin_ft` | number | 10 | Round the footprint, 0–200 |
| `level_ft` | number | | Its level above the sea, within 20,000 ft |
| `rise_ft` | number | 10 | Or its level above the ground at the middle, within 150 ft (below zero: a pit); with `id`, only when given |
| `edge_ft` | number | 0 | The slope round it, 0 (sheer) to 60 |

```json
{ "name": "set_plateau", "arguments": { "around": "c:3", "margin_ft": 15, "rise_ft": 25, "edge_ft": 0 } }
```

### `list_plateaus`
The ground shaped by hand, world-wide, in one `{items, found}` list (`limit` 1 to 1000, default 100): plateaus and pits
`{id: "h:...", what: "plateau", level_ft, edge_ft, x_ft, y_ft, bbox, poly}` (`level_ft` above the sea, `poly` the outline), then
brushed blocks `{id: "g:...", what: "ground", x_ft, y_ft, size_ft: 80, squares_shaped, raised_ft, lowered_ft}` (`x_ft`/`y_ft` the
block's north-west corner). `within` (a box, a polygon or a circle `{at, radius_ft}`) keeps those with their middle or any outline point (a ground block: its middle or a corner) inside it. `delete_plateau` takes plateaus away; `ground_heights` reads the heights.

### `delete_plateau`
Take plateaus or pits away by `id` or `ids` (all or none); the ground goes back as it was (brushed changes stay).

### `ground_heights`
Every square of the box reaching `radius_ft` (default 25, at most 200) east, west, north and south of a place:
`origin` (the first square's middle),
`columns`, `rows`, `heights_generated` and `heights_shaped` (rows north to south, ft above the sea), their
`generated` and `shaped` ranges, `squares_shaped`, `wet` (`[column, row]` of squares under water), and the
`plateaus` (id, level, edge, middle, bbox) and `brushed_blocks` there. The battlemap draws a little relief on
top (knolls, hollows) and levels road beds under roads.

```json
{ "name": "ground_heights", "arguments": { "x_ft": 1288677, "y_ft": 2229693, "radius_ft": 60 } }
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

## What agents can't do

Asked for any of these, say so rather than looking for a tool:
- **Reshape the world.** The sketch (coastlines, mountains, rivers and roads drawn in the app's Sketch mode) is the
  user's to draw; no tool draws, changes or erases a stroke, or generates the world again. Sea and land stay as they
  are: no sea, lake or river can be added, moved or removed, and `create_feature` refuses water.
- **Draw political borders.** There are no realms, provinces or border lines to read or make.
- **Make or remove settlements.** Cities, towns and villages are generated; an agent can rename, annotate and hide
  them, lay a town out anew (`edit_town`), add and take away buildings, but not found a new city or delete one.
- **Move a feature.** Names, notes, tags and hiding change; a site's, a river's or a settlement's place does not
  (delete and create a site again to put it elsewhere; a building: `update_building`).
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
| `no such feature: …` | A bad id: check it with `search_features`. |
| `that place is under water: pick dry land` / `off the map` | `create_feature` refused the position. |
| `… is not a created site` | `delete_feature` only removes `c:` sites. Use `hide_feature` for generated ones. |
| `… was deleted with c:<n> (restore_feature brings it back)` / `… is already deleted` | A deleted site's ids are retired; `restore_feature` undoes the delete. |
| `under a ruin: …` / `under an entrance: …` / `'under' is for ruins and entrances` | Only ruins and entrances choose what lies beneath them. |
| `themes for a …: …` / `size must be …` / `levels: 1 to 6` / `… has nothing underground` | `create_feature` options that don't fit the site. |
| `not saved: …` | `set_site_design`: the plan breaks a rule play mode needs (the reasons follow). |
| `only underground sites (u:<layout>:<k>) can be designed` / `a city's sewers can't be designed` | `get_site_design` / `set_site_design` take `u:` sites and `b:` buildings, not sewers or keeps' deep dungeons. |
| `a ruin or a yard has no inside to design` | Only roofed buildings have interiors. |
| `a building's levels are its cellars and storeys` | A building's plan can't add or take away floors: change its storeys with `update_building`; cellars with `cellars <n>`. |
| `a building has 0 to 3 levels below ground` | `cellars <n>` (or a `level -4` block) asks for too many. |
| `it needs a cellar, for the trapdoor to the sewers` (or `the stairs down to the deep dungeons`) | A building with a way to another site keeps at least one cellar. |
| `'x' at 4,7 is not in its rooms list` / `level 3 is new: give its grid` | A plan that can't be read. |
