import { Graphics } from 'pixi.js';
import { INK, blob, oval, shade } from './common';
import { BARK, moss } from './stones';
import { OUT } from '../pen';
import { lit } from './plants';

/** End grain of a cut trunk: pale wood, growth rings, a crack. */
export function endGrain(g: Graphics, x: number, y: number, rx: number, ry: number, bark: readonly number[]) {
  g.poly(oval(x, y, rx, ry, 0)).fill(0xc89a68).stroke({ width: 1.5, color: INK });
  for (const f of [0.68, 0.4]) g.poly(oval(x - rx * 0.05, y, rx * f, ry * f, 0)).stroke({ width: 1.2, color: shade(bark[2], 0.85), alpha: 0.9 });
  g.moveTo(x, y).lineTo(x + rx * 0.7, y - ry * 0.5).stroke({ width: 1.2, color: bark[0], alpha: 0.8 });
}

/** Fallen log along x: an uneven, slightly bent trunk that thins toward its splintered end (-x), lumpy
 * bark edges, a soft lit band along the top, furrows, a knot, a bent broken branch stub and a cut end with
 * rings (+x); moss, or a few mushrooms, on some. */
export function fallenLog(g: Graphics, r: number, rnd: () => number, v: number) {
  const bark = BARK[v];
  const len = r * 1.7;
  const w = r * 0.44;
  const x0 = -len / 2;
  const x1 = len / 2 - w * 0.2;
  const bend = (rnd() - 0.5) * w * 0.5;
  // Centre line and half-width at t along the trunk (0 at the splintered end, 1 at the cut end).
  const mid = (t: number) => bend * Math.sin(Math.PI * t);
  const half = (t: number) => (w / 2) * (0.82 + 0.18 * t);
  const n = 10;
  const top: number[] = [];
  const bot: number[] = [];
  for (let i = 0; i <= n; i++) {
    const t = i / n;
    const x = x0 + w * 0.2 + (x1 - x0 - w * 0.2) * t;
    const lump = i === n ? 0 : (rnd() - 0.5) * w * 0.12;
    top.push(x, mid(t) - half(t) + lump);
    bot.push(x, mid(t) + half(t) + (i === n ? 0 : (rnd() - 0.5) * w * 0.12));
  }
  // A bent, tapering branch stub, broken off.
  const st = 0.35 + 0.3 * rnd();
  const sx = x0 + len * st;
  const side = rnd() < 0.5 ? 1 : -1;
  const sy = mid(st) + side * half(st) * 0.6;
  const kx = sx + w * 0.35 + (rnd() - 0.5) * w * 0.2;
  const ky = sy + side * w * 0.55;
  const ex = kx + w * 0.05;
  const ey = ky + side * w * 0.3;
  const bw = w * 0.23;
  const stub = [sx - bw, sy, kx - bw * 0.7, ky, ex - bw * 0.5, ey, ex + bw * 0.1, ey - side * bw * 0.4, ex + bw * 0.5, ey, kx + bw * 0.7, ky, sx + bw, sy];
  g.poly(stub).fill(bark[1]).stroke({ width: 2, color: INK, join: 'round' });
  g.circle(ex, ey - side * bw * 0.15, bw * 0.35).fill(0xb08a5a);
  // The trunk outline: the top edge, the cut end, the bottom edge back, then the splintered end.
  const outline = [...top];
  for (let i = bot.length - 2; i >= 0; i -= 2) outline.push(bot[i], bot[i + 1]);
  const ys = mid(0);
  outline.push(x0 + w * 0.1, ys + w * 0.25, x0 - w * 0.05, ys + w * 0.1, x0 + w * 0.15, ys - w * 0.02, x0, ys - w * 0.2);
  g.poly(outline).fill(bark[0]).stroke({ width: OUT, color: INK, join: 'round' });
  // The body (a little in from the shaded side), then a soft lit band near the top edge.
  const body: number[] = [];
  const band: number[] = [];
  for (let i = 0; i <= n; i++) {
    const x = top[i * 2];
    body.push(x, top[i * 2 + 1] + w * 0.05);
    band.push(x, top[i * 2 + 1] + w * 0.12);
  }
  for (let i = n; i >= 0; i--) {
    const t = i / n;
    body.push(top[i * 2], mid(t) + half(t) * 0.55);
    band.push(top[i * 2], top[i * 2 + 1] + w * 0.24);
  }
  g.poly(body).fill(bark[1]);
  g.poly(band).fill({ color: bark[2], alpha: 0.45 });
  // Furrows along the bark, following the bend.
  for (let i = 0; i < 7; i++) {
    const t0 = 0.1 + rnd() * 0.6;
    const t1 = t0 + 0.15 + rnd() * 0.15;
    const f = (rnd() - 0.5) * 1.4;
    const p = (t: number) => [x0 + w * 0.2 + (x1 - x0 - w * 0.2) * t, mid(t) + half(t) * f];
    const [ax, ay] = p(t0);
    const [cx, cy] = p((t0 + t1) / 2);
    const [bx, by] = p(t1);
    g.moveTo(ax, ay).quadraticCurveTo(cx, cy + (rnd() - 0.5) * 3, bx, by);
  }
  g.stroke({ width: 1.3, color: bark[0], alpha: 0.85, cap: 'round' });
  const kt = 0.45 + 0.2 * rnd();
  g.poly(oval(x0 + len * kt, mid(kt) + w * 0.05, w * 0.13, w * 0.08, 0)).fill(bark[0]).stroke({ width: 1, color: INK });
  // The cut end: muted wood, rings and a crack.
  const ey0 = mid(1);
  g.poly(oval(x1, ey0, w * 0.22, half(1), 0)).fill(0xb08658).stroke({ width: 1.5, color: INK });
  for (const k of [0.66, 0.36]) g.poly(oval(x1 - w * 0.02, ey0, w * 0.22 * k, half(1) * k, 0)).stroke({ width: 1.1, color: shade(bark[1], 0.9), alpha: 0.9 });
  g.moveTo(x1, ey0).lineTo(x1 + w * 0.12, ey0 - half(1) * 0.6).stroke({ width: 1.1, color: bark[0], alpha: 0.8 });
  if (v === 1) {
    for (let i = 0; i < 3; i++) {
      const t = 0.2 + 0.25 * i;
      moss(g, x0 + len * t, mid(t) - half(t) * 0.45, w * 0.3, rnd);
    }
  }
  if (v === 3) {
    for (let i = 0; i < 3; i++) {
      const t = 0.3 + 0.15 * i;
      const mx = x0 + len * t;
      const my = mid(t) + half(t) * (0.95 + 0.15 * rnd());
      g.circle(mx, my, w * 0.16).fill(0xc8a070).stroke({ width: 1.2, color: INK });
      g.circle(mx - w * 0.04, my - w * 0.04, w * 0.07).fill(0xe8d0a0);
    }
  }
}

