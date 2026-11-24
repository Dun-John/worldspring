import { Graphics } from 'pixi.js';
import { FT, INK, IRON, WOOD, blob, oval, shade } from './common';
import { LINE, OUT } from './farm';
import { GREY_WOOD } from './stable';
import { barrel, bevel, crate } from './yard';
import { STONE, stone } from './stones';

// Roadmap 5 harbours (M20): moored boats, bollards, dock cranes, net racks and buoys.

/** Half a boat's beam at `u` along it (-1 the stern, 1 the stem): full forward, a fine bow, the stern a
 * transom `tr` of the beam wide. */
export const beamAt = (u: number, B: number, tr: number) => (u > 0 ? B * (1 - Math.pow(u, 2.4)) * (1 - 0.12 * u) : B * (1 - (1 - tr) * u * u));

/** A hull's outline along x, the bow toward +x: half its length `L`, half its beam `B`, moved by (dx, dy). */
function hullPts(L: number, B: number, tr: number, dx = 0, dy = 0, n = 20): number[] {
  const pts: number[] = [];
  for (let i = 0; i <= n; i++) {
    const u = -1 + (2 * i) / n;
    pts.push(u * L + dx, -beamAt(u, B, tr) + dy);
  }
  for (let i = n - 1; i >= 0; i--) {
    const u = -1 + (2 * i) / n;
    pts.push(u * L + dx, beamAt(u, B, tr) + dy);
  }
  return pts;
}

/** Hull paint: a muted slate teal and a red ochre. */
const BOAT_PAINT = [0x5f7f80, 0x97644a];
const ROPE = 0xc8b07a;
const NET = [0x8a6e4a, 0x5e4a36, 0x6e7a52, 0xb8a27a];
const CORK = 0xd6b27a;
const WICKER = 0xb08a52;

/** A small boat's open shell, bow toward +x: the hull's side (painted or bare) round the gunwale cap, lit
 * along the north; the inside sunk, its north and west walls in shadow; floor boards and thwarts at `thwarts`
 * (along it, -1 to 1). Gives the inside's half length and beam. */
