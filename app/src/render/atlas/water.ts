import { Graphics } from 'pixi.js';
import { INK, blob, oval, shade } from './common';
import { boughs, leaf, lit, whorl } from './plants';
import { STONE, moss, stone } from './stones';
import { footGrass, polyR } from './street';
import { bevel } from './yard';
import { fade } from './hazards';

// Parks and gardens.

/** `n` points round (0, 0) at radius `R(a)`, from angle `ph`. */
function around(n: number, R: (a: number) => number, ph = 0): number[] {
  const pts: number[] = [];
  for (let i = 0; i < n; i++) {
    const a = ph + (i / n) * Math.PI * 2;
    pts.push(Math.cos(a) * R(a), Math.sin(a) * R(a));
  }
  return pts;
}

/** A closed curve through the middles of a polygon's edges, its corners pulling it round (a smoothed
 * outline); fill or stroke it after. */
export function smooth(g: Graphics, pts: number[]) {
  const n = pts.length / 2;
  const mid = (i: number): [number, number] => {
    const [a, b] = [i % n, (i + 1) % n];
    return [(pts[a * 2] + pts[b * 2]) / 2, (pts[a * 2 + 1] + pts[b * 2 + 1]) / 2];
  };
  g.moveTo(...mid(n - 1));
  for (let i = 0; i < n; i++) g.quadraticCurveTo(pts[i * 2], pts[i * 2 + 1], ...mid(i));
  g.closePath();
}

/** The band between two outlines of as many points (a rim seen from above), each piece lit as it faces the
 * light, both outlines inked. */
export function rimBand(g: Graphics, outer: number[], inner: number[], tone: number, ink = 2.2, light = 0.2) {
  g.poly(outer).fill(tone);
  const n = outer.length / 2;
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const mid = Math.atan2(outer[i * 2 + 1] + outer[j * 2 + 1], outer[i * 2] + outer[j * 2]);
    g.poly([outer[i * 2], outer[i * 2 + 1], outer[j * 2], outer[j * 2 + 1], inner[j * 2], inner[j * 2 + 1], inner[i * 2], inner[i * 2 + 1]]).fill(shade(tone, 0.9 + light * lit(mid)));
  }
  g.poly(outer).stroke({ width: ink, color: INK, join: 'round' });
  g.poly(inner).stroke({ width: Math.max(1.2, ink * 0.6), color: INK, join: 'round' });
}

/** Water seen from above inside the outline `R(a)` (a basin, a pond): deeper toward the middle, the shadow
 * of its rim or bank along the NW side, two ripples and a glint; murky water green and still. */
function water(g: Graphics, R: (a: number) => number, rnd: () => number, murky = false) {
  const t = murky ? [0x2e4a2a, 0x4a6a3c, 0x6e8a58] : [0x22566a, 0x3a7f8a, 0x6ab0a8];
  const s = (R(0) + R(Math.PI / 2) + R(Math.PI) + R(Math.PI * 1.5)) / 4;
  g.poly(around(64, R)).fill(t[1]);
  g.poly(around(48, (a) => R(a) * 0.66).map((p) => p + s * 0.05)).fill({ color: t[0], alpha: 0.4 });
  g.poly(around(36, (a) => R(a) * 0.36).map((p) => p + s * 0.08)).fill({ color: t[0], alpha: 0.35 });
  // The rim's shadow: a crescent inside its NW side, widest facing the light.
  const crescent: number[] = [];
  const [a0, a1] = [Math.PI * 0.62, Math.PI * 1.88];
  for (let i = 0; i <= 24; i++) {
    const a = a0 + ((a1 - a0) * i) / 24;
    crescent.push(Math.cos(a) * R(a), Math.sin(a) * R(a));
  }
  for (let i = 24; i >= 0; i--) {
    const a = a0 + ((a1 - a0) * i) / 24;
    const k = Math.max(0, R(a) - s * 0.13 * Math.sin(((a - a0) / (a1 - a0)) * Math.PI));
    crescent.push(Math.cos(a) * k, Math.sin(a) * k);
  }
  g.poly(crescent).fill({ color: 0x123848, alpha: murky ? 0.3 : 0.4 });
  if (murky) {
    for (let i = 0; i < 7; i++) {
      const a = rnd() * Math.PI * 2;
      const d = R(a) * 0.75 * Math.sqrt(rnd());
      g.poly(blob(Math.cos(a) * d, Math.sin(a) * d, s * (0.05 + 0.05 * rnd()), 7, 0.5, rnd)).fill({ color: 0x8a9a52, alpha: 0.7 });
    }
    return;
  }
  for (let i = 0; i < 2; i++) {
    const a = rnd() * Math.PI * 2;
    const d = R(a) * (0.3 + 0.3 * rnd());
    const [x, y, w] = [Math.cos(a) * d, Math.sin(a) * d, s * (0.12 - i * 0.03)];
    g.moveTo(x - w, y).quadraticCurveTo(x, y - w * 0.35, x + w, y).stroke({ width: 1.5, color: t[2], alpha: 0.9, cap: 'round' });
  }
  const ga = Math.PI * 1.25;
  g.poly(oval(Math.cos(ga) * R(ga) * 0.5, Math.sin(ga) * R(ga) * 0.5, s * 0.1, s * 0.045, -0.5, 10)).fill({ color: 0xe6f4ec, alpha: 0.75 });
}

