import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { BRASS, DARK_WOOD, INK, IRON, LINE, LINEN, type Light, OUT, WOOD, board, castShadow, frame, lit, litEdges, oval, patch, planks, quad, ring, shade } from './common';
import { GILT, MARBLE, candle } from './sacred';
import { PLUSH, STONE, WALL_GAP } from './home';
import { screenAxes } from './under';

// ---------------------------------------------------------------------------------------------------
// Great halls and ballrooms (`interior::FURNITURE`: throne dais to refreshment table).

export type Pt = [number, number];
type Rect = (u0: number, s0: number, u1: number, s1: number) => number[];
export type At = (u: number, s: number) => Pt;

/** A box inside an item's frame: `R` and `A` as the frame's `rect` and `at`, with `u` and `s` running 0 to 1 over
 * the box's `u0..u1` along and `s0..s1` across. */
function box(rect: Rect, at: At, u0: number, s0: number, u1: number, s1: number): { R: Rect; A: At } {
  const U = (u: number) => u0 + (u1 - u0) * u;
  const S = (s: number) => s0 + (s1 - s0) * s;
  return { R: (a, b, c, d) => rect(U(a), S(b), U(c), S(d)), A: (u, s) => at(U(u), S(s)) };
}

/** A polyline through `pts`, inked: a dark stroke, then the colour over it. */
export function inkLine(g: Graphics, pts: Pt[], w: number, color: number, curve = false) {
  for (const [ww, c] of [
    [w + 0.024, INK],
    [w, color],
  ] as const) {
    g.moveTo(...pts[0]);
    if (curve && pts.length === 3) g.quadraticCurveTo(...pts[1], ...pts[2]);
    else for (const p of pts.slice(1)) g.lineTo(...p);
    g.stroke({ width: ww, color: c, cap: 'round', join: 'round' });
  }
}

/** Oak for carved seats and the timber dais: shade, face, lit. */
export const OAK = [0x4a3220, 0x6e4c2e, 0x8e6a42];
/** Cushions: shade, cloth, lit. Red velvet, oxblood leather, blue, green, plum. */
export const VELVET = [0x6a1c18, 0xa83228, 0xd0604a];
export const LEATHER = [0x4a2418, 0x74382a, 0x985238];
export const CUSHIONS = [VELVET, [0x22345a, 0x3a5488, 0x5a78aa], [0x2a4224, 0x44663a, 0x62865a], [0x40243e, 0x663c62, 0x885a82]];

/** A chair of state from above, in its box (`s` 0 at its back): a frame with arms, a cushion filling the seat, a
 * tall back; its crest a pointed gilt one, a carved zigzag, or (a chair of office) three pinnacles. */
