import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { boughs, lit, whorl } from './plants';
import { BARK, STONE, moss, stone } from './stones';
import { endGrain } from './wood';
import { bevel } from './yard';

/** A quad's corners as one point list, for `poly`. */
export function quad4(a: [number, number], b: [number, number], c: [number, number], d: [number, number]): number[] {
  return [...a, ...b, ...c, ...d];
}

/** Where an upright thing drawn from a high angle stands in its frame: its foot, px below the centre, by kind
 * id (its drop shadow falls from there: `BattlemapLayer`). Things seen from straight above stand at the centre. */
export const UPRIGHT_FOOT: Record<number, number> = { 47: 1.7 * FT, 48: 1.85 * FT, 49: 1.1 * FT, 50: 2.15 * FT, 57: 1.3 * FT, 60: 2.3 * FT, 101: 2.2 * FT };

/** The high-angle view of upright things (as the headstone; the style guide's `hk` 0.5, `dk` 0.87): a ground
 * point `x` across and `d` deep (toward the viewer) and `h` up, in feet, to frame pixels, the foot at (0, `foot`). */
export function upright(foot: number) {
  return (x: number, d: number, h: number): [number, number] => [x * FT, foot + (d * 0.87 - h * 0.5) * FT];
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

/** Notice board, from a high angle (it only reads from the side): a planked board on two posts, papers pinned
 * to it askew (lines of writing, a seal, a sketched face on a wanted bill), the board's top edge showing;
 * a little shingled roof over some (v1, v3), the last weathered grey. */
export function noticeBoard(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[47]);
  const wood = v === 3 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  const ink = { width: 2, color: INK, join: 'round' as const };
  const [hw, b0, b1, top] = [2.6, 2.6, 6, v === 1 || v === 3 ? 6.6 : 6.4];
  for (const x of [-hw + 0.1, hw - 1.1]) footGrass(g, x * FT, P(0, 0.15, 0)[1], 0.13 * r + 1, rnd);
  // The posts.
  for (const x of [-hw + 0.5, hw - 0.5]) {
    g.poly(quad4(P(x - 0.25, 0, 0), P(x + 0.25, 0, 0), P(x + 0.25, 0, top), P(x - 0.25, 0, top))).fill(wood[0]).stroke(ink);
    g.poly(quad4(P(x - 0.22, 0, 0.1), P(x - 0.02, 0, 0.1), P(x - 0.02, 0, top), P(x - 0.22, 0, top))).fill(wood[1]);
  }
  // The board: its top edge seen from above, then the face, planked across.
  g.poly(quad4(P(-hw, -0.4, b1), P(hw, -0.4, b1), P(hw, 0, b1), P(-hw, 0, b1))).fill(wood[2]).stroke(ink);
  g.poly(quad4(P(-hw, 0, b0), P(hw, 0, b0), P(hw, 0, b1), P(-hw, 0, b1))).fill(wood[1]).stroke(ink);
  for (const h of [b0 + 1.15, b0 + 2.3]) g.moveTo(...P(-hw + 0.1, 0, h)).lineTo(...P(hw - 0.1, 0, h));
  g.stroke({ width: 1.2, color: wood[0], alpha: 0.8 });
  g.poly(quad4(P(-hw, 0, b1 - 0.35), P(hw, 0, b1 - 0.35), P(hw, 0, b1), P(-hw, 0, b1))).fill({ color: 0xfff1cf, alpha: 0.25 });
  // Papers: each a little askew, with writing; the biggest a bill with a face, one sealed.
  const papers: [number, number, number, number][] = v === 2 ? [[-1.5, 4.2, 1.5, 2.4], [0.6, 4.4, 1, 1.4], [1.7, 3.6, 0.8, 1.1]] : [[-1.6, 4.4, 1, 1.4], [-0.2, 4.1, 1.2, 1.7], [1.4, 4.6, 0.9, 1.2], [0.9, 3.2, 0.8, 0.9]];
  papers.forEach(([x, h, pw, ph], i) => {
    if (v === 3 && i === 3) return;
    const t = (rnd() - 0.5) * 0.25;
    const c = (u: number, k: number) => P(x + u * pw * 0.5 - k * t, 0, h + k * ph * 0.5 + u * t * 0.4);
    const paper = v === 3 && i === 2 ? 0xd8ccae : [0xf2e8cc, 0xf6efe0, 0xe8dcc0][i % 3];
    g.poly(quad4(c(-1, -1), c(1, -1), c(1, 1), c(-1, 1))).fill(paper).stroke({ width: 1.2, color: INK, join: 'round' });
    if (v === 2 && i === 0) {
      // WANTED: a bold line, a head and shoulders sketched, a line under.
      g.moveTo(...c(-0.6, 0.72)).lineTo(...c(0.6, 0.72)).stroke({ width: 2.2, color: 0x3a2a1e, cap: 'round' });
      g.poly(oval(...c(0, 0.18), 0.3 * FT, 0.2 * FT, 0, 12)).stroke({ width: 1.3, color: 0x5a4632 });
      g.moveTo(...c(-0.55, -0.42)).quadraticCurveTo(...c(0, -0.05), ...c(0.55, -0.42)).stroke({ width: 1.3, color: 0x5a4632 });
      g.moveTo(...c(-0.5, -0.7)).lineTo(...c(0.5, -0.7)).stroke({ width: 1.2, color: 0x6a5642, cap: 'round' });
    } else {
      for (let k = 0; k < 3; k++) g.moveTo(...c(-0.6, 0.5 - k * 0.42)).lineTo(...c(0.6 - 0.3 * rnd(), 0.5 - k * 0.42));
      g.stroke({ width: 1, color: 0x6a5642, cap: 'round' });
    }
    if (i === 1) g.circle(...c(0.45, -0.6), 1.8).fill(0xb5323a).stroke({ width: 0.8, color: INK });
    g.circle(...c(0, 0.85), 1.1).fill(i % 2 ? 0xc9a24a : 0x9a3a2e);
  });
  if (v === 3) g.poly(quad4(P(0.6, 0, 3.35), P(1.3, 0, 3.15), P(1.25, 0, 2.75), P(0.65, 0, 2.85))).fill(0xc8bc9e).stroke({ width: 1, color: INK, join: 'round' });
  if (v === 1 || v === 3) {
    // The roof: its far plane a lit strip behind the ridge, the near one in shade over the board's top.
    // (The slate one lighter and slighter, so the board shows under it.)
    const [eave, ridge, o, near] = v === 1 ? [b1 + 0.3, b1 + 1.8, hw + 0.45, 1] : [b1 + 0.35, b1 + 1.4, hw + 0.15, 0.6];
    const roof = v === 1 ? [0x7a3a2a, 0xa85a3e] : [0x7e7a72, 0xa29e94];
    const at = (x: number, f: number) => P(x, f * near, ridge - (ridge - eave) * Math.abs(f));
    g.poly(quad4(at(-o, -0.6), at(o, -0.6), at(o, 0), at(-o, 0))).fill(roof[1]).stroke(ink);
    g.poly(quad4(at(-o, 0), at(o, 0), at(o, 1), at(-o, 1))).fill(roof[0]).stroke(ink);
    // Shingles in staggered courses.
    for (const f of [0.34, 0.67]) g.moveTo(...at(-o + 0.05, f)).lineTo(...at(o - 0.05, f));
    for (let row = 0; row < 3; row++) {
      for (let x = -o + 0.35 + (row % 2) * 0.35; x < o - 0.2; x += 0.7) g.moveTo(...at(x, row * 0.33 + 0.03)).lineTo(...at(x, row * 0.33 + 0.31));
    }
    g.stroke({ width: 1, color: shade(roof[0], 0.68) });
    g.moveTo(...at(-o, 0)).lineTo(...at(o, 0)).stroke({ width: 2, color: shade(roof[0], 0.55) });
  }
}