/** Rings where falling water meets the pool below: short white arcs round a circle of radius `R`. */
function dripRing(g: Graphics, R: number, n: number, rnd: () => number) {
  for (let i = 0; i < n; i++) {
    const a = ((i + rnd() * 0.5) / n) * Math.PI * 2;
    g.moveTo(Math.cos(a) * R, Math.sin(a) * R).arc(0, 0, R, a, a + (Math.PI * 2 * 0.45) / n);
  }
  g.stroke({ width: 2, color: 0xe6f4ec, alpha: 0.75, cap: 'round' });
}

/** A jet from above: white spray thrown out every way from (x, y), droplets round it. */
function jet(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  for (let i = 0; i < 12; i++) {
    const a = (i / 12) * Math.PI * 2 + rnd() * 0.3;
    const l = s * (0.7 + 0.3 * rnd());
    g.moveTo(x + Math.cos(a) * s * 0.15, y + Math.sin(a) * s * 0.15).lineTo(x + Math.cos(a) * l, y + Math.sin(a) * l);
  }
  g.stroke({ width: 2.2, color: 0xf2fafa, alpha: 0.85, cap: 'round' });
  for (let i = 0; i < 8; i++) {
    const a = rnd() * Math.PI * 2;
    const d = s * (0.8 + 0.5 * rnd());
    g.circle(x + Math.cos(a) * d, y + Math.sin(a) * d, 1.3).fill({ color: 0xf2fafa, alpha: 0.8 });
  }
  g.circle(x, y, s * 0.28).fill(0xf6fcfc).stroke({ width: 1.2, color: 0x6ab0a8 });
}

/** A round stone bowl from above at (0, 0): its rim (outer radius `R`) lit toward the light, water inside. */
function bowl(g: Graphics, R: number, tones: readonly number[], rnd: () => number) {
  g.circle(R * 0.12, R * 0.14, R).fill({ color: 0x0e2a34, alpha: 0.35 });
  rimBand(g, around(36, () => R), around(36, () => R * 0.74), tones[2], 2);
  water(g, () => R * 0.74, rnd);
}

/** Fountain (a garden's; a square's big raised basin is drawn with the square): a round stone basin, an
 * upper bowl on a pedestal throwing up a jet (v0); a quatrefoil basin round an urn pouring four ways (v1);
 * three tiers in an octagonal basin (v2); an old one, its water gone low, still and green below a stained
 * band, its rim cracked and a stone come away, the bowl broken off its pedestal, moss (v3). */
