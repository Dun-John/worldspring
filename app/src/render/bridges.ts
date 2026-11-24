// Bridges and piers on the battlemap, drawn in code like the atlas (assets/STYLE_GUIDE.md): a
// city's stone bridge with parapets, refuges and cutwaters; an old stone bridge, mossy and worn,
// on some roads and in some towns and cities; everywhere else a worn timber bridge of uneven
// planks with a post-and-rail fence; piers as the same planks on pilings.
import type { Graphics } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { PX } from './scale';
import { INK, FT, OUT } from './pen';

export { INK, FT };
/** Towards the light (north-west, the upper left of the screen). */
export const LX = -Math.SQRT1_2;
export const LY = -Math.SQRT1_2;
export const PLUM = 0x4a2f3a;
/** Wood as the props draw it (the fallen log, carts): dark, mid, light. */
export const WOOD = [0x5e4024, 0x7d5a3c, 0xb08a5a];
/** Plank tones, [mid, lit edge]: warm browns, one a little greyed with age. */
const PLANKS: [number, number][] = [
  [0x8a6238, 0xb08a5a],
  [0x7d5a3c, 0xa8825a],
  [0x93693e, 0xbc9262],
  [0x80664a, 0xa88d6a],
];
/** The underside between the planks: a lighter warm mid-brown, greyer than the planks so it reads as the side. */
const UNDERSIDE = 0x8e6e4c;
/** The deck's bold ink round its outer edge: every built edge's (`OUT`; quay coping, breakwater). */
const PLANK_INK = OUT;
/** The light ink between the planks: thin, part see-through, so the deck reads as boards, not a grid of bars. */
const SEAM = { width: 1.2, color: INK, alpha: 0.38, join: 'round' as const };
/** The planks' grain and knots: a soft shade of the plank itself. */
const grainOf = (base: number) => mix(base, WOOD[0], 0.45);
/** Stone: dark, mid, light, sunlit. */
const STONE_BASE = [0x6e675c, 0x9a9182, 0xc4baa6, 0xe4dccb];
export const STONE = STONE_BASE;
/** Old stone, weathered a shade darker and warmer: dark, mid, light, sunlit. */
const OLD_STONE = [0x5f584b, 0x8a8170, 0xaea38b, 0xd2c6a8];
/** Moss: dark, mid, light. */
export const MOSS = [0x4f7a32, 0x6f9a3e, 0xa8c45a];
/** Ivy leaves: dark, mid, light (the temperate leaf ramp). */
const IVY = [0x2f5a2c, 0x4f8a38, 0x8fbf4a];

/** `a` blended `t` of the way towards `b`. */
export function mix(a: number, b: number, t: number): number {
  const ch = (s: number) => Math.round(((a >> s) & 255) * (1 - t) + ((b >> s) & 255) * t);
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}

/** What a bridge stands on, from its chunk (`BattlemapLayer.bridgeGround`): the town's stone tint, the water's colour
 * under it, the land's at its ends, and whether a point is dry ground. Without it: plain grey stone, stock water. */
export interface BridgeGround {
  tone: ArrayLike<number>;
  water: number;
  land: number;
  dry: (x: number, y: number) => boolean;
}
/** The ground of the bridge being drawn (drawing is synchronous). */
let cur: BridgeGround | null = null;
/** A stone colour tinted by the town's stone (as the plaza's ramp: grey times the culture's tone). */
function tint(c: number): number {
  if (!cur) return c;
  const ch = (sh: number, k: number) => Math.max(0, Math.min(255, Math.round(((c >> sh) & 255) * cur!.tone[k])));
  return (ch(16, 0) << 16) | (ch(8, 1) << 8) | ch(0, 2);
}
/** A soft darker shade of what lies beneath (not black). */
const shadeOf = (under: number) => mix(under, 0x1a1c24, 0.5);
const waterShade = () => shadeOf(cur?.water ?? 0x4d8085);

/** A stable random per bridge, from where it is. */
export function rngAt(x: number, y: number): () => number {
  let seed = (Math.round(x * 7.1) * 73856093) ^ (Math.round(y * 3.3) * 19349663);
  return () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
}

/** A bridge's frame: `a` along the span from its first end, `b` across from one side (px). */
interface Frame {
  ox: number;
  oy: number;
  ux: number;
  uy: number;
  vx: number;
  vy: number;
  len: number;
  wid: number;
}

const atF = (f: Frame, a: number, b: number): [number, number] => [f.ox + f.ux * a + f.vx * b, f.oy + f.uy * a + f.vy * b];
/** How much a direction (a, b) in the frame faces the light, -1 .. 1. */
function facing(f: Frame, a: number, b: number): number {
  const l = Math.hypot(a, b) || 1;
  return ((f.ux * a + f.vx * b) * LX + (f.uy * a + f.vy * b) * LY) / l;
}

/** A cast shadow: the water's darker shade where the ground below is water, a faint darker land where it is dry
 * (cell by cell inside the polygon, so a shadow never darkens the bank like a scorch). */
function castShadow(g: Graphics, poly: number[], alpha: number) {
  if (!cur) return void g.poly(poly).fill({ color: waterShade(), alpha });
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (let i = 0; i < poly.length; i += 2) {
    x0 = Math.min(x0, poly[i]);
    x1 = Math.max(x1, poly[i]);
    y0 = Math.min(y0, poly[i + 1]);
    y1 = Math.max(y1, poly[i + 1]);
  }
  const inside = (x: number, y: number) => {
    let c = false;
    for (let i = 0, j = poly.length - 2; i < poly.length; j = i, i += 2) {
      if (poly[i + 1] > y !== poly[j + 1] > y && x < ((poly[j] - poly[i]) * (y - poly[i + 1])) / (poly[j + 1] - poly[i + 1]) + poly[i]) c = !c;
    }
    return c;
  };
  const wet: number[][] = [];
  const dryCells: number[][] = [];
  const cs = FT * 0.5;
  for (let y = y0; y < y1; y += cs) {
    for (let x = x0; x < x1; x += cs) {
      if (!inside(x + cs / 2, y + cs / 2)) continue;
      (cur.dry(x + cs / 2, y + cs / 2) ? dryCells : wet).push([x, y]);
    }
  }
  const col = mix(cur.land, 0x1a1c24, 0.4);
  for (const [x, y] of wet) g.rect(x, y, cs + 0.5, cs + 0.5);
  if (wet.length) g.fill({ color: waterShade(), alpha });
  for (const [x, y] of dryCells) g.rect(x, y, cs + 0.5, cs + 0.5);
  if (dryCells.length) g.fill({ color: col, alpha: Math.min(alpha, 0.16) * 0.8 });
}

