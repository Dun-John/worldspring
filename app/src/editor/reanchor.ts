// After the world is regenerated from a changed sketch, generated features can move (a
// town placed a few miles away, a range redrawn) and so get new ids. Edits on a feature that
// no longer exists move to the nearest feature of the same kind that took its place. And the
// layout numbers of places shift (more or fewer settlements and sites before them): changes on
// places inside them follow those that come out the same, the rest are set aside.
import type { Edits, Feature, Npc, Overlay, Plot, Stroke } from '../gen/protocol';
import { LAYOUT_ID_FIELDS, PARKED_LAYOUT } from '../sync/ops';
import { GONE_ID, SETTLEMENT_KINDS, SITE_KINDS, WAS_ID } from '../ui/gazetteer';

/** How far a feature may have moved and still be the same one (ft): 40 mi, or more for big
 * ones (a range's or forest's anchor sits mid-way along it and moves as its shape does). */
const reachFor = (f: Feature) => Math.max(40 * 5280, 0.6 * f.extent_ft);

/** Ids of buildings, districts, wall towers, sites underground and created sites: not moved. */
const LOCAL = /^[bdtuwkc]:/;

/** Every id `edits` hold where a place is named (`LAYOUT_ID_FIELDS`: keys, a set's ids, where NPCs
 * are, plot points' places; a town's bare layout number as `t:<n>`, which no other id is). */
function eachId(edits: Edits, f: (id: string) => void) {
  for (const [field, where] of Object.entries(LAYOUT_ID_FIELDS)) {
    const v = (edits as Record<string, unknown>)[field];
    if (!v) continue;
    if (where === 'keys' || where === 'towns') for (const k of Object.keys(v)) f(where === 'towns' ? `t:${k}` : k);
    else if (where === 'values') for (const k of v as string[]) f(k);
    else if (where === 'npcs') for (const n of Object.values(v as Record<string, Npc>)) n.location && f(n.location.id);
    else for (const p of Object.values(v as Record<string, Plot>)) for (const k of p.anchors ?? []) f(k);
  }
}

/** `edits` with every id `rename` changes changed (bare town keys in `towns` as `bare`), and how
 * many entries changed. */
function renamed(edits: Edits, rename: (id: string, bare?: boolean) => string): { edits: Edits; n: number } {
  let n = 0;
  const to = (id: string, bare = false) => {
    const r = rename(id, bare);
    if (r !== id) n++;
    return r;
  };
  const rekeyed = <T>(r: Record<string, T>, bare = false) => Object.fromEntries(Object.entries(r).map(([k, v]) => [to(k, bare), v]));
  const out: Edits = { ...edits };
  if (edits.renames) out.renames = rekeyed(edits.renames);
  if (edits.notes) out.notes = rekeyed(edits.notes);
  if (edits.hidden) out.hidden = [...new Set(edits.hidden.map((k) => to(k)))].sort();
  if (edits.designs) out.designs = rekeyed(edits.designs);
  if (edits.buildings) out.buildings = rekeyed(edits.buildings);
  if (edits.towns) out.towns = rekeyed(edits.towns, true);
  if (edits.npcs)
    out.npcs = Object.fromEntries(
      Object.entries(edits.npcs).map(([k, v]) => {
        const id = v.location && to(v.location.id);
        return [k, v.location && id !== v.location.id ? { ...v, location: { ...v.location, id: id! } } : v];
      }),
    );
  if (edits.plots)
    out.plots = Object.fromEntries(
      Object.entries(edits.plots).map(([k, p]) => {
        const anchors = (p.anchors ?? []).map((a) => to(a));
        return [k, anchors.some((x, i) => x !== p.anchors[i]) ? { ...p, anchors } : p];
      }),
    );
  return { edits: out, n };
}

/** `edits` with ids of vanished features moved to their nearest same-kind successors (in
 * `after`, by their positions in `before`): names, notes and hidden marks, where NPCs are and
 * plot points' places. What moved (features) and how many entries; null if nothing did. */
export function reanchor(edits: Edits, before: Feature[], after: Feature[]): { edits: Edits; moved: [string, string][]; n: number } | null {
  const now = new Set(after.map((f) => f.id));
  const was = new Map(before.map((f) => [f.id, f]));
  const keyed = new Set<string>();
  eachId(edits, (id) => keyed.add(id));
  const taken = new Set([...keyed].filter((id) => now.has(id)));
  const moved: [string, string][] = [];
  for (const id of keyed) {
    if (now.has(id) || LOCAL.test(id)) continue;
    const old = was.get(id);
    if (!old) continue;
    let best: Feature | null = null;
    let bestD = reachFor(old);
    for (const f of after) {
      if (f.kind !== old.kind || taken.has(f.id)) continue;
      const d = Math.hypot(f.x - old.x, f.y - old.y);
      if (d < bestD) [best, bestD] = [f, d];
    }
    if (best) {
      taken.add(best.id);
      moved.push([id, best.id]);
    }
  }
  if (!moved.length) return null;
  const to = new Map(moved);
  // (A town's bare layout number is no feature id.)
  const r = renamed(edits, (id, bare) => (bare ? id : (to.get(id) ?? id)));
  return { edits: r.edits, moved, n: r.n };
}

