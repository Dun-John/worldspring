import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { LINE, OUT } from './farm';
import { BARK, STONE, moss, stone } from './stones';
import { endGrain } from './wood';
import { footGrass, upright } from './street';
import { UPRIGHT_FOOT } from '../atlas';

/** An anvil from above on a stump before a smithy, along x, its horn to +x (to -x in v3): dark iron,
 * its four feet showing past a flat face worn bright, a hardy hole at the heel. A small wooden quench
 * tub of dark water beside it (v1, v3), tongs on the stump (v1), a hammer on it (v2). */
export function anvil(g: Graphics, r: number, rnd: () => number, v: number) {
  const f = v === 3 ? -1 : 1;
  const X = (x: number) => f * x;
  const flipped = (pts: number[]) => pts.map((p, i) => (i % 2 ? p : X(p)));
  if (v === 1 || v === 3) {
    // The tub: a hooped stave rim, dark water.
    const [tx, ty, T] = [X(0.55 * r), 0.48 * r, 0.26 * r];
    g.circle(tx, ty, T).fill(WOOD[1]).stroke({ width: LINE, color: INK });
    g.circle(tx - T * 0.1, ty - T * 0.1, T * 0.86).stroke({ width: 1.4, color: WOOD[2], alpha: 0.85 });
    g.circle(tx, ty, T * 0.68).fill(0x2a2620).stroke({ width: 1.2, color: INK });
    g.ellipse(tx + T * 0.18, ty + T * 0.2, T * 0.26, T * 0.1).fill({ color: 0x6a6656, alpha: 0.7 });
  }
  // The stump.
  const [sx, sy] = [X(-0.12 * r), -0.04 * r];
  g.poly(oval(sx, sy + r * 0.05, r * 0.5, r * 0.44, 0)).fill(BARK[0][1]).stroke({ width: OUT, color: INK });
  endGrain(g, sx, sy, r * 0.42, r * 0.37, BARK[0]);
  // Its feet: a waisted block under the face, flaring out past it at both ends.
  const [h0, h1] = [-0.52 * r, 0.24 * r];
  const feet = flipped([sx * f + h0 + 2, sy - 0.32 * r, sx * f - 0.14 * r, sy - 0.16 * r, sx * f + h1, sy - 0.32 * r, sx * f + h1, sy + 0.32 * r, sx * f - 0.14 * r, sy + 0.16 * r, sx * f + h0 + 2, sy + 0.32 * r]);
  g.poly(feet).fill(IRON[0]).stroke({ width: LINE, color: INK, join: 'round' });
  // The face and the horn tapering off it.
  const w = 0.2 * r;
  const face = flipped([
    sx * f + h0, sy - w,
    sx * f + h1, sy - w,
    sx * f + h1 + 0.2 * r, sy - w * 0.55,
    sx * f + h1 + 0.4 * r, sy - 0.02 * r,
    sx * f + h1 + 0.2 * r, sy + w * 0.5,
    sx * f + h1, sy + w,
    sx * f + h0, sy + w,
  ]);
  g.poly(face).fill(IRON[1]).stroke({ width: OUT, color: INK, join: 'round' });
  // Lit along its north edge, the face worn bright, the hardy hole.
  g.poly(flipped([sx * f + h0 + 2, sy - w + 2, sx * f + h1 + 0.18 * r, sy - w * 0.5 + 1, sx * f + h1 + 0.36 * r, sy - 0.02 * r, sx * f + h1, sy - w * 0.35, sx * f + h0 + 2, sy - w * 0.35])).fill(IRON[2]);
  const worn = sx * f + h0 + 0.14 * r;
  g.rect(Math.min(X(worn), X(worn + 0.4 * r)), sy - w * 0.25, 0.4 * r, w * 0.55).fill({ color: IRON[3], alpha: 0.75 });
  g.rect(X(sx * f + h0 + 0.08 * r) - 2, sy - 2, 4, 4).fill(INK);
  if (v === 1) {
    // Tongs laid over the stump's edge.
    const [a, b] = [[X(-0.3 * r), 0.36 * r], [X(0.2 * r), 0.42 * r]];
    for (const d of [-1.8, 1.8]) g.moveTo(a[0], a[1] + d).lineTo(b[0], b[1] + d * 0.3);
    g.stroke({ width: 3.4, color: INK, cap: 'round' });
    for (const d of [-1.8, 1.8]) g.moveTo(a[0], a[1] + d).lineTo(b[0], b[1] + d * 0.3);
    g.stroke({ width: 1.5, color: IRON[2], cap: 'round' });
  }
  if (v === 2) {
    // A hammer on the stump, its haft out over the edge.
    const [hx, hy] = [X(-0.32 * r), 0.3 * r];
    g.moveTo(hx, hy).lineTo(hx + X(0.36 * r), hy + 0.14 * r).stroke({ width: 4.4, color: INK, cap: 'round' });
    g.moveTo(hx, hy).lineTo(hx + X(0.36 * r), hy + 0.14 * r).stroke({ width: 2.2, color: WOOD[2], cap: 'round' });
    g.roundRect(hx - 3.5, hy - 5.5, 7, 11, 1.5).fill(IRON[1]).stroke({ width: LINE * 0.75, color: INK });
  }
  void rnd;
}

