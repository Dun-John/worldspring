// Water drawn by hand (`Edits.waters`) where the tiles don't draw it: coarser than the three finest
// levels (those shape it into the terrain, `payload::shaped`, and draw it as the sea's water). The
// outlines' fill in the shallow water's colour over an ink rim drawn first, so overlapping outlines
// are one water with one shore line. Each outline is drawn by its shaped shore (`water.ts` `shore`). An inlet or basin joins the sea's water: its fill only, no rim.
import { AlphaFilter, Container, Graphics } from 'pixi.js';
import type { Water } from '../gen/protocol';
import type { Camera } from './camera';
import { shore } from '../editor/water';

/** The terrain shader's shallow water (`terrain.frag`: 0.70, 0.78, 0.77) and its shore ink. */
const FILL = 0xb3c7c4;
const INK = 0x332e29;
/** The rim's width (px): half of it shows outside the fill. */
const RIM_PX = 3;

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

  /** Drawn for the camera at `fade` (1 shown, 0 gone: the tiles draw it at the finest levels). */
  update(cam: Camera, fade: number) {
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
    const key = `${cam.ppf}|${x0}|${y0}|${x1}|${y1}`;
    if (key === this.key) return;
    this.key = key;
    const g = this.g;
    g.clear();
    this.drawn = 0;
    const shown: { pts: number[]; rim: boolean }[] = [];
    for (const [, w] of this.waters) {
      let [bx0, by0, bx1, by1] = [Infinity, Infinity, -Infinity, -Infinity];
      const pts0 = shore(w);
      for (const [x, y] of pts0) [bx0, by0, bx1, by1] = [Math.min(bx0, x), Math.min(by0, y), Math.max(bx1, x), Math.max(by1, y)];
      if (bx1 < x0 || bx0 > x1 || by1 < y0 || by0 > y1) continue;
      if (Math.max(bx1 - bx0, by1 - by0) * cam.ppf < 1) continue;
      const pts: number[] = [];
      for (const [x, y] of pts0) pts.push(...cam.worldToScreen(x, y));
      shown.push({ pts, rim: w.kind !== 'inlet' && w.kind !== 'basin' });
    }
    for (const s of shown) if (s.rim) g.poly(s.pts, true).stroke({ width: RIM_PX, color: INK, alpha: 0.9, join: 'round' });
    for (const s of shown) g.poly(s.pts, true).fill({ color: FILL });
    this.drawn = shown.length;
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}
