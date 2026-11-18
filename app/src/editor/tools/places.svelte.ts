// Edit › Places: a site or settlement put on the map by a click (`armPlace`), a village or town
// added at once (a created settlement) or by a pin and a remake (Take back), a settlement taken
// away again, and the info panel's actions for a created site. App keeps `syncTool()` / `go()`,
// the keymap's Escape and the markup reading these.
import { untrack } from 'svelte';
import type { Created, Edits, Feature, Overlay, Stroke, WorldFile } from '../../gen/protocol';
import type { PointerTool } from '../../render/MapView';
import { shell, type Ask, type Remake } from '../../ui/shell/layout.svelte';
import type { SiteChoice } from '../../ui/PlacePanel.svelte';
import { MI, pinStroke, type PinTier } from '../sketcher';
import { madeCount, madeOn, type MadeOn } from '../reanchor';
import { tokensIn, type Carried } from '../../play/carry';
import type { PlayState } from '../../play/state';
import { diffEdits } from '../../sync/ops';
import { sentence, SETTLEMENT_KINDS, type Selection } from '../../ui/gazetteer';
import { existingSite, GEN_VERSION } from '../../world/world';

/** A settlement change waiting for a remake: the pin it is to have, in place of stroke `index`
 * of the world's sketch (null: added), and the settlement it is for. */
export type Waiting = { id: string; name: string; index: number | null; pin: Stroke };
/** What the last remake did with the changes (App's `generate` leaves it in `lastFollowed`). */
type Followed = { edits: Edits; kept: number; parked: number; removed: number };
/** A settlement a remake takes away (its notes set aside), as App's `generate` takes it. */
type Gone = { id: string; name: string; li: number };

/** What Places needs from the app. */
export interface PlacesHost {
  readonly view: import('../../render/MapView').MapView;
  /** The map's element (the ring is placed in page px). */
  readonly container: HTMLDivElement;
  readonly edits: Edits;
  readonly world: WorldFile;
  readonly renames: Record<string, string>;
  readonly overlay: Overlay | null;
  selection: Selection | null;
  readonly settlements: Feature[];
  readonly worldShown: boolean;
  readonly playOn: boolean;
  readonly busy: boolean;
  /** The site designer open (its rooms unsaved when `dirty`). */
  readonly designer: { dirty: boolean } | null;
  readonly play: { readonly state: PlayState };
  readonly placeHidden: string[];
  /** Changes made to the edits (`applyEdits`), counted: whether any came in meanwhile. */
  readonly editCount: number;
  /** How many changes the last remake made itself. */
  readonly remapCount: number;
  readonly lastFollowed: Followed | null;
  readonly lastCarried: Carried | null;
  /** The Objects tool's ring over the map (screen px). */
  ring: { x: number; y: number; r: number } | null;
  toast(text: string, undo?: () => void, more?: Pick<import('../../ui/Toasts.svelte').Toast, 'action' | 'sticky' | 'ms'>): number;
  dismiss(id: number): void;
  applyEdits(next: Edits, change: import('../../sync/mapd').Change, author: string, opts?: { record?: boolean; send?: boolean; quiet?: boolean }): void;
  syncTool(): void;
  ask(kind: Ask['kind'], proceed: () => void, generate?: boolean): void;
  generate(w: WorldFile, o?: { confirmed?: boolean; quiet?: boolean; gone?: Gone[] }): Promise<boolean>;
  remakeOf(w: WorldFile): Remake | null;
  playSaid(c: Carried | null): string;
  select(s: Selection): void;
  sketchDirty(): boolean;
  drawnBuilding(id: string): Created | null;
  layoutIndexOf(id: string): number;
  setHidden(id: string, hide: boolean): void;
  editBuilding(id: string): void;
  editWorks(id: string): void;
  /** Put the village or town `id` at (x, y) (Move on its card). */
  moveSettlement(id: string, x: number, y: number): Promise<unknown>;
  /** Changes that waited for a remake taken back wait again. */
  waitAgain(list: Waiting[]): void;
}

