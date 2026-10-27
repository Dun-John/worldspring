// Curtain walls, wall towers and gatehouses on the battlemap, drawn in code like the atlas and the
// bridges (assets/STYLE_GUIDE.md): warm stone in a few flat tones lit from the north-west, bold
// ink, blocks a little out of true. A wall is a flagged walk between two crenellated parapets;
// a tower a flagged fighting top inside its merlons with the spiral stair's hatch; a gate the
// passage between its two towers under an arch at each end, portcullis grooves and the doors
// swung back. A ruin's works lose merlons, crack and moss over, spill rubble where they broke.
// A palace's forecourt has a low coped garden wall instead, its gate between two solid piers with
// ball finials, the leaves (wrought iron or timber) standing open; its court's walks are gravel.
import type { Graphics } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { PX } from './atlas';
import { FT, INK, LX, LY, mix, MOSS, PLUM, rngAt, STONE, WOOD } from './bridges';
import { gravelPatch } from './furniture';

/** The walk and a tower's top: flags between the stone's mid and light tones. */
export const WALK = 0xb0a692;
/** Iron: dark, mid, light. */
const IRON = [0x4a4e56, 0x6e7278, 0x9aa0a6];
/** A parapet's width (px): merlons stand on it. */
export const PARAPET = FT * 1.6;

/** Flags on a wall's or tower's last point (`battlemap::WALL_*`). */
const BARE_A = 1;
const BARE_B = 2;
const GATE = 1;
const RUINED = 4;
/** A square tower (a castle's): its sides run at the angle after the flags. */
const SQUARE = 8;

const flagsOf = (s: Shape, at: number) => (s.pts.length > at ? Math.round(s.pts[at] / PX) : 0);
/** How much a direction faces the light, -1 .. 1. */
const lit = (x: number, y: number) => (x * LX + y * LY) / (Math.hypot(x, y) || 1);

/** Stone blocks (outlines) with a drop shadow: the block's sides in a shaded tone, its top in
 * `tone` set toward the light, a bold ink outline. */
function blocks(g: Graphics, quads: number[][], tone: () => number, shadow = FT * 0.35) {
  for (const b of quads) g.poly(b.map((v) => v + shadow));
  if (quads.length) g.fill({ color: 0x000000, alpha: 0.25 });
  for (const b of quads) {
    const t = tone();
    const n = b.length / 2;
    let [x, y] = [0, 0];
    for (let k = 0; k < n; k++) [x, y] = [x + b[k * 2] / n, y + b[k * 2 + 1] / n];
    g.poly(b).fill(mix(t, PLUM, 0.3));
    g.poly(b.map((v, k) => (k % 2 ? y + (v - y) * 0.72 + LY * FT * 0.18 : x + (v - x) * 0.72 + LX * FT * 0.18))).fill(t);
    g.poly(b).stroke({ width: 1.8, color: INK, join: 'round' });
  }
}

/** Loose rubble round (x, y): `n` blocks of mixed sizes within `spread` px. */
function rubble(x: number, y: number, n: number, spread: number, rnd: () => number): number[][] {
  const out: number[][] = [];
  for (let k = 0; k < n; k++) {
    const [cx, cy] = [x + (rnd() - 0.5) * 2 * spread, y + (rnd() - 0.5) * 2 * spread];
    const r = FT * (0.55 + rnd() * 0.75);
    const t = rnd() * Math.PI;
    const pts: number[] = [];
    for (let m = 0; m < 5; m++) {
      const a = t + (m / 5) * Math.PI * 2;
      const rr = r * (0.75 + rnd() * 0.4);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    out.push(pts);
  }
  return out;
}

const stoneTone = (rnd: () => number) => () => (rnd() < 0.35 ? STONE[2] : STONE[1]);

/** A point `a` along a straight run of parapet and `d` across it. */
type At = (a: number, d: number) => [number, number];

/** The crenels notched through a straight run of parapet from `a0` to `a1` (`d0` its walk side,
 * `d1` its outer face): notches about 1.4-1.7 ft between merlons about 2.4-3 ft, a little out of
 * rhythm; a ruin's wider here and there, a stone fallen outside. Pushed to `crenels`. */
function crenelRun(at: At, a0: number, a1: number, d0: number, d1: number, rnd: () => number, crenels: number[][], ruined = false, fallen: number[][] = []) {
  const side = Math.sign(d1 - d0);
  for (let a = a0 + FT * (1.2 + rnd() * 1.2); a < a1 - FT * 1.6; ) {
    let c = FT * (1.4 + rnd() * 0.3);
    if (ruined && rnd() < 0.35) {
      c += FT * (2.5 + rnd() * 2);
      if (rnd() < 0.5) fallen.push(...rubble(...at(a + c / 2, d1 + side * FT * 1.4), 1, FT * 0.8, rnd));
    }
    const b = Math.min(a + c, a1 - FT * 0.4);
    crenels.push([...at(a, d0 + side * 1.5), ...at(b, d0 + side * 1.5), ...at(b, d1 - side * 1.5), ...at(a, d1 - side * 1.5)]);
    a = b + FT * (2.4 + rnd() * 0.6);
  }
}

/** Crenels: dark notches through the coping, inked. */
function crenelFill(g: Graphics, crenels: number[][]) {
  for (const q of crenels) g.poly(q);
  if (crenels.length) g.fill(mix(STONE[0], PLUM, 0.3)).stroke({ width: 1.2, color: INK, alpha: 0.8, join: 'round' });
}

/** A convex outline's side as a parapet run: `at` from its first corner (`d` inward), its length,
 * how far a line `d` in starts (`k0 * d`) and ends short (`k1 * d`) of its corners, whether it
 * faces the light. */
interface Side {
  at: At;
  len: number;
  k0: number;
  k1: number;
  toLight: boolean;
}

/** The sides of a convex outline (either turn); those in `open` (by index) carry no parapet, and
 * their neighbours' parapets run square to the outline there. */
function sidesOf(pts: number[], open: number[] = []): Side[] {
  const m = pts.length / 2;
  let area = 0;
  for (let i = 0; i < m; i++) {
    const j = (i + 1) % m;
    area += pts[i * 2] * pts[j * 2 + 1] - pts[j * 2] * pts[i * 2 + 1];
  }
  const turn = area < 0 ? -1 : 1;
  const raw = Array.from({ length: m }, (_, i) => {
    const [ax, ay] = [pts[i * 2], pts[i * 2 + 1]];
    const [bx, by] = [pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const len = Math.hypot(bx - ax, by - ay);
    const [ux, uy] = len > 1e-6 ? [(bx - ax) / len, (by - ay) / len] : [1, 0];
    const [nx, ny] = [-uy * turn, ux * turn];
    return { i, ax, ay, ux, uy, nx, ny, len };
  }).filter((e) => e.len > 1e-6);
  // tan of half the turn at the corner between two sides: how far a line `d` in starts from it.
  const half = (p: (typeof raw)[0], q: (typeof raw)[0]) => (turn * (p.ux * q.uy - p.uy * q.ux)) / Math.max(1e-3, 1 + p.ux * q.ux + p.uy * q.uy);
  return raw.flatMap((e, k) => {
    if (open.includes(e.i)) return [];
    const [p, q] = [raw[(k + raw.length - 1) % raw.length], raw[(k + 1) % raw.length]];
    return [
      {
        at: (a: number, d: number): [number, number] => [e.ax + e.ux * a + e.nx * d, e.ay + e.uy * a + e.ny * d],
        len: e.len,
        k0: open.includes(p.i) ? 0 : half(p, e),
        k1: open.includes(q.i) ? 0 : half(e, q),
        toLight: lit(-e.nx, -e.ny) > 0,
      },
    ];
  });
}

/** Where a line from (px, py) along (dx, dy) runs inside the convex outline `pts` (its `t` range). */
function clipLine(pts: number[], px: number, py: number, dx: number, dy: number): [number, number] {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  let [t0, t1] = [-Infinity, Infinity];
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    let [nx, ny] = [-(by - ay), bx - ax];
    if ((cx - ax) * nx + (cy - ay) * ny < 0) [nx, ny] = [-nx, -ny];
    // Inside where (p + t d - a) . n >= 0.
    const num = (px - ax) * nx + (py - ay) * ny;
    const den = dx * nx + dy * ny;
    if (Math.abs(den) < 1e-9) {
      if (num < 0) return [0, 0];
    } else if (den > 0) t0 = Math.max(t0, -num / den);
    else t1 = Math.min(t1, -num / den);
  }
  return t1 > t0 ? [t0, t1] : [0, 0];
}

/**
 * A flat top's floor inside a convex outline: flags in courses along its longest side, their
 * joints staggered course to course (a tower top's).
 */
export function flagFloor(g: Graphics, pts: number[], rnd: () => number) {
  g.poly(pts).fill(WALK);
  const m = pts.length / 2;
  let [ux, uy, best] = [1, 0, 0];
  for (let i = 0; i < m; i++) {
    const [dx, dy] = [pts[((i + 1) % m) * 2] - pts[i * 2], pts[((i + 1) % m) * 2 + 1] - pts[i * 2 + 1]];
    const l = Math.hypot(dx, dy);
    if (l > best) [ux, uy, best] = [dx / l, dy / l, l];
  }
  const [vx, vy] = [-uy, ux];
  let [lo, hi] = [Infinity, -Infinity];
  for (let i = 0; i < m; i++) {
    const o = pts[i * 2] * vx + pts[i * 2 + 1] * vy;
    [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
  }
  const row = FT * 2.8;
  for (let o = lo + row * (0.3 + rnd() * 0.7); o < hi; o += row) {
    const [px, py] = [vx * o, vy * o];
    const [t0, t1] = clipLine(pts, px, py, ux, uy);
    if (t1 <= t0) continue;
    g.moveTo(px + ux * t0, py + uy * t0).lineTo(px + ux * t1, py + uy * t1);
    for (let t = t0 + FT * (0.5 + rnd() * 2); t < t1; t += FT * (2.4 + rnd() * 1.4)) {
      const [qx, qy] = [px + ux * t, py + uy * t];
      const reach = Math.min(row, clipLine(pts, qx, qy, vx, vy)[1]);
      if (reach > 0) g.moveTo(qx, qy).lineTo(qx + vx * reach, qy + vy * reach);
    }
  }
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.4 });
}

