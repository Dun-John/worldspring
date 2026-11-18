import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { BRASS, DARK_WOOD, INK, IRON, LINE, type Light, OUT, WOOD, castShadow, frame, oval, patch, quad, ring } from './common';
import { BURLAP } from './tomb';
import { barrel } from './home';
import { rock, screenAxes } from './under';

/** A stretch of track across a rail's square: `at(u, o)` is the point a fraction `u` along it from where it starts,
 * `o` squares off its middle line; `to` how far along it runs (1: to the far side) and `stop` whether it ends at a
 * buffer beam there. */
export type RailRun = { at: (u: number, o: number) => [number, number]; len: number; to: number; stop: boolean };

/** The stretches of track across a rail's square at `x`, `y` that join it to the rails beside it (`links`: north 1,
 * east 2, south 4, west 8): straight across between opposite sides, a quarter turn between two sides that meet
 * (round the corner between them, its middle line half a square out), the straight through a junction with a stub
 * to a branch's side, and from a lone neighbour most of the way across to a buffer beam. None: across, west to
 * east. */
export function railRuns(x: number, y: number, links: number): RailRun[] {
  const SIDE: [number, number][] = [[0, -1], [1, 0], [0, 1], [-1, 0]];
  const has = (i: number) => ((links >> i) & 1) === 1;
  const [cx, cy] = [x + 0.5, y + 0.5];
  // (Straight from side `a` toward the opposite side, its offsets to the left of the way it runs.)
  const straight = (a: number, to = 1, stop = false): RailRun => {
    const [sx, sy] = SIDE[a];
    const [dx, dy] = [-sx, -sy];
    return { at: (u, o) => [cx + sx * 0.5 + dx * u - dy * o, cy + sy * 0.5 + dy * u + dx * o], len: 1, to, stop };
  };
  const count = [0, 1, 2, 3].filter(has).length;
  if (count === 0) return [straight(3)];
  if (count === 1) return [straight([0, 1, 2, 3].find(has)!, 0.8, true)];
  const ns = has(0) && has(2);
  const ew = has(1) && has(3);
  if (count === 2 && !ns && !ew) {
    // A quarter turn round the corner where the two sides meet.
    const [a, b] = [0, 1, 2, 3].filter(has);
    const [kx, ky] = [cx + (SIDE[a][0] + SIDE[b][0]) * 0.5, cy + (SIDE[a][1] + SIDE[b][1]) * 0.5];
    const t0 = Math.atan2(-SIDE[b][1], -SIDE[b][0]);
    let t1 = Math.atan2(-SIDE[a][1], -SIDE[a][0]);
    if (t1 - t0 > Math.PI) t1 -= Math.PI * 2;
    if (t0 - t1 > Math.PI) t1 += Math.PI * 2;
    const turn = Math.sign(t1 - t0);
    // (Offsets to the left of the way it runs: out from the corner when it turns clockwise on screen.)
    return [{ at: (u, o) => { const t = t0 + (t1 - t0) * u; const r = 0.5 - turn * o; return [kx + Math.cos(t) * r, ky + Math.sin(t) * r]; }, len: Math.PI / 4, to: 1, stop: false }];
  }
  const runs: RailRun[] = [];
  if (ew) runs.push(straight(3));
  if (ns) runs.push(straight(0));
  // A branch off the straight: a stub from its side to the near rail.
  for (const i of [0, 1, 2, 3]) if (has(i) && !has((i + 2) % 4)) runs.push(straight(i, 0.3, false));
  return runs;
}

/** Rails: two iron rails on wooden sleepers in a strip of gravel, along each of the piece's stretches of track
 * (`railRuns`) so neighbours join, edge to edge; a buffer beam where the line ends. */