// Roadmap 5 cemeteries: cairns, standing stones, yews, a lychgate, kurgans and tomb slabs.

/** Cairn: a heap of rounded field stones about 4 ft across, the big ones round its foot, smaller ones piled
 * on them toward the middle and a capstone on top; moss on one, a lighter heap of fresh stones on another. */
export function cairn(g: Graphics, r0: number, rnd: () => number, v: number) {
  // (Drawn a third over its catalog size, a darker heap on a dark ring and a shadow, so it stands out among headstones.)
  const r = r0 * 1.35;
  const tones = STONE[v].map((c) => shade(c, 0.82));
  g.ellipse(r * 0.22, r * 0.3, r * 1.12, r * 0.95).fill({ color: 0x1c1a14, alpha: 0.3 });
  g.circle(0, 0, r * 0.98).fill(tones[0]).stroke({ width: OUT, color: INK });
  const ring = (n: number, d: number, s: number, off: number) => {
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < n; i++) {
      const a = ph + ((i + (rnd() - 0.5) * 0.4) / n) * Math.PI * 2;
      stone(g, Math.cos(a) * d * r + off, Math.sin(a) * d * r + off, r * s * (0.85 + 0.3 * rnd()), tones, rnd, 9);
    }
  };
  ring(8, 0.66, 0.3, 0);
  ring(5, 0.36, 0.28, -r * 0.04);
  stone(g, -r * 0.08, -r * 0.08, r * 0.27, v === 3 ? STONE[0] : tones, rnd, 9);
  if (v === 1) moss(g, -r * 0.42, -r * 0.4, r * 0.18, rnd);
}

/** Standing stone, from a high angle like the headstone (upright things only read from the side): a tall rough
 * slab, unworked, narrowing to a broken top (its top showing behind), lit down its left side, shaded on the
 * right, a crack or two and pale lichen; grass and a fallen chip at its foot; one leaning. */
