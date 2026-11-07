/**
 * v63 countryside: the battlemap's field and croft bounds and a windmill's sails (`town::rural`), as
 * vectors over the ground. Placeholders until the sprite batch (V3b) draws them in the cartoon
 * style: a hedgerow a green band, a dry-stone wall a grey one, a post-and-rail fence a thin brown
 * line with posts, sails a cross of four arms.
 */
import type { Graphics } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { PX } from './atlas';
import { INK, STONE, WOOD } from './bridges';

/** `ShapeKind` ids (`battlemap.rs`, v63 countryside). */
export const HEDGE = 14;
export const DRY_WALL = 15;
export const FENCE = 16;
export const SAILS = 17;

/** Whether a shape is one of these bounds (drawn with the walls, under the roofs). */
export const isBound = (kind: number) => kind === HEDGE || kind === DRY_WALL || kind === FENCE;

/** Hedgerows, dry-stone walls and fences: one straight stretch each (`pts` its ends, `size` thick). */
export function drawBounds(g: Graphics, shapes: Shape[]) {
  for (const s of shapes) {
    const [x0, y0, x1, y1] = s.pts;
    if (s.kind === HEDGE) {
      g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: s.size + 2, color: INK, cap: 'round' });
      g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: s.size, color: 0x4d6e2d, cap: 'round' });
    } else if (s.kind === DRY_WALL) {
      g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: s.size + 2, color: INK, cap: 'square' });
      g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: s.size, color: STONE[2], cap: 'square' });
    } else if (s.kind === FENCE) {
      g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: Math.max(2, s.size), color: WOOD[1] });
      const l = Math.hypot(x1 - x0, y1 - y0);
      const n = Math.max(1, Math.round(l / (PX * 1.6)));
      for (let k = 0; k <= n; k++) g.circle(x0 + ((x1 - x0) * k) / n, y0 + ((y1 - y0) * k) / n, 2.2).fill(WOOD[0]);
    }
  }
}

/** A windmill's sails (`pts`: the hub, then the angle × PX; `size` their reach): a cross of arms. */
export function drawSails(g: Graphics, s: Shape) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const a = s.pts[2] / PX;
  for (let k = 0; k < 4; k++) {
    const t = a + (k * Math.PI) / 2;
    g.moveTo(cx, cy).lineTo(cx + Math.cos(t) * s.size, cy + Math.sin(t) * s.size).stroke({ width: PX * 0.5, color: INK });
    g.moveTo(cx, cy).lineTo(cx + Math.cos(t) * s.size, cy + Math.sin(t) * s.size).stroke({ width: PX * 0.3, color: 0xe4dccb });
  }
  g.circle(cx, cy, PX * 0.35).fill(WOOD[0]).stroke({ width: 1.5, color: INK });
}

/** Tactical info for a bound under a point (chunk px), if one is within half a square of it. */
export function boundAt(shapes: Shape[], x: number, y: number): { name: string; cover: number; notes: string } | null {
  for (const s of shapes) {
    if (!isBound(s.kind)) continue;
    const [x0, y0, x1, y1] = s.pts;
    const [dx, dy] = [x1 - x0, y1 - y0];
    const l2 = dx * dx + dy * dy || 1;
    const t = Math.max(0, Math.min(1, ((x - x0) * dx + (y - y0) * dy) / l2));
    if (Math.hypot(x - x0 - t * dx, y - y0 - t * dy) > PX * 0.5 + s.size / 2) continue;
    const name = s.kind === HEDGE ? 'hedgerow' : s.kind === DRY_WALL ? 'dry-stone wall' : 'fence';
    return { name, cover: s.kind === FENCE ? 0 : 1, notes: 'difficult terrain · about 4 ft, seen over' };
  }
  return null;
}
