import { PX } from '../scale';
import { mix, STONE, PLUM, rngAt, FT, MOSS, INK, LX, LY, WOOD } from '../bridges';
import type { Graphics } from 'pixi.js';
import type { Shape } from '../../gen/battlePrep';
import { IRON, hull, lit, pairs } from './base';

/** A garden wall's height (ft): its shadow's reach. */
const GARDEN_FT = 4;
/** A gate pier's half side (px; `battlemap`'s, 7-ft piers): where a forecourt gate's leaves hang. */
const PIER_HALF = PX * 0.7;

/** A coping stone's tone as its face meets the light (`l`, -1 .. 1): sunlit toward cream, shaded toward plum. */
const coping = (l: number, rnd: () => number) => {
  const t = l >= 0 ? mix(STONE[2], STONE[3], l * 0.7) : mix(STONE[2], mix(STONE[1], PLUM, 0.25), -l * 0.85);
  return mix(t, rnd() < 0.5 ? STONE[1] : STONE[3], rnd() * 0.18);
};

/**
 * A forecourt's low garden wall (`ShapeKind::GardenWall`, one straight segment each, `size` thick):
 * a short soft shadow (it stands about 4 ft), a coping of flat stones along its top seen straight down,
 * joints across it, the odd tuft of moss; a
 * bold ink outline. Segments meeting end to end are mitred into one neat corner; a free end (at a
 * gate pier or the building's wall) is cut square.
 */
