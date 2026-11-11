// Main-thread handle to the coordinator worker.
import type { PreparedChunk } from './battlePrep';
import { EDIT_FIELDS } from '../sync/ops';
import type { UnderResult } from '../ui/gazetteer';

/** Most entries of a field sent as a patch; beyond, the whole field. */
const PATCH_MAX = 2000;
import type { Ask, CultureChange, Crossing, Plateau, Wharf, BuildingEdit, PieceEdit, BuildingFuncs, Created, DesignReply, EditsPatch, DistrictLabel, Edits, Interior, MadeSettlement, NamedPlaces, SiteDesign, UnderCatalog, FromCoordinator, NameEntry, Parents, GenStats, Geom, Hit, Overlay, PlaceInfo, Rect, Remade, RemakeInput, TileMsg, ToCoordinator, TownEdit, TownPlan, TownReport, TownRequest, WantTile, WorldFile } from './protocol';

export interface Ready {
  geom: Geom;
  t0Ms: number;
  overlay: Overlay;
  /** Battlemap object kinds with 5e tactical data, indexed by kind id - 1. */
  catalog: KindInfo[];
}

export interface KindInfo {
  id: number;
  name: string;
  radius: number;
  blocks_move: boolean;
  blocks_sight: boolean;
  cover: number;
  difficult: boolean;
  height_ft: number;
  hazard: { name: string; effect: string } | null;
  feature: boolean;
}

export class GenClient {
  /** `epoch`: the last edits (`setEdits`) the tile was made with. */
  onTile: (t: TileMsg, epoch: number) => void = () => {};
  onStats: (s: GenStats) => void = () => {};
  onError: (message: string) => void = (m) => console.error('[gen]', m);
  onProgress: (stage: string, frac: number) => void = () => {};
  onBattlemap: (level: number, x: number, y: number, chunk: PreparedChunk, epoch: number) => void = () => {};
  /** The battlemap object catalog, early (before T0 is done): lets the renderer warm up. */
  onCatalog: (catalog: KindInfo[]) => void = () => {};

  private worker = new Worker(new URL('./coordinator.worker.ts', import.meta.url), { type: 'module' });
  private readyResolve: ((r: Ready) => void) | null = null;
  private asks = new Map<number, (json: string) => void>();
  private nextAsk = 1;

  constructor() {
    this.worker.onmessage = (e: MessageEvent<FromCoordinator>) => {
      const m = e.data;
      if (m.type === 'tile') this.onTile(m.tile, m.epoch);
      else if (m.type === 'stats') this.onStats(m.stats);
      else if (m.type === 'progress') this.onProgress(m.stage, m.frac);
      else if (m.type === 'battlemap') this.onBattlemap(m.level, m.x, m.y, m.chunk, m.epoch);
      else if (m.type === 'catalog') this.onCatalog(JSON.parse(m.catalog) as KindInfo[]);
      else if (m.type === 'ready')
        this.readyResolve?.({ geom: m.geom, t0Ms: m.t0Ms, overlay: JSON.parse(m.overlay) as Overlay, catalog: JSON.parse(m.catalog) as KindInfo[] });
      else if (m.type === 'answer') {
        this.asks.get(m.id)?.(m.json);
        this.asks.delete(m.id);
      } else if (m.type === 'error') this.onError(m.message);
    };
  }

  /** (Re)initialize with a world file. Resolves once T0 exists in every generator worker. */
  init(world: WorldFile): Promise<Ready> {
    const cores = navigator.hardwareConcurrency || 4;
    const workers = Math.max(1, Math.min(6, cores - 2));
    return new Promise((resolve, reject) => {
      this.readyResolve = resolve;
      try {
        this.send({ type: 'init', world, workers });
      } catch (e) {
        // (Never sent, a world that can't be copied to the workers: they keep the one shown.)
        this.readyResolve = null;
        reject(e);
        return;
      }
      this.sentFields = null;
    });
  }

  /** The building or settlement at a world position, if any. */
  query(x: number, y: number): Promise<Hit | null> {
    return this.ask({ op: 'query', x, y }).then((j) => JSON.parse(j) as Hit | null);
  }

  /** Districts and named buildings matching a search string (the first call builds every settlement). */
  searchBuildings(q: string, rect?: [number, number, number, number]): Promise<Extract<Hit, { kind: 'building' | 'district' }>[]> {
    return this.ask({ op: 'search', q, rect }).then((j) => JSON.parse(j) as Extract<Hit, { kind: 'building' | 'district' }>[]);
  }

