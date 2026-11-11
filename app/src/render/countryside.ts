/**
 * v63 countryside: the battlemap's field and croft bounds and a windmill's sails (`town::rural`), as
 * vectors over the ground, in the cartoon style of the sprites: a hedgerow a band of round leafy
 * clumps (the leafy trees' canopy, smaller) with a bush or a tree standing out of it now and then and
 * a soft shadow to the south-east; a dry-stone wall rough stones in two rows either side of a coping
 * line; a post-and-rail fence two stout rails side by side under square posts every 6 ft; sails four
 * lattice arms, cloth on two or three, a hub cap and a faint shadow.
 *
 * A run comes in straight pieces that meet end to end at its corners (mitred there, as
 * `walls.ts` mitres a garden wall) and are cut at chunk edges. Everything along a piece (clumps,
 * bushes, stones, posts) is laid on a lattice of world distance along its line and hashed by the line, so a
 * piece cut at a chunk edge carries on in the next chunk as if it were one.
 */
import type { Graphics } from 'pixi.js';
import { SQ, type Shape } from '../gen/battlePrep';
import { PX } from './atlas';
import { FT, INK, LX, LY, mix, PLUM, STONE, WOOD } from './bridges';

/** `ShapeKind` ids (`battlemap.rs`, v63 countryside). */
export const HEDGE = 14;
export const DRY_WALL = 15;
export const FENCE = 16;
export const SAILS = 17;

/** Whether a shape is one of these bounds (drawn with the walls, under the roofs). */
export const isBound = (kind: number) => kind === HEDGE || kind === DRY_WALL || kind === FENCE;

type Pt = [number, number];

/** Integers to [0, 1), the same in every chunk. */
function hash(a: number, b: number, c: number): number {
  let h = Math.imul(a | 0, 0x27d4eb2d) ^ Math.imul(b | 0, 0x165667b1) ^ Math.imul(c | 0, 0x2545f491);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39);
  return ((h ^ (h >>> 15)) >>> 0) / 4294967296;
}

/** One end of a piece: its corners on the piece's left (+n) and right, and how it ends: mitred into the
 * next piece, cut at the chunk's edge (the run goes on next door), or free. */
interface End {
  left: Pt;
  right: Pt;
  p: Pt;
  mitred: boolean;
  cut: boolean;
}

/** A straight piece, turned to point east-ish whichever way it came, so its lattice and sides are
 * the same in every chunk; `lit` how much its left side faces the light. Its lattices are laid by
 * world position on its main axis (x if it runs more east-west than north-south, else y): `c0` the
 * world coordinate of its start on that axis, `cu` its direction's part on it, `cq` that part
 * rounded (the same in every chunk; a direction worked out from a chunk's piece is only nearly
 * the same, and far from the origin a lattice of world distance along the line would drift by
 * many px); `o0`, `ou` the same on the other axis. */
interface Run {
  x0: number;
  y0: number;
  l: number;
  ux: number;
  uy: number;
  nx: number;
  ny: number;
  h: number;
  axis: number;
  c0: number;
  cu: number;
  cq: number;
  o0: number;
  ou: number;
  lit: number;
  ends: [End, End];
  /** A field's hedge, round it or a row in it (a third point (1, 0) on its shape): long, so drawn
   * the cheaper way (fewer, bigger clumps, a coarser outline, fewer bushes and trees). */
  row: boolean;
}

/** How far along a piece node `k` of a lattice every `step` lies (nominally `step` apart along it). */
const nodeA = (r: Run, k: number, step: number) => (k * step * r.cq - r.c0) / r.cu;
/** Where on a lattice every `step` a point `a` along a piece lies (a fractional node). */
const lat = (r: Run, a: number, step: number) => (r.c0 + a * r.cu) / (step * r.cq);
/** The nodes of a lattice every `step` from `lo` to `hi` along a piece, in order along it. */
function nodes(r: Run, step: number, lo: number, hi: number): number[] {
  const [p, q] = [lat(r, lo, step), lat(r, hi, step)];
  const out: number[] = [];
  for (let k = Math.ceil(Math.min(p, q)); k <= Math.floor(Math.max(p, q)); k++) out.push(k);
  return r.cu > 0 ? out : out.reverse();
}
/** A hash of node `k` of a lattice every `step` on a piece, by where it lies in the world. */
const hk = (r: Run, k: number, step: number, salt: number) => hash(k, Math.round((r.o0 + nodeA(r, k, step) * r.ou) / 8) * 2 + r.axis, salt);

function runs(shapes: Shape[], ox: number, oy: number, side: number): Run[] {
  const segs = shapes.map((s) => {
    let [x0, y0, x1, y1] = s.pts;
    if (x1 < x0 - 1e-3 || (Math.abs(x1 - x0) <= 1e-3 && y1 < y0)) [x0, y0, x1, y1] = [x1, y1, x0, y0];
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const axis = Math.abs(ux) >= Math.abs(uy) ? 0 : 1;
    const [c0, cu, o0, ou] = axis ? [y0 + oy, uy, x0 + ox, ux] : [x0 + ox, ux, y0 + oy, uy];
    const row = s.pts.length > 4 && Math.round(s.pts[4] / PX) === 1;
    return { x0, y0, x1, y1, l, ux, uy, h: s.size / 2, axis, c0, cu, cq: Math.round(Math.abs(cu) * 64) / 64, o0, ou, row };
  });
  const near = (x: number, y: number, px: number, py: number) => Math.abs(x - px) < 1.5 && Math.abs(y - py) < 1.5;
  const onEdge = (x: number, y: number) => Math.min(Math.abs(x), Math.abs(x - side), Math.abs(y), Math.abs(y - side)) < 0.75;
  return segs.map((s) => {
    const [nx, ny] = [-s.uy, s.ux];
    const ends = ([0, 1] as const).map((end): End => {
      const p: Pt = end ? [s.x1, s.y1] : [s.x0, s.y0];
      const e: End = { left: [p[0] + nx * s.h, p[1] + ny * s.h], right: [p[0] - nx * s.h, p[1] - ny * s.h], p, mitred: false, cut: false };
      const o = segs.find((t) => t !== s && (near(t.x0, t.y0, ...p) || near(t.x1, t.y1, ...p)));
      if (o) {
        // Heading into the corner along this one, out of it along the other.
        const din = end ? [s.ux, s.uy] : [-s.ux, -s.uy];
        const dout = near(o.x0, o.y0, ...p) ? [o.ux, o.uy] : [-o.ux, -o.uy];
        const [ax, ay, bx, by] = [-din[1], din[0], -dout[1], dout[0]];
        const ml = Math.hypot(ax + bx, ay + by);
        if (ml > 0.2) {
          const [mx, my] = [(ax + bx) / ml, (ay + by) / ml];
          const k = Math.min(s.h * 3, s.h / (mx * ax + my * ay));
          const q: Pt = [p[0] + mx * k, p[1] + my * k];
          const q2: Pt = [p[0] - mx * k, p[1] - my * k];
          [e.left, e.right] = end ? [q, q2] : [q2, q];
          e.mitred = true;
        }
      } else e.cut = onEdge(...p);
      return e;
    }) as [End, End];
    const { x0, y0, l, ux, uy, h, axis, c0, cu, cq, o0, ou, row } = s;
    return { x0, y0, l, ux, uy, nx, ny, h, axis, c0, cu, cq, o0, ou, lit: nx * LX + ny * LY, ends, row };
  });
}