/** What `edits` hold on settlement `id` (layout `li`, -1 for none): on its id, or on a layout-local
 * id in its layout (its buildings, districts, towers, ways down, levels and rooms; its streets
 * re-planned). Not the sites standing in it: they stay. */
export interface MadeOn {
  notes: number;
  names: number;
  npcs: number;
  plots: number;
  designs: number;
  buildings: number;
  towns: number;
  hidden: number;
  renamed: boolean;
}

export function madeOn(edits: Edits, id: string, li: number): MadeOn {
  const on = (k: string) => k === id || (li >= 0 && Number(LOCAL_ID.exec(k)?.[2] ?? -1) === li);
  const keys = (r?: object) => Object.keys(r ?? {});
  return {
    notes: keys(edits.notes).filter(on).length,
    names: keys(edits.renames).filter((k) => k !== id && on(k)).length,
    renamed: !!edits.renames?.[id],
    hidden: (edits.hidden ?? []).filter(on).length,
    npcs: Object.values(edits.npcs ?? {}).filter((n) => n.location && on(n.location.id)).length,
    plots: Object.values(edits.plots ?? {}).filter((p) => (p.anchors ?? []).some(on)).length,
    designs: keys(edits.designs).filter(on).length,
    buildings: keys(edits.buildings).filter(on).length,
    towns: li >= 0 && edits.towns?.[String(li)] ? 1 : 0,
  };
}

/** The prep worth asking about (not the place's own name or hidden mark). */
export const madeCount = (m: MadeOn) => m.notes + m.names + m.npcs + m.plots + m.designs + m.buildings + m.towns;

/** `edits` with the ids of places taken away on purpose (`removed`, gone from `after`) set aside
 * at `gone:<n>:<id>` (n past those in use, so a place taken away twice keeps both): notes, NPCs'
 * places and plot points' places. Its hidden mark goes (nothing to hide). Its name stays (as
 * renamed, else the name it had) when anything is left on it, so the entries say what they were
 * on; else it goes too. How many entries were set aside; null if nothing was. */
export function setAsideRemoved(edits: Edits, removed: { id: string; name: string }[], after: Feature[]): { edits: Edits; n: number } | null {
  const now = new Set(after.map((f) => f.id));
  // (Still on the map: nothing to set aside.)
  const gone = removed.filter((r) => !now.has(r.id));
  if (!gone.length) return null;
  let k = 1;
  eachId(edits, (id) => {
    const m = GONE_ID.exec(id);
    if (m) k = Math.max(k, Number(m[1]) + 1);
  });
  const to = new Map(gone.map((r) => [r.id, `gone:${k++}:${r.id}`]));
  const out = renamed(edits, (id, bare) => (bare ? id : (to.get(id) ?? id))).edits;
  const ids = new Set(to.values());
  if (out.hidden) out.hidden = out.hidden.filter((h) => !ids.has(h));
  if (out.hidden && !out.hidden.length) delete out.hidden;
  let n = 0;
  const renames = { ...(out.renames ?? {}) };
  for (const r of gone) {
    const g = to.get(r.id)!;
    const used = (out.notes?.[g] ? 1 : 0) + Object.values(out.npcs ?? {}).filter((x) => x.location?.id === g).length + Object.values(out.plots ?? {}).filter((p) => p.anchors.includes(g)).length;
    n += used;
    if (used) renames[g] ??= r.name;
    else delete renames[g];
  }
  out.renames = renames;
  if (!Object.keys(renames).length) delete out.renames;
  return n || edits.hidden?.some((h) => to.has(h)) || gone.some((r) => edits.renames?.[r.id]) ? { edits: out, n } : null;
}

/** A world's layouts as the app counts them: the settlements, then the world's own sites (their
 * features, in the generator's order), then the created ones from `first`; and what each is drawn
 * from (`Overlay.layout_ids`). */
interface Layouts {
  list: Feature[];
  first: number;
  ids: number[] | undefined;
}

