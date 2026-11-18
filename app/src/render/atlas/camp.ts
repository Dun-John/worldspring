import { Graphics } from 'pixi.js';
import { INK, blob, oval, shade } from './common';
import { lit } from './plants';
import { BARK, STONE, stone } from './stones';

/** A-frame tent, its door at +x: two canvas planes sagging between the poles (the north one lit), seams
 * down them, a ridge pole with its ends showing, guy ropes curving out to pegs, the doorway with its flaps
 * tied back; a patch sewn on some. */
function tent(g: Graphics, r: number, rnd: () => number, v: number) {
  const canvas = [0xd8cba8, 0xbfa36a, 0x9a5442][v];
  const [L, W] = [r * 0.78, r * 0.6];
  // Guy ropes and pegs first, under the canvas.
  for (const [sx, sy] of [
    [-1, -1],
    [1, -1],
    [-1, 1],
    [1, 1],
  ]) {
    const [x0, y0] = [sx * L * 0.85, sy * W * 0.95];
    const [x1, y1] = [sx * L * 1.12, sy * W * 1.45];
    g.moveTo(x0, y0).quadraticCurveTo(x0 + sx * L * 0.05, (y0 + y1) / 2 + sy * 3, x1, y1).stroke({ width: 1.3, color: 0x8a7a5a });
    g.circle(x1, y1, 2.3).fill(0x5a4028).stroke({ width: 1, color: INK });
  }
  // The canvas: the eaves sag between the corners.
  const sag = W * 0.1;
  const plane = (sy: number) => {
    g.moveTo(-L, 0).lineTo(L, 0).lineTo(L, sy * W).quadraticCurveTo(0, sy * (W - sag), -L, sy * W).closePath();
  };
  plane(-1);
  g.fill(shade(canvas, 1.08));
  plane(1);
  g.fill(shade(canvas, 0.74));
  // Seams down each plane, and the shade the ridge throws on the lit side's lower edge.
  for (let i = 1; i < 4; i++) {
    const x = -L + (2 * L * i) / 4 + (rnd() - 0.5) * 4;
    g.moveTo(x, -1).lineTo(x + (rnd() - 0.5) * 3, -W + sag * 0.6).stroke({ width: 1.2, color: shade(canvas, 0.85), alpha: 0.9 });
    g.moveTo(x, 1).lineTo(x + (rnd() - 0.5) * 3, W - sag * 0.6).stroke({ width: 1.2, color: shade(canvas, 0.58), alpha: 0.9 });
  }
  g.moveTo(-L, -W).quadraticCurveTo(0, -W + sag, L, -W).lineTo(L, -W + W * 0.14).quadraticCurveTo(0, -W + sag + W * 0.14, -L, -W + W * 0.14).closePath().fill({ color: 0xffffff, alpha: 0.12 });
  if (v === 1) {
    const [px, py] = [-L * 0.35, W * 0.45];
    g.rect(px, py, L * 0.25, W * 0.25).fill(shade(0x8a6a3a, 0.9)).stroke({ width: 1, color: shade(canvas, 0.45) });
  }
  // Outline, the ridge pole (its ends past the canvas), the doorway at +x.
  plane(-1);
  g.stroke({ width: 2.5, color: INK, join: 'round' });
  plane(1);
  g.stroke({ width: 2.5, color: INK, join: 'round' });
  g.moveTo(-L - 4, 0).lineTo(L + 4, 0).stroke({ width: 3.5, color: INK, cap: 'round' });
  g.moveTo(-L - 4, 0).lineTo(L + 4, 0).stroke({ width: 1.8, color: 0x8a6238, cap: 'round' });
  g.poly([L, -W * 0.5, L, W * 0.5, L - W * 0.4, 0]).fill(0x2a2018).stroke({ width: 1.8, color: INK, join: 'round' });
  g.poly([L, -W * 0.5, L + r * 0.12, -W * 0.72, L - r * 0.02, -W * 0.18]).fill(shade(canvas, 1.15)).stroke({ width: 1.5, color: INK, join: 'round' });
  g.poly([L, W * 0.5, L + r * 0.12, W * 0.72, L - r * 0.02, W * 0.18]).fill(shade(canvas, 0.8)).stroke({ width: 1.5, color: INK, join: 'round' });
}

