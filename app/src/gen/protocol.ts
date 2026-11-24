// Messages between the main thread, the coordinator worker and generator workers,
// plus the terrain payload layout (mirrors crates/worldgen/src/payload.rs).
import type { TileRivers, TileSitePolys } from './tileGeometry';
import type { LinkEdit, PlotStatus, RoofMaterial, RoofShape, RoofStyle, SiteSize, Stance, Tint } from './protocol/edits';
import type { WharfKind } from './protocol/features';

export * from './protocol/world';
export * from './protocol/edits';
export * from './protocol/features';
export * from './protocol/town';
export * from './protocol/build';
export * from './protocol/ask';
export * from './protocol/interior';
export * from './protocol/messages';

/** Widest a castle may be and narrowest (ft), longest a wall (`worldgen::world`), how far past
 * its outline a castle's or wall's layout reaches (`WORKS_MARGIN_FT`). */
export const CASTLE_MAX_FT = 1500;
export const CASTLE_MIN_FT = 80;
export const WALL_MAX_FT = 3000;
/** Longest a closed wall (a ring) may be, ft round (`worldgen::world::WALL_RING_MAX_FT`). */
export const WALL_RING_MAX_FT = 24000;
export const WORKS_MARGIN_FT = 40;
export const ROOFS: RoofStyle[] = ['hip', 'battlements', 'cone'];
export const TINTS: Tint[] = ['terracotta', 'slate', 'thatch', 'shingle', 'moss'];
export const ROOF_MATERIALS: RoofMaterial[] = ['thatch', 'slate', 'clay_tile', 'shingle', 'turf', 'lead', 'copper', 'glazed_tile', 'flat_clay', 'hide', 'plank', 'whitewash', 'stone'];
/** The materials the old tints map to (`roof::TINT_MATERIAL`). */
export const TINT_MATERIAL: Record<Tint, RoofMaterial> = { terracotta: 'clay_tile', slate: 'slate', thatch: 'thatch', shingle: 'shingle', moss: 'turf' };
export const ROOF_MATERIAL_RGB = [0xa88b5c, 0x5c6470, 0x995742, 0x7a6656, 0x7f7b54, 0x7c848c, 0x6a9282, 0x4a718b, 0xb89a72, 0x9c8a6c, 0x86684a, 0xc4baa7, 0x8d8a82];
/** Five colours per material (`Look.shade` 0-4, matte; index 2 is `ROOF_MATERIAL_RGB`, the usual one). Slate
 * keeps two hues (cool grey, warm grey-moss), glaze is dusty (saturation about 0.5), plaster a warm cream. */
