// Edit › Build: the eleven modes' tools (buildings, castles and walls, crossings, water, roads,
// harbours, clearing, Stamp, Row and Plot), the menus' state, the worker ops they call and their
// undo steps. App keeps `syncTool()` / `go()` and calls `armBuild` / `disarmBuild`.
import { untrack } from 'svelte';
import type { BuildingEdit, BuildingFuncs, PieceEdit, Created, Crossing, Edits, Plot, BlockFill, Tint, Water } from '../../gen/protocol';
import { TINT_MATERIAL } from '../../gen/protocol';
import { fillColour } from '../streets';
import { BuildTool, buildOptions, defaultBuild, settingsOf, type BuildSettings, type BuildShape, type Pt } from '../build';
import { CrossingTool, SHOP_FRONT_FT, crossingOf, crossingProblem, defaultCross, shopsWidth, type CrossSettings } from '../crossing';
import { HarbourTool, defaultHarbour, wharfOf, wharfProblem, type HarbourSettings } from '../harbour';
import { MAX_WATERS, WaterTool, centre, defaultWater, depthOf, flows, isDug, isLine, legacy, lookOf, ofSea, shore, streamLevels, streamSamples, waterProblem, widthOf, type WaterSettings } from '../water';
import { MAX_PATHS, PathDrawTool, defaultPath, kindWidth, pathProblem, type PathSettings } from '../paths';
import { ClearAreaTool, type ClearShape } from '../clearArea';
import { WorksTool, defaultWorks, type WorksSettings } from '../works';
import type { BuildMode, QuickInfo } from '../../ui/BuildPanel.svelte';
import { PathTool, StampTool, defaultLasso, defaultRow, defaultStamp, type LassoSettings, type RowSettings, type StampSettings } from '../quick';
import { newObjectId } from '../scatter';
import { type Change } from '../../sync/mapd';
import { hitName, sentence, type BuildingHit } from '../../ui/gazetteer';
import { existingSite } from '../../world/world';

/** What the Build tools need from the app. */
export interface BuildHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: Edits;
  readonly renames: Record<string, string>;
  readonly firstCreated: number;
  readonly buildOn: boolean;
  selection: import('../../ui/gazetteer').Selection | null;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
  drawnBuilding(id: string): Created | null;
  drawnBuildingId(cid: string): string;
  fitDesign(bid: string, floors: number): Promise<{ designs: Edits['designs']; aside: string }>;
}

