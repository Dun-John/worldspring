// A square's fountain, speaker's dais, tree planter, market cross and steppe mound (`drawDais` kinds 0 to 4), drawn to sit in the town's own ground: the
// stone tinted by the culture's tone, the water by the ground about (green in the wet, low in the desert, frozen
// on snow), a soft shadow of the ground's own colour, a kerb or worn ring into the paving, and a little dressing
// by ground. Every size from a village trough (3 ft) to a city fountain (14 ft) comes from the radius it is given.
import type { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { INK, LINE, OUT } from '../pen';
import { rngAt } from '../bridges';
import { lit } from '../atlas/plants';
import { polyR } from '../atlas/street';

/** What a plaza feature stands on. */
export interface PlazaGround {
  /** The town's stone tint (`stoneTone`: r, g, b factors; 1, 1, 1 plain grey). */
  tone: ArrayLike<number>;
  /** The surface at its foot (`ground.frag` ids: paving or ground). */
  foot: number;
  /** The natural ground about the square (the biome's: grass, forest floor, sand, snow...), -1 none found. */
  nature: number;
  /** The chunk's corner in world px (for the shape's hash: the same place, the same shape). */
  wx: number;
  wy: number;
}

/** The ground's colours by surface id, as `ground.frag` `SURF` (cobble before the stone tint). */
const SURF = [
  [0.4, 0.54, 0.25], [0.31, 0.35, 0.19], [0.64, 0.6, 0.36], [0.85, 0.76, 0.55], [0.91, 0.93, 0.96], [0.5, 0.49, 0.46],
  [0.33, 0.29, 0.21], [0.27, 0.25, 0.24], [0.89, 0.87, 0.83], [0.52, 0.42, 0.29], [0.3, 0.5, 0.52], [0.15, 0.29, 0.38],
  [1.0, 0.45, 0.1], [0.77, 0.87, 0.93], [0.66, 0.62, 0.55], [0.6, 0.48, 0.33], [0.55, 0.4, 0.25], [0.58, 0.5, 0.38],
  [0.66, 0.62, 0.36], [0.62, 0.59, 0.53], [0.76, 0.71, 0.6], [0.53, 0.53, 0.37],
];
/** Surfaces that are the land's own (not paving, water or building). */
export const NATURAL = new Set([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 13, 21]);
const PAVED = new Set([14, 16, 19, 20]);

/** Cobbles' tints by culture (`ground.frag` `uStone`, times the cobble grey; `Layout::culture`):
 * north cool blue-grey granite, west warm grey-buff, south warm honey limestone, dry pale ochre
 * sandstone, steppe khaki fieldstone. Each within about 8% of the grey's value, saturation <= 0.35. */
export const STONE_TONES = [
  [0.96, 0.97, 1.04],
  [1.01, 1.0, 0.97],
  [1.07, 1.04, 0.89],
  [1.15, 1.08, 0.95],
  [0.99, 1.0, 0.89],
];
/** A chunk's `stone` word's tint: its culture's, varied a little by city; none (roads out of
 * town) the plain grey. */
export function stoneTone(stone: number): Float32Array {
  if (!stone) return new Float32Array([1, 1, 1]);
  const t = STONE_TONES[((stone >>> 24) + 4) % 5];
  const h = Math.imul(stone & 0xffffff, 0x9e3779b1) >>> 0;
  // (Up to 3% in value and 2% in warmth either way.)
  const v = 1 + 0.06 * ((h >>> 24) / 255 - 0.5);
  const warm = 0.04 * (((h >>> 16) & 255) / 255 - 0.5);
  return new Float32Array([t[0] * v * (1 + warm), t[1] * v, t[2] * v * (1 - warm)]);
}

const hex = (r: number, g: number, b: number) => {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v * 255)));
  return (c(r) << 16) | (c(g) << 8) | c(b);
};
const rgb = (c: number) => [((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255];
const mul = (c: number, f: number) => {
  const [r, g, b] = rgb(c);
  return hex(r * f, g * f, b * f);
};
const mix = (a: number, b: number, t: number) => {
  const [x, y] = [rgb(a), rgb(b)];
  return hex(x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t, x[2] + (y[2] - x[2]) * t);
};
/** A surface's colour (cobble tinted by the town's stone). */
export function groundColour(id: number, tone: ArrayLike<number> = [1, 1, 1]): number {
  const s = SURF[id] ?? SURF[14];
  return id === 14 ? hex(s[0] * tone[0], s[1] * tone[1], s[2] * tone[2]) : hex(s[0], s[1], s[2]);
}

/** The stone's ramp from the town's tone: dark (toward plum), mid, light, sunlit (toward cream), joint. */
function stoneRamp(tone: ArrayLike<number>) {
  const [r, g, b] = rgb(0xaba395);
  const mid = hex(r * tone[0], g * tone[1], b * tone[2]);
  return {
    dark: mix(mul(mid, 0.68), 0x4a2f3a, 0.14),
    mid,
    light: mix(mul(mid, 1.1), 0xfff1cf, 0.12),
    sun: mix(mul(mid, 1.18), 0xfff1cf, 0.28),
    joint: mix(mul(mid, 0.7), 0x4a2f3a, 0.1),
  };
}
type Ramp = ReturnType<typeof stoneRamp>;

type Rf = (a: number) => number;
/** `n` points round (cx, cy) at radius `R(a)`. */
function around(cx: number, cy: number, R: Rf, n = 64, dx = 0, dy = 0): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    pts.push(cx + dx + Math.cos(a) * R(a), cy + dy + Math.sin(a) * R(a));
  }
  return pts;
}
/** The band between `Ro` and `Ri` from angle `a0` to `a1`. */
function arcBand(cx: number, cy: number, Ro: Rf, Ri: Rf, a0: number, a1: number, n = 16): number[] {
  const pts: number[] = [];
  for (let i = 0; i <= n; i++) {
    const a = a0 + ((a1 - a0) * i) / n;
    pts.push(cx + Math.cos(a) * Ro(a), cy + Math.sin(a) * Ro(a));
  }
  for (let i = n; i >= 0; i--) {
    const a = a0 + ((a1 - a0) * i) / n;
    pts.push(cx + Math.cos(a) * Ri(a), cy + Math.sin(a) * Ri(a));
  }
  return pts;
}
/** A lumpy patch of radius `s` (ground dressing, moss, snow). */
function lump(x: number, y: number, sx: number, sy: number, rot: number, rnd: () => number, n = 8): number[] {
  const pts: number[] = [];
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const k = 0.75 + 0.4 * rnd();
    const [u, v] = [Math.cos(a) * sx * k, Math.sin(a) * sy * k];
    pts.push(x + u * c - v * s, y + u * s + v * c);
  }
  return pts;
}

