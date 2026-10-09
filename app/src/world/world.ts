// World files on the main thread: defaults (mirroring crates/worldgen/src/world.rs), the
// shareable URL form, and file import/export.
import type { Created, Edits, WorldFile, WorldParams } from '../gen/protocol';
import { EDIT_FIELDS } from '../sync/ops';
import { assetIds, bundleAssets } from './assets';
import { PINNED } from './versions';

/** Must match `worldgen::world::GEN_VERSION`. */
export const GEN_VERSION = 56;

export const DEFAULT_PARAMS: WorldParams = {
  width_mi: 1200,
  height_mi: 900,
  sea_level_ft: 0,
  max_elev_ft: 14000,
  land_fraction: 0.45,
  ruggedness: 1,
  procedural_mountains: 1,
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
  generated_roads: true,
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

/** Biomes the sketch can paint: the tunable ones and those only ever painted. */
export const PAINT_BIOMES: [string, string][] = [...TUNABLE_BIOMES, ['salt_flat', 'Salt flat'], ['blighted_woods', 'Blighted woods'], ['ashlands', 'Ashlands']];

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

/** A site created again within this of one of its kind (with the same beneath) is that one. */
const SAME_SPOT_FT = 300;

/** What lies beneath a created site, as `world.rs` `Created::under_kind` has it. */
function underOf(c: Created): string | null {
  if (c.kind === 'ruin' || c.kind === 'entrance') return c.under ?? 'dungeon';
  return ['cave', 'mine', 'lava_tube'].includes(c.kind) ? c.kind : null;
}

/** The live site `c` (being created, asked for at `asked`) would duplicate, if any: the same kind
 * and beneath within `SAME_SPOT_FT` of either point, or a building with the same footprint (as
 * `world.rs` `Created::same_site`, which mapd uses). */
export function existingSite(edits: Edits, c: Created, asked: [number, number]): Created | null {
  const near = (o: Created, p: [number, number]) => Math.hypot(o.x - p[0], o.y - p[1]) <= SAME_SPOT_FT;
  const samePoly = (a: [number, number][], b: [number, number][]) => a.length === b.length && a.every((p, i) => Math.abs(p[0] - b[i][0]) <= 2 && Math.abs(p[1] - b[i][1]) <= 2);
  return (
    (edits.created ?? []).find(
      (o) => !o.removed && o.kind === c.kind && (c.kind === 'building' ? samePoly(o.poly ?? [], c.poly ?? []) : underOf(o) === underOf(c) && (near(o, [c.x, c.y]) || near(o, asked))),
    ) ?? null
  );
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

/** Edits longer than this (as JSON) stay out of plain links (`#w=`); the browser keeps them
 * (see `library.ts` `saveEdits`), and exported files carry them. */
const LINK_EDITS_MAX = 4000;
/** Edits longer than this stay out of deflated links (`#z=`) too. */
const DEFLATED_EDITS_MAX = 64 * 1024;
/** A link (after `#`) longer than this is deflated (`#z=`); if it is still longer, the world
 * is kept in this browser and the link only names it (`#lib=`), so the address bar stays short. */
const LINK_MAX = 8 * 1024;

/** The generator version whose edits were stored under `worldKey` alone (before edits were kept
 * per version); `editsKey` falls back to it. */
export const UNVERSIONED_EDITS_GEN = 51;

/** Where a world's edits are kept: by seed and parameters (a redrawn sketch keeps them, as
 * `sameWorld` does) and generator version (another generator puts features elsewhere). */
export function editsKey(w: WorldFile): string {
  return `${worldKey(w)}@${w.gen_version}`;
}

/** A 64-bit FNV-style hash of `text`, as hex. */
function hash64(text: string): string {
  let h1 = 0x811c9dc5;
  let h2 = 0x01000193;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 0x01000193);
    h2 = Math.imul(h2 ^ c, 0x5bd1e995) ^ (h2 >>> 15);
  }
  return (h1 >>> 0).toString(16).padStart(8, '0') + (h2 >>> 0).toString(16).padStart(8, '0');
}

/** Parameters added after worlds were first keyed: part of the key only when changed, so every
 * older world keeps its key (and its edits). */
const LATER_PARAMS = new Set(['procedural_mountains', 'generated_roads']);

