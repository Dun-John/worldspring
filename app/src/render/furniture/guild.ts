import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { type At, CUSHIONS, LEATHER, OAK, type Pt, inkLine } from './hall';
import { BRASS, DARK_WOOD, INK, IRON, LINE, LINEN, type Light, OUT, WOOD, board, castShadow, frame, oval, patch, planks, quad, shade } from './common';
import { ARMS, GILT, SILVER, VELVET } from './tables';
import { WALL_GAP } from './home';

// ---------------------------------------------------------------------------------------------------------
// Guild and civic pieces, and the stable stall.

/** Straw and hay: shade, mid, light, sunlit. */
const STRAW = [0x9a7430, 0xd9b04f, 0xf0d688, 0xfff0b8];
const COPPER = [0x6a3a1e, 0xb06a3a, 0xd8925a];
/** Notices and ledger pages, and the writing on them. */
const PAPERS = [0xf2e8cc, 0xf6efe0, 0xe8dcc0];
const WRITING = 0x6a5642;

/** An item's frame's unit steps along (`U`) and across (`S`), in grid axes. */
function steps(at: At): { U: Pt; S: Pt } {
  const [o, u, s] = [at(0, 0), at(1, 0), at(0, 1)];
  return { U: [u[0] - o[0], u[1] - o[1]], S: [s[0] - o[0], s[1] - o[1]] };
}

/** Points about `c`: `a` along `ax`, `b` along `ay` (unit steps), both turned by `ang`. */
function turned(c: Pt, ax: Pt, ay: Pt, ang = 0): (a: number, b: number) => Pt {
  const [co, sn] = [Math.cos(ang), Math.sin(ang)];
  const X: Pt = [ax[0] * co + ay[0] * sn, ax[1] * co + ay[1] * sn];
  const Y: Pt = [ay[0] * co - ax[0] * sn, ay[1] * co - ax[1] * sn];
  return (a, b) => [c[0] + X[0] * a + Y[0] * b, c[1] + X[1] * a + Y[1] * b];
}

/** A coin from above in a metal's tones (shade, face, lit). */
function coin(g: Graphics, x: number, y: number, r: number, m: readonly number[], light: Light) {
  g.circle(x, y, r).fill(m[0]).stroke({ width: 0.01, color: INK });
  g.circle(x + light[0] * r * 0.18, y + light[1] * r * 0.18, r * 0.72).fill(m[1]);
  g.circle(x + light[0] * r * 0.4, y + light[1] * r * 0.4, r * 0.25).fill({ color: m[2], alpha: 0.8 });
}

/** Strongbox, its front toward +y: squatter and heavier than a chest, of black iron plate or oak strapped with
 * iron; two riveted straps over the lid, iron corners, a drop handle at each end, a big padlock on its hasp. */
export function strongbox(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { x, y } = f;
  const iron = v % 2 === 0;
  const t = iron ? IRON : v === 1 ? DARK_WOOD : WOOD;
  const strap = iron ? [0x24221f, 0x3e4044] : [IRON[0], IRON[1]];
  const stud = v === 2 ? BRASS : iron ? IRON[2] : 0x9aa0a6;
  const [x0, y0, x1, y1] = [x + 0.22, y + 0.27, x + 0.78, y + 0.71];
  castShadow(g, quad([x0, y0, x1, y1], rnd, 0), light, 0.14);
  for (const [hx, sd] of [
    [x0, -1],
    [x1, 1],
  ]) {
    for (const [w, c] of [
      [0.05, INK],
      [0.026, IRON[2]],
    ])
      g.moveTo(hx, y + 0.41).quadraticCurveTo(hx + sd * 0.1, y + 0.49, hx, y + 0.57).stroke({ width: w, color: c, cap: 'round' });
  }
  const face = board(g, [x0, y0, x1, y1], t, light, rnd, true, 0.06, iron ? 0 : 1);
  if (iron) g.poly(quad([face[0] + 0.05, face[1] + 0.05, face[2] - 0.05, face[3] - 0.05], rnd, 0.004)).stroke({ width: LINE, color: IRON[0] });
  else {
    planks(g, face, true, 2, t[0], rnd);
    g.poly(quad([x0 + 0.03, y0 + 0.03, x1 - 0.03, y1 - 0.03], rnd, 0.004)).stroke({ width: 0.035, color: strap[1] });
  }
  for (const sx of [x0 + 0.11, x1 - 0.18]) {
    g.poly(quad([sx, y0 - 0.005, sx + 0.07, y1 + 0.005], rnd, 0.004)).fill(strap[1]).stroke({ width: LINE, color: INK });
    for (let k = 0; k < 4; k++) g.circle(sx + 0.035, y0 + 0.06 + ((y1 - y0 - 0.12) * k) / 3, 0.015).fill(stud);
  }
  for (const [cx, cy, dx, dy] of [
    [x0, y0, 1, 1],
    [x1, y0, -1, 1],
    [x1, y1, -1, -1],
    [x0, y1, 1, -1],
  ]) {
    g.poly([cx, cy, cx + dx * 0.12, cy, cx + dx * 0.12, cy + dy * 0.045, cx + dx * 0.045, cy + dy * 0.045, cx + dx * 0.045, cy + dy * 0.12, cx, cy + dy * 0.12]).fill(strap[0]).stroke({ width: 0.012, color: INK });
  }
  // The hasp down the front, the padlock hanging from it: its shackle, a brass body, the keyhole.
  const mx = x + 0.5;
  g.poly(quad([mx - 0.05, y1 - 0.14, mx + 0.05, y1 + 0.01], rnd, 0.003)).fill(strap[1]).stroke({ width: LINE, color: INK });
  g.circle(mx, y1 - 0.1, 0.016).fill(stud);
  for (const [w, c] of [
    [0.042, INK],
    [0.02, IRON[2]],
  ])
    g.moveTo(mx - 0.045, y1 + 0.06).arc(mx, y1 + 0.06, 0.045, Math.PI, Math.PI * 2).stroke({ width: w, color: c });
  g.poly(quad([mx - 0.08, y1 + 0.05, mx + 0.08, y1 + 0.17], rnd, 0.004)).fill(BRASS).stroke({ width: LINE, color: INK });
  g.rect(mx - 0.065 - light[0] * 0.005, y1 + 0.06, 0.13, 0.025).fill({ color: 0xfff1cf, alpha: 0.35 });
  g.circle(mx, y1 + 0.1, 0.017).fill(INK);
  g.rect(mx - 0.007, y1 + 0.1, 0.014, 0.04).fill(INK);
}

/** Counting desk: a desk under a baize (or leather) cloth, a ledger open on it with figures in columns and a
 * quill across it, an ink jar, coins in stacks and slid out in a row to count, a pair of scales. */