/** A point `a` along a piece and `d` across it (toward its left). */
const at = (r: Run, a: number, d: number): Pt => [r.x0 + r.ux * a + r.nx * d, r.y0 + r.uy * a + r.ny * d];
/** How far along a piece a point lies. */
const along = (r: Run, p: Pt) => (p[0] - r.x0) * r.ux + (p[1] - r.y0) * r.uy;

/**
 * One side (`sd` 1 its left, -1 its right) of a piece from its start to its end, as flat points:
 * across `s` of its half-width, swollen by `bump(k)` (a fraction) at lattice nodes `k` every `step`
 * px of world distance, the swelling fading out into a mitred corner; moved by `shift` (a cut end
 * only across, so the next chunk's piece meets it).
 */
function edge(r: Run, sd: 1 | -1, step: number, bump: (k: number) => number, s: number, shift: Pt = [0, 0]): number[] {
  const [A, B] = r.ends;
  const corner = (E: End) => (sd > 0 ? E.left : E.right);
  const a0 = A.mitred ? along(r, corner(A)) : 0;
  const a1 = B.mitred ? along(r, corner(B)) : r.l;
  const fade = (a: number) => Math.max(0, Math.min(1, A.mitred ? a / (r.h * 1.5) : 1, B.mitred ? (r.l - a) / (r.h * 1.5) : 1));
  const across = (a: number) => {
    const w = lat(r, a, step);
    const k = Math.floor(w);
    const b = bump(k) + (bump(k + 1) - bump(k)) * (w - k);
    return sd * r.h * s * (1 + b * fade(a));
  };
  const along0 = shift[0] * r.ux + shift[1] * r.uy;
  const out: number[] = [];
  const end = (E: End, a: number) => {
    if (E.mitred) {
      const q = corner(E);
      out.push(E.p[0] + (q[0] - E.p[0]) * s + shift[0], E.p[1] + (q[1] - E.p[1]) * s + shift[1]);
    } else {
      const p = at(r, a, across(a));
      const [sx, sy] = E.cut ? [shift[0] - r.ux * along0, shift[1] - r.uy * along0] : shift;
      out.push(p[0] + sx, p[1] + sy);
    }
  };
  end(A, 0);
  for (const k of nodes(r, step, Math.max(0, a0) + 0.5, Math.min(a1, r.l) - 0.5)) {
    const a = nodeA(r, k, step);
    const p = at(r, a, across(a));
    out.push(p[0] + shift[0], p[1] + shift[1]);
  }
  end(B, r.l);
  return out;
}

/** Reverse a flat point list. */
const rev = (pts: number[]) => {
  const out: number[] = [];
  for (let i = pts.length - 2; i >= 0; i -= 2) out.push(pts[i], pts[i + 1]);
  return out;
};

/** A piece's outline as a closed ring: its left side, its right side back. */
const ring = (L: number[], R: number[]) => [...L, ...rev(R)];

/** Ink a piece's outline: its sides, and across its free ends only (none over a mitre or a chunk's edge). */
function inkOutline(g: Graphics, r: Run, L: number[], R: number[], width: number) {
  const [A, B] = r.ends;
  const openA = A.mitred || A.cut;
  const openB = B.mitred || B.cut;
  if (openA && openB) {
    g.poly(L, false);
    g.poly(R, false);
  } else if (openA) g.poly([...L, ...rev(R)], false);
  else if (openB) g.poly([...rev(R), ...L], false);
  else g.poly([...L, ...rev(R)], true);
  g.stroke({ width, color: INK, join: 'round', cap: 'round' });
}

/** Hedgerow leaves (dark, mid, light): the leafy trees' greens, darker than any grass. */
const LEAF = [0x2f4a22, 0x4a6b2b, 0x77983b];
/** A hedgerow's bigger bushes and its trees: dark, mid, light. */
const BUSH = [0x34502a, 0x557a32, 0x88a846];
const TREES = [
  [0x2f4a22, 0x4d6e2d, 0x7a9a3c],
  [0x3a4a20, 0x62702c, 0x9aa24a],
];
/** Ink round a hedgerow (as wide outside it as an outline over half a square across). */
const HEDGE_INK = 2.7;

/** A hedgerow's lattices: its clumps, its bushes and trees (one in most cells). */
const CLUMP = FT * 1.7;
/** A field's row's clumps apart (fewer, bigger: `Run::row`). */
const ROW_CLUMP = FT * 2.6;
const CELL = FT * 60;

/** A round clump of leaves on a piece: `a` along, `d` across (toward its left), `r` its radius (px). */
interface Clump {
  a: number;
  d: number;
  r: number;
  /** Its lattice node (an end's clump: none). */
  k?: number;
}

