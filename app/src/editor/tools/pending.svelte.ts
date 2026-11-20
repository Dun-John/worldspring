// Settlement changes waiting for a remake (a pin changed or moved on the card, one remake makes
// them all), a settlement card's change worked out and made, a village or town added by hand
// moved, and what the Gazetteer brings back (deleted settlements, removed roads, changed fences).
// App keeps the markup reading these and the `waiting` check when another world opens.
import type { ChangedFences, Created, Edits, Feature, Overlay, PinKind, PlanningNow, RemovedRoad, SettlementFacts, Stroke, TownEdit, WorldFile } from '../../gen/protocol';
import type { MapView } from '../../render/MapView';
import type { Ask, Remake, Section } from '../../ui/shell/layout.svelte';
import type { UndoHistory } from '../../ui/shell/history.svelte';
import { CULTURES, type SettleAsk, type SettlePlan, type WaitAsk } from '../../ui/SettlementEdit.svelte';
import { withCut, withFence } from '../select';
import { MI } from '../units';
import { PIN_KINDS, pinStroke, type PinTier } from '../sketcher';
import type { Carried } from '../../play/carry';
import type { Change } from '../../sync/mapd';
import type { Step } from '../../sync/history';
import { sentence, SETTLEMENT_KINDS, type Selection } from '../../ui/gazetteer';
import type { AsideGroup } from '../../world/setaside';
import type { PlaceTools } from './places.svelte';

/** A settlement change waiting for a remake: the pin it is to have, in place of stroke `index`
 * of the world's sketch (null: added), and the settlement it is for. */
export type Waiting = { id: string; name: string; index: number | null; pin: Stroke };
/** What the last remake did with the changes (App's `generate` leaves it in `lastFollowed`). */
type Followed = { edits: Edits; kept: number; parked: number; removed: number };