export const ROOF_SHADES = [
  [0x86775f, 0x957d58, 0xa88b5c, 0xb59765, 0x765d42], // thatch: silver, dark straw, golden, bright gold, brown
  [0x5f6874, 0x71766a, 0x5c6470, 0x6a6560, 0x55575c], // slate: blue-grey, moss-grey, grey, warm grey, dark
  [0x864c40, 0xb06a44, 0x995742, 0x86614c, 0xac6350], // clay tile: dark brick, orange, red, brown, salmon
  [0x665a50, 0x6e6a5c, 0x7a6656, 0x8a7258, 0x8c8074], // shingle: dark, grey-green, brown, warm, weathered
  [0x6a7c4e, 0x727756, 0x7f7b54, 0x8c8454, 0x6a6a4b], // turf: green, moss, olive, dry, dark
  [0x6c747c, 0x727e86, 0x7c848c, 0x8a8c88, 0x9098a0], // lead: dark, blue, grey, warm, light
  [0x9a6a4c, 0x7c8e68, 0x6a9282, 0x5f877a, 0x7fa192], // copper: bare, olive, verdigris, teal, pale
  [0x4f7868, 0x476486, 0x4a718b, 0x5d8790, 0x675786], // glazed tile: green, deep blue, blue, turquoise, violet
  [0xa8885f, 0xb0a080, 0xb89a72, 0xc4a47c, 0x9a8268], // flat clay: darker, pale, sand, warm, umber
  [0x7c6a52, 0x8a7c64, 0x9c8a6c, 0xaa9a7c, 0x9a7e66], // hide: dark, grey, tan, pale, reddish
  [0x6e5a44, 0x7a6a56, 0x86684a, 0x94744e, 0x8a6a56], // plank: dark, grey, brown, warm, rosy
  [0xb6b3aa, 0xbdb399, 0xc4baa7, 0xcbc1ae, 0xbdac96], // whitewash: cool, cream, lime, bright, sand
  [0x74726c, 0x827e72, 0x8d8a82, 0x9b968a, 0x7a7770], // stone: dark, warm, grey, pale, weathered
];
/** Felt for a round roof (`Roof.shade` 0-4): cream, off-white, grey, warm cream, dusty grey (a felt hall's). */
export const FELT = [0xc9bea6, 0xc4bdad, 0xaea595, 0xc2b394, 0xa39a88];
/** A yurt's felt (`Roof.shade` 0-7, R5 towns 115 `town::yurts::felt`): cream, off-white, grey, warm cream, dusty
 * grey, smoke-browned, cream (with a red band), pale (with a sooty crown). */
export const YURT_FELT = [0xd3c6a6, 0xe0dccf, 0xa9a192, 0xcdb88f, 0xb5ad9d, 0x8a6843, 0xd0c3a2, 0xdcd6c6];
/** A roof material's colour in a variant (0-4; bad values: the usual). */
export const roofRgb = (material: number, shade = 2): number => ROOF_SHADES[material]?.[shade] ?? ROOF_MATERIAL_RGB[material] ?? ROOF_MATERIAL_RGB[3];
export const ROOF_SHAPES: RoofShape[] = ['hip', 'gable', 'cone', 'dome', 'flat', 'round'];
export const MAX_FLOORS = 8;
export const SITE_SIZES: SiteSize[] = ['small', 'medium', 'large', 'huge'];
/** Themes by kind of site underground (`worldgen::under::THEMES`; each kind's first is its usual one). */
export const THEMES: Record<string, string[]> = {
  dungeon: ['dungeon', 'prison', 'temple', 'wizard_lair', 'bandit_hideout', 'dwarven_hall', 'goblin_warren', 'flooded_vault', 'cult_sanctum', 'labyrinth', 'collapsed_fortress', 'sunken_temple'],
  crypt: ['crypt', 'tomb', 'ossuary'],
  catacombs: ['catacombs'],
  cave: ['cave', 'fungal', 'crystal', 'ice', 'flooded', 'beast_den'],
  mine: ['mine'],
  lava_tube: ['lava_tube', 'lava_cave', 'forge', 'fire_temple'],
};
export const STANCES: Stance[] = ['hostile', 'unfriendly', 'neutral', 'friendly', 'allied'];
export const PLOT_STATUSES: PlotStatus[] = ['idea', 'active', 'resolved'];

/** How a link put down by hand goes (`links::WAYS`; the first is the default). */
export const LINK_WAYS = ['stairs', 'ladder', 'passage', 'trapdoor', 'rope', 'portal'] as const;

/** The sites at either end of the paired exits that differ between two edits' `links`. */
export function linkSites(a: Record<string, LinkEdit> | undefined, b: Record<string, LinkEdit> | undefined): Set<string> {
  const out = new Set<string>();
  if (a === b) return out;
  const [pa, pb] = [a ?? {}, b ?? {}];
  const site = (level: string) => level.slice(2, level.lastIndexOf(':'));
  for (const id of new Set([...Object.keys(pa), ...Object.keys(pb)])) {
    if (JSON.stringify(pa[id]) === JSON.stringify(pb[id])) continue;
    for (const l of [pa[id], pb[id]]) if (l) for (const e of [l.a, l.b]) out.add(site(e.level));
  }
  return out;
}

