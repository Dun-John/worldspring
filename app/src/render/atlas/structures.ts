import { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { leaf, lit } from './plants';
import { blockTop, footGrass, inked, ngon, rod } from './street';
import { LINE, OUT } from '../pen';
import { BARK, STONE, moss, stone } from './stones';
import { bench, clippedBall, flower5, plank } from './garden';
import { bevel, bust } from './yard';
import { rimBand } from './water';
import { stripedCanopy } from './camp';
import { endGrain } from './wood';

/** A roof from above over the polygon `pts` (round (0, 0)): a plane to each side up to the peak, lit as it
 * faces the light, courses of shingles lapped along it, hips from the peak to the corners. Planes left out
 * (`gone`) show the timbers under them instead: rafters down the hips, purlins across. */
function hipRoof(g: Graphics, pts: number[][], tone: number, gone: Set<number> = new Set()) {
  const n = pts.length;
  for (let i = 0; i < n; i++) {
    if (gone.has(i)) continue;
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    const mid = Math.atan2(a[1] + b[1], a[0] + b[0]);
    const t = shade(tone, 0.84 + 0.28 * lit(mid));
    g.poly([0, 0, a[0], a[1], b[0], b[1]]).fill(t);
    [0.38, 0.56, 0.74, 0.92].forEach((f, row) => {
      const [x0, y0, x1, y1] = [a[0] * f, a[1] * f, b[0] * f, b[1] * f];
      g.moveTo(x0, y0).lineTo(x1, y1);
      // Shingles: short joints up from each course toward the peak, staggered row by row.
      const k = Math.max(2, Math.round(Math.hypot(x1 - x0, y1 - y0) / 9));
      for (let s = 0; s < k; s++) {
        const u = (s + 0.25 + (row % 2) * 0.5) / k;
        const [x, y] = [x0 + (x1 - x0) * u, y0 + (y1 - y0) * u];
        g.moveTo(x, y).lineTo((x * (f - 0.15)) / f, (y * (f - 0.15)) / f);
      }
    });
    g.stroke({ width: 1.1, color: shade(t, 0.7), cap: 'round' });
  }
  for (const i of gone) {
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    inked(g, () => [0.45, 0.75].forEach((f) => g.moveTo(a[0] * f, a[1] * f).lineTo(b[0] * f, b[1] * f)), 1.8, WOOD[1]);
  }
  if (gone.size) {
    inked(g, () => pts.forEach(([x, y]) => g.moveTo(0, 0).lineTo(x, y)), 2.2, WOOD[1]);
  } else {
    for (const [x, y] of pts) g.moveTo(0, 0).lineTo(x, y);
    g.stroke({ width: 2.4, color: shade(tone, 0.55) });
  }
  for (let i = 0; i < n; i++) {
    if (gone.has(i)) continue;
    const [a, b] = [pts[i], pts[(i + 1) % n]];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: 2.5, color: INK, cap: 'round' });
}

/** A finial at (x, y): a gilt ball, lit on the NW. */
function finial(g: Graphics, x: number, y: number, s: number) {
  g.circle(x, y, s).fill(0xc9a24a).stroke({ width: 1.8, color: INK });
  g.circle(x - s * 0.3, y - s * 0.3, s * 0.4).fill(0xfff1c0);
}

/** A dome from above (radius `R`): its east side in shade, a crescent of light on its NW shoulder, eight
 * straight ribs from the crown to the eaves, a painted cornice round its foot, a lantern on top (a drum
 * with windows, its cap, a gilt finial). */
function dome(g: Graphics, R: number, tones: readonly number[], cornice: number) {
  g.circle(0, 0, R + 3).fill(INK);
  g.circle(0, 0, R + 1).fill(cornice);
  g.circle(0, 0, R * 0.92).fill(tones[0]);
  g.circle(-R * 0.1, -R * 0.1, R * 0.8).fill(tones[1]);
  g.circle(-R * 0.18, -R * 0.18, R * 0.68).fill(tones[2]);
  g.circle(-R * 0.04, -R * 0.04, R * 0.62).fill(tones[1]);
  for (let i = 0; i < 8; i++) {
    const a = (i / 8) * Math.PI * 2 + Math.PI / 8;
    g.moveTo(Math.cos(a) * R * 0.26, Math.sin(a) * R * 0.26).lineTo(Math.cos(a) * R * 0.9, Math.sin(a) * R * 0.9);
  }
  g.stroke({ width: 2.2, color: shade(tones[0], 0.72), cap: 'round' });
  g.circle(0, 0, R * 0.92).stroke({ width: 1.5, color: INK });
  g.circle(R * 0.03, R * 0.04, R * 0.15).fill({ color: 0x0e1a14, alpha: 0.3 });
  g.circle(0, 0, R * 0.13).fill(cornice).stroke({ width: 1.6, color: INK });
  g.circle(0, 0, R * 0.075).fill(tones[1]).stroke({ width: 1.2, color: INK });
  finial(g, 0, 0, R * 0.04);
}

