import type { Shape } from '../../gen/battlePrep';
import { rngAt, FT, INK, PLUM, MOSS, mix } from '../bridges';
import { PX } from '../scale';
import type { Graphics } from 'pixi.js';
import { type At, BARE_A, BARE_B, BATTER, LINE, MAT_EARTH, MAT_PALISADE, MAT_STONE, OUT, PARAPET, RUINED, ST, STAKE, WK, blocks, brick, crenelFill, crenelRun, flagsOf, lit, lookOf, rubble, stoneTone, use } from './base';
import { logPoly, logs, stakeTops } from './towers';

/** A wall segment as drawn: its ends, length and frame (`at`: `a` along, `d` across toward its left, +n), its
 * half thickness, outline (a bare end broken), its own random stream and flags. */
function wallSeg(s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const l = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
  const flags = flagsOf(s, 4);
  const rnd = rngAt(x0 + x1, y0 + y1);
  const half = s.size / 2;
  const at = (a: number, d: number): [number, number] => [x0 + ux * a - uy * d, y0 + uy * a + ux * d];
  // A bare end's broken line: a few points across it, pushed in or out a little.
  const broken = (a: number, dir: number) => [-1, -0.6, -0.2, 0.2, 0.6, 1].map((t) => at(a + dir * rnd() * FT * 2.6, t * half));
  const endA = flags & BARE_A ? broken(0, 1) : [at(0, -half), at(0, half)];
  const endB = flags & BARE_B ? broken(l, -1) : [at(l, -half), at(l, half)];
  const outline = [...endA, ...endB.reverse()].flat();
  return { x0, y0, x1, y1, l, ux, uy, side: Math.sign(Math.round((s.pts[5] ?? 0) / PX)), half, at, outline, rnd, bareA: !!(flags & BARE_A), bareB: !!(flags & BARE_B), ruined: !!(flags & RUINED) };
}
export type Seg = ReturnType<typeof wallSeg>;

/**
 * Curtain walls (`ShapeKind::Wall`, one segment each, `size` thick) by their look (the style on point 6):
 * stone in its stone's tones and its group's merlons (`stoneWalls`), a timber palisade (`palisades`), rammed
 * earth (`earthWalls`); each look's together, so its fills batch. `gates` (`ShapeKind::Gate`) tell a palisade
 * which way the town lies where its run doesn't turn.
 */
export function drawWalls(g: Graphics, walls: Shape[], gates: Shape[] = []) {
  const sets = new Map<number, Shape[]>();
  for (const s of walls) {
    const l = lookOf(s, 6);
    const key = l.material * 64 + l.stone * 8 + l.merlons;
    const set = sets.get(key);
    if (set) set.push(s);
    else sets.set(key, [s]);
  }
  for (const set of sets.values()) {
    const l = lookOf(set[0], 6);
    use(l);
    const segs = (l.material === MAT_STONE || l.material === MAT_EARTH ? set.flatMap(breaks) : set).map(wallSeg);
    if (l.material === MAT_PALISADE) palisades(g, segs, gates);
    else if (l.material === MAT_EARTH) earthWalls(g, segs);
    else stoneWalls(g, segs, gates);
  }
  use(null);
}

/** A ruined curtain cut into stretches with gaps between (the walk open to the ground), each stretch's cut ends
 * bare (broken, rubble heaped); other walls whole. */
function breaks(s: Shape): Shape[] {
  if (!(flagsOf(s, 4) & RUINED)) return [s];
  const [x0, y0, x1, y1] = s.pts;
  const l = Math.hypot(x1 - x0, y1 - y0);
  if (l < FT * 16) return [s];
  const rnd = rngAt(x0 * 0.7 + x1, y0 * 0.7 + y1);
  rnd();
  const n = l > FT * 46 ? 2 : 1;
  const cuts: [number, number][] = [];
  for (let k = 0; k < n; k++) {
    const c = (l * (k + 0.3 + rnd() * 0.4)) / n;
    const w = FT * (7 + rnd() * 6);
    cuts.push([Math.max(FT * 4, c - w / 2), Math.min(l - FT * 4, c + w / 2)]);
  }
  const out: Shape[] = [];
  const flags = flagsOf(s, 4);
  let a = 0;
  const piece = (from: number, to: number, fl: number) => {
    if (to - from < FT * 3) return;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const pts = s.pts.slice();
    [pts[0], pts[1], pts[2], pts[3], pts[4]] = [x0 + ux * from, y0 + uy * from, x0 + ux * to, y0 + uy * to, fl * PX];
    out.push({ ...s, pts });
  };
  cuts.forEach(([c0, c1], k) => {
    piece(a, c0, flags | BARE_B | (k ? BARE_A : 0));
    a = c1;
  });
  piece(a, l, flags | BARE_A);
  return out.length ? out : [s];
}