/**
 * A ring of dressed stones between `Ro` and `Ri`, each block lit as it faces the light (light from the
 * north-west), a joint between blocks about `block` px long (jittered so they don't tick like a dial).
 */
function stoneBand(g: Graphics, cx: number, cy: number, Ro: Rf, Ri: Rf, st: Ramp, block: number, rnd: () => number, light = 0.22, ph = 0) {
  const mean = (Ro(0) + Ro(Math.PI / 2) + Ro(Math.PI) + Ro(Math.PI * 1.5)) / 4;
  const n = Math.max(6, Math.round((Math.PI * 2 * mean) / block));
  const cuts: number[] = [];
  for (let i = 0; i < n; i++) cuts.push(ph + ((i + (rnd() - 0.5) * 0.35) / n) * Math.PI * 2);
  for (let i = 0; i < n; i++) {
    const [a0, a1] = [cuts[i], i + 1 < n ? cuts[i + 1] : cuts[0] + Math.PI * 2];
    const f = lit((a0 + a1) / 2);
    const tone = f > 0.35 ? mix(st.mid, st.light, light * 4 * (f - 0.35)) : f < -0.35 ? mix(st.mid, st.dark, light * 1.6 * (-f - 0.35)) : st.mid;
    g.poly(arcBand(cx, cy, Ro, Ri, a0, a1, 6)).fill(mix(tone, st.mid, (rnd() - 0.5) * 0.3));
  }
  for (const a of cuts) g.moveTo(cx + Math.cos(a) * Ri(a), cy + Math.sin(a) * Ri(a)).lineTo(cx + Math.cos(a) * Ro(a), cy + Math.sin(a) * Ro(a));
  g.stroke({ width: LINE * 0.8, color: st.joint, alpha: 0.9 });
}

/** The soft contact shadow of the outline `R`: the ground darkened, a little to the south-east. */
function contactShadow(g: Graphics, cx: number, cy: number, R: Rf, foot: number, paved: boolean) {
  // One short shape thrown to the south-east, at the outline's own radius so it never shows on the lit sides.
  const o = PX * 0.12;
  g.poly(around(cx, cy, R, 64, o, o)).fill({ color: mul(foot, 0.6), alpha: 0.16 });
}

/** Where it meets the ground: a kerb of the paving's own stone (radial joints, a few stray stones) on a paved
 * square, else a worn ring of trodden earth with a few tufts where feet don't reach. */
function apron(g: Graphics, cx: number, cy: number, R: Rf, gr: PlazaGround, rnd: () => number) {
  const foot = groundColour(gr.foot, gr.tone);
  if (PAVED.has(gr.foot)) {
    const w = PX * 0.3;
    const outer: Rf = (a) => R(a) + w;
    g.poly(around(cx, cy, outer)).fill({ color: mul(foot, 0.9), alpha: 0.9 });
    g.poly(around(cx, cy, outer)).stroke({ width: LINE * 0.8, color: mul(foot, 0.66), alpha: 0.8 });
    const mean = R(0.3) + w;
    const n = Math.max(8, Math.round((Math.PI * 2 * mean) / (PX * 0.6)));
    for (let i = 0; i < n; i++) {
      const a = ((i + (rnd() - 0.5) * 0.3) / n) * Math.PI * 2;
      g.moveTo(cx + Math.cos(a) * R(a), cy + Math.sin(a) * R(a)).lineTo(cx + Math.cos(a) * outer(a), cy + Math.sin(a) * outer(a));
    }
    g.stroke({ width: LINE * 0.8, color: mul(foot, 0.66), alpha: 0.8 });
    // A few stones come loose beyond the kerb.
    const stray = 3 + Math.floor(rnd() * 3);
    for (let i = 0; i < stray; i++) {
      const a = rnd() * Math.PI * 2;
      const d = outer(a) + PX * (0.08 + rnd() * 0.25);
      const s = PX * (0.07 + rnd() * 0.06);
      g.poly(lump(cx + Math.cos(a) * d, cy + Math.sin(a) * d, s * 1.3, s, a + rnd(), rnd, 6)).fill(mul(foot, 1.04 + rnd() * 0.06)).stroke({ width: LINE * 0.7, color: mul(foot, 0.62) });
    }
    return;
  }
  // Trodden earth (on snow, packed grey snow; on sand, darker sand), wobbling out from the foot.
  const [p1, p2] = [rnd() * 6.28, rnd() * 6.28];
  const worn: Rf = (a) => R(a) + PX * (0.32 + 0.1 * Math.sin(3 * a + p1) + 0.07 * Math.cos(5 * a + p2));
  const snowy = gr.foot === 4 || gr.foot === 13;
  const earth = snowy ? 0xc2ccd8 : gr.foot === 3 ? mul(foot, 0.86) : [1, 6, 7].includes(gr.foot) ? mix(foot, 0x856b4a, 0.35) : 0x856b4a;
  g.poly(around(cx, cy, worn)).fill({ color: earth, alpha: snowy ? 0.55 : 0.42 });
}