export function standingStone(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // (A cooler, darker grey than the headstones' stone.)
  const tones = [STONE[1], STONE[3], STONE[0], STONE[2]][v].map((c) => shade(c, 0.78));
  // (Drawn well over life size, so it stands taller than the headstones do.)
  const S = 1.6;
  const P0 = upright(UPRIGHT_FOOT[101]);
  const P = (x: number, d: number, h: number) => P0(x * S, d * S, h * S);
  const H = [8, 9.2, 7, 6.4][v];
  const lean = v === 3 ? 0.12 : 0;
  // The outline across its front (x, h), from the left foot up and over the top to the right foot.
  const j = () => (rnd() - 0.5) * 0.25;
  const out: [number, number][] = [
    [-1.5, 0], [-1.6 + j(), H * 0.3], [-1.35 + j(), H * 0.62], [-1.0 + j(), H * 0.9], [-0.45, H], [0.25 + j(), H - 0.15], [0.75, H - 0.55 + j()],
    [1.05 + j(), H * 0.8], [1.3 + j(), H * 0.5], [1.5 + j(), H * 0.2], [1.45, 0],
  ];
  const face = (d: number) => out.flatMap(([x, h]) => P(x + lean * h, d, h));
  const ink = { width: 2.2, color: INK, join: 'round' as const };
  // Its shadow cast to the lower right across the grass.
  {
    const [b0, b1] = [P(-1.5, 0.4, 0), P(1.45, 0.4, 0)];
    const [dx, dy] = [H * S * FT * 0.3, H * S * FT * 0.12];
    g.poly([b0[0], b0[1], b1[0], b1[1], b1[0] + dx, b1[1] + dy, b0[0] + dx * 0.9 + 6, b0[1] + dy + 4]).fill({ color: 0x1c1a14, alpha: 0.36 });
  }
  for (const x of [-1.7, 1.6]) footGrass(g, x * S * FT, P(0, 0.2, 0)[1], 6, rnd);
  // Its top, seen from above behind the front face, then the face.
  g.poly(face(-0.7)).fill(shade(tones[1], 1.05)).stroke(ink);
  g.poly(face(0)).fill(tones[1]).stroke(ink);
  // Lit down the left, in shade on the right.
  // (Points across the front as (x, fraction of the height), leaning with it.)
  const band = (pts: [number, number][]) => pts.flatMap(([x, f]) => P(x + lean * H * f, 0, H * f));
  g.poly(band([[-1.5, 0.01], [-1.6, 0.3], [-1.3, 0.62], [-0.95, 0.88], [-0.6, 0.82], [-0.95, 0.4], [-1.05, 0.02]])).fill({ color: tones[3], alpha: 0.6 });
  g.poly(band([[0.6, 0.72], [1.05, 0.8], [1.3, 0.5], [1.45, 0.005], [0.4, 0.005], [0.75, 0.35]])).fill({ color: tones[0], alpha: 0.5 });
  // A crack down from the top, a fainter one across.
  const c0 = 0.1 + (rnd() - 0.5) * 0.5;
  g.moveTo(...P(c0 + lean * H, 0, H - 0.2)).lineTo(...P(c0 + 0.3 + lean * H * 0.75, 0, H * 0.75)).lineTo(...P(c0 + lean * H * 0.55, 0, H * 0.55));
  g.stroke({ width: 1.8, color: shade(tones[0], 0.7), cap: 'round', join: 'round' });
  g.moveTo(...P(-1.1 + lean * H * 0.3, 0, H * 0.3)).lineTo(...P(-0.3 + lean * H * 0.33, 0, H * 0.34)).stroke({ width: 1.2, color: shade(tones[0], 0.8), cap: 'round' });
  // Lichen: a few pale crusts.
  for (let i = 0; i < 3; i++) {
    const [x, h] = [-0.8 + 1.2 * rnd(), H * (0.25 + 0.55 * rnd())];
    const [px, py] = P(x + lean * h, 0, h);
    g.poly(oval(px, py, 3 + rnd() * 2.5, 2 + rnd() * 1.5, rnd(), 9)).fill({ color: i === 0 ? 0xc8c48a : 0xb4b88e, alpha: 0.85 });
  }
  if (v === 1) moss(g, P(-0.9, 0, 0.6)[0], P(-0.9, 0, 0.6)[1], 6, rnd);
  footGrass(g, P(0.2, 0.2, 0)[0], P(0.2, 0.2, 0)[1] + 1, 4.5, rnd);
}

/** Yew from above: a dark, dense evergreen, its crown rounded to a blunt cone: whorls of many short, soft
 * lobes, each smaller and nearer the light than the one below, the darkest green of the trees; red berries
 * on some. */
