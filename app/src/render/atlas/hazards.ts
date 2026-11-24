import { Graphics } from 'pixi.js';
import { boughs, lit, whorl } from './plants';
import { INK, blob, shade } from './common';
import { stone } from './stones';
import { OUT } from '../pen';

/** Points of `pts` scaled by `k` about (cx, cy) and moved by (dx, dy). */
function scaled(pts: number[], k: number, cx = 0, cy = 0, dx = 0, dy = 0): number[] {
  return pts.map((p, i) => (i % 2 ? cy + (p - cy) * k + dy : cx + (p - cx) * k + dx));
}

/** A patch that fades into the ground: the shape drawn six times, shrinking, each very faint. */
export function fade(g: Graphics, pts: number[], color: number, alpha: number) {
  for (let i = 0; i < 6; i++) g.poly(scaled(pts, 1 - i * 0.025)).fill({ color, alpha: alpha / 2.5 });
}

/** A crystal shard from (x, y) out along `a`: a long kite split down its spine, the side toward the light
 * paler, with a glint near the tip. */
function shard(g: Graphics, x: number, y: number, a: number, len: number, w: number, tones: readonly number[], line: number) {
  const c = Math.cos(a);
  const s = Math.sin(a);
  const tip = [x + c * len, y + s * len];
  const l = [x + c * len * 0.35 - s * w, y + s * len * 0.35 + c * w];
  const rt = [x + c * len * 0.35 + s * w, y + s * len * 0.35 - c * w];
  const base = [x - c * w * 0.4, y - s * w * 0.4];
  g.poly([...base, ...l, ...tip, ...rt]).fill(tones[0]).stroke({ width: 1.8, color: line, join: 'round' });
  // The half facing the light (NW) is the lit facet.
  const lSide = lit(a + Math.PI / 2) > lit(a - Math.PI / 2) ? l : rt;
  g.poly([...base, ...lSide, ...tip]).fill(tones[2]);
  g.moveTo(base[0], base[1]).lineTo(tip[0], tip[1]).stroke({ width: 1, color: tones[1], alpha: 0.9 });
  g.poly([tip[0] - c * len * 0.25, tip[1] - s * len * 0.25, lSide[0] * 0.3 + tip[0] * 0.7, lSide[1] * 0.3 + tip[1] * 0.7, tip[0], tip[1]]).fill({ color: tones[3], alpha: 0.9 });
}

/** A ring of shards round a short central point, the shaded ones first. */
function crystals(g: Graphics, r: number, rnd: () => number, n: number, tones: readonly number[], line: number) {
  const ph = rnd() * Math.PI * 2;
  const parts: [number, number, number][] = [];
  for (let i = 0; i < n; i++) parts.push([ph + ((i + (rnd() - 0.5) * 0.6) / n) * Math.PI * 2, r * (0.6 + 0.35 * rnd()), r * (0.16 + 0.06 * rnd())]);
  parts.sort((p, q) => lit(p[0]) - lit(q[0]));
  for (const [a, len, w] of parts) shard(g, Math.cos(a) * r * 0.1, Math.sin(a) * r * 0.1, a, len, w, tones, line);
  for (let i = 0; i < 4; i++) {
    const a = ph + (i / 4) * Math.PI * 2 + 0.4;
    shard(g, 0, 0, a, r * 0.32, r * 0.14, tones.map((t) => shade(t, 1.08)), line);
  }
}

/** Snowdrift: a lumpy mound with soft lavender shade on the SE, two lighter tiers toward the light and
 * wind-swept ridge lines. */