/** Round pavilion (a camp leader's): panels in two colours sagging between the ribs (lit from the NW),
 * a scalloped valance round the rim, a gilt finial with a pennant, the door at +x with its flap tied
 * back. */
function pavilion(g: Graphics, r: number, rnd: () => number) {
  const [a, b] = [0xe2d6b4, 0x8a2e26];
  const R = r * 0.86;
  stripedCanopy(g, R, 12, a, b);
  // The door: a dark slit with its flap tied back.
  g.poly([R, -R * 0.16, R, R * 0.16, R * 0.72, 0]).fill(0x2a2018).stroke({ width: 2, color: INK, join: 'round' });
  g.poly([R, -R * 0.16, R * 1.1, -R * 0.36, R * 0.84, -R * 0.1]).fill(a).stroke({ width: 1.5, color: INK, join: 'round' });
  g.circle(0, 0, R * 0.11).fill(0xd9b36a).stroke({ width: 2, color: INK });
  g.circle(-R * 0.03, -R * 0.03, R * 0.045).fill(0xfff1c0);
  const f = 0.3 + rnd() * 0.3;
  g.poly([0, 0, R * 0.5, -R * 0.12 - f * 8, R * 0.42, R * 0.06]).fill(b).stroke({ width: 1.5, color: INK, join: 'round' });
}

/** A canvas canopy from above round (0, 0), radius `R`: `n` panels in two colours sagging between the ribs
 * (lit from the NW), a seam down each, inked ribs, a scalloped valance round the rim. On a canopy raised on
 * posts (a bandstand's: `drop` > 0) the panels' corners start at angle `ph`, so its eaves follow the stage's
 * polygon, and the valance is one band of darker canvas hanging `drop` px out past them, scalloped. */
export function stripedCanopy(g: Graphics, R: number, n: number, a: number, b: number, drop = 0, ph = 0) {
  const at = (i: number, k: number): [number, number] => [Math.cos(ph + (i / n) * Math.PI * 2) * k, Math.sin(ph + (i / n) * Math.PI * 2) * k];
  if (drop) {
    // The valance: from corner to corner, three scallops to a side, hanging out past the eaves.
    const c0 = at(0, R + drop * 0.9);
    g.moveTo(...c0);
    for (let i = 0; i < n; i++) {
      const [p, q] = [at(i, R + drop * 0.9), at(i + 1, R + drop * 0.9)];
      const mid = ph + ((i + 0.5) / n) * Math.PI * 2;
      for (let k = 0; k < 3; k++) {
        const [u0, u1] = [k / 3, (k + 1) / 3];
        const [x1, y1] = [p[0] + (q[0] - p[0]) * u1, p[1] + (q[1] - p[1]) * u1];
        const [mx, my] = [p[0] + (q[0] - p[0]) * (u0 + u1) * 0.5, p[1] + (q[1] - p[1]) * (u0 + u1) * 0.5];
        g.quadraticCurveTo(mx + Math.cos(mid) * drop * 1.1, my + Math.sin(mid) * drop * 1.1, x1, y1);
      }
    }
    g.closePath().fill(shade(b, 0.62)).stroke({ width: 1.5, color: INK, join: 'round' });
  }
  // (A canopy with few panels sags further between them, so its eaves curve in rather than out.)
  const sag = drop ? 0.9 : 0.94;
  for (let i = 0; i < n; i++) {
    const t0 = ph + (i / n) * Math.PI * 2;
    const t1 = ph + ((i + 1) / n) * Math.PI * 2;
    const tm = (t0 + t1) / 2;
    // Each panel sags a little between its ribs.
    g.moveTo(0, 0)
      .lineTo(Math.cos(t0) * R, Math.sin(t0) * R)
      .quadraticCurveTo(Math.cos(tm) * R * sag, Math.sin(tm) * R * sag, Math.cos(t1) * R, Math.sin(t1) * R)
      .closePath()
      .fill(shade(i % 2 ? b : a, 0.9 + 0.22 * lit(tm)));
    g.moveTo(Math.cos(tm) * R * 0.25, Math.sin(tm) * R * 0.25).lineTo(Math.cos(tm) * R * 0.85, Math.sin(tm) * R * 0.85).stroke({ width: 1, color: shade(i % 2 ? b : a, 0.75), alpha: 0.6 });
  }
  for (let i = 0; i < n; i++) {
    const t = ph + (i / n) * Math.PI * 2;
    g.moveTo(0, 0).lineTo(Math.cos(t) * R, Math.sin(t) * R);
  }
  g.stroke({ width: 1.4, color: INK, alpha: 0.6 });
  if (drop) {
    // The eaves, inked.
    for (let i = 0; i < n; i++) {
      const [t0, t1] = [ph + (i / n) * Math.PI * 2, ph + ((i + 1) / n) * Math.PI * 2];
      const tm = (t0 + t1) / 2;
      g.moveTo(Math.cos(t0) * R, Math.sin(t0) * R).quadraticCurveTo(Math.cos(tm) * R * sag, Math.sin(tm) * R * sag, Math.cos(t1) * R, Math.sin(t1) * R);
    }
    g.stroke({ width: 2, color: INK, join: 'round' });
    return;
  }
  // Scalloped valance round the rim.
  for (let i = 0; i < n * 2; i++) {
    const t = ((i + 0.5) / (n * 2)) * Math.PI * 2;
    g.circle(Math.cos(t) * R, Math.sin(t) * R, R * 0.08).fill(shade(i % 2 ? b : a, 0.85 + 0.2 * lit(t))).stroke({ width: 1, color: INK });
  }
}

