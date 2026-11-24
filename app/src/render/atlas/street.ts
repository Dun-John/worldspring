import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { boughs, lit, whorl } from './plants';
import { BARK, STONE, moss, stone } from './stones';
import { endGrain } from './wood';
import { bevel } from './yard';
import { LINE, OUT } from '../pen';

/** A hand-made box from (x0, y0) to (x1, y1) as points for `poly`: its corners and the middle of each side a
 * little out of true (`j` px), so no edge is ruler-straight. */
export function handBox(x0: number, y0: number, x1: number, y1: number, rnd: () => number, j = 0.9): number[] {
  const J = (k = 1) => (rnd() - 0.5) * j * k;
  const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
  return [x0 + J(), y0 + J(), mx + J(), y0 + J(0.6), x1 + J(), y0 + J(), x1 + J(0.6), my + J(), x1 + J(), y1 + J(), mx + J(), y1 + J(0.6), x0 + J(), y1 + J(), x0 + J(0.6), my + J()];
}

/** The top of a block, a plank or a post seen straight down, (x0, y0) to (x1, y1), in `tones` [dark, mid, light]:
 * the mid tone, a lit strip along its north and west edges, a shade tapering along its south and east, inked
 * (`ink` px, `LINE` for small things, `OUT` past half a square). Returns its outline. */
export function blockTop(g: Graphics, x0: number, y0: number, x1: number, y1: number, tones: readonly number[], rnd: () => number, ink = LINE): number[] {
  const pts = handBox(x0, y0, x1, y1, rnd, Math.min(0.9, (x1 - x0) * 0.08, (y1 - y0) * 0.08) + 0.3);
  g.poly(pts).fill(tones[1]);
  const b = Math.max(1.1, Math.min(x1 - x0, y1 - y0) * 0.2);
  g.poly([x0 + b * 1.5, y1, x1, y1, x1, y0 + b * 1.5, x1 - b, y0 + b * 2.2, x1 - b, y1 - b, x0 + b * 2.2, y1 - b]).fill({ color: tones[0], alpha: 0.75 });
  g.poly([x0, y0, x1 - b * 1.5, y0, x1 - b * 2.2, y0 + b * 0.8, x0 + b * 0.8, y0 + b * 0.8, x0 + b * 0.8, y1 - b * 2.2, x0, y1 - b * 1.5]).fill({ color: tones[2], alpha: 0.85 });
  g.poly(pts).stroke({ width: ink, color: INK, join: 'round' });
  return pts;
}

/** A round thing seen straight down at (x, y), radius `s` px: the dark tone showing as a crescent on the
 * side away from the light, the mid tone over it, a light patch toward the NW, inked. */
export function domeTop(g: Graphics, x: number, y: number, s: number, tones: readonly number[], rnd: () => number, ink = LINE, n = 12) {
  const out = blob(x, y, s, n, 0.08, rnd);
  g.poly(out).fill(tones[0]);
  g.poly(out.map((p, i) => (p - (i % 2 ? y : x)) * 0.84 + (i % 2 ? y : x) - s * 0.1)).fill(tones[1]);
  g.poly(oval(x - s * 0.3, y - s * 0.32, s * 0.38, s * 0.3, -0.7, 10)).fill({ color: tones[2], alpha: 0.85 });
  g.poly(out).stroke({ width: ink, color: INK, join: 'round' });
  return out;
}

/** Worn ground at (x, y), `rx` by `ry` px: bare trodden earth fading into the grass at its edge. */
export function trodden(g: Graphics, x: number, y: number, rx: number, ry: number, rnd: () => number) {
  g.poly(blob(x, y, 1, 12, 0.25, rnd).map((p, i) => (i % 2 ? y + (p - y) * ry : x + (p - x) * rx))).fill({ color: 0x8a7350, alpha: 0.35 });
  g.poly(blob(x, y, 1, 10, 0.3, rnd).map((p, i) => (i % 2 ? y + (p - y) * ry * 0.65 : x + (p - x) * rx * 0.7))).fill({ color: 0x9a8058, alpha: 0.35 });
}

/** A few blades of grass at (x, y), as at a headstone's foot. */
export function footGrass(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  whorl(g, x, y, s, boughs(7, rnd), 0.4);
  g.fill(0x5a7e36).stroke({ width: 1, color: 0x34461e, join: 'round' });
}

/** Water in a trough or basin (its inside, a rounded box): fresh water, the rim's shadow along its north and
 * west sides, two ripples and one glint. */
