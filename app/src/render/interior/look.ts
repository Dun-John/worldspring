import { ROOF_SLOPE, OPEN_BELOW, TERRACE, type InteriorItem, type InteriorLevel } from '../../gen/protocol';
import { VARIED, UNDER_DRAWN, PLATFORMS } from '../furniture';
import { Graphics, GlProgram } from 'pixi.js';
import { INK } from '../pen';
import vertex from '../shaders/terrain.vert?raw';
import undergroundFragment from '../shaders/underground.frag?raw';
import { YURT_ROOMS, type Pt } from '../roomShapes';

/** Underground props drawn against the wall beside them (their look depends on its side). */
export const ON_WALL = new Set(['tunnel', 'cobweb', 'ore_vein', 'niche', 'chains', 'sconce', 'banner', 'pipe', 'ladder']);
/** Underground props with a back against a wall (a dais's throne; a collapsed wall, the wall it fell from). */
export const BACKED = new Set(['dais', 'collapsed_wall', 'throne', 'steps', 'climb_ladder', 'arcane_workbench']);
/** Underground props sharing a building furniture kind's name but drawn their own way below ground (a boss's stone
 * throne); in a building, still the furniture. */
const OWN_BELOW = new Set(['throne']);
/** What a wall prop may lie over: the floor and what is strewn on it. */
export const UNDERFOOT = new Set(['sewage', 'lava', 'rail', 'guano', 'moss', 'rubble', 'debris', 'sulfur', 'drain', 'grille']);
/** Props drawn along the run of a sewer under them (`runDir`). */
export const ALONG_RUN = new Set(['drain', 'grille']);
/** An item's `back` (0 north 1 east 2 south 3 west) as a unit step in grid axes. */
const FACES: [number, number][] = [[0, -1], [1, 0], [0, 1], [-1, 0]];
/** The side set on an item in its design, if any. */
export const setBack = (f: InteriorItem): [number, number] | null => (f.back !== undefined && f.back >= 0 && f.back < 4 ? FACES[f.back] : null);
/** The side (0-3) a unit step points to (a diagonal counts as north or south); null for none. */
export const sideOf = ([dx, dy]: [number, number]): number | null => (!dx && !dy ? null : Math.abs(dy) >= Math.abs(dx) ? (dy < 0 ? 0 : 2) : dx > 0 ? 1 : 3);
/** Underground props drawn like the building furniture of the same name (`bookshelf` as a bookcase). */
const SHARED: Record<string, string> = {
  chest: 'chest', barrel: 'barrel', crate: 'crate', table: 'table', rug: 'rug', altar: 'altar', statue: 'statue', pillar: 'pillar',
  cage: 'cage', rack: 'rack', weapon_rack: 'weapon_rack', cot: 'cot', sarcophagus: 'sarcophagus', winch: 'winch', trapdoor: 'trapdoor',
  bookshelf: 'bookcase', desk: 'desk', long_table: 'long_table', vat: 'vat', anvil: 'anvil', forge: 'forge', hearth: 'hearth', keg_rack: 'keg_rack',
  chair_of_office: 'chair_of_office',
};
/** The kind a building's furniture or an underground prop is drawn as with the furniture's drawing
 * (in a building: everything but the indoor props, drawn as underground); none for the rest. */
export function sharedKind(kind: string, underground: boolean): string | undefined {
  if (underground) return SHARED[kind];
  return VARIED.has(kind) || !UNDER_DRAWN.has(kind) || OWN_BELOW.has(kind) ? kind : undefined;
}
/** Props scattered differently per placement: one of four variants, picked by position. */
export const SCATTERED = new Set(['boulder', 'guano', 'moss', 'rubble', 'debris', 'sulfur', 'ore_vein', 'nest', 'rat_nest']);
export { INK };
/** A lock's colour 1-31 (`Door.lock`) and its key's (`Item.variant`); 0 a key with no colour. */
export const LOCK_COLORS = [0x9a958b, 0xd8412f, 0xe8892a, 0xe8cc2a, 0x3fae52, 0x2f86d8, 0x9a56d4, 0xf0ece0];
/** Colours 8-31 step round the hue wheel (`MAX_LOCK`), in lightness bands so neighbours differ. */
export const lockColor = (n: number) => {
  if (n >= 0 && n < LOCK_COLORS.length) return LOCK_COLORS[n];
  if (n > 31) return LOCK_COLORS[0];
  const k = n - LOCK_COLORS.length;
  return hsl(((k * 47 + 20) % 360) / 360, 0.62, [0.5, 0.36, 0.64][k % 3]);
};
function hsl(h: number, s: number, l: number): number {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    return Math.round(255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1))));
  };
  return (f(0) << 16) | (f(8) << 8) | f(4);
}
/** Underground terrain (`kit::TERRAIN`) drawn on the floor under everything else, and the spans over it (`kit::SPANS`). */
export const TERRAIN_KINDS = new Set(['water_shallow', 'water_deep', 'chasm', 'rubble_slope']);
export const SPAN_KINDS = new Set(['rope_bridge', 'stone_bridge', 'stepping_stone']);
export const hash2 = (i: number, j: number, k: number) => {
  const v = Math.sin(i * 127.1 + j * 311.7 + k * 74.7) * 43758.5453;
  return v - Math.floor(v);
};
/** A colour scaled toward black (`f` < 1) or white (`f` > 1). */
export function tone(color: number, f: number): number {
  const ch = (k: number) => {
    const c = (color >> k) & 255;
    return Math.round(f < 1 ? c * f : c + (255 - c) * (f - 1)) & 255;
  };
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}
/** Iron: dark, mid, light. */
export const IRON = [0x34383e, 0x6c737c, 0xaab2bb];
/** A padlock about half a square tall on a door, centred on (cx, cy): a body in the lock's colour (lit top,
 * shaded foot, keyhole), a steel shackle, inked; open, the shackle lifted with one leg out of the body. */
