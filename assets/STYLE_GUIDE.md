# Sprite style guide

Rules for battlemap art: the objects scattered on outdoor maps (`battlemap::CATALOG`, drawn by `DRAW` in
`app/src/render/atlas.ts`), building furniture (`drawFurniture`) and underground props (`drawUnderProp`, both in
`app/src/render/furniture.ts`). All of it is drawn in code, so it looks like one set.

The target is the **cozy** look of hand-drawn VTT terrain (Nimblehold-like): a sunny afternoon,
warm and inviting even in a ruin, drawn with simple broad strokes and generous colour. The world can be dangerous; the
paint should not feel grim. Zoomed out, the map stays ink on parchment; this guide is only for the
battlemap zoom.

## The look in eight rules

1. **Straight down,** orthographic, seen from directly above. No perspective, no visible sides except what a
   dome or bevel suggests. **Exception:** upright things that only read from the side are drawn from a high
   angle, with screen up kept up even in rotated sites (`screenAxes(light)`): headstones, glowing fungus,
   crystals, the skull pile, urn, hanging lantern, candelabrum, iron maiden and stocks. "Higher" means nearer
   straight down, about 60 degrees (heights at half, round sections as fat ovals: `hk` 0.5, `dk` 0.87), never
   side-on.
2. **Warm light from the north-west** (upper left of the screen). Lit edges go toward cream and yellow,
   the shaded side toward warm plum and blue-brown, never toward grey or black.
3. **Hand-drawn, not painterly.** Simple broad strokes and flat areas of colour, shaded with 2-3 clear
   tones per material instead of blended gradients (a gradient is a few flat slices; Pixi's `FillGradient`
   came out flat in the atlas). Fewer, bigger shapes beat detail: a sprite is seen at 40-80 px on screen, so
   every shape and stroke must still be easy to tell apart from a distance. Busy fine texture (tall grass,
   painted leaves) turns to noise at that size.
4. **Detailed and cartoonish where it counts.** Apart from the leafy trees, things are built from layered shapes
   and bold strokes in the manner of the conifer (stacked star whorls), palm (leaflet-stroke fronds) and dead tree
   (tapering gnarled limbs, a lit edge): more detail than a blob, never a painting.
5. **Irregular, organic and a little imperfect.** Natural things have lopsided silhouettes of mixed sizes: no
   centre-plus-ring arrangement, no radial star or flower layout, no perfect circle outline. Made things are
   hand-made: no perfectly rounded corners, no ruler-straight edges (`quad`, `board`), no perfect cylinders.
6. **Matte.** Wood, logs and stone are never shiny: a lit edge and a darker side, not a hot highlight. Glints
   belong to glass, water, metal and gems.
7. **A confident ink outline** round the silhouette, slightly uneven like a pen line. Inner lines mark the main
   forms (planks, cracks, folds) in a darker tone of the local colour. Ground patches and sparse plants use a
   dark tone of their own colour instead of ink, and fade into the ground at the edge.
8. **Shadows are soft.** Atlas objects get the app's drop shadow (smaller and fainter for sparse plants:
   `BattlemapLayer` `SPARSE`); furniture and props cast their own (`castShadow`), away from the light, longer for
   taller things. Never dark enough to read as a hole.

## Settled so far

- **Size against the 5-ft square.** Life-sized: a body about 5.5 ft (1.1 squares) inside its 1x2 spot, a pail
  about 1.5 ft across, a lantern small, weapons on a rack no longer than real ones. Oversized props get sent back.
- **Read at a glance.** Each batch goes through a blind critique (below). Watch for accidental faces (two spots
  and a mark: an ore vein read as a skull, cushion tufts as eyes and a mouth), wheel- or target-like rings, and
  spidery or star-shaped arms; break them up asymmetrically or change the view.
- **Depth.** Containers look deep: a cart or ore cart shows its inside walls, its load lies in the bottom clear of
  the walls, the near wall shades it. Tubs and baths have thin walls.
- **Fire** is a heap of soft glowing ovals, deep orange out to a pale yellow core (`hearthFire`, `fireTop`),
  never triangle flames or a starburst ("like an explosion"). No logs in front of a hearth.
- **Piles are never stacked.** Rock piles and rubble are loose spills of mixed sizes (rubble: dark, angular,
  one or two big chunks), never a neat pyramid.