/** Which side of each segment faces out of the town (+1 its +n, -1 its -n): a run of segments end to end turns
 * toward the town it rings, so its outside is the side it turns from; a straight run's away from the way the
 * nearest gate leads in (`gates`), or +n with none. */
function outsides(segs: Seg[], gates: Shape[] = [], strict = false): number[] {
  const n = segs.length;
  const root = segs.map((_, i) => i);
  const find = (i: number): number => (root[i] === i ? i : (root[i] = find(root[i])));
  const links: [number, number][] = [];
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      if (i !== j && Math.abs(segs[i].x1 - segs[j].x0) + Math.abs(segs[i].y1 - segs[j].y0) < 2) {
        links.push([i, j]);
        root[find(i)] = find(j);
      }
    }
  }
  const turn = new Array(n).fill(0);
  for (const [i, j] of links) turn[find(i)] += segs[i].ux * segs[j].uy - segs[i].uy * segs[j].ux;
  return segs.map((s, i) => {
    // (The generator's own word where it has one: `[flags, side]`, side +1 its +n.)
    if (s.side) return s.side;
    const t = turn[find(i)];
    if (strict && Math.abs(t) <= 1e-3 && !gates.length) return 0;
    if (Math.abs(t) > 1e-3 || !gates.length) return t > 1e-3 ? -1 : 1;
    // (The nearest gate's way in: its side of the line between its towers.)
    const [mx, my] = [(s.x0 + s.x1) / 2, (s.y0 + s.y1) / 2];
    let [best, nx, ny] = [Infinity, 0, 0];
    for (const q of gates) {
      const [x0, y0, x1, y1] = q.pts;
      const d = Math.hypot((x0 + x1) / 2 - mx, (y0 + y1) / 2 - my);
      const l = Math.hypot(x1 - x0, y1 - y0) || 1;
      const sd = flagsOf(q, 6) < 0 ? -1 : 1;
      if (d < best) [best, nx, ny] = [d, (-(y1 - y0) / l) * sd, ((x1 - x0) / l) * sd];
    }
    return -s.uy * nx + s.ux * ny > 0 ? -1 : 1;
  });
}

/** Where the light falls on a segment's side `side` (+1 its +n). */
const sideLit = (s: Seg, side: number) => lit(-s.uy * side, s.ux * side);

/** A segment's end where the next run goes on from it (neither end bare): on its `inner` side (+1 its +n, the
 * turn's inside) a line `d` across it stops `k * |d|` short of the end (the mitre with the next run's). */
interface Mitre {
  k: number;
  inner: number;
}
/** A turn's outer corner: round the joint at (x, y) from angle `t0` through `sweep`; `lit` for each half (the
 * first segment's, the second's) whether that side is the one whose parapet shades the walk. */
interface Corner {
  x: number;
  y: number;
  t0: number;
  sweep: number;
  lit: [boolean, boolean];
}

/** Where runs of a curtain meet end to end (`segs`, each joint once): each segment's ends' mitres (`[A, B]`)
 * and the outer corners round the joints. */
