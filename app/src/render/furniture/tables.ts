// The furniture's tables, apart from its drawing code (render/sprites.ts, loaded later): which pieces are drawn
// here and how many looks each has, their names for the designer, and the colours those tables name.
import { PLATFORMS } from '../../gen/protocol';
import { IRON } from './common';

/** Gilt and bronze fittings: shade, face, lit. */
export const GILT = [0x8a6a2a, 0xc8a04a, 0xf0d488];
export const BRONZE = [0x5a4220, 0x9a7438, 0xd0a85a];
/** White metal (blades, silver coin, a castle argent): shade, face, lit. */
export const SILVER = [0x7a7e84, 0xb8bcc0, 0xe4e6e8];
/** Red velvet (cushions, a banner's cloth): shade, cloth, lit. */
export const VELVET = [0x6a1c18, 0xa83228, 0xd0604a];

/** House colours (`Item::variant` of a wall banner and a standing banner, the same index on both, so a castle's
 * banners can all fly one house's): the field (shade, cloth, lit), its charge's colour and the charge. Even
 * houses' banners end in a point, odd ones in a swallowtail. */
export const HOUSES: { name: string; field: readonly number[]; charge: number; mark: 'tower' | 'cross' | 'bend' | 'star' | 'crescent' | 'quarterly' | 'chevron' | 'saltire' }[] = [
  { name: 'Red, a gold tower', field: [0x6e2219, 0x9e3428, 0xbc4c3a], charge: 0xe8bc4e, mark: 'tower' },
  { name: 'Blue, a white cross', field: [0x23365e, 0x34528a, 0x4a6ca6], charge: 0xf0e8d4, mark: 'cross' },
  { name: 'Green, a gold bend', field: [0x2e4a22, 0x46682e, 0x5e8240], charge: 0xe8bc4e, mark: 'bend' },
  { name: 'Black, a gold star', field: [0x221e26, 0x37313c, 0x4e4654], charge: 0xe8bc4e, mark: 'star' },
  { name: 'Purple, a white crescent', field: [0x48284a, 0x6a3e68, 0x865682], charge: 0xf0e8d4, mark: 'crescent' },
  { name: 'White and red quarterly', field: [0xb0a68e, 0xe4d8bc, 0xf6efe0], charge: 0xa83226, mark: 'quarterly' },
  { name: 'Gold, a black chevron', field: [0x9a7428, 0xd0a442, 0xe8c466], charge: 0x2e2830, mark: 'chevron' },
  { name: 'Teal, a white saltire', field: [0x1e4a48, 0x2e6e6a, 0x46908a], charge: 0xf0e8d4, mark: 'saltire' },
];

/** The pieces drawn in their room's floor colour and their building's house (`drawFurniture`'s `tone`). */
export const HALL_TONED = new Set(['throne_dais', 'high_table']);

/** The house a building's banners fly when none is set (`Item::variant` absent): one per building, from its id
 * (FNV-1a), so every banner put down in one castle hall matches. */
export function houseOf(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 0x01000193) >>> 0;
  return h % HOUSES.length;
}

/** A temple's pieces drawn for its god (`interior::GOD_LOOKS`, `town::temple::god_look`): the god's kinds
 * (`town::temple::GOD_KINDS`), each with the stone of its statue, the metal of its candle stands and fittings
 * (shade, face, lit), an accent for its emblem and carving, the cloth of its altars and the glow of its
 * candles. */