/** Signpost, from a high angle: a post on a few stones, finger boards pointing off it with their lettering
 * (those pointing away behind it, those toward the viewer in front): two (v0), three (v1), a weathered post
 * with one finger broken off and hanging by a nail (v2), four painted fingers under a little cap at a
 * crossroads (v3). */
export function signpost(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[48]);
  const wood = v === 2 ? [0x5e5850, 0x857c70, 0xaaa092] : WOOD;
  const ink = { width: 2, color: INK, join: 'round' as const };
  const top = 8.6;
  // Fingers: [angle on the ground (0 east, +y toward the viewer), height, length].
  const looks: [number, number, number][][] = [
    [[-0.3, 7.4, 2.6], [Math.PI + 0.35, 6.6, 2.5]],
    [[-0.25, 7.6, 2.6], [Math.PI - 0.45, 6.9, 2.4], [0.5, 6.1, 2.3]],
    [[Math.PI + 0.3, 7.4, 2.5]],
    [[-0.4, 7.6, 2.5], [Math.PI + 0.4, 7.6, 2.5], [0.5, 6.8, 2.4], [Math.PI - 0.5, 6.8, 2.4]],
  ];
  const fingers = looks[v];
  const paint = v === 3 ? [0xb8ac90, 0xe8dcc0] : [wood[1], wood[2]];
  // A finger: along it `u` ft and `k` ft up its face, turned down by `droop` (radians, in its own plane) when
  // it hangs loose from its nail.
  const finger = ([a, h, len]: [number, number, number], droop = 0) => {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [dc, ds] = [Math.cos(droop), Math.sin(droop)];
    const at = (u: number, k: number) => {
      const [uu, kk] = [u * dc + k * ds, k * dc - u * ds];
      return P(c * uu, s * uu, h + kk);
    };
    const th = 0.45;
    const board = [...at(0.2, th), ...at(len - 0.5, th), ...at(len, 0), ...at(len - 0.5, -th), ...at(0.2, -th)];
    if (!droop) g.poly([...at(0.2, th + 0.25), ...at(len - 0.5, th + 0.25), ...at(len - 0.42, th), ...at(0.2, th)]).fill(paint[1]).stroke({ width: 1.4, color: INK, join: 'round' });
    g.poly(board).fill(paint[0]).stroke(ink);
    for (let k = 0; k < 3; k++) {
      const u0 = 0.55 + k * 0.6;
      if (u0 + 0.4 > len - 0.5) break;
      g.moveTo(...at(u0, 0.05 * (k % 2 ? -1 : 1))).lineTo(...at(u0 + 0.35 + 0.15 * rnd(), 0.05 * (k % 2 ? -1 : 1)));
    }
    g.stroke({ width: 1.6, color: v === 3 ? 0x2a2420 : shade(wood[0], 0.7), cap: 'round' });
  };
  // Stones and grass at the foot.
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2 + rnd() * 0.6;
    stone(g, Math.cos(a) * 0.55 * FT, P(0, Math.sin(a) * 0.5 + 0.1, 0)[1], 0.3 * FT + rnd() * 2, STONE[i % 2], rnd, 7);
  }
  footGrass(g, -0.9 * FT, P(0, 0.3, 0)[1], 0.2 * r + 2, rnd);
  for (const f of fingers) if (Math.sin(f[0]) < 0) finger(f);
  // The post: its face with a lit west edge, its top (or cap) seen from above.
  const w = 0.3;
  g.poly(quad4(P(-w, 0, 0), P(w, 0, 0), P(w, 0, top), P(-w, 0, top))).fill(wood[0]).stroke(ink);
  g.poly(quad4(P(-w + 0.04, 0, 0.1), P(-0.02, 0, 0.1), P(-0.02, 0, top), P(-w + 0.04, 0, top))).fill(wood[1]);
  if (v === 3) g.poly([...P(-0.5, 0, top), ...P(0.5, 0, top), ...P(0, 0, top + 1)]).fill(0x7a3a2a).stroke(ink);
  else g.poly(quad4(P(-w, -0.5, top), P(w, -0.5, top), P(w, 0, top), P(-w, 0, top))).fill(wood[2]).stroke({ width: 1.5, color: INK, join: 'round' });
  for (const f of fingers) if (Math.sin(f[0]) >= 0) finger(f);
  if (v === 2) {
    // A finger broken off its peg (the splintered stub left on the post), hanging steeply by one nail at its
    // butt; lichen on the post.
    g.poly([...P(w - 0.05, 0, 7.75), ...P(0.75, 0, 7.7), ...P(0.6, 0, 7.5), ...P(0.85, 0, 7.35), ...P(w - 0.05, 0, 7.3)]).fill(paint[0]).stroke({ width: 1.5, color: INK, join: 'round' });
    finger([0, 6.9, 2.1], 1.05);
    g.circle(...P(0.32, 0, 6.9), 1.6).fill(IRON[2]).stroke({ width: 0.8, color: INK });
    for (const h of [3.2, 4.4]) g.circle(...P(-0.05, 0, h), 2.6).fill({ color: 0x9aa86a, alpha: 0.8 });
  }
}

