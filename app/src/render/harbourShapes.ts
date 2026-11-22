// Harbour structures drawn in code on the battlemap (Roadmap 5 M20), in the atlas props' cartoon look:
// few broad shapes, three close matte tones lit from the north-west (the upper left), bold ink.
//
//   drawBreakwaterArmour(g, pts, sq = PX, widthFt = 9, seed = 0)
//     A few big rounded pale stones along the polyline `pts` (px), a band about `widthFt` across, one
//     line of foam on the water side (the left-hand normal (-uy, ux) of the line's run).
//   drawCog(g, x, y, dir, lengthFt = 70, sq = PX, look = 0)
//     A moored cog centred at (x, y), its bow toward `dir` (radians, 0 = +x): hull, deck planks, castles fore
//     and aft, a hatch, the mast with its top, the sail furled on the yard. `lengthFt` 60-80; `look` 0 tarred
//     hull, 1 a red-ochre wale and a striped sail.
//   drawLantern(g, x, y, radiusFt = 9.5, sq = PX, lit = true)
//     A lighthouse's top seen from above at (x, y), its tower `radiusFt` at the foot: the tapering tower's
//     courses, the gallery corbelled out round the top with its rail, the glazed lantern under a copper cap,
//     and (lit) the only glow on the battlemap, since it is alight.
//
// `sq` is px a 5-ft square (the battlemap's shapes are at `PX`); every size and line width scales with it.
// Each draws into `g` and leaves its transform as it was.
import type { Graphics } from 'pixi.js';
import { oval, PX, rng, shade } from './atlas';
import { INK, FT, OUT, LINE } from './pen';

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

/** `poly` (flat x, y) cut to the side of the line through (ax, ay) where (nx, ny) points; each kept
 * vertex flagged when the edge that leaves it lies along the cut. */
function cutPoly(poly: number[], ax: number, ay: number, nx: number, ny: number): [number[], boolean[]] {
  const out: number[] = [];
  const along: boolean[] = [];
  const n = poly.length / 2;
  const side = (i: number) => (poly[i * 2] - ax) * nx + (poly[i * 2 + 1] - ay) * ny;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const [di, dj] = [side(i), side(j)];
    if (di >= 0) {
      out.push(poly[i * 2], poly[i * 2 + 1]);
      along.push(false);
    }
    if (di >= 0 !== dj >= 0) {
      const t = di / (di - dj);
      out.push(poly[i * 2] + (poly[j * 2] - poly[i * 2]) * t, poly[i * 2 + 1] + (poly[j * 2 + 1] - poly[i * 2 + 1]) * t);
      // (Entering the cut: the edge from here runs along it to where the outline comes back out.)
      along.push(di >= 0);
    }
  }
  return [out, along];
}

/** One big rounded stone from above (atlas px): a smooth lopsided round, the shaded tone with the lit
 * tone over its north-west, a soft ink outline; `w` scales the line widths. With `cut` [ax, ay, nx, ny]
 * only its part on the (nx, ny) side of that line shows (the rest lies under the made ground), its
 * outline left open along the cut. */
function boulder(g: Graphics, x: number, y: number, s: number, rnd: () => number, w: (px: number) => number, cut?: number[]) {
  const n = 16;
  const pts: number[] = [];
  const [p2, p3, turn] = [rnd() * 6.3, rnd() * 6.3, rnd() * Math.PI];
  const squash = 0.8 + 0.12 * rnd();
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2;
    const r = s * (1 + 0.09 * Math.sin(2 * a + p2) + 0.05 * Math.sin(3 * a + p3));
    const [px, py] = [Math.cos(a) * r, Math.sin(a) * r * squash];
    pts.push(px * Math.cos(turn) - py * Math.sin(turn), px * Math.sin(turn) + py * Math.cos(turn));
  }
  const at = (k: number, dx: number, dy: number) => pts.map((p, i) => p * k + (i % 2 ? y + dy : x + dx));
  const clip = (poly: number[]): [number[], boolean[]] => (cut ? cutPoly(poly, cut[0], cut[1], cut[2], cut[3]) : [poly, poly.filter((_, i) => i % 2 === 0).map(() => false)]);
  const [body, along] = clip(at(1, 0, 0));
  if (body.length < 6) return;
  g.poly(body).fill(RUBBLE[0]);
  const [lit] = clip(at(0.8, -s * 0.12, -s * 0.12));
  if (lit.length >= 6) g.poly(lit).fill(RUBBLE[2]);
  const m = body.length / 2;
  if (!along.includes(true)) {
    g.poly(body).stroke({ width: w(OUT), color: INK, alpha: 0.8, join: 'round' });
    return;
  }
  // The open outline: from the vertex where it comes out of the cut round to where it goes back in.
  const start = (along.indexOf(true) + 1) % m;
  g.moveTo(body[start * 2], body[start * 2 + 1]);
  for (let q = 1; q < m; q++) {
    const i = (start + q) % m;
    g.lineTo(body[i * 2], body[i * 2 + 1]);
    if (along[i]) break;
  }
  g.stroke({ width: w(OUT), color: INK, alpha: 0.8, join: 'round', cap: 'round' });
}

