import type { Graphics } from 'pixi.js';
import { ROOF_INK, type Roof } from '../../gen/battlePrep';
import { PX } from '../atlas';
import { rngAt } from '../bridges';
import { banner, cornerHatch, flagFloor, PARAPET, parapetRing, roundParapet, roundTop, roundWalk, WALK } from '../walls';

/** `c` scaled by `k` per channel (shading a lit or shadowed face). */
function shadeColor(c: number, k: number) {
  const f = (v: number) => Math.max(0, Math.min(255, Math.round(v * k)));
  return (f(c >> 16) << 16) | (f((c >> 8) & 255) << 8) | f(c & 255);
}

/** How lit a face sloping down toward (nx, ny) is under the NW light. */
const faceLight = (nx: number, ny: number) => 0.86 + 0.24 * (-nx - ny) * Math.SQRT1_2;

const SLATE = 0x5d6670;

/**
 * A round tower seen from above: its shadow, a stone ring, and a conical slate roof in lit and
 * shadowed wedges with a finial (or, when `flat`, a wall tower's crenellated fighting top).
 */
function roundTower(g: Graphics, x: number, y: number, r: number, lift: number, flat: boolean) {
  g.circle(x + lift, y + lift, r).fill({ color: 0x000000, alpha: 0.32 });
  if (flat) return void roundTop(g, x, y, r, rngAt(x, y), false, [], true);
  g.circle(x, y, r).fill(0x8f897d).stroke({ width: 2.5, color: ROOF_INK });
  const n = 16;
  const rr = r * 0.92;
  for (let k = 0; k < n; k++) {
    const a0 = (k / n) * Math.PI * 2;
    const a1 = ((k + 1) / n) * Math.PI * 2;
    const am = (a0 + a1) / 2;
    g.poly([x, y, x + Math.cos(a0) * rr, y + Math.sin(a0) * rr, x + Math.cos(a1) * rr, y + Math.sin(a1) * rr]).fill(shadeColor(SLATE, faceLight(Math.cos(am), Math.sin(am))));
  }
  // Slate courses round the cone.
  for (const f of [0.35, 0.6, 0.82]) g.circle(x, y, rr * f).stroke({ width: 1, color: 0x2f353b, alpha: 0.45 });
  g.circle(x, y, rr).stroke({ width: 2, color: ROOF_INK });
  g.circle(x, y, r * 0.1).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
}

/** Dark lead (flats and lead roofs), and its rolls. */
const LEAD = 0x5f656b;
/** In a keep body's top code (`town::DONJON_LOW`): its donjon at its first corner's end. */
const DONJON_LOW = 4;
const LEAD_ROLL = 0x80878d;

/**
 * The top of a four-cornered piece of a keep (`town::keep_tops`), from corner 0 along its longer
 * side: `top` 0 open battlements, a wall walk of stone flags inside the parapet (a broad one's
 * middle a flat of dark lead with its rolls), a stair hatch in a corner or (`cap`, the donjon) a
 * cap house under a slate pyramid in the middle; 1 or 2 a hipped roof of lead (its rolls up the
 * slopes) or slate (its courses) inside the wall walk. Its parapet (`parapetRing`) and outline last.
 */
