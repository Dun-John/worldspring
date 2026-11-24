import { FT, rngAt, INK, mix, LX, LY, PLUM, MOSS, WOOD } from '../bridges';
import type { Graphics } from 'pixi.js';
import type { Shape } from '../../gen/battlePrep';
import { PX } from '../scale';
import { type At, D_BACK, GATE, GATE_ARCHED, GATE_SQUARE, LINE, type Look, MAT_EARTH, MAT_PALISADE, OUT, PARAPET, RUINED, SHAPE_D, SHAPE_OCTAGON, SHAPE_SQUARE, SQUARE, ST, WK, blocks, breach, crenelFill, crenelRun, flagsOf, gateForm, lit, lookOf, rubble, stoneTone, use } from './base';
import { banner } from './flags';
import { flagFloor, roundTop, stairHatch } from './tops';

/** Packed earth on a bastion's top: dark, mid, light (three close matte tones). */
const EARTH_TOP = [0xa48d6a, 0xbca580, 0xd0bc96];
/** The ramp up onto a bastion from the town (`town::wallside::RAMP_FT`, `RAMP_HALF`): ft out from the corner, half
 * its width. */
const RAMP_FT = 10;
const RAMP_HALF = 6.5;
/** How deep the level landings at the ramp's foot and head are (ft). */
const RAMP_LAND = 3;

/** A bastion's ramp (px): its foot along the open back (on either side of the corner `v`), its head out along the
 * middle line: foot on one side, the corner, foot on the other side, head, head. */
export function bastionRamp(pts: number[]): number[] {
  const [vx, vy] = [pts[10], pts[11]];
  let [ox, oy] = [pts[4] - vx, pts[5] - vy];
  const lo = Math.hypot(ox, oy) || 1;
  [ox, oy] = [ox / lo, oy / lo];
  const [sx, sy] = [-oy, ox];
  const [hw, len] = [RAMP_HALF * FT, RAMP_FT * FT];
  // (Where the ramp's side, `side` of the middle line, meets the back edge on that side: v to f1 or v to f2.)
  const foot = (side: number): [number, number] => {
    for (const f of [0, 8]) {
      const [dx, dy] = [pts[f] - vx, pts[f + 1] - vy];
      const lat = dx * sx + dy * sy;
      if (lat * side <= 0) continue;
      const t = Math.min(1, hw / Math.abs(lat));
      return [vx + dx * t, vy + dy * t];
    }
    return [vx + sx * hw * side, vy + sy * hw * side];
  };
  const [a, b] = [foot(-1), foot(1)];
  return [...a, vx, vy, ...b, vx + ox * len + sx * hw, vy + oy * len + sy * hw, vx + ox * len - sx * hw, vy + oy * len - sy * hw];
}

/**
 * A star bastion's top (`Layout::bastions`: f1, g1, tip, g2, f2, the corner, px, then its style): an earth terreplein
 * (packed dirt, a worn track across it, a few stones), held up along its open back by a low stone revetment that
 * casts its shadow on the town, with a ramp cut up into it from the town at the corner. Its curtain (drawn over its
 * edge by `drawWalls`) keeps its walk and merlons; its props (a ballista, shot, crates) are battlemap objects.
 */