export function countingDesk(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  const { U, S } = steps(at);
  const wood = v % 2 ? DARK_WOOD : WOOD;
  castShadow(g, quad(rect(0.06, 0.1, len - 0.06, 0.9), rnd, 0), light, 0.1);
  board(g, rect(0.06, 0.1, len - 0.06, 0.9), wood, light, rnd, along, 0.05, 2);
  const cloth = v < 2 ? [0x2a4428, 0x3e5e3a] : [0x4a2418, 0x74382a];
  g.poly(quad(rect(0.17, 0.19, len - 0.17, 0.81), rnd, 0.004)).fill(cloth[1]).stroke({ width: LINE, color: cloth[0] });
  // The ledger, open: its leather cover, two pages, the gutter's shade, lines of writing and a column of
  // figures ruled in red, a ribbon hanging out.
  const L = turned(at(0.66, 0.5), U, S, -0.1);
  const poly4 = (a0: number, b0: number, a1: number, b1: number) => [...L(a0, b0), ...L(a1, b0), ...L(a1, b1), ...L(a0, b1)];
  castShadow(g, poly4(-0.28, -0.2, 0.28, 0.2), light, 0.03);
  g.poly(poly4(-0.28, -0.2, 0.28, 0.2)).fill([0x6a2a22, 0x2e4a5a, 0x5a3a1e, 0x3a2a4a][v % 4]).stroke({ width: 0.02, color: INK, join: 'round' });
  g.moveTo(...L(0.06, 0.18)).lineTo(...L(0.09, 0.27)).stroke({ width: 0.022, color: 0xa83228, cap: 'round' });
  for (const sd of [-1, 1]) g.poly(poly4(sd * 0.012, -0.175, sd * 0.255, 0.175)).fill(PAPERS[1]).stroke({ width: 0.012, color: INK });
  g.poly(poly4(-0.045, -0.17, 0.045, 0.17)).fill({ color: LINEN[0], alpha: 0.55 });
  g.moveTo(...L(0, -0.18)).lineTo(...L(0, 0.18)).stroke({ width: 0.012, color: WRITING });
  for (let k = 0; k < 5; k++) {
    const b = -0.12 + k * 0.06;
    g.moveTo(...L(-0.22, b)).lineTo(...L(-0.06 - rnd() * 0.06, b));
    g.moveTo(...L(0.04, b)).lineTo(...L(0.12 + rnd() * 0.02, b));
    g.moveTo(...L(0.17, b)).lineTo(...L(0.23, b));
  }
  g.stroke({ width: 0.014, color: WRITING, cap: 'round' });
  g.moveTo(...L(0.155, -0.15)).lineTo(...L(0.155, 0.15)).stroke({ width: 0.01, color: 0xa83228 });
  // The quill across the right-hand page, the ink jar beyond the book.
  const [qa, qb] = [L(0.08, 0.1), L(0.36, -0.14)];
  const [dx, dy] = [qb[0] - qa[0], qb[1] - qa[1]];
  const m = Math.hypot(dx, dy) || 1;
  const [nx, ny] = [-dy / m, dx / m];
  const mid = [qa[0] + dx * 0.55, qa[1] + dy * 0.55];
  g.poly([qa[0], qa[1], mid[0] + nx * 0.05, mid[1] + ny * 0.05, qb[0], qb[1], mid[0] - nx * 0.028, mid[1] - ny * 0.028]).fill(LINEN[2]).stroke({ width: 0.012, color: INK, join: 'round' });
  g.moveTo(qa[0], qa[1]).lineTo(qb[0], qb[1]).stroke({ width: 0.01, color: LINEN[0] });
  const [jx, jy] = at(0.28, 0.3);
  g.circle(jx, jy, 0.065).fill(0x2a2a3a).stroke({ width: LINE, color: INK });
  g.circle(jx, jy, 0.035).fill(0x0e0e16);
  g.circle(jx + light[0] * 0.03, jy + light[1] * 0.03, 0.015).fill({ color: 0xffffff, alpha: 0.5 });
  // Coins: stacks (their shadows as tall as they are), a row slid out from one, a few loose.
  const cu = len - 0.86;
  const metals = [GILT, GILT, SILVER, COPPER];
  for (const [u, s, k, n] of [
    [cu - 0.1, 0.33, 0, 5],
    [cu + 0.06, 0.29, 1, 3],
    [cu + 0.13, 0.45, 2, 4],
    [cu - 0.04, 0.47, 3, 2],
  ]) {
    const [x, y] = at(u, s);
    castShadow(g, oval(x, y, 0.05, 0.05, 0, 12), light, 0.025 * n);
    coin(g, x, y, 0.05, metals[k], light);
  }
  for (let i = 0; i < 4; i++) coin(g, ...at(cu - 0.2 + i * 0.055, 0.66 - i * 0.012), 0.048, GILT, light);
  coin(g, ...at(cu + 0.2, 0.68), 0.046, SILVER, light);
  // The scales: a wooden foot, two brass pans (a coin in one, a weight in the other), the beam across them on
  // its pillar.
  const B = turned(at(len - 0.36, 0.5), U, S, 0.18);
  g.poly(quad([...B(-0.07, -0.07), ...B(0.07, 0.07)], rnd, 0.004)).fill(DARK_WOOD[1]).stroke({ width: LINE, color: INK });
  for (const sd of [-1, 1]) {
    const [px, py] = B(0, sd * 0.21);
    g.circle(px, py, 0.09).fill(0x8a6a2a).stroke({ width: LINE, color: INK });
    g.circle(px + light[0] * 0.018, py + light[1] * 0.018, 0.068).fill(BRASS);
  }
  coin(g, ...B(0.01, -0.2), 0.04, SILVER, light);
  const [wx, wy] = B(-0.01, 0.21);
  g.rect(wx - 0.03, wy - 0.03, 0.06, 0.06).fill(IRON[1]).stroke({ width: 0.01, color: INK });
  inkLine(g, [B(0, -0.21), B(0, 0.21)], 0.02, GILT[1]);
  const [kx, ky] = B(0, 0);
  g.circle(kx, ky, 0.035).fill(GILT[1]).stroke({ width: 0.012, color: INK });
  g.circle(kx + light[0] * 0.012, ky + light[1] * 0.012, 0.015).fill(GILT[2]);
}

/** Job board on a wall, falling out into the room as a tapestry does: a planked board hung on two pegs, notices
 * pinned to it askew (writing, a seal); one with a wanted bill, a guild's with a painted header, one with a map. */