/** Tent: an A-frame in three canvases, or a round pavilion. */
export function tents(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v === 3) return pavilion(g, r, rnd);
  tent(g, r, rnd, v);
}

/** Flames from above: curved tongues of fire all swept the same way, in layers from dark orange out to a
 * white-hot heart, nudged toward the light. */
export function flames(g: Graphics, x: number, y: number, s: number, rnd: () => number) {
  const swirl = rnd() < 0.5 ? 1 : -1;
  const layers: [number, number, number][] = [
    [1, 0xd8461a, 6],
    [0.72, 0xf28a1e, 5],
    [0.48, 0xffc23a, 4],
  ];
  layers.forEach(([k, color, n], l) => {
    const cx = x - l * s * 0.05;
    const cy = y - l * s * 0.05;
    const ph = rnd() * Math.PI * 2;
    for (let i = 0; i < n; i++) {
      const a = ph + (i / n) * Math.PI * 2 + (rnd() - 0.5) * 0.4;
      const len = s * k * (0.75 + 0.25 * rnd());
      const w = s * k * 0.45;
      const c = Math.cos(a);
      const sn = Math.sin(a);
      // A tongue: wide at the base, bowed sideways, hooking at the tip.
      const tip = [cx + c * len - sn * len * 0.35 * swirl, cy + sn * len + c * len * 0.35 * swirl];
      const m = [cx + c * len * 0.5 - sn * len * 0.1 * swirl, cy + sn * len * 0.5 + c * len * 0.1 * swirl];
      g.moveTo(cx - sn * w, cy + c * w)
        .quadraticCurveTo(m[0] - sn * w * 0.9, m[1] + c * w * 0.9, tip[0], tip[1])
        .quadraticCurveTo(m[0] + sn * w * 0.5, m[1] - c * w * 0.5, cx + sn * w, cy - c * w)
        .closePath()
        .fill(color);
      if (l === 0) g.stroke({ width: 1.2, color: 0x7a2a0e, join: 'round' });
    }
  });
  g.circle(x - s * 0.12, y - s * 0.12, s * 0.2).fill(0xfff2c0);
}

/** Campfire: a warm glow, a ring of faceted stones round the ash, barked logs crossed in it (cut ends
 * showing), and flames — or, on the last, glowing embers with a wisp of smoke. */