export function drawBastion(g: Graphics, s: Shape) {
  const pts = s.pts.slice(0, 12);
  use(lookOf(s, 12));
  const rnd = rngAt(pts[4], pts[5]);
  const [vx, vy] = [pts[10], pts[11]];
  let [ox, oy] = [pts[4] - vx, pts[5] - vy];
  const depth = Math.hypot(ox, oy) || 1;
  [ox, oy] = [ox / depth, oy / depth];
  const [sx, sy] = [-oy, ox];
  const at = (a: number, l: number): [number, number] => [vx + ox * a + sx * l, vy + oy * a + sy * l];
  // The earth, and a few broad patches of it lighter and darker (no lines across it).
  g.poly(pts).fill(EARTH_TOP[1]);
  const patch = (cx: number, cy: number, r: number, tone: number, alpha: number) => {
    const n = 9;
    const q: number[] = [];
    for (let k = 0; k < n; k++) {
      const t = (k / n) * Math.PI * 2;
      const rr = r * (0.75 + rnd() * 0.4);
      q.push(cx + Math.cos(t) * rr * 1.3, cy + Math.sin(t) * rr);
    }
    g.poly(q).fill({ color: tone, alpha });
  };
  for (let k = 0; k < 3; k++) {
    const [px, py] = at(depth * (0.35 + rnd() * 0.4), (rnd() - 0.5) * depth * 0.5);
    patch(px, py, FT * (3 + rnd() * 2.5), k === 0 ? EARTH_TOP[0] : EARTH_TOP[2], k === 0 ? 0.3 : 0.45);
  }
  // A worn track from the ramp's head toward the point, lighter.
  const [t0x, t0y] = at(RAMP_FT * FT, 0);
  const [t1x, t1y] = at(depth * 0.72, (rnd() - 0.5) * FT * 2);
  g.moveTo(t0x, t0y).quadraticCurveTo((t0x + t1x) / 2 + sx * FT, (t0y + t1y) / 2 + sy * FT, t1x, t1y).stroke({ width: FT * 3.2, color: EARTH_TOP[2], alpha: 0.55, cap: 'round' });
  // Shadow of the curtain's parapet on the earth, along the faces away from the light.
  for (let i = 0; i < 4; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[i * 2 + 2], pts[i * 2 + 3]];
    const [mx, my] = [(ax + bx) / 2 - vx, (ay + by) / 2 - vy];
    if (lit(mx, my) < 0.1) continue;
    const l = Math.hypot(bx - ax, by - ay) || 1;
    let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
    if (nx * mx + ny * my > 0) [nx, ny] = [-nx, -ny];
    const w = s.size / 2 + FT * 1.2;
    g.poly([ax, ay, bx, by, bx + nx * w, by + ny * w, ax + nx * w, ay + ny * w]).fill({ color: EARTH_TOP[0], alpha: 0.2 });
  }
  // A few stones in the earth.
  for (let k = 0; k < 4; k++) {
    const [px, py] = at(depth * (0.3 + rnd() * 0.5), (rnd() - 0.5) * depth * 0.7);
    if (!inside(pts, px, py)) continue;
    const r = FT * (0.35 + rnd() * 0.25);
    g.ellipse(px, py, r * 1.2, r).fill(ST[2]).stroke({ width: LINE * 0.8, color: INK });
  }
  // The ramp: a flight of stone steps cut up into the earth from the town, a level landing at its foot (on the
  // town's ground) and at its head (on the top); each step's nose inked, lit on its tread, its riser's shade below.
  const ramp = bastionRamp(pts);
  const hw = RAMP_HALF * FT;
  const [fa, fb] = [ramp.slice(0, 2), ramp.slice(4, 6)];
  // (Which foot stands on the back edge from the corner to f1, which on the one to f2.)
  const onF1 = Math.abs((fa[0] - vx) * (pts[1] - vy) - (fa[1] - vy) * (pts[0] - vx)) < Math.abs((fb[0] - vx) * (pts[1] - vy) - (fb[1] - vy) * (pts[0] - vx));
  const [foot1, foot2] = onF1 ? [fa, fb] : [fb, fa];
  const footAx = (p: number[]) => (p[0] - vx) * ox + (p[1] - vy) * oy;
  const start = Math.min(footAx(fa), footAx(fb));
  const [len, land, cope] = [RAMP_FT * FT, RAMP_LAND * FT, FT * 1.3];
  // (The landing at the foot: flags on the town's ground along the ramp's foot.)
  const below = [...fa, vx, vy, ...fb, fb[0] - ox * land, fb[1] - oy * land, fa[0] - ox * land, fa[1] - oy * land];
  g.poly(below).fill(ST[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(start - land * 0.5, -hw)).lineTo(...at(start - land * 0.5, hw)).stroke({ width: LINE * 0.7, color: ST[0], alpha: 0.7 });
  // (The landing at the head: flags level with the top.)
  const above = [...at(len, -hw), ...at(len, hw), ...at(len + land, hw), ...at(len + land, -hw)];
  g.poly(above).fill(ST[2]);
  g.moveTo(...at(len + land * 0.5, -hw + cope)).lineTo(...at(len + land * 0.5, hw - cope)).stroke({ width: LINE * 0.7, color: ST[0], alpha: 0.7 });
  g.poly(above).stroke({ width: LINE, color: INK, join: 'round' });
  // (The flight: its floor, then each step's riser shade, lit tread and inked nose, clipped to the ramp: its sides,
  // and the back edges near its foot.)
  g.poly(ramp).fill(ST[1]);
  const lat = (a: number, side: number) => {
    const f = side < 0 ? fa : fb;
    const fAx = footAx(f);
    if (a >= fAx) return side * (hw - cope);
    return side * Math.max(0, (hw * Math.max(0, a)) / Math.max(1e-6, fAx) - cope);
  };
  // (Few, big steps: about 3.5 ft deep, a dark riser band under each nose, the tread lit and light.)
  const pitch = (len - start) / Math.max(3, Math.round((len - start) / (FT * 3.5)));
  const steps: number[] = [];
  for (let a = start + pitch; a < len - FT * 0.3; a += pitch) steps.push(a);
  for (const a of steps) {
    const [l0, l1] = [lat(a, -1), lat(a, 1)];
    if (l1 - l0 < FT) continue;
    g.poly([...at(a - pitch + FT * 0.1, l0), ...at(a - FT * 0.9, l0), ...at(a - FT * 0.9, l1), ...at(a - pitch + FT * 0.1, l1)]);
  }
  g.fill(mix(ST[2], 0xffffff, 0.15));
  for (const a of steps) {
    const [l0, l1] = [lat(a, -1), lat(a, 1)];
    if (l1 - l0 < FT) continue;
    g.poly([...at(a, l0), ...at(a, l1), ...at(a - FT * 0.9, l1), ...at(a - FT * 0.9, l0)]);
  }
  g.fill({ color: ST[3], alpha: 0.95 });
  for (const a of steps) {
    const [l0, l1] = [lat(a, -1), lat(a, 1)];
    if (l1 - l0 < FT) continue;
    g.moveTo(...at(a, l0)).lineTo(...at(a, l1));
  }
  g.moveTo(...at(len, -hw)).lineTo(...at(len, hw));
  g.stroke({ width: LINE * 1.2, color: INK, cap: 'round' });
  // The revetment: a stone coping along the open back and down the ramp's sides (the earth stands up behind it),
  // its shadow on the town where it faces away from the light, inked on both edges.
  const runs: [number, number, number, number][] = [
    [pts[8], pts[9], foot2[0], foot2[1]],
    [foot1[0], foot1[1], pts[0], pts[1]],
    [fb[0], fb[1], ramp[6], ramp[7]],
    [fa[0], fa[1], ramp[8], ramp[9]],
  ];
  const [cx, cy] = at(depth * 0.55, 0);
  runs.forEach(([ax, ay, bx, by], idx) => {
    const l = Math.hypot(bx - ax, by - ay);
    if (l < FT * 0.5) return;
    // (Into the earth: toward the top's middle for the back; into the ramp from its sides.)
    let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
    const away = idx >= 2 ? -Math.sign(((ax + bx) / 2 - vx) * sx + ((ay + by) / 2 - vy) * sy) : 0;
    const [rx, ry] = idx >= 2 ? [sx * away, sy * away] : [cx - ax, cy - ay];
    if (nx * rx + ny * ry < 0) [nx, ny] = [-nx, -ny];
    // (The back's shadow out over the town where it looks away from the light; a side's is the riser shade.)
    if (idx < 2 && -(nx * LX + ny * LY) < 0.2) g.poly([ax, ay, bx, by, bx - nx * FT * 1.2, by - ny * FT * 1.2, ax - nx * FT * 1.2, ay - ny * FT * 1.2]).fill({ color: PLUM, alpha: 0.16 });
    const band = [ax, ay, bx, by, bx + nx * cope, by + ny * cope, ax + nx * cope, ay + ny * cope];
    g.poly(band).fill(nx * LX + ny * LY < 0 ? ST[2] : mix(ST[1], ST[2], 0.4));
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: LINE, color: INK, cap: 'round' });
    if (idx >= 2) g.moveTo(ax + nx * cope, ay + ny * cope).lineTo(bx + nx * cope, by + ny * cope).stroke({ width: LINE, color: INK, cap: 'round' });
    else g.moveTo(ax + nx * cope, ay + ny * cope).lineTo(bx + nx * cope, by + ny * cope).stroke({ width: LINE * 0.75, color: ST[0], alpha: 0.8 });
  });
  use(null);
}

