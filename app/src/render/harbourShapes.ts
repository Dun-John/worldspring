// Harbour structures drawn in code on the battlemap (Roadmap 5 M20), in the atlas props' cartoon look:
// few broad shapes, three close matte tones lit from the north-west (the upper left), bold ink.
//
//   drawBreakwaterArmour(g, pts, sq = PX, widthFt = 9, seed = 0)
//     Big pale rubble stones heaped along the polyline `pts` (px), a band about `widthFt` across.
//   drawCog(g, x, y, dir, lengthFt = 70, sq = PX, look = 0)
//     A moored cog centred at (x, y), its bow toward `dir` (radians, 0 = +x): hull, deck planks, castles fore
//     and aft, a hatch, the mast with its top, the sail furled on the yard. `lengthFt` 60-80; `look` 0 tarred
//     hull, 1 a red-ochre wale and a striped sail.
//   drawLantern(g, x, y, radiusFt = 6, sq = PX, lit = true)
//     A lighthouse's lamp gallery seen from above at (x, y): railing, glazed lantern round a copper cap, and
//     (lit) the only glow on the battlemap, since it is alight.
//
// `sq` is px a 5-ft square (the battlemap's shapes are at `PX`); every size and line width scales with it.
// Each draws into `g` and leaves its transform as it was.
import type { Graphics } from 'pixi.js';
import { oval, PX, rng, shade } from './atlas';

const INK = 0x1d1a14;
const FT = PX / 5;
const OUT = 0.045 * PX;
const LINE = 0.025 * PX;
/** Pale rubble (limestone, granite): dark, mid, light, sunlit. */
const RUBBLE = [0x8c8678, 0xb8b0a0, 0xd6cebc, 0xece6d6];
const WOOD = [0x5c3b22, 0x8a5a32, 0xb98552, 0xe0b47a];
const IRON = [0x34363c, 0x4a4e56, 0x6e7278, 0x9aa0a6];
const CANVAS = [0xb4a684, 0xd2c6a4, 0xe8dec4];

/** Draws `body` in a frame at (x, y) turned by `dir` and scaled to `sq` (so it works in atlas px, `PX` a
 * square); `w` scales line widths to match (Pixi doesn't scale them with the transform). */
function framed(g: Graphics, x: number, y: number, dir: number, sq: number, body: (w: (px: number) => number) => void) {
  const k = sq / PX;
  g.save();
  // (Each call composes after the last, in the parent's frame: scale, then turn, then move.)
  g.scaleTransform(k, k).rotateTransform(dir).translateTransform(x, y);
  body((px) => px * k);
  g.restore();
}

/** One big rubble stone from above (atlas px): a dark rim on the shaded side, the body, a lit top facet
 * toward the north-west, a sunlit spot; `w` scales the line widths. */
function rubble(g: Graphics, x: number, y: number, s: number, rnd: () => number, w: (px: number) => number) {
  const n = 7 + Math.floor(rnd() * 3);
  const pts: number[] = [];
  const ph = rnd() * Math.PI * 2;
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    const r = s * (0.78 + 0.3 * rnd());
    pts.push(Math.cos(a) * r, Math.sin(a) * r * 0.86);
  }
  const at = (k: number, dx: number, dy: number) => pts.map((p, i) => p * k + (i % 2 ? y + dy : x + dx));
  g.poly(at(1, 0, 0)).fill(RUBBLE[0]).stroke({ width: w(OUT), color: INK, join: 'round' });
  g.poly(at(0.84, -s * 0.07, -s * 0.07)).fill(RUBBLE[1]);
  g.poly(at(0.5, -s * 0.2, -s * 0.2)).fill(RUBBLE[2]);
  g.poly(oval(x - s * 0.34, y - s * 0.34, s * 0.16, s * 0.1, -0.7, 10)).fill(RUBBLE[3]);
}

/** Breakwater armour along `pts` (px): big pale stones, two staggered rows across the line, their wet feet
 * darkened, a soft line of foam beyond the outer row. */
