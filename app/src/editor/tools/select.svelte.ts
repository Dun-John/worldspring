// Edit › Build › Select: click what is on the map to see it listed (the top one open, with
// Delete where a delete exists and Change opening the tool that made it), or drag a box (Shift:
// a lasso) to list what is inside, counted by kind, and delete all that can be. Each delete is
// one undo step (`withoutThings` puts the existing deletes together). App keeps `syncTool()`.
import type { BuildingEdit, Created, Edits, Thing } from '../../gen/protocol';
import type { Pt } from '../build';
import { SelectTool, countOf, inArea, keyOf, ringOf, withoutThings } from '../select';
import type { BuildingHit } from '../../ui/gazetteer';

/** What the Select tool needs from the app. */
export interface SelectHost {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: Edits;
  readonly build: import('./build.svelte').BuildTools;
  nameOf(id: string): string;
  /** A settlement's or place's card opened (false: it is not on the map). */
  showFeature(id: string): boolean;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
}

/** A click asks `pick` this far round it (screen px). */
const CLICK_PX = 8;
/** The widest area asked over (ft from its middle to its corners). */
const MAX_AREA_FT = 6000;

export function selectTools(host: SelectHost) {
  /** What a click found (topmost first), or what an area holds. */
  let things = $state<Thing[]>([]);
  /** The one open in the card (a click's list). */
  let open = $state(0);
  /** Where the click was, and how far round it was asked. */
  let at = $state<{ p: Pt; r: number } | null>(null);
  /** The area dragged. */
  let area = $state<Pt[] | null>(null);
  let busy = $state(false);
  let armed: SelectTool | null = null;
  let seq = 0;

  const ftPerPx = () => 1 / host.view.cam.ppf;

  function arm(): SelectTool {
    armed ??= new SelectTool({
      ftPerPx,
      clicked: (p) => void click(p),
      drawn: (poly) => void drag(poly),
      shown: () => (area ? things.slice(0, 300) : things[open] ? [things[open]] : []).map((t) => t.bbox),
      area: () => area,
    });
    return armed;
  }

  const unique = (list: Thing[]) => {
    const seen = new Set<string>();
    return list.filter((t) => !seen.has(keyOf(t)) && !!seen.add(keyOf(t)));
  };

  async function click(p: Pt) {
    const r = CLICK_PX * ftPerPx();
    const s = ++seq;
    const found = unique(await host.view.gen.pick(p[0], p[1], r));
    if (s !== seq) return;
    area = null;
    at = { p, r };
    things = found;
    open = 0;
  }

  async function drag(poly: Pt[]) {
    const ring = ringOf(poly);
    if (ring.r > MAX_AREA_FT) return host.toast('Zoom in to choose over a smaller area');
    const s = ++seq;
    area = poly;
    at = null;
    const found = inArea(unique(await host.view.gen.pick(ring.at[0], ring.at[1], ring.r)), poly);
    if (s !== seq) return;
    things = found;
    open = 0;
  }

  /** The edits changed (here, by undo, by an agent): what is listed is asked for again. */
  function refresh() {
    if (!armed) return;
    if (at) {
      const { p, r } = at;
      const key = things[open] ? keyOf(things[open]) : null;
      const s = ++seq;
      void host.view.gen.pick(p[0], p[1], r).then((found) => {
        if (s !== seq) return;
        things = unique(found);
        open = Math.max(0, things.findIndex((t) => keyOf(t) === key));
      });
    } else if (area) {
      const poly = area;
      const ring = ringOf(poly);
      const s = ++seq;
      void host.view.gen.pick(ring.at[0], ring.at[1], ring.r).then((found) => {
        if (s !== seq) return;
        things = inArea(unique(found), poly);
      });
    }
  }

  /** Nothing chosen any more (true if something was). */
  function clear(): boolean {
    const had = !!at || !!area || things.length > 0;
    seq++;
    things = [];
    at = null;
    area = null;
    open = 0;
    return had;
  }

  /** The change's words for the history: a lone thing's as its own tool says it. */
  function changeOf(list: Thing[]): import('../../sync/mapd').Change {
    if (list.length > 1) return { tool: 'delete_selected', count: list.length, ids: list.map((t) => t.id) };
    const t = list[0];
    const id = t.id as string;
    switch (t.kind) {
      case 'building':
        return { tool: 'remove_buildings', removed: [id], count: 1, name: t.name ?? 'a building' };
      case 'drawn_road':
        return { tool: 'remove_paths', ids: [id] };
      case 'water':
        return { tool: 'delete_water', ids: [id] };
      case 'crossing':
        return { tool: 'remove_crossings', ids: [id], kind: host.edits.crossings?.[id]?.kind };
      case 'harbour':
        return { tool: 'remove_wharves', ids: [id], kind: host.edits.wharves?.[id]?.kind };
      default:
        return { tool: 'delete_feature', id };
    }
  }

  /** Take `list` away (those that can be), as one undo step. */
  async function remove(list: Thing[]) {
    const can = list.filter((t) => t.removable && t.id);
    if (!can.length || busy) return;
    busy = true;
    try {
      const gen: Record<string, BuildingEdit> = {};
      for (const t of can) {
        if (t.kind !== 'building') continue;
        const r = await host.view.gen.buildingEdit(t.id as string, { remove: true });
        if (r && 'edit' in r && r.edit) gen[t.id as string] = r.edit;
      }
      const kept = can.filter((t) => t.kind !== 'building' || t.id! in gen);
      if (!kept.length) return void host.toast('The map could not answer: try again');
      const before = host.edits;
      let { next, towns } = withoutThings(before, kept, gen);
      if (towns) {
        const r = await host.view.gen.setAside($state.snapshot(before.created ?? []) as Created[], $state.snapshot(next) as Edits);
        if (r && !('error' in r)) next = r.edits;
      }
      if (host.edits !== before) return void host.toast('The map changed meanwhile: try again');
      host.applyEdits(next, changeOf(kept), 'user');
    } finally {
      busy = false;
    }
  }

  /** The tool that made `t` opened, with it chosen where that tool can choose one. */
  async function change(t: Thing) {
    const B = host.build;
    const id = t.id;
    switch (t.kind) {
      case 'drawn_building':
        B.setBuildMode('building');
        if (id) B.editBuilding(id);
        return;
      case 'castle':
      case 'wall':
        if (id) B.editWorks(id);
        return;
      case 'plot':
        return B.setBuildMode('lasso');
      case 'building': {
        B.setBuildMode('building');
        const [x0, y0, x1, y1] = t.bbox;
        const hit = await host.view.gen.query((x0 + x1) / 2, (y0 + y1) / 2);
        if (hit?.kind === 'building' && hit.id === id) void B.editGenerated(hit as BuildingHit);
        return;
      }
      case 'drawn_road':
        B.setBuildMode('road');
        B.roadPicked = id;
        return;
      case 'water':
        B.setBuildMode('water');
        B.waterPicked = id;
        return;
      case 'crossing':
        B.setBuildMode('crossing');
        if (id) B.pickCrossing(id);
        return;
      case 'harbour':
        B.setBuildMode('harbour');
        B.harbourPicked = id;
        return;
      case 'settlement':
      case 'site':
        host.go('edit', 'sites');
        if (id) host.showFeature(id);
        return;
      case 'district':
      case 'street':
        return host.go('edit', 'town');
    }
  }

  /** Whether `t` has a tool to change it in. */
  const changeable = (t: Thing) => !['road', 'lane', 'hedge', 'field_wall', 'fence', 'entrance', 'tower'].includes(t.kind);

  /** What an area holds, counted by kind (in pick's order). */
  const counts = $derived.by(() => {
    if (!area) return [];
    const by = new Map<Thing['kind'], { n: number; can: number }>();
    for (const t of things) {
      const c = by.get(t.kind) ?? { n: 0, can: 0 };
      c.n++;
      if (t.removable && t.id) c.can++;
      by.set(t.kind, c);
    }
    return [...by].map(([kind, c]) => ({ kind, n: c.n, can: c.can, words: countOf(kind, c.n) }));
  });

  return {
    arm,
    get tool() {
      return armed;
    },
    disarm() {
      clear();
      armed = null;
    },
    get things() {
      return things;
    },
    get open() {
      return open;
    },
    set open(v) {
      open = v;
    },
    get area() {
      return area;
    },
    get busy() {
      return busy;
    },
    get counts() {
      return counts;
    },
    refresh,
    clear,
    remove,
    /** The thing open in a click's card taken away (false: none, or it can't be). */
    removeOpen(): boolean {
      const t = !area && armed ? things[open] : undefined;
      if (!t?.removable || !t.id) return false;
      void remove([t]);
      return true;
    },
    change,
    changeable,
    townName: (t: Thing) => (t.town && t.town !== t.id ? host.nameOf(t.town) : null),
  };
}

export type SelectTools = ReturnType<typeof selectTools>;
