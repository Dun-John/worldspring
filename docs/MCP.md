# Worldspring MCP: full reference

`mapd` exposes the open world to agents (any MCP client) through 34 tools. With them an agent can:
- read and search the world: features, buildings, districts, underground sites, routes, battlemaps;
- look at the map as you see it, or move your view;
- edit the world: rename anything (down to a dungeon's levels and rooms), write notes, hide features, and create
  sites that are as playable as generated ones;
- keep the DM's notebook: NPCs and plot points;
- put objects on battlemaps (and take generated ones away), with uploaded sprites;
- draw buildings and towers, with what they are, their storeys and roofs.

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
- **Places:** any tool that needs a place takes either an `id` or both `x_ft` and `y_ft`.
- **Names** in results already include renames, and search matches them. Hidden features are left out of search
  unless you ask for them.
- **Extents:** search results, `get_feature` and `features_near` give each place's `extent`: `bbox_ft` (`[x0, y0,
  x1, y1]`) with `area_sq_mi`, or a river's `length_mi`. Settlements' and sites' areas are their built-up circle.
- **Which world:** every tool takes an optional `world`, the hash `world_overview` returns. If mapd has another
  world open (the user opened a different one in the app), the call is refused and nothing happens. Pass it in
  long scripted sessions.

### Ids

| Pattern | Example | What |
|---|---|---|
| `<kind>:<hash>` | `city:bab44567`, `range:e48b3d59`, `river:…` | Named features: continents, seas, ranges, peaks, forests, rivers, lakes, settlements, ruins, caves… |
| `b:<layout>:<i>` | `b:0:57` | A building in a settlement or site |
| `d:<layout>:<q>` | `d:0:3` | A district |
| `t:<layout>:<k>` | `t:0:2` | A wall tower |
| `u:<layout>:<k>` | `u:312:0` | An underground site: dungeon, crypt, cave, mine or lava tube |
| `w:<layout>:<sx>:<sy>` | `w:0:2:1` | A sewer section |
| `k:<layout>:…` | | A keep's dungeon |
| `c:<n>` | `c:2` | A site created by you or an agent (a building drawn by hand too) |
| `l:<site>:<level>` | `l:u:312:0:1` | A building's or site's level (levels count from the bottom), for renaming |
| `r:<site>:<level>:<room>` | `r:u:312:0:1:4` | A room on a level, for renaming |
| `n:<id>` | `n:k3x9q0ab` | An NPC (the notebook) |
| `p:<id>` | `p:0c7rtw2m` | A plot point (the notebook) |
| `o:<id>` | `o:4f0x2kq9` | A battlemap object put down by hand |
| `x:<id>` | `x:9a1b7c3d` | Generated battlemap objects taken away (a clear) |
| `s:<asset>` | `s:0123…cdef` | An uploaded sprite, as an object kind |

### Feature kinds

| Group | Kinds |
|---|---|
| Settlements | `metropolis`, `city`, `town`, `village` |
| Sites | `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube`; created or drawn in the sketch only: `entrance`; created only: `building` |
| Nature | `continent`, `island`, `ocean`, `sea`, `bay`, `range`, `peak`, `pass`, `volcano`, `river`, `lake`, `waterfall` |
| Roads | drawn and named in the sketch only: `road` (its `detail` is its class: king's road, road or track) |
| Regions | `forest`, `jungle`, `taiga`, `desert`, `swamp`, `plains`, `tundra`, `glacier`, `salt_flat`; painted in the sketch only: `blight` (blighted woods), `ashlands`; drawn in the sketch only: `region` |
| Inside settlements | `district`, `building`, `tower`, `underground` (in `list_names`) |
| Inside buildings and sites | `level`, `room` (in `list_names`) |

---

## Read tools

These never change anything, and work whether or not the map app is open.

### `world_overview`
A summary of the world:
- its size;
- its land masses;
- how many features of each kind it has;
- its largest settlements, with population and position;
- how many sites have been created;
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
named businesses (inns, temples, smithies…). Each result has its `extent`.

| Parameter | Type | Default | |
|---|---|---|---|
| `query` | string | required | Part of a name. Empty, together with `kind`, lists every feature of that kind. |
| `kind` | string | none | Only this kind: `city`, `ruin`, `river`, `building`, `district`, … |
| `limit` | integer | 25 | 1–200 |
| `include_hidden` | boolean | false | Also return hidden features. |

```json
{ "name": "search_features", "arguments": { "query": "gate" } }
{ "name": "search_features", "arguments": { "query": "", "kind": "volcano" } }
{ "name": "search_features", "arguments": { "query": "Prancing", "kind": "building" } }
```
> *"Find every inn called something with 'Pony'."* · *"List all the volcanoes."* · *"Is there a town named Wolfbarrow?"*

### `get_feature`
One feature in detail. What comes back depends on the feature:

| For | Fields |
|---|---|
| Any feature | `name`, `kind`, `detail`, position, elevation, `notes`, `hidden` |
| Any place | `in`: the regions it lies in (continent, range, forest…), and `nearest_settlements` |
| Settlements | Roads to other settlements, with road miles; districts; notable buildings; ways underground |
| Sites | The layout: buildings, and ways underground with their `u:` ids |
| Buildings and underground sites | Levels and rooms |

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
| Settlement | Its districts and businesses |
| District | Its businesses |
| Building or underground site | Its levels, with rooms and hazards |
| Region | Its settlements and sites |

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |

```json
{ "name": "list_children", "arguments": { "id": "city:bab44567" } }
{ "name": "list_children", "arguments": { "id": "u:312:0" } }
```
> *"What shops are in the market district?"* · *"Walk me through the dungeon under the Sunken Bastion, level by level."*

### `list_names`
Everything that can be renamed, with ids to pass to `rename_feature`. Each entry has `id`, `kind`, `name` (as
renamed), `generated` (the name before renames), `x_ft`/`y_ft` where it has a spot, `enter: true` for buildings and
sites with levels, and `hidden: true` for hidden features.

| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | optional | Only this kind: `river`, `range`, `city`… or `district`, `building`, `tower`, `underground`, `level`, `room`. |
| `settlement` | string | optional | A settlement's or site's id: its districts, businesses, wall and gate towers and underground sites (sewer grates aside). |
| `within` | string | optional | A building's or underground site's id: its levels (`l:`) and their rooms (`r:`). |

Without `settlement` or `within` it lists every named feature: oceans, seas, rivers, ranges, settlements, sites
and created sites.

```json
{ "name": "list_names", "arguments": { "kind": "river" } }
{ "name": "list_names", "arguments": { "settlement": "city:bab44567", "kind": "district" } }
{ "name": "list_names", "arguments": { "within": "u:312:0" } }
```
> *"Give every room in the crypt under the Sunken Bastion a proper name."* · *"Which rivers have I renamed?"*

### `describe_location`
What is at a point:
- elevation and biome;
- `in`: the areas it lies in, largest first (continent or island, sea, range, forest, lake…);
- the building, district or site there;
- `nearby`: named features within a tenth of their size (2 to 10 miles), nearest first, measured to their nearest
  edge or course, with `distance_mi` and `direction`;
- the nearest settlements.

| Parameter | Type | | |
|---|---|---|---|
| `id` or `x_ft` + `y_ft` | | required | |

```json
{ "name": "describe_location", "arguments": { "x_ft": 2310194, "y_ft": 4127619 } }
```
> *"What's at 437 miles east, 781 miles south?"* · *"What's right around the Ruins of Neling?"*

### `features_near`
Everything named within a radius of a place, nearest first. Distances are to each feature's nearest edge or
course (0, direction `here`, inside an area). Each result has `distance_mi`, `direction` and `extent`.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` or `x_ft` + `y_ft` | | required | |
| `radius_mi` | number | 5 | |
| `kinds` | string array | all | Only these kinds. Districts and businesses come when `kinds` names `district` or `building`, or with no `kinds` within a mile. |
| `limit` | integer | 50 | 1–500; `found` says how many there were. |
| `include_hidden` | boolean | false | |