- **Furniture** is strictly top-down. Wall pieces stand clear of the wall's ink with an open front toward the room
  (bookcases half depth with book spines showing, shelves tiered); a bookcase may stand in the middle of a room.
  Tables are bare. Hinges and fittings small.
- **Water** has an organic, lobed edge with no grey rim or ink outline: pale shallows to a deeper middle, flat
  ripple rings, one glint. Thin ice is only its crack (the rest transparent).
- **Hazards:** those only the DM sees (traps, pressure plates) are bold red markers, easy to spot; those players
  see (a cave-in, a sinkhole) look natural.
- **Trees:** the leafy trees (deciduous, jungle, acacia, willow), bushes and thickets keep the soft overlapping
  round canopy (`canopy()`); conifers, palms and dead trees in the detailed style. Brambles sit between the tall
  grass and the trees. Some old drawings stay by choice (the ruined wall, the outdoor rubble).
- **References:** when there is a reference image (the forge, the crystal, the ore vein), match its shapes,
  proportions and colour, translated into these rules.

## Colour

Hue shift is what makes it cozy: every colour gets warmer and yellower as it lightens, and cooler and
redder-purple as it darkens. Build each material as a ramp of 3–4 tones (dark, mid, light, sunlit)
rather than one colour darkened and lightened.

### Shared tones