export function rails(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, links = 0) {
  const runs = railRuns(f.x, f.y, links);
  const pts = (r: RailRun, o: number, u0 = 0, u1 = r.to) => {
    const n = Math.max(2, Math.ceil(((u1 - u0) * r.len) / 0.06) + 1);
    return Array.from({ length: n }, (_, i) => r.at(u0 + ((u1 - u0) * i) / (n - 1), o));
  };
  const line = (ps: [number, number][]) => {
    g.moveTo(...ps[0]);
    for (const p of ps.slice(1)) g.lineTo(...p);
  };
  // (Unit steps along the run and across it at `u`.)
  const frame = (r: RailRun, u: number): [[number, number], [number, number]] => {
    const [p, q] = [r.at(u, 0), r.at(u, 1)];
    const [nx, ny] = [q[0] - p[0], q[1] - p[1]];
    return [[ny, -nx], [nx, ny]];
  };
  // A rect `a` along and `b` across, round the point `u` along, `o` off.
  const box = (r: RailRun, u: number, o: number, a: number, b: number, j: number) => {
    const [c, [tx, ty], [nx, ny]] = [r.at(u, o), ...frame(r, u)];
    const n = () => (rnd() - 0.5) * 2 * j;
    return [[-1, -1], [1, -1], [1, 1], [-1, 1]].flatMap(([s, t]) => [c[0] + tx * a * s + nx * b * t + n(), c[1] + ty * a * s + ny * b * t + n()]);
  };
  // The gravel bed (past the buffer a little), stones strewn on it.
  for (const r of runs) {
    const u1 = r.stop ? r.to + 0.06 : r.to === 0.3 ? 0.5 : r.to;
    g.poly([...pts(r, -0.36, 0, u1), ...pts(r, 0.36, 0, u1).reverse()].flat()).fill({ color: 0x5a554c, alpha: 0.35 });
    for (let i = 0; i < Math.round(14 * u1 * r.len); i++) g.circle(...r.at(rnd() * u1, (rnd() * 2 - 1) * 0.34), 0.01 + 0.01 * rnd()).fill({ color: 0x8a857a, alpha: 0.6 });
  }
  // Sleepers, every half square.
  for (const r of runs) {
    for (const u of [0.25, 0.75]) {
      if (u > r.to) continue;
      g.poly(box(r, u, 0, 0.07, 0.34, 0.006)).fill(DARK_WOOD[1]).stroke({ width: 0.018, color: INK });
      line([r.at(u - 0.02 / r.len, -0.3), r.at(u - 0.015 / r.len, 0.3)]);
      g.stroke({ width: 0.014, color: DARK_WOOD[0] });
    }
  }
  // The rails: inked iron, the edge toward the light lit, spikes into each sleeper.
  for (const r of runs) {
    const [, [nx, ny]] = frame(r, r.to / 2);
    const lit = nx * light[0] + ny * light[1] > 0 ? 1 : -1;
    for (const o of [-0.2, 0.2]) {
      line(pts(r, o));
      g.stroke({ width: 0.056, color: INK });
      line(pts(r, o));
      g.stroke({ width: 0.034, color: IRON[1] });
      line(pts(r, o + lit * 0.013));
      g.stroke({ width: 0.018, color: IRON[2] });
      for (const u of [0.25, 0.75]) if (u <= r.to) g.circle(...r.at(u, o + 0.05), 0.012).fill(IRON[0]);
    }
  }
  // Buffer beams: a baulk of timber across the rails' ends, iron plates where they meet it.
  for (const r of runs.filter((r) => r.stop)) {
    g.poly(box(r, r.to + 0.02, 0, 0.055, 0.33, 0.006)).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    line([r.at(r.to + 0.02 - 0.03, -0.3), r.at(r.to + 0.02 - 0.03, 0.3)]);
    g.stroke({ width: 0.014, color: WOOD[2] });
    for (const o of [-0.2, 0.2]) g.poly(box(r, r.to - 0.04, o, 0.025, 0.045, 0)).fill(IRON[0]).stroke({ width: 0.018, color: INK });
  }
}

/** Ore cart: a deep tapered iron-strapped tub (wider at the rim than the floor), its inside walls showing and
 * the near wall's shadow across the ore lying in the bottom, glinting gold; four wheels showing at its corners, a
 * push bar at the back and a coupling hook at the front. */
