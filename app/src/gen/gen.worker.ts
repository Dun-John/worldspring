// Generator worker: owns one WASM instance with the world + T0 context and runs jobs.
// Stateless apart from that context; every job carries its dependency data.
import { chunkTransfer, prepareChunk, type KindTactics } from './battlePrep';
import init, { Ctx, battlemap_catalog_json, building_funcs_json, under_catalog_json } from './pkg/worldgen_wasm.js';
import { KIND_BATTLEMAP, type FromGen, type ToGen } from './protocol';

/** Object heights by kind (orders a chunk's objects for drawing). */
let kinds: KindTactics[] | null = null;

let ctx: Ctx | null = null;
// One load of the WASM module, however many messages ask for it (warm-up, then init).
let wasm: ReturnType<typeof init> | null = null;
const load = () => (wasm ??= init());

const post = (msg: FromGen, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(msg, transfer);

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
    } else if (m.type === 'ask') {
      if (!ctx) throw new Error('worker not initialized');
      const [a, c] = [m.ask, ctx];
      // A question that fails is answered with null, so nothing waits on it for ever.
      const answer = (): string =>
        a.op === 'query'
          ? c.query_json(a.x, a.y)
          : a.op === 'search'
            ? c.search_json(a.q, a.rect ? new Float64Array(a.rect) : undefined)
            : a.op === 'inview'
              ? c.in_view_json(...a.rect)
              : a.op === 'interior'
                ? c.interior_json(a.id)
                : a.op === 'place'
                  ? c.place_json(a.id)
                  : a.op === 'names'
                    ? c.names_json(a.scope)
                    : a.op === 'spot'
                      ? c.creation_spot_json(a.kind, a.under, a.id, a.x, a.y)
                      : a.op === 'building'
                        ? c.building_spot_json(JSON.stringify(a.poly), a.func, a.id)
                        : a.op === 'funcs'
                          ? building_funcs_json()
                          : a.op === 'bedit'
                          ? c.building_edit_json(a.id, a.change)
                          : a.op === 'bin'
                          ? c.buildings_in_json(JSON.stringify(a.poly))
                          : a.op === 'works'
                          ? c.works_spot_json(JSON.stringify(a.site))
                          : a.op === 'design'
                            ? c.design_json(a.id, a.design, a.action)
                            : a.op === 'undercat'
                              ? under_catalog_json()
                              : c.districts_json(a.settlement);
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