export function jobBoard(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { along, len, rect, at } = frame(f, back);
  const { U, S } = steps(at);
  const top = WALL_GAP - 0.02;
  const s1 = 0.86;
  const wood = v === 2 ? DARK_WOOD : v === 3 ? [0x8a6a44, 0xb08a5a, 0xc8a274] : WOOD;
  castShadow(g, quad(rect(0.08, top, len - 0.08, s1), rnd, 0), light, 0.05);
  const face = board(g, rect(0.08, top, len - 0.08, s1), wood, light, rnd, along, 0.07, 0);
  planks(g, face, along, 3, wood[0], rnd);
  for (const u of [0.3, len - 0.3]) g.circle(...at(u, top + 0.03), 0.032).fill(DARK_WOOD[0]).stroke({ width: 0.012, color: INK });
  let s0 = top + 0.08;
  if (v === 2) {
    g.poly(quad(rect(0.16, top + 0.08, len - 0.16, top + 0.17), rnd, 0.003)).fill(0x8a2a24).stroke({ width: LINE, color: INK });
    g.moveTo(...at(0.22, top + 0.125)).lineTo(...at(len - 0.22, top + 0.125)).stroke({ width: 0.014, color: GILT[2], alpha: 0.9 });
    s0 = top + 0.18;
  }
  /** A paper `w` by `h`, its middle at (u, s), turned by `ang`, pinned at its top. */
  const paper = (u: number, s: number, w: number, h: number, ang: number, tone: number, what: 'notice' | 'sealed' | 'wanted' | 'map') => {
    const P = turned(at(u, s), U, S, ang);
    const [a, b] = [w / 2, h / 2];
    const pts = [...P(-a, -b), ...P(a, -b), ...P(a, b), ...P(-a, b)];
    castShadow(g, pts, light, 0.012);
    g.poly(pts).fill(tone).stroke({ width: 0.014, color: INK, join: 'round' });
    if (what === 'wanted') {
      g.moveTo(...P(-a * 0.65, -b * 0.66)).lineTo(...P(a * 0.65, -b * 0.66)).stroke({ width: 0.03, color: 0x3a2a1e, cap: 'round' });
      g.poly(oval(...P(0, -b * 0.12), 0.05, 0.06, 0, 12)).stroke({ width: 0.014, color: 0x5a4632 });
      g.moveTo(...P(-a * 0.5, b * 0.42)).quadraticCurveTo(...P(0, b * 0.02), ...P(a * 0.5, b * 0.42)).stroke({ width: 0.014, color: 0x5a4632 });
      g.moveTo(...P(-a * 0.55, b * 0.7)).lineTo(...P(a * 0.55, b * 0.7)).stroke({ width: 0.016, color: WRITING, cap: 'round' });
    } else if (what === 'map') {
      g.moveTo(...P(-a * 0.8, -b * 0.2)).bezierCurveTo(...P(-a * 0.3, -b * 0.7), ...P(0, b * 0.3), ...P(a * 0.4, -b * 0.1)).quadraticCurveTo(...P(a * 0.7, -b * 0.3), ...P(a * 0.85, -b * 0.75));
      g.stroke({ width: 0.016, color: 0x4a6a8a, cap: 'round' });
      for (const [hx, hy] of [
        [-a * 0.45, b * 0.35],
        [-a * 0.2, b * 0.55],
      ])
        g.moveTo(...P(hx - 0.04, hy + 0.025)).lineTo(...P(hx, hy - 0.025)).lineTo(...P(hx + 0.04, hy + 0.025));
      g.stroke({ width: 0.012, color: WRITING, join: 'round' });
      for (let k = 0; k < 4; k++) g.moveTo(...P(-a * 0.1 + k * a * 0.14, b * 0.25 - k * b * 0.08)).lineTo(...P(-a * 0.02 + k * a * 0.14, b * 0.22 - k * b * 0.08));
      g.stroke({ width: 0.012, color: 0x8a3a2a, cap: 'round' });
      const [xx, xy] = P(a * 0.52, -b * 0.02);
      g.moveTo(xx - 0.035, xy - 0.035).lineTo(xx + 0.035, xy + 0.035).moveTo(xx + 0.035, xy - 0.035).lineTo(xx - 0.035, xy + 0.035).stroke({ width: 0.02, color: 0xb5323a, cap: 'round' });
    } else {
      for (let k = 0; k < 3; k++) g.moveTo(...P(-a * 0.7, -b * 0.4 + k * b * 0.42)).lineTo(...P(a * (0.2 + 0.5 * rnd()), -b * 0.4 + k * b * 0.42));
      g.stroke({ width: 0.015, color: WRITING, cap: 'round' });
      if (what === 'sealed') g.circle(...P(a * 0.45, b * 0.6), 0.032).fill(0xa8322e).stroke({ width: 0.01, color: INK });
    }
    const pins = what === 'map' ? [P(-a + 0.03, -b + 0.03), P(a - 0.03, -b + 0.03)] : [P(0, -b + 0.03)];
    for (const [px, py] of pins) g.circle(px, py, 0.022).fill(rnd() < 0.5 ? 0xb5323a : BRASS).stroke({ width: 0.008, color: INK });
  };
  const n = Math.max(2, Math.round((len - 0.2) * 3));
  const s2 = s1 - 0.06;
  const big = v === 1 ? Math.floor(n / 2) : -1;
  for (let i = 0; i < n; i++) {
    if (v === 3 && i < 2) continue;
    const u = 0.22 + ((len - 0.44) * (i + 0.5)) / n + (rnd() - 0.5) * 0.05;
    const [w, h] = [0.26 + rnd() * 0.07, 0.21 + rnd() * 0.06];
    const s = Math.min(s2 - h / 2, s0 + h / 2 + 0.02 + (i % 2 ? 0.16 : 0.02) + rnd() * 0.04);
    if (i === big) continue;
    paper(u, s, w, h, (rnd() - 0.5) * 0.3, PAPERS[i % 3], i === 1 || i === n - 2 ? 'sealed' : 'notice');
  }
  if (big >= 0) paper(0.22 + ((len - 0.44) * (big + 0.5)) / n, (s0 + s2) / 2, 0.34, Math.min(0.4, s2 - s0 - 0.02), (rnd() - 0.5) * 0.12, PAPERS[1], 'wanted');
  if (v === 3) paper(0.22 + ((len - 0.44) * 1) / n, (s0 + s2) / 2, Math.min(0.62, len * 0.32), Math.min(0.44, s2 - s0 - 0.02), (rnd() - 0.5) * 0.08, 0xe0d0a8, 'map');
}

/** A horse's coat (shade, coat, lit) and its mane and tail: bay, dapple grey, chestnut with a flaxen mane. */
const COATS = [
  { body: [0x4a2410, 0x84441f, 0xb06a38], mane: 0x1a1410, legs: 0x1e1814, muzzle: 0x2a1a12 },
  { body: [0x5e5a54, 0xaaa69c, 0xe6e2d6], mane: 0x46423e, legs: 0x56524c, muzzle: 0x46423e },
  { body: [0x7a3a14, 0xb8642a, 0xdc9450], mane: 0xe8d098, legs: 0x9a5222, muzzle: 0x6a3214 },
];

/** A horse from above in the frame `at(u, s)` (`s` from the nose end, about `k` squares per half-length, 2 k nose
 * to tail, centred on `u` = `mid`); `v` 1 bay with a star, 2 dapple grey, 3 chestnut with a blaze (more repeat
 * the coats). `bend` is how far its head is turned to the side `sd` (the stall's head down at the manger), `rope` a
 * halter rope's control and end points, `blanket` a felt saddle blanket. */
