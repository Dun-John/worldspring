// Edit › Build › Select: click what is on the map to see it listed (the top one open, with
// Delete where a delete exists and Change opening the tool that made it), or drag a box (Shift:
// a lasso) to list what is inside, counted by kind, and delete all that can be. Each delete is
// one undo step (`withoutThings` puts the existing deletes together). App keeps `syncTool()`.
import type { BuildingEdit, Created, Edits, Thing } from '../../gen/protocol';
import type { Pt } from '../build';
import { SelectTool, countOf, inArea, keyOf, ringOf, underFirst, withoutThings } from '../select';
import type { BuildingHit } from '../../ui/gazetteer';

/** What the Select tool needs from the app. */
export interface SelectApp {
  readonly view: import('../../render/MapView').MapView;
  readonly edits: Edits;
  readonly build: import('./build.svelte').BuildTools;
  nameOf(id: string): string;
  /** A settlement's or place's card opened (false: it is not on the map). */
  showFeature(id: string): boolean;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  go(to: import('../../ui/shell/layout.svelte').Section | null, tab?: string): void;
  /** The settlements and places a thing can join (layout index, middle, reach in ft). */
  townChoices(): TownChoice[];
}

export interface TownChoice {
  id: string;
  name: string;
  layout: number;
  x: number;
  y: number;
  extent: number;
}

/** Towns this close to a thing's edge (ft) are offered when it joins; the generator decides what is near enough. */
const OFFER_FT = 800;

/** A click asks `pick` this far round it (screen px), but never over `MAX_PICK_FT`. */
const PICK_PX = 8;
const MAX_PICK_FT = 60;
/** The widest area asked over (ft from its middle to its corners). */
const MAX_AREA_FT = 6000;