export function oreCart(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [hw, hh] = [0.33, 0.24];
  castShadow(g, [cx - hw, cy - hh, cx + hw, cy - hh, cx + hw, cy + hh, cx - hw, cy + hh], light, 0.4);
  // Wheels at the corners, poking out of the long sides.
  for (const [dx, dy] of [[-0.21, -1], [0.21, -1], [-0.21, 1], [0.21, 1]]) {
    const r = [cx + dx - 0.1, cy + dy * (hh + 0.035) - 0.035, cx + dx + 0.1, cy + dy * (hh + 0.035) + 0.035];
    g.roundRect(r[0], r[1], r[2] - r[0], r[3] - r[1], 0.03).fill(IRON[0]).stroke({ width: 0.018, color: INK });
    g.moveTo(r[0] + 0.03, (r[1] + r[3]) / 2).lineTo(r[2] - 0.03, (r[1] + r[3]) / 2).stroke({ width: 0.014, color: IRON[2] });
  }
  // Push bar at the back, coupling at the front.
  g.moveTo(cx - hw, cy - 0.14).lineTo(cx - hw - 0.1, cy - 0.14).lineTo(cx - hw - 0.1, cy + 0.14).lineTo(cx - hw, cy + 0.14).stroke({ width: 0.04, color: INK, join: 'round' });
  g.moveTo(cx - hw, cy - 0.14).lineTo(cx - hw - 0.1, cy - 0.14).lineTo(cx - hw - 0.1, cy + 0.14).lineTo(cx - hw, cy + 0.14).stroke({ width: 0.026, color: WOOD[1], join: 'round' });
  g.moveTo(cx + hw, cy).lineTo(cx + hw + 0.08, cy).stroke({ width: 0.032, color: INK, cap: 'round' });
  g.circle(cx + hw + 0.1, cy, 0.03).stroke({ width: 0.018, color: IRON[1] });
  // The tub: the rim, sloping sides in, the floor.
  g.roundRect(cx - hw, cy - hh, hw * 2, hh * 2, 0.05).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  const [iw, ih] = [hw - 0.1, hh - 0.1];
  const lit = (x: number, y: number) => (x * light[0] + y * light[1] > 0 ? WOOD[2] : WOOD[0]);
  for (const [ax, ay, bx2, by2, nx, ny] of [
    [-1, -1, 1, -1, 0, -1],
    [1, -1, 1, 1, 1, 0],
    [1, 1, -1, 1, 0, 1],
    [-1, 1, -1, -1, -1, 0],
  ]) {
    g.poly([cx + ax * (hw - 0.03), cy + ay * (hh - 0.03), cx + bx2 * (hw - 0.03), cy + by2 * (hh - 0.03), cx + bx2 * iw, cy + by2 * ih, cx + ax * iw, cy + ay * ih]).fill(lit(-nx, -ny));
  }
  g.rect(cx - iw, cy - ih, iw * 2, ih * 2).fill(0x2e2a26).stroke({ width: 0.018, color: INK });
  // Iron straps: round the rim, down each corner.
  g.roundRect(cx - hw + 0.015, cy - hh + 0.015, hw * 2 - 0.03, hh * 2 - 0.03, 0.04).stroke({ width: 0.022, color: IRON[0] });
  for (const [kx, ky] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
    g.moveTo(cx + kx * (hw - 0.02), cy + ky * (hh - 0.02)).lineTo(cx + kx * iw, cy + ky * ih).stroke({ width: 0.03, color: IRON[0] });
    g.circle(cx + kx * (hw - 0.035), cy + ky * (hh - 0.035), 0.01).fill(IRON[2]);
  }
  // The load, down in the bottom: lumps kept clear of the walls.
  const ORE = [0x3e3a36, 0x6a645c, 0x8e877c, 0xa8a090];
  for (let i = 0; i < 12; i++) {
    const sz = 0.03 + 0.02 * rnd();
    rock(g, cx + (rnd() * 2 - 1) * (iw - sz - 0.01), cy + (rnd() * 2 - 1) * (ih - sz - 0.01), sz, ORE, light, rnd, 5);
  }
  for (let i = 0; i < 6; i++) {
    const [x, y] = [cx + (rnd() * 2 - 1) * (iw - 0.03), cy + (rnd() * 2 - 1) * (ih - 0.03)];
    g.poly([x, y - 0.018, x + 0.011, y, x, y + 0.018, x - 0.011, y]).fill(0xf0c850);
  }
  // The walls on the light's side shade the bottom along their foot.
  const sh = 0.06;
  if (light[0] < 0) g.rect(cx - iw, cy - ih, sh, ih * 2).fill({ color: 0x000000, alpha: 0.35 });
  if (light[0] > 0) g.rect(cx + iw - sh, cy - ih, sh, ih * 2).fill({ color: 0x000000, alpha: 0.35 });
  if (light[1] < 0) g.rect(cx - iw, cy - ih, iw * 2, sh).fill({ color: 0x000000, alpha: 0.35 });
  if (light[1] > 0) g.rect(cx - iw, cy + ih - sh, iw * 2, sh).fill({ color: 0x000000, alpha: 0.35 });
  g.rect(cx - iw, cy - ih, iw * 2, ih * 2).stroke({ width: 0.016, color: INK });
}