function mitres(segs: Seg[]): { cut: (Mitre | null)[][]; corners: Corner[] } {
  const cut: (Mitre | null)[][] = segs.map(() => [null, null]);
  const corners: Corner[] = [];
  const end = (s: Seg, e: number) => (e ? [s.x1, s.y1] : [s.x0, s.y0]);
  const litSide = (s: Seg) => (lit(-s.uy, s.ux) > 0 ? 1 : -1);
  segs.forEach((s, i) => {
    for (const e of [0, 1]) {
      if (e ? s.bareB : s.bareA) continue;
      const [x, y] = end(s, e);
      for (let j = 0; j < segs.length && !cut[i][e]; j++) {
        const t = segs[j];
        for (const f of [0, 1]) {
          if (j === i || (f ? t.bareB : t.bareA)) continue;
          const [tx, ty] = end(t, f);
          if (Math.abs(tx - x) + Math.abs(ty - y) >= 2) continue;
          // (Into the joint along s, away from it along t.)
          const [ix, iy] = e ? [s.ux, s.uy] : [-s.ux, -s.uy];
          const [ox, oy] = f ? [-t.ux, -t.uy] : [t.ux, t.uy];
          const [cr, dt] = [ix * oy - iy * ox, ix * ox + iy * oy];
          if (dt < -0.97) continue;
          const k = Math.abs(cr) / (1 + dt);
          const inner = -s.uy * ox + s.ux * oy > 0 ? 1 : -1;
          cut[i][e] = { k, inner };
          if (i < j && k > 0.02) {
            const innerT = t.uy * ix - t.ux * iy > 0 ? 1 : -1;
            const t0 = Math.atan2(s.ux * -inner, -s.uy * -inner);
            const t1 = Math.atan2(t.ux * -innerT, -t.uy * -innerT);
            corners.push({ x, y, t0, sweep: Math.atan2(Math.sin(t1 - t0), Math.cos(t1 - t0)), lit: [litSide(s) === -inner, litSide(t) === -innerT] });
          }
          break;
        }
      }
    }
  });
  return { cut, corners };
}

/** An annulus's sector round (x, y) from `r0` to `r1`, angle `t0` through `sweep`, as flat px pairs. */
function sector(x: number, y: number, r0: number, r1: number, t0: number, sweep: number): number[] {
  const n = Math.max(2, Math.ceil(Math.abs(sweep) / 0.12));
  const q: number[] = [];
  for (let k = 0; k <= n; k++) q.push(x + Math.cos(t0 + (sweep * k) / n) * r1, y + Math.sin(t0 + (sweep * k) / n) * r1);
  for (let k = n; k >= 0; k--) q.push(x + Math.cos(t0 + (sweep * k) / n) * r0, y + Math.sin(t0 + (sweep * k) / n) * r0);
  return q;
}

/**
 * Stone curtains: a soft shadow, ink and the stone body (round joints where no end is bare), a flagged walk
 * down the middle, a parapet with merlons along each side, the shade of the parapet facing the light across
 * the walk; moss and cracks; rubble where an end stands bare; a ruin's with merlons gone and stones fallen.
 */