type Dress = 'moss' | 'sand' | 'snow' | 'ash' | null;
/** What dressing the ground about calls for. */
function dressOf(gr: PlazaGround): Dress {
  // Paving with no clear biome about it gets none (a dry square in a garden town is not a swamp).
  if (PAVED.has(gr.foot) && gr.nature < 0) return null;
  const n = gr.nature >= 0 ? gr.nature : gr.foot;
  if (n === 4 || n === 13 || gr.foot === 4) return 'snow';
  if (n === 3 || gr.foot === 3) return 'sand';
  if (n === 7) return 'ash';
  if ((n === 1 || n === 6) && !PAVED.has(gr.foot)) return 'moss';
  return null;
}

/** Sparse dressing at the foot and on the north side of the stone: moss in the wet, sand drifted against
 * the north-west foot, a snow cap on the north side, ash dust all round. */
function dress(g: Graphics, cx: number, cy: number, R: Rf, top: Rf, kind: Dress, gr: PlazaGround, rnd: () => number) {
  if (!kind) return;
  const r = R(0);
  const n = Math.max(3, Math.round(r / (PX * 0.45)));
  if (kind === 'moss') {
    for (let i = 0; i < n; i++) {
      const a = Math.PI * (1.0 + rnd() * 0.9);
      const d = (i % 2 ? top(a) : R(a)) - PX * 0.04;
      const s = PX * (0.09 + rnd() * 0.1);
      g.poly(lump(cx + Math.cos(a) * d, cy + Math.sin(a) * d, s * 1.6, s, a + Math.PI / 2, rnd)).fill({ color: i % 3 ? 0x5f7a3e : 0x6f9a3e, alpha: 0.6 });
    }
    return;
  }
  if (kind === 'sand') {
    const sand = groundColour(3);
    for (let i = 0; i < Math.max(2, n >> 1); i++) {
      const a = Math.PI * (0.95 + 0.6 * rnd());
      const d = R(a) + PX * 0.02;
      const s = PX * (0.16 + rnd() * 0.14) * Math.min(1.6, 0.6 + r / (PX * 2));
      g.poly(lump(cx + Math.cos(a) * d, cy + Math.sin(a) * d, s * 2.2, s, a + Math.PI / 2, rnd, 10)).fill({ color: sand, alpha: 0.92 });
      g.poly(lump(cx + Math.cos(a) * (d + s * 0.2), cy + Math.sin(a) * (d + s * 0.2), s * 1.4, s * 0.5, a + Math.PI / 2, rnd)).fill({ color: mix(sand, 0xfff0c8, 0.4), alpha: 0.7 });
    }
    return;
  }
  if (kind === 'snow') {
    // Drifts lying along the north side, a lavender shade under each.
    for (let i = 0; i < Math.max(2, n - 1); i++) {
      const a = Math.PI * (1.1 + (0.75 * (i + rnd() * 0.6)) / Math.max(2, n - 1));
      const d = (i % 2 ? top(a) : R(a)) - PX * 0.06;
      const s = PX * (0.09 + rnd() * 0.06);
      const l = s * (2.6 + rnd() * 1.6);
      const [x, y] = [cx + Math.cos(a) * d, cy + Math.sin(a) * d];
      g.poly(lump(x + PX * 0.03, y + PX * 0.04, l, s, a + Math.PI / 2, rnd, 10)).fill({ color: 0xb9c6dc, alpha: 0.6 });
      g.poly(lump(x, y, l, s, a + Math.PI / 2, rnd, 10)).fill({ color: 0xe8edf3, alpha: 0.92 });
    }
    return;
  }
  // Ash: a grey dust here and there.
  for (let i = 0; i < n + 3; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R(a) * (0.4 + rnd() * 0.75);
    const s = PX * (0.1 + rnd() * 0.14);
    g.poly(lump(cx + Math.cos(a) * d, cy + Math.sin(a) * d, s * 1.5, s, rnd() * 3, rnd)).fill({ color: 0x5a5458, alpha: 0.28 });
  }
}

/** The outline of the given variant: 0 round (a little hand-made), 1 octagon, 2 quatrefoil (fountains) or hexagon
 * (daises, a side facing the steps), 3 square (a trough). */
function outline(R0: number, v: number, a: number, dais: boolean, rnd: () => number): Rf {
  const p = rnd() * 6.28;
  if (v === 1) return polyR(R0, 8, a + Math.PI / 8);
  if (v === 2) return dais ? polyR(R0, 6, a + Math.PI / 6) : (t) => R0 * (0.84 + 0.16 * Math.abs(Math.cos(2 * (t - a))));
  if (v === 3) return polyR(R0, 4, Math.PI / 4);
  return (t) => R0 * (1 + 0.012 * Math.sin(3 * t + p));
}
const scaled = (R: Rf, k: number, d = 0): Rf => (t) => R(t) * k + d;

/** Treads following the outline `R` on the approach side (angle `a`): `n` low steps each `depth` px. */
function treads(g: Graphics, cx: number, cy: number, R: Rf, a: number, n: number, depth: number, st: Ramp) {
  const r = R(a);
  for (let k = n; k >= 1; k--) {
    const span = Math.min(0.75, (PX * 0.95) / r + 0.04 * k);
    const Ro: Rf = (t) => R(t) + depth * k;
    const pts = arcBand(cx, cy, Ro, (t) => R(t) - PX * 0.05, a - span, a + span, 14);
    g.poly(pts).fill(k === 1 ? st.mid : mix(st.mid, st.dark, 0.18)).stroke({ width: LINE, color: INK, join: 'round' });
    // Its front: lit if it faces the light, else a dark riser.
    const f = lit(a);
    g.poly(arcBand(cx, cy, Ro, (t) => Ro(t) - PX * 0.06, a - span, a + span, 14)).fill({ color: f > 0 ? st.sun : st.dark, alpha: 0.6 });
  }
}