/**
 * Battlements round a convex outline (a keep's, a flat-topped building's), in the curtain's
 * style: inside the outline a band of coping lit on its edge toward the light, crenels notched
 * through it, inked along the walk and round the outline; the shade of the lit sides' parapets
 * on what lies inside; `walk` px of wall walk inside the parapet given, its flags' joints across
 * it. Sides in `open` (by index) have none (where a higher piece stands against them).
 */
export function parapetRing(g: Graphics, pts: number[], rnd: () => number, walk = 0, open: number[] = []) {
  const sides = sidesOf(pts, open);
  const band = (s: Side, d0: number, d1: number) => [...s.at(s.k0 * d0, d0), ...s.at(s.len - s.k1 * d0, d0), ...s.at(s.len - s.k1 * d1, d1), ...s.at(s.k0 * d1, d1)];
  const P = PARAPET;
  if (walk > 0) {
    for (const s of sides) {
      const [a0, a1] = [s.k0 * (P + walk), s.len - s.k1 * (P + walk)];
      for (let a = a0 + rnd() * FT * 2; a < a1; a += FT * (2.4 + rnd() * 1.4)) {
        g.moveTo(...s.at(a, P)).lineTo(...s.at(a + (rnd() - 0.5) * FT * 0.4, P + walk));
        if (rnd() < 0.3 && walk > FT * 2) g.moveTo(...s.at(a, P + walk / 2)).lineTo(...s.at(a + FT * (1.6 + rnd()), P + walk / 2 + (rnd() - 0.5) * FT * 0.3));
      }
    }
    g.stroke({ width: 1.3, color: STONE[0], alpha: 0.6 });
  }
  for (const s of sides) if (s.toLight) g.poly(band(s, P, P + FT * 0.6));
  g.fill({ color: PLUM, alpha: 0.25 });
  for (const s of sides) g.poly(band(s, 0, P));
  g.fill(STONE[2]);
  for (const s of sides) {
    const e = s.toLight ? FT * 0.3 : P - FT * 0.3;
    g.moveTo(...s.at(s.k0 * e, e)).lineTo(...s.at(s.len - s.k1 * e, e));
  }
  g.stroke({ width: FT * 0.3, color: STONE[3], alpha: 0.8 });
  const crenels: number[][] = [];
  for (const s of sides) crenelRun(s.at, s.k0 * P, s.len - s.k1 * P, P, 0, rnd, crenels);
  for (const s of sides) g.moveTo(...s.at(s.k0 * P, P)).lineTo(...s.at(s.len - s.k1 * P, P));
  g.stroke({ width: 1.8, color: INK });
  crenelFill(g, crenels);
  g.poly(pts).stroke({ width: 2.5, color: INK, join: 'round' });
}

/**
 * A stair's hatch: an opening with steps going down into the dark, its timber lid lying open
 * beside it. `hp(a, b)`: a point `a` along the hatch (`hl` long) and `b` across it (`hw` half
 * wide) from its middle; the lid lies on its +b side.
 */
export function stairHatch(g: Graphics, hp: At, hl: number, hw: number) {
  const hq = (a0: number, a1: number, b0: number, b1: number) => [...hp(a0, b0), ...hp(a1, b0), ...hp(a1, b1), ...hp(a0, b1)];
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = hl / 2 - (k + 1) * hl * 0.21;
    g.poly(hq(a, a + hl * 0.21, -hw + 1.5, hw - 1.5)).fill(mix(STONE[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 2);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]);
  g.poly(hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 0.7)).fill({ color: WOOD[2], alpha: 0.7 });
  for (const t of [0.33, 0.66]) g.moveTo(...hp(-hl / 2, hw + FT * 0.3 + hw * 2 * t)).lineTo(...hp(hl / 2, hw + FT * 0.3 + hw * 2 * t));
  g.stroke({ width: 1.2, color: WOOD[0] });
  for (const t of [0.22, 0.78]) g.moveTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3)).lineTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3 + hw * 2));
  g.stroke({ width: FT * 0.28, color: IRON[0] });
  g.poly(lid).stroke({ width: 1.8, color: INK, join: 'round' });
}

/**
 * A stair hatch in a convex flat top's corner (the one at the end of the side facing most away
 * from the light), across the walk inside the parapet, its lid laid back along it; `room` how far
 * in from the outline there is flat to put it (none if too little).
 */
export function cornerHatch(g: Graphics, pts: number[], room: number) {
  const sides = sidesOf(pts);
  if (!sides.length) return;
  let s = sides[0];
  let low = Infinity;
  for (const q of sides) {
    const [x0, y0] = q.at(0, 0);
    const [x1, y1] = q.at(1, 0);
    const [x2, y2] = q.at(0, 1);
    const l = lit(-(x2 - x0), -(y2 - y0)) + lit(x1 - x0, y1 - y0) * 0.5;
    if (l < low) [s, low] = [q, l];
  }
  const hw = FT * 1.6;
  const hl = Math.min(FT * 4.5, room - PARAPET - FT * 1.6);
  if (hl < FT * 2.5 || s.len < (PARAPET + FT * 1.6 + hw * 4) * 2) return;
  const along = s.len - s.k1 * PARAPET - FT * 0.8 - hw;
  const depth = PARAPET + FT * 0.8 + hl / 2;
  stairHatch(g, (a, b) => s.at(along - b, depth - a), hl, hw);
}

/**
 * A round flat top (a wall tower's, a turret's, a wizard's tower's) at (x, y), `r` its radius: a
 * ring of masonry a little out of round, a flagged fighting top inside it, the stair's hatch, the
 * parapet with its crenels round it (a ruin's broken over a stretch, its stones to `fallen`) and
 * the ring's lit and shaded faces. Its shadow is the caller's. Returns where the hatch faces and
 * where a ruin's parapet broke.
 */
export function roundTop(g: Graphics, x: number, y: number, r: number, rnd: () => number, ruined = false, fallen: number[][] = []) {
  const ring: number[] = [];
  const n = 22;
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const rr = r * (1 + (rnd() - 0.5) * 0.035);
    ring.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr);
  }
  g.poly(ring).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags inside the parapet, their joints, the parapet's shade on the lit side.
  const rf = r - PARAPET;
  g.circle(x, y, rf).fill(WALK);
  const turn = rnd() * Math.PI;
  const [cu, su] = [Math.cos(turn), Math.sin(turn)];
  // Courses of flags across the top (turned at random), their joints staggered course to course.
  const row = FT * 2.8;
  for (let o = -rf + row * (0.3 + rnd() * 0.7); o < rf; o += row) {
    const h = Math.sqrt(Math.max(0, rf * rf - o * o));
    const [px, py] = [x - su * o, y + cu * o];
    g.moveTo(px - cu * h, py - su * h).lineTo(px + cu * h, py + su * h);
    const h1 = Math.sqrt(Math.max(0, rf * rf - (o + row) * (o + row)));
    const span = Math.min(h, h1 || h);
    for (let t = -span + FT * (0.5 + rnd() * 2); t < span; t += FT * (2.4 + rnd() * 1.4)) {
      g.moveTo(px + cu * t, py + su * t).lineTo(px + cu * t - su * Math.min(row, rf - o), py + su * t + cu * Math.min(row, rf - o));
    }
  }
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.4 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (rf - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (rf - FT * 0.3));
  g.arc(x, y, rf - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
  // The stair's hatch near the rim.
  const th = rnd() * Math.PI * 2;
  const [ex, ey] = [Math.cos(th), Math.sin(th)];
  const [hl, hw] = [Math.min(FT * 4.5, rf * 0.5), FT * 1.6];
  const hc = rf - hl / 2 - FT * 0.8;
  stairHatch(g, (a, b) => [x + ex * (hc + a) - ey * b, y + ey * (hc + a) + ex * b], hl, hw);
  const lost = roundParapet(g, x, y, r, turn, rnd, ruined, fallen);
  return { ex, ey, rf, lost };
}

