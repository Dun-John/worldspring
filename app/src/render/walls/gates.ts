import type { Graphics } from 'pixi.js';
import type { Shape } from '../../gen/battlePrep';
import { FT, rngAt, mix, INK, MOSS, PLUM, WOOD } from '../bridges';
import { PX } from '../scale';
import { type At, GATE_ARCHED, GATE_BENT, GATE_LOG, GATE_SQUARE, IRON, LINE, PARAPET, ST, STAKE, TONES, WK, blocks, flagsOf, gateForm, lit, lookOf, rubble, stoneTone, use, setStone } from './base';
import { banner } from './flags';
import { cornerHatch, flagFloor, parapetRing } from './tops';
import { inset, logPoly, logs, stakeTops } from './towers';

/** A dashed line from `p` to `q` (px): `on` drawn, `off` left, for the caller to stroke. */
function dashes(g: Graphics, p: [number, number], q: [number, number], on: number, off: number) {
  const l = Math.hypot(q[0] - p[0], q[1] - p[1]) || 1;
  const [ux, uy] = [(q[0] - p[0]) / l, (q[1] - p[1]) / l];
  for (let t = 0; t < l; t += on + off) g.moveTo(p[0] + ux * t, p[1] + uy * t).lineTo(p[0] + ux * Math.min(l, t + on), p[1] + uy * Math.min(l, t + on));
}

/**
 * A gatehouse of one block (`town::gatehouse`, in the gate's frame: `a` along from the first tower to the
 * second, `b` across into the town, a bent entrance's mirrored to turn toward the second tower): a square gate
 * tower over its passage (the North's; the South's with an arch at each end of the passage), or the Dry's bent
 * entrance, its way turning inside a square block between its two square towers (those are its `Tower`s).
 * The block's top flagged inside its parapet of merlons, the stair's hatch in a corner; the way through under
 * it shown through the top, its sides dashed, a lintel or an arch over each mouth, a portcullis slot inside the
 * outer one, the doors standing half open. A ruin's has no doors, merlons gone, rubble.
 */
/** A one-block gatehouse's frame (ft; see `drawGateBlock`) and its block's outline (px). */
export function blockFrame(s: Shape, form: number) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const side = flagsOf(s, 6) < 0 ? -1 : 1;
  const bent = form === GATE_BENT;
  const mirror = bent && flagsOf(s, 7) < 0;
  const at = (a: number, b: number): [number, number] => {
    const [p, q] = [mirror ? L - a * FT : a * FT, b * FT * side];
    return [x0 + ux * p - uy * q, y0 + uy * p + ux * q];
  };
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const l = L / FT;
  return { at, quad, l, bent, solid: bent ? quad(12, l - 12, -12, 30) : quad(-4, l + 4, -16, 16) };
}

