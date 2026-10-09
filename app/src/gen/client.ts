// Main-thread handle to the coordinator worker.
import type { PreparedChunk } from './battlePrep';
import { EDIT_FIELDS } from '../sync/ops';

/** Most entries of a field sent as a patch; beyond, the whole field. */
const PATCH_MAX = 2000;
import type { Ask, BuildingEdit, BuildingFuncs, Created, DesignReply, EditsPatch, DistrictLabel, Edits, Interior, SiteDesign, UnderCatalog, FromCoordinator, NameEntry, GenStats, Geom, Hit, Overlay, PlaceInfo, Rect, TileMsg, ToCoordinator, TownEdit, TownPlan, TownReport, TownRequest, WantTile, WorldFile } from './protocol';

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
    this.sentFields = null;
    const cores = navigator.hardwareConcurrency || 4;
    const workers = Math.max(1, Math.min(6, cores - 2));
    return new Promise((resolve) => {
      this.readyResolve = resolve;
      this.send({ type: 'init', world, workers });
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
    return this.ask({ op: 'inview', rect }).then((j) => JSON.parse(j) as Extract<Hit, { kind: 'building' | 'district' }>[]);
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

  /** Where a site of `kind` can be created near (x, y), and a name for it; null if the
   * generator could not answer. */
  spot(kind: string, under: string | undefined, id: string, x: number, y: number): Promise<{ x: number; y: number; name: string } | { error: string } | null> {
    return this.ask({ op: 'spot', kind, under, id, x, y }).then((j) => JSON.parse(j));
  }

  /** Whether a building drawn by hand can stand there (`id`: its created id), its point and a
   * name for it; null if the generator could not answer. */
  buildingSpot(poly: [number, number][], func: string | undefined, id: string): Promise<{ x: number; y: number; name: string } | { error: string } | null> {
    return this.ask({ op: 'building', poly, func, id }).then((j) => JSON.parse(j));
  }

  /** A generated building's edit with `change` made (`{func, floors, poly, roof, tint, structure}`;
   * an empty string or `auto` goes back to as generated), or `{remove: true}`: the edit (null: as
   * generated) or why not; null if the generator could not answer. */
  buildingEdit(id: string, change: object): Promise<{ edit: BuildingEdit | null } | { error: string } | null> {
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
    return this.ask({ op: 'districts', settlement }).then((j) => JSON.parse(j) as DistrictLabel[]);
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
  setEdits(edits: Edits, rects: Rect[], epoch: number, battleRects: Rect[] = [], hint?: { prev: Edits; changed: Record<string, string[]> }) {
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
    this.send({ type: 'edits', fields, patches, rects, epoch, battleRects });
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
