/**
 * v63 countryside: the battlemap's field and croft bounds and a windmill's sails (`town::rural`), as
 * vectors over the ground, in the cartoon style of the sprites: a hedgerow a lumpy leafy band with a
 * soft shadow to the south-east; a dry-stone wall rough stones in two rows either side of a coping
 * line; a post-and-rail fence two brown rails on posts every 8 ft, seen from a little above; sails
 * four lattice arms, cloth on two or three, a hub cap and a faint shadow.
 *
 * A run comes in straight pieces that meet end to end at its corners (mitred there, as
 * `walls.ts` mitres a garden wall) and are cut at chunk edges. Everything along a piece (lumps,
 * stones, posts) is laid on a lattice of world distance along its line and hashed by the line, so a
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
 * the same in every chunk: `base` is the world distance along its line at its start, `line` a hash
 * of the line, `lit` how much its left side faces the light. */
interface Run {
  x0: number;
  y0: number;
  l: number;
  ux: number;
  uy: number;
  nx: number;
  ny: number;
  h: number;
  base: number;
  line: number;
  lit: number;
  ends: [End, End];
}

function runs(shapes: Shape[], ox: number, oy: number, side: number): Run[] {
  const segs = shapes.map((s) => {
    let [x0, y0, x1, y1] = s.pts;
    if (x1 < x0 - 1e-3 || (Math.abs(x1 - x0) <= 1e-3 && y1 < y0)) [x0, y0, x1, y1] = [x1, y1, x0, y0];
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const across = Math.round(((x0 + ox) * -uy + (y0 + oy) * ux) / 8);
    return { x0, y0, x1, y1, l, ux, uy, h: s.size / 2, base: (x0 + ox) * ux + (y0 + oy) * uy, line: across * 31 + Math.round(Math.atan2(uy, ux) * 200) };
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
    return { x0: s.x0, y0: s.y0, l: s.l, ux: s.ux, uy: s.uy, nx, ny, h: s.h, base: s.base, line: s.line, lit: nx * LX + ny * LY, ends };
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
    const w = (r.base + a) / step;
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
  for (let k = Math.floor((r.base + Math.max(0, a0)) / step) + 1; ; k++) {
    const a = k * step - r.base;
    if (a >= Math.min(a1, r.l) - 0.5) break;
    if (a <= a0 + 0.5) continue;
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

/** A free end's rounded cap (a hedge's clipped end) past its point, `s` of the half-width. */
function cap(r: Run, E: End, dir: 1 | -1, s: number, shift: Pt = [0, 0]): number[] {
  if (E.mitred || E.cut) return [];
  const h = r.h * s;
  const out: number[] = [];
  for (const [f, d] of [
    [0.45, 0.75],
    [0.62, 0],
    [0.45, -0.75],
  ]) {
    const p: Pt = [E.p[0] + r.ux * dir * f * h + r.nx * d * h * dir, E.p[1] + r.uy * dir * f * h + r.ny * d * h * dir];
    out.push(p[0] + shift[0], p[1] + shift[1]);
  }
  return out;
}

/** A piece's outline as a closed ring: its left side, the end cap, its right side back. */
const ring = (r: Run, L: number[], R: number[], caps: boolean, s = 1, shift: Pt = [0, 0]) => [...L, ...(caps ? cap(r, r.ends[1], 1, s, shift) : []), ...rev(R), ...(caps ? cap(r, r.ends[0], -1, s, shift) : [])];

/** Ink a piece's outline: its sides, and across its free ends only (none over a mitre or a chunk's edge). */
function inkOutline(g: Graphics, r: Run, L: number[], R: number[], caps: boolean, width: number) {
  const [A, B] = r.ends;
  const openA = A.mitred || A.cut;
  const openB = B.mitred || B.cut;
  const capB = caps ? cap(r, B, 1, 1) : [];
  const capA = caps ? cap(r, A, -1, 1) : [];
  if (openA && openB) {
    g.poly(L, false);
    g.poly(R, false);
  } else if (openA) g.poly([...L, ...capB, ...rev(R)], false);
  else if (openB) g.poly([...rev(R), ...capA, ...L], false);
  else g.poly([...L, ...capB, ...rev(R), ...capA], true);
  g.stroke({ width, color: INK, join: 'round', cap: 'round' });
}

/** Hedgerow leaves: dark, mid, light. */
const LEAF = [0x4b6d36, 0x688a42, 0x8fb058];

/**
 * Hedgerows (about 3.5 ft thick, 5 ft tall): a soft shadow to the south-east, a lumpy band of leaves
 * (lumps every 2 ft or so, the odd big one) in three tones, the top lit toward the north-west, leaves
 * picked out light on the lit side and dark on the other, a bold ink outline.
 */
function drawHedges(g: Graphics, rs: Run[]) {
  // Lumps of leaves about 2 ft long, rounded (four nodes each), each its own height, the odd one big.
  const step = FT * 0.55;
  const bump = (r: Run, sd: number) => (k: number) => {
    const m = Math.floor(k / 4);
    const t = (k - m * 4) / 4;
    const h = hash(m, r.line, sd + 3);
    const amp = h > 0.88 ? 0.34 : 0.1 + 0.16 * h;
    return t === 0 ? -0.05 : amp * (t === 0.5 ? 1 : 0.8);
  };
  const sides = rs.map((r) => [edge(r, 1, step, bump(r, 1), 1), edge(r, -1, step, bump(r, -1), 1)]);
  const reach = FT * 2.1;
  for (const [f, alpha] of [
    [1, 0.11],
    [0.55, 0.15],
  ]) {
    rs.forEach((r, i) => g.poly(ring(r, sides[i][0], sides[i][1], true).map((v) => v + reach * f)));
    g.fill({ color: 0x000000, alpha });
  }
  rs.forEach((r, i) => g.poly(ring(r, sides[i][0], sides[i][1], true)));
  g.fill(LEAF[0]);
  for (const [s, k, tone, alpha] of [
    [0.8, 0.17, LEAF[1], 1],
    [0.42, 0.44, LEAF[2], 0.85],
  ] as const) {
    rs.forEach((r) => {
      const sh: Pt = [LX * r.h * k, LY * r.h * k];
      const L = edge(r, 1, step, bump(r, 1), s, sh);
      const R = edge(r, -1, step, bump(r, -1), s, sh);
      g.poly(ring(r, L, R, true, s, sh));
    });
    g.fill({ color: tone, alpha });
  }
  // Leaves, a few every couple of feet, light toward the light and dark away from it.
  const dot = FT * 1.6;
  for (const light of [true, false]) {
    for (const r of rs) {
      for (let k = Math.ceil(r.base / dot); k * dot - r.base < r.l; k++) {
        const a = k * dot - r.base + (hash(k, r.line, 7) - 0.5) * dot * 0.6;
        if (a < 2 || a > r.l - 2) continue;
        const d = (hash(k, r.line, 8) * 2 - 1) * r.h * 0.68;
        const sunny = d * r.lit > 0 || (Math.abs(r.lit) < 0.2 && hash(k, r.line, 9) < 0.5);
        if (sunny !== light) continue;
        const [x, y] = at(r, a, d);
        g.circle(x, y, 1.5 + 1.6 * hash(k, r.line, 10));
      }
    }
    g.fill(light ? { color: 0x86aa52, alpha: 0.75 } : { color: LEAF[0], alpha: 0.8 });
  }
  // A crescent of shade along the side away from the light, over the leaves, inside the outline.
  rs.forEach((r, i) => {
    const sd = r.lit >= 0 ? -1 : 1;
    const E = sd > 0 ? sides[i][0] : sides[i][1];
    const sh: Pt = [LX * r.h * 0.2, LY * r.h * 0.2];
    const inner = edge(r, sd as 1 | -1, step, bump(r, sd), 0.8, sh);
    g.poly([...E, ...rev(inner)]);
  });
  g.fill({ color: LEAF[0], alpha: 0.55 });
  rs.forEach((r, i) => inkOutline(g, r, sides[i][0], sides[i][1], true, 2));
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
  const rough = (r: Run, sd: number) => (k: number) => (hash(k, r.line, sd + 11) - 0.5) * 0.18;
  const sides = rs.map((r) => [edge(r, 1, step, rough(r, 1), 1), edge(r, -1, step, rough(r, -1), 1)]);
  const reach = FT * 1.5;
  for (const [f, alpha] of [
    [1, 0.11],
    [0.55, 0.15],
  ]) {
    rs.forEach((r, i) => g.poly(ring(r, sides[i][0], sides[i][1], false).map((v) => v + reach * f)));
    g.fill({ color: 0x000000, alpha });
  }
  rs.forEach((r, i) => g.poly(ring(r, sides[i][0], sides[i][1], false)));
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
      const k0 = Math.floor((r.base - len) / len);
      const cut = (k: number) => k * len + (sd > 0 ? 0 : len * 0.5) + (hash(k, r.line, sd + 20) - 0.5) * len * 0.5 - r.base;
      for (let k = k0; cut(k) < r.l + r.h; k++) {
        const [a0, a1] = [cut(k) + 1, cut(k + 1) - 1];
        if (a1 < -r.h || a1 - a0 < 2) continue;
        const outer = r.h * (0.86 + 0.1 * hash(k, r.line, sd + 24));
        const j = (n: number) => (hash(k, r.line, sd * 7 + n) - 0.5) * 2.4;
        const pts = stone(a0, a1, sd * (r.h * 0.36 + 1), sd * outer, j);
        buckets[Math.floor(hash(k, r.line, sd + 30) * 3)].push(pts);
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
    for (let k = Math.ceil(r.base / jt); k * jt - r.base < r.l; k++) {
      const a = k * jt - r.base + (hash(k, r.line, 40) - 0.5) * jt * 0.5;
      if (a < 1.5 || a > r.l - 1.5) continue;
      const tilt = (hash(k, r.line, 41) - 0.5) * FT * 0.3;
      g.moveTo(...at(r, a - tilt, c)).lineTo(...at(r, a + tilt, -c));
    }
    g.stroke({ width: 1.3, color: STONE[0], alpha: 0.85 });
    // The coping's arris toward the light.
    const sun = r.lit >= 0 ? 1 : -1;
    g.moveTo(...at(r, clampA(0, sun * c, 0), sun * c * 0.7)).lineTo(...at(r, clampA(r.l, sun * c, 0), sun * c * 0.7));
    g.stroke({ width: 1.4, color: STONE[3], alpha: 0.5 * Math.max(0.3, Math.abs(r.lit)) });
  }
  rs.forEach((r, i) => inkOutline(g, r, sides[i][0], sides[i][1], false, 2.3));
}

/** How far up the screen a foot of height is drawn (upright things are seen from a little above). */
const RISE = 0.35 * FT;

/**
 * Post-and-rail fences: two rails (at about 2 and 3.5 ft) on posts every 8 ft, seen from a little
 * above, so the rails stand just up the screen from the line on the ground; the posts' and rails'
 * faint shadows to the south-east. A post at every corner and free end; none where a chunk cuts it.
 */
function drawFences(g: Graphics, rs: Run[]) {
  const every = FT * 8;
  const rails = [3.5, 1.9];
  const posts: Pt[] = [];
  const seen = new Set<string>();
  const post = (p: Pt) => {
    const key = `${Math.round(p[0])},${Math.round(p[1])}`;
    if (!seen.has(key)) seen.add(key), posts.push(p);
  };
  for (const r of rs) {
    for (const [E, a] of [
      [r.ends[0], 0],
      [r.ends[1], r.l],
    ] as const) {
      if (!E.cut) post(at(r, a, 0));
    }
    // (Clear of an end's own post; a post on a chunk's edge is drawn on its far side.)
    const [lo, hi] = [r.ends[0].cut ? 0 : FT * 3, r.ends[1].cut ? r.l : r.l - FT * 3];
    for (let k = Math.ceil((r.base + lo) / every); k * every - r.base < hi; k++) post(at(r, k * every - r.base, 0));
  }
  const ends = (r: Run): [Pt, Pt] => [at(r, 0, 0), at(r, r.l, 0)];
  // Shadows.
  for (const r of rs) {
    const [p, q] = ends(r);
    g.moveTo(p[0] + FT * 1.1, p[1] + FT * 1.1).lineTo(q[0] + FT * 1.1, q[1] + FT * 1.1);
  }
  g.stroke({ width: FT * 0.3, color: 0x000000, alpha: 0.13, cap: 'butt' });
  for (const p of posts) g.moveTo(p[0], p[1]).lineTo(p[0] + FT * 1.6, p[1] + FT * 1.6);
  g.stroke({ width: FT * 0.4, color: 0x000000, alpha: 0.16, cap: 'round' });
  // The rails: ink under both, then the wood, its top edge lit.
  const rail = FT * 0.36;
  for (const [width, color] of [
    [rail + 3.2, INK],
    [rail, WOOD[1]],
  ] as const) {
    for (const r of rs) {
      const [p, q] = ends(r);
      for (const hgt of rails) g.moveTo(p[0], p[1] - hgt * RISE).lineTo(q[0], q[1] - hgt * RISE);
    }
    // (Butt ends: a rail cut at a chunk's edge carries on next door; corners and free ends have posts.)
    g.stroke({ width, color, cap: 'butt', join: 'round' });
  }
  for (const r of rs) {
    const [p, q] = ends(r);
    for (const hgt of rails) g.moveTo(p[0], p[1] - hgt * RISE - rail * 0.25).lineTo(q[0], q[1] - hgt * RISE - rail * 0.25);
  }
  g.stroke({ width: rail * 0.5, color: mix(WOOD[2], 0xffffff, 0.3), alpha: 1 });
  // The posts, foot to top, a lighter top.
  const pw = FT * 0.42;
  for (const [width, color] of [
    [pw + 3.2, INK],
    [pw, WOOD[1]],
  ] as const) {
    for (const p of posts) g.moveTo(p[0], p[1]).lineTo(p[0], p[1] - 4 * RISE);
    g.stroke({ width, color, cap: 'butt' });
  }
  for (const p of posts) g.moveTo(p[0] - pw * 0.22, p[1] - 0.3 * RISE).lineTo(p[0] - pw * 0.22, p[1] - 3.9 * RISE);
  g.stroke({ width: pw * 0.35, color: WOOD[2], alpha: 0.7 });
  for (const p of posts) g.rect(p[0] - pw / 2, p[1] - 4 * RISE - pw * 0.3, pw, pw * 0.6);
  g.fill(WOOD[2]);
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
    if (of.length) draw(g, runs(of, ox, oy, side));
  }
}

/** Sail cloth: lit, shaded. */
const CLOTH = [0xe6dcc6, 0xc9bca2];

/**
 * A windmill's sails (`pts`: the hub, then the angle × PX; `size` their reach): four arms, each a
 * whip (the long timber) with a lattice of bars and laths on its trailing side, cloth spread over two
 * or three of them; a hub cap over the middle; a faint shadow cast down to the south-east. Small, it
 * reads as a pale cross.
 */
export function drawSails(g: Graphics, s: Shape) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const a0 = s.pts[2] / PX;
  const R = s.size;
  const [r0, w] = [R * 0.2, R * 0.21];
  const spread = hash(Math.round(a0 * 1000), 5, 1) < 0.5 ? [0, 2] : [0, 1, 2];
  const arms = [0, 1, 2, 3].map((k) => {
    const t = a0 + (k * Math.PI) / 2;
    const [dx, dy, ex, ey] = [Math.cos(t), Math.sin(t), -Math.sin(t), Math.cos(t)];
    const P = (r: number, x: number): Pt => [cx + dx * r + ex * x, cy + dy * r + ey * x];
    return { P, cloth: spread.includes(k), lit: ex * LX + ey * LY };
  });
  const frame = (P: (r: number, x: number) => Pt) => [...P(r0, w * 0.1), ...P(R, w * 0.1), ...P(R, w), ...P(r0 + w * 0.3, w)];
  // The shadow, well off (they stand high).
  const off = R * 0.14;
  for (const { P } of arms) g.poly(frame(P).map((v) => v + off));
  g.fill({ color: 0x000000, alpha: 0.12 });
  for (const { P } of arms) g.moveTo(...P(0, 0).map((v) => v + off) as Pt).lineTo(...P(R, 0).map((v) => v + off) as Pt);
  g.stroke({ width: FT * 0.6, color: 0x000000, alpha: 0.12 });
  const bars = 7;
  for (const { P, cloth, lit } of arms) {
    const box = frame(P);
    if (cloth) {
      // Cloth over the lattice: a shaded strip along its outer edge, the bars showing as folds.
      g.poly(box).fill(lit > 0 ? CLOTH[0] : mix(CLOTH[0], CLOTH[1], 0.5));
      g.poly([...P(r0 + w * 0.2, w * 0.72), ...P(R, w * 0.72), ...P(R, w), ...P(r0 + w * 0.3, w)]).fill({ color: CLOTH[1], alpha: 0.8 });
      for (let i = 1; i < bars; i++) {
        const r = r0 + ((R - r0) * i) / bars;
        g.moveTo(...P(r, w * 0.12)).lineTo(...P(r, w * 0.98));
      }
      g.stroke({ width: 1.6, color: mix(CLOTH[1], WOOD[0], 0.35), alpha: 0.8 });
    } else {
      // Bare lattice: bars across, a lath along the middle and the hemlath along the edge.
      for (let i = 0; i <= bars; i++) {
        const r = r0 + w * 0.3 * (1 - i / bars) + ((R - r0 - w * 0.3 * (1 - i / bars)) * i) / bars;
        g.moveTo(...P(r, w * 0.1)).lineTo(...P(r, w));
      }
      g.moveTo(...P(r0 + w * 0.15, w * 0.55)).lineTo(...P(R, w * 0.55));
      g.moveTo(...P(r0 + w * 0.3, w)).lineTo(...P(R, w));
      g.stroke({ width: 4.4, color: INK, cap: 'round' });
      for (let i = 0; i <= bars; i++) {
        const r = r0 + w * 0.3 * (1 - i / bars) + ((R - r0 - w * 0.3 * (1 - i / bars)) * i) / bars;
        g.moveTo(...P(r, w * 0.1)).lineTo(...P(r, w));
      }
      g.moveTo(...P(r0 + w * 0.15, w * 0.55)).lineTo(...P(R, w * 0.55));
      g.moveTo(...P(r0 + w * 0.3, w)).lineTo(...P(R, w));
      g.stroke({ width: 2, color: WOOD[2], cap: 'round' });
    }
    if (cloth) g.poly(box, true).stroke({ width: 2.6, color: INK, join: 'round' });
  }
  // The whips, over the sails.
  const whip = FT * 0.7;
  for (const [width, color] of [
    [whip + 3.4, INK],
    [whip, WOOD[1]],
  ] as const) {
    for (const { P } of arms) g.moveTo(...P(0, 0)).lineTo(...P(R * 1.02, 0));
    g.stroke({ width, color, cap: 'round' });
  }
  for (const { P, lit } of arms) {
    const sd = lit > 0 ? 1 : -1;
    g.moveTo(...P(R * 0.1, sd * whip * 0.22)).lineTo(...P(R, sd * whip * 0.22));
  }
  g.stroke({ width: whip * 0.3, color: WOOD[2], alpha: 0.8 });
  // The hub cap.
  const hub = FT * 1.4;
  g.circle(cx, cy, hub).fill(WOOD[0]).stroke({ width: 2.6, color: INK });
  g.circle(cx - hub * 0.12, cy - hub * 0.12, hub * 0.78).fill(WOOD[1]);
  g.circle(cx - hub * 0.32, cy - hub * 0.32, hub * 0.32).fill({ color: WOOD[2], alpha: 0.9 });
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
