// Generator worker: owns one WASM instance with the world + T0 context and runs jobs.
// Stateless apart from that context; every job carries its dependency data.
import { chunkTransfer, prepareChunk, type KindTactics } from './battlePrep';
import init, { Ctx, battlemap_catalog_json, building_funcs_json, under_catalog_json } from './pkg/worldgen_wasm.js';
import { KIND_BATTLEMAP, type FromGen, type ToGen } from './protocol';
import type { Ask } from './protocol/ask';

/** Object heights by kind (orders a chunk's objects for drawing). */
let kinds: KindTactics[] | null = null;

let ctx: Ctx | null = null;
// One load of the WASM module, however many messages ask for it (warm-up, then init).
let wasm: ReturnType<typeof init> | null = null;
const load = () => (wasm ??= init());

const post = (msg: FromGen, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(msg, transfer);

type AskOf<K extends Ask['op']> = Extract<Ask, { op: K }>;
/** One handler per question, each exactly its call into the WASM context (a missing op fails to compile). */
const ASK: { [K in Ask['op']]: (a: AskOf<K>, c: Ctx) => string } = {
  query: (a, c) => c.query_json(a.x, a.y),
  pick: (a, c) => c.pick_json(a.x, a.y, a.radius),
  search: (a, c) => c.search_json(a.q, a.rect ? new Float64Array(a.rect) : undefined),
  inview: (a, c) => c.in_view_json(...a.rect),
  interior: (a, c) => c.interior_json(a.id),
  place: (a, c) => c.place_json(a.id),
  names: (a, c) => c.names_json(a.scope),
  parents: (_a, c) => c.parents_json(),
  members: (a, c) => c.members_json(a.town, a.add, a.remove),
  reach: (a, c) => c.reach_json(a.ids, a.within),
  spot: (a, c) => c.creation_spot_json(a.kind, a.under, a.id, a.x, a.y),
  bridgelots: (a, c) => c.bridge_lots_json(a.input),
  islets: (_a, c) => c.islets_json(),
  islet: (a, c) => c.islet_json(a.key, a.x, a.y, a.radius, a.rise),
  wharfcheck: (a, c) => c.wharf_check_json(a.wharf, a.lighthouse),
  building: (a, c) => c.building_spot_json(JSON.stringify(a.poly), a.func, a.id),
  stamp: (a, c) => c.stamp_json(a.input),
  row: (a, c) => c.row_json(a.input),
  parcel: (a, c) => c.parcel_check_json(a.input),
  funcs: () => building_funcs_json(),
  bedit: (a, c) => c.building_edit_json(a.id, a.change),
  bedits: (a, c) => JSON.stringify(a.ids.map((id) => JSON.parse(c.building_edit_json(id, a.change)))),
  bin: (a, c) => c.buildings_in_json(JSON.stringify(a.poly)),
  works: (a, c) => c.works_spot_json(JSON.stringify(a.site)),
  design: (a, c) => c.design_json(a.id, a.design, a.action),
  undercat: () => under_catalog_json(),
  townplan: (a, c) => c.town_plan_json(a.layout),
  townchange: (a, c) => c.town_change_json(a.layout, a.request),
  streetplan: (a, c) => c.street_plan_json(a.layout),
  streetchange: (a, c) => c.street_change_json(a.layout, a.request),
  named: (_a, c) => c.named_json(),
  under: (a, c) => c.under_search_json(a.q, a.filters),
  made: (_a, c) => c.made_json(),
  settlefacts: (a, c) => c.settlement_facts_json(a.id),
  roadcut: (a, c) => c.road_cut_json(a.input),
  removedroads: (_a, c) => c.removed_roads_json(),
  fencechange: (a, c) => c.fence_change_json(a.input),
  fencesnear: (a, c) => c.fences_near_json(a.x, a.y, a.r, a.limit),
  changedfences: (_a, c) => c.changed_fences_json(),
  bridgemarks: (_a, c) => c.bridge_marks_json(),
  madespot: (a, c) => c.settlement_spot_json(a.kind, a.id, a.x, a.y),
  setaside: (a, c) => c.set_aside_changed_json(a.old, a.edits),
  setculture: (a, c) => c.set_culture_json(a.layout, a.culture, a.edits),
  setplanning: (a, c) => c.set_planning_json(a.layout, a.planning, a.auto, a.edits),
  restoremade: (a, c) => c.restore_made_json(a.id, a.edits),
  remakeprints: (a, c) => c.remake_prints_json(a.edits, new Uint32Array(a.played)),
  follow: (a, c) => c.follow_json(a.input),
  districts: (a, c) => c.districts_json(a.settlement),
};

self.onmessage = async (e: MessageEvent<ToGen>) => {
  const m = e.data;
  try {
    if (m.type === 'warm') {
      await load();
    } else if (m.type === 'init') {
      await load();
      ctx?.free();
      ctx = new Ctx(m.worldJson);
      const start = performance.now();
      let t0: ArrayBuffer | null = null;
      let overlay: string | null = null;
      if (m.t0) {
        ctx.load_t0(new Uint8Array(m.t0));
      } else {
        let lastPost = 0;
        const bytes = ctx.gen_t0((stage: string, frac: number) => {
          const now = performance.now();
          if (now - lastPost > 50 || frac >= 1) {
            lastPost = now;
            post({ type: 'progress', stage, frac });
          }
        });
        t0 = bytes.buffer as ArrayBuffer;
        overlay = ctx.overlay_json();
      }
      post({ type: 'inited', t0, overlay, ms: performance.now() - start }, t0 ? [t0] : []);
    } else if (m.type === 'edits') {
      if (m.json) ctx?.set_edit_fields(m.json);
      if (m.patch) ctx?.patch_edits(m.patch);
    } else if (m.type === 'places') {
      ctx?.set_place_names(m.json);
    } else if (m.type === 'ask') {
      if (!ctx) throw new Error('worker not initialized');
      const [a, c] = [m.ask, ctx];
      // A question that fails is answered with null, so nothing waits on it for ever.
      const answer = (): string => ASK[a.op](a as never, c);
      let json = 'null';
      try {
        json = answer();
      } catch (err) {
        console.error(`[gen] ${a.op} failed:`, err);
      }
      post({ type: 'answer', id: m.id, json });
    } else if (m.type === 'job') {
      if (!ctx) throw new Error('worker not initialized');
      const start = performance.now();
      if (m.kind === KIND_BATTLEMAP) {
        // Decoded and tessellated here, off the main thread.
        kinds ??= JSON.parse(battlemap_catalog_json()) as KindTactics[];
        const chunk = prepareChunk(ctx.gen_battlemap(m.level, m.x, m.y, m.parent!).buffer as ArrayBuffer, m.x, m.y, kinds);
        post({ type: 'done', id: m.id, buf: null, chunk, ms: performance.now() - start }, chunkTransfer(chunk));
      } else {
        const buf = ctx.gen_terrain(m.level, m.x, m.y, m.parent ?? undefined).buffer as ArrayBuffer;
        post({ type: 'done', id: m.id, buf, ms: performance.now() - start }, [buf]);
      }
    }
  } catch (err) {
    post({ type: 'error', id: m.type === 'job' ? m.id : undefined, message: String(err) });
  }
};
