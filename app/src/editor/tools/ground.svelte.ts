// Edit › Ground: the brush that raises, lowers, smooths or puts back the battlemap's ground, or
// draws plateaus and pits (one undo step per stroke). App keeps `syncTool()` / `go()` and calls
// `armGround` / `disarmGround`.
import { untrack } from 'svelte';
import type { Edits, Plateau } from '../../gen/protocol';
import { defaultGround, GroundTool, type GroundSettings } from '../ground';

/** What the Ground tool needs from the app. */
export interface GroundHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: import('../../gen/protocol').Edits;
  readonly container: HTMLDivElement;
  readonly hud: import('../../render/MapView').HudState | null;
  /** (Its ring and chip are the Objects tool's.) */
  readonly objects: import('./objects.svelte').ObjectsTools;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: import('../../gen/protocol').Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
}

export function groundTools(host: GroundHost) {
  // Ground: while its tab is open, the map's pointer brushes the battlemap's ground up, down,
  // smooth or back, or draws plateaus and pits.
  let ground = $state<GroundSettings>(defaultGround());
  /** The plateau chosen (Choose). */
  let groundSel = $state<string | null>(null);
  let groundArmed: GroundTool | null = null;
  /** Hills can be raised on the map's four finest levels too (L11 to L14 on a 1,200-mile world:
   * a mountain miles across is dragged out there), not only on the battlemap. */
  const hillsNear = $derived(!host.view.interior && (host.hud?.layer.level ?? 0) >= (host.view.geom?.max_level ?? 99) - 3);
  /** The battlemap is showing (the ground can be shaped), or the map near enough for a hill. */
  const groundNear = $derived((host.hud?.ftPerPx ?? 99) < 0.7 || (ground.mode === 'hill' && hillsNear));
  const groundChosen = $derived(groundSel && host.edits.plateaus?.[groundSel] ? { id: groundSel, plateau: host.edits.plateaus[groundSel] } : null);

  function armGround() {
    if (groundArmed) return;
    groundArmed = new GroundTool({
      settings: () => ground,
      edits: () => host.edits,
      battlemap: () => (host.view.battle?.stats.alpha ?? 0) > 0 && !host.view.interior,
      hillsHere: () => hillsNear,
      ground: (gx, gy) => host.view.battle?.groundAt(gx, gy) ?? null,
      sea: () => host.view.geom?.sea_level_ft ?? 0,
      preview: (box, at) => host.view.battle?.previewGround(box, at),
      unpreview: () => host.view.battle?.unpreviewGround(),
      commit: (g, p, label) => commitGround(g, p, label),
      ring: (x, y, r) => {
        if (r === null) return (host.objects.ring = null);
        const [sx, sy] = host.view.cam.worldToPage(x, y);
        const b = host.container.getBoundingClientRect();
        host.objects.ring = { x: b.left + sx, y: b.top + sy, r: r * host.view.cam.ppf };
      },
      chip: (x, y, text) => host.objects.chipAt(x, y, text),
      select: (id) => (groundSel = id),
      selected: () => groundSel,
      ppf: () => host.view.cam.ppf,
      hint: (t) => host.toast(t),
    });
  }

  function disarmGround() {
    groundArmed?.cancel();
    groundArmed?.dispose();
    groundArmed = null;
    groundSel = null;
    host.objects.ring = null;
    host.objects.chip = null;
  }

  // The ring and chip follow the settings as the panel changes them.
  $effect(() => {
    void [ground.mode, ground.radius, ground.offset];
    untrack(() => groundArmed)?.refresh();
  });

  /** Ground blocks brushed (or, null, cleared) and plateaus set or taken away, as one change. */
  function commitGround(blocks: Record<string, string | null>, plateaus: Record<string, Plateau | null>, label: { tool: string; mode?: string; count?: number; changed?: boolean }) {
    const e = host.edits;
    const g = { ...(e.ground ?? {}) };
    for (const [k, v] of Object.entries(blocks)) {
      if (v) g[k] = v;
      else delete g[k];
    }
    const h = { ...(e.plateaus ?? {}) };
    for (const [k, v] of Object.entries(plateaus)) {
      if (v) h[k] = v;
      else delete h[k];
    }
    const next: Edits = { ...e };
    if (Object.keys(blocks).length) next.ground = g;
    if (Object.keys(plateaus).length) next.plateaus = h;
    // (A plateau taken away takes its name, note and hidden mark: Undo brings them back.)
    const gone = Object.keys(plateaus).filter((k) => !plateaus[k]);
    if (gone.some((k) => e.renames?.[k] !== undefined)) next.renames = Object.fromEntries(Object.entries(e.renames!).filter(([k]) => !gone.includes(k)));
    if (gone.some((k) => e.notes?.[k] !== undefined)) next.notes = Object.fromEntries(Object.entries(e.notes!).filter(([k]) => !gone.includes(k)));
    if (gone.some((k) => e.hidden?.includes(k))) next.hidden = e.hidden!.filter((k) => !gone.includes(k));
    host.applyEdits(next, label, 'user');
  }

  return {
    get ground() { return ground; },
    set ground(v) { ground = v; },
    get groundArmed() { return groundArmed; },
    set groundArmed(v) { groundArmed = v; },
    get groundNear() { return groundNear; },
    groundChosen,
    armGround,
    disarmGround,
  };
}

export type GroundTools = ReturnType<typeof groundTools>;