function troughWater(g: Graphics, x0: number, y0: number, x1: number, y1: number, round: number) {
  g.roundRect(x0, y0, x1 - x0, y1 - y0, round).fill(0x3a7f8a).stroke({ width: 1.5, color: shade(0x255a70, 0.8) });
  g.roundRect(x0, y0, x1 - x0, Math.min(4, (y1 - y0) * 0.3), round).fill({ color: 0x1e4a5c, alpha: 0.7 });
  g.roundRect(x0, y0, Math.min(4, (x1 - x0) * 0.2), y1 - y0, round).fill({ color: 0x1e4a5c, alpha: 0.5 });
  const [mx, my] = [(x0 + x1) / 2 + (x1 - x0) * 0.1, (y0 + y1) / 2 + 1];
  for (const [dx, dy, k] of [
    [0, 0, 1],
    [(x1 - x0) * 0.16, (y1 - y0) * 0.18, 0.7],
  ]) {
    const rw = (x1 - x0) * 0.09 * k;
    g.moveTo(mx + dx - rw, my + dy).quadraticCurveTo(mx + dx, my + dy - 2.5, mx + dx + rw, my + dy).stroke({ width: 1.2, color: 0x6ab0a8, alpha: 0.85, cap: 'round' });
  }
  g.poly(oval(x0 + (x1 - x0) * 0.26, y0 + (y1 - y0) * 0.42, (x1 - x0) * 0.08, (y1 - y0) * 0.1, -0.2, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
}

/** Water trough, along x: a planked timber trough with iron bands (v0), a stone one with a chipped rim and
 * moss on its north side (v1), a hollowed log (v2), or a stone trough fed by a pump at its +x end (v3). */
export function waterTrough(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const W = 1.05 * FT;
  const [x0, x1] = v === 3 ? [-3.6 * FT, 1.6 * FT] : [-3 * FT, 3 * FT];
  if (v === 2) {
    // A log split along its length and hollowed out: lumpy bark a little thicker toward +x, ridged along its
    // length and lit along the top, the pale split face round the water, a cut end with rings at each end.
    const bark = BARK[0];
    const [e0, e1] = [x0 + 0.35 * FT, x1 - 0.35 * FT];
    const half = (t: number) => W * (0.9 + 0.18 * t);
    const n = 9;
    const top: number[] = [];
    const bot: [number, number][] = [];
    for (let i = 0; i <= n; i++) {
      const t = i / n;
      const x = e0 + (e1 - e0) * t;
      const ends = i === 0 || i === n;
      top.push(x, -half(t) + (ends ? 0 : (rnd() - 0.5) * 2.6));
      bot.push([x, half(t) + (ends ? 0 : (rnd() - 0.5) * 2.6)]);
    }
    g.poly([...top, ...bot.reverse().flat()]).fill(bark[1]).stroke({ width: 2.5, color: INK, join: 'round' });
    for (let i = 0; i < 12; i++) {
      const x = e0 + 0.3 * FT + rnd() * (e1 - e0 - 0.9 * FT);
      const side = i % 2 ? 1 : -1;
      const yy = side * W * (0.72 + 0.16 * rnd());
      g.moveTo(x, yy).lineTo(x + 7 + rnd() * 9, yy + (rnd() - 0.5) * 1.5).stroke({ width: 1.8, color: shade(bark[0], 0.7), cap: 'round' });
    }
    g.moveTo(e0 + 4, -half(0) + 3).lineTo(e1 - 4, -half(1) + 3).stroke({ width: 2, color: bark[2], alpha: 0.8, cap: 'round' });
    g.roundRect(e0 + 3, -W * 0.6, e1 - e0 - 6, W * 1.2, W * 0.5).fill(0xc89a68).stroke({ width: 1.2, color: shade(bark[0], 0.8) });
    troughWater(g, e0 + 7, -W * 0.42, e1 - 7, W * 0.44, W * 0.36);
    endGrain(g, e0, 0, 0.3 * FT, half(0) * 0.96, bark);
    endGrain(g, e1, 0, 0.3 * FT, half(1) * 0.96, bark);
    return;
  }
  const tones = v === 0 ? WOOD : STONE[v === 1 ? 0 : 2];
  const rim = v === 0 ? 0.32 * FT : 0.45 * FT;
  // The box, its corners a little out of true.
  const j = () => (rnd() - 0.5) * 1.6;
  const box = [x0 + j(), -W + j(), x1 + j(), -W + j(), x1 + j(), W + j(), x0 + j(), W + j()];
  g.poly(box).fill(tones[1]).stroke({ width: 2.5, color: INK, join: 'round' });
  troughWater(g, x0 + rim, -W + rim, x1 - rim, W - rim, 2);
  // Rim tops: the north and west lit, the south and east in shade.
  g.poly([x0 + 1, -W + 1, x1 - 1, -W + 1, x1 - rim, -W + rim, x0 + rim, -W + rim]).fill(tones[2]);
  g.poly([x0 + 1, -W + 1, x0 + rim, -W + rim, x0 + rim, W - rim, x0 + 1, W - 1]).fill(shade(tones[2], 0.94));
  g.poly([x0 + 1, W - 1, x1 - 1, W - 1, x1 - rim, W - rim, x0 + rim, W - rim]).fill(tones[0]);
  g.poly([x1 - 1, -W + 1, x1 - 1, W - 1, x1 - rim, W - rim, x1 - rim, -W + rim]).fill(shade(tones[0], 0.92));
  g.poly(box).stroke({ width: 2.5, color: INK, join: 'round' });
  g.rect(x0 + rim, -W + rim, x1 - x0 - rim * 2, (W - rim) * 2).stroke({ width: 1.2, color: shade(tones[0], 0.7) });
  if (v === 0) {
    // Plank joints along the rim, an iron band round each end.
    for (const x of [x0 + (x1 - x0) * 0.36, x0 + (x1 - x0) * 0.7]) {
      g.moveTo(x, -W + 1).lineTo(x, -W + rim).moveTo(x - 3, W - rim).lineTo(x - 3, W - 1);
    }
    g.stroke({ width: 1.2, color: WOOD[0] });
    for (const x of [x0 + 0.6 * FT, x1 - 0.6 * FT]) {
      g.moveTo(x, -W - 1).lineTo(x, -W + rim).moveTo(x, W - rim).lineTo(x, W + 1).stroke({ width: 3, color: IRON[1] });
      g.moveTo(x - 1, -W).lineTo(x - 1, -W + rim - 1).stroke({ width: 1, color: IRON[3] });
    }
  } else {
    // Stone: joints between the blocks of the rim, a chip out of the south rim, moss on the north.
    for (const x of [x0 + (x1 - x0) * 0.4, x0 + (x1 - x0) * 0.78]) g.moveTo(x, -W + 1).lineTo(x + 1, -W + rim);
    g.moveTo(x0 + (x1 - x0) * 0.56, W - rim).lineTo(x0 + (x1 - x0) * 0.56, W - 1);
    g.stroke({ width: 1.2, color: shade(tones[0], 0.8) });
    const cx = x0 + (x1 - x0) * 0.24;
    g.poly([cx - 4, W - 1, cx + 5, W - 1, cx + 2, W - rim + 1, cx - 2, W - rim + 2]).fill(shade(tones[0], 0.8)).stroke({ width: 1, color: INK, join: 'round' });
    if (v === 1) {
      moss(g, x0 + (x1 - x0) * 0.66, -W + rim * 0.4, rim * 0.7, rnd);
      moss(g, x0 + rim * 0.4, -W + rim * 0.9, rim * 0.5, rnd);
    }
  }
  if (v === 3) {
    // The pump: a timber housing at the end, its handle out to the side, the spout over the water.
    const [px0, px1] = [x1 + 0.3 * FT, x1 + 1.5 * FT];
    g.moveTo(x1 - rim - 2, 0).lineTo(px0 + 2, 0).stroke({ width: 6, color: INK, cap: 'round' });
    g.moveTo(x1 - rim - 2, 0).lineTo(px0 + 2, 0).stroke({ width: 3.5, color: IRON[2], cap: 'round' });
    g.poly(oval(x1 - rim - 2, 0, 3.2, 2.4, 0, 10)).stroke({ width: 1, color: 0xe6f4ec, alpha: 0.8 });
    bevel(g, px0, -0.6 * FT, px1, 0.6 * FT, 2.5, WOOD);
    g.circle((px0 + px1) / 2, 0, 2.2).fill(IRON[1]).stroke({ width: 1, color: INK });
    const [hx, hy] = [px1 + 0.5 * FT, -1.6 * FT];
    g.moveTo((px0 + px1) / 2, 0).lineTo(hx, hy).stroke({ width: 5, color: INK, cap: 'round' });
    g.moveTo((px0 + px1) / 2, 0).lineTo(hx, hy).stroke({ width: 2.6, color: IRON[2], cap: 'round' });
    g.circle(hx, hy, 2.6).fill(WOOD[1]).stroke({ width: 1.2, color: INK });
  }
}

/** A bill lying flat on the ground at (x, y), `w` by `h` px turned by `a`: a pale sheet, a corner curled up,
 * ink lines of writing across it. */
function fallenBill(g: Graphics, x: number, y: number, w: number, h: number, a: number, paper: number, rnd: () => number) {
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const at = (u: number, v: number): [number, number] => [x + c * u - s * v, y + s * u + c * v];
  const [hw, hh] = [w / 2, h / 2];
  const j = () => (rnd() - 0.5) * 0.8;
  g.poly([...at(-hw + j(), -hh + j()), ...at(hw + j(), -hh + j()), ...at(hw + j(), hh - 2.5), ...at(hw - 2.5, hh + j()), ...at(-hw + j(), hh + j())]).fill(paper).stroke({ width: LINE * 0.75, color: INK, join: 'round' });
  g.poly([...at(hw, hh - 2.5), ...at(hw - 2.5, hh), ...at(hw - 2.8, hh - 2.8)]).fill(shade(paper, 0.82));
  for (let k = 0; k < 3; k++) {
    const v = -hh + 2.2 + k * ((h - 4.4) / 2);
    const len = (w - 4) * (k === 0 ? 0.6 : 0.9 - rnd() * 0.25);
    g.moveTo(...at(-hw + 2, v)).lineTo(...at(-hw + 2 + len, v));
  }
  g.stroke({ width: 0.016 * 64 * 1.1, color: 0x3a3028, alpha: 0.85, cap: 'round' });
}

/** Notice board, straight down: all timber, its height told by its long thin shadow (cast with the
 * battlemap's): two square post tops about 4 ft apart, the top edge of the board a slim plank between them,
 * the ground trodden bare in front where people stand to read and a bill or two blown down lying flat in it;
 * a narrow shingled roof over some (v1, v3), two slopes either side of a ridge from above; weathered grey
 * (v3), a second plank, a cross-rail, on v2. */
export function noticeBoard(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const wood = v === 3 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  const [px, ps] = [2.0 * FT, 0.26 * FT];
  const hw = px - ps;
  const t = 0.15 * FT;
  trodden(g, 0, 1.25 * FT, 2.0 * FT, 0.75 * FT, rnd);
  footGrass(g, -2.6 * FT, 0.55 * FT, 5, rnd);
  footGrass(g, 2.5 * FT, -0.5 * FT, 4.5, rnd);
  // Bills blown down, flat in the trodden patch.
  const paper = v === 3 ? 0xd8ccae : 0xf0e6c8;
  const bills: [number, number, number][] = [[[-0.7, 1.25, -0.35]], [[0.8, 1.35, 0.4]], [[-0.9, 1.1, 0.25], [0.95, 1.5, -0.5]], [[0.4, 1.3, 0.6]]][v] as [number, number, number][];
  for (const [x, y, a] of bills) fallenBill(g, x * FT, y * FT, 0.7 * FT, 0.9 * FT, a, paper, rnd);
  if (v === 2) footGrass(g, 0.1 * FT, 1.95 * FT, 3.5, rnd);
  if (v === 1 || v === 3) {
    // The roof from above: a narrow strip over board and posts, the north slope lit, the south in shade,
    // shingle joints in two staggered courses a slope, a ridge board along the middle.
    const roof = v === 1 ? [0x5a3a22, 0x7a5232, 0x9e7248] : [0x5a564e, 0x7a766c, 0x9c988c];
    const [o, n, s] = [2.45 * FT, -0.42 * FT, 0.42 * FT];
    const j = () => (rnd() - 0.5) * 1;
    const north = [-o + j(), n + j(), o + j(), n + j(), o, 0, -o, 0];
    const south = [-o, 0, o, 0, o + j(), s + j(), -o + j(), s + j()];
    g.poly(north).fill(roof[2]);
    g.poly(south).fill(roof[1]);
    for (const [y0, y1] of [
      [n, 0],
      [0, s],
    ]) {
      for (let x = -o + 7 + (y0 < 0 ? 0 : 6); x < o - 4; x += 13) g.moveTo(x, y0 + 1.5).lineTo(x + 0.3, y1 - 1.5);
    }
    g.stroke({ width: 1, color: roof[0], alpha: 0.6, cap: 'round' });
    g.moveTo(-o + 1, s - 1.2).lineTo(o - 1, s - 1.2).stroke({ width: 1.4, color: roof[0], alpha: 0.8 });
    g.poly([-o, -1.2, o, -1.2, o, 1.2, -o, 1.2]).fill(roof[0]);
    g.moveTo(-o, -0.6).lineTo(o, -0.6).stroke({ width: 0.8, color: roof[2], alpha: 0.9 });
    g.poly([...north.slice(0, 4), ...south.slice(4, 8)]).stroke({ width: OUT, color: INK, join: 'round' });
    return;
  }
  // The board's top edge between the posts (a second plank, its cross-rail, behind it on v2), a line of grain.
  if (v === 2) blockTop(g, -hw, -0.5 * FT, hw, -0.5 * FT + t * 1.6, wood.map((c) => shade(c, 0.94)), rnd);
  blockTop(g, -hw - 1, -t, hw + 1, t, wood, rnd);
  g.moveTo(-hw + 5, 0.5).lineTo(-0.3 * FT, -0.3).moveTo(0.4 * FT, 0.2).lineTo(hw - 6, 0.4).stroke({ width: 0.9, color: shade(wood[0], 0.9), cap: 'round' });
  // The posts' tops: square, end grain.
  for (const sx of [-1, 1]) {
    blockTop(g, sx * px - ps, -ps, sx * px + ps, ps, wood, rnd);
    g.circle(sx * px - 0.5, -0.5, ps * 0.45).stroke({ width: 0.9, color: wood[0], alpha: 0.8 });
  }
}

/** Signpost, straight down: the post's top a small inked disc (its end grain showing) on a few stones and grass
 * at its foot, its finger boards' top edges radiating flat from it, pointed at their ends, the higher ones over the
 * lower: two (v0), three (v1), a weathered post with one finger broken off at its peg and hanging by a nail, lichen
 * on the post's top (v2), four weathered fingers at uneven angles at a crossroads (v3); ink ticks for the lettering. */
export function signpost(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const wood = v === 2 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  // Fingers: [angle (0 east, +y south), height (ft, for which lies over which), length (ft)].
  const looks: [number, number, number][][] = [
    [[-0.3, 7.4, 2.6], [Math.PI + 0.35, 6.6, 2.5]],
    [[-0.25, 7.6, 2.6], [Math.PI - 0.45, 6.9, 2.4], [0.75, 6.1, 2.3]],
    [[Math.PI + 0.3, 7.4, 2.3], [-1.05, 6.8, 2.0]],
    [[-0.22, 7.6, 2.5], [Math.PI + 0.62, 7.3, 2.2], [1.0, 6.8, 2.1], [Math.PI - 0.28, 6.5, 2.4]],
  ];
  // (The crossroads post's fingers bleached by the weather, still wood.)
  const paint = v === 3 ? [0x7e6e54, 0xa89676, 0xc6b494] : wood;
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + rnd() * 0.8;
    stone(g, Math.cos(a) * 0.5 * FT, Math.sin(a) * 0.46 * FT + 1, 0.17 * FT + rnd() * 0.8, STONE[i % 2], rnd, 7);
  }
  footGrass(g, -0.95 * FT, 0.55 * FT, 5, rnd);
  footGrass(g, 0.7 * FT, 0.85 * FT, 4, rnd);
  // A finger's top edge from u0 to `len` ft out along `a`, `th` ft thick: lit along the side toward the light.
  const finger = (a: number, len: number, th = 0.34, u0 = 0.15, broken = false) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const at = (u: number, w: number) => [c * u * FT - s * w * FT, s * u * FT + c * w * FT];
    const h = th / 2;
    const tip = broken ? [...at(len, h * 0.4), ...at(len - 0.12, -h * 0.2), ...at(len + 0.06, -h * 0.7)] : [...at(len - 0.42, h), ...at(len, 0), ...at(len - 0.42, -h)];
    const pts = [...at(u0, h), ...tip, ...at(u0, -h)];
    if (broken) pts.splice(2, 0, ...at(len - 0.1, h));
    g.poly(pts).fill(paint[1]);
    // (The +w side faces the light when its normal (-s, c) points NW.)
    const litSide = -s * -1 + c * -1 > 0 ? 1 : -1;
    g.moveTo(...(at(u0 + 0.05, litSide * h * 0.5) as [number, number])).lineTo(...(at(len - 0.5, litSide * h * 0.5) as [number, number]));
    g.stroke({ width: 1.2, color: paint[2], alpha: 0.9, cap: 'round' });
    if (!broken) {
      // The lettering's top edges: a few ink ticks along it.
      for (let u = u0 + 0.5; u < len - 0.55; u += 0.26 + rnd() * 0.12) g.moveTo(...(at(u, -h * 0.35) as [number, number])).lineTo(...(at(u + 0.04, h * 0.35) as [number, number]));
      g.stroke({ width: 1.1, color: INK, alpha: 0.7, cap: 'round' });
    }
    g.poly(pts).stroke({ width: LINE, color: INK, join: 'round' });
  };
  const fingers = [...looks[v]].sort((p, q) => p[1] - q[1]);
  if (v === 2) {
    // The broken one hangs steeply from its nail, so from above it shows short, under the others.
    finger(0.2, 1.15, 0.26, 0.2);
    g.circle(0.33 * FT, 0.07 * FT, 1.4).fill(IRON[2]).stroke({ width: 1, color: INK });
  }
  for (const [a, , len] of fingers) finger(a, len);
  if (v === 2) finger(0.05, 0.62, 0.34, 0.15, true);
  // The post's top: end grain, lit toward the light.
  const R = 0.32 * FT;
  g.circle(0, 0, R).fill(wood[1]);
  g.circle(-R * 0.18, -R * 0.2, R * 0.66).fill(wood[2]);
  g.circle(0.4, 0.4, R * 0.62).stroke({ width: 1, color: wood[0], alpha: 0.7 });
  g.circle(0.2, 0.2, R * 0.3).stroke({ width: 1, color: wood[0], alpha: 0.8 });
  g.moveTo(R * 0.15, -R * 0.1).lineTo(R * 0.8, -R * 0.35).stroke({ width: 1, color: wood[0], alpha: 0.7, cap: 'round' });
  g.circle(0, 0, R).stroke({ width: LINE, color: INK });
  if (v === 2) g.circle(-R * 0.3, R * 0.25, 1.8).fill({ color: 0x9aa86a, alpha: 0.9 });
}