export function drawBreakwaterArmour(g: Graphics, pts: [number, number][], sq = PX, widthFt = 9, seed = 0) {
  if (pts.length < 2) return;
  const k = sq / PX;
  const rnd = rng((seed ^ Math.imul(Math.round(pts[0][0]), 73856093) ^ Math.imul(Math.round(pts[0][1]), 19349663)) >>> 0);
  // Stones along the line, every ~1.8 ft, alternately either side of it, packed so they touch.
  const half = (widthFt / 2) * FT * k;
  const stones: [number, number, number][] = [];
  let carry = 0;
  let side = 1;
  for (let i = 1; i < pts.length; i++) {
    const [ax, ay] = pts[i - 1];
    const [bx, by] = pts[i];
    const len = Math.hypot(bx - ax, by - ay);
    if (len < 1e-6) continue;
    const [ux, uy] = [(bx - ax) / len, (by - ay) / len];
    const step = 1.8 * FT * k;
    let t = carry;
    for (; t < len; t += step) {
      const off = side * half * (0.45 + 0.15 * rnd());
      side = -side;
      const along = (rnd() - 0.5) * 0.5 * FT * k;
      stones.push([ax + ux * (t + along) - uy * off, ay + uy * (t + along) + ux * off, (2.0 + 0.7 * rnd()) * FT]);
    }
    carry = t - len;
  }
  const [ex, ey] = pts[pts.length - 1];
  stones.push([ex, ey, 2.6 * FT]);
  framed(g, 0, 0, 0, sq, (w) => {
    // The stones' wet feet and a soft fringe of foam along the whole band: soft, no ink.
    const line = (width: number, color: number, alpha: number, d = 0) => {
      g.moveTo(pts[0][0] / k + d, pts[0][1] / k + d);
      for (const [x, y] of pts.slice(1)) g.lineTo(x / k + d, y / k + d);
      g.stroke({ width: w(width), color, alpha, cap: 'round', join: 'round' });
    };
    line((widthFt + 5) * FT, 0xe8f0ea, 0.1);
    line((widthFt + 3) * FT, 0xe8f0ea, 0.12);
    line((widthFt + 1.5) * FT, 0x2a3e40, 0.22, 0.4 * FT);
    // The inner row first so the outer stones sit over it.
    const order = stones.map((s, i) => [s, i % 2] as const).sort((a, b) => a[1] - b[1]);
    for (const [[x, y, s]] of order) rubble(g, x / k, y / k, s, rnd, w);
  });
}

/** Half a cog's beam at `u` along it (-1 the sternpost, 1 the stem): round-bellied, both ends drawn in. */
const cogBeam = (u: number, B: number) => (u > 0 ? B * Math.pow(1 - Math.pow(u, 2.2), 0.62) : B * Math.pow(1 - Math.pow(-u, 3.2), 0.5));

function cogHull(L: number, B: number, dx = 0, dy = 0, u0 = -1, u1 = 1, n = 28): number[] {
  const pts: number[] = [];
  for (let i = 0; i <= n; i++) {
    const u = u0 + ((u1 - u0) * i) / n;
    pts.push(u * L + dx, -cogBeam(u, B) + dy);
  }
  for (let i = n; i >= 0; i--) {
    const u = u0 + ((u1 - u0) * i) / n;
    pts.push(u * L + dx, cogBeam(u, B) + dy);
  }
  return pts;
}