function layoutsOf(o: Overlay): Layouts {
  const settlements = o.features.filter((f) => SETTLEMENT_KINDS.includes(f.kind));
  const sites = o.features.filter((f) => SITE_KINDS.includes(f.kind) && !f.id.startsWith('c:'));
  return { list: [...settlements, ...sites], first: o.first_created ?? settlements.length + sites.length, ids: o.layout_ids };
}

/** A layout-local id: the part before its layout number, the number, and what follows it. */
const LOCAL_ID = /^((?:[lr]:)?[bdtukw]:)(\d+)((?::.*)?)$/;

/** The layout numbers `edits` name places in (below `PARKED_LAYOUT`), ascending. */
export function layoutsIn(edits: Edits): number[] {
  const out = new Set<number>();
  eachId(edits, (id) => {
    const m = LOCAL_ID.exec(id);
    if (m && Number(m[2]) < PARKED_LAYOUT) out.add(Number(m[2]));
  });
  return [...out].sort((a, b) => a - b);
}

/** The world's own layouts among `lis` (in `before`) whose place the remade world (`after`) has
 * drawn alike: the same feature (id, kind and place) from the same seed (the same
 * `layout_ids` entry). Old layout number → new; the others aren't in it. */
export function matchLayouts(lis: number[], before: Overlay, after: Overlay): Map<number, number> {
  const [a, b] = [layoutsOf(before), layoutsOf(after)];
  const out = new Map<number, number>();
  // (Counted otherwise than the generator counts them, the world's own places can't be matched.)
  if (a.list.length !== a.first || b.list.length !== b.first) {
    console.warn(`[remake] the map counts ${a.list.length} and ${b.list.length} of the world's own places, the generator ${a.first} and ${b.first}: their changes are set aside`);
    return out;
  }
  const index = new Map(b.list.map((f, i) => [f.id, i]));
  for (const li of lis) {
    if (li >= a.first) continue;
    const f = a.list[li];
    const i = index.get(f.id);
    const g = i === undefined ? null : b.list[i];
    if (!g || g.kind !== f.kind || Math.abs(g.x - f.x) > 1 || Math.abs(g.y - f.y) > 1) continue;
    // (Without the generator's word on how they are drawn, only those whose number is the same.)
    const drawn = a.ids && b.ids ? a.ids[li] !== undefined && a.ids[li] === b.ids[i!] : li === i;
    if (drawn) out.set(li, i!);
  }
  return out;
}

/** Where the places of layouts `lis` go from one world (`before`) to the same world remade
 * (`after`): a created site (one of `created`) keeps its place among the created ones; one of
 * the world's own its new number when `matchLayouts` pairs it and `same` says it was laid out
 * the same. Null: it came out differently, or is gone. */
export function layoutTargets(lis: number[], before: Overlay, after: Overlay, created: number, same: (from: number, to: number) => boolean): (li: number) => number | null {
  const [aFirst, bFirst] = [layoutsOf(before).first, layoutsOf(after).first];
  const matched = matchLayouts(lis, before, after);
  return (li) => {
    if (li >= aFirst) return li < aFirst + created ? bFirst + li - aFirst : null;
    const to = matched.get(li);
    return to !== undefined && same(li, to) ? to : null;
  };
}

/** A layout-local id's layout number (null: the surface, or no such id). */
export function layoutOfId(id: string): number | null {
  const m = LOCAL_ID.exec(id);
  return m ? Number(m[2]) : null;
}

/** A layout-local id with its layout number replaced. */
export function withLayout(id: string, li: number): string {
  const m = LOCAL_ID.exec(id)!;
  return `${m[1]}${li}${m[3]}`;
}

/** Where a layout's place stands in a world (world ft): its feature, or a created one's entry
 * (`edits.created`); null if there is none. */
export function layoutAt(o: Overlay, edits: Edits, li: number): [number, number] | null {
  const l = layoutsOf(o);
  const f = li < l.first ? l.list[li] : edits.created?.[li - l.first];
  return f ? [f.x, f.y] : null;
}

/**
 * `edits` with their layout-local ids (`LAYOUT_ID_FIELDS`) following their places from one world
 * (`before`) to the same world remade (`after`): a created site keeps its place among the created
 * ones (it is generated from its id); one of the world's own settlements or sites keeps its keys
 * when `matchLayouts` pairs it and `same` says it was laid out the same (its fingerprint as
 * generated: a road or river redrawn past it lays it out anew), so its buildings, districts, ways
 * down and rooms are the ones the keys name. Every other layout's keys are set aside, at
 * `PARKED_LAYOUT` and up (each layout a number of its own), so none lands on another place.
 * The keys of layouts taken away on purpose (`removed`) are set aside too, counted apart
 * (`removed`, not `parked`). Null if nothing moved.
 */