export function drawHorse(
  g: Graphics, at: At, k: number, mid: number, sd: number, light: Light, rnd: () => number, v: number,
  o: { bend?: number; shadow?: boolean; rope?: [Pt, Pt]; blanket?: boolean } = {},
) {
  const { bend = 0.17, shadow = true, rope, blanket } = o;
  // The horse from above, its head down in the manger: a long head (narrow at the nose, broad at the eyes) with
  // ears pricked at the poll, the neck under its mane, the barrel, the rump and the tail; hooves showing beside the
  // neck and behind; its halter tied to the manger.
  const coat = COATS[(v - 1) % COATS.length];
  const { U, S } = steps(at);
  const [c0, c1, c2] = coat.body;
  const cl = (s: number) => mid + sd * bend * Math.max(0, 1.05 - s) ** 1.4;
  // (Half widths down it, `s` from the wall in half-stall lengths: the muzzle, the nose, the cheeks and eyes, the
  // throat at the poll, the neck widening to the shoulders, the barrel, the points of the hips, the rump.)
  const prof: [number, number][] = [
    [0.4, 0.056], [0.43, 0.074], [0.48, 0.08], [0.55, 0.074], [0.62, 0.08], [0.7, 0.108], [0.77, 0.126], [0.83, 0.12],
    [0.89, 0.088], [0.94, 0.1], [1.0, 0.124], [1.07, 0.16], [1.14, 0.2], [1.22, 0.23], [1.32, 0.236], [1.44, 0.232],
    [1.54, 0.218], [1.64, 0.24], [1.73, 0.25], [1.81, 0.226], [1.87, 0.17], [1.915, 0.09],
  ];
  const H = (s: number, a: number, w: number): Pt => at(cl(s) + a * w, s * k);
  const body = (sc: number, dx: number, dy: number): number[] => {
    const pts: Pt[] = [H(0.375, 0, 0)];
    for (const [s, w] of prof) pts.push(H(s, -1, w * sc));
    pts.push(H(1.89, 0, 0));
    for (const [s, w] of [...prof].reverse()) pts.push(H(s, 1, w * sc));
    return pts.flatMap(([x, y]) => [x + dx, y + dy]);
  };
  if (shadow) castShadow(g, body(1, 0, 0), light, 0.32);
  // Legs and hooves under it, showing beside the neck and either side of the tail.
  for (const [s, a] of [
    [1.05, -0.17], [1.08, 0.17], [1.9, -0.13], [1.88, 0.13],
  ]) {
    const [x, y] = H(s, a, 1);
    const fore = s < 1.5 ? -1 : 1;
    g.poly(oval(x, y, 0.052, 0.052, 0, 10)).fill(coat.legs).stroke({ width: LINE, color: INK });
    const [hx, hy] = H(s + fore * 0.04, a * 1.04, 1);
    g.poly(oval(hx, hy, 0.036, 0.03, Math.atan2(U[1], U[0]), 10)).fill(0x2e2620).stroke({ width: 0.014, color: INK });
  }
  // The tail, hanging behind and swished to one side, strands in it.
  const tail = [H(1.8, -0.7, 0.08), H(1.92, -0.9, 0.08), H(1.98, sd * 0.2, 0.08), H(2.0, sd * 1.4, 0.08), H(1.94, sd * 1.9, 0.08), H(1.88, sd * 1.2, 0.08), H(1.8, 0.7, 0.08)];
  g.poly(tail.flat()).fill(coat.mane).stroke({ width: LINE, color: INK, join: 'round' });
  for (const o of [-0.3, 0.3]) g.moveTo(...H(1.88, o, 0.08)).lineTo(...H(1.97, sd * 0.9 + o, 0.08));
  g.stroke({ width: 0.012, color: shade(coat.mane, coat.mane > 0x808080 ? 0.75 : 1.8) });
  g.poly(body(1, 0, 0)).fill(c0).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(body(0.84, light[0] * 0.028, light[1] * 0.028)).fill(c1);
  g.poly(body(0.45, light[0] * 0.06, light[1] * 0.06)).fill({ color: c2, alpha: 0.4 });
  if (v === 2) {
    // Dapples: pale rounds ringed darker over the barrel and rump.
    for (const [s, a] of [
      [1.24, -0.45], [1.42, 0.4], [1.62, -0.25], [1.34, 0.02], [1.54, 0.6], [1.72, 0.3],
    ]) {
      const [x, y] = H(s, a, 0.22);
      g.poly(patch(x, y, 0.04, 0.036, rnd, 0.2, 9)).fill({ color: 0xf0ece2, alpha: 0.6 }).stroke({ width: 0.01, color: c0, alpha: 0.6 });
    }
  }
  if (blanket) {
    // A felt saddle blanket over the back: red, a blue border, a pale diamond in its middle.
    const felt = (s0: number, s1: number, w: number): number[] => {
      const pts: Pt[] = [];
      for (let i = 0; i <= 4; i++) pts.push(H(s0 + ((s1 - s0) * i) / 4, -1, w));
      for (let i = 4; i >= 0; i--) pts.push(H(s0 + ((s1 - s0) * i) / 4, 1, w));
      return pts.flat();
    };
    g.poly(felt(1.2, 1.64, 0.2)).fill(0xa8443a);
    g.poly(felt(1.25, 1.59, 0.15)).stroke({ width: 0.03, color: 0x3e5a8a, join: 'round' });
    g.poly([...H(1.33, 0, 0), ...H(1.42, -1, 0.07), ...H(1.51, 0, 0), ...H(1.42, 1, 0.07)]).fill(0xe6d6b0).stroke({ width: 0.012, color: 0x6a2420 });
    g.poly(felt(1.2, 1.64, 0.2)).stroke({ width: LINE, color: INK, join: 'round' });
  }
  // Forms: the line of the shoulders across the base of the neck, the points of the hips, the muzzle darker.
  g.moveTo(...H(1.18, -0.85, 0.21)).quadraticCurveTo(...H(1.1, 0, 0), ...H(1.18, 0.85, 0.21));
  g.moveTo(...H(1.62, -0.9, 0.23)).quadraticCurveTo(...H(1.68, -0.6, 0.23), ...H(1.74, -0.82, 0.23));
  g.moveTo(...H(1.62, 0.9, 0.23)).quadraticCurveTo(...H(1.68, 0.6, 0.23), ...H(1.74, 0.82, 0.23));
  g.stroke({ width: 0.014, color: c0, cap: 'round' });
  const muz: Pt[] = [];
  for (let i = 0; i <= 6; i++) muz.push(H(0.4 + i * 0.025, -1, 0.06 + (i ? 0.008 : -0.012)));
  for (let i = 6; i >= 0; i--) muz.push(H(0.4 + i * 0.025, 1, 0.06 + (i ? 0.008 : -0.012)));
  g.poly(muz.flat()).fill({ color: coat.muzzle, alpha: 0.75 });
  for (const e of [-1, 1]) g.poly(oval(...H(0.43, e, 0.032), 0.016, 0.01, Math.atan2(S[1], S[0]), 8)).fill(INK);
  // A white blaze down the chestnut's face, a star on the bay's forehead; the eyes on the sides of the head.
  if (v === 3) g.poly([...H(0.8, -0.2, 0.12), ...H(0.8, 0.2, 0.12), ...H(0.55, 0.25, 0.074), ...H(0.47, 0.4, 0.074), ...H(0.47, -0.4, 0.074), ...H(0.55, -0.25, 0.074)]).fill(0xf4efe2);
  if (v === 1) g.poly(patch(...H(0.76, 0, 0), 0.026, 0.022, rnd, 0.2, 8)).fill(0xf4efe2);
  for (const e of [-1, 1]) g.circle(...H(0.75, e, 0.105), 0.019).fill(INK);
  // The mane down the crest of the neck, falling to one side, and the forelock between the ears.
  const mane: Pt[] = [H(0.89, -0.15 * sd, 0.088), H(1.0, -0.12 * sd, 0.124), H(1.1, -0.1 * sd, 0.18), H(1.2, 0, 0.22)];
  for (const [s, w] of [[1.17, 0.21], [1.1, 0.18], [1.04, 0.14], [0.97, 0.11], [0.91, 0.09]] as const) mane.push(H(s, sd * 0.62, w), H(s - 0.03, sd * 0.36, w));
  g.poly(mane.flat()).fill(coat.mane).stroke({ width: LINE, color: INK, join: 'round' });
  g.poly([...H(0.9, -0.3, 0.088), ...H(0.9, 0.3, 0.088), ...H(0.8, 0.2, 0.12), ...H(0.77, -0.05, 0.12)]).fill(coat.mane).stroke({ width: 0.012, color: INK, join: 'round' });
  // The ears pricked forward and out at the poll.
  for (const e of [-1, 1]) {
    const ear = [H(0.91, e * 0.04, 1), H(0.85, e * 0.1, 1), H(0.79, e * 0.2, 1), H(0.86, e * 0.19, 1), H(0.93, e * 0.13, 1)];
    g.poly(ear.flat()).fill(c1).stroke({ width: 0.02, color: INK, join: 'round' });
    g.poly([...H(0.89, e * 0.1, 1), ...H(0.82, e * 0.18, 1), ...H(0.87, e * 0.17, 1)]).fill(c0);
  }
  // The halter: a noseband, a strap behind the ears, a rope to the manger's corner.
  g.moveTo(...H(0.52, -1, 0.078)).lineTo(...H(0.52, 1, 0.078));
  g.moveTo(...H(0.93, -1, 0.096)).lineTo(...H(0.93, 1, 0.096));
  g.stroke({ width: 0.02, color: LEATHER[1] });
  if (rope) g.moveTo(...H(0.52, sd, 0.078)).quadraticCurveTo(...rope[0], ...rope[1]).stroke({ width: 0.016, color: 0xc8b088, cap: 'round' });
}