export function fountain(g: Graphics, r: number, rnd: () => number, v: number) {
  const st = STONE[[0, 2, 0, 3][v]];
  const R0 = r * 0.93;
  const rim = r * 0.15;
  const shape = v === 1 ? (a: number) => R0 * (0.8 + 0.2 * Math.abs(Math.cos(2 * a))) : v === 2 ? polyR(R0, 8, Math.PI / 8) : () => R0;
  const inner = (a: number) => shape(a) - rim;
  rimBand(g, around(64, shape), around(64, inner), st[2], 2.5, v === 3 ? 0.4 : 0.2);
  // Joints across the rim: at the octagon's corners, the quatrefoil's cusps and lobes, else every tenth.
  const joints = v === 2 ? 8 : v === 1 ? 8 : 10;
  for (let i = 0; i < joints; i++) {
    const a = (v === 2 ? Math.PI / 8 : 0.2) + (i / joints) * Math.PI * 2;
    g.moveTo(Math.cos(a) * (inner(a) + 1.5), Math.sin(a) * (inner(a) + 1.5)).lineTo(Math.cos(a) * (shape(a) - 1.5), Math.sin(a) * (shape(a) - 1.5));
  }
  g.stroke(v === 3 ? { width: 2.2, color: shade(st[0], 0.6) } : { width: 1.3, color: shade(st[0], 0.85) });
  if (v === 3) {
    // Gone low: the basin's inside stained and dry down to the waterline, streaks down it, green water
    // lying low, and the rim's shadow falling deep across the NW side.
    g.poly(around(64, inner)).fill(st[1]);
    for (let i = 0; i < 14; i++) {
      const a = (i / 14) * Math.PI * 2 + rnd() * 0.3;
      const k = 0.88 + 0.05 * rnd();
      g.moveTo(Math.cos(a) * inner(a) * 0.99, Math.sin(a) * inner(a) * 0.99).lineTo(Math.cos(a) * inner(a) * k, Math.sin(a) * inner(a) * k);
    }
    g.stroke({ width: 1.6, color: st[0], alpha: 0.8, cap: 'round' });
    water(g, (a) => inner(a) * 0.8, rnd, true);
    g.poly(around(64, (a) => inner(a) * 0.8)).stroke({ width: 1.2, color: 0x3e3a2a, alpha: 0.8 });
    // The old waterline: an olive-brown stain round the basin a little above the water.
    g.poly(around(64, (a) => inner(a) * 0.85)).stroke({ width: 3, color: 0x6a5a2e });
    const shadow: number[] = [];
    for (let i = 0; i <= 24; i++) {
      const a = Math.PI * (0.62 + (1.26 * i) / 24);
      shadow.push(Math.cos(a) * inner(a), Math.sin(a) * inner(a));
    }
    for (let i = 24; i >= 0; i--) {
      const a = Math.PI * (0.62 + (1.26 * i) / 24);
      const k = inner(a) - r * 0.26 * Math.sin((i / 24) * Math.PI);
      shadow.push(Math.cos(a) * k, Math.sin(a) * k);
    }
    g.poly(shadow).fill({ color: 0x1a2418, alpha: 0.4 });
  } else water(g, inner, rnd);
  g.poly(around(64, inner)).stroke({ width: 1.5, color: INK, join: 'round' });
  if (v === 0) {
    g.circle(r * 0.06, r * 0.08, r * 0.38).fill({ color: 0x0e2a34, alpha: 0.25 });
    dripRing(g, r * 0.42, 9, rnd);
    bowl(g, r * 0.34, st, rnd);
    jet(g, 0, 0, r * 0.2, rnd);
  } else if (v === 1) {
    // Four streams arching out of the urn into the lobes, rings where they land.
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * Math.PI * 2;
      const [c, s] = [Math.cos(a), Math.sin(a)];
      const [x0, y0, x1, y1] = [c * r * 0.18, s * r * 0.18, c * r * 0.52, s * r * 0.52];
      const [mx, my] = [c * r * 0.36 - s * r * 0.04, s * r * 0.36 + c * r * 0.04];
      g.moveTo(x0, y0).quadraticCurveTo(mx, my, x1, y1).stroke({ width: 5, color: 0x5e9ea4, alpha: 0.7, cap: 'round' });
      g.moveTo(x0, y0).quadraticCurveTo(mx, my, x1, y1).stroke({ width: 2.4, color: 0xf2fafa, alpha: 0.9, cap: 'round' });
      g.circle(x1, y1, r * 0.06).stroke({ width: 1.5, color: 0xe6f4ec, alpha: 0.8 });
      g.circle(x1, y1, r * 0.1).stroke({ width: 1.2, color: 0xe6f4ec, alpha: 0.5 });
    }
    // The urn from above: its swell lit toward the light, the lip round its dark mouth, bubbling.
    g.circle(r * 0.05, r * 0.06, r * 0.27).fill({ color: 0x0e2a34, alpha: 0.3 });
    g.circle(0, 0, r * 0.26).fill(st[1]).stroke({ width: 2, color: INK });
    g.circle(-r * 0.05, -r * 0.05, r * 0.19).fill(st[2]);
    g.circle(0, 0, r * 0.17).fill(st[3]).stroke({ width: 1.5, color: INK });
    g.circle(0, 0, r * 0.11).fill(0x2a6070).stroke({ width: 1, color: shade(st[0], 0.7) });
    g.circle(-r * 0.02, -r * 0.02, r * 0.05).fill({ color: 0xf2fafa, alpha: 0.9 });
    // A lily pad in one lobe.
    const [lx, ly, ls] = [-r * 0.6, r * 0.08, r * 0.09];
    g.moveTo(lx, ly).arc(lx, ly, ls, 0.5, Math.PI * 2 - 0.1).closePath().fill(0x5e8a3a).stroke({ width: 1, color: 0x2a3a18 });
  } else if (v === 2) {
    g.circle(r * 0.08, r * 0.1, r * 0.5).fill({ color: 0x0e2a34, alpha: 0.2 });
    dripRing(g, r * 0.56, 11, rnd);
    bowl(g, r * 0.48, st, rnd);
    dripRing(g, r * 0.3, 7, rnd);
    bowl(g, r * 0.24, st.map((c) => shade(c, 1.05)), rnd);
    jet(g, 0, 0, r * 0.12, rnd);
  } else {
    // A stone gone from the rim (a dark gap, the stone lying outside), a crack, moss on the north rim,
    // brown leaves on the water; the pedestal still standing, broken off short, a piece of its bowl leaning
    // at its foot.
    const ga = Math.PI * 0.3;
    const gap = [ga - 0.2, ga + 0.2].flatMap((a) => [Math.cos(a) * (shape(a) - 1), Math.sin(a) * (shape(a) - 1)]);
    const gin = [ga + 0.2, ga - 0.2].flatMap((a) => [Math.cos(a) * (inner(a) + 1), Math.sin(a) * (inner(a) + 1)]);
    g.poly([...gap, ...gin]).fill(0x3e3a32).stroke({ width: 1.5, color: INK, join: 'round' });
    stone(g, Math.cos(ga) * r * 1.0, Math.sin(ga) * r * 1.0 - 4, r * 0.13, st, rnd, 6);
    const ca = Math.PI * 1.1;
    g.moveTo(Math.cos(ca) * inner(ca), Math.sin(ca) * inner(ca)).lineTo(Math.cos(ca + 0.05) * (inner(ca) + rim * 0.5), Math.sin(ca + 0.05) * (inner(ca) + rim * 0.5)).lineTo(Math.cos(ca - 0.02) * shape(ca), Math.sin(ca - 0.02) * shape(ca));
    g.stroke({ width: 1.6, color: INK, join: 'round' });
    moss(g, -r * 0.3, -r * 0.82, r * 0.16, rnd);
    moss(g, -r * 0.78, -r * 0.3, r * 0.12, rnd);
    for (let i = 0; i < 3; i++) {
      const a = Math.PI * (0.1 + i * 0.55) + rnd() * 0.3;
      leaf(g, Math.cos(a) * r * 0.48, Math.sin(a) * r * 0.48, rnd() * Math.PI * 2, r * 0.18, r * 0.06, [0x6a4a28, 0x7e5a32, 0x5e4426][i], 0.4);
    }
    // The pedestal: a round column broken off short, its shadow on the water, the east side in shade, the
    // break a jagged lit top.
    const pr = r * 0.22;
    g.poly(oval(pr * 0.7, pr * 0.8, pr * 1.1, pr * 0.8, 0.7, 14)).fill({ color: 0x0e1a14, alpha: 0.4 });
    g.circle(0, 0, pr).fill(st[1]).stroke({ width: 2.2, color: INK });
    g.moveTo(Math.cos(-Math.PI / 4) * pr, Math.sin(-Math.PI / 4) * pr).arc(0, 0, pr, -Math.PI / 4, Math.PI * 0.75).closePath().fill({ color: st[0], alpha: 0.8 });
    // The break: a jagged face, lit on its NW side, darker toward the SE, a crack across it.
    const top = Array.from({ length: 7 }, (_, i) => {
      const a = 0.3 + (i / 7) * Math.PI * 2;
      const k = pr * (i % 2 ? 0.46 + 0.06 * rnd() : 0.64 + 0.1 * rnd());
      return [Math.cos(a) * k - pr * 0.1, Math.sin(a) * k - pr * 0.1];
    }).flat();
    g.poly(top).fill(st[2]).stroke({ width: 1.4, color: INK, join: 'round' });
    g.poly(top.map((p) => p * 0.5 - pr * 0.1)).fill({ color: st[3], alpha: 0.45 });
    g.moveTo(-pr * 0.4, -pr * 0.12).lineTo(-pr * 0.1, -pr * 0.2).lineTo(pr * 0.06, -pr * 0.04).lineTo(pr * 0.28, pr * 0.02).stroke({ width: 1.3, color: shade(st[0], 0.7), join: 'round' });
    // A piece of the bowl lying at its SE foot: a curved slab of stone, its outer edge lit.
    const [bx, by, br] = [r * 0.32, r * 0.3, r * 0.2];
    const [s0, s1] = [Math.PI * 1.05, Math.PI * 1.7];
    g.moveTo(bx + Math.cos(s0) * br, by + Math.sin(s0) * br)
      .arc(bx, by, br, s0, s1)
      .lineTo(bx + Math.cos(s1) * br * 0.45, by + Math.sin(s1) * br * 0.45)
      .arc(bx, by, br * 0.45, s1, s0, true)
      .closePath()
      .fill(st[0])
      .stroke({ width: 1.8, color: INK, join: 'round' });
    g.moveTo(bx + Math.cos(s0 + 0.08) * br * 0.94, by + Math.sin(s0 + 0.08) * br * 0.94).arc(bx, by, br * 0.94, s0 + 0.08, s1 - 0.08).stroke({ width: 2.2, color: st[2], cap: 'round' });
    footGrass(g, -r * 0.88, r * 0.42, r * 0.14, rnd);
  }
}

