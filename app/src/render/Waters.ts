// Water drawn by hand (`Edits.waters`) over the tiles: coarser than the three finest levels it is
// all there is of it; at those the terrain has it too (`payload::shaped`, drawn as the sea's
// water) and this gives each kind its look over that, until the battlemap comes in. The
// outlines' fill in the shallow water's colour over an ink rim drawn first, so overlapping outlines
// are one water with one shore line. Each outline is drawn by its shaped shore (`water.ts` `shore`).
// Each kind has its look, in a few flat shapes: a pond lighter and greener; a lake the usual blue,
// darker toward its middle; an inlet the sea's own, no rim (it joins the sea's water); a basin deep
// and dark inside a pale stone quay; a moat dark and still with a thin ink line along each side; a
// stream a thin line of shallow water at its width (1.5 px at least), inked each side.
import { AlphaFilter, Container, Graphics } from 'pixi.js';
import type { Water } from '../gen/protocol';
import type { Camera } from './camera';
import { centre, contains, shore, shoreRings, widthOf } from '../editor/water';

/** The terrain shader's shallow water (`terrain.frag`: 0.70, 0.78, 0.77) and its shore ink. */
const FILL = 0xb3c7c4;
const INK = 0x332e29;
/** Each kind's water: a pond's lighter and greener, a lake's middle darker, a basin's and moat's deep. */
const POND = 0xc0d4c2;
const LAKE_MID = 0xa1b8b9;
const BASIN = 0x89a3b0;
const MOAT = 0x95abb4;
const STREAM = 0xbbcfcc;
/** A basin's quay: pale stone, 6 ft wide (2 px at least). */
const QUAY = 0xd9d3c6;
const QUAY_FT = 6;
/** The rim's width (px): half of it shows outside the fill. */
const RIM_PX = 3;
/** A moat's ink each side, and a stream's narrowest (px). */
const THIN_PX = 2;
const STREAM_MIN_PX = 1.5;

type Shown = { kind: Water['kind']; pts: number[]; rings: number[][]; mid: number[] | null; line: number[] | null; px: number };

const FILLS: Record<Water['kind'], number> = { inlet: FILL, basin: BASIN, lake: FILL, pond: POND, moat: MOAT, stream: STREAM };

export class Waters {
  readonly container = new Container();
  private readonly g = new Graphics();
  private readonly fader = new AlphaFilter();
  private waters: [string, Water][] = [];
  private key = '';
  /** Outlines drawn last (for checks). */
  drawn = 0;

  constructor() {
    this.container.addChild(this.g);
    this.container.visible = false;
  }

  set(waters: Record<string, Water> | undefined) {
    const next = Object.entries(waters ?? {}).filter(([, w]) => w.pts.length >= 3);
    this.waters = next;
    this.key = '';
  }