/** What the waiting changes and the settlement card need from the app. */
export interface PendingHost {
  readonly view: MapView;
  readonly edits: Edits;
  readonly world: WorldFile;
  readonly renames: Record<string, string>;
  readonly overlay: Overlay | null;
  selection: Selection | null;
  readonly worldShown: boolean;
  readonly playOn: boolean;
  readonly busy: boolean;
  /** How long the last world took to generate (s). */
  readonly genSeconds: number | null;
  /** The site designer open (its rooms unsaved when `dirty`). */
  readonly designer: { dirty: boolean } | null;
  readonly asideGroups: AsideGroup[];
  /** Changes made to the edits (`applyEdits`), counted: whether any came in meanwhile. */
  readonly editCount: number;
  /** How many changes the last remake made itself. */
  readonly remapCount: number;
  readonly lastFollowed: Followed | null;
  readonly lastCarried: Carried | null;
  readonly undoHistory: UndoHistory;
  /** Edit › Places (Take back, the place menu). */
  readonly places: PlaceTools;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  undo(step?: Step): void;
  applyEdits(next: Edits, change: Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  ask(kind: Ask['kind'], proceed: () => void, generate?: boolean): void;
  generate(w: WorldFile, o?: { confirmed?: boolean; quiet?: boolean }): Promise<boolean>;
  remakeOf(w: WorldFile): Remake | null;
  playSaid(c: Carried | null): string;
  sketchDirty(): boolean;
  nameOf(id: string): string;
  layoutIndexOf(id: string): number;
  go(to: Section | null, tab?: string): void;
  flyToSize(x: number, y: number, size: number): void;
}

export function pendingTools(host: PendingHost) {
  // Settlement changes waiting for a remake (the world's own, and those made by a pin): each is a
  // pin in the sketch as mapd's `update_settlement` stores it (in place of the settlement's own
  // pin, else one that `replaces` it), kept here until one remake makes them all.

  /** The changes waiting, for the world they were made on (`waitKey`): another drops them. */
  let waiting = $state.raw<{ key: string; list: Waiting[] } | null>(null);
  const waitKey = (w: WorldFile) => JSON.stringify([w.seed, w.params, w.gen_version, w.sketch ?? null]);
  const waitingList = $derived(waiting && host.worldShown && waiting.key === waitKey(host.world) ? waiting.list : []);

  /** The world's sketch with the changes waiting made. */
  function waitingSketch(): Stroke[] {
    const strokes = ($state.snapshot(host.world.sketch?.strokes ?? []) as Stroke[]).slice();
    const more: Stroke[] = [];
    for (const w of waitingList) {
      if (w.index !== null && w.index < strokes.length) strokes[w.index] = w.pin;
      else more.push(w.pin);
    }
    return [...strokes, ...more];
  }

  /** Settlement `id`'s change waiting set to `to` (null: none waits), as one Undo step. */
  function putWaiting(id: string, to: Waiting | null, label: string) {
    const was = waitingList.find((w) => w.id === id) ?? null;
    const set = (v: Waiting | null) => {
      const rest = waitingList.filter((w) => w.id !== id);
      const list = v ? [...rest, v] : rest;
      waiting = list.length ? { key: waitKey(host.world), list } : null;
    };
    set(to);
    const step: Step = { label, author: 'user', ops: [], inverse: [], undo: () => set(was), redo: () => set(to) };
    host.undoHistory.push(step);
    host.toast(label, () => host.undo(step));
  }

  /** The selected settlement as its pin would take it (the world's own and pin-made ones). */
  let cardFacts = $state.raw<{ id: string; f: SettlementFacts } | null>(null);
  $effect(() => {
    const sel = host.selection;
    void host.overlay;
    if (sel?.kind !== 'feature' || !SETTLEMENT_KINDS.includes(sel.feature.kind) || sel.feature.id.startsWith('c:')) return;
    const id = sel.feature.id;
    void host.view.gen.settlementFacts(id).then((r) => {
      if (host.selection?.kind === 'feature' && host.selection.feature.id === id) cardFacts = r && !('error' in r) ? { id, f: r } : null;
    });
  });

  /** A pin's options compared: what `to` changes from `from`, in words ("a city, the capital"). */
  function waitWords(from: { tier: string; kind: string; capital: boolean; at: [number, number] }, to: Stroke): string {
    const parts: string[] = [];
    if (to.tier && to.tier !== from.tier) parts.push(`a ${to.tier}`);
    if ((to.kind ?? '') !== from.kind) parts.push(to.kind ? `living by ${(PIN_KINDS.find(([k]) => k === to.kind)?.[1] ?? to.kind).toLowerCase()}` : 'living from its land');
    if (!!to.capital !== from.capital) parts.push(to.capital ? 'the realm’s capital' : 'not the capital');
    if (Math.hypot(to.pts[0][0] - from.at[0], to.pts[0][1] - from.at[1]) > 1) parts.push('moved');
    return parts.join(', ');
  }

  /** What the settlement's pin is now (its own, else as the world placed it), to compare with. */
  function pinBase(f: SettlementFacts): { tier: string; kind: string; capital: boolean; at: [number, number] } {
    const own = f.pin !== null ? host.world.sketch?.strokes?.[f.pin] : undefined;
    return own ? { tier: own.tier ?? f.tier, kind: own.kind ?? '', capital: !!own.capital, at: own.pts[0] } : { tier: f.tier, kind: f.kind, capital: f.capital, at: f.spot };
  }

  /** Change one of the world's own settlements, or one made by a pin: its pin, waiting for a
   * remake (as mapd's `update_settlement`); one Undo step. An error in words, else null. */
  async function waitChange(id: string, ask: { tier?: PinTier; lives_by?: string; capital?: boolean; at?: [number, number] }): Promise<string | null> {
    if (host.playOn) return 'Stop playing to change it: it remakes the world';
    const f = cardFacts?.id === id ? cardFacts.f : await host.view.gen.settlementFacts(id).then((r) => (r && !('error' in r) ? r : null));
    if (!f) return 'The map could not answer: try again';
    if (ask.lives_by === 'port' && !f.port_ok) return 'A port stands on a sea or lake shore: this one is inland';
    if (ask.lives_by === 'fishing' && !f.fishing_ok) return 'A fishing village stands on a shore or a river: this one has neither';
    const now = waitingList.find((w) => w.id === id);
    const own = f.pin !== null ? ($state.snapshot(host.world.sketch?.strokes?.[f.pin]) as Stroke | undefined) : undefined;
    // (In place of the world's own: as it was placed, and as it is named now.)
    const s: Stroke = now ? { ...now.pin } : own ? { ...own } : { ...pinStroke(f.spot, { tier: f.tier, kind: f.kind as PinKind, capital: f.capital }), replaces: f.planned, name: host.nameOf(id) };
    if (ask.tier) {
      s.tier = ask.tier;
      // (What it lives by stays while the new tier allows it: a port is no village's, a fishing
      // village no town's; else it is from its land.)
      const k = s.kind ?? '';
      if ((k === 'port' && ask.tier === 'village') || (k === 'fishing' && ask.tier !== 'village')) delete s.kind;
    }
    if (ask.lives_by !== undefined) {
      if (ask.lives_by) s.kind = ask.lives_by;
      else delete s.kind;
      if (ask.lives_by === 'fishing' && s.tier !== 'village') return 'A fishing settlement is a village: make it a village first';
      if (ask.lives_by === 'port' && s.tier === 'village') return 'A port is a town or bigger: make it a town first';
    }
    if (ask.capital !== undefined) {
      if (ask.capital) s.capital = true;
      else delete s.capital;
    }
    if (ask.at) s.pts = [[Math.round(ask.at[0]), Math.round(ask.at[1])]];
    const name = host.nameOf(id);
    const what = waitWords(pinBase(f), s);
    if (!what) {
      // (Back as it is: nothing waits.)
      if (now) putWaiting(id, null, `${name}: back as it is`);
      return null;
    }
    if (now && JSON.stringify(now.pin) === JSON.stringify(s)) return null;
    putWaiting(id, { id, name, index: f.pin, pin: s }, `${name}: ${what} (waits for a remake)`);
    return null;
  }

  /** A pin-made or the world's own settlement moved: its pin, at (x, y), waiting for a remake. */
  async function waitMove(id: string, x: number, y: number) {
    const f = cardFacts?.id === id ? cardFacts.f : null;
    const near = (host.overlay?.features ?? []).find((g) => g.id !== id && SETTLEMENT_KINDS.includes(g.kind) && Math.hypot(g.x - x, g.y - y) < 3 * MI);
    if (near) return host.toast(`Too close to ${near.name}: keep settlements 3 miles apart`);
    const pins = host.world.sketch?.strokes ?? [];
    if (pins.some((p, i) => p.tool === 'pin' && i !== f?.pin && Math.hypot(p.pts[0][0] - x, p.pts[0][1] - y) < 3 * MI)) return host.toast('Too close to a settlement placed by hand: keep them 3 miles apart');
    const e = await waitChange(id, { at: [x, y] });
    if (e) host.toast(e);
    else reselectFeature(id);
  }

  /** The Dock's bar while settlement changes wait for a remake. */
  const waitBar = $derived(
    waitingList.length ? { n: waitingList.length, run: () => void remakeWaiting(), busy: host.busy || host.playOn, title: host.playOn ? 'Stop playing to remake the world' : `Remake the world with them${host.genSeconds ? ` (about ${host.genSeconds} s)` : ''}` } : null,
  );

  /** Remake the world with the settlement changes waiting (asked first when it has changes, as
   * any remake is); Take back makes it again as it was. */
  async function remakeWaiting() {
    if (host.playOn) return host.toast('Stop playing to remake the world');
    if (host.busy || !waitingList.length) return;
    if (host.sketchDirty() || host.designer?.dirty) return host.sketchDirty() ? host.ask('sketch', () => void remakeWaiting(), false) : host.ask('design', () => void remakeWaiting());
    const list = waitingList;
    const next: WorldFile = { ...($state.snapshot(host.world) as WorldFile), sketch: { strokes: waitingSketch() } };
    const run = async () => {
      if (host.playOn || host.busy) return host.toast(host.playOn ? 'Stop playing to remake the world' : 'The world is being made: try again');
      host.places.clearAdded();
      const before = $state.snapshot(host.world) as WorldFile;
      const sel = host.selection?.kind === 'feature' ? host.selection.feature.id : null;
      const n0 = host.editCount;
      if (!(await host.generate({ ...before, sketch: next.sketch }, { confirmed: true, quiet: true }))) return;
      waiting = null;
      const m = host.lastFollowed as Followed | null;
      const count = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
      const kept = m ? (m.parked ? ` Kept ${count(m.kept)}; ${count(m.parked)} set aside: their places came out differently.` : ` Kept ${count(m.kept)} with the remade world.`) : '';
      const missing = (host.overlay?.conflicts ?? []).filter((c) => c.message.includes('no settlement placed')).map((c) => c.message);
      const said = host.playSaid(host.lastCarried);
      const names = list.map((w) => w.name);
      const who = names.length > 1 ? `${names.slice(0, -1).join(', ')} and ${names.at(-1)}` : names[0];
      const text = `Remade the world with the changes to ${who}.${missing.length ? ` ${missing.join('; ')}.` : ''}${kept}${said ? ` ${said}` : ''}`;
      host.places.offerTakeBack({
        name: `the changes to ${who}`,
        text,
        seed: before.seed,
        params: JSON.stringify(before.params),
        sketchBefore: before.sketch,
        sketchAfter: JSON.stringify(host.world.sketch ?? null),
        editsBefore: host.editCount - n0 === host.remapCount ? (before.edits ?? {}) : null,
        count: host.editCount,
        parked: m?.parked ?? 0,
        toast: 0,
        id: null,
        li: -1,
        stroke: -1,
        editsAfter: host.edits,
        changes: list,
      });
      if (sel) reselectFeature(sel);
    };
    if (host.remakeOf(next)) host.ask('regen', () => void run());
    else await run();
  }

  /** A settlement card's change worked out (not made): its culture (any settlement, through
   * `setCulture`), or a village or town added by hand given another tier, living or name (its
   * changes inside set aside as mapd's `update_made` does: `setAside`). */
  async function planSettlement(id: string, ask: SettleAsk): Promise<SettlePlan> {
    const name = host.nameOf(id);
    const count = host.editCount;
    const now = $state.snapshot(host.edits) as Edits;
    const unanswered = { error: 'The map could not answer: try again' };
    if (ask.culture !== undefined) {
      const li = host.layoutIndexOf(id);
      if (li < 0) return { error: 'It is not on the map now' };
      const r = await host.view.gen.setCulture(li, ask.culture, now);
      if (!r) return unanswered;
      if ('error' in r) return { error: sentence(r.error) };
      if (r.unchanged || !r.edits) return { unchanged: true };
      const n = r.set_aside ?? 0;
      return { edits: r.edits, change: { tool: 'update_settlement', id, name, culture: CULTURES[ask.culture], set_aside: n }, set_aside: n, by: r.set_aside_by ?? {}, count };
    }
    if (ask.planning !== undefined) {
      const li = host.layoutIndexOf(id);
      if (li < 0) return { error: 'It is not on the map now' };
      const to = ask.planning === 'auto' ? undefined : ask.planning;
      const auto = to === undefined;
      const r = await host.view.gen.setPlanning(li, to === undefined ? undefined : to / 100, auto, now);
      if (!r) return unanswered;
      if ('error' in r) return { error: sentence(r.error) };
      if (!('edits' in r) || r.unchanged || !r.edits) return { unchanged: true };
      const n = r.set_aside ?? 0;
      return { edits: r.edits, change: { tool: 'update_settlement', id, name, planning: to === undefined ? 'as rolled' : `${to}`, set_aside: n }, set_aside: n, by: r.set_aside_by ?? {}, count };
    }
    const old = now.created ?? [];
    const k = old.findIndex((c) => c.id === id && !c.removed);
    if (k < 0) return { error: 'It was deleted meanwhile' };
    const c: Created = { ...old[k] };
    if (ask.tier) c.kind = ask.tier;
    if (ask.lives_by) c.lives_by = ask.lives_by;
    const to = ask.name?.trim();
    if (to !== undefined) {
      if (!to) return { error: 'Give it a name' };
      c.name = to;
    }
    // (A name given wins over a rename made before, as in mapd.)
    const renamed = to !== undefined && now.renames?.[id] !== undefined;
    if (c.kind === old[k].kind && c.lives_by === old[k].lives_by && c.name === old[k].name && !renamed) return { unchanged: true };
    if (c.kind !== old[k].kind) {
      const spot = await host.view.gen.settlementSpot(c.kind, id, c.x, c.y);
      if (!spot) return unanswered;
      if ('error' in spot) return { error: `A ${c.kind} won’t fit here: ${spot.error}` };
    }
    const next: Edits = { ...now, created: old.map((x, i) => (i === k ? c : x)) };
    if (renamed) {
      const { [id]: _, ...rest } = now.renames!;
      next.renames = rest;
    }
    const r = await host.view.gen.setAside(old, next);
    if (!r) return unanswered;
    if ('error' in r) return { error: sentence(r.error) };
    const change: Change = { tool: 'update_settlement', id, name, set_aside: r.moved };
    if (ask.tier) change.kind = ask.tier;
    if (ask.lives_by) change.lives_by = ask.lives_by;
    if (to !== undefined) change.renamed = to;
    return { edits: r.edits, change, set_aside: r.moved, by: r.by ?? {}, count };
  }

  /** Make a settlement card's change, worked out by `planSettlement` (unless others came since). */
  function applySettlement(p: Extract<SettlePlan, { edits: Edits }>) {
    if (p.count !== host.editCount) return host.toast('The world changed meanwhile: try again');
    host.applyEdits(p.edits, p.change, 'user');
    reselectFeature(String(p.change.id));
  }

  /** The card of a settlement shown anew (its tier or name may have changed). */
  function reselectFeature(id: string) {
    const f = host.overlay?.features.find((g) => g.id === id);
    if (f) host.selection = { kind: 'feature', feature: f };
  }

  /** Move the village or town added by hand `id`: the next click puts it there (its ring and the
   * spot check, itself left out). */
  function startMove(id: string) {
    const c = host.edits.created?.find((x) => x.id === id && !x.removed);
    if (!c || (c.kind !== 'village' && c.kind !== 'town')) return;
    host.places.armPlace({ kind: c.kind, settlement: { tier: c.kind, livesBy: '', capital: false, wards: '' } }, host.nameOf(id), id);
  }

  /** Put the village or town added by hand `id` at (x, y) where the generator allows: its id,
   * name, notes, NPCs and plots kept, the changes made inside it set aside. One step. */
  async function moveSettlement(id: string, x: number, y: number) {
    if (!id.startsWith('c:')) return void waitMove(id, x, y);
    const now = $state.snapshot(host.edits) as Edits;
    const old = now.created ?? [];
    const k = old.findIndex((c) => c.id === id && !c.removed);
    if (k < 0) return host.toast('It was deleted meanwhile');
    const spot = await host.view.gen.settlementSpot(old[k].kind, id, x, y);
    if (!spot) return host.toast('The map could not answer: try again');
    if ('error' in spot) return host.toast(sentence(spot.error));
    const next: Edits = { ...now, created: old.map((c, i) => (i === k ? { ...c, x: spot.x, y: spot.y } : c)) };
    const r = await host.view.gen.setAside(old, next);
    if (!r || 'error' in r) return host.toast(r ? sentence(r.error) : 'The map could not answer: try again');
    host.applyEdits(r.edits, { tool: 'move_settlement', id, name: host.nameOf(id), set_aside: r.moved }, 'user');
    reselectFeature(id);
  }

  /** The villages and towns added by hand and deleted, for the Gazetteer (what comes back). */
  const removedMade = $derived(
    (host.edits.created ?? [])
      .filter((c) => c.removed && (c.kind === 'village' || c.kind === 'town'))
      .map((c) => {
        const n = host.asideGroups.find((g) => g.key === `layout:${c.aside}`)?.entries.length ?? 0;
        return { id: c.id, name: host.renames[c.id] ?? c.name, what: `${c.kind}${n ? ` · brings back ${n} ${n === 1 ? 'change' : 'changes'} set aside` : ''}` };
      }),
  );

  /** Roads, streets and paths taken off the map, as the generator names them (for the Gazetteer). */
  let removedRoads = $state<RemovedRoad[]>([]);
  let removedRoadsSeq = 0;
  $effect(() => {
    const ids = host.edits.removed_roads;
    void host.edits.towns;
    const s = ++removedRoadsSeq;
    if (!ids?.length) {
      removedRoads = [];
      return;
    }
    // (After the generators have the edits.)
    setTimeout(() => void host.view.gen.removedRoads().then((list) => s === removedRoadsSeq && (removedRoads = list)), 0);
  });

  /** Bring the removed road `id` back (or forget it, if it is not there any more): one step. */
  async function bringBackRoad(id: string) {
    const forget = removedRoads.find((r) => r.id === id)?.found === false;
    const cut = await host.view.gen.roadCut([id], true);
    if (!cut) return host.toast('The map could not answer: try again');
    if ('error' in cut) return host.toast(sentence(cut.error));
    let next = withCut($state.snapshot(host.edits) as Edits, cut);
    if (!cut.removed_roads && host.edits.removed_roads?.includes(id)) next = { ...next, removed_roads: host.edits.removed_roads.filter((x) => x !== id) };
    for (const k of Object.keys(cut.towns)) host.view.hintTown(k, cut.towns[k], cut.rects);
    host.applyEdits(next, { tool: 'remove_roads', restore: true, forget, ids: [id], also: cut.also.length }, 'user');
  }

  /** Towns whose fences and hedges were changed by hand (for the Gazetteer). */
  let changedFences = $state<ChangedFences[]>([]);
  let changedFencesSeq = 0;
  function askChangedFences() {
    const towns = host.edits.towns;
    const s = ++changedFencesSeq;
    if (!towns || !Object.values(towns).some((t) => t?.fences)) {
      changedFences = [];
      return;
    }
    // (After the generators have the edits.)
    setTimeout(() => void host.view.gen.changedFences().then((list) => s === changedFencesSeq && (changedFences = list)), 0);
  }
  $effect(askChangedFences);

  /** Bring a town's changed fences back (or forget the changes, if they are set aside): one step. */
  async function bringBackFences(key: string) {
    const row = changedFences.find((c) => c.town === key);
    if (!row) return;
    if (!row.found) {
      const { fences: _set, ...rest } = host.edits.towns?.[key] ?? { at: [0, 0] as [number, number] };
      // (A town's edit holding only its place is no edit.)
      const towns = { ...(host.edits.towns ?? {}) };
      if (Object.keys(rest).some((k) => k !== 'at')) towns[key] = rest as TownEdit;
      else delete towns[key];
      return host.applyEdits({ ...($state.snapshot(host.edits) as Edits), towns }, { tool: 'remove_fences', restore: true, forget: true, ids: row.ids }, 'user');
    }
    const out = await host.view.gen.fenceChange({ ids: row.ids, op: 'restore' });
    if (!out) return host.toast('The map could not answer: try again');
    if ('error' in out) return host.toast(sentence(out.error));
    if (!out.done.length) return host.toast(`Not brought back: ${[...new Set(out.refused.map((r) => r.why))].join('; ')}`);
    host.applyEdits(withFence($state.snapshot(host.edits) as Edits, out), { tool: 'remove_fences', restore: true, ids: row.ids }, 'user');
  }

  /** Restore the deleted village or town `id` where it was (if the spot is still free), with
   * what was set aside when it was deleted. One step. */
  async function restoreSettlement(id: string) {
    const name = host.nameOf(id);
    const r = await host.view.gen.restoreMade(id, $state.snapshot(host.edits) as Edits);
    if (!r) return host.toast('The map could not answer: try again');
    if ('error' in r) return host.toast(`Can’t restore ${name}: ${r.error}`);
    host.applyEdits(r.edits, { tool: 'restore_feature', id, name, brought_back: r.back }, 'user');
  }

  /** The culture of the selected settlement as it is laid out now (asked of the generator). */
  let cardCulture = $state<{ id: string; culture: number } | null>(null);
  $effect(() => {
    const sel = host.selection;
    void host.edits;
    if (sel?.kind !== 'feature' || !SETTLEMENT_KINDS.includes(sel.feature.kind)) return;
    const id = sel.feature.id;
    const li = host.layoutIndexOf(id);
    if (li < 0) return;
    void host.view.gen.setCulture(li, undefined, $state.snapshot(host.edits) as Edits).then((r) => {
      const k = r && 'from' in r ? CULTURES.findIndex((c) => c.toLowerCase() === r.from) : -1;
      if (host.selection?.kind === 'feature' && host.selection.feature.id === id) cardCulture = k >= 0 ? { id, culture: k } : null;
    });
  });

  /** How planned the selected town's streets are now (null: a village, or one added by hand). */
  let cardPlanning = $state<{ id: string; p: PlanningNow } | null>(null);
  $effect(() => {
    const sel = host.selection;
    void host.edits;
    if (sel?.kind !== 'feature' || !SETTLEMENT_KINDS.includes(sel.feature.kind)) return;
    const id = sel.feature.id;
    const li = host.layoutIndexOf(id);
    if (li < 0 || id.startsWith('c:')) return;
    void host.view.gen.setPlanning(li, undefined, false, $state.snapshot(host.edits) as Edits).then((r) => {
      if (host.selection?.kind === 'feature' && host.selection.feature.id === id) cardPlanning = r && 'value' in r ? { id, p: r } : null;
    });
  });

  /** The selected settlement's Settlement block (culture for all; tier, living, name and Move
   * for a village or town added by hand). */
  function settleCard(f: Feature) {
    if (!SETTLEMENT_KINDS.includes(f.kind)) return null;
    const c = f.id.startsWith('c:') ? host.edits.created?.find((x) => x.id === f.id && !x.removed) : undefined;
    const fx = !c && cardFacts?.id === f.id ? cardFacts.f : null;
    const lives = f.detail?.split(', pop.')[0] ?? null;
    return {
      made: !!c,
      tier: c?.kind ?? f.kind,
      livesBy: c ? (c.lives_by ?? null) : lives,
      capital: !!f.detail?.endsWith(', capital'),
      culture: cardCulture?.id === f.id ? cardCulture.culture : null,
      planning: cardPlanning?.id === f.id && !f.id.startsWith('c:') ? cardPlanning.p : null,
      name: c?.name ?? f.name,
      plan: (a: SettleAsk) => planSettlement(f.id, a),
      apply: applySettlement,
      onMove: c ? () => startMove(f.id) : fx?.pin != null ? () => startPinMove(f.id) : undefined,
      ...(fx ? ownCard(f.id, fx) : {}),
    };
  }

  /** The Settlement block's part for one of the world's own or a pin-made settlement: as its pin
   * would have it, with the change waiting for a remake. */
  function ownCard(id: string, fx: SettlementFacts) {
    const w = waitingList.find((x) => x.id === id);
    const base = pinBase(fx);
    const name = host.nameOf(id);
    return {
      own: {
        tier: w?.pin.tier ?? fx.tier,
        livesBy: w ? (w.pin.kind ?? '') : base.kind,
        capital: w ? !!w.pin.capital : base.capital,
        portOk: fx.port_ok,
        fishingOk: fx.fishing_ok,
        waiting: w ? waitWords(base, w.pin) : null,
        byPin: fx.pin !== null,
      },
      wait: (a: WaitAsk) => waitChange(id, a),
      onCancel: w ? () => putWaiting(id, null, `${name}: change cancelled`) : undefined,
      onSketch: fx.pin !== null ? () => showInSketch(fx) : undefined,
      onRemake: w ? () => void remakeWaiting() : undefined,
    };
  }

  /** Move the settlement made by pin `id`: the next click puts its pin there (it waits for a
   * remake). */
  function startPinMove(id: string) {
    const fx = cardFacts?.id === id ? cardFacts.f : null;
    if (!fx) return;
    const tier = (waitingList.find((x) => x.id === id)?.pin.tier ?? fx.tier) as PinTier;
    host.places.armPlace({ kind: tier, settlement: { tier, livesBy: '', capital: false, wards: '' } }, host.nameOf(id), id);
  }

  /** World › Sketch at a pin-made settlement's pin. */
  function showInSketch(fx: SettlementFacts) {
    const at = waitingList.find((x) => x.index === fx.pin)?.pin.pts[0] ?? host.world.sketch?.strokes?.[fx.pin!]?.pts[0] ?? fx.spot;
    host.go('world', 'sketch');
    host.flyToSize(at[0], at[1], 12 * MI);
  }

  return {
    get waiting() {
      return waiting;
    },
    set waiting(v) {
      waiting = v;
    },
    get waitBar() {
      return waitBar;
    },
    get removedMade() {
      return removedMade;
    },
    get removedRoads() {
      return removedRoads;
    },
    get changedFences() {
      return changedFences;
    },
    waitKey,
    waitingSketch,
    askChangedFences,
    bringBackRoad,
    bringBackFences,
    restoreSettlement,
    moveSettlement,
    settleCard,
  };
}