/** A duck on the water at (x, y), heading `a`: its body, the wake spreading behind it, its head and bill; a
 * mallard drake or a white duck. */
function duck(g: Graphics, x: number, y: number, a: number, s: number, white: boolean) {
  const [c, sn] = [Math.cos(a), Math.sin(a)];
  const at = (u: number, w: number): [number, number] => [x + c * u - sn * w, y + sn * u + c * w];
  g.moveTo(...at(-s * 0.9, -s * 0.2)).lineTo(...at(-s * 2.6, -s * 1.1)).moveTo(...at(-s * 0.9, s * 0.2)).lineTo(...at(-s * 2.6, s * 1.1));
  g.stroke({ width: 1.5, color: 0xe6f4ec, alpha: 0.7, cap: 'round' });
  const body = white ? [0xd8d4c8, 0xf6f2e8] : [0x6e604c, 0x9a8a70];
  g.poly(oval(...at(-s * 0.1, 0), s, s * 0.6, a, 14)).fill(body[0]).stroke({ width: 1.5, color: INK });
  g.poly(oval(...at(-s * 0.2, -s * 0.08), s * 0.7, s * 0.36, a, 12)).fill(body[1]);
  g.poly([...at(s * 1.25, -s * 0.12), ...at(s * 1.7, 0), ...at(s * 1.25, s * 0.12)]).fill(0xe0a030).stroke({ width: 1, color: INK, join: 'round' });
  g.circle(...at(s * 0.95, 0), s * 0.38).fill(white ? 0xf6f2e8 : 0x2e6a3a).stroke({ width: 1.5, color: INK });
  if (!white) g.circle(...at(s * 0.62, 0), s * 0.36).stroke({ width: 1.2, color: 0xf6f2e8 });
}