/** How far along a piece things may reach: to its cut ends (the next chunk draws the rest), freely past
 * its other ends. */
const reach = (r: Run): [number, number] => [r.ends[0].cut ? 0 : -Infinity, r.ends[1].cut ? r.l : Infinity];

/**
 * A disc (`n` points round) as a flat point list, squashed onto the piece's cut ends where it crosses
 * them (a circle cut by a line: the next chunk's piece draws the other side of it).
 */
function disc(r: Run, x: number, y: number, rad: number, n: number): number[] {
  const [lo, hi] = reach(r);
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const t = (i / n) * Math.PI * 2;
    let [px, py] = [x + Math.cos(t) * rad, y + Math.sin(t) * rad];
    const a = along(r, [px, py]);
    const fix = a < lo ? lo - a : a > hi ? hi - a : 0;
    px += r.ux * fix;
    py += r.uy * fix;
    out.push(px, py);
  }
  return out;
}

/**
 * A piece's clumps: two staggered rows on a lattice of world distance along its line (so a piece cut at
 * a chunk's edge carries on next door), each clump its own size, the odd one swollen; the line wavers
 * a foot or so either way (straightening into a corner). A free or mitred end gets a round clump
 * of its own: a clipped end, or the turn of a corner. `hw`: the half-width.
 */
function clumps(r: Run, hw: number): Clump[] {
  const [A, B] = r.ends;
  const wave = FT * 14;
  const swing = (a: number) => {
    const w = lat(r, a, wave);
    const k = Math.floor(w);
    const t = w - k;
    const v = (j: number) => hk(r, j, wave, 51) * 2 - 1;
    const ease = Math.min(1, A.mitred ? a / (FT * 8) : 1, B.mitred ? (r.l - a) / (FT * 8) : 1);
    return (v(k) + (v(k + 1) - v(k)) * t * t * (3 - 2 * t)) * FT * 0.9 * Math.max(0, ease);
  };
  const out: Clump[] = [];
  // (A row: fewer, bigger clumps.)
  const step = r.row ? ROW_CLUMP : CLUMP;
  const grow = r.row ? 1.22 : 1;
  const [lo, hi] = [A.cut ? -hw * 1.5 : hw * 0.55, B.cut ? r.l + hw * 1.5 : r.l - hw * 0.55];
  for (const k of nodes(r, step, lo - step, hi + step)) {
    const a = nodeA(r, k, step) + (hk(r, k, step, 52) - 0.5) * step * 0.7;
    if (a < lo || a > hi) continue;
    const side = k & 1 ? 1 : -1;
    const big = hk(r, k, step, 55) > 0.88 ? 1.15 : 1;
    out.push({ a, d: swing(a) + side * hw * (0.16 + 0.24 * hk(r, k, step, 53)) / grow, r: hw * (0.62 + 0.2 * hk(r, k, step, 54)) * big * grow, k });
  }
  for (const [E, a, corner] of [
    [A, hw * 0.4, 0],
    [B, r.l - hw * 0.4, r.l],
  ] as const) {
    if (E.cut) continue;
    if (E.mitred) out.push({ a: corner, d: 0, r: hw * 0.98 });
    else out.push({ a, d: swing(a), r: hw * 0.8 });
  }
  return out;
}

/**
 * A piece's outline round the union of its clumps, as its left and right sides (flat points, from its
 * start to its end): sampled on the world lattice between, more closely round a free end's curve,
 * straight across a cut end.
 */
function envelope(r: Run, cl: Clump[]): [number[], number[]] {
  const [A, B] = r.ends;
  const first = cl.reduce((m, c) => (c.a - c.r < m.a - m.r ? c : m), cl[0]);
  const last = cl.reduce((m, c) => (c.a + c.r > m.a + m.r ? c : m), cl[0]);
  const a0 = A.cut ? 0 : first.a - first.r;
  const a1 = B.cut ? r.l : last.a + last.r;
  const at0: number[] = [a0];
  if (!A.cut) for (const t of [0.06, 0.22, 0.45]) at0.push(a0 + first.r * t);
  const step = r.row ? FT * 0.95 : FT * 0.42;
  for (const k of nodes(r, step, a0, a1)) at0.push(nodeA(r, k, step));
  if (!B.cut) for (const t of [0.45, 0.22, 0.06]) at0.push(a1 - last.r * t);
  at0.push(a1);
  at0.sort((p, q) => p - q);
  const n = at0.length - 1;
  const samples = at0.filter((a, i) => i === 0 || i === n || (a > at0[i - 1] + 0.5 && a < at0[i + 1] - 0.5));
  const L: number[] = [];
  const R: number[] = [];
  for (const a of samples) {
    let [hiD, loD] = [-Infinity, Infinity];
    for (const c of cl) {
      const o = a - c.a;
      if (Math.abs(o) > c.r) continue;
      const s = Math.sqrt(c.r * c.r - o * o);
      hiD = Math.max(hiD, c.d + s);
      loD = Math.min(loD, c.d - s);
    }
    if (hiD < loD) continue;
    L.push(...at(r, a, hiD));
    R.push(...at(r, a, loD));
  }
  return [L, R];
}

/** A leafy round (`n` points) in the canopy manner: ink, dark, a mid round toward the light, a light
 * spot; `rs` the rounds (x, y, radius), `tones` dark, mid, light; the ink drawn `ink` wider. */
function leafy(g: Graphics, r: Run, rs: [number, number, number][], tones: number[], ink: number) {
  if (!rs.length) return;
  for (const [x, y, rad] of rs) g.poly(disc(r, x, y, rad + ink, 16));
  g.fill(INK);
  for (const [x, y, rad] of rs) g.poly(disc(r, x, y, rad, 16));
  g.fill(tones[0]);
  for (const [x, y, rad] of rs) g.poly(disc(r, x + LX * rad * 0.18, y + LY * rad * 0.18, rad * 0.78, 14));
  g.fill(tones[1]);
  for (const [x, y, rad] of rs) g.poly(disc(r, x + LX * rad * 0.34, y + LY * rad * 0.34, rad * 0.42, 10));
  g.fill({ color: tones[2], alpha: 0.9 });
}