/** Water's tones by the ground: grey-blue; green-tinged in the wet; greyer under ash. */
function waterTones(gr: PlazaGround) {
  const n = gr.nature >= 0 ? gr.nature : gr.foot;
  if (!PAVED.has(gr.foot) && (n === 1 || n === 6)) return { deep: 0x46695c, mid: 0x56806e, light: 0x82a28a, foam: 0xd8e4d2 };
  const mid = mix(0x5c8a97, groundColour(gr.foot, gr.tone), n === 7 ? 0.25 : 0.12);
  return { deep: mix(mid, 0x2a4450, 0.35), mid, light: mix(mid, 0xe6f4ec, 0.35), foam: 0xe2ece8 };
}

/** A fountain on a square (`r`: its outer radius, px; `a`: toward its steps). */
export function drawPlazaFountain(g: Graphics, cx: number, cy: number, r: number, a: number, gr: PlazaGround) {
  const rnd = rngAt(gr.wx + cx, gr.wy + cy);
  const st = stoneRamp(gr.tone);
  const rs = r / PX;
  const small = rs < 0.95;
  const big = rs >= 1.9;
  const v = small ? (rnd() < 0.5 ? 0 : 3) : Math.floor(rnd() * 3);
  const dressing = dressOf(gr);
  const dry = dressing === 'sand';
  const frozen = dressing === 'snow';
  const W = waterTones(gr);
  const foot = groundColour(gr.foot, gr.tone);
  const paved = PAVED.has(gr.foot);
  const kerb = paved ? PX * 0.3 : 0;
  const nT = small ? 0 : big ? 2 : 1;
  const depth = PX * 0.16;
  const plinthW = small ? 0 : PX * Math.min(0.26, 0.16 + rs * 0.03);
  const rimW = PX * Math.min(0.3, Math.max(0.16, rs * 0.09));
  // `r` is the whole's outer reach: the treads (on one side) stand inside it, so the round is drawn that much smaller.
  const P = outline(r - kerb - nT * depth, v, a, false, rnd);
  const Rr = scaled(P, 1, -plinthW);
  const Ri = scaled(P, 1, -plinthW - rimW);
  apron(g, cx, cy, P, gr, rnd);
  contactShadow(g, cx, cy, P, foot, PAVED.has(gr.foot));
  if (!small) treads(g, cx, cy, P, a, nT, depth, st);
  // The low plinth all round, then the basin's rim on it (its own short shadow on the plinth).
  if (!small) {
    stoneBand(g, cx, cy, P, Rr, { ...st, mid: mix(st.mid, st.dark, 0.12) }, PX * 1.1, rnd, 0.18, 0.4);
    g.poly(around(cx, cy, P)).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(around(cx, cy, Rr, 64, PX * 0.06, PX * 0.07)).fill({ color: st.dark, alpha: 0.35 });
  }
  stoneBand(g, cx, cy, Rr, Ri, st, PX * (small ? 0.55 : 0.85), rnd, 0.25, v === 1 ? a + Math.PI / 8 : 0.2);
  g.poly(around(cx, cy, Rr)).stroke({ width: small ? OUT : LINE, color: INK, join: 'round' });
  // The water (or ice, or a low pool on a dry basin floor).
  const inner = around(cx, cy, Ri);
  const ri = Ri(0);
  if (dry) {
    g.poly(inner).fill(mix(st.dark, groundColour(3), 0.45));
    g.poly(arcBand(cx, cy, scaled(Ri, 0.97), scaled(Ri, 0.52), -0.4, Math.PI * 0.9, 18)).fill(W.mid);
    g.poly(arcBand(cx, cy, scaled(Ri, 0.97), scaled(Ri, 0.8), -0.25, Math.PI * 0.75, 14)).fill({ color: W.deep, alpha: 0.5 });
    g.poly(lump(cx - ri * 0.42, cy - ri * 0.36, ri * 0.38, ri * 0.2, -0.8, rnd, 10)).fill({ color: groundColour(3), alpha: 0.9 });
  } else if (frozen) {
    g.poly(inner).fill(0xc8dde8);
    g.poly(arcBand(cx, cy, Ri, (t) => Ri(t) - ri * 0.16 * Math.max(0, lit(t)), Math.PI * 0.6, Math.PI * 1.9, 20)).fill({ color: 0x8ab8da, alpha: 0.45 });
    // Cracks out from the middle, jagged.
    for (let k = 0; k < 3; k++) {
      let t = rnd() * Math.PI * 2;
      let d = ri * 0.3;
      g.moveTo(cx + Math.cos(t) * d, cy + Math.sin(t) * d);
      while (d < ri * 0.9) {
        d += ri * (0.15 + rnd() * 0.12);
        t += (rnd() - 0.5) * 0.4;
        g.lineTo(cx + Math.cos(t) * Math.min(d, ri * 0.92), cy + Math.sin(t) * Math.min(d, ri * 0.92));
      }
    }
    g.stroke({ width: LINE * 0.8, color: 0x5a8fb8, alpha: 0.75 });
  } else {
    g.poly(inner).fill(W.mid);
    // A deeper, lopsided patch off the middle to the south-east (not a ring).
    g.poly(lump(cx + ri * 0.28, cy + ri * 0.3, ri * 0.4, ri * 0.3, 0.8, rnd, 9)).fill({ color: W.deep, alpha: 0.28 });
    // The rim's shadow on the water, a crescent inside its north-west side.
    g.poly(arcBand(cx, cy, Ri, (t) => Ri(t) - ri * 0.14 * Math.max(0, lit(t) + 0.2), Math.PI * 0.55, Math.PI * 1.95, 24)).fill({ color: W.deep, alpha: 0.6 });
  }
  g.poly(inner).stroke({ width: LINE, color: INK, join: 'round' });
  if (small) {
    // A trough's spout on its north rim, the stream falling in and ripples where it lands.
    const sa = -Math.PI / 2 + (rnd() - 0.5) * 0.6;
    const [ux, uy] = [Math.cos(sa), Math.sin(sa)];
    const [px, py] = [cx + ux * (Ri(sa) + rimW * 0.5), cy + uy * (Ri(sa) + rimW * 0.5)];
    const sw = PX * 0.09;
    g.poly([px - uy * sw - ux * rimW * 0.8, py + ux * sw - uy * rimW * 0.8, px + uy * sw - ux * rimW * 0.8, py - ux * sw - uy * rimW * 0.8,
      px + uy * sw, py - ux * sw, px - uy * sw, py + ux * sw]).fill(st.light).stroke({ width: LINE * 0.8, color: INK });
    if (!dry && !frozen) {
      const [lx, ly] = [px - ux * rimW * 1.6, py - uy * rimW * 1.6];
      g.moveTo(px - ux * rimW * 0.8, py - uy * rimW * 0.8).lineTo(lx, ly).stroke({ width: LINE * 1.4, color: W.foam, alpha: 0.85, cap: 'round' });
      for (const [k, al] of [[0.1, 0.8], [0.17, 0.5]]) {
        const rr = PX * k;
        g.moveTo(lx + Math.cos(0.4) * rr, ly + Math.sin(0.4) * rr).arc(lx, ly, rr, 0.4, 0.4 + Math.PI * 1.3);
        g.stroke({ width: LINE * 0.8, color: W.foam, alpha: al });
      }
    }
    dress(g, cx, cy, Rr, Ri, dressing, gr, rnd);
    return;
  }
  // The pedestal's bowl (a lower and an upper one on a city's), each with its short shadow on the water.
  const bowls = big ? [Math.min(PX * 0.95, r * 0.3), Math.min(PX * 0.95, r * 0.3) * 0.48] : [Math.max(PX * 0.32, Math.min(PX * 0.7, r * 0.27))];
  bowls.forEach((bR, i) => {
    const B: Rf = v === 1 ? polyR(bR, 8, a + Math.PI / 8) : (t) => bR * (1 + 0.015 * Math.sin(3 * t + i));
    const o = PX * (0.1 - i * 0.03);
    g.poly(around(cx, cy, B, 40, o, o)).fill({ color: dry || frozen ? st.dark : W.deep, alpha: 0.45 });
    if (!dry && !frozen) {
      // Drips falling off its lip, rings just round it.
      const n = Math.max(6, Math.round((bR / PX) * 9));
      for (let k = 0; k < n; k++) {
        const t = ((k + rnd() * 0.6) / n) * Math.PI * 2;
        const rr = bR * 1.13 + PX * 0.03;
        g.moveTo(cx + Math.cos(t) * rr, cy + Math.sin(t) * rr).arc(cx, cy, rr, t, t + (Math.PI * 0.8) / n);
      }
      g.stroke({ width: LINE * 0.9, color: W.foam, alpha: 0.7, cap: 'round' });
    }
    stoneBand(g, cx, cy, B, scaled(B, 0.74), { ...st, mid: st.light }, PX * 0.6, rnd, 0.3, 0.6);
    g.poly(around(cx, cy, B, 40)).stroke({ width: bR * 2 > PX * 0.5 ? OUT : LINE, color: INK, join: 'round' });
    const bin = around(cx, cy, scaled(B, 0.74), 40);
    g.poly(bin).fill(dry ? mix(st.dark, groundColour(3), 0.6) : frozen ? 0xbcd6e2 : W.mid).stroke({ width: LINE * 0.8, color: INK });
    if (!dry && !frozen) g.poly(arcBand(cx, cy, scaled(B, 0.74), scaled(B, 0.6), Math.PI * 0.7, Math.PI * 1.8, 12)).fill({ color: W.deep, alpha: 0.5 });
  });
  if (!dry && !frozen) {
    // Ripples out from the pedestal, broken and few.
    const b0 = bowls[0];
    for (let k = 0; k < 4; k++) {
      const t = rnd() * Math.PI * 2;
      const rr = b0 * (1.45 + rnd() * 0.5);
      if (rr > ri * 0.9) continue;
      g.moveTo(cx + Math.cos(t) * rr, cy + Math.sin(t) * rr).arc(cx, cy, rr, t, t + 0.5 + rnd() * 0.5);
    }
    g.stroke({ width: LINE * 0.9, color: W.light, alpha: 0.85, cap: 'round' });
    // The jet from above: spray thrown out unevenly, a soft froth where it rises.
    const top = bowls[bowls.length - 1];
    const jr = Math.min(top * 0.55, PX * 0.3);
    for (let k = 0; k < 4; k++) {
      const t = (k / 4) * Math.PI * 2 + rnd() * 1.2;
      const l = jr * (0.5 + rnd() * 0.6);
      g.moveTo(cx + Math.cos(t) * jr * 0.2, cy + Math.sin(t) * jr * 0.2).lineTo(cx + Math.cos(t) * l, cy + Math.sin(t) * l);
    }
    g.stroke({ width: LINE, color: W.foam, alpha: 0.85, cap: 'round' });
    g.poly(lump(cx + jr * 0.05, cy, jr * 0.5, jr * 0.36, rnd() * 3, rnd, 8)).fill({ color: W.foam, alpha: 0.55 });
  }
  dress(g, cx, cy, P, Rr, dressing, gr, rnd);
}