/** `pts` moved `d` px along their left-hand normal (-uy, ux), the normals averaged at the bends. */
function offsetLine(pts: [number, number][], d: number, mitre = false): [number, number][] {
  if (mitre) return mitreLine(pts, d);
  return pts.map((p, i) => {
    const [a, b] = [pts[Math.max(0, i - 1)], pts[Math.min(pts.length - 1, i + 1)]];
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
    return [p[0] - ((b[1] - a[1]) / l) * d, p[1] + ((b[0] - a[0]) / l) * d];
  });
}

/** `pts` (px) offset `d` along their left-hand normal, each corner mitred (a closed ring's ends,
 * which meet, joined too): a retaining wall turning a yard's corners at its full width. */
function mitreLine(pts: [number, number][], d: number): [number, number][] {
  const n = pts.length;
  const ring = n > 2 && Math.hypot(pts[0][0] - pts[n - 1][0], pts[0][1] - pts[n - 1][1]) < 1e-3;
  const nrm = (a: [number, number], b: [number, number]): [number, number] => {
    const l = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1;
    return [-(b[1] - a[1]) / l, (b[0] - a[0]) / l];
  };
  return pts.map((p, i) => {
    const prev = i > 0 ? pts[i - 1] : ring ? pts[n - 2] : null;
    const next = i < n - 1 ? pts[i + 1] : ring ? pts[1] : null;
    const n0 = prev ? nrm(prev, p) : nrm(p, next!);
    const n1 = next ? nrm(p, next) : n0;
    const c = 1 + n0[0] * n1[0] + n0[1] * n1[1];
    const [mx, my] = c > 0.25 ? [(n0[0] + n1[0]) / c, (n0[1] + n1[1]) / c] : [n1[0], n1[1]];
    return [p[0] + mx * d, p[1] + my * d];
  });
}

/** Breakwater armour along `pts` (px), the water on their left-hand normal (-uy, ux): a few big
 * rounded stones in two staggered rows, the outer ones bigger, seated in a soft wet shade, and one
 * line of foam along the water side. */