function drawGateBlock(g: Graphics, s: Shape, form: number) {
  const { at, l, bent, solid } = blockFrame(s, form);
  const atPx: At = (a, b) => at(a / FT, b / FT);
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(s.pts[0] + s.pts[2], s.pts[1] + s.pts[3]);
  // The way through (ft), its mouths by edge: the first edge the way in, the other the way out.
  const m = l / 2;
  const way: [number, number][] = bent
    ? [
        [17, -12],
        [27, -12],
        [27, 15],
        [l - 12, 15],
        [l - 12, 25],
        [17, 25],
      ]
    : [
        [m - 6, -16],
        [m + 6, -16],
        [m + 6, 16],
        [m - 6, 16],
      ];
  const mouths = bent ? [0, 3] : [0, 2];
  g.poly(solid.map((v) => v + PX * 1.1)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(solid).fill(ST[1]);
  flagFloor(g, inset(solid, PARAPET), rnd);
  const fallen: number[][] = [];
  parapetRing(g, solid, rnd, 0, [], [], ruined, fallen);
  if (!bent && !ruined) cornerHatch(g, solid, FT * 10);
  // The way under the top, seen through it: paved, its sides dashed.
  const wayPts = way.flatMap(([a, b]) => at(a, b));
  // (A bent way is drawn plain and walled: the turn must read, so it is paved clear of the block's top, its sides inked.)
  g.poly(wayPts).fill(bent ? { color: mix(ST[0], 0xb8ae9a, 0.6), alpha: 0.96 } : { color: mix(ST[0], 0xa39a8a, 0.45), alpha: 0.6 });
  for (let i = 0; i < way.length; i++) {
    if (mouths.includes(i)) continue;
    const j = (i + 1) % way.length;
    if (bent) g.moveTo(...at(...way[i])).lineTo(...at(...way[j]));
    else dashes(g, at(...way[i]), at(...way[j]), FT * 0.9, FT * 0.6);
  }
  g.stroke(bent ? { width: 3.2, color: INK, alpha: 0.95, join: 'round', cap: 'round' } : { width: 2.2, color: INK, alpha: 0.85 });
  if (bent) {
    // A chevron along the way: in, across, out.
    const mid = (p: [number, number], q: [number, number]) => at((p[0] + q[0]) / 2, (p[1] + q[1]) / 2);
    g.moveTo(...mid(way[0], way[1])).lineTo(...at(22, 20)).lineTo(...mid(way[3], way[4])).stroke({ width: 1.4, color: INK, alpha: 0.35 });
  }
  // Over each mouth a lintel, or the South's arch (voussoirs, the keystone lit); a portcullis slot inside the outer.
  mouths.forEach((e, k) => {
    const [p, q] = [way[e], way[(e + 1) % way.length]];
    const n = way[(e + 2) % way.length];
    // (Inward: square to the mouth, toward the way's next corner.)
    const [ex, ey] = [q[0] - p[0], q[1] - p[1]];
    const el = Math.hypot(ex, ey) || 1;
    let [ix, iy] = [-ey / el, ex / el];
    if ((n[0] - q[0]) * ix + (n[1] - q[1]) * iy < 0) [ix, iy] = [-ix, -iy];
    const pt = (t: number, d: number) => at(p[0] + ex * t + ix * d, p[1] + ey * t + iy * d);
    const band: number[] = [];
    const steps = 8;
    for (let i = 0; i <= steps; i++) band.push(...pt(i / steps, 0));
    for (let i = steps; i >= 0; i--) band.push(...pt(i / steps, form === GATE_ARCHED ? 1 + 1.8 * Math.sin((Math.PI * i) / steps) : 2));
    g.poly(band).fill(ST[2]).stroke({ width: LINE * 1.2, color: INK, join: 'round' });
    for (let i = 1; i < steps; i++) {
      if (form === GATE_ARCHED && i === steps / 2) continue;
      const d = form === GATE_ARCHED ? 1 + 1.8 * Math.sin((Math.PI * i) / steps) : 2;
      g.moveTo(...pt(i / steps, 0.2)).lineTo(...pt(i / steps, d - 0.2));
    }
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.8 });
    if (form === GATE_ARCHED) g.poly([...pt(0.44, 0), ...pt(0.56, 0), ...pt(0.56, 2.6), ...pt(0.44, 2.6)]).fill(ST[3]).stroke({ width: LINE, color: INK, join: 'round' });
    if (k === 0) {
      const d = form === GATE_ARCHED ? 3.6 : 3;
      g.poly([...pt(0, d), ...pt(1, d), ...pt(1, d + 0.7), ...pt(0, d + 0.7)]).fill(0x2a2520);
      if (!ruined) g.moveTo(...pt(0.03, d + 0.35)).lineTo(...pt(0.97, d + 0.35)).stroke({ width: FT * 0.22, color: IRON[1] });
    }
  });
  // The doors inside the way in, standing half open.
  if (!ruined) {
    const [a0, a1] = bent ? [17, 27] : [m - 6, m + 6];
    const b0 = bent ? -12 : -16;
    gateDoors(g, atPx, a0 * FT, a1 * FT, (b0 + 5.2) * FT, rnd);
    banner(g, ...at(bent ? (12 + l - 12) / 2 + 4 : m - 10, bent ? 8 : 4));
  } else {
    fallen.push(...rubble(...at(m + (rnd() - 0.5) * 20, (rnd() - 0.5) * 12), 6, FT * 3, rnd));
    for (let k = 0; k < 5; k++) g.ellipse(...at(m + (rnd() - 0.5) * l, (rnd() - 0.5) * 24), FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
  }
  blocks(g, fallen, stoneTone(rnd));
}

