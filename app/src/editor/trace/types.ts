// A picture to trace a sketch from: held for the session only (never saved anywhere), drawn
// under the sketch and placed over the map by a transform. Shared by the underlay and the
// trace worker.
import type { Stroke } from '../../gen/protocol';

export interface TraceImage {
  bitmap: ImageBitmap;
  w: number;
  h: number;
}

/** Where the picture lies: its centre (world ft), its scale (ft per image pixel) and its turn
 * (degrees, clockwise, the screen's way with y down). */
export interface TraceXform {
  cx: number;
  cy: number;
  ftPerPx: number;
  rot: number;
  /** Down the picture, if not `ftPerPx` (a copy resized by a slightly different ratio each way). */
  ftPerPxY?: number;
}

/** What can be traced, each from a colour picked on the picture. */
export type TraceKind = 'water' | 'mountain' | 'forest' | 'river';
export const TRACE_KINDS: TraceKind[] = ['water', 'mountain', 'forest', 'river'];

export interface TraceParams {
  /** 0..1: how far from the picked colour a pixel may be and still count. */
  threshold: number;
  /** 0..1: how much the shapes are smoothed (before and after outlining). */
  smooth: number;
  /** Shapes smaller than this (image px²) are dropped: specks, lettering, tiny islands. */
  minSize: number;
}

export interface TraceRequest {
  pixels: Uint8ClampedArray;
  w: number;
  h: number;
  xform: TraceXform;
  picks: Partial<Record<TraceKind, [number, number, number]>>;
  params: Record<TraceKind, TraceParams>;
}

export interface TraceResult {
  strokes: Stroke[];
  /** The kind each stroke came from (parallel to `strokes`), for the preview's toggles. */
  kinds: TraceKind[];
  counts: Record<TraceKind, number>;
  /** Shapes left out past the stroke cap (the smallest). */
  dropped: number;
  ms: number;
}

/** An image pixel's place in the world. */
export function imageToWorld(w: number, h: number, xf: TraceXform, px: number, py: number): [number, number] {
  const a = (xf.rot * Math.PI) / 180;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const [dx, dy] = [(px - w / 2) * xf.ftPerPx, (py - h / 2) * (xf.ftPerPxY ?? xf.ftPerPx)];
  return [xf.cx + dx * c - dy * s, xf.cy + dx * s + dy * c];
}

/** A world point's place on the image (pixels; may be outside it). */
export function worldToImage(w: number, h: number, xf: TraceXform, x: number, y: number): [number, number] {
  const a = (xf.rot * Math.PI) / 180;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const [dx, dy] = [x - xf.cx, y - xf.cy];
  return [(dx * c + dy * s) / xf.ftPerPx + w / 2, (-dx * s + dy * c) / (xf.ftPerPxY ?? xf.ftPerPx) + h / 2];
}