/** Whether (x, y) falls inside a polygon of flat px pairs. */
function inside(q: number[], x: number, y: number): boolean {
  let hit = false;
  for (let i = 0, j = q.length / 2 - 1; i < q.length / 2; j = i++) {
    const [xi, yi, xj, yj] = [q[i * 2], q[i * 2 + 1], q[j * 2], q[j * 2 + 1]];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) hit = !hit;
  }
  return hit;
}

/**
 * A wall or gate tower's top (`ShapeKind::Tower`: its centre, `size` its radius, then flags): a
 * long soft shadow, a ring of masonry a little out of round with merlons on it, lit on the
 * north-west; a flagged fighting top with the spiral stair's hatch; a gate tower's banner. A
 * ruin's has merlons gone, a stretch of its parapet broken and rubble on the top.
 */
export function drawWallTower(g: Graphics, s: Shape) {
  const look = lookOf(s, 4);
  use(look);
  towerBody(g, s, look);
  use(null);
}

/** A tower by its look: a square or arched gate tower is half its gatehouse (`drawGate` draws it whole); a
 * palisade's a log blockhouse, the rest by their shape; a round one squat in the North (a broad splayed foot,
 * a short shadow), a tapering bastion in rammed earth (its battered face round it). */
function towerBody(g: Graphics, s: Shape, look: Look) {
  const [x, y, r] = [s.pts[0], s.pts[1], s.size];
  const flags = flagsOf(s, 2);
  const shape = look.shape;
  if (flags & GATE && (gateForm(s) === GATE_SQUARE || gateForm(s) === GATE_ARCHED)) return;
  if (look.material === MAT_PALISADE && (flags & SQUARE || shape === SHAPE_SQUARE)) return drawLogTower(g, s);
  if (flags & SQUARE || shape === SHAPE_SQUARE) return drawSquareTower(g, s);
  if (shape === SHAPE_D || shape === SHAPE_OCTAGON) return drawShapedTower(g, s, shape);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  rnd();
  rnd();
  const earth = look.material === MAT_EARTH;
  const squat = !earth && look.group === 0;
  // (The foot round it: a squat tower's splayed plinth, an earth bastion's battered face, wider.)
  const foot = earth ? FT * 2.2 : squat ? FT * 1.1 : 0;
  // Shadow: the round swept away from the light (one outline, so it doesn't darken twice); a low one's short.
  // (Cast no further than the tower's own radius: D runs along both axes, so its length is D * sqrt 2.)
  const rs = r + foot;
  const D = Math.min(PX * (squat ? 0.7 : earth ? 0.8 : 1.1), rs * Math.SQRT1_2);
  const shadow: number[] = [];
  for (let k = 0; k <= 12; k++) {
    const t = Math.PI * 0.75 + (k / 12) * Math.PI;
    shadow.push(x + Math.cos(t) * rs, y + Math.sin(t) * rs);
  }
  for (let k = 0; k <= 12; k++) {
    const t = -Math.PI * 0.25 + (k / 12) * Math.PI;
    shadow.push(x + D + Math.cos(t) * rs, y + D + Math.sin(t) * rs);
  }
  g.poly(shadow).fill({ color: 0x000000, alpha: 0.28 });
  if (foot) {
    // The foot in its shaded tone, a crescent lit toward the light, inked.
    g.circle(x, y, rs).fill(mix(ST[1], PLUM, 0.2)).stroke({ width: OUT, color: INK });
    g.moveTo(x + Math.cos(Math.PI * 0.8) * (r + foot / 2), y + Math.sin(Math.PI * 0.8) * (r + foot / 2));
    g.arc(x, y, r + foot / 2, Math.PI * 0.8, Math.PI * 1.7);
    g.stroke({ width: foot * 0.8, color: ST[2], alpha: 0.9, cap: 'round' });
  }
  const fallen: number[][] = [];
  const { ex, ey, rf, lost } = roundTop(g, x, y, r, rnd, ruined, fallen, earth);
  if (ruined) {
    fallen.push(...rubble(x + Math.cos(lost) * rf * 0.5, y + Math.sin(lost) * rf * 0.5, 2, rf * 0.3, rnd, 1.6));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = [x + (rnd() - 0.5) * rf * 1.4, y + (rnd() - 0.5) * rf * 1.4];
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
  // A gate tower flies the banner.
  if (gate && !ruined) banner(g, x - ex * rf * 0.4, y - ey * rf * 0.4);
}

/** A D's or an octagon's outline (px), as `wallstyle::footprint` lays it: `r` its radius, (`ux`, `uy`)
 * the way a D faces out or an octagon's sides run. */
export function shapedOutline(x: number, y: number, r: number, ux: number, uy: number, shape: number): number[] {
  const at = (a: number, b: number) => [x + ux * a - uy * b, y + uy * a + ux * b];
  const out: number[] = [];
  if (shape === SHAPE_D) {
    out.push(...at(-D_BACK * FT, -r));
    for (let k = 0; k <= 8; k++) {
      const t = -Math.PI / 2 + (Math.PI * k) / 8;
      out.push(...at(r * Math.cos(t), r * Math.sin(t)));
    }
    out.push(...at(-D_BACK * FT, r));
  } else {
    const rc = r / Math.cos(Math.PI / 8);
    for (let k = 0; k < 8; k++) {
      const t = Math.PI / 8 + (Math.PI * 2 * k) / 8;
      out.push(...at(rc * Math.cos(t), rc * Math.sin(t)));
    }
  }
  return out;
}

/** A convex outline drawn in by `d` (px), corner for corner. */
export function inset(pts: number[], d: number): number[] {
  const n = pts.length / 2;
  let area = 0;
  for (let i = 0; i < n; i++) area += pts[i * 2] * pts[((i + 1) % n) * 2 + 1] - pts[((i + 1) % n) * 2] * pts[i * 2 + 1];
  const sg = area > 0 ? 1 : -1;
  const normal = (i: number, j: number) => {
    const [dx, dy] = [pts[j * 2] - pts[i * 2], pts[j * 2 + 1] - pts[i * 2 + 1]];
    const l = Math.hypot(dx, dy) || 1;
    return [(-dy / l) * sg, (dx / l) * sg];
  };
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const [ax, ay] = normal((i + n - 1) % n, i);
    const [bx, by] = normal(i, (i + 1) % n);
    const k = d / Math.max(0.3, 1 + ax * bx + ay * by);
    out.push(pts[i * 2] + (ax + bx) * k, pts[i * 2 + 1] + (ay + by) * k);
  }
  return out;
}

