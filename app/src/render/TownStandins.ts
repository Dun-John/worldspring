// A town's plan alone (`town::standin`: ward patches, main streets, bridges, wall and gates),
// drawn in place of its streets while the tile with its real layout is still being made: at the
// zooms where tiles draw towns (`SITE_MIN_LEVEL` on), over a town drawn from a coarser tile.
// In the tiles' own colours, a low-detail version of the same plan; it fades out as the real
// tile fades in. A town whose tile comes in first never shows one.
import { Container, Graphics } from 'pixi.js';
import type { TownStandin } from '../gen/protocol';
import type { Rect } from '../gen/protocol';
import type { Camera } from './camera';

// (The tiles' colours: `tileGeometry` a block (kind 3), a plaza (1), a park's ward; `tileroad.frag`
// a main street and a street from the city zooms out; `tileink.frag` a limestone wall in its ink.)
const BLOCK = 0x9e8f80;
const PLAZA = 0xdbd1b5;
const PARK = 0x999e80;
const MAIN_STREET = 0xe0d9c2;
const STREET = 0xc2b8a1;
const WALL = 0xdbcca6;
const INK = 0x2b241c;
const WARD_PLAZA = 0;
const WARD_PARK = 12;
/** Main streets, the streets between patches and the wall (ft), each at least so many px. */
const MAIN_FT = 18;
const MAIN_PX = 2;
const LANE_FT = 7;
const LANE_PX = 1;
const WALL_FT = 9;
const WALL_PX = 2.5;
/** How fast it comes and goes (ms, an exponential's time constant). */
const EASE_MS = 120;

interface Town {
  index: number;
  x: number;
  y: number;
  r: number;
}

interface Drawn {
  g: Graphics;
  /** Its origin (the town's middle, world ft): drawn about it, so float32 holds the detail. */
  o: [number, number];
  /** The ft per px it was drawn for (its lines' least widths). */
  ftpx: number;
  alpha: number;
}

export class TownStandins {
  readonly container = new Container();
  /** Plans by settlement (null: none to draw), asked once each per edit generation. */
  private plans = new Map<number, TownStandin | null>();
  private asked = new Set<number>();
  private drawn = new Map<number, Drawn>();
  private generation = 0;
  private last = 0;
  /** Stand-ins showing now (for checks). */
  shown = 0;

  constructor(private readonly ask: (town: number) => Promise<TownStandin | null>) {}

  /** The world's edits changed: plans are asked again. */
  reset() {
    this.generation++;
    this.plans.clear();
    this.asked.clear();
    for (const d of this.drawn.values()) d.g.destroy();
    this.drawn.clear();
  }

  /** `ready`: how far the real plan is drawn over a rect (`TileLayer.siteReady`); `show`: false
   * where the tiles aren't drawn. */
  update(cam: Camera, now: number, towns: readonly Town[], ready: (r: Rect, now: number) => number | null, show: boolean) {
    const dt = Math.min(200, Math.max(0, now - this.last));
    this.last = now;
    const k = 1 - Math.exp(-dt / EASE_MS);
    const [x0, y0, x1, y1] = cam.viewRect();
    const want = new Map<number, number>();
    if (show) {
      for (const t of towns) {
        const r = t.r * 1.3;
        if (t.x + r < x0 || t.x - r > x1 || t.y + r < y0 || t.y - r > y1) continue;
        const done = ready([t.x - r, t.y - r, t.x + r, t.y + r], now);
        if (done === null || done >= 1) continue;
        want.set(t.index, 1 - done);
        if (!this.asked.has(t.index)) {
          this.asked.add(t.index);
          const gen = this.generation;
          void this.ask(t.index).then(
            (p) => gen === this.generation && this.plans.set(t.index, p),
            () => gen === this.generation && this.plans.set(t.index, null),
          );
        }
      }
    }
    for (const [i, p] of this.plans) if (p && want.has(i) && !this.drawn.has(i)) {
        const t = towns.find((t) => t.index === i)!;
        this.drawn.set(i, { g: this.container.addChild(new Graphics()), o: [t.x, t.y], ftpx: 0, alpha: 0 });
      }
    const ftpx = 1 / cam.ppf;
    this.shown = 0;
    for (const [i, d] of this.drawn) {
      d.alpha += ((want.get(i) ?? 0) - d.alpha) * k;
      if (!want.has(i) && d.alpha < 0.01) {
        d.g.destroy();
        this.drawn.delete(i);
        continue;
      }
      if (d.ftpx === 0 || d.ftpx / ftpx > 1.25 || ftpx / d.ftpx > 1.25) {
        d.ftpx = ftpx;
        draw(d.g, this.plans.get(i)!, ftpx, d.o);
      }
      const [sx, sy] = cam.worldToScreen(d.o[0], d.o[1]);
      d.g.position.set(sx, sy);
      d.g.scale.set(cam.ppf);
      d.g.alpha = d.alpha;
      this.shown++;
    }
    this.container.visible = this.drawn.size > 0;
  }

  destroy() {
    this.reset();
    this.container.destroy({ children: true });
  }
}

/** A flat point list as pairs (world ft), about `o`. */
function line(g: Graphics, pts: number[], o: [number, number], from = 0) {
  g.moveTo(pts[from] - o[0], pts[from + 1] - o[1]);
  for (let k = from + 2; k + 1 < pts.length; k += 2) g.lineTo(pts[k] - o[0], pts[k + 1] - o[1]);
}

function draw(g: Graphics, p: TownStandin, ftpx: number, o: [number, number]) {
  g.clear();
  const local = (pts: number[]) => pts.map((v, k) => v - o[k & 1]);
  const lane = Math.max(LANE_FT, LANE_PX * ftpx);
  for (const patch of p.p) {
    if (patch.length < 7) continue;
    const ward = patch[0];
    g.poly(local(patch.slice(1)), true)
      .fill({ color: ward === WARD_PLAZA ? PLAZA : ward === WARD_PARK ? PARK : BLOCK })
      .stroke({ width: lane, color: STREET, join: 'round' });
  }
  const main = Math.max(MAIN_FT, MAIN_PX * ftpx);
  for (const s of [...p.s, ...p.b]) {
    if (s.length < 4) continue;
    line(g, s, o);
    g.stroke({ width: main, color: MAIN_STREET, join: 'round', cap: 'round' });
  }
  const wall = Math.max(WALL_FT, WALL_PX * ftpx);
  for (const w of p.w) {
    if (w.length < 5) continue;
    line(g, w, o, 1);
    if (w[0]) g.closePath();
    g.stroke({ width: wall + 2 * ftpx, color: INK, join: 'round', cap: 'round' });
    line(g, w, o, 1);
    if (w[0]) g.closePath();
    g.stroke({ width: wall, color: WALL, join: 'round', cap: 'round' });
  }
  // A gate: the main street through the wall.
  for (let k = 0; k + 1 < p.g.length; k += 2) g.circle(p.g[k] - o[0], p.g[k + 1] - o[1], Math.max(main, wall) * 0.75).fill({ color: MAIN_STREET });
}