function stoneWalls(g: Graphics, segs: Seg[], gates: Shape[]) {
  // (Merlons on the outer parapet only; where the outside can't be told, both.)
  const out = outsides(segs, gates, true);
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
  g.fill(ST[1]);
  // The walk between the parapets, its flags' joints and the shade of the lit side's parapet.
  for (const s of segs) {
    const w = s.half - PARAPET;
    const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
    if (a1 > a0) g.poly([...s.at(a0, -w), ...s.at(a1, -w), ...s.at(a1, w), ...s.at(a0, w)]);
  }
  joints((x, y) => g.circle(x, y, segs[0].half - PARAPET));
  g.fill(WK);
  // (Where runs turn, the inner parapets mitred, the outer one carried round the joint: `mitres`.)
  const { cut, corners } = mitres(segs);
  const lim = (s: Seg, i: number, e: number, base: number, d: number) => {
    const m = cut[i][e];
    return m && Math.sign(d) === m.inner ? base + (e ? -1 : 1) * Math.abs(d) * m.k : base;
  };
  segs.forEach((s, i) => {
    const w = s.half - PARAPET;
    const [lo, hi] = [lim(s, i, 0, 0, (cut[i][0]?.inner ?? 1) * s.half), lim(s, i, 1, s.l, (cut[i][1]?.inner ?? 1) * s.half)];
    for (let a = s.rnd() * FT * 2; a < s.l; a += FT * (2.4 + s.rnd() * 1.4)) {
      const b = a + (s.rnd() - 0.5) * FT * 0.4;
      if (a > lo && b > lo && a < hi && b < hi) g.moveTo(...s.at(a, -w)).lineTo(...s.at(b, w));
      if (s.rnd() < 0.3) {
        const [c, d] = [a + FT * (1.6 + s.rnd()), (s.rnd() - 0.5) * FT * 0.3];
        if (a > lo && c < hi) g.moveTo(...s.at(a, 0)).lineTo(...s.at(c, d));
      }
    }
  });
  g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  if (brick()) {
    // Rose brick: a few bold light courses along the walk, their joints staggered.
    for (const s of segs) {
      const w = s.half - PARAPET;
      const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
      for (const d of [-w * 0.5, w * 0.5]) g.moveTo(...s.at(a0, d)).lineTo(...s.at(a1, d));
      for (let a = a0 + FT * 1.5, j = 0; a < a1; a += FT * 4.5, j++) g.moveTo(...s.at(a, j % 2 ? 0 : -w)).lineTo(...s.at(a, j % 2 ? w : 0));
    }
    g.stroke({ width: LINE * 0.8, color: ST[3], alpha: 0.85 });
  }
  segs.forEach((s, i) => {
    const w = s.half - PARAPET;
    // The parapet on the side the light comes from shades the walk beside it.
    const side = lit(-s.uy, s.ux) > 0 ? 1 : -1;
    const [d0, d1] = [side * w, side * (w - FT * 0.6)];
    g.poly([...s.at(lim(s, i, 0, 0, d0), d0), ...s.at(lim(s, i, 1, s.l, d0), d0), ...s.at(lim(s, i, 1, s.l, d1), d1), ...s.at(lim(s, i, 0, 0, d1), d1)]);
  });
  for (const c of corners) {
    const w = segs[0].half - PARAPET;
    if (c.lit[0]) g.poly(sector(c.x, c.y, w - FT * 0.6, w, c.t0, c.sweep / 2));
    if (c.lit[1]) g.poly(sector(c.x, c.y, w - FT * 0.6, w, c.t0 + c.sweep / 2, c.sweep / 2));
  }
  g.fill({ color: PLUM, alpha: 0.25 });
  // Each parapet one band of coping (lit on its edge toward the light), the crenels notches
  // through it (a ruin's wider, merlons gone), inked along the walk.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  segs.forEach((s, i) => {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) return;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * (s.half - PARAPET), side * s.half];
      const [p0, p1, q0, q1] = [lim(s, i, 0, a0, d0), lim(s, i, 1, a1, d0), lim(s, i, 0, a0, d1), lim(s, i, 1, a1, d1)];
      if (Math.min(p1, q1) <= Math.max(p0, q0)) continue;
      g.poly([...s.at(p0, d0), ...s.at(p1, d0), ...s.at(q1, d1), ...s.at(q0, d1)]).fill(ST[2]);
      const toLight = lit(-s.uy * side, s.ux * side) > 0;
      const e = toLight ? d1 - side * FT * 0.3 : d0 + side * FT * 0.3;
      g.moveTo(...s.at(lim(s, i, 0, a0, e), e)).lineTo(...s.at(lim(s, i, 1, a1, e), e)).stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8 });
      if (!out[i] || out[i] === side) crenelRun(s.at, Math.max(p0, q0), Math.min(p1, q1), d0, d1, s.rnd, crenels, s.ruined, fallen);
      g.moveTo(...s.at(p0, d0)).lineTo(...s.at(p1, d0));
    }
  });
  // The outer parapet round each turn's joint, its coping and merlons carried on round.
  for (const c of corners) {
    const [r0, r1] = [segs[0].half - PARAPET, segs[0].half];
    g.poly(sector(c.x, c.y, r0, r1, c.t0, c.sweep)).fill(ST[2]);
    const rm = (r0 + r1) / 2;
    const dir = Math.sign(c.sweep);
    const at: At = (a, d) => [c.x + Math.cos(c.t0 + (dir * a) / rm) * d, c.y + Math.sin(c.t0 + (dir * a) / rm) * d];
    crenelRun(at, 0, Math.abs(c.sweep) * rm, r0, r1, rngAt(c.x * 1.7, c.y * 1.3), crenels);
  }
  g.stroke({ width: brick() ? 2.6 : 1.8, color: INK, join: 'miter' });
  crenelFill(g, crenels);
  // Moss on the stone and the odd crack; a ruin's more of both.
  for (const s of segs) {
    const n = Math.floor((s.l / (FT * 24)) * (s.ruined ? 1.5 : 1) + s.rnd());
    for (let k = 0; k < n; k++) {
      const [mx, my] = s.at(s.rnd() * s.l, (s.rnd() < 0.5 ? -1 : 1) * (s.half - PARAPET * 0.5));
      const r = FT * (0.3 + s.rnd() * 0.25);
      g.ellipse(mx, my, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.85 });
      g.ellipse(mx - r * 0.3, my - r * 0.25, r * 0.6, r * 0.45).fill({ color: MOSS[1], alpha: 0.85 });
    }
    for (let k = 0; k < (s.ruined ? 2 : 1); k++) {
      if (s.rnd() < 0.5) continue;
      const a = s.rnd() * s.l;
      const d = (s.rnd() - 0.5) * s.half;
      g.moveTo(...s.at(a, d)).lineTo(...s.at(a + FT * 0.8, d + FT * 0.5)).lineTo(...s.at(a + FT * 1.5, d + FT * 0.2));
    }
  }
  g.stroke({ width: 1.4, color: ST[0], alpha: 0.9, join: 'round' });
  // Rubble spilled where a stretch broke off, and below a ruin's walls.
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 2, 0), 3, s.half * 0.7, s.rnd, 2));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 2, 0), 3, s.half * 0.7, s.rnd, 2));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (22 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (s.half + FT * 1.5)), 1, FT * 1.2, s.rnd, 1.8));
  }
  blocks(g, fallen, stoneTone(segs[0]?.rnd ?? Math.random));
}