export function padlock(g: Graphics, cx: number, cy: number, color: number, open: boolean) {
  const [bw, bh, r] = [0.36, 0.27, 0.105];
  const top = cy - 0.06;
  const lift = open ? 0.1 : 0;
  const arcY = top - 0.06 - lift;
  const shackle = (width: number, c: number) => {
    g.moveTo(cx + r, top + 0.02).lineTo(cx + r, arcY).arc(cx, arcY, r, 0, Math.PI, true).lineTo(cx - r, open ? arcY + 0.05 : top + 0.02).stroke({ width, color: c, cap: 'round' });
  };
  shackle(0.095, INK);
  shackle(0.055, 0xc9ced4);
  g.arc(cx, arcY, r, Math.PI * 1.1, Math.PI * 1.45).stroke({ width: 0.018, color: 0xf4f6f8, cap: 'round' });
  // The body: two flat tones (lit upper face, shaded foot) and an ink edge.
  g.roundRect(cx - bw / 2, top, bw, bh, 0.06).fill(tone(color, 0.72));
  g.roundRect(cx - bw / 2 + 0.02, top + 0.015, bw - 0.04, bh * 0.62, 0.05).fill(color);
  g.moveTo(cx - bw / 2 + 0.06, top + 0.04).lineTo(cx + bw / 2 - 0.08, top + 0.04).stroke({ width: 0.022, color: tone(color, 1.45), cap: 'round' });
  g.roundRect(cx - bw / 2, top, bw, bh, 0.06).stroke({ width: 0.035, color: INK });
  // The keyhole.
  g.circle(cx, top + 0.11, 0.034).fill(INK);
  g.poly([cx - 0.018, top + 0.12, cx + 0.018, top + 0.12, cx + 0.03, top + 0.205, cx - 0.03, top + 0.205]).fill(INK);
}
/** A key about 0.7 square across, centred on (cx, cy): an iron key (ring bow, shaft, two-toothed bit) lying
 * aslant, a tag in its colour tied to the bow, and a faint ring of its colour round it so the DM finds it. */
