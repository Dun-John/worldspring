import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { rng } from '../atlas';
import { BRASS, DARK_WOOD, DYES, INK, IRON, LINE, LINEN, type Light, OUT, WOOD, board, castShadow, frame, oval, patch, planks, quad, ring, shade } from './common';
import { END_GAP, STONE, WALL_GAP, masonry } from './home';
import { BRONZE, GILT } from './tables';
import { stageCurtain } from './hangings';

/** Pale stone for statues, effigies and fine work: shade, face, lit. */
export const MARBLE = [0x8e8a80, 0xc4beb0, 0xe2dccd];

/** A candle in a holder, its flame and a little glow. */
export function candle(g: Graphics, x: number, y: number) {
  g.circle(x, y, 0.09).fill({ color: 0xf59e0b, alpha: 0.18 });
  g.circle(x, y, 0.05).fill(GILT[1]).stroke({ width: 0.012, color: INK });
  g.circle(x, y, 0.03).fill(LINEN[2]);
  g.circle(x, y, 0.015).fill(0xffc23a);
}

/** Altar against a wall: a block of pale stone, a cloth laid across it and hanging over the front, a
 * candle at each end and a book open in the middle, a step before it. */
export function altar(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(0.14, 0.78, len - 0.14, 0.96), STONE, light, rnd, along, 0.03, 0);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.8), MARBLE, light, rnd, along, 0.06, 0);
  const cloth = [0x8a2e26, 0x3e5a8a, 0xf0e8d4, 0x6a4a6a][v % 4];
  const [c0, c1] = [len * 0.32, len * 0.68];
  g.poly(quad(rect(c0, WALL_GAP + 0.04, c1, 0.88), rnd, 0.005)).fill(cloth).stroke({ width: LINE, color: INK });
  g.poly(quad(rect(c0 + 0.04, 0.8, c1 - 0.04, 0.86), rnd, 0.003)).fill(GILT[1]);
  const [bx0, by0, bx1, by1] = rect(len / 2 - 0.16, 0.38, len / 2 + 0.16, 0.6);
  g.rect(bx0, by0, bx1 - bx0, by1 - by0).fill(LINEN[2]).stroke({ width: 0.012, color: INK });
  g.moveTo(...at(len / 2, 0.38)).lineTo(...at(len / 2, 0.6)).stroke({ width: 0.012, color: LINEN[0] });
  for (const u of [END_GAP + 0.2, len - END_GAP - 0.2]) candle(g, ...at(u, 0.5));
}

/** Sarcophagus: the stone effigy on its tomb chest without the sword — a knight lying with head on a pillow,
 * hands together in prayer, a dog at the feet. */
export function sarcophagus(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  stoneEffigy(g, f, light, rnd, false);
}

/** Bell, hung in a frame: two heavy beams across the loft, the headstock between them, the bronze bell
 * under it (seen as its crown and lip), its rope running down off one side. */
export function bell(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  const wid = along ? f.h : f.w;
  const [cx, cy] = at(len / 2, wid / 2);
  const R = Math.min(len, wid) * 0.32;
  // The bell: its lip (widest), the waist, the crown, lit toward the light.
  g.poly(ring(cx, cy, R, rnd, 0.008, 32)).fill(BRONZE[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx + light[0] * R * 0.08, cy + light[1] * R * 0.08, R * 0.86).fill(BRONZE[1]);
  g.circle(cx, cy, R * 0.6).stroke({ width: 0.02, color: BRONZE[0] });
  g.circle(cx + light[0] * R * 0.25, cy + light[1] * R * 0.25, R * 0.42).fill({ color: BRONZE[2], alpha: 0.7 });
  g.circle(cx, cy, R * 0.22).fill(BRONZE[0]).stroke({ width: LINE, color: INK });
  // Beams across, the headstock over the crown.
  for (const s of [wid / 2 - R - 0.1, wid / 2 + R + 0.02]) board(g, rect(0.06, s, len - 0.06, s + 0.1), DARK_WOOD, light, rnd, true, 0.025, 1);
  board(g, rect(len / 2 - 0.12, wid / 2 - R - 0.06, len / 2 + 0.12, wid / 2 + R + 0.06), WOOD, light, rnd, false, 0.03, 1);
  // The rope, down off the wheel at one side.
  const [rx, ry] = at(len / 2 + R + 0.1, wid / 2);
  g.moveTo(...at(len / 2 + 0.12, wid / 2)).lineTo(rx, ry).stroke({ width: 0.03, color: 0xc8b080 });
  g.circle(rx, ry, 0.06).stroke({ width: 0.025, color: 0xc8b080 });
  g.circle(rx, ry, 0.03).stroke({ width: 0.02, color: 0xc8b080 });
}

/** Statue on a plinth, seen from above: a long shadow across the floor, a plain stepped square plinth,
 * and on it a figure in bronze or warm marble — a big head on broad shoulders over a plain robe, kept
 * simple so it reads at a glance. */
export function statue(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  const outer = rect(0.08, Math.max(0.08, WALL_GAP - 0.08), 0.92, 0.94);
  castShadow(g, quad(outer, rnd, 0), light, 0.9);
  board(g, outer, STONE.map((c) => shade(c, 0.85)), light, rnd, true, 0.05, 0);
  const [ox0, oy0, ox1, oy1] = outer;
  board(g, [ox0 + 0.1, oy0 + 0.1, ox1 - 0.1, oy1 - 0.1], STONE, light, rnd, true, 0.04, 0);
  // Bronze (one gone green) or warm marble.
  const t = [
    [0x6a4a22, 0xa07a3e, 0xd0aa62],
    [0x8a7a62, 0xd2c4a6, 0xf0e4c8],
    [0x3e5e52, 0x5e8a74, 0x8ab8a0],
    [0x8a7a62, 0xd2c4a6, 0xf0e4c8],
  ][v % 4];
  const [cx, cy] = at(0.5, 0.52);
  const ink = { width: OUT, color: INK, join: 'round' as const };
  // A plain robe: a rounded body round the shoulders.
  g.poly(ring(cx, cy, 0.3, rnd, 0.01, 20)).fill(t[0]).stroke(ink);
  g.poly(ring(cx + light[0] * 0.02, cy + light[1] * 0.02, 0.25, rnd, 0.008, 20)).fill(t[1]);
  // Broad shoulders across it.
  const [sx, sy] = [at(0.18, 0.52), at(0.82, 0.52)];
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.2, color: INK, cap: 'round' });
  g.moveTo(...sx).lineTo(...sy).stroke({ width: 0.15, color: t[1], cap: 'round' });
  g.moveTo(sx[0] + light[0] * 0.02, sx[1] + light[1] * 0.03).lineTo(sy[0] + light[0] * 0.02, sy[1] + light[1] * 0.03).stroke({ width: 0.04, color: t[2], cap: 'round', alpha: 0.7 });
  // The big head.
  g.circle(cx, cy, 0.17).fill(t[1]).stroke(ink);
  g.circle(cx + light[0] * 0.05, cy + light[1] * 0.05, 0.085).fill(t[2]);
}

/** Pillar (a building's): a stone column from above in pale fine stone (`stoneColumn`). */
export function pillar(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  stoneColumn(g, f, light, MARBLE.map((c) => shade(c, 0.92)), rnd);
}

/** A stone column's three tones (shade, face, lit) from a site's stone (`UnderLook.stone`, 0-1). */
export function columnTones(stone: readonly number[]): number[] {
  const tone = (k: number) => stone.reduce((c, v) => (c << 8) | Math.min(255, Math.round(v * 255 * k)), 0);
  return [tone(0.8), tone(1.04), tone(1.2)];
}

/** A stone column seen from straight above, in the same family as the rock column and the statue:
 * its square plinth showing at the corners, a short shadow away from the light, the round capital
 * inked at OUT with a crescent of shade on the side away from the light, and the capital's flat top
 * inside its moulding ring, lit toward the light. `t`: shade, face, lit (`columnTones`). */