/** A deck (`battlemap::ShapeKind::Deck`): its first edge runs along the span; `size` is the style. */
export function drawDeck(g: Graphics, s: Shape, gr?: BridgeGround) {
  cur = gr ?? null;
  if (s.pts.length < 8) return;
  const [x0, y0, x1, y1] = s.pts;
  const [x3, y3] = [s.pts[6], s.pts[7]];
  const len = Math.hypot(x1 - x0, y1 - y0) || 1;
  const wid = Math.hypot(x3 - x0, y3 - y0) || 1;
  const f: Frame = { ox: x0, oy: y0, ux: (x1 - x0) / len, uy: (y1 - y0) / len, vx: (x3 - x0) / wid, vy: (y3 - y0) / wid, len, wid };
  const style = Math.round(s.size / PX);
  if (style === 4) return quayDeck(g, s);
  if (style === 2) stoneBridge(g, f, rngAt(x0 + x1, y0 + y1));
  else if (style === 3) oldStoneBridge(g, f, rngAt(x0 + x1, y0 + y1));
  else timber(g, f, rngAt(x0 + x1, y0 + y1), style === 1);
}

/** A timber shore quay's piece (`Deck` style 4): its face's points, then its back's the other way
 * (as many), each piece about 40 ft along the shore. */
function quayFrame(s: Shape) {
  const m = Math.floor(s.pts.length / 4);
  const face: [number, number][] = [];
  const back: [number, number][] = [];
  for (let k = 0; k < m; k++) {
    face.push([s.pts[k * 2], s.pts[k * 2 + 1]]);
    const j = 2 * m - 1 - k;
    back.push([s.pts[j * 2], s.pts[j * 2 + 1]]);
  }
  return { m, face, back };
}

/** A timber shore quay's deck: planks laid from the face back to the shore, mid-brown with a lit
 * edge, the odd grain stroke or knot, inked boldly over the underside, as the piers'. */
function quayDeck(g: Graphics, s: Shape) {
  const { m, face, back } = quayFrame(s);
  if (m < 2) return;
  const along = [0];
  for (let k = 1; k < m; k++) along.push(along[k - 1] + Math.hypot(face[k][0] - face[k - 1][0], face[k][1] - face[k - 1][1]));
  const run = along[m - 1];
  if (run < FT) return;
  // The face's and the back's point `a` along the face (px).
  const ends = (a: number): [[number, number], [number, number]] => {
    let k = 0;
    while (k < m - 2 && along[k + 1] < a) k++;
    const t = Math.min(1, Math.max(0, (a - along[k]) / (along[k + 1] - along[k] || 1)));
    const lerp = (p: [number, number][]): [number, number] => [p[k][0] + (p[k + 1][0] - p[k][0]) * t, p[k][1] + (p[k + 1][1] - p[k][1]) * t];
    return [lerp(face), lerp(back)];
  };
  // A point `d` px from the face toward the back (`d` < 0 out over the water).
  const at = (a: number, d: number): [number, number] => {
    const [f, b] = ends(a);
    const w = Math.hypot(b[0] - f[0], b[1] - f[1]) || 1;
    return [f[0] + ((b[0] - f[0]) / w) * d, f[1] + ((b[1] - f[1]) / w) * d];
  };
  const wid = (a: number) => {
    const [f, b] = ends(a);
    return Math.hypot(b[0] - f[0], b[1] - f[1]);
  };
  const rnd = rngAt(face[0][0] + face[m - 1][0], face[0][1] + face[m - 1][1]);
  g.poly([...face.flat(), ...[...back].reverse().flat()]).fill(UNDERSIDE);
  for (let a = 0; a < run - FT * 0.3; ) {
    const a1 = Math.min(run, a + FT * (2.2 + rnd() * 0.2));
    const [base, lit] = PLANKS[Math.floor(rnd() * PLANKS.length)];
    const out = -FT * (0.1 + rnd() * 0.4);
    const w0 = wid(a), w1 = wid(a1);
    if (Math.max(w0, w1) < FT * 0.4) {
      a = a1 + FT * 0.22;
      continue;
    }
    const plank = (t0: number, t1: number, inset: number) => [...at(a + (a1 - a) * t0, out + inset), ...at(a + (a1 - a) * t1, out + inset), ...at(a + (a1 - a) * t1, w1 - inset * 0.3), ...at(a + (a1 - a) * t0, w0 - inset * 0.3)];
    g.poly(plank(0, 1, 0)).fill(lit);
    g.poly(plank(0.3, 1, 0)).fill(base);
    g.poly(plank(0.9, 1, 0)).fill(mix(base, WOOD[0], 0.4));
    if (rnd() < 0.55) {
      const ga = 0.3 + rnd() * 0.3;
      const gd0 = Math.max(w0, w1) * (0.15 + rnd() * 0.3);
      const gd1 = gd0 + Math.max(w0, w1) * (0.2 + rnd() * 0.3);
      const [gx0, gy0] = at(a + (a1 - a) * ga, gd0);
      const [gx1, gy1] = at(a + (a1 - a) * (ga + (rnd() - 0.5) * 0.25), (gd0 + gd1) / 2);
      const [gx2, gy2] = at(a + (a1 - a) * ga, gd1);
      g.moveTo(gx0, gy0).quadraticCurveTo(gx1, gy1, gx2, gy2).stroke({ width: FT * 0.09, color: grainOf(base), alpha: 0.7, cap: 'round' });
    }
    if (rnd() < 0.12) {
      const [kx, ky] = at(a + (a1 - a) * (0.4 + rnd() * 0.2), Math.max(w0, w1) * (0.2 + rnd() * 0.6));
      g.ellipse(kx, ky, FT * 0.2, FT * 0.2).fill({ color: grainOf(base), alpha: 0.7 });
    }
    g.poly(plank(0, 1, 0)).stroke(SEAM);
    // (The deck's outer edge, over the water, inked boldly.)
    g.moveTo(...at(a, out)).lineTo(...at(a1, out)).stroke({ width: PLANK_INK, color: INK, cap: 'round' });
    a = a1 + FT * 0.12;
  }
}