/** Stocks, straight down (as the stocks indoors): a low, pale timber platform, the tops of two posts, and between
 * them the top edge of the board (about 5 ft by 0.6), split along its middle, the head hole and wrist holes showing as notches in the
 * split; an iron hinge at one end and a padlock's hasp at the other, straw scattered before it; ankle stocks with a
 * bench behind to sit on (v1); a pillory, a shorter board on one post (v2); rotten fruit thrown at it (v3). */
export function stocks(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // (The platform low and pale beside the board, so the board reads as the thing standing up.)
  const plat = [0x9a7c58, 0xae9068, 0xbea078];
  const [px, py, pc] = v === 2 ? [1.7, 1.0, -0.2] : v === 1 ? [2.9, 1.25, -0.4] : [2.9, 0.95, 0];
  const deck = handBox(-px * FT, (pc - py) * FT, px * FT, (pc + py) * FT, rnd, 0.8);
  g.poly(deck).fill(plat[1]).stroke({ width: LINE * 0.8, color: shade(plat[0], 0.7), join: 'round' });
  // The platform's boards along x.
  for (let y = pc - py + 0.65; y < pc + py - 0.2; y += 0.65) {
    const gap = (rnd() - 0.3) * px * FT;
    g.moveTo(-px * FT + 3, y * FT + (rnd() - 0.5)).lineTo(gap - 2, y * FT).moveTo(gap + 2, y * FT + (rnd() - 0.5)).lineTo(px * FT - 3, y * FT);
  }
  g.stroke({ width: 1, color: shade(plat[0], 0.9), cap: 'round' });
  if (v !== 1 && v !== 2) {
    // Straw before it, where the sitter's knees go.
    for (let i = 0; i < 14; i++) {
      const [x, y, a] = [(rnd() - 0.5) * 3.6 * FT, (0.45 + rnd() * 0.45) * FT, (rnd() - 0.5) * 1.6];
      const l = 3 + rnd() * 3;
      g.moveTo(x, y).lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l).stroke({ width: 1.2, color: i % 3 ? 0xd8b860 : 0xb0903e, cap: 'round' });
    }
  }
  if (v === 1) blockTop(g, -1.9 * FT, -1.4 * FT, 1.9 * FT, -0.85 * FT, plat.map((c) => shade(c, 0.85)), rnd);
  const hw = v === 2 ? 1.4 : 2.4;
  // The posts: at the ends, or one behind the middle of a pillory's board.
  const posts = v === 2 ? [[0, -0.5]] : [[-hw - 0.25, 0], [hw + 0.25, 0]];
  for (const [x, y] of posts) blockTop(g, (x - 0.25) * FT, (y - 0.25) * FT, (x + 0.25) * FT, (y + 0.25) * FT, WOOD, rnd);
  const d = 0.3 * FT;
  for (const [y0, y1] of [
    [-d, 0],
    [0, d],
  ]) {
    blockTop(g, -hw * FT, y0, hw * FT, y1, WOOD, rnd, LINE * 0.75);
  }
  // The holes, half cut in each plank: dark notches straddling the split.
  const holes = v === 1 ? [[-1.2, 0.15], [-0.62, 0.15], [0.62, 0.15], [1.2, 0.15]] : v === 2 ? [[-0.75, 0.14], [0, 0.24], [0.75, 0.14]] : [[-0.95, 0.15], [0, 0.25], [0.95, 0.15]];
  for (const [x, s] of holes) {
    g.ellipse(x * FT, 0, s * FT, Math.min(s * FT * 0.75, d * 0.85)).fill(0x1e140c);
    g.moveTo(x * FT - s * FT, 0).quadraticCurveTo(x * FT, -s * FT * 1.24, x * FT + s * FT, 0).stroke({ width: 1.1, color: INK, cap: 'round' });
    g.moveTo(x * FT - s * FT, 0).quadraticCurveTo(x * FT, s * FT * 1.24, x * FT + s * FT, 0).stroke({ width: 1.1, color: INK, cap: 'round' });
  }
  g.poly(handBox(-hw * FT, -d, hw * FT, d, rnd, 0.6)).stroke({ width: OUT, color: INK, join: 'round' });
  // An iron hinge strap across the split at one end, a hasp and padlock at the other.
  const e = (hw - 0.3) * FT;
  g.rect(e - 6, -d - 1, 3.4, d * 2 + 2).fill(IRON[1]).stroke({ width: 1.1, color: INK });
  g.rect(e - 0.5, -d * 0.6, 3.2, d * 1.4).fill(IRON[2]).stroke({ width: 1.1, color: INK });
  g.roundRect(e - 0.6, d + 0.4, 4.4, 4.2, 1.2).fill(IRON[0]).stroke({ width: 1.1, color: INK });
  g.circle(e + 1.6, d + 2.5, 0.7).fill(IRON[3]);
  if (v === 3) {
    // Rotten fruit: splats on the board and the boards before it, a whole one rolled off.
    const fruit = [0xa8382e, 0x8a9a3a, 0x7a5030, 0xc8642e];
    for (let i = 0; i < 6; i++) {
      const [x, y] = [(rnd() - 0.5) * 3.8 * FT, (i < 2 ? (rnd() - 0.5) * 0.4 : 0.5 + rnd() * 0.8) * FT];
      const col = fruit[i % 4];
      g.poly(blob(x, y, 2.6 + rnd() * 2, 8, 0.6, rnd)).fill({ color: col, alpha: 0.85 });
      for (let k = 0; k < 3; k++) g.circle(x + (rnd() - 0.5) * 9, y + (rnd() - 0.5) * 7, 0.9 + rnd() * 0.6).fill({ color: col, alpha: 0.8 });
    }
    g.circle(1.6 * FT, 1.15 * FT, 0.22 * FT).fill(0x8a2a24).stroke({ width: LINE * 0.7, color: INK });
    g.circle(1.6 * FT - 1, 1.15 * FT - 1, 0.09 * FT).fill(0xc85a4a);
  }
}