export function snowdrift(g: Graphics, r: number, rnd: () => number, v: number) {
  const pts = blob(0, 0, r * 0.95, 14, 0.18, rnd);
  g.poly(pts).fill(0xb9c6dc).stroke({ width: 1.8, color: 0x8a98b4, join: 'round' });
  g.poly(scaled(pts, 0.84, 0, 0, -r * 0.06, -r * 0.06)).fill(0xdfe6ef);
  g.poly(scaled(blob(0, 0, r * 0.95, 11, 0.22, rnd), 0.55, 0, 0, -r * 0.16, -r * 0.16)).fill(0xf2f4f6);
  g.poly(blob(-r * 0.3, -r * 0.3, r * 0.14, 7, 0.3, rnd)).fill({ color: 0xfffaf0, alpha: 0.9 });
  const a = rnd() * Math.PI * 2 + v;
  for (let i = 0; i < 3; i++) {
    const d = r * (0.25 + 0.2 * i);
    const b = a + (rnd() - 0.5) * 0.6;
    g.moveTo(Math.cos(b - 0.7) * d, Math.sin(b - 0.7) * d).quadraticCurveTo(Math.cos(b) * d * 1.15, Math.sin(b) * d * 1.15, Math.cos(b + 0.7) * d, Math.sin(b + 0.7) * d);
  }
  g.stroke({ width: 1.5, color: 0xaab8d0, alpha: 0.9, cap: 'round' });
}

/** One faceted ice pinnacle from above: an irregular outline round a short ridge, each side a flat facet
 * toned by the way it faces the light, ridge lines where the tone turns. */
function iceBlock(g: Graphics, cx: number, cy: number, s: number, rnd: () => number, tones: readonly number[]) {
  const n = 5 + Math.floor(rnd() * 2);
  const ph = rnd() * Math.PI * 2;
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.5) / n) * Math.PI * 2;
    const d = s * (0.72 + 0.35 * rnd());
    pts.push(cx + Math.cos(a) * d, cy + Math.sin(a) * d);
  }
  const u = rnd() * Math.PI;
  const ax = cx + s * 0.1;
  const ay = cy + s * 0.08;
  const ends = [
    [ax - Math.cos(u) * s * 0.2, ay - Math.sin(u) * s * 0.2],
    [ax + Math.cos(u) * s * 0.2, ay + Math.sin(u) * s * 0.2],
  ];
  const near = (i: number) => {
    const [x, y] = [pts[i * 2], pts[i * 2 + 1]];
    return Math.hypot(x - ends[0][0], y - ends[0][1]) < Math.hypot(x - ends[1][0], y - ends[1][1]) ? 0 : 1;
  };
  const toneOf: number[] = [];
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const l = lit(Math.atan2((pts[i * 2 + 1] + pts[j * 2 + 1]) / 2 - ay, (pts[i * 2] + pts[j * 2]) / 2 - ax));
    toneOf.push(l > 0.3 ? 2 : l < -0.3 ? 0 : 1);
  }
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [ei, ej] = [ends[near(i)], ends[near(j)]];
    const f = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1], ...ej];
    if (ei !== ej) f.push(...ei);
    g.poly(f).fill(tones[toneOf[i]]).stroke({ width: 1, color: tones[toneOf[i]], join: 'round' });
  }
  // Ridge lines: the crest, and the edges where one facet's tone turns into the next.
  g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]);
  for (let i = 0; i < n; i++) {
    if (toneOf[i] === toneOf[(i + n - 1) % n]) continue;
    const e = ends[near(i)];
    g.moveTo(pts[i * 2], pts[i * 2 + 1]).lineTo(e[0], e[1]);
  }
  g.stroke({ width: 1.4, color: shade(tones[0], 0.7), cap: 'round' });
  g.poly(pts).stroke({ width: OUT * 0.8, color: INK, join: 'round' });
  return pts;
}

/** Ice spire from straight above: a cluster of faceted ice pinnacles, the tallest in the middle, standing
 * in a pale skirt of snow and meltwater, with a short shadow to the SE. */