/** A moored cog, from above (see the top of the file). */
export function drawCog(g: Graphics, x: number, y: number, dir: number, lengthFt = 70, sq = PX, look = 0) {
  const rnd = rng((Math.imul(Math.round(x), 2654435761) ^ Math.imul(Math.round(y), 40503) ^ look) >>> 0);
  // The sun's shadows fall to the south-east of the map, whichever way the ship lies.
  const [sx, sy] = [Math.cos(-dir) - Math.sin(-dir), Math.sin(-dir) + Math.cos(-dir)];
  framed(g, x, y, dir, sq, (w) => {
    const L = (lengthFt / 2) * FT;
    const B = L * 0.34;
    const hull = look === 1 ? [0x74553a, 0x93694a, 0xad825a] : [0x6e5238, 0x8a6a48, 0xa68454];
    const deck = [0x8a6a44, 0xae8a5c, 0xc8a676];
    const shadow = (pts: number[], d: number, a = 0.24) => g.poly(pts.map((p, i) => p + (i % 2 ? sy : sx) * d)).fill({ color: 0x000000, alpha: a });
    // Its shadow on the water, the rudder and the bowsprit.
    shadow(cogHull(L, B), 0.9 * FT, 0.2);
    g.roundRect(-L - 1.8 * FT, -0.4 * FT, 2.4 * FT, 0.8 * FT, 0.3 * FT).fill(hull[1]).stroke({ width: w(OUT), color: INK, join: 'round' });
    g.moveTo(L * 0.82, 0).lineTo(L + 6 * FT, 0).stroke({ width: w(0.7 * FT + 2.8), color: INK, cap: 'round' });
    g.moveTo(L * 0.82, 0).lineTo(L + 6 * FT, 0).stroke({ width: w(0.7 * FT), color: WOOD[2], cap: 'round' });
    // The hull's side, its strakes, the wale; the rail lit along the north.
    g.poly(cogHull(L, B)).fill(hull[1]).stroke({ width: w(OUT), color: INK, join: 'round' });
    for (const f of [0.93, 0.86]) g.poly(cogHull(L * (1 - (1 - f) * 0.5), B * f)).stroke({ width: w(1.4), color: hull[0], alpha: 0.8 });
    if (look === 1) g.poly(cogHull(L * 0.965, B * 0.965)).stroke({ width: w(0.4 * FT), color: 0x97644a });
    const rail = cogHull(L * 0.82, B * 0.8);
    g.poly(rail).fill(hull[2]);
    const n = rail.length / 4;
    g.moveTo(rail[4], rail[5]);
    for (let i = 3; i < n - 2; i++) g.lineTo(rail[i * 2], rail[i * 2 + 1]);
    g.stroke({ width: w(2), color: shade(hull[2], 1.3), alpha: 0.8, cap: 'round', join: 'round' });
    // The deck, sunk inside the bulwarks (their shadow along the north and west), planked fore and aft.
    const [Ld, Bd] = [L * 0.78, B * 0.74];
    g.poly(cogHull(Ld, Bd)).fill(deck[0]).stroke({ width: w(LINE), color: INK, join: 'round' });
    g.poly(cogHull(Ld - 0.6 * FT, Bd - 0.5 * FT, 0.35 * FT, 0.35 * FT)).fill(deck[1]);
    const plank = 1.4 * FT;
    for (let yy = -Bd + plank; yy < Bd - plank * 0.5; yy += plank) {
      let [a, b] = [-1, 1];
      while (b > a && cogBeam(b, Bd) < Math.abs(yy) + 3) b -= 0.01;
      while (a < b && cogBeam(a, Bd) < Math.abs(yy) + 3) a += 0.01;
      if (b > a) g.moveTo(a * Ld + 3, yy).lineTo(b * Ld - 3, yy);
      // A butt joint or two.
      for (let j = 0; j < 2; j++) {
        const xx = (a + (b - a) * (0.25 + 0.5 * rnd())) * Ld;
        g.moveTo(xx, yy).lineTo(xx, yy + plank);
      }
    }
    g.stroke({ width: w(1.2), color: deck[0], alpha: 0.7 });
    // The hatch amidships: a coaming round boards.
    const [hx, hw, hh] = [-0.18 * L, 0.16 * L, 0.32 * B];
    shadow([hx - hw, -hh, hx + hw, -hh, hx + hw, hh, hx - hw, hh], 0.35 * FT, 0.25);
    g.rect(hx - hw, -hh, hw * 2, hh * 2).fill(WOOD[1]).stroke({ width: w(OUT), color: INK, join: 'round' });
    g.rect(hx - hw + 0.4 * FT, -hh + 0.4 * FT, hw * 2 - 0.8 * FT, hh * 2 - 0.8 * FT).fill(deck[2]);
    for (let j = 1; j < 4; j++) g.moveTo(hx - hw + (j * hw) / 2, -hh + 0.4 * FT).lineTo(hx - hw + (j * hw) / 2, hh - 0.4 * FT);
    g.stroke({ width: w(1.4), color: WOOD[1] });
    // The castles, raised over the deck: the aftcastle over the stern, a small forecastle in the bow.
    const castle = (u0: number, u1: number) => {
      const pts = cogHull(L * 0.82, B * 0.8, 0, 0, u0, u1, 12);
      shadow(pts, 0.8 * FT, 0.3);
      g.poly(pts).fill(deck[2]).stroke({ width: w(OUT), color: INK, join: 'round' });
      const inner = cogHull(L * 0.82 - 1.1 * FT, B * 0.8 - 0.8 * FT, 0, 0, u0 + (u0 < 0 ? 0.03 : 0.06), u1 - (u0 < 0 ? 0.06 : 0.03), 12);
      g.poly(inner).fill(deck[1]).stroke({ width: w(1.2), color: deck[0] });
      g.poly(cogHull(L * 0.82 - 1.5 * FT, B * 0.8 - 1.1 * FT, 0.25 * FT, 0.25 * FT, u0 + (u0 < 0 ? 0.05 : 0.08), u1 - (u0 < 0 ? 0.08 : 0.05), 12)).fill(deck[2]);
      // Its planks run across the ship.
      for (let u = u0 + 0.06; u < u1 - 0.04; u += (1.4 * FT) / (L * 0.82)) {
        const half = cogBeam(u, B * 0.8) - 0.9 * FT;
        if (half > FT) g.moveTo(u * L * 0.82, -half).lineTo(u * L * 0.82, half);
      }
      g.stroke({ width: w(1.1), color: deck[0], alpha: 0.5 });
    };
    castle(-1, -0.58);
    castle(0.74, 1);
    // The yard across the ship at the mast, the sail furled on it, lashed with gaskets.
    const mx = 0.08 * L;
    const yl = B * 1.12;
    const sail = (look === 1 ? [0xb88a6a, 0xd6aa84, 0xecd0a8] : CANVAS) as number[];
    g.moveTo(mx, -yl - 1.6 * FT).lineTo(mx, yl + 1.6 * FT).stroke({ width: w(0.5 * FT + 2.8), color: INK, cap: 'round' });
    g.moveTo(mx, -yl - 1.6 * FT).lineTo(mx, yl + 1.6 * FT).stroke({ width: w(0.5 * FT), color: WOOD[2], cap: 'round' });
    const roll: number[] = [];
    const m = 12;
    for (let i = 0; i <= m; i++) roll.push(mx - (1.0 + 0.18 * Math.sin(i * 1.9)) * FT - 0.2 * FT, -yl + (2 * yl * i) / m);
    for (let i = m; i >= 0; i--) roll.push(mx + (1.0 + 0.18 * Math.cos(i * 1.4)) * FT - 0.2 * FT, -yl + (2 * yl * i) / m);
    shadow(roll, 1.6 * FT, 0.22);
    g.poly(roll).fill(sail[1]).stroke({ width: w(OUT), color: INK, join: 'round' });
    g.roundRect(mx - 1.0 * FT, -yl + 0.6 * FT, 0.8 * FT, yl * 2 - 1.2 * FT, 0.4 * FT).fill(sail[2]);
    if (look === 1) for (let i = 1; i < 8; i += 2) g.rect(mx - 1.0 * FT, -yl + (2 * yl * i) / 8, 2 * FT, (2 * yl) / 8).fill({ color: 0x9a4a3c, alpha: 0.55 });
    for (let i = 1; i < 6; i++) {
      const yy = -yl + (2 * yl * i) / 6;
      g.moveTo(mx - 1.25 * FT, yy - 1).lineTo(mx + 1.05 * FT, yy + 1);
    }
    g.stroke({ width: w(2), color: shade(sail[0], 0.7), cap: 'round' });
    // Shrouds from the masthead down to the rails, a forestay out to the bowsprit, a backstay aft.
    const stays: [number, number][] = [];
    for (const s of [-1, 1]) for (const u of [-0.06, 0.04, 0.14]) stays.push([u * L, s * cogBeam(u, B * 0.82)]);
    stays.push([L + 5.4 * FT, 0], [-0.62 * L, 0]);
    for (const [ax, ay] of stays) g.moveTo(mx, 0).lineTo(ax, ay);
    g.stroke({ width: w(1.4), color: 0x3a2c1e, alpha: 0.85 });
    // The masthead: the round top (a fighting top), highest of all, and the mast's end inside it.
    const T = 2.3 * FT;
    g.circle(mx + sx * 2.2 * FT, sy * 2.2 * FT, T).fill({ color: 0x000000, alpha: 0.2 });
    g.circle(mx, 0, T).fill(WOOD[1]).stroke({ width: w(OUT), color: INK });
    g.circle(mx - 1.5, -1.5, T * 0.78).fill(WOOD[0]);
    g.circle(mx + 1, 1, T * 0.66).fill(shade(WOOD[1], 0.9));
    for (let i = 0; i < 10; i++) {
      const a = (i / 10) * Math.PI * 2;
      g.moveTo(mx + Math.cos(a) * T * 0.8, Math.sin(a) * T * 0.8).lineTo(mx + Math.cos(a) * T, Math.sin(a) * T);
    }
    g.stroke({ width: w(1.2), color: WOOD[0] });
    g.circle(mx, 0, 0.55 * FT).fill(WOOD[2]).stroke({ width: w(LINE), color: INK });
    g.circle(mx - 1, -1, 0.25 * FT).fill(WOOD[3]);
  });
}