/** Ore vein: a streak in the rock itself, not an object on it: broken, tapering ribbons in wall-rock tones that fade
 * out into the wall (no outline round the whole), running along the wall's edge and a little into it, with a few
 * chunky muted ore fragments set in it (rust-gold, verdigris or tarnished silver) and loose rubble at its foot. */
export function oreVein(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const ore = [
    [0x4a3418, 0x8a6a30, 0xb09450, 0xd8bc70],
    [0x26403a, 0x4a7868, 0x78a090, 0xa8d4bc],
    [0x383c42, 0x6e767e, 0x98a0a4, 0xc8ced0],
    [0x4a3418, 0x8a6a30, 0xb09450, 0xd8bc70],
  ][v % 4];
  const ROCK = [0x34302a, 0x48433a, 0x5c564b];
  const u0 = 0.04 + 0.12 * rnd();
  const u1 = 0.96 - 0.12 * rnd();
  // A ribbon along a wandering line from u0..u1: width tapers to nothing at both ends, d0 is its mean depth.
  const ribbon = (ua: number, ub: number, d0: number, wd: number, col: number, alpha: number) => {
    const n = 10;
    const hi: number[] = [];
    const lo: number[] = [];
    const ph = rnd() * 6;
    for (let i = 0; i <= n; i++) {
      const t = i / n;
      const u = ua + (ub - ua) * t;
      const c = d0 + 0.05 * Math.sin(ph + t * 5) + 0.03 * (rnd() - 0.5);
      const w = wd * Math.pow(Math.sin(Math.PI * t), 0.8) * (0.55 + 0.7 * rnd());
      hi.push(...at(u, c - w * 0.5));
      lo.unshift(...at(u, c + w * 0.5));
    }
    const pts: number[] = [...hi];
    for (let i = 0; i < lo.length; i += 2) pts.push(lo[i], lo[i + 1]);
    g.poly(pts).fill({ color: col, alpha });
  };
  // The rock round it, stained: wide faint patches reaching into the wall and out onto its face.
  ribbon(u0 - 0.04, u1 + 0.04, 0.02, 0.42, ROCK[0], 0.35);
  ribbon(u0, u1, 0.05, 0.3, ROCK[1], 0.55);
  // Broken streaks of ore-tinted rock, each tapering: two or three, apart in depth.
  const parts = 2 + Math.floor(rnd() * 2);
  for (let i = 0; i < parts; i++) {
    const ua = u0 + (u1 - u0) * (i / parts) + 0.02;
    const ub = u0 + (u1 - u0) * ((i + 1) / parts) - 0.02 + 0.05 * rnd();
    const d0 = 0.0 + 0.09 * rnd();
    ribbon(ua, ub, d0, 0.12 + 0.05 * rnd(), ore[0], 0.9);
    ribbon(ua + 0.03, ub - 0.03, d0 - 0.02, 0.06, ore[1], 0.9);
  }
  // The lit side of the streak: a thin pale rock line.
  ribbon(u0 + 0.05, u1 - 0.1, -0.04, 0.03, ROCK[2], 0.7);
  // Chunky ore fragments set in the streak: a few angular lumps, three tones, one small lit facet.
  const flecks = 3 + Math.floor(rnd() * 3);
  for (let i = 0; i < flecks; i++) {
    const [x, y] = at(u0 + 0.06 + (u1 - u0 - 0.12) * ((i + 0.3 + 0.4 * rnd()) / flecks), 0.0 + 0.1 * rnd());
    const r = 0.028 + 0.028 * rnd();
    const rot = rnd() * Math.PI;
    const chunk: number[] = [];
    for (let k = 0; k < 5; k++) {
      const a = rot + (k / 5) * Math.PI * 2;
      const rr = r * (0.75 + 0.5 * rnd());
      chunk.push(x + Math.cos(a) * rr, y + Math.sin(a) * rr * 0.85);
    }
    g.poly(chunk).fill(ore[1]).stroke({ width: LINE, color: ore[0], join: 'round' });
    g.poly([x - r * 0.1, y - r * 0.1, x + light[0] * r * 0.7, y - r * 0.1, x - r * 0.1, y + light[1] * r * 0.7]).fill(ore[2]);
    if (i === 0) g.circle(x + light[0] * r * 0.35, y + light[1] * r * 0.35, 0.014).fill(ore[3]);
  }
  // Loose rubble at the foot.
  for (let i = 0; i < 3; i++) {
    const [x, y] = at(0.2 + rnd() * 0.6, 0.2 + rnd() * 0.1);
    rock(g, x, y, 0.02 + 0.014 * rnd(), i === 1 ? ore : ROCK, light, rnd, 5);
  }
}