export function iceSpire(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [
    [0x7a92aa, 0xa8bccc, 0xd4e0e8],
    [0x7495a6, 0xa2c0cc, 0xd0e2e6],
    [0x8090ac, 0xaebace, 0xd8deea],
    [0x7098a4, 0x9ec2ca, 0xcce4e6],
  ][v];
  const skirt = blob(r * 0.04, r * 0.06, r * 0.92, 12, 0.3, rnd);
  fade(g, skirt, 0xd4e0e6, 0.4);
  g.poly(scaled(skirt, 0.84, r * 0.04, r * 0.06)).fill({ color: 0xd8e4ea, alpha: 0.7 }).stroke({ width: 1.5, color: 0xb6c6d4, join: 'round' });
  const k = [3, 4, 5, 4][v];
  const ph = rnd() * Math.PI * 2;
  const blocks: [number, number, number][] = [];
  for (let i = 1; i < k; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.6) / (k - 1)) * Math.PI * 2;
    const s = r * (0.27 + 0.1 * rnd());
    const d = Math.min(r * (0.4 + 0.12 * rnd()), r * 0.94 - s);
    blocks.push([Math.cos(a) * d, Math.sin(a) * d, s]);
  }
  blocks.sort((p, q) => p[2] - q[2]);
  blocks.push([(rnd() - 0.5) * r * 0.12, (rnd() - 0.5) * r * 0.12, r * 0.5]);
  // The short shadow first, under every block.
  for (const [x, y, s] of blocks) g.poly(blob(x + r * 0.08, y + r * 0.09, s * 0.95, 7, 0.2, rnd)).fill({ color: 0x506680, alpha: 0.28 });
  for (const [x, y, s] of blocks) iceBlock(g, x, y, s, rnd, tones);
}

/** Basalt pillar: a honeycomb of hexagonal column tops at different heights (the higher ones lighter),
 * each bevelled toward the light, cracks across some. */
export function basaltPillar(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [0x2e2a30, 0x4a4450, 0x6a6270, 0x8a7aa8].map((c) => shade(c, 1 + (v - 1.5) * 0.04));
  const s = r * 0.3;
  const cells: [number, number, number][] = [];
  const axial = [[0, 0], [1, 0], [0, 1], [-1, 1], [-1, 0], [0, -1], [1, -1]];
  const skip = Math.floor(rnd() * 6) + 1;
  for (let i = 0; i < axial.length; i++) {
    if (i === skip) continue;
    const [q, rr] = axial[i];
    cells.push([s * Math.sqrt(3) * (q + rr / 2), s * 1.5 * rr, rnd()]);
  }
  cells.sort((p, q) => p[2] - q[2]);
  for (const [x, y, h] of cells) {
    const hex = (k: number, dx = 0) => {
      const pts: number[] = [];
      for (let i = 0; i < 6; i++) pts.push(x + dx + Math.cos(Math.PI / 6 + (i / 6) * Math.PI * 2) * s * k, y + dx + Math.sin(Math.PI / 6 + (i / 6) * Math.PI * 2) * s * k);
      return pts;
    };
    const lift = 0.75 + 0.35 * h;
    g.poly(hex(0.98)).fill(shade(tones[0], lift)).stroke({ width: 2, color: INK, join: 'round' });
    g.poly(hex(0.8, -s * 0.06)).fill(shade(tones[1], lift));
    g.poly(hex(0.5, -s * 0.14)).fill(shade(tones[2], lift));
    if (h > 0.6) {
      g.moveTo(x - s * 0.5, y + s * 0.1).lineTo(x - s * 0.1, y + s * 0.05).lineTo(x + s * 0.3, y + s * 0.4).stroke({ width: 1.2, color: tones[0], cap: 'round' });
    }
  }
}

/** Obsidian outcrop: jagged black glass shards with violet glints, on a little rubble. */
export function obsidian(g: Graphics, r: number, rnd: () => number, v: number) {
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * r * 0.7, Math.sin(a) * r * 0.7, r * 0.1, [0x2a2630, 0x3a3440, 0x4e4658, 0x7a6a98], rnd, 6);
  }
  crystals(g, r * 0.8, rnd, 5 + (v % 2), [0x1a1420, 0x2e2638, 0x4a3e5c, 0xa898d0], INK);
}

/** Bog: dark peaty water fading into the ground through a mossy margin, scum and lily pads on it, a few
 * reed tufts and bubbles. */