/** A road bridge (`ShapeKind::RoadBridge`): its centre line, `size` wide, then `[material, 0]`
 * (0 timber, 1 old stone). */
export function drawRoadBridge(g: Graphics, s: Shape, gr?: BridgeGround) {
  cur = gr ?? null;
  const [x0, y0, x1, y1] = s.pts;
  const len = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / len, (y1 - y0) / len];
  const [vx, vy] = [-uy, ux];
  const f: Frame = { ox: x0 - vx * s.size * 0.5, oy: y0 - vy * s.size * 0.5, ux, uy, vx, vy, len, wid: s.size };
  const old = s.pts.length >= 6 && Math.round(s.pts[4] / PX) === 1;
  if (old) oldStoneBridge(g, f, rngAt(x0 + x1, y0 + y1));
  else timber(g, f, rngAt(x0 + x1, y0 + y1), true);
}

/** A frame `len` long and `wid` wide whose first end's middle is (x, y), along unit (ux, uy). */
function frameFrom(x: number, y: number, ux: number, uy: number, len: number, wid: number): Frame {
  const [vx, vy] = [-uy, ux];
  return { ox: x - vx * wid * 0.5, oy: y - vy * wid * 0.5, ux, uy, vx, vy, len, wid };
}

/** River stones: wet, dark and mossy, as the bed shows through shallow water. */
const WET_STONE = [0x5d5a4f, 0x77735f, 0x8d8a72];

/**
 * A ford (`ShapeKind::Ford`): its points are stepping stones (only where there is water, laid
 * by the generator), flat and wet, some half under the water, a faint ripple round each.
 */
export function drawFord(g: Graphics, s: Shape, gr?: BridgeGround) {
  cur = gr ?? null;
  const n = s.pts.length / 2;
  const rnd = rngAt(s.pts[0] + s.pts[n * 2 - 2], s.pts[1] + s.pts[n * 2 - 1]);
  const stones: [number, number, number, number, boolean][] = [];
  for (let i = 0; i < n; i++) stones.push([s.pts[i * 2], s.pts[i * 2 + 1], FT * (1.35 + rnd() * 0.5), rnd() * Math.PI, rnd() < 0.3]);
  // Ripples, then the stones; one in three sits low, the water over its edges.
  for (const [x, y, r] of stones) g.ellipse(x + FT * 0.25, y + FT * 0.35, r * 1.3, r * 1.05).stroke({ width: 1.5, color: 0xffffff, alpha: 0.28 });
  for (const [x, y, r, turn, low] of stones) {
    const pts: number[] = [];
    for (let j = 0; j < 8; j++) {
      const t = turn + (j / 8) * Math.PI * 2;
      const rr = r * (0.82 + 0.18 * Math.sin(j * 2.1 + turn * 3));
      pts.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr * 0.8);
    }
    const a = low ? 0.7 : 1.0;
    castShadow(g, pts.map((v, i) => v + FT * (i % 2 ? 0.25 : 0.2)), 0.15 * a);
    g.poly(pts).fill({ color: WET_STONE[1], alpha: a }).stroke({ width: 1.8, color: INK, alpha: a });
    g.ellipse(x + LX * r * 0.25, y + LY * r * 0.25, r * 0.5, r * 0.35).fill({ color: WET_STONE[2], alpha: 0.6 * a });
    if (rnd() < 0.4) g.ellipse(x - LX * r * 0.3, y - LY * r * 0.3, r * 0.35, r * 0.25).fill({ color: 0x5e6b3a, alpha: 0.45 * a });
  }
}

/**
 * A ferry (`ShapeKind::Ferry`): its line runs between the ends of a timber jetty out from each
 * bank (24 ft long, as `battlemap::JETTY_FT`); a rope on posts across, and the raft (`size`
 * wide) at the third point.
 */
export function drawFerry(g: Graphics, s: Shape, gr?: BridgeGround) {
  cur = gr ?? null;
  const [x0, y0, x1, y1] = s.pts;
  const len = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / len, (y1 - y0) / len];
  const rnd = rngAt(x0 + x1, y0 + y1);
  const jetty = FT * 24;
  const jw = FT * 9;
  // Jetties: from each line end back towards its bank.
  timber(g, frameFrom(x0 - ux * jetty, y0 - uy * jetty, ux, uy, jetty, jw), rnd, false);
  timber(g, frameFrom(x1, y1, ux, uy, jetty, jw), rnd, false);
  for (const [x, y] of [
    [x0, y0],
    [x1, y1],
  ]) {
    for (const side of [-1, 1]) logEnd(g, x - uy * side * jw * 0.5, y + ux * side * jw * 0.5, FT * 0.85);
  }
  // The rope, sagging a little downstream of the line, with its shadow.
  const sag = len * 0.04;
  const rope = (dx: number, dy: number) => {
    g.moveTo(x0 + dx, y0 + dy);
    g.quadraticCurveTo((x0 + x1) * 0.5 - uy * sag + dx, (y0 + y1) * 0.5 + ux * sag + dy, x1 + dx, y1 + dy);
  };
  rope(FT * 1.2, FT * 1.2);
  g.stroke({ width: FT * 0.7, color: 0x000000, alpha: 0.2 });
  rope(0, 0);
  g.stroke({ width: FT * 1.1, color: INK });
  rope(0, 0);
  g.stroke({ width: FT * 0.5, color: 0xc9a66b });
  // The raft (where the battlemap puts its planks: the shape's third point, on the rope).
  const rw = Math.max(s.size * 1.2, FT * 14);
  const rl = rw * 1.6;
  const [cx, cy] = s.pts.length >= 6 ? [s.pts[4], s.pts[5]] : [x0 + ux * len * 0.3, y0 + uy * len * 0.3];
  const f = frameFrom(cx - ux * rl * 0.5, cy - uy * rl * 0.5, ux, uy, rl, rw);
  const corners = [atF(f, 0, 0), atF(f, rl, 0), atF(f, rl, rw), atF(f, 0, rw)].flat();
  castShadow(g, corners.map((v, i) => v + FT * (i % 2 ? 1.6 : 1.6)), 0.25);
  timber(g, f, rnd, false);
  for (const [a, b] of [
    [FT, FT * 0.2],
    [rl - FT, FT * 0.2],
    [FT, rw - FT * 0.2],
    [rl - FT, rw - FT * 0.2],
  ]) {
    logEnd(g, ...atF(f, a, b), FT * 0.7);
  }
}