/** The x range of the convex outline `pts` along the row `y` (null if it misses). */
function chord(pts: number[], y: number): [number, number] | null {
  let [lo, hi] = [Infinity, -Infinity];
  const n = pts.length / 2;
  for (let i = 0; i < n; i++) {
    const [x0, y0, x1, y1] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % n) * 2], pts[((i + 1) % n) * 2 + 1]];
    if ((y0 - y) * (y1 - y) > 0 || y0 === y1) continue;
    const x = x0 + ((y - y0) / (y1 - y0)) * (x1 - x0);
    [lo, hi] = [Math.min(lo, x), Math.max(hi, x)];
  }
  return lo < hi ? [lo, hi] : null;
}

/** The y range of the convex outline `pts` along the column `x`. */
function chordY(pts: number[], x: number): [number, number] | null {
  const sw: number[] = [];
  for (let i = 0; i < pts.length; i += 2) sw.push(pts[i + 1], pts[i]);
  return chord(sw, x);
}

/** A speaker's dais on a square (`r`: its radius, px; `a`: toward its steps): block-built, its top in coursed flags
 * inside a ring of edge stones, lit along its north-west edge, its dark riser showing to the south-east. */
export function drawPlazaDais(g: Graphics, cx: number, cy: number, r: number, a: number, gr: PlazaGround) {
  const rnd = rngAt(gr.wx + cx, gr.wy + cy);
  const st = stoneRamp(gr.tone);
  const rs = r / PX;
  const v = Math.floor(rnd() * 3);
  const nT = rs >= 1.4 ? 2 : 1;
  const depth = PX * 0.2;
  const P = outline(r - (PAVED.has(gr.foot) ? PX * 0.3 : 0) - nT * depth, v, a, true, rnd);
  const riser = PX * Math.min(0.18, 0.1 + rs * 0.02);
  // The top: set in from the outline on the south-east by the riser's height.
  const T: Rf = (t) => P(t) - riser * (0.5 - 0.5 * lit(t));
  const foot = groundColour(gr.foot, gr.tone);
  apron(g, cx, cy, P, gr, rnd);
  contactShadow(g, cx, cy, P, foot, PAVED.has(gr.foot));
  treads(g, cx, cy, P, a, nT, depth, st);
  g.poly(around(cx, cy, P)).fill(st.dark).stroke({ width: OUT, color: INK, join: 'round' });
  const top = around(cx, cy, T);
  g.poly(top).fill(st.mid);
  // Edge stones round the top, the north-west ones lit.
  const edge = Math.min(PX * 0.34, r * 0.3);
  stoneBand(g, cx, cy, T, scaled(T, 1, -edge), st, PX * 1.0, rnd, 0.35, a);
  // Coursed flags in the middle: rows across, joints staggered between them.
  const field = around(cx, cy, scaled(T, 1, -edge), 48);
  const step = PX * (rs < 1.3 ? 0.9 : 1.1);
  const rows: number[] = [];
  for (let y = cy - r + edge + step * (0.55 + rnd() * 0.3); y < cy + r - edge - PX * 0.25; y += step) rows.push(y);
  for (const y of rows) {
    const c = chord(field, y);
    if (c) g.moveTo(c[0], y).lineTo(c[1], y);
  }
  for (let i = 0; i <= rows.length; i++) {
    const [y0, y1] = [i ? rows[i - 1] : cy - r, i < rows.length ? rows[i] : cy + r];
    const mid = (y0 + y1) / 2;
    const c = chord(field, mid);
    if (!c) continue;
    const len = PX * (1.1 + rnd() * 0.5);
    for (let x = c[0] + len * (0.4 + 0.5 * rnd()); x < c[1] - PX * 0.3; x += len * (0.8 + rnd() * 0.4)) {
      const span = chordY(field, x);
      if (!span) continue;
      const [ya, yb] = [Math.max(y0, span[0]), Math.min(y1, span[1])];
      if (yb - ya > PX * 0.2) g.moveTo(x, ya).lineTo(x, yb);
    }
  }
  g.stroke({ width: LINE * 0.8, color: st.joint, alpha: 0.85 });
  g.poly(field).stroke({ width: LINE * 0.8, color: st.joint, alpha: 0.9 });
  // A worn, paler patch where the speaker stands.
  g.poly(lump(cx - r * 0.05, cy - r * 0.06, r * 0.22, r * 0.17, rnd() * 3, rnd, 9)).fill({ color: st.light, alpha: 0.3 });
  // The lit north-west lip.
  g.poly(arcBand(cx, cy, T, scaled(T, 1, -PX * 0.07), Math.PI * 0.85, Math.PI * 1.65, 18)).fill({ color: st.sun, alpha: 0.55 });
  g.poly(top).stroke({ width: LINE, color: INK, join: 'round' });
  dress(g, cx, cy, P, scaled(T, 0.85), dressOf(gr), gr, rnd);
}