export function bog(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 14, 0.15, rnd);
  fade(g, edge, 0x4a5a2a, 0.3);
  const water = scaled(blob(0, 0, r * 0.95, 13, 0.2, rnd), 0.78);
  g.poly(water).fill(0x3a4228).stroke({ width: 1.8, color: 0x2a3018, join: 'round' });
  g.poly(scaled(water, 0.75, 0, 0, r * 0.05, r * 0.05)).fill(0x2e3420);
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.5 * Math.sqrt(rnd());
    g.poly(blob(Math.cos(a) * d, Math.sin(a) * d, r * (0.08 + 0.06 * rnd()), 7, 0.4, rnd)).fill({ color: 0x8a9a4a, alpha: 0.75 });
  }
  // Lily pads: discs with a notch, lit on the NW.
  for (let i = 0; i < 3 + (v % 2); i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.45 * Math.sqrt(rnd());
    const x = Math.cos(a) * d;
    const y = Math.sin(a) * d;
    const s = r * 0.08;
    const n = rnd() * Math.PI * 2;
    g.moveTo(x, y).arc(x, y, s, n + 0.4, n + Math.PI * 2 - 0.4).closePath().fill(0x5e8a3a).stroke({ width: 1, color: 0x2a3a18 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.35).fill({ color: 0x86b052, alpha: 0.8 });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const x = Math.cos(a) * r * 0.25;
    const y = Math.sin(a) * r * 0.25;
    g.circle(x, y, r * 0.03).stroke({ width: 1, color: 0xa8b080, alpha: 0.8 });
  }
  for (let i = 0; i < 3; i++) {
    const a = rnd() * Math.PI * 2;
    const x = Math.cos(a) * r * 0.74;
    const y = Math.sin(a) * r * 0.74;
    const tips = boughs(9, rnd);
    whorl(g, x, y, r * 0.17, tips, 0.35);
    g.fill(0x5e7a3a).stroke({ width: 1.2, color: 0x34461e, join: 'round' });
    whorl(g, x - r * 0.02, y - r * 0.02, r * 0.11, tips, 0.35);
    g.fill(0x7e9a48);
  }
}

/** Quicksand: a sandy patch fading into the ground, darker wet sand in the middle drawn into a swirl,
 * ripples and a few bubbles. */
export function quicksand(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 14, 0.15, rnd);
  fade(g, edge, 0xc8a464, 0.35);
  const wet = scaled(blob(0, 0, r * 0.95, 12, 0.18, rnd), 0.72);
  g.poly(wet).fill(0xa8824c).stroke({ width: 1.5, color: 0x7a5a32, join: 'round' });
  g.poly(scaled(wet, 0.7, 0, 0, r * 0.04, r * 0.04)).fill(0x94703e);
  // The swirl: a spiral drawn in to the middle.
  const turn = v % 2 ? 1 : -1;
  g.moveTo(r * 0.6, 0);
  for (let i = 1; i <= 40; i++) {
    const t = i / 40;
    const a = turn * t * Math.PI * 4;
    const d = r * 0.6 * (1 - t);
    g.lineTo(Math.cos(a) * d, Math.sin(a) * d);
  }
  g.stroke({ width: 1.5, color: 0x6e4e2a, alpha: 0.7, cap: 'round' });
  for (let i = 0; i < 4; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.4 * Math.sqrt(rnd());
    g.circle(Math.cos(a) * d, Math.sin(a) * d, r * 0.025).stroke({ width: 1, color: 0xd8b878, alpha: 0.9 });
  }
  g.poly(scaled(edge, 0.5, 0, 0, -r * 0.35, -r * 0.35)).fill({ color: 0xecd29a, alpha: 0.35 });
}

/** Thin ice: only the fracture, over whatever ice is already there — a punched-through spot, jagged
 * cracks running out from it and tapering, side branches, and broken rings joining the cracks. Each
 * crack is dark water with a bright broken edge on the lit side. */
