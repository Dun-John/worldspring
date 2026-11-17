// Edit › Streets: the ward editor's tool (`editor/town.ts`) and, on a town laid out by hand, the
// street graph's (`editor/streets.ts`), the trace picture, and the changes they ask for (one undo
// step each). App keeps `syncTool()` / `go()` and calls `armTown` / `disarmTown`.
import { untrack } from 'svelte';
import type { Edits, Feature, StreetPlan, StreetRequest, BlockFill, P2, TownPlan, TownReport, TownRequest, OpenKind } from '../../gen/protocol';
import { townUnderlay } from '../trace/underlay.svelte';
import { type PointerTool } from '../../render/MapView';
import { shell } from '../../ui/shell/layout.svelte';
import { TownTool, type TownMode } from '../town';
import { DOTS_PPF, HAND_BRUSH_FT, StreetTool, isDrawMode, pickIn, type StreetPick } from '../streets';
import { type Change } from '../../sync/mapd';
import { describe } from '../../sync/history';
import { type Selection } from '../../ui/gazetteer';

/** What the Streets tools need from the app. */
export interface StreetsHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: Edits;
  readonly renames: Record<string, string>;
  readonly firstCreated: number;
  readonly hud: import('../../render/MapView').HudState | null;
  readonly overlay: import('../../gen/protocol').Overlay | null;
  readonly world: import('../../gen/protocol').WorldFile;
  readonly settlements: import('../../gen/protocol').Feature[];
  readonly townOn: boolean;
  readonly selection: import('../../ui/gazetteer').Selection | null;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
  layoutFeature(li: number): import('../../gen/protocol').Feature | null;
  layoutIndexOf(id: string): number;
  choose(s: import('../../ui/gazetteer').Selection, fly: boolean): Promise<void>;
  flyToSize(x: number, y: number, size: number): void;
}