function stateChair(g: Graphics, B: { R: Rect; A: At }, light: Light, rnd: () => number, wood: readonly number[], cushion: readonly number[], crest: 'point' | 'carved' | 'pinnacles', trim: number) {
  const { R, A } = B;
  castShadow(g, quad(R(0.06, 0, 0.94, 0.94), rnd, 0), light, 0.2);
  board(g, R(0.1, 0.14, 0.9, 0.94), wood, light, rnd, true, 0.03, 0);
  // The cushion filling the seat: one button in its middle, a fold across it.
  const [qx0, qy0, qx1, qy1] = R(0.26, 0.3, 0.74, 0.86);
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.06).fill(cushion[0]).stroke({ width: OUT, color: INK });
  g.roundRect(qx0 + 0.03 + light[0] * 0.012, qy0 + 0.03 + light[1] * 0.012, qx1 - qx0 - 0.06, qy1 - qy0 - 0.06, 0.05).fill(cushion[1]);
  g.poly(quad(R(0.32, 0.36, 0.5, 0.54), rnd, 0.004)).fill({ color: cushion[2], alpha: 0.55 });
  g.moveTo(...A(0.3, 0.64)).quadraticCurveTo(...A(0.5, 0.6), ...A(0.7, 0.66)).stroke({ width: 0.014, color: cushion[0], alpha: 0.8 });
  g.circle(...A(0.5, 0.58), 0.022).fill(trim).stroke({ width: 0.01, color: INK });
  // Arms, their ends carved round.
  for (const u of [0.1, 0.76]) board(g, R(u, 0.14, u + 0.14, 0.94), wood, light, rnd, false, 0.02, 0);
  for (const u of [0.17, 0.83]) g.circle(...A(u, 0.9), 0.042).fill(trim).stroke({ width: LINE, color: INK });
  // The tall back and its crest.
  board(g, R(0.04, 0, 0.96, 0.22), wood, light, rnd, true, 0.03, 0);
  if (crest === 'point') {
    g.poly([...A(0.3, 0.03), ...A(0.5, 0.3), ...A(0.7, 0.03)]).fill(GILT[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly([...A(0.4, 0.05), ...A(0.5, 0.18), ...A(0.6, 0.05)]).fill(GILT[2]);
    for (const u of [0.07, 0.93]) g.circle(...A(u, 0.11), 0.05).fill(GILT[1]).stroke({ width: LINE, color: INK });
  } else if (crest === 'carved') {
    const zig: Pt[] = [];
    for (let k = 0; k <= 8; k++) zig.push(A(0.12 + k * 0.095, k % 2 ? 0.07 : 0.15));
    g.moveTo(...zig[0]);
    for (const p of zig.slice(1)) g.lineTo(...p);
    g.stroke({ width: 0.02, color: wood[0], join: 'miter' });
    for (const u of [0.07, 0.93]) g.circle(...A(u, 0.11), 0.045).fill(wood[2]).stroke({ width: LINE, color: INK });
  } else {
    // Pinnacles: little spires along the back, seen from above as diamonds with their ridges.
    for (const [u, r] of [
      [0.08, 0.055],
      [0.5, 0.075],
      [0.92, 0.055],
    ]) {
      const [cx, cy] = A(u, 0.11);
      g.poly([cx, cy - r, cx + r, cy, cx, cy + r, cx - r, cy]).fill(wood[2]).stroke({ width: LINE, color: INK, join: 'round' });
      g.moveTo(cx - r, cy).lineTo(cx + r, cy).moveTo(cx, cy - r).lineTo(cx, cy + r).stroke({ width: 0.01, color: wood[0] });
    }
    g.poly([...A(0.36, 0.2), ...A(0.5, 0.06), ...A(0.64, 0.2)]).stroke({ width: 0.016, color: trim, join: 'round' });
  }
}

/** Throne dais, its back to the wall (w×2): `v` 0-2 two stone steps under a carpet, the platform carpeted and a
 * cloth of estate hung on the wall behind gilded high-backed thrones with red cushions; 3-5 a plain raised timber
 * platform with a step up its middle and carved oak thrones. Seated `v % 3`: one throne, a pair, a throne and a
 * lesser chair. */
export function throneDais(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const rich = v % 6 < 3;
  const seated = v % 3;
  // Depths as drawn on two squares, kept for other depths.
  const S = (s: number) => (s * wid) / 2;
  const top = WALL_GAP - 0.04;
  // The seats: middle along, width, lesser.
  const fit = Math.max(0.5, Math.min(1, seated ? (len - 0.8) / 2.2 : len - 0.6));
  const seats: [number, number, boolean][] =
    seated === 0 ? [[len / 2, 0.96 * fit, false]] : seated === 1 ? [[len / 2 - 0.55 * fit, 0.96 * fit, false], [len / 2 + 0.55 * fit, 0.96 * fit, false]] : [[len / 2 - 0.3 * fit, 0.96 * fit, false], [len / 2 + 0.62 * fit, 0.7 * fit, true]];
  const sDeep = Math.min(0.9, S(1.3) - WALL_GAP - 0.06);
  if (rich) {
    // Two steps of pale dressed stone up to a marble platform, each lower step darker, the step above shading it.
    castShadow(g, quad(rect(0.04, top, len - 0.04, S(1.96)), rnd, 0), light, 0.14);
    const steps: [number, number, readonly number[]][] = [
      [0.04, S(1.96), [0x6a6256, 0x968c7c, 0xb0a692]],
      [0.24, S(1.66), [0x7c7468, 0xaca290, 0xc8bea8]],
      [0.44, S(1.36), MARBLE],
    ];
    steps.forEach(([u, s1, t], i) => {
      board(g, rect(u, top, len - u, s1), t, light, rnd, along, 0.04, 0);
      if (i) g.poly(quad(rect(u - 0.02, s1, len - u + 0.02, s1 + 0.06), rnd, 0)).fill({ color: 0x1a120a, alpha: 0.35 });
    });
    // The platform carpeted, the runner down the steps' middle (the steps' edges showing across it).
    g.poly(quad(rect(0.56, top + 0.02, len - 0.56, S(1.3)), rnd, 0.004)).fill(0x7a2220).stroke({ width: LINE, color: INK });
    g.poly(quad(rect(0.62, top + 0.06, len - 0.62, S(1.24)), rnd, 0.003)).stroke({ width: 0.016, color: GILT[1] });
    const rw = Math.min(0.32, len * 0.1);
    g.poly(quad(rect(len / 2 - rw, S(1.24), len / 2 + rw, S(1.98)), rnd, 0.004)).fill(0x8a2a24).stroke({ width: LINE, color: INK });
    for (const u of [len / 2 - rw + 0.05, len / 2 + rw - 0.05]) g.moveTo(...at(u, S(1.3))).lineTo(...at(u, S(1.94)));
    g.stroke({ width: 0.016, color: GILT[1] });
    for (const s of [S(1.36), S(1.66)]) g.moveTo(...at(len / 2 - rw, s)).lineTo(...at(len / 2 + rw, s));
    g.stroke({ width: 0.02, color: 0x4a1410 });
    // The cloth of estate on the wall behind the seats: blue powdered with gilt, a fringed hem, its rod.
    const c0 = Math.max(0.46, Math.min(...seats.map(([c, w]) => c - w / 2)) - 0.36);
    const c1 = Math.min(len - 0.46, Math.max(...seats.map(([c, w]) => c + w / 2)) + 0.36);
    const hem = Math.min(0.7, S(1.2));
    g.poly(quad(rect(c0, top, c1, hem), rnd, 0.004)).fill(0x1e3058).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(quad(rect(c0 + 0.05, top + 0.02, c1 - 0.05, hem - 0.06), rnd, 0.003)).fill(0x2e4a82);
    for (let u = c0 + 0.14; u < c1 - 0.08; u += 0.18) {
      for (let s = top + 0.1; s < hem - 0.08; s += 0.14) {
        const [x, y] = at(u + ((Math.round((s - top) / 0.14) % 2) * 0.09), s);
        g.poly([x, y - 0.025, x + 0.02, y, x, y + 0.025, x - 0.02, y]).fill(GILT[2]);
      }
    }
    for (let u = c0 + 0.04; u < c1 - 0.03; u += 0.045) g.moveTo(...at(u, hem - 0.01)).lineTo(...at(u, hem + 0.04));
    g.stroke({ width: 0.014, color: GILT[1] });
    inkLine(g, [at(c0 - 0.06, top), at(c1 + 0.06, top)], 0.04, GILT[1]);
  } else {
    // A raised timber platform of pale boards laid front to back, its front beam dark and pegged, a step of darker
    // oak up the middle of its front, a woven rug under the seats (green with a red border, fringed at its ends).
    const front = S(1.5);
    const deck = [0x7a5634, 0xa67c4c, 0xc49a66];
    castShadow(g, quad(rect(0.06, top, len - 0.06, front), rnd, 0), light, 0.16);
    castShadow(g, quad(rect(len * 0.28, front - 0.1, len * 0.72, S(1.94)), rnd, 0), light, 0.08);
    const step = board(g, rect(len * 0.28, front - 0.1, len * 0.72, S(1.94)), OAK.map((c) => shade(c, 0.85)), light, rnd, along, 0.03, 1);
    planks(g, step, along, 2, OAK[0], rnd);
    const face = board(g, rect(0.06, top, len - 0.06, front), deck, light, rnd, !along, 0.04, 1);
    planks(g, face, !along, Math.max(4, Math.round(len * 3)), 0x6a4a2c, rnd);
    g.poly(quad(rect(0.06, front - 0.1, len - 0.06, front), rnd, 0.003)).fill(OAK[0]).stroke({ width: LINE, color: INK });
    for (let u = 0.25; u < len - 0.1; u += Math.max(0.5, (len - 0.5) / Math.round((len - 0.5) / 0.7))) g.circle(...at(u, front - 0.05), 0.02).fill(OAK[2]).stroke({ width: 0.008, color: INK });
    const m0 = Math.max(0.2, Math.min(...seats.map(([c, w]) => c - w / 2)) - 0.2);
    const m1 = Math.min(len - 0.2, Math.max(...seats.map(([c, w]) => c + w / 2)) + 0.2);
    const r1 = front - 0.16;
    for (let t = top + 0.08; t < r1 - 0.03; t += 0.05) for (const [u, e] of [[m0, -0.05], [m1, 0.05]]) g.moveTo(...at(u, t)).lineTo(...at(u + e, t));
    g.stroke({ width: 0.014, color: 0xecdcb4 });
    g.poly(quad(rect(m0, top + 0.04, m1, r1), rnd, 0.006)).fill(0x8a2a24).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(quad(rect(m0 + 0.06, top + 0.1, m1 - 0.06, r1 - 0.06), rnd, 0.004)).fill(0x3e6038);
    g.poly(quad(rect(m0 + 0.09, top + 0.13, m1 - 0.09, r1 - 0.09), rnd, 0.003)).stroke({ width: 0.014, color: 0xd8b45a, alpha: 0.85 });
  }
  for (const [c, w, lesser] of seats) {
    const d = lesser ? sDeep * 0.76 : sDeep;
    const s0 = WALL_GAP + (lesser ? 0.1 : 0.02);
    const B = box(rect, at, c - w / 2, s0, c + w / 2, s0 + d);
    if (rich) stateChair(g, B, light, rnd, lesser ? DARK_WOOD : GILT, VELVET, lesser ? 'carved' : 'point', GILT[1]);
    else stateChair(g, B, light, rnd, OAK, LEATHER, 'carved', lesser ? OAK[2] : BRASS);
  }
}

/** Low dais, its back to the far wall (w×h): a single low step other pieces stand on (`PLATFORMS`), of a piece with
 * the throne dais's platform, no carpet: `v` 0 dressed stone as the throne dais's steps, laid in blocks; 1 the throne
 * dais's timber platform, pale boards laid front to back, its front beam dark and pegged. Its riser shows dark on
 * the sides away from the light and it casts a clear drop shadow, so it reads as raised off the floor. */
export function lowDais(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const stone = v % 2 === 0;
  const r = rect(0.04, 0.04, len - 0.04, wid - 0.04);
  castShadow(g, quad(r, rnd, 0), light, stone ? 0.24 : 0.3);
  if (stone) {
    board(g, r, [0x6a6256, 0xaca290, 0xc8bea8], light, rnd, along, 0.1, 0);
    // (Its blocks: joints across it about a square apart, a joint along its middle where it is deep.)
    const n = Math.max(1, Math.round(len));
    for (let k = 1; k < n; k++) {
      const u = (len * k) / n + (rnd() - 0.5) * 0.08;
      g.moveTo(...at(u, 0.16)).lineTo(...at(u, wid - 0.16));
    }
    if (wid >= 2) g.moveTo(...at(0.16, wid / 2)).lineTo(...at(len - 0.16, wid / 2));
    g.stroke({ width: LINE, color: 0x7c7468, alpha: 0.9 });
  } else {
    const face = board(g, r, [0x4e3420, 0xa67c4c, 0xc49a66], light, rnd, !along, 0.12, 1);
    planks(g, face, !along, Math.max(4, Math.round(len * 3)), 0x6a4a2c, rnd);
    const front = wid - 0.04;
    g.poly(quad(rect(0.04, front - 0.12, len - 0.04, front), rnd, 0.003)).fill(OAK[0]).stroke({ width: LINE, color: INK });
    for (let u = 0.25; u < len - 0.1; u += Math.max(0.5, (len - 0.5) / Math.round((len - 0.5) / 0.7))) g.circle(...at(u, front - 0.06), 0.02).fill(OAK[2]).stroke({ width: 0.008, color: INK });
  }
}

/** Chair of office, its back to the wall, on a small square step (stone or timber): a high-backed carved chair seen
 * from above as a chair is, its thick back with ball finials and a carved gilt cartouche, two arms running forward
 * to scrolled ends past the seat's front, the cushion in a guild's colour between them, the step showing in front;
 * the tall back casts its shadow over the seat. */
export function chairOfOffice(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  const stone = v % 2 === 0;
  // (A timber step in pale scrubbed boards, so the oak chair stands out on it.)
  const stepT = stone ? STONE : [0x8a6a44, 0xb08a5a, 0xc8a274];
  castShadow(g, quad(rect(0.06, WALL_GAP - 0.04, 0.94, 0.98), rnd, 0), light, 0.06);
  const step = board(g, rect(0.06, WALL_GAP - 0.04, 0.94, 0.98), stepT, light, rnd, true, 0.04, stone ? 0 : 1);
  if (!stone) planks(g, step, true, 3, 0x6a4e30, rnd);
  const wood = v === 3 ? DARK_WOOD : OAK;
  const trim = v === 3 ? GILT[1] : BRASS;
  const cushion = CUSHIONS[v % 4];
  const b0 = WALL_GAP - 0.02;
  // The chair's own shadow on the step.
  castShadow(g, quad(rect(0.1, b0, 0.9, 0.84), rnd, 0), light, 0.22);
  // The seat between the arms, its front rail bowed out a little, the cushion on it.
  const seat = [...at(0.25, b0 + 0.12), ...at(0.75, b0 + 0.12), ...at(0.75, 0.72), ...at(0.5, 0.77), ...at(0.25, 0.72)];
  g.poly(seat).fill(wood[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const [qx0, qy0, qx1, qy1] = rect(0.29, b0 + 0.16, 0.71, 0.7);
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.07).fill(cushion[0]).stroke({ width: LINE, color: INK });
  g.roundRect(qx0 + 0.03 + light[0] * 0.015, qy0 + 0.03 + light[1] * 0.015, qx1 - qx0 - 0.06, qy1 - qy0 - 0.06, 0.06).fill(cushion[1]);
  g.roundRect(qx0 + 0.06 + light[0] * 0.03, qy0 + 0.06 + light[1] * 0.03, (qx1 - qx0) * 0.42, (qy1 - qy0) * 0.3, 0.04).fill({ color: cushion[2], alpha: 0.6 });
  // Arms from the back to scrolled ends past the seat's front.
  for (const u of [0.12, 0.75]) board(g, rect(u, b0 + 0.08, u + 0.13, 0.8), wood, light, rnd, false, 0.025, 0);
  for (const u of [0.185, 0.815]) {
    const [x, y] = at(u, 0.8);
    g.circle(x, y, 0.075).fill(wood[1]).stroke({ width: LINE, color: INK });
    g.circle(x + light[0] * 0.015, y + light[1] * 0.015, 0.04).fill(wood[2]);
    g.circle(x, y, 0.022).fill(trim).stroke({ width: 0.008, color: INK });
  }
  // The tall back: its shadow over the seat, the thick top rail, ball finials, a carved gilt cartouche with a
  // scroll either side.
  castShadow(g, quad(rect(0.1, b0, 0.9, b0 + 0.15), rnd, 0), light, 0.32);
  board(g, rect(0.08, b0 - 0.02, 0.92, b0 + 0.15), wood, light, rnd, true, 0.035, 1);
  for (const sd of [-1, 1]) {
    const [x, y] = at(0.5 + sd * 0.22, b0 + 0.065);
    g.moveTo(...at(0.5 + sd * 0.12, b0 + 0.065)).quadraticCurveTo(x, y - 0.06, ...at(0.5 + sd * 0.3, b0 + 0.065));
    g.stroke({ width: 0.016, color: wood[0], cap: 'round' });
  }
  const [mx, my] = at(0.5, b0 + 0.065);
  g.ellipse(mx, my, 0.1, 0.075).fill(GILT[1]).stroke({ width: LINE, color: INK });
  g.ellipse(mx + light[0] * 0.015, my + light[1] * 0.015, 0.055, 0.04).fill(GILT[2]);
  for (const u of [0.11, 0.89]) {
    const [x, y] = at(u, b0 + 0.065);
    g.circle(x, y, 0.07).fill(trim).stroke({ width: LINE, color: INK });
    g.circle(x + light[0] * 0.02, y + light[1] * 0.02, 0.03).fill({ color: 0xfff1cf, alpha: 0.7 });
  }
}

/** High table on the dais, the diners' side to the wall: a long board under a white linen cloth hanging in folds
 * over the front, a coloured runner down it, plates and cups set along the diners' side, a gilt salt in the
 * middle, candlesticks. */
export function highTable(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const runner = [0x8a2a24, 0x2e4a7a, 0x3e6a34, 0xb08a3a][v % 4];
  castShadow(g, quad(rect(0.04, 0.12, len - 0.04, 0.94), rnd, 0), light, 0.16);
  // The cloth: past the board all round, hanging lower at the front, folds down its front.
  g.poly(quad(rect(0.04, 0.12, len - 0.04, 0.94), rnd, 0.006)).fill(LINEN[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad(rect(0.1, 0.16, len - 0.1, 0.8), rnd, 0.004)).fill(LINEN[1]);
  litEdges(g, rect(0.1, 0.16, len - 0.1, 0.8), light, 0.03);
  g.fill({ color: LINEN[2], alpha: 0.8 });
  for (let u = 0.3; u < len - 0.15; u += 0.32 + rnd() * 0.08) g.moveTo(...at(u, 0.82)).lineTo(...at(u + 0.02, 0.92));
  g.stroke({ width: 0.016, color: shade(LINEN[0], 0.82) });
  g.moveTo(...at(0.1, 0.8)).lineTo(...at(len - 0.1, 0.8)).stroke({ width: 0.012, color: shade(LINEN[0], 0.85) });
  // The runner down the middle, hanging over the ends.
  g.poly(quad(rect(0.02, 0.42, len - 0.02, 0.6), rnd, 0.003)).fill(runner).stroke({ width: LINE, color: INK });
  g.moveTo(...at(0.06, 0.46)).lineTo(...at(len - 0.06, 0.46)).moveTo(...at(0.06, 0.56)).lineTo(...at(len - 0.06, 0.56));
  g.stroke({ width: 0.01, color: GILT[2], alpha: 0.8 });
  // Places along the diners' side: a plate and a gilt cup each, a loaf here and there; a roast fowl on a platter
  // in the middle, a candlestick either side of it.
  const places = Math.max(1, Math.round(len * 1.1));
  for (let i = 0; i < places; i++) {
    const u = 0.3 + ((len - 0.6) * (i + 0.5)) / places;
    if (Math.abs(u - len / 2) < 0.3) continue;
    const [px, py] = at(u, 0.32);
    g.circle(px, py, 0.1).fill(v % 2 ? 0xc0c4c8 : 0xa4a49c).stroke({ width: 0.014, color: INK });
    g.circle(px, py, 0.065).stroke({ width: 0.01, color: 0x6a6e72 });
    g.circle(...at(u + 0.17, 0.24), 0.04).fill(GILT[1]).stroke({ width: 0.012, color: INK });
    g.circle(...at(u + 0.17, 0.24), 0.022).fill(0x6a1e1a);
    if (i % 2 === 0) {
      const [bx, by] = at(u - 0.04, 0.62);
      g.poly(oval(bx, by, 0.07, 0.05, along ? 0.3 : 1.9, 12)).fill(0xc08a48).stroke({ width: 0.012, color: INK });
      g.moveTo(bx - 0.03, by - 0.01).lineTo(bx + 0.02, by + 0.02).stroke({ width: 0.01, color: 0x8a5a2a });
    }
  }
  const [sx, sy] = at(len / 2, 0.48);
  const fa = along ? 0 : Math.PI / 2;
  g.poly(oval(sx, sy, 0.24, 0.17, fa, 18)).fill(0xc0c4c8).stroke({ width: 0.016, color: INK });
  g.poly(oval(sx, sy, 0.19, 0.12, fa, 16)).stroke({ width: 0.01, color: 0x7a7e82 });
  // The fowl: its breast, two drumsticks with white knuckles toward one end.
  const [ca, sa] = [Math.cos(fa), Math.sin(fa)];
  for (const sd of [-1, 1]) {
    const [lx, ly] = [sx + ca * 0.1 - sa * sd * 0.05, sy + sa * 0.1 + ca * sd * 0.05];
    g.poly(oval(lx, ly, 0.06, 0.03, fa + sd * 0.4, 10)).fill(0x9a5424).stroke({ width: 0.01, color: INK });
    g.circle(lx + ca * 0.06, ly + sa * 0.06 + 0, 0.016).fill(0xf6efe0).stroke({ width: 0.006, color: INK });
  }
  g.poly(oval(sx - ca * 0.02, sy - sa * 0.02, 0.12, 0.085, fa, 14)).fill(0xb06a2e).stroke({ width: 0.014, color: INK });
  g.poly(oval(sx - ca * 0.04 + light[0] * 0.02, sy - sa * 0.04 + light[1] * 0.02, 0.06, 0.04, fa, 10)).fill({ color: 0xe0a060, alpha: 0.7 });
  for (const u of [len / 2 - 0.5, len / 2 + 0.5]) if (u > 0.3 && u < len - 0.3) candle(g, ...at(u, 0.56));
}

/** House colours (`Item::variant` of a wall banner and a standing banner, the same index on both, so a castle's
 * banners can all fly one house's): the field (shade, cloth, lit), its charge's colour and the charge. Even
 * houses' banners end in a point, odd ones in a swallowtail. */
export const HOUSES: { name: string; field: readonly number[]; charge: number; mark: 'tower' | 'cross' | 'bend' | 'star' | 'crescent' | 'quarterly' | 'chevron' | 'saltire' }[] = [
  { name: 'Red, a gold tower', field: [0x6e2219, 0x9e3428, 0xbc4c3a], charge: 0xe8bc4e, mark: 'tower' },
  { name: 'Blue, a white cross', field: [0x23365e, 0x34528a, 0x4a6ca6], charge: 0xf0e8d4, mark: 'cross' },
  { name: 'Green, a gold bend', field: [0x2e4a22, 0x46682e, 0x5e8240], charge: 0xe8bc4e, mark: 'bend' },
  { name: 'Black, a gold star', field: [0x221e26, 0x37313c, 0x4e4654], charge: 0xe8bc4e, mark: 'star' },
  { name: 'Purple, a white crescent', field: [0x48284a, 0x6a3e68, 0x865682], charge: 0xf0e8d4, mark: 'crescent' },
  { name: 'White and red quarterly', field: [0xb0a68e, 0xe4d8bc, 0xf6efe0], charge: 0xa83226, mark: 'quarterly' },
  { name: 'Gold, a black chevron', field: [0x9a7428, 0xd0a442, 0xe8c466], charge: 0x2e2830, mark: 'chevron' },
  { name: 'Teal, a white saltire', field: [0x1e4a48, 0x2e6e6a, 0x46908a], charge: 0xf0e8d4, mark: 'saltire' },
];

/** The house a building's banners fly when none is set (`Item::variant` absent): one per building, from its id
 * (FNV-1a), so every banner put down in one castle hall matches. */
export function houseOf(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 0x01000193) >>> 0;
  return h % HOUSES.length;
}

/** A heraldic banner hanging from its rod: `P(a, b)` is the point `a` across from its middle and `b` down from
 * the rod, `w` wide and `len` long, in house `h`'s colours with its charge, two folds down it (the one toward
 * the light paler), a gilt edge on its point or swallowtail. */
function heraldicBanner(g: Graphics, P: (a: number, b: number) => Pt, w: number, len: number, h: number, light: Light, shadow: number) {
  const house = HOUSES[h % HOUSES.length];
  const [f0, f1, f2] = house.field;
  const bt = len * 0.74;
  const hw = w / 2;
  const tail: Pt[] = h % 2 ? [P(hw, len), P(0, bt + (len - bt) * 0.3), P(-hw, len)] : [P(0, len)];
  const pts = [P(-hw, 0), P(hw, 0), P(hw, bt), ...tail, P(-hw, bt)].flat();
  if (shadow > 0) castShadow(g, pts, light, shadow);
  g.poly(pts).fill(f1).stroke({ width: OUT, color: INK, join: 'round' });
  // Which side faces the light.
  const [ax, ay] = P(1, 0);
  const [ox, oy] = P(0, 0);
  const sideLit = (ax - ox) * light[0] + (ay - oy) * light[1] > 0 ? 1 : -1;
  const fold = (a: number, color: number, alpha: number) =>
    g.poly([...P(a - w * 0.1, 0.04), ...P(a + w * 0.1, 0.04), ...P(a + w * 0.08, bt), ...P(a - w * 0.08, bt)]).fill({ color, alpha });
  fold(sideLit * w * 0.3, f2, 0.6);
  fold(-sideLit * w * 0.26, f0, 0.45);
  // The charge.
  const c = house.charge;
  const bc = bt * 0.52;
  const r = Math.min(w, bt) * 0.3;
  const fillPoly = (q: Pt[]) => g.poly(q.flat()).fill(c).stroke({ width: 0.012, color: INK, join: 'round' });
  const band = (a0: number, b0: number, a1: number, b1: number, t: number) => {
    const [dx, dy] = [a1 - a0, b1 - b0];
    const m = Math.hypot(dx, dy) || 1;
    const [nx, ny] = [(-dy / m) * t, (dx / m) * t];
    fillPoly([P(a0 + nx, b0 + ny), P(a1 + nx, b1 + ny), P(a1 - nx, b1 - ny), P(a0 - nx, b0 - ny)]);
  };
  switch (house.mark) {
    case 'tower': {
      const r = Math.min(w, bt) * 0.38;
      fillPoly([P(-0.42 * r, bc - 0.55 * r), P(-0.42 * r, bc - 0.9 * r), P(-0.24 * r, bc - 0.9 * r), P(-0.24 * r, bc - 0.72 * r), P(-0.09 * r, bc - 0.72 * r), P(-0.09 * r, bc - 0.9 * r), P(0.09 * r, bc - 0.9 * r), P(0.09 * r, bc - 0.72 * r), P(0.24 * r, bc - 0.72 * r), P(0.24 * r, bc - 0.9 * r), P(0.42 * r, bc - 0.9 * r), P(0.42 * r, bc - 0.55 * r), P(0.5 * r, bc + 0.85 * r), P(-0.5 * r, bc + 0.85 * r)]);
      g.poly([...P(-0.16 * r, bc + 0.85 * r), ...P(-0.16 * r, bc + 0.4 * r), ...P(0, bc + 0.22 * r), ...P(0.16 * r, bc + 0.4 * r), ...P(0.16 * r, bc + 0.85 * r)]).fill(f0);
      break;
    }
    case 'cross':
      fillPoly([P(-w * 0.09, 0.06), P(w * 0.09, 0.06), P(w * 0.09, bt), P(-w * 0.09, bt)]);
      fillPoly([P(-hw + 0.02, bt * 0.34), P(hw - 0.02, bt * 0.34), P(hw - 0.02, bt * 0.34 + w * 0.18), P(-hw + 0.02, bt * 0.34 + w * 0.18)]);
      break;
    case 'saltire':
      band(-hw + 0.03, 0.08, hw - 0.03, bt - 0.02, w * 0.08);
      band(hw - 0.03, 0.08, -hw + 0.03, bt - 0.02, w * 0.08);
      break;
    case 'bend':
      band(-hw + 0.02, 0.1, hw - 0.02, bt - 0.04, w * 0.1);
      break;
    case 'chevron': {
      const t = w * 0.2;
      fillPoly([P(-hw + 0.02, bt * 0.72), P(0, bt * 0.3), P(hw - 0.02, bt * 0.72), P(hw - 0.02, bt * 0.72 + t), P(0, bt * 0.3 + t), P(-hw + 0.02, bt * 0.72 + t)]);
      break;
    }
    case 'quarterly':
      fillPoly([P(-hw + 0.02, 0.05), P(0, 0.05), P(0, bt / 2), P(-hw + 0.02, bt / 2)]);
      fillPoly([P(0, bt / 2), P(hw - 0.02, bt / 2), P(hw - 0.02, bt), P(0, bt)]);
      break;
    case 'star': {
      const q: Pt[] = [];
      for (let k = 0; k < 10; k++) {
        const a = -Math.PI / 2 + (k * Math.PI) / 5;
        const rr = k % 2 ? r * 0.42 : r;
        q.push(P(Math.cos(a) * rr, bc + Math.sin(a) * rr));
      }
      fillPoly(q);
      break;
    }
    case 'crescent': {
      const q: Pt[] = [];
      for (let k = 0; k <= 14; k++) {
        const a = (-25 + (230 * k) / 14) * (Math.PI / 180);
        q.push(P(Math.cos(a) * r, bc + Math.sin(a) * r));
      }
      for (let k = 14; k >= 0; k--) {
        const a = (-10 + (200 * k) / 14) * (Math.PI / 180);
        q.push(P(Math.cos(a) * r * 0.74, bc - r * 0.28 + Math.sin(a) * r * 0.74));
      }
      fillPoly(q);
      break;
    }
  }
  // The sleeve over the rod, the gilt edge round the end.
  g.poly([...P(-hw, 0), ...P(hw, 0), ...P(hw, 0.05), ...P(-hw, 0.05)]).fill(f0);
  g.moveTo(...P(hw, bt));
  for (const p of tail) g.lineTo(...p);
  g.lineTo(...P(-hw, bt)).stroke({ width: 0.016, color: GILT[1], join: 'round' });
}

/** Wall banner: a long heraldic banner in its house's colours hung from a rod along the wall, drawn falling out
 * into the room as the tattered banner is. */
export function wallBanner(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number]) {
  const { at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  heraldicBanner(g, (a, b) => at(0.5 + a, top + b), 0.52, 0.94 - top, v, light, 0.05);
  inkLine(g, [at(0.16, top), at(0.84, top)], 0.035, DARK_WOOD[1]);
  for (const u of [0.16, 0.84]) g.circle(...at(u, top), 0.035).fill(GILT[1]).stroke({ width: 0.012, color: INK });
}

/** Standing banner, from a high angle like the candelabrum (screen up kept up): a weighted round foot, the pole,
 * a crossbar near its top with the banner hanging from it in its house's colours, a gilt spear-point finial. */
export function standingBanner(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const { pt, right } = screenAxes(light);
  const P = (a: number, b: number) => pt(cx, cy, a, b);
  const rot = Math.atan2(right[1], right[0]);
  const foot = -0.36;
  const bar = 0.42;
  castShadow(g, ring(...P(0, foot), 0.07, rnd, 0, 8), light, 1.0);
  // The foot: a stepped round base, its sides showing below.
  for (const [r, k, dy] of [
    [0.19, 0.78, -0.025],
    [0.13, 1, 0.01],
  ]) {
    g.poly(oval(...P(0, foot + dy - 0.03), r, r * 0.6, rot, 16)).fill(shade(DARK_WOOD[1], k * 0.85)).stroke({ width: LINE, color: INK });
    g.poly(oval(...P(0, foot + dy), r, r * 0.6, rot, 16)).fill(shade(DARK_WOOD[1], k)).stroke({ width: LINE, color: INK });
  }
  inkLine(g, [P(0, foot), P(0, bar + 0.1)], 0.035, DARK_WOOD[1]);
  heraldicBanner(g, (a, b) => P(a, bar - b), 0.44, 0.5, v, light, 0);
  inkLine(g, [P(-0.27, bar), P(0.27, bar)], 0.026, DARK_WOOD[1]);
  for (const a of [-0.27, 0.27]) g.circle(...P(a, bar), 0.026).fill(GILT[1]).stroke({ width: 0.01, color: INK });
  const tip = [P(-0.035, bar + 0.1), P(0, bar + 0.2), P(0.035, bar + 0.1)];
  g.poly(tip.flat()).fill(GILT[1]).stroke({ width: 0.012, color: INK, join: 'round' });
}

/** Tapestry hung along the wall and drawn falling out into the room: a wide woven scene inside a border, the rod
 * along the wall with gilt finials, a fringed hem. The scene in the batch's manner (flat colours, bold ink, a few
 * big figures): a stag hunted by a hound in green woods, a unicorn on a red field of flowers, a ship on the sea by a
 * castle, a rider on the road to a castle on a hill. */
export function tapestry(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  const [u0, u1, s1] = [0.14, len - 0.14, 0.9];
  const border = [0x6a1e1a, 0x22365e, 0x7a2a22, 0x2e4a26][v % 4];
  castShadow(g, quad(rect(u0, top, u1, s1), rnd, 0), light, 0.05);
  g.poly(quad(rect(u0, top, u1, s1), rnd, 0.004)).fill(border).stroke({ width: OUT, color: INK, join: 'round' });
  const b = 0.055;
  const [x0, y0] = [u0 + b, top + b];
  const W = u1 - u0 - 2 * b;
  const H = s1 - top - 2 * b;
  /** A point x across the scene from its left and y down from its top. */
  const P = (x: number, y: number) => at(x0 + x, y0 + y);
  const INKW = 0.016;
  const fill = (q: [number, number][], color: number, ink = true) => {
    g.poly(q.flatMap(([x, y]) => P(x, y))).fill(color);
    if (ink) g.stroke({ width: INKW, color: INK, join: 'round' });
  };
  const blob = (x: number, y: number, rx: number, ry: number, color: number, n = 14) => {
    const q: [number, number][] = [];
    for (let k = 0; k < n; k++) q.push([x + Math.cos((k / n) * Math.PI * 2) * rx, y + Math.sin((k / n) * Math.PI * 2) * ry]);
    fill(q, color);
  };
  /** A limb or a pole: an inked stroke. */
  const limb = (q: [number, number][], w: number, color: number) => {
    for (const [ww, c] of [
      [w + INKW * 1.6, INK],
      [w, color],
    ] as const) {
      g.moveTo(...P(...q[0]));
      for (const p of q.slice(1)) g.lineTo(...P(...p));
      g.stroke({ width: ww, color: c, cap: 'round', join: 'round' });
    }
  };
  /** A four-legged beast `k` tall standing (or running) on ground line `gy`, facing `d` (1 right, -1 left). */
  const beast = (x: number, gy: number, k: number, d: number, body: number, opts: { antlers?: boolean; horn?: boolean; run?: boolean; hound?: boolean }) => {
    const legW = 0.07 * k;
    const by = gy - 0.5 * k;
    const legs: [number, number, number][] = opts.run
      ? [
          [0.2, 0.42, 0.1],
          [0.14, 0.3, 0.02],
          [-0.2, -0.42, 0.1],
          [-0.26, -0.3, 0.02],
        ]
      : [
          [0.22, 0.24, 0],
          [0.13, 0.12, 0],
          [-0.18, -0.2, 0],
          [-0.26, -0.28, 0],
        ];
    // The far legs a shade darker, drawn first; the tail.
    for (const [i, [a, e, lift]] of legs.entries()) if (i % 2) limb([[x + d * a * k, by], [x + d * e * k, gy - lift * k]], legW, shade(body, 0.8));
    limb([[x - d * 0.3 * k, by - 0.06 * k], [x - d * (opts.hound ? 0.5 : 0.4) * k, by - (opts.hound ? 0.2 : 0.02) * k]], 0.05 * k, body);
    for (const [i, [a, e, lift]] of legs.entries()) if (!(i % 2)) limb([[x + d * a * k, by], [x + d * e * k, gy - lift * k]], legW, body);
    blob(x, by, 0.36 * k, (opts.hound ? 0.12 : 0.17) * k, body, 18);
    // Neck and head: slanting up and forward (a hound's low and long), the head long, pointing down a little; an ear,
    // an eye, a mane on a horse or a unicorn.
    const [nx, ny] = opts.hound ? [x + d * 0.42 * k, by - 0.12 * k] : [x + d * 0.44 * k, by - 0.34 * k];
    const neck: [number, number][] = [[x + d * 0.14 * k, by - 0.12 * k], [nx - d * 0.06 * k, ny - 0.06 * k], [nx + d * 0.06 * k, ny + 0.06 * k], [x + d * 0.34 * k, by + 0.02 * k]];
    fill(neck, body);
    if (!opts.hound && !opts.antlers) limb([[x + d * 0.16 * k, by - 0.14 * k], [nx - d * 0.05 * k, ny - 0.07 * k]], 0.035 * k, shade(body, 0.78));
    const hq: [number, number][] = [[nx - d * 0.06 * k, ny - 0.07 * k], [nx + d * 0.16 * k, ny - 0.02 * k], [nx + d * 0.24 * k, ny + 0.08 * k], [nx + d * 0.16 * k, ny + 0.11 * k], [nx - d * 0.04 * k, ny + 0.07 * k]];
    fill(hq, body);
    fill([[nx - d * 0.02 * k, ny - 0.05 * k], [nx - d * 0.05 * k, ny - 0.17 * k], [nx + d * 0.04 * k, ny - 0.06 * k]], shade(body, 0.85));
    g.circle(...P(nx + d * 0.06 * k, ny + 0.01 * k), 0.012).fill(INK);
    if (opts.antlers) {
      for (const sd of [-0.03, 0.05]) {
        const bx = nx + d * sd * k;
        limb([[bx, ny - 0.06 * k], [bx - d * 0.1 * k, ny - 0.3 * k], [bx + d * 0.02 * k, ny - 0.46 * k]], 0.022, 0xf0dca8);
        limb([[bx - d * 0.07 * k, ny - 0.22 * k], [bx + d * 0.1 * k, ny - 0.3 * k]], 0.02, 0xf0dca8);
      }
    }
    if (opts.horn) limb([[nx + d * 0.06 * k, ny - 0.06 * k], [nx + d * 0.26 * k, ny - 0.3 * k]], 0.024, GILT[2]);
  };
  const tree = (x: number, gy: number, k: number, leaf: number, fruit = 0) => {
    limb([[x, gy], [x, gy - 0.55 * k]], 0.08 * k, 0x6a4428);
    blob(x - 0.13 * k, gy - 0.62 * k, 0.2 * k, 0.17 * k, leaf);
    blob(x + 0.14 * k, gy - 0.66 * k, 0.2 * k, 0.17 * k, leaf);
    blob(x, gy - 0.86 * k, 0.22 * k, 0.17 * k, shade(leaf, 1.15));
    if (fruit)
      for (const [fx, fy] of [
        [-0.16, 0.62],
        [0.12, 0.72],
        [0.02, 0.9],
        [0.2, 0.6],
      ])
        g.circle(...P(x + fx * k, gy - fy * k), 0.026).fill(fruit).stroke({ width: 0.01, color: INK });
  };
  /** A big flower, few of them: a white or gold disc round a dot. */
  const flower = (x: number, y: number, c: number) => {
    g.circle(...P(x, y), 0.03).fill(c).stroke({ width: 0.01, color: INK });
    g.circle(...P(x, y), 0.011).fill(c === 0xf6efe0 ? 0xe8b83a : 0xb5523a);
  };
  const castle = (x: number, gy: number, k: number) => {
    const stone = 0xd8ccb0;
    fill([[x - 0.3 * k, gy], [x - 0.3 * k, gy - 0.4 * k], [x + 0.3 * k, gy - 0.4 * k], [x + 0.3 * k, gy]], stone);
    for (const tx of [-0.32, 0.32]) {
      fill([[x + (tx - 0.11) * k, gy], [x + (tx - 0.11) * k, gy - 0.66 * k], [x + (tx + 0.11) * k, gy - 0.66 * k], [x + (tx + 0.11) * k, gy]], stone);
      fill([[x + (tx - 0.15) * k, gy - 0.66 * k], [x + tx * k, gy - 0.94 * k], [x + (tx + 0.15) * k, gy - 0.66 * k]], 0xa8483a);
    }
    fill([[x - 0.09 * k, gy], [x - 0.09 * k, gy - 0.18 * k], [x, gy - 0.27 * k], [x + 0.09 * k, gy - 0.18 * k], [x + 0.09 * k, gy]], 0x3a2a22);
  };
  const scene = v % 4;
  const all: [number, number][] = [[0, 0], [W, 0], [W, H], [0, H]];
  if (scene === 0) {
    // The hunt: a stag leaping away from a hound under a tree, on two greens.
    fill(all, 0x4a7a36, false);
    fill([[0, H * 0.7], [W * 0.35, H * 0.62], [W * 0.7, H * 0.68], [W, H * 0.6], [W, H], [0, H]], 0x86a84e);
    tree(W * 0.1, H * 0.72, H * 0.8, 0x2f5a2c);
    if (W > 2) tree(W * 0.9, H * 0.66, H * 0.72, 0x2f5a2c);
    for (const [fx, fy, c] of [
      [0.3, 0.86, 0xf6efe0],
      [0.48, 0.92, 0xf2c94c],
      [0.82, 0.88, 0xf6efe0],
    ])
      if (fx * W < W - 0.1) flower(fx * W, fy * H, c);
    beast(W * 0.62, H * 0.95, H * 0.66, 1, 0xb87a40, { antlers: true, run: true });
    beast(W * 0.32, H * 0.96, H * 0.55, 1, 0xece0c4, { run: true, hound: true });
  } else if (scene === 1) {
    // The unicorn on a red field, an orange tree beside it, a few big flowers.
    fill(all, 0x9e3428, false);
    for (const [fx, fy, c] of [
      [0.08, 0.3, 0xf6efe0],
      [0.22, 0.75, 0xf2c94c],
      [0.4, 0.2, 0xf2c94c],
      [0.8, 0.82, 0xf6efe0],
      [0.92, 0.3, 0xf2c94c],
      [0.55, 0.88, 0xf6efe0],
    ])
      flower(fx * W, fy * H, c);
    tree(W * 0.72, H * 0.96, H * 0.95, 0x3e6a2e, 0xf28a1e);
    beast(W * 0.4, H * 0.96, H * 0.8, 1, 0xf6efe0, { horn: true });
  } else if (scene === 2) {
    // A ship on the sea before a castle on its headland.
    fill(all, 0xe6dcc0, false);
    fill([[0, H * 0.5], [W, H * 0.5], [W, H], [0, H]], 0x34628a);
    for (let row = 0; row < 2; row++) {
      for (let x = 0.1 + row * 0.14; x < W - 0.1; x += 0.3)
        limb([[x, H * (0.68 + row * 0.18)], [x + 0.07, H * (0.62 + row * 0.18)], [x + 0.14, H * (0.68 + row * 0.18)]], 0.016, 0xf6efe0);
    }
    fill([[W * 0.7, H * 0.5], [W * 0.78, H * 0.26], [W, H * 0.22], [W, H * 0.62], [W * 0.84, H * 0.6]], 0x5e8a40);
    castle(W * 0.88, H * 0.42, H * 0.4);
    const sx = W * 0.38;
    const k = H * 0.82;
    const wl = H * 0.62;
    fill([[sx - 0.42 * k, wl], [sx + 0.46 * k, wl], [sx + 0.3 * k, wl + 0.22 * k], [sx - 0.3 * k, wl + 0.22 * k]], 0x7a4a28);
    limb([[sx, wl], [sx, wl - 0.62 * k]], 0.022, 0x5a3820);
    fill([[sx - 0.28 * k, wl - 0.56 * k], [sx + 0.28 * k, wl - 0.56 * k], [sx + 0.32 * k, wl - 0.1 * k], [sx - 0.32 * k, wl - 0.1 * k]], 0xf6efe0);
    fill([[sx - 0.06 * k, wl - 0.56 * k], [sx + 0.06 * k, wl - 0.56 * k], [sx + 0.07 * k, wl - 0.1 * k], [sx - 0.07 * k, wl - 0.1 * k]], 0xb5523a);
    fill([[sx, wl - 0.62 * k], [sx + 0.16 * k, wl - 0.68 * k], [sx, wl - 0.74 * k]], 0xb5523a);
  } else {
    // A rider on the road to a castle on a green hill.
    fill(all, 0xd4dccc, false);
    fill([[0, H * 0.72], [W * 0.18, H * 0.42], [W * 0.42, H * 0.36], [W * 0.62, H * 0.56], [W, H * 0.62], [W, H], [0, H]], 0x6a9a48);
    castle(W * 0.28, H * 0.46, H * 0.6);
    limb([[W * 0.32, H * 0.5], [W * 0.46, H * 0.74], [W * 0.62, H * 0.98]], 0.04, 0xe0c890);
    const k = H * 0.72;
    beast(W * 0.72, H * 0.97, k, -1, 0x6a4a32, {});
    // The rider: a blue cloak over the horse's back, a pale face, a lance with a pennon.
    const [rx, ry] = [W * 0.72, H * 0.97 - k * 0.62];
    limb([[rx + 0.05, ry + 0.02], [rx - 0.22, ry - 0.3]], 0.016, 0xd8c8a0);
    fill([[rx - 0.22, ry - 0.3], [rx - 0.36, ry - 0.27], [rx - 0.24, ry - 0.24]], 0xb5523a);
    fill([[rx - 0.08, ry + 0.06], [rx - 0.06, ry - 0.14], [rx + 0.06, ry - 0.14], [rx + 0.1, ry + 0.06]], 0x34528a);
    blob(rx, ry - 0.19, 0.05, 0.05, 0xe8c8a0, 10);
  }
  // The border's gilt line, the fringed hem, the rod.
  g.poly(quad(rect(u0 + b * 0.45, top + b * 0.45, u1 - b * 0.45, s1 - b * 0.45), rnd, 0.002)).stroke({ width: 0.014, color: GILT[2], alpha: 0.9 });
  g.poly(quad(rect(u0 + b, top + b, u1 - b, s1 - b), rnd, 0.002)).stroke({ width: INKW, color: INK });
  for (let u = u0 + 0.03; u < u1 - 0.02; u += 0.05) g.moveTo(...at(u, s1)).lineTo(...at(u, s1 + 0.05));
  g.stroke({ width: 0.016, color: GILT[1] });
  inkLine(g, [at(u0 - 0.06, top), at(u1 + 0.06, top)], 0.04, DARK_WOOD[1]);
  for (const u of [u0 - 0.06, u1 + 0.06]) g.circle(...at(u, top), 0.04).fill(GILT[1]).stroke({ width: 0.012, color: INK });
}

/** Carpet runner along its length (one or two squares wide): a field with a border along both sides, a band
 * across each end with a fringe past it, a chain of small lozenges down the middle. */
export function carpetRunner(g: Graphics, f: InteriorItem, v: number, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  const wid = along ? f.h : f.w;
  const [ground, edge, motif] = [
    [0x8a2a24, 0x2e3e5a, 0xd8b45a],
    [0x2e4a7a, 0x8a2e26, 0xe0cca0],
    [0x3e6038, 0x7a2a22, 0xe0c27a],
    [0x5e3458, 0xc8a050, 0xecdcb4],
  ][v % 4];
  const m = 0.12;
  for (const u of [m, len - m]) {
    for (let s = m + 0.06; s < wid - m - 0.04; s += 0.055) g.moveTo(...at(u, s)).lineTo(...at(u + (u < len / 2 ? -0.07 : 0.07), s + (rnd() - 0.5) * 0.02));
  }
  g.stroke({ width: 0.012, color: 0xe8dcc0 });
  g.poly(quad(rect(m, m, len - m, wid - m), rnd, 0.005)).fill(edge).stroke({ width: LINE, color: INK });
  const e = Math.min(0.12, wid * 0.13);
  g.poly(quad(rect(m + 0.16, m + e, len - m - 0.16, wid - m - e), rnd, 0.003)).fill(ground);
  // The ends' bands: the edge colour with a stripe of the motif.
  for (const u of [m + 0.08, len - m - 0.08]) g.moveTo(...at(u, m + 0.03)).lineTo(...at(u, wid - m - 0.03));
  g.stroke({ width: 0.02, color: motif, alpha: 0.85 });
  g.poly(quad(rect(m + 0.2, m + e + 0.04, len - m - 0.2, wid - m - e - 0.04), rnd, 0.002)).stroke({ width: 0.012, color: motif, alpha: 0.75 });
  // A chain of lozenges down the middle, every half square.
  const mid = wid / 2;
  const r = Math.min(0.13, (wid - 2 * m - 2 * e) * 0.3);
  const n = Math.max(1, Math.round((len - 0.6) * 2));
  for (let i = 0; i < n; i++) {
    const u = 0.3 + ((len - 0.6) * (i + 0.5)) / n;
    g.poly([...at(u - r * 1.3, mid), ...at(u, mid - r), ...at(u + r * 1.3, mid), ...at(u, mid + r)]).fill(i % 2 ? motif : edge);
    g.poly([...at(u - r * 0.5, mid), ...at(u, mid - r * 0.4), ...at(u + r * 0.5, mid), ...at(u, mid + r * 0.4)]).fill(ground);
  }
}

/** Inlaid dance floor: a parquet field (basket weave, or herringbone) inside a darker banded border with a light
 * stringing and lozenges inlaid round it, a medallion in its middle: an eight-pointed star of light and dark
 * woods in a lozenge frame. Flush with the floor, so only a thin dark edge. */
export function danceFloor(g: Graphics, f: InteriorItem, v: number, rnd: () => number) {
  const { x, y, w, h } = f;
  const [field, band, inlay] = [
    [[0x9a6a3a, 0xb8854c, 0xcc9a5e], [0x4e2e1c, 0x6a4028], [0xe8cc96, 0x5a3420]],
    [[0xb08a5a, 0xccaa74, 0xdcbc86], [0x6a2e22, 0x8a3e2c], [0xf0dcae, 0x6a2e22]],
    [[0x5a3a24, 0x704a2e, 0x84583a], [0x2e1e14, 0x46301e], [0xe0c48a, 0x2e1e14]],
    [[0xa0784a, 0xbc905a, 0xd0a46a], [0x3e4a3a, 0x56664e], [0xece0c0, 0x8a2e26]],
  ][v % 4];
  // Laid exactly on its squares: the edge's line inside them.
  const [X0, Y0, X1, Y1] = [x + LINE / 2, y + LINE / 2, x + w - LINE / 2, y + h - LINE / 2];
  g.rect(X0, Y0, X1 - X0, Y1 - Y0).fill(band[0]).stroke({ width: LINE, color: 0x2a1a10 });
  const B = Math.min(0.36, Math.min(w, h) * 0.1);
  const [fx0, fy0, fx1, fy1] = [X0 + B, Y0 + B, X1 - B, Y1 - B];
  g.rect(X0 + 0.04, Y0 + 0.04, X1 - X0 - 0.08, Y1 - Y0 - 0.08).fill(band[1]);
  // Lozenges round the border.
  const lz = (cx: number, cy: number, r: number, color: number) => g.poly([cx - r, cy, cx, cy - r * 0.6, cx + r, cy, cx, cy + r * 0.6]).fill(color);
  for (let px = X0 + B; px < X1 - B + 0.01; px += 0.5) {
    lz(px, Y0 + B / 2, B * 0.32, inlay[0]);
    lz(px, Y1 - B / 2, B * 0.32, inlay[0]);
  }
  for (let py = Y0 + B + 0.5; py < Y1 - B - 0.01; py += 0.5) {
    const [r0, r1] = [B * 0.32 * 0.6, B * 0.32];
    g.poly([X0 + B / 2 - r0, py, X0 + B / 2, py - r1, X0 + B / 2 + r0, py, X0 + B / 2, py + r1]).fill(inlay[0]);
    g.poly([X1 - B / 2 - r0, py, X1 - B / 2, py - r1, X1 - B / 2 + r0, py, X1 - B / 2, py + r1]).fill(inlay[0]);
  }
  // The field.
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).fill(field[1]);
  const cell = 0.5;
  if (v % 2 === 0) {
    // Basket weave: half-square blocks of three strips, each block turned from its neighbours, the strips each
    // a shade of their own; the blocks first, then the seams (a fill takes any path drawn before it).
    const strips: [number[], boolean][] = [];
    for (let j = 0; fy0 + j * cell < fy1 - 0.01; j++) {
      for (let i = 0; fx0 + i * cell < fx1 - 0.01; i++) {
        const [bx0, by0] = [fx0 + i * cell, fy0 + j * cell];
        const [bx1, by1] = [Math.min(fx1, bx0 + cell), Math.min(fy1, by0 + cell)];
        const turned = (i + j) % 2 === 1;
        for (let k = 0; k < 3; k++) {
          const r = turned ? [bx0 + (cell * k) / 3, by0, Math.min(bx1, bx0 + (cell * (k + 1)) / 3), by1] : [bx0, by0 + (cell * k) / 3, bx1, Math.min(by1, by0 + (cell * (k + 1)) / 3)];
          if (r[2] - r[0] > 0.005 && r[3] - r[1] > 0.005) strips.push([r, turned]);
        }
      }
    }
    for (const [[a, b, c, d], turned] of strips) g.rect(a, b, c - a, d - b).fill(shade(turned ? field[1] : field[2], 0.94 + 0.12 * rnd()));
    for (const [[a, b, c, d]] of strips) g.rect(a, b, c - a, d - b);
    g.stroke({ width: 0.01, color: band[0], alpha: 0.45 });
    for (let j = 0; fy0 + j * cell < fy1 - 0.01; j++) for (let i = 0; fx0 + i * cell < fx1 - 0.01; i++) g.rect(fx0 + i * cell, fy0 + j * cell, Math.min(cell, fx1 - fx0 - i * cell), Math.min(cell, fy1 - fy0 - j * cell));
    g.stroke({ width: 0.016, color: band[0], alpha: 0.6 });
  } else {
    // Herringbone: courses of short boards laid slanting one way then the other, meeting in a zigzag.
    const rowH = 0.3;
    const step = 0.1;
    const boards: number[][] = [];
    for (let j = 0; fy0 + j * rowH < fy1 - 0.01; j++) {
      const ya = fy0 + j * rowH;
      const yb = Math.min(fy1, ya + rowH);
      const lean = (j % 2 ? 1 : -1) * rowH * 0.6;
      for (let px = fx0 - rowH; px < fx1 + rowH; px += step) {
        // A board: a parallelogram, clipped to the field.
        const q = [px, ya, px + step, ya, px + step + lean, yb, px + lean, yb];
        for (let k = 0; k < 8; k += 2) q[k] = Math.max(fx0, Math.min(fx1, q[k]));
        if (Math.max(q[0], q[2], q[4], q[6]) - Math.min(q[0], q[2], q[4], q[6]) > 0.01) boards.push(q);
      }
    }
    for (const q of boards) g.poly(q).fill(shade(field[1], 0.92 + 0.16 * rnd()));
    for (const q of boards) g.poly(q);
    g.stroke({ width: 0.01, color: band[0], alpha: 0.5 });
  }
  // A light stringing round the field.
  g.rect(fx0 - 0.03, fy0 - 0.03, fx1 - fx0 + 0.06, fy1 - fy0 + 0.06).stroke({ width: 0.025, color: inlay[0] });
  g.rect(fx0, fy0, fx1 - fx0, fy1 - fy0).stroke({ width: 0.015, color: band[0] });
  // The medallion: a lozenge frame round an eight-pointed star, each point half light, half dark.
  const [cx, cy] = [x + w / 2, y + h / 2];
  const R = Math.min(fx1 - fx0, fy1 - fy0) * 0.36;
  if (R < 0.3) return;
  g.poly([cx - R * 1.1, cy, cx, cy - R * 1.1, cx + R * 1.1, cy, cx, cy + R * 1.1]).fill(band[1]).stroke({ width: 0.02, color: band[0] });
  g.poly([cx - R * 0.98, cy, cx, cy - R * 0.98, cx + R * 0.98, cy, cx, cy + R * 0.98]).fill(field[2]).stroke({ width: 0.02, color: inlay[0] });
  for (let k = 0; k < 8; k++) {
    const a = (k * Math.PI) / 4;
    const rr = k % 2 ? R * 0.62 : R * 0.88;
    const [tx, ty] = [cx + Math.cos(a) * rr, cy + Math.sin(a) * rr];
    const ri = R * 0.24;
    const [lx0, ly0] = [cx + Math.cos(a - Math.PI / 8) * ri, cy + Math.sin(a - Math.PI / 8) * ri];
    const [lx1, ly1] = [cx + Math.cos(a + Math.PI / 8) * ri, cy + Math.sin(a + Math.PI / 8) * ri];
    g.poly([cx, cy, lx0, ly0, tx, ty]).fill(inlay[0]);
    g.poly([cx, cy, tx, ty, lx1, ly1]).fill(inlay[1]);
    g.poly([lx0, ly0, tx, ty, lx1, ly1]).stroke({ width: 0.012, color: band[0], join: 'round' });
  }
  g.poly(ring(cx, cy, R * 0.12, rnd, 0, 8)).fill(inlay[1]).stroke({ width: 0.012, color: band[0] });
}

/** Harpsichord with its stool (along its length, the keyboard at the near end): a wing-shaped case in black,
 * red, green or walnut with a gilt band, its long straight spine and the bentside sweeping in to a narrow tail;
 * the soundboard inside painted with flowers round a gilt rose, the strings running back to the bentside over
 * the bridge; the keyboard of ivory naturals and black sharps under the nameboard; a cushioned stool before it. */
export function harpsichord(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { along, len, at, rect } = frame(f);
  const wid = along ? f.h : f.w;
  const k = Math.min(1, len / 2);
  const case_ = [
    [0x1e1a1c, 0x2e2a2e, 0x46404a],
    [0x5a1a16, 0x8a2a22, 0xa84034],
    [0x1e3a2e, 0x2e5a44, 0x46745a],
    [0x4e3220, 0x74503a, 0x906a4a],
  ][v % 4];
  const W = Math.min(0.66, wid - 0.2);
  const s0 = wid / 2 - W / 2;
  const kb = 0.4 * k;
  const tail = len - 0.06;
  /** The case's width at `p` (0 at the keyboard, 1 at the tail): the cheek straight, then the bentside sweeping in. */
  const width = (p: number) => {
    const t = Math.min(1, Math.max(0, (p - 0.16) / 0.84));
    return W * (1 - 0.78 * t * t * (3 - 2 * t));
  };
  const U = (p: number) => kb + (tail - kb) * p;
  const edge = (inset: number, from: number): Pt[] => {
    const q: Pt[] = [at(U(from) + inset, s0 + inset), at(tail - inset, s0 + inset)];
    for (let i = 16; i >= 0; i--) {
      const p = from + ((1 - from) * i) / 16;
      q.push(at(Math.min(tail - inset, Math.max(U(from) + inset, U(p))), s0 + width(p) - inset));
    }
    return q;
  };
  const outline = edge(0, 0);
  castShadow(g, outline.flat(), light, 0.28);
  g.poly(outline.flat()).fill(case_[1]).stroke({ width: OUT, color: INK, join: 'round' });
  // The soundboard inside the case's rim (behind the keyboard and nameboard).
  const pk = 0.24 / (tail - kb);
  g.poly(edge(0.05, pk).flat()).fill(0xdcc08a).stroke({ width: 0.016, color: GILT[1] });
  // Strings along it from the nameboard back, each ending just short of the bentside.
  for (let i = 0; i < 13; i++) {
    const s = 0.08 + ((W - 0.16) * i) / 12;
    let p = 1;
    for (let t = pk; t <= 1; t += 0.01) {
      if (width(t) - 0.07 < s) {
        p = t;
        break;
      }
    }
    g.moveTo(...at(U(pk) + 0.03, s0 + s)).lineTo(...at(Math.max(U(pk) + 0.06, U(p) - 0.03), s0 + s));
  }
  g.stroke({ width: 0.006, color: 0x6a5a42 });
  // The bridge along the bentside, painted flowers, the rose.
  const br: Pt[] = [];
  for (let i = 0; i <= 10; i++) {
    const p = pk + 0.08 + ((0.92 - pk) * i) / 10;
    br.push(at(U(p) - 0.04, s0 + width(p) - 0.12));
  }
  g.moveTo(...br[0]);
  for (const q of br.slice(1)) g.lineTo(...q);
  g.stroke({ width: 0.016, color: 0x6a4a2a, join: 'round' });
  for (let i = 0; i < 7; i++) {
    const p = pk + 0.1 + rnd() * 0.5;
    g.circle(...at(U(p), s0 + 0.08 + rnd() * Math.max(0.05, width(p) - 0.25)), 0.016).fill([0xc8453a, 0x4f72a8, 0xf2c94c, 0x6a8a3a][i % 4]);
  }
  const [rx, ry] = at(U(0.42), s0 + W * 0.4);
  g.circle(rx, ry, 0.055).fill(GILT[1]).stroke({ width: 0.012, color: INK });
  g.circle(rx, ry, 0.028).fill(0x3a2a1a);
  g.poly(outline.flat()).stroke({ width: 0.012, color: GILT[2], alpha: 0.7 });
  // The keyboard between the cheeks: ivory naturals, black sharps in twos and threes, the nameboard behind.
  const [k0, k1] = [s0 + 0.05, s0 + W - 0.05];
  g.poly(quad(rect(kb + 0.17, k0, kb + 0.23, k1), rnd, 0.002)).fill(case_[0]).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(kb + 0.01, k0, kb + 0.17, k1), rnd, 0.002)).fill(0xece2c8).stroke({ width: LINE, color: INK });
  const keyW = 0.042;
  for (let s = k0 + keyW; s < k1 - 0.01; s += keyW) g.moveTo(...at(kb + 0.02, s)).lineTo(...at(kb + 0.17, s));
  g.stroke({ width: 0.006, color: 0x8a7a5a });
  for (let i = 1, n = 0; k0 + i * keyW < k1 - 0.01; i++, n = (n + 1) % 7) {
    if (n === 2 || n === 6) continue;
    const s = k0 + i * keyW;
    g.poly(quad(rect(kb + 0.09, s - 0.012, kb + 0.17, s + 0.012), rnd, 0)).fill(0x1e1a18);
  }
  // The stool before it.
  const st = rect(0.06, wid / 2 - 0.2, Math.max(0.2, kb - 0.06), wid / 2 + 0.2);
  board(g, st, DARK_WOOD, light, rnd, !along, 0.03, 0);
  g.roundRect(st[0] + 0.04, st[1] + 0.04, st[2] - st[0] - 0.08, st[3] - st[1] - 0.08, 0.04).fill(PLUSH[v % PLUSH.length][1]).stroke({ width: 0.012, color: INK });
}