export function keyGlyph(g: Graphics, cx: number, cy: number, color: number) {
  g.circle(cx, cy, 0.47).fill({ color, alpha: 0.13 }).stroke({ width: 0.035, color, alpha: 0.6 });
  const [c, s] = [Math.cos(-0.45) * 1.3, Math.sin(-0.45) * 1.3];
  const R = (u: number, v: number): [number, number] => [cx + 0.02 + u * c - v * s, cy + u * s + v * c];
  // The tag, hung from the bow on a string, under the key.
  const [tx, ty] = R(-0.2, 0.2);
  const ta = 0.5;
  const T = (u: number, v: number): [number, number] => [tx + (u * Math.cos(ta) - v * Math.sin(ta)) * 1.25, ty + (u * Math.sin(ta) + v * Math.cos(ta)) * 1.25];
  g.moveTo(...R(-0.2, 0.06)).lineTo(...T(-0.09, 0)).stroke({ width: 0.018, color: INK });
  const tag = [...T(-0.1, 0), ...T(-0.04, -0.07), ...T(0.13, -0.07), ...T(0.13, 0.07), ...T(-0.04, 0.07)];
  g.poly(tag).fill(tone(color, 0.75));
  g.poly([...T(-0.07, -0.01), ...T(-0.03, -0.055), ...T(0.11, -0.055), ...T(0.11, 0.02), ...T(-0.03, 0.02)]).fill(color);
  g.poly(tag).stroke({ width: 0.025, color: INK, join: 'round' });
  g.circle(...T(-0.035, 0), 0.016).fill(INK);
  // The key: ink under, iron over, a lit line along the top.
  const shape = (wd: number, col: number) => {
    g.circle(...R(-0.2, 0), 0.105 * 1.3).stroke({ width: wd, color: col });
    g.moveTo(...R(-0.1, 0)).lineTo(...R(0.3, 0)).stroke({ width: wd, color: col, cap: 'round' });
    g.moveTo(...R(0.2, 0)).lineTo(...R(0.2, 0.11)).moveTo(...R(0.29, 0)).lineTo(...R(0.29, 0.13)).stroke({ width: wd * 0.95, color: col, cap: 'square' });
  };
  shape(0.12, INK);
  shape(0.068, IRON[1]);
  g.arc(...R(-0.2, 0), 0.105 * 1.3, -0.45 + Math.PI * 1.05, -0.45 + Math.PI * 1.6).stroke({ width: 0.024, color: IRON[2] });
  g.moveTo(...R(-0.08, -0.014)).lineTo(...R(0.27, -0.014)).stroke({ width: 0.022, color: IRON[2], cap: 'round' });
}
/** A rough round outline of `n` points round (cx, cy), radius about `r`, the same for the same seed. */
export function blob(cx: number, cy: number, r: number, seed: number, n = 9): number[] {
  const out: number[] = [];
  const ph = hash2(seed, 3, 7) * Math.PI * 2;
  for (let k = 0; k < n; k++) {
    const a = ph + (k / n) * Math.PI * 2;
    const rr = r * (0.84 + 0.3 * hash2(seed, k, 11));
    out.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr * 0.9);
  }
  return out;
}
export const COVER = ['no cover', 'half cover', 'three-quarters cover', 'total cover'];
// Underground levels are painted by a shader (`underground.frag`) from a smooth floor field
// (`underField.ts`).
export const underProgram = GlProgram.from({ vertex, fragment: undergroundFragment, name: 'underground' });
/** How far the map above is dimmed under an underground level (`Sewers` too). */
export const UNDER_DIM = 0.62;
/** The piece on square (i, j) of a level: what stands on a platform before the platform. */
export function itemAt(lv: InteriorLevel, i: number, j: number): InteriorItem | undefined {
  const on = (f: InteriorItem) => i >= f.x && i < f.x + f.w && j >= f.y && j < f.y + f.h;
  return lv.furniture.find((f) => on(f) && !PLATFORMS.has(f.kind)) ?? lv.furniture.find(on);
}
/** One of `n` variants (four for most) for an item at grid square (x, y). */
export function itemVariant(x: number, y: number, n = 4): number {
  const v = Math.sin(x * 127.1 + y * 311.7) * 43758.5453;
  return Math.floor((v - Math.floor(v)) * n);
}
/** Banners, whose look is a house: one per building unless set. */
export const BANNERS = new Set(['wall_banner', 'standing_banner']);
/** Floor colour by room kind. */
export function floorColor(kind: string, z: number, natural = false, raise = 0): number {
  if (natural) {
    if (kind === 'boss chamber') return 0x7d7064;
    return raise >= 10 ? 0xa39886 : raise >= 5 ? 0x988d7c : 0x8a8072;
  }
  if (kind === 'boss chamber') return 0x8d8579;
  if (kind === 'sewer tunnel' || kind === 'service passage') return 0x8a7d6c;
  if (['arena floor'].includes(kind)) return 0xc8b184;
  if (['kitchen', 'scullery', 'baths', 'forge', 'brewhouse'].includes(kind)) return 0x8f8a80;
  // (A palace's court a lawn (its gravel walks laid on it), its ballroom's parquet the lightest wood.)
  if (kind === 'courtyard' || kind === 'garth') return LAWN;
  // (A coaching inn's yard and its arch cobbled.)
  if (kind === 'inn yard') return 0x958f83;
  if (kind === 'ballroom') return 0xbb8d5c;
  // (A roof's squares: the walk's stone, under the roof drawn over them, `drawRoofs`.)
  if (kind === 'battlements' || kind === 'wall walk' || kind === ROOF_SLOPE) return 0xa8a296;
  if (kind === OPEN_BELOW) return -1;
  // (A flat roof's mud plaster, smooth.)
  if (kind === TERRACE) return PLASTER;
  if (kind === 'stands') return 0x8a7a64;
  // (A yurt's felt floor: no joints.)
  if (YURT_ROOMS.has(kind)) return FELT_FLOOR;
  if (z < 0 || ['nave', 'sanctuary', 'crypt', 'ossuary', 'great hall', 'mess hall', 'dungeon', 'guardroom', 'armory', 'vault', 'chapel', 'oratory', 'courtroom', 'counting house', 'hall', 'entry hall', 'foyer', 'holding pen', 'beast pen', 'cell', 'warehouse floor', 'screens passage', 'mural passage', 'landing', 'entrance hall', 'gate passage', 'keep yard', 'well room', 'garrison', 'throne room', 'banqueting hall', 'royal chapel', 'stair hall', "servants' hall", 'side chapel', 'shrine', 'chapter house', 'cloister walk', 'refectory', 'arcade', 'strongroom', 'warded vault', 'trophy hall'].includes(kind))
    return 0x9d978c;
  return 0xa87a4f;
}
const STONE = (c: number) => c === 0x9d978c || c === 0x8f8a80 || c === 0xa8a296 || c === 0x958f83;
/** A court's lawn (`floorColor`). */
const LAWN = 0x7f9a56;
/** A roof terrace's plaster (`floorColor`): no joints. */
const PLASTER = 0xc4ad88;
/** A yurt's floor of undyed felt (`floorColor`): no joints. */
const FELT_FLOOR = 0xb39d78;
/** A floor square's pattern in its colour: planks along x on wood, flagstone joints on stone, none on a lawn. */
export function floorPattern(i: number, j: number, c: number): [Pt, Pt][] {
  if (c === LAWN || c === PLASTER || c === FELT_FLOOR) return [];
  if (!STONE(c)) return [1, 2, 3].map((k): [Pt, Pt] => [[i, j + k / 4], [i + 1, j + k / 4]]);
  const odd = ((j % 2) + 2) % 2;
  return [
    [[i, j + 0.5], [i + 1, j + 0.5]],
    [[i + odd * 0.5 + 0.25, j], [i + odd * 0.5 + 0.25, j + 0.5]],
    [[i + (1 - odd) * 0.5 + 0.25, j + 0.5], [i + (1 - odd) * 0.5 + 0.25, j + 1]],
  ];
}
/** Rooms open to the sky: their outer wall a parapet, with merlons. */
export const OPEN_TOP = new Set(['battlements', 'wall walk', 'tower top', ROOF_SLOPE]);
/** Rooms that are ways about a floor (`interior::is_circulation`): a railing along a hall's open
 * height where it meets one. */