function keepTop(g: Graphics, pts: number[], top: number, cap: boolean, plain: number[] = []) {
  const [p0x, p0y] = [pts[0], pts[1]];
  let [ax, ay] = [pts[2] - p0x, pts[3] - p0y];
  let [bx, by] = [pts[6] - p0x, pts[7] - p0y];
  if (Math.hypot(bx, by) > Math.hypot(ax, ay)) [ax, ay, bx, by] = [bx, by, ax, ay];
  const L = Math.hypot(ax, ay) || 1;
  const W = Math.hypot(bx, by) || 1;
  const [ux, uy] = [ax / L, ay / L];
  const [vx, vy] = [bx / W, by / W];
  const P = (s: number, t: number): [number, number] => [p0x + ux * s + vx * t, p0y + uy * s + vy * t];
  const poly = (...st: [number, number][]) => st.flatMap(([s, t]) => P(s, t));
  const rect = (s0: number, s1: number, t0: number, t1: number) => poly([s0, t0], [s1, t0], [s1, t1], [s0, t1]);
  const [cx, cy] = P(L / 2, W / 2);
  const ww = PX * 1.15;
  const rnd = rngAt(cx, cy);
  if (top === 0) {
    // Stone flags in courses, their joints staggered.
    flagFloor(g, pts, rnd);
    const pw = PX * 1.6;
    const leaded = L > PX * 7 && W > PX * 7;
    if (leaded) {
      const lead = rect(pw, L - pw, pw, W - pw);
      g.poly(lead).fill(LEAD).stroke({ width: 1.5, color: 0x3d4146 });
      for (let s = pw + PX * 1.2; s < L - pw - PX * 0.3; s += PX * 1.2) g.moveTo(...P(s, pw)).lineTo(...P(s, W - pw));
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.6 });
    }
    if (cap) {
      const c = Math.min(L, W) * 0.17;
      const [cs, ct] = [L / 2, W / 2];
      const box = rect(cs - c, cs + c, ct - c, ct + c);
      g.poly(box.map((v) => v + PX * 0.6)).fill({ color: 0x000000, alpha: 0.32 });
      g.poly(box).fill(0x8f897d).stroke({ width: 2, color: ROOF_INK });
      const r = c * 0.85;
      const apex = P(cs, ct);
      const pyr: [[number, number], [number, number], number, number][] = [
        [[cs - r, ct - r], [cs + r, ct - r], -vx, -vy],
        [[cs + r, ct - r], [cs + r, ct + r], ux, uy],
        [[cs + r, ct + r], [cs - r, ct + r], vx, vy],
        [[cs - r, ct + r], [cs - r, ct - r], -ux, -uy],
      ];
      for (const [e0, e1, nx, ny] of pyr) g.poly([...P(...e0), ...P(...e1), ...apex]).fill(shadeColor(SLATE, faceLight(nx, ny))).stroke({ width: 1.2, color: 0x2f353b, alpha: 0.7 });
      g.circle(...apex, PX * 0.14).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
    } else cornerHatch(g, pts, leaded ? pw : Math.min(L, W) / 2);
  } else {
    // The wall walk (its joints with the parapet), and a hipped roof inside it along the piece's length.
    g.poly(rect(0, L, 0, W)).fill(WALK);
    const base = top === 1 ? LEAD : SLATE;
    const [s0, s1, a, b] = [ww, L - ww, ww, W - ww];
    const h = Math.min((b - a) / 2, (s1 - s0) / 2);
    const tm = (a + b) / 2;
    const r0: [number, number] = [s0 + h, tm];
    const r1: [number, number] = [s1 - h, tm];
    const faces: [[number, number][], number, number][] = [
      [[[s0, a], [s1, a], r1, r0], -vx, -vy],
      [[[s0, b], [s1, b], r1, r0], vx, vy],
      [[[s0, a], r0, [s0, b]], -ux, -uy],
      [[[s1, a], r1, [s1, b]], ux, uy],
    ];
    for (const [fc, nx, ny] of faces) g.poly(poly(...fc)).fill(shadeColor(base, faceLight(nx, ny)));
    if (top === 1) {
      // Lead rolls from the eaves up the long slopes.
      for (let s = s0 + PX * 1.1; s < s1 - PX * 0.3; s += PX * 1.1) {
        const k = Math.min(s - s0, s1 - s, h);
        g.moveTo(...P(s, a)).lineTo(...P(s, a + k)).moveTo(...P(s, b)).lineTo(...P(s, b - k));
      }
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.55 });
    } else {
      for (let d = PX * 0.45; d < h - PX * 0.1; d += PX * 0.45) {
        g.moveTo(...P(s0 + d, a + d)).lineTo(...P(s1 - d, a + d)).lineTo(...P(s1 - d, b - d)).lineTo(...P(s0 + d, b - d)).lineTo(...P(s0 + d, a + d));
      }
      g.stroke({ width: 1, color: 0x2f353b, alpha: 0.35 });
    }
    g.moveTo(...P(s0, a)).lineTo(...P(...r0)).lineTo(...P(s0, b)).moveTo(...P(s1, a)).lineTo(...P(...r1)).lineTo(...P(s1, b));
    g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
    g.moveTo(...P(...r0)).lineTo(...P(...r1)).stroke({ width: 3, color: 0x9aa0a8 });
    g.poly(rect(s0, s1, a, b)).stroke({ width: 1.8, color: ROOF_INK });
    // (The walk's inner north-west edges shade the roof set below it.)
    g.moveTo(...P(s1, a)).lineTo(...P(s0, a)).lineTo(...P(s0, b));
    g.stroke({ width: PX * 0.35, color: 0x000000, alpha: 0.22 });
  }
  parapetRing(g, pts, rnd, top === 0 ? 0 : ww - PARAPET, [], plain);
}