/** Musicians' corner against a wall (2×2): a harp standing in the back corner (from a high angle, screen up kept
 * up), two chairs facing the room with music stands before them (an open book of music on each), a lute (or a
 * fiddle and its bow) on one chair, a tabor drum with its sticks on the floor. Odd looks are mirrored. */
export function musiciansCorner(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, at: at0 } = frame(f, back);
  const wid = Math.min(f.w, f.h);
  const flip = v % 2 === 1;
  const at = (u: number, s: number) => at0(flip ? len - u : u, s);
  const fiddle = v >> 1 === 1;
  const q = (u: number, s: number, a: number, b: number, c: number, d: number) => [...at(u + a, s + b), ...at(u + c, s + b), ...at(u + c, s + d), ...at(u + a, s + d)];
  // Chairs: a seat with a cushion, a curved back rail on the wall's side.
  const chair = (u: number, s: number, cushion: readonly number[]) => {
    castShadow(g, q(u, s, -0.2, -0.24, 0.2, 0.2), light, 0.12);
    g.poly(q(u, s, -0.19, -0.18, 0.19, 0.2)).fill(WOOD[1]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(q(u, s, -0.13, -0.11, 0.13, 0.15)).fill(cushion[1]).stroke({ width: 0.012, color: INK });
    g.poly(q(u, s, -0.1, -0.08, 0.02, 0.02)).fill({ color: cushion[2], alpha: 0.6 });
    for (const [w, c] of [
      [0.09, INK],
      [0.055, DARK_WOOD[1]],
    ] as const) {
      g.moveTo(...at(u - 0.22, s - 0.15)).quadraticCurveTo(...at(u, s - 0.28), ...at(u + 0.22, s - 0.15));
      g.stroke({ width: w, color: c, cap: 'round' });
    }
  };
  // Music stands: a tripod under a slanted desk with an open book of music on it.
  const stand = (u: number, s: number) => {
    for (const a of [0.6, 2.5, 4.4]) {
      const [px, py] = at(u + Math.cos(a) * 0.16, s + 0.04 + Math.sin(a) * 0.12);
      g.moveTo(...at(u, s + 0.04)).lineTo(px, py);
    }
    g.stroke({ width: 0.022, color: INK, cap: 'round' });
    g.poly(q(u, s, -0.23, -0.07, 0.23, 0.07)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    for (const side of [-1, 1]) g.poly(q(u, s, side < 0 ? -0.2 : 0.005, -0.11, side < 0 ? -0.005 : 0.2, 0.04)).fill(LINEN[2]).stroke({ width: 0.01, color: INK });
    for (const side of [-1, 1]) for (const d of [-0.07, -0.04, -0.01, 0.02]) g.moveTo(...at(u + side * 0.03, s + d)).lineTo(...at(u + side * 0.17, s + d));
    g.stroke({ width: 0.006, color: 0x3a3028 });
  };
  const [c1u, c2u] = [len * 0.5, len * 0.82];
  const cs = WALL_GAP + 0.3;
  chair(c1u, cs, CUSHIONS[1]);
  chair(c2u, cs + 0.06, CUSHIONS[2]);
  stand(c1u, cs + 0.62);
  stand(c2u, cs + 0.68);
  // A lute or a fiddle lying across the first chair.
  const [lx, ly] = at(c1u - 0.02, cs + 0.02);
  const [ex, ey] = at(c1u + 0.3, cs - 0.2);
  const la = Math.atan2(ey - ly, ex - lx);
  const [ca, sa] = [Math.cos(la), Math.sin(la)];
  const rb = fiddle ? 0.1 : 0.15;
  inkLine(g, [[lx, ly], [lx + ca * 0.38, ly + sa * 0.38]], 0.032, DARK_WOOD[1]);
  // The lute's pegbox bent back; the fiddle's scroll.
  if (!fiddle) inkLine(g, [[lx + ca * 0.38, ly + sa * 0.38], [lx + ca * 0.4 - sa * 0.08, ly + sa * 0.4 + ca * 0.08]], 0.03, DARK_WOOD[0]);
  else g.circle(lx + ca * 0.39, ly + sa * 0.39, 0.03).fill(DARK_WOOD[0]).stroke({ width: 0.01, color: INK });
  g.poly(oval(lx - ca * 0.05, ly - sa * 0.05, rb * 1.35, rb, la, 20)).fill(fiddle ? 0x8a4a24 : 0xc08a48).stroke({ width: LINE, color: INK });
  g.poly(oval(lx - ca * 0.07 + light[0] * 0.02, ly - sa * 0.07 + light[1] * 0.02, rb * 0.9, rb * 0.6, la, 14)).fill({ color: fiddle ? 0xa86a34 : 0xd8a868, alpha: 0.7 });
  if (fiddle) {
    for (const sd of [-1, 1]) g.poly(oval(lx - ca * 0.04 - sa * sd * 0.045, ly - sa * 0.04 + ca * sd * 0.045, 0.03, 0.008, la, 8)).fill(INK);
    inkLine(g, [[lx - ca * 0.2 - sa * 0.14, ly - sa * 0.2 + ca * 0.14], [lx + ca * 0.25 - sa * 0.18, ly + sa * 0.25 + ca * 0.18]], 0.012, 0xc8a878);
  } else {
    g.circle(lx + ca * 0.02, ly + sa * 0.02, 0.045).fill(0x2a1a0e);
    g.circle(lx + ca * 0.02, ly + sa * 0.02, 0.045).stroke({ width: 0.008, color: GILT[1] });
  }
  // The tabor on the floor before the stands, its cords round its rim, its sticks across it.
  const [dx, dy] = at(len * 0.22, Math.min(wid - 0.3, cs + 1.08));
  castShadow(g, ring(dx, dy, 0.22, rnd, 0, 12), light, 0.14);
  g.poly(ring(dx, dy, 0.24, rnd, 0.006, 20)).fill(0xa8622e).stroke({ width: OUT, color: INK });
  for (let i = 0; i < 10; i++) {
    const a0 = (i / 10) * Math.PI * 2;
    const a1 = a0 + Math.PI / 10;
    g.moveTo(dx + Math.cos(a0) * 0.235, dy + Math.sin(a0) * 0.235).lineTo(dx + Math.cos(a1) * 0.19, dy + Math.sin(a1) * 0.19).lineTo(dx + Math.cos(a1 + Math.PI / 10) * 0.235, dy + Math.sin(a1 + Math.PI / 10) * 0.235);
  }
  g.stroke({ width: 0.012, color: 0xe8d8b0 });
  g.circle(dx, dy, 0.18).fill(0xecdcb4).stroke({ width: 0.018, color: 0x6a3a1c });
  g.circle(dx + light[0] * 0.05, dy + light[1] * 0.05, 0.08).fill({ color: 0xfff1cf, alpha: 0.45 });
  for (const a of [0.45, 1.25]) {
    const p: Pt = [dx - Math.cos(a) * 0.24, dy - Math.sin(a) * 0.1];
    const e: Pt = [dx + Math.cos(a) * 0.16, dy + Math.sin(a) * 0.22];
    inkLine(g, [p, e], 0.022, WOOD[2]);
    g.circle(...e, 0.03).fill(WOOD[2]).stroke({ width: 0.01, color: INK });
  }
  // The harp in the back corner, standing: from a high angle like the other uprights (screen up kept up, heights at
  // half, its round foot a fat oval), well inside the corner's frame.
  const [hx, hy] = at(len * 0.2, WALL_GAP + 0.42);
  const { pt, right } = screenAxes(light);
  const H = (a: number, b: number) => pt(hx, hy, a, b);
  castShadow(g, ring(...H(0.02, -0.16), 0.07, rnd, 0, 8), light, 0.3);
  g.poly(oval(...H(0.02, -0.17), 0.12, 0.1, Math.atan2(right[1], right[0]), 14)).fill(WOOD[1]).stroke({ width: LINE, color: INK });
  g.poly(oval(...H(0.0, -0.15), 0.06, 0.045, Math.atan2(right[1], right[0]), 10)).fill({ color: WOOD[2], alpha: 0.8 });
  // Strings between the soundbox and the neck, falling straight down the screen.
  for (let i = 0; i < 6; i++) {
    const t = (i + 0.5) / 6;
    g.moveTo(...H(-0.08 + 0.24 * t, -0.13 + 0.3 * t)).lineTo(...H(-0.08 + 0.24 * t, 0.27 + 0.05 * Math.sin(t * Math.PI)));
  }
  g.stroke({ width: 0.012, color: 0xf0e4c0 });
  // The soundbox slanting up from the foot (its face showing, broad), the curved neck, the pillar.
  inkLine(g, [H(-0.1, -0.16), H(0.18, 0.2)], 0.1, 0xa8743a);
  inkLine(g, [H(-0.07, -0.14), H(0.17, 0.17)], 0.03, 0xc8945a);
  inkLine(g, [H(-0.12, 0.26), H(0.04, 0.36), H(0.2, 0.22)], 0.05, GILT[1], true);
  inkLine(g, [H(-0.12, -0.14), H(-0.12, 0.27)], 0.045, GILT[1]);
}

/** Refreshment table (clothed, any length): a cloth caught up in swags along both sides, a silver punch bowl in
 * the middle with its ladle and cups round it, a cake with a slice cut from it, a plate of jam tarts, grapes,
 * candles. */
export function refreshmentTable(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { len, rect, at } = frame(f);
  const cloth = [
    [0xc8a8a0, 0xecd4cc, 0xf6e8e0, 0xc8586a],
    [0xb8bcc8, 0xe0e4ec, 0xf2f4f6, 0x4f72a8],
    [0xbcc4a0, 0xe0e6c8, 0xf0f4e0, 0x6a8a3a],
    [0xc8bca0, 0xe4dac0, 0xf0e8d4, 0xc9a24a],
  ][v % 4];
  castShadow(g, quad(rect(0.04, 0.1, len - 0.04, 0.9), rnd, 0), light, 0.16);
  g.poly(quad(rect(0.04, 0.1, len - 0.04, 0.9), rnd, 0.006)).fill(cloth[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(quad(rect(0.1, 0.16, len - 0.1, 0.84), rnd, 0.004)).fill(cloth[1]);
  litEdges(g, rect(0.1, 0.16, len - 0.1, 0.84), light, 0.03);
  g.fill({ color: cloth[2], alpha: 0.8 });
  // Swags along both sides, a ribbon bow where they meet.
  const n = Math.max(2, Math.round(len * 2));
  for (const [sa, sb] of [
    [0.1, 0.17],
    [0.9, 0.83],
  ]) {
    for (let i = 0; i < n; i++) {
      const ua = 0.08 + ((len - 0.16) * i) / n;
      const ub = 0.08 + ((len - 0.16) * (i + 1)) / n;
      g.moveTo(...at(ua, sa)).quadraticCurveTo(...at((ua + ub) / 2, sb + (sb - sa) * 0.6), ...at(ub, sa));
    }
    g.stroke({ width: 0.02, color: cloth[3], alpha: 0.85 });
    for (let i = 0; i <= n; i++) g.circle(...at(0.08 + ((len - 0.16) * i) / n, sa), 0.024).fill(cloth[3]).stroke({ width: 0.008, color: INK });
  }
  // The punch bowl: a broad silver rim round red punch with slices of orange floating in it, the ladle's cup by
  // the rim and its handle out over it; silver cups round the bowl.
  const [bx, by] = at(len / 2, 0.5);
  g.poly(ring(bx, by, 0.22, rnd, 0.004, 24)).fill(0x8a9096).stroke({ width: OUT, color: INK });
  g.poly(ring(bx + light[0] * 0.012, by + light[1] * 0.012, 0.2, rnd, 0.003, 24)).fill(0xc4c8cc);
  g.circle(bx, by, 0.15).fill(0x8a1e2a).stroke({ width: 0.01, color: INK });
  g.circle(bx + light[0] * 0.05, by + light[1] * 0.05, 0.06).fill({ color: 0xc8485a, alpha: 0.5 });
  for (const [ox, oy] of [
    [-0.06, 0.05],
    [0.05, 0.07],
    [0.02, -0.07],
  ]) {
    g.circle(bx + ox, by + oy, 0.032).fill(0xf2a03a).stroke({ width: 0.008, color: 0x8a4a1a });
    g.circle(bx + ox, by + oy, 0.018).fill(0xf8d07a);
  }
  const la = Math.atan2(light[1], light[0]) + Math.PI * 0.6;
  const [lc, ls] = [Math.cos(la), Math.sin(la)];
  inkLine(g, [[bx + lc * 0.1, by + ls * 0.1], [bx + lc * 0.3, by + ls * 0.3]], 0.018, 0xd8dce0);
  g.circle(bx + lc * 0.08, by + ls * 0.08, 0.04).fill(0xb8bcc0).stroke({ width: 0.01, color: INK });
  for (const a of [la + 1.6, la + 2.4, la + 3.2, la - 1.4]) {
    const [cx, cy] = [bx + Math.cos(a) * 0.3, by + Math.sin(a) * 0.3];
    if (Math.abs(cy - by) > 0.34 || Math.abs(cx - bx) > len / 2 - 0.1) continue;
    g.circle(cx, cy, 0.05).fill(0xd8dce0).stroke({ width: 0.012, color: INK });
    g.circle(cx, cy, 0.032).fill(0x8a1e2a);
  }
  // A cake on a gilt stand: two tiers iced white and pink, piped round their edges, strawberries on top; a slice
  // lying on its side on a little plate by it, its sponge, jam and cream in layers.
  if (len >= 2) {
    const [kx, ky] = at(0.44, 0.48);
    g.poly(ring(kx, ky, 0.25, rnd, 0.004, 22)).fill(GILT[1]).stroke({ width: LINE, color: INK });
    g.poly(ring(kx, ky, 0.2, rnd, 0.006, 22)).fill(0xf6efe0).stroke({ width: LINE, color: INK });
    for (let i = 0; i < 14; i++) g.circle(kx + Math.cos((i / 14) * Math.PI * 2) * 0.175, ky + Math.sin((i / 14) * Math.PI * 2) * 0.175, 0.018).fill(0xffffff).stroke({ width: 0.006, color: 0xb8aa90 });
    g.poly(ring(kx + light[0] * 0.01, ky + light[1] * 0.01, 0.12, rnd, 0.004, 18)).fill(0xf0b8c0).stroke({ width: 0.012, color: INK });
    for (let i = 0; i < 9; i++) g.circle(kx + Math.cos((i / 9) * Math.PI * 2 + 0.3) * 0.1, ky + Math.sin((i / 9) * Math.PI * 2 + 0.3) * 0.1, 0.014).fill(0xffffff);
    for (const [ox, oy] of [
      [-0.03, -0.02],
      [0.035, -0.01],
      [0, 0.035],
    ]) {
      g.poly(oval(kx + ox, ky + oy, 0.03, 0.024, 0.4, 8)).fill(0xc8202a).stroke({ width: 0.008, color: INK });
      g.circle(kx + ox - 0.012, ky + oy - 0.016, 0.009).fill(0x4a7a2a);
    }
    // The slice on its plate (where there is room beside the bowl).
    if (len >= 3) {
      const [px, py] = at(0.86, 0.72);
      g.circle(px, py, 0.11).fill(0xf2f0ea).stroke({ width: 0.012, color: INK });
      const tri: Pt[] = [
        [px - 0.07, py + 0.05],
        [px + 0.08, py + 0.05],
        [px + 0.06, py - 0.06],
      ];
      g.poly(tri.flat()).fill(0xe0b070).stroke({ width: 0.012, color: INK, join: 'round' });
      for (const [t, c] of [
        [0.35, 0xb83a4a],
        [0.65, 0xfaf4e8],
      ] as const) {
        g.moveTo(tri[0][0] + (tri[2][0] - tri[0][0]) * t, tri[0][1] + (tri[2][1] - tri[0][1]) * t).lineTo(tri[1][0] + (tri[2][0] - tri[1][0]) * t, tri[1][1] + (tri[2][1] - tri[1][1]) * t);
        g.stroke({ width: 0.016, color: c });
      }
      g.moveTo(tri[2][0], tri[2][1]).lineTo(tri[1][0], tri[1][1]).stroke({ width: 0.02, color: 0xffffff });
    }
    // Jam tarts on a plate, grapes.
    const [tx, ty] = at(len - 0.42, 0.42);
    g.circle(tx, ty, 0.16).fill(0xd8dce0).stroke({ width: LINE, color: INK });
    for (const [k, [ox, oy]] of [
      [-0.06, -0.05],
      [0.06, -0.04],
      [0, 0.06],
    ].entries()) {
      g.circle(tx + ox, ty + oy, 0.05).fill(0xd0a060).stroke({ width: 0.008, color: INK });
      g.circle(tx + ox, ty + oy, 0.028).fill([0xa82a3a, 0xe0a030, 0x6a2a6a][(v + k) % 3]);
    }
    const [gx, gy] = at(len - 0.3, 0.72);
    for (let i = 0; i < 7; i++) g.circle(gx + (rnd() - 0.5) * 0.1, gy + (rnd() - 0.5) * 0.08, 0.026).fill(0x6a3a6a).stroke({ width: 0.006, color: INK });
  }
  for (const u of [len / 2 - 0.42, len / 2 + 0.42]) if (u > 0.25 && u < len - 0.25) candle(g, ...at(u, 0.24));
}

/** Font: an octagonal stone basin on an octagonal step, its facets lit toward the light, water in the bowl with a
 * glint; in pale marble with a gilt rim; a four-lobed basin on a step in the shape of a cross, a gilt scallop shell
 * on its rim; or old and mossy. */
export function font(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const sc = Math.min(f.w, f.h);
  const oct = (r: number, dx = 0, dy = 0) => {
    const pts: number[] = [];
    for (let i = 0; i < 8; i++) pts.push(cx + dx + Math.cos(Math.PI / 8 + (i * Math.PI) / 4) * r, cy + dy + Math.sin(Math.PI / 8 + (i * Math.PI) / 4) * r);
    return pts;
  };
  if (v === 2) {
    // The step a cross, its arms reaching the square's sides; the basin four-lobed, a lobe down each arm.
    const [ar, re] = [0.19 * sc, 0.47 * sc];
    const cross = (k: number, dx = 0, dy = 0) =>
      [[-ar, -re], [ar, -re], [ar, -ar], [re, -ar], [re, ar], [ar, ar], [ar, re], [-ar, re], [-ar, ar], [-re, ar], [-re, -ar], [-ar, -ar]].flatMap(([x, y]) => [cx + dx + x * k, cy + dy + y * k]);
    castShadow(g, cross(1), light, 0.08);
    g.poly(cross(1)).fill(shade(STONE[0], 1.05)).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(cross(0.93, light[0] * 0.015, light[1] * 0.015)).fill(STONE[2]);
    const lobes = (r: number, dx = 0, dy = 0) => {
      const pts: number[] = [];
      for (let i = 0; i < 48; i++) {
        const a = (i / 48) * Math.PI * 2;
        const rr = r * (0.74 + 0.26 * Math.abs(Math.cos(2 * a)));
        pts.push(cx + dx + Math.cos(a) * rr, cy + dy + Math.sin(a) * rr);
      }
      return pts;
    };
    castShadow(g, lobes(0.3 * sc), light, 0.22);
    g.poly(lobes(0.36 * sc)).fill(MARBLE[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(lobes(0.33 * sc, light[0] * 0.012, light[1] * 0.012)).fill(MARBLE[1]);
    g.poly(lobes(0.25 * sc)).fill(0x2e5466).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(lobes(0.18 * sc, light[0] * 0.03, light[1] * 0.03)).fill({ color: 0x4e7e8a, alpha: 0.8 });
    g.poly(oval(cx + light[0] * 0.07, cy + light[1] * 0.07, 0.05, 0.02, -0.7, 10)).fill({ color: 0xffffff, alpha: 0.5 });
    // A gilt scallop shell lying on the step's near arm, its hinge outward, its ribs fanning toward the basin.
    const [hx, hy] = [cx, cy + 0.45 * sc];
    const fan: number[] = [hx, hy];
    for (let i = 0; i <= 10; i++) {
      const a = -Math.PI / 2 - 0.85 + (1.7 * i) / 10;
      const r = 0.16 * sc * (i % 2 ? 0.9 : 1);
      fan.push(hx + Math.cos(a) * r, hy + Math.sin(a) * r);
    }
    g.poly(fan).fill(GILT[1]).stroke({ width: 0.016, color: INK, join: 'round' });
    for (let i = 1; i < 10; i += 2) {
      const a = -Math.PI / 2 - 0.85 + (1.7 * i) / 10;
      g.moveTo(hx, hy).lineTo(hx + Math.cos(a) * 0.14 * sc, hy + Math.sin(a) * 0.14 * sc);
    }
    g.stroke({ width: 0.012, color: GILT[0] });
    return;
  }
  const t = v === 1 ? MARBLE : v === 3 ? STONE.map((c) => shade(c, 0.88)) : STONE;
  castShadow(g, oct(0.32 * sc), light, 0.3);
  g.poly(oct(0.47 * sc)).fill(shade(STONE[0], 1.05)).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(oct(0.44 * sc, light[0] * 0.015, light[1] * 0.015)).fill(STONE[1]);
  g.poly(oct(0.34 * sc)).fill(t[0]).stroke({ width: OUT, color: INK, join: 'round' });
  // Facets of the basin's rim, each lit by how it faces the light.
  for (let i = 0; i < 8; i++) {
    const a0 = Math.PI / 8 + (i * Math.PI) / 4;
    const a1 = a0 + Math.PI / 4;
    const [r0, r1] = [0.33 * sc, 0.24 * sc];
    g.poly([cx + Math.cos(a0) * r0, cy + Math.sin(a0) * r0, cx + Math.cos(a1) * r0, cy + Math.sin(a1) * r0, cx + Math.cos(a1) * r1, cy + Math.sin(a1) * r1, cx + Math.cos(a0) * r1, cy + Math.sin(a0) * r1]).fill(shade(t[1], 0.92 + 0.14 * lit((a0 + a1) / 2)));
  }
  g.poly(oct(0.34 * sc)).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oct(0.23 * sc)).fill(0x2e5466).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oct(0.17 * sc, light[0] * 0.03, light[1] * 0.03)).fill({ color: 0x4e7e8a, alpha: 0.8 });
  g.poly(oval(cx + light[0] * 0.07, cy + light[1] * 0.07, 0.06, 0.022, -0.7, 10)).fill({ color: 0xffffff, alpha: 0.5 });
  if (v === 1) g.poly(oct(0.335 * sc)).stroke({ width: 0.016, color: GILT[1] });
  if (v === 3) for (let i = 0; i < 4; i++) g.poly(patch(cx + (rnd() - 0.5) * 0.5 * sc, cy - 0.3 * sc + rnd() * 0.12, 0.05, 0.03, rnd, 0.3, 8)).fill({ color: 0x5a7a32, alpha: 0.75 });
}

/** Cloister well: an octagonal stone well-head on a step, dark water in it, posts at two sides holding a small
 * gabled roof over the windlass (tile, slate, copper or shingle), the water showing either side of it, a bucket
 * on the rim. */
export function cloisterWell(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { along, len, at } = frame(f);
  const wid = along ? f.h : f.w;
  const [cx, cy] = at(len / 2, wid / 2);
  const sc = Math.min(len, wid) / 2;
  const oct = (r: number, dx = 0, dy = 0) => {
    const pts: number[] = [];
    for (let i = 0; i < 8; i++) pts.push(cx + dx + Math.cos(Math.PI / 8 + (i * Math.PI) / 4) * r * sc, cy + dy + Math.sin(Math.PI / 8 + (i * Math.PI) / 4) * r * sc);
    return pts;
  };
  castShadow(g, oct(0.62), light, 0.2);
  g.poly(oct(0.92)).fill(STONE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(oct(0.88, light[0] * 0.02, light[1] * 0.02)).fill(STONE[1]);
  g.poly(oct(0.64)).fill(MARBLE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  for (let i = 0; i < 8; i++) {
    const a0 = Math.PI / 8 + (i * Math.PI) / 4;
    const a1 = a0 + Math.PI / 4;
    const [r0, r1] = [0.62 * sc, 0.48 * sc];
    g.poly([cx + Math.cos(a0) * r0, cy + Math.sin(a0) * r0, cx + Math.cos(a1) * r0, cy + Math.sin(a1) * r0, cx + Math.cos(a1) * r1, cy + Math.sin(a1) * r1, cx + Math.cos(a0) * r1, cy + Math.sin(a0) * r1]).fill(shade(MARBLE[1], 0.9 + 0.14 * lit((a0 + a1) / 2)));
  }
  g.poly(oct(0.47)).fill(0x16242c).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(oct(0.38, sc * 0.04, sc * 0.04)).fill(0x22343e);
  g.poly(oval(cx + light[0] * 0.3 * sc - light[1] * 0.1, cy + light[1] * 0.3 * sc + light[0] * 0.1, 0.09, 0.03, -0.6, 10)).fill({ color: 0x6a9aaa, alpha: 0.7 });
  // A little gabled roof on two posts, its ridge `along` the item, the water showing either side of its eaves: red
  // tile, slate, green copper or shingle.
  const roof = [
    [0x7a3a2a, 0xa85a3e, 0xc87a56],
    [0x40464e, 0x5e666e, 0x7e868e],
    [0x3e6a5a, 0x5a8a76, 0x7aa892],
    [0x5e4026, 0x84603a, 0xa47e52],
  ][v % 4];
  const [ra, rs] = [0.54 * sc, 0.34 * sc];
  const R = (a: number, s: number) => at(len / 2 + a, wid / 2 + s);
  const pa = 0.7 * sc;
  // The posts, out past the roof's gable ends, the windlass's timber drum between them showing at its ends, the
  // crank past one post.
  for (const a of [-pa, pa]) {
    const [px, py] = R(a, 0);
    g.rect(px - 0.06, py - 0.06, 0.12, 0.12).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  }
  for (const sd of [-1, 1]) {
    const q = [...R(sd * (ra - 0.02), -0.07), ...R(sd * (pa - 0.06), -0.07), ...R(sd * (pa - 0.06), 0.07), ...R(sd * (ra - 0.02), 0.07)];
    g.poly(q).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...R(sd * (ra + 0.04), -0.07)).lineTo(...R(sd * (ra + 0.04), 0.07)).stroke({ width: 0.012, color: IRON[0] });
  }
  inkLine(g, [R(pa, 0), R(pa + 0.14, 0)], 0.03, IRON[1]);
  inkLine(g, [R(pa + 0.14, 0), R(pa + 0.14, 0.12)], 0.025, IRON[1]);
  // A bucket on the rim.
  const [bx, by] = R(0.2 * sc, 0.56 * sc);
  g.circle(bx, by, 0.08).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.circle(bx, by, 0.055).fill(0x3e5a6a);
  // The roof's shadow falls well off it (it stands head high), then its two slopes, the one toward the light paler.
  const eaves = [...R(-ra, -rs), ...R(ra, -rs), ...R(ra, rs), ...R(-ra, rs)];
  castShadow(g, eaves, light, 0.5);
  const sideLit = (R(0, -1)[0] - R(0, 0)[0]) * light[0] + (R(0, -1)[1] - R(0, 0)[1]) * light[1] > 0 ? -1 : 1;
  g.poly(eaves).fill(roof[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([...R(-ra + 0.02, sideLit * (rs - 0.02)), ...R(ra - 0.02, sideLit * (rs - 0.02)), ...R(ra - 0.02, 0), ...R(-ra + 0.02, 0)]).fill(roof[2]);
  g.poly([...R(-ra + 0.02, -sideLit * (rs - 0.02)), ...R(ra - 0.02, -sideLit * (rs - 0.02)), ...R(ra - 0.02, 0), ...R(-ra + 0.02, 0)]).fill(roof[1]);
  // Courses down each slope: rounded tiles or shingles in scallops, slates in staggered rows, copper in standing
  // seams down the slope.
  for (const side of [-1, 1]) {
    const dark = shade(roof[0], 0.8);
    if (v % 4 === 2) {
      for (let a = -ra + 0.09; a < ra - 0.04; a += 0.11) g.moveTo(...R(a, side * 0.03)).lineTo(...R(a, side * (rs - 0.03)));
      g.stroke({ width: 0.014, color: dark });
      continue;
    }
    for (let k = 1; k <= 3; k++) {
      const sv = side * rs * (k / 3.3);
      if (v % 4 === 1) {
        g.moveTo(...R(-ra + 0.03, sv)).lineTo(...R(ra - 0.03, sv));
        for (let a = -ra + (k % 2 ? 0.08 : 0.14); a < ra - 0.04; a += 0.12) g.moveTo(...R(a, sv)).lineTo(...R(a, sv - side * rs * 0.28));
      } else {
        const st = v % 4 === 0 ? 0.09 : 0.12;
        g.moveTo(...R(-ra + 0.03, sv));
        for (let a = -ra + 0.03; a < ra - 0.03 - 0.01; a += st) g.quadraticCurveTo(...R(Math.min(ra - 0.03, a + st / 2), sv + side * 0.05), ...R(Math.min(ra - 0.03, a + st), sv));
      }
    }
    g.stroke({ width: 0.014, color: dark, join: 'round' });
  }
  // The ridge cap, and dark bargeboards along the gable ends.
  inkLine(g, [R(-ra - 0.01, 0), R(ra + 0.01, 0)], 0.05, shade(roof[0], 0.9));
  for (const sd of [-1, 1]) inkLine(g, [R(sd * ra, -rs + 0.01), R(sd * ra, 0), R(sd * ra, rs - 0.01)], 0.03, DARK_WOOD[1]);
}