export function selectTools(host: SelectApp) {
  /** What a click found (topmost first), or what an area holds. */
  let things = $state.raw<Thing[]>([]);
  /** The one open in the card (a click's list). */
  let open = $state(0);
  /** Where the click was, and how far round it was asked. */
  let at = $state<{ p: Pt; r: number } | null>(null);
  /** The area dragged. */
  let area = $state<Pt[] | null>(null);
  /** A drag draws a lasso (else a box). */
  let lasso = $state(false);
  let busy = $state(false);
  /** One plain line on what the last Join or Leave did not do. */
  let notice = $state('');
  /** Towns to choose from for a Join (when several are in reach). */
  let choosing = $state.raw<{ list: Thing[]; options: TownChoice[] } | null>(null);
  let changing = 0;
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
      lasso: () => lasso,
    });
    return armed;
  }

  const unique = (list: Thing[]) => {
    const seen = new Set<string>();
    return list.filter((t) => !seen.has(keyOf(t)) && !!seen.add(keyOf(t)));
  };

  async function click(p: Pt) {
    const r = PICK_PX * ftPerPx();
    if (r > MAX_PICK_FT) return host.toast('Zoom in to select');
    const s = ++seq;
    const found = underFirst(unique(await host.view.gen.pick(p[0], p[1], r)), p, r);
    if (s !== seq) return;
    notice = '';
    choosing = null;
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
    notice = '';
    choosing = null;
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
        things = underFirst(unique(found), p, r);
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
    notice = '';
    choosing = null;
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
      const ids = can.filter((t) => t.kind === 'building').map((t) => t.id as string);
      const answers = ids.length ? ((await host.view.gen.buildingEdits(ids, { remove: true })) ?? []) : [];
      answers.forEach((r, k) => {
        if (r && 'edit' in r && r.edit) gen[ids[k]] = r.edit;
      });
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
        const token = ++changing;
        const hit = await host.view.gen.query((x0 + x1) / 2, (y0 + y1) / 2);
        // (Still wanted: nothing else asked for meanwhile, and Build still on Building.)
        if (token === changing && B.buildMode === 'building' && hit?.kind === 'building' && hit.id === id) void B.editGenerated(hit as BuildingHit);
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

  /** What can be part of a town: things drawn by hand (not a town or place itself) and generated buildings (`b:`). */
  const memberable = (t: Thing) => !!t.id && ((t.kind === 'building' && t.id.startsWith('b:')) || (['drawn_building', 'castle', 'plot', 'wall'].includes(t.kind) && t.id.startsWith('c:')));
  /** Whether `t` is in a town now (so it can leave it). */
  const leavable = (t: Thing) => memberable(t) && !!t.town && host.townChoices().some((c) => c.id === t.town);

  /** The card's line on whose part a thing is: "Part of X · within N ft", "· by hand", or "Not part of a town"; null where it does not apply. */
  function partOf(t: Thing): string | null {
    if (!memberable(t)) return null;
    if (!t.town || t.town === t.id) return 'Not part of a town';
    const name = host.nameOf(t.town);
    if (t.why === 'hand') return `Part of ${name} · by hand`;
    if (t.why === 'gap' && t.gap_ft !== undefined) return `Part of ${name} · within ${Math.round(t.gap_ft).toLocaleString('en-US')} ft`;
    return `Part of ${name}`;
  }

  /** Towns near a thing, nearest edge first: those within reach, else the nearest. */
  function nearTowns(t: Thing): TownChoice[] {
    const [x, y] = [(t.bbox[0] + t.bbox[2]) / 2, (t.bbox[1] + t.bbox[3]) / 2];
    const ranked = host
      .townChoices()
      .map((c) => ({ c, edge: Math.hypot(c.x - x, c.y - y) - c.extent }))
      .sort((a, b) => a.edge - b.edge);
    const near = ranked.filter((r) => r.edge <= OFFER_FT).slice(0, 4);
    return (near.length ? near : ranked.slice(0, 1)).map((r) => r.c);
  }

  /** Make `town`'s members `add`ed or `remove`d (ids), as one undo step; says plainly what was not done. */
  async function setMembers(town: TownChoice | { layout: number; name: string }, ids: string[], join: boolean, note = '') {
    if (!ids.length || busy) return;
    busy = true;
    notice = note;
    choosing = null;
    try {
      const before = host.edits;
      const r = await host.view.gen.members(town.layout, join ? ids : [], join ? [] : ids);
      if (!r) return void (notice = 'The map could not answer: try again');
      if ('error' in r) return void (notice = r.error);
      const members = { ...(before.members ?? {}) };
      for (const [k, v] of Object.entries(r.members)) {
        if (!v.add?.length && !v.remove?.length) delete members[k];
        else members[k] = v;
      }
      const done = ids.length - r.refused.length;
      if (r.refused.length) notice = (note ? `${note}. ` : '') + (r.refused.length === 1 ? r.refused[0].why : `${r.refused.length} left as they were: ${r.refused[0].why}`);
      if (host.edits !== before) return void (notice = 'The map changed meanwhile: try again');
      if (done <= 0) return;
      const next: Edits = { ...before, members, created: r.created.length ? [...(before.created ?? []), ...(r.created as Created[])] : before.created };
      if (!Object.keys(members).length) delete next.members;
      host.applyEdits(next, { tool: 'set_town_members', join, count: done, town: town.name, ids }, 'user');
    } finally {
      busy = false;
    }
  }

  /** Put `list` (those that can) into a town: the nearest, or one of those in reach picked from a short list. */
  function join(list: Thing[], pick?: TownChoice) {
    const can = list.filter(memberable);
    if (!can.length || busy) return;
    if (!pick) {
      const options = nearTowns(can[0]);
      if (options.length > 1) return void (choosing = { list: can, options });
      pick = options[0];
    }
    if (!pick) return void (notice = 'There is no town to join');
    void setMembers(pick, can.map((t) => t.id as string), true);
  }

  /** Take `list` (those in a town) out of it: those of the first one's town, or a lone place for a generated building. */
  function leave(list: Thing[]) {
    const can = list.filter(leavable);
    if (!can.length || busy) return;
    const town = can[0].town!;
    const same = can.filter((t) => t.town === town);
    const c = host.townChoices().find((x) => x.id === town)!;
    const note = same.length < can.length ? `Only those in ${c.name} left now: ${can.length - same.length} more are in other towns` : '';
    void setMembers(c, same.map((t) => t.id as string), false, note);
  }

  return {
    arm,
    memberable,
    leavable,
    partOf,
    join,
    leave,
    get notice() {
      return notice;
    },
    get choosing() {
      return choosing;
    },
    cancelChoice() {
      choosing = null;
    },
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
    get lasso() {
      return lasso;
    },
    set lasso(v) {
      lasso = v;
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