export function thinIce(g: Graphics, r: number, rnd: () => number, v: number) {
  // Segments [x, y, x2, y2, width].
  const segs: number[][] = [];
  const crack = (x: number, y: number, a: number, len: number, w: number, branches: number) => {
    const steps = 6;
    const pts: number[][] = [[x, y]];
    for (let k = 1; k <= steps; k++) {
      a += (rnd() - 0.5) * 0.7;
      x += Math.cos(a) * (len / steps) * (0.7 + 0.6 * rnd());
      y += Math.sin(a) * (len / steps) * (0.7 + 0.6 * rnd());
      pts.push([x, y]);
    }
    for (let k = 1; k < pts.length; k++) segs.push([...pts[k - 1], ...pts[k], w * (1 - (k - 1) / steps) + 0.6]);
    for (let b = 0; b < branches; b++) {
      const k = 2 + Math.floor(rnd() * 3);
      const [bx, by] = pts[k];
      crack(bx, by, a + (rnd() < 0.5 ? 1 : -1) * (0.5 + 0.4 * rnd()), len * (0.3 + 0.2 * rnd()), w * 0.5, 0);
    }
    return pts;
  };
  const [cx, cy] = [(rnd() - 0.5) * r * 0.3, (rnd() - 0.5) * r * 0.3];
  const n = 6 + (v % 3);
  const ph = rnd() * Math.PI * 2;
  const radials: number[][][] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.5) / n) * Math.PI * 2;
    radials.push(crack(cx, cy, a, r * (0.5 + 0.35 * rnd()), 3.2, 1 + Math.floor(rnd() * 2)));
  }
  // Rings: from a point on one crack to the matching point on the next, not all the way round.
  for (const k of [2, 4]) {
    for (let i = 0; i < n; i++) {
      if (rnd() < 0.35) continue;
      const [x0, y0] = radials[i][k];
      const [x1, y1] = radials[(i + 1) % n][k];
      const mx = (x0 + x1) / 2 + (rnd() - 0.5) * r * 0.06;
      const my = (y0 + y1) / 2 + (rnd() - 0.5) * r * 0.06;
      segs.push([x0, y0, mx, my, 1.4], [mx, my, x1, y1, 1.2]);
    }
  }
  // Keep everything inside the frame.
  const lim = r * 0.95;
  for (const sg of segs) {
    for (const o of [0, 2]) {
      const d = Math.hypot(sg[o], sg[o + 1]);
      if (d > lim) {
        sg[o] *= lim / d;
        sg[o + 1] *= lim / d;
      }
    }
  }
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w + 1.5, color: 0x1e4660, alpha: 0.35, cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) g.moveTo(x, y).lineTo(x2, y2).stroke({ width: w, color: 0x1e4660, alpha: 0.9, cap: 'round' });
  for (const [x, y, x2, y2, w] of segs) {
    g.moveTo(x - w * 0.45, y - w * 0.45).lineTo(x2 - w * 0.45, y2 - w * 0.45).stroke({ width: Math.max(0.7, w * 0.35), color: 0xf2faff, alpha: 0.85, cap: 'round' });
  }
  // The punched-through spot: dark water with a broken white rim.
  g.poly(blob(cx, cy, r * 0.06, 7, 0.5, rnd)).fill(0x163a52).stroke({ width: 1.2, color: 0xf2faff, alpha: 0.85 });
}

/** The part of polygon `pts` with nx*x + ny*y <= c (one Sutherland-Hodgman pass). */
function clip(pts: number[], nx: number, ny: number, c: number): number[] {
  const out: number[] = [];
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) {
    const [x0, y0, x1, y1] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const d0 = nx * x0 + ny * y0 - c;
    const d1 = nx * x1 + ny * y1 - c;
    if (d0 <= 0) out.push(x0, y0);
    if (d0 <= 0 !== d1 <= 0) {
      const t = d0 / (d0 - d1);
      out.push(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
    }
  }
  return out;
}

/** Lava pool: a rim of cracked, cooled basalt round a pool of glowing orange-red melt, dark crust plates
 * floating on it with hot yellow only in the narrow cracks between them, a faint warm glow just past the rim. */