/** Widths (ft) by kind (`WharfKind::widths`) and the width a new one gets. */
export const WHARF_WIDTH: Record<WharfKind, [number, number, number]> = { quay: [10, 60, 20], pier: [6, 30, 10], breakwater: [12, 60, 24] };

/** What a patch of a town can be given (`worldgen::world::TOWN_WARDS`; `empty`: open ground). */
export const TOWN_WARDS = ['plaza', 'castle', 'temple', 'merchant', 'craft', 'noble', 'common', 'slum', 'docks', 'military', 'farm', 'park', 'empty'] as const;
export type TownWard = (typeof TOWN_WARDS)[number];
/** Lot sizes, smallest first (`worldgen::world::LOT_SIZES`). */
export const LOT_SIZES = ['small', 'medium', 'large', 'huge'] as const;
export type LotSize = (typeof LOT_SIZES)[number];
/** A generated block fill's storeys (`worldgen::town::hand::HEIGHTS`): one each, the ward's usual, one more. */
export const HEIGHTS = ['low', 'usual', 'tall'] as const;
export type Height = (typeof HEIGHTS)[number];
/** Squares a corner where streets meet can be given (`worldgen::world::SQUARE_KINDS`). */
export const SQUARE_KINDS = ['crossroads', 'well', 'fountain', 'green'] as const;
export type SquareKind = (typeof SQUARE_KINDS)[number];
/** How a park can be laid out (`worldgen::world::PARK_KINDS`). */
export const PARK_KINDS = ['formal', 'common', 'cemetery', 'horses'] as const;
/** A horse park's marks on the plan (`town::horses::plan_polys`): [rgb, alpha] by the variant. */
export const HORSE_PARK_PLAN: [[number, number, number], number][] = [
  [[0.33, 0.23, 0.14], 0.95],
  [[0.56, 0.56, 0.34], 0.85],
  [[0.38, 0.25, 0.16], 0.9],
  [[0.86, 0.74, 0.4], 0.95],
  [[0.42, 0.31, 0.2], 0.9],
];
export type ParkKind = (typeof PARK_KINDS)[number];

/** Limits on a crossing (`Crossing::WIDTH`, `LENGTH`; a ferry needs room for two 24-ft jetties). */
export const CROSSING_WIDTH: [number, number] = [5, 120];
/** A ford's or a ferry's widest (a bridge goes to `CROSSING_WIDTH[1]`). */
export const NARROW_CROSSING_MAX = 40;
export const CROSSING_LENGTH: [number, number] = [10, 2000];
export const FERRY_MIN_FT = 68;

/** Room kinds with no floor (`interior::OPEN_BELOW`, `ROOF_SLOPE`): a hall's height left open over
 * it (the room below shows through; it stops a step, not the eye) and a roof's slopes. */
export const OPEN_BELOW = 'open to below';
export const ROOF_SLOPE = 'roof';
/** A flat roof walked on inside its parapet (`interior::TERRACE`). */
export const TERRACE = 'roof terrace';
/** Pieces others stand on (`interior::hall::is_platform`): the kinds another piece may overlap (a low dais, a court's gravel walk). */
export const PLATFORMS = new Set(['low_dais', 'garden_path']);

export const TILE_N = 256;
/** Tile levels that draw site layouts (`payload::SITE_MIN_LEVEL`). */
export const SITE_MIN_LEVEL = 9;
export const TILE_SAMPLES = TILE_N + 1;
export const HALO = 4;
export const KIND_TERRAIN = 1;
export const KIND_BATTLEMAP = 2;

export const tileId = (level: number, x: number, y: number) => `${level}/${x}/${y}`;
export const battleId = (level: number, x: number, y: number) => `b/${level}/${x}/${y}`;