export function drawGardenWalls(g: Graphics, walls: Shape[]) {
  const segs = walls.map((s) => {
    const [x0, y0, x1, y1] = s.pts;
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    return { x0, y0, x1, y1, l, ux: (x1 - x0) / l, uy: (y1 - y0) / l, half: s.size / 2, rnd: rngAt(x0 * 0.7 + x1, y0 * 0.7 + y1) };
  });
  type Seg = (typeof segs)[number];
  // Each end's two corners (on the segment's left, +n, and right) and whether it is mitred.
  const ends = segs.map((s) => {
    const n = [-s.uy, s.ux];
    const out: { left: [number, number]; right: [number, number]; mitred: boolean }[] = [];
    for (const end of [0, 1]) {
      const [px, py] = end ? [s.x1, s.y1] : [s.x0, s.y0];
      let corner = { left: [px + n[0] * s.half, py + n[1] * s.half] as [number, number], right: [px - n[0] * s.half, py - n[1] * s.half] as [number, number], mitred: false };
      const o = segs.find((t) => t !== s && (Math.hypot(t.x0 - px, t.y0 - py) < 1.5 || Math.hypot(t.x1 - px, t.y1 - py) < 1.5));
      if (o) {
        // Heading into the corner along this one, out of it along the other.
        const din = end ? [s.ux, s.uy] : [-s.ux, -s.uy];
        const dout = Math.hypot(o.x0 - px, o.y0 - py) < 1.5 ? [o.ux, o.uy] : [-o.ux, -o.uy];
        const [ax, ay, bx, by] = [-din[1], din[0], -dout[1], dout[0]];
        const ml = Math.hypot(ax + bx, ay + by);
        if (ml > 0.2) {
          const [mx, my] = [(ax + bx) / ml, (ay + by) / ml];
          const k = Math.min(s.half * 3, s.half / (mx * ax + my * ay));
          const q: [number, number] = [px + mx * k, py + my * k];
          const q2: [number, number] = [px - mx * k, py - my * k];
          corner = end ? { left: q, right: q2, mitred: true } : { left: q2, right: q, mitred: true };
        }
      }
      out.push(corner);
    }
    return out;
  });
  // A point `a` along a segment and `d` across it (toward its left).
  const at = (s: Seg, a: number, d: number): [number, number] => [s.x0 + s.ux * a - s.uy * d, s.y0 + s.uy * a + s.ux * d];
  const outline = (k: number) => {
    const [A, B] = ends[k];
    return [...A.right, ...B.right, ...B.left, ...A.left];
  };
  // The shadow, short and soft (two slices).
  const reach = GARDEN_FT * FT * 0.55;
  for (const [f, alpha] of [
    [1, 0.12],
    [0.55, 0.16],
  ]) {
    for (let k = 0; k < segs.length; k++) g.poly(outline(k).map((v) => v + reach * f));
    g.fill({ color: 0x000000, alpha });
  }
  // The coping in blocks, one flat top each, joints between.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    const l = lit(-s.uy, s.ux);
    // The joints, clear of the ends' mitres.
    const margin = s.half * 1.4;
    const cuts: number[] = [0];
    for (let a = margin + s.rnd() * FT * 1.6; a < s.l - margin; a += FT * (2.2 + s.rnd() * 1.4)) cuts.push(a);
    cuts.push(s.l);
    const edge = (i: number, side: 1 | -1): [number, number] => {
      if (i === 0) return side > 0 ? A.left : A.right;
      if (i === cuts.length - 1) return side > 0 ? B.left : B.right;
      return at(s, cuts[i] + (s.rnd() - 0.5) * FT * 0.25, side * s.half);
    };
    const lefts = cuts.map((_, i) => edge(i, 1));
    const rights = cuts.map((_, i) => edge(i, -1));
    for (let i = 0; i + 1 < cuts.length; i++) {
      // Seen straight down: one flat top (no second face to read as the wall's side), the sun only warming it.
      g.poly([...lefts[i], ...lefts[i + 1], ...rights[i + 1], ...rights[i]]).fill(coping(l * 0.3, s.rnd));
    }
    // The sunlit arris along the edge toward the light.
    const sun = l >= 0 ? 1 : -1;
    g.moveTo(...at(s, A.mitred ? s.half : 0, sun * (s.half - FT * 0.28))).lineTo(...at(s, B.mitred ? s.l - s.half : s.l, sun * (s.half - FT * 0.28)));
    g.stroke({ width: FT * 0.35, color: STONE[3], alpha: 0.55 * Math.abs(l) });
    // The joints across it, a mitred end's joint too.
    for (let i = 1; i + 1 < cuts.length; i++) g.moveTo(...lefts[i]).lineTo(...rights[i]);
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.95, join: 'round' });
    for (const [E, P] of [
      [A, [s.x0, s.y0]],
      [B, [s.x1, s.y1]],
    ] as const) {
      if (E.mitred) g.moveTo(...E.left).lineTo(...P).lineTo(...E.right);
    }
    g.stroke({ width: 1.6, color: STONE[0], alpha: 0.9, join: 'round' });
    // The odd tuft of moss on the shaded face, or at the wall's foot.
    const tufts = Math.floor(s.l / (FT * 30) + s.rnd() * 0.6);
    for (let t = 0; t < tufts; t++) {
      const a = s.half * 1.5 + s.rnd() * Math.max(0, s.l - s.half * 3);
      const [mx, my] = at(s, a, -sun * s.half * (0.35 + s.rnd() * 0.25));
      const r = FT * (0.22 + s.rnd() * 0.14);
      g.ellipse(mx, my, r * 1.5, r).fill({ color: MOSS[0], alpha: 0.75 });
      g.ellipse(mx - r * 0.35, my - r * 0.3, r * 0.75, r * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  });
  // Bold ink round it all: the long sides and the square ends, not across a mitre.
  segs.forEach((s, k) => {
    const [A, B] = ends[k];
    g.moveTo(...A.left).lineTo(...B.left);
    if (!B.mitred) g.lineTo(...B.right);
    else g.moveTo(...B.right);
    g.lineTo(...A.right);
    if (!A.mitred) g.lineTo(...A.left);
  });
  g.stroke({ width: 2.6, color: INK, join: 'round', cap: 'round' });
}

