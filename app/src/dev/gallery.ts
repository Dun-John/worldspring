// Sprite gallery (`?gallery=1`): every piece of art the map draws, labelled, to choose what to
// redraw. `&kinds=15,16` shows only those battlemap kinds (and no furniture or props), `&sq=96` draws
// them at that many px a square (default 48), `&ground=948c80` on that colour with their shadows as the battlemap casts
// them. `&items=bed,chair` shows only those furniture and props
// (each in both orientations; no battlemap objects; `&looks=1` every look of each; `&floor=wood` on boards; `&nolabels=1` unlabelled, for blind crops), `&sq=` then sizing their squares (default 44). Battlemap objects come from the atlas (all variants); building furniture and
// underground props are drawn by InteriorLayer exactly as in the app, one per little room. `&turn=1` shows how
// things look turned by hand instead: battlemap props that may turn (`TURNABLE`, and the other candidates) at four
// turns on grass with their shadows, and furniture and props with a back at each of their four sides.
import { Application, Container, Graphics, Sprite, Text, type TextStyleOptions } from 'pixi.js';
import init, { battlemap_catalog_json, under_catalog_json } from '../gen/pkg/worldgen_wasm.js';
import type { KindInfo } from '../gen/client';
import type { Interior, InteriorItem, UnderCatalog } from '../gen/protocol';
import { BATTLE_GROUPS, ITEM_GROUPS, looks as insideLooks } from '../editor/itemGroups';
import { buildAtlas, looks, PX, UPRIGHT_FOOT, VARIANTS } from '../render/atlas';
import { drawnTurned, LONG_SHADOW, OWN_SHADOW, SPARSE, TURNABLE } from '../render/BattlemapLayer';
import { ORIENTED } from '../render/furniture';
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
  ['bath', 1, 2], ['bath', 2, 2], ['bucket', 1, 1], ['trapdoor', 1, 1], ['winch', 1, 1], ['telescope', 2, 2], ['barricade', 2, 1],
  ['stage', 3, 2], ['spiral_stair', 1, 1], ['link_down', 1, 1],
  ['throne_dais', 4, 2], ['throne_dais', 3, 2], ['low_dais', 3, 2], ['chair_of_office', 1, 1], ['high_table', 4, 1], ['carpet_runner', 1, 5], ['carpet_runner', 2, 4],
  ['tapestry', 3, 1], ['tapestry', 2, 1], ['wall_banner', 1, 1], ['standing_banner', 1, 1], ['font', 1, 1], ['cloister_well', 2, 2],
  ['dance_floor', 5, 5], ['dance_floor', 4, 3], ['harpsichord', 1, 2], ['musicians_corner', 2, 2], ['refreshment_table', 3, 1], ['refreshment_table', 2, 1],
];

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
  ['table', 1, 2], ['crate', 1, 1], ['barrel', 1, 1], ['altar', 2, 1], ['glyph', 2, 2],
];

const LABEL: TextStyleOptions = { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x2a2219 };
const HEAD: TextStyleOptions = { fontFamily: 'Georgia, serif', fontSize: 22, fill: 0x1d1a14, fontWeight: 'bold' };
/** Screen px per 5-ft square for furniture and props. */
const ITEM_SQ = 44;
const SLOT = 4;

/** One room per item in a grid of `cols` (each `size` squares, rock between them); an item may give the
 * side its back is against (0-3), its label and its look. */
function sampler(id: string, fn: string, items: [string, number, number, number?, string?, number?][], cols: number, size = SLOT): { it: Interior; spots: [string, number, number][] } {
  const rows = Math.ceil(items.length / cols);
  const [nx, ny] = [cols * (size + 1) + 1, rows * (size + 1) + 1];
  const cells = new Array<number>(nx * ny).fill(-1);
  const furniture: InteriorItem[] = [];
  const rooms: Interior['levels'][number]['rooms'] = [];
  const spots: [string, number, number][] = [];
  items.forEach(([kind, w, h, back, label, variant], k) => {
    const [x0, y0] = [1 + (k % cols) * (size + 1), 1 + Math.floor(k / cols) * (size + 1)];
    for (let j = 0; j < size; j++) for (let i = 0; i < size; i++) cells[(y0 + j) * nx + x0 + i] = k;
    rooms.push({ kind: ' ', squares: size * size, raise_ft: 0, center: [x0 + size / 2, y0 + size / 2] });
    // Against the room's top wall, centred (props on a wall find the rock behind them).
    const x = x0 + Math.floor((size - w) / 2);
    // With a side set: in the middle of the room, so only the side makes it face.
    const y = back === undefined ? y0 : y0 + Math.floor((size - h) / 2);
    furniture.push({ kind, name: kind, x, y, w, h, cover: 0, blocks_move: false, height_ft: 3, ...(back === undefined ? {} : { back }), ...(variant === undefined ? {} : { variant }) });
    spots.push([label ?? (w === 1 && h === 1 && kind === 'table' ? 'table (round)' : `${kind.replace(/_/g, ' ')}${w * h > 1 ? ` ${w}×${h}` : ''}`), x0, y0 + size]);
  });
  // (`&floor=wood`: rooms above ground, so their floors are boards, not flags.)
  const z = new URLSearchParams(location.search).get('floor') === 'wood' ? 0 : -1;
  const level = { z, name: '', elevation_ft: 0, cells, rooms, walls: [], doors: [], windows: [], furniture, roof: false, has_stairs: false, natural: false };
  return { it: { id, settlement: 0, building: 0, name: null, function: fn, origin: [0, 0], axis: [1, 0], across: [0, 1], nx, ny, levels: [level], entry_level: 0, stairs: [0, 0, 0, 0] }, spots };
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
  const pick = query.get('items')?.split(',');
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

  const bg = new Graphics().rect(0, 0, width, y).fill(0xefe6d2);
  app.stage.addChild(bg, root);
  app.renderer.resize(width, y);
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
