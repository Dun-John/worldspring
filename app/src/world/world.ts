// World files on the main thread: defaults (mirroring crates/worldgen/src/world.rs), the
// shareable URL form, and file import/export.
import type { Edits, WorldFile, WorldParams } from '../gen/protocol';
import { EDIT_FIELDS } from '../sync/ops';
import { assetIds, bundleAssets } from './assets';
import { PINNED } from './versions';

/** Must match `worldgen::world::GEN_VERSION`. */
export const GEN_VERSION = 52;

export const DEFAULT_PARAMS: WorldParams = {
  width_mi: 1200,
  height_mi: 900,
  sea_level_ft: 0,
  max_elev_ft: 14000,
  land_fraction: 0.45,
  ruggedness: 1,
  plate_count: 14,
  erosion: 1,
  lat_top: 62,
  lat_bottom: 8,
  wind: 'belts',
  temp_offset_c: 0,
  moisture: 1,
  volcanoes: 2,
  river_density: 1,
  settlement_density: 1,
  poi_density: 1,
  biome_weights: {},
};

/** Biomes whose weight can be tuned (names match `t0::biome::Biome::name`). */
export const TUNABLE_BIOMES: [string, string][] = [
  ['temperate_forest', 'Temperate forest'],
  ['temperate_rainforest', 'Rainforest'],
  ['grassland', 'Grassland / plains'],
  ['steppe', 'Steppe'],
  ['savanna', 'Savanna'],
  ['jungle', 'Jungle'],
  ['taiga', 'Taiga'],
  ['tundra', 'Tundra'],
  ['hot_desert', 'Sand desert'],
  ['cold_desert', 'Rocky desert'],
  ['swamp', 'Swamp / marsh'],
  ['alpine', 'Alpine'],
  ['ice', 'Ice / glacier'],
  ['volcanic', 'Volcanic waste'],
];

export function newWorld(seed: number, params: Partial<WorldParams> = {}): WorldFile {
  return { gen_version: GEN_VERSION, seed: seed >>> 0, params: { ...DEFAULT_PARAMS, ...params } };
}

/** Whether two world files describe the same world (seed and parameters; sketch and edits
 * aside: redrawing keeps a world's edits). */
export function sameWorld(a: WorldFile, b: WorldFile): boolean {
  const pa = { ...DEFAULT_PARAMS, ...a.params } as Record<string, unknown>;
  const pb = { ...DEFAULT_PARAMS, ...b.params } as Record<string, unknown>;
  const sorted = (v: unknown) => (v && typeof v === 'object' ? Object.fromEntries(Object.entries(v).sort(([x], [y]) => x.localeCompare(y))) : v);
  return a.seed === b.seed && Object.keys(DEFAULT_PARAMS).every((k) => JSON.stringify(sorted(pa[k])) === JSON.stringify(sorted(pb[k])));
}

export function randomSeed(): number {
  return Math.floor(Math.random() * 2 ** 31);
}

/** Only parameters that differ from the defaults (short share links, readable files). */
export function compactParams(p: Partial<WorldParams>): Partial<WorldParams> {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(p)) {
    const d = (DEFAULT_PARAMS as unknown as Record<string, unknown>)[k];
    if (JSON.stringify(v) !== JSON.stringify(d)) out[k] = v;
  }
  return out as Partial<WorldParams>;
}

/** Edits longer than this (as JSON) stay out of share links; the browser keeps them (see
 * `library.ts` `saveEdits`), and exported files carry them. */
const LINK_EDITS_MAX = 4000;

/** The generator version whose edits were stored under `worldKey` alone (before edits were kept
 * per version); `editsKey` falls back to it. */
export const UNVERSIONED_EDITS_GEN = 51;

/** Where a world's edits are kept: by seed and parameters (a redrawn sketch keeps them, as
 * `sameWorld` does) and generator version (another generator puts features elsewhere). */
export function editsKey(w: WorldFile): string {
  return `${worldKey(w)}@${w.gen_version}`;
}