export function streetTools(host: StreetsHost) {
  // Edit › Streets: the town in view laid out anew by hand (the ward editor, `editor/town.ts`).
  let townTool = $state.raw<TownTool | null>(null);
  let townMode = $state<TownMode>('select');
  /** The brush's size, in patch widths. */
  let townSize = $state(1);
  /** The town in view (its layout index), its plan, the patch picked. */
  let townIndex = $state(-1);
  let townPlan = $state.raw<TownPlan | null>(null);
  let townNote = $state('');
  let townPatch = $state<number | null>(null);
  /** The corner picked (a square where streets meet), if any: picked instead of a patch. */
  let townCorner = $state<number | null>(null);
  /** Why the last gate or tower asked for was refused. */
  let townWallWhy = $state('');
  $effect(() => {
    void townCorner;
    townWallWhy = '';
  });
  let townMerging = $state(false);
  let townBusy = $state(false);
  // A town laid out by hand (`Edits.streets`): its street graph (`editor/streets.ts`) in place of the ward editor.
  let streetTool = $state.raw<StreetTool | null>(null);
  let streetPlan = $state.raw<StreetPlan | null>(null);
  let streetPick = $state<StreetPick | null>(null);
  /** The points of the street being drawn; the class streets drawn get. */
  let streetDrawn = $state(0);
  let streetClass = $state<'main' | 'street'>('street');
  /** What areas painted are; a wall drawn's towers apart (ft; null: as the town has them) and name. */
  let streetOpen = $state<OpenKind>('lawn');
  let wallSpacing = $state<number | null>(null);
  let wallName = $state('');
  // The town's trace picture (session-only, DM-only): shown over the map with the tab open, fitted
  // round the town in view; Adjust hands the map's pointer to its handles.
  $effect(() => {
    host.view.townTrace = host.townOn ? townUnderlay : null;
  });
  $effect(() => {
    const f = townIndex >= 0 ? host.settlements[townIndex] : null;
    if (!f) return;
    // (Round the town's box: its middle, a little larger.)
    const b = townBox();
    if (b) townUnderlay.setHome((b[0] + b[2]) / 2, (b[1] + b[3]) / 2, 1.1 * Math.max(b[2] - b[0], 300), 1.1 * Math.max(b[3] - b[1], 300));
    else townUnderlay.setHome(f.x, f.y, 5000, 5000);
  });
  $effect(() => {
    void [host.townOn, townUnderlay.adjust, townUnderlay.visible];
    untrack(() => host.syncTool());
  });
  const townTraceTool: PointerTool = {
    down: (x, y, e) => e.button === 0 && townUnderlay.adjustDown(x, y, host.view.cam.ppf),
    move: (x, y) => townUnderlay.adjustMove(x, y),
    up: () => townUnderlay.adjustUp(),
    cancel: () => townUnderlay.adjustUp(),
    hover: () => {},
  };
  /** "Back to generated" waiting for its answer. */
  let streetBackAsk = $state(false);
  /** Whether the town in view is laid out by hand. */
  const townHand = $derived(townIndex >= 0 && !!host.edits.streets?.[String(townIndex)]);
  /** The edits' streets as last seen, and how often they have changed (the plan is read again). */
  let streetsSeen: unknown = null;
  let streetsEpoch = 0;
  /** What the plan shown was read for: the town and its edit. */
  let townRead = '';
  let townAsked = 0;
  const townSelected = $derived(townPlan?.patches.find((p) => p.patch === townPatch) ?? null);
  const townCornerPicked = $derived(townPlan?.corners.find((c) => c.corner === townCorner) ?? null);
  const townName = $derived.by(() => {
    const f = townIndex >= 0 ? host.layoutFeature(townIndex) : null;
    return f ? (host.renames[f.id] ?? f.name) : null;
  });
  const townNear = $derived(streetPlan ? (host.hud?.ftPerPx ?? 99) <= 1 / DOTS_PPF : !!townPlan && townPlan.max_move_ft / (host.hud?.ftPerPx ?? 99) >= 40);

  function armTown() {
    if (townTool) return;
    townTool = new TownTool({
      mode: () => (isDrawMode(townMode) ? 'select' : townMode),
      radius: () => townSize * (townPlan?.max_move_ft ?? 300),
      selected: () => townPatch,
      selectedCorner: () => townCorner,
      corner: (v) => {
        townCorner = v;
        townPatch = null;
        townMerging = false;
        showTownCorner(v);
      },
      moved: (moves) => void townChange({ moves }, 'corners'),
      equalize: (patches) => void townChange({ equalize: patches }, 'equalize'),
      hint: (t) => host.toast(t),
    });
    townTool.setPlan(townPlan);
    streetTool = new StreetTool({
      mode: () => townMode,
      radius: () => townSize * HAND_BRUSH_FT,
      picked: () => streetPick,
      pick: (p) => (streetPick = p),
      change: (req, what) => void streetChange(req, what),
      drawClass: () => streetClass,
      areaKind: () => streetOpen,
      wallOpts: () => ({ spacing: wallSpacing, name: wallName }),
      drawing: (n) => (streetDrawn = n),
      hint: (t) => host.toast(t),
    });
    streetTool.setPlan(streetPlan);
    townRead = '';
  }

  /** A settlement card's Streets button: null (not a settlement), or whether its layout can be edited. */
  function streetsCard(f: Feature): { ok: boolean; why: string } | null {
    const li = host.layoutIndexOf(f.id);
    if (li < 0 || !(li < host.settlements.length || (li >= host.firstCreated && (f.kind === 'village' || f.kind === 'town')))) return null;
    if (!host.view.hasTown(li)) return { ok: false, why: 'Villages grow along their roads' };
    return { ok: true, why: 'Lay out its streets, walls and wards' };
  }

  function openStreets(f: Feature) {
    void host.choose({ kind: 'feature', feature: f }, true);
    host.go('edit', 'town');
  }

  function disarmTown() {
    townTool = null;
    streetTool = null;
    streetPick = null;
    streetDrawn = 0;
    townMerging = false;
    townPatch = null;
    townCorner = null;
  }

  /** The town the selection is (its layout index), else -1: a village has no streets to edit, so it is none. */
  function selectedTown(s: Selection | null): number {
    if (s?.kind !== 'feature') return -1;
    const li = host.layoutIndexOf(s.feature.id);
    return li >= 0 && host.view.hasTown(li) ? li : -1;
  }
  /** "Change town" was pressed: the next click on the map picks the town. */
  let townPicking = $state(false);
  $effect(() => {
    if (!host.townOn) townPicking = false;
  });

  // The town edited, while the tab is open: the selected settlement's, else the one at the middle of the
  // map as seen (above a phone's sheet, not under it).
  $effect(() => {
    const c = host.hud?.center;
    void host.overlay;
    const s = host.selection;
    const top = shell.sheetTop;
    if (!host.townOn || !c || townBusy) return;
    let i = selectedTown(s);
    if (i < 0) {
      const dy = shell.phone && Number.isFinite(top) ? ((top - window.innerHeight) / 2) * (host.hud?.ftPerPx ?? 0) : 0;
      i = host.view.townAt(c.x, c.y + dy);
    }
    if (i !== untrack(() => townIndex)) {
      townIndex = i;
      townPatch = null;
      townCorner = null;
      townMerging = false;
      streetPick = null;
      untrack(() => streetTool)?.reset();
    }
  });

  // Its plan, read again when the town or its edit changes (here, by undo, from elsewhere) or
  // the world is made again.
  $effect(() => {
    if (!host.townOn || !townTool || !host.overlay) return;
    const i = townIndex;
    if (host.edits.streets !== streetsSeen) {
      streetsSeen = host.edits.streets;
      streetsEpoch++;
    }
    const edit = townHand ? `hand ${streetsEpoch}` : JSON.stringify(host.edits.towns?.[String(i)] ?? null);
    const key = `${i}|${edit}|${host.view.geom?.world_hash ?? host.world.seed}|${host.overlay.features.length}`;
    if (key === townRead) return;
    townRead = key;
    void untrack(() => readTownPlan(i));
  });

  async function readTownPlan(i: number) {
    const asked = ++townAsked;
    if (i < 0) {
      townPlan = null;
      townNote = 'Go to a town, city or metropolis to re-plan it (villages grow along their roads). New towns are added in Edit › Places.';
      townTool?.setPlan(null);
      setStreetPlan(null);
      return;
    }
    if (untrack(() => townHand)) return readStreetPlan(i, asked);
    if (!townPlan || townPlan.layout !== i) townNote = 'Reading the town’s plan…';
    const r = await host.view.gen.townPlan(i);
    if (asked !== townAsked) return;
    if ('error' in r) {
      townPlan = null;
      townNote = r.error.charAt(0).toUpperCase() + r.error.slice(1);
    } else {
      townPlan = r;
      if (townPatch !== null && !r.patches.some((p) => p.patch === townPatch)) townPatch = null;
      if (townCorner !== null && !r.corners.some((c) => c.corner === townCorner)) townCorner = null;
    }
    townTool?.setPlan(townPlan);
    setStreetPlan(null);
  }

  /** A hand town's street graph read (in place of the ward editor's plan). */
  async function readStreetPlan(i: number, asked: number) {
    if (!streetPlan || streetPlan.layout !== i) townNote = 'Reading the town’s streets…';
    const r = await host.view.gen.streetPlan(i);
    if (asked !== townAsked) return;
    townPlan = null;
    townTool?.setPlan(null);
    if ('error' in r) {
      townNote = r.error.charAt(0).toUpperCase() + r.error.slice(1);
      return setStreetPlan(null);
    }
    const p = streetPick;
    if (p && !pickIn(r, p)) streetPick = null;
    setStreetPlan(r);
  }

  function setStreetPlan(p: StreetPlan | null) {
    const was = !!streetPlan;
    streetPlan = p;
    streetTool?.setPlan(p);
    if (!p) streetPick = null;
    // (The map's tool: the street graph's, or the ward editor's.)
    if (was !== !!p && host.townOn) host.syncTool();
  }

  /** How long a street change may take before the tool is given back (the reply, if it comes, is dropped). */
  const STREET_WAIT_MS = 60_000;

  /** A change asked of the hand town in view: its records applied per key (one step to undo), or why not. */
  async function streetChange(request: StreetRequest, what: string): Promise<boolean> {
    const i = townIndex;
    if (i < 0 || townBusy) return (streetTool?.settled(), false);
    townBusy = true;
    // (The reply is computed on the edits as they are now: it applies to these or not at all.)
    const base = host.edits;
    let r: Awaited<ReturnType<typeof host.view.gen.streetChange>>;
    let timer: ReturnType<typeof setTimeout> | undefined;
    try {
      const late = new Promise<{ error: string }>((ok) => (timer = setTimeout(() => ok({ error: 'the map took too long to answer: try again' }), STREET_WAIT_MS)));
      r = await Promise.race([host.view.gen.streetChange(i, request), late]);
    } catch (e) {
      r = { error: `the change failed: ${e instanceof Error ? e.message : String(e)}` };
    } finally {
      clearTimeout(timer);
      townBusy = false;
    }
    if (i !== townIndex) return (streetTool?.settled(), false);
    if ('error' in r) {
      streetTool?.settled();
      return (host.toast(r.error.charAt(0).toUpperCase() + r.error.slice(1)), false);
    }
    // (Also what the change reads besides: the sites and obstacles, cultures, buildings, crossings.)
    const read = ['streets', 'wharves', 'towns', 'created', 'cultures', 'buildings', 'crossings'] as const;
    if (read.some((f) => host.edits[f] !== base[f])) {
      streetTool?.settled();
      return (host.toast('The map changed meanwhile: try again'), false);
    }
    const { change, report } = r;
    const next: Edits = { ...host.edits };
    let any = false;
    for (const field of ['streets', 'wharves', 'towns'] as const) {
      const recs = change[field];
      if (!recs || !Object.keys(recs).length) continue;
      const out = { ...((host.edits[field] ?? {}) as Record<string, unknown>) };
      for (const [k, v] of Object.entries(recs)) {
        if (v === null) delete out[k];
        else out[k] = v;
      }
      (next as Record<string, unknown>)[field] = out;
      any = true;
    }
    if (!any) {
      streetTool?.settled();
      return (host.toast('Nothing changed'), false);
    }
    const key = String(i);
    if (change.streets) host.view.hintStreets(key, change.streets, report.rects);
    if (change.towns && key in change.towns) host.view.hintTown(key, change.towns[key] ?? null, report.rects);
    const stayed = Array.isArray(report.stayed) ? report.stayed.length : 0;
    if (stayed) host.toast(`${stayed === 1 ? 'A junction stays' : `${stayed} junctions stay`} put: moved, ${stayed === 1 ? 'it' : 'they'} would cross the streets`);
    if (report.lost?.length) host.toast(`Gone with their buildings: ${report.lost.map((l) => l.name || l.what).join(', ')}`);
    if (request.op === 'back_to_generated' || request.op === 'lay_out_by_hand') {
      streetPick = null;
      townPatch = null;
      townCorner = null;
    }
    const { rects: _, records: __, blocks: ___, problems: ____, lost: _____, ...rest } = report;
    const moved = request.op === 'move_junctions' ? request.moves.length - stayed : undefined;
    if (request.op === 'back_to_generated') setStreetPlan(null);
    host.applyEdits(next, { tool: 'edit_streets', id: host.settlements[i]?.id, name: townName ?? undefined, what, ...rest, op: request.op, ...(moved !== undefined ? { moved } : {}) }, 'user');
    return true;
  }

  /** The street, painted area or wall picked taken away (false: none of them picked). */
  function deletePicked(): boolean {
    const p = streetPick;
    if (!p) return false;
    // (Busy: a change is on its way; the key is swallowed, not passed on.)
    if (townBusy) return true;
    let req: StreetRequest;
    if (p.kind === 'street') req = { op: 'delete_street', street: p.id };
    else if (p.kind === 'area') req = { op: 'remove_area', area: p.id };
    else if (p.kind === 'wall') req = { op: 'remove_wall', wall: p.id };
    else return false;
    // (The pick stays when the change is refused.)
    void streetChange(req, p.kind).then((ok) => {
      if (ok && streetPick === p) streetPick = null;
    });
    return true;
  }

  /** Several changes asked in turn (each its own step to undo); stops at the first refused. */
  async function streetChanges(reqs: StreetRequest[], what: string) {
    for (const r of reqs) if (!(await streetChange(r, what))) return;
  }

  /** A click on the map with the tab open: a square or a patch picked, or the neighbour to join. */
  function townPick(x: number, y: number) {
    if (streetPlan) {
      streetPick = streetTool?.pickAt(x, y) ?? null;
      return;
    }
    const p = townTool?.patchAt(x, y) ?? null;
    if (townMerging && townSelected) {
      townMerging = false;
      if (p === null || p === townSelected.patch) return;
      if (!townSelected.neighbours.includes(p)) return void host.toast('Pick a patch next to it');
      void townChange({ patches: [{ patch: townSelected.patch, merge_with: p }] }, 'merge');
      return;
    }
    townCorner = townTool?.squareAt(x, y) ?? null;
    townPatch = townCorner === null ? p : null;
    if (townCorner !== null) showTownCorner(townCorner);
  }

  /** On a phone, the corner picked is brought up out from under the sheet (into the top third). */
  function showTownCorner(v: number) {
    const at = townPlan?.corners.find((c) => c.corner === v)?.at;
    if (!shell.phone || !at) return;
    const [, y0, , y1] = host.view.cam.viewRect();
    if (at[1] > y0 + 0.4 * (y1 - y0)) host.view.flyTo({ cx: host.view.cam.cx, cy: host.view.cam.cy + at[1] - (y0 + 0.3 * (y1 - y0)), zoom: host.view.cam.zoom });
  }

  /** A change asked of the town in view: made (one step to undo), or why not. */
  async function townChange(request: TownRequest, what: string) {
    const i = townIndex;
    townWallWhy = '';
    if (i < 0 || townBusy) return void townTool?.settled();
    townBusy = true;
    const r = await host.view.gen.townChange(i, request).finally(() => (townBusy = false));
    if (i !== townIndex) return void townTool?.settled();
    if ('error' in r) {
      townTool?.settled();
      return void host.toast(r.error.charAt(0).toUpperCase() + r.error.slice(1));
    }
    const key = String(i);
    townWallWhy = (r.report.wall?.refused ?? []).map((x) => x.why).join('; ');
    if (JSON.stringify(host.edits.towns?.[key] ?? null) === JSON.stringify(r.edit)) {
      townTool?.settled();
      return void host.toast(what === 'corners' ? 'Nothing moved: the patches round those corners must stay convex' : townWallWhy ? townWallWhy.charAt(0).toUpperCase() + townWallWhy.slice(1) : 'Nothing changed');
    }
    const towns = { ...(host.edits.towns ?? {}) };
    if (r.edit) towns[key] = r.edit;
    else delete towns[key];
    host.view.hintTown(key, r.edit, r.report.rects);
    // A renamed or noted building handed on to its successor: name and note move in the same step.
    let names = host.edits.renames;
    let notes = host.edits.notes;
    for (const c of r.report.renames_carried ?? []) {
      if (names && c.from in names) {
        names = { ...names, [c.to]: names[c.from] };
        delete names[c.from];
      }
      if (notes && c.from in notes) {
        notes = { ...notes, [c.to]: notes[c.from] };
        delete notes[c.from];
      }
    }
    const lost = r.report.renames_lost ?? [];
    if (lost.length) host.toast(`Kept on the old id: ${lost.map((l) => l.name).join(', ')}`);
    host.applyEdits({ ...host.edits, towns, ...(names ? { renames: names } : {}), ...(notes ? { notes } : {}) }, { tool: 'edit_town', id: host.settlements[i]?.id, name: townName ?? undefined, what, ...townReport(r.report) }, 'user');
  }

  /** What a change did, for its description (`describe`), as an agent's reply has it. */
  function townReport(r: TownReport): Partial<TownReport> {
    const { rects: _, ...rest } = r;
    return rest;
  }

  function townZoomIn() {
    const f = host.settlements[townIndex];
    if (f) host.flyToSize(f.x, f.y, townSpan());
  }

  /** The town in view's span (ft): its box's larger side (`townBox`), else 5,000. */
  function townSpan(): number {
    const b = townBox();
    return b ? Math.max(b[2] - b[0], b[3] - b[1], 600) : 5000;
  }

  /** The town in view's box (x0, y0, x1, y1 ft): the corners of its built blocks (not farmland or
   *  empty ones) laid by hand, else of its patches in town; null when neither is known. */
  function townBox(): [number, number, number, number] | null {
    let pts: P2[] = [];
    if (streetPlan?.junctions.length) {
      const at = new Map(streetPlan.junctions.map((j) => [j.id, j.p]));
      const ends = new Map(streetPlan.streets.map((st) => [st.id, [st.a, st.b]]));
      const built = (f: BlockFill) => (f.kind === 'generated' ? f.ward !== 'farm' && f.ward !== 'village' : f.kind !== 'hand' && !(f.kind === 'open' && f.open === 'farmland'));
      for (const b of streetPlan.blocks.filter((b) => built(b.fill))) {
        const js = (b.streets ?? []).flatMap((id) => ends.get(id) ?? []);
        pts.push(...(js.length ? js.map((j) => at.get(j)).filter((p): p is P2 => !!p) : [b.at]));
      }
      if (!pts.length) pts = streetPlan.junctions.map((j) => j.p);
    } else if (townPlan) {
      const at = new Map(townPlan.corners.map((c) => [c.corner, c.at]));
      pts = townPlan.patches.filter((p) => p.in_town).flatMap((p) => p.corners.map((v) => at.get(v) ?? p.at));
    }
    if (!pts.length) return null;
    const xs = pts.map((p) => p[0]);
    const ys = pts.map((p) => p[1]);
    return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  }


  return {
    get townTool() { return townTool; },
    set townTool(v) { townTool = v; },
    get townMode() { return townMode; },
    set townMode(v) { townMode = v; },
    get townSize() { return townSize; },
    set townSize(v) { townSize = v; },
    get townIndex() { return townIndex; },
    set townIndex(v) { townIndex = v; },
    get townPlan() { return townPlan; },
    set townPlan(v) { townPlan = v; },
    get townNote() { return townNote; },
    set townNote(v) { townNote = v; },
    get townPatch() { return townPatch; },
    set townPatch(v) { townPatch = v; },
    get townCorner() { return townCorner; },
    set townCorner(v) { townCorner = v; },
    get townWallWhy() { return townWallWhy; },
    set townWallWhy(v) { townWallWhy = v; },
    get townMerging() { return townMerging; },
    set townMerging(v) { townMerging = v; },
    get townBusy() { return townBusy; },
    set townBusy(v) { townBusy = v; },
    get streetTool() { return streetTool; },
    set streetTool(v) { streetTool = v; },
    get streetPlan() { return streetPlan; },
    set streetPlan(v) { streetPlan = v; },
    get streetPick() { return streetPick; },
    set streetPick(v) { streetPick = v; },
    get streetDrawn() { return streetDrawn; },
    set streetDrawn(v) { streetDrawn = v; },
    townTraceTool,
    get streetBackAsk() { return streetBackAsk; },
    set streetBackAsk(v) { streetBackAsk = v; },
    townSelected,
    townCornerPicked,
    townName,
    townNear,
    armTown,
    streetsCard,
    openStreets,
    disarmTown,
    get townPicking() { return townPicking; },
    set townPicking(v) { townPicking = v; },
    streetChange,
    deletePicked,
    streetChanges,
    townPick,
    showTownCorner,
    townChange,
    townZoomIn,
    get streetClass() { return streetClass; },
    set streetClass(v) { streetClass = v; },
    get streetOpen() { return streetOpen; },
    set streetOpen(v) { streetOpen = v; },
    get wallSpacing() { return wallSpacing; },
    set wallSpacing(v) { wallSpacing = v; },
    get wallName() { return wallName; },
    set wallName(v) { wallName = v; },
  };
}

export type StreetsTools = ReturnType<typeof streetTools>;