/**
 * A keep from above (a rectangle with corner towers): a tall square donjon at one end (a flat
 * leaded roof inside its own crenellated parapet, a stair turret with a banner) and a lower hall
 * range along the rest: a crenellated wall walk round hipped slate roofs (one or two ridges,
 * lead gutters, chimneys). Round corner towers with conical roofs stand a little proud of the
 * walls, bartizans along long walls; the whole casts a long shadow. A keep's body among its other
 * pieces (`drawKeepParts`: `avoid`, their outlines) has no corner towers of its own, no bartizan
 * where a piece meets its wall and its stair turret at the donjon's corner farthest from them (none
 * where no corner is clear).
 */
function drawKeep(g: Graphics, pts: number[], tower: number, cornerTowers = true, avoid: number[][] = [], top = 0, low?: boolean) {
  // How far a point stands clear of the other pieces (negative inside one).
  const clearOf = (x: number, y: number) => Math.min(Infinity, ...avoid.map((a) => polyClear(a, x, y)));
  // Long axis u from corner 0, short axis v.
  const [p0x, p0y] = [pts[0], pts[1]];
  let [ax, ay] = [pts[2] - p0x, pts[3] - p0y];
  let [bx, by] = [pts[6] - p0x, pts[7] - p0y];
  if (Math.hypot(bx, by) > Math.hypot(ax, ay)) [ax, ay, bx, by] = [bx, by, ax, ay];
  const L = Math.hypot(ax, ay);
  const W = Math.hypot(bx, by);
  const [ux, uy] = [ax / L, ay / L];
  const [vx, vy] = [bx / W, by / W];
  const P = (s: number, t: number): [number, number] => [p0x + ux * s + vx * t, p0y + uy * s + vy * t];
  const poly = (...st: [number, number][]) => st.flatMap(([s, t]) => P(s, t));
  const rect = (s0: number, s1: number, t0: number, t1: number) => poly([s0, t0], [s1, t0], [s1, t1], [s0, t1]);
  let seed = (Math.round(p0x * 13.7) * 73856093) ^ (Math.round(p0y * 7.3) * 19349663);
  const rnd = () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
  const lift = PX * 0.35;
  // The donjon: a square at one end (none on a short keep).
  const D = L > W * 1.35 ? Math.min(W, L * 0.42) : 0;
  // (A keep built of pieces: at the end its top code names, as its interior has it.)
  const rolled = rnd() < 0.5;
  const donjonLow = low ?? rolled;
  const [d0, d1] = donjonLow ? [0, D] : [L - D, L];
  const [h0, h1] = donjonLow ? [D, L] : [0, L - D];
  // Shadow of the whole mass, longer from the donjon.
  g.poly(pts.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
  if (D > 0) g.poly(rect(d0, d1, 0, W).map((v) => v + PX * 2.2)).fill({ color: 0x000000, alpha: 0.22 });
  // Wall walk round the hall range.
  const ww = PX * 1.15;
  g.poly(rect(h0, h1, 0, W)).fill(WALK);
  // Hipped slate roofs inside the walk.
  const [s0, s1, t0, t1] = [h0 + (donjonLow ? 0 : ww), h1 - (donjonLow ? ww : 0), ww, W - ww];
  const bands = Math.max(1, Math.min(2, Math.round((t1 - t0) / (PX * 10))));
  const bw = (t1 - t0) / bands;
  for (let k = 0; k < bands; k++) {
    const [a, b] = [t0 + k * bw, t0 + (k + 1) * bw];
    const h = Math.min(bw / 2, (s1 - s0) / 2);
    const tm = (a + b) / 2;
    const r0: [number, number] = [s0 + h, tm];
    const r1: [number, number] = [s1 - h, tm];
    const faces: [[number, number][], number, number][] = [
      [[[s0, a], [s1, a], r1, r0], -vx, -vy],
      [[[s0, b], [s1, b], r1, r0], vx, vy],
      [[[s0, a], r0, [s0, b]], -ux, -uy],
      [[[s1, a], r1, [s1, b]], ux, uy],
    ];
    for (const [f, nx, ny] of faces) g.poly(poly(...f)).fill(shadeColor(SLATE, faceLight(nx, ny)));
    // Slate courses parallel to the eaves.
    for (let d = PX * 0.45; d < h - PX * 0.1; d += PX * 0.45) {
      g.moveTo(...P(s0 + d, a + d)).lineTo(...P(s1 - d, a + d)).lineTo(...P(s1 - d, b - d)).lineTo(...P(s0 + d, b - d)).lineTo(...P(s0 + d, a + d));
    }
    g.stroke({ width: 1, color: 0x2f353b, alpha: 0.35 });
    g.moveTo(...P(s0, a)).lineTo(...P(...r0)).lineTo(...P(s0, b)).moveTo(...P(s1, a)).lineTo(...P(...r1)).lineTo(...P(s1, b));
    g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
    g.moveTo(...P(...r0)).lineTo(...P(...r1)).stroke({ width: 3, color: 0x9aa0a8 });
    g.poly(rect(s0, s1, a, b)).stroke({ width: 1.8, color: ROOF_INK });
    // Chimneys astride the ridge.
    const nch = 1 + Math.floor(rnd() * 2);
    for (let c = 0; c < nch; c++) {
      const s = r0[0] + (r1[0] - r0[0]) * ((c + 1) / (nch + 1)) + (rnd() - 0.5) * PX;
      const q = rect(s - PX * 0.6, s + PX * 0.6, tm - PX * 0.4, tm + PX * 0.4);
      g.poly(q.map((v) => v + PX * 0.45)).fill({ color: 0x000000, alpha: 0.35 });
      g.poly(q).fill(0x8a7f70).stroke({ width: 1.8, color: ROOF_INK });
      for (const o of [-0.28, 0.28]) g.circle(...P(s + o * PX, tm), PX * 0.16).fill(0x1d1a17);
    }
  }
  for (let k = 1; k < bands; k++) g.moveTo(...P(s0, t0 + k * bw)).lineTo(...P(s1, t0 + k * bw));
  g.stroke({ width: 3, color: 0x6b7077 });
  // The walk's inner NW edges shade the roofs set below it.
  g.moveTo(...P(s1, t0)).lineTo(...P(s0, t0)).lineTo(...P(s0, t1));
  g.stroke({ width: PX * 0.35, color: 0x000000, alpha: 0.22 });
  // Its parapet and walk (none against the donjon: its side 3 at the low end, else 1).
  const range = rect(h0, h1, 0, W);
  parapetRing(g, range, rngAt(...P((h0 + h1) / 2, W / 2)), ww - PARAPET, D > 0 ? [donjonLow ? 3 : 1] : []);
  // Bartizans midway along the range's long walls (none where another piece meets the wall).
  if (h1 - h0 > PX * 18) {
    for (const t of [0, W]) {
      const at = P((h0 + h1) / 2, t);
      if (clearOf(...at) > PX * 3) roundTower(g, ...at, PX * 1.1, lift * 0.8, false);
    }
  }
  // The donjon, a storey or two above the range: its shadow falls across the range roofs.
  if (D > 0) {
    const dj = rect(d0, d1, 0, W);
    g.poly(dj).fill(0x8f897d);
    // Its top (`keepTop`): open battlements round a flat of dark lead with a cap house, or a roof
    // (the banner on its ridge).
    keepTop(g, dj, top, true, [donjonLow ? 1 : 3]);
    const pw = PX * 1.3;
    if (top !== 0) banner(g, ...P((d0 + d1) / 2, W / 2));
    // The stair turret on the roof, the banner above it (among other pieces, at the flats' corner
    // farthest from them, if one is clear).
    const inner = donjonLow ? d1 - pw - PX * 1.6 : d0 + pw + PX * 1.6;
    const outer = donjonLow ? d0 + pw + PX * 1.6 : d1 - pw - PX * 1.6;
    const spots: [number, number][] = avoid.length
      ? [P(inner, pw + PX * 1.6), P(inner, W - pw - PX * 1.6), P(outer, pw + PX * 1.6), P(outer, W - pw - PX * 1.6)]
      : [P(inner, pw + PX * 1.6)];
    let best = spots[0];
    for (const q of spots) if (clearOf(...q) > clearOf(...best)) best = q;
    if (top === 0 && (!avoid.length || clearOf(...best) > PX * 1.25 + PX * 1.5)) {
      roundTower(g, ...best, PX * 1.25, lift * 1.6, false);
      banner(g, ...best);
    }
  }
  // Corner towers above all; with no donjon the great one carries the banner. (A keep built of
  // pieces draws its own towers: `drawKeepParts`.)
  if (!cornerTowers) return;
  const rt = Math.max(tower * 0.6, Math.min(PX * 2.6, W * 0.11));
  const inset = rt * 0.55;
  const corners: [number, number][] = [
    [inset, inset],
    [L - inset, inset],
    [L - inset, W - inset],
    [inset, W - inset],
  ];
  const great = Math.floor(rnd() * 4);
  corners.forEach(([s, t], i) => {
    const onDonjon = D > 0 && s >= d0 && s <= d1;
    roundTower(g, ...P(s, t), onDonjon ? rt * 1.15 : rt, lift * (onDonjon ? 2.2 : 1.3), onDonjon);
  });
  if (D === 0) banner(g, ...P(...corners[great]));
}

/** A square tower's roof from above: its shadow, the stone below the eaves and a pyramid of slate
 * in lit and shaded faces, its ridges and a finial. */
function squareTowerRoof(g: Graphics, pts: number[], lift: number) {
  let [cx, cy] = [0, 0];
  for (let i = 0; i < 4; i++) [cx, cy] = [cx + pts[i * 2] / 4, cy + pts[i * 2 + 1] / 4];
  g.poly(pts.map((v) => v + lift)).fill({ color: 0x000000, alpha: 0.32 });
  g.poly(pts).fill(0x8f897d).stroke({ width: 2.5, color: ROOF_INK });
  const eave = pts.map((v, k) => (k % 2 ? cy + (v - cy) * 0.9 : cx + (v - cx) * 0.9));
  for (let i = 0; i < 4; i++) {
    const [ax, ay, bx, by] = [eave[i * 2], eave[i * 2 + 1], eave[((i + 1) % 4) * 2], eave[((i + 1) % 4) * 2 + 1]];
    const [mx, my] = [(ax + bx) / 2 - cx, (ay + by) / 2 - cy];
    const l = Math.hypot(mx, my) || 1;
    g.poly([ax, ay, bx, by, cx, cy]).fill(shadeColor(SLATE, faceLight(mx / l, my / l)));
  }
  // Slate courses parallel to the eaves, the hips to the apex.
  for (const f of [0.3, 0.55, 0.78]) g.poly(eave.map((v, k) => (k % 2 ? cy + (v - cy) * f : cx + (v - cx) * f))).stroke({ width: 1, color: 0x2f353b, alpha: 0.45 });
  for (let i = 0; i < 4; i++) g.moveTo(eave[i * 2], eave[i * 2 + 1]).lineTo(cx, cy);
  g.stroke({ width: 1.5, color: 0x8a9099, alpha: 0.8 });
  g.poly(eave).stroke({ width: 2, color: ROOF_INK });
  g.circle(cx, cy, PX * 0.14).fill(0xc9a227).stroke({ width: 1, color: ROOF_INK });
}

/**
 * A shell keep from above (`drawKeepParts`): a round curtain with its wall walk and parapet, lean-to
 * slate roofs against its inside round an open, flagged yard with a well; its gate at its first
 * corner (toward the bailey's gate, where the path comes up the motte).
 */
function drawShellKeep(g: Graphics, pts: number[]) {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  const R = Math.hypot(pts[0] - cx, pts[1] - cy);
  const ring = (r: number) => {
    const out: number[] = [];
    for (let k = 0; k < 40; k++) out.push(cx + Math.cos((k / 40) * Math.PI * 2) * r, cy + Math.sin((k / 40) * Math.PI * 2) * r);
    return out;
  };
  g.circle(cx + PX * 1.2, cy + PX * 1.2, R).fill({ color: 0x000000, alpha: 0.3 });
  g.circle(cx, cy, R).fill(WALK);
  const walk = R - PX * 1.2;
  const lean = walk - PX * 3.2;
  // Lean-to roofs: sloping in from the walk to the yard, shaded as they face.
  const n = 48;
  for (let k = 0; k < n; k++) {
    const [a0, a1] = [(k / n) * Math.PI * 2, ((k + 1) / n) * Math.PI * 2];
    const am = (a0 + a1) / 2;
    const q = [cx + Math.cos(a0) * walk, cy + Math.sin(a0) * walk, cx + Math.cos(a1) * walk, cy + Math.sin(a1) * walk, cx + Math.cos(a1) * lean, cy + Math.sin(a1) * lean, cx + Math.cos(a0) * lean, cy + Math.sin(a0) * lean];
    // (The face slopes down toward the yard: it faces inward.)
    g.poly(q).fill(shadeColor(SLATE, faceLight(-Math.cos(am), -Math.sin(am))));
  }
  for (const f of [0.33, 0.66]) g.poly(ring(lean + (walk - lean) * f)).stroke({ width: 1, color: 0x2f353b, alpha: 0.4 });
  for (let k = 0; k < n; k += 6) {
    const a = (k / n) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * lean, cy + Math.sin(a) * lean).lineTo(cx + Math.cos(a) * walk, cy + Math.sin(a) * walk);
  }
  g.stroke({ width: 1.5, color: 0x6b7077, alpha: 0.8 });
  // The yard: flags, its edge in the roofs' shadow, a well.
  g.poly(ring(lean)).fill(0xb0a692).stroke({ width: 2, color: ROOF_INK });
  for (let x = cx - lean; x < cx + lean; x += PX * 0.8) {
    const h = Math.sqrt(Math.max(0, lean * lean - (x - cx) ** 2));
    g.moveTo(x, cy - h).lineTo(x, cy + h);
  }
  g.stroke({ width: 1, color: 0x6e675c, alpha: 0.35 });
  g.circle(cx - PX * 0.3, cy - PX * 0.3, lean).stroke({ width: PX * 0.6, color: 0x000000, alpha: 0.15 });
  g.circle(cx, cy, PX * 0.55).fill(0x5d6670).stroke({ width: 2, color: ROOF_INK });
  g.circle(cx, cy, PX * 0.3).fill(0x1d2a33);
  // The curtain's walk and parapet, as a wall tower's.
  const rnd = rngAt(cx, cy);
  roundWalk(g, cx, cy, walk, R - PARAPET, rnd);
  roundParapet(g, cx, cy, R, rnd() * Math.PI, rnd);
  g.circle(cx, cy, R).stroke({ width: 3, color: ROOF_INK });
  g.circle(cx, cy, walk).stroke({ width: 1.8, color: ROOF_INK });
  // The gate: a passage through the wall and the lean-tos to the yard, its doors at the outer face,
  // a turret either side.
  const ga = Math.atan2(pts[1] - cy, pts[0] - cx);
  const [ux, uy] = [Math.cos(ga), Math.sin(ga)];
  const [vx, vy] = [-uy, ux];
  const hw = PX * 0.7;
  const at = (r: number, s: number): [number, number] => [cx + ux * r + vx * s, cy + uy * r + vy * s];
  g.poly([...at(lean - PX * 0.2, -hw), ...at(R + PX * 0.3, -hw), ...at(R + PX * 0.3, hw), ...at(lean - PX * 0.2, hw)])
    .fill(0x9c917e)
    .stroke({ width: 2, color: ROOF_INK });
  for (let q = lean + PX * 0.4; q < R; q += PX * 0.5) g.moveTo(...at(q, -hw)).lineTo(...at(q, hw));
  g.stroke({ width: 1, color: 0x6e675c, alpha: 0.5 });
  g.moveTo(...at(R + PX * 0.1, -hw))
    .lineTo(...at(R + PX * 0.1, hw))
    .stroke({ width: PX * 0.25, color: 0x5a4030 });
  for (const s of [-1, 1]) roundTower(g, ...at(R - PX * 0.3, s * (hw + PX * 0.75)), PX * 0.85, PX * 0.3, false);
  // (The banner over the wall across from the gate.)
  banner(g, cx - ux * R * 0.72, cy - uy * R * 0.72);
}

