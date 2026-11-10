// Sprite gallery (`?gallery=1`): every piece of art the map draws, labelled, to choose what to
// redraw. `&kinds=15,16` shows only those battlemap kinds (and no furniture or props), `&sq=96` draws
// them at that many px a square (default 48), `&ground=948c80` on that colour with their shadows as the battlemap casts
// them. `&items=bed,chair` shows only those furniture and props
// (each in both orientations; no battlemap objects; `&looks=1` every look of each; `&floor=wood` on boards; `&nolabels=1` unlabelled, for blind crops), `&sq=` then sizing their squares (default 44). Battlemap objects come from the atlas (all variants); building furniture and
// underground props are drawn by InteriorLayer exactly as in the app, one per little room. `&turn=1` shows how
// things look turned by hand instead: battlemap props that may turn (`TURNABLE`, and the other candidates) at four
// turns on grass with their shadows, and furniture and props with a back at each of their four sides. Last, a
// palace's forecourt as the battlemap draws it (garden wall, gate piers, open gates, gravel walks; `&items=forecourt`
// alone, at `&sq=` px a square, default 64); town walls by culture group (`&items=townwalls` alone, default 16, `&ruins=1`
// every panel ruined); the countryside's hedgerows, dry-stone walls, fences and windmill sails
// (`&items=countryside`, or with `&kinds=62` or `63`, at `&sq=`, default 32); and a temple's pieces in each god's look (`&items=temple` alone,
// default 56). `&items=civic` shows the guild and civic pieces and the stable stall. `&items=terrain` (alone, at
// `&sq=` px a square, default 34) shows underground terrain as the floor shader paints it: a room each of shallow
// water, deep water, a chasm and a rubble slope, a pool crossed by a rope bridge and stepping stones, and a chasm
// bridged, in a dungeon's flagstones and again in a cave. `&items=dungeon4` (alone, default 48) shows the dungeon set as the DM
// sees it in play: portcullises down and raised, locks shut and open, keys in three colours, collapsed walls.
// `&items=cemetery` (alone; `&panels=`, `&sq=`) shows the cemetery props and a cemetery per culture group.
import { Application, Container, Graphics, Sprite, Text, type TextStyleOptions } from 'pixi.js';
import init, { battlemap_catalog_json, under_catalog_json } from '../gen/pkg/worldgen_wasm.js';
import type { Shape } from '../gen/battlePrep';
import type { KindInfo } from '../gen/client';
import type { Interior, InteriorItem, UnderCatalog } from '../gen/protocol';
import type { DoorState } from '../play/state';
import { BATTLE_GROUPS, ITEM_GROUPS, looks as insideLooks } from '../editor/itemGroups';
import { buildAtlas, looks, PX, UPRIGHT_FOOT, VARIANTS } from '../render/atlas';
import { drawnTurned, LONG_SHADOW, OWN_SHADOW, SPARSE, TURNABLE } from '../render/BattlemapLayer';
import { drawBastion, drawForecourtGate, drawGardenWalls, drawGate, drawGatePier, drawGravelWalk, drawWalls, drawWallTower } from '../render/walls';
import { drawBounds, drawSails, DRY_WALL, FENCE, HEDGE, SAILS } from '../render/countryside';
import { GOD_NAMES, ORIENTED } from '../render/furniture';
import { BACKED, InteriorLayer, ON_WALL } from '../render/InteriorLayer';
import { thumbSheet, type ThumbTile } from '../render/thumbs';

/** Building furniture (`interior::item_info`), with a typical size in squares. */
const FURNITURE: [string, number, number][] = [
  ['bed', 1, 2], ['bed', 2, 2], ['cot', 1, 2], ['chest', 1, 1], ['wardrobe', 1, 1], ['sideboard', 2, 1], ['shelf', 2, 1], ['bookcase', 2, 1],
  ['barrel', 1, 1], ['keg_rack', 2, 1], ['crate', 1, 1], ['crate', 2, 2], ['table', 1, 1], ['table', 2, 1], ['long_table', 3, 1], ['desk', 2, 1],
  ['display', 2, 1], ['chair', 1, 1], ['bench', 2, 1], ['pew', 3, 1], ['pew', 2, 1], ['couch', 2, 1], ['booth_table', 1, 1], ['booth_seat', 1, 1],
  ['bar', 3, 1], ['counter', 2, 1], ['hearth', 2, 1], ['oven', 1, 1], ['forge', 2, 2], ['anvil', 1, 1], ['vat', 2, 2],
  ['cauldron', 1, 1], ['workbench', 2, 1], ['alchemy_bench', 2, 1], ['weapon_rack', 1, 1], ['weapon_rack', 2, 1], ['rack', 1, 2], ['cage', 2, 2], ['cage', 1, 1],
  ['altar', 2, 1], ['statue', 1, 1], ['pillar', 1, 1], ['sarcophagus', 1, 2], ['bell', 2, 2], ['throne', 1, 1], ['rug', 2, 3], ['rug', 2, 2], ['rug', 2, 1],
  ['bath', 1, 2], ['bath', 2, 2], ['bucket', 1, 1], ['water_jar', 1, 1], ['planter', 1, 1], ['planter', 2, 1], ['washing_line', 3, 1], ['roof_dome', 4, 4], ['trapdoor', 1, 1], ['winch', 1, 1], ['millstones', 2, 2], ['spur_wheel', 2, 2], ['telescope', 2, 2], ['barricade', 2, 1],
  ['stage', 3, 2], ['spiral_stair', 1, 1], ['link_down', 1, 1],
  ['throne_dais', 4, 2], ['throne_dais', 3, 2], ['low_dais', 3, 2], ['chair_of_office', 1, 1], ['high_table', 4, 1], ['carpet_runner', 1, 5], ['carpet_runner', 2, 4],
  ['tapestry', 3, 1], ['tapestry', 2, 1], ['wall_banner', 1, 1], ['standing_banner', 1, 1], ['font', 1, 1], ['cloister_well', 2, 2],
  ['dance_floor', 5, 5], ['dance_floor', 4, 3], ['harpsichord', 1, 2], ['musicians_corner', 2, 2], ['refreshment_table', 3, 1], ['refreshment_table', 2, 1],
  ['garden_path', 1, 4], ['garden_path', 3, 2],
  ['god_statue', 1, 1], ['god_statue', 2, 1], ['candle_stand', 1, 1], ['carved_screen', 1, 1], ['carved_screen', 2, 1], ['carved_screen', 3, 1], ['screen_gate', 1, 1], ['screen_gate', 2, 1], ['side_altar', 1, 1], ['side_altar', 2, 1],
  ['yurt_hearth', 2, 2], ['yurt_hearth', 1, 1], ['yurt_pallet', 2, 1], ['painted_chest', 1, 1], ['floor_cushion', 1, 1],
  ['strongbox', 1, 1], ['counting_desk', 2, 1], ['job_board', 2, 1], ['stable_stall', 1, 2], ['guild_arms', 1, 1], ['guild_arms', 2, 1], ['council_table', 4, 2], ['council_table', 6, 2],
];

/** Named sets for `&items=`: the guild and civic batch. */
const SETS: Record<string, string[]> = { civic: ['strongbox', 'counting_desk', 'job_board', 'stable_stall', 'guild_arms', 'council_table'] };

/** Underground props (`under::item` and the ways in and out), with their sizes in squares. */
const UNDER: [string, number, number][] = [
  ['exit', 1, 1], ['up', 1, 1], ['down', 1, 1], ['tunnel', 1, 1], ['trapdoor', 1, 1], ['ladder', 1, 1], ['pipe', 1, 1],
  ['trap', 1, 1], ['cave_in', 1, 1], ['pit', 2, 2], ['well', 1, 1], ['stalagmite', 1, 1], ['rock_column', 1, 1], ['boulder', 2, 2],
  ['crystal', 1, 1], ['fungus', 1, 1], ['mushroom', 1, 1], ['web', 1, 1], ['cobweb', 1, 1], ['guano', 2, 2], ['moss', 1, 1],
  ['pool', 2, 2], ['obsidian', 1, 1], ['basalt', 1, 1], ['vent', 1, 1], ['sulfur', 1, 1], ['scorched', 1, 2], ['glass_pool', 2, 2],
  ['bones', 1, 1], ['skeleton', 1, 2], ['skulls', 1, 1], ['campfire', 1, 1], ['rubble', 1, 1], ['debris', 1, 1], ['rat_nest', 1, 1], ['nest', 1, 1], ['ice', 1, 1], ['ice_sheet', 2, 2],
  ['timber', 1, 1], ['rail', 1, 1], ['ore_cart', 1, 1], ['ore_vein', 1, 1], ['tools', 1, 1], ['lantern', 1, 1], ['winch', 1, 1],
  ['powder', 1, 1], ['bedroll', 1, 2], ['cot', 1, 2], ['urn', 1, 1], ['brazier', 1, 1], ['candles', 1, 1], ['sarcophagus', 1, 2],
  ['effigy', 1, 2], ['coffin', 1, 2], ['niche', 1, 1], ['offering', 1, 1], ['chains', 1, 1], ['sconce', 1, 1], ['banner', 1, 1],
  ['iron_maiden', 1, 1], ['stocks', 2, 1], ['cage', 2, 2], ['rack', 1, 2], ['weapon_rack', 1, 1], ['chest', 1, 1], ['hoard', 1, 1],
  ['dais', 3, 2], ['fountain', 2, 2], ['statue', 1, 1], ['pillar', 1, 1], ['sacks', 1, 1], ['bookshelf', 1, 2], ['rug', 2, 3],
  ['table', 1, 2], ['crate', 1, 1], ['barrel', 1, 1], ['altar', 2, 1], ['glyph', 2, 2], ['collapsed_wall', 2, 1],
];