export function drawBreakwaterArmour(g: Graphics, pts: [number, number][], sq = PX, widthFt = 9, seed = 0) {
  if (pts.length < 2) return;
  const k = sq / PX;
  const rnd = rng((seed ^ Math.imul(Math.round(pts[0][0]), 73856093) ^ Math.imul(Math.round(pts[0][1]), 19349663)) >>> 0);
  const half = (widthFt / 2) * FT * k;
  // The line's pieces: [start, end, index, length] along it.
  const along: [number, number, number, number][] = [];
  let run = 0;
  for (let i = 1; i < pts.length; i++) {
    const len = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]);
    if (len < 1e-6) continue;
    along.push([run, run + len, i - 1, len]);
    run += len;
  }
  if (run < 1e-6) return;
  const pointAt = (t: number, off: number): [number, number] => {
    const seg = along.find((s) => t <= s[1]) ?? along[along.length - 1];
    const [ax, ay] = pts[seg[2]];
    const [bx, by] = pts[seg[2] + 1];
    const [ux, uy] = [(bx - ax) / seg[3], (by - ay) / seg[3]];
    const u = t - seg[0];
    return [ax + ux * u - uy * off, ay + uy * u + ux * off];
  };
  // The line's own piece at t, as the cut [ax, ay, nx, ny] (atlas px) the made ground makes there.
  const cutAt = (t: number): number[] => {
    const seg = along.find((s) => t <= s[1]) ?? along[along.length - 1];
    const [ax, ay] = pts[seg[2]];
    const [bx, by] = pts[seg[2] + 1];
    const [ux, uy] = [(bx - ax) / seg[3], (by - ay) / seg[3]];
    return [ax / k, ay / k, -uy, ux];
  };
  // An outer row every ~5.2 ft, bedded at the made ground's edge so its outer half shows on the water;
  // a row of smaller stones between them, further under, only their outer rims showing in the gaps.
  const nOut = Math.max(1, Math.round(run / (5.2 * FT * k)));
  const jig = () => (rnd() - 0.5) * 0.4 * FT * k;
  const stones: [number, number, number, number[]][] = [];
  for (let j = 0; j < nOut; j++) {
    const t = (run * (j + 0.5)) / nOut + jig();
    stones.push([...pointAt(t, half * (0.02 + 0.08 * rnd())), (2.1 + 0.4 * rnd()) * FT, cutAt(t)]);
  }
  for (let j = 0; j <= nOut; j++) {
    const t = Math.min(run, Math.max(0, (run * j) / nOut + jig()));
    stones.push([...pointAt(t, half * (0.2 + 0.1 * rnd())), (2.7 + 0.6 * rnd()) * FT, cutAt(t)]);
  }
  framed(g, 0, 0, 0, sq, (w) => {
    const line = (p: [number, number][], width: number, color: number, alpha: number, cap: 'round' | 'butt') => {
      g.moveTo(p[0][0] / k, p[0][1] / k);
      for (const [x, y] of p.slice(1)) g.lineTo(x / k, y / k);
      g.stroke({ width: w(width), color, alpha, cap, join: 'round' });
    };
    // The wet shade the stones sit in on the water side (clear of the made ground), then the one line
    // of foam along the outer row.
    line(offsetLine(pts, half * 0.75), widthFt * 0.55 * FT, 0x2a3e40, 0.2, 'butt');
    line(offsetLine(pts, half * 0.88), 0.7 * FT, 0xf2f6f0, 0.6, 'round');
    for (const [x, y, s, c] of stones) boulder(g, x / k, y / k, s, rnd, w, c);
  });
  // The made ground's edge coped and inked as every built edge on the water (`drawQuayCoping`),
  // closing the stones' open outlines along it.
  drawQuayCoping(g, pts, 1.6, seed, false, k);
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

