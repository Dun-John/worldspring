<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { runBench, runDungeonBench, runEditBench, runPlayBench, runSewerBench, type BenchResult } from './dev/bench';
  import type { BuildingFuncs, UnderCatalog, Clear, Conflict, Created, Crossing, Edits, Npc, Overlay, Placed, Plot, SpriteMeta, Stroke, WorldFile, WorldParams } from './gen/protocol';
  import SketchPanel from './editor/SketchPanel.svelte';
  import { Sketcher, drawsLand, scaleStrokes, type ToolSettings } from './editor/sketcher';
  import ShortcutsHelp from './ui/shell/ShortcutsHelp.svelte';
  import { buildKeymap } from './ui/shell/keymap';
  import { createKeyHandler, focusKind } from './ui/shell/shortcuts';
  import { reanchor } from './editor/reanchor';
  import type { PreviewReply, PreviewRequest } from './editor/preview.worker';
  import { MapView, type HudState, type InteriorState, type PointerTool } from './render/MapView';
  import TopBar from './ui/shell/TopBar.svelte';
  import MainMenu from './ui/shell/MainMenu.svelte';
  import SectionBar from './ui/shell/SectionBar.svelte';
  import Dock from './ui/shell/Dock.svelte';
  import MapControls from './ui/shell/MapControls.svelte';
  import GenerateTab from './ui/world/GenerateTab.svelte';
  import LibraryTab from './ui/world/LibraryTab.svelte';
  import { WorldDraft } from './ui/world/draft.svelte';
  import Icon from './ui/Icon.svelte';
  import Readout from './ui/shell/Readout.svelte';
  import DevStats from './ui/shell/DevStats.svelte';
  import { SECTIONS, shell, type Section } from './ui/shell/layout.svelte';
  import Confirm from './ui/shell/Confirm.svelte';
  import { UndoHistory } from './ui/shell/history.svelte';
  import FloorSelector from './ui/FloorSelector.svelte';
  import MovesMenu from './ui/MovesMenu.svelte';
  import type { Move } from './render/InteriorLayer';
  import InfoPanel from './ui/InfoPanel.svelte';
  import Names from './ui/Names.svelte';
  import PlacePanel, { type SiteChoice } from './ui/PlacePanel.svelte';
  import ScatterPanel from './ui/ScatterPanel.svelte';
  import BuildPanel from './ui/BuildPanel.svelte';
  import DesignPanel from './ui/DesignPanel.svelte';
  import { defaultDesign, SiteDesigner, type DesignSettings } from './editor/site/designer';
  import { BuildTool, buildOptions, defaultBuild, settingsOf, type BuildSettings, type Pt } from './editor/build';
  import { CrossingTool, crossingProblem, defaultCross, type CrossSettings } from './editor/crossing';
  import { defaultScatter, newObjectId, ScatterTool, type ScatterSettings } from './editor/scatter';
  import { shrink } from './render/customAtlas';
  import Notebook, { blankNpc, blankPlot, newNoteId, type Here } from './ui/Notebook.svelte';
  import { assetIds, assetUrl, getAsset, putAsset, shareAssets } from './world/assets';
  import Search from './ui/Search.svelte';
  import Toasts, { type Toast } from './ui/Toasts.svelte';
  import { lowMemory } from './ui/support';
  import { MapdSync, type Change } from './sync/mapd';
  import { PlayController } from './play/controller';
  import PlayPanel from './play/PlayPanel.svelte';
  import { describe, tidy, type Step } from './sync/history';
  import { applyOps, changedKeys, diffEdits, EDIT_FIELDS, type EditOp } from './sync/ops';
  import { breadcrumbs, frameSize, hitName, settlementAt, SETTLEMENT_KINDS, zoomFor, type Crumb, type Selection } from './ui/gazetteer';
  import { DEFAULT_PARAMS, download, editsKey, fromLocation, GEN_VERSION, newWorld, sameWorld, toHash, UNVERSIONED_EDITS_GEN, worldKey } from './world/world';
  import { buildUrl, keptVersions, PINNED } from './world/versions';
  import VersionAsk, { type VersionChoice } from './ui/shell/VersionAsk.svelte';
  import { loadEdits, saveEdits } from './world/library';

  const params = new URLSearchParams(location.search);
  // (Its keys come from the shortcuts below.)
  const view = new MapView('dm', { keys: false });
  if (import.meta.env.DEV) (window as unknown as { __map: MapView }).__map = view;
  let container: HTMLDivElement;
  // Raw: a new HUD state arrives ten times a second; only the readout and the stats read all
  // of it, the rest the few values below (which change far less often).
  let hud = $state.raw<HudState | null>(null);
  const ready = $derived(hud !== null);
  const progress = $derived(hud?.progress ?? null);
  const status = $derived(hud?.status ?? '');
  /** Close enough to draw buildings. */
  const near = $derived((hud?.ftPerPx ?? 99) < 2);
  const battlemapTier = $derived(hud?.tier === 'Battlemap');
  let bench = $state<BenchResult | null>(null);
  let benchRunning = $state(false);
  let busy = $state(false);
  // Raw: the world file is never changed in place (every edit makes new objects), so a big world's
  // edits are passed about by reference, never copied whole.
  let world = $state.raw<WorldFile>(fromLocation(location) ?? newWorld(1));
  // Raw: features are renamed in place (by the labels), and read fresh on every render.
  let overlay = $state.raw<Overlay | null>(null);
  let selection = $state.raw<Selection | null>(null);
  let inside = $state<InteriorState | null>(null);
  let places = $state(readPlaces());

  /** Business pins on or off, as this viewer left them (on at first). */
  function readPlaces(): boolean {
    try {
      return localStorage.getItem('map.places') !== '0';
    } catch {
      return true;
    }
  }

  function setPlaces(on: boolean) {
    places = on;
    try {
      localStorage.setItem('map.places', on ? '1' : '0');
    } catch {
      // Private mode: the choice just isn't remembered.
    }
  }
  let moves = $state<{ list: Move[]; x: number; y: number } | null>(null);

  // Play mode (local table): tokens, fog, measuring, and a window for the players.
  const play = new PlayController(view, 'dm');
  if (import.meta.env.DEV) (window as unknown as { __play: PlayController }).__play = play;
  let playOn = $state(false);
  let playVersion = $state(0);
  play.onChange = () => playVersion++;
  play.onError = (t) => toast(t);

  /** A play session: tokens, fog and doors are live (here and in the players' window) whatever
   * panel is open, until Stop playing. */
  function startPlay() {
    if (playOn) return;
    playOn = true;
    play.setOn(true);
    syncTool();
  }

  function stopPlay() {
    if (!playOn) return;
    playOn = false;
    play.setOn(false);
    if (shell.section === 'play') shell.section = null;
    syncTool();
  }
  const edits = $derived<Edits>(world.edits ?? {});
  const renames = $derived(edits.renames ?? {});
  const notes = $derived(edits.notes ?? {});
  const npcs = $derived(edits.npcs ?? {});
  const plots = $derived(edits.plots ?? {});
  const settlements = $derived(overlay ? overlay.features.filter((f) => SETTLEMENT_KINDS.includes(f.kind)) : []);
  // Layout indices: settlements, then points of interest (same order as T0), then created sites.
  const sites = $derived(overlay ? overlay.features.filter((f) => ['ruin', 'tower', 'waystation', 'camp', 'cave', 'mine', 'lava_tube'].includes(f.kind) && !f.id.startsWith('c:')) : []);
  // Search and breadcrumbs leave hidden features out (a new object per edit: names change in place).
  const shown = $derived.by<Overlay | null>(() => {
    const hidden = new Set(edits.hidden ?? []);
    return overlay ? { ...overlay, features: overlay.features.filter((f) => !hidden.has(f.id)) } : null;
  });
  const crumbs = $derived<Crumb[]>(shown && hud ? breadcrumbs(shown, hud.center.x, hud.center.y, hud.ftPerPx) : []);

  // Live link to mapd (agents), undo history and notices.
  const sync = new MapdSync();
  const undoHistory = new UndoHistory();
  let toasts = $state<Toast[]>([]);
  let nextToast = 1;

  function toast(text: string, undo?: () => void, more: Pick<Toast, 'action' | 'sticky'> = {}): number {
    const id = nextToast++;
    // (Notices that stay are kept when the others are trimmed.)
    toasts = [...toasts.filter((t) => t.sticky), ...toasts.filter((t) => !t.sticky).slice(-3), { id, text, undo, ...more }];
    if (!more.sticky) setTimeout(() => dismiss(id), undo || more.action ? 8000 : 4000);
    return id;
  }

  function dismiss(id: number) {
    toasts = toasts.filter((t) => t.id !== id);
  }

  function nameOf(id: string): string {
    const f = overlay?.features.find((g) => g.id === id);
    return f?.name ?? renames[id] ?? edits.created?.find((c) => c.id === id)?.name ?? id;
  }

  /**
   * The world's edits changed (here, by an agent, or by undo): apply them to the map, keep the
   * share link, tell mapd (unless it told us), and note it in the history.
   */
  function applyEdits(next: Edits, change: Change, author: string, opts: { record?: boolean; send?: boolean } = {}) {
    const before = edits;
    next = tidy(next);
    const ops = diffEdits(before, next);
    if (!ops.length) return;
    // (What changed made plain: the undo step and the saved copy must clone.)
    next = plainChanges(next, ops);
    world = { ...world, edits: next };
    view.setEdits(next, true, changedKeys(ops));
    play.setEdits(world);
    overlay = view.overlay;
    const sel = selection;
    if (sel?.kind === 'feature' && sel.feature.id.startsWith('c:') && !overlay?.features.some((f) => f.id === sel.feature.id)) selection = null;
    saveUrl();
    if (opts.send !== false) sync.ops(ops, change);
    const label = describe(change, author, nameOf);
    if (opts.record === false) {
      toast(label);
      return;
    }
    const step: Step = { label, author, ops, inverse: diffEdits(next, before) };
    undoHistory.push(step);
    toast(label, () => undo(step));
  }

  /** `next` with the values `ops` set made plain data (a value from a component may be a state
   * proxy); the ops carry the same values. A field with ops is never the one in the edits now
   * (the same object is the same field), so it was made for this change and takes the plain
   * values in place: no copy of a field of tens of thousands of objects for a brush stroke. */
  function plainChanges(next: Edits, ops: EditOp[]): Edits {
    const out = { ...next } as Record<string, unknown>;
    for (const op of ops) {
      if (op.op !== 'set' || EDIT_FIELDS[op.field] === 'set') continue;
      const v = $state.snapshot(op.value);
      op.value = v;
      const field = out[op.field];
      if (Array.isArray(field)) {
        const i = field.findIndex((x, k) => ((x as { id?: string })?.id ?? `c:${k}`) === op.key);
        if (i >= 0) field[i] = v;
      } else (field as Record<string, unknown>)[op.key] = v;
    }
    return out as Edits;
  }

  /** The edits with `ops` applied (entries they can't apply to are left alone). */
  function withOps(ops: EditOp[]): Edits {
    return applyOps(edits, ops, (_, err) => console.warn('[edits]', err)).edits;
  }

  function undo(step?: Step) {
    const s = undoHistory.takeUndo(step);
    if (s) applyEdits(withOps(s.inverse), { tool: 'undo', label: `Undid: ${s.label}` }, 'user', { record: false });
  }

  function redo() {
    const s = undoHistory.takeRedo();
    if (s) applyEdits(withOps(s.ops), { tool: 'redo', label: `Redid: ${s.label}` }, 'user', { record: false });
  }

  sync.onWorld = (w, mine) => {
    if (!sameWorld(w, world)) return true;
    if (busy) return false;
    // mapd's copy of this world keeps the edits made while the app was away; what was changed
    // here meanwhile goes on top (and to mapd).
    applyEdits(w.edits ?? {}, { tool: 'sync', label: 'Synced changes made elsewhere' }, 'mapd', { record: false, send: false });
    if (mine.length) applyEdits(withOps(mine), { tool: 'sync', label: 'Kept the changes made here meanwhile' }, 'user', { record: false });
    return true;
  };
  sync.onOps = (ops, change, author) => {
    if (!busy) applyEdits(withOps(ops), change, author, { send: false });
  };
  sync.onFocus = (x, y, size) => flyToSize(x, y, size);
  sync.onRender = (x, y, size) => view.capture(x, y, size);
  // Another tab may have its own world open: live sync follows one world at a time.
  let followNotice = 0;
  sync.onFollow = (following) => {
    if (followNotice) dismiss(followNotice);
    followNotice = following
      ? 0
      : toast('Live sync is following another tab’s world: changes here stay in this browser', undefined, {
          sticky: true,
          action: { label: 'Follow this tab', run: () => sync.follow() },
        });
  };
  let saveNotice = 0;
  sync.onSaveError = (problem) => {
    const had = saveNotice !== 0;
    if (saveNotice) dismiss(saveNotice);
    saveNotice = 0;
    if (problem === 'save') saveNotice = toast('Live sync could not save to disk: it keeps the changes and tries again', undefined, { sticky: true });
    else if (problem === 'switch') toast('Live sync can’t switch to this world until the open one is saved');
    else if (had) toast('Live sync saved the changes it was keeping');
  };
  sync.onStatus = (on) => {
    if (!on) toast('Live sync off: changes stay in this browser');
    // Agents see the portraits this browser keeps.
    else void shareAssets(assetIds(edits));
  };

  /** Keep the world in the address bar (its edits too while they are short) and its edits in
   * this browser: once a burst of changes settles (a big world's edits take a while to store),
   * at once when the page is hidden or closed. */
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function saveUrl() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(flushSave, 300);
  }
  function flushSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = null;
    const w = world;
    history.replaceState(null, '', location.pathname + (params.has('bench') ? `?bench=${params.get('bench') || 1}` : '') + toHash(w));
    void saveEdits(editsKey(w), w.edits ?? {});
  }

  /** The edits this browser keeps for `w` (by generator version; see `editsKey`). */
  async function storedEdits(w: WorldFile): Promise<Edits | null> {
    return (await loadEdits(editsKey(w))) ?? (w.gen_version === UNVERSIONED_EDITS_GEN ? await loadEdits(worldKey(w)) : null);
  }

  /** A world from another generator asked about (`VersionAsk`), until answered. */
  let versionAsk = $state<{ from: number; kept: boolean; edited: boolean; canCancel: boolean; choose: (c: VersionChoice) => void } | null>(null);
  /** A world has been shown (so a question can be cancelled back to it). */
  let worldShown = false;
  /** An older generator's build sent this world here to be upgraded (`?upgrade`): no question. */
  let upgradeSent = params.has('upgrade');
  /** The note an older generator's build shows was hidden. */
  let pinnedClosed = $state(false);

  /**
   * A world made with another generator than this build's: sent to the build for its version
   * (the newer one, or the one it was made with if the user asks), or upgraded to this one. Null
   * when the page is going elsewhere or the user cancelled.
   */
  async function settleVersion(w: WorldFile): Promise<WorldFile | null> {
    // (Its edits are kept where that build looks for them.)
    const goTo = async (gen: number | null, search = '') => {
      if (w.edits && Object.keys(w.edits).length) await saveEdits(editsKey(w), w.edits);
      location.href = buildUrl(w, gen, search);
      return null;
    };
    if (w.gen_version > GEN_VERSION) return goTo(null);
    const edits = w.edits ?? (await storedEdits(w));
    const edited = !!edits && Object.keys(edits).length > 0;
    // Development (bumped often), the newest build sent an upgrade, or nothing to lose: upgrade.
    const sent = upgradeSent;
    upgradeSent = false;
    if (!import.meta.env.DEV && !sent && edited) {
      const kept = (await keptVersions()).includes(w.gen_version);
      for (;;) {
        const c = await new Promise<VersionChoice>((choose) => (versionAsk = { from: w.gen_version, kept, edited, canCancel: worldShown, choose }));
        versionAsk = null;
        if (c === 'cancel') return null;
        if (c === 'made') return goTo(w.gen_version);
        if (c === 'upgrade') break;
        await download({ ...w, ...(edits ? { edits } : {}) }, `world-${w.seed}-v${w.gen_version}`);
      }
    }
    // Upgraded: edits it brings, else those already made on this generator, else its own.
    const up = { ...w, gen_version: GEN_VERSION };
    const already = w.edits ? null : await storedEdits(up);
    const kept = w.edits ?? already ?? edits;
    if (already && Object.keys(already).length) toast(`Opened the changes already made to this world on version ${GEN_VERSION} (version ${w.gen_version} keeps its own)`);
    else if (edited) toast(`Upgraded from version ${w.gen_version}: check that your changes still sit where they should`);
    return kept && Object.keys(kept).length ? { ...up, edits: kept } : up;
  }

  /** In an older generator's build: open this world in the newest, upgraded. */
  async function upgradeToNewest() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = null;
    await saveEdits(editsKey(world), world.edits ?? {});
    location.href = buildUrl(world, null, '?upgrade=1');
  }

  /**
   * Generate `w`. `keepSketch`: with the sketch being drawn (or else the world's), stretched
   * from the map size it was drawn at to `w`'s (a file opened or imported brings its own).
   */
  async function generate(w: WorldFile, keepSketch = false) {
    if (busy) return;
    if (w.gen_version !== GEN_VERSION) {
      busy = true;
      const settled = await settleVersion(w).finally(() => (busy = false));
      if (!settled) return;
      w = settled;
    }
    if (keepSketch) {
      const [src, from] = sketchOn ? [$state.snapshot(sketcher.strokes), sketchSize] : [$state.snapshot(world.sketch?.strokes ?? []), mapSize(world.params)];
      const to = mapSize(w.params);
      const strokes = scaleStrokes(src, to[0] / from[0], to[1] / from[1]);
      const { sketch: _, ...rest } = w;
      w = strokes.length ? { ...rest, sketch: { strokes } } : rest;
    }
    if (sketchOn) leaveSketch();
    shell.tabs.world = 'generate';
    // (The world being left keeps its last changes.)
    if (saveTimer) flushSave();
    busy = true;
    // Same world regenerated (or redrawn) keeps its edits; a different one brings its own, or
    // has those this browser kept for it.
    const same = sameWorld(w, world);
    const redrawn = same && JSON.stringify(w.sketch ?? null) !== JSON.stringify(world.sketch ?? null);
    const before = overlay?.features ?? [];
    const kept = same && world.edits && world.gen_version === w.gen_version ? world.edits : (w.edits ?? (await storedEdits(w)));
    world = kept && Object.keys(kept).length ? { ...w, edits: kept } : w;
    selection = null;
    saveUrl();
    try {
      // (The view applies the world's edits: names, created sites, hidden labels.)
      await view.loadWorld(world);
      overlay = view.overlay;
      sync.open(world);
      await play.setWorld(world, view.geom!.world_hash);
      worldShown = true;
    } finally {
      busy = false;
    }
    // (mapd's copy may have come while the world was loading.)
    sync.retake();
    // Redrawn: names and notes follow features that moved.
    const moved = redrawn && overlay ? reanchor(edits, before, overlay.features) : null;
    if (moved) applyEdits(moved.edits, { tool: 'reanchor', label: `Kept ${moved.moved.length} edited ${moved.moved.length === 1 ? 'place' : 'places'} with the redrawn world` }, 'user', { record: false });
    const n = overlay?.conflicts?.length ?? 0;
    if (w.sketch && n) toast(`The world follows your sketch, with ${n} ${n === 1 ? 'exception' : 'exceptions'}: see Sketch`);
  }

  // Sketch mode: draw over the map, see a quick preview of the world it makes, generate it.
  // The preview follows the World and Biomes settings as edited (`draft`), and the sketch
  // stretches when the map's size changes.
  const sketcher = new Sketcher();
  // The sketch tools' settings (the sketcher keeps one set for its whole life and follows these).
  let sketchSettings = $state<ToolSettings>(structuredClone(sketcher.settings));
  $effect(() => {
    sketcher.settings = $state.snapshot(sketchSettings) as ToolSettings;
  });
  // A pin takes the name typed for it, then the box clears.
  $effect(() => {
    void sketchVersion;
    if (sketcher.settings.name !== sketchSettings.name) sketchSettings.name = sketcher.settings.name;
  });
  if (import.meta.env.DEV) (window as unknown as { __sketcher: Sketcher }).__sketcher = sketcher;
  let draft = $state<{ seed: number; params: WorldParams } | null>(null);
  // The World panel's settings, kept here while the panel is shut; edits reach the sketch
  // preview through onDraft (the call itself depends on nothing here).
  const worldDraft = new WorldDraft();
  $effect(() => worldDraft.follow(world));
  $effect(() => {
    const s = worldDraft.seed >>> 0;
    const params = $state.snapshot(worldDraft.p) as WorldParams;
    untrack(() => onDraft(s, params));
  });
  /** Map size (mi) of a world's parameters. */
  const mapSize = (p: Partial<WorldParams>): [number, number] => [p.width_mi ?? DEFAULT_PARAMS.width_mi, p.height_mi ?? DEFAULT_PARAMS.height_mi];
  /** The map size (mi) the strokes being drawn are at. */
  let sketchSize: [number, number] = [DEFAULT_PARAMS.width_mi, DEFAULT_PARAMS.height_mi];

  /** The world the settings panel describes (not generated yet), without sketch or edits. */
  function draftWorld(): WorldFile {
    const { edits: _, sketch: __, ...rest } = world;
    // (From the panel's draft itself: `draft` follows it only once effects run, after the
    // handler that changed it, e.g. Random setting a seed and generating at once.)
    return { ...rest, seed: worldDraft.seed >>> 0, params: $state.snapshot(worldDraft.p) as WorldParams };
  }

  function onDraft(seed: number, params: WorldParams) {
    draft = { seed, params };
    if (!sketchOn) return;
    fitSketch();
    requestPreview();
  }

  /** Stretch the strokes to the settings' map size, if it changed. */
  function fitSketch() {
    const [w, h] = mapSize(draft?.params ?? world.params);
    if (w === sketchSize[0] && h === sketchSize[1]) return;
    const [ow, oh] = sketchSize;
    sketchSize = [w, h];
    view.sketchLayer?.setMapSize(w * 5280, h * 5280);
    // The view stretches with the drawing (it stays where it was on screen); a bigger map
    // can be zoomed out to as a whole.
    const [sx, sy] = [w / ow, h / oh];
    view.cam.minZoom = Math.min(view.cam.minZoom, view.cam.fitZoom(w * 5280, h * 5280, 0.5));
    view.cam.set({ cx: view.cam.cx * sx, cy: view.cam.cy * sy, zoom: Math.max(view.cam.minZoom, view.cam.zoom - Math.log2(Math.sqrt(sx * sy))) });
    sketcher.rescale(w / ow, h / oh);
  }
  let sketchOn = $state(false);
  let sketchVersion = $state(0);
  let sketchConflicts = $state<Conflict[]>([]);
  let previewStatus = $state('');
  let showPreview = $state(true);
  let previewWorker: Worker | null = null;
  let previewTimer: ReturnType<typeof setTimeout> | null = null;
  let previewId = 0;
  // The sketch as it stands (being drawn, or the world's), for the settings panel.
  const sketchStrokes = $derived<Stroke[]>(sketchOn ? (void sketchVersion, sketcher.strokes) : (world.sketch?.strokes ?? []));

  function enterSketch() {
    if (busy || playOn) return;
    sketcher.load($state.snapshot(world.sketch?.strokes ?? []));
    sketchSize = mapSize(world.params);
    const layer = view.enterSketch(sketcher);
    if (!layer) return;
    layer.showPreview = showPreview;
    sketcher.onDraw = () => layer.invalidate();
    sketcher.onChange = () => {
      sketchVersion++;
      layer.invalidate();
      requestPreview();
    };
    selection = null;
    sketchConflicts = overlay?.conflicts ?? [];
    sketchOn = true;
    // Settings changed but not generated yet: the sketch takes their map size now.
    fitSketch();
    requestPreview(0);
  }

  /** Leave sketch mode; `apply`: generate the world the sketch and settings describe. */
  function exitSketch(apply: boolean) {
    if (apply) void generate(draftWorld(), true);
    else leaveSketch();
  }

  /** The sketch being drawn differs from the world's (strokes not generated yet). */
  function sketchDirty(): boolean {
    return sketchOn && JSON.stringify($state.snapshot(sketcher.strokes)) !== JSON.stringify(world.sketch?.strokes ?? []);
  }

  function leaveSketch() {
    view.exitSketch();
    sketchOn = false;
    if (previewTimer) clearTimeout(previewTimer);
    previewWorker?.terminate();
    previewWorker = null;
  }

  /** A quick preview of the world the current strokes make, soon (later strokes win). */
  function requestPreview(delay = 250) {
    if (previewTimer) clearTimeout(previewTimer);
    previewTimer = setTimeout(() => {
      if (!previewWorker) {
        previewWorker = new Worker(new URL('./editor/preview.worker.ts', import.meta.url), { type: 'module' });
        previewWorker.onmessage = (e: MessageEvent<PreviewReply>) => {
          const r = e.data;
          if (r.id !== previewId || !sketchOn) return;
          if (!r.ok) {
            previewStatus = `Preview failed: ${r.error}`;
            return;
          }
          view.sketchLayer?.setPreview(r.rgba, r.w, r.h);
          sketchConflicts = r.conflicts;
          previewStatus = `Preview updated (${(r.ms / 1000).toFixed(1)} s); the world has far more detail.`;
        };
      }
      const strokes = $state.snapshot(sketcher.strokes);
      const base = draftWorld();
      const request: PreviewRequest = { id: ++previewId, worldJson: JSON.stringify(strokes.length ? { ...base, sketch: { strokes } } : base), width: 320 };
      previewStatus = 'Updating preview…';
      previewWorker.postMessage(request);
    }, delay);
  }

  function setShowPreview(on: boolean) {
    showPreview = on;
    if (view.sketchLayer) {
      view.sketchLayer.showPreview = on;
      view.sketchLayer.invalidate();
    }
  }

  function settlementName(index: number): string {
    const n = settlements.length;
    const f = index < n ? settlements[index] : sites[index - n];
    if (f) return renames[f.id] ?? f.name;
    const c = edits.created?.[index - n - sites.length];
    return c ? (renames[c.id] ?? c.name) : 'a settlement';
  }

  /** Rename a place; an empty name goes back to the generated one. A building drawn by hand
   * keeps its name in its entry (its label and the building itself show it). */
  function rename(id: string, name: string) {
    const drawn = drawnBuilding(id);
    if (drawn && name) {
      const all = { ...renames };
      delete all[drawn.id];
      delete all[`b:${layoutIndexOf(drawn.id)}:0`];
      const created = (edits.created ?? []).map((x) => (x.id === drawn.id ? { ...x, name } : x));
      return applyEdits({ ...edits, renames: all, created }, { tool: 'update_building', id: drawn.id, name }, 'user');
    }
    const all = { ...renames };
    if (name) all[id] = name;
    else delete all[id];
    applyEdits({ ...edits, renames: all }, { tool: 'rename_feature', id, name }, 'user');
  }

  // What is open: one section's panel at a time (the shell's state), each on its tab.
  const namesOn = $derived(shell.section === 'edit' && shell.tabs.edit === 'names');
  const placeOn = $derived(shell.section === 'edit' && shell.tabs.edit === 'sites');
  const scatterOn = $derived(shell.section === 'edit' && shell.tabs.edit === 'scatter');
  const buildOn = $derived(shell.section === 'edit' && shell.tabs.edit === 'build');
  const notebookOn = $derived(shell.section === 'notes');
  /** Sites underground can be designed, but not while playing. */
  const canDesign = $derived(!!inside?.id.startsWith('u:') && !playOn);
  // The place menu: put a site on the map (a click places it).
  let placeArmed = $state(false);
  /** What is being placed ("crypt"), for the banner over the map. */
  let placeLabel = $state('site');
  let placeTool: PointerTool | null = null;
  /** "Pick on map" (where an NPC is), waiting for its click. */
  let pickTool: PointerTool | null = null;

  /** The map's pointer goes to the first of: a place being picked, a site being placed, the
   * designer, the Build or Scatter tool, play's tools while a session runs. */
  function syncTool() {
    const buildTool = buildArmed && (buildMode === 'crossing' ? buildArmed.cross : buildArmed.tool);
    view.tool = pickTool ?? placeTool ?? designer ?? buildTool ?? scatterArmed?.tool ?? (playOn ? play : null);
  }

  /** Open a section (on a tab), or with null shut the panel. New sketch strokes or a site's
   * unsaved design that would be dropped are asked about first. */
  function go(to: Section | null, tab?: string) {
    const t = to && to !== 'play' ? (tab ?? shell.tabs[to]) : undefined;
    if (busy && (to === 'edit' || to === 'play')) return;
    if (to === 'edit' && t === 'design' && !designer && !canDesign) return toast(playOn ? 'Stop playing to design a site' : 'Go down into a dungeon, cave or mine to design it');
    const keepSketch = to === 'world';
    const keepDesign = to === 'edit' && t === 'design';
    if (sketchOn && !keepSketch && sketchDirty()) return ask('sketch', () => go(to, tab));
    if (designer?.dirty && !keepDesign) return ask('design', () => go(to, tab));
    if (sketchOn && !keepSketch) leaveSketch();
    if (designer && !keepDesign) closeDesigner();
    const editTab = to === 'edit' ? t : null;
    if (editTab !== 'scatter') disarmScatter();
    if (editTab !== 'build') disarmBuild();
    if (editTab !== 'sites') disarmPlace();
    pickTool = null;
    if (to && to !== shell.section) {
      shell.collapsed = false;
      shell.card = false;
      shell.snap = 'half';
    }
    shell.section = to;
    if (to && t) (shell.tabs as Record<string, string>)[to] = t;
    if (editTab === 'scatter') armScatter();
    if (editTab === 'build') armBuild();
    if (editTab === 'design' && !designer) void openDesigner();
    if (to === 'world' && t === 'sketch' && !sketchOn) enterSketch();
    if (to === 'play') startPlay();
    syncTool();
  }

  /** A section's button or key: open it, or shut it if it is open (a folded dock unfolds). */
  function toggle(s: Section) {
    if (shell.section === s && shell.collapsed) shell.collapsed = false;
    else go(shell.section === s ? null : s);
  }

  /** A tab's button: open it, or shut the panel if that tab is showing. */
  function flip(s: Exclude<Section, 'play'>, tab: string) {
    go(shell.section === s && shell.tabs[s] === tab ? null : s, tab);
  }

  /** Ask before work would be dropped; `proceed` carries on once it is dealt with. */
  function ask(kind: 'sketch' | 'design', proceed: () => void, generate = true) {
    shell.ask = { kind, proceed, generate };
  }

  async function answer(choice: 'keep' | 'drop' | 'commit') {
    const a = shell.ask;
    shell.ask = null;
    if (!a || choice === 'keep') return;
    if (a.kind === 'sketch') {
      // Generating keeps the sketch (as the world's), then carries on.
      if (choice === 'commit') await generate(draftWorld(), true);
      else leaveSketch();
    } else {
      if (choice === 'commit') saveDesign();
      closeDesigner();
    }
    a.proceed();
  }

  // Screenshot and check scripts open panels and sessions through this.
  if (import.meta.env.DEV) Object.assign(window, { __ui: { shell, go, toggle, startPlay, stopPlay, undo, redo, edits: () => edits, applyEdits, select: (s: Selection) => select(s), openNotebook: (t: 'npcs' | 'plots' | 'places', id: string | null) => openNotebook(t, id), leaveSite: () => leaveSite(), state: () => ({ playOn, sketchOn, designer: !!designer, tool: view.tool?.constructor?.name ?? (view.tool ? 'tool' : null) }) } });

  /** Leave sketch mode (new strokes asked about first); World shows Generate. */
  function stopSketch() {
    if (sketchDirty()) return ask('sketch', stopSketch);
    if (sketchOn) leaveSketch();
    shell.tabs.world = 'generate';
  }

  /** Open a world (saved or from a file); new sketch strokes are asked about first. */
  function openWorld(w: WorldFile) {
    if (sketchDirty()) return ask('sketch', () => void generate(w), false);
    void generate(w);
  }

  /** Phones: the sheet shows the selected place's card (no panel open, or asked for). */
  const cardSheet = $derived(shell.phone && !!selection && (!shell.section || shell.card));
  // A new place chosen on a phone comes up as a strip (name and actions) over the map.
  $effect(() => {
    if (!selection) shell.card = false;
    else if (untrack(() => shell.phone && !shell.section)) shell.snap = 'peek';
  });

  /** A press on the map: menus shut, the search box lets go, and on a phone the sheet drops to
   * its strip so the map is free to work on. */
  function mapTouched() {
    shell.menu = null;
    const el = document.activeElement as HTMLElement | null;
    if (focusKind(el) === 'text') el!.blur();
    if (shell.phone && shell.snap !== 'peek' && (shell.section || selection)) shell.snap = 'peek';
  }

  /** Back to the surface (the design guard first). */
  function leaveSite() {
    if (designer?.dirty) return ask('design', leaveSite);
    view.exitBuilding();
  }

  // Build: while its tab is open, the map's pointer draws buildings (or redraws the one being
  // changed).
  let build = $state<BuildSettings>(defaultBuild());
  let buildFuncs = $state<BuildingFuncs | null>(null);
  /** The building drawn by hand being changed (its created id). */
  let buildEditing = $state<string | null>(null);
  let buildArmed: { tool: BuildTool; cross: CrossingTool } | null = null;
  /** The Build tab draws buildings, or puts crossings down. */
  let buildMode = $state<'building' | 'crossing'>('building');
  let cross = $state<CrossSettings>(defaultCross());
  /** The crossing put down by hand being changed (its id). */
  let crossEditing = $state<string | null>(null);

  /** The building drawn by hand behind an id: its created id, or its building id (`b:<layout>:0`). */
  function drawnBuilding(id: string): Created | null {
    const all = edits.created ?? [];
    const m = /^b:(\d+):0$/.exec(id);
    const c = m ? all[Number(m[1]) - settlements.length - sites.length] : all.find((x) => x.id === id);
    return c && c.kind === 'building' && !c.removed ? c : null;
  }

  function armBuild() {
    if (buildArmed) return;
    if (!buildFuncs) void view.gen.buildingFuncs().then((f) => (buildFuncs = f));
    const tool = new BuildTool({
      shape: () => build.shape,
      drawn: (poly) => void buildDrawn(poly),
      hint: (t) => toast(t),
    });
    const crossTool = new CrossingTool({
      settings: () => cross,
      edits: () => edits,
      picked: () => crossEditing,
      placed: crossPlaced,
      pick: pickCrossing,
      hint: (t) => toast(t),
    });
    buildArmed = { tool, cross: crossTool };
  }

  function disarmBuild() {
    buildArmed = null;
    buildEditing = null;
    crossEditing = null;
  }

  function setBuildMode(m: 'building' | 'crossing') {
    buildMode = m;
    buildEditing = null;
    crossEditing = null;
    buildArmed?.cross.reset();
    syncTool();
  }

  /** Both banks clicked: a new crossing there, or the one being changed moved there. */
  function crossPlaced(a: Pt, b: Pt) {
    const c: Crossing = { kind: cross.kind, a, b, width: cross.width };
    const problem = crossingProblem(c);
    if (problem) return toast(problem);
    const editing = crossEditing && edits.crossings?.[crossEditing] ? crossEditing : null;
    const id = editing ?? newObjectId('v');
    applyEdits({ ...edits, crossings: { ...(edits.crossings ?? {}), [id]: c } }, { tool: 'place_crossing', id, kind: c.kind, changed: !!editing }, 'user');
  }

  /** A crossing put down earlier was clicked: the menu shows it, to change or take away. */
  function pickCrossing(id: string) {
    const c = edits.crossings?.[id];
    if (!c) return;
    crossEditing = id;
    cross = { kind: c.kind, width: c.width };
  }

  /** The crossing being changed gets the menu's kind and width (its ends stay). */
  function saveCrossing() {
    const c = crossEditing ? edits.crossings?.[crossEditing] : null;
    if (!c || !crossEditing) return;
    const next: Crossing = { ...c, kind: cross.kind, width: cross.width };
    const problem = crossingProblem(next);
    if (problem) return toast(problem);
    applyEdits({ ...edits, crossings: { ...(edits.crossings ?? {}), [crossEditing]: next } }, { tool: 'place_crossing', id: crossEditing, kind: next.kind, changed: true }, 'user');
  }

  function removeCrossing() {
    const id = crossEditing;
    const c = id ? edits.crossings?.[id] : null;
    if (!id || !c) return;
    const { [id]: _gone, ...rest } = edits.crossings ?? {};
    crossEditing = null;
    applyEdits({ ...edits, crossings: rest }, { tool: 'remove_crossings', ids: [id], kind: c.kind }, 'user');
  }

  /** A footprint drawn: a new building there, or the one being changed moved there. */
  async function buildDrawn(poly: Pt[]) {
    const tool = buildArmed?.tool;
    const editing = buildEditing ? drawnBuilding(buildEditing) : null;
    const id = editing?.id ?? `c:${(edits.created ?? []).length}`;
    const opts = buildOptions(build);
    const spot = await view.gen.buildingSpot(poly, opts.func, id);
    if (tool) tool.pending = null;
    if (!spot) return toast('The map could not answer: try again');
    if ('error' in spot) return toast(spot.error.charAt(0).toUpperCase() + spot.error.slice(1));
    const name = build.name.trim();
    if (editing) {
      const c: Created = { id: editing.id, kind: 'building', x: spot.x, y: spot.y, name: name || editing.name, poly, ...opts };
      const created = (edits.created ?? []).map((x) => (x.id === c.id ? c : x));
      applyEdits({ ...edits, created }, { tool: 'update_building', id: c.id, name: c.name }, 'user');
      return void reselect(c);
    }
    // (Numbered as it is added: another site may have come in meanwhile.)
    const nid = `c:${(edits.created ?? []).length}`;
    const c: Created = { id: nid, kind: 'building', x: spot.x, y: spot.y, name: name || spot.name, poly, ...opts };
    applyEdits({ ...edits, created: [...(edits.created ?? []), c] }, { tool: 'create_building', id: nid, name: c.name }, 'user');
    build.name = '';
  }

  /** Change the building being edited to the menu's choices (its footprint stays). */
  function saveBuilding() {
    const c = buildEditing ? drawnBuilding(buildEditing) : null;
    if (!c) return;
    const { roof: _r, tint: _t, structure: _s, ...rest } = c;
    const next: Created = { ...rest, ...buildOptions(build), name: build.name.trim() || c.name };
    const created = (edits.created ?? []).map((x) => (x.id === c.id ? next : x));
    applyEdits({ ...edits, created }, { tool: 'update_building', id: c.id, name: next.name }, 'user');
    void reselect(next);
  }

  /** The info panel shows a building drawn by hand as it is now (after a change to it). */
  async function reselect(c: Created) {
    const s = selection;
    if (s?.kind !== 'building' || drawnBuilding(s.hit.id)?.id !== c.id) return;
    const hit = await view.gen.query(c.x, c.y);
    if (hit?.kind === 'building' && selection === s) selection = { kind: 'building', hit };
  }

  /** Open the build menu on a building drawn by hand. */
  function editBuilding(id: string) {
    const c = drawnBuilding(id);
    if (!c) return;
    go('edit', 'build');
    buildEditing = c.id;
    build = settingsOf(c, (c.poly?.length ?? 0) >= 12 ? 'tower' : 'rect');
  }

  // The dungeon designer: while it is open, the map's pointer changes the site we are in (a
  // draft, saved into the edits on Save).
  let designer = $state.raw<SiteDesigner | null>(null);
  let designVersion = $state(0);
  /** The design breaks a rule a site must keep: it can't be saved. */
  const designBlocked = $derived((void designVersion, !!designer?.blocking.length));
  let designSettings = $state<DesignSettings>(defaultDesign());
  let underCatalog = $state.raw<UnderCatalog | null>(null);
  async function openDesigner() {
    const id = inside?.id;
    if (!id || !id.startsWith('u:') || designer) return;
    if (!underCatalog) underCatalog = await view.gen.underCatalog();
    const d = new SiteDesigner(view, id, {
      settings: () => designSettings,
      catalog: () => underCatalog,
      changed: () => designVersion++,
      hint: (t) => toast(t),
    });
    // (Still wanted once it has loaded: the same site, the Design tab open.)
    if (!(await d.open()) || inside?.id !== id || designer || shell.section !== 'edit' || shell.tabs.edit !== 'design') return;
    // New rooms are, at first, the kind this site has most of its own.
    const t = underCatalog.themes.find((x) => x.key === d.draft?.theme);
    designSettings.kind = t?.rooms.find((k) => k !== t.first && k !== t.passage) ?? 'chamber';
    designer = d;
    view.designing = id;
    syncTool();
  }

  /** Shut the designer; unsaved changes are dropped (the site shown as saved). */
  function closeDesigner() {
    const d = designer;
    if (!d) return;
    designer = null;
    view.designing = null;
    // (Shut while its tab is showing: Edit falls back to Names.)
    if (shell.tabs.edit === 'design') shell.tabs.edit = 'names';
    syncTool();
    if (d.dirty && view.interior?.interior.id === d.id) void view.enterBuilding(d.id, { level: view.interior.currentLevel });
  }

  function saveDesign() {
    const d = designer;
    if (!d?.draft || d.blocking.length) return;
    const e = edits;
    applyEdits({ ...e, designs: { ...(e.designs ?? {}), [d.id]: JSON.parse(JSON.stringify(d.draft)) } }, { tool: 'set_site_design', id: d.id, name: inside?.name }, 'user');
    d.markSaved();
  }

  /** The site as generated again: its design dropped. */
  function resetDesign() {
    const d = designer;
    if (!d) return;
    const e = edits;
    const designs = { ...(e.designs ?? {}) };
    delete designs[d.id];
    applyEdits({ ...e, designs }, { tool: 'reset_site_design', id: d.id, name: inside?.name }, 'user');
    void d.open();
  }

  /** Every change made to this world cleared: the world as generated. One step, so Undo brings
   * it all back (while this page is open); mapd's copy is cleared too. */
  function startOver() {
    if (designer) closeDesigner();
    view.exitBuilding();
    selection = null;
    applyEdits({}, { tool: 'start_over' }, 'user');
  }

  /** Show a problem's square (on its level). */
  function showProblem(level: number, at?: [number, number]) {
    view.setInteriorLevel(level);
    const layer = view.interior;
    if (at && layer) {
      const [x, y] = layer.toWorld(at[0] + 0.5, at[1] + 0.5);
      flyToSize(x, y, 120);
    }
  }

  // Scatter: while its menu is open, the map's pointer puts objects down and takes them away.
  let scatter = $state<ScatterSettings>(defaultScatter());
  /** The brush ring under the pointer (screen px). */
  let ring = $state<{ x: number; y: number; r: number } | null>(null);
  let scatterArmed: { tool: PointerTool } | null = null;
  const placedSprites = $derived.by(() => {
    const n: Record<string, number> = {};
    for (const o of Object.values(edits.objects ?? {})) if (typeof o.kind === 'string') n[o.kind.slice(2)] = (n[o.kind.slice(2)] ?? 0) + 1;
    return n;
  });

  function armScatter() {
    if (scatterArmed) return;
    const tool = new ScatterTool({
      settings: () => scatter,
      edits: () => edits,
      catalog: () => view.battle?.catalog ?? [],
      objectAt: (x, y) => view.battle?.objectAt(x, y) ?? null,
      preview: (objs) => view.previewObjects(objs),
      commit: (objects, cleared, label) => commitObjects(objects, cleared, label),
      ring: (x, y, r) => {
        if (r === null) return (ring = null);
        const [sx, sy] = view.cam.worldToScreen(x, y);
        const b = container.getBoundingClientRect();
        ring = { x: b.left + sx, y: b.top + sy, r: r * view.cam.ppf };
      },
      battlemap: () => (view.battle?.stats.alpha ?? 0) > 0,
      hint: (t) => toast(t),
    });
    scatterArmed = { tool };
  }

  function disarmScatter() {
    scatterArmed = null;
    ring = null;
  }

  /** Objects put down (or, null, taken away) and generated ones cleared, as one change. */
  function commitObjects(objects: Record<string, Placed | null>, cleared: Record<string, Clear>, label: { tool: string; count: number; name?: string }) {
    const e = edits;
    const o = { ...(e.objects ?? {}) };
    for (const [k, v] of Object.entries(objects)) {
      if (v) o[k] = v;
      else delete o[k];
    }
    applyEdits({ ...e, objects: o, cleared: { ...(e.cleared ?? {}), ...cleared } }, { ...label, ...(Object.keys(cleared).length ? { clears: Object.keys(cleared).length } : {}) }, 'user');
  }

  /** A picture as a new kind of object: drawn down to 64 px a square (128 for one square),
   * kept as an asset, described in `Edits.sprites`; then chosen in the menu. */
  async function uploadSprite(file: File, meta: SpriteMeta) {
    const canvas = await shrink(file, meta.size <= 1 ? 128 : Math.min(512, Math.round(meta.size * 64)));
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('The picture could not be read'))), 'image/png'));
    const id = await putAsset(blob, meta.name);
    const e = edits;
    applyEdits({ ...e, sprites: { ...(e.sprites ?? {}), [id]: meta } }, { tool: 'upload_sprite', id: `s:${id}`, name: meta.name }, 'user');
    scatter.kinds = [`s:${id}`];
  }

  /** Change an uploaded sprite's rules, or (null) remove it and every one put down. */
  function saveSprite(asset: string, meta: SpriteMeta | null) {
    const e = edits;
    const sprites = { ...(e.sprites ?? {}) };
    const name = sprites[asset]?.name ?? asset;
    if (meta) {
      sprites[asset] = meta;
      return applyEdits({ ...e, sprites }, { tool: 'upload_sprite', id: `s:${asset}`, name: meta.name }, 'user');
    }
    delete sprites[asset];
    const objects = Object.fromEntries(Object.entries(e.objects ?? {}).filter(([, o]) => o.kind !== `s:${asset}`));
    applyEdits({ ...e, sprites, objects }, { tool: 'remove_sprite', id: `s:${asset}`, name }, 'user');
  }

  /** The next click on the map puts `c` there. */
  function armPlace(c: SiteChoice, label = 'site') {
    placeLabel = label;
    disarmPlace();
    const tool: PointerTool = {
      down: () => true,
      move: () => {},
      hover: () => {},
      cancel: () => disarmPlace(),
      up: (x, y) => {
        disarmPlace();
        void createSite(c, x, y);
      },
    };
    placeTool = tool;
    placeArmed = true;
    if (shell.phone) shell.snap = 'peek';
    syncTool();
  }

  function disarmPlace() {
    placeTool = null;
    placeArmed = false;
    syncTool();
  }

  /** Create a site near (x, y), where the generator allows (dry land, out of rivers), named
   * locally unless a name was given; then select it. */
  async function createSite(c: SiteChoice, x: number, y: number) {
    const spot = await view.gen.spot(c.kind, c.under, `c:${(edits.created ?? []).length}`, x, y);
    if (!spot) return toast('The map could not answer: try again');
    if ('error' in spot) return toast(spot.error.charAt(0).toUpperCase() + spot.error.slice(1));
    // (Numbered as it is added: another site may have come in meanwhile.)
    const id = `c:${(edits.created ?? []).length}`;
    const { name, ...opts } = c;
    const site: Created = { id, ...opts, x: spot.x, y: spot.y, name: name ?? spot.name };
    applyEdits({ ...edits, created: [...(edits.created ?? []), site] }, { tool: 'create_feature', id, kind: c.kind === 'entrance' ? (c.under ?? 'dungeon').replace(/_/g, ' ') : c.kind.replace(/_/g, ' '), name: site.name }, 'user');
    const f = overlay?.features.find((g) => g.id === id);
    if (f) selection = { kind: 'feature', feature: f };
  }

  /** The info panel's actions for a created site: hide or show it, delete it, go down into its
   * site underground. */
  function createdActions(id: string) {
    const c = edits.created?.find((x) => x.id === id) ?? drawnBuilding(id);
    if (!c) return {};
    id = c.id;
    const li = layoutIndexOf(id);
    const drawn =
      c.kind === 'building'
        ? {
            onEdit: () => editBuilding(id),
            onGoIn: c.structure === 'ruin' ? undefined : () => void view.enterBuilding(`b:${li}:0`),
          }
        : {};
    const hiddenNow = (edits.hidden ?? []).includes(id);
    const under = !['tower', 'camp', 'waystation', 'building'].includes(c.kind);
    return {
      ...drawn,
      hidden: hiddenNow,
      onHide: (hide: boolean) => {
        const set = new Set(edits.hidden ?? []);
        if (hide) set.add(id);
        else set.delete(id);
        applyEdits({ ...edits, hidden: [...set].sort() }, { tool: 'hide_feature', id, hidden: hide }, 'user');
      },
      onDelete: () => {
        const all = (edits.created ?? []).map((x) => (x.id === id ? { ...x, removed: true } : x));
        applyEdits({ ...edits, created: all }, { tool: 'delete_feature', id }, 'user');
      },
      onDown: under
        ? () => {
            const li = layoutIndexOf(id);
            if (li >= 0) void view.enterBuilding(`u:${li}:0`);
          }
        : undefined,
    };
  }

  // The notebook: NPCs and plot points (the user's and agents').
  let notebookFocus = $state<{ tab: 'npcs' | 'plots' | 'places'; id: string | null; seq: number }>({ tab: 'npcs', id: null, seq: 0 });

  function openNotebook(tab: 'npcs' | 'plots' | 'places', id: string | null) {
    notebookFocus = { tab, id, seq: notebookFocus.seq + 1 };
    go('notes', tab);
  }

  /** The place in view for the notebook: the building or site we are in, else the selection. */
  const here = $derived.by<Here | null>(() => {
    if (inside) return { id: inside.id, name: inside.name, level: inside.level };
    const s = selection;
    if (!s) return null;
    return s.kind === 'feature' ? { id: s.feature.id, name: renames[s.feature.id] ?? s.feature.name } : { id: s.hit.id, name: hitName(s.hit, renames) };
  });

  /** A settlement's or site's layout index (else -1): what is in its buildings counts as there. */
  function layoutIndexOf(id: string): number {
    const n = settlements.length;
    const i = settlements.findIndex((f) => f.id === id);
    if (i >= 0) return i;
    const k = sites.findIndex((f) => f.id === id);
    if (k >= 0) return n + k;
    const c = (edits.created ?? []).findIndex((x) => x.id === id);
    return c >= 0 ? n + sites.length + c : -1;
  }

  function atPlace(id: string): (at: string | undefined) => boolean {
    const li = layoutIndexOf(id);
    const inLayout = li >= 0 ? new RegExp(`^[bdtukw]:${li}:`) : null;
    return (at) => !!at && (at === id || !!inLayout?.test(at));
  }

  const selectedId = $derived(selection ? (selection.kind === 'feature' ? selection.feature.id : selection.hit.id) : null);
  const npcsHere = $derived.by<[string, Npc][]>(() => {
    if (!selectedId) return [];
    const at = atPlace(selectedId);
    return Object.entries(npcs).filter(([, n]) => at(n.location?.id));
  });
  const plotsHere = $derived.by<[string, Plot][]>(() => {
    if (!selectedId) return [];
    const at = atPlace(selectedId);
    return Object.entries(plots).filter(([, p]) => p.anchors.some((a) => at(a)));
  });

  function saveNpc(id: string, npc: Npc | null, tool: string) {
    const all = { ...(edits.npcs ?? {}) };
    const name = npc?.name ?? all[id]?.name ?? id;
    if (npc) all[id] = npc;
    else delete all[id];
    // A deleted NPC leaves the plots they were in.
    const inPlots = npc ? edits.plots : Object.fromEntries(Object.entries(edits.plots ?? {}).map(([k, p]) => [k, p.npcs.includes(id) ? { ...p, npcs: p.npcs.filter((x) => x !== id) } : p]));
    applyEdits({ ...edits, npcs: all, plots: inPlots }, { tool, id, name }, 'user');
  }

  function savePlot(id: string, plot: Plot | null, tool: string) {
    const all = { ...(edits.plots ?? {}) };
    const name = plot?.title ?? all[id]?.title ?? id;
    if (plot) all[id] = plot;
    else delete all[id];
    applyEdits({ ...edits, plots: all }, { tool, id, name }, 'user');
  }

  function setNote(id: string, note: { text: string; tags?: string[] } | null) {
    const all = { ...notes };
    if (note) all[id] = note;
    else delete all[id];
    applyEdits({ ...edits, notes: all }, { tool: 'annotate_feature', id }, 'user');
  }

  /** Show a place: a map feature is selected (and framed); a building or district is found
   * where it stands and selected; anything else (levels, rooms) is gone to. */
  async function showPlace(id: string) {
    const f = overlay?.features.find((g) => g.id === id);
    if (f) {
      if (view.interior) view.exitBuilding();
      return select({ kind: 'feature', feature: f });
    }
    if (/^[bd]:/.test(id)) {
      const p = await view.gen.place(id);
      const hit = p ? await view.gen.query(p.x, p.y) : null;
      if (hit && (hit.kind === 'building' || hit.kind === 'district') && hit.id === id) {
        if (view.interior) view.exitBuilding();
        return select(hit.kind === 'building' ? { kind: 'building', hit } : { kind: 'district', hit });
      }
    }
    await goTo(id);
  }

  /** A new NPC or plot point at the selected place, opened in the notebook. */
  function addHere(tab: 'npcs' | 'plots') {
    const at = here;
    if (!at) return;
    const id = newNoteId(tab === 'npcs' ? 'n' : 'p');
    if (tab === 'npcs') saveNpc(id, { ...blankNpc(), location: { id: at.id, ...(at.level !== undefined ? { level: at.level } : {}) } }, 'create_npc');
    else savePlot(id, { ...blankPlot(), anchors: [at.id] }, 'create_plot');
    openNotebook(tab, id);
  }

  /** Where an NPC stands in the building or site in view (world ft), if they are on its level. */
  function npcInView(id: string): [number, number] | null {
    const n = npcs[id];
    const it = view.interior;
    const l = n?.location;
    if (!it || !l || l.id !== it.interior.id) return null;
    const level = l.level ?? it.interior.entry_level;
    if (level !== it.currentLevel) return null;
    return it.toWorld(...it.npcSpot({ id, name: n.name, level, at: l.x !== undefined && l.y !== undefined ? [l.x, l.y] : undefined }));
  }

  /** A place's name and position: map features from the overlay (as renamed), the rest
   * (buildings, districts, ways underground) from the generators. */
  async function placeInfo(id: string) {
    const f = overlay?.features.find((g) => g.id === id);
    if (f) return { id, name: renames[id] ?? f.name, generated: f.name, x: f.x, y: f.y };
    const p = await view.gen.place(id);
    return p && { ...p, name: renames[id] ?? p.generated ?? p.name };
  }

  /** Go to a place, or to an NPC: into the building or site they are in, on their level. */
  async function goTo(id: string) {
    if (designer?.dirty) return ask('design', () => void goTo(id));
    const n = npcs[id];
    const place = n ? n.location : { id };
    if (!place) return;
    // A site's level or room (`l:<site>:<level>`, `r:<site>:<level>:<room>`): into the site, on that level.
    const room = /^l:(.+):(\d+)$/.exec(place.id) ?? /^r:(.+):(\d+):\d+$/.exec(place.id);
    if (room) {
      const [site, lv] = [room[1], Number(room[2])];
      if (view.interior?.interior.id === site || (await view.enterBuilding(site))) view.setInteriorLevel(lv);
      if (place.id.startsWith('r:')) {
        const p = await view.gen.place(place.id);
        if (p) flyToSize(p.x, p.y, 120);
      }
      return;
    }
    if (/^[btuwk]:/.test(place.id)) {
      const lv = place.level;
      if (view.interior?.interior.id === place.id) {
        if (lv !== undefined) view.setInteriorLevel(lv);
      } else if (!(await view.enterBuilding(place.id, lv !== undefined ? { level: lv } : undefined))) {
        const p = await view.gen.place(place.id);
        if (p) flyToSize(p.x, p.y, 300);
        return;
      }
      const at = n ? npcInView(id) : null;
      if (at) flyToSize(at[0], at[1], 120);
      return;
    }
    if (place.x !== undefined && place.y !== undefined) return flyToSize(place.x, place.y, 400);
    const f = overlay?.features.find((g) => g.id === place.id);
    if (f) return flyToSize(f.x, f.y, frameSize(f));
    const p = await view.gen.place(place.id);
    if (p) flyToSize(p.x, p.y, /^d:/.test(place.id) ? 1500 : 300);
  }

  /** Go to a name in the names menu: a map feature is selected (and framed); the rest as `goTo`. */
  async function goName(id: string) {
    if (designer?.dirty) return ask('design', () => void goName(id));
    const f = overlay?.features.find((g) => g.id === id);
    if (f) {
      if (view.interior) view.exitBuilding();
      return select({ kind: 'feature', feature: f });
    }
    if (view.interior && !/^[lr]:/.test(id) && view.interior.interior.id !== id) view.exitBuilding();
    await goTo(id);
  }

  /** The named place nearest a point (within 3 mi; not seas or land masses). */
  function nearestNamed(x: number, y: number): string | null {
    let best: string | null = null;
    let bestD = 3 * 5280;
    for (const g of overlay?.features ?? []) {
      if (['ocean', 'sea', 'continent', 'island'].includes(g.kind)) continue;
      const d = Math.hypot(g.x - x, g.y - y);
      if (d < bestD) [best, bestD] = [g.id, d];
    }
    return best;
  }

  /** Click on the map to put an NPC there: inside the building or site in view, else at the
   * building, district or settlement under the click (else the nearest named place). */
  function pickPlace(id: string) {
    const done = () => {
      if (pickTool !== tool) return;
      pickTool = null;
      syncTool();
    };
    const tool: PointerTool = {
      down: () => true,
      move: () => {},
      hover: () => {},
      cancel: done,
      up: (x, y) => {
        done();
        void (async () => {
          const n = npcs[id];
          if (!n) return;
          let location: Npc['location'] | null = null;
          const it = view.interior;
          if (it?.inspect(x, y)) location = { id: it.interior.id, level: it.currentLevel, x, y };
          else {
            const hit = await view.gen.query(x, y);
            const town = hit?.kind === 'settlement' && overlay ? settlementAt(overlay, hit.x, hit.y)?.id : null;
            const at = hit?.kind === 'building' || hit?.kind === 'district' ? hit.id : (town ?? nearestNamed(x, y));
            if (at) location = { id: at, x, y };
          }
          if (!location) return toast('Nothing named there: pick a building, a settlement, or near a site');
          saveNpc(id, { ...$state.snapshot(n), location }, 'place_npc');
        })();
      },
    };
    pickTool = tool;
    if (shell.phone) shell.snap = 'peek';
    syncTool();
    toast(`${shell.phone ? 'Tap' : 'Click'} on the map where ${npcs[id]?.name ?? 'they'} can be found`);
  }

  /** An NPC into play as a token: where they stand inside, else the middle of the view. */
  async function dropToken(id: string) {
    const n = npcs[id];
    if (!n) return;
    const at = npcInView(id);
    const picture = n.portrait ? ((await getAsset(n.portrait))?.blob ?? null) : null;
    const t = await play.dropNpc(n.name, picture, ...(at ?? []));
    toast(t ? `${n.name} is on the map` : 'There is nowhere to put a token here');
  }

  const clampZoom = (z: number) => Math.max(view.cam.minZoom, Math.min(view.cam.maxZoom, z));

  /** Fly to frame `size` ft round a point in the part of the map the UI leaves open (beside the
   * place card and the dock, above a phone's sheet and tab bar). */
  function flyToSize(x: number, y: number, size: number) {
    const [W, H] = [view.cam.width, view.cam.height];
    const css = getComputedStyle(document.documentElement);
    const px = (name: string) => parseFloat(css.getPropertyValue(name)) || 0;
    const left = !shell.phone && selection ? 324 : 0;
    const right = px('--dock-w');
    const top = px('--top-h') + 12;
    const bottom = px('--sheet-h') + px('--tabbar-h');
    const [w, h] = [Math.max(160, W - left - right), Math.max(160, H - top - bottom)];
    const zoom = clampZoom(zoomFor(size, w, h));
    const ppf = 2 ** zoom;
    view.flyTo({ cx: x + (W / 2 - (left + w / 2)) / ppf, cy: y + (H / 2 - (top + h / 2)) / ppf, zoom });
  }

  function flyToSelection(s: Selection) {
    if (s.kind === 'feature') flyToSize(s.feature.x, s.feature.y, frameSize(s.feature));
    else if (s.kind === 'district') flyToSize(s.hit.x, s.hit.y, Math.max(600, s.hit.size_ft * 1.3));
    else flyToSize(s.hit.x, s.hit.y, Math.max(120, s.hit.size_ft * 6));
  }

  function select(s: Selection) {
    selection = s;
    flyToSelection(s);
  }

  async function pick(x: number, y: number, sx: number, sy: number) {
    const f = view.labels?.hit(sx, sy);
    const shop = f?.kind === 'place' ? view.labels?.placeHit(f.id) : null;
    if (shop) {
      selection = { kind: 'building', hit: shop };
      return;
    }
    if (f?.kind === 'district') {
      const kind = (f.detail ?? '').replace(/ district$/, '');
      selection = { kind: 'district', hit: { kind: 'district', id: f.id, settlement: Number(f.id.split(':')[1]), name: f.name, district_kind: kind, x: f.x, y: f.y, size_ft: f.extent_ft * 0.7 } };
      return;
    }
    if (f) {
      selection = { kind: 'feature', feature: f };
      return;
    }
    const hit = await view.gen.query(x, y);
    if (hit?.kind === 'building') selection = { kind: 'building', hit };
    else if (hit?.kind === 'district') selection = { kind: 'district', hit };
    else if (hit?.kind === 'settlement' && overlay) {
      const feature = settlementAt(overlay, hit.x, hit.y);
      selection = feature ? { kind: 'feature', feature } : null;
    } else selection = null;
  }

  /** Undo and redo go to the designer's history while it is open, then the sketch's (which has
   * no redo), then the world's edits. */
  function undoAny() {
    if (designer) designer.undo();
    else if (sketchOn) sketcher.undo();
    else undo();
  }

  function redoAny() {
    if (designer) designer.redo();
    else if (!sketchOn) redo();
  }

  // Undo and redo as the buttons show them (following the designer's, the sketch's or the
  // world's history).
  const canUndo = $derived((void designVersion, void sketchVersion, void undoHistory.version, designer ? designer.canUndo : sketchOn ? sketcher.canUndo : undoHistory.canUndo));
  const canRedo = $derived((void designVersion, void undoHistory.version, designer ? designer.canRedo : !sketchOn && undoHistory.canRedo));
  const undoLabel = $derived(designer || sketchOn ? null : undoHistory.undoLabel);
  const redoLabel = $derived(designer || sketchOn ? null : undoHistory.redoLabel);

  /** The battlemap grid, shown or not (kept by the map across worlds). */
  let gridOn = $state(true);
  function setGrid(on: boolean) {
    gridOn = on;
    view.setGrid(on);
  }

  /** A zoom button: a step in or out, about the middle of the view. */
  function zoomStep(d: number) {
    view.flyTo({ cx: view.cam.cx, cy: view.cam.cy, zoom: clampZoom(view.cam.zoom + d) }, 2);
  }

  /** Out to the whole map (out of the building or site first, the design guard asking). */
  function wholeMap() {
    if (designer?.dirty) return ask('design', wholeMap);
    if (view.interior) view.exitBuilding();
    view.fitWorld();
  }

  // Every keyboard shortcut (the table in keymap.ts), and what Escape steps back from.
  const keymap = buildKeymap({
    section: () => shell.section,
    editTab: () => shell.tabs.edit,
    playOn: () => playOn,
    sketching: () => sketchOn,
    designing: () => !!designer,
    inside: () => !!view.interior,
    building: () => !!buildArmed,
    tokensSelected: () => play.selection.size > 0,
    toggle,
    editTabTo: (t) => go('edit', t),
    focusSearch: () => {
      const el = document.getElementById('map-search') as HTMLInputElement | null;
      el?.focus();
      el?.select();
    },
    help: () => (shell.help = !shell.help),
    stats: () => shell.setStats(!shell.stats),
    wholeMap,
    grid: () => setGrid(!gridOn),
    places: () => view.setPlaces(!view.places),
    zoom: (dz) => view.zoomBy(dz),
    pan: (dx, dy) => view.panBy(dx, dy),
    level: (d) => view.levelBy(d),
    undo: undoAny,
    redo: redoAny,
    generate: () => (sketchOn ? exitSketch(true) : void generate(draftWorld(), true)),
    saveDesign: () => {
      if (designBlocked) toast('Fix the problems marked ✕ before saving');
      else if (!designer?.dirty) toast('Nothing new to save');
      else saveDesign();
    },
    sketchTool: (t) => (sketchSettings.tool = t),
    scatterMode: (m) => {
      scatter.mode = m;
      if (m === 'stamp' && scatter.kinds.length > 1) scatter.kinds = [scatter.kinds[0]];
    },
    buildCross: () => {
      if (buildMode !== 'crossing') return setBuildMode('crossing');
      const order = ['bridge', 'ford', 'ferry'] as const;
      const kind = order[(order.indexOf(cross.kind) + 1) % order.length];
      cross = { kind, width: crossEditing ? cross.width : kind === 'ford' ? 10 : 12 };
    },
    buildShape: (k) => {
      if (buildMode !== 'building') setBuildMode('building');
      // A round tower is a wizard's tower unless it was something else already.
      build = k === 'tower' && build.func === 'house' ? { ...build, shape: k, func: 'wizard_tower', floors: Math.max(build.floors, 4) } : { ...build, shape: k };
    },
    buildFinish: () => buildArmed?.tool.finish(),
    buildBack: () => buildArmed?.tool.back(),
    designMode: (m) => (designSettings = { ...designSettings, mode: m }),
    playTool: (t) => play.setTool(t),
    removeTokens: () => play.removeSelected(),
    escape: [
      () => (shell.help ? ((shell.help = false), true) : false),
      () => (shell.menu || moves ? ((shell.menu = null), (moves = null), true) : false),
      () => (shell.ask ? (void answer('keep'), true) : false),
      () => {
        const el = document.activeElement as HTMLElement | null;
        if (focusKind(el) !== 'text') return false;
        el!.blur();
        return true;
      },
      () => !!buildArmed?.tool.reset(),
      () => !!buildArmed?.cross.reset(),
      () => (crossEditing ? ((crossEditing = null), true) : false),
      () => (placeArmed ? (disarmPlace(), true) : false),
      () => (pickTool ? ((pickTool = null), syncTool(), true) : false),
      () => (designer ? (go('edit', 'names'), true) : false),
      () => (shell.phone && shell.card ? ((shell.card = false), true) : false),
      () => (view.interior ? (leaveSite(), true) : false),
      () => (selection ? ((selection = null), true) : false),
    ],
  });

  async function startBench() {
    if (benchRunning) return;
    benchRunning = true;
    bench = null;
    try {
      bench = await runBench(view);
    } finally {
      benchRunning = false;
    }
  }

  onMount(() => {
    view.onHud = (s) => (hud = s);
    // Underground sites take the name of the place they open from (`u:<layout>:<entrance>`,
    // sewer sections `w:<layout>:<sx>:<sy>`), unless renamed.
    view.onInterior = (s) => {
      inside = s && /^[uwk]:/.test(s.id) && !renames[s.id] ? { ...s, name: `${settlementName(Number(s.id.split(':')[1]))} · ${s.name}` } : s;
      // Out of the site being designed: the designer shuts.
      if (designer && s?.id !== designer.id) closeDesigner();
      else if (designer) designer.levelShown();
    };
    view.onPick = (x, y, sx, sy) => void pick(x, y, sx, sy);
    view.onPlaces = (on) => setPlaces(on);
    view.setPlaces(places);
    view.onMoves = (list, x, y) => (moves = list.length ? { list, x, y } : null);
    const onKey = createKeyHandler(() => keymap, () => shell.help || !!shell.ask || !!versionAsk);
    window.addEventListener('keydown', onKey);
    // Changes not yet stored are stored before the page goes.
    const flush = () => saveTimer && flushSave();
    const hidden = () => document.visibilityState === 'hidden' && flush();
    window.addEventListener('pagehide', flush);
    document.addEventListener('visibilitychange', hidden);
    if (lowMemory()) toast('This device has little memory: big cities may close the tab');
    (async () => {
      await view.mount(container);
      await generate(world);
      if (params.get('bench') === 'play') {
        go('play');
        benchRunning = true;
        bench = await runPlayBench(view, play);
        benchRunning = false;
      } else if (params.get('bench') === 'sewer') {
        benchRunning = true;
        bench = await runSewerBench(view);
        benchRunning = false;
      } else if (params.get('bench') === 'dungeon') {
        benchRunning = true;
        bench = await runDungeonBench(view);
        benchRunning = false;
      } else if (params.get('bench') === 'edit') {
        benchRunning = true;
        bench = await runEditBench(
          view,
          () => edits,
          (e) => applyEdits(e, { tool: 'sync', label: 'Bench edit' }, 'user', { record: false, send: false }),
        );
        benchRunning = false;
      } else if (params.get('bench') === 'heavy') {
        benchRunning = true;
        const { runHeavyBench } = await import('./dev/heavy');
        const r = await runHeavyBench(
          view,
          () => edits,
          (e) => applyEdits(e, { tool: 'sync', label: 'Bench edit' }, 'user', { record: false, send: false }),
        );
        if (r.error) toast(r.error);
        benchRunning = false;
      } else if (params.has('bench')) startBench();
    })();
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('pagehide', flush);
      document.removeEventListener('visibilitychange', hidden);
      sync.dispose();
      view.gen.dispose();
    };
  });