export function buildTools(app: BuildHost) {

  // Build: while its tab is open, the map's pointer draws buildings (or redraws the one being
  // changed).
  let build = $state<BuildSettings>(defaultBuild());
  let buildFuncs = $state<BuildingFuncs | null>(null);
  /** The building being changed: one drawn by hand (its created id) or one of the world's own
   * (its building id, `b:<layout>:<id>`). */
  let buildEditing = $state<string | null>(null);
  /** One of the world's own being changed: its name, and the menu's choices as it was opened
   * (only what differs from them is changed). */
  let genEditing = $state<{ name: string; was: BuildSettings } | null>(null);
  let buildArmed: { tool: BuildTool; cross: CrossingTool; water: WaterTool; harbour: HarbourTool; clear: ClearAreaTool; works: WorksTool; stamp: StampTool; path: PathTool; road: PathDrawTool } | null = null;
  /** The Build tab draws buildings, castles or walls, puts crossings down, or clears the world's
   * own buildings. */
  let buildMode = $state<BuildMode>('building');
  let works = $state<WorksSettings>(defaultWorks());
  /** The castle or wall drawn by hand being changed (its created id). */
  let worksEditing = $state<string | null>(null);
  /** The world's own buildings standing where a castle or wall would go: take them away with it? */
  let inWayAsk = $state<{ what: string; count: number; choose: (go: boolean) => void } | null>(null);
  let clearShape = $state<ClearShape>('box');
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

  function armBuild() {
    if (buildArmed) return;
    if (!buildFuncs) void app.view.gen.buildingFuncs().then((f) => (buildFuncs = f));
    const tool = new BuildTool({
      shape: () => build.shape,
      drawn: (poly) => void buildDrawn(poly),
      hint: (t) => app.toast(t),
    });
    const crossTool = new CrossingTool({
      settings: () => cross,
      edits: () => app.edits,
      picked: () => crossEditing,
      placed: crossPlaced,
      pick: pickCrossing,
      hint: (t) => app.toast(t),
    });
    const waterTool = new WaterTool({
      edits: () => app.edits,
      picked: () => waterPicked,
      brush: () => water.brush,
      dug: () => water.dug,
      width: () => water.width,
      placed: waterPlaced,
      pick: (id) => (waterPicked = id),
      hint: (t) => app.toast(t),
    });
    const harbourTool = new HarbourTool({
      settings: () => harbour,
      edits: () => app.edits,
      picked: () => harbourPicked,
      islet: (at) => void harbourIslet(at),
      wharf: (a, b) => void harbourWharf(a, b),
      pick: (id) => (harbourPicked = id),
      hint: (t) => app.toast(t),
    });
    const clear = new ClearAreaTool({
      shape: () => clearShape,
      drawn: (poly) => void clearArea(poly),
      hint: (t) => app.toast(t),
    });
    const worksTool = new WorksTool({
      mode: () => (buildMode === 'castle' ? 'castle' : 'wall'),
      settings: () => works,
      editing: () => worksSite(worksEditing),
      castle: (poly, gate) => void placeWorks({ kind: 'castle', poly, ...(gate != null ? { gate } : {}) }),
      wall: (pts, gates, closed) => void placeWorks({ kind: 'wall', pts, ...(gates.length ? { gates } : {}), ...(closed ? { closed } : {}) }),
      gates: (change) => {
        const c = worksSite(worksEditing);
        if (!c) return;
        void placeWorks('sides' in change ? { kind: 'castle', poly: c.poly, gate: change.sides[0], gates: change.sides.length > 1 ? change.sides.slice(1) : undefined } : { kind: 'wall', pts: change.pts, gates: change.gates.length ? change.gates : undefined }, true);
      },
      hint: (t) => app.toast(t),
    });
    const stampTool = new StampTool({ at: (x, y, place) => void stampAt(x, y, place), hint: (t) => app.toast(t) });
    const pathTool = new PathTool({ mode: () => (buildMode === 'lasso' ? 'lasso' : 'row'), changed: (path, done) => void pathChanged(path, done), hint: (t) => app.toast(t) });
    const roadTool = new PathDrawTool({
      edits: () => app.edits,
      picked: () => roadPicked,
      width: () => (roadPicked ? (app.edits.paths?.[roadPicked]?.width ?? kindWidth(app.edits.paths?.[roadPicked]?.type ?? road.kind)) : road.width),
      placed: roadPlaced,
      pick: (id) => (roadPicked = id),
      hint: (t) => app.toast(t),
    });
    buildArmed = { tool, cross: crossTool, water: waterTool, harbour: harbourTool, clear, works: worksTool, stamp: stampTool, path: pathTool, road: roadTool };
  }

  // Quick placement: Stamp, Row and Plot (Lasso). A ghost follows the generator's answer (the same
  // `placing` code the agent tools use); each placing is one change (one undo step).
  let stampSet = $state<StampSettings>(defaultStamp());
  let rowSet = $state<RowSettings>(defaultRow());
  let lassoSet = $state<LassoSettings>(defaultLasso());
  let quick = $state<QuickInfo>({ can: false, text: '' });
  /** The newest preview asked for (an older answer is dropped). */
  let quickToken = 0;
  /** Where the stamp's ghost was last asked for. */
  let stampLast: Pt | null = null;
  const plural = (n: number, w: string) => `${n} ${w}${n === 1 ? '' : 's'}`;

  function stampAsk(x: number, y: number) {
    return { x, y, func: build.func, size: stampSet.size, clear: stampSet.clear, ...(stampSet.facing != null ? { facing: stampSet.facing } : {}), id: `c:${(app.edits.created ?? []).length}` };
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
    const tool = buildArmed?.stamp;
    if (!tool || buildMode !== 'stamp') return;
    stampLast = [x, y];
    const token = ++quickToken;
    const r = await app.view.gen.stamp(stampAsk(x, y));
    if (token !== quickToken || buildMode !== 'stamp') return;
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
    const tool = buildArmed?.stamp;
    if (!at || !tool) return;
    const r = await app.view.gen.stamp(stampAsk(at[0], at[1]));
    if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
    const id = `c:${(app.edits.created ?? []).length}`;
    const opts = buildOptions(build);
    const old = existingSite(app.edits, { id, kind: 'building', x: 0, y: 0, name: '', poly: r.poly }, [0, 0]);
    if (old) return app.toast(`${old.name} is already here`);
    // (With generated buildings to take away the stamp's own answer holds the spot: the plain check would refuse them.)
    const spot = r.removed.length ? { x: r.x, y: r.y, name: r.name } : await app.view.gen.buildingSpot(r.poly, opts.func, id);
    if (!spot) return app.toast('The map could not answer: try again');
    if ('error' in spot) return app.toast(sentence(spot.short ?? spot.error));
    const c: Created = { id, kind: 'building', x: spot.x, y: spot.y, name: build.name.trim() || spot.name, poly: r.poly, ...opts };
    const buildings = { ...(app.edits.buildings ?? {}) };
    for (const b of r.removed) buildings[b.id] = { at: b.at, removed: true };
    app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c], ...(r.removed.length ? { buildings } : {}) }, { tool: 'place_stamp', id, name: c.name }, 'user');
    build.name = '';
    tool.ghost = null;
    tool.gone = [];
    quick = { can: false, text: '' };
  }

  /** The row or outline being drawn changed (`done`: finished). */
  async function pathChanged(path: Pt[], done: boolean) {
    const tool = buildArmed?.path;
    if (!tool) return;
    const token = ++quickToken;
    if (buildMode === 'row') {
      if (path.length < 2) {
        tool.lots = [];
        tool.refused = [];
        quick = { can: false, text: '' };
        return;
      }
      const q = { line: path, side: rowSet.side, ward: rowSet.ward, kinds: [...rowSet.kinds], ...(rowSet.depth ? { depth: rowSet.depth } : {}), gaps: rowSet.gaps, clear: rowSet.clear, seed: rowSet.seed, first_id: (app.edits.created ?? []).length };
      const r = await app.view.gen.row(q);
      if (token !== quickToken || buildMode !== 'row') return;
      if (!r || 'error' in r) {
        tool.lots = [];
        tool.refused = [];
        quick = { can: false, text: r ? sentence(r.error) : 'The map could not answer: try again', bad: true };
        return;
      }
      tool.lots = r.lots.map((l) => l.poly);
      tool.refused = r.refused.map((f) => [f.x, f.y]);
      const fresh = r.lots.filter((l) => !l.existing).length;
      const parts = [`${plural(fresh, 'building')} in ${Math.round(r.length_ft)} ft`];
      if (r.refused_count) parts.push(`${r.refused_count} would not fit (${r.refused[0].why})`);
      if (r.removed.length) parts.push(`${plural(r.removed.length, 'building')} taken away`);
      quick = { can: done && fresh > 0, text: fresh ? parts.join('; ') : 'All of these are here already', removed: r.removed.length };
      if (!done && fresh) quick.text += ' · press Enter to finish';
      return;
    }
    // Plot: only a finished outline is checked.
    tool.over = null;
    tool.bad = false;
    if (!done) return void (quick = { can: false, text: '' });
    const r = await app.view.gen.parcelCheck(path);
    if (token !== quickToken || buildMode !== 'lasso') return;
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
    const tool = buildArmed?.path;
    if (buildMode === 'row' && tool && tool.path.length >= 2) untrack(() => void pathChanged(tool.path, tool.finished));
  });
  /** The stamp's settings changed: the ghost again where it was. */
  $effect(() => {
    void stampSet.size;
    void stampSet.clear;
    void stampSet.facing;
    void build.func;
    if (buildMode === 'stamp' && stampLast) untrack(() => void stampAt(stampLast![0], stampLast![1], false));
  });

  /** Plots drawn by hand outlined while Build is open, each in its fill's colour. */
  $effect(() => {
    const plots = app.buildOn ? (app.edits.created ?? []).filter((c) => c.kind === 'plot' && !c.removed && (c.poly?.length ?? 0) >= 3) : [];
    app.view.setPlotOutlines(plots.map((c) => ({ poly: c.poly!, color: fillColour((c.fill ?? { kind: 'generated', ward: 'common' }) as BlockFill) })));
  });

  /** The row or plot being drawn, put down: one change. */
  async function quickPlace() {
    if (buildMode === 'stamp') return void stampPlace();
    const tool = buildArmed?.path;
    if (!tool || !tool.finished) return;
    const n = (app.edits.created ?? []).length;
    if (buildMode === 'row') {
      const q = { line: tool.path, side: rowSet.side, ward: rowSet.ward, kinds: [...rowSet.kinds], ...(rowSet.depth ? { depth: rowSet.depth } : {}), gaps: rowSet.gaps, clear: rowSet.clear, seed: rowSet.seed, first_id: n };
      const r = await app.view.gen.row(q);
      if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
      const made: Created[] = [];
      for (const l of r.lots) {
        if (l.existing) continue;
        made.push({ id: `c:${n + made.length}`, kind: 'building', x: l.x, y: l.y, name: l.name, poly: l.poly, func: l.func, floors: l.floors });
      }
      if (!made.length) return app.toast('All of these are here already');
      const buildings = { ...(app.edits.buildings ?? {}) };
      for (const b of r.removed) buildings[b.id] = { at: b.at, removed: true };
      app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), ...made], ...(r.removed.length ? { buildings } : {}) }, { tool: 'create_building_row', count: made.length }, 'user');
      app.toast(`${plural(made.length, 'building')} put down${r.refused_count ? `; ${r.refused_count} would not fit` : ''}`);
    } else {
      const poly = tool.path;
      const r = await app.view.gen.parcelCheck(poly);
      if (!r || 'error' in r) return app.toast(r ? sentence(r.error) : 'The map could not answer: try again');
      const fill: Record<string, unknown> =
        lassoSet.fill === 'generated'
          ? { kind: 'generated', ward: lassoSet.ward, ...(lassoSet.lots ? { lots: lassoSet.lots } : {}), ...(lassoSet.reroll ? { reroll: lassoSet.reroll } : {}) }
          : lassoSet.fill === 'hand'
            ? { kind: 'hand' }
            : { kind: 'open', open: lassoSet.open };
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

  function quickDrop() {
    buildArmed?.path.reset();
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
    const tool = buildArmed?.works;
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
    if (editing) {
      const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? c : x));
      app.applyEdits({ ...app.edits, created, buildings }, { tool: 'update_feature', id: c.id, name }, 'user');
    } else {
      // (Numbered as it is added: another site may have come in meanwhile.)
      c.id = `c:${(app.edits.created ?? []).length}`;
      app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c], buildings }, { tool: 'create_feature', id: c.id, kind: what, name }, 'user');
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
    setBuildMode(c.kind as 'castle' | 'wall');
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
    };
  }
  function disarmBuild() {
    buildArmed = null;
    worksEditing = null;
    buildEditing = null;
    genEditing = null;
    crossEditing = null;
    harbourPicked = null;
  }

  function setBuildMode(m: BuildMode) {
    buildMode = m;
    worksEditing = null;
    buildArmed?.works.reset();
    buildEditing = null;
    genEditing = null;
    crossEditing = null;
    harbourPicked = null;
    waterPicked = null;
    roadPicked = null;
    buildArmed?.cross.reset();
    buildArmed?.water.reset();
    buildArmed?.road.reset();
    buildArmed?.harbour.reset();
    buildArmed?.path.reset();
    if (buildArmed) (buildArmed.stamp.ghost = null), (buildArmed.stamp.gone = []);
    stampLast = null;
    quickToken++;
    quick = { can: false, text: '' };
    app.syncTool();
  }

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

  /** The wharf picked is taken away, its lighthouse with it. */
  function removeWharf() {
    const id = harbourPicked;
    const w = id ? app.edits.wharves?.[id] : null;
    if (!id || !w) return;
    const { [id]: _gone, ...rest } = app.edits.wharves ?? {};
    harbourPicked = null;
    app.applyEdits({ ...app.edits, wharves: rest, ...(shopsOn(id).length ? { created: withoutShops(id) } : {}) }, { tool: 'remove_wharves', ids: [id], kind: w.kind }, 'user');
  }

  /** One of the world's own buildings (not drawn by hand) behind a building id. */
  function generatedBuilding(id: string): boolean {
    const m = /^b:(\d+):(\d+)$/.exec(id);
    return !!m && (Number(m[1]) < app.firstCreated || castleOfPiece(id) >= 0);
  }

  /** Where in `edits.created` the castle made by hand is whose keep or yard building a building
   * id is (-1: it is not one). */
  function castleOfPiece(id: string): number {
    const m = /^b:(\d+):\d+$/.exec(id);
    const k = m ? Number(m[1]) - app.firstCreated : -1;
    const c = k >= 0 ? app.edits.created?.[k] : undefined;
    return c?.kind === 'castle' && !c.removed ? k : -1;
  }

  /** `created` with a castle's piece set as the generator answered (null: as generated). */
  function withPiece(p: PieceEdit): Created[] {
    return (app.edits.created ?? []).map((c) => {
      if (c.id !== p.castle) return c;
      const { pieces: old, ...rest } = c;
      const pieces = { ...(old ?? {}) };
      if (p.edit) pieces[p.key] = p.edit;
      else delete pieces[p.key];
      return Object.keys(pieces).length ? { ...rest, pieces } : rest;
    });
  }

  /** A building edit's answer in words when it is a refusal (or none), else null. */
  function refusal(r: unknown): string | null {
    if (!r) return 'The map could not answer: try again';
    const e = (r as { error?: string }).error;
    return e === undefined ? null : e.charAt(0).toUpperCase() + e.slice(1);
  }

  /** The world's own buildings with their middle in an area drawn: taken away (one change). */
  async function clearArea(poly: Pt[]) {
    const tool = buildArmed?.clear;
    const found = await app.view.gen.buildingsIn(poly);
    if (tool) tool.pending = null;
    if (!found.length) return app.toast('No buildings of the town’s own there (drawn ones go with Delete)');
    const all = { ...(app.edits.buildings ?? {}) };
    for (const b of found) all[b.id] = { at: b.at, removed: true };
    const removed = found.map((b) => b.id);
    app.applyEdits({ ...app.edits, buildings: all }, { tool: 'remove_buildings', removed, count: removed.length }, 'user');
    if (app.selection?.kind === 'building' && removed.includes(app.selection.hit.id)) app.selection = null;
  }

  /** A business's or home's key from what a building is called (its label). */
  async function funcKey(label: string): Promise<string> {
    buildFuncs ??= await app.view.gen.buildingFuncs();
    const l = label.toLowerCase();
    return buildFuncs.businesses.find((b) => b.name.toLowerCase() === l)?.key ?? buildFuncs.homes.find((h) => h.name.toLowerCase() === l)?.key ?? 'house';
  }

  /** Whether the castle piece selected has an edit (Restore is offered then only): asked of the
   * generator, which knows which of the castle's pieces it is (the keep's key is not its id). */
  let pieceChanged = $state({ id: '', has: false });
  $effect(() => {
    const sel = app.selection;
    const id = sel?.kind === 'building' ? sel.hit.id : '';
    const k = id ? castleOfPiece(id) : -1;
    const own = id.split(':')[2];
    if (k < 0) return void (pieceChanged = { id: '', has: false });
    if (app.edits.created?.[k]?.pieces?.[own]) return void (pieceChanged = { id, has: true });
    void pieceEdit(id).then((e) => {
      if (app.selection === sel) pieceChanged = { id, has: !!e };
    });
  });

  /** A castle piece's edit so far (the generator knows which of the castle's pieces it is). */
  async function pieceEdit(id: string): Promise<BuildingEdit | undefined> {
    const r = await app.view.gen.buildingEdit(id, {});
    return r && 'piece' in r ? (r.piece.edit ?? undefined) : undefined;
  }

  /** Open the build menu on one of the world's own buildings. */
  async function editGenerated(hit: BuildingHit) {
    const k = castleOfPiece(hit.id);
    const e = k >= 0 ? await pieceEdit(hit.id) : app.edits.buildings?.[hit.id];
    const settings: BuildSettings = {
      shape: 'rect',
      func: e?.func ?? (await funcKey(hit.function)),
      floors: hit.floors,
      roof: (e?.roof ?? '') as BuildSettings['roof'],
      material: (e?.material ?? (e?.tint ? TINT_MATERIAL[e.tint as Tint] : '')) as BuildSettings['material'],
      rshape: (e?.shape ?? '') as BuildSettings['rshape'],
      shade: e?.shade ?? '',
      ruin: hit.ward !== 'underground' && (e?.structure === 'ruin' || hit.function.toLowerCase() === 'ruined building'),
      name: '',
    };
    app.go('edit', 'build');
    if (buildMode !== 'building') setBuildMode('building');
    buildEditing = hit.id;
    genEditing = { name: hitName(hit, app.renames), was: settings };
    build = { ...settings };
  }

  /** The world's own building being changed gets the menu's choices (what differs from when it
   * was opened), and a footprint drawn. */
  async function saveGenerated(poly?: Pt[]) {
    const id = buildEditing;
    const g = genEditing;
    if (!id || !g) return;
    const change: Record<string, unknown> = {};
    if (build.func !== g.was.func) change.func = build.func;
    if (build.floors !== g.was.floors) change.floors = build.floors;
    if (build.roof !== g.was.roof) change.roof = build.roof || 'auto';
    if (build.material !== g.was.material) {
      change.material = build.material || 'auto';
      if (app.edits.buildings?.[id]?.tint) change.tint = 'auto';
    }
    if (build.rshape !== g.was.rshape) change.shape = build.rshape || 'auto';
    if (build.shade !== g.was.shade) {
      change.shade = build.shade === '' ? 'auto' : build.shade;
      if (app.edits.buildings?.[id]?.tint) change.tint = 'auto';
    }
    if (build.ruin !== g.was.ruin) change.structure = build.ruin ? 'ruin' : 'roofed';
    if (poly) change.poly = poly;
    const name = build.name.trim();
    const piece = castleOfPiece(id) >= 0;
    // (A castle's piece keeps its name with its other changes.)
    if (piece && name) change.name = name;
    const r = await app.view.gen.buildingEdit(id, change);
    if (buildArmed) buildArmed.tool.pending = null;
    if (!r || 'error' in r) return app.toast(refusal(r) ?? '');
    const all = { ...(app.edits.buildings ?? {}) };
    if ('edit' in r) {
      if (r.edit) all[id] = r.edit;
      else delete all[id];
    }
    const created = 'piece' in r ? withPiece(r.piece) : app.edits.created;
    const renamesNext = name && !piece ? { ...(app.edits.renames ?? {}), [id]: name } : app.edits.renames;
    // Its inside designed by hand: new storeys follow (in the same step); a new footprint sets
    // the design aside (it comes back with the footprint).
    const designed = app.edits.designs?.[id];
    let designs = app.edits.designs;
    let aside = '';
    if (designed && (poly || change.structure === 'ruin')) aside = 'Its inside, designed by hand, no longer fits: set aside';
    else if (designed && change.floors !== undefined) ({ designs, aside } = await app.fitDesign(id, build.floors));
    app.applyEdits({ ...app.edits, buildings: all, renames: renamesNext, designs, created }, { tool: 'update_building', id, name: name || g.name }, 'user');
    if (aside) app.toast(aside);
    genEditing = { name: name || g.name, was: { ...build, name: '' } };
    build.name = '';
    // The info panel and the build menu show it as it is now (a new trade brings a new name).
    const s = app.selection;
    const fp = 'edit' in r ? r.edit?.poly : undefined;
    const at = 'piece' in r ? r.piece.edit?.at : r.edit?.at;
    const c = fp?.length ? fp.reduce((a, p) => [a[0] + p[0] / fp.length, a[1] + p[1] / fp.length], [0, 0]) : at ? at : null;
    const hit = c && (await app.view.gen.query(c[0], c[1]));
    if (hit?.kind !== 'building' || hit.id !== id) return;
    if (buildEditing === id && genEditing) genEditing = { ...genEditing, name: hitName(hit, app.edits.renames ?? {}) };
    if (s?.kind === 'building' && s.hit.id === id && app.selection === s) app.selection = { kind: 'building', hit };
  }

  /** The info panel's actions for one of the world's own buildings: change it in the build menu,
   * take it away, put it back as generated. */
  function generatedActions(hit: BuildingHit) {
    const id = hit.id;
    return {
      onEdit: () => void editGenerated(hit),
      onRemove: async () => {
        const r = await app.view.gen.buildingEdit(id, { remove: true });
        if (!r || 'error' in r) return app.toast(refusal(r) ?? '');
        const change = { tool: 'remove_buildings', removed: [id], count: 1, name: hitName(hit, app.renames) };
        if ('piece' in r) app.applyEdits({ ...app.edits, created: withPiece(r.piece) }, change, 'user');
        else if (r.edit) app.applyEdits({ ...app.edits, buildings: { ...(app.edits.buildings ?? {}), [id]: r.edit } }, change, 'user');
        else return;
        if (buildEditing === id) ((buildEditing = null), (genEditing = null));
        app.selection = null;
      },
      onRestore: (castleOfPiece(id) >= 0 ? pieceChanged.id === id && pieceChanged.has : !!app.edits.buildings?.[id])
        ? async () => {
            if (castleOfPiece(id) >= 0) {
              // (The castle's piece goes back as generated, its name too.)
              // (A yard building taken away has no keep to be mistaken for: its number is its key.)
              const k = castleOfPiece(id);
              const castle = app.edits.created?.[k];
              const own = id.split(':')[2];
              let key = castle?.pieces?.[own] ? own : '';
              if (!key) {
                const r = await app.view.gen.buildingEdit(id, {});
                if (!r || !('piece' in r)) return app.toast(refusal(r) ?? '');
                key = r.piece.key;
              }
              if (!castle?.pieces?.[key]) return app.toast('It is as generated already');
              app.applyEdits({ ...app.edits, created: withPiece({ castle: castle.id, key, keep: key === 'keep', edit: null }) }, { tool: 'restore_building', id }, 'user');
            } else {
              const { [id]: _gone, ...rest } = app.edits.buildings ?? {};
              app.applyEdits({ ...app.edits, buildings: rest }, { tool: 'restore_building', id }, 'user');
            }
            if (buildEditing === id) ((buildEditing = null), (genEditing = null));
            void app.view.gen.query(hit.x, hit.y).then((h) => {
              if (h?.kind === 'building' && app.selection?.kind === 'building' && app.selection.hit.id === id) app.selection = { kind: 'building', hit: h };
            });
          }
        : undefined,
    };
  }

  /** A footprint drawn: a new building there, or the one being changed moved there. */
  async function buildDrawn(poly: Pt[]) {
    const tool = buildArmed?.tool;
    if (buildEditing && genEditing) return saveGenerated(poly);
    const editing = buildEditing ? app.drawnBuilding(buildEditing) : null;
    const id = editing?.id ?? `c:${(app.edits.created ?? []).length}`;
    const opts = buildOptions(build);
    // The same building again: that one.
    const old = editing ? null : existingSite(app.edits, { id, kind: 'building', x: 0, y: 0, name: '', poly }, [0, 0]);
    if (old) {
      if (tool) tool.pending = null;
      app.toast(`${old.name} is already here`);
      return;
    }
    const spot = await app.view.gen.buildingSpot(poly, opts.func, id);
    if (tool) tool.pending = null;
    if (!spot) return app.toast('The map could not answer: try again');
    if ('error' in spot) return app.toast((spot.short ?? spot.error).charAt(0).toUpperCase() + (spot.short ?? spot.error).slice(1));
    const name = build.name.trim();
    if (editing) {
      const c: Created = { id: editing.id, kind: 'building', x: spot.x, y: spot.y, name: name || editing.name, poly, ...opts };
      const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? c : x));
      app.applyEdits({ ...app.edits, created }, { tool: 'update_building', id: c.id, name: c.name }, 'user');
      if (app.edits.designs?.[app.drawnBuildingId(c.id)]) app.toast('Its inside, designed by hand, no longer fits: set aside');
      return void reselect(c);
    }
    // (Numbered as it is added: another site may have come in meanwhile.)
    const nid = `c:${(app.edits.created ?? []).length}`;
    const c: Created = { id: nid, kind: 'building', x: spot.x, y: spot.y, name: name || spot.name, poly, ...opts };
    app.applyEdits({ ...app.edits, created: [...(app.edits.created ?? []), c] }, { tool: 'create_building', id: nid, name: c.name }, 'user');
    build.name = '';
  }

  /** Change the building being edited to the menu's choices (its footprint stays). Its inside,
   * if designed by hand, follows new storeys (in the same step). */
  async function saveBuilding() {
    if (genEditing) return void saveGenerated();
    const c = buildEditing ? app.drawnBuilding(buildEditing) : null;
    if (!c) return;
    const { roof: _r, tint: _t, material: _m, shape: _h, shade: _d, structure: _s, ...rest } = c;
    const next: Created = { ...rest, ...buildOptions(build), name: build.name.trim() || c.name };
    const bid = app.drawnBuildingId(c.id);
    let designs = app.edits.designs;
    let aside = '';
    if (app.edits.designs?.[bid] && next.structure === 'ruin' && c.structure !== 'ruin') aside = 'Its inside, designed by hand, no longer fits: set aside';
    else if ((next.floors ?? 1) !== (c.floors ?? 1)) ({ designs, aside } = await app.fitDesign(bid, next.floors ?? 1));
    if (app.drawnBuilding(c.id) !== c) return;
    const created = (app.edits.created ?? []).map((x) => (x.id === c.id ? next : x));
    app.applyEdits({ ...app.edits, created, designs }, { tool: 'update_building', id: c.id, name: next.name }, 'user');
    if (aside) app.toast(aside);
    void reselect(next);
  }

  /** The info panel shows a building drawn by hand as it is now (after a change to it). */
  async function reselect(c: Created) {
    const s = app.selection;
    if (s?.kind !== 'building' || app.drawnBuilding(s.hit.id)?.id !== c.id) return;
    const hit = await app.view.gen.query(c.x, c.y);
    if (hit?.kind === 'building' && app.selection === s) app.selection = { kind: 'building', hit };
  }

  /** Open the build menu on a building drawn by hand. */
  function editBuilding(id: string) {
    const c = app.drawnBuilding(id);
    if (!c) return;
    app.go('edit', 'build');
    buildEditing = c.id;
    build = settingsOf(c, (c.poly?.length ?? 0) >= 12 ? 'tower' : 'rect');
  }

  /** The armed tool of the mode shown (null while the tab is shut). */
  function tool() {
    if (buildMode === 'select') return null;
    return buildArmed && (buildMode === 'crossing' ? buildArmed.cross : buildMode === 'water' ? buildArmed.water : buildMode === 'road' ? buildArmed.road : buildMode === 'harbour' ? buildArmed.harbour : buildMode === 'clear' ? buildArmed.clear : buildMode === 'stamp' ? buildArmed.stamp : buildMode === 'row' || buildMode === 'lasso' ? buildArmed.path : buildMode === 'castle' || buildMode === 'wall' ? buildArmed.works : buildArmed.tool);
  }

  /** The keymap's Build keys: the crossing's kind and the road's type go round; a shape key. */
  function keyCross() {
    if (buildMode !== 'crossing') return setBuildMode('crossing');
    const order = ['bridge', 'ford', 'ferry'] as const;
    const kind = order[(order.indexOf(cross.kind) + 1) % order.length];
    cross = { ...cross, kind, width: crossEditing ? cross.width : kind === 'ford' ? 10 : 12 };
  }
  function keyRoad() {
    if (buildMode !== 'road') return setBuildMode('road');
    if (roadPicked) return;
    const order = ['footpath', 'track', 'gravel', 'paved', 'cobbled'] as const;
    const kind = order[(order.indexOf(road.kind) + 1) % order.length];
    road = { ...road, kind, width: kindWidth(kind) };
  }
  function keyShape(k: BuildShape) {
    if (buildMode === 'castle' && k !== 'tower') return void (works.castleShape = k);
    if (buildMode !== 'building') setBuildMode('building');
    // A round tower is a wizard's tower unless it was something else already.
    build = k === 'tower' && build.func === 'house' ? { ...build, shape: k, func: 'wizard_tower', floors: Math.max(build.floors, 4) } : { ...build, shape: k };
  }
  /** Enter: the line or outline drawn ends (Stamp: the ghost is put down). */
  const finish = () => (buildMode === 'road' ? void buildArmed?.road.finish() : buildMode === 'castle' || buildMode === 'wall' ? buildArmed?.works.finish() : buildMode === 'row' || buildMode === 'lasso' ? buildArmed?.path.finish() : buildMode === 'stamp' ? void quickPlace() : buildMode === 'water' ? buildArmed?.water.finish() : buildArmed?.tool.finish());
  /** Backspace: a corner taken back. */
  const back = () => (buildMode === 'road' ? void buildArmed?.road.back() : buildMode === 'castle' || buildMode === 'wall' ? buildArmed?.works.back() : buildMode === 'row' || buildMode === 'lasso' ? buildArmed?.path.back() : buildArmed?.tool.back());
  /** Escape, in turn: a line being drawn dropped, then what is picked put down. */
  const escapes: (() => boolean)[] = [
    () => !!buildArmed?.tool.reset(),
    () => !!buildArmed?.path.reset(),
    () => !!buildArmed?.works.reset(),
    () => (worksEditing ? ((worksEditing = null), true) : false),
    () => !!buildArmed?.cross.reset(),
    () => (crossEditing ? ((crossEditing = null), true) : false),
    () => !!buildArmed?.water.reset(),
    () => (waterPicked ? ((waterPicked = null), true) : false),
    () => !!buildArmed?.road.reset(),
    () => (roadPicked ? ((roadPicked = null), true) : false),
  ];
  /** The menu's Done: nothing changed any more. */
  const done = () => ((buildEditing = null), (genEditing = null), (crossEditing = null), (worksEditing = null), (waterPicked = null), (roadPicked = null));

  return {
    tool,
    keyCross,
    keyRoad,
    keyShape,
    finish,
    back,
    escapes,
    done,
    get build() { return build; },
    set build(v) { build = v; },
    get buildFuncs() { return buildFuncs; },
    set buildFuncs(v) { buildFuncs = v; },
    get buildEditing() { return buildEditing; },
    set buildEditing(v) { buildEditing = v; },
    get genEditing() { return genEditing; },
    set genEditing(v) { genEditing = v; },
    get buildArmed() { return buildArmed; },
    get buildMode() { return buildMode; },
    set buildMode(v) { buildMode = v; },
    get works() { return works; },
    set works(v) { works = v; },
    get worksEditing() { return worksEditing; },
    set worksEditing(v) { worksEditing = v; },
    get inWayAsk() { return inWayAsk; },
    set inWayAsk(v) { inWayAsk = v; },
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
    armBuild,
    get stampSet() { return stampSet; },
    set stampSet(v) { stampSet = v; },
    get rowSet() { return rowSet; },
    set rowSet(v) { rowSet = v; },
    get lassoSet() { return lassoSet; },
    set lassoSet(v) { lassoSet = v; },
    get quick() { return quick; },
    set quick(v) { quick = v; },
    quickPlace,
    quickDrop,
    worksSite,
    saveWorks,
    editWorks,
    disarmBuild,
    setBuildMode,
    saveCrossing,
    changeRoad,
    removeRoad,
    changeWater,
    waterInfo,
    removeWater,
    removeCrossing,
    pickCrossing,
    removeWharf,
    generatedBuilding,
    withPiece,
    refusal,
    clearArea,
    editGenerated,
    saveGenerated,
    generatedActions,
    saveBuilding,
    editBuilding,
    get clearShape() { return clearShape; },
    set clearShape(v) { clearShape = v; },
  };
}

export type BuildTools = ReturnType<typeof buildTools>;