/**
 * A round parapet at (x, y) from `r` in to `r - PARAPET`: one ring of coping, crenels notched
 * through it (from angle `turn`; a ruin's broken over a stretch, returned), inked along the walk,
 * the ring's lit and shaded faces.
 */
export function roundParapet(g: Graphics, x: number, y: number, r: number, turn: number, rnd: () => number, ruined = false, fallen: number[][] = []) {
  const rf = r - PARAPET;
  const m = Math.max(8, Math.round((Math.PI * 2 * (r - PARAPET / 2)) / (FT * 4)));
  const step = (Math.PI * 2) / m;
  const lost = ruined ? rnd() * Math.PI * 2 : NaN;
  const [ri, ro] = [rf + 1.5, r - 1.5];
  g.circle(x, y, (rf + r) / 2).stroke({ width: r - rf, color: STONE[2] });
  const notch = (t0: number, t1: number) => {
    const q: number[] = [];
    for (const t of [t0, (t0 + t1) / 2, t1]) q.push(x + Math.cos(t) * ri, y + Math.sin(t) * ri);
    for (const t of [t1, (t0 + t1) / 2, t0]) q.push(x + Math.cos(t) * ro, y + Math.sin(t) * ro);
    g.poly(q);
  };
  for (let k = 0; k < m; k++) {
    const t0 = k * step + turn;
    const gone = ruined && (Math.abs(Math.atan2(Math.sin(t0 - lost), Math.cos(t0 - lost))) < 0.6 || rnd() < 0.25);
    if (gone && rnd() < 0.5) fallen.push(...rubble(x + Math.cos(t0) * (rf - FT), y + Math.sin(t0) * (rf - FT), 1, FT * 0.6, rnd));
    notch(gone ? t0 - step * 0.3 : t0 + step * 0.62, t0 + step * (gone ? 0.95 : 0.95));
  }
  g.fill(mix(STONE[0], PLUM, 0.3)).stroke({ width: 1.2, color: INK, alpha: 0.8, join: 'round' });
  g.circle(x, y, rf).stroke({ width: 1.8, color: INK });
  // The ring's lit and shaded faces.
  g.moveTo(x + Math.cos(Math.PI * 0.9) * (r - 2), y + Math.sin(Math.PI * 0.9) * (r - 2));
  g.arc(x, y, r - 2, Math.PI * 0.9, Math.PI * 1.6);
  g.stroke({ width: 2.5, color: STONE[3], alpha: 0.7 });
  g.moveTo(x + Math.cos(-Math.PI * 0.1) * (r - 2), y + Math.sin(-Math.PI * 0.1) * (r - 2));
  g.arc(x, y, r - 2, -Math.PI * 0.1, Math.PI * 0.6);
  g.stroke({ width: 3, color: PLUM, alpha: 0.3 });
  return lost;
}

/**
 * A round wall walk at (x, y) from `r1` in to `r0` (a shell keep's, inside its parapet): flags,
 * their joints across it, the shade of the parapet on the lit side.
 */
export function roundWalk(g: Graphics, x: number, y: number, r0: number, r1: number, rnd: () => number) {
  g.circle(x, y, (r0 + r1) / 2).stroke({ width: r1 - r0, color: WALK });
  const rm = (r0 + r1) / 2;
  for (let t = rnd() * 0.2; t < Math.PI * 2 - FT / rm; t += (FT * (2.4 + rnd() * 1.4)) / rm) {
    const t1 = t + ((rnd() - 0.5) * FT * 0.4) / rm;
    g.moveTo(x + Math.cos(t) * r0, y + Math.sin(t) * r0).lineTo(x + Math.cos(t1) * r1, y + Math.sin(t1) * r1);
  }
  g.stroke({ width: 1.3, color: STONE[0], alpha: 0.6 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (r1 - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (r1 - FT * 0.3));
  g.arc(x, y, r1 - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
}

/**
 * Curtain walls (`ShapeKind::Wall`, one segment each, `size` thick): a soft shadow, ink and the
 * stone body (round joints where no end is bare), a flagged walk down the middle, a parapet with
 * merlons along each side, the shade of the parapet facing the light across the walk; moss and
 * cracks; rubble where an end stands bare; a ruin's with merlons gone and stones fallen.
 */
export function drawWalls(g: Graphics, walls: Shape[]) {
  const segs = walls.map((s) => {
    const [x0, y0, x1, y1] = s.pts;
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const flags = flagsOf(s, 4);
    const rnd = rngAt(x0 + x1, y0 + y1);
    const half = s.size / 2;
    // A point `a` along the segment and `d` across it (toward its left, +n).
    const at = (a: number, d: number): [number, number] => [x0 + ux * a - uy * d, y0 + uy * a + ux * d];
    // A bare end's broken line: a few points across it, pushed in or out a little.
    const broken = (a: number, dir: number) => [-1, -0.6, -0.2, 0.2, 0.6, 1].map((t) => at(a + dir * rnd() * FT * 2.6, t * half));
    const endA = flags & BARE_A ? broken(0, 1) : [at(0, -half), at(0, half)];
    const endB = flags & BARE_B ? broken(l, -1) : [at(l, -half), at(l, half)];
    const outline = [...endA, ...endB.reverse()].flat();
    return { x0, y0, x1, y1, l, ux, uy, half, at, outline, rnd, bareA: !!(flags & BARE_A), bareB: !!(flags & BARE_B), ruined: !!(flags & RUINED) };
  });
  const joints = (fn: (x: number, y: number) => void) => {
    for (const s of segs) {
      if (!s.bareA) fn(s.x0, s.y0);
      if (!s.bareB) fn(s.x1, s.y1);
    }
  };
  // Shadows, in two soft slices.
  for (const [off, alpha] of [
    [PX * 0.85, 0.12],
    [PX * 0.5, 0.18],
  ]) {
    for (const s of segs) g.poly(s.outline.map((v) => v + off));
    joints((x, y) => g.circle(x + off, y + off, segs[0].half));
    g.fill({ color: 0x000000, alpha });
  }
  // Ink, then the stone body.
  for (const s of segs) g.poly(s.outline);
  g.stroke({ width: 5, color: INK, join: 'round' });
  joints((x, y) => g.circle(x, y, segs[0].half + 2.5));
  g.fill(INK);
  for (const s of segs) g.poly(s.outline);
  joints((x, y) => g.circle(x, y, segs[0].half));
  g.fill(STONE[1]);
  // The walk between the parapets, its flags' joints and the shade of the lit side's parapet.
  for (const s of segs) {
    const w = s.half - PARAPET;
    const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
    if (a1 > a0) g.poly([...s.at(a0, -w), ...s.at(a1, -w), ...s.at(a1, w), ...s.at(a0, w)]);
  }
  joints((x, y) => g.circle(x, y, segs[0].half - PARAPET));
  g.fill(WALK);
  for (const s of segs) {
    const w = s.half - PARAPET;
    for (let a = s.rnd() * FT * 2; a < s.l; a += FT * (2.4 + s.rnd() * 1.4)) {
      g.moveTo(...s.at(a, -w)).lineTo(...s.at(a + (s.rnd() - 0.5) * FT * 0.4, w));
      if (s.rnd() < 0.3) g.moveTo(...s.at(a, 0)).lineTo(...s.at(a + FT * (1.6 + s.rnd()), (s.rnd() - 0.5) * FT * 0.3));
    }
  }
  g.stroke({ width: 1.3, color: STONE[0], alpha: 0.6 });
  for (const s of segs) {
    const w = s.half - PARAPET;
    // The parapet on the side the light comes from shades the walk beside it.
    const side = lit(-s.uy, s.ux) > 0 ? 1 : -1;
    g.poly([...s.at(0, side * w), ...s.at(s.l, side * w), ...s.at(s.l, side * (w - FT * 0.6)), ...s.at(0, side * (w - FT * 0.6))]);
  }
  g.fill({ color: PLUM, alpha: 0.25 });
  // Each parapet one band of coping (lit on its edge toward the light), the crenels notches
  // through it (a ruin's wider, merlons gone), inked along the walk.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  for (const s of segs) {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) continue;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * (s.half - PARAPET), side * s.half];
      g.poly([...s.at(a0, d0), ...s.at(a1, d0), ...s.at(a1, d1), ...s.at(a0, d1)]).fill(STONE[2]);
      const toLight = lit(-s.uy * side, s.ux * side) > 0;
      const e = toLight ? d1 - side * FT * 0.3 : d0 + side * FT * 0.3;
      g.moveTo(...s.at(a0, e)).lineTo(...s.at(a1, e)).stroke({ width: FT * 0.3, color: STONE[3], alpha: 0.8 });
      crenelRun(s.at, a0, a1, d0, d1, s.rnd, crenels, s.ruined, fallen);
      g.moveTo(...s.at(a0, d0)).lineTo(...s.at(a1, d0));
    }
  }
  g.stroke({ width: 1.8, color: INK });
  crenelFill(g, crenels);
  // Moss on the stone and the odd crack; a ruin's more of both.
  for (const s of segs) {
    const n = Math.floor((s.l / (FT * 24)) * (s.ruined ? 3 : 1) + s.rnd());
    for (let k = 0; k < n; k++) {
      const [mx, my] = s.at(s.rnd() * s.l, (s.rnd() < 0.5 ? -1 : 1) * (s.half - PARAPET * 0.5));
      const r = FT * (0.3 + s.rnd() * 0.25);
      g.ellipse(mx, my, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.85 });
      g.ellipse(mx - r * 0.3, my - r * 0.25, r * 0.6, r * 0.45).fill({ color: MOSS[1], alpha: 0.85 });
    }
    for (let k = 0; k < (s.ruined ? 3 : 1); k++) {
      if (s.rnd() < 0.5) continue;
      const a = s.rnd() * s.l;
      const d = (s.rnd() - 0.5) * s.half;
      g.moveTo(...s.at(a, d)).lineTo(...s.at(a + FT * 0.8, d + FT * 0.5)).lineTo(...s.at(a + FT * 1.5, d + FT * 0.2));
    }
  }
  g.stroke({ width: 1.4, color: STONE[0], alpha: 0.9, join: 'round' });
  // Rubble spilled where a stretch broke off, and below a ruin's walls.
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 1.5, 0), 8, s.half * 1.2, s.rnd));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 1.5, 0), 8, s.half * 1.2, s.rnd));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (14 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (s.half + FT * 1.5)), 2, FT * 1.2, s.rnd));
  }
  blocks(g, fallen, stoneTone(segs[0]?.rnd ?? Math.random));
}

