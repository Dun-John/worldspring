// Edit › Build › Stamp, Row, Plot, and castles and walls: the menus' state, the ghosts and the
// changes they make (one undo step each). Build passes in the mode, the armed tools and its own
// menu's settings (`QuickHost`) and re-exports the fields App reads.
import { untrack } from 'svelte';
import type { Created, Edits, BlockFill, RowRoadAnswer } from '../../gen/protocol';
import { fillColour } from '../streets';
import { buildOptions, type BuildSettings, type Pt } from '../build';
import { WorksTool, defaultWorks, type WorksSettings } from '../works';
import type { BuildMode, QuickInfo } from '../../ui/BuildPanel.svelte';
import { PathTool, StampTool, defaultLasso, defaultRow, defaultStamp, type LassoSettings, type RowSettings, type StampSettings } from '../quick';
import { newObjectId } from '../scatter';
import { sentence } from '../../ui/gazetteer';
import { existingSite } from '../../world/world';
import { castleHill } from '../hill';
import { contains } from '../geom';
import type { BuildHost } from './build.svelte';

/** What the quick-placement tools read from Build. */
export interface QuickHost {
  mode(): BuildMode;
  armed(): { stamp: StampTool; path: PathTool; works: WorksTool } | null;
  build(): BuildSettings;
  setMode(m: BuildMode): void;
}