export function remapLayouts(
  edits: Edits,
  before: Overlay,
  after: Overlay,
  same: (from: number, to: number) => boolean,
  removed: number[] = [],
): { edits: Edits; moved: number; parked: number; removed: number } | null {
  const lis = layoutsIn(edits);
  const target = layoutTargets(lis, before, after, edits.created?.length ?? 0, same);
  // (Set aside past what is set aside already, so two remakes never share a number.)
  let next = PARKED_LAYOUT;
  eachId(edits, (id) => {
    const m = LOCAL_ID.exec(id);
    if (m && Number(m[2]) >= PARKED_LAYOUT) next = Math.max(next, Number(m[2]) + 1);
  });
  const to = new Map<number, number>();
  const parkedAt = new Set<number>();
  const removedAt = new Set<number>();
  for (const li of lis) {
    const t = removed.includes(li) ? null : target(li);
    if (t === null) (removed.includes(li) ? removedAt : parkedAt).add(next);
    to.set(li, t ?? next++);
  }
  let parked = 0;
  let gone = 0;
  const r = renamed(edits, (id, bare) => {
    const m = bare ? /^()(\d+)()$/.exec(id) : LOCAL_ID.exec(id);
    const li = m ? Number(m[2]) : -1;
    const t = to.get(li);
    if (t === undefined || t === li) return id;
    if (parkedAt.has(t)) parked++;
    else if (removedAt.has(t)) gone++;
    return `${m![1]}${t}${m![3]}`;
  });
  return r.n ? { edits: r.edits, moved: r.n - parked - gone, parked, removed: gone } : null;
}

/** A settlement's or site's feature id, as the generator makes them (`kind:hash`). */
const PLACE_ID = /^([a-z_]{2,}):[0-9a-f]+b*$/;
const madeAnew = (id: string) => {
  const m = PLACE_ID.exec(id);
  return !!m && (SETTLEMENT_KINDS.includes(m[1]) || SITE_KINDS.includes(m[1]));
};

/**
 * Edits made with generator `from`, for a world upgraded past a generator that makes every town
 * and site anew (`crossesRenumber`): every layout-local id set aside at `PARKED_LAYOUT` and up (a
 * number for each layout: its buildings, rooms and dungeons are others now), and every
 * settlement's and site's feature id at `was:<from>:<id>` (its id may name another place now):
 * names, notes, hidden marks, where NPCs are, plot points' places. Natural features, created
 * places (`c:`), roads and regions keep theirs, and so does everything kept by world position.
 * How many entries were set aside; null if none.
 */
export function upgradeEdits(edits: Edits, from: number): { edits: Edits; n: number } | null {
  let next = PARKED_LAYOUT;
  eachId(edits, (id) => {
    const m = LOCAL_ID.exec(id);
    if (m && Number(m[2]) >= PARKED_LAYOUT) next = Math.max(next, Number(m[2]) + 1);
  });
  const to = new Map(layoutsIn(edits).map((li) => [li, next++]));
  const r = renamed(edits, (id, bare) => {
    const m = bare ? /^()(\d+)()$/.exec(id) : LOCAL_ID.exec(id);
    const t = m ? to.get(Number(m[2])) : undefined;
    if (t !== undefined) return `${m![1]}${t}${m![3]}`;
    return !bare && madeAnew(id) ? `was:${from}:${id}` : id;
  });
  return r.n ? r : null;
}

/** How far a settlement or site drawn in the sketch may stand from where it was drawn (ft). */
const PIN_REACH = 5280;

/** After an upgrade (`upgradeEdits`), the settlements and sites drawn in the sketch (`strokes`'
 * pins and sites) take back what was made on them: they stand where they were drawn in both
 * generators, so an id of one that the upgraded world has again (`after`, within a mile of a pin
 * or site drawn) is the same place. How many entries came back; null if none. */
export function restorePinned(edits: Edits, after: Feature[], strokes: Stroke[]): { edits: Edits; n: number } | null {
  const drawn = strokes.filter((s) => (s.tool === 'pin' || s.tool === 'site') && s.pts.length).map((s) => s.pts[0]);
  if (!drawn.length) return null;
  const at = new Map(after.map((f) => [f.id, f]));
  const r = renamed(edits, (id, bare) => {
    const m = bare ? null : WAS_ID.exec(id);
    const f = m && at.get(m[2]);
    return f && drawn.some(([x, y]) => Math.hypot(f.x - x, f.y - y) <= PIN_REACH) ? m![2] : id;
  });
  return r.n ? r : null;
}