export function placeTools(host: PlacesHost) {
  // The place menu: put a site on the map (a click places it).
  let placeArmed = $state(false);
  /** What is being placed ("crypt"), for the banner over the map. */
  let placeLabel = $state('site');
  /** The village or town added by hand being moved (its id), else null: one being placed. */
  let placeMove = $state<string | null>(null);
  let placeTool: PointerTool | null = null;

  /** Why the spot under the pointer won't do, while placing a village or town (null: it will). */
  let placeNote = $state<string | null>(null);
  let spotSeq = 0;
  let spotAt = 0;

  /** A village or town goes in at once, unless it is to be a realm's capital. */
  const isInstant = (c: SiteChoice) => !!c.settlement && (c.settlement.tier === 'village' || c.settlement.tier === 'town') && !c.settlement.capital;


  /** Ask the generator whether a village or town can go under the pointer (at most one question
   * in flight; the last answer wins). */
  function checkSpot(c: SiteChoice, x: number, y: number, id = `c:${(host.edits.created ?? []).length}`) {
    const now = performance.now();
    if (now - spotAt < 120) return;
    spotAt = now;
    const seq = ++spotSeq;
    void host.view.gen.settlementSpot(c.settlement!.tier, id, x, y).then((r) => {
      if (seq === spotSeq && placeArmed) placeNote = r && 'error' in r ? sentence(r.error) : null;
    });
  }

  /** Add a village or town at (x, y) as the generator allows, with no remake: a created
   * settlement (undoable), named and cultured from where it stands. */
  async function createSettlement(c: SiteChoice, x: number, y: number) {
    const st = c.settlement!;
    if (host.world.gen_version !== GEN_VERSION) return host.toast('This world was made with an older generator: open it upgraded to add settlements');
    const id = `c:${(host.edits.created ?? []).length}`;
    const spot = await host.view.gen.settlementSpot(st.tier, id, x, y);
    if (!spot) return host.toast('The map could not answer: try again');
    if ('error' in spot) return host.toast(sentence(spot.error));
    const nid = `c:${(host.edits.created ?? []).length}`;
    const lives_by = st.livesBy && st.livesBy !== 'port' && st.livesBy !== 'fishing' ? st.livesBy : spot.lives_by;
    const site: Created = { id: nid, kind: st.tier, x: spot.x, y: spot.y, name: c.name ?? spot.name, culture: spot.culture, lives_by };
    host.applyEdits({ ...host.edits, created: [...(host.edits.created ?? []), site] }, { tool: 'create_settlement', id: nid, kind: st.tier, name: site.name }, 'user');
    const f = host.overlay?.features.find((g) => g.id === nid);
    if (f) host.selection = { kind: 'feature', feature: f };
    placedName = site.name;
    placedId = nid;
    placeNote = null;
  }

  /** The next click on the map puts `c` there (`move`: the village or town added by hand, of that
   * id, goes there instead). */
  /** What Places added at once last (its strip: Place another, Done), else null. */
  let placedName = $state<string | null>(null);
  let placedId: string | null = null;
  // (Selecting something else puts the "Added X" line away.)
  $effect(() => {
    const sel = host.selection;
    if (placedName && placedId && !(sel?.kind === 'feature' && sel.feature.id === placedId)) untrack(() => (placedName = null));
  });
  /** The remake's Added whose Places strip was put away (Done). */
  let addedDone = $state.raw<Added | null>(null);

  function armPlace(c: SiteChoice, label = 'site', move: string | null = null) {
    disarmPlace();
    placedName = null;
    placeLabel = label;
    placeMove = move;
    // A settlement's ring: about how far it reaches, at a middling size for its tier.
    const reach = c.settlement ? SETTLEMENT_REACH_FT[c.settlement.tier] : 0;
    const showRing = (x: number, y: number) => {
      const [sx, sy] = host.view.cam.worldToPage(x, y);
      const b = host.container.getBoundingClientRect();
      host.ring = { x: b.left + sx, y: b.top + sy, r: Math.max(6, reach * host.view.cam.ppf) };
    };
    // (A pin moved waits for a remake: no spot check of a village added at once.)
    const instant = isInstant(c) && !(move && !move.startsWith('c:'));
    const tool: PointerTool = {
      down: () => true,
      move: (x, y) => reach && showRing(x, y),
      hover: (x, y) => {
        if (reach) showRing(x, y);
        if (instant) checkSpot(c, x, y, move ?? undefined);
      },
      leave: () => ((host.ring = null), (placeNote = null)),
      hoverFollows: true,
      cancel: () => disarmPlace(),
      up: (x, y) => {
        disarmPlace();
        void (move ? host.moveSettlement(move, x, y) : c.settlement ? (isInstant(c) ? createSettlement(c, x, y) : addSettlement(c, x, y)) : createSite(c, x, y));
      },
    };
    placeTool = tool;
    placeArmed = true;
    if (shell.phone) shell.snap = 'peek';
    host.syncTool();
  }

  function disarmPlace() {
    placeMove = null;
    placeNote = null;
    placeTool = null;
    placeArmed = false;
    host.ring = null;
    host.syncTool();
  }

  /** How far a settlement of each tier reaches (ft), at the middle of its tier's sizes (as
   * `town::reach` makes it): the ring shown while placing one. */
  const SETTLEMENT_REACH_FT: Record<PinTier, number> = { village: 660, town: 1000, city: 2900, metropolis: 5800 };

  /** A settlement added from Places: the world before and after (its sketch and changes), so
   * Take back can make it again as it was. */
  interface Added {
    name: string;
    /** The toast's words. */
    text: string;
    seed: number;
    params: string;
    sketchBefore: WorldFile['sketch'];
    sketchAfter: string;
    /** The changes as they were before (null: others came in while the world was remade, so
     * they can't simply be put back), and `editCount` once it was added. */
    editsBefore: Edits | null;
    count: number;
    parked: number;
    toast: number;
    /** The settlement placed (null: none was), its layout number then, and its pin's stroke. */
    id: string | null;
    li: number;
    stroke: number;
    /** The changes right after it came (Undo back to them still counts as untouched). */
    editsAfter: Edits;
    /** A remake with settlement changes that waited for it (Take back has them wait again), not
     * a settlement added. */
    changes?: Waiting[];
  }
  let added = $state.raw<Added | null>(null);

  function clearAdded() {
    if (added) host.dismiss(added.toast);
    added = null;
  }

  /** The world shown is the one `a` was added to, as it was left then. */
  const isAddedWorld = (a: Added) => host.worldShown && host.world.seed === a.seed && JSON.stringify(host.world.params) === a.params && JSON.stringify(host.world.sketch ?? null) === a.sketchAfter;

  /** The notice of a settlement added, with Take back (it fades: Take back stays in Places, on the
   * settlement's card and in Undo). */
  function offerTakeBack(a: Added) {
    if (added) host.dismiss(added.toast);
    added = { ...a, toast: host.toast(a.text, undefined, { ms: 30_000, action: { label: 'Take back', run: () => takeBack() } }) };
  }

  /** Add a settlement at (x, y): a pin in the sketch with the options chosen, and the world
   * remade round it (asked first when it has changes, as any remake is); then say what came
   * of it, with Take back. */
  async function addSettlement(c: SiteChoice, x: number, y: number) {
    const st = c.settlement!;
    if (host.playOn) return host.toast('Stop playing to remake the world');
    if (host.busy) return;
    if (host.world.gen_version !== GEN_VERSION) return host.toast('This world was made with an older generator: open it upgraded to add settlements');
    const strokes = ($state.snapshot(host.world.sketch?.strokes ?? []) as Stroke[]).slice();
    // (Within 3 miles of another settlement none is placed: the generator's rule.)
    const near = (host.overlay?.features ?? []).find((f) => SETTLEMENT_KINDS.includes(f.kind) && Math.hypot(f.x - x, f.y - y) < 3 * MI);
    if (near) return host.toast(`Too close to ${near.name}: keep settlements 3 miles apart`);
    if (strokes.some((s) => s.tool === 'pin' && Math.hypot(s.pts[0][0] - x, s.pts[0][1] - y) < 3 * MI)) return host.toast('Too close to a settlement placed by hand: keep them 3 miles apart');
    const spot = await host.view.gen.spot('settlement', undefined, 'c:0', x, y);
    if (!spot) return host.toast('The map could not answer: try again');
    if ('error' in spot) return host.toast(spot.error.charAt(0).toUpperCase() + spot.error.slice(1));
    const pin = pinStroke([x, y], { tier: st.tier, kind: st.livesBy, capital: st.capital, wards: st.wards, name: c.name, added: true });
    const next: WorldFile = { ...($state.snapshot(host.world) as WorldFile), sketch: { strokes: [...strokes, pin] } };
    const run = async () => {
      if (host.playOn || host.busy) return host.toast(host.playOn ? 'Stop playing to remake the world' : 'The world is being made: try again');
      clearAdded();
      // (As they are now: the question may have been open a while.)
      const before = $state.snapshot(host.world) as WorldFile;
      const n0 = host.editCount;
      if (!(await host.generate({ ...before, sketch: next.sketch }, { confirmed: true, quiet: true }))) return;
      const k = strokes.length;
      const conflicts = (host.overlay?.conflicts ?? []).filter((x) => x.stroke === k).map((x) => x.message);
      const missing = conflicts.find((m) => m.includes('no settlement placed'));
      // (The generator's word on which settlement the pin made.)
      const li = missing ? -1 : (host.overlay?.added?.find(([, s]) => s === k)?.[0] ?? -1);
      const made = li >= 0 ? (host.settlements[li] ?? null) : null;
      const name = made?.name ?? c.name ?? `the ${st.tier}`;
      const m = host.lastFollowed as Followed | null;
      const count = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
      const kept = m ? (m.parked ? ` Kept ${count(m.kept)}; ${count(m.parked)} set aside: their places came out differently.` : ` Kept ${count(m.kept)} with the remade world.`) : '';
      const other = conflicts.filter((x) => x !== missing);
      const said = host.playSaid(host.lastCarried);
      const text = `${missing ? `No ${st.tier} placed: ${missing.replace(/^This (pin|settlement) /, 'it ').replace(/: no settlement placed$/, '')}.` : `Added ${name}.${other.length ? ` ${other.join('; ')}.` : ''}`}${kept}${said ? ` ${said}` : ''}`;
      offerTakeBack({
        name,
        text,
        seed: before.seed,
        params: JSON.stringify(before.params),
        sketchBefore: before.sketch,
        sketchAfter: JSON.stringify(host.world.sketch ?? null),
        // (Only the remake's own change came in while it was made.)
        editsBefore: host.editCount - n0 === host.remapCount ? (before.edits ?? {}) : null,
        count: host.editCount,
        parked: m?.parked ?? 0,
        toast: 0,
        id: made?.id ?? null,
        li,
        stroke: k,
        editsAfter: host.edits,
      });
      if (made) host.select({ kind: 'feature', feature: made });
    };
    if (host.remakeOf(next)) host.ask('regen', () => void run());
    else await run();
  }

  /** A settlement to take away again: its feature id (null: the pin placed none), layout number,
   * name and pin's stroke; `a` when it is the one last added (Take back: its changes can be put
   * back as they were). */
  type Removal = { id: string | null; li: number; name: string; stroke: number; a: Added | null; card?: boolean };

  /** The question before a settlement is taken away: what was made on it, and whether the world
   * has other changes a remake may set aside. */
  let removeAsk = $state<{ r: Removal; on: MadeOn | null; tokens: number; others: boolean; choose: (go: boolean) => void } | null>(null);

  /** What was made on a place, in words ("1 note, 1 NPC and 2 building changes"). */
  function madeWords(on: MadeOn): string {
    const parts = (
      [
        [on.notes, 'note', 'notes'],
        [on.npcs, 'NPC', 'NPCs'],
        [on.plots, 'plot point', 'plot points'],
        [on.names, 'name', 'names'],
        [on.buildings, 'building change', 'building changes'],
        [on.designs, 'designed interior', 'designed interiors'],
        [on.towns, 'its streets re-planned', 'its streets re-planned'],
      ] as [number, string, string][]
    )
      .filter(([k]) => k > 0)
      .map(([k, one, many]) => (one.startsWith('its') ? one : `${k} ${k === 1 ? one : many}`));
    return parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}` : (parts[0] ?? '');
  }

  /**
   * Take a settlement added from Places away again: the world made again without its pin (asked
   * first when something was made on it, and always when `confirm` or not the one just added).
   * What was made on it is set aside (`gone:` in Notes, its layout's changes parked), never handed
   * to a neighbour. The one last added gets the changes back as they were before it came, when
   * none were made since (or they were undone) and none came while either world was made.
   */
  async function removeSettlement(r: Removal, confirm = false) {
    if (removeAsk) return;
    if (host.playOn || host.busy) return host.toast(host.playOn ? 'Stop playing to remake the world' : 'The world is being made: try again');
    // (Strokes or a design not saved yet are asked about first.)
    if (host.sketchDirty() || host.designer?.dirty) return host.sketchDirty() ? host.ask('sketch', () => void removeSettlement(r, confirm), false) : host.ask('design', () => void removeSettlement(r, confirm));
    const a = r.a;
    const moved = () => {
      clearAdded();
      host.toast(a?.changes ? 'The world has changed since: make the changes again on the settlements’ cards' : 'The world has changed since: erase the settlement in World › Sketch');
    };
    if (a && !isAddedWorld(a)) return moved();
    if (!host.worldShown) return;
    const on = r.id ? madeOn(host.edits, r.id, r.li) : null;
    // (Tokens in it wait outside: asked about as what was made on it is.)
    const tokens = r.id && r.li >= 0 ? tokensIn(host.play.state, r.li) : 0;
    if (confirm || !a || (on && madeCount(on)) || tokens) {
      const go = await new Promise<boolean>((choose) => (removeAsk = { r, on, tokens, others: Object.keys(host.edits).length > 0, choose }));
      removeAsk = null;
      if (!go) return;
      if (host.playOn || host.busy) return host.toast(host.playOn ? 'Stop playing to remake the world' : 'The world is being made: try again');
      if (a && !isAddedWorld(a)) return moved();
    }
    if (a) clearAdded();
    const untouched = !!a?.editsBefore && (host.editCount === a.count || !diffEdits(a.editsAfter, host.edits).length);
    const { sketch: _, ...rest } = $state.snapshot(host.world) as WorldFile;
    const strokes = ($state.snapshot(host.world.sketch?.strokes ?? []) as Stroke[]).filter((_, i) => i !== r.stroke);
    const sketch = a ? a.sketchBefore : strokes.length ? { strokes } : undefined;
    const back: WorldFile = sketch ? { ...rest, sketch } : rest;
    const n0 = host.editCount;
    const gone: Gone[] = r.id ? [{ id: r.id, name: r.name, li: r.li }] : [];
    if (!(await host.generate(back, { confirmed: true, quiet: true, gone }))) return;
    const restore = untouched && host.editCount - n0 === host.remapCount;
    const m = restore ? null : (host.lastFollowed as Followed | null);
    if (restore) host.applyEdits(a!.editsBefore!, { tool: 'reanchor', label: `Took back ${r.name}` }, 'user', { record: false, quiet: true });
    // (Changes that waited for the remake taken back wait again.)
    if (a?.changes) host.waitAgain(a.changes);
    const count = (n: number) => `${n} ${n === 1 ? 'change' : 'changes'}`;
    const what = m?.removed && on ? madeWords(on) : '';
    // (Not put back as they were: what was set aside with it, what the remake kept or set aside,
    // and those set aside when it was added.)
    const said = restore
      ? ''
      : `${what ? ` Set aside in Notes, as “Deleted: ${r.name}”: ${what}.` : ''}${m?.kept ? ` Kept ${count(m.kept)} with the remade world.` : ''}${m?.parked ? ` ${count(m.parked)} set aside: their places came out differently.` : ''}${a?.parked ? ` ${count(a.parked)} set aside when it was added stay aside: others were made since.` : ''}`;
    const played = host.playSaid(host.lastCarried);
    host.toast(`${a && !r.card ? 'Took back' : 'Deleted'} ${r.name}.${said}${played ? ` ${played}` : ''}`, undefined, said || played ? { ms: 12_000 } : {});
  }

  /** Take back the settlement last added (`confirm`: always asked, as from Undo). */
  function takeBack(confirm = false) {
    const a = added;
    if (a) void removeSettlement({ id: a.id, li: a.li, name: a.name, stroke: a.stroke, a }, confirm);
  }

  /** The card's Remove for a settlement added from Places: Take back when it is the one last
   * added, else the world made again without its pin. Undefined for any other place. */
  function unsettleAction(id: string): (() => void) | undefined {
    const li = host.settlements.findIndex((f) => f.id === id);
    const stroke = li < 0 ? undefined : host.overlay?.added?.find(([l]) => l === li)?.[1];
    if (stroke === undefined) return undefined;
    if (added?.id === id) return () => void removeSettlement({ id, li: added!.li, name: added!.name, stroke: added!.stroke, a: added, card: true }, true);
    const name = host.renames[id] ?? host.settlements[li].name;
    return () => void removeSettlement({ id, li, name, stroke, a: null });
  }

  /** Create a site near (x, y), where the generator allows (dry land, out of rivers), named
   * locally unless a name was given; then select it. */
  async function createSite(c: SiteChoice, x: number, y: number) {
    const { name, ...opts } = c;
    // The same site again: that one (as mapd does).
    const again = (at: Created) => {
      const old = existingSite(host.edits, at, [x, y]);
      if (!old) return false;
      host.toast(`${old.name} is already here`);
      const f = host.overlay?.features.find((g) => g.id === old.id);
      if (f) host.selection = { kind: 'feature', feature: f };
      return true;
    };
    if (again({ id: '', ...opts, x, y, name: '' })) return;
    const spot = await host.view.gen.spot(c.kind, c.under, `c:${(host.edits.created ?? []).length}`, x, y);
    if (!spot) return host.toast('The map could not answer: try again');
    if ('error' in spot) return host.toast(spot.error.charAt(0).toUpperCase() + spot.error.slice(1));
    // (Numbered as it is added: another site may have come in meanwhile.)
    const id = `c:${(host.edits.created ?? []).length}`;
    const site: Created = { id, ...opts, x: spot.x, y: spot.y, name: name ?? spot.name };
    if (again(site)) return;
    host.applyEdits({ ...host.edits, created: [...(host.edits.created ?? []), site] }, { tool: 'create_feature', id, kind: c.kind === 'entrance' ? (c.under ?? 'dungeon').replace(/_/g, ' ') : c.kind.replace(/_/g, ' '), name: site.name }, 'user');
    const f = host.overlay?.features.find((g) => g.id === id);
    if (f) host.selection = { kind: 'feature', feature: f };
    placedName = site.name;
    placedId = id;
  }

  /** The info panel's actions for a created site: hide or show it, delete it, go down into its
   * site underground. */
  function createdActions(id: string) {
    const c = host.edits.created?.find((x) => x.id === id) ?? host.drawnBuilding(id);
    if (!c) return {};
    id = c.id;
    const li = host.layoutIndexOf(id);
    const drawn =
      c.kind === 'building'
        ? {
            onEdit: () => host.editBuilding(id),
            onGoIn: c.structure === 'ruin' ? undefined : () => void host.view.enterBuilding(`b:${li}:0`),
          }
        : c.kind === 'castle' || c.kind === 'wall'
          ? { onEdit: () => host.editWorks(id) }
          : {};
    const hiddenNow = host.placeHidden.includes(id);
    const under = !['tower', 'camp', 'waystation', 'building', 'castle', 'wall', 'village', 'town'].includes(c.kind);
    return {
      ...drawn,
      hidden: hiddenNow,
      onHide: (hide: boolean) => host.setHidden(id, hide),
      onDelete: () => {
        const old = host.edits.created ?? [];
        const all = old.map((x) => (x.id === id ? { ...x, removed: true } : x));
        const next = { ...host.edits, created: all };
        // (A village or town takes its building, ward, name and note edits with it: set aside.)
        if (c.kind === 'village' || c.kind === 'town') {
          void host.view.gen.setAside($state.snapshot(old) as Created[], $state.snapshot(next) as Edits).then((r) => {
            host.applyEdits(r && !('error' in r) ? r.edits : next, { tool: 'delete_feature', id }, 'user');
          });
          return;
        }
        host.applyEdits(next, { tool: 'delete_feature', id }, 'user');
      },
      onDown: under
        ? () => {
            const li = host.layoutIndexOf(id);
            if (li >= 0) void host.view.enterBuilding(`u:${li}:0`);
          }
        : undefined,
    };
  }

  return {
    get placeArmed() {
      return placeArmed;
    },
    get placeLabel() {
      return placeLabel;
    },
    get placeMove() {
      return placeMove;
    },
    get placeTool() {
      return placeTool;
    },
    get placeNote() {
      return placeNote;
    },
    get placedName() {
      return placedName;
    },
    set placedName(v) {
      placedName = v;
    },
    get addedDone() {
      return addedDone;
    },
    set addedDone(v) {
      addedDone = v;
    },
    get added() {
      return added;
    },
    get removeAsk() {
      return removeAsk;
    },
    isAddedWorld,
    clearAdded,
    offerTakeBack,
    armPlace,
    disarmPlace,
    createSettlement,
    addSettlement,
    takeBack,
    unsettleAction,
    madeWords,
    createSite,
    createdActions,
  };
}

export type PlaceTools = ReturnType<typeof placeTools>;