/**
 * Hedgerows (drawn about 4.5 ft thick, 5 ft tall): a band of overlapping round clumps of leaves in the leafy
 * trees' manner, the same greens everywhere, darker than the grass: a soft shadow to the south-east,
 * a bold ink outline round the whole band (corners and crossings join into one), each clump dark
 * with a mid round toward the light and a light spot, a few leaves picked out; a round clipped end.
 * Every 40-80 ft or so (by world distance along the line) a bigger bush or a hedgerow tree stands
 * out of it.
 */
function drawHedges(g: Graphics, rs: Run[]) {
  const hw = FT * 2.35;
  const cls = rs.map((r) => clumps(r, hw));
  const sides = rs.map((r, i) => (cls[i].length ? envelope(r, cls[i]) : [[], []]));
  const rings = sides.map(([L, R]) => ring(L, R));
  // Bushes and trees: a lattice of 60 ft cells along the line, most with one somewhere in its middle.
  const extras = rs.map((r) => {
    const out: { a: number; d: number; tree: boolean; rad: number; seed: number }[] = [];
    const cell = CELL;
    const [lo, hi] = [r.ends[0].cut ? -FT * 14 : FT * 5, r.ends[1].cut ? r.l + FT * 14 : r.l - FT * 5];
    for (const m of nodes(r, cell, lo - cell, hi + cell)) {
      if (hk(r, m, cell, 60) > (r.row ? 0.5 : 0.85)) continue;
      const a = nodeA(r, m, cell) + (0.33 + 0.34 * hk(r, m, cell, 61)) * cell;
      if (a < lo || a > hi) continue;
      const tree = hk(r, m, cell, 62) < 0.45;
      const h = hk(r, m, cell, 63);
      out.push({ a, d: (hk(r, m, cell, 64) - 0.5) * hw * (tree ? 0.6 : 1.1), tree, rad: tree ? FT * (5 + 2.5 * h) : FT * (2.6 + 0.8 * h), seed: m });
    }
    return out;
  });
  // A soft shadow to the south-east, the bushes' and trees' with it.
  const off = FT * 2.1;
  for (const [f, alpha] of [
    [1, 0.11],
    [0.55, 0.15],
  ]) {
    rings.forEach((p, i) => p.length && !(rs[i].row && f < 1) && g.poly(p.map((v) => v + off * f)));
    rs.forEach((r, i) => {
      for (const e of extras[i]) {
        const [x, y] = at(r, e.a, e.d);
        const o = e.tree ? off * 2 * f : off * 1.1 * f;
        g.poly(disc(r, x + o, y + o, e.rad * (e.tree ? 0.9 : 1), 18));
      }
    });
    g.fill({ color: 0x000000, alpha });
  }
  // The ink round the band first, then the leaves over its inner half: an outline round the whole.
  // (A field's hedges' joins bevelled: as good at this weight, and far fewer points.)
  for (const row of [false, true]) {
    const opened: number[][] = [];
    const closed: number[][] = [];
    rs.forEach((r, i) => {
      const [L, R] = sides[i];
      if (!L.length || r.row !== row) return;
      const [A, B] = r.ends;
      if (A.cut && B.cut) opened.push(L, R);
      else if (A.cut) opened.push([...L, ...rev(R)]);
      else if (B.cut) opened.push([...rev(R), ...L]);
      else closed.push(rings[i]);
    });
    const join = row ? 'bevel' : 'round';
    for (const p of opened) g.poly(p, false);
    // (Butt ends across a chunk's edge: the band carries on next door.)
    if (opened.length) g.stroke({ width: HEDGE_INK * 2, color: INK, join, cap: 'butt' });
    for (const p of closed) g.poly(p, true);
    if (closed.length) g.stroke({ width: HEDGE_INK * 2, color: INK, join });
  }
  rings.forEach((p) => p.length && g.poly(p));
  g.fill(LEAF[0]);
  // The mid leaves: the band again from its clumps a little smaller and toward the light, one mass
  // with the dark left as a scalloped crescent on the shaded side; then each clump's light spot,
  // bigger on the band's lit side.
  rs.forEach((r, i) => {
    if (!cls[i].length) return;
    const [ta, tn] = [LX * r.ux + LY * r.uy, LX * r.nx + LY * r.ny];
    const [L, R] = envelope(r, cls[i].map((c) => ({ a: c.a + ta * c.r * 0.16, d: c.d + tn * c.r * 0.16, r: c.r * 0.8 })));
    if (L.length) g.poly([...L, ...rev(R)]);
  });
  g.fill(LEAF[1]);
  // (Not on every clump, nor all one size: in a row they would read as beads on a string.)
  rs.forEach((r, i) => {
    for (const c of cls[i]) {
      const h = c.k === undefined ? 0.5 : hk(r, c.k, CLUMP, 56);
      if (h < 0.3) continue;
      const sunny = Math.max(0.6, Math.min(1, 0.8 + (c.d * r.lit) / (r.h * 2))) * (0.7 + 0.45 * h);
      const [x, y] = at(r, c.a, c.d);
      g.poly(disc(r, x + LX * c.r * 0.34, y + LY * c.r * 0.34, c.r * 0.34 * sunny, r.row ? 8 : 10));
    }
  });
  g.fill({ color: LEAF[2], alpha: 0.7 });
  // Leaves picked out, a few to a clump: light toward the light, dark away from it.
  for (const light of [true, false]) {
    rs.forEach((r, i) => {
      for (const c of cls[i]) {
        if (c.k === undefined || (r.row && (c.k & 1) === 0)) continue;
        const [x, y] = at(r, c.a, c.d);
        for (let j = 0; j < (light ? 2 : 1); j++) {
          const t = hk(r, c.k, CLUMP, (light ? 70 : 74) + j) * Math.PI * 2;
          const q = c.r * 0.5 * hk(r, c.k, CLUMP, (light ? 72 : 76) + j);
          const o = c.r * (light ? -0.14 : 0.3);
          g.poly(disc(r, x + Math.cos(t) * q + o, y + Math.sin(t) * q + o, FT * (light ? 0.13 : 0.16), 6));
        }
      }
    });
    g.fill(light ? { color: 0xa6c062, alpha: 0.7 } : { color: LEAF[0], alpha: 0.8 });
  }
  // Bushes and trees: lobes round a middle one, in the leafy trees' manner, each tree's shade on the
  // hedge under it.
  rs.forEach((r, i) => {
    for (const e of extras[i]) {
      const [x, y] = at(r, e.a, e.d);
      if (e.tree) {
        g.poly(disc(r, x + e.rad * 0.3, y + e.rad * 0.3, e.rad * 0.95, 18));
        g.fill({ color: 0x000000, alpha: 0.16 });
      }
      const n = e.tree ? 7 : 4;
      const lobes: [number, number, number][] = [];
      for (let j = 0; j < n; j++) {
        const t = (j / n) * Math.PI * 2 + hk(r, e.seed, CELL, 80 + j) * 0.6;
        const dd = e.rad * (0.35 + 0.2 * hk(r, e.seed, CELL, 90 + j));
        lobes.push([x + Math.cos(t) * dd, y + Math.sin(t) * dd, e.rad * (0.42 + 0.14 * hk(r, e.seed, CELL, 100 + j))]);
      }
      lobes.push([x, y, e.rad * 0.55]);
      leafy(g, r, lobes, e.tree ? TREES[e.seed & 1] : BUSH, HEDGE_INK);
    }
  });
}