/** A lighthouse's top from above, its tower `radiusFt` at the foot (see the top of the file). */
export function drawLantern(g: Graphics, x: number, y: number, radiusFt = 9.5, sq = PX, lit = true) {
  framed(g, x, y, 0, sq, (w) => {
    const T = radiusFt * FT;
    // The taper: the tower's top well in from its foot, the gallery corbelled out past the top, the
    // lantern's glazing inside the gallery's rail, its cap over the glass.
    const top = T * 0.6;
    const R = T * 0.72;
    const G = T * 0.42;
    const stone = [0x7c7568, 0xa29a88, 0xc6bea9, 0xe4ddcb];
    // The glow: the one allowed, since it's alight. Soft discs, warm.
    if (lit) for (let i = 0; i < 9; i++) g.circle(0, 0, T * (1.75 - i * 0.1)).fill({ color: 0xffd27a, alpha: 0.05 });
    // The tower's battered wall from above, foot to top: lit on the north-west, shaded to the
    // south-east, its courses ringing it ever smaller as it rises.
    g.circle(0, 0, T).fill(stone[0]).stroke({ width: w(OUT), color: INK });
    g.circle(-T * 0.05, -T * 0.05, T * 0.94).fill(stone[2]);
    g.circle(T * 0.025, T * 0.025, T * 0.9).fill(stone[1]);
    const rnd = rng((Math.imul(Math.round(radiusFt * 10), 2654435761) ^ 0x11e7) >>> 0);
    for (let k = 1; k <= 4; k++) {
      const r = T - (k / 5) * (T - top);
      g.circle(0, 0, r).stroke({ width: w(1.2), color: stone[0], alpha: 0.55 });
      // (The joints between its blocks, staggered course by course.)
      const n = 18 - k * 2;
      const r0 = T - ((k - 1) / 5) * (T - top);
      for (let i = 0; i < n; i++) {
        const a = ((i + (k % 2) * 0.5 + rnd() * 0.2) / n) * Math.PI * 2;
        g.moveTo(Math.cos(a) * r, Math.sin(a) * r).lineTo(Math.cos(a) * r0 * 0.99, Math.sin(a) * r0 * 0.99);
      }
      g.stroke({ width: w(1), color: stone[0], alpha: 0.45 });
    }
    // The gallery: its shadow on the tower, its flagged deck, the rail round its edge.
    g.circle(T * 0.07, T * 0.07, R).fill({ color: 0x000000, alpha: 0.22 });
    g.circle(0, 0, R).fill(stone[1]).stroke({ width: w(OUT), color: INK });
    g.circle(-1.2, -1.2, R * 0.92).fill(stone[3]);
    for (let i = 0; i < 12; i++) {
      const a = (i / 12) * Math.PI * 2 + 0.13;
      g.moveTo(Math.cos(a) * G * 1.05, Math.sin(a) * G * 1.05).lineTo(Math.cos(a) * R * 0.9, Math.sin(a) * R * 0.9);
    }
    g.stroke({ width: w(1.2), color: stone[1], alpha: 0.9 });
    if (lit) for (let i = 0; i < 4; i++) g.circle(0, 0, R * (0.9 - i * 0.07)).fill({ color: 0xffd27a, alpha: 0.1 });
    g.circle(0, 0, R * 0.92).stroke({ width: w(2.4), color: IRON[0] });
    for (let i = 0; i < 18; i++) {
      const a = (i / 18) * Math.PI * 2;
      g.circle(Math.cos(a) * R * 0.92, Math.sin(a) * R * 0.92, w(2.3)).fill(IRON[1]).stroke({ width: w(0.8), color: INK });
    }
    // The lantern (v66b, as the lamp room's drawn inside: `interior/lamp.ts`): light rays out over the
    // gallery when lit, a brass sill, a ring of twelve glass panes between thin iron mullions, warm inside.
    if (lit) {
      for (let i = 0; i < 8; i++) {
        const a = (i / 8) * Math.PI * 2 + Math.PI / 8;
        const s = 0.12;
        g.poly([Math.cos(a - s) * G, Math.sin(a - s) * G, Math.cos(a - s * 0.6) * R * 1.12, Math.sin(a - s * 0.6) * R * 1.12, Math.cos(a + s * 0.6) * R * 1.12, Math.sin(a + s * 0.6) * R * 1.12, Math.cos(a + s) * G, Math.sin(a + s) * G]).fill({ color: 0xffd65a, alpha: 0.3 });
      }
    }
    g.circle(0.45 * FT, 0.45 * FT, G * 1.04).fill({ color: 0x000000, alpha: lit ? 0.14 : 0.25 });
    g.circle(0, 0, G * 1.04).fill(0xc89a3c).stroke({ width: w(OUT), color: INK });
    g.circle(0, 0, G * 0.97).fill(lit ? 0xfff0c0 : 0x7d8e94);
    g.circle(0, 0, G * 0.97).stroke({ width: G * 0.14, color: lit ? 0xffe29a : 0x6a7c84 });
    g.circle(0, 0, G * 0.97).stroke({ width: w(1), color: INK, alpha: 0.6 });
    for (let i = 0; i < 12; i++) {
      const a = (i / 12) * Math.PI * 2;
      g.moveTo(Math.cos(a) * G * 0.68, Math.sin(a) * G * 0.68).lineTo(Math.cos(a) * G * 1.02, Math.sin(a) * G * 1.02);
    }
    g.stroke({ width: w(1.4), color: IRON[0] });
    for (const i of [7, 8]) {
      const [a, b] = [((i + 0.2) / 12) * Math.PI * 2, ((i + 0.8) / 12) * Math.PI * 2];
      g.moveTo(Math.cos(a) * G * 0.9, Math.sin(a) * G * 0.9).lineTo(Math.cos(b) * G * 0.9, Math.sin(b) * G * 0.9);
    }
    g.stroke({ width: w(1.6), color: 0xffffff, alpha: 0.8 });
    // The cap: a copper dome in eight ribbed gores, verdigris, lit on the north-west, the vent's
    // ball on top.
    const C = G * 0.52;
    const copper = [0x4f6a60, 0x6f8a7e, 0x93ab9c];
    g.circle(0, 0, C).fill(copper[0]).stroke({ width: w(LINE), color: INK, join: 'round' });
    const gore = (a0: number, color: number) => g.poly([0, 0, ...Array.from({ length: 9 }, (_, i) => [Math.cos(Math.PI * (a0 + i * 0.0625)) * C * 0.96, Math.sin(Math.PI * (a0 + i * 0.0625)) * C * 0.96]).flat()]).fill(color);
    gore(0.75, copper[2]);
    gore(0.25, shade(copper[0], 0.9));
    gore(1.25, copper[1]);
    for (let i = 0; i < 8; i++) {
      const a = ((i + 0.5) / 8) * Math.PI * 2;
      g.moveTo(Math.cos(a) * C * 0.2, Math.sin(a) * C * 0.2).lineTo(Math.cos(a) * C, Math.sin(a) * C);
    }
    g.stroke({ width: w(1.3), color: shade(copper[0], 0.75) });
    g.circle(0, 0, C * 0.22).fill(0xb89a50).stroke({ width: w(LINE), color: INK });
    g.circle(-C * 0.07, -C * 0.07, C * 0.09).fill(0xe8d48e);
  });
}