/**
 * A keep built of pieces (`Roof.parts`): its wings and forebuilding under flat leaded roofs inside
 * their parapets, the body over them (a rectangle as `drawKeep` draws a keep, without its corner
 * towers; a round one as a shell keep), its towers over all: round ones under conical slate roofs,
 * square ones under slate pyramids.
 */
function drawKeepParts(g: Graphics, r: Roof) {
  // A flat roof of dark grey lead inside its flagged walk and parapet, lead rolls across it along its length.
  const flat = (pts: number[]) => {
    const m = pts.length / 2;
    let [cx, cy] = [0, 0];
    for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
    const rnd = rngAt(cx, cy);
    flagFloor(g, pts, rnd);
    const lead = pts.map((v, k) => (k % 2 ? cy + (v - cy) * 0.7 : cx + (v - cx) * 0.7));
    g.poly(lead).fill(LEAD).stroke({ width: 1.5, color: 0x3d4146 });
    if (m === 4) {
      // (Along its longer side, from its first corner.)
      let [ux, uy] = [lead[2] - lead[0], lead[3] - lead[1]];
      let [vx, vy] = [lead[6] - lead[0], lead[7] - lead[1]];
      if (Math.hypot(vx, vy) > Math.hypot(ux, uy)) [ux, uy, vx, vy] = [vx, vy, ux, uy];
      const L = Math.hypot(ux, uy) || 1;
      for (let s = PX * 1.2; s < L - PX * 0.3; s += PX * 1.2) {
        const [x, y] = [lead[0] + (ux * s) / L, lead[1] + (uy * s) / L];
        g.moveTo(x, y).lineTo(x + vx, y + vy);
      }
      g.stroke({ width: 2, color: LEAD_ROLL, alpha: 0.6 });
      // (Its north-west edges in the parapet's shadow.)
      g.moveTo(lead[2], lead[3]).lineTo(lead[0], lead[1]).lineTo(lead[6], lead[7]);
      g.stroke({ width: PX * 0.4, color: 0x000000, alpha: 0.25 });
    }
    parapetRing(g, pts, rnd);
  };
  for (const p of r.parts) if (p.kind === 3 || p.kind === 4) (p.pts.length === 8 ? keepTop(g, p.pts, p.top & 3, false) : flat(p.pts));
  for (const p of r.parts) {
    if (p.kind !== 0) continue;
    if (p.pts.length === 8)
      drawKeep(
        g,
        p.pts,
        0,
        false,
        r.parts.filter((o) => o !== p).map((o) => o.pts),
        p.top & 3,
        (p.top & DONJON_LOW) !== 0,
      );
    else drawShellKeep(g, p.pts);
  }
  const lift = PX * 0.35;
  for (const p of r.parts) {
    if (p.kind !== 1 && p.kind !== 2) continue;
    const m = p.pts.length / 2;
    let [cx, cy] = [0, 0];
    for (let i = 0; i < m; i++) [cx, cy] = [cx + p.pts[i * 2] / m, cy + p.pts[i * 2 + 1] / m];
    if (p.kind === 1) roundTower(g, cx, cy, Math.hypot(p.pts[0] - cx, p.pts[1] - cy), lift * 1.6, false);
    else squareTowerRoof(g, p.pts, lift * 1.6);
  }
}

