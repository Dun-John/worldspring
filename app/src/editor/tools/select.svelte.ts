// Edit › Build › Select: click what is on the map to see it listed (the top one open, with
// Delete where a delete exists and Change opening the tool that made it), or drag a box (Shift:
// a lasso) to list what is inside, counted by kind, and delete all that can be. Each delete is
// one undo step (`withoutThings` puts the existing deletes together). App keeps `syncTool()`.
import type { BuildingEdit, Created, Edits, Thing } from '../../gen/protocol';
import type { Pt } from '../build';
import { SelectTool, countOf, fenceIds, inArea, keyOf, ringOf, roadIds, underFirst, withCutOver, withFence, withoutThings } from '../select';
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
  /** The settlements and places a thing can join (by layout index; `place`: a place made by pushing buildings out). */
  townChoices(): TownChoice[];
  /** Open an Edit tool; Change opened one (its Done goes
   * back to Select); `t`'s town opened in Streets with `t` chosen, or in Town wall; whether `t` is a
   * town whose streets can be edited; `t`'s info card. */
  openTool?(key: import('../../ui/shell/editTools').ToolKey): boolean;
  changedFrom?(): void;
  openTown?(t: Thing, tool: 'streets' | 'townwall'): void;
  hasStreets?(t: Thing): boolean;
  info?(t: Thing): void;
}

/** What `gen.reach` says of a thing: the towns within `OFFER_FT` of it (layout, gap ft) and why a generated building may not leave its town. */
export interface Reach {
  towns: [number, number][];
  push: string | null;
}

export interface TownChoice {
  id: string;
  name: string;
  layout: number;
  place: boolean;
}