/** A key for a world's seed and parameters. */
export function worldKey(w: WorldFile): string {
  const p = { ...DEFAULT_PARAMS, ...w.params } as Record<string, unknown>;
  const d = DEFAULT_PARAMS as unknown as Record<string, unknown>;
  const keys = Object.keys(DEFAULT_PARAMS).filter((k) => !LATER_PARAMS.has(k) || JSON.stringify(p[k]) !== JSON.stringify(d[k]));
  return hash64(JSON.stringify([w.seed >>> 0, keys.map((k) => p[k])]));
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

/** A link's JSON: the world with its parameters compacted, and its edits when given. Characters
 * past ASCII are escaped (still the same JSON), so every build can read it with `atob`. */
function linkJson(w: WorldFile, withEdits: boolean): string {
  const { edits, ...rest } = w;
  const json = JSON.stringify({ ...rest, params: compactParams(w.params), ...(withEdits && edits && Object.keys(edits).length ? { edits } : {}) });
  return json.replace(/[\u007f-\uffff]/g, (c) => '\\u' + c.charCodeAt(0).toString(16).padStart(4, '0'));
}

const b64url = (b64: string) => b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const fromB64url = (s: string) => atob(s.replace(/-/g, '+').replace(/_/g, '/'));

async function deflate(text: string): Promise<string | null> {
  if (typeof CompressionStream === 'undefined') return null;
  const bytes = new Uint8Array(await new Response(new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'))).arrayBuffer());
  let bin = '';
  for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return b64url(btoa(bin));
}

async function inflate(s: string): Promise<string> {
  const bin = fromB64url(s);
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('deflate-raw'))).text();
}

/** The key a world is kept under for `#lib=` links: its seed, parameters and sketch. */
export function fileKey(w: WorldFile): string {
  return hash64(linkJson(w, false));
}

/** The plain link to `w` (`#w=`), the one every build reads (an older generator's build is sent
 * worlds this way): its edits only while short, its sketch however long. */
export function plainHash(w: WorldFile): string {
  return '#w=' + b64url(btoa(linkJson(w, !w.edits || fits(w.edits, LINK_EDITS_MAX))));
}

/**
 * The link to `w`, its edits along while they fit: plain while short, else deflated (`#z=`),
 * else (a big sketch) kept in this browser's library, the link naming it (`#lib=`; the edits
 * are kept by `editsKey` as always). `keep` puts a `#lib` world in the library.
 */
export async function toHash(w: WorldFile, keep: (key: string, w: WorldFile) => Promise<void>): Promise<string> {
  return (await linkFor(w, keep)).hash;
}

/** `toHash`, and whether the link carries the edits. */
export async function linkFor(w: WorldFile, keep: (key: string, w: WorldFile) => Promise<void>): Promise<{ hash: string; edits: boolean }> {
  const edited = !!w.edits && Object.keys(w.edits).length > 0;
  const tries = edited && fits(w.edits, DEFLATED_EDITS_MAX) ? [true, false] : [false];
  let long = '';
  for (const withEdits of tries) {
    const json = linkJson(w, withEdits);
    if (json.length * 1.34 + 3 <= LINK_MAX) {
      const plain = '#w=' + b64url(btoa(json));
      if (plain.length <= LINK_MAX) return { hash: plain, edits: withEdits };
    }
    const z = await deflate(json);
    if (z !== null && z.length + 3 <= LINK_MAX) return { hash: '#z=' + z, edits: withEdits };
    long = z !== null ? '#z=' + z : '#w=' + b64url(btoa(json));
  }
  const key = fileKey(w);
  const { edits: _, ...file } = w;
  try {
    await keep(key, file);
  } catch {
    // Nowhere to keep it (no storage, private mode): the long link, as before.
    return { hash: long, edits: false };
  }
  return { hash: '#lib=' + key, edits: false };
}

/** A short stamp of a world's edits (field by field, as `validate` orders them). */
export function editsStamp(e: Edits | undefined): string {
  return hash64(Object.keys(EDIT_FIELDS).map((f) => JSON.stringify((e as Record<string, unknown> | undefined)?.[f] ?? null)).join('\n'));
}

/** A link to `w` that opens it in any browser (never `#lib=`): plain or deflated, however long
 * the sketch makes it, with its edits while they fit `DEFLATED_EDITS_MAX` (`edits`: they went). */
export async function shareHash(w: WorldFile): Promise<{ hash: string; edits: boolean }> {
  const edits = !!w.edits && Object.keys(w.edits).length > 0 && fits(w.edits, DEFLATED_EDITS_MAX);
  const json = linkJson(w, edits);
  const plain = '#w=' + b64url(btoa(json));
  if (plain.length <= LINK_MAX) return { hash: plain, edits };
  const z = await deflate(json);
  return { hash: z !== null && z.length + 3 < plain.length ? '#z=' + z : plain, edits };
}

/** What a link opens: a world (null if none) and, for a `#lib=` link to a world this browser
 * doesn't keep, `missing`. */
export interface Link {
  world: WorldFile | null;
  missing?: boolean;
}

/** The world `loc` links to (`#w=`, `#z=`, `#lib=`, or `?seed=`); `kept` reads a `#lib` world. */
export async function readLink(loc: Location, kept: (key: string) => Promise<WorldFile | null>): Promise<Link> {
  const m = /[#&](w|z|lib)=([A-Za-z0-9_-]+)/.exec(loc.hash);
  if (m) {
    try {
      if (m[1] === 'lib') {
        const w = await kept(m[2]);
        return w ? { world: validate(w) } : { world: null, missing: true };
      }
      return { world: validate(JSON.parse(m[1] === 'z' ? await inflate(m[2]) : fromB64url(m[2]))) };
    } catch {
      return { world: null };
    }
  }
  const seed = new URLSearchParams(loc.search).get('seed');
  return { world: seed !== null ? newWorld(Number(seed)) : null };
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