export function stoneColumn(g: Graphics, f: InteriorItem, light: Light, t: readonly number[], rnd: () => number = rng(f.w * 131 + f.h * 17 + 7)) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const [lx, ly] = light;
  const r = Math.min(f.w, f.h) * 0.38;
  const b = r * 1.16;
  const base = [cx - b, cy - b, cx + b, cy + b];
  castShadow(g, quad(base, rnd, 0), light, 0.32);
  board(g, base, t.map((c) => shade(c, 0.84)), light, rnd, true, 0.04, 0);
  // The capital: shade, its lit face shifted toward the light (a crescent of shade left away from it).
  const out = ring(cx, cy, r, rnd, 0.006, 32);
  g.poly(out).fill(t[0]);
  g.poly(ring(cx + lx * 0.055, cy + ly * 0.055, r * 0.86, rnd, 0.004, 32)).fill(t[1]);
  g.poly(out).stroke({ width: OUT, color: INK, join: 'round' });
  // Its flat top inside the moulding ring, lit on the side toward the light.
  const [tx, ty, tr] = [cx + lx * 0.015, cy + ly * 0.015, r * 0.6];
  g.circle(tx, ty, tr).fill(t[1]).stroke({ width: 0.018, color: INK, alpha: 0.75 });
  const la = Math.atan2(ly, lx);
  g.moveTo(tx + Math.cos(la - 1.0) * tr * 0.72, ty + Math.sin(la - 1.0) * tr * 0.72)
    .arc(tx, ty, tr * 0.72, la - 1.0, la + 1.0)
    .stroke({ width: 0.04, color: t[2], cap: 'round', alpha: 0.9 });
  g.moveTo(cx + Math.cos(la - 0.8) * r * 0.8, cy + Math.sin(la - 0.8) * r * 0.8)
    .arc(cx, cy, r * 0.8, la - 0.8, la + 0.8)
    .stroke({ width: 0.03, color: t[2], cap: 'round', alpha: 0.7 });
}

/** Throne against a wall: a tall back with a pointed gilt crest, carved arms, and a deep red cushion
 * filling the seat, tufted, with gilt finials at the front corners. */
export function throne(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { rect, at } = frame(f, back);
  // The seat frame, then the cushion filling it.
  board(g, rect(0.12, WALL_GAP + 0.06, 0.88, 0.9), DARK_WOOD, light, rnd, true, 0.03, 0);
  const [qx0, qy0, qx1, qy1] = rect(0.24, WALL_GAP + 0.16, 0.76, 0.86);
  g.roundRect(qx0, qy0, qx1 - qx0, qy1 - qy0, 0.07).fill(0x7a2420).stroke({ width: OUT, color: INK });
  g.roundRect(qx0 + 0.035 + light[0] * 0.015, qy0 + 0.035 + light[1] * 0.015, qx1 - qx0 - 0.07, qy1 - qy0 - 0.07, 0.06).fill(0xb04436);
  g.roundRect(qx0 + 0.07 + light[0] * 0.03, qy0 + 0.07 + light[1] * 0.03, (qx1 - qx0) * 0.4, (qy1 - qy0) * 0.3, 0.04).fill({ color: 0xd8705a, alpha: 0.6 });
  for (const [u, s0] of [
    [0.4, 0.55],
    [0.6, 0.55],
    [0.5, 0.7],
  ]) {
    g.circle(...at(u, s0), 0.022).fill(GILT[1]).stroke({ width: 0.01, color: INK });
  }
  // Arms, slim.
  for (const u of [0.12, 0.76]) board(g, rect(u, WALL_GAP + 0.06, u + 0.12, 0.9), DARK_WOOD, light, rnd, false, 0.025, 0);
  for (const u of [0.18, 0.82]) g.circle(...at(u, 0.9), 0.045).fill(GILT[1]).stroke({ width: LINE, color: INK });
  // The tall back and its pointed gilt crest.
  board(g, rect(0.06, WALL_GAP - 0.06, 0.94, WALL_GAP + 0.1), DARK_WOOD, light, rnd, true, 0.03, 0);
  g.poly([...at(0.34, WALL_GAP - 0.05), ...at(0.5, WALL_GAP + 0.12), ...at(0.66, WALL_GAP - 0.05)]).fill(GILT[1]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const u of [0.08, 0.92]) g.circle(...at(u, WALL_GAP + 0.02), 0.05).fill(GILT[1]).stroke({ width: LINE, color: INK });
}

/** Stage: a raised platform of planks — its front edge (the apron) darker, a shadow on the floor along the
 * front and sides — a flight of steps up the middle of the front, and a curtain along the back seen from above
 * as a thin gathered band (`hangings.ts`). */
export function stage(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const wid = along ? f.h : f.w;
  const front = wid - 0.36;
  // Shadow on the floor round the front and sides.
  g.poly(quad(rect(0.02, 0.1, len - 0.02, front + 0.1), rnd, 0)).fill({ color: 0x14100c, alpha: 0.28 });
  // Steps up the middle of the front: three treads, each lower one darker.
  const sw = len * 0.3;
  for (let k = 0; k < 3; k++) {
    const s0 = front - 0.04 + k * 0.11;
    board(g, rect(len / 2 - sw - k * 0.04, s0, len / 2 + sw + k * 0.04, s0 + 0.12), WOOD.map((c) => shade(c, 0.95 - k * 0.1)), light, rnd, true, 0.02, 0);
  }
  const face = board(g, rect(0.06, 0.06, len - 0.06, front), WOOD, light, rnd, along, 0.05, 2);
  planks(g, face, along, Math.round(wid * 4), WOOD[0], rnd);
  // The apron: the front edge, darker.
  g.poly(quad(rect(0.06, front - 0.07, len - 0.06, front), rnd, 0.003)).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  stageCurtain(g, at, light, len);
}

/** A ring of wooden staves bound with iron, seen from above (vats): the outer rim, the stave joints, a
 * hoop, and the inside's edge at `inner` of the radius; the contents are drawn after. */
function staveRing(g: Graphics, cx: number, cy: number, r: number, inner: number, light: Light, rnd: () => number, n = 28) {
  g.poly(ring(cx, cy, r, rnd, 0.008, 32)).fill(WOOD[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.circle(cx + light[0] * 0.01, cy + light[1] * 0.01, r * (1 + inner) / 2 + 0.01).fill(WOOD[1]);
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * r * inner, cy + Math.sin(a) * r * inner).lineTo(cx + Math.cos(a) * r * 0.98, cy + Math.sin(a) * r * 0.98);
  }
  g.stroke({ width: 0.012, color: WOOD[0] });
  g.circle(cx, cy, r * (0.5 + inner / 2)).stroke({ width: 0.03, color: IRON[0] });
  g.circle(cx, cy, r * inner).stroke({ width: LINE, color: INK });
}

/** Brewing vat: a great wooden tun, the mash inside under a head of froth, a wooden paddle across it. */
export function vat(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) / 2 - 0.08;
  staveRing(g, cx, cy, r, 0.84, light, rnd);
  g.circle(cx, cy, r * 0.84).fill(0x7a5a2a);
  g.circle(cx + light[0] * r * 0.1, cy + light[1] * r * 0.1, r * 0.6).fill(0x9a7434);
  for (let i = 0; i < 40; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.78 * Math.sqrt(rnd());
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.02 + 0.03 * rnd()).fill({ color: 0xf0e2b8, alpha: 0.85 });
  }
  const a = rnd() * Math.PI;
  const [px, py] = [Math.cos(a) * r * 1.05, Math.sin(a) * r * 1.05];
  g.moveTo(cx - px, cy - py).lineTo(cx + px, cy + py).stroke({ width: 0.08, color: INK, cap: 'round' });
  g.moveTo(cx - px, cy - py).lineTo(cx + px, cy + py).stroke({ width: 0.05, color: WOOD[2], cap: 'round' });
  g.poly(ring(cx + px * 0.7, cy + py * 0.7, 0.1, rnd, 0.01, 10)).fill(WOOD[2]).stroke({ width: LINE, color: INK });
}

/** Millstones: the runner stone in its wooden tun (the vat round the pair), grey, its furrows dressed in
 * harps round the eye; over the eye the hopper on its frame (the horse), grain in it, the shoe below. */