/**
 * A wall or gate tower's top (`ShapeKind::Tower`: its centre, `size` its radius, then flags): a
 * long soft shadow, a ring of masonry a little out of round with merlons on it, lit on the
 * north-west; a flagged fighting top with the spiral stair's hatch; a gate tower's banner. A
 * ruin's has merlons gone, a stretch of its parapet broken and rubble on the top.
 */
export function drawWallTower(g: Graphics, s: Shape) {
  const [x, y, r] = [s.pts[0], s.pts[1], s.size];
  const flags = flagsOf(s, 2);
  if (flags & SQUARE) return drawSquareTower(g, s);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  rnd();
  rnd();
  // Shadow: the round swept away from the light (one outline, so it doesn't darken twice).
  const D = PX * 1.1;
  const shadow: number[] = [];
  for (let k = 0; k <= 12; k++) {
    const t = Math.PI * 0.75 + (k / 12) * Math.PI;
    shadow.push(x + Math.cos(t) * r, y + Math.sin(t) * r);
  }
  for (let k = 0; k <= 12; k++) {
    const t = -Math.PI * 0.25 + (k / 12) * Math.PI;
    shadow.push(x + D + Math.cos(t) * r, y + D + Math.sin(t) * r);
  }
  g.poly(shadow).fill({ color: 0x000000, alpha: 0.28 });
  const fallen: number[][] = [];
  const { ex, ey, rf, lost } = roundTop(g, x, y, r, rnd, ruined, fallen);
  if (ruined) {
    fallen.push(...rubble(x + Math.cos(lost) * rf * 0.5, y + Math.sin(lost) * rf * 0.5, 5, rf * 0.3, rnd));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = [x + (rnd() - 0.5) * rf * 1.4, y + (rnd() - 0.5) * rf * 1.4];
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
  // A gate tower flies the banner.
  if (gate && !ruined) banner(g, x - ex * rf * 0.4, y - ey * rf * 0.4);
}

/** A square tower's frame: its middle, half side and the way its sides run (`SQUARE`). */
function squareFrame(s: Shape) {
  const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
  return { x: s.pts[0], y: s.pts[1], h: s.size, ux: Math.cos(t), uy: Math.sin(t) };
}

/**
 * A square wall tower seen from above (a castle's: `SQUARE`): its shadow, the stone block in its
 * shaded and lit faces, a flagged fighting top inside a parapet of merlons, the stair's hatch with
 * its lid; a ruin's with merlons gone, cracks, moss and fallen stones.
 */
function drawSquareTower(g: Graphics, s: Shape) {
  const { x, y, h, ux, uy } = squareFrame(s);
  const ruined = !!(flagsOf(s, 2) & RUINED);
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  // Shadow, cast SE.
  const D = PX * 1.1;
  g.poly(quad(-h, h, -h, h).map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(quad(-h, h, -h, h)).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags in courses inside the parapet.
  const f = h - PARAPET;
  g.poly(quad(-f, f, -f, f)).fill(WALK);
  const row = FT * 2.8;
  for (let b = -f + row * (0.3 + rnd() * 0.7); b < f; b += row) {
    g.moveTo(...at(-f, b)).lineTo(...at(f, b));
    for (let a = -f + FT * (0.5 + rnd() * 2); a < f; a += FT * (2.4 + rnd() * 1.4)) g.moveTo(...at(a, b)).lineTo(...at(a, Math.min(f, b + row)));
  }
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.4 });
  // The parapet's shade across the top on the side away from the light.
  const lx = ux * LX + uy * LY;
  const ly = -uy * LX + ux * LY;
  g.poly(lx < 0 ? quad(-f, -f + FT * 0.8, -f, f) : quad(f - FT * 0.8, f, -f, f)).fill({ color: PLUM, alpha: 0.18 });
  g.poly(ly < 0 ? quad(-f, f, -f, -f + FT * 0.8) : quad(-f, f, f - FT * 0.8, f)).fill({ color: PLUM, alpha: 0.18 });
  // The stair's hatch in a corner, its lid lying open beside it.
  const [hl, hw] = [Math.min(FT * 4.5, f * 0.6), FT * 1.6];
  const ca = f - hl / 2 - FT * 0.8;
  const cb = f - hw - FT * 0.8;
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = ca + hl / 2 - (k + 1) * hl * 0.21;
    g.poly(quad(a, a + hl * 0.21, cb - hw + 1.5, cb + hw - 1.5)).fill(mix(STONE[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = quad(ca - hl / 2, ca + hl / 2, cb - hw - FT * 0.3 - hw * 2, cb - hw - FT * 0.3);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
  // The parapet: coping round the top, crenels notched through it (a ruin's broken in places).
  g.poly(quad(-h, h, -h, h));
  g.poly(quad(-f, f, -f, f)).cut();
  g.fill(STONE[2]);
  const fallen: number[][] = [];
  const step = FT * 4;
  const sides: [number, number, number, number][] = [
    [-h, -h, 1, 0],
    [h, -h, 0, 1],
    [h, h, -1, 0],
    [-h, h, 0, -1],
  ];
  const lostSide = ruined ? Math.floor(rnd() * 4) : -1;
  for (const [si, [a0, b0, da, db]] of sides.entries()) {
    const n = Math.max(2, Math.round((2 * h) / step));
    for (let k = 0; k < n; k++) {
      const gone = ruined && (si === lostSide || rnd() < 0.25);
      if (gone && rnd() < 0.5) fallen.push(...rubble(...at(a0 + da * (k + 0.5) * ((2 * h) / n) - db * 0, b0 + db * (k + 0.5) * ((2 * h) / n)), 1, FT * 0.6, rnd));
      const t0 = (k + (gone ? 0.05 : 0.6)) * ((2 * h) / n);
      const t1 = (k + 0.95) * ((2 * h) / n);
      // Inward from this side.
      const [ia, ib] = [-db, da];
      const p0 = [a0 + da * t0, b0 + db * t0];
      const p1 = [a0 + da * t1, b0 + db * t1];
      g.poly([...at(p0[0] + ia * 1.5, p0[1] + ib * 1.5), ...at(p1[0] + ia * 1.5, p1[1] + ib * 1.5), ...at(p1[0] + ia * (PARAPET - 1.5), p1[1] + ib * (PARAPET - 1.5)), ...at(p0[0] + ia * (PARAPET - 1.5), p0[1] + ib * (PARAPET - 1.5))]);
    }
  }
  g.fill(mix(STONE[0], PLUM, 0.3)).stroke({ width: 1.2, color: INK, alpha: 0.8, join: 'round' });
  g.poly(quad(-f, f, -f, f)).stroke({ width: 1.8, color: INK, join: 'round' });
  g.poly(quad(-h, h, -h, h)).stroke({ width: 3, color: INK, join: 'round' });
  if (ruined) {
    fallen.push(...rubble(x, y, 5, f * 0.4, rnd));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at((rnd() - 0.5) * f * 1.4, (rnd() - 0.5) * f * 1.4);
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
}

/**
 * A gatehouse (`ShapeKind::Gate`: the two gate towers' centres; `size` 1 for a ruin's): the
 * passage between the towers paved in setts, masonry jambs down its sides, a stone arch across
 * each end casting its shadow in, a portcullis slot inside each arch, the two doors standing
 * half open with their swings marked; over it all the wall walk, see-through, a winch for the
 * portcullis on some. A ruin's has no doors, walk or portcullis, and one arch broken.
 */
export function drawGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const at = (a: number, b: number): [number, number] => [x0 + ux * a - uy * b, y0 + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const R = FT * 15;
  const D = FT * 11;
  const ab = FT * 2.5;
  const [p0, p1] = [R, L - R];
  // The setts of the passage, rows across the way through, and worn ruts.
  g.poly(quad(p0 - FT, p1 + FT, -D, D)).fill(0xa39a8a);
  for (let b = -D + FT * 2; b < D; b += FT * 2) {
    g.moveTo(...at(p0 - FT, b)).lineTo(...at(p1 + FT, b));
    for (let a = p0 - FT + rnd() * FT * 2.5; a < p1 + FT; a += FT * (2.6 + rnd() * 1.2)) g.moveTo(...at(a, b - FT * 2)).lineTo(...at(a, b));
  }
  g.stroke({ width: 1.2, color: 0x7a7266, alpha: 0.8 });
  for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at((p0 + p1) / 2 + d, -D)).lineTo(...at((p0 + p1) / 2 + d, D));
  g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.1 });
  // The arches' shadows falling into the passage.
  const sh = FT * 1.4;
  for (const b0 of [-D, D - ab]) g.poly(quad(p0 - FT, p1 + FT, b0, b0 + ab).map((v) => v + sh));
  g.fill({ color: 0x000000, alpha: 0.25 });
  // Jambs down the sides, coursed, lit on the side toward the light.
  for (const [a0, a1] of [
    [p0 - FT * 2, p0 + FT * 0.8],
    [p1 - FT * 0.8, p1 + FT * 2],
  ]) {
    g.poly(quad(a0, a1, -D, D)).fill(STONE[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let b = -D + FT * 1.8; b < D; b += FT * 1.8) g.moveTo(...at(a0, b)).lineTo(...at(a1, b));
    g.stroke({ width: 1.2, color: STONE[0], alpha: 0.8 });
  }
  // Portcullis grooves inside each arch: the slot the raised gate hangs in, its iron foot showing.
  for (const b of [-D + ab + FT * 0.7, D - ab - FT * 0.7]) {
    g.poly(quad(p0, p1, b - FT * 0.35, b + FT * 0.35)).fill(0x2a2520);
    if (!ruined) g.moveTo(...at(p0 + FT * 0.3, b)).lineTo(...at(p1 - FT * 0.3, b)).stroke({ width: FT * 0.22, color: IRON[1] });
  }
  // The arches: voussoirs across the way, the keystone lighter, a sunlit coping edge; a ruin's
  // has one broken partway, its stones fallen in.
  const broken = ruined ? (rnd() < 0.5 ? 0 : 1) : -1;
  const fallen: number[][] = [];
  [-D, D - ab].forEach((b0, k) => {
    const end = k === broken ? p0 - FT + (p1 - p0 + FT * 2) * (0.35 + rnd() * 0.3) : p1 + FT;
    g.poly(quad(p0 - FT, end, b0, b0 + ab)).fill(STONE[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let a = p0 - FT + FT * 1.6; a < end - FT * 0.4; a += FT * 1.6) g.moveTo(...at(a, b0)).lineTo(...at(a, b0 + ab));
    g.stroke({ width: 1.2, color: STONE[0], alpha: 0.85 });
    const mid = (p0 + p1) / 2;
    if (mid < end) g.poly(quad(mid - FT * 0.7, mid + FT * 0.7, b0, b0 + ab)).fill(STONE[2]).stroke({ width: 1.4, color: INK });
    // The coping's lit edge: the long side toward the light.
    const litSide = lit(-uy, ux) > 0 ? b0 + ab - FT * 0.25 : b0 + FT * 0.25;
    g.moveTo(...at(p0 - FT, litSide)).lineTo(...at(end, litSide)).stroke({ width: FT * 0.3, color: STONE[3], alpha: 0.7 });
    if (k === broken) fallen.push(...rubble(...at(end + FT * 2, b0 + ab / 2 + (k ? -FT * 2 : FT * 2)), 6, FT * 2, rnd));
  });
  // The wall walk carried over the gate, between the towers: overhead, so shown see-through with
  // its edges dashed; on some gates the portcullis's winch stands on it, chains to the slot.
  if (!ruined) {
    const w = FT * 4;
    const [a0, a1] = [p0 - FT, p1 + FT];
    g.poly(quad(a0, a1, -w, w)).fill({ color: WALK, alpha: 0.5 });
    for (const side of [-1, 1]) {
      const [d0, d1] = side < 0 ? [-w, -w + PARAPET] : [w - PARAPET, w];
      g.poly(quad(a0, a1, d0, d1)).fill({ color: STONE[2], alpha: 0.6 });
      for (let a = a0 + FT * 1.6; a < a1 - FT * 1.6; a += FT * 3.9) g.poly(quad(a, a + FT * 1.4, d0 + 1.5, d1 - 1.5));
    }
    g.fill({ color: mix(STONE[0], PLUM, 0.3), alpha: 0.75 });
    for (const b of [-w, w]) for (let a = a0; a < a1; a += FT * 1.5) g.moveTo(...at(a, b)).lineTo(...at(Math.min(a + FT * 0.9, a1), b));
    g.stroke({ width: 2.5, color: INK, alpha: 0.9 });
    if (rnd() < 0.5) {
      const mid = (p0 + p1) / 2;
      const vb = -w + PARAPET + FT * 1.1;
      const slot = -D + ab + FT * 0.7;
      // Chains down to the portcullis.
      for (const o of [-FT * 1.8, FT * 1.8]) {
        for (let b = vb; b > slot; b -= FT * 0.7) g.moveTo(...at(mid + o, b)).lineTo(...at(mid + o, Math.max(slot, b - FT * 0.45)));
      }
      g.stroke({ width: FT * 0.25, color: IRON[0] });
      // The drum, rope wound on it, a post at each end and a spoked wheel to turn it.
      const drum = quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb + FT * 0.7);
      g.poly(drum.map((v) => v + FT * 0.3)).fill({ color: 0x000000, alpha: 0.25 });
      g.poly(drum).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
      g.poly(quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb - FT * 0.2)).fill(WOOD[2]);
      for (let a = mid - FT * 2.4; a < mid + FT * 2.5; a += FT * 0.6) g.moveTo(...at(a, vb - FT * 0.7)).lineTo(...at(a, vb + FT * 0.7));
      g.stroke({ width: 1.2, color: WOOD[0] });
      for (const a of [mid - FT * 3.5, mid + FT * 3.5]) g.poly(quad(a - FT * 0.5, a + FT * 0.5, vb - FT * 0.6, vb + FT * 0.6)).fill(WOOD[0]).stroke({ width: 1.6, color: INK });
      const [wx, wy] = at(mid + FT * 4.6, vb);
      for (let k = 0; k < 4; k++) {
        const t = (k / 4) * Math.PI + rnd() * 0.3;
        g.moveTo(wx - Math.cos(t) * FT * 1.3, wy - Math.sin(t) * FT * 1.3).lineTo(wx + Math.cos(t) * FT * 1.3, wy + Math.sin(t) * FT * 1.3);
      }
      g.stroke({ width: FT * 0.3, color: WOOD[1], cap: 'round' });
      g.circle(wx, wy, FT * 1.1).stroke({ width: FT * 0.25, color: WOOD[0] });
      g.circle(wx, wy, FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
    }
  }
  // The doors (drawn over the walk, to be read): two timber leaves hung on the jambs just inside
  // the outer portcullis, standing half open into the passage, each with its swing marked.
  if (!ruined) {
    const bh = -D + ab + FT * 1.7;
    const len = (p1 - p0) / 2 - FT * 0.2;
    const open = 0.95 + rnd() * 0.3;
    const th = FT * 1.4;
    for (const dir of [1, -1]) {
      const hinge = dir > 0 ? p0 : p1;
      const [du, dv] = [dir * Math.cos(open), Math.sin(open)];
      // Its thickness on the outer side (toward the portcullis).
      const [nu, nv] = [dir * dv, -dir * du];
      const pt = (t: number, k: number) => at(hinge + du * t + nu * k, bh + dv * t + nv * k);
      // The swing: an arc from where the leaf shuts to where it stands, in dashes.
      for (let k = 0; k < 7; k++) {
        const [t0, t1] = [(open * k) / 7, (open * (k + 0.55)) / 7];
        g.moveTo(...at(hinge + dir * Math.cos(t0) * len, bh + Math.sin(t0) * len)).lineTo(...at(hinge + dir * Math.cos(t1) * len, bh + Math.sin(t1) * len));
      }
      g.stroke({ width: 2, color: INK, alpha: 0.6 });
      const leaf = [...pt(0, 0), ...pt(len, 0), ...pt(len, th), ...pt(0, th)];
      g.poly(leaf.map((v) => v + FT * 0.35)).fill({ color: 0x000000, alpha: 0.28 });
      g.poly(leaf).fill(WOOD[1]);
      g.poly([...pt(0, 0), ...pt(len, 0), ...pt(len, th * 0.35), ...pt(0, th * 0.35)]).fill(WOOD[2]);
      // Its boards, end on along the leaf.
      for (const k of [0.35, 0.68]) g.moveTo(...pt(0, th * k)).lineTo(...pt(len, th * k));
      g.stroke({ width: 1.2, color: WOOD[0] });
      // Iron bands across the leaf and the studs along it.
      for (const t of [0.18, 0.5, 0.82]) g.moveTo(...pt(len * t, -1)).lineTo(...pt(len * t, th + 1));
      g.stroke({ width: FT * 0.28, color: IRON[0] });
      g.poly(leaf).stroke({ width: 2, color: INK, join: 'round' });
      g.circle(...pt(0, th * 0.5), FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
    }
  }
  if (ruined) {
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * 2 * D);
      g.ellipse(mx, my, FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
    }
    blocks(g, fallen, stoneTone(rnd));
  }
}

/** A garden wall's height (ft): its shadow's reach. */
const GARDEN_FT = 4;
/** A gate pier's half side (px; `battlemap`'s, 7-ft piers): where a forecourt gate's leaves hang. */
const PIER_HALF = PX * 0.7;

/** A shape's points as pairs. */
const pairs = (pts: number[]): [number, number][] => Array.from({ length: pts.length >> 1 }, (_, k) => [pts[k * 2], pts[k * 2 + 1]]);

/** A coping stone's tone as its face meets the light (`l`, -1 .. 1): sunlit toward cream, shaded toward plum. */
const coping = (l: number, rnd: () => number) => {
  const t = l >= 0 ? mix(STONE[2], STONE[3], l * 0.7) : mix(STONE[2], mix(STONE[1], PLUM, 0.25), -l * 0.85);
  return mix(t, rnd() < 0.5 ? STONE[1] : STONE[3], rnd() * 0.18);
};

/**
 * A forecourt's low garden wall (`ShapeKind::GardenWall`, one straight segment each, `size` thick):
 * a short soft shadow (it stands about 4 ft), a saddle-backed coping of flat stones along its top,
 * the face toward the light sunlit and the other shaded, joints across it, the odd tuft of moss; a
 * bold ink outline. Segments meeting end to end are mitred into one neat corner; a free end (at a
 * gate pier or the building's wall) is cut square.
 */
export function drawGardenWalls(g: Graphics, walls: Shape[]) {
  const segs = walls.map((s) => {
    const [x0, y0, x1, y1] = s.pts;
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    return { x0, y0, x1, y1, l, ux: (x1 - x0) / l, uy: (y1 - y0) / l, half: s.size / 2, rnd: rngAt(x0 * 0.7 + x1, y0 * 0.7 + y1) };
  });
  type Seg = (typeof segs)[number];
  // Each end's two corners (on the segment's left, +n, and right) and whether it is mitred.
  const ends = segs.map((s) => {
    const n = [-s.uy, s.ux];
    const out: { left: [number, number]; right: [number, number]; mitred: boolean }[] = [];
    for (const end of [0, 1]) {
      const [px, py] = end ? [s.x1, s.y1] : [s.x0, s.y0];
      let corner = { left: [px + n[0] * s.half, py + n[1] * s.half] as [number, number], right: [px - n[0] * s.half, py - n[1] * s.half] as [number, number], mitred: false };
      const o = segs.find((t) => t !== s && (Math.hypot(t.x0 - px, t.y0 - py) < 1.5 || Math.hypot(t.x1 - px, t.y1 - py) < 1.5));
      if (o) {
        // Heading into the corner along this one, out of it along the other.
        const din = end ? [s.ux, s.uy] : [-s.ux, -s.uy];
        const dout = Math.hypot(o.x0 - px, o.y0 - py) < 1.5 ? [o.ux, o.uy] : [-o.ux, -o.uy];
        const [ax, ay, bx, by] = [-din[1], din[0], -dout[1], dout[0]];
        const ml = Math.hypot(ax + bx, ay + by);
        if (ml > 0.2) {
          const [mx, my] = [(ax + bx) / ml, (ay + by) / ml];
          const k = Math.min(s.half * 3, s.half / (mx * ax + my * ay));
          const q: [number, number] = [px + mx * k, py + my * k];
          const q2: [number, number] = [px - mx * k, py - my * k];
          corner = end ? { left: q, right: q2, mitred: true } : { left: q2, right: q, mitred: true };
        }
      }
      out.push(corner);
    }
    return out;
  });
  // A point `a` along a segment and `d` across it (toward its left).
  const at = (s: Seg, a: number, d: number): [number, number] => [s.x0 + s.ux * a - s.uy * d, s.y0 + s.uy * a + s.ux * d];
  const outline = (k: number) => {
    const [A, B] = ends[k];
    return [...A.right, ...B.right, ...B.left, ...A.left];
  };
  // The shadow, short and soft (two slices).
  const reach = GARDEN_FT * FT * 0.55;
  for (const [f, alpha] of [
    [1, 0.12],
    [0.55, 0.16],
  ]) {
    for (let k = 0; k < segs.length; k++) g.poly(outline(k).map((v) => v + reach * f));
    g.fill({ color: 0x000000, alpha });
  }
  // The coping in blocks, each in two faces either side of the ridge, lit as they face.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    const l = lit(-s.uy, s.ux);
    // The joints, clear of the ends' mitres.
    const margin = s.half * 1.4;
    const cuts: number[] = [0];
    for (let a = margin + s.rnd() * FT * 1.6; a < s.l - margin; a += FT * (2.2 + s.rnd() * 1.4)) cuts.push(a);
    cuts.push(s.l);
    const edge = (i: number, side: 1 | -1): [number, number] => {
      if (i === 0) return side > 0 ? A.left : A.right;
      if (i === cuts.length - 1) return side > 0 ? B.left : B.right;
      return at(s, cuts[i] + (s.rnd() - 0.5) * FT * 0.25, side * s.half);
    };
    const lefts = cuts.map((_, i) => edge(i, 1));
    const rights = cuts.map((_, i) => edge(i, -1));
    const ridge = cuts.map((a) => at(s, a, 0));
    for (let i = 0; i + 1 < cuts.length; i++) {
      g.poly([...ridge[i], ...ridge[i + 1], ...lefts[i + 1], ...lefts[i]]).fill(coping(l, s.rnd));
      g.poly([...ridge[i], ...ridge[i + 1], ...rights[i + 1], ...rights[i]]).fill(coping(-l, s.rnd));
    }
    // The sunlit arris along the edge toward the light.
    const sun = l >= 0 ? 1 : -1;
    g.moveTo(...at(s, A.mitred ? s.half : 0, sun * (s.half - FT * 0.28))).lineTo(...at(s, B.mitred ? s.l - s.half : s.l, sun * (s.half - FT * 0.28)));
    g.stroke({ width: FT * 0.35, color: STONE[3], alpha: 0.55 * Math.abs(l) });
    // The ridge and the joints across it, a mitred end's joint too.
    g.moveTo(...ridge[0]).lineTo(...ridge[ridge.length - 1]);
    for (let i = 1; i + 1 < cuts.length; i++) g.moveTo(...lefts[i]).lineTo(...ridge[i]).lineTo(...rights[i]);
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.95, join: 'round' });
    for (const [E, P] of [
      [A, [s.x0, s.y0]],
      [B, [s.x1, s.y1]],
    ] as const) {
      if (E.mitred) g.moveTo(...E.left).lineTo(...P).lineTo(...E.right);
    }
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.9, join: 'round' });
    // The odd tuft of moss on the shaded face, or at the wall's foot.
    const tufts = Math.floor(s.l / (FT * 30) + s.rnd() * 0.6);
    for (let t = 0; t < tufts; t++) {
      const a = s.half * 1.5 + s.rnd() * Math.max(0, s.l - s.half * 3);
      const [mx, my] = at(s, a, -sun * s.half * (0.35 + s.rnd() * 0.25));
      const r = FT * (0.22 + s.rnd() * 0.14);
      g.ellipse(mx, my, r * 1.5, r).fill({ color: MOSS[0], alpha: 0.75 });
      g.ellipse(mx - r * 0.35, my - r * 0.3, r * 0.75, r * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  });
  // Bold ink round it all: the long sides and the square ends, not across a mitre.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    g.moveTo(...A.left).lineTo(...B.left);
    if (!B.mitred) g.lineTo(...B.right);
    else g.moveTo(...B.right);
    g.lineTo(...A.right);
    if (!A.mitred) g.lineTo(...A.left);
  });
  g.stroke({ width: 2.6, color: INK, join: 'round', cap: 'round' });
}