/** A street lantern straight down at (x, y), `s` px each side of its middle: its cap of four facets up to a
 * finial (the north and west lit, the south and east in shade), its glass showing past the eaves as a glowing
 * rim when lit, a pale grey one when out. */
function lanternTop(g: Graphics, x: number, y: number, s: number, lit: boolean, rnd: () => number) {
  const j = () => (rnd() - 0.5) * 0.8;
  const glass = handBox(x - s * 1.12, y - s * 1.12, x + s * 1.12, y + s * 1.12, rnd, 0.6);
  if (lit) g.circle(x, y, s * 1.7).fill({ color: 0xffd878, alpha: 0.3 });
  g.poly(glass).fill(lit ? 0xffeeb0 : 0xb4bab8).stroke({ width: 1.1, color: INK, join: 'round' });
  const cs = [x - s + j(), y - s + j(), x + s + j(), y - s + j(), x + s + j(), y + s + j(), x - s + j(), y + s + j()];
  const [ax, ay] = [x - s * 0.08, y - s * 0.08];
  const tones = [IRON[2], IRON[0], shade(IRON[0], 0.85), IRON[1]];
  for (let i = 0; i < 4; i++) g.poly([cs[i * 2], cs[i * 2 + 1], cs[(i * 2 + 2) % 8], cs[(i * 2 + 3) % 8], ax, ay]).fill(tones[i]);
  for (let i = 0; i < 4; i++) g.moveTo(cs[i * 2], cs[i * 2 + 1]).lineTo(ax, ay);
  g.stroke({ width: 1, color: INK, alpha: 0.75 });
  g.poly(cs).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(ax, ay, Math.max(1.6, s * 0.16)).fill(IRON[3]).stroke({ width: 1.1, color: INK });
}