</script>

<div class="map" bind:this={container} onpointerdowncapture={mapTouched}></div>

{#snippet askBar()}
  {#if shell.ask}<Confirm ask={shell.ask} blocked={designBlocked} onKeep={() => void answer('keep')} onDrop={() => void answer('drop')} onCommit={() => void answer('commit')} />{/if}
{/snippet}

{#snippet floorPicker()}
  {#if inside}
    <FloorSelector
      state={inside}
      onLevel={(i) => view.setInteriorLevel(i)}
      onExit={leaveSite}
      onDesign={canDesign && !designer ? () => go('edit', 'design') : undefined}
      designed={!!edits.designs?.[inside.id]}
    />
  {/if}
{/snippet}

{#snippet stopPlaying()}
  <button class="ws-btn" onclick={stopPlay} title="End the session: tokens and fog leave the players' window"><Icon name="stop" size={14} /> Stop playing</button>
{/snippet}

{#if ready}
  <TopBar
    {crumbs}
    onFly={(c) => flyToSize(c.x, c.y, c.size)}
    working={busy ? (progress !== null ? `${status} ${Math.round(progress * 100)}%` : status) : null}
    menuOpen={shell.menu === 'main'}
    onMenu={() => (shell.menu = shell.menu === 'main' ? null : 'main')}
  >
    {#snippet search()}
      {#if overlay}
        <Search
          overlay={shown ?? overlay}
          {renames}
          searchBuildings={(q, rect) => view.gen.searchBuildings(q, rect)}
          viewRect={() => view.cam.viewRect()}
          {settlementName}
          onSelect={select}
          {npcs}
          {plots}
          {notes}
          resolve={placeInfo}
          onNpc={(id) => openNotebook('npcs', id)}
          onPlot={(id) => openNotebook('plots', id)}
          onNote={(id) => void showPlace(id)}
        />
      {/if}
    {/snippet}
    {#snippet menu()}
      <MainMenu
        {world}
        playing={playOn}
        onNewWorld={() => go('world', 'generate')}
        onLibrary={() => go('world', 'library')}
        onPlayers={() => play.openPlayerWindow()}
        onClose={() => (shell.menu = null)}
      />
    {/snippet}
  </TopBar>
  <SectionBar playing={playOn} {canUndo} {canRedo} {undoLabel} {redoLabel} onSection={toggle} onUndo={undoAny} onRedo={redoAny} />
  {#if progress !== null}<div class="progress" style:width="{Math.round(progress * 100)}%"></div>{/if}

  {#if cardSheet && selection}
    <Dock
      title="Place"
      icon="pin"
      onClose={() => ((selection = null), (shell.card = false))}
      back={shell.section ? { label: SECTIONS.find((s) => s.key === shell.section)?.label ?? 'panel', go: () => (shell.card = false) } : undefined}
    >
      {#snippet children(peek)}
        {#if selection}{@render placeCard(selection, true, peek)}{/if}
      {/snippet}
    </Dock>
  {:else if shell.section === 'world'}
    <Dock
      title="World"
      icon="globe"
      tabs={[
        { key: 'generate', label: 'Generate', icon: 'globe' },
        { key: 'sketch', label: 'Sketch', icon: 'sketch', dot: sketchOn, title: sketchOn ? 'Sketching' : 'Draw coastlines, ranges, rivers, biomes and settlements' },
        { key: 'library', label: 'Library', icon: 'folder' },
      ]}
      tab={shell.tabs.world}
      onTab={(k) => go('world', k)}
      onClose={() => go(null)}
      ask={shell.ask ? askBar : undefined}
    >
      {#snippet children(peek)}
        {#if shell.tabs.world === 'sketch' && sketchOn}
          <SketchPanel
            {sketcher}
            bind:settings={sketchSettings}
            version={sketchVersion}
            conflicts={sketchConflicts}
            {previewStatus}
            {showPreview}
            {busy}
            {peek}
            onPreview={setShowPreview}
            onConflict={(c) => flyToSize(c.x, c.y, 60 * 5280)}
            onGenerate={() => exitSketch(true)}
            onStop={stopSketch}
          />
        {:else if !peek}
          {#if shell.tabs.world === 'generate'}
            <GenerateTab
              draft={worldDraft}
              {world}
              {busy}
              {status}
              {progress}
              sketching={sketchOn}
              strokes={sketchStrokes.length}
              sketchLand={drawsLand(sketchStrokes)}
              paintedBiomes={sketchStrokes.filter((s) => s.tool === 'biome').length}
              pins={sketchStrokes.filter((s) => s.tool === 'pin').length}
              onGenerate={() => void generate(draftWorld(), true)}
            />
          {:else if shell.tabs.world === 'sketch'}
            {#if playOn}
              <div class="empty">
                <p>Sketching redraws the world, so it waits until the session ends.</p>
                <button class="ws-btn" onclick={stopPlay}><Icon name="stop" size={14} /> Stop playing</button>
              </div>
            {:else}
              <div class="empty">
                <p>Draw coastlines, mountain ranges, rivers, biomes and settlements over the map, then generate the world they describe.</p>
                <button class="ws-btn primary" onclick={enterSketch} disabled={busy}><Icon name="sketch" size={16} /> Start sketching</button>
              </div>
            {/if}
          {:else}
            <LibraryTab {world} {busy} onOpen={openWorld} onStartOver={startOver} />
          {/if}
        {/if}
      {/snippet}
    </Dock>
  {:else if shell.section === 'edit'}
    <Dock
      title="Edit"
      icon="pencil"
      tabs={[
        { key: 'names', label: 'Names', icon: 'tag', kbd: 'N' },
        { key: 'sites', label: 'Sites', icon: 'pin', kbd: 'S' },
        { key: 'build', label: 'Build', icon: 'building', kbd: 'B' },
        { key: 'scatter', label: 'Scatter', icon: 'tree', kbd: 'C' },
        { key: 'design', label: 'Design', icon: 'room', kbd: 'D', disabled: !canDesign && !designer, title: designer || canDesign ? 'Design this site (D)' : playOn ? 'Stop playing to design a site' : 'Go down into a dungeon, cave or mine to design it' },
      ]}
      tab={shell.tabs.edit}
      onTab={(k) => go('edit', k)}
      onClose={() => go(null)}
      ask={shell.ask ? askBar : undefined}
    >
      {#snippet children(peek)}
        {#if namesOn && overlay}
          {#if !peek}
            <Names
              features={overlay.features}
              {renames}
              hidden={edits.hidden ?? []}
              layoutOf={layoutIndexOf}
              list={(scope) => view.gen.names(scope)}
              onRename={rename}
              onGo={(id) => void goName(id)}
            />
          {/if}
        {:else if placeOn}
          <PlacePanel armed={placeArmed} {peek} onPlace={armPlace} onCancel={disarmPlace} />
        {:else if buildOn}
          <BuildPanel
            bind:settings={build}
            funcs={buildFuncs}
            editing={buildEditing ? (drawnBuilding(buildEditing)?.name ?? null) : null}
            mode={buildMode}
            bind:cross
            crossEditing={crossEditing ? (edits.crossings?.[crossEditing]?.kind ?? null) : null}
            {near}
            {peek}
            onSave={saveBuilding}
            onDone={() => ((buildEditing = null), (crossEditing = null))}
            onMode={setBuildMode}
            onCrossSave={saveCrossing}
            onCrossRemove={removeCrossing}
            onZoomIn={() => view.flyTo({ cx: view.cam.cx, cy: view.cam.cy, zoom: clampZoom(Math.max(view.cam.zoom, 0.5)) })}
          />
        {:else if scatterOn}
          <ScatterPanel
            {peek}
              bind:settings={scatter}
              catalog={view.battle?.catalog ?? []}
              sprites={edits.sprites ?? {}}
              placed={placedSprites}
              icon={(k) => view.battle?.icon(k) ?? Promise.resolve(null)}
              picture={assetUrl}
              onUpload={uploadSprite}
              onSprite={saveSprite}
            />
        {:else if designer}
          <DesignPanel
            {peek}
              {designer}
              version={designVersion}
              bind:settings={designSettings}
              catalog={underCatalog}
              name={inside?.name ?? 'the site'}
              {renames}
              saved={!!edits.designs?.[designer.id]}
              onRename={rename}
              onSave={saveDesign}
              onReset={resetDesign}
              onProblem={showProblem}
            />
        {:else if shell.tabs.edit === 'design' && !peek}
          <div class="empty"><p>Opening the designer…</p></div>
        {/if}
      {/snippet}
    </Dock>
  {:else if shell.section === 'notes'}
    <Dock
      title="Notes"
      icon="book"
      tabs={[
        { key: 'npcs', label: `NPCs (${Object.keys(npcs).length})`, icon: 'user' },
        { key: 'plots', label: `Plots (${Object.keys(plots).length})`, icon: 'scroll' },
        { key: 'places', label: `Places (${Object.keys(notes).length})`, icon: 'note' },
      ]}
      tab={notebookFocus.tab}
      onTab={(k) => openNotebook(k as 'npcs' | 'plots' | 'places', null)}
      onClose={() => go(null)}
      ask={shell.ask ? askBar : undefined}
    >
      {#snippet children(peek)}
        {#if !peek}
          <Notebook
            {npcs}
            {plots}
            {renames}
            {here}
            focus={notebookFocus}
            {notes}
            onNote={setNote}
            onShow={(id) => void showPlace(id)}
            playing={playOn}
            resolve={placeInfo}
            levelName={(id, l) => (inside?.id === id ? (inside.levels[l]?.name ?? null) : null)}
            onNpc={saveNpc}
            onPlot={savePlot}
            onGo={(id) => void goTo(id)}
            onPick={pickPlace}
            onToken={(id) => void dropToken(id)}
            onError={(t) => toast(t)}
            onFocus={openNotebook}
          />
        {/if}
      {/snippet}
    </Dock>
  {:else if shell.section === 'play'}
    <Dock title="Play" icon="pawn" onClose={() => go(null)} actions={stopPlaying} ask={shell.ask ? askBar : undefined}>
      {#snippet children(peek)}
        <PlayPanel {play} version={playVersion} {peek} />
      {/snippet}
    </Dock>
  {:else if shell.ask}
    <div class="asking">{@render askBar()}</div>
  {/if}

  {#if selection && !shell.phone}
    <!-- (Short landscape screens fold the card down while a panel is open.) -->
    {@render placeCard(selection, false, shell.short && !!shell.section)}
  {/if}
  {#if shell.phone && selection && shell.section && !shell.card}
    <button class="chip" onclick={() => ((shell.card = true), (shell.snap = 'half'))}><Icon name="pin" size={16} /><span>{here?.name ?? 'Selected place'}</span><Icon name="chevron-up" size={16} /></button>
  {/if}

  {#snippet placeCard(sel: Selection, docked: boolean, peek: boolean)}
    <InfoPanel
      {docked}
      {peek}
      selection={sel}
      {renames}
      {notes}
      {settlementName}
      onRename={rename}
      onNote={setNote}
      {npcsHere}
      {plotsHere}
      onOpen={openNotebook}
      onAdd={addHere}
      {...sel.kind === 'feature' && sel.feature.id.startsWith('c:') ? createdActions(sel.feature.id) : sel.kind === 'building' && drawnBuilding(sel.hit.id) ? createdActions(sel.hit.id) : {}}
      onFly={() => selection && flyToSelection(selection)}
      onClose={() => (selection = null)}
      onEnter={async () => {
        if (selection?.kind !== 'building') return false;
        view.lastWay = { at: [selection.hit.x, selection.hit.y], kind: 'enter', t: performance.now() };
        return view.enterBuilding(selection.hit.id);
      }}
    />
  {/snippet}

  <MapControls
    grid={gridOn}
    {places}
    battlemap={battlemapTier}
    {canUndo}
    {undoLabel}
    onGrid={setGrid}
    onPlaces={(on) => view.setPlaces(on)}
    onZoom={zoomStep}
    onWhole={wholeMap}
    onUndo={undoAny}
    floors={inside ? floorPicker : undefined}
  />
  {#if placeArmed}
    <div class="banner" role="status">
      <Icon name="pin" size={16} /> {shell.phone ? 'Tap' : 'Click'} the map to place the {placeLabel}
      <button class="ws-btn" onclick={disarmPlace}>Cancel</button>
    </div>
  {/if}
  {#if scatterOn && ring}<div class="ring" style="left: {ring.x - ring.r}px; top: {ring.y - ring.r}px; width: {ring.r * 2}px; height: {ring.r * 2}px" class:erase={scatter.mode === 'erase'}></div>{/if}
  {#if hud}
    <Readout {hud} inside={!!inside} />
    {#if shell.stats}<DevStats {hud} {bench} {benchRunning} onBench={startBench} onClose={() => shell.setStats(false)} />{/if}
  {/if}
  <Toasts {toasts} onDismiss={dismiss} />
  {#if shell.help}<ShortcutsHelp list={keymap} onClose={() => (shell.help = false)} />{/if}
  {#if versionAsk}<VersionAsk from={versionAsk.from} to={GEN_VERSION} kept={versionAsk.kept} edited={versionAsk.edited} canCancel={versionAsk.canCancel} onChoose={versionAsk.choose} />{/if}
  {#if PINNED && !pinnedClosed}
    <div class="pinned ws-panel" role="status">
      <span>Worldspring as it was at generator version {GEN_VERSION}, kept for the worlds made with it.</span>
      <button class="ws-btn primary" onclick={() => void upgradeToNewest()}>Open in the newest</button>
      <button class="ws-icon-btn" onclick={() => (pinnedClosed = true)} aria-label="Hide" title="Hide"><Icon name="x" size={16} /></button>
    </div>
  {/if}
  {#if moves && inside}
    <MovesMenu moves={moves.list} x={moves.x} y={moves.y} onMove={(m) => void view.move(m)} onClose={() => (moves = null)} />
  {/if}
{/if}

<style>
  .ring {
    position: fixed;
    z-index: var(--z-ring);
    pointer-events: none;
    border: 2px solid rgba(58, 50, 42, 0.85);
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(245, 236, 214, 0.7);
  }
  .ring.erase {
    border-color: rgba(163, 58, 42, 0.9);
    border-style: dashed;
  }
  .map {
    position: fixed;
    inset: 0;
  }
  .progress {
    position: fixed;
    z-index: var(--z-progress);
    top: 0;
    left: 0;
    height: 3px;
    background: var(--gold);
    transition: width 0.2s;
    pointer-events: none;
  }
  .banner {
    position: fixed;
    z-index: var(--z-chip);
    top: calc(12px + var(--top-h, 44px) + 8px);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 12px;
    font: 13px var(--font);
    color: var(--accent-ink);
    background: var(--accent);
    border-radius: 999px;
    box-shadow: var(--shadow-lg);
    white-space: nowrap;
  }
  .chip {
    position: fixed;
    z-index: var(--z-chip);
    left: 50%;
    transform: translateX(-50%);
    bottom: calc(var(--tabbar-h, 0px) + var(--sheet-h, 0px) + 8px);
    max-width: calc(100vw - 120px);
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 40px;
    padding: 0 12px;
    font: bold 13px var(--font);
    color: var(--ink);
    background: var(--paper-solid);
    border: 1px solid var(--line);
    border-radius: 999px;
    box-shadow: var(--shadow-lg);
    cursor: pointer;
  }
  .chip span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pinned {
    position: fixed;
    bottom: calc(12px + env(safe-area-inset-bottom));
    left: 50%;
    transform: translateX(-50%);
    z-index: var(--z-chip);
    display: flex;
    align-items: center;
    gap: 8px;
    width: max-content;
    max-width: calc(100vw - 32px);
    padding: 6px 6px 6px 12px;
    font-size: 13px;
  }
  .asking {
    position: fixed;
    top: calc(12px + var(--bar-h, 44px) + 8px);
    right: 12px;
    z-index: var(--z-menu);
    width: min(340px, calc(100vw - 24px));
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    color: var(--ink-2);
  }
  .empty p {
    margin: 0;
  }
</style>