/** Mining tools laid side by side: a pickaxe (a curved two-pointed iron head across the top of a long haft), a
 * shovel (D-grip, a flat pointed spade blade) and a sledgehammer (a heavy block head), each a little askew. */
export function miningTools(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const haft = (a: [number, number], b: [number, number], w: number) => {
    g.moveTo(...a).lineTo(...b).stroke({ width: w + 0.025, color: INK, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: WOOD[1], cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w * 0.3, color: WOOD[2], cap: 'round' });
  };
  const tool = (ox: number, a: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    return (u: number, k: number): [number, number] => [x + ox + c * k - s * u, y + 0.5 + s * k + c * u];
  };
  // The pickaxe: u runs down the haft, k across.
  const K = tool(0.22, (rnd() - 0.5) * 0.25);
  castShadow(g, [...K(-0.34, -0.2), ...K(-0.34, 0.2), ...K(0.36, 0.03), ...K(0.36, -0.03)], light, 0.04);
  haft(K(-0.3, 0), K(0.38, 0), 0.04);
  for (const [w, col] of [
    [0.08, INK],
    [0.052, IRON[1]],
  ] as const) {
    g.moveTo(...K(-0.24, -0.2)).quadraticCurveTo(...K(-0.36, 0), ...K(-0.24, 0.2)).stroke({ width: w, color: col, cap: 'round' });
  }
  g.moveTo(...K(-0.28, -0.12)).quadraticCurveTo(...K(-0.355, 0), ...K(-0.28, 0.12)).stroke({ width: 0.015, color: IRON[2], cap: 'round' });
  g.poly([...K(-0.36, -0.05), ...K(-0.26, -0.05), ...K(-0.26, 0.05), ...K(-0.36, 0.05)]).fill(IRON[0]).stroke({ width: 0.018, color: INK });
  // The shovel.
  const S = tool(0.5, (rnd() - 0.5) * 0.2);
  haft(S(-0.32, 0), S(0.08, 0), 0.034);
  g.poly([...S(-0.4, -0.06), ...S(-0.34, -0.06), ...S(-0.34, 0.06), ...S(-0.4, 0.06)]).stroke({ width: 0.026, color: INK });
  g.moveTo(...S(-0.4, 0)).lineTo(...S(-0.34, 0)).stroke({ width: 0.02, color: WOOD[1] });
  const blade = [...S(0.06, -0.1), ...S(0.06, 0.1), ...S(0.32, 0.1), ...S(0.42, 0), ...S(0.32, -0.1)];
  castShadow(g, blade, light, 0.03);
  g.poly(blade).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...S(0.09, -0.06), ...S(0.09, 0.06), ...S(0.3, 0.06), ...S(0.36, 0), ...S(0.3, -0.06)]).fill(IRON[2]);
  g.moveTo(...S(0.07, 0)).lineTo(...S(0.22, 0)).stroke({ width: 0.016, color: IRON[0] });
  // The sledgehammer.
  const H = tool(0.79, (rnd() - 0.5) * 0.25);
  haft(H(-0.18, 0), H(0.36, 0), 0.034);
  const head = [...H(-0.32, -0.11), ...H(-0.32, 0.11), ...H(-0.16, 0.11), ...H(-0.16, -0.11)];
  castShadow(g, head, light, 0.08);
  g.poly(head).fill(IRON[0]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...H(-0.3, -0.09), ...H(-0.3, 0.06), ...H(-0.18, 0.06), ...H(-0.18, -0.09)]).fill(IRON[1]);
  g.moveTo(...H(-0.3, -0.09)).lineTo(...H(-0.3, 0.06)).stroke({ width: 0.014, color: IRON[2] });
}