/** An iron rod from `a` to `b` (an ink line under a lit one), or one of another `color`. */
export function rod(g: Graphics, a: [number, number], b: [number, number], w = 2.4, color = IRON[2]) {
  inked(g, () => g.moveTo(...a).lineTo(...b), w, color);
}

/** A stroke inked: the path `trace` draws, stroked in ink 2.4 px wider, then in `color` over it. */
export function inked(g: Graphics, trace: () => void, w: number, color: number) {
  for (const [width, c] of [
    [w + 2.4, INK],
    [w, color],
  ] as const) {
    trace();
    g.stroke({ width, color: c, cap: 'round', join: 'round' });
  }
}

/** Lamp post, straight down: the lantern's top, its foot hidden beneath it, a soft warm pool of light on the
 * ground round it when lit: an iron post's lantern, glowing (v0); a timber post's top with a short iron scroll
 * bracket out over the lantern hung under it (v1); an iron post with two arms swept out, a lantern standing on
 * each (v2); a lamp gone out, the ends of the lamplighter's crossbar showing past it (v3). */
export function lampPost(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const pool = (x: number, rx: number, ry: number) => {
    for (let i = 0; i < 5; i++) g.ellipse(x, 0, rx * (1 - i * 0.16), ry * (1 - i * 0.16)).fill({ color: 0xf8c860, alpha: 0.07 });
  };
  const s = 0.72 * FT;
  if (v === 1) {
    const lx = 0.9 * FT;
    pool(lx * 0.6, 1.9 * FT, 1.9 * FT);
    blockTop(g, -1.1 * FT, -0.3 * FT, -0.5 * FT, 0.3 * FT, [0x3e342c, 0x5a4c40, 0x786858], rnd);
    lanternTop(g, lx, 0, s * 0.85, true, rnd);
    // The bracket over it: out from the post, a curl rising at its end (a loop from above), the chain's ring.
    inked(g, () => g.moveTo(-0.5 * FT, 0).lineTo(lx + 0.55 * FT, -1), 2, IRON[2]);
    inked(g, () => g.moveTo(lx + 0.55 * FT, -1).quadraticCurveTo(lx + 0.8 * FT, -1.5, lx + 0.78 * FT, -0.12 * FT), 1.8, IRON[2]);
    g.circle(lx + 0.72 * FT, -0.14 * FT, 1.9).fill(IRON[1]).stroke({ width: 1.1, color: INK });
    g.circle(lx, -0.5, 1.8).stroke({ width: 2.6, color: INK }).circle(lx, -0.5, 1.8).stroke({ width: 1, color: IRON[3] });
    return;
  }
  if (v === 2) {
    pool(0, 2.4 * FT, 1.55 * FT);
    const lx = 1.7 * FT;
    for (const side of [-1, 1]) inked(g, () => g.moveTo(0, 0).quadraticCurveTo(side * lx * 0.55, 0.22 * FT, side * lx, 0), 2.4, IRON[2]);
    for (const side of [-1, 1]) lanternTop(g, side * lx, 0, s * 0.8, true, rnd);
    g.circle(0, 0, 0.22 * FT).fill(IRON[1]).stroke({ width: LINE, color: INK });
    g.circle(-1, -1, 0.09 * FT).fill(IRON[3]);
    return;
  }
  if (v === 0) pool(0, 2.3 * FT, 2.3 * FT);
  if (v === 3) rod(g, [-1.05 * FT, 0.12 * FT], [1.05 * FT, 0.12 * FT], 2.2);
  lanternTop(g, 0, 0, s, v === 0, rnd);
}