  /** Districts and businesses inside a rectangle (x0, y0, x1, y1 ft). */
  inView(rect: [number, number, number, number]): Promise<Extract<Hit, { kind: 'building' | 'district' }>[]> {
    // (None while the world is being made again.)
    return this.ask({ op: 'inview', rect }).then((j) => (JSON.parse(j) as Extract<Hit, { kind: 'building' | 'district' }>[] | null) ?? []);
  }

  /** An interior by building or tower id, or null if it can't be entered. */
  interior(id: string): Promise<Interior | null> {
    return this.ask({ op: 'interior', id }).then((j) => JSON.parse(j) as Interior | null);
  }

  /** A place by any id (feature, building, district, site): its name and where it is. */
  place(id: string): Promise<PlaceInfo | null> {
    return this.ask({ op: 'place', id }).then((j) => JSON.parse(j) as PlaceInfo | null);
  }

  /** What can be renamed in a layout (by index) or in a building or site (by id); null if the
   * generator could not answer. */
  names(scope: number | string): Promise<NameEntry[] | null> {
    return this.ask({ op: 'names', scope: String(scope) }).then((j) => JSON.parse(j) as NameEntry[] | null);
  }

  /** The sites standing in settlements, as layout indices; null if the generator could not answer. */
  parents(): Promise<Parents | null> {
    return this.ask({ op: 'parents' }).then((j) => JSON.parse(j) as Parents | null);
  }

  /** Where a site of `kind` can be created near (x, y), and a name for it; null if the
   * generator could not answer. */
  spot(kind: string, under: string | undefined, id: string, x: number, y: number): Promise<{ x: number; y: number; name: string } | { error: string } | null> {
    return this.ask({ op: 'spot', kind, under, id, x, y }).then((j) => JSON.parse(j));
  }

  /** Whether a building drawn by hand can stand there (`id`: its created id), its point and a
   * name for it; null if the generator could not answer. */
  buildingSpot(poly: [number, number][], func: string | undefined, id: string): Promise<{ x: number; y: number; name: string } | { error: string; short?: string } | null> {
    return this.ask({ op: 'building', poly, func, id }).then((j) => JSON.parse(j));
  }

  /** Shop lots along a bridge (`rows` ft; lane absent: what the width leaves), or why not. */
  bridgeLots(crossing: Crossing, rows: { depth: number; frontage: number; lane?: number } | null): Promise<{ lots: { poly: [number, number][]; func: string }[] } | { error: string } | null> {
    return this.ask({ op: 'bridgelots', input: JSON.stringify({ crossing, rows }) }).then((j) => JSON.parse(j));
  }

  /** The created sites with the shops on crossing `id` laid out again on its new ends and width
   * (`old`: it as it was), as the generator does for agents; the whole list. */
  followBridge(id: string, crossing: Crossing, old: Crossing | undefined, created: Created[]): Promise<{ created: Created[] } | { error: string } | null> {
    return this.ask({ op: 'bridgelots', input: JSON.stringify({ crossing, follow: { id, old, created } }) }).then((j) => JSON.parse(j));
  }

  /** A hand islet round a point: its plateau and the water it stands in, or why not. */
  islet(key: string, x: number, y: number, radius: number, rise: number): Promise<{ plateau: Plateau; water: { kind: string; id: string | null; name: string | null; depth_ft: number } } | { error: string } | null> {
    return this.ask({ op: 'islet', key, x, y, radius, rise }).then((j) => JSON.parse(j));
  }

  /** Whether a wharf can be put down (a breakwater's lighthouse too, when asked): why not, or its lighthouse. */
  wharfCheck(wharf: Wharf, lighthouse: boolean): Promise<{ error: string } | { lighthouse?: { poly: [number, number][]; x: number; y: number; name: string } } | null> {
    return this.ask({ op: 'wharfcheck', wharf: JSON.stringify(wharf), lighthouse }).then((j) => JSON.parse(j));
  }

  /** A generated building's edit with `change` made (`{func, floors, poly, roof, tint, structure}`;
   * an empty string or `auto` goes back to as generated), or `{remove: true}`: the edit (null: as
   * generated) or why not; null if the generator could not answer. */
  buildingEdit(id: string, change: object): Promise<{ edit: BuildingEdit | null } | { piece: PieceEdit } | { error: string } | null> {
    return this.ask({ op: 'bedit', id, change: JSON.stringify(change) }).then((j) => JSON.parse(j));
  }