/** How far (x, y) stands outside a convex polygon `pts` (px; negative inside, by its nearest edge). */
function polyClear(pts: number[], x: number, y: number): number {
  const m = pts.length / 2;
  let inside = true;
  let sign = 0;
  let d = Infinity;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const [ex, ey] = [bx - ax, by - ay];
    const c = ex * (y - ay) - ey * (x - ax);
    if (c !== 0) {
      if (sign === 0) sign = Math.sign(c);
      else if (Math.sign(c) !== sign) inside = false;
    }
    const l2 = ex * ex + ey * ey || 1;
    const t = Math.max(0, Math.min(1, ((x - ax) * ex + (y - ay) * ey) / l2));
    d = Math.min(d, Math.hypot(ax + ex * t - x, ay + ey * t - y));
  }
  return inside ? -d : d;
}

/**
 * A flat stone roof inside battlements as the curtain's (a keep: `drawKeep`): a flagged top in a
 * parapet with its crenels, a stair hatch in a corner; a round one (a wizard's tower, an
 * observatory: many corners about its middle) a wall tower's round fighting top.
 */
export function drawBattlements(g: Graphics, r: Roof) {
  const { pts, tower } = r;
  const m = pts.length / 2;
  if (r.parts.length) return drawKeepParts(g, r);
  if (tower > 0 && m === 4) return drawKeep(g, pts, tower, true, [], r.top);
  // (A castle's keep with no corner towers, roofed: `keepTop`, its shadow under it.)
  if (r.top !== 0 && m === 4) {
    g.poly(pts.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
    return keepTop(g, pts, r.top, true);
  }
  let cx = 0;
  let cy = 0;
  for (let i = 0; i < m; i++) {
    cx += pts[i * 2] / m;
    cy += pts[i * 2 + 1] / m;
  }
  const rnd = rngAt(cx, cy);
  let [lo, hi] = [Infinity, 0];
  for (let i = 0; i < m; i++) {
    const d = Math.hypot(pts[i * 2] - cx, pts[i * 2 + 1] - cy);
    [lo, hi] = [Math.min(lo, d), Math.max(hi, d)];
  }
  if (m >= 8 && lo > hi * 0.9 && hi > PARAPET * 2.5) return void roundTop(g, cx, cy, hi, rnd, false, [], true);
  flagFloor(g, pts, rnd);
  if (m === 4) {
    const [a, b] = [Math.hypot(pts[2] - pts[0], pts[3] - pts[1]), Math.hypot(pts[4] - pts[2], pts[5] - pts[3])];
    cornerHatch(g, pts, Math.min(a, b) / 2);
  }
  parapetRing(g, pts, rnd);
}
