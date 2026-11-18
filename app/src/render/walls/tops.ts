import type { Graphics } from 'pixi.js';
import { FT, PLUM, INK, mix, WOOD } from '../bridges';
import { PX } from '../scale';
import { type At, IRON, LINE, PARAPET, ST, type Side, WK, breach, brick, clipLine, crenelFill, crenelRun, lit, marksFill, merlonMark, notchTone, rhythm, rubble, sidesOf } from './base';

/**
 * A flat top's floor inside a convex outline: flags in courses along its longest side, their
 * joints staggered course to course (a tower top's).
 */
export function flagFloor(g: Graphics, pts: number[], rnd: () => number) {
  g.poly(pts).fill(WK);
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
  if (brick()) g.stroke({ width: LINE * 0.8, color: ST[3], alpha: 0.85 });
  else g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
}

/**
 * Battlements round a convex outline (a keep's, a flat-topped building's), in the curtain's
 * style: inside the outline a band of coping lit on its edge toward the light, crenels notched
 * through it, inked along the walk and round the outline; the shade of the lit sides' parapets
 * on what lies inside; `walk` px of wall walk inside the parapet given, its flags' joints across
 * it. Sides in `open` (by index) have none (where a higher piece stands against them); sides in
 * `plain` a coping with no crenels (a wall between two flat tops: one wall, not a fence); a ruin's merlons
 * gone here and there, their stones to `fallen`.
 */