/** Hanging lantern, seen from high up (about 60 degrees, like the urn): a square iron lantern turned corner-on, its
 * pyramid cap and ring on top, the two near glass sides glowing, a short run of chain above, a pool of warm light on
 * the floor under it. */
export function hangingLantern(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  void rnd;
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 5; i++) g.circle(cx, cy, 0.4 - i * 0.06).fill({ color: 0xf8c860, alpha: 0.06 });
  const { pt } = screenAxes(light);
  const [hk, dk] = [0.5, 0.87];
  // (x across, y depth, h height) to the grid.
  const Q = (x: number, y: number, h: number) => pt(cx, cy, x, h * hk + y * dk);
  const sq = (s: number, h: number) => [Q(0, s, h), Q(s, 0, h), Q(0, -s, h), Q(-s, 0, h)];
  const s = 0.1;
  const [bot, top] = [sq(s * 1.05, -0.12), sq(s, 0.08)];
  const flat = (pts: [number, number][]) => pts.flat();
  // Base, the near glass sides, iron corner posts, the cap.
  g.poly(flat(bot)).fill(IRON[0]).stroke({ width: 0.018, color: INK, join: 'round' });
  for (const [i, j, col] of [
    [3, 2, 0xfff0b0],
    [2, 1, 0xf0c060],
  ] as const) {
    g.poly(flat([bot[i], bot[j], top[j], top[i]])).fill(col).stroke({ width: 0.018, color: INK, join: 'round' });
  }
  for (const i of [3, 2, 1]) g.moveTo(...bot[i]).lineTo(...top[i]);
  g.stroke({ width: 0.016, color: IRON[0], cap: 'round' });
  const apex = Q(0, 0, 0.18);
  const lit = [IRON[1], IRON[0], IRON[1], IRON[2]];
  for (let i = 0; i < 4; i++) g.poly(flat([top[i], top[(i + 1) % 4], apex])).fill(lit[i]);
  g.poly(flat(top)).stroke({ width: 0.016, color: INK, join: 'round' });
  for (let i = 0; i < 4; i++) g.moveTo(...top[i]).lineTo(...apex);
  g.stroke({ width: 0.016, color: INK, alpha: 0.8 });
  // The ring on top and the chain above it.
  const [ax, ay] = apex;
  g.poly(oval(ax, ay, 0.028, 0.025, 0, 12)).stroke({ width: 0.022, color: INK });
  g.poly(oval(ax, ay, 0.028, 0.025, 0, 12)).stroke({ width: 0.014, color: IRON[1] });
  for (let i = 1; i <= 3; i++) {
    const [lx, ly] = Q(0, 0, 0.18 + i * 0.045);
    g.poly(oval(lx, ly, 0.01, 0.015, 0, 8)).stroke({ width: 0.014, color: IRON[0] });
  }
}

/** Blasting powder kegs: three small kegs standing together, a red flame painted on each lid, grainy black powder
 * spilled by them, a tarred fuse running from one keg's bung across the floor to a coil, its end frayed. */