/** The bed's soil by the ground: a muted loam mixed with the ground about; snow lying on it on snow, dry earth on sand,
 * a grey loam under ash. */
function soilOf(gr: PlazaGround, kind: Dress) {
  const foot = groundColour(gr.nature >= 0 && PAVED.has(gr.foot) ? gr.nature : gr.foot, gr.tone);
  if (kind === 'snow') return { bed: 0xe4e9f0, shade: 0xb9c6dc, fleck: 0x6a5a48 };
  if (kind === 'sand') return { bed: mix(0x9a7a52, groundColour(3), 0.45), shade: 0x7a5e3e, fleck: 0xc9b08a };
  if (kind === 'ash') return { bed: mix(0x4e4440, foot, 0.2), shade: 0x2e2826, fleck: 0x6a6266 };
  const bed = mix(0x5a4632, foot, 0.28);
  return { bed, shade: mix(bed, 0x2a1e16, 0.45), fleck: mix(bed, 0x6f8a44, 0.5) };
}

/** A tree's planter on a square or in a yard (`r`: its outer radius, px; `a`: a bearing for its shape): a jointed
 * stone kerb, round or octagonal by the place's hash, round a bed of soil (the tree's sprite stands on its
 * middle, which is kept calm). */
export function drawPlazaPlanter(g: Graphics, cx: number, cy: number, r: number, a: number, gr: PlazaGround) {
  const rnd = rngAt(gr.wx + cx, gr.wy + cy);
  const st = stoneRamp(gr.tone);
  const rs = r / PX;
  const small = rs < 0.8;
  const paved = PAVED.has(gr.foot);
  const v = rnd() < 0.5 ? 0 : 1;
  const dressing = dressOf(gr);
  const foot = groundColour(gr.foot, gr.tone);
  // (An octagon's flats stand at ~0.98 of `r`, its corners a little past: the tree's crown must leave a ring of kerb round it.)
  const rr = r - (paved ? PX * 0.15 : 0);
  const P = outline(v === 1 ? rr / 0.94 : rr, v, a, false, rnd);
  const cope = PX * Math.min(0.34, Math.max(0.18, rs * 0.18));
  const Ri = scaled(P, 1, -cope);
  apron(g, cx, cy, P, gr, rnd);
  contactShadow(g, cx, cy, P, foot, paved);
  stoneBand(g, cx, cy, P, Ri, st, PX * (small ? 0.6 : 0.85), rnd, 0.25, v === 1 ? a + Math.PI / 8 : 0.3);
  g.poly(around(cx, cy, P)).stroke({ width: OUT, color: INK, join: 'round' });
  // The bed: the coping's shade inside its north-west side, a few clods (or bare earth through the snow) by the edge.
  const S = soilOf(gr, dressing);
  const bed = around(cx, cy, Ri);
  const ri = Ri(0.3);
  g.poly(bed).fill(S.bed);
  g.poly(arcBand(cx, cy, Ri, (t) => Ri(t) - ri * 0.16 * Math.max(0, lit(t) + 0.25), Math.PI * 0.5, Math.PI * 2, 24)).fill({ color: S.shade, alpha: 0.5 });
  // (The tree's crown covers the bed, so nothing is drawn on it but the shade under the kerb.)
  g.poly(bed).stroke({ width: LINE, color: INK, join: 'round' });
  dress(g, cx, cy, P, Ri, dressing === 'ash' ? null : dressing, gr, rnd);
}