/** Stable stall, its head to the wall: board partitions down both long sides ending in posts, straw on the
 * floor, a manger of hay across the head; empty (a forkful of fresh straw) or a horse seen from above, its head
 * down in the manger, tied to it. */
export function stableStall(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, given: [number, number]) {
  // (The head is on a short side: a long side given is taken as the end nearer it.)
  let back = given;
  if (f.w !== f.h && (f.h > f.w ? back[0] !== 0 : back[1] !== 0)) back = f.h > f.w ? [0, -1] : [-1, 0];
  const { along, rect, at } = frame(f, back);
  const [W, D] = back[1] !== 0 ? [f.w, f.h] : [f.h, f.w];
  const k = D / 2;
  // Straw over the floor, fading at its edge, strands in it.
  const straw: number[] = [];
  const edge = (u: number, s: number) => straw.push(...at(u + (rnd() - 0.5) * 0.05, s + (rnd() - 0.5) * 0.05));
  for (let i = 0; i <= 4; i++) edge(0.06 + ((W - 0.12) * i) / 4, 0.4 * k);
  for (let i = 1; i <= 6; i++) edge(W - 0.06, 0.4 * k + ((D - 0.06 - 0.4 * k) * i) / 6);
  for (let i = 3; i >= 0; i--) edge(0.06 + ((W - 0.12) * i) / 4, D - 0.06);
  for (let i = 5; i >= 1; i--) edge(0.06, 0.4 * k + ((D - 0.06 - 0.4 * k) * i) / 6);
  g.poly(straw).fill({ color: STRAW[1], alpha: 0.22 });
  // Strands, each its own stroke: a dark edge under a yellow line, a little bent, lying every way.
  const strands: [Pt, Pt, Pt][] = [];
  const strand = (x: number, y: number, l: number) => {
    const a = rnd() * Math.PI;
    const [c, s] = [Math.cos(a) * l, Math.sin(a) * l];
    const b = (rnd() - 0.5) * l * 0.6;
    strands.push([[x - c, y - s], [x - Math.sin(a) * b, y + Math.cos(a) * b], [x + c, y + s]]);
  };
  for (let i = 0; i < 30 * k * W; i++) strand(...at(0.12 + rnd() * (W - 0.24), 0.45 * k + rnd() * (D - 0.55 * k)), 0.05 + rnd() * 0.06);
  if (v === 0) {
    // A forkful of fresh straw toward the back, a heap of strands.
    const [hx, hy] = at(W * 0.55, D * 0.66);
    g.poly(patch(hx, hy, 0.22 * Math.min(W, 1), 0.18, rnd, 0.35, 14)).fill({ color: STRAW[1], alpha: 0.5 });
    for (let i = 0; i < 26; i++) strand(hx + (rnd() - 0.5) * 0.4, hy + (rnd() - 0.5) * 0.34, 0.05 + rnd() * 0.05);
  }
  for (const [p0, p1, p2] of strands) g.moveTo(...p0).quadraticCurveTo(...p1, ...p2);
  g.stroke({ width: 0.034, color: STRAW[0], alpha: 0.9, cap: 'round' });
  for (const old of [true, false]) {
    strands.forEach(([p0, p1, p2], i) => {
      if ((i % 3 === 2) === old) g.moveTo(...p0).quadraticCurveTo(...p1, ...p2);
    });
    g.stroke({ width: 0.017, color: STRAW[old ? 1 : 2], cap: 'round' });
  }
  // The partitions on both long sides (centred on the stall's edges, so a row of stalls shares them), a post at
  // each one's end; the manger across the head, deep, hay heaped in it.
  const pEnd = D * 0.84;
  for (const u of [0, W]) castShadow(g, quad(rect(u - 0.05, 0, u + 0.05, pEnd), rnd, 0), light, 0.24);
  castShadow(g, quad(rect(0.06, WALL_GAP - 0.04, W - 0.06, 0.5 * k), rnd, 0), light, 0.12);
  for (const u of [0, W]) {
    board(g, rect(u - 0.05, 0.02, u + 0.05, pEnd), WOOD, light, rnd, !along, 0.025, 0);
    board(g, rect(u - 0.075, pEnd - 0.03, u + 0.075, pEnd + 0.12), DARK_WOOD, light, rnd, !along, 0.03, 0);
  }
  board(g, rect(0.06, WALL_GAP - 0.04, W - 0.06, 0.5 * k), WOOD, light, rnd, along, 0.04, 1);
  g.poly(quad(rect(0.13, WALL_GAP + 0.02, W - 0.13, 0.5 * k - 0.07), rnd, 0.004)).fill(shade(WOOD[0], 0.7)).stroke({ width: LINE, color: INK });
  for (let i = 0; i < 4 * W; i++) {
    const [x, y] = at(0.2 + ((W - 0.4) * (i + 0.5)) / (4 * W) + (rnd() - 0.5) * 0.04, (WALL_GAP + 0.5 * k - 0.05) / 2 + (rnd() - 0.5) * 0.03);
    g.poly(patch(x, y, 0.1, 0.07, rnd, 0.3, 10)).fill(i % 2 ? STRAW[2] : STRAW[1]).stroke({ width: 0.014, color: STRAW[0], join: 'round' });
  }
  for (let i = 0; i < 6 * W; i++) {
    const [x, y] = at(0.18 + rnd() * (W - 0.36), WALL_GAP + 0.04 + rnd() * (0.5 * k - WALL_GAP - 0.1));
    const a = rnd() * Math.PI;
    g.moveTo(x - Math.cos(a) * 0.06, y - Math.sin(a) * 0.06).lineTo(x + Math.cos(a) * 0.06, y + Math.sin(a) * 0.06);
  }
  g.stroke({ width: 0.014, color: STRAW[0], alpha: 0.8, cap: 'round' });
  if (v === 0) return;
  // The horse from above, its head down in the manger, its halter tied to the manger's corner.
  const sd = v % 2 ? 1 : -1;
  const mid = W / 2;
  drawHorse(g, at, k, mid, sd, light, rnd, v, { rope: [at(mid + sd * 0.24, 0.46 * k), at(sd > 0 ? W - 0.16 : 0.16, 0.5 * k - 0.05)] });
}