  /** The world's own buildings with their middle inside a polygon (ft): ids and middles as generated. */
  buildingsIn(poly: [number, number][]): Promise<{ id: string; at: [number, number] }[]> {
    return this.ask({ op: 'bin', poly }).then((j) => JSON.parse(j) ?? []);
  }

  /** Whether a castle or wall drawn by hand can stand there: its point, a name for it and the
   * world's own buildings in its way (to take away with it); null if the generator could not answer. */
  worksSpot(site: Created): Promise<{ x: number; y: number; name: string; in_way: { id: string; at: [number, number] }[] } | { error: string } | null> {
    return this.ask({ op: 'works', site }).then((j) => JSON.parse(j));
  }

  /** What a building drawn by hand can be. */
  buildingFuncs(): Promise<BuildingFuncs> {
    return this.ask({ op: 'funcs' }).then((j) => JSON.parse(j) as BuildingFuncs);
  }

  /** An underground site as a design: `design` (else the site's own), changed by `action`
   * (`{doors: level|null}`, `{furnish: {level, room, seed}}`, `{original: true}`), with the site
   * it builds and its problems. */
  design(id: string, design?: SiteDesign, action?: object): Promise<DesignReply> {
    return this.ask({ op: 'design', id, design: design && JSON.stringify(design), action: action && JSON.stringify(action) }).then(
      (j) => (JSON.parse(j) as DesignReply | null) ?? { error: 'The map could not answer: try again' },
    );
  }

  /** What the designer offers: props, room kinds, themes. */
  underCatalog(): Promise<UnderCatalog> {
    return this.ask({ op: 'undercat' }).then((j) => JSON.parse(j) as UnderCatalog);
  }

  /** A town's plan for the ward editor (patches, corners, walls), or why it has none. */
  townPlan(layout: number): Promise<TownPlan | { error: string }> {
    return this.ask({ op: 'townplan', layout }).then((j) => (JSON.parse(j) as TownPlan | { error: string } | null) ?? { error: 'The map could not answer: try again' });
  }

  /** A town changed as asked, the edits left alone: the town's edit it then has (null: as
   * generated) and what the change does, or why not. */
  townChange(layout: number, request: TownRequest): Promise<{ edit: TownEdit | null; report: TownReport } | { error: string }> {
    return this.ask({ op: 'townchange', layout, request: JSON.stringify(request) }).then(
      (j) => (JSON.parse(j) as { edit: TownEdit | null; report: TownReport } | { error: string } | null) ?? { error: 'The map could not answer: try again' },
    );
  }

  /** A settlement's named districts, for labels. */
  districts(settlement: number): Promise<DistrictLabel[]> {
    return this.ask({ op: 'districts', settlement }).then((j) => (JSON.parse(j) as DistrictLabel[] | null) ?? []);
  }

  /** The roads someone named and the crossings put down by hand (labels, search, gazetteer). */
  named(): Promise<NamedPlaces> {
    return this.ask({ op: 'named' }).then((j) => (JSON.parse(j) as NamedPlaces | null) ?? { labels: [], roads: [], crossings: [] });
  }

  /** Underground sites matching a search (`filters`: kinds, levels_min/max, rect, near, radius_ft, limit). */
  searchUnder(q: string, filters: object): Promise<UnderResult> {
    // (None while the world is being made again; an error, `{error}`, as none.)
    return this.ask({ op: 'under', q, filters: JSON.stringify(filters) }).then((j) => {
      const r = JSON.parse(j) as Partial<UnderResult> | null;
      return { items: r?.items ?? [], found: r?.found ?? 0, truncated: !!r?.truncated };
    });
  }

  /** The villages and towns added by hand, as the generator has them now (after every edit
   * sent before); null if it could not answer. */
  made(): Promise<MadeSettlement[] | null> {
    return this.ask({ op: 'made' }).then((j) => JSON.parse(j) as MadeSettlement[] | null);
  }

  /** Whether a village or town (`kind`; `id`: its created id) can be added at (x, y), with the
   * name, culture and living it would have; null if the generator could not answer. */
  settlementSpot(kind: string, id: string, x: number, y: number): Promise<{ x: number; y: number; name: string; culture: number; lives_by: string } | { error: string } | null> {
    return this.ask({ op: 'madespot', kind, id, x, y }).then((j) => JSON.parse(j));
  }