export function parapetRing(g: Graphics, pts: number[], rnd: () => number, walk = 0, open: number[] = [], plain: number[] = [], ruined = false, fallen: number[][] = []) {
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
    g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  }
  for (const s of sides) if (s.toLight) g.poly(band(s, P, P + FT * 0.6));
  g.fill({ color: PLUM, alpha: 0.25 });
  for (const s of sides) g.poly(band(s, 0, P));
  g.fill(ST[2]);
  for (const s of sides) {
    const e = s.toLight ? FT * 0.3 : P - FT * 0.3;
    g.moveTo(...s.at(s.k0 * e, e)).lineTo(...s.at(s.len - s.k1 * e, e));
  }
  g.stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8 });
  const crenels: number[][] = [];
  for (const s of sides) if (!plain.includes(s.i)) crenelRun(s.at, s.k0 * P, s.len - s.k1 * P, P, 0, rnd, crenels, ruined, fallen);
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
    g.poly(hq(a, a + hl * 0.21, -hw + 1.5, hw - 1.5)).fill(mix(ST[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
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
export function roundTop(g: Graphics, x: number, y: number, r: number, rnd: () => number, ruined = false, fallen: number[][] = [], smooth = false) {
  const ring: number[] = [];
  const n = 22;
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const rr = r * (1 + (rnd() - 0.5) * 0.035);
    ring.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr);
  }
  // (`smooth`, a keep's turret or a tower's top that is no wall's: a true circle, so its coping joins the walls it stands on.)
  if (smooth) g.circle(x, y, r).fill(ST[1]).stroke({ width: 3, color: INK });
  else g.poly(ring).fill(ST[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags inside the parapet, their joints, the parapet's shade on the lit side.
  const rf = r - PARAPET;
  g.circle(x, y, rf).fill(WK);
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
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
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
  const [rm0, rm1, rc0, rc1] = rhythm();
  const pitch = FT * (rm0 + rm1 / 2 + rc0 + rc1 / 2);
  const m = Math.max(6, Math.round((Math.PI * 2 * (r - PARAPET / 2)) / pitch));
  const step = (Math.PI * 2) / m;
  // (The crenel's share of each step, and a merlon's marks between: `at` round the ring, `a` along at mid-parapet.)
  const share = Math.min(0.5, (FT * (rc0 + rc1 / 2)) / pitch);
  const rm = r - PARAPET / 2;
  const at: At = (a, d) => [x + Math.cos(a / rm) * d, y + Math.sin(a / rm) * d];
  const lost = ruined ? rnd() * Math.PI * 2 : NaN;
  const [ri, ro] = [rf + 1.5, r - 1.5];
  g.circle(x, y, (rf + r) / 2).stroke({ width: r - rf, color: ST[2] });
  const notch = (t0: number, t1: number) => {
    const q: number[] = [];
    for (const t of [t0, (t0 + t1) / 2, t1]) q.push(x + Math.cos(t) * ri, y + Math.sin(t) * ri);
    for (const t of [t1, (t0 + t1) / 2, t0]) q.push(x + Math.cos(t) * ro, y + Math.sin(t) * ro);
    g.poly(q);
  };
  for (let k = 0; k < m; k++) {
    const t0 = k * step + turn;
    const gone = ruined && (Math.abs(Math.atan2(Math.sin(t0 - lost), Math.cos(t0 - lost))) < 0.8 || rnd() < 0.25);
    if (gone && rnd() < 0.5) fallen.push(...rubble(x + Math.cos(t0) * (rf - FT), y + Math.sin(t0) * (rf - FT), 1, FT * 0.6, rnd));
    notch(gone ? t0 - step * 0.3 : t0 + step * (0.95 - share), t0 + step * 0.95);
    if (!gone) merlonMark(at, (t0 - step * 0.05) * rm, (t0 + step * (0.95 - share)) * rm, rf, r);
  }
  g.fill(notchTone()).stroke({ width: LINE * 0.8, color: INK, alpha: 0.9, join: 'round' });
  marksFill(g);
  g.circle(x, y, rf).stroke({ width: 1.8, color: INK });
  // The ring's lit and shaded faces.
  g.moveTo(x + Math.cos(Math.PI * 0.9) * (r - 2), y + Math.sin(Math.PI * 0.9) * (r - 2));
  g.arc(x, y, r - 2, Math.PI * 0.9, Math.PI * 1.6);
  g.stroke({ width: 2.5, color: ST[3], alpha: 0.7 });
  g.moveTo(x + Math.cos(-Math.PI * 0.1) * (r - 2), y + Math.sin(-Math.PI * 0.1) * (r - 2));
  g.arc(x, y, r - 2, -Math.PI * 0.1, Math.PI * 0.6);
  g.stroke({ width: 3, color: PLUM, alpha: 0.3 });
  if (ruined) breach(g, x, y, r, lost, Math.min(14, (r / FT) * 1.3), rnd, fallen);
  return lost;
}

/**
 * A round wall walk at (x, y) from `r1` in to `r0` (a shell keep's, inside its parapet): flags,
 * their joints across it, the shade of the parapet on the lit side.
 */
export function roundWalk(g: Graphics, x: number, y: number, r0: number, r1: number, rnd: () => number) {
  g.circle(x, y, (r0 + r1) / 2).stroke({ width: r1 - r0, color: WK });
  const rm = (r0 + r1) / 2;
  for (let t = rnd() * 0.2; t < Math.PI * 2 - FT / rm; t += (FT * (2.4 + rnd() * 1.4)) / rm) {
    const t1 = t + ((rnd() - 0.5) * FT * 0.4) / rm;
    g.moveTo(x + Math.cos(t) * r0, y + Math.sin(t) * r0).lineTo(x + Math.cos(t1) * r1, y + Math.sin(t1) * r1);
  }
  g.stroke({ width: 1.3, color: ST[0], alpha: 0.6 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (r1 - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (r1 - FT * 0.3));
  g.arc(x, y, r1 - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
}

/** A banner streaming east on the wind from a pole at (x, y). */
export function banner(g: Graphics, x: number, y: number) {
  g.moveTo(x, y).lineTo(x + PX * 0.2, y - PX * 1.8).stroke({ width: 2.5, color: 0x3b2f25 });
  g.poly([x + PX * 0.2, y - PX * 1.8, x + PX * 1.7, y - PX * 1.5, x + PX * 1.4, y - PX * 1.25, x + PX * 1.75, y - PX * 0.95, x + PX * 0.15, y - PX * 1.05])
    .fill(0x9b2226)
    .stroke({ width: 1.5, color: INK });
}
