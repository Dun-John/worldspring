import type { Graphics } from 'pixi.js';
import type { Shape } from '../../gen/battlePrep';
import { rngAt, FT, LX, LY, mix, INK } from '../bridges';
import { PX } from '../scale';
import { sprites } from '../spriteCode';
import { BATTER, GATE, GATE_ARCHED, GATE_BENT, GATE_SQUARE, MAT_EARTH, MAT_PALISADE, OUT, PARAPET, SHAPE_D, SHAPE_OCTAGON, SHAPE_SQUARE, SQUARE, flagsOf, gateForm, lookOf, pairs, towerShape } from './base';
import { inset, shapedOutline, squareFrame } from './towers';
import { blockFrame } from './gates';

/** A dry ditch's sides (ft down from a bank's top to the floor) and its outer bank's lip (ft wide). */
const DITCH_SIDE = 3.2;
const DITCH_LIP = 3;
/** Earth on a ditch's side turned from the light, toward it, and on its outer bank's lip. */
const CUT_DARK = 0x5a4430;
const CUT_LIT = 0xb09468;
const CUT_LIP = 0xd9c79c;
/**
 * A dry ditch's bank along its top (`ShapeKind::DitchBank`: the two ends of a stretch, the way
 * down into the ditch, whether its outer bank lies behind): its side a band of earth down to the
 * floor, dark where it is turned from the light (NW) and light toward it, the outer bank's lip
 * pale behind it (`ink` false); the top inked by hand, a little wavering (`ink`: over every
 * stretch's earth).
 */
export function drawDitchBank(g: Graphics, s: Shape, ink: boolean) {
  const [ax, ay, bx, by] = s.pts;
  const L = Math.hypot(bx - ax, by - ay);
  if (!(L > 0)) return;
  const rnd = rngAt(ax + bx, ay + by);
  const [nx, ny] = [-(by - ay) / L, (bx - ax) / L];
  const dl = Math.hypot(s.pts[4] ?? 0, s.pts[5] ?? 0);
  if (!ink) {
    if (!(dl > 0)) return;
    const [dx, dy] = [s.pts[4] / dl, s.pts[5] / dl];
    // (Overlapping a little at the ends: a stretch's band meets the next's without a seam.)
    const [ux, uy] = [((bx - ax) / L) * FT * 0.4, ((by - ay) / L) * FT * 0.4];
    const band = (from: number, to: number, color: number, alpha: number) => {
      const [f, t] = [from * FT, to * FT];
      g.poly([ax - ux + dx * f, ay - uy + dy * f, bx + ux + dx * f, by + uy + dy * f, bx + ux + dx * t, by + uy + dy * t, ax - ux + dx * t, ay - uy + dy * t]).fill({ color, alpha });
    };
    const lit = 0.5 + 0.5 * (dx * LX + dy * LY);
    const side = mix(CUT_DARK, CUT_LIT, lit);
    band(0, DITCH_SIDE * 0.7, side, 1);
    band(DITCH_SIDE * 0.7, DITCH_SIDE, side, 0.5);
    // (Its foot: a dark line where the side meets the floor, under the lit side only faint.)
    band(DITCH_SIDE * 0.62, DITCH_SIDE * 0.7, CUT_DARK, 0.5 - 0.35 * lit);
    if ((s.pts[6] ?? 0) > 0) band(-DITCH_LIP, 0, CUT_LIP, 0.45);
    return;
  }
  const n = Math.max(1, Math.round(L / (FT * 3)));
  g.moveTo(ax, ay);
  for (let k = 1; k < n; k++) {
    const t = k / n;
    const w = (rnd() - 0.5) * FT * 0.3;
    g.lineTo(ax + (bx - ax) * t + nx * w, ay + (by - ay) * t + ny * w);
  }
  g.lineTo(bx, by).stroke({ width: OUT, color: INK, cap: 'round', join: 'round' });
}

/** A gravel walk on a court's lawn (`ShapeKind::GravelWalk`: its rectangle's corners), drawn as a
 * building's court draws it (`gravelPatch`), along its longer side. */
export function drawGravelWalk(g: Graphics, s: Shape) {
  const q = pairs(s.pts.slice(0, 8));
  if (q.length < 4) return;
  const [e1, e3] = [Math.hypot(q[1][0] - q[0][0], q[1][1] - q[0][1]), Math.hypot(q[3][0] - q[0][0], q[3][1] - q[0][1])];
  if (!(e1 > 0 && e3 > 0)) return;
  const [o, a, b] = e1 >= e3 ? [q[0], q[1], q[3]] : [q[0], q[3], q[1]];
  const [la, lb] = [Math.max(e1, e3), Math.min(e1, e3)];
  const [ux, uy, vx, vy] = [(a[0] - o[0]) / la, (a[1] - o[1]) / la, (b[0] - o[0]) / lb, (b[1] - o[1]) / lb];
  const at = (u: number, t: number): [number, number] => [o[0] + (ux * u + vx * t) * PX, o[1] + (uy * u + vy * t) * PX];
  sprites().gravelPatch(g, at, la / PX, lb / PX, PX, rngAt((q[0][0] + q[2][0]) / 2, (q[0][1] + q[2][1]) / 2), [LX, LY]);
}

