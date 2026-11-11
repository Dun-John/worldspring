// Gazetteer helpers for the UI: feature search, framing zooms, breadcrumbs, names with edits.
import { PARKED_LAYOUT } from '../sync/ops';
import type { Feature, Hit, Overlay } from '../gen/protocol';
import type { IconName } from './icons';

export type BuildingHit = Extract<Hit, { kind: 'building' }>;
export type DistrictHit = Extract<Hit, { kind: 'district' }>;

export type Selection = { kind: 'feature'; feature: Feature } | { kind: 'building'; hit: BuildingHit } | { kind: 'district'; hit: DistrictHit };

export const SETTLEMENT_KINDS = ['metropolis', 'city', 'town', 'village'];

/** A layout-local id set aside when the world was remade (`PARKED_LAYOUT` and up): it names no
 * place any more. */
export function setAside(id: string): boolean {
  const m = /^(?:[lr]:)?[bdtukw]:(\d+)(?::|$)/.exec(id);
  return !!m && Number(m[1]) >= PARKED_LAYOUT;
}

/** A place taken away on purpose (a settlement taken back or removed): what was made on it is
 * kept under `gone:<n>:<its id>`, which names nothing and is never moved to another place. */
export const GONE_ID = /^gone:(\d+):(([a-z_]+):.*)$/;

/** A settlement or site of a world upgraded to a generator that makes them anew (`RENUMBERED`):
 * what was made on it is kept under `was:<version made with>:<its id>`, which names nothing. */
export const WAS_ID = /^was:(\d+):(([a-z_]+):.*)$/;

/** A place no longer on the map whose id keeps what was made on it (taken away on purpose,
 * `gone:`, or made anew by an upgrade, `was:`): its kind, else null. */
export const removedKind = (id: string): string | null => GONE_ID.exec(id)?.[3] ?? WAS_ID.exec(id)?.[3] ?? null;

/** A feature's id (`kind:hash`, a `b` added on a clash), as the generator makes them. */
const FEATURE_ID = /^([a-z_]{2,}):[0-9a-f]+b*$/;
const a = (what: string) => `${/^[aeiou]/.test(what) ? 'an' : 'a'} ${what}`;
const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

/** What to call a place no name was found for: what became of it (taken away, set aside when the
 * world was remade, or a feature the map no longer has), else its id. */
export function unnamed(id: string): string {
  const w = WAS_ID.exec(id);
  if (w) return `${cap(a(kindLabel(w[3])))} of version ${w[1]}`;
  const k = removedKind(id);
  if (k) return `Removed: ${a(kindLabel(k))}`;
  if (setAside(id)) return 'Set aside when the world was remade or upgraded';
  const f = FEATURE_ID.exec(id);
  return f ? `No longer on the map: ${a(kindLabel(f[1]))}` : id;
}

/** A place's name as listed: a place taken away by its name then (`renames`), marked so; else
 * its rename, else `fallback()`. */
export const listedName = (id: string, renames: Record<string, string>, fallback: () => string): string => {
  const w = WAS_ID.exec(id);
  if (w) return renames[id] ? `${renames[id]} (version ${w[1]})` : unnamed(id);
  return removedKind(id) ? (renames[id] ? `Removed: ${renames[id]}` : unnamed(id)) : (renames[id] ?? fallback());
};
/** Kinds of sites (`worldgen::agent::SITE_KINDS`): those not created by hand are the points of
 * interest, whose order gives their layout indices after the settlements'. The last three are
 * only ever created. */
export const SITE_KINDS = ['ruin', 'tower', 'camp', 'waystation', 'cave', 'mine', 'lava_tube', 'entrance', 'building', 'castle', 'wall'];
const REGION_KINDS = ['range', 'forest', 'jungle', 'taiga', 'desert', 'swamp', 'plains', 'tundra', 'glacier', 'blight', 'ashlands', 'region', 'volcano'];

const KIND_LABEL: Record<string, string> = {
  range: 'mountain range',
  salt_lake: 'salt lake',
  salt_flat: 'salt flat',
  waystation: 'roadside inn',
  tower: "wizard's tower",
  entrance: 'way underground',
  blight: 'blighted woods',
};