/** A heater shield of arms hung on the wall: `P(a, b)` the point `a` across from its middle and `b` down from
 * its top, `hw` its half width, `h` its height; the field, a rim, one bold charge for arms `look`. */
function shieldOfArms(g: Graphics, P: (a: number, b: number) => Pt, hw: number, h: number, look: number, light: Light) {
  const arms = ARMS[look % ARMS.length];
  const [f0, f1, f2] = arms.field;
  const side: [number, number][] = [];
  for (let i = 0; i <= 8; i++) {
    const t = i / 8;
    side.push([hw * (1 - t * t), h * ((1 - t) * (1 - t) * 0.42 + 2 * (1 - t) * t * 0.84 + t * t)]);
  }
  const outline = (sc: number, dx = 0, dy = 0): number[] => {
    const cy = h * 0.42;
    const q: [number, number][] = [[-hw, 0], [hw, 0], ...side, ...side.slice(0, -1).reverse().map(([a, b]): [number, number] => [-a, b])];
    return q.flatMap(([a, b]) => {
      const [x, y] = P(a * sc, cy + (b - cy) * sc);
      return [x + dx, y + dy];
    });
  };
  castShadow(g, outline(1), light, 0.05);
  g.poly(outline(1)).fill(f0).stroke({ width: 0.055, color: INK, join: 'round' });
  g.poly(outline(0.92, light[0] * 0.02, light[1] * 0.02)).fill(f1);
  g.poly(outline(0.92)).stroke({ width: 0.03, color: arms.rim, join: 'round' });
  const [ox, oy] = P(0, 0);
  const [ax, ay] = P(1, 0);
  const lit = (ax - ox) * light[0] + (ay - oy) * light[1] > 0 ? 1 : -1;
  g.poly([...P(lit * hw * 0.55, h * 0.08), ...P(lit * hw * 0.74, h * 0.08), ...P(lit * hw * 0.58, h * 0.5), ...P(lit * hw * 0.44, h * 0.5)]).fill({ color: f2, alpha: 0.3 });
  const Q = (a: number, b: number): Pt => P(a * hw, b * h);
  const E = 0.02;
  const fillQ = (q: [number, number][], color: number) => g.poly(q.flatMap(([a, b]) => Q(a, b))).fill(color).stroke({ width: E, color: INK, join: 'round' });
  const lineQ = (q: [number, number][], w: number, color: number) => inkLine(g, q.map(([a, b]) => Q(a, b)), w, color);
  switch (look % ARMS.length) {
    case 0: {
      // Scales: the pillar on its foot, a thick beam, two deep pans hung from it.
      fillQ([[-0.36, 0.82], [0.36, 0.82], [0.2, 0.72], [-0.2, 0.72]], GILT[1]);
      lineQ([[0, 0.2], [0, 0.74]], 0.045, GILT[1]);
      lineQ([[-0.66, 0.26], [0.66, 0.26]], 0.042, GILT[1]);
      for (const sd of [-1, 1]) {
        g.moveTo(...Q(sd * 0.66, 0.26)).lineTo(...Q(sd * 0.92, 0.5)).moveTo(...Q(sd * 0.66, 0.26)).lineTo(...Q(sd * 0.4, 0.5)).stroke({ width: 0.018, color: GILT[2] });
        fillQ([[sd * 0.96, 0.49], [sd * 0.36, 0.49], [sd * 0.44, 0.6], [sd * 0.66, 0.65], [sd * 0.88, 0.6]], GILT[1]);
      }
      g.circle(...Q(0, 0.17), 0.045).fill(GILT[1]).stroke({ width: E, color: INK });
      break;
    }
    case 1:
      // A big anvil, a hammer raised over it.
      fillQ([[-0.86, 0.5], [-0.46, 0.46], [0.7, 0.46], [0.7, 0.6], [0.28, 0.64], [0.22, 0.72], [0.52, 0.84], [-0.52, 0.84], [-0.22, 0.72], [-0.28, 0.64], [-0.46, 0.6]], SILVER[1]);
      g.moveTo(...Q(-0.4, 0.5)).lineTo(...Q(0.62, 0.5)).stroke({ width: 0.02, color: SILVER[2] });
      lineQ([[0.2, 0.4], [-0.16, 0.06]], 0.05, OAK[2]);
      fillQ([[0.1, 0.02], [0.5, 0.26], [0.36, 0.42], [-0.04, 0.18]], SILVER[1]);
      break;
    case 2:
      // Two broad swords crossed, points up.
      for (const sd of [-1, 1]) {
        const [b0, b1] = [Q(sd * 0.58, 0.84), Q(-sd * 0.54, 0.1)];
        const [dx, dy] = [b1[0] - b0[0], b1[1] - b0[1]];
        const m = Math.hypot(dx, dy) || 1;
        const [nx, ny] = [-dy / m, dx / m];
        const at2 = (t: number, o: number): Pt => [b0[0] + dx * t + nx * o, b0[1] + dy * t + ny * o];
        g.poly([...at2(0.24, -0.045), ...at2(0.88, -0.038), ...at2(1, 0), ...at2(0.88, 0.038), ...at2(0.24, 0.045)]).fill(SILVER[1]).stroke({ width: E, color: INK, join: 'round' });
        g.moveTo(...at2(0.27, 0)).lineTo(...at2(0.88, 0)).stroke({ width: 0.014, color: SILVER[2] });
        inkLine(g, [at2(0.05, 0), at2(0.22, 0)], 0.04, LEATHER[1]);
        inkLine(g, [at2(0.23, -0.12), at2(0.23, 0.12)], 0.04, GILT[1]);
        g.circle(...at2(0.03, 0), 0.042).fill(GILT[1]).stroke({ width: E, color: INK });
      }
      break;
    case 3: {
      // One big emblem: a gold five-pointed star over an open book.
      for (const sd of [-1, 1]) fillQ([[0, 0.6], [sd * 0.1, 0.55], [sd * 0.86, 0.53], [sd * 0.86, 0.8], [sd * 0.1, 0.82], [0, 0.87]], 0x9a2a26);
      for (const sd of [-1, 1]) fillQ([[0, 0.6], [sd * 0.1, 0.55], [sd * 0.78, 0.53], [sd * 0.78, 0.76], [sd * 0.1, 0.78], [0, 0.83]], PAPERS[1]);
      for (const sd of [-1, 1]) for (let k2 = 0; k2 < 2; k2++) g.moveTo(...Q(sd * 0.2, 0.62 + k2 * 0.08)).lineTo(...Q(sd * 0.64, 0.61 + k2 * 0.08));
      g.stroke({ width: 0.016, color: WRITING, cap: 'round' });
      const [sx, sy] = Q(0, 0.32);
      const q: number[] = [];
      for (let i = 0; i < 10; i++) {
        const a = -Math.PI / 2 + (i * Math.PI) / 5;
        const r = i % 2 ? hw * 0.33 : hw * 0.78;
        q.push(...P(Math.cos(a) * r, 0.32 * h + Math.sin(a) * r * 0.8));
      }
      g.poly(q).fill(GILT[2]).stroke({ width: E, color: INK, join: 'round' });
      g.circle(sx, sy, hw * 0.12).fill(0xfff4c8);
      break;
    }
    default: {
      // One tall red tower: three merlons, a gate, a window; a blue flag flying from its top.
      const red = 0xa83226;
      lineQ([[0.06, 0.34], [0.06, 0.06]], 0.024, DARK_WOOD[1]);
      fillQ([[0.08, 0.06], [0.62, 0.11], [0.5, 0.16], [0.6, 0.22], [0.08, 0.22]], 0x2a4a8a);
      fillQ([[-0.44, 0.86], [-0.44, 0.31], [-0.24, 0.31], [-0.24, 0.39], [-0.1, 0.39], [-0.1, 0.31], [0.1, 0.31], [0.1, 0.39], [0.24, 0.39], [0.24, 0.31], [0.44, 0.31], [0.44, 0.86]], red);
      g.moveTo(...Q(-0.44, 0.45)).lineTo(...Q(0.44, 0.45)).stroke({ width: 0.014, color: 0x6a1a14 });
      g.poly([...Q(-0.18, 0.86), ...Q(-0.18, 0.7), ...Q(0, 0.62), ...Q(0.18, 0.7), ...Q(0.18, 0.86)]).fill(0x2a1a14);
      g.poly([...Q(-0.05, 0.58), ...Q(-0.05, 0.5), ...Q(0.05, 0.5), ...Q(0.05, 0.58)]).fill(0x2a1a14);
    }
  }
}