/** A stone's tone on a face lit `l` (-1 away from the light .. 1 toward it), one of three close tones. */
const stoneTone = (l: number, t: number) => {
  const base = l >= 0 ? mix(STONE[1], STONE[2], 0.3 + 0.55 * l) : mix(STONE[1], mix(STONE[0], PLUM, 0.2), -l * 0.55);
  return t === 0 ? mix(base, STONE[3], 0.22) : t === 1 ? base : mix(base, STONE[0], 0.22);
};

/**
 * Dry-stone walls (about 2.5 ft thick, 4 ft tall): a short soft shadow, rough stones laid in a row on
 * either face (each face lit as it turns to the light, every stone one of three close tones, dark
 * joints between) either side of a pale coping line of stones set on edge across the top; a rough
 * ink outline.
 */
function drawDryWalls(g: Graphics, rs: Run[]) {
  const step = FT * 0.75;
  const rough = (r: Run, sd: number) => (k: number) => (hk(r, k, step, sd + 11) - 0.5) * 0.18;
  const sides = rs.map((r) => [edge(r, 1, step, rough(r, 1), 1), edge(r, -1, step, rough(r, -1), 1)]);
  const reach = FT * 1.5;
  for (const [f, alpha] of [
    [1, 0.11],
    [0.55, 0.15],
  ]) {
    rs.forEach((_, i) => g.poly(ring(sides[i][0], sides[i][1]).map((v) => v + reach * f)));
    g.fill({ color: 0x000000, alpha });
  }
  rs.forEach((_, i) => g.poly(ring(sides[i][0], sides[i][1])));
  g.fill(mix(STONE[0], PLUM, 0.18));
  for (const r of rs) {
    // How far along a point `d` across may reach before the mitre (or the piece's end).
    const lim = (E: End, d: number, a: number) => {
      if (!E.mitred) return a;
      const q = d >= 0 ? E.left : E.right;
      return a + ((along(r, q) - a) * Math.abs(d)) / r.h;
    };
    const clampA = (a: number, d: number, inset: number) => {
      const lo = lim(r.ends[0], d, 0) + (r.ends[0].cut || r.ends[0].mitred ? 0 : inset);
      const hi = lim(r.ends[1], d, r.l) - (r.ends[1].cut || r.ends[1].mitred ? 0 : inset);
      return Math.max(lo, Math.min(hi, a));
    };
    // A stone's four corners, each nudged a little (`j`) unless it sits on a chunk's edge.
    const stone = (a0: number, a1: number, d0: number, d1: number, j: (n: number) => number): number[] => {
      const out: number[] = [];
      ([[a0, d0], [a1, d0], [a1, d1], [a0, d1]] as const).forEach(([a, d], i) => {
        const c = clampA(a, d, 1);
        const onCut = (r.ends[0].cut && c <= 0) || (r.ends[1].cut && c >= r.l);
        const p = at(r, c, d);
        out.push(p[0] + (onCut ? 0 : j(i * 2)), p[1] + (onCut ? 0 : j(i * 2 + 1)));
      });
      return out;
    };
    // The stones on each face, joints staggered between the faces, in three tones per face.
    const len = FT * 1.35;
    for (const sd of [1, -1] as const) {
      const l = sd * r.lit;
      const buckets: number[][][] = [[], [], []];
      // (The joints on a lattice of world position, the next node along the piece `dir` on.)
      const dir = r.cu > 0 ? 1 : -1;
      const cut = (k: number) => nodeA(r, k, len) + (sd > 0 ? 0 : len * 0.5) + (hk(r, k, len, sd + 20) - 0.5) * len * 0.5;
      for (const k of nodes(r, len, -len * 2 - r.h, r.l + r.h)) {
        const [a0, a1] = [cut(k) + 1, cut(k + dir) - 1];
        if (a1 < -r.h || a1 - a0 < 2) continue;
        const outer = r.h * (0.86 + 0.1 * hk(r, k, len, sd + 24));
        const j = (n: number) => (hk(r, k, len, sd * 7 + n) - 0.5) * 2.4;
        const pts = stone(a0, a1, sd * (r.h * 0.36 + 1), sd * outer, j);
        buckets[Math.floor(hk(r, k, len, sd + 30) * 3)].push(pts);
      }
      buckets.forEach((b, t) => {
        if (!b.length) return;
        for (const pts of b) g.poly(pts);
        g.fill(stoneTone(l, t));
      });
    }
    // The coping: a pale line of stones on edge across the top, their joints close together.
    const c = r.h * 0.3;
    g.poly([...at(r, clampA(0, c, 0), c), ...at(r, clampA(r.l, c, 0), c), ...at(r, clampA(r.l, -c, 0), -c), ...at(r, clampA(0, -c, 0), -c)]);
    g.fill(mix(STONE[2], STONE[3], 0.35));
    const jt = FT * 0.8;
    for (const k of nodes(r, jt, 0, r.l)) {
      const a = nodeA(r, k, jt) + (hk(r, k, jt, 40) - 0.5) * jt * 0.5;
      if (a < 1.5 || a > r.l - 1.5) continue;
      const tilt = (hk(r, k, jt, 41) - 0.5) * FT * 0.3;
      g.moveTo(...at(r, a - tilt, c)).lineTo(...at(r, a + tilt, -c));
    }
    g.stroke({ width: 1.3, color: STONE[0], alpha: 0.85 });
    // The coping's arris toward the light.
    const sun = r.lit >= 0 ? 1 : -1;
    g.moveTo(...at(r, clampA(0, sun * c, 0), sun * c * 0.7)).lineTo(...at(r, clampA(r.l, sun * c, 0), sun * c * 0.7));
    g.stroke({ width: 1.4, color: STONE[3], alpha: 0.5 * Math.max(0.3, Math.abs(r.lit)) });
  }
  rs.forEach((r, i) => inkOutline(g, r, sides[i][0], sides[i][1], 2.6));
}

