import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { candle } from './sacred';
import { GODS } from './tables';
import { END_GAP, STONE, WALL_GAP } from './home';
import { INK, IRON, LINE, type Light, OUT, board, castShadow, frame, glowPool, oval, quad, ring, shade } from './common';
import { OAK, type Pt, inkLine } from './hall';
import { screenAxes } from './under';

/** A god's emblem (look `v`), `r` squares across about (cx, cy), its top toward `up` (a unit step): the sun's
 * disc and rays, the dawn's half sun over the horizon, the moon's crescent, the sea's trident over a wave, the
 * storm's bolt, the forge's hammer over its anvil, the harvest's sheaf, the Watcher's eye under seven stars;
 * in `color` with an ink line. */
function emblem(g: Graphics, v: number, cx: number, cy: number, r: number, up: Pt, color: number) {
  const [ux, uy] = up;
  const P = (a: number, b: number): Pt => [cx + (-uy * a + ux * b) * r, cy + (ux * a + uy * b) * r];
  const ink = { width: Math.max(0.008, 0.06 * r), color: INK, join: 'round' as const };
  const poly = (pts: Pt[]) => g.poly(pts.flat()).fill(color).stroke(ink);
  const disc = (a: number, b: number, rr: number, from = 0, to = Math.PI * 2, n = 16) => {
    const pts: Pt[] = [];
    for (let i = 0; i <= n; i++) {
      const t = from + ((to - from) * i) / n;
      pts.push(P(a + Math.cos(t) * rr, b + Math.sin(t) * rr));
    }
    return pts;
  };
  switch (v % GODS.length) {
    case 0: {
      // Rays round a disc, long and short by turns (a little uneven, as cut by hand).
      const rays: Pt[] = [];
      for (let i = 0; i < 16; i++) {
        const t = (i / 16) * Math.PI * 2;
        const rr = i % 2 ? 0.34 : i % 4 ? 0.44 : 0.5;
        rays.push(P(Math.cos(t) * rr, Math.sin(t) * rr));
      }
      poly(rays);
      poly(disc(0, 0, 0.24));
      break;
    }
    case 1: {
      // The half sun on the horizon, its rays fanning up.
      const fan: Pt[] = [P(-0.46, -0.08)];
      for (let i = 0; i <= 8; i++) {
        const t = Math.PI * (i / 8);
        const rr = i % 2 ? 0.32 : 0.48;
        fan.push(P(Math.cos(t) * -rr, -0.08 + Math.sin(t) * rr));
      }
      fan.push(P(0.46, -0.08));
      poly(fan);
      poly(disc(0, -0.08, 0.22, 0, Math.PI, 10));
      poly([P(-0.5, -0.12), P(0.5, -0.12), P(0.5, -0.2), P(-0.5, -0.2)]);
      break;
    }
    case 2: {
      // A crescent, its horns up and to one side.
      const pts: Pt[] = [];
      for (let i = 0; i <= 14; i++) pts.push(P(Math.cos(0.6 + (i / 14) * 4.6) * 0.42, Math.sin(0.6 + (i / 14) * 4.6) * 0.42));
      for (let i = 14; i >= 0; i--) pts.push(P(0.16 + Math.cos(0.75 + (i / 14) * 4.3) * 0.32, 0.06 + Math.sin(0.75 + (i / 14) * 4.3) * 0.32));
      poly(pts);
      break;
    }
    case 3: {
      // A trident over a wave.
      poly([P(-0.04, -0.3), P(0.04, -0.3), P(0.04, 0.22), P(-0.04, 0.22)]);
      poly([P(-0.26, 0.16), P(0.26, 0.16), P(0.26, 0.24), P(0.04, 0.26), P(0.04, 0.46), P(0, 0.52), P(-0.04, 0.46), P(-0.04, 0.26), P(-0.26, 0.24)]);
      for (const a of [-0.26, 0.26]) poly([P(a - 0.04, 0.2), P(a + 0.04, 0.2), P(a + 0.03, 0.42), P(a, 0.48), P(a - 0.03, 0.42)]);
      const wave: Pt[] = [];
      for (let i = 0; i <= 12; i++) wave.push(P(-0.48 + (0.96 * i) / 12, -0.34 + 0.06 * Math.sin((i / 12) * Math.PI * 3)));
      for (let i = 12; i >= 0; i--) wave.push(P(-0.48 + (0.96 * i) / 12, -0.46 + 0.06 * Math.sin((i / 12) * Math.PI * 3)));
      poly(wave);
      break;
    }
    case 4:
      // A bolt, zigzag.
      poly([P(0.1, 0.5), P(-0.22, 0.04), P(-0.02, 0.04), P(-0.14, -0.5), P(0.24, 0.0), P(0.04, 0.0), P(0.2, 0.5)]);
      break;
    case 5:
      // A hammer over an anvil.
      poly([P(-0.42, -0.2), P(0.42, -0.2), P(0.3, -0.08), P(0.12, -0.08), P(0.16, -0.32), P(0.26, -0.44), P(-0.26, -0.44), P(-0.16, -0.32), P(-0.12, -0.08), P(-0.3, -0.08)]);
      poly([P(-0.05, -0.04), P(0.05, -0.04), P(0.12, 0.46), P(0.04, 0.48)]);
      poly([P(-0.08, 0.34), P(0.28, 0.42), P(0.3, 0.56), P(-0.1, 0.5)]);
      break;
    case 6:
      // A sheaf: three stalks fanning up, their ears, a band round them.
      for (const k of [-1, 0, 1]) {
        const [a0, a1] = [k * 0.08, k * 0.28];
        poly([P(a0 - 0.025, -0.46), P(a0 + 0.025, -0.46), P(a1 + 0.025, 0.12), P(a1 - 0.025, 0.12)]);
        g.poly(oval(...P(a1, 0.3), 0.08 * r, 0.2 * r, Math.atan2(P(0, 1)[1] - cy, P(0, 1)[0] - cx) + (Math.PI / 2) * 0 - k * 0.3, 10)).fill(color).stroke(ink);
      }
      poly([P(-0.16, -0.12), P(0.16, -0.12), P(0.16, -0.22), P(-0.16, -0.22)]);
      break;
    default: {
      // The Watcher's eye, seven stars over it.
      const eye: Pt[] = [];
      for (let i = 0; i <= 10; i++) eye.push(P(-0.46 + (0.92 * i) / 10, -0.16 + 0.24 * Math.sin((i / 10) * Math.PI)));
      for (let i = 10; i >= 0; i--) eye.push(P(-0.46 + (0.92 * i) / 10, -0.16 - 0.2 * Math.sin((i / 10) * Math.PI)));
      poly(eye);
      g.poly(disc(0, -0.14, 0.13).flat()).fill(INK);
      for (let i = 0; i < 7; i++) {
        const t = Math.PI * (0.14 + (0.72 * i) / 6);
        g.circle(...P(Math.cos(t) * 0.42, 0.06 + Math.sin(t) * 0.36), 0.055 * r).fill(color).stroke({ ...ink, width: ink.width * 0.7 });
      }
    }
  }
}