export const GODS: { name: string; stone: number[]; metal: number[]; accent: number; cloth: number[]; glow: number }[] = [
  { name: 'The Sun and the Morning Lord', stone: [0x8a7a62, 0xd8c8a4, 0xf2e6c6], metal: GILT, accent: 0xf2c94c, cloth: [0x8a5a1a, 0xc98a2a, 0xe8b850], glow: 0xffc23a },
  { name: 'The Dawn', stone: [0x9a7268, 0xd8ae9e, 0xf2d2c4], metal: [0x8a4e3e, 0xc88672, 0xf0bca4], accent: 0xe88aa0, cloth: [0x8a3a48, 0xc8707e, 0xe8a0aa], glow: 0xffb27a },
  { name: 'The Moon and the Veiled Lady', stone: [0x80869a, 0xc4c8d4, 0xeceef4], metal: [0x5e646c, 0xa4aab4, 0xe0e4ea], accent: 0xdce4f4, cloth: [0x26305a, 0x3e4a7a, 0x5e6a9a], glow: 0xe4ecff },
  { name: 'The Deep Sea', stone: [0x3e5e52, 0x5e8a74, 0x8ab8a0], metal: [0x2e5a52, 0x4e8a7a, 0x8ac0aa], accent: 0x8ad0c0, cloth: [0x1e4a4e, 0x2e6a6a, 0x4e8a86], glow: 0xb8eadc },
  { name: 'The Storm', stone: [0x4a5260, 0x727c8c, 0x9ca6b4], metal: IRON, accent: 0xf2d45a, cloth: [0x2e4470, 0x4f72a8, 0x7a98c8], glow: 0xffe08a },
  { name: 'The Forge', stone: [0x4a3a30, 0x6e5848, 0x947a64], metal: BRONZE, accent: 0xf28a1e, cloth: [0x5e1e18, 0x8a2e26, 0xb0483a], glow: 0xff9a3a },
  { name: 'The Harvest and the Hearth', stone: [0x96764c, 0xc8a878, 0xe8cc9a], metal: [0x7a5a2a, 0xb08a42, 0xe0bc6a], accent: 0xe8c860, cloth: [0x4a6a2a, 0x6a8a3a, 0x8eac52], glow: 0xffc860 },
  { name: 'The Seven and the Watcher', stone: [0x3e3a46, 0x605a6c, 0x8a8494], metal: [0x3a3644, 0x686078, 0xa49ab8], accent: 0xc8b8f0, cloth: [0x34284e, 0x4a3a6a, 0x6a5a8e], glow: 0xe0d4ff },
];

/** The god's kinds by look, for the designer's Look menu. */
export const GOD_NAMES = GODS.map((g) => g.name);

/** A guild's arms or a town's (`Item::variant` of guild_arms): the field (shade, field, lit), the shield's rim, the
 * cloth of the banner it hangs on (two squares). */
export const ARMS: { name: string; field: readonly number[]; rim: number; cloth: readonly number[] }[] = [
  { name: "Merchants' guild: gold scales on blue", field: HOUSES[1].field, rim: GILT[1], cloth: VELVET },
  { name: 'Craft guild: an anvil and hammer on red', field: HOUSES[0].field, rim: GILT[1], cloth: [0x1c2a4a, 0x2e4a7a, 0x4a6aa0] },
  { name: "Adventurers' guild: crossed swords on green", field: HOUSES[2].field, rim: SILVER[1], cloth: [0x5a3a1e, 0x8a5a2e, 0xb07a44] },
  { name: "Mages' guild: a star over a book on midnight blue", field: [0x161c3a, 0x252e5c, 0x384480], rim: GILT[1], cloth: VELVET },
  { name: "The town's arms: a red tower on gold", field: [0x9a7428, 0xd0a442, 0xe8c466], rim: SILVER[1], cloth: [0x1c2a4a, 0x2e4a7a, 0x4a6aa0] },
];

/** Furniture with a back (InteriorLayer.backSide): to the wall, a bed's head to the wall at its end, a
 * pew's away from the altar, a booth seat's away from its table. */