export function millstones(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) / 2 - 0.08;
  staveRing(g, cx, cy, r, 0.86, light, rnd);
  const rs = r * 0.84;
  g.circle(cx, cy, rs).fill(STONE[1]);
  g.circle(cx + light[0] * rs * 0.08, cy + light[1] * rs * 0.08, rs * 0.86).fill(STONE[2]);
  // The harps: furrows in sectors, each run off the radius at a slant.
  const harps = 8;
  for (let k = 0; k < harps; k++) {
    const a = (k / harps) * Math.PI * 2;
    for (let j = 0; j < 3; j++) {
      const t = 0.28 + j * 0.2;
      const [ax, ay] = [cx + Math.cos(a) * rs * 0.2, cy + Math.sin(a) * rs * 0.2];
      const b = a + 0.26 + j * 0.1;
      g.moveTo(ax + Math.cos(a + Math.PI / 2) * rs * t * 0.3, ay + Math.sin(a + Math.PI / 2) * rs * t * 0.3).lineTo(cx + Math.cos(b) * rs * 0.94, cy + Math.sin(b) * rs * 0.94);
    }
  }
  g.stroke({ width: 0.016, color: STONE[0], alpha: 0.8 });
  g.circle(cx, cy, rs).stroke({ width: LINE, color: INK });
  // The horse: two bars across the tun, the hopper on them.
  for (const sd of [-1, 1]) {
    const y = cy + sd * r * 0.3;
    g.poly(quad([cx - r * 0.98, y - 0.05, cx + r * 0.98, y + 0.05], rnd, 0.004)).fill(WOOD[1]).stroke({ width: LINE, color: INK });
  }
  const h = r * 0.42;
  g.poly([cx - h, cy - h, cx + h, cy - h, cx + h, cy + h, cx - h, cy + h]).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([cx - h, cy - h, cx + h, cy - h, cx + h * 0.4, cy - h * 0.4, cx - h * 0.4, cy - h * 0.4]).fill(WOOD[2]);
  g.poly([cx - h, cy - h, cx - h * 0.4, cy - h * 0.4, cx - h * 0.4, cy + h * 0.4, cx - h, cy + h]).fill({ color: WOOD[2], alpha: 0.7 });
  g.poly([cx - h * 0.4, cy - h * 0.4, cx + h * 0.4, cy - h * 0.4, cx + h * 0.4, cy + h * 0.4, cx - h * 0.4, cy + h * 0.4]).fill(0xc9a862).stroke({ width: 0.018, color: INK });
  g.circle(cx, cy, 0.05).fill(0x6e5a30);
}

/** The great spur wheel on the upright shaft, from above: its rim of wooden cogs, the two pairs of
 * clasp arms boxing the shaft, the shaft's square head, a stone nut meshed on its rim. */
export function spurWheel(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = Math.min(f.w, f.h) / 2 - 0.16;
  // The cogs, then the rim over their roots.
  const n = 28;
  for (let k = 0; k < n; k++) {
    const a = (k / n) * Math.PI * 2;
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [px, py] = [-s * 0.035, c * 0.035];
    g.poly([cx + c * r * 0.9 - px, cy + s * r * 0.9 - py, cx + c * (r + 0.11) - px, cy + s * (r + 0.11) - py, cx + c * (r + 0.11) + px, cy + s * (r + 0.11) + py, cx + c * r * 0.9 + px, cy + s * r * 0.9 + py]);
  }
  g.fill(WOOD[2]).stroke({ width: 0.016, color: INK });
  g.circle(cx, cy, r).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * 0.02, cy + light[1] * 0.02, r * 0.86).fill(WOOD[1]);
  g.circle(cx, cy, r * 0.72).fill(0x2a2018).stroke({ width: LINE, color: INK });
  // The clasp arms: two pairs of beams across, crossing round the shaft.
  const q = r * 0.16;
  for (const along of [true, false]) {
    for (const sd of [-1, 1]) {
      const rect = along ? [cx - r * 0.95, cy + sd * q - 0.055, cx + r * 0.95, cy + sd * q + 0.055] : [cx + sd * q - 0.055, cy - r * 0.95, cx + sd * q + 0.055, cy + r * 0.95];
      g.poly(quad(rect, rnd, 0.004)).fill(WOOD[1]).stroke({ width: LINE, color: INK });
      g.moveTo(...((along ? [rect[0], rect[1] + 0.025] : [rect[0] + 0.025, rect[1]]) as [number, number])).lineTo(...((along ? [rect[2], rect[1] + 0.025] : [rect[0] + 0.025, rect[3]]) as [number, number])).stroke({ width: 0.016, color: WOOD[2] });
    }
  }
  // The upright shaft's head, iron-banded.
  g.poly(quad([cx - q * 0.85, cy - q * 0.85, cx + q * 0.85, cy + q * 0.85], rnd, 0.003)).fill(WOOD[2]).stroke({ width: LINE, color: INK });
  g.circle(cx, cy, q * 0.5).stroke({ width: 0.02, color: IRON[1] });
  // A stone nut meshed on the rim (to the south-east, away from the light).
  const [nx, ny] = [cx - light[0] * (r + 0.2), cy - light[1] * (r + 0.2)];
  const nr = 0.17;
  const teeth: number[] = [];
  for (let k = 0; k < 20; k++) {
    const a = (k / 20) * Math.PI * 2;
    const rr = k % 2 ? nr : nr + 0.05;
    teeth.push(nx + Math.cos(a) * rr, ny + Math.sin(a) * rr);
  }
  g.poly(teeth).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(nx, ny, 0.05).fill(IRON[1]).stroke({ width: 0.016, color: INK });
}

/** A lighthouse's lamp from above (v66b): the lamp room's centrepiece, drawn past its one square to fill the
 * middle of the round 3x3 room (a walkable ring left inside the glazing), in the lantern's look on the map
 * (`harbourShapes.drawLantern`): a brass pedestal ring, a great lens of eight bullseye panels in iron frames,
 * pale glass glowing yellow at its heart, warm rays out between the frames. */
export function lighthouseLamp(g: Graphics, f: InteriorItem, light: Light, _rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = 1.05 * Math.max(1, Math.min(f.w, f.h));
  const SILL = 0xc89a3c;
  // (Its glow and rays on the floor round it, between the lens's frames.)
  g.circle(cx, cy, r * 1.08).fill({ color: 0xffe29a, alpha: 0.3 });
  for (let k = 0; k < 8; k++) {
    const a = ((k + 0.5) / 8) * Math.PI * 2;
    const [ux, uy] = [Math.cos(a), Math.sin(a)];
    g.poly([cx + ux * r * 0.8 - uy * r * 0.13, cy + uy * r * 0.8 + ux * r * 0.13, cx + ux * r * 1.22, cy + uy * r * 1.22, cx + ux * r * 0.8 + uy * r * 0.13, cy + uy * r * 0.8 - ux * r * 0.13]).fill({ color: 0xffd65a, alpha: 0.8 });
  }
  // The pedestal: a broad brass ring, its shadow away from the light, lit on the light's side.
  g.circle(cx - light[0] * r * 0.06, cy - light[1] * r * 0.06, r * 0.86).fill({ color: 0x000000, alpha: 0.2 });
  g.circle(cx, cy, r * 0.84).fill(shade(SILL, 0.8)).stroke({ width: OUT, color: INK });
  g.circle(cx + light[0] * r * 0.03, cy + light[1] * r * 0.03, r * 0.78).fill(SILL);
  for (let k = 0; k < 8; k++) {
    const a = (k / 8) * Math.PI * 2;
    g.circle(cx + Math.cos(a) * r * 0.79, cy + Math.sin(a) * r * 0.79, r * 0.045).fill(shade(SILL, 0.65)).stroke({ width: LINE * 0.7, color: INK });
  }
  // The lens: eight bullseye panels of pale glass, each with its rings round a warm eye, in iron frames.
  const L = r * 0.7;
  g.circle(cx, cy, L).fill(0xfff0c0).stroke({ width: LINE, color: INK });
  for (let k = 0; k < 8; k++) {
    const a = ((k + 0.5) / 8) * Math.PI * 2;
    const [ex, ey] = [cx + Math.cos(a) * L * 0.62, cy + Math.sin(a) * L * 0.62];
    for (const q of [0.28, 0.18]) g.circle(ex, ey, L * q).stroke({ width: 0.016, color: 0xd9a838, alpha: 0.85 });
    g.circle(ex, ey, L * 0.09).fill({ color: 0xffc21f, alpha: 0.9 });
  }
  for (let k = 0; k < 8; k++) {
    const a = (k / 8) * Math.PI * 2;
    g.moveTo(cx + Math.cos(a) * L * 0.3, cy + Math.sin(a) * L * 0.3).lineTo(cx + Math.cos(a) * L, cy + Math.sin(a) * L);
  }
  g.stroke({ width: 0.035, color: IRON[0], cap: 'round' });
  // (The burner at its heart glowing through, a glint on the glass toward the light.)
  g.circle(cx, cy, L * 0.3).fill(0xffe29a).stroke({ width: 0.03, color: IRON[0] });
  g.circle(cx, cy, L * 0.2).fill(0xffc21f);
  g.circle(cx, cy, L * 0.1).fill(0xfffbe6);
  const [gx, gy] = [cx + light[0] * L * 0.8, cy + light[1] * L * 0.8];
  g.moveTo(gx - light[1] * L * 0.18, gy + light[0] * L * 0.18).lineTo(gx + light[1] * L * 0.18, gy - light[0] * L * 0.18).stroke({ width: 0.04, color: 0xffffff, alpha: 0.85, cap: 'round' });
}

