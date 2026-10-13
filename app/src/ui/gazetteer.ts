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

/** What to call a place no name was found for: its id, or what became of it. */
export const unnamed = (id: string) => (setAside(id) ? 'Set aside when the world was remade' : id);
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
