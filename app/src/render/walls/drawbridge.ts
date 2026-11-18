import type { Graphics } from 'pixi.js';
import type { Shape } from '../../gen/battlePrep';
import { rngAt, FT, WOOD, INK } from '../bridges';
import { IRON, flagsOf, hull, lit, pairs } from './base';
import { GATE_DEPTH } from './gates';

/**
 * A castle's drawbridge, lowered (`ShapeKind::Drawbridge`: the gate's middle on the curtain's line,
 * the far end on the moat's far bank, then `[wet, 0]`; `size` its width): a heavy timber deck out
 * from under the gatehouse, its planks laid across it between two stout side beams (the sunlit one
 * lit along its top), iron bands across it and an iron-shod lifting edge, nailed; the lifting
 * chains from rings at its far corners back along its beams into the gatehouse (drawn over its
 * inner end); its shadow on the water, or further down on a dry ditch's floor, to the south-east.
 * Inked bold. Its landing on the far bank (`drawDrawbridgeSill`) is drawn apart: it stays when play
 * raises the deck (`drawRaisedDrawbridge`).
 */
export function drawDrawbridge(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const wet = flagsOf(s, 4) === 1;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5;
  const rnd = rngAt(ax + bx, ay + by);
  // `p` along it from the gate (0) to the lifting edge (1), `q` across it (-1 .. 1).
  const P = (p: number, q: number): [number, number] => [ax + ux * L * p + vx * half * q, ay + uy * L * p + vy * half * q];
  const deck = [...P(0, -1), ...P(1, -1), ...P(1, 1), ...P(0, 1)];
  const drop = FT * (wet ? 1.3 : 3.2);
  g.poly(hull(pairs([...deck, ...deck.map((v) => v + drop)]))).fill({ color: 0x000000, alpha: wet ? 0.24 : 0.32 });
  // The planks across it, some a shade lighter, their joints a little out of true.
  const beam = 0.2;
  g.poly(deck).fill(WOOD[1]);
  const n = Math.max(6, Math.round(L / (FT * 1.1)));
  const j = () => (rnd() - 0.5) * 0.25;
  for (let k = 0; k < n; k++) if (rnd() < 0.3) g.poly([...P(k / n, -1), ...P((k + 1) / n, -1), ...P((k + 1) / n, 1), ...P(k / n, 1)]).fill({ color: WOOD[2], alpha: 0.32 });
  for (let k = 1; k < n; k++) {
    const t = k / n + j() / n;
    g.moveTo(...P(t, -1 + beam)).lineTo(...P(t + j() / n, 1 - beam));
  }
  g.stroke({ width: 1.3, color: WOOD[0] });
  // The side beams, the sunlit one's top lit.
  const sun = lit(vx, vy) >= 0 ? 1 : -1;
  for (const q of [-1, 1]) {
    g.poly([...P(0, q), ...P(1, q), ...P(1, q * (1 - beam)), ...P(0, q * (1 - beam))]).fill(q === sun ? WOOD[1] : WOOD[0]);
    g.moveTo(...P(0, q * (1 - beam))).lineTo(...P(1, q * (1 - beam)));
  }
  g.stroke({ width: 1.4, color: INK, alpha: 0.7 });
  g.poly([...P(0, sun * 0.97), ...P(1, sun * 0.97), ...P(1, sun * 0.9), ...P(0, sun * 0.9)]).fill({ color: WOOD[2], alpha: 0.9 });
  // Iron bands across it, and the shod lifting edge; nail heads along them.
  const band = (p0: number, p1: number) => {
    g.poly([...P(p0, -1), ...P(p1, -1), ...P(p1, 1), ...P(p0, 1)]).fill(IRON[0]);
    const pm = (p0 + p1) / 2;
    for (const q of [-0.62, -0.2, 0.2, 0.62]) g.circle(...P(pm, q), 1.2).fill(IRON[2]);
  };
  const t = (FT * 0.5) / L;
  band(0.34 - t * 0.5, 0.34 + t * 0.5);
  band(0.68 - t * 0.5, 0.68 + t * 0.5);
  band(1 - t * 1.4, 1);
  // The chains: from a ring at each far corner back along the beam, links alternately flat and
  // edge-on.
  const ring = 0.86;
  for (const q of [-1, 1]) {
    const [rx, ry] = P(ring, q * (1 - beam * 0.5));
    const links = Math.max(3, Math.round((L * ring) / (FT * 0.9)));
    for (let k = 0; k < links; k++) {
      const [p0, p1] = [ring * (1 - k / links), ring * (1 - (k + 0.8) / links)];
      const [c0, c1] = [P(p0, q * (1 - beam * 0.5)), P(p1, q * (1 - beam * 0.5))];
      if (k % 2) {
        g.moveTo(...c0).lineTo(...c1).stroke({ width: 3.4, color: INK, cap: 'round' });
        g.moveTo(...c0).lineTo(...c1).stroke({ width: 1.6, color: IRON[1], cap: 'round' });
      } else {
        const [mx, my] = [(c0[0] + c1[0]) / 2, (c0[1] + c1[1]) / 2];
        g.circle(mx, my, FT * 0.3).stroke({ width: 2.6, color: INK }).circle(mx, my, FT * 0.3).stroke({ width: 1.2, color: IRON[2] });
      }
    }
    g.circle(rx, ry, FT * 0.42).stroke({ width: 3, color: INK }).circle(rx, ry, FT * 0.42).stroke({ width: 1.4, color: IRON[1] });
  }
  g.poly(deck).stroke({ width: 2.6, color: INK, join: 'round' });
}