export function campfire(g: Graphics, r: number, rnd: () => number, v: number) {
  if (v !== 3) for (let i = 0; i < 5; i++) g.circle(0, 0, r * (0.98 - i * 0.08)).fill({ color: 0xf59e0b, alpha: 0.05 });
  g.circle(0, 0, r * 0.62).fill(0x2a2420).stroke({ width: 2, color: INK });
  g.circle(r * 0.06, r * 0.06, r * 0.46).fill({ color: 0x6a645c, alpha: 0.6 });
  const n = 9;
  for (let i = 0; i < n; i++) {
    const a = (i / n) * Math.PI * 2 + rnd() * 0.3;
    stone(g, Math.cos(a) * r * 0.7, Math.sin(a) * r * 0.7, r * (0.15 + 0.05 * rnd()), STONE[i % 2], rnd, 7);
  }
  const bark = BARK[0];
  for (const a of [0.4, 2.5, 4.4].map((t) => t + (rnd() - 0.5) * 0.3)) {
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [x0, y0, x1, y1] = [-c * r * 0.48, -s * r * 0.48, c * r * 0.48, s * r * 0.48];
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: r * 0.16 + 3, color: INK, cap: 'round' });
    g.moveTo(x0, y0).lineTo(x1, y1).stroke({ width: r * 0.16, color: bark[1], cap: 'round' });
    g.moveTo(x0 - s * r * 0.03, y0 + c * r * 0.03).lineTo(x1 - s * r * 0.03, y1 + c * r * 0.03).stroke({ width: r * 0.05, color: bark[2], alpha: 0.6, cap: 'round' });
    g.circle(x1, y1, r * 0.07).fill(0xc89a68).stroke({ width: 1, color: INK });
  }
  if (v === 3) {
    for (let i = 0; i < 12; i++) {
      const a = rnd() * Math.PI * 2;
      const d = r * 0.32 * Math.sqrt(rnd());
      g.circle(Math.cos(a) * d, Math.sin(a) * d, 1.8 + rnd() * 2).fill(rnd() < 0.5 ? 0xc2410c : 0xf97316);
    }
    for (let i = 0; i < 3; i++) g.circle(-r * 0.12 - i * r * 0.1, -r * 0.12 - i * r * 0.12, r * (0.12 + i * 0.05)).fill({ color: 0xd8d4cc, alpha: 0.32 - i * 0.08 });
  } else {
    flames(g, 0, 0, r * 0.52, rnd);
  }
}

/** Firewood: split logs stacked side by side, each a little uneven, the cut ends (+x) pale with rings,
 * some split to show the pale wood; a chopping block with an axe bitten into it on some. */