/** (R6) A stockade's five-foot piece along x (the atlas's stockade, kind 152): a palisade's stakes from
 * above (`stakeTops`, in timber), three to the piece, a little out of line. */
export function stockadeStakes(g: Graphics, rnd: () => number) {
  const prev = ST;
  setStone(TONES[5]);
  const n = 3;
  const step = PX / n;
  const stakes: [number, number][] = [];
  for (let k = 0; k < n; k++) stakes.push([-PX * 0.5 + (k + 0.5) * step + (rnd() - 0.5) * FT * 0.12, (rnd() - 0.5) * FT * 0.14]);
  stakeTops(g, stakes, STAKE);
  setStone(prev);
}

/**
 * A palisade's log gate (`GATE_LOG`) between its two log towers: packed earth through the gap, a great post at
 * each side, the two leaves (stakes lashed to rails, their tops seen from above) swung open into the town with
 * their swings marked, a plank walk over the gap between the towers, seen through. A ruin's has no leaves or walk,
 * a post gone and logs lying.
 */
function logGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const side = flagsOf(s, 6) < 0 ? -1 : 1;
  const at: At = (a, b) => [x0 + ux * a - uy * b * side, y0 + uy * a + ux * b * side];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const [p0, p1] = [FT * 12, L - FT * 12];
  const D = FT * 6;
  g.poly(quad(p0 - FT, p1 + FT, -D, D)).fill(0xa08c6a);
  const mid = (p0 + p1) / 2;
  for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at(mid + d, -D)).lineTo(...at(mid + d, D));
  g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.12 });
  // The posts, and the leaves hung on them.
  const R = FT * 0.9;
  const posts: [number, number][] = [];
  if (!ruined || rnd() < 0.5) posts.push(at(p0 + R, 0));
  posts.push(at(p1 - R, 0));
  const len = (p1 - p0) / 2 - R;
  const stakes: [number, number][] = [];
  if (!ruined) {
    const open = 1.05 + rnd() * 0.3;
    for (const dir of [1, -1]) {
      const hinge = dir > 0 ? p0 + R : p1 - R;
      const [du, dv] = [dir * Math.cos(open), Math.sin(open)];
      for (let k = 0; k < 7; k++) {
        const [t0, t1] = [(open * k) / 7, (open * (k + 0.55)) / 7];
        g.moveTo(...at(hinge + dir * Math.cos(t0) * len, Math.sin(t0) * len)).lineTo(...at(hinge + dir * Math.cos(t1) * len, Math.sin(t1) * len));
      }
      g.stroke({ width: 2, color: INK, alpha: 0.6 });
      // The rail behind the stakes, then the stakes along it.
      const rail = [...at(hinge + du * R, dv * R + FT * 0.3), ...at(hinge + du * len, dv * len + FT * 0.3)];
      g.moveTo(rail[0], rail[1]).lineTo(rail[2], rail[3]).stroke({ width: FT * 0.5, color: ST[0], cap: 'round' });
      for (let t = R + STAKE; t < len; t += STAKE * 1.9) stakes.push(at(hinge + du * t, dv * t));
    }
    // The walk over the gap, between the towers: planks, see-through, its edges dashed.
    g.poly(quad(p0 - FT * 0.5, p1 + FT * 0.5, -FT * 3.5, -FT * 1)).fill({ color: WK, alpha: 0.5 });
    for (let a = p0 + FT; a < p1; a += FT * 1.1) g.moveTo(...at(a, -FT * 3.5)).lineTo(...at(a, -FT * 1));
    g.stroke({ width: 1.1, color: ST[0], alpha: 0.4 });
    for (const b of [-FT * 3.5, -FT * 1]) dashes(g, at(p0 - FT * 0.5, b), at(p1 + FT * 0.5, b), FT * 0.9, FT * 0.6);
    g.stroke({ width: 2.2, color: INK, alpha: 0.85 });
  }
  for (const [x, y] of posts) g.circle(x + PX * 0.5, y + PX * 0.5, R);
  g.fill({ color: 0x000000, alpha: 0.22 });
  stakeTops(g, posts, R);
  stakeTops(g, stakes, STAKE);
  if (ruined) {
    const lying: number[][] = [];
    for (let k = 0; k < 3; k++) {
      const [cx, cy] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * D * 2);
      const t = rnd() * Math.PI;
      const [vx, vy] = [Math.cos(t), Math.sin(t)];
      lying.push(logPoly((p, q) => [cx + vx * p - vy * q, cy + vy * p + vx * q], -FT * 3, FT * 3, -STAKE, STAKE));
    }
    logs(g, lying, []);
  }
}