/** A drawbridge's landing on the far bank: a timber sill a little wider than the deck, its boards along it. */
export function drawDrawbridgeSill(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5;
  const P = (p: number, q: number): [number, number] => [ax + ux * L * p + vx * half * q, ay + uy * L * p + vy * half * q];
  const sill = (FT * 2.5) / L;
  const land = [...P(1, -1.15), ...P(1 + sill, -1.15), ...P(1 + sill, 1.15), ...P(1, 1.15)];
  g.poly(land).fill(WOOD[0]);
  for (const q of [-0.4, 0.4]) g.moveTo(...P(1 + sill * 0.08, q)).lineTo(...P(1 + sill * 0.92, q));
  g.stroke({ width: 1.1, color: WOOD[2], alpha: 0.6 });
  g.poly(land).stroke({ width: 2.2, color: INK, join: 'round' });
}

/**
 * A drawbridge raised (play: the DM shut it as a door): its leaf stands upright against the gatehouse's
 * outer arch (`GATE_DEPTH` out from the curtain's line), seen from above as a short timber stub across the gateway a little wider than the deck: its outer face
 * toned by the light from the north-west, the planks' ends along its top, the iron-shod lifting edge
 * nailed along it, a chain ring at each end; its tall shadow falls on the moat to the south-east.
 */
export function drawRaisedDrawbridge(g: Graphics, s: Shape) {
  const [ax, ay, bx, by] = s.pts;
  const wet = flagsOf(s, 4) === 1;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  const [vx, vy] = [-uy, ux];
  const half = s.size * 0.5 * 1.08;
  const th = FT * 2.75;
  // `p` out from the gatehouse's outer arch (ft), `q` across it (-1 .. 1).
  const out = FT * GATE_DEPTH;
  const P = (p: number, q: number): [number, number] => [ax + ux * (out + p) + vx * half * q, ay + uy * (out + p) + vy * half * q];
  const slab = [...P(0, -1), ...P(th, -1), ...P(th, 1), ...P(0, 1)];
  const drop = FT * (wet ? 5 : 7);
  g.poly(hull(pairs([...slab, ...slab.map((v) => v + drop)]))).fill({ color: 0x000000, alpha: wet ? 0.26 : 0.32 });
  // The top, then the outer face's strip in the light's tone.
  g.poly(slab).fill(WOOD[1]);
  const face = lit(ux, uy);
  g.poly([...P(th * 0.62, -1), ...P(th, -1), ...P(th, 1), ...P(th * 0.62, 1)]).fill(face > 0.3 ? WOOD[2] : face < -0.3 ? WOOD[0] : WOOD[1]);
  // The planks' ends along the top.
  const n = Math.max(4, Math.round((half * 2) / (FT * 1.1)));
  const rnd = rngAt(ax + bx, ay + by);
  for (let k = 1; k < n; k++) {
    const q = -1 + (2 * k) / n + (rnd() - 0.5) * 0.04;
    g.moveTo(...P(th * 0.08, q)).lineTo(...P(th * 0.62, q));
  }
  g.stroke({ width: 1.2, color: WOOD[0] });
  // The iron-shod lifting edge along the top, nailed.
  g.poly([...P(th * 0.24, -0.97), ...P(th * 0.46, -0.97), ...P(th * 0.46, 0.97), ...P(th * 0.24, 0.97)]).fill(IRON[0]);
  for (let k = 0; k < n; k++) g.circle(...P(th * 0.35, -1 + (2 * k + 1) / n), 1.1).fill(IRON[2]);
  g.poly(slab).stroke({ width: 2.6, color: INK, join: 'round' });
  // A chain ring at each end, back toward the gatehouse.
  for (const q of [-1, 1]) {
    const [rx, ry] = P(-FT * 0.15, q * 0.9);
    g.circle(rx, ry, FT * 0.38).stroke({ width: 3, color: INK }).circle(rx, ry, FT * 0.38).stroke({ width: 1.4, color: IRON[1] });
  }
}