export function yew(g: Graphics, r: number, rnd: () => number, v: number) {
  const tones = [
    [0x2a4a30, 0x3a6040, 0x4c7a50, 0x6a9a62],
    [0x2c4c2a, 0x3c6238, 0x4e7c46, 0x6c9c5a],
    [0x284a38, 0x386048, 0x4a7a5a, 0x68986e],
    [0x2e502c, 0x40683a, 0x528046, 0x729c5a],
  ][v];
  const layers: [number, number][] = [
    [1, 22],
    [0.76, 17],
    [0.53, 13],
    [0.3, 9],
  ].map(([k, n]) => [k * 0.82, n]) as [number, number][];
  // A tier's edge: small round lobes, bulging out between dips, a little uneven.
  const tier = (cx: number, cy: number, rr: number, n: number, depth: number, jit: number[]) => {
    const ph = jit[n] * Math.PI;
    const at = (i: number, k: number): [number, number] => {
      const a = ph + (i / n) * Math.PI * 2;
      return [cx + Math.cos(a) * rr * k, cy + Math.sin(a) * rr * k];
    };
    g.moveTo(...at(0, 1 - depth));
    for (let i = 0; i < n; i++) {
      const bulge = 1 + depth * (0.6 + 0.8 * jit[i]);
      g.quadraticCurveTo(...at(i + 0.5, bulge), ...at(i + 1, 1 - depth));
    }
    g.closePath();
  };
  layers.forEach(([k, n], l) => {
    const off = -l * r * 0.07;
    const jit = Array.from({ length: n + 1 }, () => rnd());
    tier(off, off, r * k, n, 0.05, jit);
    g.fill(tones[l]);
    if (l === 0) g.stroke({ width: OUT, color: INK, join: 'round' });
    else g.stroke({ width: 1.5, color: shade(tones[l - 1], 0.7), join: 'round' });
    const h = r * k * 0.07;
    tier(off - h, off - h, r * k * 0.84, n, 0.05, jit);
    g.fill({ color: shade(tones[l], 1.3), alpha: 0.5 });
    tier(off - h * 1.6, off - h * 1.6, r * k * 0.55, n, 0.05, jit);
    g.fill({ color: tones[3], alpha: 0.28 });
  });
  if (v === 1 || v === 3) {
    for (let i = 0; i < 9; i++) {
      const a = rnd() * Math.PI * 2;
      const d = r * (0.35 + 0.55 * rnd());
      g.circle(Math.cos(a) * d - r * 0.05, Math.sin(a) * d - r * 0.05, 2.2).fill(0xc8362a).stroke({ width: 1, color: 0x4a1410 });
    }
  }
}

/** Lychgate from above, its ridge along x and the way through it along y: a short gabled roof (the north
 * plane paler, rows of tiles or shingles, a dark ridge, barge boards at the gable ends), its four corner posts
 * showing past the gables, a step of flags under the eaves either side; a dark opening and a pair of gate posts at each end where the way goes through. */
