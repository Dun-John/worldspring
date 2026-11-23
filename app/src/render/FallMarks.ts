// Waterfalls on the map, region and site zooms: a pale curtain band across the river with a few
// streaks, an inked lip and a foam pool below (`drawFallMark`), as wide as the river where that
// reads, else the small map symbol. Far out only the notable falls show (the higher the drop, the
// farther out), so the continent isn't wallpapered with them. The battlemap draws the real fall over it.
import { Container, Graphics } from 'pixi.js';
import type { Camera } from './camera';
import type { GenFall } from '../gen/protocol';
import { drawFallMark, sheetRun } from './fallShapes';

const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

/** The smallest mark (px across): the map symbol. */
const MIN_W = 20;

/** Continent zoom starts here (ft per px, MapView's TIERS): no fall marks from here out. */
const CONTINENT_FTPX = 1500;

/** The drop (ft) a fall needs to show at `ftpx` ft per px: every fall at site zoom, only the
 * highest toward the whole map. */
export function dropShownFrom(ftpx: number): number {
  return ftpx <= 20 ? 0 : 70 * Math.log2(ftpx / 20);
}

export class FallMarks {
  readonly container = new Container();
  private readonly g = new Graphics();
  private falls: GenFall[] = [];
  /** Marks drawn last (for checks). */
  drawn = 0;
  private key = '';

  constructor() {
    this.container.addChild(this.g);
    this.container.visible = false;
  }

  /** The widest channel and longest sheet (ft) of any fall: they bound a mark's reach for culling. */
  private maxWidth = 0;
  private maxRun = 0;

  set(falls: GenFall[]) {
    // Sorted by x once, so a camera move only walks the falls in the view's columns.
    this.falls = [...falls].sort((a, b) => a.x - b.x);
    this.maxWidth = 0;
    this.maxRun = 0;
    for (const f of falls) {
      this.maxWidth = Math.max(this.maxWidth, f.width);
      this.maxRun = Math.max(this.maxRun, sheetRun(f));
    }
    this.key = '';
  }

  get total(): number {
    return this.falls.length;
  }

  /** Drawn for the camera; `show`: false where the tiles aren't; `under`: how far the battlemap
   * has come in over them (0-1: the mark fades out under its own fall). */
  update(cam: Camera, show: boolean, under: number) {
    const ftpx = 1 / cam.ppf;
    // Gone at continent zoom (1500 ft/px and out), faded out over the last of region zoom.
    const far = Math.min(1, Math.max(0, (CONTINENT_FTPX - ftpx) / 500));
    const fade = (1 - Math.min(1, Math.max(0, under))) * far;
    this.container.visible = show && fade > 0.02 && this.falls.length > 0;
    if (!this.container.visible) {
      this.drawn = 0;
      this.key = '';
      return;
    }
    this.container.alpha = fade;
    const [x0, y0, x1, y1] = cam.viewRect();
    const key = `${cam.ppf}|${x0}|${y0}|${x1}|${y1}`;
    if (key === this.key) return;
    this.key = key;
    const g = this.g;
    g.clear();
    this.drawn = 0;
    const need = dropShownFrom(ftpx);
    // (The farthest any mark reaches past its spot, as `pad` below at its largest.)
    const wMax = Math.max(MIN_W, this.maxWidth * cam.ppf * 1.35);
    const padMax = (wMax + Math.max(wMax * 0.5, this.maxRun * cam.ppf, 5)) * 2 * ftpx;
    const falls = this.falls;
    let lo = 0;
    let hi = falls.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (falls[mid].x < x0 - padMax) lo = mid + 1;
      else hi = mid;
    }
    for (let i = lo; i < falls.length && falls[i].x <= x1 + padMax; i++) {
      const f = falls[i];
      const a = smooth(need - 25, need + 25, f.drop);
      if (a < 0.02) continue;
      // (A little wider than the channel at the lip: the tiles draw the river a touch wide.)
      const w = Math.max(MIN_W, f.width * cam.ppf * 1.35);
      const l = Math.max(w * 0.5, sheetRun(f) * cam.ppf, 5);
      const pad = (w + l) * 2 * ftpx;
      if (f.x < x0 - pad || f.x > x1 + pad || f.y < y0 - pad || f.y > y1 + pad) continue;
      const [sx, sy] = cam.worldToScreen(f.x, f.y);
      drawFallMark(g, sx, sy, f.dir[0], f.dir[1], w, l, a);
      this.drawn++;
    }
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}