/** The god's statue on its plinth, seen from above and over life size (a monument): a long shadow, a stepped
 * plinth of the god's stone, the god's emblem on its front; on it the figure (a robe about broad shoulders, the
 * head) in the god's stone or metal, with what marks the god: the sun's rayed halo, the dawn's half sun rising
 * behind the head, the moon's veil and crescent, the sea's trident, the storm's bolt held high, the forge's
 * hammer, the harvest's sheaf, the Watcher's hood and his staff with its eye. A wide plinth (2 squares) has an
 * offering bowl at each end. */
export function godStatue(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const god = GODS[v % GODS.length];
  const outer = rect(0.06, Math.max(0.06, WALL_GAP - 0.1), len - 0.06, wid - 0.04);
  castShadow(g, quad(outer, rnd, 0), light, 0.85);
  board(g, outer, god.stone.map((c) => shade(c, 0.78)), light, rnd, true, 0.05, 0);
  const [ox0, oy0, ox1, oy1] = outer;
  board(g, [ox0 + 0.09, oy0 + 0.09, ox1 - 0.09, oy1 - 0.09], god.stone.map((c) => shade(c, 0.92)), light, rnd, true, 0.04, 0);
  // (The way toward the wall and toward the room, in grid axes.)
  const [wx, wy] = [at(0, 0)[0] - at(0, 1)[0], at(0, 0)[1] - at(0, 1)[1]];
  const up: Pt = [wx, wy];
  const front: Pt = [-wx, -wy];
  emblem(g, v, ...at(len / 2, wid - 0.13), 0.2, up, god.accent);
  if (len >= 2) {
    for (const u of [0.32, len - 0.32]) {
      const [bx, by] = at(u, wid - 0.3);
      g.circle(bx, by, 0.11).fill(shade(god.metal[1], 0.9)).stroke({ width: LINE, color: INK });
      g.circle(bx, by, 0.07).fill(shade(god.metal[0], 0.9));
      g.circle(bx + light[0] * 0.03, by + light[1] * 0.03, 0.03).fill(god.accent);
    }
  }
  const t = v % GODS.length === 5 || v % GODS.length === 3 ? god.metal : god.stone;
  const [cx, cy] = at(len / 2, 0.46);
  const ink = { width: OUT, color: INK, join: 'round' as const };
  const S = 1.18;
  const T = (a: number, b: number): Pt => [cx + (-wy * a + front[0] * b) * S, cy + (wx * a + front[1] * b) * S];
  // (Behind the head, toward the wall: a halo, the rising half sun, a crescent.)
  const k = v % GODS.length;
  if (k === 0 || k === 1) {
    const rays: Pt[] = [];
    const n = k === 0 ? 18 : 11;
    for (let i = 0; i < n; i++) {
      const th = k === 0 ? (i / n) * Math.PI * 2 : Math.PI + (Math.PI * i) / (n - 1);
      const rr = i % 2 ? 0.3 : 0.4;
      rays.push(T(Math.cos(th) * rr, -0.08 + Math.sin(th) * rr * (k === 0 ? 1 : 0.9)));
    }
    if (k === 1) rays.push(T(0.38, -0.08), T(-0.38, -0.08));
    g.poly(rays.flat()).fill(GODS[k].metal[1]).stroke(ink);
    g.poly(rays.flat().map((c, i) => (i % 2 ? cy + (c - cy) * 0.78 : cx + (c - cx) * 0.78))).fill({ color: GODS[k].metal[2], alpha: 0.6 });
  }
  if (k === 2) {
    const cres: Pt[] = [];
    for (let i = 0; i <= 12; i++) cres.push(T(Math.cos(Math.PI + (Math.PI * i) / 12) * 0.28, -0.12 + Math.sin(Math.PI + (Math.PI * i) / 12) * 0.2));
    for (let i = 12; i >= 0; i--) cres.push(T(Math.cos(Math.PI + (Math.PI * i) / 12) * 0.22, -0.16 + Math.sin(Math.PI + (Math.PI * i) / 12) * 0.1));
    g.poly(cres.flat()).fill(god.metal[2]).stroke(ink);
  }
  // A plain robe round the shoulders, broad shoulders across it.
  g.poly(ring(cx, cy, 0.31 * S, rnd, 0.01, 20)).fill(t[0]).stroke(ink);
  g.poly(ring(cx + light[0] * 0.02, cy + light[1] * 0.02, 0.26 * S, rnd, 0.008, 20)).fill(t[1]);
  const [sx, sy] = [T(-0.33, 0.02), T(0.33, 0.02)];
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.21 * S, color: INK, cap: 'round' });
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.16 * S, color: t[1], cap: 'round' });
  g.moveTo(sx[0] + light[0] * 0.02, sx[1] + light[1] * 0.03).lineTo(sy[0] + light[0] * 0.02, sy[1] + light[1] * 0.03).stroke({ width: 0.04, color: t[2], cap: 'round', alpha: 0.7 });
  // What it holds, over the robe.
  const held = (pts: Pt[], w: number, c: number) => inkLine(g, pts, w, c);
  if (k === 3) {
    held([T(0.3, 0.22), T(-0.16, -0.36)], 0.035, god.metal[1]);
    g.poly([T(-0.3, -0.3), T(-0.02, -0.42), T(-0.06, -0.48), T(-0.34, -0.36)].flat()).fill(god.metal[1]).stroke(ink);
    for (const p of [T(-0.36, -0.36), T(-0.2, -0.46), T(-0.04, -0.5)]) held([p, [p[0] + (wx * 0.1), p[1] + (wy * 0.1)]], 0.028, god.metal[2]);
  }
  if (k === 4) {
    const bolt: Pt[] = [T(0.26, 0.04), T(0.44, -0.18), T(0.34, -0.2), T(0.5, -0.44), T(0.28, -0.22), T(0.38, -0.2), T(0.22, 0.02)];
    g.poly(bolt.flat()).fill(god.accent).stroke(ink);
  }
  if (k === 5) {
    held([T(0.28, 0.2), T(0.2, -0.26)], 0.04, OAK[1]);
    g.poly([T(0.06, -0.24), T(0.36, -0.3), T(0.38, -0.18), T(0.08, -0.12)].flat()).fill(IRON[1]).stroke(ink);
  }
  if (k === 6) {
    for (const a of [-0.1, 0, 0.1]) held([T(a * 0.6, 0.24), T(a * 1.6, -0.1)], 0.026, GODS[6].metal[1]);
    for (const a of [-0.1, 0, 0.1]) g.poly(oval(...T(a * 1.9, -0.17), 0.05, 0.09, Math.atan2(front[1], front[0]) + Math.PI / 2 + a * 2, 10)).fill(GODS[6].accent).stroke({ width: LINE, color: INK });
    held([T(-0.09, 0.1), T(0.09, 0.1)], 0.03, GODS[6].cloth[1]);
  }
  if (k === 7) {
    held([T(0.3, 0.3), T(0.3, -0.36)], 0.032, god.metal[1]);
    const [ex, ey] = T(0.3, -0.4);
    g.circle(ex, ey, 0.08).fill(god.accent).stroke(ink);
    g.circle(ex, ey, 0.035).fill(INK);
  }
  // The head (hooded or veiled for the moon and the Watcher).
  if (k === 2 || k === 7) {
    g.poly(oval(...T(0, -0.03), 0.2 * S, 0.24 * S, Math.atan2(front[1], front[0]), 16)).fill(k === 2 ? god.stone[2] : god.stone[1]).stroke(ink);
    g.poly(oval(...T(0, 0.05), 0.12 * S, 0.13 * S, 0, 14)).fill(shade(t[1], 0.85));
    if (k === 2) g.poly(oval(...T(0, -0.08), 0.15 * S, 0.17 * S, Math.atan2(front[1], front[0]), 14)).fill({ color: 0xffffff, alpha: 0.35 });
  } else {
    g.circle(cx, cy, 0.18 * S).fill(t[1]).stroke(ink);
    g.circle(cx + light[0] * 0.05, cy + light[1] * 0.05, 0.09 * S).fill(t[2]);
    if (k === 5) g.circle(cx, cy, 0.18 * S).stroke({ width: 0.02, color: god.metal[0], alpha: 0.6 });
  }
}