export function powderKegs(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  // Spilled powder: a grainy black drift.
  for (let i = 0; i < 40; i++) {
    const [x, y] = [cx + 0.22 + (rnd() - 0.5) * 0.24 * (1 + rnd()), cy + 0.3 + (rnd() - 0.5) * 0.14];
    g.circle(x, y, 0.008 + 0.01 * rnd()).fill(i % 4 ? 0x1a1816 : 0x4a4640);
  }
  const kegs: [number, number][] = [
    [-0.17, -0.14],
    [0.17, -0.12],
    [0.02, 0.15],
  ];
  for (const [dx, dy] of kegs) castShadow(g, ring(cx + dx, cy + dy, 0.17, rnd, 0.005, 14), light, 0.35);
  for (const [dx, dy] of kegs) {
    const [x, y] = [cx + dx, cy + dy];
    barrel(g, x, y, 0.17, light, rnd, LINE);
    g.poly([x, y - 0.075, x + 0.04, y + 0.0, x + 0.035, y + 0.045, x, y + 0.06, x - 0.035, y + 0.045, x - 0.04, y]).fill(0xc0301c).stroke({ width: 0.018, color: INK });
    g.poly([x, y - 0.02, x + 0.017, y + 0.02, x, y + 0.04, x - 0.017, y + 0.02]).fill(0xf0a030);
  }
  // The fuse: from the front keg's bung down over its side and across the floor to a coil.
  const [bx, by] = [cx + 0.02 + 0.17 * 0.3, cy + 0.15 - 0.17 * 0.1];
  const [kx, ky] = [cx - 0.3, cy + 0.36];
  const path = () => g.moveTo(bx, by).bezierCurveTo(bx - 0.02, by + 0.2, kx + 0.2, ky + 0.06, kx + 0.09, ky);
  const coil = () => {
    g.moveTo(kx + 0.09, ky);
    for (let i = 1; i <= 40; i++) {
      const t = (i / 40) * Math.PI * 2 * 2.5;
      const r = 0.09 - (i / 40) * 0.06;
      g.lineTo(kx + Math.cos(t) * r, ky + Math.sin(t) * r * 0.8);
    }
  };
  for (const [w, c] of [
    [0.03, INK],
    [0.016, 0x4a3420],
  ] as const) {
    path();
    g.stroke({ width: w, color: c, cap: 'round' });
    coil();
    g.stroke({ width: w, color: c, cap: 'round', join: 'round' });
  }
  const [ex, ey] = [kx + Math.cos(Math.PI * 5) * 0.03, ky];
  for (const b of [-0.5, 0, 0.5]) g.moveTo(ex, ey).lineTo(ex + Math.cos(Math.PI + b) * 0.03, ey + Math.sin(Math.PI + b) * 0.03);
  g.stroke({ width: 0.014, color: 0x8a6a40, cap: 'round' });
}

/** Bedroll: a ground pad with a wool blanket over it, turned back at the head, a rolled cloak for a pillow, a strap
 * and buckle lying loose; rumpled. */