export function quickTools(app: BuildHost, host: QuickHost) {
  let works = $state<WorksSettings>(defaultWorks());
  /** The castle or wall drawn by hand being changed (its created id). */
  let worksEditing = $state<string | null>(null);
  /** The world's own buildings standing where a castle or wall would go: take them away with it? */
  let inWayAsk = $state<{ what: string; count: number; choose: (go: boolean) => void } | null>(null);

  // Quick placement: Stamp, Row and Plot (Lasso). A ghost follows the generator's answer (the same
  // `placing` code the agent tools use); each placing is one change (one undo step).
  let stampSet = $state<StampSettings>(defaultStamp());
  let rowSet = $state<RowSettings>(defaultRow());
  let lassoSet = $state<LassoSettings>(defaultLasso());
  /** the plot drawn by hand being changed from Select (its created id): its fill, ward,
   * lots, layout and name rewritten in place as they are set, one step to undo each. */
  let plotEditing = $state<string | null>(null);
  let plotTimer: ReturnType<typeof setTimeout> | null = null;
  let quick = $state<QuickInfo>({ can: false, text: '' });
  /** The newest preview asked for (an older answer is dropped). */
  let quickToken = 0;
  /** Where the stamp's ghost was last asked for. */
  let stampLast: Pt | null = null;
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? '' : 's'}`;

  function stampAsk(x: number, y: number) {
    return { x, y, func: host.build().func, size: stampSet.size, clear: stampSet.clear, ...(stampSet.facing != null ? { facing: stampSet.facing } : {}), id: `c:${(app.edits.created ?? []).length}` };
  }

  /** The ghost's question in flight, and the newest place asked meanwhile (one at a time, the latest wins). */
  let stampBusy = false;
  let stampNext: Pt | null = null;
  /** A click's building is being put down: no preview is asked meanwhile. */
  let stampPlacing = false;
  /** The pointer is at (x, y) in Stamp: the ghost there, or (a click) the building put down. */
  async function stampAt(x: number, y: number, place: boolean) {
    if (place) {
      // A click always puts down at the click: no queued preview runs after it (it would take the answer's token).
      stampNext = null;
      stampPlacing = true;
      try {
        return await stampRun(x, y, true);
      } finally {
        stampPlacing = false;
      }
    }
    if (stampPlacing) return;
    stampLast = [x, y];
    if (stampBusy) {
      stampNext = [x, y];
      return;
    }
    stampBusy = true;
    try {
      let at: Pt | null = [x, y];
      while (at) {
        stampNext = null;
        await stampRun(at[0], at[1], false);
        at = stampPlacing ? null : (stampNext as Pt | null);
      }
    } finally {
      stampBusy = false;
    }
  }

  async function stampRun(x: number, y: number, place: boolean) {
    const tool = host.armed()?.stamp;
    if (!tool || host.mode() !== 'stamp') return;
    stampLast = [x, y];
    const token = ++quickToken;
    const r = await app.view.gen.stamp(stampAsk(x, y));
    if (token !== quickToken || host.mode() !== 'stamp') return;
    if (!r) return void (quick = { can: false, text: 'The map could not answer: try again', bad: true });
    if ('error' in r) {
      tool.ghost = null;
      tool.gone = [];
      quick = { can: false, text: sentence(r.error), bad: true };
      return place ? app.toast(quick.text) : undefined;
    }
    tool.ghost = { poly: r.poly, facing: r.facing, ok: true };
    tool.gone = r.removed.map((b) => b.at);
    quick = { can: true, text: `${r.name}${r.removed.length ? `: ${plural(r.removed.length, 'building')} taken away` : ''}${r.moved_ft > 5 ? `, ${Math.round(r.moved_ft)} ft from the click` : ''}`, removed: r.removed.length };
    if (place) void stampPlace([x, y]);
  }

  /** Put down the ghost's building (with the generated ones under it, if asked): one change; Stamp stays on. */
  async function stampPlace(at: Pt | null = stampLast) {
    const tool = host.armed()?.stamp;
    if (!at || !tool) return;
    const r = await app.view.gen.stamp(stampAsk(at[0], at[1]));
    if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
    const id = `c:${(app.edits.created ?? []).length}`;
    const opts = buildOptions(host.build());
    const old = existingSite(app.edits, { id, kind: 'building', x: 0, y: 0, name: '', poly: r.poly }, [0, 0]);
    if (old) return app.toast(`${old.name} is already here`);
    // (With generated buildings to take away the stamp's own answer holds the spot: the plain check would refuse them.)
    const spot = r.removed.length ? { x: r.x, y: r.y, name: r.name } : await app.view.gen.buildingSpot(r.poly, opts.func, id);
    if (!spot) return app.toast('The map could not answer: try again');
    if ('error' in spot) return app.toast(sentence(spot.short ?? spot.error));
    const c: Created = { id, kind: 'building', x: spot.x, y: spot.y, name: host.build().name.trim() || spot.name, poly: r.poly, ...opts };
    const buildings = { ...(app.edits.buildings ?? {}) };
    for (const b of r.removed) buildings[b.id] = { at: b.at, removed: true };
    app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c], ...(r.removed.length ? { buildings } : {}) }, { tool: 'place_stamp', id, name: c.name }, 'user');
    host.build().name = '';
    tool.ghost = null;
    tool.gone = [];
    quick = { can: false, text: '' };
  }

  /** The row's road, if asked for (the width only past what the kind alone gives). */
  const roadAsk = () => (rowSet.road.on ? { road: { kind: rowSet.road.kind, width: rowSet.road.width } } : {});

  /** The row or outline being drawn changed (`done`: finished). */
  async function pathChanged(path: Pt[], done: boolean) {
    const tool = host.armed()?.path;
    if (!tool) return;
    const token = ++quickToken;
    if (host.mode() === 'row') {
      if (path.length < 2) {
        tool.lots = [];
        tool.refused = [];
        tool.roadLine = null;
        quick = { can: false, text: '' };
        return;
      }
      const q = { line: path, side: rowSet.side, ward: rowSet.ward, kinds: [...rowSet.kinds], ...(rowSet.depth ? { depth: rowSet.depth } : {}), gaps: rowSet.gaps, clear: rowSet.clear, seed: rowSet.seed, first_id: (app.edits.created ?? []).length, ...roadAsk() };
      const r = await app.view.gen.row(q);
      if (token !== quickToken || host.mode() !== 'row') return;
      if (!r || 'error' in r) {
        tool.lots = [];
        tool.refused = [];
        tool.roadLine = null;
        quick = { can: false, text: r ? sentence(r.error) : 'The map could not answer: try again', bad: true };
        return;
      }
      tool.lots = r.lots.map((l) => l.poly);
      tool.refused = r.refused.map((f) => [f.x, f.y]);
      tool.roadLine = r.road?.line ?? null;
      tool.roadWidth = rowSet.road.width;
      const fresh = r.lots.filter((l) => !l.existing).length;
      const newRoad = !!r.road && !r.road.kept;
      const parts = [`${plural(fresh, 'building')} in ${Math.round(r.length_ft)} ft`];
      if (r.refused_count) parts.push(`${r.refused_count} would not fit (${r.refused[0].why})`);
      const gone = r.removed.length + (r.road && !r.road.kept ? r.road.removed.length : 0);
      if (gone) parts.push(`${plural(gone, 'building')} taken away`);
      if (r.road) parts.push(r.road.kept ? 'the road is here already' : r.road.as === 'street' ? 'with a street' : 'with a road');
      quick = { can: done && (fresh > 0 || newRoad), text: fresh || newRoad ? parts.join('; ') : 'All of these are here already', removed: gone };
      if (!done && fresh) quick.text += ' · press Enter to finish';
      return;
    }
    // Plot: only a finished outline is checked.
    tool.over = null;
    tool.bad = false;
    if (!done) return void (quick = { can: false, text: '' });
    const r = await app.view.gen.parcelCheck(path);
    if (token !== quickToken || host.mode() !== 'lasso') return;
    if (!r) return void (quick = { can: false, text: 'The map could not answer: try again', bad: true });
    if ('error' in r) {
      tool.bad = true;
      tool.over = r.over ?? null;
      return void (quick = { can: false, text: sentence(r.error), bad: true });
    }
    quick = { can: true, text: 'Ready: this outline can be a plot' };
  }

  /** The row's settings changed: lay it again over the same path. */
  $effect(() => {
    void JSON.stringify(rowSet);
    void app.edits.created?.length;
    const tool = host.armed()?.path;
    if (host.mode() === 'row' && tool && tool.path.length >= 2) untrack(() => void pathChanged(tool.path, tool.finished));
  });
  /** The stamp's settings changed: the ghost again where it was. */
  $effect(() => {
    void stampSet.size;
    void stampSet.clear;
    void stampSet.facing;
    void host.build().func;
    if (host.mode() === 'stamp' && stampLast) untrack(() => void stampAt(stampLast![0], stampLast![1], false));
  });

  /** Plots drawn by hand outlined while Build is open, each in its fill's colour. */
  $effect(() => {
    const plots = app.buildOn ? (app.edits.created ?? []).filter((c) => c.kind === 'plot' && !c.removed && (c.poly?.length ?? 0) >= 3) : [];
    app.view.setPlotOutlines(plots.map((c) => ({ poly: c.poly!, color: fillColour((c.fill ?? { kind: 'generated', ward: 'common' }) as BlockFill) })));
  });

  /** `next` with a row's road written: a drawn road, a hand town's street records, or a generated town's edit. */
  function withRoad(next: Edits, road: RowRoadAnswer): Edits {
    const out: Edits = { ...next };
    if (road.path) out.paths = { ...(next.paths ?? {}), [newObjectId('rw')]: road.path };
    for (const field of ['streets', 'wharves', 'towns'] as const) {
      const recs = road[field];
      if (!recs || !Object.keys(recs).length) continue;
      const m = { ...((next[field] ?? {}) as Record<string, unknown>) };
      for (const [k, v] of Object.entries(recs)) {
        if (v === null) delete m[k];
        else m[k] = v;
      }
      (out as Record<string, unknown>)[field] = m;
    }
    if (road.towns && road.town != null) {
      const k = String(road.town);
      if (road.towns[k]) {
        const [x0, y0, x1, y1] = road.line.reduce((b, p) => [Math.min(b[0], p[0]), Math.min(b[1], p[1]), Math.max(b[2], p[0]), Math.max(b[3], p[1])], [Infinity, Infinity, -Infinity, -Infinity]);
        const w = rowSet.road.width + 160;
        app.view.hintTown(k, road.towns[k], [[x0 - w, y0 - w, x1 + w, y1 + w]]);
      }
    }
    return out;
  }

  /** The row or plot being drawn, put down: one change. */
  async function quickPlace() {
    if (host.mode() === 'stamp') return void stampPlace();
    const tool = host.armed()?.path;
    if (!tool || !tool.finished) return;
    const n = (app.edits.created ?? []).length;
    if (host.mode() === 'row') {
      const q = { line: tool.path, side: rowSet.side, ward: rowSet.ward, kinds: [...rowSet.kinds], ...(rowSet.depth ? { depth: rowSet.depth } : {}), gaps: rowSet.gaps, clear: rowSet.clear, seed: rowSet.seed, first_id: n, ...roadAsk() };
      const r = await app.view.gen.row(q);
      if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
      const made: Created[] = [];
      for (const l of r.lots) {
        if (l.existing) continue;
        made.push({ id: `c:${n + made.length}`, kind: 'building', x: l.x, y: l.y, name: l.name, poly: l.poly, func: l.func, floors: l.floors, ...(l.faces ? { faces: l.faces } : {}) });
      }
      const road = r.road && !r.road.kept ? r.road : null;
      if (!made.length && !road) return app.toast('All of these are here already');
      const buildings = { ...(app.edits.buildings ?? {}) };
      for (const b of r.removed) buildings[b.id] = { at: b.at, removed: true };
      let next: Edits = { ...app.edits, created: [...(app.edits.created ?? []), ...made], ...(r.removed.length ? { buildings } : {}) };
      if (road) next = withRoad(next, road);
      app.applyEdits(next, { tool: 'create_building_row', count: made.length, ...(road ? { road: road.as } : {}) }, 'user');
      // (The history toast says what went down, with Undo; only what did not fit is added.)
      if (r.refused_count) app.toast(`${r.refused_count} would not fit`);
    } else {
      // (A new plot drawn: the one being changed is let go first.)
      if (plotEditing) plotDone();
      const poly = tool.path;
      const r = await app.view.gen.parcelCheck(poly);
      if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
      const fill = plotFill();
      const c: Created = { id: `c:${n}`, kind: 'plot', x: r.x, y: r.y, name: lassoSet.name.trim(), poly, fill };
      if (existingSite(app.edits, c, [c.x, c.y])) return app.toast('There is a plot here already');
      // (The world's own buildings under it give way to its fill, in the same change.)
      const removed = r.removed ?? [];
      const buildings = { ...(app.edits.buildings ?? {}) };
      for (const b of removed) buildings[b.id] = { at: b.at, removed: true };
      app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c], ...(removed.length ? { buildings } : {}) }, { tool: 'create_parcel', id: c.id, name: c.name }, 'user');
      app.toast(`Plot made${removed.length ? `; ${plural(removed.length, 'building')} taken away for it` : ''}`);
    }
    tool.reset();
    quick = { can: false, text: '' };
  }

  /** A plot drawn by hand, by its created id. */
  function plotSite(id: string | null): Created | null {
    const c = id ? app.edits.created?.find((x) => x.id === id) : null;
    return c && !c.removed && c.kind === 'plot' ? c : null;
  }

  /** The plot's fill as Fill an area's settings say it. */
  function plotFill(): Record<string, unknown> {
    return lassoSet.fill === 'generated'
      ? { kind: 'generated', ward: lassoSet.ward, ...(lassoSet.lots ? { lots: lassoSet.lots } : {}), ...(lassoSet.reroll ? { reroll: lassoSet.reroll } : {}) }
      : lassoSet.fill === 'hand'
        ? { kind: 'hand' }
        : { kind: 'open', open: lassoSet.open };
  }

  /** Open Fill an area on a plot drawn by hand, its settings loaded (Select's Change). */
  function editPlot(id: string) {
    const c = plotSite(id);
    if (!c) return;
    host.setMode('lasso');
    const f = (c.fill ?? { kind: 'generated', ward: 'common' }) as { kind: string; ward?: string; lots?: string; reroll?: number; open?: string };
    const d = defaultLasso();
    lassoSet = { fill: f.kind === 'hand' ? 'hand' : f.kind === 'open' ? 'open' : 'generated', ward: f.ward ?? d.ward, lots: f.lots ?? '', reroll: f.reroll ?? 0, open: f.open ?? d.open, name: c.name ?? '' };
    plotEditing = c.id;
  }

  /** The plot being changed given the menu's settings now: one step to undo (nothing if the same). */
  function savePlot() {
    if (plotTimer) clearTimeout(plotTimer);
    plotTimer = null;
    const c = plotSite(plotEditing);
    if (!c) return;
    const fill = plotFill();
    const name = lassoSet.name.trim();
    if (JSON.stringify(fill) === JSON.stringify(c.fill ?? null) && name === (c.name ?? '')) return;
    const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? { ...x, fill, name } : x));
    app.applyEdits({ ...app.edits, created }, { tool: 'update_parcel', id: c.id, name }, 'user');
  }

  // (A change to the plot's settings is written once they stop changing for a moment: typing its
  // name is one step, not one per letter.)
  $effect(() => {
    const v = JSON.stringify(lassoSet);
    if (!plotEditing) return;
    void v;
    untrack(() => {
      if (plotTimer) clearTimeout(plotTimer);
      plotTimer = setTimeout(savePlot, 500);
    });
  });

  /** The plot being changed taken away (one step to undo). */
  function removePlot() {
    const c = plotSite(plotEditing);
    if (plotTimer) clearTimeout(plotTimer);
    plotTimer = null;
    plotEditing = null;
    if (!c) return;
    const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? { ...x, removed: true } : x));
    app.applyEdits({ ...app.edits, created }, { tool: 'delete_feature', id: c.id }, 'user');
  }

  /** Done with the plot being changed: what is set is written now. */
  function plotDone() {
    if (plotTimer) savePlot();
    plotEditing = null;
  }

  function quickDrop() {
    host.armed()?.path.reset();
    quick = { can: false, text: '' };
  }

  /** A castle or wall drawn by hand, by its created id. */
  function worksSite(id: string | null): Created | null {
    const c = id ? app.edits.created?.find((x) => x.id === id) : null;
    return c && !c.removed && (c.kind === 'castle' || c.kind === 'wall') ? c : null;
  }

  /** A castle's or wall's options from the menu (its outline, line and gates aside). */
  function worksOptions(kind: string): Partial<Created> {
    if (kind === 'wall') return works.closed ? { closed: true } : {};
    const o: Partial<Created> = {};
    if (!works.keep) o.keep = false;
    if (!works.yardBuildings) o.yard_buildings = false;
    if (works.ruin) o.structure = 'ruin';
    if (works.keep && works.keepShape !== 'auto') o.keep_shape = works.keepShape;
    if (works.moat !== 'auto') o.moat = works.moat;
    return o;
  }

  /** A castle or wall drawn, or the one being changed drawn again or given new options or gates
   * (`keepOpts`: with its options as they were): checked by the generator; the world's own
   * buildings in its way go with it, in the same change, if the user says so. */
  async function placeWorks(shape: Partial<Created> & { kind: 'castle' | 'wall' }, keepOpts = false) {
    const tool = host.armed()?.works;
    const done = () => {
      if (tool) tool.pending = null;
    };
    const editing = worksSite(worksEditing);
    if (editing && editing.kind !== shape.kind) return void (done(), app.toast(`Draw a ${editing.kind} to change it`));
    const id = editing?.id ?? `c:${(app.edits.created ?? []).length}`;
    const opts: Partial<Created> =
      keepOpts && editing
        ? { keep: editing.keep, keep_shape: editing.keep_shape, keep_poly: editing.keep_poly, yard_buildings: editing.yard_buildings, moat: editing.moat, structure: editing.structure, closed: editing.closed }
        : worksOptions(shape.kind);
    const draft = { id, x: 0, y: 0, name: '', ...opts, ...shape } as Created;
    for (const k of Object.keys(draft) as (keyof Created)[]) if (draft[k] === undefined) delete draft[k];
    if (!editing && existingSite(app.edits, draft, [0, 0])) {
      done();
      return app.toast('That is already here');
    }
    const spot = await app.view.gen.worksSpot(draft);
    if (!spot) return void (done(), app.toast('The map could not answer: try again'));
    if ('error' in spot) return void (done(), app.toast(spot.error.charAt(0).toUpperCase() + spot.error.slice(1)));
    const what = shape.kind;
    if (spot.in_way.length) {
      const yes = await new Promise<boolean>((choose) => (inWayAsk = { what, count: spot.in_way.length, choose }));
      inWayAsk = null;
      if (!yes) return void done();
    }
    done();
    const name = works.name.trim() || editing?.name || spot.name;
    // (A castle's pieces changed by hand stay through a redraw or new options.)
    // (A wall's ends put onto the walls they meet: a T-join.)
    const c: Created = { ...draft, x: spot.x, y: spot.y, name, ...(editing?.pieces ? { pieces: editing.pieces } : {}), ...(spot.pts ? { pts: spot.pts } : {}) };
    const all = { ...(app.edits.buildings ?? {}) };
    for (const b of spot.in_way) all[b.id] = { at: b.at, removed: true };
    const buildings = spot.in_way.length ? all : app.edits.buildings;
    // (A hill raised under a castle: its own entry, chosen and taken away in Edit › Ground.)
    let plateaus = app.edits.plateaus;
    if (what === 'castle' && works.hill && c.poly?.length) {
      const under = Object.values(plateaus ?? {}).some((p) => p.kind === 'hill' && contains(p.pts, c.x, c.y));
      if (!under) {
        const hid = newObjectId('h');
        const h = castleHill(hid, c.poly as [number, number][]);
        if (typeof h === 'string') app.toast(h);
        else plateaus = { ...(plateaus ?? {}), [hid]: h };
      }
    }
    if (editing) {
      const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? c : x));
      app.applyEdits({ ...app.edits, created, buildings, ...(plateaus ? { plateaus } : {}) }, { tool: 'update_feature', id: c.id, name }, 'user');
    } else {
      // (Numbered as it is added: another site may have come in meanwhile.)
      c.id = `c:${(app.edits.created ?? []).length}`;
      app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c], buildings, ...(plateaus ? { plateaus } : {}) }, { tool: 'create_feature', id: c.id, kind: what, name }, 'user');
    }
    works.name = '';
  }

  /** The castle or wall being changed gets the menu's options (its outline, line and gates stay). */
  function saveWorks() {
    const c = worksSite(worksEditing);
    if (!c) return;
    const { keep: _k, keep_shape: _ks, keep_poly: kp, yard_buildings: _y, moat: _m, structure: _s, closed: _c, ...rest } = c;
    // (A drawn keep keeps its outline while it stays drawn.)
    const drawn = works.keep && works.keepShape === 'drawn' && kp?.length ? { keep_poly: kp } : {};
    void placeWorks({ ...rest, ...drawn, ...worksOptions(c.kind), kind: c.kind as 'castle' | 'wall' });
  }

  /** Open the build menu on a castle or wall drawn by hand. */
  function editWorks(id: string) {
    const c = worksSite(id);
    if (!c) return;
    app.go('edit', 'build');
    host.setMode(c.kind as 'castle' | 'wall');
    worksEditing = c.id;
    works = {
      castleShape: (c.poly?.length ?? 0) === 4 ? 'rect' : 'poly',
      keep: c.keep !== false,
      keepShape: c.keep_shape ?? (c.keep_poly?.length ? 'drawn' : 'auto'),
      moat: c.moat ?? 'auto',
      yardBuildings: c.yard_buildings !== false,
      ruin: c.structure === 'ruin',
      closed: !!c.closed,
      name: '',
      // (On while a hill stands under it; turned on, Save raises one.)
      hill: Object.values(app.edits.plateaus ?? {}).some((p) => p.kind === 'hill' && contains(p.pts, c.x, c.y)),
    };
  }

  /** Leaving a mode: what Stamp, Row, Plot and the works hold is let go (the plot being changed is written first). */
  function closePlot() {
    if (plotEditing) plotDone();
  }
  function leave() {
    worksEditing = null;
    host.armed()?.works.reset();
    host.armed()?.path.reset();
    if (host.armed()) (host.armed()!.stamp.ghost = null), (host.armed()!.stamp.gone = []);
    stampLast = null;
    quickToken++;
    quick = { can: false, text: '' };
  }

  return {
    closePlot,
    leave,
    plotSite,
    plotDone,
    worksSite,
    quickPlace,
    quickDrop,
    editPlot,
    removePlot,
    saveWorks,
    editWorks,
    get stampSet() { return stampSet; },
    set stampSet(v) { stampSet = v; },
    get rowSet() { return rowSet; },
    set rowSet(v) { rowSet = v; },
    get lassoSet() { return lassoSet; },
    set lassoSet(v) { lassoSet = v; },
    get quick() { return quick; },
    set quick(v) { quick = v; },
    get plotEditing() { return plotEditing; },
    get works() { return works; },
    set works(v) { works = v; },
    get worksEditing() { return worksEditing; },
    set worksEditing(v) { worksEditing = v; },
    get inWayAsk() { return inWayAsk; },
    set inWayAsk(v) { inWayAsk = v; },
    stampAt,
    pathChanged,
    placeWorks,
  };
}