```json
{ "name": "features_near", "arguments": { "x_ft": 2310194, "y_ft": 4127619, "radius_mi": 15, "kinds": ["river", "lake"] } }
{ "name": "features_near", "arguments": { "id": "city:bab44567", "radius_mi": 0.5, "kinds": ["building"] } }
```
> *"Which rivers and lakes are within 15 miles of here?"* · *"What's the lake just north of the capital called?"*

### `route`
The distance between two places:
- `by_road`: road miles (plus any stretch off the road) and the settlements passed on the way;
- `straight_mi`: the distance as the crow flies, and its `direction`;
- `travel`: D&D 5e travel days at normal (24 mi/day), fast (30) and slow (18) pace.

| Parameter | Type | | |
|---|---|---|---|
| `from` | id string, or `{ "x_ft", "y_ft" }` | required | |
| `to` | id string, or `{ "x_ft", "y_ft" }` | required | |

```json
{ "name": "route", "arguments": { "from": "metropolis:6eee9055", "to": "village:6db3886e" } }
{ "name": "route", "arguments": { "from": "city:bab44567", "to": { "x_ft": 1200000, "y_ft": 2500000 } } }
```
> *"How many days does it take to ride from Agentholm to Geirgai at a fast pace?"* · *"What's the route from Gatewatch to the capital, and which towns does it pass?"*