function boatShell(g: Graphics, L: number, B: number, tr: number, tones: readonly number[], paint: number | null, thwarts: number[]): [number, number] {
  const side = (paint === null ? 0.3 : 0.42) * FT;
  const cap = 0.3 * FT;
  g.poly(hullPts(L, B, tr)).fill(paint ?? tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  if (paint !== null) g.poly(hullPts(L - side * 0.6, B - side * 0.45, tr)).fill(shade(paint, 0.94));
  g.poly(hullPts(L - side * 1.3, B - side, tr)).fill(tones[2]);
  const [Li, Bi] = [L - (side + cap) * 1.3, B - side - cap];
  g.poly(hullPts(Li, Bi, tr * 0.95)).fill(shade(tones[1], 0.84)).stroke({ width: 1.2, color: INK, join: 'round' });
  g.poly(hullPts(Li - 0.32 * FT, Bi - 0.28 * FT, tr * 0.9, 0.16 * FT, 0.14 * FT)).fill(tones[1]);
  // The cap's lit north edge.
  const mid = hullPts(L - side * 1.3 - cap * 0.6, B - side - cap * 0.5, tr);
  g.moveTo(mid[4], mid[5]);
  for (let i = 6; i <= 34; i += 2) g.lineTo(mid[i], mid[i + 1]);
  g.stroke({ width: 1.6, color: tones[3], alpha: 0.85, cap: 'round', join: 'round' });
  // Floor boards.
  for (const f of [-0.34, 0.34]) g.moveTo(-Li * 0.78, f * Bi).lineTo(Li * 0.5, f * Bi);
  g.stroke({ width: 1.1, color: shade(tones[0], 1.1), alpha: 0.7 });
  for (const u of thwarts) {
    const [x, w, t] = [u * Li, beamAt(u, Bi, tr * 0.95) + 1, 0.42 * FT];
    g.rect(x - t, -w, t * 2, w * 2).fill(tones[2]).stroke({ width: LINE, color: INK, join: 'round' });
    g.rect(x - t + 1.2, -w + 1.5, t * 0.6, w * 2 - 3).fill({ color: tones[3], alpha: 0.6 });
  }
  return [Li, Bi];
}

/** An oar lying from its grip (x0, y0) to its blade's tip (x1, y1): an inked ash loom, the blade widening to it. */
function oar(g: Graphics, x0: number, y0: number, x1: number, y1: number, color = 0xc9ac7c) {
  const len = Math.hypot(x1 - x0, y1 - y0);
  const [ux, uy] = [(x1 - x0) / len, (y1 - y0) / len];
  const [nx, ny] = [-uy, ux];
  const [bl, bw, sw] = [1.8 * FT, 0.3 * FT, 0.13 * FT];
  const [bx, by] = [x1 - ux * bl, y1 - uy * bl];
  g.moveTo(x0, y0).lineTo(bx, by).stroke({ width: sw * 2 + 2.6, color: INK, cap: 'round' });
  const at = (a: number, b: number) => [bx + ux * a + nx * b, by + uy * a + ny * b];
  g.poly([...at(-0.2 * FT, sw), ...at(bl * 0.45, bw), ...at(bl - 2, bw * 0.9), ...at(bl, 0), ...at(bl - 2, -bw * 0.9), ...at(bl * 0.45, -bw), ...at(-0.2 * FT, -sw)])
    .fill(color)
    .stroke({ width: LINE, color: INK, join: 'round' });
  g.moveTo(x0, y0).lineTo(bx, by).stroke({ width: sw * 2, color, cap: 'round' });
  g.moveTo(x0, y0).lineTo(x0 + ux * 0.7 * FT, y0 + uy * 0.7 * FT).stroke({ width: sw * 2, color: shade(color, 0.72), cap: 'round' });
}

/** A coil of rope from above: inked, three turns, the outer one lit on the north-west. */
function ropeCoil(g: Graphics, x: number, y: number, R: number) {
  g.circle(x, y, R).fill(shade(ROPE, 0.82)).stroke({ width: LINE, color: INK });
  for (const k of [0.7, 0.42]) g.circle(x, y, R * k).stroke({ width: 1.2, color: shade(ROPE, 0.58) });
  g.circle(x, y, R * 0.2).fill(shade(ROPE, 0.5));
  g.moveTo(x + Math.cos(Math.PI * 0.85) * R * 0.85, y + Math.sin(Math.PI * 0.85) * R * 0.85)
    .arc(x, y, R * 0.85, Math.PI * 0.85, Math.PI * 1.65)
    .stroke({ width: 1.8, color: shade(ROPE, 1.12), cap: 'round' });
}

/** A round wicker basket from above, its weave in ticks round the rim, holding fish or cork floats. */
function basketOf(g: Graphics, x: number, y: number, R: number, what: 'fish' | 'floats', rnd: () => number) {
  g.circle(x, y, R).fill(WICKER).stroke({ width: LINE, color: INK });
  for (let k = 0; k < 14; k++) {
    const a = (k / 14) * Math.PI * 2;
    g.moveTo(x + Math.cos(a) * R * 0.8, y + Math.sin(a) * R * 0.8).lineTo(x + Math.cos(a) * R * 0.98, y + Math.sin(a) * R * 0.98);
  }
  g.stroke({ width: 1.1, color: shade(WICKER, 0.66) });
  g.circle(x, y, R * 0.78).fill(shade(WICKER, 0.62));
  g.circle(x + 1, y + 1, R * 0.66).fill(shade(WICKER, 0.8));
  for (let k = 0; k < 4; k++) {
    const a = rnd() * Math.PI * 2;
    const d = R * 0.3 * Math.sqrt(rnd());
    const [cx, cy, rot] = [x + Math.cos(a) * d, y + Math.sin(a) * d, rnd() * Math.PI];
    if (what === 'fish') {
      const [c, s] = [Math.cos(rot), Math.sin(rot)];
      const tail = [cx - c * R * 0.42, cy - s * R * 0.42];
      g.poly([tail[0], tail[1], tail[0] - c * R * 0.14 - s * R * 0.12, tail[1] - s * R * 0.14 + c * R * 0.12, tail[0] - c * R * 0.14 + s * R * 0.12, tail[1] - s * R * 0.14 - c * R * 0.12])
        .fill(0x8a9894)
        .stroke({ width: 1.1, color: 0x3a4440 });
      g.poly(oval(cx, cy, R * 0.4, R * 0.13, rot, 12)).fill(0xa8b4b0).stroke({ width: 1.1, color: 0x3a4440 });
      g.poly(oval(cx - s * R * 0.03, cy + c * R * 0.03 - 0.6, R * 0.28, R * 0.05, rot, 10)).fill(0xd8e0dc);
    } else {
      g.poly(oval(cx, cy, R * 0.22, R * 0.15, rot, 10)).fill(CORK).stroke({ width: 1.1, color: INK });
    }
  }
}

/** A heap of net from above: a lumpy pile lit on the north-west, its folds and a few mesh lines, cork
 * floats caught along its edge. */
function netHeap(g: Graphics, x: number, y: number, s: number, color: number, rnd: () => number) {
  const pts = blob(x, y, s, 11, 0.3, rnd);
  g.poly(pts).fill(color).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly(blob(x - s * 0.18, y - s * 0.2, s * 0.55, 9, 0.3, rnd)).fill(shade(color, 1.16));
  for (let k = 0; k < 6; k++) {
    const a = rnd() * Math.PI * 2;
    const d = s * 0.55 * rnd();
    const [cx, cy] = [x + Math.cos(a) * d, y + Math.sin(a) * d];
    const b = rnd() * Math.PI;
    g.moveTo(cx - Math.cos(b) * s * 0.3, cy - Math.sin(b) * s * 0.3).quadraticCurveTo(cx + Math.sin(b) * s * 0.12, cy - Math.cos(b) * s * 0.12, cx + Math.cos(b) * s * 0.3, cy + Math.sin(b) * s * 0.3);
  }
  g.stroke({ width: 1.2, color: shade(color, 0.62), cap: 'round' });
  for (let i = 0; i < pts.length; i += 6) g.poly(oval(pts[i] * 0.92 + x * 0.08, pts[i + 1] * 0.92 + y * 0.08, s * 0.16, s * 0.11, rnd() * 3, 10)).fill(CORK).stroke({ width: 1.1, color: INK });
}

/** Wicker lobster pot from above: a rounded box, its hoops across, the neck's mouth dark on top. */
function lobsterPot(g: Graphics, x: number, y: number, w: number, h: number, rot: number) {
  const [c, s] = [Math.cos(rot), Math.sin(rot)];
  const P = (u: number, v: number) => [x + u * c - v * s, y + u * s + v * c];
  g.poly(oval(x, y, w, h, rot, 16)).fill(WICKER).stroke({ width: LINE, color: INK, join: 'round' });
  for (const u of [-0.5, 0, 0.5]) {
    const [a, b] = [P(u * w, -h * 0.92), P(u * w, h * 0.92)];
    g.moveTo(a[0], a[1]).lineTo(b[0], b[1]);
  }
  g.stroke({ width: 1.4, color: shade(WICKER, 0.66) });
  const [lx, ly] = P(-w * 0.3, -h * 0.35);
  g.poly(oval(lx, ly, w * 0.35, h * 0.3, rot, 10)).fill({ color: shade(WICKER, 1.18), alpha: 0.8 });
  g.poly(oval(x, y, w * 0.24, h * 0.3, rot, 10)).fill(0x4a3420).stroke({ width: 1.2, color: shade(WICKER, 0.6) });
}

/** Rowboat, bow toward +x (about 13 ft): an open clinker boat, three thwarts, a ring at the stem; bare wood
 * with its oars shipped (v0), a slate-teal hull with a rope coiled in the bow (v1), weathered grey with
 * bilge water aft and one oar left askew (v2), a basket of the day's catch (v3). */
export function rowboat(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  const [L, B, tr] = [6.4 * FT, 2.25 * FT, 0.6];
  const [Li, Bi] = boatShell(g, L, B, tr, tones, v === 1 ? BOAT_PAINT[0] : null, [-0.62, 0.02, 0.5]);
  if (v === 2) {
    g.poly(oval(-Li * 0.38, Bi * 0.08, Li * 0.2, Bi * 0.5, 0, 14)).fill({ color: 0x6f9298, alpha: 0.75 });
    g.poly(oval(-Li * 0.42, -Bi * 0.06, Li * 0.06, Bi * 0.12, 0, 8)).fill({ color: 0xe2eeea, alpha: 0.7 });
    oar(g, -Li * 0.72, Bi * 0.3, Li * 0.64, -Bi * 0.22, 0xb4aa94);
  } else {
    if (v === 3) basketOf(g, -Li * 0.3, Bi * 0.05, 0.78 * FT, 'fish', rnd);
    for (const f of [-0.4, 0.4]) oar(g, -Li * 0.8, f * Bi, Li * 0.64, f * Bi * 0.86);
    if (v === 1) ropeCoil(g, Li * 0.72, 0, 0.5 * FT);
  }
  g.circle(L - 0.55 * FT, 0, 0.17 * FT).stroke({ width: 3.2, color: INK }).circle(L - 0.55 * FT, 0, 0.17 * FT).stroke({ width: 1.5, color: IRON[3] });
}

/** Fishing boat, bow toward +x (about 22 ft): half-decked, the stub of its mast stepped at the foredeck,
 * a rudder hung on the transom and its tiller; a net heaped in the well and a basket of fish (v0), red
 * ochre with crates and a coil of rope (v1), weathered grey with lobster pots (v2), the sail furled on its
 * boom laid along the boat (v3). */
export function fishingBoat(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 2 ? GREY_WOOD : WOOD;
  const [L, B, tr] = [10.4 * FT, 3.7 * FT, 0.55];
  // The rudder (its blade astern of the transom).
  g.roundRect(-L - 1.0 * FT, -0.24 * FT, 1.5 * FT, 0.48 * FT, 0.2 * FT).fill(tones[1]).stroke({ width: LINE, color: INK, join: 'round' });
  const [Li, Bi] = boatShell(g, L, B, tr, tones, v === 1 ? BOAT_PAINT[1] : null, [-0.5, 0.12]);
  // Foredeck and stern deck, planked fore and aft.
  const deck = (u0: number, u1: number) => {
    const pts: number[] = [];
    const [Ld, Bd] = [Li + 0.3 * FT, Bi + 0.25 * FT];
    for (let i = 0; i <= 10; i++) {
      const u = u0 + ((u1 - u0) * i) / 10;
      pts.push(u * Ld, -beamAt(u, Bd, tr));
    }
    for (let i = 10; i >= 0; i--) {
      const u = u0 + ((u1 - u0) * i) / 10;
      pts.push(u * Ld, beamAt(u, Bd, tr));
    }
    g.poly(pts).fill(tones[2]).stroke({ width: 1.2, color: INK, join: 'round' });
    for (const f of [-0.5, -0.17, 0.17, 0.5]) {
      let [a, b] = [u0, u1];
      while (b > a && beamAt(b, Bd, tr) < Math.abs(f) * Bd + 2) b -= 0.01;
      while (a < b && beamAt(a, Bd, tr) < Math.abs(f) * Bd + 2) a += 0.01;
      if (b > a) g.moveTo(a * Ld + 1, f * Bd).lineTo(b * Ld - 1, f * Bd);
    }
    g.stroke({ width: 1.1, color: shade(tones[1], 0.8), alpha: 0.8 });
  };
  deck(0.5, 1);
  deck(-1, -0.8);
  // A shadow on the well's floor under the foredeck's edge, to the south-east.
  g.rect(0.5 * (Li + 0.3 * FT) + 1.5, -Bi * 0.62, 0.32 * FT, Bi * 1.3).fill({ color: 0x000000, alpha: 0.18 });
  // The tiller over the stern deck.
  g.moveTo(-L + 0.2 * FT, 0).lineTo(-L + 3.2 * FT, 0.35 * FT).stroke({ width: 0.3 * FT + 2.6, color: INK, cap: 'round' });
  g.moveTo(-L + 0.2 * FT, 0).lineTo(-L + 3.2 * FT, 0.35 * FT).stroke({ width: 0.3 * FT, color: tones[2], cap: 'round' });
  if (v === 0) {
    netHeap(g, -Li * 0.22, Bi * 0.05, 1.7 * FT, NET[0], rnd);
    basketOf(g, Li * 0.32, 0, 0.82 * FT, 'fish', rnd);
  } else if (v === 1) {
    crate(g, -Li * 0.26, -Bi * 0.3, 0.62 * FT, 0.12, true);
    crate(g, -Li * 0.18, Bi * 0.34, 0.56 * FT, -0.2, false);
    ropeCoil(g, Li * 0.32, 0.1 * FT, 0.7 * FT);
  } else if (v === 2) {
    lobsterPot(g, -Li * 0.25, -Bi * 0.26, 0.85 * FT, 0.6 * FT, 0.15);
    lobsterPot(g, -Li * 0.2, Bi * 0.34, 0.8 * FT, 0.58 * FT, -0.25);
    ropeCoil(g, Li * 0.3, 0, 0.62 * FT);
  }
  // The mast's stub, stepped through the foredeck in its partners.
  const mx = 0.56 * Li;
  bevel(g, mx - 0.62 * FT, -0.62 * FT, mx + 0.62 * FT, 0.62 * FT, 1.6, tones, LINE);
  if (v === 3) {
    // The sail furled on its boom, laid aft along the boat, lashed every few feet.
    const [x0, x1, w] = [mx - 0.3 * FT, -Li * 0.78, 0.55 * FT];
    g.roundRect(x1 - 0.5 * FT, -0.14 * FT, x0 - x1 + 0.6 * FT, 0.28 * FT, 0.1 * FT).fill(tones[1]).stroke({ width: LINE, color: INK });
    const roll: number[] = [];
    const n = 9;
    for (let i = 0; i <= n; i++) roll.push(x0 + ((x1 - x0) * i) / n, -w * (0.75 + 0.25 * Math.sin(i * 1.7)) - 0.15 * FT);
    for (let i = n; i >= 0; i--) roll.push(x0 + ((x1 - x0) * i) / n, w * (0.75 + 0.25 * Math.cos(i * 1.3)) - 0.15 * FT);
    g.poly(roll).fill(0xcfc2a0).stroke({ width: OUT, color: INK, join: 'round' });
    g.roundRect(x1 + 0.3 * FT, -w * 0.72 - 0.15 * FT, x0 - x1 - 0.6 * FT, w * 0.55, w * 0.25).fill(0xe6dcc2);
    for (let i = 1; i < 5; i++) {
      const x = x0 + ((x1 - x0) * i) / 5;
      g.moveTo(x, -w - 0.15 * FT).lineTo(x + 2, w - 0.1 * FT);
    }
    g.stroke({ width: 1.8, color: shade(ROPE, 0.6), cap: 'round' });
    basketOf(g, -Li * 0.2, Bi * 0.6, 0.62 * FT, 'fish', rnd);
  }
  g.circle(mx, 0, 0.78 * FT).fill(tones[1]).stroke({ width: OUT, color: INK });
  g.circle(mx - 1, -1, 0.56 * FT).fill(shade(tones[2], 1.06)).stroke({ width: 1.6, color: tones[0] });
  g.circle(mx, 0, 0.26 * FT).fill(tones[0]).stroke({ width: LINE, color: INK });
  g.circle(mx - 1.5, -1.5, 0.1 * FT).fill(tones[3]);
}

/** Bollard on a quay, seen from straight above (about 1.2 ft across): a stout post's domed head, lit on the
 * north-west and shaded to the south-east, the flared foot peeping out only on the shaded side (its height
 * told by that and its shadow): cast iron with a lit cap (v0); the same with a mooring line taken a turn
 * round it and running off east (v1); a weathered timber post, its top checked, a rope turn round it (v2);
 * dressed stone, an iron ring by its foot (v3). */
export function bollard(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  void rnd;
  const R = 0.5 * FT;
  const t = v === 2 ? GREY_WOOD : v === 3 ? STONE[0] : IRON;
  const rope = (draw: () => void, w: number) => {
    draw();
    g.stroke({ width: w + 2.4, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color: ROPE, cap: 'round', join: 'round' });
  };
  // Rope tail running off to the frame edge (v1 east, v2 south-west).
  const tail = v === 1 ? 0 : v === 2 ? Math.PI * 0.8 : null;
  if (tail !== null) {
    const [c, s] = [Math.cos(tail), Math.sin(tail)];
    rope(() => g.moveTo(c * R * 1.25, s * R * 1.25).quadraticCurveTo(c * 1.5 * R + s * 4, s * 1.5 * R - c * 4, c * 2.3 * R, s * 2.3 * R - c * 3), 3);
  }
  // The flared foot on the shaded side, then the neck wrapped in rope (a pale ring outside the cap).
  g.circle(R * 0.2, R * 0.2, R * 1.12).fill(shade(t[0], 0.85)).stroke({ width: LINE, color: INK });
  if (v !== 3) {
    g.circle(0, 0, R * 1.3).fill(ROPE).stroke({ width: LINE, color: INK });
    g.circle(0, 0, R * 1.14).stroke({ width: 1.2, color: shade(ROPE, 0.6) });
    g.circle(-R * 0.1, -R * 0.1, R * 1.22).stroke({ width: 1.3, color: shade(ROPE, 1.15), alpha: 0.8 });
  }
  // The mushroom head: a wide flat cap with a smaller raised centre, one lit NW crescent.
  g.circle(0, 0, R).fill(t[1]).stroke({ width: LINE, color: INK });
  g.circle(R * 0.1, R * 0.1, R * 0.62).fill(shade(t[1], 0.88)).stroke({ width: 1.2, color: t[0] });
  g.circle(-R * 0.06, -R * 0.06, R * 0.56).fill(t[2]);
  g.moveTo(-R * 0.82, R * 0.1).arc(0, 0, R * 0.84, Math.PI * 0.95, Math.PI * 1.55).stroke({ width: 1.8, color: t[3], cap: 'round' });
  if (v === 2) g.moveTo(-R * 0.3, -R * 0.1).lineTo(R * 0.1, R * 0.05).lineTo(R * 0.3, -R * 0.25).stroke({ width: 1.1, color: t[0], cap: 'round', join: 'round' });
  if (v === 3) {
    g.circle(R * 1.15, R * 1.05, R * 0.38).stroke({ width: 3.4, color: INK });
    g.circle(R * 1.15, R * 1.05, R * 0.38).stroke({ width: 1.8, color: IRON[2] });
  }
}

/** Dock crane from straight above, its jib out toward +x (about 22 ft over all): a round timber turntable on
 * footing stones with the square post top in its middle; one long tapered jib out over the water, iron-banded,
 * a block at its tip and the rope dropping to the load; at the back a treadwheel, a long slatted drum on
 * its frame, its rope running to the post. Under the tip a barrel in a sling (v0), a crate in a net (v1),
 * sacks in a cargo net (v2), or, weathered grey, the hook hanging empty (v3). */
export function dockCrane(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 3 ? GREY_WOOD : WOOD;
  const px = -2.6 * FT;
  const ink2 = (draw: () => void, w: number, color: number) => {
    draw();
    g.stroke({ width: w + 2.4, color: INK, cap: 'round', join: 'round' });
    draw();
    g.stroke({ width: w, color, cap: 'round', join: 'round' });
  };
  // Footing stones peeping out under the turntable, then the turntable: a disc of planks, lit on the NW.
  for (const a of [0.8, 2.4, 3.9, 5.5]) stone(g, px + Math.cos(a) * 2.7 * FT, Math.sin(a) * 2.7 * FT, 0.65 * FT, STONE[1], rnd, 7);
  const T = 2.7 * FT;
  g.circle(px, 0, T).fill(tones[1]).stroke({ width: OUT, color: INK });
  g.circle(px - 2, -2, T * 0.86).fill(shade(tones[2], 0.95));
  for (let k = -2; k <= 2; k++) {
    const y = k * 0.95 * FT;
    const hw = Math.sqrt(Math.max(0, T * T - y * y)) - 2;
    g.moveTo(px - hw, y + 0.47 * FT).lineTo(px + hw, y + 0.47 * FT);
  }
  g.stroke({ width: 1.3, color: tones[0], alpha: 0.75 });
  g.circle(px, 0, T * 0.93).stroke({ width: 2.4, color: IRON[1] });
  // The treadwheel at the back: a long drum (its axle along x) on two trestles, slats across its tread.
  const [wx, wl, ww] = [-8.0 * FT, 4.2 * FT, 1.6 * FT];
  for (const sx of [-1, 1]) bevel(g, wx + sx * ww - 0.35 * FT, -1.1 * FT, wx + sx * ww + 0.35 * FT, 1.1 * FT, 1.5, tones, LINE);
  g.poly([wx - ww + 0.3 * FT, -wl, wx + ww - 0.3 * FT, -wl + 1, wx + ww - 0.3 * FT, wl, wx - ww + 0.3 * FT, wl - 1]).fill(shade(tones[0], 0.9)).stroke({ width: OUT, color: INK, join: 'round' });
  for (let y = -wl + 0.5 * FT; y < wl - 0.2 * FT; y += 0.62 * FT) g.rect(wx - ww + 0.55 * FT, y, ww * 2 - 1.1 * FT, 0.32 * FT).fill(shade(tones[2], 0.9 + 0.15 * (y < 0 ? 1 : 0)));
  for (const sx of [-1, 1]) g.moveTo(wx + sx * (ww - 0.42 * FT), -wl + 2).lineTo(wx + sx * (ww - 0.42 * FT), wl - 2);
  g.stroke({ width: 0.24 * FT, color: tones[1] });
  for (const sx of [-1, 1]) g.circle(wx + sx * (ww + 0.05 * FT), 0, 0.3 * FT).fill(IRON[2]).stroke({ width: LINE, color: INK });
  // Its rope, wound off the axle to the post.
  ink2(() => g.moveTo(wx + ww + 0.3 * FT, 0.15 * FT).quadraticCurveTo(px - 2.2 * FT, 0.5 * FT, px - 0.8 * FT, 0.2 * FT), 2, ROPE);
  // The jib: one long timber tapering out to its tip, an iron band or two, a lit north edge.
  const tx = 9.4 * FT;
  const [w0, w1] = [0.68 * FT, 0.38 * FT];
  const j = [px - 1.4 * FT, -w0, tx, -w1, tx, w1, px - 1.4 * FT, w0];
  g.poly(j).fill(tones[1]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly([px - 1.4 * FT + 2, -w0 + 1.5, tx - 2, -w1 + 1.2, tx - 2, -w1 * 0.2, px - 1.4 * FT + 2, -w0 * 0.2]).fill(tones[2]);
  for (const f of [0.35, 0.7]) {
    const x = px + (tx - px) * f;
    const w = w0 + (w1 - w0) * ((x - px + 1.4 * FT) / (tx - px + 1.4 * FT));
    g.rect(x - 0.12 * FT, -w, 0.24 * FT, w * 2).fill(IRON[1]).stroke({ width: 1, color: INK });
  }
  // The post top over it all at the turntable's middle, its end grain.
  bevel(g, px - 0.75 * FT, -0.75 * FT, px + 0.75 * FT, 0.75 * FT, 2.2, tones, OUT);
  g.circle(px, 0, 0.32 * FT).stroke({ width: 1.1, color: tones[0], alpha: 0.8 });
  // The block at the tip; the rope from it down to the load hanging a little to the south.
  const [lx, ly] = [tx + 0.1 * FT, 1.0 * FT];
  ink2(() => g.moveTo(tx - 0.1 * FT, 0).quadraticCurveTo(tx + 0.25 * FT, ly * 0.4, lx, ly), 1.8, ROPE);
  const sling = (R: number) => {
    for (const a of [0.6, 2.2, 3.7, 5.3]) g.moveTo(lx, ly).lineTo(lx + Math.cos(a) * R, ly + Math.sin(a) * R);
    g.stroke({ width: 4, color: INK, cap: 'round' });
    for (const a of [0.6, 2.2, 3.7, 5.3]) g.moveTo(lx, ly).lineTo(lx + Math.cos(a) * R, ly + Math.sin(a) * R);
    g.stroke({ width: 2, color: ROPE, cap: 'round' });
  };
  if (v === 0) {
    barrel(g, lx, ly, 1.0 * FT, rnd);
    sling(0.95 * FT);
  } else if (v === 1) {
    crate(g, lx, ly, 0.9 * FT, 0.25, true);
    sling(1.2 * FT);
  } else if (v === 2) {
    for (const [dx, dy, a] of [
      [-0.45, -0.4, 0.3],
      [0.5, -0.25, -0.4],
      [0, 0.5, 1.2],
    ]) {
      const [cx, cy] = [lx + dx * FT, ly + dy * FT];
      g.poly(oval(cx, cy, 0.72 * FT, 0.5 * FT, a, 14)).fill(0xc8b080).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(oval(cx - 2, cy - 2, 0.4 * FT, 0.25 * FT, a, 10)).fill(0xe0cca0);
    }
    g.poly(oval(lx, ly, 1.15 * FT, 0.95 * FT, 0, 16)).stroke({ width: 2.2, color: shade(NET[0], 0.7) });
  } else {
    ink2(() => g.moveTo(lx, ly).arc(lx, ly + 0.3 * FT, 0.3 * FT, -Math.PI / 2, Math.PI * 0.9), 2.2, IRON[3]);
  }
  g.roundRect(tx - 0.45 * FT, -0.42 * FT, 0.9 * FT, 0.84 * FT, 0.25 * FT).fill(tones[2]).stroke({ width: LINE, color: INK });
  g.circle(tx, 0, 0.16 * FT).fill(IRON[1]);
}

/** Net rack, along x (10 ft), seen from straight above: a rail between two round post tops, a net thrown
 * over the rail to dry and spilling to the ground on both sides in a loose, wavy-edged sheet; the fold along
 * the rail paler, the south side in shade, the mesh a few toned crossing lines, cork floats as dots along its
 * edges; one brown net (v0), a tarred net and a green one side by side (v1), a pale hemp net over part of the
 * rail with the rest heaped at the foot (v2), weathered grey posts, a basket of floats by them (v3). */
export function netRack(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  const tones = v === 3 ? GREY_WOOD : WOOD;
  const X = 4.3 * FT;
  const nets: [number, number, number][] = [[[-3.8, 3.8, NET[0]]], [[-3.9, -0.1, NET[1]], [0.15, 3.9, NET[2]]], [[-3.8, 1.1, NET[3]]], [[-3.8, 3.8, 0x857a58]]][v] as [number, number, number][];
  // The rail and posts first: the net lies over the rail; its ends show past the net.
  g.moveTo(-X - 0.3 * FT, 0).lineTo(X + 0.3 * FT, 0).stroke({ width: 0.34 * FT + 2.8, color: INK, cap: 'round' });
  g.moveTo(-X - 0.3 * FT, 0).lineTo(X + 0.3 * FT, 0).stroke({ width: 0.34 * FT, color: tones[2], cap: 'round' });
  for (const sx of [-1, 1]) {
    g.circle(sx * X, 0, 0.42 * FT).fill(tones[1]).stroke({ width: LINE, color: INK });
    g.circle(sx * X - 1, -1, 0.26 * FT).fill(tones[2]);
    g.circle(sx * X - 1, -1, 0.12 * FT).stroke({ width: 1, color: tones[0], alpha: 0.7 });
  }
  if (v === 3) basketOf(g, 2.9 * FT, 2.75 * FT, 0.72 * FT, 'floats', rnd);
  for (const [a, b, color] of nets) {
    const [x0, x1] = [a * FT, b * FT];
    const W = (x1 - x0) / FT;
    // The sheet's two ground edges: wavy, lying a little further out in the middle of each drop.
    const edge = (sy: number): number[] => {
      const pts: number[] = [];
      const n = Math.max(4, Math.round(W * 1.6));
      for (let i = 0; i <= n; i++) {
        const t = i / n;
        const x = x0 + (x1 - x0) * t;
        const sag = Math.sin(Math.PI * t) * 0.5 + (rnd() - 0.5) * 0.7;
        pts.push(x + (rnd() - 0.5) * 3, sy * (1.95 + sag) * FT);
      }
      return pts;
    };
    const north = edge(-1);
    const south = edge(1);
    const half = (e: number[]): number[] => {
      const pts = [x0 - 2, 0];
      pts.push(...e);
      pts.push(x1 + 2, 0);
      return pts;
    };
    g.poly(half(north)).fill(shade(color, 1.08));
    g.poly(half(south)).fill(shade(color, 0.8));
    // Mesh: a few crossing lines, toned, kept inside the sheet.
    const mesh = shade(color, 0.58);
    const lim = 1.6 * FT;
    for (let x = x0 - lim * 2; x < x1; x += 0.85 * FT) {
      const [ax, bx] = [Math.max(x, x0 + 3), Math.min(x + lim * 2, x1 - 3)];
      if (bx <= ax) continue;
      for (const d of [-1, 1]) g.moveTo(ax, d * (ax - x - lim)).lineTo(bx, d * (bx - x - lim));
    }
    g.stroke({ width: 1, color: mesh, alpha: 0.6, cap: 'round' });
    // Folds where it drapes: a few dark creases from the rail out to the edge, on both sides.
    for (const e of [north, south]) for (let i = 2; i < e.length / 2 - 1; i += 3) g.moveTo(e[i * 2] + (rnd() - 0.5) * 6, 0).lineTo(e[i * 2], e[i * 2 + 1] * 0.85);
    g.stroke({ width: 2.2, color: shade(color, 0.5), alpha: 0.7, cap: 'round' });
    // The fold over the rail: a pale ridge, its shaded south lip.
    g.moveTo(x0, 0).lineTo(x1, 0).stroke({ width: 0.3 * FT, color: shade(color, 1.3), cap: 'round' });
    g.moveTo(x0, 0.22 * FT).lineTo(x1, 0.22 * FT).stroke({ width: 1.3, color: shade(color, 0.55), cap: 'round' });
    // Soft edges in the net's own dark, then the cork floats along both edges.
    for (const e of [north, south]) {
      g.moveTo(x0 - 2, 0);
      for (let i = 0; i < e.length; i += 2) g.lineTo(e[i], e[i + 1]);
      g.lineTo(x1 + 2, 0).stroke({ width: LINE, color: shade(color, 0.45), join: 'round', cap: 'round' });
    }
    for (const e of [north, south]) for (let i = 1; i < e.length / 2 - 1; i += 2) g.circle(e[i * 2], e[i * 2 + 1] * 0.94, 0.17 * FT).fill(CORK).stroke({ width: 1.1, color: INK });
  }
  if (v === 2) netHeap(g, 2.75 * FT, 1.5 * FT, 1.2 * FT, NET[3], rnd);
}

/** Buoy afloat, from above, ripples round it: a can buoy painted red with a pale band and a ring on top
 * for its chain (v0), a tarred cask on its side, hooped, its chain going down (v1), a raft of corks with a
 * stick and a red pennant (v2), a pale ochre cone with a dark band under an iron cross top mark (v3). */
export function buoy(g: Graphics, r: number, rnd: () => number, v: number) {
  void r;
  // Ripples on the water: soft on purpose, no ink.
  for (const [k, a] of [
    [1.0, 0.5],
    [1.3, 0.3],
  ])
    g.poly(oval(0.12 * FT, 0.14 * FT, 1.55 * FT * k, 1.3 * FT * k, 0.2, 22)).stroke({ width: 1.6, color: 0xe2eeea, alpha: a });
  g.poly(oval(0.18 * FT, 0.2 * FT, 1.3 * FT, 1.1 * FT, 0, 18)).fill({ color: 0x1e3438, alpha: 0.22 });
  const R = 1.05 * FT;
  if (v === 1) {
    for (let k = 0; k < 4; k++) g.circle(1.35 * FT + k * 0.32 * FT, 0.1 * FT, 0.13 * FT).stroke({ width: 1.4, color: IRON[2], alpha: 0.9 - k * 0.2 });
    const tar = [0x4e3e2e, 0x6e5640, 0x8e7458, 0xb0957a];
    g.roundRect(-1.35 * FT, -0.85 * FT, 2.7 * FT, 1.7 * FT, 0.75 * FT).fill(tar[1]).stroke({ width: OUT, color: INK });
    g.roundRect(-1.2 * FT, -0.72 * FT, 2.4 * FT, 0.6 * FT, 0.3 * FT).fill(tar[2]);
    g.roundRect(-1.0 * FT, -0.62 * FT, 2.0 * FT, 0.18 * FT, 0.09 * FT).fill(tar[3]);
    for (const x of [-0.8, -0.3, 0.3, 0.8]) g.moveTo(x * FT, -0.82 * FT).lineTo(x * FT, 0.82 * FT);
    g.stroke({ width: 2, color: IRON[1] });
    g.circle(1.3 * FT, 0, 0.16 * FT).stroke({ width: 1.8, color: IRON[2] });
    return;
  }
  if (v === 2) {
    for (const [x, y, a] of [
      [-0.4, -0.25, 0.4],
      [0.45, -0.1, -0.3],
      [0, 0.45, 1.1],
    ]) {
      g.poly(oval(x * FT, y * FT, 0.7 * FT, 0.48 * FT, a, 14)).fill(0xc29a62).stroke({ width: LINE, color: INK, join: 'round' });
      g.poly(oval(x * FT - 1.5, y * FT - 1.5, 0.42 * FT, 0.25 * FT, a, 10)).fill(CORK);
    }
    g.poly([0.1 * FT, -0.05 * FT, 1.35 * FT, -0.45 * FT, 0.95 * FT, 0.15 * FT, 1.3 * FT, 0.55 * FT]).fill(0x9a4a3c).stroke({ width: 1.3, color: INK, join: 'round' });
    g.circle(0, 0, 0.22 * FT).fill(WOOD[1]).stroke({ width: LINE, color: INK });
    g.circle(-0.7, -0.7, 0.11 * FT).fill(WOOD[3]);
    void rnd;
    return;
  }
  const [dark, lit, band] = v === 0 ? [0x7e3e32, 0x9a4a3c, 0xe2d8c2] : [0x9a8450, 0xbea468, 0x3a3630];
  g.circle(0, 0, R).fill(dark).stroke({ width: LINE, color: INK });
  g.circle(-1.5, -1.5, R * 0.84).fill(lit);
  g.circle(-0.6, -0.6, R * 0.66).stroke({ width: 0.22 * FT, color: band });
  g.poly(oval(-R * 0.42, -R * 0.4, R * 0.22, R * 0.13, -0.7, 10)).fill({ color: 0xffffff, alpha: 0.35 });
  if (v === 0) {
    g.circle(-0.6, -0.6, R * 0.24).stroke({ width: 3.6, color: INK }).circle(-0.6, -0.6, R * 0.24).stroke({ width: 1.8, color: IRON[3] });
  } else {
    for (const a of [0.4, 0.4 + Math.PI / 2]) {
      const [c, s] = [Math.cos(a) * R * 0.5, Math.sin(a) * R * 0.5];
      g.moveTo(-0.6 - c, -0.6 - s).lineTo(-0.6 + c, -0.6 + s);
    }
    g.stroke({ width: 4.2, color: INK, cap: 'round' });
    for (const a of [0.4, 0.4 + Math.PI / 2]) {
      const [c, s] = [Math.cos(a) * R * 0.5, Math.sin(a) * R * 0.5];
      g.moveTo(-0.6 - c, -0.6 - s).lineTo(-0.6 + c, -0.6 + s);
    }
    g.stroke({ width: 2, color: IRON[2], cap: 'round' });
  }
}