export function lychgate(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const roof = [
    [0x6e3424, 0x9a5038, 0xbe6e4c],
    [0x7a4020, 0xa85e30, 0xcc8048],
    [0x5a3e26, 0x84603a, 0xa8804e],
    [0x5c2e22, 0x84442e, 0xa65e40],
  ][v];
  // (12 by 7 ft over the gap: a roof, not a post, at the battlemap's middle zoom.)
  const [rx, ry, px, py] = [6 * FT, 3.5 * FT, 6.45 * FT, 2.6 * FT];
  // Flagstones under the eaves where the way goes through.
  for (const s of [-1, 1]) {
    g.rect(-1.6 * FT, s > 0 ? ry - 2 : -ry - 1.3 * FT + 2, 3.2 * FT, 1.3 * FT).fill(STONE[1][2]).stroke({ width: LINE, color: INK });
    g.moveTo(0, s > 0 ? ry - 2 : -ry + 2).lineTo(0, s > 0 ? ry + 1.3 * FT - 2 : -ry - 1.3 * FT + 2).stroke({ width: 1, color: STONE[1][0] });
  }
  // The posts, past the gable ends.
  for (const sx of [-1, 1]) for (const sy of [-1, 1]) g.rect(sx * px - 0.45 * FT, sy * py - 0.45 * FT, 0.9 * FT, 0.9 * FT).fill(WOOD[1]).stroke({ width: LINE, color: INK });
  // The roof: an eave overhang with its shadow, the south plane in shade, the north lit, tile rows along the ridge.
  const ov = 0.5 * FT;
  g.rect(-rx - ov + 3, -ry - ov + 4, rx * 2 + ov * 2, ry * 2 + ov * 2).fill({ color: 0x1c1a14, alpha: 0.32 });
  g.rect(-rx - ov, -ry - ov, rx * 2 + ov * 2, ry * 2 + ov * 2).fill(shade(roof[0], 0.8)).stroke({ width: OUT, color: INK, join: 'round' });
  g.rect(-rx, -ry, rx * 2, ry).fill(shade(roof[2], 1.12));
  g.rect(-rx, 0, rx * 2, ry).fill(shade(roof[0], 0.9));
  for (let k = 1; k < 4; k++) {
    const y = (ry * k) / 4;
    g.moveTo(-rx + 2, -y).lineTo(rx - 2, -y);
    g.moveTo(-rx + 2, y).lineTo(rx - 2, y);
  }
  g.stroke({ width: 1.4, color: shade(roof[0], 0.6), alpha: 0.85 });
  if (v === 3) for (let i = 0; i < 3; i++) moss(g, -rx * 0.6 + rnd() * rx * 1.2, -ry * 0.6 + rnd() * ry * 1.4, 5, rnd);
  // Barge boards, the eave edges and a heavy ridge: a dark line, a pale cap on its lit side.
  for (const sx of [-1, 1]) g.rect(sx * rx - (sx > 0 ? 0.35 * FT : 0), -ry, 0.35 * FT, ry * 2).fill(WOOD[0]);
  g.moveTo(-rx, -ry).lineTo(rx, -ry).moveTo(-rx, ry).lineTo(rx, ry).stroke({ width: 2.4, color: shade(roof[0], 0.45) });
  g.moveTo(-rx, 0).lineTo(rx, 0).stroke({ width: 6, color: shade(roof[0], 0.5), cap: 'round' });
  g.moveTo(-rx + 2, -2).lineTo(rx - 2, -2).stroke({ width: 3, color: shade(roof[2], 1.3), cap: 'round' });
  g.rect(-rx, -ry, rx * 2, ry * 2).stroke({ width: OUT, color: INK, join: 'round' });
  // The way through: a dark threshold cut in each eave, flags worn pale before it, between two stout gate posts.
  for (const sy of [-1, 1]) {
    const y0 = sy > 0 ? ry - 0.9 * FT : -ry - ov;
    g.rect(-1.5 * FT, y0, 3 * FT, 0.9 * FT + ov).fill({ color: 0x120c07, alpha: 0.9 }).stroke({ width: LINE, color: INK });
    for (const sx of [-1, 1]) g.rect(sx * 1.5 * FT - 0.4 * FT, sy * ry - 0.4 * FT + sy * ov * 0.6, 0.8 * FT, 0.8 * FT).fill(WOOD[2]).stroke({ width: LINE, color: INK });
  }
}

/** Kurgan: a broad burial mound under grass, drawn at its catalog size (bigger or smaller by its scale): a
 * soft rounded rise on its own shadow, a darker band of turf round its foot, in shade on its south-east side
 * and lit toward the north-west, its crown paler, tufts of grass, a few stones at its foot; on its top a
 * standing stone (a slab laid flat on one, a stone figure on another), a worn path up one, a kerb of stones
 * right round the last. */
