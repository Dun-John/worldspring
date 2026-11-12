// The trace picture's session state: the image, where it lies, how strongly it shows, and the
// "Adjust" drag logic. Held in memory only: never in the world file, IndexedDB, links, exports,
// mapd or the players' window. Slice 2b reads `underlay.image` / `underlay.xf` (picks, preview).
import { imageToWorld, worldToImage, type TraceImage, type TraceXform } from './types';

const MI = 5280;
/** Handle size and the rotate handle's reach (screen px). */
export const HANDLE_PX = 9;
export const ROTATE_PX = 30;

type Drag = { kind: 'move'; dx: number; dy: number } | { kind: 'scale'; half: number } | { kind: 'rotate' };

class Underlay {
  image = $state.raw<TraceImage | null>(null);
  xf = $state<TraceXform>({ cx: 0, cy: 0, ftPerPx: 1, rot: 0 });
  opacity = $state(0.5);
  shown = $state(true);
  adjust = $state(false);
  /** The map's size (ft), for Fit. */
  private map: [number, number] = [1, 1];
  private drag: Drag | null = null;

  setMap(wFt: number, hFt: number) {
    this.map = [wFt, hFt];
  }

  /** Take a picture file (images only); false if it isn't one or can't be read. */
  async load(file: File): Promise<boolean> {
    if (!file.type.startsWith('image/')) return false;
    let bitmap: ImageBitmap;
    try {
      bitmap = await createImageBitmap(file);
    } catch {
      return false;
    }
    this.image?.bitmap.close();
    this.image = { bitmap, w: bitmap.width, h: bitmap.height };
    this.shown = true;
    this.fit();
    return true;
  }

  /** Centre it on the map and scale it so the whole picture fits. */
  fit() {
    const im = this.image;
    if (!im) return;
    this.xf = { cx: this.map[0] / 2, cy: this.map[1] / 2, ftPerPx: Math.min(this.map[0] / im.w, this.map[1] / im.h), rot: 0 };
  }

  remove() {
    this.image?.bitmap.close();
    this.image = null;
    this.adjust = false;
    this.drag = null;
  }

  /** Width across (miles), for the panel's field. */
  get widthMi() {
    return this.image ? (this.image.w * this.xf.ftPerPx) / MI : 0;
  }
  set widthMi(mi: number) {
    if (this.image && mi > 0) this.xf.ftPerPx = (mi * MI) / this.image.w;
  }

  /** Is the picture in view and drawn (sketch tab open, an image, not hidden)? */
  get visible() {
    return !!this.image && this.shown;
  }

  /** The handles' places (world ft): corners, and the rotate knob out from the top edge. */
  handles(ppf: number): { corners: [number, number][]; rotate: [number, number]; top: [number, number] } | null {
    const im = this.image;
    if (!im) return null;
    const c = ([[0, 0], [im.w, 0], [im.w, im.h], [0, im.h]] as [number, number][]).map(([x, y]) => imageToWorld(im.w, im.h, this.xf, x, y));
    const top = imageToWorld(im.w, im.h, this.xf, im.w / 2, 0);
    const a = (this.xf.rot * Math.PI) / 180;
    const k = ROTATE_PX / ppf;
    return { corners: c, top, rotate: [top[0] + Math.sin(a) * k, top[1] - Math.cos(a) * k] };
  }

  /** A press at world (x, y) while adjusting: does it take hold of the picture? */
  adjustDown(x: number, y: number, ppf: number): boolean {
    const im = this.image;
    if (!im || !this.adjust) return false;
    const hit = HANDLE_PX * 1.8 / ppf;
    const hs = this.handles(ppf)!;
    if (Math.hypot(x - hs.rotate[0], y - hs.rotate[1]) <= hit) {
      this.drag = { kind: 'rotate' };
      return true;
    }
    if (hs.corners.some((p) => Math.hypot(x - p[0], y - p[1]) <= hit)) {
      this.drag = { kind: 'scale', half: Math.hypot(im.w, im.h) / 2 };
      return true;
    }
    const [px, py] = worldToImage(im.w, im.h, this.xf, x, y);
    if (px >= 0 && py >= 0 && px <= im.w && py <= im.h) {
      this.drag = { kind: 'move', dx: x - this.xf.cx, dy: y - this.xf.cy };
      return true;
    }
    return false;
  }

  adjustMove(x: number, y: number) {
    const d = this.drag;
    if (!d) return;
    const xf = this.xf;
    if (d.kind === 'move') {
      xf.cx = x - d.dx;
      xf.cy = y - d.dy;
    } else if (d.kind === 'scale') {
      const r = Math.hypot(x - xf.cx, y - xf.cy);
      if (r > 1) xf.ftPerPx = r / d.half;
    } else {
      xf.rot = (Math.atan2(y - xf.cy, x - xf.cx) * 180) / Math.PI + 90;
      xf.rot = Math.round((((xf.rot + 180) % 360) + 360) % 360 - 180);
    }
  }

  adjustUp() {
    this.drag = null;
  }

  get dragging() {
    return this.drag !== null;
  }
}

export const underlay = new Underlay();
