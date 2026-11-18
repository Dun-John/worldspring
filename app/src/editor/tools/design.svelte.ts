// Edit › Rooms: the site designer's lifecycle. While it is open, the map's pointer changes the
// site we are in (a draft, saved into the edits on Save): open, shut, save, reset to the site as
// generated, a problem's square shown; and a building's inside fitted to new storeys. App keeps
// the markup reading these, `L` (ways across) and the undo/redo routing.
import type { BuildingEdit, Edits, PieceEdit, UnderCatalog } from '../../gen/protocol';
import type { InteriorState, MapView } from '../../render/MapView';
import type { Change } from '../../sync/mapd';
import { keepPlaces, shiftLevels } from '../../sync/ops';
import type { Selection } from '../../ui/gazetteer';
import { shell } from '../../ui/shell/layout.svelte';
import { defaultDesign, SiteDesigner, type DesignSettings } from '../site/designer';
import { belowGround } from '../site/model';
import type { BuildTools } from './build.svelte';
import type { ObjectsTools } from './objects.svelte';

/** What the site designer needs from the app. */
export interface DesignHost {
  readonly view: MapView;
  readonly edits: Edits;
  /** The place in view inside, if any. */
  readonly inside: InteriorState | null;
  selection: Selection | null;
  /** Edit › Build (a building's storeys changed with its design). */
  readonly build: Pick<BuildTools, 'refusal' | 'withPiece'>;
  /** Edit › Objects (the designer's chip, place bar and held piece). */
  readonly objects: Pick<ObjectsTools, 'chipAt' | 'placeBar' | 'setHeld'>;
  /** Rooms › Ways across (on, the designer shut). */
  readonly links: { readonly linkOn: boolean };
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  drawnBuilding(id: string): import('../../gen/protocol').Created | null;
  flyToSize(x: number, y: number, size: number): void;
}

