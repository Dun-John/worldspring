// Curtain walls, wall towers and gatehouses on the battlemap, drawn in code like the atlas and the
// bridges (assets/STYLE_GUIDE.md): warm stone in a few flat tones lit from the north-west, bold
// ink, blocks a little out of true. A wall is a flagged walk between two crenellated parapets;
// a tower a flagged fighting top inside its merlons with the spiral stair's hatch; a gate the
// passage between its two towers under an arch at each end, portcullis grooves and the doors
// swung back. A ruin's works lose merlons, crack and moss over, spill rubble where they broke.
import type { Graphics } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { PX } from './atlas';
import { FT, INK, LX, LY, mix, MOSS, PLUM, rngAt, STONE, WOOD } from './bridges';

/** The walk and a tower's top: flags between the stone's mid and light tones. */
const WALK = 0xb0a692;
/** Iron: dark, mid, light. */
const IRON = [0x4a4e56, 0x6e7278, 0x9aa0a6];
/** A parapet's width (px): merlons stand on it. */
const PARAPET = FT * 1.6;

/** Flags on a wall's or tower's last point (`battlemap::WALL_*`). */
const BARE_A = 1;
const BARE_B = 2;
const GATE = 1;
const RUINED = 4;

const flagsOf = (s: Shape, at: number) => (s.pts.length > at ? Math.round(s.pts[at] / PX) : 0);
/** How much a direction faces the light, -1 .. 1. */
const lit = (x: number, y: number) => (x * LX + y * LY) / (Math.hypot(x, y) || 1);

/** Stone blocks (outlines) with a drop shadow: the block's sides in a shaded tone, its top in
 * `tone` set toward the light, a bold ink outline. */
function blocks(g: Graphics, quads: number[][], tone: () => number, shadow = FT * 0.35) {
  for (const b of quads) g.poly(b.map((v) => v + shadow));
  if (quads.length) g.fill({ color: 0x000000, alpha: 0.25 });
  for (const b of quads) {
    const t = tone();
    const n = b.length / 2;
    let [x, y] = [0, 0];
    for (let k = 0; k < n; k++) [x, y] = [x + b[k * 2] / n, y + b[k * 2 + 1] / n];
    g.poly(b).fill(mix(t, PLUM, 0.3));
    g.poly(b.map((v, k) => (k % 2 ? y + (v - y) * 0.72 + LY * FT * 0.18 : x + (v - x) * 0.72 + LX * FT * 0.18))).fill(t);
    g.poly(b).stroke({ width: 1.8, color: INK, join: 'round' });
  }
}

/** Loose rubble round (x, y): `n` blocks of mixed sizes within `spread` px. */
function rubble(x: number, y: number, n: number, spread: number, rnd: () => number): number[][] {
  const out: number[][] = [];
  for (let k = 0; k < n; k++) {
    const [cx, cy] = [x + (rnd() - 0.5) * 2 * spread, y + (rnd() - 0.5) * 2 * spread];
    const r = FT * (0.55 + rnd() * 0.75);
    const t = rnd() * Math.PI;
    const pts: number[] = [];
    for (let m = 0; m < 5; m++) {
      const a = t + (m / 5) * Math.PI * 2;
      const rr = r * (0.75 + rnd() * 0.4);
      pts.push(cx + Math.cos(a) * rr, cy + Math.sin(a) * rr);
    }
    out.push(pts);
  }
  return out;
}

const stoneTone = (rnd: () => number) => () => (rnd() < 0.35 ? STONE[2] : STONE[1]);

/**
 * Curtain walls (`ShapeKind::Wall`, one segment each, `size` thick): a soft shadow, ink and the
 * stone body (round joints where no end is bare), a flagged walk down the middle, a parapet with
 * merlons along each side, the shade of the parapet facing the light across the walk; moss and
 * cracks; rubble where an end stands bare; a ruin's with merlons gone and stones fallen.
 */