/** Cauldron: a round black iron pot on three short legs, the embers of a fire glowing round its foot, a
 * bubbling brew inside. */
export function cauldron(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const [cx, cy] = [f.x + 0.5, f.y + 0.5];
  const r = 0.34;
  g.circle(cx, cy, r + 0.1).fill({ color: 0xf59e0b, alpha: 0.12 });
  for (let i = 0; i < 14; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r + 0.02 + 0.06 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, 0.02 + 0.015 * rnd()).fill([0xc2410c, 0xf97316, 0xffc23a][Math.floor(rnd() * 3)]);
  }
  for (const a of [0.5, 2.6, 4.7]) g.circle(cx + Math.cos(a) * (r + 0.04), cy + Math.sin(a) * (r + 0.04), 0.045).fill(IRON[0]).stroke({ width: LINE, color: INK });
  g.circle(cx, cy, r).fill(0x24221e).stroke({ width: OUT, color: INK });
  g.circle(cx, cy, r * 0.86).stroke({ width: 0.035, color: 0x4a4844 });
  const la = Math.atan2(light[1], light[0]);
  g.moveTo(cx + Math.cos(la - 0.9) * r * 0.93, cy + Math.sin(la - 0.9) * r * 0.93)
    .arc(cx, cy, r * 0.93, la - 0.9, la + 0.9)
    .stroke({ width: 0.02, color: 0x8a8884 });
  const brew = [
    [0x3a6a2a, 0x6aa84a, 0xb0e080],
    [0x5a2a6a, 0x8a4aa8, 0xd0a0f0],
    [0x7a3a1a, 0xb0602a, 0xf0b070],
    [0x2a5a6a, 0x4a8aa8, 0x9ad0e0],
  ][v % 4];
  g.circle(cx, cy, r * 0.76).fill(brew[0]);
  g.circle(cx + light[0] * r * 0.1, cy + light[1] * r * 0.1, r * 0.56).fill(brew[1]);
  for (let i = 0; i < 6; i++) {
    const a = rnd() * Math.PI * 2;
    const d = r * 0.55 * Math.sqrt(rnd());
    const br = 0.025 + 0.025 * rnd();
    g.circle(cx + Math.cos(a) * d, cy + Math.sin(a) * d, br).fill(brew[1]).stroke({ width: 0.012, color: brew[2] });
  }
}

/** Alchemy bench against a wall: a work top with glassware — round flasks of coloured liquid, a retort,
 * a little burner under a flask, a mortar and pestle, a scatter of herbs — and jars along the wall. */
export function alchemyBench(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, 0.92), DARK_WOOD, light, rnd, along, 0.05, 2);
  const liquids = [0x6aa84a, 0x8a4aa8, 0xc84a3a, 0x4a8aa8, 0xd8b040];
  const flask = (u: number, s0: number, r: number, c: number) => {
    const [x, y] = at(u, s0);
    g.circle(x, y, r).fill({ color: 0xd8e8e8, alpha: 0.5 }).stroke({ width: LINE, color: INK });
    g.circle(x, y, r * 0.75).fill(c);
    g.circle(x - r * 0.3, y - r * 0.3, r * 0.25).fill({ color: 0xffffff, alpha: 0.7 });
  };
  // Jars along the wall.
  for (let u = END_GAP + 0.14, k = v; u < len - END_GAP - 0.08; u += 0.17, k++) {
    const [x, y] = at(u, WALL_GAP + 0.1);
    g.circle(x, y, 0.06).fill(0x8a7a62).stroke({ width: 0.012, color: INK });
    g.circle(x, y, 0.03).fill(liquids[(k * 3) % liquids.length]);
  }
  // A retort: a round belly with a long neck reaching out.
  const [rx, ry] = at(len * 0.3, 0.56);
  const [nx, ny] = at(len * 0.3 + 0.36, 0.74);
  g.moveTo(rx, ry).lineTo(nx, ny).stroke({ width: 0.05, color: INK, cap: 'round' });
  g.moveTo(rx, ry).lineTo(nx, ny).stroke({ width: 0.03, color: 0xc8dcdc, cap: 'round' });
  flask(len * 0.3, 0.56, 0.13, liquids[v % liquids.length]);
  // A flask on a little burner, its flame glowing.
  const [bx, by] = at(len * 0.62, 0.5);
  g.circle(bx, by, 0.11).fill({ color: 0xf59e0b, alpha: 0.2 });
  for (const a of [0, 2.1, 4.2]) g.moveTo(bx, by).lineTo(bx + Math.cos(a) * 0.12, by + Math.sin(a) * 0.12);
  g.stroke({ width: 0.02, color: IRON[1] });
  flask(len * 0.62, 0.5, 0.09, liquids[(v + 2) % liquids.length]);
  // Mortar and pestle, a scatter of herbs.
  const [mx, my] = at(len - END_GAP - 0.24, 0.72);
  g.circle(mx, my, 0.09).fill(STONE[1]).stroke({ width: LINE, color: INK });
  g.circle(mx, my, 0.055).fill(STONE[0]);
  g.moveTo(mx, my).lineTo(mx + 0.1, my - 0.08).stroke({ width: 0.03, color: STONE[2], cap: 'round' });
  for (let i = 0; i < 6; i++) g.poly(oval(...at(len * 0.45 + rnd() * 0.3, 0.74 + rnd() * 0.12), 0.03, 0.012, rnd() * Math.PI, 6)).fill(0x5a8a3a);
}

/** A weapon lying on the rack's pegs along `u` from u0 to u1, its point toward u1: a sword, a spear, an axe
 * or a mace. */
