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

/**
 * A ladder standing against the wall, seen from straight above (`ladder`, `grateLadder`, the ruins'
 * `climbLadder`): it leans a little, its feet about a foot out from the wall and its top at the wall, so
 * from above it is strongly foreshortened, a fifth of a square deep and a foot and a half wide. The rail tops
 * at the wall are nearest the eye (bold ends, the lightest tone); the rails taper down and out from them to
 * two small feet; the rungs between bunch closer toward the wall, each overlapping the one below. What tells
 * its height is its shadow: a long, ladder-shaped soft shadow cast on the floor away from the light from the
 * feet. `tones`: dark, mid, light (`WOOD`, `IRON`); `u` the middle of the ladder along the wall.
 */
export function standingLadder(g: Graphics, f: InteriorItem, light: Light, wall: [number, number], tones: readonly number[] = WOOD, u = 0.5) {
  const { at } = frame(f, wall);
  const [lx, ly] = light;
  // The light's way in the ladder's own axes (u along the wall, s out from it).
  const [o, au, as] = [at(0, 0), at(1, 0), at(0, 1)];
  const lu = lx * (au[0] - o[0]) + ly * (au[1] - o[1]);
  const ls = lx * (as[0] - o[0]) + ly * (as[1] - o[1]);
  const half = 0.15;
  const [foot, top] = [0.27, 0.03];
  // Up the ladder (t: 0 the feet, 1 the top) to s out from the wall: the rungs bunch toward the top.
  const sAt = (t: number) => foot + (top - foot) * Math.pow(t, 0.75);
  // The shadow, away from the light and out onto the floor (a light behind the wall still throws it clear).
  let [du, ds] = [-lu, Math.max(-ls, 0.35)];
  const dn = Math.hypot(du, ds) || 1;
  [du, ds] = [du / dn, ds / dn];
  const L = 0.85;
  const sp = (side: number, t: number) => at(u + side * half + du * L * t, foot + (top - foot) * t + ds * L * t);
  const RUNGS = [0.18, 0.4, 0.62, 0.84];
  for (const [w, a] of [[0.08, 0.1], [0.048, 0.18]]) {
    for (const side of [-1, 1]) g.moveTo(...sp(side, 0)).lineTo(...sp(side, 1.04));
    g.stroke({ width: w, color: 0x14100c, alpha: a, cap: 'round' });
    for (const t of RUNGS) g.moveTo(...sp(-1, t)).lineTo(...sp(1, t));
    g.stroke({ width: w * 0.8, color: 0x14100c, alpha: a, cap: 'butt' });
  }
  // The feet, each with a scuff of shadow.
  for (const side of [-1, 1]) {
    const [fx, fy] = at(u + side * half, foot);
    g.poly(oval(fx - lx * 0.012, fy - ly * 0.012, 0.04, 0.04, 0, 10)).fill({ color: 0x14100c, alpha: 0.22 });
    g.circle(fx, fy, 0.024).fill(tones[0]).stroke({ width: LINE, color: INK });
  }
  // The rungs, from the feet up: each a little bigger than the one below (nearer the eye), its top edge lit.
  RUNGS.forEach((t, k) => {
    const s = sAt(t);
    const w = 0.026 + k * 0.003;
    const [a, b] = [at(u - half, s), at(u + half, s)];
    g.moveTo(...a).lineTo(...b).stroke({ width: w + 2 * LINE, color: INK, cap: 'butt' });
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: tones[1], cap: 'butt' });
    const [c, d] = [at(u - half, s - w * 0.28), at(u + half, s - w * 0.28)];
    g.moveTo(...c).lineTo(...d).stroke({ width: 0.014, color: tones[2], cap: 'butt' });
  });
  // The rails, tapering from the top (near) to the feet (far), lit down the side toward the light.
  const litSide = lu < 0 ? -1 : 1;
  for (const side of [-1, 1]) {
    const x = u + side * half;
    const [h0, h1] = [0.016, 0.026];
    g.poly([...at(x - h0, foot), ...at(x + h0, foot), ...at(x + h1, top), ...at(x - h1, top)]).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...at(x + litSide * h0 * 0.4, foot)).lineTo(...at(x + litSide * h1 * 0.45, top)).stroke({ width: 0.014, color: tones[2] });
  }
  // The rail tops at the wall, nearest the eye: bold ends in the lightest tone, a glint toward the light.
  const pale = shade(tones[2], 1.16);
  for (const side of [-1, 1]) {
    const [cx, cy] = at(u + side * half, top);
    if (tones === IRON) g.circle(cx, cy, 0.036).fill(pale).stroke({ width: LINE, color: INK });
    else g.roundRect(cx - 0.036, cy - 0.036, 0.072, 0.072, 0.018).fill(pale).stroke({ width: LINE, color: INK });
    g.circle(cx + lx * 0.012, cy + ly * 0.012, 0.014).fill(shade(pale, 1.12));
  }
}