/** Towns whose edge is this close to a thing (ft) are offered when it joins (`gen.reach` measures it). */
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
  /** What the generator says each chosen thing could join and whether it may leave its town (by id). */
  let reach = $state.raw<Record<string, Reach>>({});
  let reachSeq = 0;
  let changing = 0;
  let armed: SelectTool | null = null;
  let seq = 0;
  /** plain Select, or Join or leave a town (only what can join listed: the panel's part, 8b-1b). */
  let purpose = $state<'all' | 'members'>('all');

  const ftPerPx = () => 1 / host.view.cam.ppf;

  /** Ask which towns each chosen thing that can be part of one could join (Join's offers, Leave's reasons). */
  function loadReach(found: Thing[]) {
    const ids = found.filter((t) => t.id && (t.id.startsWith('c:') || t.id.startsWith('b:')) && (t.kind === 'building' || t.kind === 'drawn_building' || t.kind === 'castle' || t.kind === 'plot' || t.kind === 'wall')).map((t) => t.id as string).slice(0, 300);
    const s = ++reachSeq;
    if (!ids.length) return void (reach = {});
    void host.view.gen.reach(ids, OFFER_FT).then((r) => {
      if (s === reachSeq) reach = r ?? {};
    });
  }

  function arm(): SelectTool {
    armed ??= new SelectTool({
      ftPerPx,
      clicked: (p) => void click(p),
      drawn: (poly) => void drag(poly),
      shown: () => (area ? shown.slice(0, 300) : shown[open] ? [shown[open]] : []).map((t) => t.bbox),
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
    loadReach(found);
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
    loadReach(found);
  }

  /** The edits changed (here, by undo, by an agent): what is listed is asked for again. */
  function refresh() {
    if (!armed) return;
    if (at) {
      const { p, r } = at;
      const key = shown[open] ? keyOf(shown[open]) : null;
      const s = ++seq;
      void host.view.gen.pick(p[0], p[1], r).then((found) => {
        if (s !== seq) return;
        things = underFirst(unique(found), p, r);
        open = Math.max(0, shown.findIndex((t) => keyOf(t) === key));
        loadReach(things);
      });
    } else if (area) {
      const poly = area;
      const ring = ringOf(poly);
      const s = ++seq;
      void host.view.gen.pick(ring.at[0], ring.at[1], ring.r).then((found) => {
        if (s !== seq) return;
        things = inArea(unique(found), poly);
        loadReach(things);
      });
    }
  }

  /** Nothing chosen any more (true if something was). */
  function clear(): boolean {
    const had = !!at || !!area || things.length > 0;
    seq++;
    reachSeq++;
    reach = {};
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
    if (/^(rd|mt|tr|pp):/.test(id)) return { tool: 'remove_roads', ids: [id] };
    if (/^(fr|fh|f|fg):/.test(id)) return { tool: 'remove_fences', ids: [id] };
    switch (t.kind) {
      case 'building':
        return { tool: 'remove_buildings', removed: [id], count: 1, name: t.name ?? 'a building' };
      case 'drawn_road':
        return { tool: 'remove_paths', ids: [id] };
      case 'water':
        return { tool: 'delete_water', ids: [id] };
      case 'crossing':
        return { tool: 'remove_crossings', ids: [id], kind: host.edits.crossings?.[id]?.kind };
      case 'street':
      case 'lane':
        if (id.startsWith('ts:')) return { tool: 'remove_town_street', id };
        return { tool: 'delete_feature', id };
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
      // Roads, streets and paths: the generator says what comes off (and the towns' streets that lead there).
      const rids = roadIds(can);
      const cut = rids.length ? await host.view.gen.roadCut(rids) : null;
      if (cut && 'error' in cut) return void host.toast(cut.error);
      const cutOk = !!cut && cut.done.length > 0;
      const done = new Set(cut && cutOk ? cut.done : []);
      // Fences, hedges and walls (and gaps cut in them), by the same kind of answer.
      const fids = fenceIds(can);
      const fcut = fids.length ? await host.view.gen.fenceChange({ ids: fids, op: 'remove' }) : null;
      if (fcut && 'error' in fcut) return void host.toast(fcut.error);
      const fenceOk = !!fcut && fcut.done.length > 0;
      const fdone = new Set(fcut && fenceOk ? fcut.done : []);
      const kept = can.filter((t) => (t.kind !== 'building' || t.id! in gen) && (!rids.includes(t.id!) || done.has(t.id!)) && (!fids.includes(t.id!) || fdone.has(t.id!)));
      if (!kept.length) return void host.toast([...(cut?.refused ?? []), ...(fcut?.refused ?? [])].length ? `Not taken away: ${[...(cut?.refused ?? []), ...(fcut?.refused ?? [])].map((r) => r.why).join('; ')}` : 'The map could not answer: try again');
      const before = host.edits;
      // (The cut goes in first: its town edits come from `before` and hold only the removals; the street
      // removals of `withoutThings` are then made on top of them.)
      let { next, towns } = withoutThings(before, kept, gen);
      if (cut && cutOk) next = withCutOver(before, next, cut);
      if (fcut && fenceOk) next = withFence(next, fcut);
      if (towns) {
        const r = await host.view.gen.setAside($state.snapshot(before.created ?? []) as Created[], $state.snapshot(next) as Edits);
        if (r && !('error' in r)) next = r.edits;
      }
      if (host.edits !== before) return void host.toast('The map changed meanwhile: try again');
      if (cut && cutOk) for (const k of Object.keys(cut.towns)) host.view.hintTown(k, cut.towns[k], cut.rects);
      const change = changeOf(kept);
      if (cut && cutOk && cut.also.length) change.also = cut.also.length;
      if (list.length > 1 && fids.length) change.fences = fids.length;
      host.applyEdits(next, change, 'user');
      // What was taken away is no longer listed (or outlined on the map).
      const gone = new Set<string>([...kept.map((t) => t.id as string), ...(cut && cutOk ? [...cut.done, ...cut.also] : [])]);
      const refusedAll = [...(cut?.refused ?? []), ...(fcut?.refused ?? [])];
      things = things.filter((t) => !t.id || !gone.has(t.id));
      open = Math.min(open, Math.max(0, shown.length - 1));
      notice = refusedAll.length ? `Not taken away (${refusedAll.length}): ${refusedAll.map((r) => r.why).filter((w, i, a) => a.indexOf(w) === i).join('; ')}` : '';
      if (notice) host.toast(notice);
    } finally {
      busy = false;
    }
  }

  /** The tool that made `t` opened, with it chosen where that tool can choose one (Its
   * Done comes back here). */
  async function change(t: Thing) {
    if (host.openTool) {
      const id = t.id;
      switch (t.kind) {
        case 'plot':
          if (id) host.build.editPlot(id);
          return void host.changedFrom?.();
        case 'district':
        case 'street':
          return void host.openTown?.(t, 'streets');
        case 'settlement':
        case 'site':
          if (!host.openTool('places')) return;
          if (id) host.showFeature(id);
          return;
      }
      const p = change0(t);
      host.changedFrom?.();
      return p;
    }
    return change0(t);
  }

  async function change0(t: Thing) {
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
      case 'fence':
      case 'drawn_fence':
        B.setBuildMode('fence');
        B.fences.fence = { ...B.fences.fence, mode: 'gate' };
        return;
      case 'drawn_road':
        B.setBuildMode('road');
        B.roadPicked = id;
        return;
      case 'water':
        B.setBuildMode(id && host.edits.waters?.[id]?.kind === 'lava' ? 'lava' : 'water');
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
  const changeable = (t: Thing) => !['road', 'lane', 'path', 'fence', 'drawn_fence', 'fence_gap', 'entrance', 'tower'].includes(t.kind);
  /** Whether `t` is a fence or hedge the card can give another kind or a gate (not a gap, not where it is not removable). */
  const fenced = (t: Thing) => (t.kind === 'fence' || t.kind === 'drawn_fence') && t.removable && !!t.id;
  /** Whether `t`'s kind can be changed (a generated garden wall keeps its own). */
  const kindable = (t: Thing) => fenced(t) && t.fence !== 'garden_wall';
  /** The fence `t` made another kind. */
  const setKind = (t: Thing, kind: import('../../gen/protocol').FenceKind) => (t.id ? host.build.fences.change([t.id], kind).then((out) => {
    if (out?.done.length) things = things.filter((x) => x.id !== t.id);
  }) : undefined);

  /** Join or leave a town lists only what can be part of one. */
  const shown = $derived(purpose === 'members' ? things.filter((t) => memberable(t)) : things);

  /** What an area holds, counted by kind (in pick's order). */
  const counts = $derived.by(() => {
    if (!area) return [];
    const by = new Map<Thing['kind'], { n: number; can: number }>();
    for (const t of shown) {
      const c = by.get(t.kind) ?? { n: 0, can: 0 };
      c.n++;
      if (t.removable && t.id) c.can++;
      by.set(t.kind, c);
    }
    return [...by].map(([kind, c]) => ({ kind, n: c.n, can: c.can, words: countOf(kind, c.n) }));
  });

  /** What can be part of a town: things drawn by hand (not a town or place itself) and generated buildings (`b:`). */
  const drawn = (t: Thing) => !!t.id && ['drawn_building', 'castle', 'plot', 'wall'].includes(t.kind) && t.id.startsWith('c:');
  /** A generated building is only moved between places and its town (it can be pushed out of its own). */
  const generated = (t: Thing) => !!t.id && t.kind === 'building' && t.id.startsWith('b:');
  const memberable = (t: Thing) => drawn(t) || generated(t);
  const inTown = (t: Thing) => (t.town && t.town !== t.id ? host.townChoices().find((c) => c.id === t.town) : undefined);
  /** The towns `t` could join: those within reach, nearest first, but the one it is in; generated buildings only move to places or back. */
  function options(t: Thing): TownChoice[] {
    const r = t.id ? reach[t.id] : undefined;
    if (!r) return [];
    const all = host.townChoices();
    return r.towns.map(([layout]) => all.find((c) => c.layout === layout)).filter((c): c is TownChoice => !!c && c.id !== t.town);
  }
  /** Whether Join is on offer for `t`: it has a town in reach it is not in. */
  const joinable = (t: Thing) => memberable(t) && options(t).length > 0 && (drawn(t) || !!inTown(t)?.place);
  /** Why `t` cannot leave its town, if it cannot (a generated building the generator would refuse). */
  const leaveBlock = (t: Thing): string | null => (generated(t) && !inTown(t)?.place ? (t.id ? reach[t.id]?.push ?? null : null) : null);
  /** Whether `t` is in a town now (so it can leave it): a drawn thing in one, a generated building in its own (pushed out) or out in a place (back). */
  const leavable = (t: Thing) => memberable(t) && !!inTown(t);

  /** The card's line on whose part a thing is: "Part of X · within N ft", "· by hand", or "Not part of a town"; null where it does not apply. */
  function partOf(t: Thing): string | null {
    if (!memberable(t)) return null;
    if (!t.town || t.town === t.id) return 'Not part of a town';
    const name = host.nameOf(t.town);
    if (t.why === 'hand') return `Part of ${name} · by hand`;
    if (t.why === 'gap' && t.gap_ft !== undefined) {
      const ft = Math.round(t.gap_ft).toLocaleString('en-US');
      return t.gap_ft < 1 ? `Part of ${name}` : `Part of ${name} · ${ft} ft from its edge or from a member`;
    }
    return `Part of ${name}`;
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
      if (done <= 0 || (!Object.keys(r.members).length && !r.created.length)) return;
      const next: Edits = { ...before, members, created: r.created.length ? [...(before.created ?? []), ...(r.created as Created[])] : before.created };
      if (!Object.keys(members).length) delete next.members;
      host.applyEdits(next, { tool: 'set_town_members', join, count: done, town: town.name, ids }, 'user');
    } finally {
      busy = false;
    }
  }

  /** Put `list` (those that can) into a town: the nearest, or one of those in reach picked from a short list. */
  function join(list: Thing[], pick?: TownChoice) {
    const can = list.filter(joinable);
    if (!can.length || busy) return;
    if (!pick) {
      const opts = options(can[0]);
      if (opts.length > 1) return void (choosing = { list: can, options: opts });
      pick = opts[0];
    }
    const to = pick;
    const ids = can.filter((t) => t.town !== to.id).map((t) => t.id as string);
    void setMembers(to, ids, true);
  }

  /** Take `list` (those in a town) out of it: those of the first one's town, or a lone place for a generated building. */
  function leave(list: Thing[]) {
    const can = list.filter((t) => leavable(t) && !leaveBlock(t));
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
    joinable,
    leavable,
    leaveBlock,
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
    get purpose() {
      return purpose;
    },
    set purpose(v) {
      purpose = v;
    },
    disarm() {
      clear();
      armed = null;
    },
    get things() {
      return shown;
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
      const t = !area && armed ? shown[open] : undefined;
      if (!t?.removable || !t.id) return false;
      void remove([t]);
      return true;
    },
    change,
    changeable,
    fenced,
    kindable,
    setKind,
    /** The area's things of one kind taken away (those that can be), as one step (Select's per-kind Delete). */
    removeKind: (kind: Thing['kind']) => void remove(shown.filter((t) => t.kind === kind)),
    /** The card's extra links (Select's Info and Change). */
    get links() {
      return { info: host.info, openTown: host.openTown, hasStreets: host.hasStreets };
    },
    townName: (t: Thing) => (t.town && t.town !== t.id ? host.nameOf(t.town) : null),
  };
}

export type SelectTools = ReturnType<typeof selectTools>;