/**
 * Worn timber in the props' manner (the fallen log, crates): broad planks across the span, two to
 * a 5-ft square, each a little skewed, two flat tones (the lit edge, the body) with a dark far
 * edge, the odd bold grain stroke or knot, inked boldly over a mid-brown underside. A post-and-rail
 * fence of logs along both sides, or log pilings for a pier.
 */
function timber(g: Graphics, f: Frame, rnd: () => number, fence: boolean) {
  const at = (a: number, b: number) => atF(f, a, b);
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const { len, wid } = f;
  // Shadow on the water, the underside and the stringers the planks lie on.
  const sh = FT * 2;
  castShadow(g, quad(0, len, -FT * 0.3, wid + FT * 0.3).map((v) => v + sh), 0.25);
  g.poly(quad(FT * 0.2, len - FT * 0.2, 0, wid)).fill(UNDERSIDE).stroke({ width: PLANK_INK, color: INK, join: 'round' });
  const nStr = wid > FT * 18 ? 4 : 3;
  for (let k = 0; k < nStr; k++) {
    const b = wid * (0.12 + (0.76 * k) / (nStr - 1));
    g.poly(quad(FT * 0.2, len - FT * 0.2, b - FT * 0.5, b + FT * 0.5)).fill(WOOD[1]);
  }
  const litA1 = facing(f, 1, 0) > 0;
  // Planks two to a square, laid from the first end; the last one takes up what is left.
  const n = Math.max(1, Math.round((len - FT * 0.3) / (FT * 2.5)));
  const pw = (len - FT * 0.3) / n;
  for (let k = 0; k < n; k++) {
    const a0 = FT * 0.15 + pw * k + FT * 0.06;
    const a1 = FT * 0.15 + pw * (k + 1) - FT * 0.06;
    const w = a1 - a0;
    const [base, lit] = PLANKS[Math.floor(rnd() * PLANKS.length)];
    const b0 = FT * (0.05 + rnd() * 0.25);
    const b1 = wid - FT * (0.05 + rnd() * 0.25);
    const [s0, s1] = [(rnd() - 0.5) * FT * 0.12, (rnd() - 0.5) * FT * 0.12];
    // A point `t` of the way across the plank (0 the a0 edge) at its b0 or b1 end, `d` in from it.
    const end0 = (t: number) => at(a0 + s0 + w * t, b0);
    const end1 = (t: number) => at(a0 + s1 + w * t, b1);
    const band = (t0: number, t1: number) => [...end0(t0), ...end0(t1), ...end1(t1), ...end1(t0)];
    const outline = band(0, 1);
    // Two flat tones: the lit side of the plank, then the rest, then a dark edge on the far side.
    g.poly(outline).fill(lit);
    const dark = mix(base, WOOD[0], 0.4);
    if (litA1) {
      g.poly(band(0, 0.72)).fill(base);
      g.poly(band(0, 0.1)).fill(dark);
    } else {
      g.poly(band(0.28, 1)).fill(base);
      g.poly(band(0.9, 1)).fill(dark);
    }
    // One bold grain stroke on some planks, and the odd knot.
    if (rnd() < 0.55) {
      const ga = 0.35 + rnd() * 0.3;
      const gb = 0.1 + rnd() * 0.4;
      const gl = 0.25 + rnd() * 0.25;
      const [gx0, gy0] = at(a0 + w * ga, b0 + (b1 - b0) * gb);
      const [gx1, gy1] = at(a0 + w * (ga + (rnd() - 0.5) * 0.3), b0 + (b1 - b0) * (gb + gl * 0.5));
      const [gx2, gy2] = at(a0 + w * ga, b0 + (b1 - b0) * (gb + gl));
      g.moveTo(gx0, gy0).quadraticCurveTo(gx1, gy1, gx2, gy2).stroke({ width: FT * 0.09, color: grainOf(base), alpha: 0.7, cap: 'round' });
    }
    if (rnd() < 0.12) {
      const [kx, ky] = at(a0 + w * (0.4 + rnd() * 0.2), b0 + (b1 - b0) * (0.2 + rnd() * 0.6));
      g.ellipse(kx, ky, FT * 0.2, FT * 0.2).fill({ color: grainOf(base), alpha: 0.7 });
    }
    g.poly(outline).stroke(SEAM);
  }
  if (fence) railings(g, f, rnd);
  else pilings(g, f, rnd);
}

/** A log seen end on, standing up: bark rim, pale cut top with a growth ring (the fallen log's end). */
function logEnd(g: Graphics, x: number, y: number, r: number) {
  const [cx, cy] = [x + LX * r * 0.08, y + LY * r * 0.08];
  castShadow(g, Array.from({ length: 10 }, (_, i) => [x + FT * 0.5 + Math.cos(i * 0.628) * r, y + FT * 0.5 + Math.sin(i * 0.628) * r]).flat(), 0.25);
  g.circle(x, y, r).fill(WOOD[0]).stroke({ width: 2.2, color: INK });
  g.circle(cx, cy, r * 0.74).fill(0xd2a26a);
  g.circle(cx, cy, r * 0.42).stroke({ width: 1.2, color: 0xa87a4c });
  g.circle(cx, cy, r * 0.1).fill(0xa87a4c);
}