/** A parapet between a convex outline and the same drawn in (`inner`, corner for corner): one band of coping,
 * crenels notched through it side by side in the merlons' rhythm (a corner always a merlon; a ruin's wider,
 * stones to `fallen`), each side's lit edge toward the light and a shaded crescent away from it, inked. */
function ringParapet(g: Graphics, outer: number[], inner: number[], rnd: () => number, ruined = false, fallen: number[][] = []) {
  const n = outer.length / 2;
  g.poly(outer);
  g.poly(inner).cut();
  g.fill(ST[2]);
  const crenels: number[][] = [];
  const P = (i: number, k: number): [number, number] => [outer[i * 2] + (inner[i * 2] - outer[i * 2]) * k, outer[i * 2 + 1] + (inner[i * 2 + 1] - outer[i * 2 + 1]) * k];
  let [cx, cy] = [0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + outer[i * 2] / n, cy + outer[i * 2 + 1] / n];
  const lits: number[] = [];
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [mx, my] = [(outer[i * 2] + outer[j * 2]) / 2, (outer[i * 2 + 1] + outer[j * 2 + 1]) / 2];
    lits.push(lit(mx - cx, my - cy));
    const len = Math.hypot(outer[j * 2] - outer[i * 2], outer[j * 2 + 1] - outer[i * 2 + 1]) * 0.5 + Math.hypot(inner[j * 2] - inner[i * 2], inner[j * 2 + 1] - inner[i * 2 + 1]) * 0.5;
    // (`a` along the side at mid-parapet, `d` in from the outline: 0 at its face, PARAPET at the walk.)
    const at: At = (a, d) => {
      const [t, k] = [a / len, d / PARAPET];
      const [p, q] = [P(i, k), P(j, k)];
      return [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
    };
    if (len > FT * 3.4) crenelRun(at, FT * 0.3, len - FT * 0.3, PARAPET, 0, rnd, crenels, ruined, fallen);
  }
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    if (lits[i] > 0.1) g.moveTo(...P(i, 0.18)).lineTo(...P(j, 0.18));
  }
  g.stroke({ width: FT * 0.3, color: ST[3], alpha: 0.8, cap: 'round' });
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    if (lits[i] < -0.1) g.moveTo(...P(i, 0.15)).lineTo(...P(j, 0.15));
  }
  g.stroke({ width: 3, color: PLUM, alpha: 0.3, cap: 'round' });
  g.poly(inner).stroke({ width: 1.8, color: INK, join: 'round' });
  crenelFill(g, crenels);
  g.poly(outer).stroke({ width: OUT, color: INK, join: 'round' });
}