/** A ladder against the wall, up to a grate in the street above: `standingLadder` over a square of grey light
 * falling through the grate onto its foot (a ladder turned to a curving wall has none: a tilted box reads as a
 * pale slab). */
export function ladder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number], turned = false, tones: readonly number[] = WOOD) {
  const { at, rect } = frame(f, wall);
  if (!turned) {
    g.poly(quad(rect(0.2, 0.02, 0.8, 0.62), rnd, 0)).fill({ color: 0xd8dce0, alpha: 0.18 });
    for (let k = 1; k < 4; k++) g.moveTo(...at(0.2 + k * 0.15, 0.02)).lineTo(...at(0.2 + k * 0.15, 0.62));
    g.stroke({ width: 0.03, color: 0x14100c, alpha: 0.25 });
  }
  standingLadder(g, f, light, wall, tones);
}

/** A sewer's way out (`exit` named for a street grate), seen from above: an iron `standingLadder` against the
 * wall and the grate's daylight on the floor at its foot, a pale warm rectangle (half a square by two fifths)
 * crossed by the shadows of five bars, in a soft warm halo. */
export function grateLadder(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const SUN = 0xffe6a0;
  const [cx, cy] = at(0.5, 0.26);
  g.poly(patch(cx, cy, 0.42, 0.36, rnd, 0.1)).fill({ color: SUN, alpha: 0.12 });
  g.poly(patch(cx, cy, 0.33, 0.28, rnd, 0.08)).fill({ color: SUN, alpha: 0.14 });
  const [s0, s1] = [0.05, 0.47];
  g.poly([...at(0.25, s0), ...at(0.75, s0), ...at(0.75, s1), ...at(0.25, s1)]).fill({ color: 0xfff0c4, alpha: 0.4 });
  for (let k = 0; k < 5; k++) g.moveTo(...at(0.3 + k * 0.1, s0 + 0.01)).lineTo(...at(0.3 + k * 0.1, s1 - 0.01));
  g.stroke({ width: 0.032, color: 0x2a2418, alpha: 0.3 });
  standingLadder(g, f, light, wall, IRON);
}

/** A sewer's outfall, where a run empties to the river or the sea, seen from above: the tunnel's mouth in the
 * wall under a deep band of arch stones (the voussoirs, seen from above, on the wall line) between two jamb
 * blocks, the mouth dark behind an iron grille (a rail, five round bar-tops), and the water running out through
 * it: a sheet as wide as the mouth at the grille, foaming against the bars, widening and fading into the floor,
 * a few flow lines; a faint wedge of daylight on the floor inside. */