/** Pond: a muddy bank fading into the ground, still water deeper toward the middle; lily pads with a flower
 * or two and stones on the bank (v0); reeds and cattails crowding one side (v1); a formal pool in a stone
 * coping lit along the north and west, koi under lily pads (v2); a duck pond, the ducks' wakes behind them
 * and a feather on the bank (v3). */
export function pond(g: Graphics, r: number, rnd: () => number, v: number) {
  const pad = (x: number, y: number, s: number, flower: number | null) => {
    const n = rnd() * Math.PI * 2;
    g.moveTo(x, y).arc(x, y, s, n + 0.4, n + Math.PI * 2 - 0.4).closePath().fill(0x5e8a3a).stroke({ width: 1.2, color: 0x2a3a18 });
    g.circle(x - s * 0.3, y - s * 0.3, s * 0.35).fill({ color: 0x86b052, alpha: 0.8 });
    if (flower !== null) {
      for (let p = 0; p < 6; p++) {
        const pa = (p / 6) * Math.PI * 2;
        g.poly(oval(x + Math.cos(pa) * s * 0.35, y + Math.sin(pa) * s * 0.35, s * 0.36, s * 0.16, pa, 8)).fill(shade(flower, 1 + 0.12 * lit(pa))).stroke({ width: 0.8, color: shade(flower, 0.55) });
      }
      g.circle(x, y, s * 0.16).fill(0xf2c94c);
    }
  };
  if (v === 2) {
    // A formal pool: the coping's blocks lit along the north and west, clear water, koi, lily pads.
    const [W, H, d] = [r * 0.9, r * 0.6, r * 0.13];
    bevel(g, -W, -H, W, H, d, STONE[1].map((c) => shade(c, 1.08)), 2.5);
    for (let x = -W + r * 0.32; x < W - d; x += r * 0.34) g.moveTo(x, -H + 1).lineTo(x, -H + d).moveTo(x + r * 0.15, H - d).lineTo(x + r * 0.15, H - 1);
    for (let y = -H + r * 0.3; y < H - d; y += r * 0.36) g.moveTo(-W + 1, y).lineTo(-W + d, y).moveTo(W - d, y + r * 0.1).lineTo(W - 1, y + r * 0.1);
    g.stroke({ width: 1.2, color: shade(STONE[1][0], 0.85) });
    const [w, h] = [W - d, H - d];
    water(g, (a) => Math.min(w / Math.max(1e-6, Math.abs(Math.cos(a))), h / Math.max(1e-6, Math.abs(Math.sin(a)))), rnd);
    g.rect(-w, -h, w * 2, h * 2).stroke({ width: 1.5, color: INK });
    for (const [x, y, a, c] of [
      [-r * 0.35, -r * 0.1, 0.3, 0xe8742a],
      [r * 0.1, r * 0.22, Math.PI + 0.4, 0xf2efe6],
      [r * 0.45, -r * 0.18, Math.PI * 0.85, 0xe8742a],
    ] as const) {
      const [cs, sn] = [Math.cos(a), Math.sin(a)];
      const at = (u: number, k: number): [number, number] => [x + cs * u - sn * k, y + sn * u + cs * k];
      g.poly([...at(-r * 0.08, 0), ...at(-r * 0.14, -r * 0.045), ...at(-r * 0.14, r * 0.045)]).fill({ color: c, alpha: 0.85 });
      g.poly(oval(...at(0, 0), r * 0.085, r * 0.032, a, 12)).fill({ color: c, alpha: 0.9 }).stroke({ width: 1, color: shade(c, 0.55), alpha: 0.8 });
      if (c === 0xf2efe6) g.circle(...at(r * 0.02, 0), r * 0.02).fill(0xe8742a);
    }
    pad(-r * 0.55, r * 0.3, r * 0.09, 0xf6efe0);
    pad(-r * 0.4, r * 0.38, r * 0.07, null);
    pad(r * 0.62, r * 0.32, r * 0.08, null);
    return;
  }
  const [p1, p2] = [rnd() * Math.PI * 2, rnd() * Math.PI * 2];
  const R = (a: number) => r * 0.72 * (1 + 0.09 * Math.sin(2 * a + p1) + 0.06 * Math.sin(3 * a + p2)) * (v === 1 ? 1 + 0.08 * Math.cos(2 * a) : 1);
  fade(g, around(48, (a) => R(a) * 1.16), 0x4a3e26, 0.4);
  smooth(g, around(40, (a) => R(a) * 1.07));
  g.fill(0x76643e);
  smooth(g, around(40, (a) => R(a) * 1.07));
  g.stroke({ width: 1.2, color: 0x4a3e26, alpha: 0.7 });
  water(g, R, rnd);
  g.poly(around(64, R)).stroke({ width: 2, color: 0x24403c, join: 'round' });
  // Grass at the water's edge here and there.
  for (let i = 0; i < 5; i++) {
    const a = rnd() * Math.PI * 2;
    footGrass(g, Math.cos(a) * R(a) * 1.08, Math.sin(a) * R(a) * 1.08, r * 0.09, rnd);
  }
  if (v === 0) {
    for (let i = 0; i < 6; i++) {
      const a = p1 + i * 1.1;
      const d = R(a) * (0.35 + 0.4 * rnd());
      pad(Math.cos(a) * d, Math.sin(a) * d, r * (0.06 + 0.03 * rnd()), i < 2 ? 0xe89ab0 : null);
    }
    for (let i = 0; i < 3; i++) {
      const a = p2 + i * 0.5;
      stone(g, Math.cos(a) * R(a) * 1.1, Math.sin(a) * R(a) * 1.1, r * (0.07 + 0.03 * rnd()), STONE[i % 2], rnd, 7);
    }
  } else if (v === 1) {
    // Reeds crowding one side, cattails standing out of them.
    const greens = [0x5e8a48, 0x7aa456, 0x9cc070];
    for (let i = 0; i < 5; i++) {
      const a = p1 + (i - 2) * 0.32;
      const [x, y] = [Math.cos(a) * R(a) * 0.98, Math.sin(a) * R(a) * 0.98];
      const rr = r * (0.2 + 0.05 * rnd());
      const tips = boughs(12, rnd).map(([t, k]): [number, number] => [t, k * (0.65 + 0.35 * rnd())]);
      whorl(g, x, y, rr, tips, 0.35);
      g.fill(greens[0]).stroke({ width: 1.5, color: shade(greens[0], 0.45), join: 'round' });
      whorl(g, x - rr * 0.08, y - rr * 0.08, rr * 0.7, tips, 0.35);
      g.fill(greens[1]);
      whorl(g, x - rr * 0.14, y - rr * 0.14, rr * 0.38, boughs(8, rnd), 0.4);
      g.fill(greens[2]);
      for (let k = 0; k < 2; k++) {
        const ca = rnd() * Math.PI * 2;
        const [cx, cy] = [x + Math.cos(ca) * rr * 0.45, y + Math.sin(ca) * rr * 0.45];
        g.poly(oval(cx, cy, r * 0.05, r * 0.024, ca)).fill(0x6e4428).stroke({ width: 1.5, color: 0x3a2414 });
        g.poly(oval(cx - 1, cy - 1, r * 0.03, r * 0.01, ca)).fill(0x9a6a40);
      }
    }
    for (let i = 0; i < 3; i++) {
      const a = p1 + Math.PI + (i - 1) * 0.7;
      const d = R(a) * 0.55;
      pad(Math.cos(a) * d, Math.sin(a) * d, r * 0.07, null);
    }
  } else {
    for (let i = 0; i < 3; i++) {
      const a = p1 + i * 2.1;
      const d = R(a) * (0.3 + 0.2 * i);
      duck(g, Math.cos(a) * d, Math.sin(a) * d, rnd() * Math.PI * 2, r * 0.07, i === 1);
    }
    const fa = p2 + 1;
    leaf(g, Math.cos(fa) * R(fa) * 1.035, Math.sin(fa) * R(fa) * 1.035, fa + Math.PI / 2, r * 0.1, r * 0.03, 0xf2efe6);
    stone(g, Math.cos(p2) * R(p2) * 1.08, Math.sin(p2) * R(p2) * 1.08, r * 0.08, STONE[2], rnd, 7);
  }
}
