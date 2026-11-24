import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { INK, IRON, LINE, type Light, OUT, WOOD, bone, castShadow, frame, lit, oval, patch, quad, ring, shade, skullTop } from './common';
import { STONE, masonry } from './home';

/** Gravel walk across a court's lawn (`gravelPatch`), along its long side. */
export function gravelWalk(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, at } = frame(f);
  gravelPatch(g, at, len, along ? f.h : f.w, 1, rnd, light);
}

/** Gravel: the parks' paths' colour (`ground.frag`, gravel), its darker edge, a mottle each way, and
 * pebbles dark, warm and pale. */
const GRAVEL = { mid: 0xc2b599, edge: 0xa39477, light: 0xd4c9ae, shade: 0xb2a487, peb: [0x8c7f6a, 0xa8977a, 0xdcd1b6] };

/**
 * A gravel walk, as the parks' paths are gravelled: cream gravel mottled in soft patches, a sparse
 * scatter of bigger pebbles in three tones (each with its shadow), the long sides soft and a little
 * uneven in a darker gravel (never inked), lit along the side toward the light and shaded along
 * the other. Fills its rectangle end to end (walks meet the fountain and each other there); its long
 * sides only wobble inward. `at(s, t)` is the point `s` squares along it (0..len) and `t` across
 * (0..wid), `unit` the size of a square where it draws (1 in a building, PX on the battlemap) and
 * `light` the way toward the light in the same axes, so a building and the battlemap draw the same.
 */
export function gravelPatch(g: Graphics, at: (s: number, t: number) => [number, number], len: number, wid: number, unit: number, rnd: () => number, light: Light = [-Math.SQRT1_2, -Math.SQRT1_2]) {
  if (len <= 0 || wid <= 0) return;
  const pt = (s: number, t: number) => at(Math.min(len, Math.max(0, s)), Math.min(wid, Math.max(0, t)));
  // The long sides' uneven line, inward of the edge by `e` at each step along.
  const n = Math.max(2, Math.round(len * 4));
  const ins: number[][] = Array.from({ length: n + 1 }, () => [0.006 + rnd() * 0.035, 0.006 + rnd() * 0.035]);
  const side = (t0: number, dir: number, d: number, k: number) => pt((len * k) / n, t0 + dir * (ins[k][t0 ? 1 : 0] + d));
  const band = (t0: number, dir: number, d0: number, d1: number) => {
    const pts: number[] = [];
    for (let k = 0; k <= n; k++) pts.push(...side(t0, dir, d0, k));
    for (let k = n; k >= 0; k--) pts.push(...side(t0, dir, d1, k));
    return pts;
  };
  // The body, its long sides uneven.
  const body: number[] = [];
  for (let k = 0; k <= n; k++) body.push(...side(0, 1, 0, k));
  for (let k = n; k >= 0; k--) body.push(...side(wid, -1, 0, k));
  g.poly(body).fill(GRAVEL.mid);
  // Mottle: soft patches a shade lighter and darker (each a few nested rounds, so its edge fades),
  // kept inside.
  const blobs = Math.max(2, Math.round(len * wid * 1.6));
  for (let k = 0; k < blobs; k++) {
    const r = Math.min(wid * 0.32, 0.18 + rnd() * 0.2);
    const [s, t] = [r + rnd() * Math.max(0, len - 2 * r), r * 0.8 + 0.06 + rnd() * Math.max(0, wid - 1.6 * r - 0.12)];
    const turn = rnd() * Math.PI;
    const rs = Array.from({ length: 9 }, () => 0.75 + rnd() * 0.35);
    for (const f of [1, 0.78, 0.55]) {
      const pts: number[] = [];
      for (let m = 0; m < 9; m++) {
        const a = turn + (m / 9) * Math.PI * 2;
        pts.push(...pt(s + Math.cos(a) * r * rs[m] * f * 1.25, t + Math.sin(a) * r * rs[m] * f * 0.8));
      }
      g.poly(pts).fill({ color: k % 2 ? GRAVEL.light : GRAVEL.shade, alpha: 0.2 });
    }
  }
  // The long sides: the darker edge, soft (two bands), then lit or shaded inside it as the side faces.
  const [o0, o1] = [at(0, 0), at(0, 1)];
  const [nx, ny] = [o0[0] - o1[0], o0[1] - o1[1]];
  const facing = (nx * light[0] + ny * light[1]) / (Math.hypot(nx, ny) || 1);
  for (const [t0, dir, lit] of [
    [0, 1, facing],
    [wid, -1, -facing],
  ]) {
    const w = Math.min(0.12, wid * 0.14);
    g.poly(band(t0, dir, 0, w)).fill({ color: GRAVEL.edge, alpha: 0.3 });
    g.poly(band(t0, dir, 0, w * 0.45)).fill({ color: GRAVEL.edge, alpha: 0.4 });
    if (lit > 0.2) g.poly(band(t0, dir, w, w + Math.min(0.09, wid * 0.1))).fill({ color: 0xf4ead0, alpha: 0.4 * lit });
    else if (lit < -0.2) g.poly(band(t0, dir, w * 0.6, w + Math.min(0.14, wid * 0.14))).fill({ color: 0x4a2f3a, alpha: 0.13 * -lit });
  }
  // Grains: a sparse scatter, a little set in.
  const [lx, ly] = light;
  const lh = Math.hypot(lx, ly) || 1;
  // (Toward the light in the walk's own axes, so a pebble's shadow falls away from it.)
  const [p0, ps, pt1] = [at(0, 0), at(1, 0), at(0, 1)];
  const [su, sv] = [((ps[0] - p0[0]) * lx + (ps[1] - p0[1]) * ly) / (unit * lh), ((pt1[0] - p0[0]) * lx + (pt1[1] - p0[1]) * ly) / (unit * lh)];
  const pebble = (s: number, t: number, r: number, tone: number, alpha: number) => {
    const pts: number[] = [];
    const turn = rnd() * Math.PI;
    const k = 1 + rnd() * 0.45;
    for (let m = 0; m < 6; m++) {
      const a = turn + (m / 6) * Math.PI * 2;
      const rr = r * (0.72 + rnd() * 0.4);
      pts.push(Math.cos(a) * rr * k, Math.sin(a) * rr);
    }
    const [c, sn] = [Math.cos(turn), Math.sin(turn)];
    const shape = (ds: number, dt: number, f: number) => {
      const out: number[] = [];
      for (let m = 0; m < 6; m++) out.push(...pt(s + ds + (pts[m * 2] * c - pts[m * 2 + 1] * sn) * f, t + dt + (pts[m * 2] * sn + pts[m * 2 + 1] * c) * f));
      return out;
    };
    g.poly(shape(-su * r * 0.45, -sv * r * 0.45, 1)).fill({ color: 0x4a2f3a, alpha: 0.22 * alpha });
    g.poly(shape(0, 0, 1)).fill({ color: tone, alpha });
    if (tone !== GRAVEL.peb[0]) g.poly(shape(su * r * 0.3, sv * r * 0.3, 0.45)).fill({ color: 0xfff1cf, alpha: 0.25 * alpha });
  };
  const m = 0.07;
  // Small grains on a jittered lattice, then bigger pebbles, fewer.
  const cell = 0.2;
  for (let s = 0; s < len; s += cell) {
    for (let t = m; t < wid - m; t += cell) {
      if (rnd() < 0.45) continue;
      const tone = GRAVEL.peb[Math.floor(rnd() * 3)];
      pebble(s + cell * (0.15 + rnd() * 0.7), t + cell * rnd() * 0.9, 0.022 + rnd() * 0.014, tone, 0.75);
    }
  }
  const big = Math.round(len * wid * (6 + rnd()));
  for (let k = 0; k < big; k++) {
    const r = 0.035 + rnd() * 0.03;
    const s = r + rnd() * Math.max(0, len - 2 * r);
    const t = m + r + rnd() * Math.max(0, wid - 2 * (m + r));
    pebble(s, t, r, GRAVEL.peb[k % 3], 0.95);
  }
}

/** Stairs up out of a level: a stone-framed flight rising toward the top of its square, each step higher
 * and lighter than the one below, a pale arrow up; out to the surface, daylight falling down them. */
export function stairsUp(g: Graphics, f: InteriorItem, rnd: () => number, daylight: boolean) {
  const { x, y } = f;
  if (daylight) for (const [k, a] of [[0.75, 0.1], [0.55, 0.12], [0.35, 0.14]]) g.circle(x + 0.5, y + 0.3, k).fill({ color: 0xfff3d0, alpha: a });
  masonry(g, [x + 0.04, y + 0.04, x + 0.96, y + 0.96], STONE, rnd, 0.16);
  g.poly(quad([x + 0.16, y + 0.16, x + 0.84, y + 0.84], rnd, 0.004)).fill(STONE[0]).stroke({ width: LINE, color: INK });
  const n = 5;
  for (let k = 0; k < n; k++) {
    // The top step (k = 0) at y + 0.16, lightest.
    const y0 = y + 0.16 + k * 0.136;
    const c = shade(daylight ? 0xd8cfbd : STONE[2], 1.12 - k * 0.12);
    g.rect(x + 0.18, y0, 0.64, 0.12).fill(c);
    g.moveTo(x + 0.18, y0 + 0.12).lineTo(x + 0.82, y0 + 0.12).stroke({ width: 0.02, color: shade(c, 0.6) });
  }
  g.moveTo(x + 0.5, y + 0.72).lineTo(x + 0.5, y + 0.3).moveTo(x + 0.38, y + 0.42).lineTo(x + 0.5, y + 0.28).lineTo(x + 0.62, y + 0.42);
  g.stroke({ width: 0.06, color: daylight ? 0xfff6d8 : 0xf2e6c8, cap: 'round', join: 'round' });
}

/** A tunnel leaving through the rock wall: a straight passage about a square wide running away into the
 * rock, fading smoothly from the floor's colour at its mouth to black, rough rock faces either side. */
export function tunnel(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const [mouth, end] = [0.2, -0.75];
  const half = (s0: number) => 0.4 - (mouth - s0) * 0.04;
  // The passage: many thin slices, each a shade darker, so the fade reads as one smooth run.
  const n = 24;
  for (let k = 0; k < n; k++) {
    const s0 = mouth + ((end - mouth) * k) / n;
    const s1 = mouth + ((end - mouth) * (k + 1)) / n + (k < n - 1 ? -0.004 : 0);
    const t = (k + 0.5) / n;
    const c = shade(STONE[1], Math.max(0.05, 1 - Math.pow(t, 0.8) * 0.95));
    g.poly([...at(0.5 - half(s0), s0), ...at(0.5 + half(s0), s0), ...at(0.5 + half(s1), s1), ...at(0.5 - half(s1), s1)]).fill(c);
  }
  // Rough rock faces either side, running straight back.
  for (const side of [-1, 1]) {
    const inner: number[] = [];
    for (let k = 0; k <= 6; k++) {
      const sk = mouth + 0.04 + ((end - mouth - 0.04) * k) / 6;
      inner.push(...at(0.5 + side * (half(sk) + (rnd() - 0.5) * 0.03), sk));
    }
    const pts = [...inner];
    for (let k = 6; k >= 0; k--) {
      const sk = mouth + 0.04 + ((end - mouth - 0.04) * k) / 6;
      pts.push(...at(0.5 + side * (0.5 + (rnd() - 0.5) * 0.03), sk));
    }
    g.poly(pts).fill(0x4a443c);
    for (let k = 0; k < 6; k++) g.moveTo(inner[k * 2], inner[k * 2 + 1]).lineTo(inner[k * 2 + 2], inner[k * 2 + 3]);
    g.stroke({ width: OUT, color: INK, cap: 'round' });
  }
}