/** A retaining wall along `pts` (px), the low ground on their left-hand normal (`battlemap::retaining`):
 * the wall's shadow on the low ground, wider the more it stands over it (`riseFt`), then a quay's coping
 * along its top (`drawQuayCoping`, dry), inked along the face. */
export function drawRetainingWall(g: Graphics, pts: [number, number][], widthFt = 2.2, riseFt = 4, seed = 0, k = 1) {
  if (pts.length < 2) return;
  const strip = (a: [number, number][], b: [number, number][]) => g.poly([...a.flat(), ...[...b].reverse().flat()]);
  // (The face as the eye catches it from a little off overhead, a band of darker stone wider the
  // taller it stands, inked at its foot; the shadow on the low ground past it.)
  const face = Math.min(3.5, Math.max(0.8, 0.3 * riseFt)) * FT * k;
  const sh = Math.min(3, 0.6 + 0.18 * riseFt) * FT * k;
  const foot = offsetLine(pts, face, true);
  strip(foot, offsetLine(pts, face + sh, true)).fill({ color: 0x2b2620, alpha: 0.24 });
  strip(pts, foot).fill(shade(RUBBLE[1], 0.78));
  g.moveTo(foot[0][0], foot[0][1]);
  for (const [x, y] of foot.slice(1)) g.lineTo(x, y);
  g.stroke({ width: LINE * k, color: INK, alpha: 0.85, cap: 'butt', join: 'round' });
  drawQuayCoping(g, pts, widthFt, seed, false, k, true);
}

/** A stone quay front's coping along its face `pts` (px), the water on their left-hand normal: a
 * course of pale dressed blocks `widthFt` across laid along the edge, their joints square to it,
 * inked boldly along the water, a lit arris just inside (`soft`: a retaining wall's, a shade
 * duller and inked lighter, against grass). Every built edge on the water wears it
 * (quay fronts; a breakwater's edges, `wet` false: its armour shades the water), the timber piers
 * the same ink. `k`: px per atlas px (a square `k * PX` px). */
export function drawQuayCoping(g: Graphics, pts: [number, number][], widthFt = 1.6, seed = 0, wet = true, k = 1, soft = false) {
  if (pts.length < 2) return;
  const rnd = rng((seed ^ Math.imul(Math.round(pts[0][0]), 73856093) ^ Math.imul(Math.round(pts[0][1]), 19349663)) >>> 0);
  const wid = widthFt * FT * k;
  const inner = offsetLine(pts, -wid, soft);
  const line = (p: [number, number][], width: number, color: number, alpha: number) => {
    g.moveTo(p[0][0], p[0][1]);
    for (const [x, y] of p.slice(1)) g.lineTo(x, y);
    g.stroke({ width, color, alpha, cap: 'butt', join: 'round' });
  };
  // (The wall's wet face below it, a soft shade on the water.)
  if (wet) line(offsetLine(pts, wid * 0.35), wid * 0.7, 0x2a3e40, 0.3);
  g.poly([...pts.flat(), ...[...inner].reverse().flat()]).fill(soft ? shade(RUBBLE[2], 0.9) : RUBBLE[2]);
  line(offsetLine(pts, -wid * 0.3, soft), wid * 0.25, RUBBLE[3], 0.8);
  // The joints: one every 2.4-3.4 ft along the face, square across the course (one stroke for all).
  let carry = rnd() * 2 * FT * k;
  let joints = 0;
  for (let i = 1; i < pts.length; i++) {
    const [ax, ay] = pts[i - 1];
    const [bx, by] = pts[i];
    const len = Math.hypot(bx - ax, by - ay);
    if (len < 1e-6) continue;
    const [ux, uy] = [(bx - ax) / len, (by - ay) / len];
    let t = carry;
    while (t < len) {
      const [x, y] = [ax + ux * t, ay + uy * t];
      g.moveTo(x, y).lineTo(x + uy * wid, y - ux * wid);
      joints++;
      t += (2.4 + rnd()) * FT * k;
    }
    carry = t - len;
  }
  if (joints) g.stroke({ width: LINE * k, color: RUBBLE[0], alpha: 0.75 });
  line(inner, LINE * k, RUBBLE[0], 0.7);
  line(pts, (soft ? LINE * 1.4 : OUT) * k, INK, 1);
}