/** Candle stand, from a high angle as the candelabrum: a round foot, a tall stem and on it a wide round tray
 * (its rim showing) set with candles of mixed heights, the god's emblem cut in a little disc at the top of the
 * stem, flames and a soft glow of the god's colour; in the god's metal. */
export function candleStand(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const god = GODS[v % GODS.length];
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, right, up } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  const rot = Math.atan2(right[1], right[0]);
  const m = god.metal;
  glowPool(g, ...P(0, 0.12), 0.6, god.glow);
  castShadow(g, ring(...P(0, -0.4), 0.09, rnd, 0, 8), light, 1.1);
  // The foot and the stem.
  g.poly(oval(...P(0, -0.42), 0.17, 0.1, rot, 16)).fill(shade(m[1], 0.75)).stroke({ width: LINE, color: INK });
  g.poly(oval(...P(0, -0.39), 0.12, 0.07, rot, 14)).fill(m[1]).stroke({ width: LINE, color: INK });
  inkLine(g, [P(0, -0.38), P(0, 0.1)], 0.04, m[1]);
  g.poly(oval(...P(0, -0.12), 0.05, 0.03, rot, 10)).fill(m[2]).stroke({ width: 0.012, color: INK });
  // The tray, its candles from the back row forward, their flames.
  g.poly(oval(...P(0, 0.12), 0.36, 0.2, rot, 22)).fill(shade(m[0], 1.05)).stroke({ width: OUT, color: INK });
  g.poly(oval(...P(0, 0.135), 0.31, 0.16, rot, 20)).fill(m[1]);
  g.poly(oval(...P(0, 0.15), 0.25, 0.11, rot, 18)).fill({ color: 0x2a2018, alpha: 0.35 });
  const spots: [number, number, number][] = [[-0.2, 0.2, 0.12], [0.02, 0.24, 0.16], [0.21, 0.19, 0.1], [-0.11, 0.1, 0.14], [0.12, 0.09, 0.09], [-0.24, 0.07, 0.07], [0.27, 0.06, 0.12], [0, 0.02, 0.1]];
  for (const [a, b, h] of spots) {
    const hh = h * (0.85 + 0.3 * rnd());
    const c = [...P(a - 0.022, b + hh), ...P(a + 0.022, b + hh), ...P(a + 0.022, b), ...P(a - 0.022, b)];
    g.poly(c).fill(0xf4ecd8).stroke({ width: 0.01, color: INK, join: 'round' });
    g.poly([...P(a - 0.022, b + hh), ...P(a - 0.006, b + hh), ...P(a - 0.006, b), ...P(a - 0.022, b)]).fill(0xfffaf0);
    const ua = Math.atan2(up[1], up[0]);
    g.poly(oval(...P(a, b + hh + 0.03), 0.016, 0.028, ua, 10)).fill(0xf9a23a);
    g.poly(oval(...P(a, b + hh + 0.025), 0.008, 0.014, ua, 8)).fill(0xfff2c0);
  }
  // The god's emblem in a disc on the stem below the tray.
  const [ex, ey] = P(0, -0.2);
  g.circle(ex, ey, 0.085).fill(m[0]).stroke({ width: 0.012, color: INK });
  emblem(g, v, ex, ey, 0.13, up, god.accent);
}