const LABEL: TextStyleOptions = { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x2a2219 };
const HEAD: TextStyleOptions = { fontFamily: 'Georgia, serif', fontSize: 22, fill: 0x1d1a14, fontWeight: 'bold' };
/** Screen px per 5-ft square for furniture and props. */
const ITEM_SQ = 44;
const SLOT = 4;

/** One room per item in a grid of `cols` (each `size` squares, rock between them); an item may give the
 * side its back is against (0-3), its label and its look. Items too big for a room follow in rows of
 * their own, in rooms big enough for the biggest of them. */
function sampler(id: string, fn: string, items: [string, number, number, number?, string?, number?][], cols: number, size = SLOT): { it: Interior; spots: [string, number, number][] } {
  const fits = (w: number, h: number) => w <= size && h <= size;
  const small = items.filter(([, w, h]) => fits(w, h)).length;
  const big = Math.max(size, ...items.filter(([, w, h]) => !fits(w, h)).map(([, w, h]) => Math.max(w, h) + 1));
  const bigCols = Math.max(1, Math.floor((cols * (size + 1)) / (big + 1)));
  const rows = Math.ceil(small / cols);
  const bigRows = Math.ceil((items.length - small) / bigCols);
  const [nx, ny] = [Math.max(cols * (size + 1), bigCols * (big + 1)) + 1, rows * (size + 1) + bigRows * (big + 1) + 1];
  const cells = new Array<number>(nx * ny).fill(-1);
  const furniture: InteriorItem[] = [];
  const rooms: Interior['levels'][number]['rooms'] = [];
  const spots: [string, number, number][] = [];
  let [ks, kb] = [0, 0];
  items.forEach(([kind, w, h, back, label, variant], k) => {
    // Its room: the next in the grid, or past the grid in the big rooms' rows.
    const room = fits(w, h) ? size : big;
    const n = fits(w, h) ? ks++ : kb++;
    const per = fits(w, h) ? cols : bigCols;
    const [x0, y0] = [1 + (n % per) * (room + 1), 1 + (fits(w, h) ? 0 : rows * (size + 1)) + Math.floor(n / per) * (room + 1)];
    for (let j = 0; j < room; j++) for (let i = 0; i < room; i++) cells[(y0 + j) * nx + x0 + i] = k;
    rooms.push({ kind: ' ', squares: room * room, raise_ft: 0, center: [x0 + room / 2, y0 + room / 2] });
    // Against the room's top wall, centred (props on a wall find the rock behind them).
    const x = x0 + Math.floor((room - w) / 2);
    // With a side set: in the middle of the room, so only the side makes it face.
    const y = back === undefined ? y0 : y0 + Math.floor((room - h) / 2);
    furniture.push({ kind, name: kind, x, y, w, h, cover: 0, blocks_move: false, height_ft: 3, ...(back === undefined ? {} : { back }), ...(variant === undefined ? {} : { variant }) });
    spots.push([label ?? (w === 1 && h === 1 && kind === 'table' ? 'table (round)' : `${kind.replace(/_/g, ' ')}${w * h > 1 ? ` ${w}×${h}` : ''}`), x0, y0 + room]);
  });
  // (`&floor=wood`: rooms above ground, so their floors are boards, not flags.)
  const z = new URLSearchParams(location.search).get('floor') === 'wood' ? 0 : -1;
  const level = { z, name: '', elevation_ft: 0, cells, rooms, walls: [], doors: [], windows: [], furniture, roof: false, has_stairs: false, natural: false };
  return { it: { id, settlement: 0, building: 0, name: null, function: fn, origin: [0, 0], axis: [1, 0], across: [0, 1], nx, ny, levels: [level], entry_level: 0, stairs: [0, 0, 0, 0] }, spots };
}

/** The dungeon set (`&items=dungeon4`): pairs of rooms with a door between them as the DM sees them in play
 * (portcullis down, raised, locked; locked doors shut and unlocked), keys in three colours, and collapsed walls
 * of each size against a wall. Returns the level, the doors' states and the labels' spots. */
function dungeonSet(): { it: Interior; states: DoorState[]; spots: [string, number, number][] } {
  type Pair = [string, string, number, DoorState, boolean];
  // (Label, door kind, lock colour, its state, the rooms one above the other.)
  const pairs: Pair[] = [
    ['portcullis down', 'portcullis', 0, {}, false],
    ['portcullis raised', 'portcullis', 0, { open: true }, false],
    ['portcullis, red lock', 'portcullis', 1, {}, false],
    ['portcullis, blue lock off', 'portcullis', 5, { unlocked: true }, false],
    ['door, yellow lock', 'door', 3, {}, false],
    ['door, green lock off', 'door', 4, { unlocked: true }, false],
    ['portcullis down (N-S)', 'portcullis', 0, {}, true],
    ['portcullis raised (N-S)', 'portcullis', 0, { open: true }, true],
    ['door, purple lock (N-S)', 'door', 6, {}, true],
    ['door, white lock off (N-S)', 'door', 7, { unlocked: true }, true],
  ];
  const item = (kind: string, x: number, y: number, w: number, h: number, extra: Partial<InteriorItem> = {}): InteriorItem => ({ kind, name: kind, x, y, w, h, cover: 0, blocks_move: false, height_ft: 0, ...extra });
  // (Room size, its piece, label.)
  const singles: [number, InteriorItem, string][] = [
    [3, item('key', 1, 1, 1, 1, { variant: 1 }), 'key, red'],
    [3, item('key', 1, 1, 1, 1, { variant: 5 }), 'key, blue'],
    [3, item('key', 1, 1, 1, 1, { variant: 3 }), 'key, yellow'],
    [4, item('collapsed_wall', 1, 0, 1, 1), 'collapsed wall 1×1'],
    [4, item('collapsed_wall', 1, 0, 2, 1), 'collapsed wall 2×1'],
    [4, item('collapsed_wall', 1, 0, 2, 2), 'collapsed wall 2×2'],
    [4, item('collapsed_wall', 0, 1, 1, 2, { back: 3 }), 'collapsed wall 1×2 (west)'],
    [4, item('collapsed_wall', 1, 2, 2, 2, { back: 2 }), 'collapsed wall 2×2 (south)'],
  ];
  const [nx, ny] = [43, 18];
  const cells = new Array<number>(nx * ny).fill(-1);
  const rooms: Interior['levels'][number]['rooms'] = [];
  const walls: Interior['levels'][number]['walls'] = [];
  const doors: Interior['levels'][number]['doors'] = [];
  const furniture: InteriorItem[] = [];
  const states: DoorState[] = [];
  const spots: [string, number, number][] = [];
  const room = (x0: number, y0: number, w: number, h: number) => {
    const k = rooms.length;
    for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) cells[(y0 + j) * nx + x0 + i] = k;
    rooms.push({ kind: ' ', squares: w * h, raise_ft: 0, center: [x0 + w / 2, y0 + h / 2] });
    return k;
  };
  pairs.forEach(([label, kind, lock, st, vertical], n) => {
    // Six pairs side by side (7 squares each) in the first row, the rest under them (4 squares each).
    const [x0, y0] = vertical ? [1 + (n - 6) * 4, 5] : [1 + n * 7, 1];
    const [w, h] = [3, 3];
    const a = room(x0, y0, w, h);
    const b = vertical ? room(x0, y0 + 3, w, h) : room(x0 + 3, y0, w, h);
    if (vertical) {
      walls.push({ a: [x0, y0 + 3], b: [x0 + 1, y0 + 3], exterior: false }, { a: [x0 + 2, y0 + 3], b: [x0 + 3, y0 + 3], exterior: false });
      doors.push({ a: [x0 + 1, y0 + 3], b: [x0 + 2, y0 + 3], kind, rooms: [a, b], ...(lock ? { lock } : {}) });
    } else {
      walls.push({ a: [x0 + 3, y0], b: [x0 + 3, y0 + 1], exterior: false }, { a: [x0 + 3, y0 + 2], b: [x0 + 3, y0 + 3], exterior: false });
      doors.push({ a: [x0 + 3, y0 + 1], b: [x0 + 3, y0 + 2], kind, rooms: [a, b], ...(lock ? { lock } : {}) });
    }
    states.push(st);
    spots.push([label, x0, vertical ? y0 + 6 : y0 + 3]);
  });
  singles.forEach(([size, f, label], n) => {
    // Keys beside the upright pairs, collapsed walls in a row under them.
    const [x, y] = n < 3 ? [17 + n * 4, 6] : [1 + (n - 3) * 5, 13];
    room(x, y, size, size);
    furniture.push({ ...f, x: f.x + x, y: f.y + y });
    spots.push([label, x, y + size]);
  });
  const level = { z: -1, name: '', elevation_ft: 0, cells, rooms, walls, doors, windows: [], furniture, roof: false, has_stairs: false, natural: false };
  return { it: { id: 'u:dungeon-set', settlement: 0, building: 0, name: null, function: 'dungeon', origin: [0, 0], axis: [1, 0], across: [0, 1], nx, ny, levels: [level], entry_level: 0, stairs: [0, 0, 0, 0] }, states, spots };
}