/** Log posts every 7-9 ft along both sides with a log rail between, sagging a little; one may be broken. */
function railings(g: Graphics, f: Frame, rnd: () => number) {
  const at = (a: number, b: number) => atF(f, a, b);
  const { len, wid } = f;
  const n = Math.max(2, Math.round(len / (FT * 8)) + 1);
  const rw = FT * 1.05;
  for (const b of [-FT * 0.15, wid + FT * 0.15]) {
    const out = b < 0 ? -1 : 1;
    const posts: number[] = [];
    for (let k = 0; k < n; k++) {
      const a = FT * 0.7 + ((len - FT * 1.4) * k) / (n - 1);
      posts.push(k === 0 || k === n - 1 ? a : a + (rnd() - 0.5) * FT * 0.8);
    }
    const brokenAt = n > 3 && rnd() < 0.5 ? 1 + Math.floor(rnd() * (n - 3)) : -1;
    const sags = posts.map(() => out * (rnd() - 0.3) * FT * 0.25);
    // Each rail as a path: whole between posts, or two stubs where one is broken.
    const path = (dx: number, dy: number) => {
      for (let k = 0; k + 1 < n; k++) {
        const [a, c] = [posts[k], posts[k + 1]];
        const p = (t: number, db = 0): [number, number] => {
          const [x, y] = at(a + (c - a) * t, b + sags[k] * Math.sin(t * Math.PI) + db);
          return [x + dx, y + dy];
        };
        if (k === brokenAt) {
          g.moveTo(...p(0)).lineTo(...p(0.3, out * FT * 0.5));
          g.moveTo(...p(1)).lineTo(...p(0.76, out * FT * 0.3));
        } else g.moveTo(...p(0)).lineTo(...p(0.5)).lineTo(...p(1));
      }
    };
    path(FT * 0.9, FT * 0.9);
    g.stroke({ width: rw, color: shadeOf(WOOD[1]), alpha: 0.22, cap: 'round', join: 'round' });
    path(0, 0);
    g.stroke({ width: rw + 4.4, color: INK, cap: 'round', join: 'round' });
    path(0, 0);
    g.stroke({ width: rw, color: WOOD[1], cap: 'round', join: 'round' });
    path(LX * rw * 0.22, LY * rw * 0.22);
    g.stroke({ width: rw * 0.38, color: WOOD[2], cap: 'round', join: 'round' });
    for (const a of posts) logEnd(g, ...at(a, b), FT * (0.8 + rnd() * 0.12));
  }
}

/** A pier's pilings: log ends along both sides, standing out of the water. */
function pilings(g: Graphics, f: Frame, rnd: () => number) {
  const { len, wid } = f;
  const n = Math.max(2, Math.round(len / (FT * 9)) + 1);
  for (const b of [-FT * 0.3, wid + FT * 0.3]) {
    for (let k = 0; k < n; k++) {
      const a = FT * 0.8 + ((len - FT * 1.6) * k) / (n - 1);
      logEnd(g, ...atF(f, a, b), FT * (0.85 + rnd() * 0.15));
    }
  }
}

/**
 * A city's stone bridge: paved in setts with worn wheel ruts, a parapet of coping stones along
 * both sides bulging into a refuge over each pier, the piers' pointed cutwaters out in the
 * stream with a wake round their tips, and squared newels at the four corners.
 */
function stoneBridge(g: Graphics, f: Frame, rnd: () => number) {
  const STONE = STONE_BASE.map(tint);
  const at = (a: number, b: number) => atF(f, a, b);
  const { len, wid } = f;
  const tp = FT * 2;
  const land = Math.min(FT * 12, len * 0.2);
  const nPiers = Math.max(0, Math.round((len - 2 * land) / (FT * 30)));
  const piers = Array.from({ length: nPiers }, (_, k) => land + ((len - 2 * land) * (k + 1)) / (nPiers + 1));
  const [hw, bay, pr] = [FT * 3.6, FT * 2.4, FT * 8.5];
  // One side's outer edge, bays and all: `out` -1 for the b = 0 side, 1 for the b = wid side.
  const edge = (out: number, inset: number): number[] => {
    const b = (d: number) => (out < 0 ? -d + inset : wid + d - inset);
    const pts: number[] = [...at(0, b(0))];
    for (const c of piers) {
      pts.push(...at(c - hw * 1.2, b(0)), ...at(c - hw * 0.6 + inset * 0.5, b(bay)), ...at(c + hw * 0.6 - inset * 0.5, b(bay)), ...at(c + hw * 1.2, b(0)));
    }
    pts.push(...at(len, b(0)));
    return pts;
  };
  const backwards = (pts: number[]) => pts.flatMap((_, i) => (i % 2 ? [] : [pts[pts.length - 2 - i], pts[pts.length - 1 - i]]));
  const deck = [...edge(-1, 0), ...backwards(edge(1, 0))];
  const cutwater = (c: number, out: number): number[][] => {
    const b = (d: number) => (out < 0 ? -d : wid + d);
    const left = [...at(c - hw, b(-FT)), ...at(c - hw, b(pr * 0.3)), ...at(c, b(pr)), ...at(c, b(-FT))];
    const right = [...at(c, b(-FT)), ...at(c, b(pr)), ...at(c + hw, b(pr * 0.3)), ...at(c + hw, b(-FT))];
    return [left, right];
  };
  const sh = FT * 3.2;
  castShadow(g, deck.map((v) => v + sh), 0.25);
  for (const c of piers) for (const out of [-1, 1]) for (const p of cutwater(c, out)) castShadow(g, p.map((v) => v + sh), 0.25);
  // Cutwaters: a wake round the tip, two facets lit or shaded by which way they face.
  for (const c of piers) {
    for (const out of [-1, 1]) {
      const b = (d: number) => (out < 0 ? -d : wid + d);
      g.moveTo(...at(c - hw - FT * 0.5, b(pr * 0.3))).lineTo(...at(c, b(pr + FT * 0.7))).lineTo(...at(c + hw + FT * 0.5, b(pr * 0.3)));
      g.stroke({ width: 1.6, color: 0xe6f4ec, alpha: 0.4, join: 'round' });
      const [left, right] = cutwater(c, out);
      for (const [p, na] of [[left, -1], [right, 1]] as const) {
        const lit = facing(f, na * pr * 0.7, out * hw);
        const tone = lit > 0.3 ? STONE[2] : lit > -0.3 ? STONE[1] : mix(STONE[0], PLUM, 0.15);
        g.poly(p).fill(tone);
      }
      g.poly([...left.slice(0, 6), ...right.slice(4, 8)]).stroke({ width: 2.2, color: INK, join: 'round' });
      g.moveTo(...at(c, b(pr))).lineTo(...at(c, b(bay)));
      g.moveTo(...at(c - hw, b(pr * 0.3 + FT * 0.2))).lineTo(...at(c - hw * 0.45, b(pr * 0.62)));
      g.moveTo(...at(c + hw, b(pr * 0.3 + FT * 0.2))).lineTo(...at(c + hw * 0.45, b(pr * 0.62)));
      g.stroke({ width: 1.2, color: STONE[0] });
    }
  }
  // Paving: rows of setts across the span, some a shade lighter or darker, and worn ruts.
  g.poly(deck).fill(tint(0xa39a8a));
  const joints: number[][] = [];
  for (let a = 0; a < len; ) {
    const a1 = Math.min(len, a + FT * (2 + rnd() * 0.5));
    joints.push([...at(a1, 0), ...at(a1, wid)]);
    for (let b = -rnd() * FT * 2.4; b < wid; ) {
      const b1 = b + FT * (2.8 + rnd() * 1.2);
      if (b1 < wid && b1 > 0) joints.push([...at(a + FT * 0.05, b1), ...at(a1 - FT * 0.05, b1)]);
      const r = rnd();
      if (r < 0.16) {
        const [c0, c1] = [Math.max(0, b) + FT * 0.15, Math.min(wid, b1) - FT * 0.15];
        if (c1 > c0) g.poly([...at(a + FT * 0.15, c0), ...at(a1 - FT * 0.15, c0), ...at(a1 - FT * 0.15, c1), ...at(a + FT * 0.15, c1)]).fill(r < 0.08 ? tint(0xaea594) : tint(0x989081));
      }
      b = b1;
    }
    a = a1;
  }
  for (const [x0, y0, x1, y1] of joints) g.moveTo(x0, y0).lineTo(x1, y1);
  g.stroke({ width: 1.3, color: tint(0x7a7266), alpha: 0.8 });
  if (wid > FT * 14) {
    for (const d of [-FT * 2.6, FT * 2.6]) g.moveTo(...at(0, wid / 2 + d)).lineTo(...at(len, wid / 2 + d));
    g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.1 });
  }
  // Parapets: their shadow on the deck, then ink, stone, the coping and its sunlit edge.
  const rails = [edge(-1, tp * 0.5), edge(1, tp * 0.5)];
  for (const p of rails) g.poly(p.map((v) => v + FT * 0.8), false);
  g.stroke({ width: tp, color: shadeOf(tint(0xa39a8a)), alpha: 0.25, join: 'round' });
  for (const p of rails) g.poly(p, false);
  g.stroke({ width: tp + 4, color: INK, join: 'round' });
  for (const p of rails) g.poly(p, false);
  g.stroke({ width: tp, color: STONE[1], join: 'round' });
  for (const p of rails) g.poly(p, false);
  g.stroke({ width: tp * 0.55, color: STONE[2], join: 'round' });
  for (const p of rails) g.poly(p.map((v, i) => v + (i % 2 ? LY : LX) * tp * 0.24), false);
  g.stroke({ width: 1.5, color: STONE[3], alpha: 0.9, join: 'round' });
  for (const p of rails) {
    for (let i = 0; i + 3 < p.length; i += 2) {
      const [x0, y0] = [p[i], p[i + 1]];
      const l = Math.hypot(p[i + 2] - x0, p[i + 3] - y0) || 1;
      const [ux, uy] = [(p[i + 2] - x0) / l, (p[i + 3] - y0) / l];
      for (let s = FT * (1 + rnd() * 2); s < l - FT; s += FT * (2.6 + rnd() * 1.4)) {
        const [x, y] = [x0 + ux * s, y0 + uy * s];
        g.moveTo(x - uy * tp * 0.5, y + ux * tp * 0.5).lineTo(x + uy * tp * 0.5, y - ux * tp * 0.5);
      }
    }
  }
  g.stroke({ width: 1.4, color: STONE[0] });
  // Newels at the four corners.
  for (const a of [tp * 0.6, len - tp * 0.6]) {
    for (const b of [tp * 0.5, wid - tp * 0.5]) {
      const n = tp * 0.95;
      g.poly([...at(a - n, b - n), ...at(a + n, b - n), ...at(a + n, b + n), ...at(a - n, b + n)].map((v) => v + FT * 0.6)).fill({ color: shadeOf(tint(0xa39a8a)), alpha: 0.25 });
      g.poly([...at(a - n, b - n), ...at(a + n, b - n), ...at(a + n, b + n), ...at(a - n, b + n)]).fill(STONE[1]).stroke({ width: 2.2, color: INK, join: 'round' });
      const m = n * 0.62;
      const [dx, dy] = [LX * n * 0.18, LY * n * 0.18];
      g.poly([...at(a - m, b - m), ...at(a + m, b - m), ...at(a + m, b + m), ...at(a - m, b + m)].map((v, i) => v + (i % 2 ? dy : dx))).fill(STONE[2]);
    }
  }
}