/** A ladder against the wall, up to a grate in the street above: two rails, rungs, a square of grey light
 * falling through the grate onto its foot. */
export function ladder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number], turned = false) {
  const { at, rect } = frame(f, wall);
  // The grate's light on the floor below it (a ladder turned to a curving wall has none: a tilted box reads as a pale slab).
  if (!turned) {
    g.poly(quad(rect(0.2, 0.02, 0.8, 0.62), rnd, 0)).fill({ color: 0xd8dce0, alpha: 0.18 });
    for (let k = 1; k < 4; k++) g.moveTo(...at(0.2 + k * 0.15, 0.02)).lineTo(...at(0.2 + k * 0.15, 0.62));
    g.stroke({ width: 0.03, color: 0x14100c, alpha: 0.25 });
  }
  // Lit from the light's side: the rails and rungs get a bright edge toward it and a dark one away.
  const [lx, ly] = light;
  const d = 0.014;
  const line = (x0: number, y0: number, x1: number, y1: number, w: number, color: number, ox = 0, oy = 0) =>
    g.moveTo(x0 + ox, y0 + oy).lineTo(x1 + ox, y1 + oy).stroke({ width: w, color, cap: 'round' });
  for (const u of [0.35, 0.65]) {
    const [a, b] = [at(u, -0.22), at(u, 0.7)];
    line(a[0], a[1], b[0], b[1], 0.07, INK);
    line(a[0], a[1], b[0], b[1], 0.04, WOOD[1]);
    line(a[0], a[1], b[0], b[1], 0.014, WOOD[2], lx * d, ly * d);
  }
  for (const s0 of [-0.08, 0.14, 0.32, 0.5]) {
    const [a, b] = [at(0.35, s0), at(0.65, s0)];
    line(a[0], a[1], b[0], b[1], 0.05, INK);
    line(a[0], a[1], b[0], b[1], 0.028, WOOD[2]);
    line(a[0], a[1], b[0], b[1], 0.01, WOOD[0], -lx * d, -ly * d);
  }
}

/** A drain outlet set flush in the wall, seen from above: a round mouth in a rim (0 iron, bolted; 1 brick,
 * a grate across it; 2 stone, broken away at one side), half in the wall, dark inside with the far side of
 * the bore catching the light. A stain and a thin trickle of muck run out across the floor to a small puddle. */
export function pipe(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const look = v % 3;
  const [cu, cs] = [0.5, 0.04];
  const [rx, ry, ix, iy] = [0.31, 0.19, 0.18, 0.085];
  const rot = wall[0] !== 0 ? Math.PI / 2 : 0;
  const tones = look === 0 ? IRON : look === 1 ? [0x5e4234, 0x80604c, 0x9a7a62] : [0x5c5850, 0x7c776c, 0x96917f];
  const on = (t: number, k: number) => at(cu + Math.cos(t) * (ix + (rx - ix) * k), cs + Math.sin(t) * (iy + (ry - iy) * k));
  const [end, drift] = [0.4, (rnd() - 0.5) * 0.12];
  const FILTH = 0x46502e;
  // The stain, and the puddle spread along the foot of the wall, as flat as the scum on the floor.
  g.poly(oval(...at(cu + drift * 0.4, 0.24), 0.17, 0.15, rot, 14)).fill({ color: 0x343a26, alpha: 0.5 });
  const edge: number[] = [];
  for (let i = 0; i < 14; i++) {
    const t = (i / 14) * Math.PI * 2;
    const k = 0.75 + rnd() * 0.45;
    edge.push(...at(cu + drift + Math.cos(t) * 0.17 * k, end + Math.sin(t) * 0.075 * k));
  }
  g.poly(edge).fill(FILTH).stroke({ width: 0.018, color: INK, alpha: 0.45, join: 'round' });
  g.poly(oval(...at(cu + drift - 0.04, end - 0.015), 0.05, 0.016, rot, 8)).fill({ color: 0x626a46, alpha: 0.9 });
  // The rim: blocks turned to the light lit, away from it shaded. (Broken: a run of them gone.)
  const n = look === 1 ? 8 : 24;
  const [a0, a1] = [1, 5];
  const gone = (i: number) => look === 2 && i >= a0 && i < a1;
  const [ox, oy] = at(cu, cs);
  for (let i = 0; i < n; i++) {
    if (gone(i)) continue;
    const [t0, t1] = [(i / n) * Math.PI * 2, ((i + 1) / n) * Math.PI * 2];
    const [mx, my] = on((t0 + t1) / 2, 1);
    const lit = ((mx - ox) * light[0] + (my - oy) * light[1]) / Math.max(Math.hypot(mx - ox, my - oy), 1e-6);
    g.poly([...on(t0, 0), ...on(t0, 1), ...on(t1, 1), ...on(t1, 0)]).fill(tones[lit > 0.35 ? 2 : lit < -0.35 ? 0 : 1]);
  }
  // Ink round the rim that is left, with a line across each end of a break.
  const arc = (i0: number, i1: number) => {
    const pts: number[] = [];
    for (let i = i0; i <= i1; i++) pts.push(...on((i / n) * Math.PI * 2, 1));
    return pts;
  };
  if (look === 2) {
    g.poly(arc(a1, n + a0), false).stroke({ width: OUT, color: INK, join: 'round', cap: 'round' });
    for (const i of [a0, a1]) g.moveTo(...on((i / n) * Math.PI * 2, 0)).lineTo(...on((i / n) * Math.PI * 2, 1)).stroke({ width: LINE, color: INK, cap: 'round' });
    // Where the rim fell: chips on the floor beside it.
    for (const [u, s, r] of [[0.86, 0.22, 0.035], [0.78, 0.31, 0.025]]) g.poly(ring(...at(u, s), r, rnd, 0.008, 6)).fill(tones[1]).stroke({ width: 0.018, color: INK });
  } else {
    g.poly(arc(0, n - 1)).stroke({ width: OUT, color: INK, join: 'round' });
  }
  if (look === 1) {
    for (let i = 0; i < n; i++) g.moveTo(...on((i / n) * Math.PI * 2, 0)).lineTo(...on((i / n) * Math.PI * 2, 1)).stroke({ width: 0.02, color: INK });
  }
  if (look === 0) {
    for (const t of [0.8, 2.34, 3.93, 5.5]) g.circle(...on(t, 0.5), 0.02).fill(IRON[0]).stroke({ width: 0.018, color: INK });
  }
  // The bore: dark, the far side catching the light (the dark shifted toward the light).
  g.poly(oval(...at(cu, cs), ix, iy, rot, 18)).fill(shade(tones[0], 0.72));
  g.poly(oval(...at(cu, cs), ix, iy, rot, 18).map((c, i) => c + light[i % 2] * 0.024)).fill(0x0e0c0a);
  g.poly(oval(...at(cu, cs), ix, iy, rot, 18)).stroke({ width: LINE, color: INK });
  if (look === 1) {
    for (const u of [-0.085, 0, 0.085]) g.moveTo(...at(cu + u, cs - iy * 0.95)).lineTo(...at(cu + u, cs + iy * 0.95)).stroke({ width: 0.024, color: IRON[0], cap: 'round' });
  }
  // The muck, out over the lip of the mouth and across the floor.
  g.moveTo(...at(cu, cs + iy * 0.5)).quadraticCurveTo(...at(cu - drift * 0.6, 0.26), ...at(cu + drift, end)).stroke({ width: 0.024, color: FILTH, cap: 'round' });
}

/** An old well: a ring of fitted stones round dark water with a glint, a rope down into it from a beam
 * across, a bucket on the rim. */
export function well(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.42;
  g.poly(ring(cx, cy, r, rnd, 0.01, 24)).fill(STONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const n = 10;
  for (let i = 0; i < n; i++) {
    const a0 = (i / n) * Math.PI * 2 + 0.05;
    const a1 = ((i + 1) / n) * Math.PI * 2 - 0.05;
    const pts: number[] = [];
    for (let k = 0; k <= 3; k++) pts.push(cx + Math.cos(a0 + ((a1 - a0) * k) / 3) * r * 0.95, cy + Math.sin(a0 + ((a1 - a0) * k) / 3) * r * 0.95);
    for (let k = 3; k >= 0; k--) pts.push(cx + Math.cos(a0 + ((a1 - a0) * k) / 3) * r * 0.66, cy + Math.sin(a0 + ((a1 - a0) * k) / 3) * r * 0.66);
    g.poly(pts).fill(shade(STONE[1], 0.95 + 0.15 * lit((a0 + a1) / 2)));
  }
  g.circle(cx, cy, r * 0.62).fill(0x2a4250).stroke({ width: LINE, color: INK });
  g.circle(cx - light[0] * r * 0.06, cy - light[1] * r * 0.06, r * 0.5).fill(0x14222a);
  g.poly(oval(cx + light[0] * r * 0.25, cy + light[1] * r * 0.25, r * 0.18, r * 0.07, -0.6)).fill({ color: 0x6a9aaa, alpha: 0.7 });
  g.moveTo(cx - r * 1.02, cy).lineTo(cx + r * 1.02, cy).stroke({ width: 0.05, color: INK, cap: 'round' });
  g.moveTo(cx - r * 1.02, cy).lineTo(cx + r * 1.02, cy).stroke({ width: 0.025, color: WOOD[1], cap: 'round' });
  g.circle(cx, cy, 0.03).fill(0xc8b080).stroke({ width: 0.018, color: INK });
  const [bx, by] = [cx + r * 0.7, cy + r * 0.7];
  g.circle(bx, by, 0.09).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.circle(bx, by, 0.06).fill(0x3e5a6a);
}

/** A pit: a hole in the floor with a broken, rocky rim, dropping away into darkness in rough rings. */
export function pit(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) * 0.46;
  const rough = (k: number, j: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 22; i++) {
      const a = (i / 22) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * j);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    return pts;
  };
  g.poly(rough(1, 0.12)).fill(STONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(rough(0.88, 0.14)).fill(0x4a4238);
  g.poly(rough(0.72, 0.16).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.06)).fill(0x221c16);
  g.poly(rough(0.52, 0.18).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.1)).fill(0x100c0a);
  g.poly(rough(0.3, 0.2).map((p, i) => p - (i % 2 ? light[1] : light[0]) * r * 0.12)).fill(0x050404);
  // Broken stones on the rim, a crack or two running out from it.
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(ring(cx + Math.cos(a) * r * 0.95, cy + Math.sin(a) * r * 0.95, 0.06 + 0.04 * rnd(), rnd, 0.03, 6)).fill(STONE[1]).stroke({ width: LINE, color: INK });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r).lineTo(cx + Math.cos(a + 0.1) * r * 1.12, cy + Math.sin(a + 0.1) * r * 1.12);
  }
  g.stroke({ width: LINE, color: INK });
}

/** A hidden pressure plate (seen only by the DM): a bold red-tinted square with a thick dashed red border
 * and a warning glyph (!) on the plate, so it stands out at a glance. */
