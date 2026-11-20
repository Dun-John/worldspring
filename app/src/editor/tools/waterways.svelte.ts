// Edit › Build › Crossing, Water, Road and Docks & islets: the menus' state and the changes they
// make (one undo step each). Build makes the tools and re-exports the fields App reads.
import type { Created, Crossing, Edits, Water } from '../../gen/protocol';
import type { Pt } from '../build';
import { SHOP_FRONT_FT, crossingOf, crossingProblem, defaultCross, shopsWidth, type CrossSettings } from '../crossing';
import { defaultHarbour, shoreQuayOf, wharfOf, wharfProblem, type HarbourSettings } from '../harbour';
import { MAX_WATERS, centre, defaultWater, depthOf, flows, isDug, isLine, legacy, lookOf, ofSea, shore, streamLevels, streamSamples, waterProblem, widthOf, type WaterSettings } from '../water';
import { MAX_PATHS, defaultPath, kindWidth, pathProblem, type PathSettings } from '../paths';
import { newObjectId } from '../scatter';
import type { BuildHost } from './build.svelte';

export function waterways(app: BuildHost) {
  let cross = $state<CrossSettings>(defaultCross());
  /** The crossing put down by hand being changed (its id). */
  let crossEditing = $state<string | null>(null);
  /** Build › Water: the kind new outlines get, and the water picked (its id), else drawing new ones. */
  let water = $state<WaterSettings>(defaultWater());
  let waterPicked = $state<string | null>(null);
  /** Build › Road: the type, width and name new roads get, and the road picked (its id), else drawing new ones. */
  let road = $state<PathSettings>(defaultPath());
  let roadPicked = $state<string | null>(null);
  let harbour = $state<HarbourSettings>(defaultHarbour());
  /** The quay, pier or breakwater picked (its id). */
  let harbourPicked = $state<string | null>(null);

  /** The shops standing on a crossing now (their created ids). */
  const shopsOn = (id: string): Created[] => (app.edits.created ?? []).filter((c) => c.on === id && !c.removed && c.kind === 'building');

  /** `edits.created` with the shops on crossing `id` taken away (they stay in the list, removed). */
  const withoutShops = (id: string): Created[] => (app.edits.created ?? []).map((c) => (c.on === id && !c.removed && c.kind === 'building' ? { ...c, removed: true } : c));

  /** `created` with shops laid along bridge `c` (`id`) as the menu's depth says; or why not. */
  async function withShops(id: string, c: Crossing, created: Created[]): Promise<Created[] | string> {
    const r = await app.view.gen.bridgeLots(c, { depth: cross.shopDepth, frontage: SHOP_FRONT_FT, lane: cross.shopLane, vary: cross.shopVary, id });
    if (!r) return 'The map could not answer: try again';
    if ('error' in r) return r.error.replace(/^shops: /, '').replace(/^./, (x) => x.toUpperCase());
    const out = [...created];
    for (const lot of r.lots) {
      const x = lot.poly.reduce((s, p) => s + p[0], 0) / lot.poly.length;
      const y = lot.poly.reduce((s, p) => s + p[1], 0) / lot.poly.length;
      out.push({ id: `c:${out.length}`, kind: 'building', x, y, name: '', poly: lot.poly, floors: lot.floors, func: lot.func, on: id });
    }
    return out;
  }

  /** Both banks clicked: a new crossing there, or the one being changed moved there; a bridge
   * with its shops (laid out again if it had them) as one change. */
  async function crossPlaced(a: Pt, b: Pt) {
    const c: Crossing = crossingOf(cross, a, b);
    const withShopsOn = cross.shops && c.kind === 'bridge';
    if (withShopsOn) c.width = Math.max(c.width, shopsWidth(cross.shopDepth, cross.shopLane));
    const problem = crossingProblem(c);
    if (problem) return app.toast(problem);
    const editing = crossEditing && app.edits.crossings?.[crossEditing] ? crossEditing : null;
    const id = editing ?? newObjectId('v');
    const next: Edits = { ...app.edits, crossings: { ...(app.edits.crossings ?? {}), [id]: c } };
    const had = editing ? shopsOn(editing).length > 0 : false;
    if (withShopsOn || had) {
      let created: Created[] | string = app.edits.created ?? [];
      if (editing && had && withShopsOn) {
        const r = await app.view.gen.followBridge(id, c, app.edits.crossings?.[id], created);
        created = !r ? 'The map could not answer: try again' : 'error' in r ? r.error : r.created;
      } else {
        if (had) created = withoutShops(id);
        if (withShopsOn) created = await withShops(id, c, created);
      }
      if (typeof created === 'string') return app.toast(created);
      next.created = created;
    }
    app.applyEdits(next, { tool: 'place_crossing', id, kind: c.kind, changed: !!editing }, 'user');
  }

  /** A crossing put down earlier was clicked: the menu shows it, to change or take away. */
  function pickCrossing(id: string) {
    const c = app.edits.crossings?.[id];
    if (!c) return;
    crossEditing = id;
    const shops = shopsOn(id);
    // (Their depth as they stand: the first one's short side across the deck.)
    const sides = shops.map((s) => {
      const q = s.poly ?? [];
      return q.length >= 4 ? [Math.hypot(q[1][0] - q[0][0], q[1][1] - q[0][1]), Math.hypot(q[2][0] - q[1][0], q[2][1] - q[1][1])] : [0, 0];
    });
    const depths = sides.map((d) => Math.min(d[0], d[1]));
    const depth = depths.length ? Math.max(...depths) : cross.shopDepth;
    const vary = depths.length > 0 && (Math.max(...depths) - Math.min(...depths) > 0.5 || sides.some((d) => Math.abs(Math.max(d[0], d[1]) - Math.max(sides[0][0], sides[0][1])) > 0.5));
    cross = { kind: c.kind, width: c.width, material: c.material ?? 'timber', shops: shops.length > 0, shopDepth: Math.max(10, Math.min(40, Math.round(depth))), shopLane: cross.shopLane, shopVary: depths.length ? vary : cross.shopVary };
  }

  /** The crossing being changed gets the menu's kind, width and material (its ends stay); its
   * shops follow it, or go (the toggle off) or come (on). */
  async function saveCrossing() {
    const id = crossEditing;
    const c = id ? app.edits.crossings?.[id] : null;
    if (!c || !id) return;
    const next: Crossing = crossingOf(cross, c.a, c.b);
    const withShopsOn = cross.shops && next.kind === 'bridge';
    if (withShopsOn) next.width = Math.max(next.width, shopsWidth(cross.shopDepth, cross.shopLane));
    const problem = crossingProblem(next);
    if (problem) return app.toast(problem);
    const out: Edits = { ...app.edits, crossings: { ...(app.edits.crossings ?? {}), [id]: next } };
    const had = shopsOn(id).length > 0;
    if (withShopsOn || had) {
      let created: Created[] | string = app.edits.created ?? [];
      if (had && withShopsOn) {
        const r = await app.view.gen.followBridge(id, next, c, created);
        created = !r ? 'The map could not answer: try again' : 'error' in r ? r.error : r.created;
      } else {
        if (had) created = withoutShops(id);
        if (withShopsOn) created = await withShops(id, next, created);
      }
      if (typeof created === 'string') return app.toast(created);
      out.created = created;
    }
    app.applyEdits(out, { tool: 'place_crossing', id, kind: next.kind, changed: true }, 'user');
  }

  /** The level an outline takes (as `mapd`'s `create_water`): the water's where it reaches the sea
   * or a lake (the highest), else the sea's for a legacy inlet or basin, else 1 ft under the lowest
   * ground on its shore, as the loaded tiles have them. Water with no kind reaches the sea within
   * 4 ft of its shore (read out there too) and then stands at its level exactly, else half a foot
   * over it at least (so it looks the sea's only where it joins it). */
  function waterLevel(w: Water): number {
    const sea = app.view.geom?.sea_level_ft ?? 0;
    const modern = !legacy(w);
    // Along its shore as shaped (`Water::shore`), at most 96 points of it.
    const sh = shore(w);
    const every = Math.max(1, Math.ceil(sh.length / 96));
    const pts = sh.filter((_, i) => i % every === 0);
    const n = pts.length;
    const per = Math.max(1, Math.floor(96 / n));
    let a2 = 0;
    for (let i = 0; i < n; i++) a2 += pts[i][0] * pts[(i + 1) % n][1] - pts[(i + 1) % n][0] * pts[i][1];
    const out = modern ? (a2 > 0 ? -4 : 4) : 0;
    let [wet, low] = [-Infinity, Infinity];
    for (let i = 0; i < n; i++) {
      const [a, b] = [pts[i], pts[(i + 1) % n]];
      const l = Math.max(1e-9, Math.hypot(b[0] - a[0], b[1] - a[1]));
      const [ox, oy] = [((b[1] - a[1]) / l) * out, (-(b[0] - a[0]) / l) * out];
      for (let s = 0; s < per; s++) {
        const t = s / per;
        const [x, y] = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        const at = app.view.surfaceAt(x, y);
        if (at) {
          if (at[1] > at[0]) wet = Math.max(wet, at[1]);
          low = Math.min(low, at[0]);
        }
        const o = out ? app.view.surfaceAt(x + ox, y + oy) : null;
        if (o && o[1] > o[0]) wet = Math.max(wet, o[1]);
      }
    }
    if (wet > -Infinity) return modern && wet <= sea + 1 ? sea : Math.round(wet * 10) / 10;
    if (ofSea(w) || low === Infinity) return sea;
    return Math.round(Math.max(modern ? sea + 0.5 : sea, low - 1) * 10) / 10;
  }

  /** A stream's levels along its line (`waters::stream_levels`), as the loaded tiles have the ground. */
  function streamLevelsOf(w: Water): number[] {
    const sea = app.view.geom?.sea_level_ft ?? 0;
    const at = streamSamples(centre(w)).map(([x, y]): [number, number | null] => {
      const a = app.view.surfaceAt(x, y);
      return a ? [a[0], a[1] > a[0] ? a[1] : null] : [sea + 1, null];
    });
    return streamLevels(at, sea);
  }

  /** Water as the tool draws it (no kind: its look follows it): a polygon, or a brushed line of
   * the width set (`closed`: a ring); its level kept from `prev` while its points and whether it
   * flows stay. */
  function waterOf(pts: Pt[], o: { line: boolean; closed: boolean; width?: number; depth: number; dug: boolean }, prev?: Water): Water {
    const base: Water = { pts, level: 0, depth: o.depth, dug: o.dug, ...(o.line ? { line: true, width: o.width, ...(o.closed ? { closed: true } : {}) } : {}) };
    if (flows(base)) {
      base.levels = prev?.levels?.length && flows(prev) && prev.pts === pts ? prev.levels : streamLevelsOf(base);
      base.level = Math.max(...base.levels);
    } else base.level = prev && !flows(prev) && prev.pts === pts ? prev.level : waterLevel(base);
    return base;
  }

  /** Water drawn in Build › Water: of the shape, width, depth and shore set. */
  function waterPlaced(pts: Pt[], closed = false) {
    const w = waterOf(pts, { line: water.brush, closed: water.brush && closed, width: water.brush ? water.width : undefined, depth: water.depth, dug: water.dug });
    const bad = waterProblem(w);
    if (bad) return app.toast(bad);
    if (Object.keys(app.edits.waters ?? {}).length >= MAX_WATERS) return app.toast(`A world holds at most ${MAX_WATERS} waters`);
    const id = newObjectId('e');
    waterPicked = id;
    app.applyEdits({ ...app.edits, waters: { ...(app.edits.waters ?? {}), [id]: w } }, { tool: 'create_water', id, kind: lookOf(w, app.view.geom?.sea_level_ft ?? 0).kind }, 'user');
  }

  /** A line ended in Build › Road: a road of the menu's type, width and name along it. */
  function roadPlaced(pts: Pt[]) {
    const r = { pts, type: road.kind, ...(road.width !== kindWidth(road.kind) ? { width: road.width } : {}), ...(road.name.trim() ? { name: road.name.trim() } : {}) };
    const bad = pathProblem(r);
    if (bad) return app.toast(bad);
    if (Object.keys(app.edits.paths ?? {}).length >= MAX_PATHS) return app.toast(`A world holds at most ${MAX_PATHS} roads drawn by hand`);
    const id = newObjectId('rw');
    roadPicked = id;
    app.applyEdits({ ...app.edits, paths: { ...(app.edits.paths ?? {}), [id]: r } }, { tool: 'place_path', id, kind: r.type }, 'user');
  }

  /** The road picked: another type (its width the type's unless it had its own), width or name. */
  function changeRoad(change: { kind?: PathSettings['kind']; width?: number; name?: string }) {
    const id = roadPicked;
    const r = id ? app.edits.paths?.[id] : null;
    if (!id || !r) return;
    const next = { ...r };
    if (change.kind && change.kind !== r.type) {
      next.type = change.kind;
      if (r.width === undefined || r.width === kindWidth(change.kind)) delete next.width;
    }
    if (change.width !== undefined) {
      if (change.width === kindWidth(next.type)) delete next.width;
      else next.width = change.width;
    }
    if (change.name !== undefined) {
      if (change.name.trim()) next.name = change.name.trim();
      else delete next.name;
    }
    if (JSON.stringify(next) === JSON.stringify(r)) return;
    app.applyEdits({ ...app.edits, paths: { ...(app.edits.paths ?? {}), [id]: next } }, { tool: 'place_path', id, kind: next.type, changed: true }, 'user');
  }

  function removeRoad() {
    const id = roadPicked;
    if (!id || !app.edits.paths?.[id]) return;
    const { [id]: _gone, ...rest } = app.edits.paths ?? {};
    const notes = app.edits.notes && id in app.edits.notes ? Object.fromEntries(Object.entries(app.edits.notes).filter(([k]) => k !== id)) : app.edits.notes;
    roadPicked = null;
    app.applyEdits({ ...app.edits, paths: rest, notes }, { tool: 'remove_paths', ids: [id] }, 'user');
  }

  /** The water picked: another width, depth or shore (water made with a kind loses it, keeping
   * its depth and shore), or name. */
  function changeWater(change: { name?: string; width?: number; depth?: number; dug?: boolean }) {
    const id = waterPicked;
    const w = id ? app.edits.waters?.[id] : null;
    if (!id || !w) return;
    let next = app.edits;
    const width = change.width !== undefined && isLine(w) && change.width !== widthOf(w) ? change.width : undefined;
    const depth = change.depth !== undefined && change.depth !== depthOf(w) ? change.depth : undefined;
    const dug = change.dug !== undefined && change.dug !== isDug(w) ? change.dug : undefined;
    if (width !== undefined || depth !== undefined || dug !== undefined) {
      const nw = waterOf(w.pts, { line: isLine(w), closed: !!w.closed && !flows(w), width: isLine(w) ? (width ?? widthOf(w)) : undefined, depth: depth ?? depthOf(w), dug: dug ?? isDug(w) }, w);
      const bad = waterProblem(nw);
      if (bad) return app.toast(bad);
      next = { ...next, waters: { ...(next.waters ?? {}), [id]: nw } };
    }
    if (change.name !== undefined && change.name.trim() !== (app.edits.renames?.[id] ?? '')) {
      const { [id]: _was, ...kept } = app.edits.renames ?? {};
      next = { ...next, renames: change.name.trim() ? { ...kept, [id]: change.name.trim() } : kept };
    }
    if (next !== app.edits) app.applyEdits(next, { tool: 'update_water', id }, 'user');
  }

  /** The picked water for the panel: its look, name, width (a line's), depth and shore. */
  function waterInfo(w: Water, name: string) {
    return { look: lookOf(w, app.view?.geom?.sea_level_ft ?? 0).kind, name, width: isLine(w) ? widthOf(w) : null, depth: depthOf(w), dug: isDug(w) };
  }

  function removeWater() {
    const id = waterPicked;
    if (!id || !app.edits.waters?.[id]) return;
    const { [id]: _gone, ...rest } = app.edits.waters ?? {};
    const drop = <T,>(m: Record<string, T> | undefined) => {
      if (!m || !(id in m)) return m;
      const { [id]: _x, ...r } = m;
      return r;
    };
    waterPicked = null;
    app.applyEdits({ ...app.edits, waters: rest, renames: drop(app.edits.renames), notes: drop(app.edits.notes), hidden: app.edits.hidden?.filter((h) => h !== id) }, { tool: 'delete_water', ids: [id] }, 'user');
  }

  function removeCrossing() {
    const id = crossEditing;
    const c = id ? app.edits.crossings?.[id] : null;
    if (!id || !c) return;
    const { [id]: _gone, ...rest } = app.edits.crossings ?? {};
    crossEditing = null;
    // (Its shops go with it.)
    app.applyEdits({ ...app.edits, crossings: rest, ...(shopsOn(id).length ? { created: withoutShops(id) } : {}) }, { tool: 'remove_crossings', ids: [id], kind: c.kind }, 'user');
  }

  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  /** An islet clicked: raised there (the generator says where one can stand). */
  async function harbourIslet(at: Pt) {
    const key = newObjectId('h');
    const r = await app.view.gen.islet(key, at[0], at[1], harbour.radius, harbour.rise);
    if (!r) return app.toast('The map could not answer: try again');
    if ('error' in r) return app.toast(cap(r.error));
    app.applyEdits({ ...app.edits, plateaus: { ...(app.edits.plateaus ?? {}), [key]: r.plateau } }, { tool: 'raise_islet', id: key, changed: false }, 'user');
  }

  /** A quay, pier or breakwater dragged: checked (its root on the bank, a lighthouse's place), then
   * put down with its lighthouse as one change. */
  async function harbourWharf(a: Pt, b: Pt) {
    const w = wharfOf(harbour, a, b);
    const problem = wharfProblem(w);
    if (problem) return app.toast(problem);
    const lighthouse = w.kind === 'breakwater' && harbour.lighthouse;
    const r = await app.view.gen.wharfCheck(w, lighthouse);
    if (!r) return app.toast('The map could not answer: try again');
    if ('error' in r) return app.toast(cap(r.error));
    const id = newObjectId('q');
    const next: Edits = { ...app.edits, wharves: { ...(app.edits.wharves ?? {}), [id]: w } };
    if (r.lighthouse) {
      const l = r.lighthouse;
      const created = app.edits.created ?? [];
      next.created = [...created, { id: `c:${created.length}`, kind: 'building', x: l.x, y: l.y, name: l.name, poly: l.poly, floors: 3, func: 'lighthouse', on: id }];
    }
    app.applyEdits(next, { tool: 'place_wharf', id, kind: w.kind, changed: false, lighthouse: !!r.lighthouse }, 'user');
  }

  /** R7 173: a quay brushed along the shore: the stroke snapped to the water's edge by the
   * generator (`wharf::shore_line`), checked, then put down as one change. */
  async function harbourShore(stroke: Pt[]) {
    const s = await app.view.gen.shoreLine(stroke);
    if (!s) return app.toast('The map could not answer: try again');
    if ('error' in s) return app.toast(cap(s.error));
    const w = shoreQuayOf(harbour, s.pts);
    const r = await app.view.gen.wharfCheck(w, false);
    if (!r) return app.toast('The map could not answer: try again');
    if ('error' in r) return app.toast(cap(r.error));
    const id = newObjectId('q');
    app.applyEdits({ ...app.edits, wharves: { ...(app.edits.wharves ?? {}), [id]: w } }, { tool: 'place_wharf', id, kind: 'quay', changed: false, shore: true }, 'user');
  }

  /** The wharf picked is taken away, its lighthouse with it. */
  function removeWharf() {
    const id = harbourPicked;
    const w = id ? app.edits.wharves?.[id] : null;
    if (!id || !w) return;
    const { [id]: _gone, ...rest } = app.edits.wharves ?? {};
    harbourPicked = null;
    app.applyEdits({ ...app.edits, wharves: rest, ...(shopsOn(id).length ? { created: withoutShops(id) } : {}) }, { tool: 'remove_wharves', ids: [id], kind: w.kind }, 'user');
  }


  return {
    crossPlaced,
    pickCrossing,
    saveCrossing,
    waterPlaced,
    roadPlaced,
    changeRoad,
    removeRoad,
    changeWater,
    waterInfo,
    removeWater,
    removeCrossing,
    harbourIslet,
    harbourWharf,
    harbourShore,
    removeWharf,
    get cross() { return cross; },
    set cross(v) { cross = v; },
    get crossEditing() { return crossEditing; },
    set crossEditing(v) { crossEditing = v; },
    get water() { return water; },
    set water(v) { water = v; },
    get waterPicked() { return waterPicked; },
    set waterPicked(v) { waterPicked = v; },
    get road() { return road; },
    set road(v) { road = v; },
    get roadPicked() { return roadPicked; },
    set roadPicked(v) { roadPicked = v; },
    get harbour() { return harbour; },
    set harbour(v) { harbour = v; },
    get harbourPicked() { return harbourPicked; },
    set harbourPicked(v) { harbourPicked = v; },
  };
}
