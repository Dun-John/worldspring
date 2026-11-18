// Edit › Build: the eleven modes' tools (buildings, castles and walls, crossings, water, roads,
// harbours, clearing, Stamp, Row and Plot), the menus' state, the worker ops they call and their
// undo steps. App keeps `syncTool()` / `go()` and calls `armBuild` / `disarmBuild`.
import type { BuildingEdit, BuildingFuncs, PieceEdit, Created, Edits, Tint } from '../../gen/protocol';
import { TINT_MATERIAL, WHARF_WIDTH } from '../../gen/protocol';
import { BuildTool, buildOptions, defaultBuild, settingsOf, type BuildSettings, type BuildShape, type Pt } from '../build';
import { CrossingTool } from '../crossing';
import { HarbourTool } from '../harbour';
import { WaterTool } from '../water';
import { PathDrawTool, kindWidth } from '../paths';
import { FENCE_KINDS } from '../fence';
import { fenceTools } from './fences.svelte';
import { waterways } from './waterways.svelte';
import { quickTools } from './quick.svelte';
import { WorksTool } from '../works';
import type { BuildMode } from '../../ui/BuildPanel.svelte';
import { PathTool, StampTool } from '../quick';
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
  dismissToast(id: number): void;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
  drawnBuilding(id: string): Created | null;
  drawnBuildingId(cid: string): string;
  fitDesign(bid: string, floors: number): Promise<{ designs: Edits['designs']; aside: string }>;
}