/** Fence timber: the rails' and posts' wood, a lit face. */
const RAIL = mix(WOOD[0], WOOD[1], 0.45);
const RAIL_LIT = mix(WOOD[1], WOOD[2], 0.6);

/**
 * Post-and-rail fences (about 1.75 ft across), seen from above: a pair of stout rails side by side in
 * dark wood, each in its own ink, a lit edge on the side toward the light; square posts every 6 ft
 * over them, inked, lit toward the north-west; the fence's and posts' shadows to the south-east. A
 * post at every corner and free end; none where a chunk cuts it.
 */
function drawFences(g: Graphics, rs: Run[], bold = false) {
  // (R5 towns, 114: a horse park's corral or a yurt's pen, drawn a third thicker by the generator, bolder
  // and darker, its posts closer, so it reads round the trodden ground at a town's battle zoom.)
  const k = bold ? 2 : 1;
  const every = FT * (bold ? 5 : 6);
  const posts: [Pt, Run][] = [];
  const seen = new Set<string>();
  const post = (p: Pt, r: Run) => {
    const key = `${Math.round(p[0])},${Math.round(p[1])}`;
    if (!seen.has(key)) seen.add(key), posts.push([p, r]);
  };
  for (const r of rs) {
    for (const [E, a] of [
      [r.ends[0], 0],
      [r.ends[1], r.l],
    ] as const) {
      if (!E.cut) post(at(r, a, 0), r);
    }
    // (Clear of an end's own post; a post on a chunk's edge is drawn on its far side.)
    const [lo, hi] = [r.ends[0].cut ? 0 : FT * 2.5, r.ends[1].cut ? r.l : r.l - FT * 2.5];
    for (const k of nodes(r, every, lo, hi)) {
      const a = nodeA(r, k, every);
      if (a < hi) post(at(r, a, 0), r);
    }
  }
  const line = (r: Run, d: number, shift = 0) => {
    const [p, q] = [at(r, 0, d), at(r, r.l, d)];
    g.moveTo(p[0] + shift, p[1] + shift).lineTo(q[0] + shift, q[1] + shift);
  };
  // Shadows.
  for (const r of rs) line(r, 0, FT * 0.9 * k);
  g.stroke({ width: FT * 1.3 * k, color: 0x000000, alpha: bold ? 0.24 : 0.15, cap: 'butt' });
  const pw = FT * 1.45 * (bold ? 1.3 : 1);
  const [rail0, lit0] = bold ? [mix(WOOD[0], WOOD[1], 0.05), mix(WOOD[1], WOOD[2], 0.2)] : [RAIL, RAIL_LIT];
  const square = (p: Pt, r: Run, half: number, shift = 0) => {
    const c = (sa: number, sd: number) => [p[0] + (r.ux * sa + r.nx * sd) * half + shift, p[1] + (r.uy * sa + r.ny * sd) * half + shift];
    g.poly([...c(-1, -1), ...c(1, -1), ...c(1, 1), ...c(-1, 1)]);
  };
  for (const [p, r] of posts) square(p, r, pw * 0.55, FT * 1.1 * k);
  g.fill({ color: 0x000000, alpha: bold ? 0.3 : 0.2 });
  // The rails: ink under both, the wood, a lit edge on each.
  // (Butt ends: a rail cut at a chunk's edge carries on next door; corners and free ends have posts.)
  const rail = FT * 0.5 * k;
  const apart = FT * 0.5 * (bold ? 1.25 : 1);
  for (const [width, color] of [
    [rail + 3.4 * k, INK],
    [rail, rail0],
  ] as const) {
    for (const r of rs) for (const d of [apart, -apart]) line(r, d);
    g.stroke({ width, color, cap: 'butt', join: 'round' });
  }
  for (const r of rs) {
    const sun = r.lit >= 0 ? 1 : -1;
    for (const d of [apart, -apart]) line(r, d + sun * rail * 0.22);
  }
  g.stroke({ width: rail * 0.34, color: lit0, cap: 'butt' });
  // The posts: square tops in ink, the wood, the half toward the light lit.
  for (const [p, r] of posts) square(p, r, pw / 2);
  g.fill(rail0).stroke({ width: 1.8 * k, color: INK, join: 'round' });
  for (const [p, r] of posts) {
    const h = pw / 2 - 1.2;
    const c = (sa: number, sd: number) => [p[0] + (r.ux * sa + r.nx * sd) * h, p[1] + (r.uy * sa + r.ny * sd) * h];
    // The corner most toward the light, and the two beside it.
    const best = [
      [-1, -1],
      [1, -1],
      [1, 1],
      [-1, 1],
    ].reduce((m, q) => ((r.ux * q[0] + r.nx * q[1]) * LX + (r.uy * q[0] + r.ny * q[1]) * LY > (r.ux * m[0] + r.nx * m[1]) * LX + (r.uy * m[0] + r.ny * m[1]) * LY ? q : m));
    g.poly([...c(best[0], best[1]), ...c(-best[0], best[1]), ...c(best[0], -best[1])]);
  }
  g.fill({ color: lit0, alpha: 0.8 });
}