/** A cross's shadow on the steps below it (SE, from the light in the NW), from (x, y): the shaft `len` px,
 * the arms across it near the end in proportion, a ring round the crossing on a wheel cross. */
function crossShadow(g: Graphics, x: number, y: number, len: number, wheel: boolean) {
  const sh = { color: 0x2a1a20, alpha: 0.38 };
  const [ux, uy] = [Math.SQRT1_2, Math.SQRT1_2];
  const at = (u: number, k: number) => [x + ux * u - uy * k, y + uy * u + ux * k];
  const w = len * 0.085;
  const c = len * 0.72;
  const arm = len * 0.3;
  g.poly([...at(0, -w), ...at(c - w, -w), ...at(c - w, -arm), ...at(c + w, -arm), ...at(c + w, -w), ...at(len, -w), ...at(len, w), ...at(c + w, w), ...at(c + w, arm), ...at(c - w, arm), ...at(c - w, w), ...at(0, w)]).fill(sh);
  if (wheel) {
    const [rx, ry] = at(c, 0);
    g.circle(rx, ry, len * 0.17).stroke({ width: w * 1.2, ...sh });
  }
}

/** A stone cross straight down from over its head, at (x, y), `k` times life size: the tops of its arms a bar
 * across x, lit along the north edge, the head (the highest part) a little paler at the middle, inked; its shape
 * as a cross is told by its shadow (`crossShadow`). */