/** Underground terrain (`&items=terrain`): a room each (7 x 6, the terrain against its top wall) of shallow
 * water, deep water, a chasm and a rubble slope; then a pool (deep in shallow) crossed by a rope bridge and
 * stepping stones, and a chasm with a rope bridge over it. `fn`: 'dungeon' (flagstones, square) or 'cave'. */
function terrainRooms(fn: 'dungeon' | 'cave'): Interior {
  const item = (kind: string, x: number, y: number, w: number, h: number): InteriorItem => ({ kind, name: kind, x, y, w, h, cover: 0, blocks_move: false, height_ft: 0 });
  const rooms: [number, number, InteriorItem[]][] = [
    [7, 6, [item('water_shallow', 1, 0, 5, 3)]],
    [7, 6, [item('water_deep', 1, 0, 5, 3)]],
    [7, 6, [item('chasm', 1, 0, 5, 3)]],
    [7, 6, [item('rubble_slope', 1, 0, 5, 3)]],
    [9, 7, [item('water_shallow', 0, 1, 9, 5), item('water_deep', 1, 2, 7, 3), item('rope_bridge', 0, 3, 9, 1), item('stepping_stone', 6, 1, 1, 1), item('stepping_stone', 6, 2, 1, 1), item('stepping_stone', 7, 4, 1, 1), item('stepping_stone', 6, 5, 1, 1)]],
    [7, 7, [item('chasm', 0, 2, 7, 3), item('rope_bridge', 3, 1, 1, 5)]],
    [7, 7, [item('chasm', 0, 2, 7, 3), item('stone_bridge', 0, 3, 7, 1)]],
    [7, 7, [item('chasm', 2, 0, 3, 7), item('stone_bridge', 3, 0, 1, 7)]],
  ];
  // Two rows: the four kinds, then the crossings.
  const rows = [rooms.slice(0, 4), rooms.slice(4)];
  const nx = Math.max(...rows.map((r) => r.reduce((a, [w]) => a + w + 1, 1)));
  const heights = rows.map((r) => Math.max(...r.map(([, h]) => h)));
  const ny = heights.reduce((a, h) => a + h + 1, 1);
  const cells = new Array<number>(nx * ny).fill(-1);
  const furniture: InteriorItem[] = [];
  const list: Interior['levels'][number]['rooms'] = [];
  let y0 = 1;
  rows.forEach((row, r) => {
    let x0 = 1;
    for (const [w, h, items] of row) {
      const k = list.length;
      for (let j = 0; j < h; j++) for (let i = 0; i < w; i++) cells[(y0 + j) * nx + x0 + i] = k;
      list.push({ kind: ' ', squares: w * h, raise_ft: 0, center: [x0 + w / 2, y0 + h / 2] });
      for (const f of items) furniture.push({ ...f, x: f.x + x0, y: f.y + y0 });
      x0 += w + 1;
    }
    y0 += heights[r] + 1;
  });
  const level = { z: -1, name: '', elevation_ft: 0, cells, rooms: list, walls: [], doors: [], windows: [], furniture, roof: false, has_stairs: false, natural: fn === 'cave' };
  return { id: `u:terrain-${fn}`, settlement: 0, building: 0, name: null, function: fn, origin: [0, 0], axis: [1, 0], across: [0, 1], nx, ny, levels: [level], entry_level: 0, stairs: [0, 0, 0, 0] };
}