export const ORIENTED = new Set([
  'shelf', 'bookcase', 'hearth', 'oven', 'forge', 'workbench', 'counter', 'bar', 'keg_rack', 'bed', 'couch', 'sideboard', 'pew', 'booth_seat', 'altar', 'statue', 'throne', 'alchemy_bench', 'weapon_rack', 'stage',
  'throne_dais', 'chair_of_office', 'high_table', 'tapestry', 'wall_banner', 'musicians_corner', 'low_dais',
  'god_statue', 'carved_screen', 'screen_gate', 'side_altar',
  'job_board', 'guild_arms', 'stable_stall',
  'yurt_hearth', 'yurt_pallet', 'painted_chest', 'floor_cushion',
]);

/** Platforms other pieces stand on, drawn under them. */
export { PLATFORMS };

/** Furniture with other than four looks (`interior::item_looks`): the throne dais's three seatings in stone and
 * gilt, then on timber; a banner's house (`HOUSES`). */
const LOOKS: Record<string, number> = {
  pipe: 3,
  throne_dais: 6,
  wall_banner: HOUSES.length,
  standing_banner: HOUSES.length,
  low_dais: 2,
  // (A temple's pieces: one look per kind of god, `GODS`.)
  god_statue: 8,
  candle_stand: 8,
  carved_screen: 8,
  screen_gate: 8,
  side_altar: 8,
  // A guild's arms, or the town's (`ARMS`).
  guild_arms: ARMS.length,
  // A key's colour (1-7 opens the lock of that colour; `interior::Door::lock`).
  key: 32,
  // A burial niche: its look `v % 4` (body, bones, slab, empty), the rest only varies the details.
  niche: 12,
};

/** How many looks a piece of furniture has (`Item::variant` 0 to this - 1). */
export const looksOf = (kind: string): number => LOOKS[kind] ?? 4;

/** A lock's colour by number (`lockColor`): 0-7 as named, 8-31 a hue round the wheel in a dark, usual or light band. */
export function lockName(v: number): string {
  if (v < 8) return ['No colour', 'Red', 'Orange', 'Yellow', 'Green', 'Blue', 'Purple', 'White'][v];
  const k = v - 8;
  const hue = (k * 47 + 20) % 360;
  const names: [number, string][] = [[15, 'red'], [45, 'orange'], [70, 'yellow'], [95, 'lime'], [160, 'green'], [190, 'teal'], [215, 'sky'], [255, 'blue'], [290, 'violet'], [330, 'magenta'], [361, 'rose']];
  const base = names.find(([h]) => hue < h)![1];
  const band = ['', 'dark ', 'light '][k % 3];
  return `${(band + base).replace(/^./, (c) => c.toUpperCase())} (${v})`;
}

/** What a look of a piece is, for the designer's Look menu: named where they differ by more than colour. */
export function lookName(kind: string, v: number): string {
  if (kind === 'wall_banner' || kind === 'standing_banner') return HOUSES[v % HOUSES.length].name;
  if (kind === 'throne_dais') return `${['One throne', 'Two thrones', 'A throne and a lesser chair'][v % 3]}, ${v < 3 ? 'stone and gilt' : 'timber'}`;
  if (kind === 'tapestry') return ['A stag hunt', 'A unicorn', 'Ships by a castle', 'A rider and a castle'][v % 4];
  if (kind === 'low_dais') return ['Stone', 'Timber'][v % 2];
  if (['god_statue', 'candle_stand', 'carved_screen', 'screen_gate', 'side_altar'].includes(kind)) return GOD_NAMES[v % GOD_NAMES.length];
  if (kind === 'guild_arms') return ARMS[v % ARMS.length].name;
  if (kind === 'key') return lockName(v);
  if (kind === 'strongbox') return ['Black iron', 'Walnut, iron-strapped', 'Iron, brass studs', 'Oak, iron-strapped'][v % 4];
  if (kind === 'counting_desk') return ['Oak, green baize', 'Walnut, green baize', 'Oak, red leather', 'Walnut, red leather'][v % 4];
  if (kind === 'job_board') return ['Notices', 'A wanted bill', 'A painted header', 'A map'][v % 4];
  if (kind === 'stable_stall') return ['Empty', 'A bay horse', 'A grey horse', 'A chestnut horse'][v % 4];
  if (kind === 'hearth') return ['Logs and flame', 'An ember bed', 'Banked', 'Laid ready, cold'][v % 4];
  if (kind === 'council_table') return ['Oval, oak', 'Oak, green baize', 'Oval, walnut', 'Walnut, a blue runner'][v % 4];
  return `Look ${v + 1}`;
}

