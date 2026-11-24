// Options for the tower and keep flags (`tops.ts` `banner`, sprite audit R7 P8b), shown in the gallery's
// `&panels=options`; not drawn on the map yet. Strictly from above: the pole is a dot, the flag its rippling
// ribbon streaming downwind, its shadow cast well off toward the SE on the roof (the flag flies high). Atlas
// pixels (`PX` a square), the same call as `banner(g, x, y)`.
import type { Graphics } from 'pixi.js';
import { PX } from '../scale';
import { INK, LINE } from '../pen';

/** The flag's ribbon round a pole at (x, y): `w` wide (squares), `len` long, `ripples` waves, as a polygon
 * and, for two tones, the ribbon cut into its half-waves. */
function ribbon(x: number, y: number, w: number, len: number, ripples: number): { pts: number[]; halves: number[][]; north: [number, number][] } {
  const n = 28;
  const mid: [number, number][] = [];
  for (let k = 0; k <= n; k++) {
    const t = k / n;
    // The wave grows downwind; a little lopsided so it isn't a sine from a book.
    const amp = 0.055 * (0.35 + t * 0.9);
    mid.push([x + t * len * PX, y + Math.sin(t * Math.PI * 2 * ripples + 0.4 + t * 0.6) * amp * PX]);
  }
  const left: [number, number][] = [];
  const right: [number, number][] = [];
  for (let k = 0; k <= n; k++) {
    const [ax, ay] = mid[Math.max(0, k - 1)];
    const [bx, by] = mid[Math.min(n, k + 1)];
    const m = Math.hypot(bx - ax, by - ay) || 1;
    const [nx, ny] = [-(by - ay) / m, (bx - ax) / m];
    const hw = (w / 2) * (1 - (k / n) * 0.3) * PX;
    left.push([mid[k][0] + nx * hw, mid[k][1] + ny * hw]);
    right.push([mid[k][0] - nx * hw, mid[k][1] - ny * hw]);
  }
  // A shallow swallowtail notch at the fly.
  const tail: [number, number] = [mid[n][0] - 0.1 * PX, mid[n][1]];
  const pts = [...left, tail, ...[...right].reverse()].flat();
  const halves: number[][] = [];
  const per = n / (ripples * 2);
  for (let h = 0; h < ripples * 2; h++) {
    const k0 = Math.round(h * per);
    const k1 = Math.min(n, Math.round((h + 1) * per));
    const q: [number, number][] = [...left.slice(k0, k1 + 1), ...right.slice(k0, k1 + 1).reverse()];
    halves.push(q.flat());
  }
  // `left` is the south edge (y down), `right` the north.
  return { pts, halves, north: right };
}

/** Flag option B (`wide` false) and C (`wide` true): the pole a small ink-ringed dot, a thin rippling ribbon
 * (B 0.11 square wide, C 0.16) about 1.2 squares long streaming east in the flag's colour, B one tone with a lit
 * edge to the north, C its half-waves in two tones (those turned to the light lighter); the soft shadow of
 * flag and pole on the roof, offset to the SE. */
export function bannerFromAbove(g: Graphics, x: number, y: number, wide = false, color = 0x9b2226) {
  const [w, ripples] = wide ? [0.16, 2] : [0.11, 1.5];
  const len = 1.2;
  const [dx, dy] = [0.42 * PX, 0.55 * PX];
  const { pts, halves, north } = ribbon(x, y, w, len, ripples);
  // The shadow: pole and flag, soft.
  g.moveTo(x, y).lineTo(x + dx, y + dy).stroke({ width: 0.035 * PX, color: 0x14100c, alpha: 0.16, cap: 'round' });
  g.poly(pts.map((p, k) => p + (k % 2 ? dy : dx))).fill({ color: 0x14100c, alpha: 0.18 });
  const tone = (f: number) => {
    const ch = (s: number) => Math.min(255, Math.round(((color >> s) & 255) * f));
    return (ch(16) << 16) | (ch(8) << 8) | ch(0);
  };
  g.poly(pts).fill(color);
  if (wide) {
    // Alternate half-waves face up into the light or away from it.
    halves.forEach((q, h) => g.poly(q).fill(h % 2 ? tone(0.72) : tone(1.18)));
  } else {
    // The lit north edge of the hem.
    g.moveTo(north[1][0], north[1][1] + 0.022 * PX);
    for (const [px, py] of north.slice(2, -3)) g.lineTo(px, py + 0.022 * PX);
    g.stroke({ width: 0.018 * PX, color: tone(1.3), alpha: 0.9 });
  }
  g.poly(pts).stroke({ width: LINE, color: INK, join: 'round' });
  // The pole: a dot with its gilt cap.
  g.circle(x, y, 0.07 * PX).fill(0x725440).stroke({ width: LINE, color: INK });
  g.circle(x - 0.015 * PX, y - 0.015 * PX, 0.032 * PX).fill(0xc8a04a);
}