  /** `edits` with the edits made in each village or town added by hand that they remove, move or
   * lay out anew (from `old`, the created sites before) set aside, as the generator would. */
  setAside(old: Created[], edits: Edits): Promise<{ edits: Edits; moved: number } | { error: string } | null> {
    return this.ask({ op: 'setaside', old: JSON.stringify(old), edits: JSON.stringify(edits) }).then((j) => JSON.parse(j));
  }

  /** `edits` with settlement `layout` laid out in `culture` (0 north, 1 west, 2 south, 3 dry,
   * 4 steppe) and the edits keyed to its layout set aside (`set_aside`, by field `set_aside_by`), to
   * show before it is applied (as one change, through `applyEdits`); `unchanged` when it is in that
   * culture already. Without `culture`: `{from}`, the culture it is laid out in now. */
  setCulture(layout: number, culture: number | undefined, edits: Edits): Promise<CultureChange | { error: string } | null> {
    return this.ask({ op: 'setculture', layout, culture, edits: JSON.stringify(edits) }).then((j) => JSON.parse(j));
  }

  /** The fingerprints of the world's own layouts a remake must find alike to keep what is made on
   * them (those `edits` name and `played`), as `[layout, print]`; null if the generator could
   * not answer. */
  remakePrints(edits: Edits, played: number[]): Promise<[number, number][] | null> {
    return this.ask({ op: 'remakeprints', edits: JSON.stringify(edits), played }).then((j) => JSON.parse(j));
  }

  /** The edits made on the world before following their places into this one, remade from a
   * changed sketch (`t0::remap::follow_into`); null if the generator could not answer. */
  follow(input: RemakeInput): Promise<Remade | { error: string } | null> {
    return this.ask({ op: 'follow', input: JSON.stringify(input) }).then((j) => JSON.parse(j));
  }

  private ask(ask: Ask): Promise<string> {
    const id = this.nextAsk++;
    return new Promise((resolve) => {
      this.asks.set(id, resolve);
      this.send({ type: 'ask', id, ask });
    });
  }

  /** Live edits for every generator; created sites changed within `rects`, so what was made
   * there is made again (results from then on carry `epoch`). */
  /** `hint`: the edits these follow and the keys each field's changes touch since (from their
   * ops); used for a field when the workers have that field as it was, so it isn't searched. */
  /** `groundRects`: where the ground was shaped (the three finest levels' tiles draw it). */
  setEdits(edits: Edits, rects: Rect[], epoch: number, battleRects: Rect[] = [], hint?: { prev: Edits; changed: Record<string, string[]> }, groundRects: Rect[] = []) {
    // Only the fields that changed (edits are never changed in place: a field that is the same
    // object is unchanged); of a keyed field, only the entries that changed, unless most did.
    const fields: Record<string, string> = {};
    const patches: Record<string, EditsPatch> = {};
    const now = edits as Record<string, unknown>;
    const was = this.sentFields;
    for (const [f, shape] of Object.entries(EDIT_FIELDS)) {
      if (was && was[f] === now[f]) continue;
      const [a, b] = [was?.[f], now[f] ?? (shape === 'map' ? {} : [])] as [Record<string, unknown> | undefined, Record<string, unknown>];
      if (shape === 'map' && a) {
        const set: Record<string, unknown> = {};
        const unset: string[] = [];
        let n = 0;
        const keys = hint && a === (hint.prev as Record<string, unknown>)[f] ? (hint.changed[f] ?? []) : null;
        if (keys) {
          for (const k of keys) if (n++ < PATCH_MAX) (k in b ? (set[k] = b[k]) : unset.push(k));
        } else {
          for (const k in b) if (a[k] !== b[k] && n++ < PATCH_MAX) set[k] = b[k];
          for (const k in a) if (!(k in b) && n++ < PATCH_MAX) unset.push(k);
        }
        if (n < PATCH_MAX) {
          patches[f] = { set, unset };
          continue;
        }
      }
      fields[f] = JSON.stringify(b);
    }
    this.sentFields = { ...now };
    this.send({ type: 'edits', fields, patches, rects, epoch, battleRects, groundRects });
  }
  /** The edits' fields as last sent (by reference). */
  private sentFields: Record<string, unknown> | null = null;

  want(tiles: WantTile[]) {
    this.send({ type: 'want', tiles });
  }

  dispose() {
    this.worker.terminate();
  }

  private send(m: ToCoordinator) {
    this.worker.postMessage(m);
  }
}