/**
 * A timber palisade (3 ft): a row of stakes along its outer face seen from above, each sharpened to a point
 * (its lit facet toward the light), inked; behind them, on the town side, a walk of planks laid across, its
 * inner edge inked; the stakes' shadow cast long. A ruin's has stakes gone and some lying fallen.
 */
function palisades(g: Graphics, segs: Seg[], gates: Shape[]) {
  const out = outsides(segs, gates);
  // (Each stake: its middle, kept; a ruin's gaps left out.)
  const stakes: [number, number][] = [];
  const fallen: [Seg, number, number][] = [];
  segs.forEach((s, i) => {
    const d = out[i] * (s.half - STAKE);
    const [a0, a1] = [s.bareA ? FT * 2 : 0, s.bareB ? s.l - FT * 2 : s.l];
    for (let a = a0 + STAKE * 0.6; a < a1; a += STAKE * 2.05) {
      if (s.ruined && s.rnd() < 0.3) {
        if (s.rnd() < 0.3) fallen.push([s, a, -out[i]]);
        continue;
      }
      stakes.push(s.at(a + (s.rnd() - 0.5) * FT * 0.12, d + (s.rnd() - 0.5) * FT * 0.14));
    }
  });
  // Shadows: the stakes' long, the walk's short.
  for (const [x, y] of stakes) g.circle(x + PX * 0.7, y + PX * 0.7, STAKE);
  g.fill({ color: 0x000000, alpha: 0.2 });
  // The walk: planks across it, the shade of the stakes on it where the light comes over them.
  const walk = (s: Seg, i: number) => {
    const [b0, b1] = [-out[i] * s.half, out[i] * (s.half - STAKE * 1.6)];
    return { b0, b1, q: [...s.at(0, b0), ...s.at(s.l, b0), ...s.at(s.l, b1), ...s.at(0, b1)] };
  };
  segs.forEach((s, i) => g.poly(walk(s, i).q.map((v) => v + PX * 0.25)));
  g.fill({ color: 0x000000, alpha: 0.15 });
  segs.forEach((s, i) => g.poly(walk(s, i).q));
  g.fill(WK);
  segs.forEach((s, i) => {
    const { b0, b1 } = walk(s, i);
    for (let a = FT * (0.4 + s.rnd() * 0.6); a < s.l; a += FT * (1.0 + s.rnd() * 0.25)) g.moveTo(...s.at(a, b0)).lineTo(...s.at(a, b1));
  });
  g.stroke({ width: 1.1, color: ST[0], alpha: 0.6 });
  segs.forEach((s, i) => {
    if (sideLit(s, out[i]) > 0) {
      const b1 = out[i] * (s.half - STAKE * 1.6);
      g.poly([...s.at(0, b1), ...s.at(s.l, b1), ...s.at(s.l, b1 - out[i] * FT * 0.5), ...s.at(0, b1 - out[i] * FT * 0.5)]);
    }
  });
  g.fill({ color: PLUM, alpha: 0.22 });
  segs.forEach((s, i) => {
    const b0 = -out[i] * s.half;
    g.moveTo(...s.at(0, b0)).lineTo(...s.at(s.l, b0));
  });
  g.stroke({ width: OUT, color: INK, cap: 'round' });
  // The stakes: each a cone from above.
  stakeTops(g, stakes, STAKE);
  // A ruin's fallen stakes, lying out from the line.
  const lying: number[][] = [];
  for (const [s, a, side] of fallen) {
    const t = (s.rnd() - 0.5) * 1.2;
    const [cx, cy] = s.at(a, side * (s.half + FT * 2.5));
    const [vx, vy] = [Math.cos(Math.atan2(s.uy, s.ux) + Math.PI / 2 + t), Math.sin(Math.atan2(s.uy, s.ux) + Math.PI / 2 + t)];
    lying.push(logPoly((p, q) => [cx + vx * p - vy * q, cy + vy * p + vx * q], -FT * 3, FT * 3, -STAKE, STAKE));
  }
  logs(g, lying, []);
}