/** Furniture drawn here: one of its looks each (`looksOf`), picked by position (InteriorLayer.itemKey). */
export const VARIED = new Set([
  'water_trough', 'barrels', 'cart',
  'bed', 'cot', 'table', 'long_table', 'chair', 'bench', 'chest', 'wardrobe', 'shelf', 'bookcase',
  'hearth', 'oven', 'forge', 'anvil', 'workbench', 'counter', 'bar', 'barrel', 'keg_rack', 'crate',
  'sideboard', 'desk', 'display', 'couch', 'pew', 'booth_table', 'booth_seat', 'rug', 'bath', 'bucket',
  'altar', 'statue', 'pillar', 'sarcophagus', 'bell', 'throne', 'stage',
  'vat', 'cauldron', 'alchemy_bench', 'weapon_rack', 'rack', 'cage', 'winch', 'telescope', 'barricade',
  'trapdoor', 'spiral_stair', 'link_down',
  'millstones', 'spur_wheel', 'hoist_trapdoor', 'sack_hoist', 'lighthouse_lamp',
  'throne_dais', 'carpet_runner', 'chair_of_office', 'high_table', 'tapestry', 'wall_banner', 'standing_banner', 'font', 'cloister_well',
  'dance_floor', 'harpsichord', 'musicians_corner', 'refreshment_table', 'low_dais', 'garden_path',
  'god_statue', 'candle_stand', 'carved_screen', 'screen_gate', 'side_altar',
  'strongbox', 'counting_desk', 'job_board', 'stable_stall', 'guild_arms', 'council_table',
  'roof_dome', 'water_jar', 'planter', 'washing_line',
  'yurt_hearth', 'yurt_pallet', 'painted_chest', 'floor_cushion',
]);

/** A yurt's pieces, drawn inside its ring (`Fit`) when they stand in one. */
export const IN_RING = new Set(['yurt_pallet', 'painted_chest', 'floor_cushion']);

/** Underground props drawn here (InteriorLayer: ways in and out, hazards): one of four looks by position;
 * those against the rock wall take its side. */
export const UNDER_DRAWN = new Set([
  'exit', 'up', 'down', 'tunnel', 'ladder', 'pipe', 'well', 'pit', 'trap', 'cave_in',
  'stalagmite', 'rock_column', 'boulder', 'crystal', 'fungus', 'mushroom', 'toadstool', 'shelf_fungus', 'mycelium', 'web', 'cobweb', 'guano', 'moss', 'nest', 'rat_nest', 'ice', 'ice_sheet',
  'obsidian', 'basalt', 'vent', 'sulfur', 'scorched', 'glass_pool',
  'bones', 'skeleton', 'skulls', 'coffin', 'effigy', 'niche', 'urn', 'offering',
  'campfire', 'rubble', 'debris', 'timber', 'rail', 'ore_cart', 'ore_vein', 'tools', 'lantern', 'powder', 'bedroll', 'sacks',
  'brazier', 'candles', 'chains', 'sconce', 'banner', 'iron_maiden', 'stocks', 'hoard', 'dais', 'fountain', 'glyph',
  'collapsed_wall', 'drain', 'grille', 'outfall', 'grate_ladder',
  'throne', 'steps', 'climb_ladder', 'wall_stub', 'fallen_column', 'arcane_workbench', 'arcane_focus', 'bone_mound', 'totem', 'scaffold', 'cracked_floor',
  'flooded_pit',
]);