function weapon(g: Graphics, at: (u: number, s: number) => [number, number], u0: number, u1: number, s: number, kind: number) {
  const steel = 0xc8ccd0;
  const shaft = (a: number, b: number, w: number, c: number) => {
    g.moveTo(...at(a, s)).lineTo(...at(b, s)).stroke({ width: w + 0.025, color: INK, cap: 'round' });
    g.moveTo(...at(a, s)).lineTo(...at(b, s)).stroke({ width: w, color: c, cap: 'round' });
  };
  if (kind === 0) {
    // Sword: grip, crossguard, blade.
    shaft(u0, u0 + 0.14, 0.03, 0x5a3a22);
    g.moveTo(...at(u0 + 0.14, s - 0.07)).lineTo(...at(u0 + 0.14, s + 0.07)).stroke({ width: 0.035, color: INK, cap: 'round' });
    g.moveTo(...at(u0 + 0.14, s - 0.07)).lineTo(...at(u0 + 0.14, s + 0.07)).stroke({ width: 0.018, color: BRASS, cap: 'round' });
    g.poly([...at(u0 + 0.15, s - 0.025), ...at(u1 - 0.06, s - 0.02), ...at(u1, s), ...at(u1 - 0.06, s + 0.02), ...at(u0 + 0.15, s + 0.025)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else if (kind === 1) {
    // Spear: a long haft, a leaf-shaped head.
    shaft(u0, u1 - 0.12, 0.025, WOOD[2]);
    g.poly([...at(u1 - 0.14, s), ...at(u1 - 0.08, s - 0.04), ...at(u1, s), ...at(u1 - 0.08, s + 0.04)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else if (kind === 2) {
    // Axe: a haft, a crescent blade on one side.
    shaft(u0, u1, 0.03, WOOD[2]);
    g.poly([...at(u1 - 0.18, s), ...at(u1 - 0.22, s - 0.14), ...at(u1 - 0.06, s - 0.14), ...at(u1 - 0.04, s)]).fill(steel).stroke({ width: 0.012, color: INK });
  } else {
    // Mace: a haft, a flanged iron head.
    shaft(u0, u1 - 0.08, 0.03, WOOD[1]);
    const [hx, hy] = at(u1 - 0.06, s);
    g.circle(hx, hy, 0.06).fill(IRON[1]).stroke({ width: 0.012, color: INK });
    for (let i = 0; i < 6; i++) {
      const a = (i / 6) * Math.PI * 2;
      g.moveTo(hx, hy).lineTo(hx + Math.cos(a) * 0.08, hy + Math.sin(a) * 0.08);
    }
    g.stroke({ width: 0.02, color: IRON[0] });
  }
}

/** Weapon rack against a wall: a frame of two rails with pegs, weapons laid along it — swords, spears,
 * axes, a mace — and a round shield hung at one end. */
export function weaponRack(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  board(g, rect(END_GAP, WALL_GAP, len - END_GAP, WALL_GAP + 0.08), DARK_WOOD, light, rnd, along, 0.02, 0);
  for (const u of [END_GAP + 0.04, len - END_GAP - 0.1]) board(g, rect(u, WALL_GAP, u + 0.06, 0.88), DARK_WOOD, light, rnd, !along, 0.015, 0);
  // Weapons at their own lengths (a sword about 3.5 ft, a spear 6), as many as fit along each row.
  const lengths = [0.72, 1.2, 0.6, 0.55];
  const rows = [0.4, 0.55, 0.7, 0.84];
  let k = v;
  for (const s0 of rows) {
    let u = END_GAP + 0.06;
    const end = len - END_GAP - 0.04;
    while (true) {
      let kind = k % 4;
      if (u + lengths[kind] > end) kind = [2, 3, 0].find((c) => u + lengths[c] <= end) ?? -1;
      if (kind < 0) break;
      weapon(g, at, u, u + lengths[kind], s0, kind);
      u += lengths[kind] + 0.12;
      k++;
    }
    k += 3;
  }
  // A shield hung at one end, its boss and rim.
  const [sx, sy] = at(len - END_GAP - 0.22, WALL_GAP + 0.14);
  const dye = DYES[(v + 1) % DYES.length];
  g.circle(sx, sy, 0.13).fill(dye).stroke({ width: OUT, color: INK });
  g.circle(sx, sy, 0.11).stroke({ width: 0.02, color: IRON[1] });
  g.circle(sx, sy, 0.04).fill(IRON[2]).stroke({ width: 0.012, color: INK });
}

/** The rack (torture): a heavy wooden frame, a roller across each end with ropes to the bed, a big wheel
 * at the head to turn it, the bed's boards stained. */
export function tortureRack(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  for (const s of [0.14, 0.78]) board(g, rect(0.06, s, len - 0.06, s + 0.08), DARK_WOOD, light, rnd, along, 0.02, 0);
  const bed = board(g, rect(0.24, 0.24, len - 0.24, 0.76), WOOD, light, rnd, along, 0.03, 1);
  planks(g, bed, along, 3, WOOD[0], rnd);
  g.poly(ring(...at(len * 0.55, 0.5), 0.1, rnd, 0.05, 10)).fill({ color: 0x5a1a14, alpha: 0.5 });
  for (const u of [0.12, len - 0.12]) {
    g.moveTo(...at(u, 0.12)).lineTo(...at(u, 0.88)).stroke({ width: 0.12, color: INK, cap: 'round' });
    g.moveTo(...at(u, 0.12)).lineTo(...at(u, 0.88)).stroke({ width: 0.08, color: WOOD[1], cap: 'round' });
    for (const s of [0.36, 0.64]) g.moveTo(...at(u, s)).lineTo(...at(u < 1 ? 0.34 : len - 0.34, s));
    g.stroke({ width: 0.022, color: 0xc8b080 });
  }
  // The wheel at the head.
  const [wx, wy] = at(0.12, 0.94);
  g.circle(wx, wy, 0.1).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  for (let i = 0; i < 4; i++) {
    const a = (i / 4) * Math.PI * 2;
    g.moveTo(wx, wy).lineTo(wx + Math.cos(a) * 0.14, wy + Math.sin(a) * 0.14);
  }
  g.stroke({ width: 0.025, color: WOOD[2] });
}

/** Iron cage: dark straw on its floor between thick bars running one way across its top, cross-straps, a
 * heavy frame with corner posts, and a door in its right-hand side, hinged, a padlock hanging outside. */
export function cage(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y, w, h } = f;
  const [x0, y0, x1, y1] = [x + 0.08, y + 0.08, x + w - 0.08, y + h - 0.08];
  g.rect(x0, y0, x1 - x0, y1 - y0).fill(0x2e261c);
  for (let i = 0; i < 14 * w * h; i++) {
    const [px, py] = [x0 + rnd() * (x1 - x0), y0 + rnd() * (y1 - y0)];
    const a = rnd() * Math.PI;
    g.moveTo(px, py).lineTo(px + Math.cos(a) * 0.1, py + Math.sin(a) * 0.1);
  }
  g.stroke({ width: 0.014, color: 0x8a7440, alpha: 0.8 });
  const bar = (ax: number, ay: number, bx: number, by: number, wdt: number) => {
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: wdt + 0.025, color: INK });
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: wdt, color: IRON[1] });
    g.moveTo(ax - 0.01, ay).lineTo(bx - 0.01, by).stroke({ width: wdt * 0.3, color: IRON[2] });
  };
  // Thick bars one way, about a fifth of a square apart, then the cross-straps.
  const n = Math.max(3, Math.round((x1 - x0) / 0.19));
  for (let i = 1; i < n; i++) bar(x0 + ((x1 - x0) * i) / n, y0, x0 + ((x1 - x0) * i) / n, y1, 0.05);
  const straps = h >= 2 ? 3 : 2;
  for (let j = 1; j <= straps; j++) bar(x0, y0 + ((y1 - y0) * j) / (straps + 1), x1, y0 + ((y1 - y0) * j) / (straps + 1), 0.04);
  g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 0.09, color: INK });
  g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 0.055, color: IRON[1] });
  for (const [cx, cy] of [
    [x0, y0],
    [x1, y0],
    [x0, y1],
    [x1, y1],
  ]) {
    g.rect(cx - 0.05, cy - 0.05, 0.1, 0.1).fill(IRON[0]).stroke({ width: LINE, color: INK });
  }
  // The door, in the right-hand side: a heavier stretch of the frame there, hinge knuckles at its ends,
  // and a big padlock hanging outside the bars at its middle.
  const [dy0, dy1] = [y0 + (y1 - y0) * 0.3, y0 + (y1 - y0) * 0.7];
  g.moveTo(x1, dy0).lineTo(x1, dy1).stroke({ width: 0.13, color: INK });
  g.moveTo(x1, dy0).lineTo(x1, dy1).stroke({ width: 0.08, color: IRON[2] });
  for (const hy of [dy0, dy1]) g.rect(x1 - 0.07, hy - 0.035, 0.14, 0.07).fill(IRON[0]).stroke({ width: 0.012, color: INK });
  const ly = (dy0 + dy1) / 2;
  g.moveTo(x1 + 0.04, ly - 0.05).arc(x1 + 0.08, ly - 0.05, 0.04, Math.PI, Math.PI * 2).stroke({ width: 0.022, color: IRON[2] });
  g.rect(x1 + 0.02, ly - 0.05, 0.12, 0.11).fill(BRASS).stroke({ width: LINE, color: INK });
  g.circle(x1 + 0.08, ly + 0.005, 0.015).fill(INK);
}