/** Carved screen across the nave, seen from above: a thin oak partition along its back edge (toward the crossing),
 * so that pieces in a row read as one screen from pier to pier: its top rail carved with a row of round
 * quatrefoils (`tracery`), a post at each end and at every square, each post capped in the god's metal, and on a
 * piece of two squares or more the god's emblem in a roundel on its middle post; its shadow falls on the floor. */
export function carvedScreen(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const god = GODS[v % GODS.length];
  const [s0, s1] = SCREEN_RAIL;
  castShadow(g, quad(rect(0, s0, len, s1), rnd, 0), light, 1.1);
  // The rail: dark oak, its lit edge, the tracery carved along it.
  board(g, rect(0, s0, len, s1), OAK, light, rnd, along, 0.025, 0);
  g.poly(quad(rect(0.02, s0 + 0.03, len - 0.02, s1 - 0.03), rnd, 0.002)).fill(shade(OAK[1], 1.08)).stroke({ width: LINE, color: INK });
  tracery(g, at, len, god, 1, len >= 2 ? 0.17 : 0);
  screenPosts(g, at, rect, rnd, god, Array.from({ length: Math.floor(len + 1e-6) + 1 }, (_, u) => Math.min(Math.max(u, 0.07), len - 0.07)), 0.07);
  if (len >= 2) {
    const [ex, ey] = at(len / 2, (s0 + s1) / 2);
    g.circle(ex, ey, 0.15).fill(god.cloth[0]).stroke({ width: LINE, color: INK });
    const wall: Pt = [at(0, 0)[0] - at(0, 1)[0], at(0, 0)[1] - at(0, 1)[1]];
    emblem(g, v, ex, ey, 0.22, wall, god.accent);
  }
}