export function trap(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y } = f;
  g.poly(quad([x + 0.1, y + 0.1, x + 0.9, y + 0.9], rnd, 0)).fill({ color: 0xd8301e, alpha: 0.28 });
  g.poly(quad([x + 0.26, y + 0.26, x + 0.74, y + 0.74], rnd, 0.004)).fill(shade(STONE[1], 0.9)).stroke({ width: 0.02, color: INK });
  const d = 0.1;
  for (const [ax, ay, bx, by] of [
    [0.1, 0.1, 0.9, 0.1],
    [0.9, 0.1, 0.9, 0.9],
    [0.9, 0.9, 0.1, 0.9],
    [0.1, 0.9, 0.1, 0.1],
  ]) {
    const len = Math.hypot(bx - ax, by - ay);
    for (let t = 0; t < len; t += d * 2) {
      const [t0, t1] = [t / len, Math.min(1, (t + d) / len)];
      g.moveTo(x + ax + (bx - ax) * t0, y + ay + (by - ay) * t0).lineTo(x + ax + (bx - ax) * t1, y + ay + (by - ay) * t1);
    }
  }
  g.stroke({ width: 0.06, color: 0xd8301e, cap: 'round' });
  g.moveTo(x + 0.5, y + 0.36).lineTo(x + 0.5, y + 0.54).stroke({ width: 0.07, color: 0xd8301e, cap: 'round' });
  g.circle(x + 0.5, y + 0.64, 0.04).fill(0xd8301e);
}

/** An unstable ceiling (players see it too): grit and small stones fallen on the floor beneath it, cracks
 * in the floor where bigger ones struck. */
export function caveIn(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [cx, cy] = [x + w / 2, y + h / 2];
  for (let i = 0; i < 40; i++) g.circle(x + 0.15 + rnd() * (w - 0.3), y + 0.15 + rnd() * (h - 0.3), 0.016 + 0.012 * rnd()).fill(i % 3 ? 0x6e655a : 0x8a8070).stroke({ width: 0.018, color: INK });
  for (let i = 0; i < 5; i++) {
    const [px, py] = [x + 0.2 + rnd() * (w - 0.4), y + 0.2 + rnd() * (h - 0.4)];
    const r = 0.06 + 0.04 * rnd();
    g.poly(ring(px, py, r, rnd, 0.03, 6)).fill(CAVE_ROCK[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(ring(px - 0.01, py - 0.012, r * 0.55, rnd, 0.02, 6)).fill(CAVE_ROCK[2]);
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    let [px, py] = [cx, cy];
    g.moveTo(px, py);
    for (let k = 0; k < 3; k++) {
      px += Math.cos(a + (rnd() - 0.5)) * 0.1;
      py += Math.sin(a + (rnd() - 0.5)) * 0.1;
      g.lineTo(px, py);
    }
  }
  g.stroke({ width: LINE, color: INK, cap: 'round', join: 'round' });
}

/** A faceted stone from above: a dark rim on the shaded side, its face, a lit top facet toward the light
 * with facet lines down to it, a sunlit spot, a crack on big ones. */
export function rock(g: Graphics, x: number, y: number, s: number, t: readonly number[], light: Light, rnd: () => number, n = 8) {
  const base: number[] = [];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const rr = s * (0.82 + 0.3 * rnd());
    base.push(Math.cos(a) * rr, Math.sin(a) * rr);
  }
  const at = (k: number, dx: number, dy: number) => base.map((p, i) => p * k + (i % 2 ? y + dy : x + dx));
  g.poly(at(1, 0, 0)).fill(t[0]).stroke({ width: s < 0.25 ? LINE : OUT, color: INK, join: 'round' });
  const body = at(0.9, light[0] * s * 0.1, light[1] * s * 0.1);
  g.poly(body).fill(t[1]);
  const top = at(0.52, light[0] * s * 0.18, light[1] * s * 0.18);
  g.poly(top).fill(t[2]);
  for (let i = 0; i < base.length; i += 4) g.moveTo(top[i], top[i + 1]).lineTo(body[i], body[i + 1]);
  g.stroke({ width: 0.014, color: t[0], alpha: 0.7, cap: 'round' });
  if (s > 0.3) {
    const a = rnd() * Math.PI * 2;
    let [cx, cy] = [x + Math.cos(a) * s * 0.8, y + Math.sin(a) * s * 0.8];
    g.moveTo(cx, cy);
    for (let k = 0; k < 3; k++) {
      cx += (x - cx) * 0.3 + (rnd() - 0.5) * s * 0.15;
      cy += (y - cy) * 0.3 + (rnd() - 0.5) * s * 0.15;
      g.lineTo(cx, cy);
    }
    g.stroke({ width: 0.02, color: shade(t[0], 0.7), cap: 'round', join: 'round' });
  }
}

/** Daylight through a drain grate in the street overhead (`under::drain_grates`), on a sewer's floor: a soft warm
 * pool in two flat steps, brightest where the grate's 2 x 1.5 ft of light lands, its bars faintly across it. No glow:
 * it lies on the floor like a painted patch. `run`: the way the tunnel runs (the street's way, which the grate follows). */
export function drainLight(g: Graphics, f: InteriorItem, rnd: () => number, run: [number, number]) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const [ux, uy] = run;
  const [vx, vy] = [-uy, ux];
  const at = (a: number, b: number): [number, number] => [cx + ux * a + vx * b, cy + uy * a + vy * b];
  const SUN = 0xffe6a0;
  g.poly(patch(cx, cy, 0.3, 0.27, rnd, 0.1)).fill({ color: SUN, alpha: 0.14 });
  g.poly(patch(cx, cy, 0.22, 0.2, rnd, 0.08)).fill({ color: SUN, alpha: 0.16 });
  g.poly([...at(-0.2, -0.15), ...at(0.2, -0.15), ...at(0.2, 0.15), ...at(-0.2, 0.15)]).fill({ color: 0xfff0c4, alpha: 0.26 });
  for (let k = 0; k < 4; k++) g.moveTo(...at(-0.12 + k * 0.08, -0.14)).lineTo(...at(-0.12 + k * 0.08, 0.14));
  g.stroke({ width: 0.02, color: 0x2a261c, alpha: 0.2 });
}

/** An iron grille across a sewer channel where runs meet, seen from above: a frame bar fixed in each kerb (`span`:
 * how far the channel reaches either side of `c`, its run's line), two thin rails between them, 5-7 bars along the
 * flow lit down one side, a shadow away from the light and the muck caught on its upstream side. `run`: the way the
 * sewage runs here (downstream). */
export function grille(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, run: [number, number], c: [number, number], span: [number, number]) {
  const [cx, cy] = c;
  const [ux, uy] = run;
  const [vx, vy] = [-uy, ux];
  const at = (a: number, b: number): [number, number] => [cx + ux * a + vx * b, cy + uy * a + vy * b];
  const band = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const [lx, ly] = light;
  const [b0, b1] = [-span[0] - 0.12, span[1] + 0.12];
  const L = 0.2;
  // Muck against it upstream, in a few flat blobs.
  const blobs = Math.max(2, Math.round((b1 - b0) / 0.4));
  for (let i = 0; i < blobs; i++) {
    const [mx, my] = at(-L - 0.06 - rnd() * 0.05, b0 + 0.2 + ((b1 - b0 - 0.4) * (i + 0.5)) / blobs + (rnd() - 0.5) * 0.1);
    g.poly(patch(mx, my, 0.1, 0.07, rnd, 0.25, 10)).fill({ color: 0x6c7046, alpha: 0.8 });
  }
  // The shadow, faint, away from the light.
  g.poly(band(-L, L, b0, b1).map((v, i) => v - (i % 2 ? ly : lx) * 0.07)).fill({ color: 0x000000, alpha: 0.18 });
  // Two thin rails across, kerb to kerb, under the bars.
  for (const a of [-L * 0.6, L * 0.6]) g.poly(band(a - 0.025, a + 0.025, b0, b1)).fill(IRON[0]).stroke({ width: LINE, color: INK, join: 'round' });
  // The bars along the flow, lit on the side toward the light.
  const n = Math.min(7, Math.max(5, Math.round((b1 - b0) / 0.22)));
  const side = vx * lx + vy * ly < 0 ? -1 : 1;
  for (let k = 0; k < n; k++) {
    const b = b0 + 0.1 + ((b1 - b0 - 0.2) * (k + 0.5)) / n;
    g.poly(band(-L, L, b - 0.032, b + 0.032)).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...at(-L + 0.03, b + side * 0.014)).lineTo(...at(L - 0.03, b + side * 0.014)).stroke({ width: 0.018, color: IRON[2] });
  }
  // The frame bars at the kerbs, set in the stone.
  for (const b of [b0, b1]) {
    g.poly(band(-L - 0.04, L + 0.04, b - 0.06, b + 0.06)).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...at(-L, b + side * 0.025)).lineTo(...at(L, b + side * 0.025)).stroke({ width: 0.02, color: IRON[2] });
  }
}

const CAVE_ROCK = [0x5a5246, 0x857a6a, 0xa89c88, 0xc8bca6];

/** Stalagmite: a cone of wet rock rising from the floor — rings of flowstone climbing to a pale tip set toward
 * the light, a drip glint on it, a shadow away from the light. */
export function stalagmite(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.36;
  castShadow(g, patch(cx, cy, r, r, rnd, 0.1, 14), light, 0.5);
  const rings = 4;
  for (let k = 0; k < rings; k++) {
    const kk = 1 - k * 0.22;
    const off = (k / rings) * r * 0.45;
    g.poly(patch(cx + light[0] * off, cy + light[1] * off, r * kk, r * kk, rnd, 0.12, 16)).fill(shade(CAVE_ROCK[1], 0.92 + k * 0.1)).stroke({ width: k === 0 ? OUT : 0.016, color: k === 0 ? INK : CAVE_ROCK[0], join: 'round' });
  }
  const [tx, ty] = [cx + light[0] * r * 0.5, cy + light[1] * r * 0.5];
  g.circle(tx, ty, r * 0.12).fill(CAVE_ROCK[3]).stroke({ width: 0.018, color: INK });
  g.circle(tx + light[0] * 0.02, ty + light[1] * 0.02, 0.025).fill({ color: 0xffffff, alpha: 0.7 });
}

/** Rock column: a thick pillar of flowstone joining floor and ceiling — a lumpy outline, ripples of flowstone
 * round it, lit down one side, a long shadow. */
export function rockColumn(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.44;
  castShadow(g, patch(cx, cy, r, r, rnd, 0.1, 16), light, 1.1);
  const out = patch(cx, cy, r, r, rnd, 0.1, 20);
  g.poly(out).fill(CAVE_ROCK[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.03, cy + light[1] * 0.03, r * 0.86, r * 0.86, rnd, 0.08, 20)).fill(CAVE_ROCK[1]);
  for (const k of [0.68, 0.48, 0.28]) g.poly(patch(cx + light[0] * (0.86 - k) * 0.1, cy + light[1] * (0.86 - k) * 0.1, r * k, r * k, rnd, 0.1, 16)).stroke({ width: 0.018, color: CAVE_ROCK[0], alpha: 0.8 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.78, cy + Math.sin(la - 0.9) * r * 0.78)
    .arc(cx, cy, r * 0.78, la - 0.9, la + 0.9)
    .stroke({ width: 0.04, color: CAVE_ROCK[2], alpha: 0.8 });
}

/** Boulder: a big faceted stone, a pebble or two at its foot, a shadow away from the light. */
export function caveBoulder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const s = Math.min(f.w, f.h) * 0.38;
  castShadow(g, patch(cx, cy, s, s, rnd, 0.15, 10), light, 0.5);
  for (let i = 0; i < 2; i++) {
    const a = Math.PI * 0.25 + (rnd() - 0.5) * 2;
    rock(g, cx + Math.cos(a) * s * 1.15, cy + Math.sin(a) * s * 1.15, s * 0.16, CAVE_ROCK, light, rnd, 6);
  }
  rock(g, cx, cy, s, CAVE_ROCK, light, rnd, 9);
}