/** The convex hull of points, as a flat list round it. */
function hull(p: [number, number][]): number[] {
  const s = [...p].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const cross = (o: number[], a: number[], b: number[]) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const half = (list: [number, number][]) => {
    const h: [number, number][] = [];
    for (const q of list) {
      while (h.length >= 2 && cross(h[h.length - 2], h[h.length - 1], q) <= 0) h.pop();
      h.push(q);
    }
    h.pop();
    return h;
  };
  return [...half(s), ...half([...s].reverse())].flat();
}

/**
 * A forecourt gate's pier (`ShapeKind::GatePier`: its middle, then its sides' angle; `size` half its
 * side): a solid square of stone seen from above, its shadow to the south-east; the cap's fillet,
 * then its weathered slopes stepping in (lit on the faces toward the light, shaded on the others),
 * its flat top and a ball finial on a little plinth, the ball shaded round and casting its own shadow.
 */
export function drawGatePier(g: Graphics, s: Shape) {
  const [x, y, h] = [s.pts[0], s.pts[1], s.size];
  const t = s.pts.length > 2 ? s.pts[2] / PX : 0;
  const [ux, uy] = [Math.cos(t), Math.sin(t)];
  const rnd = rngAt(x * 1.7 + y * 0.23, y * 0.9 - x * 0.31);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  // A square `r` from the middle, its corners a little out of true.
  const square = (r: number, j = 0.6) => {
    const out: number[] = [];
    for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) out.push(...at(a * r + (rnd() - 0.5) * j, b * r + (rnd() - 0.5) * j));
    return out;
  };
  // The shadow: the block swept away from the light.
  const D = PX * 0.38;
  const foot = square(h, 0);
  g.poly(hull(pairs([...foot, ...foot.map((v) => v + D)]))).fill({ color: 0x000000, alpha: 0.22 });
  // The fillet, the widest of the cap.
  g.poly(square(h)).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  g.poly(square(h * 0.9)).fill(STONE[2]);
  // The weathering: four slopes from the fillet up to the top, each as it faces the light.
  const a0 = square(h * 0.84, 0.4);
  const a1 = square(h * 0.58, 0.4);
  for (let k = 0; k < 4; k++) {
    const m = (k + 1) % 4;
    const face = [a0[k * 2], a0[k * 2 + 1], a0[m * 2], a0[m * 2 + 1], a1[m * 2], a1[m * 2 + 1], a1[k * 2], a1[k * 2 + 1]];
    // Its outward way: from the middle to its outer edge's midpoint.
    const l = lit((a0[k * 2] + a0[m * 2]) / 2 - x, (a0[k * 2 + 1] + a0[m * 2 + 1]) / 2 - y);
    g.poly(face).fill(l >= 0 ? mix(STONE[2], STONE[3], 0.3 + l * 0.6) : mix(STONE[1], PLUM, 0.12 - l * 0.28));
  }
  g.poly(a0).stroke({ width: 1.6, color: INK, alpha: 0.85, join: 'round' });
  for (let k = 0; k < 4; k++) g.moveTo(a0[k * 2], a0[k * 2 + 1]).lineTo(a1[k * 2], a1[k * 2 + 1]);
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.8 });
  // The flat top, and the finial's plinth on it.
  g.poly(a1).fill(STONE[2]).stroke({ width: 1.6, color: INK, alpha: 0.9, join: 'round' });
  const pl = square(h * 0.36, 0.3);
  g.poly(pl.map((v) => v + h * 0.12)).fill({ color: PLUM, alpha: 0.25 });
  g.poly(pl).fill(STONE[1]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.poly(square(h * 0.28, 0.2)).fill(mix(STONE[2], STONE[1], 0.3));
  // The ball: its shadow on the top, a shaded round lit toward the light, a soft sheen (stone: matte).
  const r = h * 0.29;
  g.circle(x + r * 0.45, y + r * 0.45, r).fill({ color: PLUM, alpha: 0.3 });
  const ball = (k: number, ox: number, oy: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 18; i++) {
      const a = (i / 18) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * 0.05);
      pts.push(x + ox + Math.cos(a) * rr, y + oy + Math.sin(a) * rr);
    }
    return pts;
  };
  const round = ball(1, 0, 0);
  g.poly(round).fill(mix(STONE[1], PLUM, 0.18));
  g.poly(ball(0.78, LX * r * 0.2, LY * r * 0.2)).fill(STONE[2]);
  g.poly(ball(0.4, LX * r * 0.38, LY * r * 0.38)).fill({ color: STONE[3], alpha: 0.85 });
  g.poly(round).stroke({ width: 2, color: INK, join: 'round' });
}