/** A flat top's floor inside its parapet (`inner`): flags, the shade of the lit sides' parapets, the stair's
 * hatch in from the side facing most away from the light (at `cx`, `cy` its middle). */
function shapedTop(g: Graphics, inner: number[], cx: number, cy: number, rnd: () => number) {
  flagFloor(g, inner, rnd);
  const n = inner.length / 2;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [mx, my] = [(inner[i * 2] + inner[j * 2]) / 2, (inner[i * 2 + 1] + inner[j * 2 + 1]) / 2];
    if (lit(mx - cx, my - cy) > 0.2) g.moveTo(inner[i * 2], inner[i * 2 + 1]).lineTo(inner[j * 2], inner[j * 2 + 1]);
  }
  g.stroke({ width: FT * 1.2, color: PLUM, alpha: 0.2 });
  // (The hatch: its long side across the way from the middle to the shaded side.)
  let reach = Infinity;
  for (let i = 0; i < n; i++) reach = Math.min(reach, Math.hypot(inner[i * 2] - cx, inner[i * 2 + 1] - cy));
  const [ex, ey] = [-LX, -LY];
  const [hl, hw] = [Math.min(FT * 4.5, reach * 0.45), FT * 1.6];
  if (hl < FT * 2.5) return;
  const hc = reach * 0.85 - hl / 2 - FT * 0.8;
  stairHatch(g, (a, b) => [cx + ex * (hc + a) - ey * b, cy + ey * (hc + a) + ex * b], hl, hw);
}