export const CIRCULATION = new Set(['hall', 'corridor', 'landing', 'screens passage', 'mural passage', 'entrance hall', 'cloister walk', 'keep yard', 'nave', "minstrels' gallery", 'gate passage', 'courtyard', 'long gallery', 'stair hall']);
/** Furniture fill by kind. */
export 
function itemColor(kind: string): number {
  switch (kind) {
    case 'bed':
    case 'cot':
      return 0x7d5a3c;
    case 'rug':
      return 0x8b3a3a;
    case 'hearth':
    case 'oven':
    case 'forge':
      return 0x4a4440;
    case 'barrel':
    case 'keg_rack':
    case 'vat':
      return 0x6e4a2c;
    case 'crate':
      return 0x9a7a4e;
    case 'altar':
    case 'statue':
    case 'pillar':
    case 'sarcophagus':
    case 'bell':
      return 0xb8b2a6;
    case 'cage':
      return 0x55524d;
    case 'trapdoor':
      return 0x6a4a2a;
    case 'bath':
      return 0x7fa3b5;
    case 'telescope':
    case 'anvil':
      return 0x3f4448;
    default:
      return 0x5e4028;
  }
}

/** A building's walls as drawn inside (a matte band in an ink outline, `roomShapes.WALL_OUT`), by
 * what it is built of: dressed stone for fortifications, temples and halls; timber for barns,
 * mills and sheds; else daub over the frame. Dark enough to read as wall against any floor. */
const STONE_WALLS = /keep|castle|tower|gate|temple|shrine|chapel|church|cathedral|abbey|monaster|bank|hall|guard|barrack|fort|lighthouse|prison|jail|citadel|palace|library|mausoleum|treasury|armou?ry|bastion|wall|arena/i;
const TIMBER_WALLS = /barn|stable|mill|warehouse|shed|farm|cabin|hut|longhouse|boathouse|granary|yurt|lodge/i;
export function wallTone(fn: string): number {
  if (STONE_WALLS.test(fn)) return 0x57534c;
  if (TIMBER_WALLS.test(fn)) return 0x4e3b2a;
  return 0x675c4d;
}
/** A window's glass, seated in its ink frame. */
export const GLASS = 0xb3cbd1;
