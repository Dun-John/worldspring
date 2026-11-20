// What an edit redraws: the pure part of `MapView.setEdits`. From the edits before and after (and
// the keys each field's changes touch), the areas of tiles, battlemaps and ground each changed field
// reaches, and the ids whose labels, roads or fences change. MapView does the rest (labels, the
// towns' hints and reach, the generators).

import { type Clear, type Created, type Edits, type Placed, type Rect, WORKS_MARGIN_FT } from '../gen/protocol';
import { curve as pathCurve, pathWidth } from '../editor/paths';
import { BLOCK, parseKey } from '../editor/groundCodec';
import { shore } from '../editor/water';
import { fenceBox, fenceId } from '../editor/fence';

/** How far a created site's layout reaches (`town::sites::SITE_REACH_FT`), with a margin. */
const SITE_REACH_FT = 260 + 100;

/** How far (ft) from its point a created site's layout reaches, with a margin (as
 * `worldgen::world::Created::reach`: a castle's, wall's or plot's farthest corner and its towers). */
export function siteReach(c: Created): number {
  const pts = c.kind === 'castle' || c.kind === 'plot' ? c.poly : c.kind === 'wall' ? c.pts : null;
  if (!pts?.length) return SITE_REACH_FT;
  return Math.max(...pts.map((p) => Math.hypot(p[0] - c.x, p[1] - c.y))) + WORKS_MARGIN_FT + 100;
}

/** Created kinds that are settlements added by hand (`world::CREATED_SETTLEMENTS`). */
export const isMade = (c: Created) => c.kind === 'village' || c.kind === 'town';

/** What a created site's layout depends on (not its name, but a drawn building's: it is the
 * building's own). */
const siteKey = (c: Created) => JSON.stringify({ ...c, name: c.kind === 'building' || c.kind === 'castle' ? c.name : null });

/** The ids in a keyed field that may have changed: the keys its ops touch (`changed`), else every key of either. */
export const keyedIds = (changed: Record<string, string[]> | undefined, field: string, a: object, b: object): Iterable<string> =>
  changed?.[field] ?? (changed ? [] : new Set([...Object.keys(a), ...Object.keys(b)]));

/** What one change of the edits redraws, as far as the edits alone say. */
export interface EditDirty {
  /** `Edits.created` changed at all. */
  createdChanged: boolean;
  /** The site tiles round created sites changed (not villages and towns added by hand: `made`). */
  createdRects: Rect[];
  /** Indices into `created` of castles whose pieces changed. */
  castles: number[];
  /** A village or town added by hand changed. */
  made: boolean;
  /** Labels to rename (`[id, name or null]`), in order. */
  relabel: [string, string | null][];
  /** A name drawn inside (levels, rooms, sites) changed. */
  renamedInside: boolean;
  /** A road's name changed, or a road was hidden or shown. */
  roadNames: boolean;
  /** Battlemap areas of objects put down, taken away and cleared, and of sprites whose rules changed. */
  battleRects: Rect[];
  /** Objects put down or changed (as they are now), in order. */
  put: Placed[];
  /** Site tile areas along crossings, drawn roads and wharves. */
  worksRects: Rect[];
  /** Areas of ground changed: brushed blocks (one box), plateaus, water drawn by hand. */
  ground: Rect[];
  /** The world's islets (`i:`) taken away or brought back. */
  islets: string[];
  /** Site tile areas round the world's own buildings changed. */
  buildingRects: Rect[];
  /** Those buildings' ids. */
  replaced: string[];
  /** Generated fence runs changed in a town edit that changed nothing else (`fenceId`). */
  fenceIds: string[];
  /** Town edits (`Edits.towns` keys) that changed more than their fences. */
  replanned: string[];
  /** Towns' members changed: the areas of the created sites joined or left, and the towns touched; null if unchanged. */
  members: { rects: Rect[]; towns: Set<number> } | null;
  /** Towns laid out by hand as streets whose records changed (`<li>`); null if unchanged. */
  streets: Set<string> | null;
  /** Settlements laid out in another culture (`Edits.cultures` keys) changed. */
  cultures: string[];
  /** Roads taken off or brought back. */
  cutIds: string[];
  /** Battlemap areas round fences drawn by hand and gaps cut. */
  fenceRects: Rect[];
}

