// After the world is regenerated from a changed sketch, generated features can move (a
// town placed a few miles away, a range redrawn) and so get new ids. Edits on a feature that
// no longer exists move to the nearest feature of the same kind that took its place. And the
// layout numbers of places shift (more or fewer settlements and sites before them): changes on
// places inside them follow those that come out the same, the rest are set aside.
import type { Edits, Feature, Npc, Overlay, Plot } from '../gen/protocol';
import { LAYOUT_ID_FIELDS, PARKED_LAYOUT } from '../sync/ops';
import { SETTLEMENT_KINDS, SITE_KINDS } from '../ui/gazetteer';

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
 * drawn alike: the same feature (id, kind and place) from the same index and seed (the same
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

/**
 * `edits` with their layout-local ids (`LAYOUT_ID_FIELDS`) following their places from one world
 * (`before`) to the same world remade (`after`): a created site keeps its place among the created
 * ones (it is generated from its id); one of the world's own settlements or sites keeps its keys
 * when `matchLayouts` pairs it and `same` says it was laid out the same (its fingerprint as
 * generated: a road or river redrawn past it lays it out anew), so its buildings, districts, ways
 * down and rooms are the ones the keys name. Every other layout's keys are set aside, at
 * `PARKED_LAYOUT` and up (each layout a number of its own), so none lands on another place.
 * Null if nothing moved.
 */
export function remapLayouts(edits: Edits, before: Overlay, after: Overlay, same: (from: number, to: number) => boolean): { edits: Edits; moved: number; parked: number } | null {
  const [aFirst, bFirst] = [layoutsOf(before).first, layoutsOf(after).first];
  const created = edits.created?.length ?? 0;
  const lis = layoutsIn(edits);
  const matched = matchLayouts(lis, before, after);
  /** Where layout `li`'s keys go: its new number, or null if set aside. */
  const target = (li: number): number | null => {
    if (li >= aFirst) return li < aFirst + created ? bFirst + li - aFirst : null;
    const to = matched.get(li);
    return to !== undefined && same(li, to) ? to : null;
  };
  // (Set aside past what is set aside already, so two remakes never share a number.)
  let next = PARKED_LAYOUT;
  eachId(edits, (id) => {
    const m = LOCAL_ID.exec(id);
    if (m && Number(m[2]) >= PARKED_LAYOUT) next = Math.max(next, Number(m[2]) + 1);
  });
  const to = new Map<number, number>();
  const parkedAt = new Set<number>();
  for (const li of lis) {
    const t = target(li);
    if (t === null) parkedAt.add(next);
    to.set(li, t ?? next++);
  }
  let parked = 0;
  const r = renamed(edits, (id, bare) => {
    const m = bare ? /^()(\d+)()$/.exec(id) : LOCAL_ID.exec(id);
    const li = m ? Number(m[2]) : -1;
    const t = to.get(li);
    if (t === undefined || t === li) return id;
    if (parkedAt.has(t)) parked++;
    return `${m![1]}${t}${m![3]}`;
  });
  return r.n ? { edits: r.edits, moved: r.n - parked, parked } : null;
}