/**
 * An old stone bridge, mossy and worn: rough flags of mixed sizes across the deck, some sunken or
 * cracked, grass in the joints; a parapet of rubble coping, its joints uneven, a stone gone here
 * and there, moss and a little ivy on it; rounded cutwaters with a wet band at the waterline;
 * worn stumps of newels (one may be gone). Refuges only on a wide deck.
 */
function oldStoneBridge(g: Graphics, f: Frame, rnd: () => number) {
  const STONE = STONE_BASE.map(tint);
  const at = (a: number, b: number) => atF(f, a, b);
  const { len, wid } = f;
  const S = OLD_STONE.map(tint);
  const tp = FT * 2;
  const land = Math.min(FT * 12, len * 0.2);
  const nPiers = Math.max(0, Math.round((len - 2 * land) / (FT * 26)));
  const piers = Array.from({ length: nPiers }, (_, k) => land + ((len - 2 * land) * (k + 1)) / (nPiers + 1) + (rnd() - 0.5) * FT * 2);
  const [hw, bay, pr] = [FT * 3.4, wid > FT * 15 ? FT * 2 : 0, FT * 7];
  // One side's outer edge (refuges and all): `out` -1 for the b = 0 side, 1 for the b = wid side.
  const edge = (out: number, inset: number): number[] => {
    const b = (d: number) => (out < 0 ? -d + inset : wid + d - inset);
    const pts: number[] = [...at(0, b(0))];
    if (bay > 0) {
      for (const c of piers) pts.push(...at(c - hw * 1.2, b(0)), ...at(c - hw * 0.6 + inset * 0.5, b(bay)), ...at(c + hw * 0.6 - inset * 0.5, b(bay)), ...at(c + hw * 1.2, b(0)));
    }
    pts.push(...at(len, b(0)));
    return pts;
  };
  const backwards = (pts: number[]) => pts.flatMap((_, i) => (i % 2 ? [] : [pts[pts.length - 2 - i], pts[pts.length - 1 - i]]));
  const deck = [...edge(-1, 0), ...backwards(edge(1, 0))];
  // A rounded cutwater as two halves either side of its nose, out from one side.
  const cutwater = (c: number, out: number): number[][] => {
    const b = (d: number) => (out < 0 ? -d : wid + d);
    const arc = (t0: number, t1: number) => {
      const pts: number[] = [];
      for (let k = 0; k <= 4; k++) {
        const t = t0 + ((t1 - t0) * k) / 4;
        pts.push(...at(c - hw * Math.cos(t), b(pr * 0.3 + pr * 0.62 * Math.sin(t))));
      }
      return pts;
    };
    return [
      [...at(c - hw, b(-FT)), ...arc(0, Math.PI / 2), ...at(c, b(-FT))],
      [...at(c, b(-FT)), ...arc(Math.PI / 2, Math.PI), ...at(c + hw, b(-FT))],
    ];
  };
  const sh = FT * 3;
  castShadow(g, deck.map((v) => v + sh), 0.25);
  for (const c of piers) for (const out of [-1, 1]) for (const p of cutwater(c, out)) castShadow(g, p.map((v) => v + sh), 0.25);
  // Cutwaters: a wake, two worn facets lit or shaded, a wet dark band and moss at the waterline.
  for (const c of piers) {
    for (const out of [-1, 1]) {
      const b = (d: number) => (out < 0 ? -d : wid + d);
      g.moveTo(...at(c - hw - FT * 0.6, b(pr * 0.3))).quadraticCurveTo(...at(c, b(pr * 1.35)), ...at(c + hw + FT * 0.6, b(pr * 0.3)));
      g.stroke({ width: 1.6, color: 0xe6f4ec, alpha: 0.35 });
      const [left, right] = cutwater(c, out);
      for (const [p, na] of [[left, -1], [right, 1]] as const) {
        const lit = facing(f, na * pr * 0.7, out * hw);
        g.poly(p).fill(lit > 0.3 ? S[2] : lit > -0.3 ? S[1] : mix(S[0], PLUM, 0.2));
      }
      // The rim: the left half's arc, then the right's (each without its base corners).
      const rim = [...left.slice(2, left.length - 2), ...right.slice(4, right.length - 2)];
      g.poly(rim, false).stroke({ width: FT * 0.7, color: 0x2f3a33, alpha: 0.45, join: 'round' });
      g.poly([...left.slice(0, left.length - 2), ...right.slice(4)]).stroke({ width: 2.2, color: INK, join: 'round' });
      if (rnd() < 0.5) {
        const t = 0.6 + rnd() * 1.9;
        const [mx, my] = at(c - hw * Math.cos(t) * 0.75, b(pr * 0.3 + pr * 0.45 * Math.sin(t)));
        const r = FT * (0.35 + rnd() * 0.25);
        g.ellipse(mx, my, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.9 });
      }
    }
  }
  // The deck: rough flags of mixed sizes, a few sunken or cracked, grass in the joints.
  g.poly(deck).fill(tint(0x8f8673));
  const joints: number[][] = [];
  const tufts: [number, number][] = [];
  for (let a = 0; a < len; ) {
    const a1 = Math.min(len, a + FT * (2.4 + rnd() * 1.6));
    joints.push([...at(a1, 0), ...at(a1, wid)]);
    for (let b = -rnd() * FT * 3; b < wid; ) {
      const b1 = b + FT * (2.6 + rnd() * 2.4);
      if (b1 < wid && b1 > 0) joints.push([...at(a + FT * 0.05, b1), ...at(a1 - FT * 0.05, b1)]);
      const r = rnd();
      const [c0, c1] = [Math.max(0, b) + FT * 0.2, Math.min(wid, b1) - FT * 0.2];
      if (r < 0.3 && c1 > c0) {
        g.poly([...at(a + FT * 0.2, c0), ...at(a1 - FT * 0.2, c0), ...at(a1 - FT * 0.2, c1), ...at(a + FT * 0.2, c1)]).fill(r < 0.12 ? tint(0x9e957f) : r < 0.24 ? tint(0x857c69) : tint(0x7a7160));
      }
      // A crack across a flag now and then.
      if (r > 0.86 && c1 > c0) joints.push([...at(a + (a1 - a) * 0.2, c0 + (c1 - c0) * rnd()), ...at(a + (a1 - a) * 0.55, c0 + (c1 - c0) * rnd()), ...at(a + (a1 - a) * 0.8, c0 + (c1 - c0) * rnd())]);
      if (rnd() < 0.12 && b1 > 0 && b1 < wid) tufts.push(at(a + (a1 - a) * (0.2 + rnd() * 0.6), b1));
      b = b1;
    }
    a = a1;
  }
  for (const j of joints) g.poly(j, false);
  g.stroke({ width: 1.4, color: tint(0x625a4b), alpha: 0.85, join: 'round' });
  if (wid > FT * 9) {
    for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at(0, wid / 2 + d)).lineTo(...at(len, wid / 2 + d));
    g.stroke({ width: FT * 1.2, color: PLUM, alpha: 0.12 });
  }
  for (const [x, y] of tufts) {
    for (let k = 0; k < 3; k++) {
      const t = -Math.PI / 2 + (k - 1) * 0.6 + (rnd() - 0.5) * 0.3;
      g.moveTo(x, y).lineTo(x + Math.cos(t) * FT * 0.7, y + Math.sin(t) * FT * 0.7);
    }
  }
  g.stroke({ width: 2, color: MOSS[1], cap: 'round' });
  // Parapets: their shadow on the deck, then one band of rubble coping (ink, stone, a sunlit
  // edge), its stones' joints at uneven spacing; here and there a stone gone from the top.
  const rails = [edge(-1, tp * 0.5), edge(1, tp * 0.5)];
  for (const p of rails) g.poly(p.map((v) => v + FT * 0.8), false);
  g.stroke({ width: tp, color: shadeOf(tint(0x8f8673)), alpha: 0.25, join: 'round' });
  for (const p of rails) g.poly(p, false);
  g.stroke({ width: tp + 4, color: INK, join: 'round' });
  for (const p of rails) g.poly(p, false);
  g.stroke({ width: tp, color: S[1], join: 'round' });
  for (const p of rails) g.poly(p.map((v, i) => v + (i % 2 ? LY : LX) * tp * 0.12), false);
  g.stroke({ width: tp * 0.5, color: S[2], join: 'round' });
  const gaps: number[][] = [];
  const mossy: [number, number, number][] = [];
  const ivy: [number, number, number, number][] = [];
  rails.forEach((p, side) => {
    // The outward normal, away from the deck.
    const [nx, ny] = side === 0 ? [-f.vx, -f.vy] : [f.vx, f.vy];
    for (let i = 0; i + 3 < p.length; i += 2) {
      const [x0, y0] = [p[i], p[i + 1]];
      const l = Math.hypot(p[i + 2] - x0, p[i + 3] - y0) || 1;
      const [ux, uy] = [(p[i + 2] - x0) / l, (p[i + 3] - y0) / l];
      const half = tp * 0.5;
      const q = (a: number, d: number): [number, number] => [x0 + ux * a + nx * d, y0 + uy * a + ny * d];
      for (let s = FT * (0.6 + rnd() * 1.5); s < l - FT * 0.6; s += FT * (1.3 + rnd() * 1.7)) {
        // A joint across the coping, a little skewed.
        const k = (rnd() - 0.5) * FT * 0.35;
        g.moveTo(...q(s + k, -half)).lineTo(...q(s - k, half));
        if (rnd() < 0.035 && s > FT * 3 && s < l - FT * 4) {
          const w = FT * (1.2 + rnd() * 0.8);
          gaps.push([...q(s, -half * 0.55), ...q(s + w, -half * 0.55), ...q(s + w, half * 0.75), ...q(s, half * 0.75)]);
        }
        if (rnd() < 0.08) mossy.push([...q(s + FT * 0.6, (rnd() - 0.3) * half * 0.8), FT * (0.25 + rnd() * 0.2)]);
        if (rnd() < 0.035) ivy.push([...q(s + FT * 0.5, half), nx, ny]);
      }
    }
  });
  g.stroke({ width: 1.4, color: S[0] });
  // A stone gone from the coping: the rough core shows, a step down.
  for (const b of gaps) g.poly(b).fill(mix(S[0], PLUM, 0.2)).stroke({ width: 1.4, color: INK, join: 'round' });
  for (const [x, y, r] of mossy) {
    g.ellipse(x, y, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.9 });
    g.ellipse(x - r * 0.3, y - r * 0.25, r * 0.6, r * 0.45).fill({ color: MOSS[1], alpha: 0.9 });
  }
  // Ivy trailing down the outer face: a few small dark leaves hugging the stone.
  for (const [x, y, nx, ny] of ivy) {
    for (let k = 0; k < 4; k++) {
      const d = FT * (0.1 + rnd() * 0.8);
      const sx = (rnd() - 0.5) * FT * 1.4;
      const [lx, ly] = [x + nx * d - ny * sx, y + ny * d + nx * sx];
      const r = FT * (0.28 + rnd() * 0.14);
      g.circle(lx, ly, r).fill(IVY[0]).stroke({ width: 1, color: INK, alpha: 0.7 });
      g.circle(lx + LX * r * 0.3, ly + LY * r * 0.3, r * 0.45).fill(IVY[1]);
    }
  }
  // Worn stumps of newels at the corners; one may be gone.
  const lost = rnd() < 0.3 ? Math.floor(rnd() * 4) : -1;
  let corner = 0;
  for (const a of [tp * 0.6, len - tp * 0.6]) {
    for (const b of [tp * 0.5, wid - tp * 0.5]) {
      if (corner++ === lost) continue;
      const n = tp * 0.85;
      const pts: number[] = [];
      for (let m = 0; m < 8; m++) {
        const t = (m / 8) * Math.PI * 2 + Math.PI / 8;
        const r = n * (1.12 + (rnd() - 0.5) * 0.2);
        pts.push(...at(a + Math.cos(t) * r, b + Math.sin(t) * r));
      }
      g.poly(pts.map((v) => v + FT * 0.6)).fill({ color: shadeOf(tint(0x8f8673)), alpha: 0.25 });
      g.poly(pts).fill(S[1]).stroke({ width: 2.2, color: INK, join: 'round' });
      const [cx, cy] = at(a, b);
      g.circle(cx + LX * n * 0.2, cy + LY * n * 0.2, n * 0.55).fill(S[2]);
      if (rnd() < 0.5) g.circle(...at(a + n * 0.4, b + n * 0.3), n * 0.35).fill(MOSS[1]);
    }
  }
}