export async function runGallery(target: HTMLElement) {
  // The map page is a fixed full-screen canvas; this one scrolls.
  document.documentElement.style.overflow = 'auto';
  document.body.style.cssText = 'background:#efe6d2;overflow:auto;height:auto;position:static';
  target.style.cssText = 'position:static;height:auto';
  await init();
  const catalog = JSON.parse(battlemap_catalog_json()) as KindInfo[];
  // Kinds the item pickers have no group for (they show under "Other"): give them one in itemGroups.ts.
  const under = JSON.parse(under_catalog_json()) as UnderCatalog;
  const inside = [...under.props, ...under.building.furniture, ...under.building.props].map((p) => p.kind).filter((k) => !ITEM_GROUPS[k]);
  const grouped = new Set(BATTLE_GROUPS.flatMap(([, kinds]) => kinds));
  const outside = catalog.filter((k) => !grouped.has(k.id)).map((k) => k.name);
  if (inside.length || outside.length) console.warn('[gallery] in no picker group:', [...new Set(inside), ...outside].join(', '));
  const app = new Application();
  const width = Math.max(1200, window.innerWidth - 20);
  await app.init({ width, height: 400, background: 0xefe6d2, antialias: true, resolution: window.devicePixelRatio || 1, autoDensity: true });
  InteriorLayer.renderer = app.renderer;
  target.appendChild(app.canvas);
  const root = new Container();
  let y = 16;
  const head = (s: string) => {
    const t = new Text({ text: s, style: HEAD });
    t.position.set(16, y);
    root.addChild(t);
    y += 40;
  };

  // Battlemap objects: every atlas frame, all variants, at about 48 px a square.
  const query = new URLSearchParams(location.search);
  const only = query.get('kinds')?.split(',').map(Number);
  const pick = query.get('items')?.split(',').flatMap((k) => SETS[k] ?? [k]);
  const sq = Number(query.get('sq')) || 48;
  const ground = query.has('ground') ? parseInt(query.get('ground') || '948c80', 16) : null;
  if (query.has('turn')) {
    turning(app, catalog, root, head, () => y, (v) => (y = v), width, sq);
    const bg = new Graphics().rect(0, 0, width, y).fill(0xefe6d2);
    app.stage.addChild(bg, root);
    app.renderer.resize(width, y);
    // The pickers' thumbnail sheets, as they come out of `thumbSheet` (one canvas each).
    const atlas = buildAtlas(app.renderer, catalog);
    const sheets: [string, ThumbTile[]][] = [
      ['Battlemap', catalog.map((k) => ({ key: String(k.id), node: new Sprite(atlas.frames[k.id][0]) }))],
      ['Building', InteriorLayer.thumbs(false, FURNITURE.map(([kind, w, h]) => ({ kind, w, h, key: `${kind}:${w}x${h}` })))],
      ['Underground', InteriorLayer.thumbs(true, UNDER.map(([kind, w, h]) => ({ kind, w, h, key: `${kind}:${w}x${h}` })))],
    ];
    for (const [name, tiles] of sheets) {
      const n = tiles.length;
      const sheet = thumbSheet(app.renderer, tiles, 64);
      const h = document.createElement('h2');
      h.textContent = `${name} thumbnails (${sheet.rects.size} of ${n} on one sheet)`;
      h.style.cssText = 'font:bold 22px Georgia,serif;margin:16px';
      sheet.canvas.style.cssText = 'margin:0 16px;background:#cfc4ad';
      target.append(h, sheet.canvas);
    }
    return;
  }
  if (!pick) {
    head(`Battlemap objects (atlas, ${catalog.length} kinds × ${VARIANTS} variants; market stalls 16, four to a row)`);
    const atlas = buildAtlas(app.renderer, catalog);
    const scale = sq / PX;
    // On the ground, every frame at full size (as on the battlemap), with the ground round it.
    const widest = Math.max(...catalog.filter((info) => !only || only.includes(info.id)).map((info) => atlas.frames[info.id][0].width));
    const cell = Math.max(104, Math.ceil((sq / 48) * 104), ground === null ? 0 : Math.ceil(widest * scale + 64 * scale));
    const cellW = 4 * cell + 24;
    const per = Math.max(1, Math.floor((width - 32) / cellW));
    let rowH = 0;
    catalog
      .filter((info) => !only || only.includes(info.id))
      .forEach((info, k) => {
        if (k % per === 0 && k) {
          y += rowH + 30;
          rowH = 0;
        }
        const x0 = 16 + (k % per) * cellW;
        const rows = Math.ceil(looks(info.id) / VARIANTS);
        for (let v = 0; v < looks(info.id); v++) {
          const s = new Sprite(atlas.frames[info.id][v]);
          const fit = Math.min(scale, (cell - 4) / s.texture.width);
          const [cx, cy] = [x0 + cell / 2 + (v % VARIANTS) * cell, y + cell / 2 + Math.floor(v / VARIANTS) * cell];
          if (ground !== null) {
            root.addChild(new Graphics().rect(cx - cell / 2 + 2, cy - cell / 2 + 2, cell - 4, cell - 4).fill(ground));
            if (info.height_ft >= 1.5 && !info.hazard && !OWN_SHADOW.has(info.name)) root.addChild(dropShadow(atlas, info, cx, cy, fit));
          }
          s.scale.set(fit);
          s.anchor.set(0.5);
          s.position.set(cx, cy);
          root.addChild(s);
        }
        const t = new Text({ text: `${info.id} ${info.name.replace(/_/g, ' ')} (r ${info.radius})`, style: LABEL });
        t.position.set(x0, y + rows * cell + 2);
        root.addChild(t);
        rowH = Math.max(rowH, rows * cell + 2);
      });
    y += rowH + 50;
  }

  // Building furniture and underground props, drawn as in the app.
  const SQ = pick ? Number(query.get('sq')) || ITEM_SQ : ITEM_SQ;
  // `&looks=1`: every look of each chosen piece, labelled with it.
  const allLooks = query.has('looks');
  type Spot = [string, number, number, number?, string?, number?];
  const chosen = (list: [string, number, number][], underground: boolean): Spot[] =>
    pick
      ? list
          .filter(([k]) => pick.includes(k))
          .flatMap(([k, w, h]): [string, number, number][] => (w === h ? [[k, w, h]] : [[k, w, h], [k, h, w]]))
          .flatMap(([k, w, h]): Spot[] => (allLooks ? Array.from({ length: insideLooks(k, underground) }, (_, v): Spot => [k, w, h, undefined, `${k.replace(/_/g, ' ')} ${w}×${h} · ${v}`, v]) : [[k, w, h]]))
      : list;
  // Rooms big enough for the biggest piece shown.
  const slot = Math.max(SLOT, ...(pick ? FURNITURE.filter(([k]) => pick.includes(k)).map(([, w, h]) => Math.max(w, h) + 1) : []));
  const cols = Math.max(4, Math.floor((width - 32) / ((slot + 1) * SQ)));
  for (const [title, id, fn, items] of [
    ['Building furniture (item atlas)', 'b:gallery', 'house', FURNITURE],
    ['Underground props (item atlas)', 'u:gallery', 'dungeon', UNDER],
  ] as const) {
    if (only) break;
    const list = chosen(items as unknown as [string, number, number][], id.startsWith('u'));
    if (!list.length) continue;
    head(title);
    const { it, spots } = sampler(id, fn, list, cols, slot);
    const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: null });
    const holder = new Container();
    holder.addChild(layer.container);
    holder.scale.set(SQ);
    holder.position.set(16, y);
    // The layer dims the map behind it: none here.
    layer.container.children[0].visible = false;
    root.addChild(holder);
    for (const [label, gx, gy] of query.has('nolabels') ? [] : spots) {
      const t = new Text({ text: label, style: { ...LABEL, fill: id.startsWith('u') ? 0xf3ead8 : 0x2a2219, fontSize: 12 } });
      t.position.set(16 + gx * SQ + 2, y + (gy - 0.95) * SQ);
      root.addChild(t);
    }
    y += it.ny * SQ + 40;
  }

  // A temple's pieces in each god's look, a row a god (`&items=temple` alone, at `&sq=` px a square).
  if (!only && (!pick || pick.includes('temple'))) {
    head("Temple pieces by the god's kind: statue, candle stand, carved screen and its gate, side altar");
    const tsq = pick ? Number(query.get('sq')) || 56 : ITEM_SQ;
    const pieces: [string, number, number][] = [['god_statue', 1, 1], ['god_statue', 2, 1], ['candle_stand', 1, 1], ['carved_screen', 2, 1], ['screen_gate', 1, 1], ['side_altar', 1, 1], ['side_altar', 2, 1]];
    const list: Spot[] = GOD_NAMES.flatMap((name, v) => pieces.map(([k, w, h]): Spot => [k, w, h, undefined, `${k.replace(/_/g, ' ')} ${w}×${h} · ${name}`, v]));
    const { it, spots } = sampler('b:temple', 'temple', list, pieces.length, 3);
    const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: null });
    const holder = new Container();
    holder.addChild(layer.container);
    holder.scale.set(tsq);
    holder.position.set(16, y);
    layer.container.children[0].visible = false;
    root.addChild(holder);
    for (const [label, gx, gy] of query.has('nolabels') ? [] : spots) {
      const t = new Text({ text: label, style: { ...LABEL, fontSize: 10 } });
      t.position.set(16 + gx * tsq + 2, y + (gy - 0.95) * tsq);
      root.addChild(t);
    }
    y += it.ny * tsq + 40;
  }

  // Underground terrain as the shader paints it (`&items=terrain` alone).
  if (!only && (!pick || pick.includes('terrain'))) {
    const tsq = pick ? Number(query.get('sq')) || 34 : ITEM_SQ;
    for (const [title, fn] of [
      ['Terrain in a dungeon: shallow water, deep water, chasm, rubble slope; a pool and a chasm crossed', 'dungeon'],
      ['Terrain in a cave', 'cave'],
    ] as const) {
      head(title);
      const it = terrainRooms(fn);
      const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: null });
      const holder = new Container();
      holder.addChild(layer.container);
      holder.scale.set(tsq);
      holder.position.set(16, y);
      layer.container.children[0].visible = false;
      root.addChild(holder);
      y += it.ny * tsq + 40;
    }
  }

  // The dungeon set: portcullises, locks, keys, collapsed walls (`&items=dungeon4` alone, at `&sq=` px a square).
  if (!only && (!pick || pick.includes('dungeon4'))) {
    head('Dungeon set: portcullis down and raised, locks shut and open (DM view), keys, collapsed walls');
    const dsq = pick ? Number(query.get('sq')) || 48 : ITEM_SQ;
    const { it, states, spots } = dungeonSet();
    const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: (_loc, k) => states[k] ?? {} });
    const holder = new Container();
    holder.addChild(layer.container);
    holder.scale.set(dsq);
    holder.position.set(16, y);
    layer.container.children[0].visible = false;
    root.addChild(holder);
    for (const [label, gx, gy] of query.has('nolabels') ? [] : spots) {
      const t = new Text({ text: label, style: { ...LABEL, fill: 0xf3ead8, fontSize: 11 } });
      t.position.set(16 + gx * dsq + 2, y + (gy - 0.6) * dsq);
      root.addChild(t);
    }
    y += it.ny * dsq + 40;
  }

  // A palace's forecourt as the battlemap draws it (`&items=forecourt` alone).
  if (!only && (!pick || pick.includes('forecourt') || pick.includes('garden_path'))) {
    head('Forecourt: low garden wall, gate piers, open gates (iron, timber), gravel walks (battlemap)');
    y = forecourt(root, y, width, query.has('nolabels'), Number(query.get('sq')) || PX);
  }

  // Town walls by culture group (`&items=townwalls` alone; `&ruins=1` all ruined).
  if (!only && (!pick || pick.includes('townwalls'))) {
    head('Town walls by culture group: curtain, corner and wall towers, gatehouse; a ruin; a star bastion (battlemap)');
    y = townWalls(root, y, width, query.has('nolabels'), Number(query.get('sq')) || 16, query.has('ruins'), query.get('panels'));
  }

  // Cemeteries: their props and one laid out per culture group (`&items=cemetery` alone).
  if (!only && pick?.includes('cemetery')) {
    head('Cemeteries: their props on graveyard grass, then one per culture group (battlemap)');
    y = cemeteries(app, catalog, root, y, width, query.has('nolabels'), Number(query.get('sq')) || 0, query.get('panels'));
  }

  // The countryside's bounds and sails (`&items=countryside`, or with the fruit tree or water wheel kinds).
  if (pick?.includes('countryside') || only?.includes(62) || only?.includes(63)) {
    head('Countryside: hedgerow, dry-stone wall, fence (two chunks side by side, cut at the line between), windmill sails');
    y = countryside(root, y, Number(query.get('sq')) || 32);
  }

  const bg = new Graphics().rect(0, 0, width, y).fill(0xefe6d2);
  app.stage.addChild(bg, root);
  app.renderer.resize(width, y);
}

/** Field bounds over two chunks' worth of grass (each drawn apart, its pieces cut at its edges, as the
 * battlemap draws them), then sails at two turns and two sizes; at `sq` px a square. */