/** What changing the edits from `prev` to `next` redraws (`changed`: the keys each field's changes touch). */
export function editDirty(prev: Edits, next: Edits, changed?: Record<string, string[]>): EditDirty {
  const keys = (field: string, a: object, b: object) => keyedIds(changed, field, a, b);
  const before = prev.created ?? [];
  const after = next.created ?? [];
  const createdRects: Rect[] = [];
  const castles: number[] = [];
  let made = false;
  for (let k = 0; k < Math.max(before.length, after.length); k++) {
    const a = before[k];
    const b = after[k];
    if (a && b && siteKey(a) === siteKey(b)) continue;
    if ((a?.kind === 'castle' || b?.kind === 'castle') && JSON.stringify(a?.pieces ?? null) !== JSON.stringify(b?.pieces ?? null)) castles.push(k);
    for (const c of [a, b]) {
      if (!c) continue;
      // (A village or town added by hand: what it redraws comes from the generator, its pad and
      // track as it has them, once it has the edits: `refreshMade`.)
      if (isMade(c)) {
        made = true;
        continue;
      }
      const r = siteReach(c);
      createdRects.push([c.x - r, c.y - r, c.x + r, c.y + r]);
    }
  }
  const createdChanged = JSON.stringify(before) !== JSON.stringify(after);
  // Changed names, and created sites' (their labels are new).
  const pr = prev.renames ?? {};
  const nr = next.renames ?? {};
  const relabel: [string, string | null][] = [];
  // (Only names drawn inside, levels, rooms and sites, redraw the building or site we are in.)
  let renamedInside = false;
  // (Roads someone named, renamed or hidden: their labels asked for again once these are sent.)
  let roadNames = prev.hidden !== next.hidden && [...(prev.hidden ?? []), ...(next.hidden ?? [])].some((h) => h.startsWith('rd:'));
  // (Fields that are the same object are unchanged: edits are never changed in place.)
  if (pr !== nr || createdChanged) for (const id of createdChanged ? new Set([...Object.keys(pr), ...Object.keys(nr)]) : keys('renames', pr, nr)) {
    if (pr[id] !== nr[id] && /^[lrbtuwk]:/.test(id)) renamedInside = true;
    if (pr[id] !== nr[id] || (createdChanged && id.startsWith('c:'))) relabel.push([id, nr[id] ?? null]);
    if (pr[id] !== nr[id] && id.startsWith('rd:')) roadNames = true;
  }
  // Objects put down or taken away: only the battlemaps holding them are made again.
  const battleRects: Rect[] = [];
  const put: Placed[] = [];
  const po = prev.objects ?? {};
  const no = next.objects ?? {};
  if (po !== no) for (const id of keys('objects', po, no)) {
    const [a, b] = [po[id], no[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const o of [a, b]) if (o) battleRects.push([o.x - 1, o.y - 1, o.x + 1, o.y + 1]);
    if (b) put.push(b);
  }
  const pc = prev.cleared ?? {};
  const nc = next.cleared ?? {};
  const reach = (c: Clear) => (c.r ?? 1.25) + 1;
  if (pc !== nc) for (const id of keys('cleared', pc, nc)) {
    const [a, b] = [pc[id], nc[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const c of [a, b]) if (c) battleRects.push([c.x - reach(c), c.y - reach(c), c.x + reach(c), c.y + reach(c)]);
  }
  // A sprite's rules changed: every chunk using it.
  const ps = prev.sprites ?? {};
  const ns = next.sprites ?? {};
  if (ps !== ns) for (const asset of new Set([...Object.keys(ps), ...Object.keys(ns)])) {
    if (JSON.stringify(ps[asset]) === JSON.stringify(ns[asset])) continue;
    for (const o of Object.values(no)) if (o.kind === `s:${asset}`) battleRects.push([o.x - 1, o.y - 1, o.x + 1, o.y + 1]);
  }
  // Crossings put down by hand: the battlemaps and site tiles along them (decks are drawn on
  // both; a ferry's jetties reach past its ends).
  const worksRects: Rect[] = [];
  const pv = prev.crossings ?? {};
  const nv = next.crossings ?? {};
  if (pv !== nv) for (const id of keys('crossings', pv, nv)) {
    const [a, b] = [pv[id], nv[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const c of [a, b]) {
      if (!c) continue;
      const r = c.width / 2 + 40;
      const box: Rect = [Math.min(c.a[0], c.b[0]) - r, Math.min(c.a[1], c.b[1]) - r, Math.max(c.a[0], c.b[0]) + r, Math.max(c.a[1], c.b[1]) + r];
      worksRects.push(box);
    }
  }
  // Roads drawn by hand: the battlemaps and site tiles along them (their band, its bridges'
  // decks, and the world's own buildings it takes away: 160 ft takes in the largest).
  const pp = prev.paths ?? {};
  const np = next.paths ?? {};
  if (pp !== np) for (const id of keys('paths', pp, np)) {
    const [a, b] = [pp[id], np[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const r of [a, b]) {
      if (!r) continue;
      const line = pathCurve(r.pts);
      const e = pathWidth(r) / 2 + 160;
      worksRects.push([Math.min(...line.map((q) => q[0])) - e, Math.min(...line.map((q) => q[1])) - e, Math.max(...line.map((q) => q[0])) + e, Math.max(...line.map((q) => q[1])) + e]);
    }
  }
  // Quays, piers and breakwaters built by hand: the battlemaps and site tiles along them (a quay
  // fills in water behind it, a breakwater is a raised strip: a margin of their width and 60 ft).
  const qv = prev.wharves ?? {};
  const qn = next.wharves ?? {};
  if (qv !== qn) for (const id of keys('wharves', qv, qn)) {
    const [a, b] = [qv[id], qn[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const w of [a, b]) {
      if (!w) continue;
      const r = (w.width ?? 20) + 60;
      worksRects.push([Math.min(...w.pts.map((q) => q[0])) - r, Math.min(...w.pts.map((q) => q[1])) - r, Math.max(...w.pts.map((q) => q[0])) + r, Math.max(...w.pts.map((q) => q[1])) + r]);
    }
  }
  // Ground shaped by hand: the battlemaps over it, and every building that may stand on it
  // (a building rides on the ground under its corners: 160 ft takes in the largest).
  const ground: Rect[] = [];
  const pg = prev.ground ?? {};
  const ng = next.ground ?? {};
  const box: Rect = [Infinity, Infinity, -Infinity, -Infinity];
  const grow = (r: Rect) => {
    box[0] = Math.min(box[0], r[0]);
    box[1] = Math.min(box[1], r[1]);
    box[2] = Math.max(box[2], r[2]);
    box[3] = Math.max(box[3], r[3]);
  };
  if (pg !== ng) for (const key of keys('ground', pg, ng)) {
    if (pg[key] === ng[key]) continue;
    const b = parseKey(key);
    if (b) grow([b[0] * BLOCK * 5 - 5, b[1] * BLOCK * 5 - 5, (b[0] + 1) * BLOCK * 5 + 5, (b[1] + 1) * BLOCK * 5 + 5]);
  }
  // (A stroke's blocks are one box, so the rects edited grow by one a stroke.)
  if (box[0] <= box[2]) ground.push([...box]);
  const ph = prev.plateaus ?? {};
  const nh = next.plateaus ?? {};
  if (ph !== nh) for (const id of keys('plateaus', ph, nh)) {
    const [a, b] = [ph[id], nh[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const p of [a, b]) {
      if (!p) continue;
      const e = Math.max(p.edge ?? 0, 5) + 5;
      ground.push([Math.min(...p.pts.map((q) => q[0])) - e, Math.min(...p.pts.map((q) => q[1])) - e, Math.max(...p.pts.map((q) => q[0])) + e, Math.max(...p.pts.map((q) => q[1])) + e]);
    }
  }
  // Water drawn by hand: its outline and its bank (`waters::BANK_FT`) round it, as it was and is
  // (an outline over another cuts the other's shore there too).
  const pw = prev.waters ?? {};
  const nw = next.waters ?? {};
  if (pw !== nw) for (const id of keys('waters', pw, nw)) {
    const [a, b] = [pw[id], nw[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    for (const w of [a, b]) {
      if (!w) continue;
      const e = 20;
      const sh = shore(w);
      ground.push([Math.min(...sh.map((q) => q[0])) - e, Math.min(...sh.map((q) => q[1])) - e, Math.max(...sh.map((q) => q[0])) + e, Math.max(...sh.map((q) => q[1])) + e]);
    }
  }
  // The world's own islets taken away or brought back (`Edits.hidden`, `i:` ids).
  let islets: string[] = [];
  if (prev.hidden !== next.hidden) {
    const ph = new Set((prev.hidden ?? []).filter((h) => h.startsWith('i:')));
    const nh = new Set((next.hidden ?? []).filter((h) => h.startsWith('i:')));
    islets = [...ph, ...nh].filter((h) => ph.has(h) !== nh.has(h));
  }
  // The world's own buildings changed or taken away: round their middle as generated (a
  // castle keep reaches ~150 ft) and any new footprint.
  const pb = prev.buildings ?? {};
  const nb = next.buildings ?? {};
  const buildingRects: Rect[] = [];
  const replaced: string[] = [];
  if (pb !== nb) for (const id of keys('buildings', pb, nb)) {
    const [a, b] = [pb[id], nb[id]];
    if (a === b || JSON.stringify(a) === JSON.stringify(b)) continue;
    replaced.push(id);
    for (const e of [a, b]) {
      if (!e) continue;
      const pts: [number, number][] = [[e.at[0] - 150, e.at[1] - 150], [e.at[0] + 150, e.at[1] + 150], ...(e.poly ?? [])];
      buildingRects.push([Math.min(...pts.map((p) => p[0])) - 5, Math.min(...pts.map((p) => p[1])) - 5, Math.max(...pts.map((p) => p[0])) + 5, Math.max(...pts.map((p) => p[1])) + 5]);
    }
  }
  // Towns laid out anew by hand (the ward editor).
  const pt = prev.towns ?? {};
  const nt = next.towns ?? {};
  const fenceIds: string[] = [];
  const replanned: string[] = [];
  if (pt !== nt) for (const key of keys('towns', pt, nt)) {
    if (JSON.stringify(pt[key]) === JSON.stringify(nt[key])) continue;
    // Only its fences and hedges changed: the generator says what they redraw (`redrawFences`).
    if (JSON.stringify({ ...pt[key], fences: 0, at: 0 }) === JSON.stringify({ ...nt[key], fences: 0, at: 0 }) && (pt[key]?.fences || nt[key]?.fences)) {
      for (const k of new Set([...Object.keys(pt[key]?.fences ?? {}), ...Object.keys(nt[key]?.fences ?? {})])) {
        if (JSON.stringify(pt[key]?.fences?.[k]) !== JSON.stringify(nt[key]?.fences?.[k])) fenceIds.push(fenceId(Number(key), k));
      }
      continue;
    }
    replanned.push(key);
  }
  // Towns' members changed by hand: each created thing that joined or left (its own area), and the
  // towns touched (a generated building's: the town it was laid out in).
  const pm = prev.members ?? {};
  const nm = next.members ?? {};
  let members: EditDirty['members'] = null;
  if (pm !== nm) {
    const rects: Rect[] = [];
    const towns = new Set<number>();
    const ids = new Set<string>();
    for (const key of keys('members', pm, nm)) {
      if (JSON.stringify(pm[key]) === JSON.stringify(nm[key])) continue;
      towns.add(Number(key));
      for (const m of [pm[key], nm[key]]) for (const id of [...(m?.add ?? []), ...(m?.remove ?? [])]) ids.add(id);
    }
    const made = [...(prev.created ?? []), ...(next.created ?? [])];
    for (const id of ids) {
      const c = made.find((x) => x.id === id);
      if (c) {
        const pts: [number, number][] = [[c.x - 150, c.y - 150], [c.x + 150, c.y + 150], ...(c.poly ?? []), ...(c.pts ?? [])];
        rects.push([Math.min(...pts.map((p) => p[0])) - 5, Math.min(...pts.map((p) => p[1])) - 5, Math.max(...pts.map((p) => p[0])) + 5, Math.max(...pts.map((p) => p[1])) + 5]);
      } else {
        // (A generated building: the town it was laid out in.)
        const li = /^b:(\d+):/.exec(id)?.[1];
        if (li !== undefined) towns.add(Number(li));
      }
    }
    members = { rects, towns };
  }
  // Towns laid out by hand as streets (`Edits.streets`, keys `<li>` and `<li>/…`; `lanes/…` lie
  // outside any town).
  const hs = (prev.streets ?? {}) as Record<string, unknown>;
  const hn = (next.streets ?? {}) as Record<string, unknown>;
  let streets: Set<string> | null = null;
  if (hs !== hn) {
    streets = new Set<string>();
    for (const key of keys('streets', hs, hn)) if (hs[key] !== hn[key]) streets.add(key.split('/')[0]);
  }
  // The world's own settlements laid out in another culture (`Edits.cultures`).
  const pk = prev.cultures ?? {};
  const nk = next.cultures ?? {};
  const cultures: string[] = [];
  if (pk !== nk) for (const key of keys('cultures', pk, nk)) if (pk[key] !== nk[key]) cultures.push(key);
  // (161: and planned otherwise, `Edits.planning`: re-laid the same way.)
  const plp = prev.planning ?? {};
  const nlp = next.planning ?? {};
  if (plp !== nlp) for (const key of keys('planning', plp, nlp)) if (plp[key] !== nlp[key] && !cultures.includes(key)) cultures.push(key);
  // (189: and given another street pattern, `Edits.patterns`.)
  const ptp = prev.patterns ?? {};
  const ntp = next.patterns ?? {};
  if (ptp !== ntp) for (const key of keys('patterns', ptp, ntp)) if (ptp[key] !== ntp[key] && !cultures.includes(key)) cultures.push(key);
  // Roads, streets and paths taken off the map or brought back.
  const cutIds: string[] = [];
  if (prev.removed_roads !== next.removed_roads) {
    const was = new Set(prev.removed_roads ?? []);
    const now = new Set(next.removed_roads ?? []);
    for (const id of [...was, ...now]) if (was.has(id) !== now.has(id)) cutIds.push(id);
  }
  // Fences and hedges drawn by hand or gaps cut: round their lines now.
  const fenceRects: Rect[] = [];
  if (prev.fences !== next.fences || prev.fence_gaps !== next.fence_gaps) {
    for (const [was, now] of [[prev.fences, next.fences], [prev.fence_gaps, next.fence_gaps]] as const) {
      for (const id of new Set([...Object.keys(was ?? {}), ...Object.keys(now ?? {})])) {
        if (JSON.stringify(was?.[id]) === JSON.stringify(now?.[id])) continue;
        for (const f of [was?.[id], now?.[id]]) if (f) fenceRects.push(fenceBox(f.pts, 'r' in f && f.r ? f.r : 0));
      }
    }
  }
  return { createdChanged, createdRects, castles, made, relabel, renamedInside, roadNames, battleRects, put, worksRects, ground, islets, buildingRects, replaced, fenceIds, replanned, members, streets, cultures, cutIds, fenceRects };
}