/** Crystal cluster (after a reference picture): long faceted shards of uneven length radiating outward from a shared
 * heart in different directions, each split into flat facets in light and dark tones with a highlight streak, and a
 * few stubby upright prisms standing in the middle, seen from a high angle with their flat six-sided tops showing. */
export function crystal(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  // Pale milky quartz: three close greyish tones with only a faint cool tint (no glow, no hairlines).
  const t = [
    [0x8c9094, 0xa9adb0, 0xc4c7c8],
    [0x908f94, 0xadacb1, 0xc7c6c9],
    [0x8b9291, 0xa8afae, 0xc3c8c7],
    [0x8e9195, 0xabaeb2, 0xc5c7ca],
  ][v % 4].map((c) => c);
  t.push(t[2]);
  const line = INK;
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, up } = screenAxes(light);
  // (A cluster's size varies a lot: a few small shards up to a big one.)
  let accent = false;
  const sz = 0.55 + 0.6 * rnd();
  // A small stone base the shards grow from.
  rock(g, cx - light[0] * 0.02, cy - light[1] * 0.02, 0.2 * sz + 0.06, CAVE_ROCK, light, rnd, 8);
  // One long faceted prism lying outward along angle `a`: two long faces split by an off-centre ridge, a pointed
  // tip of its own two facets, a crack across each long face; the face turned to the light is pale.
  const shard = (a: number, len: number, w: number) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const p = (u: number, k: number): [number, number] => [cx + c * u - s * k, cy + s * u + c * k];
    const lit = -s * light[0] + c * light[1] > 0 ? 1 : -1;
    const r = w * 0.2 * (rnd() - 0.5);
    const sh = len * (0.6 + 0.15 * rnd());
    const [B, BL, BR] = [p(-0.04, r), p(-0.04, w), p(-0.04, -w)];
    const [SL, SR, SM] = [p(sh, w * 0.92), p(sh, -w * 0.92), p(sh + w * 0.3, r)];
    const T = p(len, r * 0.5);
    const tone = (k: number, tip: boolean) => (k === lit ? (tip && !accent ? ((accent = true), t[3]) : t[2]) : tip ? t[1] : t[0]);
    g.poly([...B, ...BL, ...SL, ...SM]).fill(tone(1, false));
    g.poly([...B, ...BR, ...SR, ...SM]).fill(tone(-1, false));
    g.poly([...SL, ...T, ...SM]).fill(tone(1, true));
    g.poly([...SR, ...T, ...SM]).fill(tone(-1, true));
    g.poly([...BL, ...SL, ...T, ...SR, ...BR]).stroke({ width: OUT, color: line, join: 'round' });
  };
  // Eight uneven prisms all round, the ones pointing away (screen up) first so the near ones overlap them.
  const n = 5 + Math.floor(rnd() * 4);
  const ph = rnd() * Math.PI * 2;
  const outs: [number, number, number][] = [];
  for (let i = 0; i < n; i++) { const k = sz * (0.5 + rnd() * 0.8); outs.push([ph + ((i + (rnd() - 0.5) * 0.8) / n) * Math.PI * 2, (0.3 + 0.16 * rnd()) * k, (0.08 + 0.035 * rnd()) * k]); }
  outs.sort((p, q) => (Math.cos(q[0]) * up[0] + Math.sin(q[0]) * up[1]) - (Math.cos(p[0]) * up[0] + Math.sin(p[0]) * up[1]));
  for (const [a, len, w] of outs) shard(a, len, w);
  // Stubby upright hexagonal prisms in the middle, seen from high up: three side faces, a flat hexagonal top.
  const stubs: [number, number, number, number][] = [
    [0.02, 0.05, 0.16, 0.09],
    [-0.11, -0.02, 0.11, 0.08],
    [0.12, -0.06, 0.08, 0.07],
  ];
  for (const [a, b, h, w] of stubs) {
    const base = pt(cx, cy, a * sz, b * sz);
    const at = (x: number, y: number, z: number) => pt(base[0], base[1], x * w * sz, y * w * sz * 0.45 + z * sz);
    const ring = (z: number) => [at(-1, 0, z), at(-0.5, -1, z), at(0.5, -1, z), at(1, 0, z), at(0.5, 1, z), at(-0.5, 1, z)];
    const [lo, hi] = [ring(0), ring(h)];
    const face = (i: number, j: number, col: number) => g.poly([...lo[i], ...lo[j], ...hi[j], ...hi[i]]).fill(col);
    face(0, 1, t[2]);
    face(1, 2, t[1]);
    face(2, 3, t[0]);
    g.poly(hi.flat()).fill(t[2]);
    g.poly([...lo[0], ...lo[1], ...lo[2], ...lo[3], ...hi[3], ...hi[4], ...hi[5], ...hi[0]]).stroke({ width: OUT, color: line, join: 'round' });
  }
}

/** Ice formation: a rounded, melted mound of pale ice with stubby icicles standing out of it, a frosted fringe
 * round its foot, cracks and bubbles inside; cold, not glowing. */
export function iceFormation(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 30; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.38 + 0.1 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.012 + 0.01 * rnd()).fill({ color: 0xf0f8ff, alpha: 0.8 });
  }
  const mound = patch(cx, cy, 0.38, 0.34, rnd, 0.12, 20);
  g.poly(mound).fill(0xa8cce0).stroke({ width: OUT, color: 0x3e6a88, join: 'round' });
  g.poly(patch(cx + light[0] * 0.04, cy + light[1] * 0.04, 0.3, 0.26, rnd, 0.1, 18)).fill(0xcae4f2);
  g.poly(patch(cx + light[0] * 0.1, cy + light[1] * 0.1, 0.14, 0.11, rnd, 0.1, 12)).fill({ color: 0xffffff, alpha: 0.75 });
  // Stubby icicles standing out of the mound: short rounded cones.
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.08 + 0.12 * rnd();
    const [px, py] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
    g.circle(px, py, 0.06).fill(0xe0f0fa).stroke({ width: 0.014, color: 0x5a8aa8 });
    g.circle(px + light[0] * 0.02, py + light[1] * 0.02, 0.025).fill(0xffffff);
  }
  g.moveTo(cx - 0.2, cy + 0.05).lineTo(cx - 0.05, cy + 0.12).lineTo(cx + 0.05, cy + 0.04).stroke({ width: 0.014, color: 0x5a8aa8, alpha: 0.8 });
  for (let i = 0; i < 4; i++) g.circle(cx + (rnd() - 0.5) * 0.4, cy + (rnd() - 0.5) * 0.3, 0.012).stroke({ width: 0.014, color: 0xffffff });
}

/** For things drawn upright from a high angle (glowing fungus, crystals): screen up and screen right in grid axes,
 * from the light (screen NW, so screen up is the light turned 45 degrees), and a point `a` right and `b` up of
 * (x, y). */
export function screenAxes(light: Light) {
  const la = Math.atan2(light[1], light[0]) + Math.PI / 4;
  const up: [number, number] = [Math.cos(la), Math.sin(la)];
  const right: [number, number] = [Math.cos(la + Math.PI / 2), Math.sin(la + Math.PI / 2)];
  const pt = (x: number, y: number, a: number, b: number): [number, number] => [x + right[0] * a + up[0] * b, y + right[1] * a + up[1] * b];
  return { up, right, pt };
}

/** Glowing fungus, seen from a high bird's-eye angle (upright things foreshortened, their tops showing): a clump of
 * luminous mushrooms standing out of a dark patch of ground — pale stems curving gently, caps seen mostly from above
 * (a glowing top, the darker gilled rim showing below), the tallest at the back — a wide soft glow on the floor, a
 * few spores drifting. */
export function fungus(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, right } = screenAxes(light);
  const rot = Math.atan2(right[1], right[0]);
  const c = v % 2 ? [0x3a2f4c, 0x66507e, 0x8a789e, 0xc4bad0] : [0x24433e, 0x3f7a70, 0x68968c, 0xb4cdc4];
  // [right, up the screen, cap radius]: seen from about 60 degrees up, so the caps are fat ovals; back ones first.
  const caps: [number, number, number][] = [
    [-0.06, 0.12, 0.2],
    [0.2, 0.04, 0.15],
    [-0.24, -0.06, 0.13],
    [0.05, -0.16, 0.16],
    [0.28, -0.22, 0.09],
  ].map(([a, d, r]) => [a + (rnd() - 0.5) * 0.04, d + (rnd() - 0.5) * 0.03, r * (0.92 + 0.16 * rnd())]);
  for (const [a, d, r] of caps) {
    const [x, y] = pt(cx, cy, a, d);
    // A sliver of pale stem at the foot of the cap (none on the smallest).
    if (r > 0.1) g.poly(oval(...pt(x, y, 0, -r * 0.62), r * 0.3, r * 0.26, rot, 10)).fill(0xcfc6b0).stroke({ width: LINE, color: INK });
    g.poly(oval(...pt(x, y, 0.02, -r * 0.3), r * 1.02, r * 0.72, rot, 22)).fill({ color: 0x14100c, alpha: 0.3 });
    // Mid tone to the edge; the darker tone only as a crescent on the side away from the light.
    g.poly(oval(x, y, r, r * 0.8, rot, 22)).fill(c[0]);
    g.poly(oval(...pt(x, y, -r * 0.1, r * 0.1), r * 0.9, r * 0.71, rot, 22)).fill(c[1]);
    g.poly(oval(x, y, r, r * 0.8, rot, 22)).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(oval(...pt(x, y, -r * 0.3, r * 0.26), r * 0.36, r * 0.24, rot, 12)).fill({ color: c[2], alpha: 0.85 });
    if ((a * 7 + d * 3 + v) % 2 > 1) {
      for (let k = 0; k < 7; k++) {
        const t = (k / 7) * Math.PI * 2;
        g.moveTo(...pt(x, y, Math.cos(t) * r * 0.2, Math.sin(t) * r * 0.16)).lineTo(...pt(x, y, Math.cos(t) * r * 0.62, Math.sin(t) * r * 0.5));
      }
      g.stroke({ width: 0.014, color: c[0], alpha: 0.6 });
    } else {
      for (let k = 0; k < 3; k++) g.circle(...pt(x, y, (rnd() - 0.3) * r * 0.9, (rnd() - 0.3) * r * 0.5), r * 0.07).fill({ color: c[3], alpha: 0.8 });
    }
  }
}

/** Giant mushrooms: two or three broad caps, each with a darker rim, a lit crown and pale spots, gills
 * showing under the edge on the shaded side. */