function countryside(root: Container, y0: number, sq: number): number {
  const [W, H] = [14, 9];
  const shape = (kind: number, size: number, pts: number[]): Shape => ({ kind, size, pts });
  const runs: [number, number, [number, number][]][] = [
    [HEDGE, 3.5, [[1.5, 7.6], [1.5, 1.5], [20, 1.2], [20.7, 7.4]]],
    [DRY_WALL, 2.5, [[3.6, 6.2], [11.5, 4.1], [17.5, 6.4], [26.6, 5.9], [26.9, 1.4]]],
    [FENCE, 0.8, [[4, 3.4], [11, 3.0]]],
    [FENCE, 0.8, [[9.5, 8.4], [24.2, 8.1], [24.4, 3.4]]],
  ];
  // A segment cut to the chunk [0, W*PX] x [0, H*PX] (chunk px), if any of it is inside.
  const clip = (x0: number, y0: number, x1: number, y1: number): number[] | null => {
    let [t0, t1] = [0, 1];
    const [dx, dy] = [x1 - x0, y1 - y0];
    for (const [p, q] of [[-dx, x0], [dx, W * PX - x0], [-dy, y0], [dy, H * PX - y0]]) {
      if (p === 0) {
        if (q < 0) return null;
        continue;
      }
      const t = q / p;
      if (p < 0) t0 = Math.max(t0, t);
      else t1 = Math.min(t1, t);
    }
    return t0 < t1 ? [x0 + dx * t0, y0 + dy * t0, x0 + dx * t1, y0 + dy * t1] : null;
  };
  for (const c of [0, 1]) {
    const g = new Graphics();
    g.rect(0, 0, W * PX, H * PX).fill(GRASS);
    const shapes: Shape[] = [];
    for (const [kind, ft, pts] of runs) {
      for (let i = 0; i + 1 < pts.length; i++) {
        const seg = clip((pts[i][0] - c * W) * PX, pts[i][1] * PX, (pts[i + 1][0] - c * W) * PX, pts[i + 1][1] * PX);
        if (seg) shapes.push(shape(kind, (ft / 5) * PX, seg));
      }
    }
    drawBounds(g, shapes, c * W * PX, 0, W * PX);
    const panel = new Container();
    const mask = new Graphics().rect(-PX, -PX, (W + 2) * PX, (H + 2) * PX).fill(0xffffff);
    panel.addChild(g, mask);
    g.mask = mask;
    panel.scale.set(sq / PX);
    panel.position.set(16 + c * W * sq, y0);
    root.addChild(panel);
  }
  let y = y0 + H * sq + 30;
  // Sails over a plain roof, at two turns, full size and small (about the battlemap's farthest).
  const S = 12;
  let x = 16;
  [
    [0.3, sq * 0.4],
    [1.05, sq * 0.4],
    [0.3, sq],
    [1.05, sq],
  ].forEach(([turn, k]) => {
    const g = new Graphics();
    g.rect(0, 0, S * PX, S * PX).fill(GRASS);
    g.rect((S / 2 - 1.6) * PX, (S / 2 - 1.4) * PX, 3.2 * PX, 2.8 * PX).fill(0x8a7a62).stroke({ width: 2.9, color: 0x1d1a14 });
    drawSails(g, shape(SAILS, 5.4 * PX, [(S / 2) * PX, (S / 2 - 0.6) * PX, turn * PX, 0]));
    g.scale.set(k / PX);
    g.position.set(x, y);
    x += S * k + 16;
    root.addChild(g);
  });
  return y + S * sq + 30;
}

/** Ground colours (`ground.frag`): grass, and a plaza's paving (its cobbles). */
const GRASS = 0x668a40;
const PAVING = 0x948c80;

/** A forecourt's corner on the battlemap's ground, at `sq` px a square, drawn by `walls.ts` from shapes
 * as `battlePrep` gives them (px): two stretches of garden wall meeting at a corner, a gate between its
 * piers standing open, gravel walks 3×2 and 1×4; on grass and on paving, square to the map, turned and
 * at 45 degrees.
 * Returns the y below it. */
function forecourt(root: Container, y0: number, width: number, nolabels: boolean, sq: number): number {
  const [W, H] = [10, 8];
  const [pw, ph] = [W * sq, H * sq];
  const panels: [string, number, number, number][] = [
    ['iron gate, on grass', GRASS, 0, 0],
    ['iron gate, on paving, turned', PAVING, 0, -0.35],
    ['iron gate, on grass, at 45 degrees', GRASS, 0, Math.PI / 4],
    ['timber gate, on paving', PAVING, 1, 0],
    ['timber gate, on grass, turned', GRASS, 1, 0.35],
    ['timber gate, on paving, at 45 degrees', PAVING, 1, -Math.PI / 4],
  ];
  const per = Math.max(1, Math.floor((width - 8) / (pw + 24)));
  panels.forEach(([label, ground, style, turn], k) => {
    const [ox, oy] = [16 + (k % per) * (pw + 24), y0 + Math.floor(k / per) * (ph + 30)];
    const g = new Graphics();
    g.rect(0, 0, W * PX, H * PX).fill(ground);
    // A point (squares) of the panel, turned about its middle (the light stays in the north-west).
    const [c, sn] = [Math.cos(turn), Math.sin(turn)];
    const P = (x: number, y: number): [number, number] => [(W / 2 + (x - W / 2) * c - (y - H / 2) * sn) * PX, (H / 2 + (x - W / 2) * sn + (y - H / 2) * c) * PX];
    const shape = (kind: number, size: number, pts: number[]): Shape => ({ kind, size, pts });
    // The front wall along y 5.4 (the forecourt to the north of it), the side wall up from its corner;
    // the gate 3 squares across, as `town::palace::GATE_SQ`.
    const gap = 3;
    const [fy, pier] = [5.4, 0.7];
    const [a, b] = [3.9 + pier, 3.9 + pier + gap];
    const walls = [
      shape(10, 0.4 * PX, [...P(1.2, 0.5), ...P(1.2, fy)]),
      shape(10, 0.4 * PX, [...P(1.2, fy), ...P(a - 2 * pier, fy)]),
      shape(10, 0.4 * PX, [...P(b + 2 * pier, fy), ...P(9.6, fy)]),
    ];
    const piers = [a - pier, b + pier].map((x) => shape(11, pier * PX, [...P(x, fy), turn * PX, 0]));
    const gate = shape(12, style * PX, [...P(a, fy), ...P(b, fy), ...P((a + b) / 2, fy - 2)]);
    const walk = (x0: number, y0: number, x1: number, y1: number) => shape(13, 0, [...P(x0, y0), ...P(x1, y0), ...P(x1, y1), ...P(x0, y1)]);
    // As the battlemap layers them: walks on the ground, then the walls, the piers, the gate.
    drawGravelWalk(g, walk((a + b) / 2 - 0.5, 1.4, (a + b) / 2 + 0.5, 5.1));
    drawGravelWalk(g, walk(2, 0.8, 4.2, 2.6));
    drawGardenWalls(g, walls);
    for (const s of piers) drawGatePier(g, s);
    drawForecourtGate(g, gate);
    // (Clipped to its panel.)
    const panel = new Container();
    const clip = new Graphics().rect(0, 0, W * PX, H * PX).fill(0xffffff);
    panel.addChild(g, clip);
    g.mask = clip;
    panel.scale.set(sq / PX);
    panel.position.set(ox, oy);
    root.addChild(panel);
    if (nolabels) return;
    const t = new Text({ text: label, style: LABEL });
    t.position.set(ox, oy + ph + 4);
    root.addChild(t);
  });
  return y0 + Math.ceil(panels.length / per) * (ph + 30) + 30;
}

/** A town wall's style code (`town::wallstyle::WallStyle::code`): group, tower shape, material, merlons, stone,
 * outline, gatehouse. */
const wallStyle = (group: number, shape: number, material: number, merlons: number, stone: number, gatehouse: number) =>
  group | (shape << 3) | (material << 5) | (merlons << 7) | (stone << 10) | (gatehouse << 15);

/** Town walls by culture group (item 104), as `walls.ts` draws them from shapes as `battlePrep` gives them (px),
 * at `sq` px a square: per look a stretch of curtain turning at a corner tower, a wall tower along it and the
 * gatehouse (the town to the south, open country to the north); a ruined run; a star bastion. `ruins` draws
 * every panel ruined. Returns the y below it. */