export function kurgan(g: Graphics, r: number, rnd: () => number, v: number) {
  const grass = [
    [0x4a6a2e, 0x668a40, 0x82a452, 0xa0bc6a],
    [0x52682e, 0x6e8840, 0x8ca252, 0xaab86c],
    [0x4e6a34, 0x6a8a46, 0x86a45a, 0xa2bc74],
    [0x566c30, 0x728c42, 0x90a656, 0xb0be70],
  ][v];
  const R = r * 0.9;
  const rim = blob(0, 0, R, 28, 0.06, rnd);
  // Its shadow, then the darker turf at its foot, then the shaded side: the body drawn again a little
  // smaller and toward the light.
  g.poly(rim.map((p, i) => p * 0.98 + R * (i % 2 ? 0.07 : 0.08))).fill({ color: 0x1c1a14, alpha: 0.28 });
  g.poly(rim).fill(shade(grass[0], 0.5)).stroke({ width: 3, color: shade(grass[0], 0.36), join: 'round' });
  g.poly(rim.map((p) => p * 0.88 - R * 0.02)).fill(grass[0]);
  g.poly(rim.map((p) => p * 0.8 - R * 0.07)).fill(grass[1]);
  g.poly(rim.map((p) => p * 0.66 - R * 0.12)).fill(grass[2]);
  g.poly(rim.map((p) => p * 0.4 - R * 0.17)).fill({ color: grass[3], alpha: 0.8 });
  // Tufts of grass on its flanks.
  for (let i = 0; i < 10; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R * (0.3 + 0.55 * rnd());
    footGrass(g, Math.cos(a) * d, Math.sin(a) * d, 5 + rnd() * 3, rnd);
  }
  if (v === 2) {
    // A worn path up its south side to the top.
    g.moveTo(R * 0.1, R * 0.98).quadraticCurveTo(R * 0.3, R * 0.5, -R * 0.12, -R * 0.15).stroke({ width: 9, color: 0xb09a6e, alpha: 0.75, cap: 'round' });
  }
  // Stones at its foot: a few, or a kerb right round it.
  const n = v === 3 ? 22 : 6;
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = v === 3 ? ph + (i / n) * Math.PI * 2 : ph + rnd() * Math.PI * 2;
    stone(g, Math.cos(a) * R * 0.97, Math.sin(a) * R * 0.97, v === 3 ? 11 : 9 + rnd() * 5, STONE[1], rnd, 7);
  }
  const [x, y] = [-R * 0.16, -R * 0.14];
  // (What stands on top, about a sixth of the mound across: `k` px a unit below, about its crown.)
  const k = R / 80;
  const at = (pts: number[]) => pts.map((d, i) => (i % 2 ? y : x) + d * k);
  if (v === 2) {
    // A slab laid flat on its crown, where the path ends: cracked, lit on its upper edge.
    g.ellipse(x + 4 * k, y + 5 * k, 17 * k, 10 * k).fill({ color: 0x1d2a12, alpha: 0.3 });
    g.roundRect(x - 14 * k, y - 8 * k, 28 * k, 16 * k, 3 * k).fill(STONE[1][1]).stroke({ width: 2 * k, color: INK });
    g.roundRect(x - 12 * k, y - 7 * k, 24 * k, 4 * k, 2 * k).fill(STONE[1][2]);
    g.poly(at([-3, -8, 1, -1, -1, 8]), false).stroke({ width: 1.4 * k, color: STONE[1][0], cap: 'round' });
  } else if (v !== 1) {
    // A standing stone on its crown, from a high angle: a rough upright, its top showing, its shadow behind.
    const t = STONE[v === 3 ? 3 : 0];
    g.ellipse(x + 8 * k, y + 3 * k, 15 * k, 6 * k).fill({ color: 0x1d2a12, alpha: 0.32 });
    g.poly(at([-6, 2, -7, -20, -2, -26, 5, -24, 7, 2])).fill(t[1]).stroke({ width: 2 * k, color: INK, join: 'round' });
    g.poly(at([-7, -20, -2, -26, 5, -24, 1, -19])).fill(t[2]).stroke({ width: 1.4 * k, color: INK, join: 'round' });
    g.poly(at([-4, -14, -4, -2]), false).stroke({ width: 1.6 * k, color: t[2], alpha: 0.7, cap: 'round' });
  }
  if (v === 1) {
    // A stone figure on its crown, from a high angle: a worn upright block with a face.
    g.ellipse(x + 6 * k, y + 4 * k, 16 * k, 7 * k).fill({ color: 0x1d2a12, alpha: 0.3 });
    g.roundRect(x - 8 * k, y - 28 * k, 16 * k, 30 * k, 6 * k).fill(STONE[2][1]).stroke({ width: 2 * k, color: INK });
    g.roundRect(x - 8 * k, y - 32 * k, 16 * k, 9 * k, 4 * k).fill(STONE[2][2]).stroke({ width: 1.6 * k, color: INK });
    for (const [a, b, c] of [[-4, -1.5, -17], [1.5, 4, -17], [-4, 4, -6]]) g.moveTo(x + a * k, y + c * k).lineTo(x + b * k, y + c * k);
    g.stroke({ width: 1.6 * k, color: STONE[2][0], cap: 'round' });
  }
}