/** A guild's arms on a wall, falling out into the room as a banner does: on one square a shield hung from a
 * peg; on two a banner on a rod (tasselled cords at its ends, a fringed swallowtail) with the shield on it. */
export function guildArms(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number, back: [number, number]) {
  const { len, rect, at } = frame(f, back);
  const top = WALL_GAP - 0.02;
  const big = len >= 2;
  const c = len / 2;
  if (big) {
    const [k0, k1, k2] = ARMS[v % ARMS.length].cloth;
    const [bw, bt, bl] = [0.5, 0.62, 0.74];
    const B = (a: number, b: number): Pt => at(c + a, top + b);
    const cloth = [B(-bw, 0), B(bw, 0), B(bw, bt), B(bw * 0.98, bl), B(0, bt + 0.02), B(-bw * 0.98, bl), B(-bw, bt)];
    castShadow(g, cloth.flat(), light, 0.04);
    g.poly(cloth.flat()).fill(k1).stroke({ width: OUT, color: INK, join: 'round' });
    const [ax, ay] = B(1, 0);
    const [ox, oy] = B(0, 0);
    const sideLit = (ax - ox) * light[0] + (ay - oy) * light[1] > 0 ? 1 : -1;
    for (const [a, color, alpha] of [
      [sideLit * 0.38, k2, 0.55],
      [-sideLit * 0.38, k0, 0.5],
    ] as const)
      g.poly([...B(a - 0.05, 0.04), ...B(a + 0.05, 0.04), ...B(a + 0.04, bt - 0.02), ...B(a - 0.04, bt - 0.02)]).fill({ color, alpha });
    // A gilt border down the sides and along the swallowtail, the hem fringed.
    g.poly([B(-bw + 0.05, 0.03), B(-bw + 0.05, bt), B(-bw * 0.9, bl - 0.05), B(0, bt - 0.04), B(bw * 0.9, bl - 0.05), B(bw - 0.05, bt), B(bw - 0.05, 0.03)].flat(), false).stroke({ width: 0.022, color: GILT[1], join: 'round' });
    for (let i = 0; i <= 10; i++) {
      const t = i / 10;
      const a = (t * 2 - 1) * bw * 0.98;
      const b = bt + 0.02 + (bl - bt - 0.02) * Math.abs(t * 2 - 1);
      g.moveTo(...B(a, b)).lineTo(...B(a, b + 0.045));
    }
    g.stroke({ width: 0.016, color: GILT[1], cap: 'round' });
    // The rod along the wall, gilt finials; a cord from each end, a tassel hanging beside the cloth.
    inkLine(g, [B(-bw - 0.12, 0), B(bw + 0.12, 0)], 0.04, DARK_WOOD[1]);
    for (const sd of [-1, 1]) {
      g.circle(...B(sd * (bw + 0.13), 0), 0.04).fill(GILT[1]).stroke({ width: 0.014, color: INK });
      g.moveTo(...B(sd * (bw + 0.08), 0.01)).quadraticCurveTo(...B(sd * (bw + 0.11), 0.22), ...B(sd * (bw + 0.07), 0.4)).stroke({ width: 0.016, color: GILT[1], cap: 'round' });
      g.poly([...B(sd * (bw + 0.05), 0.38), ...B(sd * (bw + 0.09), 0.38), ...B(sd * (bw + 0.12), 0.52), ...B(sd * (bw + 0.02), 0.52)]).fill(GILT[1]).stroke({ width: 0.014, color: INK, join: 'round' });
      g.circle(...B(sd * (bw + 0.07), 0.38), 0.025).fill(GILT[2]).stroke({ width: 0.01, color: INK });
    }
  }
  const [hw, h] = big ? [0.3, 0.56] : [0.27, 0.6];
  const P = (a: number, b: number): Pt => at(c + a, top + (big ? 0.05 : 0.04) + b);
  if (!big) g.poly(quad(rect(c - 0.05, top - 0.03, c + 0.05, top + 0.07), rnd, 0.003)).fill(LEATHER[1]).stroke({ width: 0.012, color: INK });
  shieldOfArms(g, P, hw, h, v, light);
  if (!big) g.circle(...at(c, top + 0.01), 0.03).fill(DARK_WOOD[1]).stroke({ width: 0.012, color: INK });
}