/**
 * A forecourt gate's pier (`ShapeKind::GatePier`: its middle, then its sides' angle; `size` half its
 * side): a solid square of stone seen from above, its shadow to the south-east; the cap's fillet,
 * then its weathered slopes stepping in (lit on the faces toward the light, shaded on the others),
 * its flat top and a ball finial on a little plinth, the ball shaded round and casting its own shadow.
 */
export function drawGatePier(g: Graphics, s: Shape) {
  const [x, y, h] = [s.pts[0], s.pts[1], s.size];
  const t = s.pts.length > 2 ? s.pts[2] / PX : 0;
  const [ux, uy] = [Math.cos(t), Math.sin(t)];
  const rnd = rngAt(x * 1.7 + y * 0.23, y * 0.9 - x * 0.31);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  // A square `r` from the middle, its corners a little out of true.
  const square = (r: number, j = 0.6) => {
    const out: number[] = [];
    for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) out.push(...at(a * r + (rnd() - 0.5) * j, b * r + (rnd() - 0.5) * j));
    return out;
  };
  // The shadow: the block swept away from the light.
  const D = PX * 0.38;
  const foot = square(h, 0);
  g.poly(hull(pairs([...foot, ...foot.map((v) => v + D)]))).fill({ color: 0x000000, alpha: 0.22 });
  // The fillet, the widest of the cap.
  g.poly(square(h)).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  g.poly(square(h * 0.9)).fill(STONE[2]);
  // The weathering: four slopes from the fillet up to the top, each as it faces the light.
  const a0 = square(h * 0.84, 0.4);
  const a1 = square(h * 0.58, 0.4);
  for (let k = 0; k < 4; k++) {
    const m = (k + 1) % 4;
    const face = [a0[k * 2], a0[k * 2 + 1], a0[m * 2], a0[m * 2 + 1], a1[m * 2], a1[m * 2 + 1], a1[k * 2], a1[k * 2 + 1]];
    // Its outward way: from the middle to its outer edge's midpoint.
    const l = lit((a0[k * 2] + a0[m * 2]) / 2 - x, (a0[k * 2 + 1] + a0[m * 2 + 1]) / 2 - y);
    g.poly(face).fill(l >= 0 ? mix(STONE[2], STONE[3], 0.3 + l * 0.6) : mix(STONE[1], PLUM, 0.12 - l * 0.28));
  }
  g.poly(a0).stroke({ width: 1.6, color: INK, alpha: 0.85, join: 'round' });
  for (let k = 0; k < 4; k++) g.moveTo(a0[k * 2], a0[k * 2 + 1]).lineTo(a1[k * 2], a1[k * 2 + 1]);
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.8 });
  // The flat top, and the finial's plinth on it.
  g.poly(a1).fill(STONE[2]).stroke({ width: 1.6, color: INK, alpha: 0.9, join: 'round' });
  const pl = square(h * 0.36, 0.3);
  g.poly(pl.map((v) => v + h * 0.12)).fill({ color: PLUM, alpha: 0.25 });
  g.poly(pl).fill(STONE[1]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.poly(square(h * 0.28, 0.2)).fill(mix(STONE[2], STONE[1], 0.3));
  // The ball: its shadow on the top, a shaded round lit toward the light, a soft sheen (stone: matte).
  const r = h * 0.29;
  g.circle(x + r * 0.45, y + r * 0.45, r).fill({ color: PLUM, alpha: 0.3 });
  const ball = (k: number, ox: number, oy: number) => {
    const pts: number[] = [];
    for (let i = 0; i < 18; i++) {
      const a = (i / 18) * Math.PI * 2;
      const rr = r * k * (1 + (rnd() - 0.5) * 0.05);
      pts.push(x + ox + Math.cos(a) * rr, y + oy + Math.sin(a) * rr);
    }
    return pts;
  };
  const round = ball(1, 0, 0);
  g.poly(round).fill(mix(STONE[1], PLUM, 0.18));
  g.poly(ball(0.78, LX * r * 0.2, LY * r * 0.2)).fill(STONE[2]);
  g.poly(ball(0.4, LX * r * 0.38, LY * r * 0.38)).fill({ color: STONE[3], alpha: 0.85 });
  g.poly(round).stroke({ width: 2, color: INK, join: 'round' });
}