/** A D or an octagonal tower: its shadow, the stone outline, a flagged top with the stair's hatch inside its
 * parapet of merlons; a ruin's with merlons gone, rubble and moss on the top. */
function drawShapedTower(g: Graphics, s: Shape, shape: number) {
  const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
  const [x, y, r, ux, uy] = [s.pts[0], s.pts[1], s.size, Math.cos(t), Math.sin(t)];
  const flags = flagsOf(s, 2);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const outer = shapedOutline(x, y, r, ux, uy, shape);
  const D = PX * 1.1;
  g.poly(outer.map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(outer).fill(ST[1]);
  const inner = inset(outer, PARAPET);
  // (Its middle: a D's between its back and its face.)
  const [cx, cy] = shape === SHAPE_D ? [x + ux * ((r - D_BACK * FT) / 2), y + uy * ((r - D_BACK * FT) / 2)] : [x, y];
  shapedTop(g, inner, cx, cy, rnd);
  const fallen: number[][] = [];
  ringParapet(g, outer, inner, rnd, ruined, fallen);
  if (ruined) {
    breach(g, x, y, r * 0.95, Math.atan2(uy, ux) + (rnd() - 0.5) * 2.4, Math.min(12, (r / FT) * 1.2), rnd, fallen);
    fallen.push(...rubble(cx + (rnd() - 0.5) * r * 0.6, cy + (rnd() - 0.5) * r * 0.6, 2, r * 0.25, rnd, 1.5));
    for (let k = 0; k < 4; k++) g.ellipse(cx + (rnd() - 0.5) * r * 1.1, cy + (rnd() - 0.5) * r * 1.1, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
  }
  blocks(g, fallen, stoneTone(rnd));
  if (gate && !ruined) banner(g, cx + LX * r * 0.3, cy + LY * r * 0.3);
}

/** A rounded log (a capsule) from `a0` to `a1` along and `b0` to `b1` across in `at`'s frame. */
export function logPoly(at: At, a0: number, a1: number, b0: number, b1: number): number[] {
  const [rb, mb] = [(b1 - b0) / 2, (b0 + b1) / 2];
  const out: number[] = [];
  for (let k = 0; k <= 6; k++) {
    const t = -Math.PI / 2 + (Math.PI * k) / 6;
    out.push(...at(a1 - rb + Math.cos(t) * rb * 0.7, mb + Math.sin(t) * rb));
  }
  for (let k = 0; k <= 6; k++) {
    const t = Math.PI / 2 + (Math.PI * k) / 6;
    out.push(...at(a0 + rb + Math.cos(t) * rb * 0.7, mb + Math.sin(t) * rb));
  }
  return out;
}

/** Logs (`logPoly` outlines) in timber: mid tone, the half toward the light lit, the ink round each. `toLight`
 * whether each log's +b side faces the light. */
export function logs(g: Graphics, polys: number[][], lits: number[][]) {
  for (const p of polys) g.poly(p);
  if (polys.length) g.fill(ST[1]);
  for (const p of lits) g.poly(p);
  if (lits.length) g.fill(ST[2]);
  for (const p of polys) g.poly(p);
  if (polys.length) g.stroke({ width: LINE * 1.3, color: INK, join: 'round' });
}

/**
 * A palisade's tower, a log blockhouse seen from above (square, `SQUARE`'s frame): its shadow, a log laid
 * along each side, the two crossing the others at the corners with their ends standing out; inside them a
 * plank fighting top and the stair's hatch. A ruin's has a side's log fallen and planks gone.
 */
function drawLogTower(g: Graphics, s: Shape) {
  const { x, y, h, ux, uy } = squareFrame(s);
  const flags = flagsOf(s, 2);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const D = PX * 0.9;
  g.poly(quad(-h, h, -h, h).map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  // The plank top inside the logs.
  const f = h - PARAPET;
  g.poly(quad(-h + FT * 0.4, h - FT * 0.4, -h + FT * 0.4, h - FT * 0.4)).fill(WK);
  for (let b = -f + FT * 1.1; b < f; b += FT * 1.1) g.moveTo(...at(-f, b)).lineTo(...at(f, b));
  for (let b = -f; b < f; b += FT * 1.1) {
    const a = (rnd() - 0.5) * f * 1.2;
    g.moveTo(...at(a, b)).lineTo(...at(a, Math.min(f, b + FT * 1.1)));
  }
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.6 });
  const lx = ux * LX + uy * LY;
  const ly = -uy * LX + ux * LY;
  g.poly(lx < 0 ? quad(-f, -f + FT * 0.8, -f, f) : quad(f - FT * 0.8, f, -f, f)).fill({ color: PLUM, alpha: 0.2 });
  g.poly(ly < 0 ? quad(-f, f, -f, -f + FT * 0.8) : quad(-f, f, f - FT * 0.8, f)).fill({ color: PLUM, alpha: 0.2 });
  const [hl, hw] = [Math.min(FT * 4.5, f * 0.6), FT * 1.6];
  const [ca, cb] = [f - hl / 2 - FT * 0.6, f - hw - FT * 0.6];
  if (!ruined) stairHatch(g, (a, b) => at(ca + a, cb - b), hl, hw);
  // The logs: along a (the sides at b = -h, h) under, along b over, each a log's thickness in from the face.
  const L = PARAPET;
  const e = FT * 0.9;
  const lost = ruined ? Math.floor(rnd() * 4) : -1;
  const under: number[][] = [];
  const over: number[][] = [];
  const litU: number[][] = [];
  const litO: number[][] = [];
  const fallen: number[][] = [];
  for (const [k, sg] of [
    [0, -1],
    [1, 1],
  ]) {
    const [b0, b1] = sg < 0 ? [-h, -h + L] : [h - L, h];
    const a1 = k * 2 === lost ? h * (0.1 + rnd() * 0.4) : h + e;
    const mb = (b0 + b1) / 2;
    under.push(logPoly(at, -h - e, a1, b0, b1));
    litU.push(ly < 0 ? logPoly(at, -h - e, a1, b0, mb) : logPoly(at, -h - e, a1, mb, b1));
    const atB: At = (a, b) => at(b, a);
    const a2 = k * 2 + 1 === lost ? h * (0.1 + rnd() * 0.4) : h + e;
    over.push(logPoly(atB, -h - e, a2, b0, b1));
    litO.push(lx < 0 ? logPoly(atB, -h - e, a2, b0, mb) : logPoly(atB, -h - e, a2, mb, b1));
    if (k * 2 === lost || k * 2 + 1 === lost) fallen.push(...rubble(...at(sg * (h + FT * 2), sg * h * 0.6), 2, FT * 1.4, rnd));
  }
  // (Each log's half toward the light lit: `ly` and `lx` how much +b and +a face it.)
  logs(g, under, litU);
  logs(g, over, litO);
  // End grain where the logs stand out at the corners: a ring on each end.
  for (const sa of [-1, 1]) {
    for (const sb of [-1, 1]) {
      g.circle(...at(sa * (h + e * 0.4), sb * (h - L / 2)), L * 0.2);
      g.circle(...at(sb * (h - L / 2), sa * (h + e * 0.4)), L * 0.2);
    }
  }
  g.stroke({ width: 1.1, color: ST[0], alpha: 0.7 });
  blocks(g, fallen, () => ST[1]);
  if (gate && !ruined) banner(g, ...at(-f * 0.4, -f * 0.4));
}

/** A square tower's frame: its middle, half side and the way its sides run (`SQUARE`). */
export function squareFrame(s: Shape) {
  const t = s.pts.length > 3 ? s.pts[3] / PX : 0;
  return { x: s.pts[0], y: s.pts[1], h: s.size, ux: Math.cos(t), uy: Math.sin(t) };
}

/**
 * A square wall tower seen from above (a castle's: `SQUARE`): its shadow, the stone block in its
 * shaded and lit faces, a flagged fighting top inside a parapet of merlons, the stair's hatch with
 * its lid; a ruin's with merlons gone, cracks, moss and fallen stones.
 */
function drawSquareTower(g: Graphics, s: Shape) {
  const { x, y, h, ux, uy } = squareFrame(s);
  const ruined = !!(flagsOf(s, 2) & RUINED);
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  const at = (a: number, b: number): [number, number] => [x + ux * a - uy * b, y + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  // Shadow, cast SE.
  const D = PX * 1.1;
  g.poly(quad(-h, h, -h, h).map((v) => v + D)).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(quad(-h, h, -h, h)).fill(ST[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags in courses inside the parapet.
  const f = h - PARAPET;
  g.poly(quad(-f, f, -f, f)).fill(WK);
  const row = FT * 2.8;
  for (let b = -f + row * (0.3 + rnd() * 0.7); b < f; b += row) {
    g.moveTo(...at(-f, b)).lineTo(...at(f, b));
    for (let a = -f + FT * (0.5 + rnd() * 2); a < f; a += FT * (2.4 + rnd() * 1.4)) g.moveTo(...at(a, b)).lineTo(...at(a, Math.min(f, b + row)));
  }
  g.stroke({ width: 1.2, color: ST[0], alpha: 0.4 });
  // The parapet's shade across the top on the side away from the light.
  const lx = ux * LX + uy * LY;
  const ly = -uy * LX + ux * LY;
  g.poly(lx < 0 ? quad(-f, -f + FT * 0.8, -f, f) : quad(f - FT * 0.8, f, -f, f)).fill({ color: PLUM, alpha: 0.18 });
  g.poly(ly < 0 ? quad(-f, f, -f, -f + FT * 0.8) : quad(-f, f, f - FT * 0.8, f)).fill({ color: PLUM, alpha: 0.18 });
  // The stair's hatch in a corner, its lid lying open beside it.
  const [hl, hw] = [Math.min(FT * 4.5, f * 0.6), FT * 1.6];
  const ca = f - hl / 2 - FT * 0.8;
  const cb = f - hw - FT * 0.8;
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = ca + hl / 2 - (k + 1) * hl * 0.21;
    g.poly(quad(a, a + hl * 0.21, cb - hw + 1.5, cb + hw - 1.5)).fill(mix(ST[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(quad(ca - hl / 2, ca + hl / 2, cb - hw, cb + hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = quad(ca - hl / 2, ca + hl / 2, cb - hw - FT * 0.3 - hw * 2, cb - hw - FT * 0.3);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]).stroke({ width: 1.8, color: INK, join: 'round' });
  // The parapet: coping round the top, crenels notched through it in the merlons' rhythm (a ruin's broken in places).
  const fallen: number[][] = [];
  ringParapet(g, quad(-h, h, -h, h), quad(-f, f, -f, f), rnd, ruined, fallen);
  if (ruined) {
    breach(g, x, y, h, Math.atan2(uy, ux) + Math.floor(rnd() * 4) * (Math.PI / 2), Math.min(12, (h / FT) * 1.2), rnd, fallen);
    fallen.push(...rubble(x, y, 2, f * 0.4, rnd, 1.5));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at((rnd() - 0.5) * f * 1.4, (rnd() - 0.5) * f * 1.4);
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
}

/** Stake tops (`r` px across half), each a cone from above: mid body, its facet toward the light lit, inked. */
export function stakeTops(g: Graphics, stakes: [number, number][], r: number) {
  for (const [x, y] of stakes) g.circle(x, y, r);
  if (!stakes.length) return;
  g.fill(ST[1]);
  for (const [x, y] of stakes) {
    g.moveTo(x, y);
    g.arc(x, y, r * 0.92, Math.PI * 0.75, Math.PI * 1.75);
    g.closePath();
  }
  g.fill(ST[2]);
  for (const [x, y] of stakes) g.circle(x + LX * r * 0.3, y + LY * r * 0.3, r * 0.3);
  g.fill(ST[3]);
  for (const [x, y] of stakes) g.circle(x, y, r);
  g.stroke({ width: OUT * 0.8, color: INK });
  for (const [x, y] of stakes) g.moveTo(x - LX * r * 0.15, y - LY * r * 0.15).lineTo(x + LX * r * 0.7, y + LY * r * 0.7);
  g.stroke({ width: 1, color: ST[0], alpha: 0.6 });
}
