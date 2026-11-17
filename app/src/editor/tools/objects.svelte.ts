// Edit › Objects: the scatter tool (battlemap objects put down or cleared by hand), its ghost,
// ring and chip, the place bar, and pictures uploaded as new kinds of object (one undo step per
// change). App keeps `syncTool()` / `go()` and calls `armScatter` / `disarmScatter`.
import { untrack } from 'svelte';
import type { Clear, Placed, SpriteMeta } from '../../gen/protocol';
import { shell } from '../../ui/shell/layout.svelte';
import type { BarAt, HeldView, Placer } from '../placing';
import { defaultScatter, ScatterTool, type ScatterSettings } from '../scatter';
import { shrink } from '../../render/customAtlas';
import { putAsset } from '../../world/assets';
import { type Change } from '../../sync/mapd';

/** What the Objects tools need from the app. */
export interface ObjectsHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: import('../../gen/protocol').Edits;
  readonly designSettings: import('../site/designer').DesignSettings;
  readonly designer: import('../site/designer').SiteDesigner | null;
  readonly scatterOn: boolean;
  readonly container: HTMLDivElement;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: import('../../gen/protocol').Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
}

export function objectTools(host: ObjectsHost) {
  // Scatter: while its menu is open, the map's pointer puts objects down and takes them away.
  let scatter = $state<ScatterSettings>(defaultScatter());
  /** The brush ring under the pointer (screen px). */
  let ring = $state<{ x: number; y: number; r: number } | null>(null);
  /** The chip by what is in the hand (screen px). */
  let chip = $state<{ x: number; y: number; text: string } | null>(null);
  /** On a touch screen, the place bar by the ghost (world ft), and where it is on screen (kept
   * under the ghost as the map moves). */
  let placeBar = $state.raw<BarAt>(null);
  /** The piece picked up in Objects or Rooms, for the panel's "in hand" row. */
  let held = $state.raw<HeldView>(null);
  // (Told on every pointer move: only a change is taken.)
  const setHeld = (h: HeldView) => {
    if (h?.name !== held?.name || h?.turn !== held?.turn || h?.turns !== held?.turns || h?.auto !== held?.auto || h?.size !== held?.size || h?.kind !== held?.kind || h?.variant !== held?.variant) held = h;
  };
  let placeBarAt = $state<{ x: number; y: number; r: number } | null>(null);
  /** What places things by hand in the open tab: Objects, or Rooms on Props. */
  function placer(): Placer | null {
    // (Inside, Objects has nothing in the hand that shows: its keys go on down, Escape leaves.)
    if (host.scatterOn && scatterArmed && !host.view.interior) return scatterArmed.tool;
    if (host.designer && shell.section === 'edit' && shell.tabs.edit === 'design' && host.designSettings.mode === 'prop') return host.designer;
    return null;
  }

  /** The chip at a world point (none: null text). */
  function chipAt(x: number, y: number, text: string | null) {
    if (text === null) return (chip = null);
    const [sx, sy] = host.view.cam.worldToPage(x, y);
    const b = host.container.getBoundingClientRect();
    chip = { x: b.left + sx, y: b.top + sy, text };
  }

  $effect(() => {
    const at = placeBar;
    if (!at) return void (placeBarAt = null);
    let raf = 0;
    const tick = () => {
      const [sx, sy] = host.view.cam.worldToPage(at.x, at.y);
      const b = host.container.getBoundingClientRect();
      const next = { x: b.left + sx, y: b.top + sy, r: at.r * host.view.cam.ppf };
      if (!placeBarAt || Math.abs(next.x - placeBarAt.x) + Math.abs(next.y - placeBarAt.y) + Math.abs(next.r - placeBarAt.r) > 0.5) placeBarAt = next;
      raf = requestAnimationFrame(tick);
    };
    untrack(tick);
    return () => cancelAnimationFrame(raf);
  });
  let scatterArmed: { tool: ScatterTool } | null = null;
  const placedSprites = $derived.by(() => {
    const n: Record<string, number> = {};
    for (const o of Object.values(host.edits.objects ?? {})) if (typeof o.kind === 'string') n[o.kind.slice(2)] = (n[o.kind.slice(2)] ?? 0) + 1;
    return n;
  });

  function armScatter() {
    if (scatterArmed) return;
    const tool = new ScatterTool({
      settings: () => scatter,
      edits: () => host.edits,
      catalog: () => host.view.battle?.catalog ?? [],
      objectAt: (x, y) => host.view.battle?.objectAt(x, y) ?? null,
      preview: (objs) => host.view.previewObjects(objs),
      commit: (objects, cleared, label) => commitObjects(objects, cleared, label),
      ring: (x, y, r) => {
        if (r === null) return (ring = null);
        const [sx, sy] = host.view.cam.worldToPage(x, y);
        const b = host.container.getBoundingClientRect();
        ring = { x: b.left + sx, y: b.top + sy, r: r * host.view.cam.ppf };
      },
      ghost: (o, ok) => host.view.battle?.setGhost(o, ok),
      chip: (x, y, text) => chipAt(x, y, text),
      bar: (at) => (placeBar = at),
      held: setHeld,
      ppf: () => host.view.cam.ppf,
      battlemap: () => (host.view.battle?.stats.alpha ?? 0) > 0 && !host.view.interior,
      roofed: (x, y) => !!host.view.battle?.roofAt(x, y),
      loaded: (x, y) => !!host.view.battle?.loadedAt(x, y),
      hint: (t) => host.toast(t),
    });
    scatterArmed = { tool };
  }

  function disarmScatter() {
    scatterArmed?.tool.dispose();
    scatterArmed = null;
    ring = null;
    chip = null;
    placeBar = null;
    held = null;
  }

  // The ghost follows the settings as the panel changes them (kind, turn, size).
  $effect(() => {
    void [scatter.mode, scatter.kinds, scatter.kinds.length, scatter.rotation, scatter.scale, scatter.vary];
    scatterArmed?.tool.refresh();
  });
  $effect(() => {
    void [host.designSettings.mode, host.designSettings.prop, host.designSettings.pw, host.designSettings.ph, host.designSettings.back, host.designSettings.variant];
    untrack(() => host.designer)?.refresh();
  });

  /** Objects put down (or, null, taken away) and generated ones cleared, as one change. */
  function commitObjects(objects: Record<string, Placed | null>, cleared: Record<string, Clear>, label: { tool: string; count: number; name?: string; changed?: string[] }) {
    const e = host.edits;
    const o = { ...(e.objects ?? {}) };
    for (const [k, v] of Object.entries(objects)) {
      if (v) o[k] = v;
      else delete o[k];
    }
    host.applyEdits({ ...e, objects: o, cleared: { ...(e.cleared ?? {}), ...cleared } }, { ...label, ...(Object.keys(cleared).length ? { clears: Object.keys(cleared).length } : {}) }, 'user');
  }

  async function uploadSprite(file: File, meta: SpriteMeta) {
    const canvas = await shrink(file, meta.size <= 1 ? 128 : Math.min(512, Math.round(meta.size * 64)));
    const blob = await new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('The picture could not be read'))), 'image/png'));
    const id = await putAsset(blob, meta.name);
    const e = host.edits;
    host.applyEdits({ ...e, sprites: { ...(e.sprites ?? {}), [id]: meta } }, { tool: 'upload_sprite', id: `s:${id}`, name: meta.name }, 'user');
    scatter.kinds = [`s:${id}`];
  }

  /** Change an uploaded sprite's rules, or (null) remove it and every one put down. */
  function saveSprite(asset: string, meta: SpriteMeta | null) {
    const e = host.edits;
    const sprites = { ...(e.sprites ?? {}) };
    const name = sprites[asset]?.name ?? asset;
    if (meta) {
      sprites[asset] = meta;
      return host.applyEdits({ ...e, sprites }, { tool: 'upload_sprite', id: `s:${asset}`, name: meta.name }, 'user');
    }
    delete sprites[asset];
    const objects = Object.fromEntries(Object.entries(e.objects ?? {}).filter(([, o]) => o.kind !== `s:${asset}`));
    host.applyEdits({ ...e, sprites, objects }, { tool: 'remove_sprite', id: `s:${asset}`, name }, 'user');
  }


  return {
    get scatter() { return scatter; },
    set scatter(v) { scatter = v; },
    get ring() { return ring; },
    set ring(v) { ring = v; },
    get chip() { return chip; },
    set chip(v) { chip = v; },
    get placeBar() { return placeBar; },
    set placeBar(v) { placeBar = v; },
    get held() { return held; },
    set held(v) { held = v; },
    setHeld,
    get placeBarAt() { return placeBarAt; },
    set placeBarAt(v) { placeBarAt = v; },
    placer,
    chipAt,
    get scatterArmed() { return scatterArmed; },
    set scatterArmed(v) { scatterArmed = v; },
    placedSprites,
    armScatter,
    disarmScatter,
    uploadSprite,
    saveSprite,
  };
}

export type ObjectsTools = ReturnType<typeof objectTools>;