/** How far a gatehouse reaches out each side of its curtain's line (ft): where a drawbridge's leaf hangs
 * when play raises it. */
export const GATE_DEPTH = 11;

/** A gatehouse (`ShapeKind::Gate`) by its look and form: one block (`drawGateBlock`), a palisade's log gate
 * (`logGate`), or twin drum towers (`drumGate`), in its stone's tones. */
export function drawGate(g: Graphics, s: Shape) {
  const form = gateForm(s);
  use(lookOf(s, 4));
  if (form === GATE_SQUARE || form === GATE_ARCHED || form === GATE_BENT) drawGateBlock(g, s, form);
  else if (form === GATE_LOG) logGate(g, s);
  else drumGate(g, s);
  use(null);
}

/**
 * A gatehouse (`ShapeKind::Gate`: the two gate towers' centres; `size` 1 for a ruin's): the
 * passage between the towers paved in setts, masonry jambs down its sides, a stone arch across
 * each end casting its shadow in, a portcullis slot inside each arch, the two doors standing
 * half open with their swings marked; over it all the wall walk, see-through, a winch for the
 * portcullis on some. A ruin's has no doors, walk or portcullis, and one arch broken.
 */
function drumGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const at = (a: number, b: number): [number, number] => [x0 + ux * a - uy * b, y0 + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const R = FT * 15;
  const D = FT * GATE_DEPTH;
  const ab = FT * 2.5;
  const [p0, p1] = [R, L - R];
  // The setts of the passage, rows across the way through, and worn ruts.
  g.poly(quad(p0 - FT, p1 + FT, -D, D)).fill(0xa39a8a);
  for (let b = -D + FT * 2; b < D; b += FT * 2) {
    g.moveTo(...at(p0 - FT, b)).lineTo(...at(p1 + FT, b));
    for (let a = p0 - FT + rnd() * FT * 2.5; a < p1 + FT; a += FT * (2.6 + rnd() * 1.2)) g.moveTo(...at(a, b - FT * 2)).lineTo(...at(a, b));
  }
  g.stroke({ width: 1.2, color: 0x7a7266, alpha: 0.8 });
  for (const d of [-FT * 2.4, FT * 2.4]) g.moveTo(...at((p0 + p1) / 2 + d, -D)).lineTo(...at((p0 + p1) / 2 + d, D));
  g.stroke({ width: FT * 1.1, color: PLUM, alpha: 0.1 });
  // The arches' shadows falling into the passage.
  const sh = FT * 1.4;
  for (const b0 of [-D, D - ab]) g.poly(quad(p0 - FT, p1 + FT, b0, b0 + ab).map((v) => v + sh));
  g.fill({ color: 0x000000, alpha: 0.25 });
  // Jambs down the sides, coursed, lit on the side toward the light.
  for (const [a0, a1] of [
    [p0 - FT * 2, p0 + FT * 0.8],
    [p1 - FT * 0.8, p1 + FT * 2],
  ]) {
    g.poly(quad(a0, a1, -D, D)).fill(ST[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let b = -D + FT * 1.8; b < D; b += FT * 1.8) g.moveTo(...at(a0, b)).lineTo(...at(a1, b));
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.8 });
  }
  // Portcullis grooves inside each arch: the slot the raised gate hangs in, its iron foot showing.
  for (const b of [-D + ab + FT * 0.7, D - ab - FT * 0.7]) {
    g.poly(quad(p0, p1, b - FT * 0.35, b + FT * 0.35)).fill(0x2a2520);
    if (!ruined) g.moveTo(...at(p0 + FT * 0.3, b)).lineTo(...at(p1 - FT * 0.3, b)).stroke({ width: FT * 0.22, color: IRON[1] });
  }
  // The arches: voussoirs across the way, the keystone lighter, a sunlit coping edge; a ruin's
  // has one broken partway, its stones fallen in.
  const broken = ruined ? (rnd() < 0.5 ? 0 : 1) : -1;
  const fallen: number[][] = [];
  [-D, D - ab].forEach((b0, k) => {
    const end = k === broken ? p0 - FT + (p1 - p0 + FT * 2) * (0.35 + rnd() * 0.3) : p1 + FT;
    g.poly(quad(p0 - FT, end, b0, b0 + ab)).fill(ST[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let a = p0 - FT + FT * 1.6; a < end - FT * 0.4; a += FT * 1.6) g.moveTo(...at(a, b0)).lineTo(...at(a, b0 + ab));
    g.stroke({ width: 1.2, color: ST[0], alpha: 0.85 });
    const mid = (p0 + p1) / 2;
    if (mid < end) g.poly(quad(mid - FT * 0.7, mid + FT * 0.7, b0, b0 + ab)).fill(ST[2]).stroke({ width: 1.4, color: INK });
    // The coping's lit edge: the long side toward the light.
    const litSide = lit(-uy, ux) > 0 ? b0 + ab - FT * 0.25 : b0 + FT * 0.25;
    g.moveTo(...at(p0 - FT, litSide)).lineTo(...at(end, litSide)).stroke({ width: FT * 0.3, color: ST[3], alpha: 0.7 });
    if (k === broken) fallen.push(...rubble(...at(end + FT * 2, b0 + ab / 2 + (k ? -FT * 2 : FT * 2)), 6, FT * 2, rnd));
  });
  // The wall walk carried over the gate, between the towers: overhead, so shown see-through with
  // its edges dashed; on some gates the portcullis's winch stands on it, chains to the slot.
  if (!ruined) {
    const w = FT * 4;
    const [a0, a1] = [p0 - FT, p1 + FT];
    g.poly(quad(a0, a1, -w, w)).fill({ color: WK, alpha: 0.5 });
    for (const side of [-1, 1]) {
      const [d0, d1] = side < 0 ? [-w, -w + PARAPET] : [w - PARAPET, w];
      g.poly(quad(a0, a1, d0, d1)).fill({ color: ST[2], alpha: 0.6 });
      for (let a = a0 + FT * 1.6; a < a1 - FT * 1.6; a += FT * 3.9) g.poly(quad(a, a + FT * 1.4, d0 + 1.5, d1 - 1.5));
    }
    g.fill({ color: mix(ST[0], PLUM, 0.3), alpha: 0.75 });
    for (const b of [-w, w]) for (let a = a0; a < a1; a += FT * 1.5) g.moveTo(...at(a, b)).lineTo(...at(Math.min(a + FT * 0.9, a1), b));
    g.stroke({ width: 2.5, color: INK, alpha: 0.9 });
    if (rnd() < 0.5) {
      const mid = (p0 + p1) / 2;
      const vb = -w + PARAPET + FT * 1.1;
      const slot = -D + ab + FT * 0.7;
      // Chains down to the portcullis.
      for (const o of [-FT * 1.8, FT * 1.8]) {
        for (let b = vb; b > slot; b -= FT * 0.7) g.moveTo(...at(mid + o, b)).lineTo(...at(mid + o, Math.max(slot, b - FT * 0.45)));
      }
      g.stroke({ width: FT * 0.25, color: IRON[0] });
      // The drum, rope wound on it, a post at each end and a spoked wheel to turn it.
      const drum = quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb + FT * 0.7);
      g.poly(drum.map((v) => v + FT * 0.3)).fill({ color: 0x000000, alpha: 0.25 });
      g.poly(drum).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
      g.poly(quad(mid - FT * 3, mid + FT * 3, vb - FT * 0.7, vb - FT * 0.2)).fill(WOOD[2]);
      for (let a = mid - FT * 2.4; a < mid + FT * 2.5; a += FT * 0.6) g.moveTo(...at(a, vb - FT * 0.7)).lineTo(...at(a, vb + FT * 0.7));
      g.stroke({ width: 1.2, color: WOOD[0] });
      for (const a of [mid - FT * 3.5, mid + FT * 3.5]) g.poly(quad(a - FT * 0.5, a + FT * 0.5, vb - FT * 0.6, vb + FT * 0.6)).fill(WOOD[0]).stroke({ width: 1.6, color: INK });
      const [wx, wy] = at(mid + FT * 4.6, vb);
      for (let k = 0; k < 4; k++) {
        const t = (k / 4) * Math.PI + rnd() * 0.3;
        g.moveTo(wx - Math.cos(t) * FT * 1.3, wy - Math.sin(t) * FT * 1.3).lineTo(wx + Math.cos(t) * FT * 1.3, wy + Math.sin(t) * FT * 1.3);
      }
      g.stroke({ width: FT * 0.3, color: WOOD[1], cap: 'round' });
      g.circle(wx, wy, FT * 1.1).stroke({ width: FT * 0.25, color: WOOD[0] });
      g.circle(wx, wy, FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
    }
  }
  // The doors (drawn over the walk, to be read).
  if (!ruined) gateDoors(g, at, p0, p1, -D + ab + FT * 1.7, rnd);
  if (ruined) {
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * 2 * D);
      g.ellipse(mx, my, FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
    }
    blocks(g, fallen, stoneTone(rnd));
  }
}