/** Where across its square the carved screen's rail runs (squares from its back edge), and its gate's beam. */
const SCREEN_RAIL = [0.05, 0.27];

/** The carved screen's tracery along its rail, `len` squares (`frame`'s `at`): two round quatrefoils a square
 * between its posts (none under the emblem at the middle, `clear` squares about it), each a pale carved roundel
 * ringed in ink with four dark lobes pierced through it round a boss of the god's metal; `alpha` under 1 for the
 * gate's beam overhead, seen through. */
function tracery(g: Graphics, at: (u: number, s: number) => [number, number], len: number, god: (typeof GODS)[number], alpha: number, clear: number) {
  const [s0, s1] = SCREEN_RAIL;
  const r = 0.085;
  const us: number[] = [];
  for (let k = 0; k < Math.round(len); k++) us.push(k + 0.32, k + 0.68);
  for (const u of us.filter((u) => Math.abs(u - len / 2) >= clear + r)) {
    const [cx, cy] = at(u, (s0 + s1) / 2);
    g.circle(cx, cy, r).fill({ color: shade(OAK[2], 1.1), alpha }).stroke({ width: 0.012, color: INK, alpha });
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) g.circle(cx + dx * r * 0.4, cy + dy * r * 0.4, r * 0.32).fill({ color: 0x24180f, alpha });
    g.circle(cx, cy, r * 0.16).fill({ color: god.metal[1], alpha });
  }
}