/**
 * A rammed earth wall: one smooth tan body, its battered faces each side (the one toward the light lit, the
 * other in shade), a parapet with rounded merlons along each, a packed walk between; few big shapes. A ruin's
 * crumbles: merlons gone, lumps of earth fallen.
 */
function earthWalls(g: Graphics, segs: Seg[]) {
  const joints = (fn: (x: number, y: number) => void) => {
    for (const s of segs) {
      if (!s.bareA) fn(s.x0, s.y0);
      if (!s.bareB) fn(s.x1, s.y1);
    }
  };
  const half = segs[0].half;
  for (const s of segs) g.poly(s.outline.map((v) => v + PX * 0.6));
  joints((x, y) => g.circle(x + PX * 0.6, y + PX * 0.6, half));
  g.fill({ color: 0x000000, alpha: 0.2 });
  for (const s of segs) g.poly(s.outline);
  g.stroke({ width: OUT * 2, color: INK, join: 'round' });
  joints((x, y) => g.circle(x, y, half + OUT));
  g.fill(INK);
  for (const s of segs) g.poly(s.outline);
  joints((x, y) => g.circle(x, y, half));
  g.fill(ST[1]);
  const B = FT * BATTER;
  const P = PARAPET;
  const band = (s: Seg, d0: number, d1: number) => [...s.at(0, d0), ...s.at(s.l, d0), ...s.at(s.l, d1), ...s.at(0, d1)];
  // The battered faces: lit toward the light, shaded away.
  for (const s of segs) for (const side of [-1, 1]) if (sideLit(s, side) > 0) g.poly(band(s, side * (half - B), side * half));
  g.fill(ST[2]);
  for (const s of segs) for (const side of [-1, 1]) if (sideLit(s, side) <= 0) g.poly(band(s, side * (half - B), side * half));
  g.fill(mix(ST[1], PLUM, 0.22));
  // The walk, packed: one tone, the shade of the lit parapet across it.
  const w = half - B - P;
  for (const s of segs) g.poly(band(s, -w, w));
  joints((x, y) => g.circle(x, y, w));
  g.fill(WK);
  for (const s of segs) {
    const side = sideLit(s, 1) > 0 ? 1 : -1;
    g.poly(band(s, side * w, side * (w - FT * 0.7)));
  }
  g.fill({ color: PLUM, alpha: 0.2 });
  // The parapets: coping, rounded merlons, inked along the walk and along the top of the batter.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  for (const s of segs) {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) continue;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * w, side * (w + P)];
      g.poly([...s.at(a0, d0), ...s.at(a1, d0), ...s.at(a1, d1), ...s.at(a0, d1)]).fill(ST[2]);
      crenelRun(s.at, a0, a1, d0, d1, s.rnd, crenels, s.ruined, fallen);
      g.moveTo(...s.at(a0, d0)).lineTo(...s.at(a1, d0));
    }
  }
  g.stroke({ width: 1.8, color: INK });
  for (const s of segs) for (const side of [-1, 1]) g.moveTo(...s.at(0, side * (w + P))).lineTo(...s.at(s.l, side * (w + P)));
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.7 });
  crenelFill(g, crenels);
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 2, 0), 3, half * 0.7, s.rnd, 2));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 2, 0), 3, half * 0.7, s.rnd, 2));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (24 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (half + FT * 1.5)), 1, FT * 1.4, s.rnd, 1.8));
  }
  blocks(g, fallen, stoneTone(segs[0].rnd));
}