/** Portcullis winch: a drum on an axle between two posts, shaded as a cylinder, its chain wound round it in
 * rows of links and running off into a dark slot in the floor; a big iron crank at each end, a toothed
 * ratchet with its pawl. */
export function winch(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  const [cy, dy0, dy1] = [y + 0.42, y + 0.24, y + 0.6];
  const link = (lx: number, ly: number, rx: number, ry: number) => {
    g.poly(oval(lx, ly, rx, ry, 0, 10)).stroke({ width: 0.028, color: INK });
    g.poly(oval(lx, ly, rx, ry, 0, 10)).stroke({ width: 0.014, color: IRON[2] });
  };
  // The slot in the floor, and the slack chain running into it.
  g.poly(quad([x + 0.38, y + 0.84, x + 0.62, y + 0.94], rnd, 0.004)).fill(0x0e0c0a).stroke({ width: LINE, color: INK });
  for (let k = 0; k < 4; k++) link(x + 0.5, dy1 + 0.04 + k * 0.06, 0.03, 0.04);
  // Posts.
  for (const px of [x + 0.1, x + 0.76]) board(g, [px, y + 0.14, px + 0.14, y + 0.7], DARK_WOOD, light, rnd, false, 0.02, 0);
  // The drum, a cylinder along x: dark at its edges, a light band down the middle.
  g.rect(x + 0.24, dy0, 0.52, dy1 - dy0).fill(WOOD[0]).stroke({ width: OUT, color: INK });
  g.rect(x + 0.24, dy0 + 0.06, 0.52, dy1 - dy0 - 0.12).fill(WOOD[1]);
  g.rect(x + 0.24, cy - 0.04 + light[1] * 0.02, 0.52, 0.06).fill({ color: WOOD[2], alpha: 0.9 });
  // Chain wound round it: rows of links across the drum.
  for (let k = 0; k < 6; k++) for (let j = 0; j < 3; j++) link(x + 0.29 + k * 0.085, dy0 + 0.07 + j * 0.11, 0.03, 0.05);
  // Big cranks: an L-handle out past each post.
  for (const [ax, dir] of [
    [x + 0.06, -1],
    [x + 0.94, 1],
  ]) {
    g.moveTo(ax, cy).lineTo(ax, cy + 0.3).stroke({ width: 0.07, color: INK, cap: 'round' });
    g.moveTo(ax, cy).lineTo(ax, cy + 0.3).stroke({ width: 0.04, color: IRON[1], cap: 'round' });
    g.moveTo(ax, cy + 0.3).lineTo(ax + dir * 0.06, cy + 0.3).stroke({ width: 0.07, color: INK, cap: 'round' });
    g.moveTo(ax, cy + 0.3).lineTo(ax + dir * 0.06, cy + 0.3).stroke({ width: 0.045, color: WOOD[1], cap: 'round' });
    g.circle(ax, cy, 0.05).fill(IRON[1]).stroke({ width: LINE, color: INK });
  }
  // The ratchet: a toothed wheel on the axle by the right post, its pawl hooked in.
  const [rx, ry] = [x + 0.83, y + 0.08];
  const teeth: number[] = [];
  for (let i = 0; i < 16; i++) {
    const a = (i / 16) * Math.PI * 2;
    const r = i % 2 ? 0.07 : 0.095;
    teeth.push(rx + Math.cos(a) * r, ry + Math.sin(a) * r);
  }
  g.poly(teeth).fill(IRON[1]).stroke({ width: LINE, color: INK, join: 'round' });
  g.circle(rx, ry, 0.03).fill(IRON[0]);
  g.moveTo(x + 0.66, y + 0.06).lineTo(rx - 0.08, ry + 0.01).stroke({ width: 0.035, color: INK, cap: 'round' });
  g.moveTo(x + 0.66, y + 0.06).lineTo(rx - 0.08, ry + 0.01).stroke({ width: 0.018, color: IRON[2], cap: 'round' });
}

/** Telescope: a long brass tube on a wooden tripod, pointed up and away across the room — its lens end
 * wide, the eyepiece narrow — a stool by the eyepiece. */