| Role | Hex | Use |
|---|---|---|
| Ink | `#2b2118` | Silhouette outline (warm umber; the code's current `#1d1a14` is too cold and dark) |
| Shade glaze | `#4a2f3a` | Multiply over shaded sides at 25–40% |
| Sun glaze | `#fff1cf` | Highlights on NW edges at 30–60% |
| Warm white | `#f6efe0` | Brightest "white" (cloth, bone, flowers, sparkles) |

### Materials

| Material | Dark | Mid | Light | Sunlit / accent |
|---|---|---|---|---|
| Leaves, temperate | `#2f5a2c` | `#4f8a38` | `#8fbf4a` | `#c8dd6a` |
| Leaves, autumn (accent variant) | `#8a3f24` | `#c9822f` | `#d9a441` | `#f0cf6a` |
| Conifer needles | `#1f4a3a` | `#2f6a48` | `#5f9a5a` | `#9cc27a` |
| Jungle / rainforest | `#1d5a3a` | `#2f8a48` | `#6abf4f` | `#e0d050` (fruit, sun) |
| Acacia / savanna / dry grass | `#6a7a2e` | `#98a83e` | `#c8c864` | `#e8dc8a` |
| Willow / sage | `#4a7a5a` | `#78a070` | `#b0cc90` | `#d8e8b4` |
| Bark, logs, stumps | `#4e3424` | `#7d5a3c` | `#a8825a` | `#e0b47a` (cut ends) |
| Worked wood (barrels, carts, crates) | `#5c3b22` | `#8a5a32` | `#b98552` | `#e0b47a` |
| Stone, boulders, walls | `#6e675c` | `#9a9182` | `#c4baa6` | `#e4dccb` |
| Moss and lichen (on stone, wood) | `#4f7a32` | `#6f9a3e` | `#a8c45a` | – |
| Straw, thatch, hay | `#9a7430` | `#d9b04f` | `#f0d688` | `#fff0b8` |
| Sand, desert | `#b08550` | `#d9b26e` | `#ecd29a` | `#fff0c8` |
| Cactus | `#3f6a3a` | `#5a8a4a` | `#7aa85a` | `#f2c94c` (blooms) |
| Bone | `#b8a888` | `#dccfae` | `#efe4c8` | `#f6efe0` |
| Iron | `#4a4e56` | `#6e7278` | `#9aa0a6` | `#d8dce0` |
| Brass, gold | `#8a6a2a` | `#c9a24a` | `#e8c868` | `#fff0a8` |

### Cloth and paint (tents, awnings, bedrolls, banners)

Muted dyes, as if sun-faded: cream `#ecdcb4`, ochre `#d1a24c`, madder red `#b5523a`, teal `#3f7f7a`,
plum `#7a4a6a`, cornflower `#4f72a8`, moss `#6a8a3a`. Stripes pair a dye with cream.

### Small bright accents

Use sparingly (flowers, fruit, potions, mushrooms), and they are what make a scene feel lived-in:
poppy `#e0533d`, buttercup `#f2c94c`, lavender `#9b7fd4`, rose `#e88aa0`, warm white `#f6efe0`;
mushroom caps `#c8453a` with `#f6ead6` spots, or `#d98a3a`; magic `#8a5ab0` with a `#c8a8f0` glow.

### Water, ice and snow

| | Dark | Mid | Light | Accent |
|---|---|---|---|---|
| Fresh water | `#255a70` | `#3a7f8a` | `#5aa6a0` | `#e6f4ec` (foam, glints) |
| Swamp water | `#3a4228` | `#4f5a32` | `#6a7040` | `#8a9a4a` (scum) |
| Snow | `#b9c6dc` (lavender shade) | `#dfe6ef` | `#f2f4f6` | `#fffaf0` (sunlit) |
| Ice | `#5a8fb8` | `#8ab8da` | `#a8d4ea` | `#eef8ff` |

### Heat and dark places

| | Dark | Mid | Light | Core |
|---|---|---|---|---|
| Fire, embers | `#c2410c` | `#f28a1e` | `#ffc23a` | `#fff2c0` |
| Lava | `#3a2620` (crust) | `#ff7a1a` | `#ffb238` | `#fff2c0` |
| Basalt, obsidian, ash | `#2e2a30` | `#4a4450` | `#6a6270` | `#8a7aa8` (glint) |

Fire, lava and magic may glow: a soft halo of the light tone, fading out within the frame (the frame must
stay transparent at its edge).

### Underground and indoors

Same rules, lit by torches and lamps instead of the sun: highlights lean amber (`#ffd89a`), shadows lean
violet-brown. Keep props a step lighter and warmer than the floor they sit on so they read in fog and dim
light; rugs and cloth may use the dyes above, slightly deeper.

### Against the ground

Sprites are seen on the ground shader (`app/src/render/shaders/ground.frag`): grass `#668a40`, forest
floor `#4f5930`, dry grass `#a39a5c`, sand `#d9c28c`, dirt `#856b4a`, mud `#544a36`, rock `#7f7d75`,
cobble `#948c80`, snow `#e8edf5`. An object must separate from the ground it usually stands on by value
(lighter or darker), not by hue alone: trees and bushes are darker than grass at the edge and brighter at
the sunlit top; stones are lighter than dirt and mud.

The ground is cooler and more olive than this palette. When the first new sprites land, check them on
seed 1's forests, a town and a desert; the ground may need a warmer pass to match.

## Shapes by family

- **Leafy trees, bushes, thickets:** soft overlapping round lobes (`canopy()`), lit on the NW; no trunk from above.
- **Conifers, palms, dead trees:** stacked star whorls; leaflet-stroke fronds with one dried; bare tapering limbs.
- **Brambles, tall grass, reeds:** a few bold strokes, not busy, faint shadows.
- **Rocks and boulders:** faceted chunky stones with a lit top facet and a crack; moss on the north side.
- **Props (barrels, crates, carts, stalls, wells, tents):** clean, slightly toy-like shapes with plank and hoop
  lines and a few nail dots; a little wear, never broken unless the kind says so.
- **Ground cover (grass, flowers, pebbles, bones):** loose clusters on transparent ground, not a filled patch,
  thinning at the edge.
- **Hazards (bog, quicksand, thin ice, sinkholes, vents):** soft-edged patches that fade to transparent, readable
  as danger by colour, not by a hard border.

## Checking new art

Work in small themed batches, approved item by item:
- Look at each piece alone in `?gallery=1&items=...&sq=192` (or `&kinds=...`) and on a battlemap with `shot.mjs`:
  it reads at 64 px per square and at half that, the light comes from the upper left like its neighbours, it
  separates from its usual ground, its colours sit within the ramps above.
- **Blind critique:** show a fresh pair of eyes unlabelled crops in shuffled order, told only the theme and that
  the view is top-down (a few things from a high angle); they guess each with a confidence from 1 to 5. Tell them
  the answers for the weak ones and ask for a ranked list of fixes, choose per item, and re-test changed items with
  someone new when useful.
- Review screenshots are scratch: delete them once a batch is approved.