export function designTools(host: DesignHost) {
  // The dungeon designer: while it is open, the map's pointer changes the site we are in (a
  // draft, saved into the edits on Save).
  let designer = $state.raw<SiteDesigner | null>(null);
  let designVersion = $state(0);
  /** The design breaks a rule a site must keep: it can't be saved. */
  const designBlocked = $derived((void designVersion, !!designer?.blocking.length));
  let designSettings = $state<DesignSettings>(defaultDesign());
  let underCatalog = $state.raw<UnderCatalog | null>(null);
  async function openDesigner() {
    const id = host.inside?.id;
    if (!id || !/^[ub]:/.test(id) || designer) return;
    if (!underCatalog) underCatalog = await host.view.gen.underCatalog();
    const d = new SiteDesigner(host.view, id, {
      settings: () => designSettings,
      catalog: () => underCatalog,
      changed: () => designVersion++,
      hint: (t) => host.toast(t),
      piece: (kind) => {
        if (kind.startsWith('s:')) {
          const m = host.edits.sprites?.[kind.slice(2)];
          return m ? { name: m.name, blocks: m.blocks_move } : null;
        }
        const c = underCatalog;
        const p = id.startsWith('b:') ? (c?.building.furniture.find((f) => f.kind === kind) ?? c?.building.props.find((f) => f.kind === kind)) : c?.props.find((f) => f.kind === kind);
        return p ? { name: p.name, blocks: p.blocks } : null;
      },
      chip: (x, y, text) => host.objects.chipAt(x, y, text),
      bar: (at) => (host.objects.placeBar = at),
      held: host.objects.setHeld,
      ppf: () => host.view.cam.ppf,
    });
    // (Still wanted once it has loaded: the same site, the Design tab open.)
    if (!(await d.open()) || host.inside?.id !== id || designer || shell.section !== 'edit' || shell.tabs.edit !== 'design') return;
    // New rooms are, at first, the kind this site has most of its own (a building's: a
    // storeroom); the tools and the props are the ones it has.
    const t = underCatalog.themes.find((x) => x.key === d.draft?.theme);
    const inBuilding = d.draft?.kind === 'building';
    const mode = designSettings.mode;
    const bad = inBuilding ? mode === 'corridor' || mode === 'rock' : mode === 'wall' || mode === 'merge';
    designSettings = {
      ...designSettings,
      mode: bad ? 'room' : mode,
      kind: inBuilding ? 'storeroom' : (t?.rooms.find((k) => k !== t.first && k !== t.passage) ?? 'chamber'),
      ...(inBuilding ? { prop: 'table', pw: 2, ph: 1 } : { prop: 'chest', pw: 1, ph: 1 }),
      back: null,
      variant: null,
    };
    designer = d;
    host.view.designing = id;
    host.syncTool();
    // Phones: the site sits in the part of the map above the sheet, not under it.
    if (shell.phone) {
      await new Promise((r) => setTimeout(r, 250));
      const sheet = parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--sheet-h')) || 0;
      if (sheet > 0) host.view.flyTo({ cx: host.view.cam.cx, cy: host.view.cam.cy + sheet / 2 / 2 ** host.view.cam.zoom, zoom: host.view.cam.zoom });
    }
  }

  /** A building's inside designed by hand, fitted to `floors` storeys (`designs` with it), or why
   * it is set aside. */
  async function fitDesign(bid: string, floors: number): Promise<{ designs: Edits['designs']; aside: string }> {
    const designed = host.edits.designs?.[bid];
    if (!designed) return { designs: host.edits.designs, aside: '' };
    const fit = await host.view.gen.design(bid, designed, { refit: floors });
    if (!('error' in fit) && !fit.problems.some((p) => p.blocking)) return { designs: { ...host.edits.designs, [bid]: fit.design }, aside: '' };
    return { designs: host.edits.designs, aside: 'Its inside, designed by hand, can’t follow the new storeys: set aside' };
  }

  /** Shut the designer; unsaved changes are dropped (the site shown as saved). */
  function closeDesigner() {
    const d = designer;
    if (!d) return;
    d.dispose();
    designer = null;
    host.view.designing = null;
    // (Shut while its tab is showing: Edit falls back to Places, unless on to the ways across.)
    if (shell.tabs.edit === 'design' && !host.links.linkOn) shell.tabs.edit = 'sites';
    host.syncTool();
    if (d.dirty && host.view.interior?.interior.id === d.id) void host.view.enterBuilding(d.id, { level: host.view.interior.currentLevel });
  }

  /** The draft into the edits; a building's storeys changed in the designer change the building
   * too (one step). */
  async function saveDesign() {
    const d = designer;
    if (!d?.draft || d.blocking.length) return;
    const draft = JSON.parse(JSON.stringify(d.draft)) as NonNullable<typeof d.draft>;
    const floors = d.storeysChanged ? d.storeys : null;
    const drawn = floors !== null ? host.drawnBuilding(d.id) : null;
    let built: { edit: BuildingEdit | null } | { piece: PieceEdit } | null = null;
    if (floors !== null && !drawn) {
      const r = await host.view.gen.buildingEdit(d.id, { floors });
      if (!r || 'error' in r) return host.toast(host.build.refusal(r) ?? '');
      built = r;
      if (designer !== d) return;
    }
    const e = host.edits;
    const buildings = { ...(e.buildings ?? {}) };
    if (built && 'edit' in built) {
      if (built.edit) buildings[d.id] = built.edit;
      else delete buildings[d.id];
    }
    const created = built && 'piece' in built ? host.build.withPiece(built.piece) : drawn && floors !== null ? (e.created ?? []).map((x) => (x.id === drawn.id ? { ...x, floors } : x)) : e.created;
    // (Levels dug or filled in below: names and places on the others follow them.)
    host.applyEdits({ ...shiftLevels(e, d.id, d.shift), buildings, created, designs: { ...(e.designs ?? {}), [d.id]: draft } }, { tool: 'set_site_design', id: d.id, name: host.inside?.name }, 'user');
    d.markSaved();
  }

  /** The site as generated again: its design dropped (names and places on its levels follow
   * them, if it has more or fewer below ground as generated). */
  async function resetDesign() {
    const d = designer;
    if (!d) return;
    const generated = await host.view.gen.design(d.id, undefined, { original: true });
    if (designer !== d) return;
    const shift = 'error' in generated ? 0 : belowGround(generated.design) - d.belowSaved;
    const e = host.edits;
    const designs = { ...(e.designs ?? {}) };
    delete designs[d.id];
    host.applyEdits({ ...shiftLevels(e, d.id, shift), designs }, { tool: 'reset_site_design', id: d.id, name: host.inside?.name }, 'user');
    void d.open();
  }

  /** Every change made to this world cleared: the world as generated. One step, so Undo brings
   * it all back (while this page is open); mapd's copy is cleared too. */
  function startOver() {
    if (designer) closeDesigner();
    host.view.exitBuilding();
    host.selection = null;
    // (Sites made keep their places, removed: the ids after them are indices, here and in mapd.)
    host.applyEdits(keepPlaces(host.edits, {}), { tool: 'start_over' }, 'user');
  }

  /** Show a problem's square (on its level). */
  function showProblem(level: number, at?: [number, number]) {
    host.view.setInteriorLevel(level);
    const layer = host.view.interior;
    if (at && layer) {
      const [x, y] = layer.toWorld(at[0] + 0.5, at[1] + 0.5);
      host.flyToSize(x, y, 120);
    }
  }

  return {
    get designer() {
      return designer;
    },
    get designVersion() {
      return designVersion;
    },
    get designBlocked() {
      return designBlocked;
    },
    get designSettings() {
      return designSettings;
    },
    set designSettings(v) {
      designSettings = v;
    },
    get underCatalog() {
      return underCatalog;
    },
    fitDesign,
    openDesigner,
    closeDesigner,
    saveDesign,
    resetDesign,
    startOver,
    showProblem,
  };
}

export type DesignTools = ReturnType<typeof designTools>;