function townWalls(root: Container, y0: number, width: number, nolabels: boolean, sq: number, ruins: boolean, picks: string | null): number {
  // (Tower shapes: round, square, D, octagon; materials: stone, palisade, earth; merlons: broad, square,
  // swallowtail, stepped, stakes, rounded; stones: granite, limestone, sandstone, rose brick, mud brick, timber,
  // earth; gatehouses: square tower, twin drums, arched, bent, log.)
  type Panel = { label: string; group: number; mat: number; mer: number; stone: number; house: number; along: number; corner: number; ruined?: boolean; close?: boolean };
  const panels: Panel[] = [
    { label: '1 North: granite, squat round towers, broad merlons, a square gate tower', group: 0, mat: 0, mer: 0, stone: 0, house: 0, along: 0, corner: 0 },
    { label: '2 West: limestone, D towers, round corner, square crenels, twin drums', group: 1, mat: 0, mer: 1, stone: 1, house: 1, along: 2, corner: 0 },
    { label: '3 South: sandstone, square towers, octagon corner, swallowtails, arched gate', group: 2, mat: 0, mer: 2, stone: 2, house: 2, along: 1, corner: 3 },
    { label: '4 South: rose brick, the same', group: 2, mat: 0, mer: 2, stone: 3, house: 2, along: 1, corner: 3 },
    { label: '5 Dry: mud brick, square towers set close, stepped merlons, bent entrance', group: 3, mat: 0, mer: 3, stone: 4, house: 3, along: 1, corner: 1, close: true },
    { label: '6 Steppe (and poor North/West): palisade, log blockhouses, log gate', group: 4, mat: 1, mer: 4, stone: 5, house: 4, along: 1, corner: 1 },
    { label: '7 Steppe rich: rammed earth, round towers, rounded merlons', group: 4, mat: 2, mer: 5, stone: 6, house: 1, along: 0, corner: 0 },
    { label: '8 West, ruined', group: 1, mat: 0, mer: 1, stone: 1, house: 1, along: 2, corner: 0, ruined: true },
  ];
  const [W, H, wy] = [31, 12, 5];
  const [pw, ph] = [W * sq, H * sq];
  const per = Math.max(1, Math.floor((width - 16) / (pw + 16)));
  // (`&panels=7-9` or `&panels=2,8` shows only those, numbered as the labels; three to a row, the star bastion the ninth.)
  const shown = new Set<number>();
  for (const part of (picks ?? '1-9').split(',')) {
    const [a, b] = part.split('-').map(Number);
    for (let k = a; k <= (b || a); k++) shown.add(k);
  }
  const slots = [...shown].filter((k) => k >= 1 && k <= 9).sort((a, b) => a - b);
  const BH = 20;
  const starSq = shown.size < 9 ? sq : sq * 0.8;
  const rowH = (r: number) => Math.max(...slots.slice(r * per, r * per + per).map((k) => (k === 9 ? BH * starSq : ph) + 30));
  const rowY = (r: number) => Array.from({ length: r }, (_, i) => rowH(i)).reduce((a, b) => a + b, y0);
  const shape = (kind: number, size: number, pts: number[]): Shape => ({ kind, size, pts });
  const P = (x: number, y: number) => [x * PX, y * PX];
  const place = (g: Graphics, ox: number, oy: number, w: number, h: number, label: string, sc = sq) => {
    const panel = new Container();
    const clip = new Graphics().rect(0, 0, w * PX, h * PX).fill(0xffffff);
    panel.addChild(g, clip);
    g.mask = clip;
    panel.scale.set(sc / PX);
    panel.position.set(ox, oy);
    root.addChild(panel);
    if (nolabels) return;
    const t = new Text({ text: label, style: LABEL });
    t.position.set(ox, oy + h * sc + 4);
    root.addChild(t);
  };
  const where = (n: number): [number, number] => {
    const i = slots.indexOf(n);
    return [16 + (i % per) * (pw + 16), rowY(Math.floor(i / per))];
  };
  panels.forEach((p, k) => {
    if (!shown.has(k + 1)) return;
    const [ox, oy] = where(k + 1);
    const ruined = ruins || !!p.ruined;
    const g = new Graphics();
    g.rect(0, 0, W * PX, H * PX).fill(GRASS);
    g.rect(3 * PX, wy * PX, (W - 3) * PX, (H - wy) * PX).fill(PAVING);
    const code = (sh: number) => wallStyle(p.group, sh, p.mat, p.mer, p.stone, p.house) * PX;
    const form = (1 + p.house) * PX;
    const wallFt = p.mat === 1 ? 3 : p.mat === 2 ? 11 : 8;
    // Its towers: the corner at (3, wy), along the curtain at 11 (and 7, 15 set close), the gate's two 52 ft apart.
    const [ga, gb] = [16, 16 + 52 / 5];
    const along = p.close ? [7, 11] : p.ruined || ruins ? [11] : [10];
    const wall = (x0: number, y0w: number, x1: number, y1: number) => shape(0, wallFt * (PX / 5), [...P(x0, y0w), ...P(x1, y1), (ruined ? 4 : 0) * PX, 0, code(0), 0]);
    const stops = [3, ...along, ga];
    const walls = [wall(3, H + 1, 3, wy), ...stops.slice(0, -1).map((x, i) => wall(x, wy, stops[i + 1], wy)), wall(gb, wy, W + 1, wy)];
    const rf = ruined ? 4 : 0;
    // (A tower: its centre, flags and angle, style and gate form; a square's or octagon's sides along the wall, a D facing out, north.)
    const tower = (x: number, sh: number, r: number, flags: number, f = 0) => shape(1, r * (PX / 5), [...P(x, wy), (flags | rf | (sh === 1 ? 8 : 0)) * PX, (sh === 2 ? -Math.PI / 2 : 0) * PX, code(sh), f]);
    const towers = [tower(3, p.corner, 11, 0), ...along.map((x) => tower(x, p.along, 11, 0))];
    if (p.house === 3 || p.house === 4) towers.push(tower(ga, 1, 12, 1, form), tower(gb, 1, 12, 1, form));
    else if (p.house === 1) towers.push(tower(ga, 0, 15, 1, form), tower(gb, 0, 15, 1, form));
    const gate = shape(9, ruined ? PX : 0, [...P(ga, wy), ...P(gb, wy), code(0), form, PX, PX]);
    drawWalls(g, walls, [gate]);
    drawGate(g, gate);
    for (const t of towers) drawWallTower(g, t);
    place(g, ox, oy, W, H, p.label);
  });
  const rows = Math.ceil(slots.length / per);
  const y = rowY(rows) + 10;
  // A star bastion (West, limestone) at a corner of the town (to the south-east): f1, g1, tip, g2, f2, the corner.
  if (shown.has(9)) {
    const BW = 31;
    const g = new Graphics();
    g.rect(0, 0, BW * PX, BH * PX).fill(GRASS);
    g.rect(14 * PX, 11 * PX, (BW - 14) * PX, (BH - 11) * PX).fill(PAVING);
    const code = wallStyle(1, 0, 0, 1, 1, 1) * PX;
    const ring: [number, number][] = [
      [BW + 1, 11],
      [21.2, 11],
      [21.2, 6.6],
      [4.27, 1.27],
      [9.6, 18.2],
      [14, 18.2],
      [14, BH + 1],
    ];
    const poly = [ring[1], ring[2], ring[3], ring[4], ring[5], [14, 11]].flatMap(([x, yy]) => P(x, yy));
    drawBastion(g, shape(20, 0, [...poly, code, 0]));
    drawWalls(g, ring.slice(0, -1).map(([x, yy], i) => shape(0, 12 * (PX / 5), [...P(x, yy), ...P(...ring[i + 1]), (ruins ? 4 : 0) * PX, 0, code, 0])));
    const [bx, by] = where(9);
    place(g, bx, by, BW, BH, '9 West rich: a star bastion at a corner (its faces wall, its platform paved)', starSq);
  }
  return y;
}

/** Roadmap 5 cemetery props (`battlemap::Kind` 100-105) as the gallery shows them until the generator's
 * catalog has them (its entries win once it does). */
const CEMETERY_KINDS: KindInfo[] = (
  [
    [100, 'cairn', 0.4, 4],
    [101, 'standing stone', 0.3, 8],
    [102, 'yew', 1.6, 20],
    [103, 'lychgate', 1.0, 10],
    [104, 'kurgan', 3.5, 12],
    [105, 'tomb slab', 0.5, 2],
  ] as const
).map(([id, name, radius, height_ft]) => ({ id, name, radius, blocks_move: true, blocks_sight: false, cover: 1, difficult: false, height_ft, hazard: null, feature: true }));

/** Cemeteries (`&items=cemetery` alone): each cemetery prop on the graveyard's grass, every look (yews and
 * lychgates two), then a cemetery laid out by hand per culture group (rows of headstones, paths, the group's
 * tombs and trees): North cairns and standing stones, West yews and a lychgate, South tomb slabs between tomb
 * houses (plain blocks here), Dry tomb slabs round small domed tombs, Steppe kurgans. Three panels a row at
 * `sq` px a square (default to fit the width); `&panels=2,4` or `1-3` picks them. Returns the y below it. */