/** A gate's doors: two timber leaves hung on the jambs (`p0`, `p1` along, `bh` across in `at`'s frame) just
 * inside the outer portcullis, standing half open into the passage, each with its swing marked. */
function gateDoors(g: Graphics, at: At, p0: number, p1: number, bh: number, rnd: () => number) {
  const len = (p1 - p0) / 2 - FT * 0.2;
  const open = 0.95 + rnd() * 0.3;
  const th = FT * 1.4;
  for (const dir of [1, -1]) {
    const hinge = dir > 0 ? p0 : p1;
    const [du, dv] = [dir * Math.cos(open), Math.sin(open)];
    // Its thickness on the outer side (toward the portcullis).
    const [nu, nv] = [dir * dv, -dir * du];
    const pt = (t: number, k: number) => at(hinge + du * t + nu * k, bh + dv * t + nv * k);
    // The swing: an arc from where the leaf shuts to where it stands, in dashes.
    for (let k = 0; k < 7; k++) {
      const [t0, t1] = [(open * k) / 7, (open * (k + 0.55)) / 7];
      g.moveTo(...at(hinge + dir * Math.cos(t0) * len, bh + Math.sin(t0) * len)).lineTo(...at(hinge + dir * Math.cos(t1) * len, bh + Math.sin(t1) * len));
    }
    g.stroke({ width: 2, color: INK, alpha: 0.6 });
    const leaf = [...pt(0, 0), ...pt(len, 0), ...pt(len, th), ...pt(0, th)];
    g.poly(leaf.map((v) => v + FT * 0.35)).fill({ color: 0x000000, alpha: 0.28 });
    g.poly(leaf).fill(WOOD[1]);
    g.poly([...pt(0, 0), ...pt(len, 0), ...pt(len, th * 0.35), ...pt(0, th * 0.35)]).fill(WOOD[2]);
    // Its boards, end on along the leaf.
    for (const k of [0.35, 0.68]) g.moveTo(...pt(0, th * k)).lineTo(...pt(len, th * k));
    g.stroke({ width: 1.2, color: WOOD[0] });
    // Iron bands across the leaf and the studs along it.
    for (const t of [0.18, 0.5, 0.82]) g.moveTo(...pt(len * t, -1)).lineTo(...pt(len * t, th + 1));
    g.stroke({ width: FT * 0.28, color: IRON[0] });
    g.poly(leaf).stroke({ width: 2, color: INK, join: 'round' });
    g.circle(...pt(0, th * 0.5), FT * 0.35).fill(IRON[1]).stroke({ width: 1.2, color: INK });
  }
}