export function outfall(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const pts = (...uv: number[]) => {
    const out: number[] = [];
    for (let i = 0; i < uv.length; i += 2) out.push(...at(uv[i], uv[i + 1]));
    return out;
  };
  const [lx, ly] = light;
  // The daylight through the mouth, a faint wedge widening onto the floor.
  g.poly(pts(0.24, 0.1, 0.76, 0.1, 0.98, 0.96, 0.02, 0.96)).fill({ color: 0xfff0c4, alpha: 0.06 });
  g.poly(pts(0.3, 0.1, 0.7, 0.1, 0.84, 0.7, 0.16, 0.7)).fill({ color: 0xfff0c4, alpha: 0.07 });
  // The dark mouth between the arch and the grille, a dull gleam of the water going in.
  g.poly(pts(0.22, -0.1, 0.78, -0.1, 0.78, 0.17, 0.22, 0.17)).fill(0x12100c);
  g.poly(pts(0.3, -0.06, 0.7, -0.06, 0.72, 0.12, 0.28, 0.12)).fill({ color: 0x6e7c70, alpha: 0.22 });
  // The stream: nested sheets, each shorter and a little narrower, so it fades out widening into the floor.
  const WATER = [0x6e7c70, 0x8e9c8e, 0xc4d0c2];
  const n = 8;
  const jit = Array.from({ length: n + 1 }, () => [(rnd() - 0.5) * 0.05, (rnd() - 0.5) * 0.05]);
  for (const [len, narrow, a] of [[0.92, 1, 0.26], [0.74, 0.94, 0.3], [0.56, 0.88, 0.34], [0.4, 0.84, 0.4], [0.26, 0.82, 0.45]]) {
    const edge = (side: number) => {
      const out: number[] = [];
      for (let k = 0; k <= n; k++) {
        const s = 0.16 + (len * k) / n;
        const hw = (0.27 + (s - 0.16) * 0.26) * narrow + (k ? jit[k][side < 0 ? 0 : 1] : 0);
        out.push(...at(0.5 + side * hw, s));
      }
      return out;
    };
    const l = edge(-1);
    const r = edge(1);
    const sheet: number[] = [...l];
    for (let k = r.length - 2; k >= 0; k -= 2) sheet.push(r[k], r[k + 1]);
    g.poly(sheet).fill({ color: WATER[0], alpha: a });
  }
  // Flow lines, drawn in toward the mouth.
  for (const [u0, u1, s0, bend] of [[0.36, 0.42, 0.78, -0.05], [0.62, 0.57, 0.66, 0.05], [0.5, 0.5, 0.86, 0.04]]) {
    const [a, b, c] = [at(u0, s0), at((u0 + u1) / 2 + bend, (s0 + 0.24) / 2), at(u1, 0.24)];
    g.moveTo(...a).quadraticCurveTo(...b, ...c).stroke({ width: 0.02, color: WATER[2], alpha: 0.75, cap: 'round' });
  }
  // Foam against the bars.
  for (let k = 0; k < 6; k++) {
    const [fx, fy] = at(0.27 + k * 0.092 + (rnd() - 0.5) * 0.03, 0.24 + rnd() * 0.04);
    g.poly(patch(fx, fy, 0.03 + rnd() * 0.02, 0.022 + rnd() * 0.012, rnd, 0.25, 8)).fill({ color: 0xeef2e8, alpha: 0.8 });
  }
  // The grille across the mouth: its shadow away from the light, the rail, five round bar-tops (as `grille`).
  const rail = pts(0.18, 0.15, 0.82, 0.15, 0.82, 0.2, 0.18, 0.2);
  const bars = [0.27, 0.385, 0.5, 0.615, 0.73].map((u) => at(u, 0.175));
  g.poly(rail.map((v, i) => v - (i % 2 ? ly : lx) * 0.06)).fill({ color: 0x000000, alpha: 0.22 });
  for (const [bx, by] of bars) g.circle(bx - lx * 0.05, by - ly * 0.05, 0.045).fill({ color: 0x000000, alpha: 0.2 });
  g.poly(rail).fill(IRON[0]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const [bx, by] of bars) {
    g.circle(bx, by, 0.05).fill(IRON[1]).stroke({ width: LINE, color: INK });
    g.circle(bx + lx * 0.014, by + ly * 0.014, 0.018).fill(IRON[2]);
  }
  // The jambs, and the arch stones over the mouth on the wall line, a short shadow under them.
  const [b0, b1] = [-0.44, -0.04];
  const jambs = [pts(0.04, b0, 0.22, b0, 0.22, 0.22, 0.04, 0.22), pts(0.78, b0, 0.96, b0, 0.96, 0.22, 0.78, 0.22)];
  const band = pts(0.04, b0, 0.96, b0, 0.96, b1, 0.04, b1);
  castShadow(g, band, light, 0.08);
  for (const j of jambs) castShadow(g, j, light, 0.08);
  for (const j of jambs) g.poly(j).fill(STONE[1]).stroke({ width: LINE, color: INK, join: 'round' });
  // (Five stones, their joints fanned a little about the mouth's middle.)
  const m = 5;
  const joint = (j: number, s: number) => (j === 0 ? 0.04 : j === m ? 0.96 : 0.04 + (0.92 * j) / m + (j - m / 2) * 0.035 * (s < 0 ? -1 : 1));
  for (let k = 0; k < m; k++) {
    const c = shade(STONE[k % 2 ? 1 : 2], 0.94 + rnd() * 0.12);
    g.poly(pts(joint(k, -1), b0, joint(k + 1, -1), b0, joint(k + 1, 1), b1, joint(k, 1), b1)).fill(c).stroke({ width: LINE, color: INK, join: 'round' });
  }
  g.poly(band).stroke({ width: OUT, color: INK, join: 'round' });
}