### `list_roads`
The road network:
- `named`: the roads drawn and named in the world's sketch, each with its id, name, class, length and the
  settlements along it, in order;
- `roads`: the roads between settlements and junctions, longest first, each with its `class` (king's road, road,
  track), `length_mi`, and its ends `from` and `to` (a settlement's `id` and `name`, or a junction's
  `x_ft`/`y_ft`); a drawn road's pieces say which (`road`: its name, or `drawn: true`);
- `count`: how many roads matched, `shown`: how many are listed.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` or `x_ft` + `y_ft` | | none | Only roads within `radius_mi` of this place. |
| `radius_mi` | number | 25 | |
| `settlement` | id string | none | Only roads ending at this settlement. |
| `class` | `kings_road`, `road` or `track` | all | |
| `drawn` | boolean | false | Only roads drawn in the sketch. |
| `limit` | integer | 50 | 1–1000. |

```json
{ "name": "list_roads", "arguments": { "settlement": "city:bab44567" } }
{ "name": "list_roads", "arguments": { "drawn": true } }
{ "name": "list_roads", "arguments": { "x_ft": 2310194, "y_ft": 4127619, "radius_mi": 40, "class": "kings_road" } }
```
> *"Which roads leave Bladegarden, and where do they go?"* · *"Which towns does the Gilded Roadway pass through?"*

### `get_battlemap`
The 640-ft battlemap chunk (128 × 128 five-foot squares) that contains a place, for running a fight there. It
lists:
- the elevation range, surfaces and buildings;
- each kind of object in the chunk, with its 5e rules: cover, whether it blocks movement or sight, difficult
  terrain, hazards and their effects;
- the objects within `radius_squares` of the place, by grid square (`near_focus`), each with its centre
  (`x_ft`, `y_ft`) and either its `id` (put down by hand: `remove_objects` takes it) or its built-in `kind`
  (generated: `remove_objects` takes it by `at`);
- the clears in the chunk (`cleared`: id, centre, kind or radius; `restore_objects` takes them).

| Parameter | Type | Default | |
|---|---|---|---|
| `id` or `x_ft` + `y_ft` | | required | |
| `radius_squares` | number | 12 | 1–64 |

```json
{ "name": "get_battlemap", "arguments": { "id": "c:2", "radius_squares": 20 } }
```
> *"Set up an ambush at the Sunken Bastion: where's the cover, and are there any hazards?"* · *"What terrain is on the bridge outside Gatewatch?"*

---

## View tools

These need the map app open and connected. If it isn't, the tool says so.

### `render_view`
A 1,536 × 1,024 PNG screenshot of the map, as the app draws it. The app loads everything at that place first,
then puts your view back where it was. Waits up to 45 s.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` or `x_ft` + `y_ft` | | required | |
| `size_ft` | number | 3000 | The ground across the image (50–5,000,000). |

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
| `id` or `x_ft` + `y_ft` | | required | |
| `size_ft` | number | 3000 | How much ground fits on screen. |

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
the steps before it did: a site created in one step can be renamed or annotated in the next (`c:<n>` ids count
up from `world_overview`'s `created_sites`). The whole batch is saved once, logged as one entry and shown in the
app as one notice. If a step fails, nothing is changed, and the error names the step. Returns each step's result.

```json
{ "name": "batch", "arguments": { "world": "b3a8412f4fa3a3b1", "steps": [
  { "tool": "rename_feature", "arguments": { "id": "d:4:2", "name": "Tri-Spire Ward" } },
  { "tool": "annotate_feature", "arguments": { "id": "d:4:2", "text": "Three wizard towers lean over its lanes." } },
  { "tool": "create_feature", "arguments": { "kind": "ruin", "under": "crypt", "x_ft": 2325000, "y_ft": 4127000 } },
  { "tool": "rename_feature", "arguments": { "id": "c:0", "name": "The Evening Nip" } }
] } }
```
> *"Name every district in the capital and give each a line of lore, all in one go."*

### `rename_feature`
Renames anything with a name: a feature, building, district, tower, underground site, or a site's level (`l:`) or
room (`r:`). Use `list_names` for the ids. The app shows the new names everywhere, including the level list and
the room labels inside.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | required | An empty string restores the generated name. |

```json
{ "name": "rename_feature", "arguments": { "id": "city:bab44567", "name": "Gatewatch" } }
{ "name": "rename_feature", "arguments": { "id": "b:0:57", "name": "The Gilded Griffon" } }
{ "name": "rename_feature", "arguments": { "id": "r:u:312:0:1:4", "name": "Hall of the Drowned King" } }
{ "name": "rename_feature", "arguments": { "id": "city:bab44567", "name": "" } }
```
> *"Rename the capital to Agentholm."* · *"Call the biggest tavern in Bulol 'The Drunken Kraken'."* · *"Put Fyckmere's old name back."*

### `annotate_feature`
Sets a feature's notes: lore, adventure hooks, secrets for the DM. The app shows them in the info panel.

| Parameter | Type | Default | |
|---|---|---|---|
| `id` | string | required | |
| `text` | string | required | |
| `tags` | string[] | none | Added to the existing tags. |
| `append` | boolean | false | Add to the existing notes instead of replacing them. |

```json
{ "name": "annotate_feature", "arguments": { "id": "c:2", "text": "The night watch vanishes one by one; the trail leads here.", "tags": ["hook"] } }
{ "name": "annotate_feature", "arguments": { "id": "c:2", "text": "Secret: the baron funds the cult.", "tags": ["secret"], "append": true } }
```
> *"Write a short history for every ruin in the Emberwood."* · *"Add a rumour to the Gilded Griffon that the innkeeper is a retired assassin."*

### `create_feature`
Adds a new site, built by the same generators as the world's own: map symbol, buildings, battlemap, interiors,
and a full multi-level site underground (a ruin's dungeon, crypt or catacombs; a cave, mine or lava tube; or an
`entrance`, a bare way down with nothing built above it). The result includes the new `c:<n>` id and its
underground ids.

Creating the same site again adds nothing: if a live site of the same `kind` (and the same `under`) stands within
300 ft of the place asked for, or of the spot the new one would take, the result is that site, with
`"existing": true`. A script that creates sites can be run again safely.

| Parameter | Type | | |
|---|---|---|---|
| `kind` | string | required | `ruin`, `tower`, `camp`, `waystation`, `cave`, `mine`, `lava_tube`, `entrance` |
| `id` or `x_ft` + `y_ft` | | required | Where to put it. With an `id`, the site goes at that feature's position. |
| `name` | string | optional | If left out, it is named in the local culture's style. |
| `under` | string | optional | What lies beneath: for a ruin `dungeon`, `crypt` or `catacombs`; for an entrance any of those or `cave`, `mine`, `lava_tube` (default `dungeon`). |
| `size` | string | optional | The site underground: `small`, `medium`, `large`, `huge`. |
| `levels` | integer | optional | Levels underground, 1 to 6. |
| `theme` | string | optional | The site's theme, one of its kind's (below). |

A `camp` is tents round a campfire in a levelled clearing (bedrolls, firewood, a cart with its stores), anywhere;
a `waystation` is an inn with stables set back from the nearest road. A camp is nudged up to 70 ft so its clearing
lies within one battlemap chunk.

Anything left out of `size`, `levels` and `theme` is chosen as for the world's own sites. Towers, camps and inns
have nothing underground, so they take none of them. Themes by kind of site:

| Kind | Themes (the first is the usual one) |
|---|---|
| dungeon | `dungeon`, `prison`, `temple`, `wizard_lair`, `bandit_hideout`, `dwarven_hall`, `goblin_warren`, `flooded_vault` |
| crypt | `crypt`, `tomb`, `ossuary` |
| catacombs | `catacombs` |
| cave | `cave`, `fungal`, `crystal`, `ice`, `flooded`, `beast_den` |
| mine | `mine` |
| lava_tube | `lava_tube` |

A theme sets the rooms (a prison's cell blocks and warden's office, a dwarven hall's forges and great hall), how many
there are, corridor width and the floor. Every site, whatever its theme, size and depth, ends in one boss chamber on
its deepest level, and every square is reachable.

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

### `update_feature`
Changes a name and the notes in one call. Unlike `annotate_feature`, `tags` here replace the existing tags.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | |
| `name` | string | optional | |
| `notes` | string | optional | Replaces the notes text. |
| `tags` | string[] | optional | Replaces the tags. |

```json
{ "name": "update_feature", "arguments": { "id": "c:2", "name": "Bastion of the Drowned", "notes": "Flooded at high tide.", "tags": ["dungeon", "undead"] } }
```

### `hide_feature`
Hides a feature from the map's labels and from search, or shows it again. It still exists, and
`search_features` with `include_hidden` finds it.

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
Removes a created site (`c:<n>`), a building drawn by hand too. Generated features can't be deleted, only hidden. The id is retired and never
reused.

| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | Must be a `c:` id. |

```json
{ "name": "delete_feature", "arguments": { "id": "c:0" } }
```
> *"Delete the ruin we made earlier."*

## Notebook tools: NPCs and plot points

The DM's notebook holds NPCs (`n:<id>`) and plot points (`p:<id>`). They are written by the user (the app's
**Notebook**) or by agents, never generated. Edits to them are saved, logged and shown live like the other edit
tools. Ids are random (`n:k3x9q0ab`), so the app and an agent can create at the same time.

- An NPC can be **placed** at any place id. Placed in a building or site (`b:`, `t:`, `u:`, `k:`, `w:`), they
  show inside it on the DM's map as a marker, on their `level` (default: the way in), at their point if given,
  else in that level's biggest room. The DM can drop them into play as a token.
- A plot point is tied to places (`anchors`) and NPCs (`npcs`).
- `get_feature` and `describe_location` include the NPCs and plot points there (`npcs`, `plots`). For a
  settlement or site that includes those in its buildings, districts, towers and ways underground.
- `focus_view` and `route` take an NPC's id (where they are) or a plot's (its first place).
- The players' window never gets NPCs, plots or notes.

### `list_npcs`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches name, appearance, mannerisms, goals, notes, attitude. |
| `location` | string | optional | A place id; a settlement's id also finds NPCs in its buildings and districts. |
| `tag` | string | optional | |
| `stance` | string | optional | `hostile`, `unfriendly`, `neutral`, `friendly` or `allied`. |

Returns `{count, npcs: [{id, name, stance, status, location: {id, name, level?, x_ft?, y_ft?}, tags}]}`.

### `get_npc`
An NPC in full: `name`, `appearance`, `mannerisms`, `attitude: {stance, text}`, `goals`, `notes`, `tags`,
`portrait` (an asset id), `status`, `location`, and the `plots` they are in.

### `create_npc`
| Parameter | Type | | |
|---|---|---|---|
| `name` | string | required | |
| `appearance`, `mannerisms`, `goals`, `notes` | string | optional | `notes` are the DM's (secrets). |
| `stance` | string | optional | Attitude toward the players (default `neutral`). |
| `attitude` | string | optional | Why they feel that way, what would change it. |
| `tags` | string[] | optional | |
| `status` | string | optional | Free text: alive, dead, missing, imprisoned… |
| `portrait` | string | optional | An asset id (`PUT /assets/<id>`, or a picture uploaded in the app). |
| `location` | string | optional | A place id. |
| `level` | integer | optional | Inside a building or site: 0 is the lowest level. |
| `x_ft`, `y_ft` | number | optional | A point to stand at. Without `location`, the building, district or settlement there (else the nearest named place) is used. |

```json
{ "name": "create_npc", "arguments": { "name": "Old Brannoc", "appearance": "One eye, a scarred chin", "stance": "unfriendly", "attitude": "Distrusts adventurers since the fire", "goals": "Sell the inn and retire", "location": "b:0:620", "tags": ["innkeeper"] } }
```
> *"Give the Jolly Lantern an innkeeper with a secret, and put him behind the bar."*

### `update_npc`
`id` plus any of `create_npc`'s fields except the place; only the fields given change.

### `place_npc`
`id` plus `location` (and `level`) or `x_ft`/`y_ft`. An empty `location` takes them off the map.

### `delete_npc`
`id`. They are also taken out of every plot point.

### `list_plots`
| Parameter | Type | | |
|---|---|---|---|
| `query` | string | optional | Matches title and text. |
| `status` | string | optional | `idea`, `active` or `resolved`. |
| `anchor` | string | optional | A place id. |
| `npc` | string | optional | An NPC id. |
| `tag` | string | optional | |

### `get_plot`
A plot point in full: `title`, `text`, `status`, `anchors` and `npcs` (each with its name), `tags`.

### `create_plot`
| Parameter | Type | | |
|---|---|---|---|
| `title` | string | required | |
| `text` | string | optional | What is going on, hooks, how it may play out. |
| `status` | string | optional | `idea` (default), `active` or `resolved`. |
| `anchors` | string[] | optional | Place ids (not NPCs). |
| `npcs` | string[] | optional | NPC ids; they must exist. |
| `tags` | string[] | optional | |

```json
{ "name": "create_plot", "arguments": { "title": "Smugglers in the cellar", "text": "Barrels move at night...", "status": "active", "anchors": ["b:0:620"], "npcs": ["n:k3x9q0ab"] } }
```
> *"Make a plot point for the smuggling ring, tie it to the inn and the harbour district, and involve Brannoc."*

### `update_plot`
`id` plus any of `create_plot`'s fields; lists given replace the old ones.

### `delete_plot`
`id`.

---

## Battlemap objects: scatter and sprites

Battlemaps are generated with their trees, rocks, props and hazards. On top of that, objects can be **put down**
by hand (`o:<id>` entries) and generated ones **taken away** (clears, `x:<id>` entries), by the user (the app's
**Scatter** menu) or by agents. Pictures can be **uploaded** as new kinds of object, with their own tactical
rules (`s:<asset>` kinds). Objects block, cover and hinder in play mode by their kind's rules, and
`get_battlemap` lists them. Like the other edits, they are saved, logged, shown live, and leave the world hash
alone; only the battlemap chunk holding each object (by its centre) is made again.

- A **built-in kind** is named by its id or name (`12` or `"boulder"`); `list_sprites` lists them all with their
  rules.
- An **uploaded sprite** is named `s:<asset>` (or by its name). Its picture is in the asset store
  (`/assets/<id>`; the id is the first 16 bytes of the picture's SHA-256 in hex, as the app makes it). A top-down
  picture with a transparent background works best, at about 64 px a square.

### `list_sprites`
No parameters. Returns `{built_in: [{kind, name, rules}], uploaded: [{kind, name, picture, rules, placed}]}`;
`rules` are cover, blocks_movement, blocks_sight, difficult, height_ft, size_squares and any hazard.

### `upload_sprite`
| Parameter | Type | | |
|---|---|---|---|
| `data_base64` | string | one of these | The picture (png, jpeg, webp, gif or svg), base64 (a data URL works too). |
| `path` | string | | A picture file on the machine mapd runs on. |
| `asset` | string | | A picture already uploaded: change its name or rules. |
| `name` | string | optional | |
| `size` | number | optional | Squares across at scale 1 (default 1; 0.2–40). |
| `cover` | integer | optional | 0 none (default), 1 half, 2 three-quarters, 3 full. |
| `blocks_move`, `blocks_sight`, `difficult` | boolean | optional | Default false. |
| `height_ft` | number | optional | Default 3. Objects 5 ft and taller draw above tokens' shadows. |

Returns `{kind: "s:<asset>", asset}`.

```json
{ "name": "upload_sprite", "arguments": { "path": "C:/art/altar.png", "name": "Blood altar", "size": 2, "cover": 2, "blocks_move": true, "height_ft": 4 } }
```

### `place_objects`
| Parameter | Type | | |
|---|---|---|---|
| `objects` | array | required | 1–500 of `{kind, x_ft, y_ft, rot_deg?, scale?, variant?}`. |

`kind` is a built-in kind (id or name) or an uploaded sprite. `rot_deg` turns it clockwise (uploaded sprites,
and built-in props drawn turned: logs, walls, carts, stalls, tents…). `scale` is 0.2–5 (default 1). `variant`
(0–7) picks a built-in kind's look (default by chance). Positions must be on the map. Returns
`{tool, ids}`.

```json
{ "name": "place_objects", "arguments": { "objects": [ { "kind": "boulder", "x_ft": 412500, "y_ft": 380120 }, { "kind": "s:0123456789abcdef0123456789abcdef", "x_ft": 412520, "y_ft": 380120, "rot_deg": 90 } ] } }
```
> *"Put a ring of standing stones (boulders) round the hilltop east of Bulol, with my altar sprite in the middle."*

### `remove_objects`
| Parameter | Type | | |
|---|---|---|---|
| `ids` | string[] | optional | Objects put down by hand (`o:…`). |
| `at` | array | optional | Generated objects: `{kind, x_ft, y_ft}` each, as `get_battlemap` lists them (matched within a quarter square). |
| `area` | object | optional | `{x_ft, y_ft, radius_ft, kinds?}`: everything in the circle, placed and generated (only built-in `kinds`, if given). Radius up to 640 ft. |

Returns `{tool, removed: [o:…], clears: [x:…]}`.

```json
{ "name": "remove_objects", "arguments": { "area": { "x_ft": 412500, "y_ft": 380120, "radius_ft": 30, "kinds": ["deciduous tree", "conifer"] } } }
```
> *"Clear the trees from a 60-ft circle in the woods north of the ruin so there's room for a fight."*

### `restore_objects`
`ids` (clears, `x:…`) and/or `area` (`{x_ft, y_ft, radius_ft}`: every clear touching the circle). The generated
objects come back.

## Buildings drawn by hand

Buildings and towers can be **drawn**, by the user (the app's **Build** menu) or by agents: a footprint on the map
and what the building is. Each one is a created site (`c:<n>`, kind `building`) with a layout of its own, so it
gets everything a generated building has: a roof and walls on the battlemap, an interior with its rooms and
furniture on every storey (a cellar too), its name on the map and in search. Its building id is `b:<layout>:0`
(returned as `building`); use it with `get_feature`, `place_npc` and interiors, as for any building. Like the
other edits, buildings are saved, logged, shown live and leave the world hash alone.

- **Where:** on dry land, out of rivers, clear of other buildings (walls may touch), roads, streets and town
  walls. Anything else is refused with the reason (`that overlaps The Gilded Goose`, `that is on a road`).
- **Footprint:** any simple shape of 3–64 corners, at least 10 ft across, every corner within 200 ft of its
  middle. L-shapes and other concave plans are fine (their roofs are drawn as several hips).
- **What it is (`func`):** a business by its catalog key (`inn`, `tavern`, `blacksmith`, `temple`, `castle`,
  `observatory`, `library`… as `list_names` shows them) or a home: `hovel`, `house` (default), `townhouse`,
  `tenement`, `noble_estate`, `farmhouse`, `wizard_tower`. It decides the rooms inside: a taproom and guest
  rooms for an inn, a forge for a smithy, cells for a prison, a round stair and study floors for a wizard's tower
  (when the footprint is round).
- **Roof:** `hip` (pitched; L-shapes get one per wing), `battlements` (a walkable roof with a parapet: the
  interior gains a roof level) or `cone`; left out, as its kind has it (battlements on castles, barracks and
  towers). **Tint:** `terracotta`, `slate`, `thatch`, `shingle` or `moss` (left out: picked).
- **Structure:** `ruin` leaves broken walls with no roof and no way in.
- Delete one with `delete_feature`, hide its label with `hide_feature`; notes and NPCs go on its `c:` id or
  its building id.

### `create_building`
A building with the same footprint as one already drawn (every corner within 2 ft) is not drawn twice: the
result is that building, with `"existing": true`.

| Parameter | Type | | |
|---|---|---|---|
| `poly` | `[[x_ft, y_ft], …]` | one of these | The footprint's corners in order. |
| `rect` | object | | `{x_ft, y_ft, width_ft, depth_ft, angle_deg?}`: a rectangle about its middle, `width` along `angle_deg` (clockwise from east). |
| `circle` | object | | `{x_ft, y_ft, radius_ft, sides?}`: a round tower (8–32 sides, default 16). |
| `snap` | boolean | optional | Default true: corners of a `poly` or unturned `rect`, and a circle's middle, snap to the 5-ft grid. |
| `name` | string | optional | Else named for its trade in the local style ("The Drowned Lantern", "Harlan's Smithy"), or what it is ("House"). |
| `func` | string | optional | What it is (see above). |
| `floors` | integer | optional | Storeys above ground, 1–8 (default 1; a wizard's tower 4). |
| `roof` | string | optional | `hip`, `battlements`, `cone` or `auto`. |
| `tint` | string | optional | `terracotta`, `slate`, `thatch`, `shingle`, `moss` or `auto`. |
| `structure` | string | optional | `roofed` (default) or `ruin`. |

Returns the site as `get_feature` does, plus `building` (its building id).

```json
{ "name": "create_building", "arguments": { "rect": { "x_ft": 412500, "y_ft": 380120, "width_ft": 50, "depth_ft": 30 }, "func": "inn", "floors": 2, "tint": "thatch" } }
{ "name": "create_building", "arguments": { "circle": { "x_ft": 412600, "y_ft": 380200, "radius_ft": 15 }, "func": "wizard_tower", "floors": 5, "roof": "cone", "tint": "slate", "name": "The Needle" } }
{ "name": "create_building", "arguments": { "poly": [[412700, 380100], [412750, 380100], [412750, 380120], [412720, 380120], [412720, 380145], [412700, 380145]], "func": "blacksmith" } }
```
> *"Build a two-storey inn with a thatched roof at the crossroads south of Bulol, and an L-shaped smithy beside it."*

### `update_building`
`id` (the building's `c:` id) and any of the parameters of `create_building`; those left out stay. A new
footprint (`poly`, `rect` or `circle`) moves or reshapes it and is checked like a new one (it never collides with
itself).

```json
{ "name": "update_building", "arguments": { "id": "c:4", "floors": 3, "roof": "battlements" } }
```

## Underground sites designed by hand

An underground site (`u:<layout>:<k>`: a dungeon, crypt, catacombs, cave, mine or lava tube; not a city's sewers)
can be **redesigned**, by the user (the app's designer: *Design this site* inside it) or by agents, as a text
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
items: exit 2,15; down 21,16; statue 20,19; sarcophagus 8,21 1x2; trap 14,18

level 2: Level 2 · the deep
…
```

- Levels are numbered from **1 at the top**. (Their rename ids, `l:<site>:<i>`, count from the bottom.)
- `rooms:` a symbol per room (`a`–`z`, `A`–`Z`, `0`–`9`, then `#$%&+?@<>~^*!`): its kind (as the generator names
  rooms: `guard room`, `crypt of the old lords`, `boss chamber`, `cavern`…), `+5`/`+10` for a raised floor,
  and a name in quotes (it becomes the room's rename). A kind the generator doesn't know becomes a chamber named
  as written.
- `grid:` one row per line, one character per 5-ft square (`.` rock); `x` counts across from 0, `y` down from 0.
  Rows may be short (the rest is rock).
- `doors:` `x,y e` is a door between square x,y and the one east of it, `x,y s` the one south; `secret` makes it
  a secret door. `doors: auto` puts doors wherever a room would be shut off.
- `items:` `kind x,y [wxh]`: props by kind or name (`chest`, `sarcophagus`, `pillar`, `pit`, `trap`, `altar`,
  `brazier`, `well`, `cage`, `bookshelf`, `statue`, `mushroom`, `web`, `lava`… every prop underground; the
  designer's prop list shows them with their rules), and the ways `exit`, `up`, `down`.
- Only what the text gives changes: a level block with only `items:` replaces only that level's items.
- The header's `<n> levels` adds levels below the deepest (new ones start as rock: give their grids) or fills
  in the deepest ones. A missing `up` under a level's `down` and a missing `exit` at the entry are filled in.

### `get_site_design`
`id`. Returns a summary (`designed`, kind, theme, grid size, entry, levels, `problems`) and the plan as text.

### `set_site_design`
| Parameter | Type | | |
|---|---|---|---|
| `id` | string | required | The site. |
| `text` | string | required | The plan, or the parts to change. |
| `furnish` | boolean | optional | Fill rooms with no props from their kind's kit (a boss chamber gets its dais, pillars and hoard). |
| `dry_run` | boolean | optional | Check it and return what would be saved, without saving. |

```json
{ "name": "set_site_design", "arguments": { "id": "u:168:0", "text": "site · 3 levels\nlevel 2\nitems: up 12,9; down 30,8\nlevel 3: Level 3 · the vault\nrooms: a=landing; b=boss chamber\ngrid:\n…\ndoors: auto", "furnish": true } }
```
> *"Dig a third level under the Fallen Shrine's crypt: a landing under a new stair from level 2, a flooded
> vault beyond it as the boss chamber, furnished."*

### `reset_site_design`
`id`: the site as generated again.

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
| `the change was made, but could not be saved to disk: …` | The change stands in memory and is saved again every few seconds. |
| `step N (tool) failed: … Nothing was changed.` | A `batch` step failed; the batch was not applied. |
| `no such feature: …` | A bad id: check it with `search_features`. |
| `that place is under water: pick dry land` / `off the map` | `create_feature` refused the position. |
| `… is not a created site` | `delete_feature` only removes `c:` sites. Use `hide_feature` for generated ones. |
| `under a ruin: …` / `under an entrance: …` / `'under' is for ruins and entrances` | Only ruins and entrances choose what lies beneath them. |
| `themes for a …: …` / `size must be …` / `levels: 1 to 6` / `… has nothing underground` | `create_feature` options that don't fit the site. |
| `not saved: …` | `set_site_design`: the plan breaks a rule play mode needs (the reasons follow). |
| `only underground sites (u:<layout>:<k>) can be designed` / `a city's sewers can't be designed` | `get_site_design` / `set_site_design` take `u:` sites, not sewers, keeps' deep dungeons or buildings. |
| `'x' at 4,7 is not in its rooms list` / `level 3 is new: give its grid` | A plan that can't be read. |