/** Stump: a ragged bark ring with roots spreading from it, the cut face with rings and a crack. */
export function stump(g: Graphics, r: number, rnd: () => number, v: number) {
  const bark = BARK[v];
  const ph = rnd() * Math.PI * 2;
  // Roots: short rounded flares where the trunk meets the ground.
  for (let i = 0; i < 5; i++) {
    const a = ph + (i / 5) * Math.PI * 2 + rnd() * 0.4;
    const d = r * (0.55 + 0.08 * rnd());
    g.poly(oval(Math.cos(a) * d, Math.sin(a) * d, r * 0.36, r * 0.2, a, 12))
      .fill(shade(bark[1], 1 + 0.15 * lit(a)))
      .stroke({ width: 1.8, color: INK, join: 'round' });
  }
  g.poly(blob(0, 0, r * 0.6, 10, 0.15, rnd)).fill(bark[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(blob(-r * 0.03, -r * 0.03, r * 0.52, 10, 0.12, rnd)).fill(bark[1]);
  g.circle(-r * 0.02, -r * 0.02, r * 0.42).fill(0xc89a68);
  g.circle(-r * 0.06, -r * 0.06, r * 0.25).fill({ color: 0xe0b47a, alpha: 0.8 });
  for (const f of [0.3, 0.16]) g.circle(-r * 0.02, -r * 0.02, r * f).stroke({ width: 1, color: shade(bark[2], 0.85), alpha: 0.9 });
  const a = rnd() * Math.PI * 2;
  g.moveTo(-r * 0.02, -r * 0.02).lineTo(Math.cos(a) * r * 0.4, Math.sin(a) * r * 0.4).stroke({ width: 1.2, color: bark[0] });
  if (v === 1) moss(g, -r * 0.3, -r * 0.5, r * 0.22, rnd);
}

/** Old bones: pale bone with a dark brown outline so they show on any ground — a skull, ribs, long bones. */
export function bones(g: Graphics, r: number, rnd: () => number, v: number) {
  const [mid, light] = [0xdccfae, 0xf2e8cc];
  const line = 0x4a3e30;
  const longBone = (x: number, y: number, a: number, len: number, w: number) => {
    const c = Math.cos(a);
    const s = Math.sin(a);
    const ends = [
      [x - c * len * 0.5, y - s * len * 0.5],
      [x + c * len * 0.5, y + s * len * 0.5],
    ];
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: w + 3, color: line, cap: 'round' });
    for (const [ex, ey] of ends) for (const side of [1, -1]) g.circle(ex - s * side * w * 0.45, ey + c * side * w * 0.45, w * 0.62).fill(line);
    g.moveTo(ends[0][0], ends[0][1]).lineTo(ends[1][0], ends[1][1]).stroke({ width: w, color: mid, cap: 'round' });
    for (const [ex, ey] of ends) for (const side of [1, -1]) g.circle(ex - s * side * w * 0.45, ey + c * side * w * 0.45, w * 0.45).fill(mid);
    g.moveTo(ends[0][0] - s * w * 0.2, ends[0][1] + c * w * 0.2).lineTo(ends[1][0] - s * w * 0.2, ends[1][1] + c * w * 0.2).stroke({ width: w * 0.3, color: light, cap: 'round' });
  };
  const skull = (x: number, y: number, s: number, a: number) => {
    const c = Math.cos(a);
    const sn = Math.sin(a);
    const p = (u: number, w: number) => [x + c * u - sn * w, y + sn * u + c * w];
    g.poly(oval(x, y, s, s * 0.8, a)).fill(mid).stroke({ width: 1.5, color: line });
    const [fx, fy] = p(s * 0.75, 0);
    g.poly(oval(fx, fy, s * 0.45, s * 0.42, a)).fill(mid).stroke({ width: 1.5, color: line });
    g.poly(oval(...(p(-s * 0.2, -s * 0.2) as [number, number]), s * 0.5, s * 0.35, a)).fill({ color: light, alpha: 0.9 });
    for (const side of [1, -1]) {
      const [ex, ey] = p(s * 0.55, side * s * 0.35);
      g.circle(ex, ey, s * 0.18).fill(0x2a2218);
    }
  };
  const ribs = (x: number, y: number, a: number, s: number) => {
    const c = Math.cos(a);
    const sn = Math.sin(a);
    longBone(x, y, a, s * 2, s * 0.22);
    for (let i = 0; i < 4; i++) {
      const u = -s * 0.75 + i * s * 0.5;
      for (const side of [1, -1]) {
        const bx = x + c * u;
        const by = y + sn * u;
        const tx = bx - sn * side * s * 0.9 + c * s * 0.25;
        const ty = by + c * side * s * 0.9 + sn * s * 0.25;
        const mx = bx - sn * side * s * 0.6 - c * s * 0.15;
        const my = by + c * side * s * 0.6 - sn * s * 0.15;
        g.moveTo(bx, by).quadraticCurveTo(mx, my, tx, ty).stroke({ width: 4.5, color: line, cap: 'round' });
        g.moveTo(bx, by).quadraticCurveTo(mx, my, tx, ty).stroke({ width: 2.2, color: mid, cap: 'round' });
      }
    }
  };
  const a = rnd() * Math.PI * 2;
  if (v === 0) {
    ribs(r * 0.15, r * 0.1, a, r * 0.45);
    skull(-r * 0.5, -r * 0.35, r * 0.24, a + 2.5);
  } else if (v === 1) {
    longBone(0, 0, a, r * 1.3, r * 0.14);
    longBone(r * 0.2, r * 0.35, a + 1.2, r * 0.8, r * 0.1);
    longBone(-r * 0.35, r * 0.15, a - 0.8, r * 0.6, r * 0.09);
  } else if (v === 2) {
    longBone(r * 0.1, r * 0.3, a, r * 1.1, r * 0.13);
    skull(-r * 0.25, -r * 0.3, r * 0.3, a + 1);
  } else {
    longBone(0, r * 0.1, a + 0.7, r * 1.1, r * 0.12);
    longBone(0, r * 0.1, a - 0.7, r * 1.1, r * 0.12);
    skull(0, -r * 0.2, r * 0.3, a - Math.PI / 2);
  }
}