/** Steps up to a platform at its south edge (`y0`): treads `w` px wide, each a little wider and lower. */
function stepsDown(g: Graphics, y0: number, w: number, n: number, depth: number, tones: readonly number[]) {
  for (let k = n - 1; k >= 0; k--) {
    const hw = w / 2 + k * 3;
    g.rect(-hw, y0 - 4, hw * 2, depth * (k + 1) + 4).fill(shade(tones[1], 1 - k * 0.06)).stroke({ width: 1.8, color: INK, join: 'round' });
    g.moveTo(-hw + 1, y0 + depth * k + 1).lineTo(hw - 1, y0 + depth * k + 1).stroke({ width: 1.5, color: tones[2] });
  }
}

/** Gazebo, from above: an octagonal shingled roof with a gilt finial over steps at its south side (v0); a
 * white open pavilion, its balustrade round a painted floor under a copper dome gone green (v1); a pergola,
 * timber beams and rafters with climbing roses over a paved floor and a bench (v2); an old one fallen in,
 * half its roof gone to the rafters over the boards, ivy on it (v3). */
export function gazebo(g: Graphics, r: number, rnd: () => number, v: number) {
  const oct = (R: number) => ngon(R, 8, Math.PI / 8);
  if (v === 2) {
    const S = r * 0.82;
    const st = STONE[1];
    g.rect(-S, -S, S * 2, S * 2).fill(st[2]).stroke({ width: 2, color: INK });
    for (let k = 1; k < 5; k++) g.moveTo(-S, -S + (S * 2 * k) / 5).lineTo(S, -S + (S * 2 * k) / 5);
    for (let k = 0; k < 5; k++) for (let i = 1; i < 4; i++) g.moveTo(-S + (S * 2 * (i + (k % 2) * 0.5)) / 4, -S + (S * 2 * k) / 5).lineTo(-S + (S * 2 * (i + (k % 2) * 0.5)) / 4, -S + (S * 2 * (k + 1)) / 5);
    g.stroke({ width: 1.2, color: shade(st[0], 0.9) });
    bench(g, r, rnd, 0);
    // The timbers' shadows on the floor, then the posts, the two beams and the rafters across them.
    const [B, rafters] = [r * 0.68, 8];
    const xs = Array.from({ length: rafters }, (_, i) => -r * 0.72 + (i * r * 1.44) / (rafters - 1));
    for (const x of xs) g.moveTo(x + 6, -r * 0.82 + 6).lineTo(x + 6, r * 0.82 + 6);
    for (const y of [-B, B]) g.moveTo(-r * 0.86 + 6, y + 6).lineTo(r * 0.86 + 6, y + 6);
    g.stroke({ width: 5, color: 0x2a1a10, alpha: 0.22 });
    for (const [x, y] of [
      [-B, -B],
      [B, -B],
      [B, B],
      [-B, B],
    ]) {
      bevel(g, x - 6, y - 6, x + 6, y + 6, 2, WOOD, 2);
    }
    for (const y of [-B, B]) plank(g, -r * 0.86, y - 4.5, r * 0.86, y + 4.5, WOOD, rnd);
    for (const x of xs) {
      g.rect(x - 3, -r * 0.82, 6, r * 1.64).fill(WOOD[2]).stroke({ width: 1.8, color: INK, join: 'round' });
      g.moveTo(x - 1.5, -r * 0.8).lineTo(x - 1.5, r * 0.8).stroke({ width: 1, color: WOOD[3], alpha: 0.7 });
    }
    // Climbing roses over two corners and along a beam: leafy clumps (as a canopy), roses in them.
    for (const [x, y, s] of [
      [-B, -B, r * 0.26],
      [B, B, r * 0.3],
      [r * 0.15, -B, r * 0.2],
    ]) {
      const parts: [number, number, number][] = [[x, y, s * 0.5]];
      for (let i = 0; i < 6; i++) {
        const a = (i / 6) * Math.PI * 2 + rnd() * 0.6;
        const d = s * (0.35 + 0.2 * rnd());
        parts.push([x + Math.cos(a) * d, y + Math.sin(a) * d, s * (0.36 + 0.12 * rnd())]);
      }
      for (const [px, py, pr] of parts) g.circle(px, py, pr + 2).fill(INK);
      for (const [px, py, pr] of parts) g.circle(px, py, pr).fill(0x2e4a24);
      for (const [px, py, pr] of parts) g.circle(px - pr * 0.18, py - pr * 0.18, pr * 0.76).fill(0x46682e);
      for (const [px, py, pr] of parts) g.circle(px - pr * 0.34, py - pr * 0.34, pr * 0.38).fill({ color: 0x6a8c3e, alpha: 0.9 });
      for (let i = 0; i < 5; i++) {
        const a = rnd() * Math.PI * 2;
        const d = s * 0.7 * Math.sqrt(rnd());
        flower5(g, x + Math.cos(a) * d, y + Math.sin(a) * d, 4.2, i % 2 ? 0xe88aa0 : 0xc8323a, 0xf2c94c, rnd());
      }
    }
    return;
  }
  if (v === 1) {
    // Hexagonal floor, its balustrade, posts at the corners, the dome over the middle.
    const hex = (R: number) => ngon(R, 6, 0);
    stepsDown(g, r * 0.76, r * 0.42, 2, r * 0.09, [0xc8c2b4, 0xe6e0d2, 0xf8f4ea]);
    const floor = hex(r * 0.9);
    g.poly(floor.flat()).fill(0xd6cfc0).stroke({ width: 2.5, color: INK, join: 'round' });
    for (let k = -6; k <= 6; k++) {
      const w = r * 0.9 - (Math.abs(k) * r * 0.12) / Math.sqrt(3) - 1;
      g.moveTo(-w, k * r * 0.12).lineTo(w, k * r * 0.12);
    }
    g.stroke({ width: 1, color: 0xb4ac9c });
    g.poly(floor.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    const rail = hex(r * 0.82);
    for (let i = 0; i < 6; i++) {
      // (The south side, corner 1 to 2, is open for the steps.)
      if (i === 1) continue;
      const [a, b] = [rail[i], rail[(i + 1) % 6]];
      for (let k = 1; k < 7; k++) {
        const [x, y] = [a[0] + ((b[0] - a[0]) * k) / 7, a[1] + ((b[1] - a[1]) * k) / 7];
        g.circle(x, y, 2.2).fill(0xf6f2e8).stroke({ width: 1, color: INK });
      }
    }
    // The rail round, open at the steps (the side facing south, from corner 1 to corner 2).
    const open = [...rail.slice(2), ...rail.slice(0, 2)].flat();
    inked(g, () => g.poly(open, false), 3.6, 0xf6f2e8);
    for (const [x, y] of rail) g.rect(x - 4, y - 4, 8, 8).fill(0xf6f2e8).stroke({ width: 1.5, color: INK });
    g.circle(r * 0.08, r * 0.1, r * 0.62).fill({ color: 0x1a2a30, alpha: 0.25 });
    dome(g, r * 0.6, [0x2e6a58, 0x4e8e76, 0x8ac4a8], 0xf2eee4);
    return;
  }
  // v0 and v3: octagonal, on a timber floor with steps at the south side.
  stepsDown(g, r * 0.74, r * 0.4, 2, r * 0.1, STONE[0]);
  if (v === 3) {
    const floor = oct(r * 0.86);
    g.poly(floor.flat()).fill(WOOD[1]).stroke({ width: 2.5, color: INK, join: 'round' });
    const apo = r * 0.86 * Math.cos(Math.PI / 8);
    for (let k = -6; k <= 6; k++) {
      const w = Math.min(apo, apo * Math.SQRT2 - Math.abs(k * r * 0.12)) - 1;
      g.moveTo(-w, k * r * 0.12).lineTo(w, k * r * 0.12);
    }
    g.stroke({ width: 1, color: WOOD[0] });
    g.poly(floor.flat()).stroke({ width: 2.5, color: INK, join: 'round' });
    // Fallen shingles and a broken rafter on the boards.
    for (let i = 0; i < 6; i++) {
      const [x, y] = [r * (0.1 + 0.4 * rnd()), r * (-0.2 + 0.6 * rnd())];
      const a = rnd() * Math.PI;
      g.poly(oval(x, y, 4.5, 2.6, a, 6)).fill(0x6a4a30).stroke({ width: 1, color: INK });
    }
    rod(g, [r * 0.15, r * 0.5], [r * 0.6, r * 0.2], 3, WOOD[1]);
  }
  hipRoof(g, oct(r * 0.88), v === 3 ? 0x6e5a42 : 0x7e5a3a, v === 3 ? new Set([0, 1, 2, 7]) : new Set());
  if (v === 3) {
    // Ivy over the broken edge, moss on the planes left.
    for (const [x, y, s] of [
      [r * 0.62, -r * 0.18, r * 0.16],
      [-r * 0.1, r * 0.58, r * 0.14],
      [-r * 0.5, -r * 0.4, r * 0.12],
    ]) {
      g.poly(blob(x, y, s, 9, 0.5, rnd)).fill(0x34502a).stroke({ width: 1.5, color: INK, join: 'round' });
      for (let i = 0; i < 5; i++) {
        const a = rnd() * Math.PI * 2;
        g.circle(x + Math.cos(a) * s * 0.5, y + Math.sin(a) * s * 0.5, s * 0.22).fill(0x5a7e36);
      }
    }
    g.circle(0, 0, r * 0.06).fill(WOOD[0]).stroke({ width: 1.5, color: INK });
    return;
  }
  finial(g, 0, 0, r * 0.07);
}

/** Bandstand, from above: a raised octagonal stage on a stone base, a railing of balusters round it open at
 * the steps on the south side; a red roof over it (v0); open to the sky, chairs in a half ring with music
 * stands, a big drum, the conductor's box, bunting on the rail (v1); a green ogee dome on iron with a gilt
 * finial and pennant (v2); a canopy of striped canvas, scalloped round its edge (v3). */
export function bandstand(g: Graphics, r: number, rnd: () => number, v: number) {
  const ph = Math.PI / 8;
  const octPts = (R: number) => ngon(R, 8, ph).flat();
  const st = STONE[0];
  stepsDown(g, r * 0.8, r * 0.36, 3, r * 0.06, st);
  rimBand(g, octPts(r * 0.9), octPts(r * 0.8), st[2], 2.5);
  // The deck: boards along x, clipped to the octagon by hand.
  const deck = octPts(r * 0.8);
  g.poly(deck).fill(WOOD[2]);
  const apo = r * 0.8 * Math.cos(Math.PI / 8);
  for (let y = -apo + 7; y < apo; y += 7) {
    const w = Math.min(apo, apo * Math.SQRT2 - Math.abs(y)) - 1;
    g.moveTo(-w, y).lineTo(w, y);
  }
  g.stroke({ width: 1, color: WOOD[1] });
  g.poly(deck).stroke({ width: 1.8, color: INK, join: 'round' });
  const iron = v === 2;
  const railTone = iron ? IRON[1] : 0xd8ceb4;
  const rail = octPts(r * 0.74);
  const pts: [number, number][] = Array.from({ length: 8 }, (_, i) => [rail[i * 2], rail[i * 2 + 1]]);
  // The south side (corners 1 to 2) is open for the steps. (Under a canopy's valance the rail is hidden.)
  for (let i = 0; i < 8; i++) {
    if (i === 1 || v === 3) continue;
    const [a, b] = [pts[i], pts[(i + 1) % 8]];
    for (let k = 1; k < 8; k++) {
      const [x, y] = [a[0] + ((b[0] - a[0]) * k) / 8, a[1] + ((b[1] - a[1]) * k) / 8];
      g.circle(x, y, 2).fill(railTone).stroke({ width: 1, color: INK });
    }
  }
  const run = [...pts.slice(2), ...pts.slice(0, 2)].flat();
  if (v !== 3) {
    inked(g, () => g.poly(run, false), 3.6, railTone);
    for (const [x, y] of pts) g.rect(x - 4.5, y - 4.5, 9, 9).fill(railTone).stroke({ width: 1.5, color: INK });
  }
  if (v === 1) {
    // Bunting along the rail on the north side.
    for (const i of [4, 5, 6]) {
      const [a, b] = [pts[i], pts[(i + 1) % 8]];
      for (let k = 0; k < 4; k++) {
        const [t0, t1] = [k / 4 + 0.03, (k + 1) / 4 - 0.03];
        const [x0, y0, x1, y1] = [a[0] + (b[0] - a[0]) * t0, a[1] + (b[1] - a[1]) * t0, a[0] + (b[0] - a[0]) * t1, a[1] + (b[1] - a[1]) * t1];
        const [mx, my] = [(x0 + x1) / 2, (y0 + y1) / 2];
        const [nx, ny] = [-mx / Math.hypot(mx, my), -my / Math.hypot(mx, my)];
        g.poly([x0, y0, x1, y1, mx + nx * 9, my + ny * 9]).fill([0xb5523a, 0xd1a24c, 0x4f72a8, 0xecdcb4][(k + i) % 4]).stroke({ width: 1.2, color: INK, join: 'round' });
      }
    }
    // Chairs in a half ring facing the conductor's box at the front, a stand before each, the big drum
    // at the back.
    for (let i = 0; i < 7; i++) {
      const a = Math.PI * 1.1 + (i / 6) * Math.PI * 0.8;
      const [x, y] = [Math.cos(a) * r * 0.48, Math.sin(a) * r * 0.48];
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const at = (u: number, w: number) => [x + c * u - s * w, y + s * u + c * w];
      g.poly([...at(-6, -6), ...at(6, -6), ...at(6, 6), ...at(-6, 6)]).fill(WOOD[1]).stroke({ width: 1.5, color: INK, join: 'round' });
      g.poly([...at(5, -7), ...at(9, -7), ...at(9, 7), ...at(5, 7)]).fill(WOOD[0]).stroke({ width: 1.5, color: INK, join: 'round' });
      const [sx, sy] = [Math.cos(a) * r * 0.32, Math.sin(a) * r * 0.32];
      // A stand: its iron desk across, a sheet of music open on it.
      rod(g, [sx - s * 7, sy + c * 7], [sx + s * 7, sy - c * 7], 3, IRON[1]);
      g.moveTo(sx - s * 4.5, sy + c * 4.5).lineTo(sx + s * 4.5, sy - c * 4.5).stroke({ width: 2.6, color: 0xe2d8c0 });
    }
    g.circle(r * 0.03, -r * 0.47, r * 0.15).fill({ color: 0x1a1008, alpha: 0.3 });
    g.circle(0, -r * 0.5, r * 0.14).fill(0x8a2e26).stroke({ width: 2, color: INK });
    g.circle(0, -r * 0.5, r * 0.11).fill(0xd8ccae).stroke({ width: 1.2, color: INK });
    bevel(g, -r * 0.09, r * 0.14, r * 0.09, r * 0.3, 2, WOOD, 2);
    return;
  }
  g.circle(r * 0.1, r * 0.12, r * 0.66).fill({ color: 0x1a2a30, alpha: 0.25 });
  if (v === 0) {
    hipRoof(g, ngon(r * 0.66, 8, ph), 0x9a5640);
    finial(g, 0, 0, r * 0.06);
    rod(g, [-r * 0.12, -r * 0.02], [r * 0.12, -r * 0.02], 1.6);
    g.poly([r * 0.12, -r * 0.02, r * 0.07, -r * 0.06, r * 0.07, r * 0.02]).fill(IRON[2]).stroke({ width: 1, color: INK });
  } else if (v === 2) {
    dome(g, r * 0.54, [0x3a6656, 0x5a8672, 0x86aa96], 0x3a4240);
  } else {
    // A canvas canopy in eight broad stripes on its posts, its valance hanging out past the rim, a small
    // finial at the peak.
    stripedCanopy(g, r * 0.68, 8, 0xdccfae, 0xa8503a, r * 0.06, Math.PI / 8);
    finial(g, 0, 0, r * 0.05);
  }
}

/** Garden statue, straight down, on a moulded pedestal about 2.5 ft square (its base and the cornice over the
 * die): a robed figure in marble, head and shoulders, holding a laurel wreath up before it (v0); a stone urn, its
 * scrolled handles out to the sides, its mouth full of flowers and ivy spilling over the rim (v1); a warrior in
 * bronze gone green, a crested helm, a round shield on the left arm, a spear held out past the pedestal (v2); an
 * obelisk on stepped plinths, its tapering faces and pyramid point seen from above (v3). */
export function gardenStatue(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const st = STONE[0].map((c) => shade(c, 1.1));
  footGrass(g, -1.5 * FT, 1.1 * FT, 5, rnd);
  footGrass(g, 1.45 * FT, 1.35 * FT, 4, rnd);
  if (v === 3) {
    blockTop(g, -1.35 * FT, -1.35 * FT, 1.35 * FT, 1.35 * FT, st, rnd, OUT);
    blockTop(g, -1.0 * FT, -1.0 * FT, 1.0 * FT, 1.0 * FT, st.map((c) => shade(c, 1.05)), rnd);
    // The shaft tapering up from 0.65 ft to 0.38 ft each side, each face toward the light paler; the point.
    const [s0, s1] = [0.7 * FT, 0.46 * FT];
    const [ax, ay] = [-1.2, -1.2];
    const ring = (s: number) => [-s, -s, s, -s, s, s, -s, s];
    const [o, i] = [ring(s0), ring(s1)];
    // (North and west toward the light, south and east in shade, the point's facets the plainest split.)
    const face = [st[2], shade(st[0], 0.95), shade(st[0], 0.85), st[1]];
    const point = [shade(st[3], 1.04), shade(st[0], 0.9), shade(st[0], 0.78), st[2]];
    for (let k = 0; k < 4; k++) {
      const [a, b] = [k * 2, ((k + 1) % 4) * 2];
      g.poly([o[a], o[a + 1], o[b], o[b + 1], i[b], i[b + 1], i[a], i[a + 1]]).fill(face[k]);
      g.poly([i[a], i[a + 1], i[b], i[b + 1], ax, ay]).fill(point[k]);
    }
    for (let k = 0; k < 4; k++) g.moveTo(o[k * 2], o[k * 2 + 1]).lineTo(i[k * 2], i[k * 2 + 1]).lineTo(ax, ay);
    g.stroke({ width: 1.2, color: INK, join: 'round' });
    g.poly(i).stroke({ width: 1.4, color: INK, join: 'round' });
    g.poly(o).stroke({ width: LINE, color: INK, join: 'round' });
    moss(g, -1.15 * FT, 0.6 * FT, 0.28 * FT, rnd);
    return;
  }
  // The pedestal: its base, the cornice over the die.
  blockTop(g, -1.25 * FT, -1.25 * FT, 1.25 * FT, 1.25 * FT, st, rnd, OUT);
  blockTop(g, -1.02 * FT, -1.02 * FT, 1.02 * FT, 1.02 * FT, st.map((c) => shade(c, 1.07)), rnd);
  if (v === 1) {
    // The urn: two scrolled handles out to the sides, its belly showing past the lip on the shaded side, the
    // round lip lit on the NW, a dark mouth inside it, a few flowers and one trail of ivy over the rim.
    const tones = st.map((c) => shade(c, 1.08));
    for (const side of [-1, 1]) {
      inked(g, () => g.moveTo(side * 0.7 * FT, -0.28 * FT).quadraticCurveTo(side * 1.2 * FT, -0.32 * FT, side * 1.1 * FT, 0.12 * FT).quadraticCurveTo(side * 1.0 * FT, 0.38 * FT, side * 0.74 * FT, 0.26 * FT), 3, tones[2]);
    }
    g.circle(1.2, 1.4, 0.9 * FT).fill(tones[0]).stroke({ width: LINE, color: INK });
    g.circle(0, 0, 0.8 * FT).fill(tones[1]).stroke({ width: LINE, color: INK });
    g.poly(oval(-0.12 * FT, -0.14 * FT, 0.62 * FT, 0.48 * FT, -0.7, 16)).fill({ color: tones[2], alpha: 0.9 });
    g.circle(0.4, 0.4, 0.55 * FT).fill(0x2a1e14).stroke({ width: LINE * 0.75, color: INK });
    g.poly(oval(0.25 * FT, 0.22 * FT, 0.2 * FT, 0.12 * FT, 0.7, 10)).fill({ color: 0x4a3626, alpha: 0.8 });
    clippedBall(g, -0.16 * FT, -0.12 * FT, 0.26 * FT, [0x2e4a24, 0x46682e, 0x6a8c3e], rnd);
    for (let k = 0; k < 3; k++) {
      const d = (0.7 + k * 0.18) * FT;
      leaf(g, Math.cos(2.2 + k * 0.08) * d, Math.sin(2.2 + k * 0.08) * d, 2.2 + (rnd() - 0.5) * 0.8, 0.26 * FT, 2.2, k % 2 ? 0x46682e : 0x5a7e36);
    }
    for (let k = 0; k < 2; k++) flower5(g, (-0.05 + k * 0.28) * FT, (k * 0.22 - 0.2) * FT, 3.4, k ? 0xc8323a : 0xe88aa0, 0xf2c94c, rnd());
    return;
  }
  const tones = v === 0 ? [0xb4b0a6, 0xdedad2, 0xf6f4ee] : [0x4a7c6a, 0x6ea68e, 0x9ccab2];
  const K = 1.1;
  // Facing south, toward the path, give or take.
  const a = Math.PI / 2 + (rnd() - 0.5) * 0.4;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const p = (u: number, w: number): [number, number] => [(c * u - s * w) * FT * K, (s * u + c * w) * FT * K];
  // The robe's hem showing past the shoulders behind.
  g.poly(oval(...p(-0.35, 0), 0.5 * FT * K, 0.72 * FT * K, a, 14)).fill(tones[0]).stroke({ width: LINE, color: INK, join: 'round' });
  if (v === 2) {
    // The spear held out forward past the pedestal, its leaf blade; the shield on the left arm, edge out.
    rod(g, p(-0.6, 0.62), p(1.0, 0.68), 1.8, tones[1]);
    g.poly([...p(0.95, 0.6), ...p(1.38, 0.69), ...p(0.95, 0.78), ...p(0.86, 0.69)]).fill(tones[2]).stroke({ width: 1.1, color: INK, join: 'round' });
    g.poly(oval(...p(0.3, -0.92), 0.82 * FT * K, 0.32 * FT * K, a + 0.25, 16)).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(oval(...p(0.38, -0.98), 0.2 * FT * K, 0.12 * FT * K, a + 0.25, 10)).fill(tones[2]);
  } else {
    // Both arms out forward to the wreath held up before the face.
    for (const side of [-1, 1]) inked(g, () => g.moveTo(...p(0.1, side * 0.62)).quadraticCurveTo(...p(0.6, side * 0.55), ...p(0.88, side * 0.35)), 2.8, tones[1]);
  }
  const head = bust(g, 0, 0, K, a, tones, rnd);
  // The head paler than the shoulders (it stands highest, nearest the light).
  const [hx, hy] = head(0.12, 0);
  g.circle(hx - 1, hy - 1, 0.26 * FT * K).fill({ color: shade(tones[2], 1.04), alpha: 0.95 });
  g.circle(hx, hy, 0.37 * FT * K).stroke({ width: LINE, color: INK });
  if (v === 2) {
    // The helm's crest from brow to nape.
    inked(g, () => g.moveTo(...head(0.42, 0)).lineTo(...head(-0.2, 0)), 1.8, 0xa8382e);
    for (let k = 0; k < 3; k++) g.circle(...p(-0.2 + k * 0.2, 0.45 - k * 0.25), 1.8).fill({ color: 0x8ac4a8, alpha: 0.7 });
    return;
  }
  // The wreath: a ring of laurel before the face, clear of the head, leaves along it.
  const [wx, wy] = p(0.95, 0);
  const wr = 0.42 * FT * K;
  const laurel = [0x3e6040, 0x5e8a54, 0x8ab478];
  inked(g, () => g.circle(wx, wy, wr), 2.6, laurel[0]);
  for (let i = 0; i < 12; i++) {
    const t = (i / 12) * Math.PI * 2 + rnd() * 0.2;
    const k = i % 2 ? 1.14 : 0.86;
    leaf(g, wx + Math.cos(t) * wr * k, wy + Math.sin(t) * wr * k, t + Math.PI / 2 + (i % 2 ? 0.5 : -0.5), 5, 2.1, laurel[1 + (Math.cos(t) + Math.sin(t) < 0 ? 1 : 0)]);
  }
}

/** Footbridge, along x from bank to bank, arching over the water (its shadow is cast with the battlemap's,
 * drawn out along it): the deck lit toward its western end as it rises to the light, in shade toward the
 * eastern. Timber planks with handrails on posts (v0); an old humpback in warm stone, flagged in staggered
 * rows, its parapets of blocks flaring at the ends (v1); natural boards with white-painted rails and lattice
 * (v2); two logs side by side with a rope rail (v3). */
export function footbridge(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const [L, W] = [1.5 * PX, (v === 3 ? 0.3 : 0.36) * PX];
  // Light over the deck: brighter toward the west end, darker toward the east, by steps.
  const halves = (y0: number, y1: number, k = 0.2) => {
    const n = 8;
    for (let i = 0; i < n; i++) {
      const [x0, x1] = [-L + 2 + ((L * 2 - 4) * i) / n, -L + 2 + ((L * 2 - 4) * (i + 1)) / n];
      const t = ((i + 0.5) / n) * 2 - 1;
      g.rect(x0, y0, x1 - x0, y1 - y0).fill({ color: t < 0 ? 0xfff1cf : 0x1a1008, alpha: k * Math.abs(t) });
    }
  };
  if (v === 3) {
    for (const [y0, y1] of [
      [-W, -1],
      [1, W],
    ]) {
      const bark = BARK[1];
      g.roundRect(-L, y0, L * 2, y1 - y0, (y1 - y0) / 2).fill(bark[1]).stroke({ width: 2.2, color: INK });
      g.roundRect(-L + 4, y0 + 3, L * 2 - 8, (y1 - y0) * 0.3, 2).fill({ color: bark[2], alpha: 0.7 });
      for (let i = 0; i < 9; i++) {
        const x = -L + 10 + rnd() * (L * 2 - 20);
        const y = y0 + (y1 - y0) * (0.4 + 0.4 * rnd());
        g.moveTo(x, y).lineTo(x + 6 + rnd() * 8, y).stroke({ width: 1.4, color: shade(bark[0], 0.7), cap: 'round' });
      }
      for (const x of [-L + (y1 - y0) * 0.3, L - (y1 - y0) * 0.3]) endGrain(g, x, (y0 + y1) / 2, (y1 - y0) * 0.3, (y1 - y0) * 0.48, bark);
    }
    halves(-W, W);
    // A rope rail on three posts along the north side.
    const posts = [-L * 0.85, 0, L * 0.85];
    for (const x of posts) g.circle(x, -W - 4, 4).fill(WOOD[1]).stroke({ width: 1.8, color: INK });
    for (let i = 0; i < 2; i++) {
      const [a, b] = [posts[i], posts[i + 1]];
      inked(g, () => g.moveTo(a, -W - 4).quadraticCurveTo((a + b) / 2, -W + 6, b, -W - 4), 1.8, 0xc8a870);
    }
    return;
  }
  if (v === 1) {
    const st = [0x7a6448, 0xa88e68, 0xcab48a, 0xe8d6b0];
    // Flags in three rows along the deck, the joints of each row staggered from the next.
    g.rect(-L, -W, L * 2, W * 2).fill(st[2]).stroke({ width: 2, color: INK });
    const [y0, y1] = [-W + 6, W - 6];
    for (let row = 0; row < 3; row++) {
      const [a, b] = [y0 + ((y1 - y0) * row) / 3, y0 + ((y1 - y0) * (row + 1)) / 3];
      if (row) g.moveTo(-L + 2, a).lineTo(L - 2, a + (rnd() - 0.5) * 1.5);
      for (let x = -L + 6 + row * 7 + rnd() * 6; x < L - 6; x += 15 + rnd() * 8) g.moveTo(x, a + 1).lineTo(x + (rnd() - 0.5) * 2, b - 1);
    }
    g.stroke({ width: 1.3, color: shade(st[0], 0.85) });
    for (let i = 0; i < 6; i++) g.rect(-L + 8 + rnd() * (L * 2 - 24), y0 + rnd() * (y1 - y0 - 4), 9, 3).fill({ color: st[3], alpha: 0.5 });
    // The hump: the west half lit as it rises toward the light, the east half in shade, a pale crown line
    // across the top.
    g.rect(-L + 2, -W + 2, L - 2, W * 2 - 4).fill({ color: 0xfff1cf, alpha: 0.22 });
    g.rect(0, -W + 2, L - 2, W * 2 - 4).fill({ color: 0x2a1a10, alpha: 0.24 });
    g.moveTo(0, -W + 3).lineTo(0, W - 3).stroke({ width: 2.2, color: st[3], alpha: 0.9 });
    // The parapets: a low band of blocks of the same stone along each side, splaying out at the ends.
    for (const side of [-1, 1]) {
      const y = side * (W - 3.5);
      const run = (x0: number, y0: number, x1: number, y1: number, n: number) => {
        const [dx, dy] = [(x1 - x0) / n, (y1 - y0) / n];
        const len = Math.hypot(dx, dy);
        const [nx, ny] = [(-dy / len) * 3.5, (dx / len) * 3.5];
        for (let k = 0; k < n; k++) {
          const [ax, ay, bx, by] = [x0 + dx * k, y0 + dy * k, x0 + dx * (k + 1), y0 + dy * (k + 1)];
          const tone = shade(st[1], (k + (side > 0 ? 1 : 0)) % 2 ? 0.94 : 1.04);
          g.poly([ax - nx, ay - ny, bx - nx, by - ny, bx + nx, by + ny, ax + nx, ay + ny]).fill(tone).stroke({ width: 1.5, color: INK, join: 'round' });
          g.moveTo(ax - nx * 0.8, ay - ny * 0.8).lineTo(bx - nx * 0.8, by - ny * 0.8).stroke({ width: 1.2, color: st[3], alpha: 0.8 });
        }
      };
      run(-L - 2, y + side * 8, -L + 14, y, 2);
      run(-L + 14, y, L - 14, y, 10);
      run(L - 14, y, L + 2, y + side * 8, 2);
    }
    moss(g, -L * 0.55, -W, 6, rnd);
    moss(g, L * 0.7, W - 1, 5, rnd);
    return;
  }
  // Timber: planks across, rails on posts along both sides (painted white, with a lattice, on v2).
  const deck = v === 2 ? WOOD.map((c) => shade(c, 1.06)) : WOOD;
  g.rect(-L, -W, L * 2, W * 2).fill(deck[1]).stroke({ width: 2.2, color: INK });
  for (let x = -L + 8; x < L - 2; x += 8) g.moveTo(x, -W + 1).lineTo(x, W - 1);
  g.stroke({ width: 1.2, color: deck[0] });
  for (let i = 0; i < 6; i++) {
    const x = -L + 10 + rnd() * (L * 2 - 20);
    g.circle(x, -W + 6, 0.9).fill(IRON[0]);
    g.circle(x, W - 6, 0.9).fill(IRON[0]);
  }
  halves(-W, W);
  const white = 0xf6f2e8;
  const posts = v === 2 ? [-L + 5, 0, L - 5] : [-L + 4, -L / 3, L / 3, L - 4];
  for (const side of [-1, 1]) {
    const y = side * (W - 4);
    if (v === 2) {
      // The lattice between the posts, inked, then the white rail over it (its ends cut square).
      for (const [width, color] of [
        [3.6, INK],
        [1.6, white],
      ] as const) {
        for (let i = 0; i + 1 < posts.length; i++) {
          const [a, b] = [posts[i], posts[i + 1]];
          for (let k = 0; k < 3; k++) {
            const [x0, x1] = [a + 4 + ((b - a - 8) * k) / 3, a + 4 + ((b - a - 8) * (k + 1)) / 3];
            g.moveTo(x0, y - 5.5).lineTo(x1, y + 5.5).moveTo(x0, y + 5.5).lineTo(x1, y - 5.5);
          }
        }
        g.stroke({ width, color, cap: 'butt' });
      }
    }
    rod(g, [-L + 2, y], [L - 2, y], v === 2 ? 3 : 3.2, v === 2 ? white : WOOD[2]);
    for (const x of posts) {
      const ps = v === 2 ? 5 : 3.5;
      g.rect(x - ps, y - ps, ps * 2, ps * 2).fill(v === 2 ? white : WOOD[0]).stroke({ width: 1.6, color: INK });
      if (v === 2) g.circle(x, y, 2).fill(0xc9a24a).stroke({ width: 1, color: INK });
    }
  }
  for (const x of [-L - 1, L + 1]) {
    for (const y of [-W + 2, W - 2]) stone(g, x, y, 4 + rnd() * 1.5, STONE[1], rnd, 6);
  }
}