/**
 * A forecourt's gate (`ShapeKind::ForecourtGate`: the opening's two ends at the piers' inner faces,
 * then a point inside; `size` its style, 0 wrought iron, 1 timber), seen from above as the map's other
 * doors and gates are: two leaves standing open, each hung on its pier's inner corner and swung into
 * the forecourt a little past square, the pair mirrored about the gate's axis and turned with it,
 * drawn over the piers' edges, each with its swing marked in dashes. A timber leaf is the top of its
 * boards, their joints across it, iron straps from the hinge along its faces; a wrought-iron one its
 * top rail, a spear finial on every bar and a C-scroll on each face, a stile post at the hinge and a
 * knob at the meeting end. Soft shadows to the south-east, an iron leaf's faint (it is open work).
 */
export function drawForecourtGate(g: Graphics, s: Shape) {
  const [ax, ay, bx, by, ix, iy] = s.pts;
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  // Into the forecourt.
  const inward = (ix - ax) * -uy + (iy - ay) * ux >= 0 ? 1 : -1;
  const timber = Math.round(s.size / PX) === 1;
  gateLeaves(g, ax, ay, bx, by, [-uy * inward, ux * inward], timber, PIER_HALF, rngAt(ax + bx, ay + by));
}

/** (R6) A fence's or wall's gate, five feet along x (the atlas's gate, kind 155): the forecourt gate's two
 * leaves (`gateLeaves`) hung on the opening's ends and swung open toward +y, timber (looks 0 and 2, 2 the
 * heavier) or plain iron bars as the railing's (look 1), each end's post top seen from above under its hinge. */
export function fenceGate(g: Graphics, look: number, rnd: () => number) {
  const h = PX * 0.5;
  const timber = look !== 1;
  // The posts' tops: squares of timber, or of iron for an iron gate.
  for (const x of [-h, h]) {
    const r = FT * (timber ? 0.45 : 0.35);
    g.rect(x - r, -r, r * 2, r * 2).fill(timber ? WOOD[1] : IRON[1]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(x - r + 1.2, -r + 1.2, r * 0.9, r * 0.9).fill(timber ? WOOD[2] : IRON[2]);
  }
  gateLeaves(g, -h, 0, h, 0, [0, 1], timber, 0, rnd, look === 2 ? 1.2 : 1, true);
}

/**
 * A gate's two leaves from above, the opening from `(ax, ay)` to `(bx, by)`, swung open toward `v` (a unit
 * vector across it): each hung `inset` px toward `v` from its end (a forecourt's on its pier's inner corner,
 * a fence's on its post), a little past square, mirrored about the gate's axis, its swing marked in dashes, a
 * soft shadow to the south-east; timber or wrought iron (`timberLeaf`, `ironLeaf`), `thick` times the usual;
 * `bars`: an iron leaf plain, as the railing it stands in (`barLeaf`).
 */
function gateLeaves(g: Graphics, ax: number, ay: number, bx: number, by: number, [vx, vy]: number[], timber: boolean, inset: number, rnd: () => number, thick = 1, bars = false) {
  const L = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / L, (by - ay) / L];
  // Its thickness from above, and its length (the two shut meet in the middle).
  const th = FT * (timber ? 1 : 0.8) * thick;
  const len = L / 2 - FT * 0.2;
  // Open a little past square, both alike.
  const open = (104 * Math.PI) / 180;
  for (const end of [0, 1]) {
    const dir = end ? -1 : 1;
    // Across the opening toward the other leaf (`dir`), swung toward `v`.
    const way = (t: number): [number, number] => [ux * dir * Math.cos(t) + vx * Math.sin(t), uy * dir * Math.cos(t) + vy * Math.sin(t)];
    // The hinge on the pier's inner corner, the leaf's face flush with the pier's face to the opening.
    const [px, py] = end ? [bx, by] : [ax, ay];
    const [hx, hy] = [px + vx * (inset - th * 0.5) + ux * dir * th * 0.5, py + vy * (inset - th * 0.5) + uy * dir * th * 0.5];
    const [dx, dy] = way(open);
    // Across the leaf, from face to face.
    const [nx, ny] = [-dy, dx];
    // A point `p` of the way along the leaf (0 at the hinge) and `q` across it (-1 .. 1, face to face).
    const P = (p: number, q: number): [number, number] => [hx + dx * len * p + nx * th * 0.5 * q, hy + dy * len * p + ny * th * 0.5 * q];
    // The swing: an arc from where the leaf shuts to where it stands, in dashes.
    for (let k = 0; k < 8; k++) {
      const [t0, t1] = [(open * k) / 8, (open * (k + 0.5)) / 8];
      const [c0, c1] = [way(t0), way(t1)];
      g.moveTo(hx + c0[0] * len, hy + c0[1] * len).lineTo(hx + c1[0] * len, hy + c1[1] * len);
    }
    g.stroke({ width: 1.6, color: INK, alpha: 0.45 });
    // Its shadow, swept away from the light.
    const outline = [...P(0, -1), ...P(1, -1), ...P(1, 1), ...P(0, 1)];
    g.poly(hull(pairs([...outline, ...outline.map((v) => v + FT * (timber ? 1.5 : 1.1))]))).fill({ color: 0x000000, alpha: timber ? 0.2 : 0.08 });
    // (Which face is toward the light: +1 or -1.)
    const sun = lit(nx, ny) >= 0 ? 1 : -1;
    if (timber) timberLeaf(g, P, len, sun, rnd);
    else if (bars) barLeaf(g, P);
    else ironLeaf(g, P, len, th, sun);
  }
}