export function lava(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 13, 0.14, rnd);
  fade(g, edge, 0xd8783a, 0.14);
  const rim = scaled(edge, 0.82);
  g.poly(rim).fill(0x2a201c).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(scaled(rim, 0.97, 0, 0, -r * 0.025, -r * 0.025)).fill(0x4a3a32);
  const pool = scaled(blob(0, 0, r * 0.95, 12, 0.2, rnd), 0.62);
  g.poly(pool).fill(0xc8441e).stroke({ width: 1.5, color: 0x1a1210, join: 'round' });
  // Cracks across the rim, from the pool's edge out.
  for (let i = 0; i < 9; i++) {
    const a = (i / 9) * Math.PI * 2 + rnd() * 0.5;
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const b = a + (rnd() - 0.5) * 0.3;
    g.moveTo(c * r * 0.55, s * r * 0.55).lineTo(Math.cos(b) * r * 0.64, Math.sin(b) * r * 0.64).lineTo(Math.cos(a) * r * 0.74, Math.sin(a) * r * 0.74);
  }
  g.stroke({ width: 1.4, color: 0x140e0c, cap: 'round', join: 'round' });
  // Crust plates: the pool split into cells about scattered points, some left open.
  // Scattered (not a hub and ring, which reads as a wheel).
  const sites: [number, number][] = [];
  for (let t = 0; t < 200 && sites.length < 7 + (v % 2); t++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.52 * Math.sqrt(rnd());
    const [x, y] = [Math.cos(a) * d, Math.sin(a) * d];
    if (sites.every(([qx, qy]) => Math.hypot(qx - x, qy - y) > r * 0.24)) sites.push([x, y]);
  }
  const m = sites.length;
  const o = Math.floor(rnd() * m);
  let o2 = 0;
  for (let j = 0; j < m; j++) if (j !== o && (o2 === o || Math.hypot(sites[j][0] - sites[o][0], sites[j][1] - sites[o][1]) < Math.hypot(sites[o2][0] - sites[o][0], sites[o2][1] - sites[o][1]))) o2 = j;
  const open = new Set([o, o2]);
  const cell = (i: number, base: number[], gap: number, openGap: number) => {
    let p = base;
    const [x, y] = sites[i];
    sites.forEach(([qx, qy], j) => {
      if (j === i || p.length < 6) return;
      const l = Math.hypot(qx - x, qy - y) || 1;
      const [nx, ny] = [(qx - x) / l, (qy - y) / l];
      p = clip(p, nx, ny, nx * (x + qx) * 0.5 + ny * (y + qy) * 0.5 - (open.has(j) ? openGap : gap));
    });
    return p;
  };
  const kept = sites.map((_, i) => i).filter((i) => !open.has(i));
  // The hot cracks: yellow round each plate where it meets another plate.
  for (const i of kept) {
    const p = cell(i, pool, 0, r * 0.08);
    if (p.length >= 6) g.poly(p).fill(0xf0b43c);
  }
  for (const i of kept) {
    const p = cell(i, pool, r * 0.03, r * 0.03);
    if (p.length < 6) continue;
    g.poly(p).fill(0x2e221e).stroke({ width: 1.2, color: 0x140e0c, join: 'round' });
    const [x, y] = sites[i];
    g.poly(scaled(p, 0.72, x, y, -r * 0.015, -r * 0.015)).fill(0x44342c);
    if (rnd() < 0.6) {
      const a = rnd() * Math.PI;
      const l = r * 0.08;
      g.moveTo(x - Math.cos(a) * l, y - Math.sin(a) * l)
        .lineTo(x + (rnd() - 0.5) * l * 0.5, y + (rnd() - 0.5) * l * 0.5)
        .lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l)
        .stroke({ width: 1.2, color: 0x140e0c, cap: 'round', join: 'round' });
    }
  }
}