function crossTop(g: Graphics, x: number, y: number, k: number, tones: readonly number[], rnd: () => number) {
  const [arm, d, sw] = [1.8 * FT * k, 0.5 * FT * k, 0.62 * FT * k];
  const ink = k >= 0.8 ? OUT : LINE;
  blockTop(g, x - arm, y - d, x + arm, y + d, [shade(tones[0], 0.85), tones[0], tones[1]], rnd, ink);
  blockTop(g, x - sw, y - d * 1.08, x + sw, y + d * 1.08, tones, rnd, ink * 0.8);
}

/** A regular polygon's corners: `n` of them round (0, 0) at radius `R`, the first at angle `ph`. */
export function ngon(R: number, n: number, ph: number): number[][] {
  return Array.from({ length: n }, (_, i) => [Math.cos(ph + (i / n) * Math.PI * 2) * R, Math.sin(ph + (i / n) * Math.PI * 2) * R]);
}

/** A regular polygon's radius at angle `a` (circumradius `R`, `n` sides, a corner at `ph`), as `around` takes it. */
export function polyR(R: number, n: number, ph: number) {
  const span = (Math.PI * 2) / n;
  return (a: number) => (R * Math.cos(Math.PI / n)) / Math.cos(((((a - ph) % span) + span) % span) - Math.PI / n);
}

/** Market cross, straight down: stepped octagonal treads (lit on the NW, a joint here and there), a socket
 * stone and on it the top of a stone cross, its arms a bar from above, its shadow falling down the steps in the
 * shape of a cross (v0); a buttercross, its octagonal roof on pillars over the steps, a little cross on its peak (v1); an old
 * one, a step cracked, moss and flowers left at its foot (v2); a wheel cross on square steps (v3). */