/**
 * Hedgerows, dry-stone walls and fences: one straight stretch each (`pts` its ends, `size` thick).
 * (`ox`, `oy`: the chunk's corner in world px, for the lattice along the lines; `side` the chunk's
 * width, where pieces are cut.)
 */
export function drawBounds(g: Graphics, shapes: Shape[], ox = 0, oy = 0, side = SQ * PX) {
  for (const [kind, draw] of [
    [HEDGE, drawHedges],
    [DRY_WALL, drawDryWalls],
    [FENCE, drawFences],
  ] as const) {
    const of = shapes.filter((s) => s.kind === kind);
    // (A corral's fence comes a third thicker than a field's: `rural::CORRAL_DRAWN_FT`.)
    const bold = kind === FENCE ? of.filter((s) => s.size > 0.19 * PX) : [];
    const rest = bold.length ? of.filter((s) => s.size <= 0.19 * PX) : of;
    if (rest.length) draw(g, runs(rest, ox, oy, side));
    if (bold.length) drawFences(g, runs(bold, ox, oy, side), true);
  }
}

/** Sail cloth: lit, shaded. */
const CLOTH = [0xe6dcc6, 0xc9bca2];
/** A tower mill's cap, painted boards over a dark tower, bare wood over a pale one: lit, shaded, the
 * ridge's highlight. */
const CAP_PAINT = [0xe3dccb, 0xbdb39d, 0xf1ecdf];
const CAP_WOOD = [0x9c7450, 0x7a5638, 0xb8916a];

/**
 * A tower mill from straight above (`pts`: the tower's middle, then the way its cap faces × PX and
 * the tower's radius, then whether its walls are pale × PX; `size` the sails' reach). The tower itself is its cone roof (its battered walls
 * rising to the curb); over it the cap, boat-shaped, blunt at the front where the windshaft comes
 * out, pointed at the back, lit on its north-west side; the tail pole and its struts running back
 * and down past the tower's foot to a small capstan wheel. The sails turn in an upright plane across
 * the front, so from above they show edge-on: a long narrow bar, each arm's stretch of it as long as
 * the arm leans across (their stop keyed by place), lattice bars across the bare ones, cloth along
 * the others, the stocks over them; its shadow cast well off to the south-east.
 */