function cemeteries(app: Application, catalog: KindInfo[], root: Container, y0: number, width: number, nolabels: boolean, sqArg: number, picks: string | null): number {
  const kinds = [catalog.find((k) => k.id === 39)!, ...CEMETERY_KINDS.map((k) => catalog.find((c) => c.id === k.id) ?? k)];
  const info = new Map(kinds.map((k) => [k.id, k]));
  const atlas = buildAtlas(app.renderer, kinds);
  const [W, H] = [18, 12];
  const per = 3;
  const sq = sqArg || Math.floor((width - 16 * (per + 1)) / per / W);
  const [pw, ph] = [W * sq, H * sq];
  const shown = new Set<number>();
  for (const part of (picks ?? '1-6').split(',')) {
    const [a, b] = part.split('-').map(Number);
    for (let k = a; k <= (b || a); k++) shown.add(k);
  }
  const slots = [...shown].filter((k) => k >= 1 && k <= 6).sort((a, b) => a - b);
  const GROUND = [0x62843e, 0x62843e, 0x62843e, 0x6e8a44, 0xb8a676, 0x8c9a56];
  const GRAVEL = 0xb4a888;
  // (A hash for the layouts: the same picture every load.)
  const h = (a: number, b: number) => {
    let t = (Math.imul(a, 73856093) ^ Math.imul(b, 19349663)) >>> 0;
    t = Math.imul(t ^ (t >>> 13), 0x5bd1e995) >>> 0;
    return ((t ^ (t >>> 15)) >>> 0) / 4294967296;
  };
  slots.forEach((n, i) => {
    const [ox, oy] = [16 + (i % per) * (pw + 16), y0 + Math.floor(i / per) * (ph + 34)];
    const panel = new Container();
    const g = new Graphics();
    const props = new Container();
    const k = sq / PX;
    const scale = n === 1 ? Math.min(pw / (13 * PX), ph / (9.2 * PX)) : k;
    g.rect(0, 0, pw / scale, ph / scale).fill(GROUND[n - 1]);
    const put = (id: number, v: number, x: number, y: number, rot = 0, s = 1) => {
      const inf = info.get(id)!;
      const [cx, cy] = [x * PX, y * PX];
      if (inf.height_ft >= 1.5 && !OWN_SHADOW.has(inf.name)) {
        const sh = dropShadow(atlas, inf, cx, cy, s);
        sh.rotation = rot;
        props.addChild(sh);
      }
      const sp = new Sprite(atlas.frames[id][v % looks(id)]);
      sp.anchor.set(0.5);
      sp.position.set(cx, cy);
      sp.rotation = rot;
      sp.scale.set(s);
      props.addChild(sp);
    };
    const labels: Text[] = [];
    const label = (s: string, x: number, y: number) => {
      if (nolabels) return;
      const t = new Text({ text: s, style: { ...LABEL, fontSize: 11 } });
      t.position.set(ox + x * PX * scale, oy + y * PX * scale);
      labels.push(t);
    };
    // (In squares: a path, a boundary wall, rows of headstones skipping what `skip` says is taken.)
    const path = (x0: number, y0p: number, x1: number, y1: number) => g.rect(x0 * PX, y0p * PX, (x1 - x0) * PX, (y1 - y0p) * PX).fill({ color: GRAVEL, alpha: 0.9 });
    const wall = (color = 0x8c867a) => g.rect(0.25 * PX, 0.25 * PX, (W - 0.5) * PX, (H - 0.5) * PX).stroke({ width: 0.7 * (PX / 5) * 2, color, alpha: 0.95 });
    const rows = (x0: number, y0r: number, x1: number, y1: number, dx: number, dy: number, id: number, skip: (x: number, y: number) => boolean, salt: number, rot = 0) => {
      for (let y = y0r, r = 0; y <= y1; y += dy, r++) {
        for (let x = x0, c = 0; x <= x1; x += dx, c++) {
          if (skip(x, y) || h(salt + r, c) < 0.12) continue;
          put(id, Math.floor(h(c, salt + r) * 4), x + (h(r, c + 7) - 0.5) * 0.15, y, rot);
        }
      }
    };
    const block = (x: number, y: number, w: number, d: number, fill: number, lit: number) => {
      g.rect((x + 0.15) * PX, (y + 0.2) * PX, w * PX, d * PX).fill({ color: 0x1d1a14, alpha: 0.25 });
      g.rect(x * PX, y * PX, w * PX, d * PX).fill(fill).stroke({ width: 0.045 * PX, color: 0x1d1a14 });
      g.rect(x * PX, y * PX, w * PX, (d / 2) * PX).fill(lit).stroke({ width: 0.045 * PX, color: 0x1d1a14 });
    };
    const dome = (x: number, y: number, s: number) => {
      g.rect((x - s / 2 + 0.15) * PX, (y - s / 2 + 0.2) * PX, s * PX, s * PX).fill({ color: 0x1d1a14, alpha: 0.25 });
      g.rect((x - s / 2) * PX, (y - s / 2) * PX, s * PX, s * PX).fill(0xd8c8a4).stroke({ width: 0.045 * PX, color: 0x1d1a14 });
      g.circle(x * PX, y * PX, s * 0.4 * PX).fill(0xe8dcc0).stroke({ width: 0.045 * PX, color: 0x1d1a14 });
      g.circle((x - s * 0.1) * PX, (y - s * 0.1) * PX, s * 0.2 * PX).fill({ color: 0xf6eedc, alpha: 0.9 });
    };
    if (n === 1) {
      // Each prop, every look: headstones and cairns, standing stones and tomb slabs, yews and lychgates.
      const line = (id: number, x0: number, y: number, step: number, count = 4, rot = 0) => {
        for (let v = 0; v < count; v++) put(id, v, x0 + v * step, y, rot);
        label(info.get(id)!.name, x0 - 0.5, y + (id === 102 || id === 103 ? 1.75 : id === 105 ? 0.55 : 0.75));
      };
      line(39, 0.9, 1.0, 1.2);
      line(100, 7.3, 1.0, 1.2);
      line(101, 0.9, 3.4, 1.2);
      line(105, 6.9, 3.4, 1.6);
      line(102, 1.8, 6.6, 3.3, 2);
      line(103, 8.2, 6.6, 2.8, 2);
    } else if (n === 2) {
      // North: a granite yard of headstones, a cairn by each family, a short row of standing stones.
      path(8.4, 0, 9.4, H);
      path(0.5, 5.6, W - 0.5, 6.4);
      wall(0x7e7c74);
      const taken = (x: number, y: number) => (x > 11.5 && y < 5.2) || (x < 4 && y > 8) || Math.hypot(x - 5.2, y - 2.2) < 0.9;
      rows(1.2, 1.2, 7.6, 4.8, 1.1, 1.5, 39, taken, 20);
      rows(10.2, 1.2, 17, 4.8, 1.1, 1.5, 39, taken, 30);
      rows(1.2, 7.4, 7.6, 11, 1.1, 1.6, 39, taken, 40);
      rows(10.2, 7.4, 17, 11, 1.1, 1.6, 39, taken, 50);
      for (const [x, y, v] of [[12.6, 1.6, 0], [15.8, 2.2, 1], [13.8, 4.2, 2], [2.4, 3.2, 3], [6.6, 1.6, 0], [11.2, 9.4, 1], [15.9, 9.6, 2]] as const) put(101, v, x, y);
      for (const [x, y, v] of [[1.4, 9.2, 0], [2.8, 10.6, 1], [1.6, 11, 2], [5.2, 2.2, 3], [12.2, 3.4, 1], [16.2, 4.4, 0], [7.0, 8.6, 2], [16.0, 7.9, 3], [10.8, 1.4, 2]] as const) put(100, v, x, y);
      label('2 North: headstones, cairns, standing stones', 0, H);
    } else if (n === 3) {
      // West: headstones in rows off a gravel walk, yews along the wall, a lychgate where the walk comes in.
      path(8.4, 0.6, 9.4, H);
      path(0.5, 5.6, W - 0.5, 6.4);
      wall(0x9a9488);
      const taken = (x: number, y: number) => (x < 3 && y < 3.2) || (x > 15 && y < 3.2) || (x < 3 && y > 9) || (x > 15 && y > 9);
      rows(1.2, 1.4, 7.6, 4.8, 1.1, 1.4, 39, taken, 60);
      rows(10.2, 1.4, 17, 4.8, 1.1, 1.4, 39, taken, 70);
      rows(1.2, 7.4, 7.6, 10.4, 1.1, 1.5, 39, taken, 80);
      rows(10.2, 7.4, 17, 10.4, 1.1, 1.5, 39, taken, 90);
      for (const [x, y, v] of [[1.7, 1.7, 0], [16.3, 1.7, 1], [1.7, 10.4, 2], [16.3, 10.4, 3]] as const) put(102, v, x, y);
      put(103, 2, 8.9, H - 1.1);
      label('3 West: headstones, yews, a lychgate', 0, H);
    } else if (n === 4) {
      // South: family tomb houses in two rows off the walk, tomb slabs in the rows between them.
      path(8.3, 0, 9.5, H);
      wall(0xb09c7c);
      for (const y of [0.8, 7.8]) {
        for (const x of [0.9, 4.6]) block(x, y, 2.6, 2.4, 0xa8603e, 0xc8805a);
        for (const x of [10.4, 14.1]) block(x, y, 2.6, 2.4, 0xa8603e, 0xc8805a);
      }
      const taken = (x: number, y: number) => Math.abs(x - 8.9) < 1;
      rows(1.2, 4.6, 16.8, 6.4, 0.9, 1.7, 105, taken, 100, -Math.PI / 2);
      label('4 South: tomb houses (blocks), tomb slabs', 0, H);
    } else if (n === 5) {
      // Dry: small domed tombs among rows of tomb slabs on bare ground.
      path(0.4, 5.5, W - 0.4, 6.5);
      wall(0xc0aa80);
      for (const [x, y] of [[3, 2.4], [14.6, 2.6], [9, 9]] as const) dome(x, y, 2.4);
      const taken = (x: number, y: number) => Math.hypot(x - 3, y - 2.4) < 2.2 || Math.hypot(x - 14.6, y - 2.6) < 2.2 || Math.hypot(x - 9, y - 9) < 2.2;
      rows(1.2, 1.3, 16.8, 4.8, 1.8, 2.3, 105, (x, y) => taken(x, y) || Math.sin(x * 12.9 + y * 7.3) > 0.5, 130, -Math.PI / 2);
      rows(1.2, 7.4, 16.8, 10.6, 1.8, 2.3, 105, (x, y) => taken(x, y) || Math.sin(x * 9.1 + y * 11.7) > 0.5, 140, -Math.PI / 2);
      label('5 Dry: domed tombs (blocks), tomb slabs', 0, H);
    } else {
      // Steppe: kurgans on the open grass, the big one kerbed, a cairn and a few stones between.
      put(104, 3, 5.0, 5.6, 0, 0.9);
      put(104, 1, 13.5, 3.6, 0, 0.6);
      put(104, 2, 14.0, 9.4, 0, 0.5);
      put(100, 2, 9.8, 10.8);
      put(101, 1, 10.2, 7.0);
      label('6 Steppe: kurgans 45, 30 and 25 ft across (scaled), a cairn, a stone', 0, H);
    }
    panel.addChild(g, props);
    const clip = new Graphics().rect(0, 0, pw / scale, ph / scale).fill(0xffffff);
    panel.addChild(clip);
    panel.mask = clip;
    panel.scale.set(scale);
    panel.position.set(ox, oy);
    root.addChild(panel, ...labels);
  });
  return y0 + Math.ceil(slots.length / per) * (ph + 34) + 10;
}