/** The carved screen's posts at `us` (squares along it), `hw` half wide, each capped in the god's metal. */
function screenPosts(g: Graphics, at: (u: number, s: number) => [number, number], rect: (u0: number, s0: number, u1: number, s1: number) => number[], rnd: () => number, god: (typeof GODS)[number], us: number[], hw: number) {
  const [s0, s1] = SCREEN_RAIL;
  const sm = (s0 + s1) / 2;
  for (const u of us) {
    g.poly(quad(rect(u - hw, s0 - 0.03, u + hw, s1 + 0.03), rnd, 0.003)).fill(OAK[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.circle(...at(u, sm), hw * 0.68).fill(god.metal[1]).stroke({ width: 0.012, color: INK });
    g.circle(...at(u, sm), hw * 0.3).fill(god.accent);
  }
}

/** The carved screen's gate (`carvedScreen`), seen from above, along the same back edge: a stone sill across the way
 * through, wider than the rail so its edges show either side; the two leaves of pierced tracery, each half the
 * opening, swung back into the nave on their posts, each swing marked in dashes; over the way, the screen's rail
 * carried across as a beam overhead, drawn see-through with its edges dashed (as a gatehouse's wall walk over its
 * gate), its tracery carried on and the god's emblem at its middle; a heavy capped post at each end. So pier, screen,
 * beam, screen and pier read as one line across the nave. */
export function screenGate(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const god = GODS[v % GODS.length];
  const [s0, s1] = SCREEN_RAIL;
  const sm = (s0 + s1) / 2;
  const hw = 0.085;
  // The sill: worn stone in blocks, its lit edge.
  const sill = rect(hw, s0 - 0.05, len - hw, s1 + 0.07);
  g.poly(quad(sill, rnd, 0.002)).fill(STONE[2]).stroke({ width: OUT, color: INK, join: 'round' });
  const lit = rect(hw, s1 + 0.03, len - hw, s1 + 0.07);
  g.poly(quad(lit, rnd, 0.001)).fill(shade(STONE[2], 1.2));
  const blocks = Math.max(2, Math.round(len * 2));
  for (let k = 1; k < blocks; k++) {
    const u = hw + ((len - 2 * hw) * k) / blocks;
    g.moveTo(...at(u, s0 - 0.05)).lineTo(...at(u, s1 + 0.07));
  }
  g.stroke({ width: 0.014, color: INK, alpha: 0.7 });
  // The leaves, open: each from its post back into the nave, as far as its square allows.
  const leaf = len / 2 - hw - 0.02;
  const open = Math.min(1.3, Math.asin(Math.min(1, (0.92 - sm) / leaf)));
  for (const dir of [1, -1]) {
    const hinge = dir > 0 ? hw : len - hw;
    const p = (t: number, k: number): [number, number] => at(hinge + dir * (Math.cos(open) * t - Math.sin(open) * k), sm + Math.sin(open) * t + Math.cos(open) * k);
    // (Its swing, from shut along the rail to where it stands.)
    for (let k = 0; k < 6; k++) {
      const [a0, a1] = [(open * k) / 6, (open * (k + 0.55)) / 6];
      g.moveTo(...at(hinge + dir * Math.cos(a0) * leaf, sm + Math.sin(a0) * leaf)).lineTo(...at(hinge + dir * Math.cos(a1) * leaf, sm + Math.sin(a1) * leaf));
    }
    g.stroke({ width: 0.018, color: INK, alpha: 0.55 });
    const pts = [...p(0, -0.05), ...p(leaf, -0.05), ...p(leaf, 0.05), ...p(0, 0.05)];
    castShadow(g, pts, light, 0.9);
    g.poly(pts).fill(OAK[1]).stroke({ width: LINE, color: INK, join: 'round' });
    const holes = Math.max(2, Math.round(leaf * 5));
    for (let k = 1; k <= holes; k++) g.circle(...p((leaf * k) / (holes + 1), 0), 0.017).fill(0x24180f);
    g.circle(...p(leaf, 0), 0.03).fill(god.accent).stroke({ width: 0.01, color: INK });
  }
  // The beam overhead: see-through, its edges dashed, the tracery carried on, the emblem at its middle.
  g.poly(rect(0, s0, len, s1)).fill({ color: OAK[1], alpha: 0.55 });
  tracery(g, at, len, god, 0.55, 0.09);
  for (const s of [s0, s1]) for (let u = hw; u < len - hw; u += 0.16) g.moveTo(...at(u, s)).lineTo(...at(Math.min(u + 0.09, len - hw), s));
  g.stroke({ width: 0.03, color: INK, alpha: 0.9 });
  const [ex, ey] = at(len / 2, sm);
  g.circle(ex, ey, 0.1).fill(god.cloth[0]).stroke({ width: LINE, color: INK });
  const wall: Pt = [at(0, 0)[0] - at(0, 1)[0], at(0, 0)[1] - at(0, 1)[1]];
  emblem(g, v, ex, ey, 0.15, wall, god.accent);
  // The posts.
  screenPosts(g, at, rect, rnd, god, [hw - 0.01, len - hw + 0.01], hw);
}

/** Side altar against the wall of a side chapel: a small block of the god's stone, a frontal of the god's cloth
 * hanging over its front with the god's emblem on it, a little painted panel standing at its back (a retable) with
 * the emblem again, two candles. */
export function sideAltar(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const god = GODS[v % GODS.length];
  board(g, rect(0.1, 0.8, len - 0.1, 0.95), STONE, light, rnd, along, 0.03, 0);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.82), god.stone.map((c) => shade(c, 0.95)), light, rnd, along, 0.05, 0);
  const wall: Pt = [at(0, 0)[0] - at(0, 1)[0], at(0, 0)[1] - at(0, 1)[1]];
  // The retable along the wall.
  g.poly(quad(rect(len * 0.2, WALL_GAP - 0.06, len * 0.8, WALL_GAP + 0.1), rnd, 0.003)).fill(god.metal[1]).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(len * 0.24, WALL_GAP - 0.03, len * 0.76, WALL_GAP + 0.07), rnd, 0.002)).fill(god.cloth[0]);
  // The frontal over the front.
  const [c0, c1] = [len * 0.24, len * 0.76];
  g.poly(quad(rect(c0, 0.42, c1, 0.9), rnd, 0.005)).fill(god.cloth[1]).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(c0 + 0.03, 0.83, c1 - 0.03, 0.88), rnd, 0.003)).fill(god.metal[1]);
  emblem(g, v, ...at(len / 2, 0.64), Math.min(0.3, (c1 - c0) * 0.62), wall, god.accent);
  for (const u of [END_GAP + 0.14, len - END_GAP - 0.14]) candle(g, ...at(u, 0.4));
}