/** A key for a world's seed and parameters. */
export function worldKey(w: WorldFile): string {
  const p = { ...DEFAULT_PARAMS, ...w.params } as Record<string, unknown>;
  const text = JSON.stringify([w.seed >>> 0, Object.keys(DEFAULT_PARAMS).map((k) => p[k])]);
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 0x01000193);
    h2 = Math.imul(h2 ^ c, 0x5bd1e995) ^ (h2 >>> 15);
  }
  return (h1 >>> 0).toString(16).padStart(8, '0') + (h2 >>> 0).toString(16).padStart(8, '0');
}

/** Whether `v` as JSON is at most `max` characters, measured entry by entry (a big world's edits
 * are not written out whole just to find they don't fit). */
function fits(v: unknown, max: number): boolean {
  let n = 0;
  const walk = (x: unknown): boolean => {
    if (x && typeof x === 'object') {
      for (const e of Object.values(x)) if (!walk(e)) return false;
      return true;
    }
    n += JSON.stringify(x ?? null).length + 4;
    return n <= max;
  };
  return walk(v) && JSON.stringify(v).length <= max;
}

export function toHash(w: WorldFile): string {
  const small = !w.edits || fits(w.edits, LINK_EDITS_MAX);
  const { edits, ...rest } = w;
  const json = JSON.stringify({ ...rest, params: compactParams(w.params), ...(small && edits && Object.keys(edits).length ? { edits } : {}) });
  return '#w=' + btoa(json).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

export function fromLocation(loc: Location): WorldFile | null {
  const m = /[#&]w=([A-Za-z0-9_-]+)/.exec(loc.hash);
  if (m) {
    try {
      const json = atob(m[1].replace(/-/g, '+').replace(/_/g, '/'));
      return validate(JSON.parse(json));
    } catch {
      return null;
    }
  }
  const seed = new URLSearchParams(loc.search).get('seed');
  return seed !== null ? newWorld(Number(seed)) : null;
}

export function validate(v: unknown): WorldFile {
  const w = v as WorldFile;
  if (!w || typeof w.seed !== 'number' || typeof w.params !== 'object') throw new Error('Not a world file');
  // Older files keep their version: the app asks whether to open them as they were made (in
  // that generator's build) or upgrade them. Newer files cannot be read, except by an older
  // generator's build, which sends them on to the newest.
  if (typeof w.gen_version !== 'number' || !Number.isInteger(w.gen_version) || w.gen_version < 1 || (w.gen_version > GEN_VERSION && !PINNED)) {
    throw new Error(`World file is for generator v${w.gen_version}; this app is v${GEN_VERSION}`);
  }
  const e = w.edits && typeof w.edits === 'object' ? (w.edits as Record<string, unknown>) : undefined;
  // Only the known fields, each the shape it should be.
  const edits = e
    ? (Object.fromEntries(
        Object.entries(EDIT_FIELDS)
          .filter(([f, shape]) => (shape === 'map' ? e[f] && typeof e[f] === 'object' && !Array.isArray(e[f]) : Array.isArray(e[f])))
          .map(([f]) => [f, structuredClone(e[f])]),
      ) as Edits)
    : undefined;
  const strokes = Array.isArray(w.sketch?.strokes) ? w.sketch.strokes.filter((s) => s && typeof s.tool === 'string' && Array.isArray(s.pts) && s.pts.length > 0) : [];
  return {
    gen_version: w.gen_version,
    seed: w.seed >>> 0,
    params: { ...DEFAULT_PARAMS, ...w.params },
    ...(strokes.length ? { sketch: { strokes: strokes.map((s) => ({ ...s, pts: s.pts.map((p) => [p[0], p[1]] as [number, number]) })) } } : {}),
    ...(edits && Object.keys(edits).length ? { edits } : {}),
  };
}

/** Save a world file, with the pictures its edits refer to (`assets`: id → data URL). */
export async function download(w: WorldFile, name: string) {
  const assets = await bundleAssets(assetIds(w.edits)).catch(() => ({}));
  const file = { ...w, params: compactParams(w.params), ...(Object.keys(assets).length ? { assets } : {}) };
  const blob = new Blob([JSON.stringify(file, null, 2)], { type: 'application/json' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = `${name.replace(/[^\w-]+/g, '_') || 'world'}.world.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}
