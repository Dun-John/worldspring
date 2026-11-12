// Auto-trace's session state: the picks, the controls, the worker and the preview result.
// Memory only, like the picture itself: never saved, shared or sent to mapd. The preview is
// drawn by SketchLayer; Accept hands the shown strokes to the sketcher (one undo step).
import type { Stroke } from '../../gen/protocol';
import type { TraceWorkerReply, TraceWorkerRequest } from './trace.worker';
import { TRACE_KINDS, worldToImage, type TraceKind, type TraceParams, type TraceResult } from './types';
import { underlay } from './underlay.svelte';

/** The long side of the copy the tracer works on (the preview's speed). */
const MAX_SIDE = 1536;

/** Defaults that trace a clean map without touching them (minSize in px² of the copy; high enough to drop lettering). */
const DEFAULTS: Record<TraceKind, TraceParams> = {
  water: { threshold: 0.35, smooth: 0.4, minSize: 1200 },
  mountain: { threshold: 0.35, smooth: 0.4, minSize: 800 },
  forest: { threshold: 0.35, smooth: 0.4, minSize: 800 },
  river: { threshold: 0.35, smooth: 0.3, minSize: 60 },
};

export const KIND_LABEL: Record<TraceKind, string> = { water: 'Water', mountain: 'Mountains', forest: 'Forest', river: 'Rivers' };

class Tracer {
  picks = $state<Partial<Record<TraceKind, [number, number, number]>>>({});
  params = $state<Record<TraceKind, TraceParams>>(structuredClone(DEFAULTS));
  show = $state<Record<TraceKind, boolean>>({ water: true, mountain: true, forest: true, river: true });
  biome = $state('temperate_forest');
  /** The kind whose colour the next click on the picture picks. */
  picking = $state<TraceKind | null>(null);
  busy = $state(false);
  error = $state('');
  result = $state.raw<TraceResult | null>(null);
  /** Bumped when the result, its toggles or the biome change (the layer redraws). */
  version = $state(0);

  private copy: { key: string; bitmap: ImageBitmap; pixels: Uint8ClampedArray; w: number; h: number; sent: boolean } | null = null;
  private worker: Worker | null = null;
  private seq = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private imgN = 0;

  /** The downsampled copy of the picture (made once per picture). */
  private ensureCopy() {
    const im = underlay.image;
    if (!im) return (this.copy = null);
    if (this.copy?.bitmap === im.bitmap) return this.copy;
    const k = Math.min(1, MAX_SIDE / Math.max(im.w, im.h));
    const [w, h] = [Math.max(1, Math.round(im.w * k)), Math.max(1, Math.round(im.h * k))];
    const cv = new OffscreenCanvas(w, h);
    const cx = cv.getContext('2d', { willReadFrequently: true })!;
    cx.imageSmoothingQuality = 'high';
    cx.drawImage(im.bitmap, 0, 0, w, h);
    const pixels = cx.getImageData(0, 0, w, h).data;
    this.copy = { key: 'img' + ++this.imgN, bitmap: im.bitmap, pixels, w, h, sent: false };
    return this.copy;
  }

  /** A click on the picture at world (x, y) while picking: take its colour. */
  pickAt(x: number, y: number): boolean {
    const kind = this.picking;
    const im = underlay.image;
    if (!kind || !im) return false;
    const [px, py] = worldToImage(im.w, im.h, underlay.xf, x, y);
    if (px < 0 || py < 0 || px >= im.w || py >= im.h) return true;
    const c = this.ensureCopy()!;
    const i = (Math.min(c.h - 1, Math.floor((py / im.h) * c.h)) * c.w + Math.min(c.w - 1, Math.floor((px / im.w) * c.w))) * 4;
    if (c.pixels[i + 3] < 128) return true;
    this.picks[kind] = [c.pixels[i], c.pixels[i + 1], c.pixels[i + 2]];
    this.picking = null;
    return true;
  }

  clearPick(kind: TraceKind) {
    delete this.picks[kind];
    if (this.picking === kind) this.picking = null;
  }

  reset(kind: TraceKind) {
    this.params[kind] = { ...DEFAULTS[kind] };
  }

  /** The picture went: forget its picks and result. */
  dropPicture() {
    this.cancel();
    this.copy = null;
    this.picks = {};
    this.picking = null;
    this.result = null;
    this.error = '';
    this.version++;
  }

  /** Re-trace soon (debounced): call when a pick, a control or the picture's place changes. */
  schedule() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    if (!underlay.image || !TRACE_KINDS.some((k) => this.picks[k])) {
      if (this.result) {
        this.result = null;
        this.version++;
      }
      this.busy = false;
      return;
    }
    this.busy = true;
    this.timer = setTimeout(() => this.run(), 250);
  }

  private run() {
    this.timer = null;
    const c = this.ensureCopy();
    const im = underlay.image;
    if (!c || !im) return;
    this.worker ??= new Worker(new URL('./trace.worker.ts', import.meta.url), { type: 'module' });
    const w = this.worker;
    w.onmessage = (e: MessageEvent<TraceWorkerReply>) => {
      const r = e.data;
      if (r.id !== this.seq) return;
      this.busy = false;
      if (r.ok) {
        this.result = r.result;
        this.error = '';
      } else this.error = r.error;
      this.version++;
    };
    const id = ++this.seq;
    const xf = { ...underlay.xf, ftPerPx: underlay.xf.ftPerPx * (im.w / c.w) };
    const pixels = c.sent ? undefined : c.pixels.slice();
    c.sent = true;
    const req: TraceWorkerRequest = { id, image: c.key, req: { pixels, w: c.w, h: c.h, xform: xf, picks: $state.snapshot(this.picks), params: $state.snapshot(this.params) } };
    w.postMessage(req, pixels ? [pixels.buffer] : []);
  }

  private cancel() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.seq++;
    this.busy = false;
    this.worker?.terminate();
    this.worker = null;
    if (this.copy) this.copy.sent = false;
  }

  /** The strokes the shown kinds make (forests in the chosen biome). */
  shownStrokes(): Stroke[] {
    const r = this.result;
    if (!r) return [];
    const out: Stroke[] = [];
    r.strokes.forEach((s, i) => {
      const k = r.kinds[i];
      if (!this.show[k]) return;
      out.push(k === 'forest' && s.tool === 'biome' ? { ...s, biome: this.biome } : s);
    });
    return out;
  }

  /** What the result holds, shown kinds only: "2 coasts, 1 lake, 1 range". */
  get summary(): string {
    const r = this.result;
    if (!r) return '';
    const n: Record<string, number> = {};
    r.strokes.forEach((s, i) => {
      if (this.show[r.kinds[i]]) n[s.tool] = (n[s.tool] ?? 0) + 1;
    });
    const NAMES: [string, string, string][] = [['land', 'coast', 'coasts'], ['lake', 'lake', 'lakes'], ['range', 'range', 'ranges'], ['massif', 'massif', 'massifs'], ['biome', 'forest', 'forests'], ['river', 'river', 'rivers']];
    const parts = NAMES.filter(([t]) => n[t]).map(([t, one, many]) => `${n[t]} ${n[t] === 1 ? one : many}`);
    return parts.length ? parts.join(', ') : 'nothing found';
  }

  /** Drop the preview (the picks and the picture stay). */
  discard() {
    this.cancel();
    this.result = null;
    this.version++;
  }
}

export const tracer = new Tracer();