/**
 * The 5-ft grid over a bridge or pier deck (the ground's grid is under it): lines on the square
 * boundaries (chunk px are a whole number of squares from the chunk's corner), clipped to the
 * deck, one screen pixel wide like the ground's.
 */
export function deckGrid(g: Graphics, s: Shape) {
  if (s.kind === 2 && Math.round(s.size / PX) === 4) {
    const { m, face, back } = quayFrame(s);
    for (let k = 0; k + 1 < m; k++) deckGrid(g, { ...s, size: 0, pts: [...face[k], ...face[k + 1], ...back[k + 1], ...back[k]] });
    return;
  }
  let q: number[];
  if (s.kind === 2 && s.pts.length >= 8) q = s.pts.slice(0, 8);
  else if (s.kind === 3 || s.kind === 18) {
    const [x0, y0, x1, y1] = s.pts;
    const len = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [vx, vy] = [(-(y1 - y0) / len) * s.size * 0.5, ((x1 - x0) / len) * s.size * 0.5];
    q = [x0 - vx, y0 - vy, x1 - vx, y1 - vy, x1 + vx, y1 + vy, x0 + vx, y0 + vy];
  } else return;
  // Where the line `axis` = `at` crosses the deck's outline, as the span along the other axis.
  const span = (axis: number, at: number): [number, number] | null => {
    let [lo, hi] = [Infinity, -Infinity];
    for (let i = 0; i < 4; i++) {
      const [a, b] = [i * 2, ((i + 1) % 4) * 2];
      const [p, r] = [q[a + axis], q[b + axis]];
      if ((p - at) * (r - at) > 0 || p === r) continue;
      const o = q[a + 1 - axis] + ((at - p) / (r - p)) * (q[b + 1 - axis] - q[a + 1 - axis]);
      [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
    }
    return hi > lo ? [lo, hi] : null;
  };
  for (const axis of [0, 1]) {
    const vals = [q[axis], q[axis + 2], q[axis + 4], q[axis + 6]];
    for (let k = Math.ceil(Math.min(...vals) / PX); k * PX <= Math.max(...vals); k++) {
      const sp = span(axis, k * PX);
      if (!sp) continue;
      if (axis === 0) g.moveTo(k * PX, sp[0]).lineTo(k * PX, sp[1]);
      else g.moveTo(sp[0], k * PX).lineTo(sp[1], k * PX);
    }
  }
  g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
}