export function drawWalls(g: Graphics, walls: Shape[]) {
  const segs = walls.map((s) => {
    const [x0, y0, x1, y1] = s.pts;
    const l = Math.hypot(x1 - x0, y1 - y0) || 1;
    const [ux, uy] = [(x1 - x0) / l, (y1 - y0) / l];
    const flags = flagsOf(s, 4);
    const rnd = rngAt(x0 + x1, y0 + y1);
    const half = s.size / 2;
    // A point `a` along the segment and `d` across it (toward its left, +n).
    const at = (a: number, d: number): [number, number] => [x0 + ux * a - uy * d, y0 + uy * a + ux * d];
    // A bare end's broken line: a few points across it, pushed in or out a little.
    const broken = (a: number, dir: number) => [-1, -0.6, -0.2, 0.2, 0.6, 1].map((t) => at(a + dir * rnd() * FT * 2.6, t * half));
    const endA = flags & BARE_A ? broken(0, 1) : [at(0, -half), at(0, half)];
    const endB = flags & BARE_B ? broken(l, -1) : [at(l, -half), at(l, half)];
    const outline = [...endA, ...endB.reverse()].flat();
    return { x0, y0, x1, y1, l, ux, uy, half, at, outline, rnd, bareA: !!(flags & BARE_A), bareB: !!(flags & BARE_B), ruined: !!(flags & RUINED) };
  });
  const joints = (fn: (x: number, y: number) => void) => {
    for (const s of segs) {
      if (!s.bareA) fn(s.x0, s.y0);
      if (!s.bareB) fn(s.x1, s.y1);
    }
  };
  // Shadows, in two soft slices.
  for (const [off, alpha] of [
    [PX * 0.85, 0.12],
    [PX * 0.5, 0.18],
  ]) {
    for (const s of segs) g.poly(s.outline.map((v) => v + off));
    joints((x, y) => g.circle(x + off, y + off, segs[0].half));
    g.fill({ color: 0x000000, alpha });
  }
  // Ink, then the stone body.
  for (const s of segs) g.poly(s.outline);
  g.stroke({ width: 5, color: INK, join: 'round' });
  joints((x, y) => g.circle(x, y, segs[0].half + 2.5));
  g.fill(INK);
  for (const s of segs) g.poly(s.outline);
  joints((x, y) => g.circle(x, y, segs[0].half));
  g.fill(STONE[1]);
  // The walk between the parapets, its flags' joints and the shade of the lit side's parapet.
  for (const s of segs) {
    const w = s.half - PARAPET;
    const [a0, a1] = [s.bareA ? FT * 2.8 : 0, s.bareB ? s.l - FT * 2.8 : s.l];
    if (a1 > a0) g.poly([...s.at(a0, -w), ...s.at(a1, -w), ...s.at(a1, w), ...s.at(a0, w)]);
  }
  joints((x, y) => g.circle(x, y, segs[0].half - PARAPET));
  g.fill(WALK);
  for (const s of segs) {
    const w = s.half - PARAPET;
    for (let a = s.rnd() * FT * 2; a < s.l; a += FT * (2.4 + s.rnd() * 1.4)) {
      g.moveTo(...s.at(a, -w)).lineTo(...s.at(a + (s.rnd() - 0.5) * FT * 0.4, w));
      if (s.rnd() < 0.3) g.moveTo(...s.at(a, 0)).lineTo(...s.at(a + FT * (1.6 + s.rnd()), (s.rnd() - 0.5) * FT * 0.3));
    }
  }
  g.stroke({ width: 1.3, color: STONE[0], alpha: 0.6 });
  for (const s of segs) {
    const w = s.half - PARAPET;
    // The parapet on the side the light comes from shades the walk beside it.
    const side = lit(-s.uy, s.ux) > 0 ? 1 : -1;
    g.poly([...s.at(0, side * w), ...s.at(s.l, side * w), ...s.at(s.l, side * (w - FT * 0.6)), ...s.at(0, side * (w - FT * 0.6))]);
  }
  g.fill({ color: PLUM, alpha: 0.25 });
  // Each parapet one band of coping (lit on its edge toward the light), the crenels notches
  // through it (a ruin's wider, merlons gone), inked along the walk.
  const crenels: number[][] = [];
  const fallen: number[][] = [];
  for (const s of segs) {
    const [a0, a1] = [s.bareA ? FT * 3 : 0, s.bareB ? s.l - FT * 3 : s.l];
    if (a1 <= a0) continue;
    for (const side of [-1, 1]) {
      const [d0, d1] = [side * (s.half - PARAPET), side * s.half];
      g.poly([...s.at(a0, d0), ...s.at(a1, d0), ...s.at(a1, d1), ...s.at(a0, d1)]).fill(STONE[2]);
      const toLight = lit(-s.uy * side, s.ux * side) > 0;
      const e = toLight ? d1 - side * FT * 0.3 : d0 + side * FT * 0.3;
      g.moveTo(...s.at(a0, e)).lineTo(...s.at(a1, e)).stroke({ width: FT * 0.3, color: STONE[3], alpha: 0.8 });
      for (let a = a0 + FT * (1.2 + s.rnd() * 1.2); a < a1 - FT * 1.6; ) {
        let c = FT * (1.4 + s.rnd() * 0.3);
        if (s.ruined && s.rnd() < 0.35) {
          c += FT * (2.5 + s.rnd() * 2);
          if (s.rnd() < 0.5) fallen.push(...rubble(...s.at(a + c / 2, side * (s.half + FT * 1.4)), 1, FT * 0.8, s.rnd));
        }
        const b = Math.min(a + c, a1 - FT * 0.4);
        crenels.push([...s.at(a, d0 + side * 1.5), ...s.at(b, d0 + side * 1.5), ...s.at(b, d1 - side * 1.5), ...s.at(a, d1 - side * 1.5)]);
        a = b + FT * (2.4 + s.rnd() * 0.6);
      }
      g.moveTo(...s.at(a0, d0)).lineTo(...s.at(a1, d0));
    }
  }
  g.stroke({ width: 1.8, color: INK });
  for (const q of crenels) g.poly(q);
  if (crenels.length) g.fill(mix(STONE[0], PLUM, 0.3)).stroke({ width: 1.2, color: INK, alpha: 0.8, join: 'round' });
  // Moss on the stone and the odd crack; a ruin's more of both.
  for (const s of segs) {
    const n = Math.floor((s.l / (FT * 24)) * (s.ruined ? 3 : 1) + s.rnd());
    for (let k = 0; k < n; k++) {
      const [mx, my] = s.at(s.rnd() * s.l, (s.rnd() < 0.5 ? -1 : 1) * (s.half - PARAPET * 0.5));
      const r = FT * (0.3 + s.rnd() * 0.25);
      g.ellipse(mx, my, r * 1.3, r).fill({ color: MOSS[0], alpha: 0.85 });
      g.ellipse(mx - r * 0.3, my - r * 0.25, r * 0.6, r * 0.45).fill({ color: MOSS[1], alpha: 0.85 });
    }
    for (let k = 0; k < (s.ruined ? 3 : 1); k++) {
      if (s.rnd() < 0.5) continue;
      const a = s.rnd() * s.l;
      const d = (s.rnd() - 0.5) * s.half;
      g.moveTo(...s.at(a, d)).lineTo(...s.at(a + FT * 0.8, d + FT * 0.5)).lineTo(...s.at(a + FT * 1.5, d + FT * 0.2));
    }
  }
  g.stroke({ width: 1.4, color: STONE[0], alpha: 0.9, join: 'round' });
  // Rubble spilled where a stretch broke off, and below a ruin's walls.
  for (const s of segs) {
    if (s.bareA) fallen.push(...rubble(...s.at(-FT * 1.5, 0), 8, s.half * 1.2, s.rnd));
    if (s.bareB) fallen.push(...rubble(...s.at(s.l + FT * 1.5, 0), 8, s.half * 1.2, s.rnd));
    if (s.ruined) for (let a = s.rnd() * FT * 20; a < s.l; a += FT * (14 + s.rnd() * 16)) fallen.push(...rubble(...s.at(a, (s.rnd() < 0.5 ? -1 : 1) * (s.half + FT * 1.5)), 2, FT * 1.2, s.rnd));
  }
  blocks(g, fallen, stoneTone(segs[0]?.rnd ?? Math.random));
}