/** Steam vent: a cracked mouth among a few sulphur-stained stones, a plume of steam drifting off it. */
export function steamVent(g: Graphics, r: number, rnd: () => number, v: number) {
  const ph = rnd() * Math.PI * 2;
  const n = 4 + (v % 2);
  for (let i = 0; i < n; i++) {
    const a = ph + ((i + (rnd() - 0.5) * 0.6) / n) * Math.PI * 2;
    const d = r * (0.48 + 0.15 * rnd());
    stone(g, Math.cos(a) * d, Math.sin(a) * d, r * (0.14 + 0.12 * rnd()), [0x6a6450, 0x9a9070, 0xc8b878, 0xe8dc90], rnd, 6);
  }
  g.poly(blob(0, 0, r * 0.3, 8, 0.35, rnd)).fill(0x1e1a18).stroke({ width: 1.5, color: INK });
  g.poly(blob(r * 0.04, r * 0.04, r * 0.16, 7, 0.3, rnd)).fill(0x3a2a20);
  g.poly(blob(0, 0, r * 0.38, 9, 0.3, rnd)).stroke({ width: 2, color: 0xd8c050, alpha: 0.6 });
  // The plume: puffs growing and fading as they drift off to one side.
  const drift = ph + Math.PI * (0.5 + rnd());
  for (let i = 0; i < 6; i++) {
    const t = i / 5;
    const x = Math.cos(drift) * r * 0.6 * t + (rnd() - 0.5) * r * 0.12;
    const y = Math.sin(drift) * r * 0.6 * t + (rnd() - 0.5) * r * 0.12;
    const s = r * (0.16 + 0.16 * t);
    g.circle(x, y, s).fill({ color: 0xe8ecf0, alpha: 0.5 - t * 0.25 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.45).fill({ color: 0xffffff, alpha: 0.5 - t * 0.25 });
  }
}

/** Sinkhole: broken ground falling away into a dark pit — a cracked, crumbling lip fading into the
 * ground, rings of shade going down, roots hanging over the edge. */
export function sinkhole(g: Graphics, r: number, rnd: () => number, v: number) {
  const edge = blob(0, 0, r * 0.95, 13, 0.15, rnd);
  fade(g, edge, 0x6a5a40, 0.3);
  const lip = scaled(edge, 0.82);
  g.poly(lip).fill(0x5a4a36).stroke({ width: 1.8, color: 0x3a2e20, join: 'round' });
  const pit = scaled(blob(0, 0, r * 0.95, 11, 0.2, rnd), 0.62, 0, 0, r * 0.04, r * 0.04);
  g.poly(pit).fill(0x3a2e22);
  g.poly(scaled(pit, 0.7, r * 0.04, r * 0.04, r * 0.03, r * 0.03)).fill(0x241c16);
  g.poly(scaled(pit, 0.4, r * 0.04, r * 0.04, r * 0.05, r * 0.05)).fill(0x120e0c);
  // Roots hanging into the pit from the lit rim.
  for (let i = 0; i < 4; i++) {
    const a = -Math.PI * 0.75 + (rnd() - 0.5) * 2.4;
    const d0 = r * 0.6;
    const d1 = r * 0.3;
    g.moveTo(Math.cos(a) * d0, Math.sin(a) * d0)
      .quadraticCurveTo(Math.cos(a + 0.3) * r * 0.45, Math.sin(a + 0.3) * r * 0.45, Math.cos(a + 0.1) * d1, Math.sin(a + 0.1) * d1)
      .stroke({ width: 1.8, color: 0x7a5a3a, cap: 'round' });
  }
  // Cracks in the ground round the lip, and a crumbled clod or two.
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    g.moveTo(Math.cos(a) * r * 0.62, Math.sin(a) * r * 0.62).lineTo(Math.cos(a + 0.1) * r * 0.85, Math.sin(a + 0.1) * r * 0.85);
  }
  g.stroke({ width: 1.3, color: 0x2a2018, alpha: 0.8, cap: 'round' });
  for (let i = 0; i < 2 + (v % 2); i++) {
    const a = rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * r * 0.5, Math.sin(a) * r * 0.5, r * 0.08, [0x4a3a2a, 0x6a5440, 0x8a7458, 0xa89070], rnd, 6);
  }
}