/** Tomb slab from above, along x (about 7 ft by 3): a ledger stone flat on the grass with a carved cross
 * (v0), a chest tomb, its lid overhanging its sides (v1), a ledger sunk at one end, cracked, an epitaph cut
 * in it and moss over it (v2), a chest tomb with a raised cross on its lid and flowers laid on it (v3). */
export function tombSlab(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = [STONE[1], STONE[0], STONE[3], STONE[2]][v];
  const [L, W] = [3.5 * FT, 1.5 * FT];
  const chest = v === 1 || v === 3;
  footGrass(g, -L - 1, W - 1, 4.5, rnd);
  footGrass(g, L + 1, -W + 3, 4, rnd);
  if (chest) {
    // Its sides, in shade below the lid on the south and east.
    g.rect(-L + 3, -W + 4, L * 2, W * 2).fill(tones[0]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(-L - 2, -W - 2, L * 2 + 4, W * 2 + 2).fill(tones[2]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(-L + 2, -W + 2, L * 2 - 4, W * 2 - 6).fill(tones[1]);
    g.rect(-L + 2, -W + 2, L * 2 - 4, 3).fill({ color: tones[3], alpha: 0.7 });
  } else {
    g.rect(-L, -W, L * 2, W * 2).fill(tones[1]).stroke({ width: 2, color: INK, join: 'round' });
    // A chamfer: lit along the north and west, in shade along the south and east.
    g.poly([-L, -W, L, -W, L - 3, -W + 3, -L + 3, -W + 3]).fill(tones[2]);
    g.poly([-L, -W, -L + 3, -W + 3, -L + 3, W - 3, -L, W]).fill(tones[2]);
    g.poly([L, W, -L, W, -L + 3, W - 3, L - 3, W - 3]).fill(tones[0]);
  }
  const groove = (pts: number[], w: number) => {
    for (let i = 0; i < pts.length; i += 4) g.moveTo(pts[i], pts[i + 1]).lineTo(pts[i + 2], pts[i + 3]);
    g.stroke({ width: w, color: shade(tones[0], 0.75), cap: 'round' });
  };
  if (v === 0 || v === 1) {
    groove([-L * 0.6, 0, L * 0.45, 0, -L * 0.25, -W * 0.62, -L * 0.25, W * 0.62], 3.4);
  } else if (v === 2) {
    // An epitaph across its head end, a crack, moss; its foot end sunk (shaded).
    for (let i = 0; i < 3; i++) groove([-L * 0.7 + i * 7, -W * 0.5, -L * 0.7 + i * 7, W * (0.2 + 0.25 * rnd())], 2);
    g.poly([L * 0.35, -W + 3, L - 3, -W + 3, L - 3, W - 3, L * 0.3, W - 3]).fill({ color: tones[0], alpha: 0.35 });
    g.moveTo(L * 0.05, -W).lineTo(L * 0.15, -W * 0.2).lineTo(L * 0.02, W * 0.4).lineTo(L * 0.12, W).stroke({ width: 1.6, color: INK, join: 'round' });
    moss(g, L * 0.6, W * 0.35, 7, rnd);
  } else {
    // A raised cross: a lit north-west edge over its shadow.
    const arms = (dx: number, dy: number) => [-L * 0.6 + dx, dy, L * 0.3 + dx, dy, -L * 0.25 + dx, -W * 0.6 + dy, -L * 0.25 + dx, W * 0.5 + dy];
    for (const [pts, w, c] of [[arms(1.5, 1.5), 5, tones[0]], [arms(0, 0), 4, tones[3]]] as const) {
      for (let i = 0; i < pts.length; i += 4) g.moveTo(pts[i], pts[i + 1]).lineTo(pts[i + 2], pts[i + 3]);
      g.stroke({ width: w, color: c, cap: 'round' });
    }
    for (let i = 0; i < 3; i++) g.circle(L * 0.45 + i * 5, -W * 0.25 + (i % 2) * 6, 3).fill([0xe0533d, 0xf2c94c, 0xf6efe0][i]).stroke({ width: 1, color: INK });
  }
}