/**
 * A wall or gate tower's top (`ShapeKind::Tower`: its centre, `size` its radius, then flags): a
 * long soft shadow, a ring of masonry a little out of round with merlons on it, lit on the
 * north-west; a flagged fighting top with the spiral stair's hatch; a gate tower's banner. A
 * ruin's has merlons gone, a stretch of its parapet broken and rubble on the top.
 */
export function drawWallTower(g: Graphics, s: Shape) {
  const [x, y, r] = [s.pts[0], s.pts[1], s.size];
  const flags = flagsOf(s, 2);
  const [gate, ruined] = [!!(flags & GATE), !!(flags & RUINED)];
  const rnd = rngAt(x * 1.3 + y * 0.31, y * 0.7 - x * 0.17);
  rnd();
  rnd();
  const ring: number[] = [];
  const n = 22;
  for (let k = 0; k < n; k++) {
    const t = (k / n) * Math.PI * 2;
    const rr = r * (1 + (rnd() - 0.5) * 0.035);
    ring.push(x + Math.cos(t) * rr, y + Math.sin(t) * rr);
  }
  // Shadow: the round swept away from the light (one outline, so it doesn't darken twice).
  const D = PX * 1.1;
  const shadow: number[] = [];
  for (let k = 0; k <= 12; k++) {
    const t = Math.PI * 0.75 + (k / 12) * Math.PI;
    shadow.push(x + Math.cos(t) * r, y + Math.sin(t) * r);
  }
  for (let k = 0; k <= 12; k++) {
    const t = -Math.PI * 0.25 + (k / 12) * Math.PI;
    shadow.push(x + D + Math.cos(t) * r, y + D + Math.sin(t) * r);
  }
  g.poly(shadow).fill({ color: 0x000000, alpha: 0.28 });
  g.poly(ring).fill(STONE[1]).stroke({ width: 3, color: INK, join: 'round' });
  // The fighting top: flags inside the parapet, their joints, the parapet's shade on the lit side.
  const rf = r - PARAPET;
  g.circle(x, y, rf).fill(WALK);
  const turn = rnd() * Math.PI;
  const [cu, su] = [Math.cos(turn), Math.sin(turn)];
  // Courses of flags across the top (turned at random), their joints staggered course to course.
  const row = FT * 2.8;
  for (let o = -rf + row * (0.3 + rnd() * 0.7); o < rf; o += row) {
    const h = Math.sqrt(Math.max(0, rf * rf - o * o));
    const [px, py] = [x - su * o, y + cu * o];
    g.moveTo(px - cu * h, py - su * h).lineTo(px + cu * h, py + su * h);
    const h1 = Math.sqrt(Math.max(0, rf * rf - (o + row) * (o + row)));
    const span = Math.min(h, h1 || h);
    for (let t = -span + FT * (0.5 + rnd() * 2); t < span; t += FT * (2.4 + rnd() * 1.4)) {
      g.moveTo(px + cu * t, py + su * t).lineTo(px + cu * t - su * Math.min(row, rf - o), py + su * t + cu * Math.min(row, rf - o));
    }
  }
  g.stroke({ width: 1.2, color: STONE[0], alpha: 0.4 });
  g.moveTo(x + Math.cos(Math.PI * 0.85) * (rf - FT * 0.3), y + Math.sin(Math.PI * 0.85) * (rf - FT * 0.3));
  g.arc(x, y, rf - FT * 0.3, Math.PI * 0.85, Math.PI * 1.65);
  g.stroke({ width: FT * 0.6, color: PLUM, alpha: 0.25 });
  // The stair's hatch: an opening in the flags with steps going down into the dark, its timber
  // lid lying open beside it.
  const th = rnd() * Math.PI * 2;
  const [ex, ey] = [Math.cos(th), Math.sin(th)];
  const [hl, hw] = [Math.min(FT * 4.5, rf * 0.5), FT * 1.6];
  const hc = rf - hl / 2 - FT * 0.8;
  // A point `a` out along the hatch (toward the rim) and `b` across it, from its middle.
  const hp = (a: number, b: number): [number, number] => [x + ex * (hc + a) - ey * b, y + ey * (hc + a) + ex * b];
  const hq = (a0: number, a1: number, b0: number, b1: number) => [...hp(a0, b0), ...hp(a1, b0), ...hp(a1, b1), ...hp(a0, b1)];
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).fill(0x221d19);
  for (let k = 0; k < 4; k++) {
    const a = hl / 2 - (k + 1) * hl * 0.21;
    g.poly(hq(a, a + hl * 0.21, -hw + 1.5, hw - 1.5)).fill(mix(STONE[1], 0x221d19, k * 0.24)).stroke({ width: 1, color: 0x221d19, alpha: 0.6 });
  }
  g.poly(hq(-hl / 2, hl / 2, -hw, hw)).stroke({ width: 2.4, color: INK, join: 'round' });
  const lid = hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 2);
  g.poly(lid.map((v) => v + FT * 0.25)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(lid).fill(WOOD[1]);
  g.poly(hq(-hl / 2, hl / 2, hw + FT * 0.3, hw + FT * 0.3 + hw * 0.7)).fill({ color: WOOD[2], alpha: 0.7 });
  for (const t of [0.33, 0.66]) g.moveTo(...hp(-hl / 2, hw + FT * 0.3 + hw * 2 * t)).lineTo(...hp(hl / 2, hw + FT * 0.3 + hw * 2 * t));
  g.stroke({ width: 1.2, color: WOOD[0] });
  for (const t of [0.22, 0.78]) g.moveTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3)).lineTo(...hp(-hl / 2 + hl * t, hw + FT * 0.3 + hw * 2));
  g.stroke({ width: FT * 0.28, color: IRON[0] });
  g.poly(lid).stroke({ width: 1.8, color: INK, join: 'round' });
  // The parapet: one ring of coping, crenels notched through it (a ruin's broken over a stretch).
  const m = Math.max(8, Math.round((Math.PI * 2 * (r - PARAPET / 2)) / (FT * 4)));
  const step = (Math.PI * 2) / m;
  const lost = ruined ? rnd() * Math.PI * 2 : NaN;
  const fallen: number[][] = [];
  const [ri, ro] = [rf + 1.5, r - 1.5];
  g.circle(x, y, (rf + r) / 2).stroke({ width: r - rf, color: STONE[2] });
  const notch = (t0: number, t1: number) => {
    const q: number[] = [];
    for (const t of [t0, (t0 + t1) / 2, t1]) q.push(x + Math.cos(t) * ri, y + Math.sin(t) * ri);
    for (const t of [t1, (t0 + t1) / 2, t0]) q.push(x + Math.cos(t) * ro, y + Math.sin(t) * ro);
    g.poly(q);
  };
  for (let k = 0; k < m; k++) {
    const t0 = k * step + turn;
    const gone = ruined && (Math.abs(Math.atan2(Math.sin(t0 - lost), Math.cos(t0 - lost))) < 0.6 || rnd() < 0.25);
    if (gone && rnd() < 0.5) fallen.push(...rubble(x + Math.cos(t0) * (rf - FT), y + Math.sin(t0) * (rf - FT), 1, FT * 0.6, rnd));
    notch(gone ? t0 - step * 0.3 : t0 + step * 0.62, t0 + step * (gone ? 0.95 : 0.95));
  }
  g.fill(mix(STONE[0], PLUM, 0.3)).stroke({ width: 1.2, color: INK, alpha: 0.8, join: 'round' });
  g.circle(x, y, rf).stroke({ width: 1.8, color: INK });
  // The ring's lit and shaded faces.
  g.moveTo(x + Math.cos(Math.PI * 0.9) * (r - 2), y + Math.sin(Math.PI * 0.9) * (r - 2));
  g.arc(x, y, r - 2, Math.PI * 0.9, Math.PI * 1.6);
  g.stroke({ width: 2.5, color: STONE[3], alpha: 0.7 });
  g.moveTo(x + Math.cos(-Math.PI * 0.1) * (r - 2), y + Math.sin(-Math.PI * 0.1) * (r - 2));
  g.arc(x, y, r - 2, -Math.PI * 0.1, Math.PI * 0.6);
  g.stroke({ width: 3, color: PLUM, alpha: 0.3 });
  if (ruined) {
    fallen.push(...rubble(x + Math.cos(lost) * rf * 0.5, y + Math.sin(lost) * rf * 0.5, 5, rf * 0.3, rnd));
    for (let k = 0; k < 4; k++) {
      const [mx, my] = [x + (rnd() - 0.5) * rf * 1.4, y + (rnd() - 0.5) * rf * 1.4];
      g.ellipse(mx, my, FT * 0.6, FT * 0.45).fill({ color: MOSS[1], alpha: 0.8 });
    }
  }
  blocks(g, fallen, stoneTone(rnd));
  // A gate tower flies the banner.
  if (gate && !ruined) banner(g, x - ex * rf * 0.4, y - ey * rf * 0.4);
}