export function drawSails(g: Graphics, s: Shape) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const a0 = s.pts[2] / PX;
  const rt = s.pts[3] > 0 ? s.pts[3] : 12 * FT;
  const R = s.size;
  const CAP = s.pts[4] > 0 ? CAP_WOOD : CAP_PAINT;
  const [dx, dy, ex, ey] = [Math.cos(a0), Math.sin(a0), -Math.sin(a0), Math.cos(a0)];
  // (u along the way the cap faces, v across it, from the tower's middle.)
  const P = (u: number, v: number): Pt => [cx + dx * u + ex * v, cy + dy * u + ey * v];
  const flat = (pts: Pt[]) => pts.flat();
  const seed = Math.round(cx * 7 + cy * 13);
  // The cap: widest a little forward of the middle, blunt at the front, pointed at the back.
  const [A, B, u0] = [rt * 0.68, rt * 0.56, rt * 0.08];
  const half = (u: number) => Math.max(0, B * (1 - ((u - u0) / (u > u0 ? 1.55 * A : 1.2 * (A + u0))) ** 2));
  const side = (sd: number) => {
    const out: Pt[] = [];
    for (let k = 0; k <= 16; k++) {
      const u = A - (2 * A * k) / 16;
      out.push(P(u, sd * half(u)));
    }
    return out;
  };
  const left = side(-1);
  const right = side(1);
  const cap = [...left, ...right.reverse()];
  // The sails: each arm's stretch along the bar (its lean across), the upper ones over the lower.
  const front = 0.95 * rt + 2 * FT;
  const phi = hash(seed, 3, 7) * (Math.PI / 2);
  const spread = hash(seed, 5, 1) < 0.5 ? [0, 2] : [0, 1, 2];
  const r0 = R * 0.2;
  const t = FT * 1.6;
  const arms = [0, 1, 2, 3]
    .map((k) => {
      const th = phi + (k * Math.PI) / 2;
      return { k, c: Math.cos(th), up: Math.sin(th) > 0, cloth: spread.includes(k) };
    })
    .sort((p, q) => Number(p.up) - Number(q.up));
  const B2 = (v0: number, v1: number, w: number, du = 0): number[] => flat([P(front + du - w, v0), P(front + du - w, v1), P(front + du + w, v1), P(front + du + w, v0)]);
  // Shadows: the sails' well off (they stand high), the cap's and the tail's close.
  const [sx, sy] = [R * 0.2, R * 0.2];
  const shade = (pts: number[], ox: number, oy: number) => pts.map((v, i) => v + (i % 2 ? oy : ox));
  for (const { c } of arms) if (Math.abs(c) > 0.05) g.poly(shade(B2(r0 * c, R * c, t), sx, sy));
  g.poly(shade(B2(-R * 0.08, R * 0.08, t * 1.4), sx, sy));
  g.fill({ color: 0x000000, alpha: 0.12 });
  g.poly(shade(flat(cap), FT * 1.4, FT * 1.4)).fill({ color: 0x000000, alpha: 0.16 });
  // The tail pole and its struts, to the capstan wheel past the tower's foot.
  const tail = rt + 5 * FT;
  const strutAt = -A * 0.35;
  for (const [width, color] of [
    [FT * 0.75 + 3.4, INK],
    [FT * 0.75, WOOD[1]],
  ] as const) {
    g.moveTo(...P(-A * 0.8, 0)).lineTo(...P(-tail, 0));
    for (const sd of [-1, 1]) g.moveTo(...P(strutAt, sd * half(strutAt) * 0.9)).lineTo(...P(-tail * 0.82, 0));
    g.stroke({ width, color, cap: 'round' });
  }
  g.moveTo(...P(-A * 0.8, ex * LX + ey * LY > 0 ? -FT * 0.15 : FT * 0.15)).lineTo(...P(-tail, ex * LX + ey * LY > 0 ? -FT * 0.15 : FT * 0.15)).stroke({ width: FT * 0.22, color: WOOD[2], alpha: 0.8 });
  const [wx, wy] = P(-tail, 0);
  const wr = FT * 1.5;
  g.poly(flat(Array.from({ length: 6 }, (_, k) => P(-tail + wr * Math.cos((k * Math.PI) / 3), wr * Math.sin((k * Math.PI) / 3))))).stroke({ width: FT * 0.3 + 2.4, color: INK, join: 'round' });
  g.poly(flat(Array.from({ length: 6 }, (_, k) => P(-tail + wr * Math.cos((k * Math.PI) / 3), wr * Math.sin((k * Math.PI) / 3))))).stroke({ width: FT * 0.3, color: WOOD[1], join: 'round' });
  for (let k = 0; k < 3; k++) g.moveTo(...P(-tail + wr * Math.cos((k * Math.PI) / 3), wr * Math.sin((k * Math.PI) / 3))).lineTo(...P(-tail - wr * Math.cos((k * Math.PI) / 3), -wr * Math.sin((k * Math.PI) / 3)));
  g.stroke({ width: FT * 0.18, color: INK });
  g.circle(wx, wy, FT * 0.35).fill(WOOD[0]).stroke({ width: 1.6, color: INK });
  // The cap: shaded, its lit half, the ridge along it, the boards' seams across, inked round.
  const litSide = ex * LX + ey * LY > 0 ? -1 : 1;
  g.poly(flat(cap)).fill(CAP[1]);
  g.poly(flat([...(litSide < 0 ? left : right.slice().reverse()), P(-A, 0), P(A, 0)])).fill(CAP[0]);
  for (let k = 1; k < 7; k++) {
    const u = A - (2 * A * k) / 7;
    const w = half(u);
    g.moveTo(...P(u, -w * 0.94)).lineTo(...P(u + FT * 0.5, 0)).lineTo(...P(u, w * 0.94));
  }
  g.stroke({ width: 1.4, color: mix(CAP[1], INK, 0.35), alpha: 0.7 });
  g.moveTo(...P(A, 0)).lineTo(...P(-A * 0.96, 0)).stroke({ width: FT * 0.32, color: CAP[2] });
  g.moveTo(...P(A, 0)).lineTo(...P(-A * 0.96, 0)).stroke({ width: 1.6, color: mix(CAP[1], INK, 0.5) });
  g.poly(flat(cap), true).stroke({ width: 4.4, color: INK, join: 'round' });
  // The windshaft out of the cap's front to the canister.
  for (const [width, color] of [
    [FT * 1.1 + 3.4, INK],
    [FT * 1.1, WOOD[1]],
  ] as const) g.moveTo(...P(A - FT, 0)).lineTo(...P(front, 0)).stroke({ width, color, cap: 'butt' });
  // The sails edge-on: each arm's frame a strip along the bar, its lattice bars across (cloth on
  // some), the stock along its middle.
  for (const { c, cloth } of arms) {
    if (Math.abs(c) < 0.05) continue;
    const [v0, v1] = [r0 * c, R * c];
    const box = B2(v0, v1, t);
    if (cloth) {
      g.poly(box).fill(CLOTH[0]);
      g.poly(B2(v0, v1, t * 0.4, litSide * t * 0.6)).fill({ color: CLOTH[1], alpha: 0.85 });
    } else {
      g.poly(box).fill({ color: WOOD[0], alpha: 0.35 });
      const bars = Math.max(2, Math.round((Math.abs(v1 - v0) / (R - r0)) * 9));
      for (let i = 0; i <= bars; i++) {
        const v = v0 + ((v1 - v0) * i) / bars;
        g.moveTo(...P(front - t, v)).lineTo(...P(front + t, v));
      }
      g.moveTo(...P(front - t, v0)).lineTo(...P(front - t, v1));
      g.moveTo(...P(front + t, v0)).lineTo(...P(front + t, v1));
      g.stroke({ width: 3.6, color: INK, cap: 'round' });
      for (let i = 0; i <= bars; i++) {
        const v = v0 + ((v1 - v0) * i) / bars;
        g.moveTo(...P(front - t, v)).lineTo(...P(front + t, v));
      }
      g.moveTo(...P(front - t, v0)).lineTo(...P(front - t, v1));
      g.moveTo(...P(front + t, v0)).lineTo(...P(front + t, v1));
      g.stroke({ width: 1.6, color: WOOD[2], cap: 'round' });
    }
    if (cloth) g.poly(box, true).stroke({ width: 2.6, color: INK, join: 'round' });
  }
  for (const { c } of arms) {
    if (Math.abs(c) < 0.05) continue;
    for (const [width, color] of [
      [FT * 0.6 + 3.2, INK],
      [FT * 0.6, WOOD[1]],
    ] as const) g.moveTo(...P(front, 0)).lineTo(...P(front, R * c * 1.02)).stroke({ width, color, cap: 'round' });
  }
  // The canister at the hub, over the stocks.
  const hub = FT * 1.2;
  g.poly(flat([P(front - hub, -hub), P(front - hub, hub), P(front + hub, hub), P(front + hub, -hub)])).fill(WOOD[0]).stroke({ width: 2.6, color: INK, join: 'round' });
  g.poly(flat([P(front - hub * 0.6, -hub * 0.6 * litSide), P(front + hub * 0.6, -hub * 0.6 * litSide), P(front + hub * 0.6, 0), P(front - hub * 0.6, 0)])).fill(WOOD[1]);
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