/** A drain pipe out of the wall, seen from above: a short run of pipe lying on the floor (0 iron, bolted
 * flanges; 1 fired clay, its joints thick collars; 2 clay broken off short, a chip beside it), lit along the side
 * toward the light, ending in a dark open mouth; a trickle of muck out of it to a small flat puddle. */
export function pipe(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, wall: [number, number]) {
  const { at } = frame(f, wall);
  const look = v % 3;
  const cu = 0.5;
  const rot = wall[0] !== 0 ? Math.PI / 2 : 0;
  const tones = look === 0 ? IRON : [0x5a4838, 0x76604a, 0x8e7a60];
  const [hw, cw] = [0.13, 0.17];
  const end = look === 2 ? 0.58 : 0.74;
  const [p0, p1] = [at(cu, 0), at(cu + 1, 0)];
  const side = Math.sign((p1[0] - p0[0]) * light[0] + (p1[1] - p0[1]) * light[1]) || 1;
  const box = (u0: number, s0: number, u1: number, s1: number) => [...at(u0, s0), ...at(u1, s0), ...at(u1, s1), ...at(u0, s1)];
  const FILTH = 0x46502e;
  // The trickle and the puddle beyond the mouth, flat on the floor.
  const drift = (rnd() - 0.5) * 0.12;
  const pool: number[] = [];
  for (let i = 0; i < 14; i++) {
    const t = (i / 14) * Math.PI * 2;
    const k = 0.75 + rnd() * 0.45;
    pool.push(...at(cu + drift + Math.cos(t) * 0.17 * k, end + 0.16 + Math.sin(t) * 0.07 * k));
  }
  g.poly(pool).fill({ color: FILTH, alpha: 0.85 });
  g.poly(oval(...at(cu + drift - 0.04, end + 0.15), 0.05, 0.016, rot, 8)).fill({ color: 0x626a46, alpha: 0.9 });
  // The run: the body in three tones, lit along one side, shaded along the other.
  g.poly(box(cu - hw, 0, cu + hw, end)).fill(tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(box(cu + side * hw * 0.25, 0, cu + side * hw * 0.85, end)).fill(tones[2]);
  g.poly(box(cu - side * hw, 0, cu - side * hw * 0.6, end)).fill(tones[0]);
  // Collars (or flanges) at the wall and along the run.
  const collars = look === 2 ? [0.03, end * 0.5] : [0.03, end * 0.5, end - 0.05];
  for (const c of collars) {
    g.poly(box(cu - cw, c - 0.035, cu + cw, c + 0.035)).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(box(cu + side * cw * 0.2, c - 0.03, cu + side * cw * 0.85, c + 0.03)).fill(tones[2]);
    if (look === 0) for (const k of [-1, 1]) g.circle(...at(cu + k * cw * 0.7, c), 0.015).fill(IRON[0]);
  }
  // The open mouth, tipped toward us: the rim, the dark bore inside it.
  const [mx, my] = at(cu, end);
  if (look === 2) {
    // Broken off: a jagged lip, and the piece that fell.
    g.poly([...at(cu - hw, end - 0.02), ...at(cu - hw * 0.3, end + 0.03), ...at(cu + hw * 0.2, end - 0.01), ...at(cu + hw, end + 0.025), ...at(cu + hw, end - 0.05), ...at(cu - hw, end - 0.05)]).fill(tones[1]);
    g.poly(ring(...at(cu + 0.24, end + 0.04), 0.04, rnd, 0.01, 6)).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
  }
  g.poly(oval(mx, my, hw, 0.05, rot, 14)).fill(tones[look === 0 ? 2 : 1]).stroke({ width: LINE, color: INK });
  g.poly(oval(mx, my, hw * 0.66, 0.032, rot, 12)).fill(0x0e0c0a);
  // The muck, out over the lip and across the floor.
  g.moveTo(...at(cu, end + 0.02)).quadraticCurveTo(...at(cu - drift * 0.5, end + 0.08), ...at(cu + drift, end + 0.14)).stroke({ width: 0.024, color: FILTH, cap: 'round' });
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
  castShadow(g, patch(cx, cy, r, r, rnd, 0.1, 16), light, 0.34);
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

const OBSIDIAN = [0x18161c, 0x2c2a32, 0x5a5664, 0x8a84a8];
const BASALT = [0x1c1a1c, 0x2e2b2e, 0x46424a, 0x66606a];

/** One angular shard of dark glass lying on the floor round (x, y) along `a`: a sharp irregular blade split by
 * a ridge into facets, the one turned to the light a flat matte grey-violet, the rest near black. */
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
  castShadow(g, [...T, ...L, ...Bl, ...Br, ...R], light, 0.04);
  g.poly([...T, ...L, ...Bl, ...M]).fill(lit ? OBSIDIAN[2] : OBSIDIAN[0]);
  g.poly([...T, ...M, ...Br, ...R]).fill(lit ? OBSIDIAN[0] : OBSIDIAN[2]);
  g.poly([...M, ...Bl, ...Br]).fill(OBSIDIAN[1]);
  g.moveTo(...T).lineTo(...M).lineTo(...Bl).moveTo(...M).lineTo(...Br).stroke({ width: 0.016, color: INK, alpha: 0.8 });
  g.poly([...T, ...L, ...Bl, ...Br, ...R]).stroke({ width: len > 0.25 ? OUT : LINE, color: INK, join: 'miter' });
}

/** Obsidian shards: a few big angular blades of black volcanic glass and a couple of chips, sharp and matte. */
export function obsidianShards(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0, n = 2 + Math.floor(rnd() * 3); i < n; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.2 + 0.22 * rnd();
    glassChip(g, cx + Math.cos(a) * d, cy + Math.sin(a) * d, rnd() * Math.PI * 2, 0.07 + 0.04 * rnd(), 0.045 + 0.015 * rnd(), light, rnd);
  }
  // Two to four big blades at random places and angles, sizes drawn fresh each time.
  const nb = 2 + Math.floor(rnd() * 3);
  const big: [number, number, number, number, number][] = [];
  for (let i = 0; i < nb; i++) {
    const a = rnd() * Math.PI * 2;
    const d = 0.26 * Math.sqrt(rnd());
    const len = (0.32 - i * 0.04) * (0.7 + 0.5 * rnd());
    big.push([Math.cos(a) * d, Math.sin(a) * d, ph + i * (1.2 + rnd() * 1.4), len, len * (0.38 + 0.12 * rnd())]);
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

/** Scorched remains: where someone burnt — an irregular soot patch on the floor, darkest in the middle and fading
 * out, grey ash in a few drifts, a few char marks (blackened bones and splinters, toned, never inked) and two dim
 * embers. No outline: it is a stain on the floor. */
export function scorchedRemains(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const long = f.h >= f.w;
  const B = 1.4;
  const p = (u: number, k: number): [number, number] => (long ? [cx + k, cy + u] : [cx + u, cy + k]);
  void light;
  // The soot: three lopsided lobed layers, fainter outward.
  const ph = [rnd() * 6.3, rnd() * 6.3];
  const soot = (k: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 26; i++) {
      const a = (i / 26) * Math.PI * 2;
      const r = k * (1 + 0.16 * Math.sin(2 * a + ph[0]) + 0.1 * Math.sin(3 * a + ph[1]) + (rnd() - 0.5) * 0.12);
      pts.push(...p(Math.sin(a) * B * 0.46 * r, Math.cos(a) * 0.4 * r));
    }
    return pts;
  };
  g.poly(soot(1.15)).fill({ color: 0x1a1210, alpha: 0.28 });
  g.poly(soot(0.92)).fill({ color: 0x1a1210, alpha: 0.4 });
  g.poly(soot(0.6)).fill({ color: 0x120c0a, alpha: 0.6 });
  // Ash in a few soft drifts.
  for (let i = 0; i < 3; i++) {
    const [x, y] = p((rnd() - 0.5) * B * 0.5, (rnd() - 0.5) * 0.3);
    g.poly(patch(x, y, 0.1 + 0.05 * rnd(), 0.06 + 0.03 * rnd(), rnd, 0.3, 10)).fill({ color: 0x7a7470, alpha: 0.5 });
  }
  // Char marks: a few blackened bones and splinters, flat dark strokes with a lit ash-grey streak.
  for (const [u0, k0, u1, k1, w] of [
    [-0.34, -0.04, -0.06, 0.02, 0.05],
    [0.04, 0.1, 0.32, 0.16, 0.045],
    [0.1, -0.14, 0.28, -0.2, 0.04],
    [-0.2, 0.2, -0.04, 0.26, 0.035],
  ]) {
    const [a, b] = [p(u0 * B, k0), p(u1 * B, k1)];
    g.moveTo(...a).lineTo(...b).stroke({ width: w, color: 0x2a2220, cap: 'round' });
    g.moveTo(...a).lineTo(...b).stroke({ width: 0.014, color: 0x6a625a, cap: 'round' });
  }
  g.poly(oval(...p(-B * 0.36, 0.02), 0.08, 0.09, 0, 12)).fill(0x2a2220);
  g.poly(oval(...p(-B * 0.37, 0), 0.045, 0.05, 0, 10)).fill(0x5a524a);
  // Two dim embers.
  for (const [u, k] of [
    [0.12, -0.04],
    [-0.1, 0.14],
  ]) {
    const [x, y] = p(u * B, k);
    g.circle(x, y, 0.035).fill({ color: 0xc86028, alpha: 0.25 });
    g.circle(x, y, 0.016).fill(0xd8702a);
  }
}

/** Cooled glassy pool: a flat sheet of black volcanic glass where lava spread and set, lobed like a spill: a
 * lumpy basalt rim merging into the floor, ropy folds frozen near its edge, angular cracks (one still faintly
 * red), one muted sheen, obsidian chips at the rim. */
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
      g.stroke({ width: 0.024, color: 0x8a3418, alpha: 0.6, cap: 'round', join: 'miter' });
    }
    line();
    g.stroke({ width: 0.014, color: c === 0 ? 0x3a0a04 : OBSIDIAN[3], alpha: c === 0 ? 1 : 0.55, cap: 'round', join: 'miter' });
  }
  // One flat sheen toward the light, muted: glass, not a mirror.
  const la = Math.atan2(light[1], light[0]) + Math.PI / 2;
  g.poly(oval(cx + light[0] * rx * 0.42, cy + light[1] * ry * 0.42, rx * 0.26, 0.03, la, 14)).fill({ color: 0x7a7488, alpha: 0.7 });
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