  /** Drawn for the camera at `fade` (1 shown, 0 gone: the battlemap is in); `fine`: at the
   * finest levels, where the terrain draws it too (an inlet there is the sea's own water). */
  update(cam: Camera, fade: number, fine = false) {
    this.container.visible = fade > 0.01 && this.waters.length > 0;
    if (!this.container.visible) {
      this.drawn = 0;
      this.key = '';
      return;
    }
    // Faded as one picture (an alpha on the container would fade each fill alone, and where
    // outlines overlap the two would show through each other).
    if (fade < 0.99) {
      this.fader.alpha = fade;
      if (!this.container.filters) this.container.filters = [this.fader];
    } else if (this.container.filters) this.container.filters = null;
    const [x0, y0, x1, y1] = cam.viewRect();
    const key = `${cam.ppf}|${x0}|${y0}|${x1}|${y1}|${fine}`;
    if (key === this.key) return;
    this.key = key;
    const g = this.g;
    g.clear();
    this.drawn = 0;
    const shown: Shown[] = [];
    const flat = (q: [number, number][]) => {
      const out: number[] = [];
      for (const [x, y] of q) out.push(...cam.worldToScreen(x, y));
      return out;
    };
    for (const [, w] of this.waters) {
      if (fine && w.kind === 'inlet') continue;
      let [bx0, by0, bx1, by1] = [Infinity, Infinity, -Infinity, -Infinity];
      const pts0 = shore(w);
      for (const [x, y] of pts0) [bx0, by0, bx1, by1] = [Math.min(bx0, x), Math.min(by0, y), Math.max(bx1, x), Math.max(by1, y)];
      if (bx1 < x0 || bx0 > x1 || by1 < y0 || by0 > y1) continue;
      if (Math.max(bx1 - bx0, by1 - by0) * cam.ppf < 1) continue;
      const stream = w.kind === 'stream';
      shown.push({
        kind: w.kind,
        pts: stream ? [] : flat(pts0),
        rings: w.kind === 'moat' ? shoreRings(w).map(flat) : [],
        mid: w.kind === 'lake' ? lakeMiddle(pts0) : null,
        line: stream ? flat(centre(w)) : null,
        px: stream ? Math.max(STREAM_MIN_PX, widthOf(w) * cam.ppf) : Math.max(2, QUAY_FT * cam.ppf),
      });
    }
    // Under every fill: the rims (a basin's quay with ink each side of it), so overlapping water is one.
    for (const s of shown) {
      if (s.line) {
        line(g, s.line).stroke({ width: s.px + 2, color: INK, alpha: 0.85, join: 'round', cap: 'round' });
      } else if (s.kind === 'basin') {
        g.poly(s.pts, true).stroke({ width: 2 * s.px + 2, color: INK, alpha: 0.85, join: 'round' });
        g.poly(s.pts, true).stroke({ width: 2 * s.px, color: QUAY, join: 'round' });
        g.poly(s.pts, true).stroke({ width: 2, color: INK, alpha: 0.7, join: 'round' });
      } else if (s.kind === 'moat') {
        for (const r of s.rings) g.poly(r, true).stroke({ width: THIN_PX, color: INK, alpha: 0.9, join: 'round' });
      } else if (s.kind !== 'inlet') g.poly(s.pts, true).stroke({ width: RIM_PX, color: INK, alpha: 0.9, join: 'round' });
    }
    for (const s of shown) {
      if (s.line) line(g, s.line).stroke({ width: s.px, color: FILLS.stream, join: 'round', cap: 'round' });
      else g.poly(s.pts, true).fill({ color: FILLS[s.kind] });
    }
    for (const s of shown) {
      if (!s.mid) continue;
      const m: number[] = [];
      for (let i = 0; i < s.mid.length; i += 2) m.push(...cam.worldToScreen(s.mid[i], s.mid[i + 1]));
      g.poly(m, true).fill({ color: LAKE_MID });
    }
    this.drawn = shown.length;
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}

function line(g: Graphics, pts: number[]): Graphics {
  g.moveTo(pts[0], pts[1]);
  for (let i = 2; i < pts.length; i += 2) g.lineTo(pts[i], pts[i + 1]);
  return g;
}

const middles = new WeakMap<[number, number][], number[] | null>();

/** A lake's darker middle (world ft, flat): its shore drawn in by a third of its mean half-width,
 * none where that would fold over itself or the lake is small. */
function lakeMiddle(sh: [number, number][]): number[] | null {
  if (middles.has(sh)) return middles.get(sh)!;
  let [a, per] = [0, 0];
  for (let i = 0; i < sh.length; i++) {
    const [p, q] = [sh[i], sh[(i + 1) % sh.length]];
    a += p[0] * q[1] - q[0] * p[1];
    per += Math.hypot(q[0] - p[0], q[1] - p[1]);
  }
  a /= 2;
  const hw = per > 0 ? (2 * Math.abs(a)) / per : 0;
  let out: number[] | null = null;
  if (hw >= 40) {
    // (The shore drawn in toward its middle, kept where all of it stays inside the shore.)
    let [cx, cy] = [0, 0];
    for (const [x, y] of sh) [cx, cy] = [cx + x / sh.length, cy + y / sh.length];
    const inset: [number, number][] = sh.map(([x, y]) => [cx + (x - cx) * 0.6, cy + (y - cy) * 0.6]);
    if (contains(sh, cx, cy) && inset.every(([x, y]) => contains(sh, x, y)) && !crossesAny(sh, inset)) out = inset.flat();
  }
  middles.set(sh, out);
  return out;
}

function crossesAny(p: [number, number][], q: [number, number][]): boolean {
  const sd = (a: number[], b: number[], c: number[]) => (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  for (let i = 0; i < p.length; i++) {
    const [a, b] = [p[i], p[(i + 1) % p.length]];
    for (let j = 0; j < q.length; j++) {
      const [c, d] = [q[j], q[(j + 1) % q.length]];
      if (sd(a, b, c) * sd(a, b, d) < 0 && sd(c, d, a) * sd(c, d, b) < 0) return true;
    }
  }
  return false;
}
