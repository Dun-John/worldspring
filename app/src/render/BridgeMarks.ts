// Bridges at region zoom (the overlay's `bridges`): where a road crosses a river, two short
// brackets `)(` either side of the river, across the road, in the road's ink. The tiles draw the
// roads only as thin lines at this zoom; the battlemap draws the real bridge, and covers these.
// Fords and ferries get no mark (a ford is the road wading, a ferry has its own dashed line).
import { Container, Graphics } from 'pixi.js';
import type { Camera } from './camera';

/** One crossing: x, y (ft), the road's unit direction (x, y), class, kind, river width (ft). */
export type BridgeRow = number[];

const INK = 0x5c3824;
const INK_TRACK = 0x80603d;
/** Half width of the deck (px) by road class: king's road, road, track. */
const HALF = [5, 4.2, 3.4];
const MIN_LEN = 20;
const PAPER = 0xe8d8b0;
const KIND_BRIDGE = 0;
/** Gone again toward the whole map (ft per px), where a mark would be a speck. */
const FAR_FROM = 250;
const FAR_TO = 600;

const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/** How much of the road class shows at `ftpx` ft per px (the tile road shader's own fade). */
function roadShown(cls: number, ftpx: number): number {
  if (cls < 0.5) return 1;
  if (cls < 1.5) return 1 - smooth(2500, 4500, ftpx);
  return 1 - smooth(6, 14, ftpx);
}

export class BridgeMarks {
  readonly container = new Container();
  private readonly g = new Graphics();
  private readonly rows: BridgeRow[];
  /** Marks drawn last (for checks). */
  drawn = 0;
  private key = '';

  constructor(rows: BridgeRow[]) {
    this.rows = rows.filter((r) => r[5] === KIND_BRIDGE);
    this.container.addChild(this.g);
  }

  get total(): number {
    return this.rows.length;
  }

  /** Drawn for the camera; `show`: false where the tiles aren't (inside, a battlemap covering). */
  update(cam: Camera, show: boolean) {
    const ftpx = 1 / cam.ppf;
    // Gone as the view comes close enough for the bridge's own build to read (the battlemap's).
    const near = smooth(3, 8, ftpx) * (1 - smooth(FAR_FROM, FAR_TO, ftpx));
    this.container.visible = show && near > 0.01 && this.rows.length > 0;
    if (!this.container.visible) {
      this.drawn = 0;
      this.key = '';
      return;
    }
    const [x0, y0, x1, y1] = cam.viewRect();
    const key = `${cam.ppf}|${x0}|${y0}|${x1}|${y1}`;
    if (key === this.key) return;
    this.key = key;
    const g = this.g;
    g.clear();
    this.drawn = 0;
    const pad = 200 * ftpx;
    for (const [x, y, dx, dy, cls, , w] of this.rows) {
      if (x < x0 - pad || x > x1 + pad || y < y0 - pad || y > y1 + pad) continue;
      const a = near * roadShown(cls, ftpx);
      if (a < 0.02) continue;
      const [sx, sy] = cam.worldToScreen(x, y);
      // A small bridge: the deck along the road across the river, a little wider than the road,
      // with a light rail either side and a short post at each end.
      const hw = HALF[Math.min(2, cls)];
      const len = Math.max(MIN_LEN, w * cam.ppf + 8) * 0.5;
      const [nx, ny] = [-dy, dx];
      const ink = cls > 1.5 ? INK_TRACK : INK;
      for (const s of [-1, 1]) {
        g.moveTo(sx - dx * len + nx * hw * s, sy - dy * len + ny * hw * s);
        g.lineTo(sx + dx * len + nx * hw * s, sy + dy * len + ny * hw * s);
        g.moveTo(sx - dx * len + nx * hw * s, sy - dy * len + ny * hw * s);
        g.lineTo(sx - dx * len + nx * hw * (s - 0.5), sy - dy * len + ny * hw * (s - 0.5));
        g.moveTo(sx + dx * len + nx * hw * s, sy + dy * len + ny * hw * s);
        g.lineTo(sx + dx * len + nx * hw * (s - 0.5), sy + dy * len + ny * hw * (s - 0.5));
      }
      g.stroke({ width: 1, color: ink, alpha: a * 0.7, cap: 'round' });
      g.moveTo(sx - dx * len, sy - dy * len);
      g.lineTo(sx + dx * len, sy + dy * len);
      g.stroke({ width: hw * 1.5, color: PAPER, alpha: a * 0.85, cap: 'butt' });
      g.moveTo(sx - dx * len, sy - dy * len);
      g.lineTo(sx + dx * len, sy + dy * len);
      g.stroke({ width: cls > 1.5 ? 1.4 : 2, color: ink, alpha: a * 0.9, cap: 'butt' });
      this.drawn++;
    }
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}