export function firewood(g: Graphics, r: number, rnd: () => number, v: number) {
  const n = 4;
  const lw = (r * 1.05) / n;
  const shift = v % 2 ? -r * 0.2 : 0;
  for (let i = 0; i < n; i++) {
    const bark = BARK[(i + v) % 4];
    const y = -r * 0.52 + (i + 0.5) * lw;
    const len = r * (1.05 + 0.15 * rnd());
    const x = shift + (rnd() - 0.5) * r * 0.12;
    const [x0, x1] = [x - len / 2, x + len / 2];
    const h = lw / 2;
    const wob = () => (rnd() - 0.5) * h * 0.25;
    const body = [x0 + h * 0.5, y - h + wob(), x0 + len * 0.5, y - h + wob(), x1, y - h, x1, y + h, x0 + len * 0.5, y + h + wob(), x0 + h * 0.5, y + h + wob(), x0, y + wob()];
    g.poly(body).fill(bark[0]).stroke({ width: 2, color: INK, join: 'round' });
    g.rect(x0 + h * 0.5, y - h * 0.7, len - h, h * 0.9).fill(bark[1]);
    g.rect(x0 + h * 0.6, y - h * 0.65, len - h * 1.4, h * 0.3).fill({ color: bark[2], alpha: 0.5 });
    if (i % 2 === 1) g.rect(x0 + h * 0.6, y + h * 0.1, len - h * 1.6, h * 0.55).fill(0xc89a68);
    g.poly(oval(x1, y, h * 0.42, h * 0.95, 0)).fill(0xc89a68).stroke({ width: 1.5, color: INK });
    g.poly(oval(x1 - 0.5, y, h * 0.22, h * 0.5, 0)).stroke({ width: 1, color: shade(bark[1], 0.9) });
  }
  if (v % 2) {
    const [sx, sy] = [r * 0.6, r * 0.48];
    const sr = r * 0.3;
    g.poly(blob(sx, sy, sr, 9, 0.12, rnd)).fill(BARK[0][0]).stroke({ width: 2, color: INK });
    g.circle(sx, sy, sr * 0.78).fill(0xc89a68);
    g.circle(sx - sr * 0.15, sy - sr * 0.15, sr * 0.35).fill({ color: 0xe0b47a, alpha: 0.8 });
    g.circle(sx, sy, sr * 0.45).stroke({ width: 1, color: 0x8a6440 });
    // The axe: a handle reaching up-right from the blade bitten into the block.
    const [hx, hy] = [sx + sr * 1.05, sy - sr * 1.1];
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: 5, color: INK, cap: 'round' });
    g.moveTo(sx, sy).lineTo(hx, hy).stroke({ width: 2.8, color: 0x9a7448, cap: 'round' });
    g.poly([sx - sr * 0.42, sy + sr * 0.05, sx + sr * 0.1, sy - sr * 0.3, sx + sr * 0.2, sy + sr * 0.1, sx - sr * 0.2, sy + sr * 0.35]).fill(0x8a8f96).stroke({ width: 1.5, color: INK, join: 'round' });
    g.moveTo(sx - sr * 0.38, sy + sr * 0.08).lineTo(sx - sr * 0.18, sy + sr * 0.3).stroke({ width: 1.2, color: 0xd8dce0 });
  }
}

/** Bedroll: a blanket laid along x with a stripe woven across it, the top corner turned back to show
 * the lining, a crease or two, and the rolled head end at +x tied with a strap. */
export function bedroll(g: Graphics, r: number, rnd: () => number, v: number) {
  const wool = [0x8a3a2e, 0x4a6a3a, 0x7a5a3a, 0x4a5a6e][v];
  const [L, W] = [r * 0.88, r * 0.42];
  g.roundRect(-L, -W, 2 * L, 2 * W, 5).fill(wool).stroke({ width: 2, color: INK });
  g.roundRect(-L + 3, -W + 2, 2 * L - 6, W * 0.45, 4).fill({ color: shade(wool, 1.3), alpha: 0.55 });
  // Stripes woven across.
  for (const x of [-L * 0.62, -L * 0.52]) g.rect(x, -W + 1, L * 0.05, 2 * W - 2).fill(shade(wool, 1.5));
  // Creases.
  for (let i = 0; i < 2; i++) {
    const x = -L * 0.2 + i * L * 0.35 + (rnd() - 0.5) * 6;
    g.moveTo(x, -W + 2).quadraticCurveTo(x + (rnd() - 0.5) * 8, 0, x - 3, W - 2).stroke({ width: 1.4, color: shade(wool, 0.62) });
  }
  // The corner turned back: a triangle of the pale lining.
  g.poly([-L + 2, -W + 2, -L + W * 1.1, -W + 2, -L + 2, -W + W * 1.1]).fill(0xe8dcc0).stroke({ width: 1.5, color: INK, join: 'round' });
  // The rolled head end: a cylinder seen from above with its spiral end.
  g.roundRect(L * 0.42, -W * 1.08, L * 0.58, W * 2.16, W * 0.5).fill(0xd8cfb8).stroke({ width: 2, color: INK });
  g.roundRect(L * 0.46, -W * 0.98, L * 0.5, W * 0.6, W * 0.3).fill({ color: 0xf2ead6, alpha: 0.8 });
  g.moveTo(L * 0.68, -W * 1.08).lineTo(L * 0.68, W * 1.08).stroke({ width: 3, color: 0x6a4a2a });
  g.moveTo(L * 0.96, -W * 0.5).quadraticCurveTo(L * 0.86, 0, L * 0.96, W * 0.5).stroke({ width: 1.2, color: 0xa89878 });
}