export function giantMushrooms(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const caps = [
    [0x4a2e42, 0x76566c, 0x9a7a90],
    [0x5a3c24, 0x86613f, 0xa88360],
    [0x34405a, 0x566488, 0x7e8aa8],
    [0x4a2e42, 0x76566c, 0x9a7a90],
  ][v % 4];
  const parts: [number, number, number][] = [
    [0.38, 0.58, 0.28],
    [0.68, 0.36, 0.22],
    [0.7, 0.72, 0.15],
  ];
  for (const [px, py, r] of parts) {
    const [x, y] = [f.x + px, f.y + py];
    g.circle(x - light[0] * r * 0.25, y - light[1] * r * 0.25, r).fill({ color: 0x14100c, alpha: 0.3 });
    g.circle(x, y, r).fill(caps[0]);
    for (let k = 0; k < 10; k++) {
      const a = (k / 10) * Math.PI * 2;
      if (Math.cos(a) * light[0] + Math.sin(a) * light[1] > 0.2) continue;
      g.moveTo(x + Math.cos(a) * r * 0.74, y + Math.sin(a) * r * 0.74).lineTo(x + Math.cos(a) * r * 0.95, y + Math.sin(a) * r * 0.97);
    }
    g.stroke({ width: 0.014, color: 0xd8c8b0, alpha: 0.7 });
    g.circle(x + light[0] * r * 0.12, y + light[1] * r * 0.12, r * 0.88).fill(caps[1]);
    g.circle(x, y, r).stroke({ width: LINE, color: INK });
    g.circle(x + light[0] * r * 0.35, y + light[1] * r * 0.35, r * 0.32).fill({ color: caps[2], alpha: 0.85 });
    for (let k = 0; k < 4; k++) {
      const a = rnd() * Math.PI * 2;
      const d = r * 0.55 * Math.sqrt(rnd());
      g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, r * (0.08 + 0.05 * rnd())).fill({ color: 0xf5ecd6, alpha: 0.85 });
    }
  }
}

/** Thick webs: a sheet of web strung across the square — radial threads from a centre, a spiral round them,
 * thicker sagging strands, a wrapped bundle caught in it. */
export function web(g: Graphics, f: InteriorItem, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5 + (rnd() - 0.5) * 0.1, f.y + 0.5 + (rnd() - 0.5) * 0.1];
  const n = 9;
  const ends: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + (rnd() - 0.5) * 0.3;
    const r = 0.45 + 0.08 * rnd();
    ends.push([Math.cos(a) * r, Math.sin(a) * r]);
  }
  g.poly(ends.flatMap(([ex, ey]) => [cx + ex, cy + ey])).fill({ color: 0xf5f5f4, alpha: 0.12 });
  for (const [ex, ey] of ends) g.moveTo(cx, cy).lineTo(cx + ex, cy + ey);
  g.stroke({ width: 0.022, color: 0xf5f5f4, alpha: 0.85 });
  for (const k of [0.22, 0.38, 0.54, 0.7, 0.86]) {
    g.moveTo(cx + ends[0][0] * k, cy + ends[0][1] * k);
    for (let i = 1; i <= n; i++) {
      const [px, py] = ends[(i - 1) % n];
      const [qx, qy] = ends[i % n];
      const sag = 0.85;
      g.quadraticCurveTo(cx + ((px + qx) / 2) * k * sag, cy + ((py + qy) / 2) * k * sag, cx + qx * k, cy + qy * k);
    }
  }
  g.stroke({ width: 0.014, color: 0xf5f5f4, alpha: 0.75 });
  // A wrapped bundle caught in it.
  const [bx, by] = [cx + ends[2][0] * 0.5, cy + ends[2][1] * 0.5];
  g.poly(oval(bx, by, 0.09, 0.05, rnd() * Math.PI, 12)).fill(0xe8e4d8).stroke({ width: 0.018, color: INK });
  g.moveTo(bx - 0.06, by - 0.02).lineTo(bx + 0.06, by + 0.02).stroke({ width: 0.014, color: 0x8a8478 });
}

/** Cobwebs in the angle of the wall: a fan of threads from the wall with sagging cross-strands. */
export function cobweb(g: Graphics, f: InteriorItem, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const [ox, oy] = at(0.5 + (rnd() - 0.5) * 0.5, 0);
  const n = 4 + Math.floor(rnd() * 5);
  const span = 0.55 + 0.3 * rnd();
  const lean = (rnd() - 0.5) * 0.5;
  const ends: number[][] = [];
  for (let i = 0; i < n; i++) {
    const a = Math.PI * (0.5 - span / 2 + (span * i) / (n - 1)) + lean + (rnd() - 0.5) * 0.12;
    const r = 0.3 + 0.3 * rnd();
    ends.push(at(0.5 + Math.cos(a) * r, Math.sin(a) * r));
  }
  g.poly([ox, oy, ...ends.flat()]).fill({ color: 0xf5f5f4, alpha: 0.1 });
  for (const [ex, ey] of ends) g.moveTo(ox, oy).lineTo(ex, ey);
  g.stroke({ width: 0.018, color: 0xf5f5f4, alpha: 0.7 });
  const rings = [0.3, 0.5, 0.65, 0.8, 0.9].filter(() => rnd() < 0.7).slice(0, 4);
  if (!rings.length) rings.push(0.5);
  const sag = 0.7 + 0.25 * rnd();
  for (const k of rings) {
    g.moveTo(ox + (ends[0][0] - ox) * k, oy + (ends[0][1] - oy) * k);
    for (let i = 1; i < n; i++) {
      const [px, py] = ends[i - 1];
      const [qx, qy] = ends[i];
      g.quadraticCurveTo(ox + ((px + qx) / 2 - ox) * k * sag, oy + ((py + qy) / 2 - oy) * k * sag, ox + (qx - ox) * k, oy + (qy - oy) * k);
    }
  }
  g.stroke({ width: 0.014, color: 0xf5f5f4, alpha: 0.6 });
}

/** Bat guano: one soft splattered pile of dirty brown-ochre droppings, blotched chalky white, small splats
 * sprayed out round it, a faint damp stain under it. */
export function guano(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [cx, cy] = [x + w / 2 + (rnd() - 0.5) * 0.2, y + h / 2 + (rnd() - 0.5) * 0.2];
  g.poly(patch(cx, cy, w * 0.42, h * 0.38, rnd, 0.15, 22)).fill({ color: 0x3a3020, alpha: 0.25 });
  // Splats sprayed out round the pile.
  for (let i = 0; i < 18; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.45 + 0.4 * rnd();
    const r = 0.025 + 0.035 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.9, r, r, rnd, 0.3, 8)).fill(rnd() < 0.3 ? 0xe6e0cc : 0x6a5430);
  }
  // The pile: a soft mound in rounded heaps, lighter where it rises.
  g.poly(patch(cx, cy, 0.42, 0.36, rnd, 0.14, 24)).fill(0x5a4628).stroke({ width: LINE, color: INK });
  g.poly(patch(cx - 0.04, cy - 0.04, 0.3, 0.25, rnd, 0.16, 20)).fill(0x7a6236);
  g.poly(patch(cx - 0.08, cy - 0.08, 0.16, 0.13, rnd, 0.2, 14)).fill(0x947a46);
  // Chalky white blotches over it.
  for (let i = 0; i < 9; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 * Math.sqrt(rnd());
    const r = 0.03 + 0.04 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.85, r, r * 0.8, rnd, 0.35, 9)).fill({ color: 0xece6d4, alpha: 0.9 });
  }
}

/** Damp moss: a low olive mat with a broken, feathery edge of short strokes, floor showing through gaps in it,
 * darker damp patches, a fine fuzz over it, a darker wet halo on the stone round it and a few water beads. */
export function caveMoss(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  g.poly(patch(cx, cy, 0.46, 0.42, rnd, 0.2, 22)).fill({ color: 0x1a1a10, alpha: 0.2 });
  // Patchy clumps rather than one sheet, so the floor shows between them.
  const clumps: [number, number, number][] = [];
  for (let i = 0; i < 11; i++) {
    const a = (i / 11) * Math.PI * 2 + rnd() * 0.5;
    const d = i < 3 ? 0.08 * rnd() : 0.2 + 0.1 * rnd();
    clumps.push([cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.9, 0.11 + 0.07 * rnd()]);
  }
  for (const [px, py, r] of clumps) g.poly(patch(px, py, r, r * 0.85, rnd, 0.3, 12)).fill(0x4a5a2a);
  for (const [px, py, r] of clumps) if (rnd() < 0.5) g.poly(patch(px, py, r * 0.5, r * 0.4, rnd, 0.3, 9)).fill({ color: 0x2e3a1c, alpha: 0.8 });
  // The feathery edge: short strokes fanning out from every clump's rim.
  for (const [px, py, r] of clumps) {
    for (let k = 0; k < 14; k++) {
      const a = (k / 14) * Math.PI * 2 + rnd() * 0.3;
      const [ex, ey] = [px + Math.cos(a) * r * 0.85, py + Math.sin(a) * r * 0.75];
      const L = 0.03 + 0.03 * rnd();
      g.moveTo(ex, ey).lineTo(ex + Math.cos(a + (rnd() - 0.5)) * L, ey + Math.sin(a + (rnd() - 0.5)) * L);
    }
  }
  g.stroke({ width: 0.014, color: 0x5e6e32, cap: 'round' });
  // Fuzz over the clumps.
  for (let i = 0; i < 60; i++) {
    const [px, py, r] = clumps[Math.floor(rnd() * clumps.length)];
    const a = rnd() * Math.PI * 2;
    const d = r * 0.7 * Math.sqrt(rnd());
    const b = rnd() * Math.PI;
    const [qx, qy] = [px + Math.cos(a) * d, py + Math.sin(a) * d];
    g.moveTo(qx, qy).lineTo(qx + Math.cos(b) * 0.025, qy + Math.sin(b) * 0.025);
  }
  g.stroke({ width: 0.014, color: 0x7a8a42, alpha: 0.85, cap: 'round' });
  for (let i = 0; i < 5; i++) {
    const [px, py] = [cx + (rnd() - 0.5) * 0.45, cy + (rnd() - 0.5) * 0.4];
    g.circle(px, py, 0.015).fill({ color: 0xc8e0e8, alpha: 0.8 });
    g.circle(px + light[0] * 0.005, py + light[1] * 0.005, 0.006).fill(0xffffff);
  }
}

/** Pool: still blue water with a lobed edge like the glassy pool's, no rim of rock — a darker wet margin on the
 * floor round it, pale shallows along the edge, deepening to the middle, ripple rings and a glint. */
export function cavePool(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.42, f.h * 0.4];
  const deep = v % 2 ? [0x1e4a62, 0x2e6a86, 0x4a8eaa, 0x7ab8cc] : [0x1e3e6a, 0x2e5a8a, 0x4a7eaa, 0x7aaad0];
  const [p3, p5, p2] = [rnd() * 6.3, rnd() * 6.3, rnd() * 6.3];
  const N = 44;
  const lobe = (k: number, j = 0, dx = 0, dy = 0): number[] => {
    const pts: number[] = [];
    for (let i = 0; i < N; i++) {
      const a = (i / N) * Math.PI * 2;
      const r = k * (1 + 0.12 * Math.sin(3 * a + p3) + 0.06 * Math.sin(5 * a + p5) + 0.05 * Math.sin(2 * a + p2) + (rnd() - 0.5) * j);
      pts.push(cx + dx + Math.cos(a) * rx * r, cy + dy + Math.sin(a) * ry * r);
    }
    return pts;
  };
  g.poly(lobe(1.1, 0.06)).fill({ color: 0x000000, alpha: 0.18 });
  g.poly(lobe(1)).fill(deep[3]);
  g.poly(lobe(0.9, 0, -light[0] * 0.02, -light[1] * 0.02)).fill(deep[2]);
  g.poly(lobe(0.7, 0, -light[0] * 0.05, -light[1] * 0.05)).fill(deep[1]);
  g.poly(lobe(0.42, 0, -light[0] * 0.08, -light[1] * 0.08)).fill(deep[0]);
  g.poly(lobe(0.97)).stroke({ width: 0.014, color: 0xd8f0f8, alpha: 0.7 });
  for (const [x, y, k] of [
    [cx + rx * 0.25, cy + ry * 0.2, 1],
    [cx - rx * 0.3, cy - ry * 0.1, 0.6],
  ]) {
    g.poly(oval(x, y, rx * 0.24 * k, ry * 0.07 * k, 0, 18)).stroke({ width: 0.014, color: 0xc8e4f0, alpha: 0.5 });
  }
  const la = Math.atan2(light[1], light[0]) + Math.PI / 2;
  g.poly(oval(cx + light[0] * rx * 0.42, cy + light[1] * ry * 0.42, rx * 0.24, 0.02, la, 14)).fill({ color: 0xffffff, alpha: 0.6 });
}

