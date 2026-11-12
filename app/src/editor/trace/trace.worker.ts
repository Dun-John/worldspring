// Auto-trace off the main thread (`engine.ts`). Create it with
// `new Worker(new URL('./trace/trace.worker.ts', import.meta.url), { type: 'module' })`.
// Only the latest request waiting is answered (older ones queued behind a trace are skipped).
// The picture is kept between requests: send `pixels` (transferred) when it changes, under a
// new `image` key, and leave them out while only the picks or parameters change.
import { trace } from './engine';
import type { TraceRequest, TraceResult } from './types';

export interface TraceWorkerRequest {
  id: number;
  /** Names the picture; `pixels` may be left out when it is the one last sent. */
  image: string;
  req: Omit<TraceRequest, 'pixels'> & { pixels?: Uint8ClampedArray };
}

export type TraceWorkerReply = { id: number; ok: true; result: TraceResult } | { id: number; ok: false; error: string };

let cached: { image: string; pixels: Uint8ClampedArray } | null = null;
let pending: TraceWorkerRequest | null = null;

const post = (r: TraceWorkerReply) => (self as unknown as Worker).postMessage(r);

function run() {
  const m = pending;
  pending = null;
  if (!m) return;
  try {
    const pixels = m.req.pixels ?? (cached?.image === m.image ? cached.pixels : null);
    if (!pixels) throw new Error('trace: no picture sent for ' + m.image);
    post({ id: m.id, ok: true, result: trace({ ...m.req, pixels }) });
  } catch (err) {
    post({ id: m.id, ok: false, error: String(err) });
  }
}

self.onmessage = (e: MessageEvent<TraceWorkerRequest>) => {
  const m = e.data;
  if (m.req.pixels) cached = { image: m.image, pixels: m.req.pixels };
  const first = !pending;
  pending = m;
  // Let any newer request still queued replace this one first.
  if (first) setTimeout(run, 0);
};