/**
 * A forecourt's gate (`ShapeKind::ForecourtGate`: the opening's two ends at the piers' inner faces,
 * then a point inside; `size` its style, 0 wrought iron, 1 timber), seen from above as the map's other
 * doors and gates are: two leaves standing open, each hung on its pier's inner corner and swung into
 * the forecourt a little past square, the pair mirrored about the gate's axis and turned with it,
 * drawn over the piers' edges, each with its swing marked in dashes. A timber leaf is the top of its
 * boards, their joints across it, iron straps from the hinge along its faces; a wrought-iron one its
 * top rail, a spear finial on every bar and a C-scroll on each face, a stile post at the hinge and a
 * knob at the meeting end. Soft shadows to the south-east, an iron leaf's faint (it is open work).
 */
export function drawForecourtGate(g: Graphics, s: Shape) {
  const [ax, ay, bx, by, ix, iy] = s.pts;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  // Into the forecourt.
  const inward = (ix - ax) * -uy + (iy - ay) * ux >= 0 ? 1 : -1;
  const [vx, vy] = [-uy * inward, ux * inward];
  const timber = Math.round(s.size / PX) === 1;
  const rnd = rngAt(ax + bx, ay + by);
  // Its thickness from above, and its length (the two shut meet in the middle).
  const th = FT * (timber ? 1 : 0.8);
  const len = L / 2 - FT * 0.2;
  // Open a little past square, both alike.
  const open = (104 * Math.PI) / 180;
  for (const end of [0, 1]) {
    const dir = end ? -1 : 1;
    // Across the opening toward the other leaf (`dir`), into the forecourt (`v`).
    const way = (t: number): [number, number] => [ux * dir * Math.cos(t) + vx * Math.sin(t), uy * dir * Math.cos(t) + vy * Math.sin(t)];
    // The hinge on the pier's inner corner, the leaf's face flush with the pier's face to the opening.
    const [px, py] = end ? [bx, by] : [ax, ay];
    const [hx, hy] = [px + vx * (PIER_HALF - th * 0.5) + ux * dir * th * 0.5, py + vy * (PIER_HALF - th * 0.5) + uy * dir * th * 0.5];
    const [dx, dy] = way(open);
    // Across the leaf, from face to face.
    const [nx, ny] = [-dy, dx];
    // A point `p` of the way along the leaf (0 at the hinge) and `q` across it (-1 .. 1, face to face).
    const P = (p: number, q: number): [number, number] => [hx + dx * len * p + nx * th * 0.5 * q, hy + dy * len * p + ny * th * 0.5 * q];
    // The swing: an arc from where the leaf shuts to where it stands, in dashes.
    for (let k = 0; k < 8; k++) {
      const [t0, t1] = [(open * k) / 8, (open * (k + 0.5)) / 8];
      const [c0, c1] = [way(t0), way(t1)];
      g.moveTo(hx + c0[0] * len, hy + c0[1] * len).lineTo(hx + c1[0] * len, hy + c1[1] * len);
    }
    g.stroke({ width: 1.6, color: INK, alpha: 0.45 });
    // Its shadow, swept away from the light.
    const outline = [...P(0, -1), ...P(1, -1), ...P(1, 1), ...P(0, 1)];
    g.poly(hull(pairs([...outline, ...outline.map((v) => v + FT * (timber ? 1.5 : 1.1))]))).fill({ color: 0x000000, alpha: timber ? 0.2 : 0.08 });
    // (Which face is toward the light: +1 or -1.)
    const sun = lit(nx, ny) >= 0 ? 1 : -1;
    if (timber) timberLeaf(g, P, len, sun, rnd);
    else ironLeaf(g, P, len, th, sun);
  }
}