/** A beast's nest: a broad mound of sticks laid round and round a hollow, longer ones straying past its edge, so
 * its outline is the sticks' own and never a regular ring. Fur lines the hollow, and in it (by look) a gnawed bone,
 * a clutch of eggs, a wad of fur or a skull. */
export function beastNest(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.42;
  g.poly(patch(cx - light[0] * 0.05, cy - light[1] * 0.05, r * 1.04, r * 0.98, rnd, 0.1, 22)).fill({ color: 0x14100c, alpha: 0.3 });
  g.poly(patch(cx, cy, r * 0.96, r * 0.9, rnd, 0.12, 22)).fill(0x46362a);
  // Sticks: round the mound rather than across it, ink first, the wood over it, the upper layer paler.
  const sticks = (n: number, at: number, len: number, wood: number) => {
    const ends: number[][] = [];
    for (let i = 0; i < n; i++) {
      const a = ((i + rnd() * 0.8) / n) * Math.PI * 2;
      const d = at * (0.86 + 0.28 * rnd());
      const b = a + Math.PI / 2 + (rnd() - 0.5) * 0.9;
      const L = len * (0.7 + 0.7 * rnd());
      const [px, py] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.94];
      ends.push([px - (Math.cos(b) * L) / 2, py - (Math.sin(b) * L) / 2, px + (Math.cos(b) * L) / 2, py + (Math.sin(b) * L) / 2]);
    }
    for (const [x0, y0, x1, y1] of ends) g.moveTo(x0, y0).lineTo(x1, y1);
    g.stroke({ width: 0.058, color: INK, cap: 'round' });
    for (const [x0, y0, x1, y1] of ends) g.moveTo(x0, y0).lineTo(x1, y1);
    g.stroke({ width: 0.03, color: wood, cap: 'round' });
  };
  sticks(13, r * 0.9, 0.3, 0x5e4630);
  sticks(11, r * 0.74, 0.26, 0x7a5c3c);
  sticks(8, r * 0.62, 0.2, 0x8e6e48);
  // The hollow, lined with fur, in shadow on the side toward the light.
  const [hx, hy] = [cx + (rnd() - 0.5) * 0.04, cy + (rnd() - 0.5) * 0.04];
  g.poly(patch(hx, hy, r * 0.46, r * 0.42, rnd, 0.14, 16)).fill(0x5e5244).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(patch(hx + light[0] * 0.05, hy + light[1] * 0.05, r * 0.36, r * 0.32, rnd, 0.18, 14)).fill(0x3a3026);
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    g.poly(patch(hx + Math.cos(a) * r * 0.4, hy + Math.sin(a) * r * 0.36, 0.06, 0.04, rnd, 0.4, 9)).fill(0x8a7a6a);
  }
  switch (v % 4) {
    case 0: {
      const a = rnd() * Math.PI;
      const [c, s] = [Math.cos(a) * 0.15, Math.sin(a) * 0.15];
      bone(g, [hx - c, hy - s], [hx + c, hy + s], 0.042, light);
      break;
    }
    case 1:
      for (const [dx, dy] of [[-0.07, 0.03], [0.07, 0.04], [0, -0.07]]) {
        g.poly(oval(hx + dx, hy + dy, 0.07, 0.055, rnd() * Math.PI, 12)).fill(0xcabfa6).stroke({ width: LINE, color: INK });
        g.circle(hx + dx + light[0] * 0.02, hy + dy + light[1] * 0.02, 0.016).fill(0xe6dcc6);
      }
      break;
    case 2:
      g.poly(patch(hx, hy, 0.12, 0.09, rnd, 0.3, 11)).fill(0x8a7a6a).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(patch(hx + light[0] * 0.03, hy + light[1] * 0.03, 0.06, 0.04, rnd, 0.3, 9)).fill(0xa49484);
      break;
    default:
      skullTop(g, hx, hy, rnd() * Math.PI * 2, 0.1, light);
  }
}

/** A rat nest: a soft loose heap of shredded straw with a frayed striped rag in it, a chewed candle and droppings
 * round it, rats with long pink tails. */
export function ratNest(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.3;
  for (let i = 0; i < 10; i++) g.poly(oval(cx + (rnd() - 0.5) * 0.8, cy + (rnd() - 0.5) * 0.8, 0.012, 0.007, rnd() * Math.PI, 6)).fill(0x2a2018);
  // A chewed candle stub at the edge.
  const [kx, ky] = [cx + r * 1.1, cy - r * 0.6];
  g.poly(oval(kx, ky, 0.07, 0.03, 0.5, 10)).fill(0xd8ccae).stroke({ width: LINE, color: INK });
  g.circle(kx + 0.05, ky + 0.02, 0.012).fill(0x2a2018);
  // A soft loose heap: overlapping unlined lumps, a broken thin edge, a small dark hollow.
  g.poly(patch(cx + 0.01, cy + 0.02, r * 1.02, r * 0.9, rnd, 0.14, 20)).fill({ color: 0x14100c, alpha: 0.22 });
  g.poly(patch(cx, cy, r, r * 0.9, rnd, 0.12, 20)).fill(0x6e6048);
  g.poly(patch(cx - light[0] * 0.03, cy - light[1] * 0.03, r * 0.82, r * 0.72, rnd, 0.16, 18)).fill(0x84745a);
  g.poly(patch(cx - light[0] * 0.08, cy - light[1] * 0.08, r * 0.45, r * 0.38, rnd, 0.2, 14)).fill({ color: 0xa08e6c, alpha: 0.7 });
  for (let i = 0; i < 3; i++) {
    const a0 = rnd() * Math.PI * 2;
    g.moveTo(cx + Math.cos(a0) * r, cy + Math.sin(a0) * r * 0.9);
    for (let k = 1; k <= 5; k++) g.lineTo(cx + Math.cos(a0 + k * 0.17) * r * (0.97 + 0.06 * rnd()), cy + Math.sin(a0 + k * 0.17) * r * 0.9 * (0.97 + 0.06 * rnd()));
  }
  g.stroke({ width: LINE, color: 0x3a2e1e, alpha: 0.8, cap: 'round' });
  g.poly(patch(cx + 0.03, cy + 0.04, r * 0.22, r * 0.17, rnd, 0.2, 10)).fill(0x2a2018);
  // Straws crossing.
  for (let i = 0; i < 14; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * (0.5 + 0.5 * rnd());
    const b = a + Math.PI / 2 + (rnd() - 0.5) * 1.2;
    const L = r * 0.35 * (0.7 + 0.6 * rnd());
    const [px, py] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
    g.moveTo(px - (Math.cos(b) * L) / 2, py - (Math.sin(b) * L) / 2).lineTo(px + (Math.cos(b) * L) / 2, py + (Math.sin(b) * L) / 2);
  }
  g.stroke({ width: 0.014, color: 0x5a4a2a, cap: 'round' });
  // A few short stray straws just outside the heap.
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const [px, py] = [cx + Math.cos(a) * r * 1.05, cy + Math.sin(a) * r * 0.95];
    const b = a + (rnd() - 0.5) * 1.4;
    g.moveTo(px, py).lineTo(px + Math.cos(b) * 0.09, py + Math.sin(b) * 0.09);
  }
  g.stroke({ width: 0.02, color: 0x7a6a48, cap: 'round' });
  // One rag: a frayed, striped patch of cloth.
  {
    const a = rnd() * Math.PI * 2;
    const [px, py] = [cx + Math.cos(a) * r * 0.35, cy + Math.sin(a) * r * 0.35];
    const [co, si] = [Math.cos(a + 0.6), Math.sin(a + 0.6)];
    const q = (u: number, k: number): [number, number] => [px + co * u - si * k, py + si * u + co * k];
    g.poly([...q(-0.1, -0.07), ...q(0.1, -0.07), ...q(0.1, 0.07), ...q(-0.1, 0.07)]).fill(0x74524a).stroke({ width: 0.018, color: INK });
    g.moveTo(...q(-0.1, 0)).lineTo(...q(0.1, 0)).stroke({ width: 0.018, color: 0x9a766a });
    for (let k = -0.06; k <= 0.06; k += 0.03) g.moveTo(...q(0.1, k)).lineTo(...q(0.13, k + 0.01));
    g.stroke({ width: 0.014, color: 0x74524a });
  }
  // Rats: bigger teardrop bodies with long pink tails curling away from the nest.
  for (let i = 0; i < 2; i++) {
    const a = i * 2.8 + rnd() * 0.6;
    const [rx, ry] = [cx + Math.cos(a) * r * 1.05, cy + Math.sin(a) * r * 1.05];
    const [c, s] = [Math.cos(a), Math.sin(a)];
    g.moveTo(rx + c * 0.08, ry + s * 0.08).quadraticCurveTo(rx + c * 0.26 - s * 0.1, ry + s * 0.26 + c * 0.1, rx + c * 0.3 + s * 0.06, ry + s * 0.3 - c * 0.06).stroke({ width: 0.024, color: 0xa88e82, cap: 'round' });
    g.poly([rx - c * 0.13, ry - s * 0.13, rx - s * 0.065, ry + c * 0.065, rx + c * 0.1, ry + s * 0.1, rx + s * 0.065, ry - c * 0.065]).fill(0x6a5e52).stroke({ width: 0.018, color: INK, join: 'round' });
    for (const k of [1, -1]) g.circle(rx - c * 0.07 - s * k * 0.035, ry - s * 0.07 + c * k * 0.035, 0.018).fill(0xa08a80);
    g.circle(rx - c * 0.12, ry - s * 0.12, 0.01).fill(INK);
  }
}

/** Sheet ice: a pale, glassy sheet over the floor with a soft irregular edge, frosted rim, cracks and glints. */
export function iceSheet(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.46, f.h * 0.44];
  const out = patch(cx, cy, rx, ry, rnd, 0.12, 26);
  g.poly(out).fill({ color: 0xe8f4fc, alpha: 0.85 }).stroke({ width: 0.025, color: 0x8ab0c8, join: 'round' });
  g.poly(patch(cx - light[0] * 0.05, cy - light[1] * 0.05, rx * 0.82, ry * 0.82, rnd, 0.1, 22)).fill({ color: 0xbcdcf0, alpha: 0.7 });
  for (let i = 0; i < 4; i++) {
    let [px, py] = [cx + (rnd() - 0.5) * rx, cy + (rnd() - 0.5) * ry];
    const a = rnd() * Math.PI * 2;
    g.moveTo(px, py);
    for (let k = 0; k < 4; k++) {
      px += Math.cos(a + (rnd() - 0.5)) * 0.15;
      py += Math.sin(a + (rnd() - 0.5)) * 0.15;
      g.lineTo(px, py);
    }
  }
  g.stroke({ width: 0.016, color: 0x6a9ab8, alpha: 0.85 });
  for (let i = 0; i < 4; i++) {
    const [px, py] = [cx + (rnd() - 0.5) * rx * 1.2, cy + (rnd() - 0.5) * ry * 1.2];
    g.moveTo(px - 0.07, py + 0.03).lineTo(px + 0.07, py - 0.03).stroke({ width: 0.025, color: 0xffffff, alpha: 0.75, cap: 'round' });
  }
}