export function telescope(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const s = Math.min(f.w, f.h);
  for (const a of [0.6, 2.7, 4.8]) {
    g.moveTo(cx, cy).lineTo(cx + Math.cos(a) * s * 0.36, cy + Math.sin(a) * s * 0.36).stroke({ width: 0.06, color: INK, cap: 'round' });
    g.moveTo(cx, cy).lineTo(cx + Math.cos(a) * s * 0.36, cy + Math.sin(a) * s * 0.36).stroke({ width: 0.035, color: WOOD[1], cap: 'round' });
  }
  const a = -Math.PI / 4 + (rnd() - 0.5) * 0.6;
  const [ux, uy] = [Math.cos(a), Math.sin(a)];
  const [wx, wy] = [-uy, ux];
  const L = s * 0.42;
  const tube = [cx - ux * L * 0.6 - wx * 0.05, cy - uy * L * 0.6 - wy * 0.05, cx + ux * L - wx * 0.1, cy + uy * L - wy * 0.1, cx + ux * L + wx * 0.1, cy + uy * L + wy * 0.1, cx - ux * L * 0.6 + wx * 0.05, cy - uy * L * 0.6 + wy * 0.05];
  g.poly(tube).fill(BRONZE[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.moveTo(cx - ux * L * 0.55 - wx * 0.02 * Math.sign(light[0] + light[1] || 1), cy - uy * L * 0.55).lineTo(cx + ux * L * 0.95, cy + uy * L * 0.95).stroke({ width: 0.025, color: BRONZE[2] });
  for (const k of [0.1, 0.55]) g.moveTo(cx + ux * L * k - wx * 0.08, cy + uy * L * k - wy * 0.08).lineTo(cx + ux * L * k + wx * 0.08, cy + uy * L * k + wy * 0.08);
  g.stroke({ width: 0.03, color: BRONZE[0] });
  g.poly(oval(cx + ux * L, cy + uy * L, 0.04, 0.11, a, 12)).fill(0x6a9ab0).stroke({ width: LINE, color: INK });
  g.circle(cx - ux * L * 0.62, cy - uy * L * 0.62, 0.035).fill(BRONZE[0]).stroke({ width: 0.012, color: INK });
  // A stool by the eyepiece.
  const [tx, ty] = [cx - ux * L * 1.15, cy - uy * L * 1.15];
  g.poly(ring(tx, ty, 0.13, rnd, 0.01, 12)).fill(WOOD[0]).stroke({ width: LINE, color: INK });
  g.circle(tx + light[0] * 0.015, ty + light[1] * 0.015, 0.1).fill(WOOD[1]);
}

/** Barricade (cheval de frise): a thick log along its length, sharpened stakes through it sticking straight
 * out on both sides in turn, each a tapering wedge with a dark iron point, lashed where it meets the log. */
export function barricade(g: Graphics, f: InteriorItem) {
  const { len, at } = frame(f);
  const n = Math.max(3, Math.round(len * 3));
  const us = Array.from({ length: n }, (_, i) => 0.22 + ((len - 0.44) * i) / (n - 1));
  us.forEach((u, i) => {
    const side = i % 2 ? 1 : -1;
    const tip = 0.5 + side * 0.44;
    // The wedge: wide at the log, narrowing to the point.
    g.poly([...at(u - 0.05, 0.5), ...at(u - 0.02, tip - side * 0.1), ...at(u + 0.02, tip - side * 0.1), ...at(u + 0.05, 0.5)]).fill(WOOD[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.moveTo(...at(u - 0.02, 0.5)).lineTo(...at(u - 0.008, tip - side * 0.12)).stroke({ width: 0.014, color: WOOD[2] });
    g.poly([...at(u - 0.025, tip - side * 0.11), ...at(u, tip), ...at(u + 0.025, tip - side * 0.11)]).fill(IRON[0]).stroke({ width: 0.012, color: INK, join: 'round' });
    g.circle(...at(u - 0.006, tip - side * 0.07), 0.01).fill(IRON[2]);
  });
  // The log, thick, its round end grain at both ends.
  g.moveTo(...at(0.12, 0.5)).lineTo(...at(len - 0.12, 0.5)).stroke({ width: 0.24, color: INK, cap: 'round' });
  g.moveTo(...at(0.12, 0.5)).lineTo(...at(len - 0.12, 0.5)).stroke({ width: 0.19, color: WOOD[0], cap: 'round' });
  g.moveTo(...at(0.16, 0.46)).lineTo(...at(len - 0.16, 0.46)).stroke({ width: 0.06, color: WOOD[1], cap: 'round' });
  for (const u of [0.1, len - 0.1]) {
    g.circle(...at(u, 0.5), 0.1).fill(0xb08a58).stroke({ width: LINE, color: INK });
    g.circle(...at(u, 0.5), 0.05).stroke({ width: 0.01, color: WOOD[0] });
  }
  // Lashings where the stakes pass through.
  for (const u of us) g.moveTo(...at(u - 0.06, 0.4)).lineTo(...at(u + 0.06, 0.6)).moveTo(...at(u + 0.06, 0.4)).lineTo(...at(u - 0.06, 0.6));
  g.stroke({ width: 0.022, color: 0xc8b080 });
}

/** Trapdoor: a hatch of planks set into the floor — a dark gap round its edge, iron strap hinges along one
 * side, an iron ring to lift it by. */
export function trapdoor(g: Graphics, f: InteriorItem, light: Light, rnd: () => number) {
  const { x, y } = f;
  g.poly(quad([x + 0.1, y + 0.1, x + 0.9, y + 0.9], rnd, 0.004)).fill(0x14100c);
  const face = board(g, [x + 0.14, y + 0.14, x + 0.86, y + 0.86], WOOD, light, rnd, false, 0.04, 1);
  planks(g, face, false, 4, WOOD[0], rnd);
  // Strap hinges along the top edge, nails in them.
  for (const hx of [x + 0.28, x + 0.64]) {
    g.poly(quad([hx, y + 0.13, hx + 0.07, y + 0.3], rnd, 0.002)).fill(IRON[1]).stroke({ width: 0.012, color: INK });
    g.circle(hx + 0.035, y + 0.24, 0.01).fill(IRON[0]);
  }
  // The ring.
  g.circle(x + 0.5, y + 0.74, 0.07).stroke({ width: 0.04, color: INK });
  g.circle(x + 0.5, y + 0.74, 0.07).stroke({ width: 0.022, color: IRON[2] });
  g.rect(x + 0.46, y + 0.64, 0.08, 0.05).fill(IRON[0]).stroke({ width: 0.012, color: INK });
}

/** A spiral stair seen from above: wedge treads fanning clockwise round a newel post, the top one
 * palest and each lower one darker, every tread a flat tone with a lit nosing and an ink line where
 * it drops to the next; the dark well showing between the lowest and the top, a handrail round the
 * outer edge. Sized to its footprint (one square, or a lighthouse's two by two). */
export function spiralStair(g: Graphics, f: InteriorItem, rnd: () => number) {
  const s = Math.min(f.w, f.h);
  const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
  const r = s * 0.47;
  const n = 12;
  const a0 = -Math.PI / 2 + (rnd() - 0.5) * 0.3;
  const step = (Math.PI * 2 * 0.9) / n;
  const [rin, rout] = [r * 0.2, r * 0.86];
  const at = (a: number, rr: number) => [cx + Math.cos(a) * rr, cy + Math.sin(a) * rr];
  const arc = (from: number, to: number, rr: number, k = 6) => Array.from({ length: k + 1 }, (_, i) => at(from + ((to - from) * i) / k, rr)).flat();
  // The well: dark, seen down past the lowest tread.
  g.circle(cx, cy, r).fill(0x14110e);
  g.poly([...arc(a0 - step * 0.9, a0, rout, 3), ...arc(a0, a0 - step * 0.9, rin, 2)]).fill(0x2a241e);
  // The treads, the lowest first.
  const top = 0xd6cdb9;
  for (let i = n - 1; i >= 0; i--) {
    const t0 = a0 + i * step;
    const t1 = t0 + step;
    const tone = shade(top, 1 - (0.62 * i) / (n - 1));
    g.poly([...arc(t0, t1, rout), ...arc(t1, t0, rin, 2)]).fill(tone);
    // (Its nosing lit along the edge where it drops to the next, the ink line on that edge.)
    const [lx0, ly0] = at(t1 - step * 0.12, rin * 1.1);
    const [lx1, ly1] = at(t1 - step * 0.1, rout * 0.97);
    g.moveTo(lx0, ly0).lineTo(lx1, ly1).stroke({ width: s * 0.03, color: shade(tone, 1.25), cap: 'round' });
    g.poly([...arc(t0, t1, rout), ...arc(t1, t0, rin, 2)]).stroke({ width: s * 0.016, color: INK, alpha: 0.75, join: 'round' });
    const [ex, ey] = at(t1, rin);
    const [fx, fy] = at(t1, rout);
    g.moveTo(ex, ey).lineTo(fx, fy).stroke({ width: s * 0.026, color: INK, cap: 'round' });
  }
  // The handrail round the outer edge, from the top tread down to the lowest, on its posts.
  const end = a0 + n * step;
  const rail = arc(a0, end, (rout + r) / 2, 40);
  g.poly(rail, false).stroke({ width: s * 0.075, color: INK, cap: 'round', join: 'round' });
  g.poly(rail, false).stroke({ width: s * 0.04, color: WOOD[1], cap: 'round', join: 'round' });
  g.poly(arc(a0, end, (rout + r) / 2 - s * 0.008, 40), false).stroke({ width: s * 0.012, color: WOOD[2], cap: 'round', join: 'round' });
  for (let i = 0; i <= n; i += 2) {
    const [px, py] = at(a0 + i * step, (rout + r) / 2);
    g.circle(px, py, s * 0.03).fill(DARK_WOOD[1]).stroke({ width: s * 0.012, color: INK });
  }
  // The outer edge of the well, inked.
  g.circle(cx, cy, r).stroke({ width: OUT * s, color: INK });
  // The newel post.
  g.circle(cx, cy, rin).fill(STONE[1]).stroke({ width: s * 0.03, color: INK });
  g.circle(cx - rin * 0.25, cy - rin * 0.25, rin * 0.45).fill(STONE[2]);
}

/** Stairs down to the deep dungeons: an opening in the floor framed in stone, steps going down into it,
 * each darker than the last until the dark swallows them, a pale arrow showing the way down. */
export function linkDown(g: Graphics, f: InteriorItem, rnd: () => number) {
  const { x, y } = f;
  masonry(g, [x + 0.04, y + 0.04, x + 0.96, y + 0.96], STONE, rnd, 0.16);
  g.poly(quad([x + 0.16, y + 0.16, x + 0.84, y + 0.84], rnd, 0.004)).fill(0x0c0a08).stroke({ width: LINE, color: INK });
  const n = 5;
  for (let k = 0; k < n; k++) {
    const y0 = y + 0.16 + k * 0.12;
    g.rect(x + 0.18, y0, 0.64, 0.1).fill(shade(STONE[2], 1 - k * 0.19));
    g.moveTo(x + 0.18, y0).lineTo(x + 0.82, y0).stroke({ width: 0.014, color: shade(STONE[2], 1.15 - k * 0.19) });
  }
  g.moveTo(x + 0.5, y + 0.26).lineTo(x + 0.5, y + 0.66).moveTo(x + 0.38, y + 0.54).lineTo(x + 0.5, y + 0.68).lineTo(x + 0.62, y + 0.54);
  g.stroke({ width: 0.06, color: 0xf2e6c8, cap: 'round', join: 'round' });
}

/** Stone effigy: a knight carved in the round lying on a tall tomb chest — the figure stands up off the slab and
 * throws its own shadow on it: head in a mail coif on a carved pillow, sloping shoulders, hands together on the
 * hilt of a sword laid down the body, a belt, pointed feet resting on a dog; moss and a crack. */
export function stoneEffigy(g: Graphics, f: InteriorItem, light: Light, rnd: () => number, sword = true) {
  const { len, rect, at } = frame(f);
  const ST = [0x4e4a44, 0x7e796e, 0xaaa496, 0xd2ccbe];
  const k = len / 2;
  const U = (u: number) => u * k;
  // The chest: its shaded sides, then the top face set toward the light, a moulding round it.
  const outer = rect(0.06, 0.08, len - 0.06, 0.92);
  castShadow(g, quad(outer, rnd, 0), light, 0.6);
  g.poly(quad(outer, rnd, 0.006)).fill(ST[0]).stroke({ width: OUT, color: INK, join: 'round' });
  const [x0, y0, x1, y1] = outer;
  const top = [x0 + 0.03 + light[0] * 0.03, y0 + 0.03 + light[1] * 0.03, x1 - 0.03 + light[0] * 0.03, y1 - 0.03 + light[1] * 0.03];
  g.poly(quad(top, rnd, 0.004)).fill(ST[1]).stroke({ width: 0.018, color: INK });
  g.poly(quad([top[0] + 0.04, top[1] + 0.04, top[2] - 0.04, top[3] - 0.04], rnd, 0.003)).stroke({ width: 0.014, color: ST[0] });
  // The pillow.
  const pil = quad(rect(U(0.12), 0.28, U(0.46), 0.72), rnd, 0.008);
  g.poly(pil.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.025)).fill({ color: 0x000000, alpha: 0.35 });
  g.poly(pil).fill(ST[2]).stroke({ width: LINE, color: INK, join: 'round' });
  for (const [u, s] of [[0.14, 0.3], [0.44, 0.3], [0.14, 0.7], [0.44, 0.7]]) g.circle(...at(U(u), s), 0.018).fill(ST[1]);
  // The figure: one outline, head to toes, mirrored about the middle.
  const prof: [number, number][] = [
    [0.19, 0.0], [0.2, 0.05], [0.24, 0.09], [0.3, 0.1], [0.37, 0.09], [0.42, 0.065], [0.46, 0.07], [0.5, 0.17], [0.56, 0.23], [0.66, 0.22],
    [0.84, 0.19], [1.0, 0.17], [1.1, 0.18], [1.3, 0.15], [1.5, 0.12], [1.62, 0.1], [1.66, 0.13], [1.74, 0.12], [1.72, 0.03], [1.68, 0.0],
  ];
  const fig: number[] = [];
  for (const [u, w] of prof) fig.push(...at(U(u), 0.5 - w));
  for (const [u, w] of [...prof].reverse()) fig.push(...at(U(u), 0.5 + w));
  g.poly(fig.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.05)).fill({ color: 0x000000, alpha: 0.35 });
  g.poly(fig).fill(ST[2]).stroke({ width: OUT, color: INK, join: 'round' });
  // Lit along the side toward the light, shaded along the other: the figure's roundness.
  const half = (sign: number) => {
    const pts: number[] = [];
    for (const [u, w] of prof) pts.push(...at(U(u), 0.5 + sign * w));
    for (const [u, w] of [...prof].reverse()) pts.push(...at(U(u), 0.5 + sign * w * 0.35));
    return pts;
  };
  const [ax, ay] = at(1, 0.2);
  const [bx, by] = at(1, 0.8);
  const toward = (bx - ax) * light[0] + (by - ay) * light[1] > 0 ? 1 : -1;
  g.poly(half(toward)).fill({ color: ST[3], alpha: 0.75 });
  g.poly(half(-toward)).fill({ color: ST[1], alpha: 0.7 });
  // The face in its coif, the shoulder line, the belt, the split between the legs.
  g.poly(oval(...at(U(0.31), 0.5), 0.055 * k, 0.055, 0, 12)).fill(ST[3]).stroke({ width: 0.014, color: ST[0] });
  g.moveTo(...at(U(0.52), 0.32)).quadraticCurveTo(...at(U(0.47), 0.5), ...at(U(0.52), 0.68)).stroke({ width: 0.014, color: ST[0] });
  g.moveTo(...at(U(1.04), 0.33)).lineTo(...at(U(1.04), 0.67)).stroke({ width: 0.03, color: ST[0] });
  g.moveTo(...at(U(1.15), 0.5)).lineTo(...at(U(1.64), 0.5)).stroke({ width: 0.014, color: ST[0] });
  // The sword down the body (not on a sarcophagus), the hands together on its hilt or in prayer.
  if (sword) {
    g.moveTo(...at(U(0.72), 0.5)).lineTo(...at(U(1.55), 0.5)).stroke({ width: 0.05, color: INK, cap: 'round' });
    g.moveTo(...at(U(0.72), 0.5)).lineTo(...at(U(1.55), 0.5)).stroke({ width: 0.034, color: ST[3], cap: 'round' });
    g.moveTo(...at(U(0.8), 0.39)).lineTo(...at(U(0.8), 0.61)).stroke({ width: 0.045, color: INK, cap: 'round' });
    g.moveTo(...at(U(0.8), 0.39)).lineTo(...at(U(0.8), 0.61)).stroke({ width: 0.03, color: ST[3], cap: 'round' });
  }
  const hands = [...at(U(0.6), 0.5), ...at(U(0.66), 0.44), ...at(U(0.76), 0.45), ...at(U(0.78), 0.5), ...at(U(0.76), 0.55), ...at(U(0.66), 0.56)];
  g.poly(hands).fill(ST[3]).stroke({ width: 0.018, color: INK, join: 'round' });
  g.moveTo(...at(U(0.61), 0.5)).lineTo(...at(U(0.77), 0.5)).stroke({ width: 0.014, color: ST[0] });
  // The dog the feet rest on, lying across the chest: a long body, its head up at one side with ears and a
  // snout, its tail curled at the other.
  const dog: number[] = [];
  for (let i = 0; i < 16; i++) {
    const t = (i / 16) * Math.PI * 2;
    dog.push(...at(U(1.84) + Math.sin(t) * 0.085, 0.53 + Math.cos(t) * 0.2));
  }
  g.poly(dog.map((q, i) => q - (i % 2 ? light[1] : light[0]) * 0.03)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(dog).fill(ST[2]).stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(...at(U(1.83), 0.38)).quadraticCurveTo(...at(U(1.87), 0.52), ...at(U(1.83), 0.66)).stroke({ width: 0.014, color: ST[0] });
  const [hx, hy] = at(U(1.8), 0.28);
  g.poly([...at(U(1.75), 0.24), ...at(U(1.71), 0.2), ...at(U(1.77), 0.21)]).fill(ST[2]).stroke({ width: 0.018, color: INK, join: 'round' });
  g.poly([...at(U(1.75), 0.33), ...at(U(1.71), 0.37), ...at(U(1.77), 0.36)]).fill(ST[2]).stroke({ width: 0.018, color: INK, join: 'round' });
  g.circle(hx, hy, 0.075).fill(ST[2]).stroke({ width: LINE, color: INK });
  g.poly(oval(...at(U(1.86), 0.24), 0.035, 0.035, 0, 10)).fill(ST[3]).stroke({ width: 0.018, color: INK });
  g.circle(...at(U(1.89), 0.23), 0.012).fill(INK);
  g.moveTo(...at(U(1.88), 0.72)).quadraticCurveTo(...at(U(1.98), 0.78), ...at(U(1.92), 0.66)).stroke({ width: 0.022, color: INK, cap: 'round' });
  g.moveTo(...at(U(1.88), 0.72)).quadraticCurveTo(...at(U(1.98), 0.78), ...at(U(1.92), 0.66)).stroke({ width: 0.014, color: ST[2], cap: 'round' });
  // Weathering.
  g.moveTo(...at(U(1.25), 0.1)).lineTo(...at(U(1.3), 0.2)).lineTo(...at(U(1.24), 0.26)).stroke({ width: 0.016, color: INK, alpha: 0.7 });
  for (const [u, s] of [[0.1, 0.12], [len - 0.12, 0.86], [U(1.4), 0.88]]) g.poly(patch(...at(u, s), 0.07, 0.05, rnd, 0.3, 10)).fill({ color: 0x5a7a3a, alpha: 0.6 });
}