/** The base of a market cross on a square (`r`: its outer radius, px; `a`: a bearing): two low steps, stepped
 * octagons or squares by the place's hash, each lit along its north-west edge with its dark riser to the south-east,
 * the socket stone with the cross's top drawn on it. */
export function drawPlazaCross(g: Graphics, cx: number, cy: number, r: number, a: number, gr: PlazaGround) {
  const rnd = rngAt(gr.wx + cx, gr.wy + cy);
  const st = stoneRamp(gr.tone);
  const rs = r / PX;
  const paved = PAVED.has(gr.foot);
  const oct = rnd() < 0.6;
  const R0 = r - (paved ? PX * 0.3 : 0);
  const shape = (R: number): Rf => (oct ? polyR(R, 8, a + Math.PI / 8) : polyR(R, 4, Math.PI / 4));
  const P = shape(R0);
  const foot = groundColour(gr.foot, gr.tone);
  apron(g, cx, cy, P, gr, rnd);
  contactShadow(g, cx, cy, P, foot, paved);
  const riser = PX * Math.min(0.16, 0.08 + rs * 0.025);
  const top = (R: Rf): Rf => (t) => R(t) - riser * (0.5 - 0.5 * lit(t));
  const step = (R: Rf, inner: Rf, block: number, ink: number) => {
    const T = top(R);
    g.poly(around(cx, cy, R)).fill(st.dark).stroke({ width: ink, color: INK, join: 'round' });
    stoneBand(g, cx, cy, T, inner, st, block, rnd, 0.3, oct ? a + Math.PI / 8 : Math.PI / 4);
    g.poly(arcBand(cx, cy, T, scaled(T, 1, -PX * 0.06), Math.PI * 0.85, Math.PI * 1.65, 18)).fill({ color: st.sun, alpha: 0.5 });
    g.poly(around(cx, cy, T)).stroke({ width: LINE, color: INK, join: 'round' });
  };
  const P2 = shape(R0 * 0.62);
  const P3 = shape(Math.max(PX * 0.3, R0 * 0.3));
  // The lower step's tread runs in to the upper's foot; the upper's to the socket stone's.
  step(P, P2, PX * 0.9, OUT);
  // The upper step's short shadow on the lower tread.
  g.poly(around(cx, cy, P2, 48, riser * 0.8, riser * 0.8)).fill({ color: st.dark, alpha: 0.35 });
  step(P2, P3, PX * 0.7, LINE);
  // The socket stone: a calm stepped top one faint joint.
  const S = top(P3);
  g.poly(around(cx, cy, P3, 48, riser * 0.5, riser * 0.5)).fill({ color: st.dark, alpha: 0.25 });
  g.poly(around(cx, cy, S, 48)).fill(st.light).stroke({ width: LINE * 0.8, color: st.joint, alpha: 0.7, join: 'round' });
  // The cross seen from above: a plus (shaft top in the middle, four short arms), lit north-west, dark south-east, its short shadow to the south-east.
  const L = Math.min(PX * 0.3, R0 * 0.3 * 0.8), w = L * 0.3, o = L * 0.3;
  const plus = (dx: number, dy: number, L2 = L, w2 = w) => [-w2, -L2, w2, -L2, w2, -w2, L2, -w2, L2, w2, w2, w2, w2, L2, -w2, L2, -w2, w2, -L2, w2, -L2, -w2, -w2, -w2].map((v, i) => v + (i % 2 ? cy + dy : cx + dx));
  g.poly(plus(o, o, L * 1.05, w * 1.1)).fill({ color: 0x2a1a20, alpha: 0.32 });
  g.poly(plus(0, 0)).fill(mix(st.light, st.dark, 0.18));
  g.poly(plus(-w * 0.25, -w * 0.25, L * 0.97, w * 0.62)).fill(mix(st.light, st.sun, 0.55));
  // Dark south-east edges of the arms and the shaft top as a small square.
  const e = w * 0.3;
  g.rect(cx + w - e, cy + w, e, L - w).fill({ color: st.dark, alpha: 0.6 });
  g.rect(cx + w, cy + w - e, L - w, e).fill({ color: st.dark, alpha: 0.6 });
  g.rect(cx - w, cy - w, w * 2, w * 2).fill(st.sun).stroke({ width: LINE * 0.7, color: st.joint, alpha: 0.8, join: 'round' });
  g.poly(plus(0, 0)).stroke({ width: OUT, color: INK, join: 'round' });
  dress(g, cx, cy, P, P2, dressOf(gr), gr, rnd);
}