export function buildTools(app: BuildHost) {
  // Build › Fence: its own controller (the tool and the changes it makes).
  const fences = fenceTools(app);

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
  let buildArmed: { tool: BuildTool; cross: CrossingTool; water: WaterTool; harbour: HarbourTool; works: WorksTool; stamp: StampTool; path: PathTool; road: PathDrawTool } | null = null;
  /** The Build tab draws buildings, castles or walls, puts crossings down, or clears the world's
   * own buildings. */
  let buildMode = $state<BuildMode>('building');

  // Crossing, Water, Road and Docks & islets; Stamp, Row, Plot and the works: their own controllers.
  const w = waterways(app);
  const q = quickTools(app, { mode: () => buildMode, armed: () => buildArmed, build: () => build, setMode: setBuildMode });
  function armBuild() {
    if (buildArmed) return;
    if (!buildFuncs) void app.view.gen.buildingFuncs().then((f) => (buildFuncs = f));
    const tool = new BuildTool({
      shape: () => build.shape,
      drawn: (poly) => void buildDrawn(poly),
      hint: (t) => app.toast(t),
    });
    const crossTool = new CrossingTool({
      settings: () => w.cross,
      edits: () => app.edits,
      picked: () => w.crossEditing,
      placed: w.crossPlaced,
      pick: w.pickCrossing,
      hint: (t) => app.toast(t),
    });
    const waterTool = new WaterTool({
      edits: () => app.edits,
      picked: () => w.waterPicked,
      brush: () => w.water.brush,
      dug: () => w.water.dug,
      width: () => w.water.width,
      placed: w.waterPlaced,
      pick: (id) => (w.waterPicked = id),
      hint: (t) => app.toast(t),
    });
    const harbourTool = new HarbourTool({
      settings: () => w.harbour,
      edits: () => app.edits,
      picked: () => w.harbourPicked,
      islet: (at) => void w.harbourIslet(at),
      wharf: (a, b) => void w.harbourWharf(a, b),
      pick: (id) => (w.harbourPicked = id),
      hint: (t) => app.toast(t),
    });
    const worksTool = new WorksTool({
      mode: () => (buildMode === 'castle' ? 'castle' : 'wall'),
      settings: () => q.works,
      editing: () => q.worksSite(q.worksEditing),
      castle: (poly, gate) => void q.placeWorks({ kind: 'castle', poly, ...(gate != null ? { gate } : {}) }),
      wall: (pts, gates, closed) => void q.placeWorks({ kind: 'wall', pts, ...(gates.length ? { gates } : {}), ...(closed ? { closed } : {}) }),
      gates: (change) => {
        const c = q.worksSite(q.worksEditing);
        if (!c) return;
        void q.placeWorks('sides' in change ? { kind: 'castle', poly: c.poly, gate: change.sides[0], gates: change.sides.length > 1 ? change.sides.slice(1) : undefined } : { kind: 'wall', pts: change.pts, gates: change.gates.length ? change.gates : undefined }, true);
      },
      hint: (t) => app.toast(t),
    });
    const stampTool = new StampTool({ at: (x, y, place) => void q.stampAt(x, y, place), hint: (t) => app.toast(t) });
    const pathTool = new PathTool({ mode: () => (buildMode === 'lasso' ? 'lasso' : 'row'), changed: (path, done) => void q.pathChanged(path, done), hint: (t) => app.toast(t) });
    const roadTool = new PathDrawTool({
      edits: () => app.edits,
      picked: () => w.roadPicked,
      width: () => (w.roadPicked ? (app.edits.paths?.[w.roadPicked]?.width ?? kindWidth(app.edits.paths?.[w.roadPicked]?.type ?? w.road.kind)) : w.road.width),
      placed: w.roadPlaced,
      pick: (id) => (w.roadPicked = id),
      hint: (t) => app.toast(t),
    });
    buildArmed = { tool, cross: crossTool, water: waterTool, harbour: harbourTool, works: worksTool, stamp: stampTool, path: pathTool, road: roadTool };
  }

  function disarmBuild() {
    buildArmed = null;
    q.worksEditing = null;
    buildEditing = null;
    genEditing = null;
    w.crossEditing = null;
    w.harbourPicked = null;
  }

  function setBuildMode(m: BuildMode) {
    q.closePlot();
    buildMode = m;
    buildEditing = null;
    genEditing = null;
    w.crossEditing = null;
    w.harbourPicked = null;
    w.waterPicked = null;
    w.roadPicked = null;
    buildArmed?.cross.reset();
    buildArmed?.water.reset();
    buildArmed?.road.reset();
    fences.tool.reset();
    buildArmed?.harbour.reset();
    q.leave();
    app.syncTool();
  }

  /** The shops standing on a crossing now (their created ids). */
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
    if (buildMode === 'fence') return fences.tool;
    return buildArmed && (buildMode === 'crossing' ? buildArmed.cross : buildMode === 'water' ? buildArmed.water : buildMode === 'road' ? buildArmed.road : buildMode === 'harbour' ? buildArmed.harbour : buildMode === 'stamp' ? buildArmed.stamp : buildMode === 'row' || buildMode === 'lasso' ? buildArmed.path : buildMode === 'castle' || buildMode === 'wall' ? buildArmed.works : buildArmed.tool);
  }

  /** The keymap's Build keys: the crossing's kind and the road's type go round; a shape key. */
  function keyCross() {
    if (buildMode !== 'crossing') return setBuildMode('crossing');
    const order = ['bridge', 'ford', 'ferry'] as const;
    const kind = order[(order.indexOf(w.cross.kind) + 1) % order.length];
    w.cross = { ...w.cross, kind, width: w.crossEditing ? w.cross.width : kind === 'ford' ? 10 : 12 };
  }
  /** H again on Docks & islets, the next kind (islet, quay, pier, breakwater). */
  function keyHarbour() {
    if (buildMode !== 'harbour') return setBuildMode('harbour');
    if (w.harbourPicked) return;
    const order = ['islet', 'quay', 'pier', 'breakwater'] as const;
    const kind = order[(order.indexOf(w.harbour.kind) + 1) % order.length];
    w.harbour = { ...w.harbour, kind, width: kind === 'islet' ? w.harbour.width : WHARF_WIDTH[kind][2] };
  }
  function keyRoad() {
    if (buildMode !== 'road') return setBuildMode('road');
    if (w.roadPicked) return;
    const order = ['footpath', 'track', 'gravel', 'paved', 'cobbled'] as const;
    const kind = order[(order.indexOf(w.road.kind) + 1) % order.length];
    w.road = { ...w.road, kind, width: kindWidth(kind) };
  }
  /** F: Build › Fence (again: the next kind). */
  function keyFence() {
    if (buildMode !== 'fence') return setBuildMode('fence');
    // (Only Draw has a kind to cycle: from Gate or Erase, F goes back to Draw.)
    if (fences.fence.mode !== 'draw') return void (fences.fence = { ...fences.fence, mode: 'draw' });
    const order = FENCE_KINDS.map((k) => k.key);
    fences.fence = { ...fences.fence, kind: order[(order.indexOf(fences.fence.kind) + 1) % order.length] };
  }
  function keyShape(k: BuildShape) {
    if (buildMode === 'castle' && k !== 'tower') return void (q.works.castleShape = k);
    if (buildMode !== 'building') setBuildMode('building');
    // A round tower is a wizard's tower unless it was something else already.
    build = k === 'tower' && build.func === 'house' ? { ...build, shape: k, func: 'wizard_tower', floors: Math.max(build.floors, 4) } : { ...build, shape: k };
  }
  /** Enter: the line or outline drawn ends (Stamp: the ghost is put down). */
  const finish = () => (buildMode === 'fence' ? void fences.tool.finish() : buildMode === 'road' ? void buildArmed?.road.finish() : buildMode === 'castle' || buildMode === 'wall' ? buildArmed?.works.finish() : buildMode === 'row' || buildMode === 'lasso' ? buildArmed?.path.finish() : buildMode === 'stamp' ? void q.quickPlace() : buildMode === 'water' ? buildArmed?.water.finish() : buildArmed?.tool.finish());
  /** Backspace: a corner taken back. */
  const back = () => (buildMode === 'fence' ? void fences.tool.back() : buildMode === 'road' ? void buildArmed?.road.back() : buildMode === 'castle' || buildMode === 'wall' ? buildArmed?.works.back() : buildMode === 'row' || buildMode === 'lasso' ? buildArmed?.path.back() : buildArmed?.tool.back());
  /** Escape, in turn: a line being drawn dropped, then what is picked put down. */
  const escapes: (() => boolean)[] = [
    () => !!buildArmed?.tool.reset(),
    () => !!buildArmed?.path.reset(),
    () => !!buildArmed?.works.reset(),
    () => (q.worksEditing ? ((q.worksEditing = null), true) : false),
    () => !!buildArmed?.cross.reset(),
    () => (w.crossEditing ? ((w.crossEditing = null), true) : false),
    () => !!buildArmed?.water.reset(),
    () => (w.waterPicked ? ((w.waterPicked = null), true) : false),
    () => !!buildArmed?.road.reset(),
    () => !!fences.tool.reset(),
    () => (w.roadPicked ? ((w.roadPicked = null), true) : false),
  ];
  /** The menu's Done: nothing changed any more. */
  const done = () => (q.plotEditing && q.plotDone(), (buildEditing = null), (genEditing = null), (w.crossEditing = null), (q.worksEditing = null), (w.waterPicked = null), (w.roadPicked = null));

  return {
    tool,
    keyCross,
    keyRoad,
    keyHarbour,
    keyFence,
    fences,
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
    get works() { return q.works; },
    set works(v) { q.works = v; },
    get worksEditing() { return q.worksEditing; },
    set worksEditing(v) { q.worksEditing = v; },
    get inWayAsk() { return q.inWayAsk; },
    set inWayAsk(v) { q.inWayAsk = v; },
    get cross() { return w.cross; },
    set cross(v) { w.cross = v; },
    get crossEditing() { return w.crossEditing; },
    set crossEditing(v) { w.crossEditing = v; },
    get water() { return w.water; },
    set water(v) { w.water = v; },
    get waterPicked() { return w.waterPicked; },
    set waterPicked(v) { w.waterPicked = v; },
    get road() { return w.road; },
    set road(v) { w.road = v; },
    get roadPicked() { return w.roadPicked; },
    set roadPicked(v) { w.roadPicked = v; },
    get harbour() { return w.harbour; },
    set harbour(v) { w.harbour = v; },
    get harbourPicked() { return w.harbourPicked; },
    set harbourPicked(v) { w.harbourPicked = v; },
    armBuild,
    get stampSet() { return q.stampSet; },
    set stampSet(v) { q.stampSet = v; },
    get rowSet() { return q.rowSet; },
    set rowSet(v) { q.rowSet = v; },
    get plotEditing() { return q.plotEditing; },
    get plotName() { return q.plotSite(q.plotEditing)?.name || (q.plotEditing ? 'the plot' : null); },
    editPlot: q.editPlot,
    removePlot: q.removePlot,
    get lassoSet() { return q.lassoSet; },
    set lassoSet(v) { q.lassoSet = v; },
    get quick() { return q.quick; },
    set quick(v) { q.quick = v; },
    quickPlace: q.quickPlace,
    quickDrop: q.quickDrop,
    worksSite: q.worksSite,
    saveWorks: q.saveWorks,
    editWorks: q.editWorks,
    disarmBuild,
    setBuildMode,
    saveCrossing: w.saveCrossing,
    changeRoad: w.changeRoad,
    removeRoad: w.removeRoad,
    changeWater: w.changeWater,
    waterInfo: w.waterInfo,
    removeWater: w.removeWater,
    removeCrossing: w.removeCrossing,
    pickCrossing: w.pickCrossing,
    removeWharf: w.removeWharf,
    generatedBuilding,
    withPiece,
    refusal,
    editGenerated,
    saveGenerated,
    generatedActions,
    saveBuilding,
    editBuilding,
  };
}

export type BuildTools = ReturnType<typeof buildTools>;