export function kindLabel(kind: string): string {
  return KIND_LABEL[kind] ?? kind.replace(/_/g, ' ');
}

const KIND_ICON: Record<string, IconName> = {
  ruin: 'ruin',
  tower: 'tower',
  camp: 'tent',
  waystation: 'mug',
  cave: 'cave',
  mine: 'pick',
  lava_tube: 'flame',
  entrance: 'stairs',
  building: 'building',
  castle: 'castle',
  wall: 'wall',
  road: 'road',
  crossing: 'bridge',
};

/** A site's icon (as on the Places tab). */
export function kindIcon(kind: string): IconName {
  return KIND_ICON[kind] ?? 'pin';
}

/** Built-up radius (ft), as `worldgen::town::urban_radius`. */
export function urbanRadius(f: Feature): number {
  const pop = Number(/pop\. ([\d,]+)/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const p = Math.sqrt(pop);
  return f.kind === 'village' ? 150 + 12 * p : f.kind === 'town' ? 11 * p : 12 * p;
}

/** Size (ft) to frame when flying to a feature. */
export function frameSize(f: Feature): number {
  if (SETTLEMENT_KINDS.includes(f.kind)) return urbanRadius(f) * 3.2;
  if (['ruin', 'tower', 'waystation', 'cave', 'mine', 'lava_tube', 'entrance'].includes(f.kind)) return 1500;
  if (f.kind === 'building') return 300;
  return f.extent_ft * 1.4;
}

/** Camera zoom (log2 px per ft) that frames `sizeFt` in a view of the given size. */
export function zoomFor(sizeFt: number, widthPx: number, heightPx: number): number {
  return Math.log2((Math.min(widthPx, heightPx) * 0.8) / Math.max(sizeFt, 40));
}

export type Rect = [number, number, number, number];

/** Whether a feature's spot lies in a rectangle (x0, y0, x1, y1 ft). */
export function featureInRect(f: Feature, r: Rect): boolean {
  return f.x >= r[0] && f.x <= r[2] && f.y >= r[1] && f.y <= r[3];
}

/** Features whose name matches, best first: prefix matches, then word starts, then substrings.
 * With `rect`, only features whose spot is in it. */
export function searchFeatures(overlay: Overlay, q: string, limit = 12, rect?: Rect): Feature[] {
  const s = q.trim().toLowerCase();
  if (!s) return [];
  const scored: [number, Feature][] = [];
  for (const f of overlay.features) {
    if (rect && !featureInRect(f, rect)) continue;
    const n = f.name.toLowerCase();
    const i = n.indexOf(s);
    if (i < 0) continue;
    const wordStart = i === 0 || /\W/.test(n[i - 1]);
    const importance = SETTLEMENT_KINDS.includes(f.kind) ? 2 : ['continent', 'ocean', 'range'].includes(f.kind) ? 1 : 0;
    scored.push([(i === 0 ? 100 : wordStart ? 50 : 0) + importance * 10 - n.length * 0.1, f]);
  }
  scored.sort((a, b) => b[0] - a[0]);
  return scored.slice(0, limit).map(([, f]) => f);
}

/** Something attached to an underground site, its levels or rooms (`undersearch` `attached`). */
export interface UnderAttached {
  what: 'note' | 'npc' | 'plot';
  /** The note's place, the NPC's or plot's id. */
  id: string;
  /** Where on the site: the site, `l:<site>:<level>` or `r:<site>:<level>:<room>`. */
  at: string;
  text?: string;
  name?: string;
}

/** What made an underground site match (`undersearch` `match`). */
export interface UnderMatch {
  what: 'name' | 'level' | 'room' | 'npc' | 'plot' | 'note' | 'kind' | 'theme';
  /** The place to open at (`l:`/`r:`, the site), or the NPC's/plot's id. */
  id: string;
  text: string;
  /** Sewers: the middle of the matched section. */
  x_ft?: number;
  y_ft?: number;
}

/** An underground site found by `GenClient.searchUnder` (`worldgen::undersearch`). */
export interface UnderHit {
  id: string;
  site_kind: string;
  theme: string;
  theme_name: string;
  name: string;
  generated: string;
  levels: number;
  size: string;
  x_ft: number;
  y_ft: number;
  settlement: { id: string; name: string } | null;
  site?: { id: string; name: string };
  /** Under an entrance made in this building's cellar. */
  in_building?: { id: string; name: string | null };
  sections?: number;
  hidden?: boolean;
  distance_ft?: number;
  match: UnderMatch | null;
  matches?: UnderMatch[];
  attached: UnderAttached[];
}

export interface UnderResult {
  items: UnderHit[];
  found: number;
  truncated: boolean;
}

/** The underground kinds, as chips (label, the index's kind name, icon). */
export const UNDER_KINDS: [string, string, IconName][] = [
  ['Dungeon', 'dungeon', 'stairs'],
  ['Crypt', 'crypt', 'tomb'],
  ['Catacombs', 'catacombs', 'skull'],
  ['Cave', 'cave', 'cave'],
  ['Mine', 'mine', 'pick'],
  ['Lava tube', 'lava tube', 'flame'],
  ['Sewer', 'sewer', 'grate'],
  ['Keep dungeon', 'keep dungeon', 'castle'],
];

/** Depth chips: a band of levels (min, max). By depth, not size: levels are what the floor
 * picker shows, the same for every kind, where a size is a grid for a site, fixed for a keep's
 * and counted in grates for sewers. Sites run 1-6 levels, few past 4. */
export const UNDER_DEPTHS: [string, number, number | undefined][] = [
  ['1 level', 1, 1],
  ['2 levels', 2, 2],
  ['3 levels', 3, 3],
  ['4+ levels', 4, undefined],
];

export const underIcon = (kind: string): IconName => UNDER_KINDS.find((k) => k[1] === kind)?.[2] ?? 'cave';

/** Where picking an underground hit opens: the matched level or room (an NPC's or plot's
 * through what it is attached to), else the site itself; a city's sewers (`w:<layout>`) at the
 * matched section, else at the section of their entrance. */
export function underTarget(h: UnderHit): string {
  const m = h.match;
  let at = h.id;
  if (m && (m.what === 'level' || m.what === 'room' || m.what === 'note')) at = m.id;
  else if (m && (m.what === 'npc' || m.what === 'plot')) at = h.attached.find((x) => x.what === m.what && x.id === m.id)?.at ?? h.id;
  if (!/^[lr]:/.test(at) && !/^w:\d+:-?\d+:-?\d+$/.test(at)) at = h.id;
  if (/^w:\d+$/.test(at)) at = `${at}:${Math.floor(h.x_ft / SEWER_SECTION_FT)}:${Math.floor(h.y_ft / SEWER_SECTION_FT)}`;
  return at;
}

/** A sewer section's side, ft (`under::SEWER_SECTION_FT`, `render/Sewers.ts` `SECTION_FT`). */
const SEWER_SECTION_FT = 480;

/** The settlement feature at a settlement's position. */
export function settlementAt(overlay: Overlay, x: number, y: number): Feature | null {
  let best: Feature | null = null;
  let bestD = Infinity;
  for (const f of overlay.features) {
    if (!SETTLEMENT_KINDS.includes(f.kind)) continue;
    const d = Math.hypot(f.x - x, f.y - y);
    if (d < bestD) {
      bestD = d;
      best = f;
    }
  }
  return bestD < 10 ? best : null;
}

export interface Crumb {
  label: string;
  x: number;
  y: number;
  size: number;
}

/** How far (ft) from its centre a place stands in a settlement, as `worldgen::town::holds_radius`. */
export function holdsRadius(f: Feature): number {
  return urbanRadius(f) * (f.kind === 'village' ? 1.4 : 2.15);
}

/** The sites the breadcrumbs may name: those shown on the map, the settlement each stands in, and
 * the one whose inside is open. `version` changes whenever any of these does. */
export interface CrumbSites {
  list: Feature[];
  townOf: (site: Feature) => Feature | null;
  inside: Feature | null;
  /** An inside open that is no site's (a settlement's building, way underground or sewers): no
   * site is named by nearness then. */
  insideOther: boolean;
  version: unknown;
}

/** Features of each kind list, per overlay (the breadcrumbs scan only these). */
const byKinds = new WeakMap<Overlay, Map<string, Feature[]>>();
/** The last breadcrumbs, and the view they were for (the HUD asks ten times a second). */
let lastCrumbs: { overlay: Overlay; key: string; version: unknown; out: Crumb[] } | null = null;

/** Where the view is: landmass › region › settlement, as far as the zoom makes meaningful; close to
 * a site (or inside it), region › its settlement › the site. */
export function breadcrumbs(overlay: Overlay, cx: number, cy: number, ftPerPx: number, sites?: CrumbSites): Crumb[] {
  // The site the view is on (close in), or inside: looked for each time (a site's reach is far
  // smaller than the cells below; the sites are a short list).
  let site = sites?.inside ?? null;
  if (!site && sites && !sites.insideOther && ftPerPx < 40) {
    let bestD = Infinity;
    for (const f of sites.list) {
      const d = Math.hypot(f.x - cx, f.y - cy) / siteReach(f);
      if (d < 1 && d < bestD) {
        bestD = d;
        site = f;
      }
    }
  }
  // The same crumbs until the view moves a tenth of its own width or zooms by a quarter step.
  const cell = ftPerPx * 150;
  const key = `${Math.round(cx / cell)},${Math.round(cy / cell)},${Math.round(Math.log2(ftPerPx) * 4)},${site?.id ?? ''}`;
  if (lastCrumbs && lastCrumbs.overlay === overlay && lastCrumbs.key === key && lastCrumbs.version === sites?.version) return lastCrumbs.out;
  const out: Crumb[] = [];
  let lists = byKinds.get(overlay);
  if (!lists) byKinds.set(overlay, (lists = new Map()));
  const nearest = (kinds: string[], within: (f: Feature) => number) => {
    let best: Feature | null = null;
    let bestD = Infinity;
    const k = kinds.join();
    let list = lists.get(k);
    if (!list) lists.set(k, (list = overlay.features.filter((f) => kinds.includes(f.kind))));
    for (const f of list) {
      const d = Math.hypot(f.x - cx, f.y - cy);
      if (d < within(f) && d / within(f) < bestD) {
        bestD = d / within(f);
        best = f;
      }
    }
    return best;
  };
  const crumb = (f: Feature): Crumb => ({ label: f.name, x: f.x, y: f.y, size: frameSize(f) });
  if (!site) {
    const land = nearest(['continent', 'island'], (f) => f.extent_ft * 0.9);
    if (land) out.push(crumb(land));
  }
  if (ftPerPx < 2500 || site) {
    const region = nearest(REGION_KINDS, (f) => f.extent_ft * 0.6);
    if (region) out.push(crumb(region));
  }
  if (site) {
    // (A site out of town has no settlement crumb.)
    const town = sites!.townOf(site);
    if (town) out.push(crumb(town));
    out.push(crumb(site));
  } else if (ftPerPx < 250) {
    const town = nearest(SETTLEMENT_KINDS, holdsRadius);
    if (town) out.push(crumb(town));
  }
  lastCrumbs = { overlay, key, version: sites?.version, out };
  return out;
}

/** How close (ft) the view's centre must be to a site for the breadcrumbs to name it: half of
 * what flying there frames, at most 750 ft (castles, walls and camps would frame miles). */
function siteReach(f: Feature): number {
  return Math.min(frameSize(f), 1500) / 2;
}

export function buildingName(hit: BuildingHit, renames: Record<string, string>): string {
  return renames[hit.id] ?? hit.name ?? hit.function;
}

export function hitName(hit: BuildingHit | DistrictHit, renames: Record<string, string>): string {
  return hit.kind === 'district' ? (renames[hit.id] ?? hit.name) : buildingName(hit, renames);
}