export function marketCross(g: Graphics, r: number, rnd: () => number, v: number) {
  const st = v === 2 ? STONE[3] : STONE[0];
  const sides = v === 3 ? 4 : 8;
  const ph = v === 3 ? Math.PI / 4 : Math.PI / 8;
  const tier = (R: number, ink: number, dk = 1) => {
    const pts = ngon(R, sides, ph);
    g.poly(pts.flat()).fill(shade(st[1], dk)).stroke({ width: ink, color: INK, join: 'round' });
    // Each edge's band: lit toward the light, shaded away from it.
    const d = Math.min(0.42 * FT, R * 0.14);
    for (let i = 0; i < sides; i++) {
      const [a, b] = [pts[i], pts[(i + 1) % sides]];
      const k = (R - d) / R;
      const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
      g.poly([a[0], a[1], b[0], b[1], b[0] * k, b[1] * k, a[0] * k, a[1] * k]).fill(shade(st[1], dk * (0.95 + 0.17 * lit(mid))));
    }
    g.poly(pts.flat()).stroke({ width: ink, color: INK, join: 'round' });
  };
  const steps = v === 1 ? [r * 0.97, r * 0.84] : v === 3 ? [r * 1.02, r * 0.78, r * 0.55] : [r * 0.95, r * 0.73, r * 0.51];
  // The bottom step darker, so the steps stand off the paving round them.
  steps.forEach((R, i) => tier(R, i ? 2 : 3, i ? 1 : 0.8));
  // A joint across each tread here and there.
  const edge = (R: number, a: number) => polyR(R, sides, ph)(a);
  for (let i = 0; i + 1 < steps.length; i++) {
    for (let k = 0; k < 3; k++) {
      const a = ph + 0.4 + k * 2.3 + i * 1.1;
      const [r0, r1] = [edge(steps[i + 1], a) + 1.5, edge(steps[i], a) - 1.5];
      g.moveTo(Math.cos(a) * r0, Math.sin(a) * r0).lineTo(Math.cos(a) * r1, Math.sin(a) * r1);
    }
  }
  g.stroke({ width: 1.2, color: shade(st[0], 0.85) });
  const cross = [0xb8b0a0, 0xd8d1c2, 0xeee8da];
  if (v === 1) {
    // Square pillars at the corners of the upper tread, standing out past the roof's corners, the dark of the
    // open hall under the eaves; the roof over them: eight planes up to a peak, each lit as it faces the
    // light, slates lapped along the eaves; a little cross on the peak.
    const R = r * 0.62;
    g.poly(ngon(R * 1.16, 8, ph).flat()).fill(0x221a14).stroke({ width: 2, color: INK, join: 'round' });
    for (const [x, y] of ngon(r * 0.76, 8, ph)) {
      const c = 0.42 * FT;
      g.rect(x - c + 2, y - c + 2, c * 2, c * 2).fill({ color: 0x2a1a20, alpha: 0.35 });
      bevel(g, x - c, y - c, x + c, y + c, 2, st, 1.8);
    }
    const pts = ngon(R, 8, ph);
    g.poly(pts.flat().map((p) => p + 5)).fill({ color: 0x140c08, alpha: 0.55 });
    for (let i = 0; i < 8; i++) {
      const [a, b] = [pts[i], pts[(i + 1) % 8]];
      const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
      const tone = shade(0x8a7a66, 0.84 + 0.28 * lit(mid));
      g.poly([0, 0, a[0], a[1], b[0], b[1]]).fill(tone);
      for (let k = 0; k < 4; k++) {
        const f = 0.55 + 0.12 * (k % 2) + 0.1 * rnd();
        const t = 0.2 + 0.2 * k;
        const [x, y] = [(a[0] + (b[0] - a[0]) * t) * f, (a[1] + (b[1] - a[1]) * t) * f];
        const [dx, dy] = [(b[0] - a[0]) * 0.08 * f, (b[1] - a[1]) * 0.08 * f];
        g.moveTo(x - dx, y - dy).lineTo(x + dx, y + dy);
      }
      g.stroke({ width: 1.2, color: shade(tone, 0.72), cap: 'round' });
    }
    // Hips from the peak to the corners, lead along them.
    for (const [x, y] of pts) g.moveTo(0, 0).lineTo(x, y);
    g.stroke({ width: 2.4, color: shade(0x6e6a62, 0.75) });
    g.poly(pts.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    g.circle(0, 0, 0.4 * FT).fill(st[2]).stroke({ width: 1.5, color: INK });
    crossTop(g, 0, 0, 0.32, cross, rnd);
    return;
  }
  if (v === 2) {
    // A crack through the lowest step with a stone come loose on the tread, moss on the north treads,
    // flowers laid on the steps.
    const a = ph + (1.3 / 8) * Math.PI * 2;
    const [r0, r1] = [edge(steps[1], a), edge(steps[0], a)];
    g.moveTo(Math.cos(a) * r0, Math.sin(a) * r0)
      .lineTo(Math.cos(a + 0.06) * (r0 + r1) * 0.5, Math.sin(a + 0.06) * (r0 + r1) * 0.5)
      .lineTo(Math.cos(a - 0.03) * r1, Math.sin(a - 0.03) * r1)
      .stroke({ width: 1.6, color: INK, join: 'round' });
    stone(g, Math.cos(a + 0.14) * (r0 + r1) * 0.5, Math.sin(a + 0.14) * (r0 + r1) * 0.5, 0.28 * FT, st, rnd, 6);
    moss(g, -steps[1] * 0.25, -steps[1] * 0.86, 0.32 * FT, rnd);
    moss(g, -steps[0] * 0.7, -steps[0] * 0.5, 0.26 * FT, rnd);
    const fa = ph + (2.5 / 8) * Math.PI * 2;
    const fr = (steps[1] + steps[2]) * 0.5;
    for (let i = 0; i < 3; i++) g.circle(Math.cos(fa) * fr + i * 4 - 4, Math.sin(fa) * fr + (i % 2) * 3 - 1, 2.4).fill([0xe0533d, 0xf2c94c, 0xf6efe0][i]).stroke({ width: 0.8, color: INK });
  }
  // The socket stone, the cross's shadow falling down the steps, then the cross's top over it (a little over
  // life size, as monuments are made).
  const s0 = steps[steps.length - 1] * 0.5;
  bevel(g, -s0, -s0, s0, s0, 3, st.map((c) => shade(c, 1.04)), 2);
  const k = 1.2;
  crossShadow(g, 0.2 * FT, 0.2 * FT, r * 0.8, v === 3);
  crossTop(g, 0, 0, k, v === 2 ? cross.map((c) => shade(c, 0.9)) : cross, rnd);
  if (v === 2) moss(g, -1.2 * FT * k, -0.2 * FT * k, 0.22 * FT, rnd);
}
