// What the edits hold on a place, where a layout stands, and an upgraded world's edits set
// aside. A world remade from a changed sketch moves its edits with their places in the generator
// (`crates/worldgen/src/t0/remap.rs`: `follow_json`), not here.
import type { Edits, Feature, Npc, Overlay, Plot, Stroke } from '../gen/protocol';
import { LAYOUT_ID_FIELDS, PARKED_LAYOUT } from '../sync/ops';
import { SETTLEMENT_KINDS, SITE_KINDS, WAS_ID } from '../ui/gazetteer';

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
  if (edits.cultures) out.cultures = rekeyed(edits.cultures, true);
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

/** A world's layouts as the app counts them: the settlements, then the world's own sites (their
 * features, in the generator's order), then the created ones from `first` (`t0::remap` counts
 * them so too). */
function layoutsOf(o: Overlay): { list: Feature[]; first: number } {
  const own = (f: Feature) => !f.id.startsWith('c:');
  const settlements = o.features.filter((f) => SETTLEMENT_KINDS.includes(f.kind) && own(f));
  const sites = o.features.filter((f) => SITE_KINDS.includes(f.kind) && own(f));
  return { list: [...settlements, ...sites], first: o.first_created ?? settlements.length + sites.length };
}

/** A layout-local id: the part before its layout number, the number, and what follows it. */
const LOCAL_ID = /^((?:[lrm]:)?[bdtukw]:)(\d+)((?::.*)?)$/;

/** The layout numbers `edits` name places in (below `PARKED_LAYOUT`), ascending. */
function layoutsIn(edits: Edits): number[] {
  const out = new Set<number>();
  eachId(edits, (id) => {
    const m = LOCAL_ID.exec(id);
    if (m && Number(m[2]) < PARKED_LAYOUT) out.add(Number(m[2]));
  });
  return [...out].sort((a, b) => a - b);
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