/** An object's drop shadow as the battlemap casts it (`BattlemapLayer`): a soft blob to the SE (from the foot of
 * an upright thing), smaller and fainter for sparse plants, at `fit` screen px per atlas px. */
function dropShadow(atlas: ReturnType<typeof buildAtlas>, info: KindInfo, cx: number, cy: number, fit: number): Sprite {
  const [size, strength] = SPARSE.get(info.name) ?? [1, 1];
  const long = LONG_SHADOW.get(info.name);
  const off = Math.min(1.3, 0.15 + info.height_ft / 45) * PX * 0.7 * (long ? long[2] : 1);
  const sh = new Sprite(atlas.shadow);
  sh.anchor.set(0.5);
  if (long) sh.scale.set(((long[0] * PX) / 128) * fit, ((long[1] * PX) / 128) * fit);
  else sh.scale.set(((info.radius * PX * size * 2.1) / 128) * fit);
  sh.alpha = (info.height_ft >= 5 ? 0.6 : 0.35) * strength;
  sh.position.set(cx + off * fit, cy + ((UPRIGHT_FOOT[info.id] ?? 0) + off) * fit);
  return sh;
}

/** Trees and plants (by id), and topiary (drawn upright): never turned by hand. */
const PLANTS = new Set([1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 18, 19, 57]);

/** `&turn=1`: battlemap props that may be turned by hand, at four turns on grass with their shadows
 * (✓: in `TURNABLE`), then furniture and props with a back, at each side (as the designer turns
 * them: a quarter turn swaps width and depth). */
function turning(app: Application, catalog: KindInfo[], root: Container, head: (s: string) => void, getY: () => number, setY: (v: number) => void, width: number, sq: number) {
  const atlas = buildAtlas(app.renderer, catalog);
  const scale = sq / PX;
  const cell = Math.max(104, Math.ceil((sq / 48) * 104));
  const cellW = 4 * cell + 24;
  const per = Math.max(1, Math.floor((width - 32) / cellW));
  // Props not drawn turned already, but for trees, plants and hazards.
  const cands = catalog.filter((k) => !k.hazard && !PLANTS.has(k.id) && !drawnTurned(k, false));
  head(`Battlemap props turned by hand: 0°, 90°, 180°, 270° (✓ turns; the rest stay as drawn)`);
  let y = getY();
  cands.forEach((info, k) => {
    if (k % per === 0 && k) y += cell + 30;
    const x0 = 16 + (k % per) * cellW;
    const tex = atlas.frames[info.id][0];
    const fit = Math.min(scale, (cell - 8) / (Math.hypot(tex.width, tex.height) || 1));
    for (let q = 0; q < 4; q++) {
      const [cx, cy] = [x0 + cell / 2 + q * cell, y + cell / 2];
      root.addChild(new Graphics().rect(cx - cell / 2 + 2, cy - cell / 2 + 2, cell - 4, cell - 4).fill(0x7f9152));
      // The shadow as the battlemap casts it: always to the SE, whatever the turn.
      if (info.height_ft >= 1.5 && !OWN_SHADOW.has(info.name)) root.addChild(dropShadow(atlas, info, cx, cy, fit));
      const s = new Sprite(tex);
      s.anchor.set(0.5);
      s.scale.set(fit);
      s.rotation = (q * Math.PI) / 2;
      s.position.set(cx, cy);
      root.addChild(s);
    }
    const t = new Text({ text: `${TURNABLE.has(info.name) ? '✓ ' : ''}${info.id} ${info.name}`, style: LABEL });
    t.position.set(x0, y + cell + 2);
    root.addChild(t);
  });
  setY(y + cell + 50);

  // Furniture and props with a back, at each of their four sides.
  // Its size with its back to the north: a long side there (a bed's head at an end).
  const sizeOf = (list: [string, number, number][], kind: string): [number, number] => {
    const all = list.filter(([k]) => k === kind);
    const [, w, h] = (kind === 'bed' ? all.find(([, w, h]) => h > w) : all.find(([, w, h]) => w >= h)) ?? all[0] ?? [kind, 1, 1];
    return kind === 'bed' || w >= h ? [w, h] : [h, w];
  };
  const sides = (kinds: string[], list: [string, number, number][]) =>
    kinds.flatMap((kind) => {
      const [w, h] = sizeOf(list, kind);
      return [0, 1, 2, 3].map((b): [string, number, number, number, string] => [kind, b % 2 ? h : w, b % 2 ? w : h, b, `${kind.replace(/_/g, ' ')} · back ${'NESW'[b]}`]);
    });
  const buildingKinds = [...ORIENTED];
  const underKinds = [...new Set(UNDER.map(([k]) => k))].filter((k) => ON_WALL.has(k) || BACKED.has(k) || ORIENTED.has(k === 'bookshelf' ? 'bookcase' : k));
  // Rooms a little smaller than the other views, two items to a row.
  const isq = 34;
  const cols = Math.max(4, Math.floor((width - 32) / ((SLOT + 1) * isq) / 4) * 4);
  for (const [title, id, fn, list] of [
    ['Building furniture with a back (N, E, S, W)', 'b:gallery', 'house', sides(buildingKinds, FURNITURE)],
    ['Underground props with a back (N, E, S, W)', 'u:gallery', 'dungeon', sides(underKinds, UNDER)],
  ] as const) {
    head(title);
    const { it, spots } = sampler(id, fn, list as [string, number, number, number, string][], cols);
    const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: null });
    const holder = new Container();
    holder.addChild(layer.container);
    holder.scale.set(isq);
    holder.position.set(16, getY());
    layer.container.children[0].visible = false;
    root.addChild(holder);
    for (const [label, gx, gy] of spots) {
      const t = new Text({ text: label, style: { ...LABEL, fill: id.startsWith('u') ? 0xf3ead8 : 0x2a2219, fontSize: 11 } });
      t.position.set(16 + gx * isq + 2, getY() + (gy - 0.95) * isq);
      root.addChild(t);
    }
    setY(getY() + it.ny * isq + 40);
  }

  // What is in the hand inside (`InteriorLayer.setGhost`): a bookcase on auto facing (it finds the
  // bottom wall), where it fits, where it blocks a way, and where it can't go.
  head('Ghosts: fits, blocks a way, can’t go');
  ['ok', 'doorway', 'bad'].forEach((state, k) => {
    const { it } = sampler('b:ghost', 'house', [['chest', 1, 1]], 1);
    const layer = new InteriorLayer(it, 0, false, false, { player: false, doors: null });
    layer.setGhost({ kind: 'bookcase', name: 'bookcase', x: 2, y: 4, w: 2, h: 1, cover: 0, blocks_move: false, height_ft: 6 }, state as 'ok' | 'doorway' | 'bad');
    const holder = new Container();
    holder.addChild(layer.container);
    holder.scale.set(isq);
    holder.position.set(16 + k * (SLOT + 2) * isq, getY());
    layer.container.children[0].visible = false;
    root.addChild(holder);
  });
  setY(getY() + (SLOT + 2) * isq + 40);
}