/** A lighthouse's lantern from above (see the top of the file). */
export function drawLantern(g: Graphics, x: number, y: number, radiusFt = 6, sq = PX, lit = true) {
  framed(g, x, y, 0, sq, (w) => {
    const R = radiusFt * FT;
    const stone = [0x8c8474, 0xb4ab98, 0xd0c8b4];
    // The glow: the one allowed, since it's alight. Soft discs, warm.
    if (lit) for (let i = 0; i < 9; i++) g.circle(0, 0, R * (2.3 - i * 0.14)).fill({ color: 0xffd27a, alpha: 0.055 });
    // The gallery's floor, flagged, warm where the lamp lights it; its shadow side to the south-east.
    g.circle(0, 0, R).fill(stone[0]).stroke({ width: w(OUT), color: INK });
    g.circle(-1.5, -1.5, R * 0.93).fill(stone[1]);
    for (let i = 0; i < 12; i++) {
      const a = (i / 12) * Math.PI * 2 + 0.13;
      g.moveTo(Math.cos(a) * R * 0.64, Math.sin(a) * R * 0.64).lineTo(Math.cos(a) * R * 0.9, Math.sin(a) * R * 0.9);
    }
    g.stroke({ width: w(1.3), color: stone[0], alpha: 0.8 });
    if (lit) for (let i = 0; i < 4; i++) g.circle(0, 0, R * (0.92 - i * 0.07)).fill({ color: 0xffd27a, alpha: 0.12 });
    // The railing round its edge: posts on a ring.
    g.circle(0, 0, R * 0.9).stroke({ width: w(2.4), color: IRON[1] });
    for (let i = 0; i < 16; i++) {
      const a = (i / 16) * Math.PI * 2;
      g.circle(Math.cos(a) * R * 0.9, Math.sin(a) * R * 0.9, 2.2).fill(IRON[0]);
    }
    // The lantern: glass round the lamp, iron glazing bars, its shadow to the south-east.
    const G = R * 0.6;
    g.circle(0.4 * FT, 0.4 * FT, G).fill({ color: 0x000000, alpha: lit ? 0.12 : 0.25 });
    g.circle(0, 0, G).fill(lit ? 0xffe08a : 0x6a7a80).stroke({ width: w(OUT), color: INK });
    if (lit) g.circle(0, 0, G * 0.9).fill(0xfff0c0);
    for (let i = 0; i < 8; i++) {
      const a = (i / 8) * Math.PI * 2 + Math.PI / 8;
      g.moveTo(Math.cos(a) * G * 0.68, Math.sin(a) * G * 0.68).lineTo(Math.cos(a) * G, Math.sin(a) * G);
    }
    g.stroke({ width: w(2.6), color: IRON[0] });
    // The cap: a copper cone, ribbed, lit on the north-west, a ball on top.
    const C = G * 0.7;
    const copper = [0x4f6a60, 0x6f8a7e, 0x93ab9c];
    g.circle(0, 0, C).fill(copper[0]).stroke({ width: w(LINE), color: INK });
    g.poly([0, 0, ...Array.from({ length: 9 }, (_, i) => [Math.cos(Math.PI * (0.75 + i * 0.0625)) * C * 0.96, Math.sin(Math.PI * (0.75 + i * 0.0625)) * C * 0.96]).flat()]).fill(copper[2]);
    g.poly([0, 0, ...Array.from({ length: 9 }, (_, i) => [Math.cos(Math.PI * (0.25 + i * 0.0625)) * C * 0.96, Math.sin(Math.PI * (0.25 + i * 0.0625)) * C * 0.96]).flat()]).fill(shade(copper[0], 0.9));
    g.poly([0, 0, ...Array.from({ length: 9 }, (_, i) => [Math.cos(Math.PI * (1.25 + i * 0.0625)) * C * 0.96, Math.sin(Math.PI * (1.25 + i * 0.0625)) * C * 0.96]).flat()]).fill(copper[1]);
    for (let i = 0; i < 8; i++) {
      const a = (i / 8) * Math.PI * 2;
      g.moveTo(Math.cos(a) * C * 0.2, Math.sin(a) * C * 0.2).lineTo(Math.cos(a) * C * 0.96, Math.sin(a) * C * 0.96);
    }
    g.stroke({ width: w(1.3), color: shade(copper[0], 0.75) });
    g.circle(0, 0, C * 0.2).fill(0xb89a50).stroke({ width: w(LINE), color: INK });
    g.circle(-C * 0.06, -C * 0.06, C * 0.08).fill(0xe8d48e);
  });
}