export function bedroll(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const wool = [
    [0x4a5a3a, 0x6a7a4a, 0x8a9a62],
    [0x6a3a2a, 0x8a4e36, 0xa86a48],
    [0x3a4a6a, 0x4e6288, 0x6a80a8],
    [0x6a5a3a, 0x8a7650, 0xa89068],
  ][v % 4];
  const pad = quad(rect(0.14, 0.14, len - 0.1, 0.86), rnd, 0.02);
  castShadow(g, pad, light, 0.05);
  g.poly(pad).fill(0x8a7650).stroke({ width: LINE, color: INK, join: 'round' });
  // Blanket, rumpled, turned back at the head end.
  const bl: number[] = [];
  for (const [u, s] of [[0.62, 0.17], [len * 0.6, 0.15], [len - 0.16, 0.18], [len - 0.14, 0.5], [len - 0.17, 0.83], [len * 0.6, 0.86], [0.62, 0.84]]) bl.push(...at(u + (rnd() - 0.5) * 0.03, s + (rnd() - 0.5) * 0.03));
  g.poly(bl).fill(wool[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const s of [0.36, 0.64]) g.moveTo(...at(0.66, s)).lineTo(...at(len - 0.18, s));
  g.stroke({ width: 0.02, color: wool[0] });
  g.moveTo(...at(0.66, 0.5)).lineTo(...at(len - 0.18, 0.5)).stroke({ width: 0.014, color: wool[2] });
  g.moveTo(...at(1.1, 0.2)).quadraticCurveTo(...at(1.18, 0.5), ...at(1.08, 0.8)).stroke({ width: 0.016, color: wool[0] });
  const turn = quad(rect(0.56, 0.16, 0.74, 0.84), rnd, 0.015);
  g.poly(turn).fill(wool[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(0.65, 0.2)).lineTo(...at(0.65, 0.8)).stroke({ width: 0.014, color: wool[1] });
  // The rolled cloak for a pillow.
  const pil = quad(rect(0.18, 0.24, 0.48, 0.76), rnd, 0.02);
  g.poly(pil).fill(0x6a5e4c).stroke({ width: LINE, color: INK, join: 'round' });
  for (const s of [0.36, 0.5, 0.64]) g.moveTo(...at(0.2, s)).quadraticCurveTo(...at(0.33, s + 0.03), ...at(0.46, s));
  g.stroke({ width: 0.014, color: 0x4a4234 });
  // A loose strap.
  g.moveTo(...at(len - 0.5, 0.88)).quadraticCurveTo(...at(len - 0.35, 0.96), ...at(len - 0.18, 0.9)).stroke({ width: 0.03, color: 0x4a3020, cap: 'round' });
  g.rect(...at(len - 0.2, 0.88), 0.04, 0.04).stroke({ width: 0.014, color: BRASS });
}

/** Grain sacks: three plump burlap sacks, each gathered at the neck and tied with cord, the cloth pulled into creases
 * running in to the tie; the weave and a stencilled mark showing; one tipped over with grain spilled from it. */
export function grainSacks(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx + 0.26, cy + 0.3, 0.13, 0.08, rnd, 0.3, 12)).fill(0xd8b868).stroke({ width: 0.018, color: INK });
  for (let i = 0; i < 16; i++) g.poly(oval(cx + 0.14 + rnd() * 0.3, cy + 0.2 + rnd() * 0.2, 0.014, 0.008, rnd() * 3, 6)).fill(0xc8a050);
  const sack = (x: number, y: number, rx: number, ry: number, a: number, tipped: boolean) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const body = oval(x, y, rx, ry, a, 20).map((q) => q + (rnd() - 0.5) * 0.02);
    castShadow(g, body, light, 0.3);
    g.poly(body).fill(BURLAP[0]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(oval(x + light[0] * 0.03, y + light[1] * 0.03, rx * 0.84, ry * 0.82, a, 16)).fill(BURLAP[1]);
    g.poly(oval(x + light[0] * 0.07, y + light[1] * 0.07, rx * 0.42, ry * 0.36, a, 12)).fill({ color: BURLAP[2], alpha: 0.6 });
    // Where the neck is: the middle on a standing sack, one end on the tipped one.
    const [nx, ny] = tipped ? [x + c * rx * 0.9, y + s * rx * 0.9] : [x - light[0] * 0.015, y - light[1] * 0.015];
    // Creases pulled in to the tie.
    for (let i = 0; i < 7; i++) {
      const b = (i / 7) * Math.PI * 2 + rnd() * 0.3;
      const [ex, ey] = [x + (Math.cos(b) * c - Math.sin(b) * s * (ry / rx)) * rx * 0.85, y + (Math.cos(b) * s + Math.sin(b) * c * (ry / rx)) * rx * 0.85];
      if (tipped && (ex - nx) * c + (ey - ny) * s > -0.02) continue;
      g.moveTo(nx, ny).quadraticCurveTo((nx + ex) / 2 + (rnd() - 0.5) * 0.04, (ny + ey) / 2 + (rnd() - 0.5) * 0.04, ex, ey);
    }
    g.stroke({ width: 0.014, color: BURLAP[0], cap: 'round' });
    // The gathered neck: a small rumpled tuft past the cord.
    const tuft = tipped ? [nx + c * 0.06, ny + s * 0.06] : [nx, ny];
    g.poly(patch(tuft[0], tuft[1], 0.05, 0.05, rnd, 0.45, 9)).fill(BURLAP[2]).stroke({ width: 0.018, color: INK, join: 'round' });
    g.circle(nx, ny, 0.028).stroke({ width: 0.018, color: 0x5a3a20 });
    g.moveTo(nx, ny).lineTo(nx + 0.05, ny + 0.06).stroke({ width: 0.014, color: 0x5a3a20, cap: 'round' });
  };
  sack(cx - 0.17, cy - 0.15, 0.18, 0.17, 0.2, false);
  sack(cx + 0.18, cy - 0.12, 0.17, 0.16, -0.3, false);
  g.poly(oval(cx - 0.24, cy - 0.04, 0.05, 0.03, 0.2, 10)).stroke({ width: 0.014, color: 0x5a3a20, alpha: 0.8 });
  sack(cx - 0.06, cy + 0.2, 0.24, 0.14, 0.3, true);
}
