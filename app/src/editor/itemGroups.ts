// What the item pickers (`ui/ItemPicker.svelte`) sort things into, and what the renderers can do with
// each kind: turn it, give it another look. Battlemap objects by catalog id; building furniture and
// underground props by kind. A kind not listed here goes in "Other" (the gallery names any), so
// nothing ever drops out of a picker.
import { ORIENTED, UNDER_DRAWN, VARIED } from '../render/furniture';
import { BACKED, ON_WALL, SCATTERED, sharedKind } from '../render/InteriorLayer';

/** Battlemap objects by group (catalog ids), in the order the picker shows them. */
export const BATTLE_GROUPS: [string, number[]][] = [
  ['Trees', [1, 2, 3, 4, 5, 6, 7]],
  ['Plants', [8, 9, 10, 11, 15, 16, 17, 18, 19]],
  ['Rocks', [12, 13, 14, 20, 21, 22, 23, 24]],
  ['Hazards', [25, 26, 27, 28, 29, 30, 31, 32]],
  ['Props', [33, 34, 35, 36, 37, 38, 39, 40, 41]],
  ['Camp', [42, 43, 44, 45]],
];

/** The groups of furniture and props inside, in the order the picker shows them. */
export const INSIDE_GROUPS = [
  'Beds & seating', 'Tables & counters', 'Storage', 'Kitchen & craft', 'Worship & tombs', 'Light & fire', 'Prison & grim',
  'Cave & nature', 'Mine & works', 'Remains & clutter', 'Traps & hazards', 'Ways & fixtures',
];

const BY_GROUP: Record<string, string[]> = {
  'Beds & seating': ['bed', 'cot', 'bedroll', 'bench', 'pew', 'chair', 'couch', 'booth_seat', 'throne', 'dais', 'stage'],
  'Tables & counters': ['table', 'long_table', 'desk', 'booth_table', 'bar', 'counter', 'display', 'sideboard'],
  Storage: ['chest', 'wardrobe', 'shelf', 'bookcase', 'bookshelf', 'barrel', 'keg_rack', 'crate', 'sacks', 'weapon_rack', 'hoard', 'powder'],
  'Kitchen & craft': ['hearth', 'oven', 'forge', 'anvil', 'workbench', 'cauldron', 'alchemy_bench', 'vat', 'bucket', 'bath', 'telescope'],
  'Worship & tombs': ['altar', 'statue', 'sarcophagus', 'urn', 'coffin', 'effigy', 'niche', 'offering', 'glyph', 'bell', 'fountain'],
  'Light & fire': ['brazier', 'candles', 'sconce', 'lantern', 'campfire'],
  'Prison & grim': ['cage', 'chains', 'iron_maiden', 'stocks', 'rack'],
  'Cave & nature': ['stalagmite', 'rock_column', 'boulder', 'crystal', 'fungus', 'mushroom', 'web', 'guano', 'moss', 'pool', 'nest', 'ice', 'ice_sheet', 'basalt', 'obsidian', 'sulfur', 'glass_pool'],
  'Mine & works': ['timber', 'rail', 'ore_cart', 'ore_vein', 'tools', 'winch'],
  'Remains & clutter': ['bones', 'skeleton', 'skulls', 'rubble', 'debris', 'cobweb', 'banner', 'rug', 'rat_nest', 'scorched'],
  'Traps & hazards': ['trap', 'pit', 'cave_in', 'vent', 'lava'],
  'Ways & fixtures': ['exit', 'up', 'down', 'tunnel', 'ladder', 'pipe', 'well', 'trapdoor', 'spiral_stair', 'link_down', 'pillar', 'barricade'],
};

/** Kind (building furniture or underground prop) → its group in the pickers. */
export const ITEM_GROUPS: Record<string, string> = Object.fromEntries(Object.entries(BY_GROUP).flatMap(([g, kinds]) => kinds.map((k) => [k, g])));

/** A kind's group inside ("Other" for one not listed). */
export const itemGroup = (kind: string): string => ITEM_GROUPS[kind] ?? 'Other';

/** Whether a piece inside can be turned to look different: one with a back or a wall side, or
 * one not square (a quarter turn swaps its width and depth). */
export function turnsInside(kind: string, underground: boolean, w = 1, h = 1): boolean {
  if (w !== h) return true;
  const shared = sharedKind(kind, underground);
  return shared ? ORIENTED.has(shared) : ON_WALL.has(kind) || BACKED.has(kind);
}

/** How many looks a piece inside has to choose from (as `InteriorLayer.itemKey` reads them). */
export function looks(kind: string, underground: boolean): number {
  const shared = sharedKind(kind, underground);
  if (shared) return VARIED.has(shared) ? 4 : 1;
  return SCATTERED.has(kind) || UNDER_DRAWN.has(kind) ? 4 : 1;
}