/** A timber gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face; `sun`
 * the face toward the light): the top of its boards, a lit band along the sunlit face, the joints
 * between the boards across it, iron straps from the hinge along both faces, nailed; its hinge pin. */
function timberLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, sun: number, rnd: () => number) {
  // A little out of true: hand-made.
  const j = () => (rnd() - 0.5) * 0.12;
  const panel = [...P(0, -1), ...P(0.5, -1 + j()), ...P(1, -1), ...P(1, 1), ...P(0.5, 1 + j()), ...P(0, 1)];
  g.poly(panel).fill(WOOD[1]);
  g.poly([...P(0.02, sun * 0.95), ...P(0.98, sun * 0.95), ...P(0.98, sun * 0.25), ...P(0.02, sun * 0.25)]).fill({ color: WOOD[2], alpha: 0.85 });
  // The boards, their joints across the leaf, some a shade lighter.
  const n = Math.max(4, Math.round(len / (FT * 1.05)));
  for (let k = 0; k < n; k++) if (rnd() < 0.35) g.poly([...P(k / n, -0.9), ...P((k + 1) / n, -0.9), ...P((k + 1) / n, 0.9), ...P(k / n, 0.9)]).fill({ color: WOOD[2], alpha: 0.3 });
  for (let k = 1; k < n; k++) g.moveTo(...P(k / n + j() * 0.2, -0.95)).lineTo(...P(k / n + j() * 0.2, 0.95));
  g.stroke({ width: 1.3, color: WOOD[0] });
  // The straps along both faces from the hinge, pointed at their ends, a nail or two.
  for (const q of [-1, 1]) {
    g.poly([...P(0, q * 1.08), ...P(0.6, q * 1.05), ...P(0.66, q * 0.95), ...P(0, q * 0.82)]).fill(IRON[0]);
    for (const p of [0.2, 0.45]) g.circle(...P(p, q * 0.95), 1.2).fill(IRON[2]);
  }
  g.poly(panel).stroke({ width: 2.2, color: INK, join: 'round' });
  g.circle(...P(0, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
}

/** A wrought-iron gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face;
 * `th` its depth, `sun` the face toward the light): the top rail, lit along its sunlit edge; a spear
 * finial on every bar, a lozenge across the rail; a C-scroll on each face, a third and two thirds of
 * the way along; a square stile post at the hinge with its pin, a knob on the meeting stile. Iron in
 * ink, each line drawn over a wider one of ink. */
function ironLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, th: number, sun: number) {
  const iron = (draw: () => void, w: number, c: number) => {
    draw();
    g.stroke({ width: w + 1.8, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color: c, cap: 'round', join: 'round' });
  };
  const n = Math.max(5, Math.round(len / (FT * 0.95)));
  // The scrolls: a C-scroll on each face, a third of the way along on one and two thirds on the
  // other, curling from the rail out and back toward the hinge.
  iron(() => {
    for (const [p0, side] of [
      [0.36, -1],
      [0.68, 1],
    ]) {
      const r = th * 0.42;
      const [cx, cy] = P(p0, side * (1 + (2 * r) / th));
      const a0 = Math.atan2(...((): [number, number] => {
        const [ox, oy] = P(p0, side);
        return [oy - cy, ox - cx];
      })());
      for (let i = 0; i <= 14; i++) {
        const a = a0 + side * (i / 14) * Math.PI * 1.7;
        const rr = r * (1 - i * 0.04);
        if (i) g.lineTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
        else g.moveTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
      }
    }
  }, 1.3, IRON[1]);
  // The top rail, hinge to meeting stile.
  iron(() => {
    g.moveTo(...P(0, 0)).lineTo(...P(1, 0));
  }, th * 0.5, IRON[0]);
  g.moveTo(...P(0.04, sun * 0.2)).lineTo(...P(0.96, sun * 0.2)).stroke({ width: 0.9, color: IRON[2], alpha: 0.85 });
  // The bars' spear finials: lozenges across the rail.
  for (let k = 1; k < n; k++) {
    const p = k / n;
    const d = Math.min(0.06, 0.3 / n);
    g.poly([...P(p - d, 0), ...P(p, -1.1), ...P(p + d, 0), ...P(p, 1.1)]).fill(IRON[1]).stroke({ width: 1, color: INK, join: 'round' });
    g.circle(...P(p, 0), Math.max(1, th * 0.12)).fill(IRON[2]);
  }
  // The hinge stile's square post and pin, the meeting stile's knob.
  g.poly([...P(-0.03, -1.1), ...P(0.07, -1.1), ...P(0.07, 1.1), ...P(-0.03, 1.1)]).fill(IRON[0]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.circle(...P(0.02, 0), FT * 0.2).fill(IRON[2]).stroke({ width: 1, color: INK });
  g.circle(...P(1, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
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
  gravelPatch(g, at, la / PX, lb / PX, PX, rngAt((q[0][0] + q[2][0]) / 2, (q[0][1] + q[2][1]) / 2), [LX, LY]);
}

/** A banner streaming east on the wind from a pole at (x, y). */
export function banner(g: Graphics, x: number, y: number) {
  g.moveTo(x, y).lineTo(x + PX * 0.2, y - PX * 1.8).stroke({ width: 2.5, color: 0x3b2f25 });
  g.poly([x + PX * 0.2, y - PX * 1.8, x + PX * 1.7, y - PX * 1.5, x + PX * 1.4, y - PX * 1.25, x + PX * 1.75, y - PX * 0.95, x + PX * 0.15, y - PX * 1.05])
    .fill(0x9b2226)
    .stroke({ width: 1.5, color: INK });
}

/** The 5-ft grid over a wall's walk or a tower's top (shown with the ground's): lines on the square
 * boundaries, clipped to the walk (a quad) or the top (a round); over a court's gravel walk too, as
 * faint as the ground's own. None over a forecourt's low wall, piers or gate. */
export function topGrid(g: Graphics, s: Shape) {
  if (s.kind === 13) {
    if (s.pts.length < 8) return;
    gridIn(g, s.pts.slice(0, 8));
    g.stroke({ width: 1, color: INK, alpha: 0.28, pixelLine: true });
    return;
  }
  if (s.kind === 1 && flagsOf(s, 2) & SQUARE) {
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
    const w = s.size / 2 - PARAPET;
    const [vx, vy] = [(-(y1 - y0) / len) * w, ((x1 - x0) / len) * w];
    gridIn(g, [x0 - vx, y0 - vy, x1 - vx, y1 - vy, x1 + vx, y1 + vy, x0 + vx, y0 + vy]);
  } else return;
  g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
}

/** The 5-ft grid's lines where they cross a convex quad. */
function gridIn(g: Graphics, q: number[]) {
  // Where the line `axis` = `at` crosses the walk's outline, as the span along the other axis.
  for (const axis of [0, 1]) {
    const vals = [q[axis], q[axis + 2], q[axis + 4], q[axis + 6]];
    for (let k = Math.ceil(Math.min(...vals) / PX); k * PX <= Math.max(...vals); k++) {
      let [lo, hi] = [Infinity, -Infinity];
      for (let i = 0; i < 4; i++) {
        const [a, b] = [i * 2, ((i + 1) % 4) * 2];
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