/**
 * A gatehouse (`ShapeKind::Gate`: the two gate towers' centres; `size` 1 for a ruin's): the
 * passage between the towers paved in setts, masonry jambs down its sides, a stone arch across
 * each end casting its shadow in, a portcullis slot inside each arch, the two doors standing
 * half open with their swings marked; over it all the wall walk, see-through, a winch for the
 * portcullis on some. A ruin's has no doors, walk or portcullis, and one arch broken.
 */
export function drawGate(g: Graphics, s: Shape) {
  const [x0, y0, x1, y1] = s.pts;
  const L = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ux, uy] = [(x1 - x0) / L, (y1 - y0) / L];
  const at = (a: number, b: number): [number, number] => [x0 + ux * a - uy * b, y0 + uy * a + ux * b];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ruined = Math.round(s.size / PX) === 1;
  const rnd = rngAt(x0 + x1, y0 + y1);
  const R = FT * 15;
  const D = FT * 11;
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
    g.poly(quad(a0, a1, -D, D)).fill(STONE[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let b = -D + FT * 1.8; b < D; b += FT * 1.8) g.moveTo(...at(a0, b)).lineTo(...at(a1, b));
    g.stroke({ width: 1.2, color: STONE[0], alpha: 0.8 });
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
    g.poly(quad(p0 - FT, end, b0, b0 + ab)).fill(STONE[1]).stroke({ width: 2.2, color: INK, join: 'round' });
    for (let a = p0 - FT + FT * 1.6; a < end - FT * 0.4; a += FT * 1.6) g.moveTo(...at(a, b0)).lineTo(...at(a, b0 + ab));
    g.stroke({ width: 1.2, color: STONE[0], alpha: 0.85 });
    const mid = (p0 + p1) / 2;
    if (mid < end) g.poly(quad(mid - FT * 0.7, mid + FT * 0.7, b0, b0 + ab)).fill(STONE[2]).stroke({ width: 1.4, color: INK });
    // The coping's lit edge: the long side toward the light.
    const litSide = lit(-uy, ux) > 0 ? b0 + ab - FT * 0.25 : b0 + FT * 0.25;
    g.moveTo(...at(p0 - FT, litSide)).lineTo(...at(end, litSide)).stroke({ width: FT * 0.3, color: STONE[3], alpha: 0.7 });
    if (k === broken) fallen.push(...rubble(...at(end + FT * 2, b0 + ab / 2 + (k ? -FT * 2 : FT * 2)), 6, FT * 2, rnd));
  });
  // The wall walk carried over the gate, between the towers: overhead, so shown see-through with
  // its edges dashed; on some gates the portcullis's winch stands on it, chains to the slot.
  if (!ruined) {
    const w = FT * 4;
    const [a0, a1] = [p0 - FT, p1 + FT];
    g.poly(quad(a0, a1, -w, w)).fill({ color: WALK, alpha: 0.5 });
    for (const side of [-1, 1]) {
      const [d0, d1] = side < 0 ? [-w, -w + PARAPET] : [w - PARAPET, w];
      g.poly(quad(a0, a1, d0, d1)).fill({ color: STONE[2], alpha: 0.6 });
      for (let a = a0 + FT * 1.6; a < a1 - FT * 1.6; a += FT * 3.9) g.poly(quad(a, a + FT * 1.4, d0 + 1.5, d1 - 1.5));
    }
    g.fill({ color: mix(STONE[0], PLUM, 0.3), alpha: 0.75 });
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
  // The doors (drawn over the walk, to be read): two timber leaves hung on the jambs just inside
  // the outer portcullis, standing half open into the passage, each with its swing marked.
  if (!ruined) {
    const bh = -D + ab + FT * 1.7;
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
  if (ruined) {
    for (let k = 0; k < 4; k++) {
      const [mx, my] = at(p0 + rnd() * (p1 - p0), (rnd() - 0.5) * 2 * D);
      g.ellipse(mx, my, FT * 0.7, FT * 0.5).fill({ color: MOSS[1], alpha: 0.75 });
    }
    blocks(g, fallen, stoneTone(rnd));
  }
}

/** A banner streaming east on the wind from a pole at (x, y). */
export function banner(g: Graphics, x: number, y: number) {
  g.moveTo(x, y).lineTo(x + PX * 0.2, y - PX * 1.8).stroke({ width: 2.5, color: 0x3b2f25 });
  g.poly([x + PX * 0.2, y - PX * 1.8, x + PX * 1.7, y - PX * 1.5, x + PX * 1.4, y - PX * 1.25, x + PX * 1.75, y - PX * 0.95, x + PX * 0.15, y - PX * 1.05])
    .fill(0x9b2226)
    .stroke({ width: 1.5, color: INK });
}

/** The 5-ft grid over a wall's walk or a tower's top (shown with the ground's): lines on the square
 * boundaries, clipped to the walk (a quad) or the top (a round). */
export function topGrid(g: Graphics, s: Shape) {
  if (s.kind === 1) {
    const [cx, cy] = [s.pts[0], s.pts[1]];
    const r = s.size - PARAPET;
    for (const axis of [0, 1]) {
      const c = axis === 0 ? cx : cy;
      for (let k = Math.ceil((c - r) / PX); k * PX <= c + r; k++) {
        const h = Math.sqrt(Math.max(0, r * r - (k * PX - c) ** 2));
        if (h <= 0) continue;
        if (axis === 0) g.moveTo(k * PX, cy - h).lineTo(k * PX, cy + h);
        else g.moveTo(cx - h, k * PX).lineTo(cx + h, k * PX);
      }
    }
  } else if (s.kind === 0) {
    const [x0, y0, x1, y1] = s.pts;
    const len = Math.hypot(x1 - x0, y1 - y0) || 1;
    const w = s.size / 2 - PARAPET;
    const [vx, vy] = [(-(y1 - y0) / len) * w, ((x1 - x0) / len) * w];
    const q = [x0 - vx, y0 - vy, x1 - vx, y1 - vy, x1 + vx, y1 + vy, x0 + vx, y0 + vy];
    // Where the line `axis` = `at` crosses the walk's outline, as the span along the other axis.
    for (const axis of [0, 1]) {
      const vals = [q[axis], q[axis + 2], q[axis + 4], q[axis + 6]];
      for (let k = Math.ceil(Math.min(...vals) / PX); k * PX <= Math.max(...vals); k++) {
        let [lo, hi] = [Infinity, -Infinity];
        for (let i = 0; i < 4; i++) {
          const [a, b] = [i * 2, ((i + 1) % 4) * 2];
          const [p, r] = [q[a + axis], q[b + axis]];
          if ((p - k * PX) * (r - k * PX) > 0 || p === r) continue;
          const o = q[a + 1 - axis] + ((k * PX - p) / (r - p)) * (q[b + 1 - axis] - q[a + 1 - axis]);
          [lo, hi] = [Math.min(lo, o), Math.max(hi, o)];
        }
        if (!(hi > lo)) continue;
        if (axis === 0) g.moveTo(k * PX, lo).lineTo(k * PX, hi);
        else g.moveTo(lo, k * PX).lineTo(hi, k * PX);
      }
    }
  } else return;
  g.stroke({ width: 1, color: INK, alpha: 0.45, pixelLine: true });
}