/** A plain iron gate leaf from above, as an iron railing is (`P(p, q)`: along it from the hinge, across it):
 * its top rail a dark bar, three bars' round tops along it. */
function barLeaf(g: Graphics, P: (p: number, q: number) => [number, number]) {
  g.moveTo(...P(0, 0)).lineTo(...P(1, 0)).stroke({ width: FT * 0.28 + 2.4, color: INK, cap: 'round' });
  g.moveTo(...P(0, 0)).lineTo(...P(1, 0)).stroke({ width: FT * 0.28, color: IRON[1], cap: 'round' });
  for (const p of [0.3, 0.62, 0.94]) g.circle(...P(p, 0), FT * 0.27).fill(IRON[1]).stroke({ width: 1.5, color: INK });
  g.circle(...P(0, 0), FT * 0.3).fill(IRON[2]).stroke({ width: 1.4, color: INK });
}

/** A timber gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face; `sun`
 * the face toward the light): the top of its boards, a lit band along the sunlit face, the joints
 * between the boards across it, iron straps from the hinge along both faces, nailed; its hinge pin. */
function timberLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, sun: number, rnd: () => number) {
  // A little out of true: hand-made.
  const j = () => (rnd() - 0.5) * 0.12;
  const panel = [...P(0, -1), ...P(0.5, -1 + j()), ...P(1, -1), ...P(1, 1), ...P(0.5, 1 + j()), ...P(0, 1)];
  g.poly(panel).fill(WOOD[1]);
  g.poly([...P(0.02, sun * 0.95), ...P(0.98, sun * 0.95), ...P(0.98, sun * 0.25), ...P(0.02, sun * 0.25)]).fill({ color: WOOD[2], alpha: 0.85 });
  // The boards, their joints across the leaf, some a shade lighter.
  const n = Math.max(4, Math.round(len / (FT * 1.05)));
  for (let k = 0; k < n; k++) if (rnd() < 0.35) g.poly([...P(k / n, -0.9), ...P((k + 1) / n, -0.9), ...P((k + 1) / n, 0.9), ...P(k / n, 0.9)]).fill({ color: WOOD[2], alpha: 0.3 });
  for (let k = 1; k < n; k++) g.moveTo(...P(k / n + j() * 0.2, -0.95)).lineTo(...P(k / n + j() * 0.2, 0.95));
  g.stroke({ width: 1.3, color: WOOD[0] });
  // The straps along both faces from the hinge, pointed at their ends, a nail or two.
  for (const q of [-1, 1]) {
    g.poly([...P(0, q * 1.08), ...P(0.6, q * 1.05), ...P(0.66, q * 0.95), ...P(0, q * 0.82)]).fill(IRON[0]);
    for (const p of [0.2, 0.45]) g.circle(...P(p, q * 0.95), 1.2).fill(IRON[2]);
  }
  g.poly(panel).stroke({ width: 2.2, color: INK, join: 'round' });
  g.circle(...P(0, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
}

/** A wrought-iron gate leaf from above (`P(p, q)`: along it from the hinge, across it face to face;
 * `th` its depth, `sun` the face toward the light): the top rail, lit along its sunlit edge; a spear
 * finial on every bar, a lozenge across the rail; a C-scroll on each face, a third and two thirds of
 * the way along; a square stile post at the hinge with its pin, a knob on the meeting stile. Iron in
 * ink, each line drawn over a wider one of ink. */
function ironLeaf(g: Graphics, P: (p: number, q: number) => [number, number], len: number, th: number, sun: number) {
  const iron = (draw: () => void, w: number, c: number) => {
    draw();
    g.stroke({ width: w + 1.8, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color: c, cap: 'round', join: 'round' });
  };
  const n = Math.max(5, Math.round(len / (FT * 0.95)));
  // The scrolls: a C-scroll on each face, a third of the way along on one and two thirds on the
  // other, curling from the rail out and back toward the hinge.
  iron(() => {
    for (const [p0, side] of [
      [0.36, -1],
      [0.68, 1],
    ]) {
      const r = th * 0.42;
      const [cx, cy] = P(p0, side * (1 + (2 * r) / th));
      const a0 = Math.atan2(...((): [number, number] => {
        const [ox, oy] = P(p0, side);
        return [oy - cy, ox - cx];
      })());
      for (let i = 0; i <= 14; i++) {
        const a = a0 + side * (i / 14) * Math.PI * 1.7;
        const rr = r * (1 - i * 0.04);
        if (i) g.lineTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
        else g.moveTo(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
      }
    }
  }, 1.3, IRON[1]);
  // The top rail, hinge to meeting stile.
  iron(() => {
    g.moveTo(...P(0, 0)).lineTo(...P(1, 0));
  }, th * 0.5, IRON[0]);
  g.moveTo(...P(0.04, sun * 0.2)).lineTo(...P(0.96, sun * 0.2)).stroke({ width: 0.9, color: IRON[2], alpha: 0.85 });
  // The bars' spear finials: lozenges across the rail.
  for (let k = 1; k < n; k++) {
    const p = k / n;
    const d = Math.min(0.06, 0.3 / n);
    g.poly([...P(p - d, 0), ...P(p, -1.1), ...P(p + d, 0), ...P(p, 1.1)]).fill(IRON[1]).stroke({ width: 1, color: INK, join: 'round' });
    g.circle(...P(p, 0), Math.max(1, th * 0.12)).fill(IRON[2]);
  }
  // The hinge stile's square post and pin, the meeting stile's knob.
  g.poly([...P(-0.03, -1.1), ...P(0.07, -1.1), ...P(0.07, 1.1), ...P(-0.03, 1.1)]).fill(IRON[0]).stroke({ width: 1.4, color: INK, join: 'round' });
  g.circle(...P(0.02, 0), FT * 0.2).fill(IRON[2]).stroke({ width: 1, color: INK });
  g.circle(...P(1, 0), FT * 0.28).fill(IRON[1]).stroke({ width: 1.2, color: INK });
}