const OBSIDIAN = [0x121018, 0x2a2634, 0x4a4560, 0x8a84a8];
const BASALT = [0x1c1a1c, 0x2e2b2e, 0x46424a, 0x66606a];

/** One flat glassy chip lying on the floor round (x, y) along `a`: a sharp irregular blade split by a ridge, its
 * half toward the light catching a pale sheen, curved ripples (glass breaks in shells), a bright cutting edge. */
function glassChip(g: Graphics, x: number, y: number, a: number, len: number, w: number, light: Light, rnd: () => number) {
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const p = (u: number, k: number): [number, number] => [x + c * u - s * k, y + s * u + c * k];
  const T = p(len, (rnd() - 0.5) * w * 0.4);
  const L = p(len * (0.05 + 0.2 * rnd()), w * (0.8 + 0.3 * rnd()));
  const Bl = p(-len * (0.45 + 0.2 * rnd()), w * (0.3 + 0.3 * rnd()));
  const Br = p(-len * (0.3 + 0.3 * rnd()), -w * (0.4 + 0.3 * rnd()));
  const R = p(len * (0.15 + 0.2 * rnd()), -w * (0.7 + 0.3 * rnd()));
  const M = p(-len * 0.1, (rnd() - 0.5) * w * 0.3);
  const lit = -s * light[0] + c * light[1] > 0;
  const sd = lit ? -1 : 1;
  castShadow(g, [...T, ...L, ...Bl, ...Br, ...R], light, 0.06);
  g.poly([...T, ...L, ...Bl, ...M]).fill(lit ? OBSIDIAN[2] : OBSIDIAN[0]);
  g.poly([...T, ...M, ...Br, ...R]).fill(lit ? OBSIDIAN[0] : OBSIDIAN[2]);
  g.poly([...M, ...Bl, ...Br]).fill(OBSIDIAN[1]);
  // Shell ripples on the dark half, the ridge, the outline.
  for (const k of [0.35, 0.6]) {
    g.moveTo(...p(len * k, sd * w * 0.15)).quadraticCurveTo(...p(len * (k - 0.05), sd * w * 0.55), ...p(len * (k - 0.25), sd * w * 0.65));
  }
  g.stroke({ width: 0.016, color: OBSIDIAN[2], alpha: 0.8 });
  g.moveTo(...T).lineTo(...M).lineTo(...Bl).moveTo(...M).lineTo(...Br).stroke({ width: 0.016, color: INK, alpha: 0.7 });
  g.poly([...T, ...L, ...Bl, ...Br, ...R]).stroke({ width: OUT, color: INK, join: 'miter' });
  g.moveTo(...(lit ? L : R)).lineTo(...T).stroke({ width: 0.014, color: OBSIDIAN[3], cap: 'round' });
  g.moveTo(...p(len * 0.3, -sd * w * 0.3)).lineTo(...p(len * 0.7, -sd * w * 0.12)).stroke({ width: 0.014, color: 0xffffff, alpha: 0.8, cap: 'round' });
}

/** Obsidian shards: a scatter of broken black volcanic glass, a few big blades and small chips, glossy and sharp. */
export function obsidianShards(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0, n = 4 + Math.floor(rnd() * 8); i < n; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.15 + 0.3 * rnd();
    glassChip(g, cx + Math.cos(a) * d, cy + Math.sin(a) * d, rnd() * Math.PI * 2, 0.04 + 0.04 * rnd(), 0.025 + 0.01 * rnd(), light, rnd);
  }
  // Two to five big blades at random places and angles, sizes drawn fresh each time.
  const nb = 2 + Math.floor(rnd() * 4);
  const big: [number, number, number, number, number][] = [];
  for (let i = 0; i < nb; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.28 * Math.sqrt(rnd());
    const len = (0.3 - i * 0.04) * (0.6 + 0.6 * rnd());
    big.push([Math.cos(a) * d, Math.sin(a) * d, ph + i * (1.2 + rnd() * 1.4), len, len * (0.32 + 0.12 * rnd())]);
  }
  for (const [dx, dy, a, len, w] of big) glassChip(g, cx + dx, cy + dy, a, len, w, light, rnd);
}

/** Basalt column: a clump of tight six-sided columns seen from above, standing to different heights: each a
 * flat hexagonal top with a lit rim, its side showing away from the light, the tallest throwing the longest
 * shadow; cracks across the tops. */
export function basaltColumns(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.13;
  const sp = r * Math.sqrt(3) * 1.02;
  const ph = rnd() * 0.5;
  const gap = Math.floor(rnd() * 6);
  const cols: [number, number, number][] = [[cx, cy, 1]];
  for (let i = 0; i < 6; i++) {
    if (i === gap) continue;
    const a = ph + Math.PI / 6 + (i * Math.PI) / 3;
    cols.push([cx + Math.cos(a) * sp, cy + Math.sin(a) * sp, 0.3 + 0.6 * rnd()]);
  }
  const hex = (x: number, y: number, k: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 6; i++) pts.push(x + Math.cos(ph + (i * Math.PI) / 3) * r * k, y + Math.sin(ph + (i * Math.PI) / 3) * r * k);
    return pts;
  };
  for (const [x, y, h] of cols) castShadow(g, hex(x, y, 1), light, 0.12 + h * 0.25);
  cols.sort((p, q) => p[2] - q[2]);
  for (const [x, y, h] of cols) {
    const off = 0.03 + h * 0.08;
    const top = hex(x, y, 1);
    const low = hex(x - light[0] * off, y - light[1] * off, 1);
    // The side: the top swept away from the light, dark, its edges ruled.
    g.poly(low).fill(BASALT[0]).stroke({ width: OUT, color: INK, join: 'round' });
    for (let i = 0; i < 6; i++) {
      const j = (i + 1) % 6;
      g.poly([top[i * 2], top[i * 2 + 1], top[j * 2], top[j * 2 + 1], low[j * 2], low[j * 2 + 1], low[i * 2], low[i * 2 + 1]]).fill(BASALT[0]);
    }
    for (let i = 0; i < 6; i++) g.moveTo(top[i * 2], top[i * 2 + 1]).lineTo(low[i * 2], low[i * 2 + 1]);
    g.stroke({ width: 0.016, color: INK, alpha: 0.8 });
    const tone = shade(BASALT[2], 0.7 + h * 0.6);
    g.poly(top).fill(tone).stroke({ width: 0.022, color: INK, join: 'round' });
    g.poly(hex(x + light[0] * 0.012, y + light[1] * 0.012, 0.72)).fill(shade(tone, 1.12));
    // The rim toward the light catches it.
    for (let i = 0; i < 6; i++) {
      const j = (i + 1) % 6;
      const [mx, my] = [(top[i * 2] + top[j * 2]) / 2 - x, (top[i * 2 + 1] + top[j * 2 + 1]) / 2 - y];
      if (mx * light[0] + my * light[1] > 0.02) g.moveTo(top[i * 2], top[i * 2 + 1]).lineTo(top[j * 2], top[j * 2 + 1]);
    }
    g.stroke({ width: 0.016, color: BASALT[3], cap: 'round' });
    const a = rnd() * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * r * 0.8, y + Math.sin(a) * r * 0.8).lineTo(x + (rnd() - 0.5) * r * 0.4, y + (rnd() - 0.5) * r * 0.4).stroke({ width: 0.016, color: INK, alpha: 0.7 });
  }
}

/** Steam vent: a dark wet hole in a low cone of rock, white salt crust round its lip, the floor damp round it, a
 * big plume of white steam rolling off to one side over the rim. */
export function steamVent(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  // The damp floor: a darker glossy patch, droplets.
  g.poly(patch(cx, cy, 0.44, 0.42, rnd, 0.12, 20)).fill({ color: 0x000000, alpha: 0.2 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.7) * 0.38, cy + Math.sin(la - 0.7) * 0.38).arc(cx, cy, 0.38, la - 0.7, la + 0.7).stroke({ width: 0.02, color: 0xffffff, alpha: 0.15, cap: 'round' });
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.32 + 0.12 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.01 + 0.008 * rnd()).fill({ color: 0xd8e4ec, alpha: 0.6 });
  }
  g.poly(patch(cx, cy, 0.29, 0.27, rnd, 0.14, 16)).fill(BASALT[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.03, cy + light[1] * 0.03, 0.22, 0.2, rnd, 0.14, 14)).fill(BASALT[2]);
  // Salt crust on the lip, then the hole: dark, a wet sheen on its far edge.
  g.poly(patch(cx, cy, 0.16, 0.14, rnd, 0.22, 14)).fill(0xe8e4d8).stroke({ width: 0.018, color: INK, join: 'round' });
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    g.circle(cx + Math.cos(a) * 0.17, cy + Math.sin(a) * 0.15, 0.012 + 0.01 * rnd()).fill(0xf4f0e6);
  }
  const hole = patch(cx, cy, 0.085, 0.07, rnd, 0.2, 12);
  g.poly(hole).fill(0x0a0a0c).stroke({ width: 0.018, color: INK, join: 'round' });
  g.moveTo(cx - light[0] * 0.06 - light[1] * 0.04, cy - light[1] * 0.06 + light[0] * 0.04)
    .lineTo(cx - light[0] * 0.06 + light[1] * 0.04, cy - light[1] * 0.06 - light[0] * 0.04)
    .stroke({ width: 0.014, color: 0x8aa4b4, alpha: 0.8, cap: 'round' });
  // The plume: rolling clusters of puffs, growing and fading as they drift, the first over the hole and rim.
  const drift = la + Math.PI + (rnd() - 0.5) * 0.8;
  const [dc, ds] = [Math.cos(drift), Math.sin(drift)];
  for (let i = 0; i < 5; i++) {
    const k = i / 4;
    const d = 0.08 + k * 0.36;
    const bend = Math.sin(k * 3.5 + 1) * 0.07;
    const [px, py] = [cx + dc * d - ds * bend, cy + ds * d + dc * bend];
    const rr = 0.07 + k * 0.08;
    const a = 0.6 - k * 0.3;
    for (const [ox, oy, kk] of [
      [-0.6, 0.9, 0.7],
      [0.5, -0.9, 0.65],
      [0, 0, 1],
    ]) {
      const [qx, qy] = [px + (dc * ox - ds * oy) * rr, py + (ds * ox + dc * oy) * rr];
      g.circle(qx, qy, rr * kk).fill({ color: 0xe8ecf0, alpha: a });
    }
    g.circle(px + light[0] * rr * 0.3, py + light[1] * rr * 0.3, rr * 0.55).fill({ color: 0xffffff, alpha: a * 0.8 });
  }
}

/** Sulfur crust: one flat ragged patch of dull ochre mineral crust round a dark vent crack, inked, with a few
 * short needle crystals on it and rust stains at its edge. Reads as a crust, not as dots. */