/** A steppe town's dais (`drawDais` kind 4): a low bank of the trodden ground itself a shade lighter, uneven round its
 * foot, its far slope shaded to the south-east, hoof prints worn round its edge, a rutted way up where its steps would
 * be, a few tufts where hooves don't reach. Everything but its shadow inside `r`. */
export function drawPlazaMound(g: Graphics, cx: number, cy: number, r: number, a: number, gr: PlazaGround) {
  const rnd = rngAt(gr.wx + cx, gr.wy + cy);
  const foot = groundColour(gr.foot, gr.tone);
  const [p1, p2, sq] = [rnd() * 6.28, rnd() * 6.28, 0.86 + rnd() * 0.12];
  // (An uneven round, a little long across the way up; at most `r` out.)
  const R = r * 0.84;
  const Rf0: Rf = (t) => {
    const l = t - a;
    const w = R * (1 + 0.08 * Math.sin(3 * l + p1) + 0.05 * Math.cos(5 * l + p2));
    const [u, v] = [Math.cos(l), Math.sin(l) * sq];
    return w * Math.hypot(u, v);
  };
  const blob = (dx: number, dy: number, k: number) => around(cx + dx, cy + dy, scaled(Rf0, k), 40);
  const snowy = gr.foot === 4 || gr.foot === 13;
  const sandy = gr.foot === 3;
  const light = mix(foot, 0xfff1cf, snowy ? 0.3 : 0.16);
  contactShadow(g, cx, cy, scaled(Rf0, 1.04), foot, false);
  g.poly(blob(0, 0, 1.04)).fill(mix(foot, light, 0.25)).stroke({ width: LINE, color: mul(foot, 0.62), alpha: 0.7 });
  // The far slope's shade, then the lit top.
  g.poly(arcBand(cx, cy, scaled(Rf0, 1.0), scaled(Rf0, 0.72), -Math.PI * 0.3, Math.PI * 0.8, 20)).fill({ color: mul(foot, 0.8), alpha: 0.45 });
  g.poly(blob(-R * 0.06, -R * 0.07, 0.76)).fill(mix(foot, light, 0.6));
  g.poly(blob(-R * 0.11, -R * 0.13, 0.5)).fill({ color: light, alpha: 0.6 });
  // The way up: a worn, paler tongue down to the foot, two ruts in it.
  const [ux, uy, vx, vy] = [Math.cos(a), Math.sin(a), -Math.sin(a), Math.cos(a)];
  const at = (al: number, ac: number): [number, number] => [cx + ux * al + vx * ac, cy + uy * al + vy * ac];
  const wd = Math.min(PX * 0.7, R * 0.35);
  const end = Math.min(Rf0(a) * 1.04, r * 0.98);
  g.poly([...at(R * 0.35, -wd * 0.7), ...at(end * 0.9, -wd), ...at(end, -wd * 0.5), ...at(end, wd * 0.5), ...at(end * 0.9, wd), ...at(R * 0.35, wd * 0.7)]).fill({ color: mix(light, 0xfff1cf, 0.15), alpha: 0.45 });
  for (const c of [-0.4, 0.4]) g.moveTo(...at(R * 0.4, wd * c)).lineTo(...at(end * 0.98, wd * c * 1.15));
  g.stroke({ width: LINE, color: mul(foot, 0.6), alpha: 0.6 });
  // Hoof prints round the edge, in pairs.
  const prints = Math.round(8 + (r / PX) * 4);
  for (let k = 0; k < prints; k++) {
    const t = rnd() * Math.PI * 2;
    const d = Rf0(t) * (0.8 + rnd() * 0.16);
    const [hx, hy] = [cx + Math.cos(t) * d, cy + Math.sin(t) * d];
    const hr = PX * (0.06 + rnd() * 0.03);
    g.moveTo(hx + hr, hy).arc(hx, hy, hr, 0, Math.PI * 1.3);
    g.moveTo(hx + PX * 0.17 + hr, hy + PX * 0.09).arc(hx + PX * 0.17, hy + PX * 0.09, hr, 0.3, Math.PI * 1.5);
  }
  g.stroke({ width: LINE * 0.8, color: mul(foot, 0.55), alpha: 0.5 });
  // Tufts on the top and the far slope (none on snow; dry ones on sand).
  if (!snowy) {
    const grass = sandy ? [0x8a7a48, 0xa8955a] : [0x6c7c38, 0x7f8f42];
    for (let k = 0; k < (sandy ? 3 : 6); k++) {
      const t = rnd() * Math.PI * 2;
      const d = Rf0(t) * (k < 2 ? rnd() * 0.45 : 0.55 + rnd() * 0.25);
      const [tx, ty] = [cx + Math.cos(t) * d, cy + Math.sin(t) * d];
      const h = PX * (0.14 + rnd() * 0.08);
      for (const dx of [-0.6, 0, 0.6]) g.moveTo(tx + dx * h * 0.3, ty).lineTo(tx + dx * h * 0.8, ty - h * (dx === 0 ? 1.1 : 0.8));
      g.stroke({ width: LINE * 1.2, color: grass[k % 2], cap: 'round' });
    }
  }
  const kind = dressOf(gr);
  if (kind === 'snow' || kind === 'ash') dress(g, cx, cy, Rf0, scaled(Rf0, 0.7), kind, gr, rnd);
}