/** Stocks, from a high angle (as the stocks indoors): two posts holding a heavy board split along its
 * middle, a head hole between two wrist holes, a hinge at one end and a padlock at the other, straw before
 * it; ankle stocks with a bench behind to sit on (v1); a pillory, the board at head height on one tall post
 * (v2); rotten fruit thrown at it (v3). */
export function stocks(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const P = upright(UPRIGHT_FOOT[49]);
  const ink = { width: 2, color: INK, join: 'round' as const };
  for (let i = 0; i < 14; i++) {
    const [x, y] = P((rnd() - 0.5) * 6, 0.7 + rnd() * 1.2, 0);
    g.moveTo(x, y).lineTo(x + (rnd() - 0.5) * 7, y + (rnd() - 0.5) * 3).stroke({ width: 1.3, color: 0xc8a860, cap: 'round' });
  }
  const pillory = v === 2;
  const hw = pillory ? 2.3 : 2.9;
  const [b0, b1] = pillory ? [4.6, 5.8] : [1.1, 2.5];
  if (v === 1) {
    // The bench behind: its seat plank seen from above with a lit edge, its front edge, four stout legs.
    const [s0, s1, sh] = [-2, -1.1, 1.6];
    for (const [x, d] of [
      [-2.2, s0 + 0.15],
      [2.2, s0 + 0.15],
      [-2.2, s1 - 0.15],
      [2.2, s1 - 0.15],
    ]) {
      g.poly(quad4(P(x - 0.17, d, sh), P(x + 0.17, d, sh), P(x + 0.17, d, 0), P(x - 0.17, d, 0))).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
    }
    g.poly(quad4(P(-2.7, s0, sh), P(2.7, s0, sh), P(2.7, s1, sh), P(-2.7, s1, sh))).fill(WOOD[2]).stroke(ink);
    g.moveTo(...P(-2.6, s0 + 0.2, sh)).lineTo(...P(2.6, s0 + 0.2, sh)).stroke({ width: 1.5, color: WOOD[3], alpha: 0.7 });
    g.moveTo(...P(-2.6, (s0 + s1) / 2, sh)).lineTo(...P(2.6, (s0 + s1) / 2, sh)).stroke({ width: 1, color: WOOD[0], alpha: 0.7 });
    g.poly(quad4(P(-2.7, s1, sh), P(2.7, s1, sh), P(2.7, s1, sh - 0.35), P(-2.7, s1, sh - 0.35))).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (pillory) {
    // A low platform of planks under the post.
    g.poly(quad4(P(-2, -1, 0.6), P(2, -1, 0.6), P(2, 1, 0.6), P(-2, 1, 0.6))).fill(WOOD[1]).stroke(ink);
    g.poly(quad4(P(-2, 1, 0.6), P(2, 1, 0.6), P(2, 1, 0), P(-2, 1, 0))).fill(WOOD[0]).stroke(ink);
    for (const x of [-1, 0, 1]) g.moveTo(...P(x, -1, 0.6)).lineTo(...P(x, 1, 0.6));
    g.stroke({ width: 1, color: WOOD[0] });
    g.poly(quad4(P(-0.3, 0, 0.6), P(0.3, 0, 0.6), P(0.3, 0, 7.2), P(-0.3, 0, 7.2))).fill(WOOD[0]).stroke(ink);
    g.poly(quad4(P(-0.26, 0, 0.7), P(-0.04, 0, 0.7), P(-0.04, 0, 7.2), P(-0.26, 0, 7.2))).fill(WOOD[1]);
    g.poly(quad4(P(-0.3, -0.5, 7.2), P(0.3, -0.5, 7.2), P(0.3, 0, 7.2), P(-0.3, 0, 7.2))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
  } else {
    for (const x of [-hw + 0.25, hw - 0.25]) {
      g.poly(quad4(P(x - 0.3, 0, 0), P(x + 0.3, 0, 0), P(x + 0.3, 0, b1 + 0.5), P(x - 0.3, 0, b1 + 0.5))).fill(WOOD[0]).stroke(ink);
      g.poly(quad4(P(x - 0.26, 0, 0.1), P(x - 0.04, 0, 0.1), P(x - 0.04, 0, b1 + 0.5), P(x - 0.26, 0, b1 + 0.5))).fill(WOOD[1]);
      g.poly(quad4(P(x - 0.3, -0.5, b1 + 0.5), P(x + 0.3, -0.5, b1 + 0.5), P(x + 0.3, 0, b1 + 0.5), P(x - 0.3, 0, b1 + 0.5))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
    }
  }
  // The board: its top seen from above, the face split along the middle, the holes cut on the split.
  const [l, rr] = pillory ? [-hw, hw] : [-hw + 0.5, hw - 0.5];
  g.poly(quad4(P(l, -0.55, b1), P(rr, -0.55, b1), P(rr, 0, b1), P(l, 0, b1))).fill(WOOD[2]).stroke(ink);
  g.poly(quad4(P(l, 0, b0), P(rr, 0, b0), P(rr, 0, b1), P(l, 0, b1))).fill(WOOD[1]).stroke(ink);
  const mid = (b0 + b1) / 2;
  g.moveTo(...P(l, 0, mid)).lineTo(...P(rr, 0, mid)).stroke({ width: 1.6, color: INK });
  const holes: [number, number][] = v === 1 ? [[-0.9, 0.24], [0.9, 0.24]] : [[0, 0.42], [-1.35, 0.24], [1.35, 0.24]];
  for (const [x, s] of holes) g.poly(oval(...P(x, 0, mid), s * FT, s * FT * 0.8, 0, 14)).fill(0x1e1712).stroke({ width: 1.2, color: INK });
  g.poly(quad4(P(rr - 0.5, 0, mid + 0.35), P(rr + 0.1, 0, mid + 0.35), P(rr + 0.1, 0, mid - 0.45), P(rr - 0.5, 0, mid - 0.45))).fill(IRON[2]).stroke({ width: 1.2, color: INK, join: 'round' });
  g.poly(oval(...P(rr - 0.2, 0, mid + 0.6), 2.2, 2.2, 0, 10)).stroke({ width: 1.3, color: IRON[3] });
  g.poly(quad4(P(l, 0, mid + 0.3), P(l + 0.4, 0, mid + 0.3), P(l + 0.4, 0, mid - 0.3), P(l, 0, mid - 0.3))).fill(IRON[1]);
  if (v === 3) {
    // Rotten fruit and an egg thrown at it: splats on the board and the ground.
    for (const [x, d, h, c] of [
      [-1.9, 0, 2.1, 0xa84a2a],
      [1.7, 0, 1.4, 0x8a9a3a],
      [0.6, 1.4, 0, 0xa84a2a],
      [-0.8, 1.1, 0, 0xe8d8a8],
      [2.4, 1.6, 0, 0x8a9a3a],
    ] as const) {
      const [sx, sy] = P(x, d, h);
      g.poly(blob(sx, sy, 3.2, 8, 0.6, rnd)).fill({ color: c, alpha: 0.85 }).stroke({ width: 1, color: shade(c, 0.6) });
      g.circle(sx - 0.8, sy - 0.8, 1).fill({ color: 0xfff1cf, alpha: 0.6 });
    }
  }
}

/** A square lantern turned corner-on, from high up (as the hanging lantern indoors): its base, the two near
 * glass sides (glowing, or plainly out: pale grey glass with a glint), corner posts, a pyramid cap,
 * a ring on top. `P` the high-angle view, `h` the lantern's foot. */
function streetLantern(g: Graphics, P: (x: number, d: number, h: number) => [number, number], x: number, h: number, lit: boolean) {
  const s = 0.5;
  const sq = (k: number, hh: number) => [P(x, k, hh), P(x + k, 0, hh), P(x, -k, hh), P(x - k, 0, hh)];
  const [bot, top] = [sq(s * 1.05, h), sq(s, h + 1)];
  const flat = (pts: [number, number][]) => pts.flat();
  if (lit) for (let i = 0; i < 5; i++) g.circle(...P(x, 0, h + 0.5), 0.9 * FT - i * 0.12 * FT).fill({ color: 0xf8c860, alpha: 0.07 });
  g.poly(flat(bot)).fill(IRON[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  for (const [i, j, col] of [
    [3, 0, lit ? 0xfff0b0 : 0xc4c8c4],
    [0, 1, lit ? 0xf0c060 : 0x9ea4a6],
  ] as const) {
    g.poly(flat([bot[i], bot[j], top[j], top[i]])).fill(col).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (!lit) g.moveTo(...P(x - 0.32, 0.12, h + 0.8)).lineTo(...P(x - 0.22, 0.24, h + 0.55)).stroke({ width: 1.2, color: 0xf2f4f2, alpha: 0.9, cap: 'round' });
  for (const i of [3, 0, 1]) g.moveTo(...bot[i]).lineTo(...top[i]);
  g.stroke({ width: 1.6, color: IRON[0], cap: 'round' });
  const apex = P(x, 0, h + 1.7);
  const lit4 = [IRON[1], IRON[0], IRON[1], IRON[2]];
  for (let i = 0; i < 4; i++) g.poly(flat([top[i], top[(i + 1) % 4], apex])).fill(lit4[(i + 1) % 4]);
  g.poly(flat(top)).stroke({ width: 1.5, color: INK, join: 'round' });
  for (let i = 0; i < 4; i++) g.moveTo(...top[i]).lineTo(...apex);
  g.stroke({ width: 1, color: INK, alpha: 0.8 });
  g.circle(apex[0], apex[1] - 1.5, 2).stroke({ width: 2.2, color: INK });
  g.circle(apex[0], apex[1] - 1.5, 2).stroke({ width: 1, color: IRON[2] });
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

/** Lamp post, from a high angle: an iron post on a stone foot with a lantern on top, glowing (v0); a timber
 * post with a short iron scroll bracket hanging its lantern (v1); an iron post with two arms swept up, a
 * lantern standing on each (v2); an iron post with its lamp out and a short crossbar for the lamplighter's
 * ladder (v3). */
export function lampPost(g: Graphics, r: number, rnd: () => number, v: number) {
  const P = upright(UPRIGHT_FOOT[50]);
  const ink = { width: 2, color: INK, join: 'round' as const };
  footGrass(g, 0.9 * FT, P(0, 0.4, 0)[1], 0.2 * r + 2, rnd);
  // The foot: a squat stone block, its top lit (a timber post is set straight in the ground).
  if (v !== 1) {
    const st = STONE[0];
    g.poly(quad4(P(-0.6, -0.6, 1), P(0.6, -0.6, 1), P(0.6, 0.6, 1), P(-0.6, 0.6, 1))).fill(st[2]).stroke(ink);
    g.poly(quad4(P(-0.6, 0.6, 1), P(0.6, 0.6, 1), P(0.6, 0.6, 0), P(-0.6, 0.6, 0))).fill(st[1]).stroke(ink);
  }
  const post = v === 1 ? WOOD : IRON;
  const [w, top] = [v === 1 ? 0.3 : 0.24, v === 1 ? 8 : v === 2 ? 7.4 : 8.3];
  const base = v === 1 ? 0 : 1;
  g.poly(quad4(P(-w, 0, base), P(w, 0, base), P(w, 0, top), P(-w, 0, top))).fill(post[1]).stroke(ink);
  g.poly(quad4(P(-w + 0.03, 0, base), P(-0.02, 0, base), P(-0.02, 0, top), P(-w + 0.03, 0, top))).fill(post[2]);
  if (v !== 1) {
    // A collar where the post meets its foot, and one under the lamp.
    for (const h of [1.3, top - 0.3]) g.poly(quad4(P(-0.3, 0, h - 0.25), P(0.3, 0, h - 0.25), P(0.3, 0, h + 0.25), P(-0.3, 0, h + 0.25))).fill(IRON[0]).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  if (v === 1) {
    // A short iron arm from the post's top curling up at its end, a curved brace under it, the lantern hung
    // from the arm on a short chain.
    const [ah, arm] = [top - 0.3, 1.85];
    g.poly(quad4(P(-w, -0.5, top), P(w, -0.5, top), P(w, 0, top), P(-w, 0, top))).fill(WOOD[2]).stroke({ width: 1.5, color: INK, join: 'round' });
    rod(g, P(w, 0, ah), P(arm, 0, ah));
    for (const [width, color] of [
      [4.4, INK],
      [2, IRON[2]],
    ] as const) {
      g.moveTo(...P(arm, 0, ah)).quadraticCurveTo(...P(arm + 0.4, 0, ah + 0.35), ...P(arm, 0, ah + 0.55)).quadraticCurveTo(...P(arm - 0.3, 0, ah + 0.45), ...P(arm - 0.15, 0, ah + 0.25));
      g.moveTo(...P(w, 0, ah - 1.5)).quadraticCurveTo(...P(0.4, 0, ah - 0.3), ...P(arm * 0.75, 0, ah));
      g.stroke({ width, color, cap: 'round', join: 'round' });
    }
    g.moveTo(...P(arm - 0.1, 0, ah)).lineTo(...P(arm - 0.1, 0, ah - 0.45)).stroke({ width: 1.4, color: IRON[0] });
    streetLantern(g, P, arm - 0.1, ah - 2.15, true);
  } else if (v === 2) {
    for (const side of [-1, 1]) {
      const [a, b, c]: [number, number][] = [P(0, 0, top - 0.6), P(side * 1.3, 0, top - 0.75), P(side * 1.75, 0, top + 0.2)];
      for (const [width, color] of [
        [5, INK],
        [2.6, IRON[2]],
      ] as const) {
        g.moveTo(...a).quadraticCurveTo(...b, ...c).stroke({ width, color, cap: 'round' });
      }
      streetLantern(g, P, side * 1.75, top + 0.2, true);
    }
    g.circle(...P(0, 0, top + 0.35), 2.6).fill(IRON[1]).stroke({ width: 1.4, color: INK });
  } else {
    if (v === 3) {
      rod(g, P(-0.75, 0, top - 1.2), P(0.75, 0, top - 1.2), 2.2);
    }
    streetLantern(g, P, 0, top, v === 0);
  }
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

/** A stone cross standing up, from a high angle (as the headstone's cross), its foot at (0, `foot`) and
 * `k` times life size: stout, so it still reads as a cross on a square seen from far up; the tops of its arms
 * and head lit behind the front face, a groove down the shaft and across the arms, the lower right in shade;
 * a ring round the crossing on a wheel cross. */
function standingCross(g: Graphics, foot: number, k: number, tones: readonly number[], wheel: boolean) {
  const P0 = upright(foot);
  const P = (x: number, d: number, h: number) => P0(x * k, d * k, h * k);
  const [sw, arm, a0, a1, head] = [0.6, 1.8, 5.9, 7.55, 9.4];
  const face = (d: number) => [
    ...P(-sw, d, 0), ...P(sw, d, 0), ...P(sw, d, a0), ...P(arm, d, a0), ...P(arm, d, a1), ...P(sw, d, a1), ...P(sw, d, head),
    ...P(-sw, d, head), ...P(-sw, d, a1), ...P(-arm, d, a1), ...P(-arm, d, a0), ...P(-sw, d, a0),
  ];
  const ink = { width: Math.max(1.2, 3 * k), color: INK, join: 'round' as const };
  g.poly(face(-0.7)).fill(tones[2]).stroke(ink);
  if (wheel) {
    // Dark between the ring and the arms, so the ring stands out.
    const [cx, cy] = P(0, 0, (a0 + a1) / 2);
    const [rx, ry] = [1.65 * FT * k, 1.65 * FT * k * 0.72];
    g.poly(oval(cx, cy, rx, ry, 0, 24)).fill(shade(tones[0], 0.45));
  }
  g.poly(face(0)).fill(tones[1]).stroke(ink);
  if (wheel) {
    // The ring, bold, standing proud of the arms: inked, lit along its upper edge.
    const [cx, cy] = P(0, 0, (a0 + a1) / 2);
    const [rx, ry] = [1.65 * FT * k, 1.65 * FT * k * 0.72];
    g.poly(oval(cx, cy, rx, ry, 0, 28)).stroke({ width: 0.3 * FT * k + 3.5, color: INK });
    g.poly(oval(cx, cy, rx, ry, 0, 28)).stroke({ width: 0.3 * FT * k, color: tones[1] });
    g.moveTo(cx - rx * 0.8, cy - ry * 0.6).quadraticCurveTo(cx, cy - ry * 1.2, cx + rx * 0.8, cy - ry * 0.6).stroke({ width: 0.1 * FT * k, color: tones[2], cap: 'round' });
  }
  if (k < 0.5) return;
  // The shade side: the east half of the shaft and the underside of the east arm.
  g.poly([...P(sw * 0.1, 0, 0.2), ...P(sw, 0, 0.2), ...P(sw, 0, a0 - 0.2), ...P(sw * 0.1, 0, a0 - 0.2)]).fill({ color: shade(tones[0], 0.8), alpha: 0.6 });
  g.poly([...P(sw, 0, a0 + 0.1), ...P(arm - 0.1, 0, a0 + 0.1), ...P(arm - 0.1, 0, (a0 + a1) / 2), ...P(sw, 0, (a0 + a1) / 2)]).fill({ color: shade(tones[0], 0.8), alpha: 0.5 });
  g.moveTo(...P(0, 0, 0.6)).lineTo(...P(0, 0, head - 0.4)).moveTo(...P(-arm + 0.35, 0, (a0 + a1) / 2)).lineTo(...P(arm - 0.35, 0, (a0 + a1) / 2));
  g.stroke({ width: 2.6, color: shade(tones[0], 0.85), cap: 'round' });
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

/** Market cross: stepped octagonal treads seen from above (lit on the NW, a joint here and there), a socket
 * stone and on it a stone cross standing up, from a high angle, its shadow falling short down the steps
 * (v0); a buttercross, its octagonal roof on pillars over the steps, a little cross on its peak (v1); an old
 * one, a step cracked, moss and flowers left at its foot (v2); a wheel cross on square steps (v3). */
export function marketCross(g: Graphics, r: number, rnd: () => number, v: number) {
  const st = v === 2 ? STONE[3] : STONE[0];
  const sides = v === 3 ? 4 : 8;
  const ph = v === 3 ? Math.PI / 4 + (rnd() - 0.5) * 0.1 : Math.PI / 8;
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
    standingCross(g, 0, 0.32, cross, false);
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
  // The socket stone, the cross's shadow falling short down the steps, then the cross standing on it (a
  // little over life size, standing a little forward on its socket, so it reads from far up).
  const s0 = steps[steps.length - 1] * 0.5;
  bevel(g, -s0, -s0, s0, s0, 3, st.map((c) => shade(c, 1.04)), 2);
  const [foot, k] = [0.6 * FT, 1.2];
  crossShadow(g, 0, foot, r * 0.8, v === 3);
  standingCross(g, foot, k, v === 2 ? cross.map((c) => shade(c, 0.9)) : cross, v === 3);
  if (v === 2) moss(g, -0.35 * FT * k, foot - 5 * FT * k * 0.5, 0.26 * FT, rnd);
}