/** The 5-ft grid over a wall's walk or a tower's top (shown with the ground's): lines on the square
 * boundaries, clipped to the walk (a quad) or the top (a round); over a court's gravel walk too, as
 * faint as the ground's own; over a bastion's earth top. None over a forecourt's low wall, piers or gate. */
export function topGrid(g: Graphics, s: Shape) {
  if (s.kind === 20) {
    // A bastion's top, as two halves either side of its middle line (each convex): f1, g1, tip, v and v, tip, g2, f2.
    const p = s.pts;
    gridIn(g, [p[0], p[1], p[2], p[3], p[4], p[5], p[10], p[11]]);
    gridIn(g, [p[10], p[11], p[4], p[5], p[6], p[7], p[8], p[9]]);
    g.stroke({ width: 1, color: INK, alpha: 0.32, pixelLine: true });
    return;
  }
  if (s.kind === 13) {
    if (s.pts.length < 8) return;
    gridIn(g, s.pts.slice(0, 8));
    g.stroke({ width: 1, color: INK, alpha: 0.28, pixelLine: true });
    return;
  }
  if (s.kind === 9) {
    // A one-block gatehouse's top, inside its parapet.
    const form = gateForm(s);
    if (form !== GATE_SQUARE && form !== GATE_ARCHED && form !== GATE_BENT) return;
    gridIn(g, inset(blockFrame(s, form).solid, PARAPET));
    g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
    return;
  }
  // (A square or arched gate tower's half: its gatehouse's top has the grid.)
  if (s.kind === 1 && flagsOf(s, 2) & GATE && (gateForm(s) === GATE_SQUARE || gateForm(s) === GATE_ARCHED)) return;
  const shape = s.kind === 1 && !(flagsOf(s, 2) & SQUARE) ? towerShape(s) : -1;
  if (shape === SHAPE_D || shape === SHAPE_OCTAGON) {
    const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
    gridIn(g, inset(shapedOutline(s.pts[0], s.pts[1], s.size, Math.cos(t), Math.sin(t), shape), PARAPET));
  } else if (s.kind === 1 && (flagsOf(s, 2) & SQUARE || towerShape(s) === SHAPE_SQUARE)) {
    // A square tower's top: its square's lines, clipped to it.
    const { x, y, h, ux, uy } = squareFrame(s);
    const f = h - PARAPET;
    const q = [-1, -1, 1, -1, 1, 1, -1, 1].map((v) => v * f);
    const pts: number[] = [];
    for (let k = 0; k < 4; k++) pts.push(x + ux * q[k * 2] - uy * q[k * 2 + 1], y + uy * q[k * 2] + ux * q[k * 2 + 1]);
    gridIn(g, pts);
  } else if (s.kind === 1) {
    const [cx, cy] = [s.pts[0], s.pts[1]];
    const r = s.size - PARAPET;
    for (const axis of [0, 1]) {
      const c = axis === 0 ? cx : cy;
      for (let k = Math.ceil((c - r) / PX); k * PX <= c + r; k++) {
        const h = Math.sqrt(Math.max(0, r * r - (k * PX - c) ** 2));
        if (h <= 0) continue;
        if (axis === 0) g.moveTo(k * PX, cy - h).lineTo(k * PX, cy + h);
        else g.moveTo(cx - h, k * PX).lineTo(cx + h, k * PX);
      }
    }
  } else if (s.kind === 0) {
    const [x0, y0, x1, y1] = s.pts;
    const len = Math.hypot(x1 - x0, y1 - y0) || 1;
    // (No grid on a palisade: no walk to stand on; rammed earth's walk inside its battered faces.)
    const mat = lookOf(s, 6).material;
    if (mat === MAT_PALISADE) return;
    const w = s.size / 2 - PARAPET - (mat === MAT_EARTH ? FT * BATTER : 0);
    const [vx, vy] = [(-(y1 - y0) / len) * w, ((x1 - x0) / len) * w];
    gridIn(g, [x0 - vx, y0 - vy, x1 - vx, y1 - vy, x1 + vx, y1 + vy, x0 + vx, y0 + vy]);
  } else return;
  g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
}

/** The 5-ft grid's lines where they cross a convex outline. */
function gridIn(g: Graphics, q: number[]) {
  const n = q.length / 2;
  // Where the line `axis` = `at` crosses the walk's outline, as the span along the other axis.
  for (const axis of [0, 1]) {
    const vals = Array.from({ length: n }, (_, i) => q[i * 2 + axis]);
    for (let k = Math.ceil(Math.min(...vals) / PX); k * PX <= Math.max(...vals); k++) {
      let [lo, hi] = [Infinity, -Infinity];
      for (let i = 0; i < n; i++) {
        const [a, b] = [i * 2, ((i + 1) % n) * 2];
        const [p, r] = [q[a + axis], q[b + axis]];
        if ((p - k * PX) * (r - k * PX) > 0 || p === r) continue;
        const o = q[a + 1 - axis] + ((k * PX - p) / (r - p)) * (q[b + 1 - axis] - q[a + 1 - axis]);
        [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
      }
      if (!(hi > lo)) continue;
      if (axis === 0) g.moveTo(k * PX, lo).lineTo(k * PX, hi);
      else g.moveTo(lo, k * PX).lineTo(hi, k * PX);
    }
  }
}
