// Map camera. World units are feet (f64 in JS numbers); zoom is log2(CSS pixels per foot).
// Tile placement is computed relative to the camera, so GPU-side values stay small and
// f32 precision never shows at battlemap zoom.

export interface CameraState {
  cx: number;
  cy: number;
  zoom: number;
}

export class Camera implements CameraState {
  cx = 0;
  cy = 0;
  zoom = 0;
  minZoom = -14;
  maxZoom = 5;
  width = 1;
  height = 1;
  /** The view turned about the middle of the screen (radians; a site with north up: `MapView`),
   * else 0. The map is drawn as if unturned and the stage turned: `worldToScreen` gives stage
   * points; `screenToWorld` takes points on the screen (the pointer), `toPage` turns a stage point
   * onto the screen (for HTML over the map). */
  turn = 0;

  private ppfZoom = NaN;
  private ppfValue = 1;

  /** Pixels per foot (memoized: hot loops ask for it per feature). */
  get ppf(): number {
    if (this.zoom !== this.ppfZoom) {
      this.ppfZoom = this.zoom;
      this.ppfValue = 2 ** this.zoom;
    }
    return this.ppfValue;
  }

  worldToScreen(x: number, y: number): [number, number] {
    const s = this.ppf;
    return [(x - this.cx) * s + this.width / 2, (y - this.cy) * s + this.height / 2];
  }

  /** A stage point (`worldToScreen`) where it shows on the screen. */
  toPage(sx: number, sy: number): [number, number] {
    return this.turned(sx, sy, this.turn);
  }

  /** `worldToScreen` on the screen (`toPage`). */
  worldToPage(x: number, y: number): [number, number] {
    const [sx, sy] = this.worldToScreen(x, y);
    return this.toPage(sx, sy);
  }

  /** A point turned by `a` about the middle of the screen. */
  private turned(sx: number, sy: number, a: number): [number, number] {
    if (!a) return [sx, sy];
    const [c, s] = [Math.cos(a), Math.sin(a)];
    const [dx, dy] = [sx - this.width / 2, sy - this.height / 2];
    return [this.width / 2 + dx * c - dy * s, this.height / 2 + dx * s + dy * c];
  }

  screenToWorld(sx: number, sy: number): [number, number] {
    [sx, sy] = this.turned(sx, sy, -this.turn);
    const s = this.ppf;
    return [(sx - this.width / 2) / s + this.cx, (sy - this.height / 2) / s + this.cy];
  }

  /** Zoom by `dz` keeping the world point under screen point (sx, sy) fixed. */
  zoomAt(dz: number, sx: number, sy: number) {
    const [wx, wy] = this.screenToWorld(sx, sy);
    [sx, sy] = this.turned(sx, sy, -this.turn);
    this.zoom = Math.min(this.maxZoom, Math.max(this.minZoom, this.zoom + dz));
    const s = this.ppf;
    this.cx = wx - (sx - this.width / 2) / s;
    this.cy = wy - (sy - this.height / 2) / s;
  }

  panPixels(dx: number, dy: number) {
    if (this.turn) [dx, dy] = [dx * Math.cos(this.turn) + dy * Math.sin(this.turn), dy * Math.cos(this.turn) - dx * Math.sin(this.turn)];
    const s = this.ppf;
    this.cx -= dx / s;
    this.cy -= dy / s;
  }

  /** Visible world rectangle [x0, y0, x1, y1] (turned: the box round it). */
  viewRect(): [number, number, number, number] {
    if (this.turn) {
      const c = [this.screenToWorld(0, 0), this.screenToWorld(this.width, 0), this.screenToWorld(0, this.height), this.screenToWorld(this.width, this.height)];
      return [Math.min(...c.map((p) => p[0])), Math.min(...c.map((p) => p[1])), Math.max(...c.map((p) => p[0])), Math.max(...c.map((p) => p[1]))];
    }
    const [x0, y0] = this.screenToWorld(0, 0);
    const [x1, y1] = this.screenToWorld(this.width, this.height);
    return [x0, y0, x1, y1];
  }

  set(s: CameraState) {
    this.cx = s.cx;
    this.cy = s.cy;
    this.zoom = Math.min(this.maxZoom, Math.max(this.minZoom, s.zoom));
  }

  /** Zoom that fits a w×h ft rectangle on screen with a margin. */
  fitZoom(w: number, h: number, margin = 0.92): number {
    return Math.log2(Math.min((this.width * margin) / w, (this.height * margin) / h));
  }
}

/**
 * Smooth fly-to (van Wijk & Nuij): zooms out and back in along an optimal path so long
 * moves stay legible. Returns camera state at t in [0, 1].
 */
export function flyPath(from: CameraState, to: CameraState, viewWidth: number) {
  const rho = 1.42;
  const w0 = viewWidth / 2 ** from.zoom;
  const w1 = viewWidth / 2 ** to.zoom;
  const dx = to.cx - from.cx;
  const dy = to.cy - from.cy;
  const u1 = Math.hypot(dx, dy);
  if (u1 < 1e-9 * Math.max(w0, w1)) {
    const S = Math.abs(Math.log(w1 / w0)) / rho;
    return {
      duration: S,
      at(t: number): CameraState {
        const w = w0 * Math.exp(Math.log(w1 / w0) * t);
        return { cx: from.cx, cy: from.cy, zoom: Math.log2(viewWidth / w) };
      },
    };
  }
  const b = (i: number) => {
    const w = i ? w1 : w0;
    const sign = i ? -1 : 1;
    return (w1 * w1 - w0 * w0 + sign * rho ** 4 * u1 * u1) / (2 * w * rho * rho * u1);
  };
  const r = (i: number) => Math.log(Math.sqrt(b(i) ** 2 + 1) - b(i));
  const r0 = r(0);
  const S = (r(1) - r0) / rho;
  return {
    duration: S,
    at(t: number): CameraState {
      const s = t * S;
      const coshr0 = Math.cosh(r0);
      const u = (w0 / (rho * rho)) * (coshr0 * Math.tanh(rho * s + r0) - Math.sinh(r0));
      const w = (w0 * coshr0) / Math.cosh(rho * s + r0);
      return { cx: from.cx + (dx * u) / u1, cy: from.cy + (dy * u) / u1, zoom: Math.log2(viewWidth / w) };
    },
  };
}