export interface TerrainPayload {
  level: number;
  x: number;
  y: number;
  base: number;
  min: number;
  max: number;
  gradRms: number;
  padded: Float32Array;
  tex: Uint16Array;
  biome: Uint8Array;
  rivers: TileRivers;
  roads: TileRivers;
  sitePolys: TileSitePolys;
  siteLines: TileRivers;
  /**
   * Each site line's look (item 104; `payload.rs` "site looks"), in the same layout as `siteLines`, line
   * for line and point for point: (x, y, a, b). A wall's a its style code (`town::wallstyle::WallStyle::code`:
   * group, stone, material), a tower's a its style code with its shape (round, square, D, octagon) and b
   * the way a D faces (an octagon's or a square's sides run), radians; 0, 0 for the rest. Absent from an
   * older payload.
   */
  siteLooks?: TileRivers;
}

const TERRAIN_MAGIC = 0x5445_5252;
const HEADER_BYTES = 48;

export function parseTerrain(buf: ArrayBuffer): TerrainPayload {
  const dv = new DataView(buf);
  if (dv.getUint32(0, true) !== TERRAIN_MAGIC) throw new Error('bad terrain payload');
  const paddedLen = dv.getUint32(32, true);
  const texLen = dv.getUint32(36, true);
  const biomeLen = dv.getUint32(40, true);
  const texOffset = HEADER_BYTES + paddedLen * 4;
  const lines = (offset: number): [TileRivers, number] => {
    const nLines = dv.getUint32(offset, true);
    const nVerts = dv.getUint32(offset + 4, true);
    const startsOffset = offset + 8;
    const vertsOffset = startsOffset + (nLines + 1) * 4;
    return [
      { starts: new Uint32Array(buf, startsOffset, nLines + 1), verts: new Float32Array(buf, vertsOffset, nVerts * 4) },
      vertsOffset + nVerts * 16,
    ];
  };
  const [rivers, roadOffset] = lines(texOffset + texLen * 2 + biomeLen);
  const [roads, siteOffset] = lines(roadOffset);
  const nPolys = dv.getUint32(siteOffset, true);
  const nPolyVerts = dv.getUint32(siteOffset + 4, true);
  const polyStarts = siteOffset + 8;
  const polyAttrs = polyStarts + (nPolys + 1) * 4;
  const polyVerts = polyAttrs + nPolys * 4;
  const sitePolys = {
    starts: new Uint32Array(buf, polyStarts, nPolys + 1),
    attrs: new Uint32Array(buf, polyAttrs, nPolys),
    verts: new Float32Array(buf, polyVerts, nPolyVerts * 2),
  };
  const [siteLines, looksOffset] = lines(polyVerts + nPolyVerts * 8);
  const siteLooks = looksOffset + 8 <= buf.byteLength ? lines(looksOffset)[0] : undefined;
  return {
    level: dv.getUint32(4, true),
    x: dv.getUint32(8, true),
    y: dv.getUint32(12, true),
    base: dv.getFloat32(16, true),
    min: dv.getFloat32(20, true),
    max: dv.getFloat32(24, true),
    gradRms: dv.getFloat32(28, true),
    padded: new Float32Array(buf, HEADER_BYTES, paddedLen),
    tex: new Uint16Array(buf, texOffset, texLen),
    biome: new Uint8Array(buf, texOffset + texLen * 2, biomeLen),
    rivers,
    roads,
    sitePolys,
    siteLines,
    siteLooks,
  };
}

/** IEEE half → float (for CPU-side reads such as elevation under the cursor). */
export function halfToFloat(h: number): number {
  const s = h & 0x8000 ? -1 : 1;
  const e = (h >> 10) & 0x1f;
  const f = h & 0x3ff;
  if (e === 0) return s * f * 2 ** -24;
  if (e === 31) return f ? NaN : s * Infinity;
  return s * (1 + f / 1024) * 2 ** (e - 15);
}