export function sulfurCrust(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.3 + 0.06 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.06 + 0.03 * rnd(), 0.05 + 0.02 * rnd(), rnd, 0.3, 9)).fill({ color: 0x8a5a2a, alpha: 0.5 });
  }
  const edge = patch(cx, cy, 0.37, 0.33, rnd, 0.14, 26);
  castShadow(g, edge, light, 0.06);
  g.poly(edge).fill(0x726232).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(patch(cx + light[0] * 0.025, cy + light[1] * 0.025, 0.28, 0.25, rnd, 0.14, 22)).fill(0x927f42);
  for (let i = 0; i < 2; i++) {
    const a = Math.atan2(light[1], light[0]) + (rnd() - 0.5) * 2;
    const d = 0.1 + 0.1 * rnd();
    g.poly(patch(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.09 + 0.04 * rnd(), 0.07 + 0.03 * rnd(), rnd, 0.3, 10)).fill(0xa8965a);
  }
  // The vent crack: a dark slit in a raised rim of crust.
  const ca = rnd() * Math.PI;
  const [kc, ks] = [Math.cos(ca), Math.sin(ca)];
  const q = (u: number, k: number): [number, number] => [cx + kc * u - ks * k, cy + ks * u + kc * k];
  g.poly([...q(-0.14, 0), ...q(-0.05, 0.045), ...q(0.04, 0.03), ...q(0.13, 0.04), ...q(0.06, -0.03), ...q(-0.04, -0.04)]).fill(0xa8965a).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...q(-0.1, 0), ...q(-0.04, 0.02), ...q(0.03, 0.01), ...q(0.09, 0.016), ...q(0.05, -0.012), ...q(-0.03, -0.016)]).fill(INK);
  // A few short needle crystals near the crack.
  for (let i = 0; i < 9; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.1 + 0.18 * Math.sqrt(rnd());
    const [x, y] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d * 0.94];
    const b = a + (rnd() - 0.5) * 1.2;
    const L = 0.05 + 0.04 * rnd();
    g.moveTo(x, y).lineTo(x + Math.cos(b) * L, y + Math.sin(b) * L).stroke({ width: 0.016, color: 0xd8c878, cap: 'round' });
  }
}

/** Scorched remains: someone burnt where they fell — a black soot burst on the floor round the body, grey ash
 * over it, charred bones (a blackened skull, ribs, long bones), a few embers still glowing. */
export function scorchedRemains(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const long = f.h >= f.w;
  // A body about 5.5 ft long (a square is 5 ft): B scales the positions along it.
  const B = 1.4;
  // Along the body (head end first) and across it.
  const p = (u: number, k: number): [number, number] => (long ? [cx + k, cy + u] : [cx + u, cy + k]);
  const burst: number[] = [];
  const n = 28;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const spike = i % 2 ? 0.75 + 0.2 * rnd() : 1 + 0.3 * rnd();
    burst.push(...p(Math.sin(a) * B * 0.5 * spike, Math.cos(a) * 0.4 * spike));
  }
  g.poly(burst).fill({ color: 0x0e0a08, alpha: 0.5 });
  g.poly(patch(cx, cy, long ? 0.3 : B * 0.44, long ? B * 0.44 : 0.3, rnd, 0.18, 22)).fill({ color: 0x0e0a08, alpha: 0.75 });
  // Grey ash in drifts over it.
  for (let i = 0; i < 6; i++) {
    const [x, y] = p((rnd() - 0.5) * B * 0.6, (rnd() - 0.5) * 0.4);
    g.poly(patch(x, y, 0.07 + 0.05 * rnd(), 0.05 + 0.04 * rnd(), rnd, 0.3, 10)).fill({ color: 0x8a8480, alpha: 0.55 });
  }
  const bone = 0x6a5e50;
  const ash = 0xd0c8b8;
  const limb = (a: [number, number], b: [number, number], w: number) => {
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: INK, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w - 0.025, color: bone, cap: 'round' });
    g.circle(...a, w * 0.75).fill(bone).stroke({ width: 0.018, color: INK });
    g.circle(...b, w * 0.75).fill(bone).stroke({ width: 0.018, color: INK });
    g.moveTo(...a).lineTo(...b).stroke({ width: 0.014, color: ash, alpha: 0.7, cap: 'round' });
  };
  limb(p(B * 0.12, 0.06), p(B * 0.36, 0.1), 0.045);
  limb(p(B * 0.1, -0.08), p(B * 0.32, -0.14), 0.042);
  limb(p(-B * 0.18, 0.2), p(-B * 0.02, 0.24), 0.035);
  // Ribs: curved bars either side of a spine.
  g.moveTo(...p(-B * 0.24, 0)).lineTo(...p(B * 0.06, 0)).stroke({ width: 0.04, color: INK, cap: 'round' });
  g.moveTo(...p(-B * 0.24, 0)).lineTo(...p(B * 0.06, 0)).stroke({ width: 0.02, color: bone, cap: 'round' });
  for (let i = 0; i < 4; i++) {
    const u = -B * 0.2 + i * B * 0.055;
    for (const sd of [-1, 1]) {
      for (const [w, c] of [
        [0.032, INK],
        [0.015, bone],
      ]) {
        g.moveTo(...p(u, sd * 0.02)).quadraticCurveTo(...p(u - 0.02, sd * 0.14), ...p(u + 0.03, sd * 0.17));
        g.stroke({ width: w, color: c, cap: 'round' });
      }
    }
  }
  // The skull at the head end: blackened, ash-grey on top, dark sockets.
  const [hx, hy] = p(-B * 0.36, 0.02);
  g.poly(oval(hx, hy, 0.085, 0.095, 0, 16)).fill(bone).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oval(hx + light[0] * 0.025, hy + light[1] * 0.025, 0.05, 0.05, 0, 12)).fill({ color: ash, alpha: 0.8 });
  g.circle(...p(-B * 0.36 + 0.025, -0.025), 0.022).fill(0x080604);
  g.circle(...p(-B * 0.36 + 0.025, 0.055), 0.022).fill(0x080604);
  g.circle(...p(-B * 0.36 + 0.06, 0.015), 0.01).fill(0x080604);
  // Embers.
  for (let i = 0; i < 7; i++) {
    const [x, y] = p((rnd() - 0.5) * B * 0.7, (rnd() - 0.5) * 0.5);
    g.circle(x, y, 0.03).fill({ color: 0xf08030, alpha: 0.25 });
    g.circle(x, y, 0.012 + 0.008 * rnd()).fill(i % 2 ? 0xf8a040 : 0xe05818);
  }
}

/** Cooled glassy pool: a flat sheet of black volcanic glass where lava spread and set, lobed like a spill: a
 * lumpy basalt rim merging into the floor, ropy folds frozen near its edge, angular cracks (one still faintly
 * red), crisp mirror glints, obsidian chips at the rim. */
export function glassPool(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [rx, ry] = [f.w * 0.42, f.h * 0.4];
  const [p3, p5, p2] = [rnd() * 6.3, rnd() * 6.3, rnd() * 6.3];
  const N = 44;
  const lobe = (k: number, j = 0): number[] => {
    const pts: number[] = [];
    for (let i = 0; i < N; i++) {
      const a = (i / N) * Math.PI * 2;
      const r = k * (1 + 0.12 * Math.sin(3 * a + p3) + 0.06 * Math.sin(5 * a + p5) + 0.05 * Math.sin(2 * a + p2) + (rnd() - 0.5) * j);
      pts.push(cx + Math.cos(a) * rx * r, cy + Math.sin(a) * ry * r);
    }
    return pts;
  };
  g.poly(lobe(1.12, 0.1)).fill({ color: BASALT[1], alpha: 0.45 });
  g.poly(lobe(1.02, 0.12)).fill(BASALT[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(lobe(0.98, 0.1)).fill(BASALT[2]);
  const glass = lobe(0.84, 0.03);
  g.poly(glass).fill(0x0c0a10).stroke({ width: LINE, color: INK, join: 'round' });
  // Ropy folds: partial rings following the edge, bunched near it.
  for (const k of [0.76, 0.68, 0.6]) {
    const ring = lobe(k, 0.02);
    const start = Math.floor(rnd() * N);
    const len = Math.floor(N * (0.3 + 0.25 * rnd()));
    for (const [w, c, a, o] of [
      [0.026, OBSIDIAN[1], 0.9, 0],
      [0.01, OBSIDIAN[2], 0.7, 0.012],
    ]) {
      for (let s = 0; s < 2; s++) {
        const i0 = (start + s * Math.floor(N / 2)) % N;
        g.moveTo(ring[i0 * 2] + light[0] * o, ring[i0 * 2 + 1] + light[1] * o);
        for (let i = 1; i < len; i++) {
          const ii = (i0 + i) % N;
          const wob = Math.sin(i * 1.9) * 0.012;
          g.lineTo(ring[ii * 2] + light[0] * o + wob, ring[ii * 2 + 1] + light[1] * o + wob);
        }
        g.stroke({ width: w, color: c, alpha: a, cap: 'round', join: 'round' });
      }
    }
  }
  // Angular cracks; the first glows dull red deep down.
  const away = Math.atan2(-light[1], -light[0]);
  for (let c = 0; c < 3; c++) {
    const a = away + (c - 1) * 1.4 + (rnd() - 0.5) * 0.5;
    let [x, y] = [cx + Math.cos(a) * rx * 0.2, cy + Math.sin(a) * ry * 0.2];
    const pts: number[] = [x, y];
    for (let k = 0; k < 4; k++) {
      const b = a + (rnd() - 0.5) * 1.2;
      x += Math.cos(b) * rx * 0.13;
      y += Math.sin(b) * ry * 0.13;
      pts.push(x, y);
    }
    const line = () => {
      g.moveTo(pts[0], pts[1]);
      for (let i = 2; i < pts.length; i += 2) g.lineTo(pts[i], pts[i + 1]);
    };
    if (c === 0) {
      line();
      g.stroke({ width: 0.05, color: 0xa02a10, alpha: 0.35, cap: 'round', join: 'miter' });
      line();
      g.stroke({ width: 0.022, color: 0xc03a14, alpha: 0.7, cap: 'round', join: 'miter' });
    }
    line();
    g.stroke({ width: 0.014, color: c === 0 ? 0x3a0a04 : OBSIDIAN[3], alpha: c === 0 ? 1 : 0.55, cap: 'round', join: 'miter' });
  }
  // Mirror glints: two crisp streaks toward the light and a star.
  const la = Math.atan2(light[1], light[0]) + Math.PI / 2;
  g.poly(oval(cx + light[0] * rx * 0.42, cy + light[1] * ry * 0.42, rx * 0.26, 0.022, la, 14)).fill({ color: 0xffffff, alpha: 0.85 });
  g.poly(oval(cx + light[0] * rx * 0.3 + Math.cos(la) * 0.12, cy + light[1] * ry * 0.3 + Math.sin(la) * 0.12, rx * 0.12, 0.014, la, 10)).fill({ color: 0xffffff, alpha: 0.7 });
  const [sx, sy] = [cx - light[0] * rx * 0.35 + Math.cos(la) * 0.1, cy - light[1] * ry * 0.35 + Math.sin(la) * 0.1];
  g.poly([sx, sy - 0.05, sx + 0.012, sy, sx, sy + 0.05, sx - 0.012, sy]).fill({ color: 0xffffff, alpha: 0.8 });
  g.poly([sx - 0.05, sy, sx, sy - 0.012, sx + 0.05, sy, sx, sy + 0.012]).fill({ color: 0xffffff, alpha: 0.8 });
  // Lumps on the rim, obsidian chips by it.
  for (let i = 0; i < 7; i++) {
    const a = ((i + rnd() * 0.7) / 7) * Math.PI * 2;
    rock(g, cx + Math.cos(a) * rx * 1.0, cy + Math.sin(a) * ry * 1.0, 0.06 + 0.05 * rnd(), BASALT, light, rnd, 6);
  }
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    glassChip(g, cx + Math.cos(a) * rx * 1.12, cy + Math.sin(a) * ry * 1.12, rnd() * Math.PI * 2, 0.06 + 0.04 * rnd(), 0.03, light, rnd);
  }
}