/** Council table: a long table, oval or squared, in oak or walnut (a baize cloth or a runner on some), its seats
 * round it: high-backed chairs down both sides, a plain one at the foot and the head's chair grander, its back
 * gilt. */
export function councilTable(g: Graphics, f: InteriorItem, light: Light, v: number, rnd: () => number) {
  const { along, len, rect, at } = frame(f);
  const { U, S } = steps(at);
  const wid = along ? f.h : f.w;
  const round = v % 2 === 0;
  const wood = v < 2 ? WOOD : DARK_WOOD;
  const cushion = [LEATHER, CUSHIONS[2], VELVET, CUSHIONS[1]][v % 4];
  const chairWood = v < 2 ? OAK : DARK_WOOD;
  const [cu, cs] = [len / 2, wid / 2];
  const [rx, ry] = [len / 2 - 0.5, wid / 2 - 0.5];
  const p = round ? 2.6 : 9;
  const N = 60;
  const jit = Array.from({ length: N }, () => (rnd() - 0.5) * 0.012);
  const shape = (inset: number, dx = 0, dy = 0): number[] => {
    const pts: number[] = [];
    for (let i = 0; i < N; i++) {
      const t = (i / N) * Math.PI * 2;
      const [c, s] = [Math.cos(t), Math.sin(t)];
      const u = cu + Math.sign(c) * Math.pow(Math.abs(c), 2 / p) * (rx - inset) + c * jit[i];
      const w = cs + Math.sign(s) * Math.pow(Math.abs(s), 2 / p) * (ry - inset) + s * jit[i];
      const [x, y] = at(u, w);
      pts.push(x + dx, y + dy);
    }
    return pts;
  };
  /** The table's half width across at `u`. */
  const half = (u: number) => ry * Math.pow(Math.max(0, 1 - Math.pow(Math.min(1, Math.abs(u - cu) / rx), p)), 1 / p);
  /** A chair at `c`, `toward` the table (unit), `ax` across it; the head's grander. */
  const seat = (c: Pt, ax: Pt, toward: Pt, grand: boolean) => {
    const P = turned(c, ax, toward, (rnd() - 0.5) * 0.08);
    const q = (a0: number, b0: number, a1: number, b1: number) => [...P(a0, b0), ...P(a1, b0), ...P(a1, b1), ...P(a0, b1)];
    const hw = grand ? 0.2 : 0.165;
    const bk = grand ? -0.25 : -0.2;
    castShadow(g, q(-hw, -0.16, hw, 0.17), light, 0.1);
    g.poly(q(-hw, -0.16, hw, 0.17)).fill(chairWood[0]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(q(-hw + 0.035, -0.12, hw - 0.035, 0.14)).fill(cushion[1]).stroke({ width: LINE, color: INK, join: 'round' });
    g.poly(q(-hw + 0.06, -0.09, hw * 0.1, 0.02)).fill({ color: cushion[2], alpha: 0.45 });
    castShadow(g, q(-hw - 0.02, bk, hw + 0.02, -0.12), light, grand ? 0.34 : 0.24);
    g.poly(q(-hw - 0.02, bk, hw + 0.02, -0.12)).fill(chairWood[1]).stroke({ width: OUT, color: INK, join: 'round' });
    g.poly(q(-hw, bk + 0.015, hw, bk + 0.035)).fill({ color: chairWood[2], alpha: 0.7 });
    const trim = grand ? GILT[1] : chairWood[2];
    for (const sd of [-1, 1]) g.circle(...P(sd * (hw + 0.005), (bk - 0.12) / 2), grand ? 0.045 : 0.032).fill(trim).stroke({ width: 0.012, color: INK });
    if (grand) {
      const [mx, my] = P(0, (bk - 0.12) / 2);
      g.ellipse(mx, my, 0.07, 0.045).fill(GILT[1]).stroke({ width: 0.012, color: INK });
      g.ellipse(mx + light[0] * 0.012, my + light[1] * 0.012, 0.035, 0.022).fill(GILT[2]);
    }
  };
  const neg = (d: Pt): Pt => [-d[0], -d[1]];
  const n = Math.max(1, Math.round((len - 1.2) / 0.75));
  for (let i = 0; i < n; i++) {
    const u = 0.6 + ((len - 1.2) * (i + 0.5)) / n;
    const off = half(u) + 0.17;
    seat(at(u, cs - off), U, S, false);
    seat(at(u, cs + off), U, neg(S), false);
  }
  seat(at(cu - rx - 0.17, cs), neg(S), U, true);
  seat(at(cu + rx + 0.17, cs), S, neg(U), false);
  // The top: dark rim, a lit edge toward the light, the face; plank seams down it.
  castShadow(g, shape(0), light, 0.12);
  g.poly(shape(0)).fill(wood[0]).stroke({ width: OUT, color: INK, join: 'round' });
  g.poly(shape(0.045, light[0] * 0.03, light[1] * 0.03)).fill(wood[2]);
  g.poly(shape(0.065, light[0] * 0.012, light[1] * 0.012)).fill(wood[1]);
  for (const o of [-ry / 3, ry / 3]) {
    const reach = rx * Math.pow(Math.max(0, 1 - Math.pow(Math.abs(o) / ry, p)), 1 / p) - 0.12;
    g.moveTo(...at(cu - reach, cs + o)).lineTo(...at(cu + reach, cs + o + (rnd() - 0.5) * 0.01));
  }
  g.stroke({ width: LINE, color: wood[0], alpha: 0.8 });
  for (let i = 0; i < 3 * len; i++) {
    const s0 = cs + (rnd() - 0.5) * ry * 1.4;
    const u0 = cu - rx * 0.6 + rnd() * rx;
    g.moveTo(...at(u0, s0)).quadraticCurveTo(...at(u0 + 0.15, s0 + (rnd() - 0.5) * 0.03), ...at(u0 + 0.3, s0));
  }
  g.stroke({ width: 0.014, color: wood[0], alpha: 0.55, cap: 'round' });
  if (v === 1) g.poly(shape(0.2)).fill(0x3e5e3a).stroke({ width: LINE, color: 0x2a3a26 });
  if (v === 3) {
    g.poly(quad(rect(cu - rx - 0.04, cs - 0.15, cu + rx + 0.04, cs + 0.15), rnd, 0.003)).fill(0x2e4a7a).stroke({ width: LINE, color: INK });
    for (const o of [-0.1, 0.1]) g.moveTo(...at(cu - rx, cs + o)).lineTo(...at(cu + rx, cs + o));
    g.stroke({ width: 0.012, color: GILT[2], alpha: 0.8 });
  }
}
