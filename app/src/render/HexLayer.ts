// The overland hex grid over the map (`Edits.hexes`, `hex.ts`): one quad over the view whose
// shader draws every hex edge (resolution independent, the same cost at any zoom), and hex
// numbers (`CCRR`) from a pool once the hexes are big enough to hold them. The DM's window and
// the players' both draw one (`MapView.setHexGrid`).
//
// A hex crawl (play) adds a second quad under the lines (`setCrawl`): in the players' window every
// hex not yet explored is washed over (a land/water texture made once, 257² RGBA8, from the
// whole-map tile: `setWash`), in the DM's lightly hatched. Which hexes are explored is a mask of
// one RGBA8 texel per hex (the map's hexes, at most `MASK_MAX`² round the view), written only
// when they change. Two draw calls at most, the same at any zoom.
import { BitmapText, BufferImageSource, Container, Geometry, GlProgram, Mesh, Shader, UniformGroup } from 'pixi.js';
import type { Camera } from './camera';
import { hexAt, hexAxial, hexCentre, hexDigits, hexFrame, hexKey, hexLabel, hexRange, hexSpan, NO_HEXES, parseHexKey, type Hex, type HexFrame, type HexSeen } from './hex';
import crawlFragment from './shaders/crawl.frag?raw';
import fragment from './shaders/hex.frag?raw';
import vertex from './shaders/hex.vert?raw';

/** Lines fade in as the hexes grow from `TINY_PX` to `SMALL_PX` across on screen. */
const TINY_PX = 10;
const SMALL_PX = 22;
/** The grid fades out as the battlemap's 5-ft squares fade in (`BattlemapLayer` FADE_START/END, ft per px). */
const BATTLE_FROM = 0.7;
const BATTLE_TO = 0.3;
/** Numbers once a hex is this many px across (fading in over the next `NUM_FADE_PX`); a map of
 * over 99 columns or rows, with six-digit numbers, waits for `NUM_PX_WIDE`. */
const NUM_PX = 70;
const NUM_PX_WIDE = 90;
const NUM_FADE_PX = 16;
/** At most this many numbers (a screenful of 70-px hexes is ~500). */
const NUM_MAX = 900;
const FONT = ['Palatino Linotype', 'Book Antiqua', 'Palatino', 'Georgia', 'serif'];

/** The explored-hexes mask is at most this many hexes a side (a finer grid's round the view). */
const MASK_MAX = 1024;
/** The crawl's soft edge (on the explored side of the border: a hex not explored is washed
 * from its edge in): this part of a hex, but no more than `BAND_FT` (a battlemap's or a town's
 * worth, so nothing shows through close up), and never less than `BAND_PX`. */
const BAND_PX = 10;
const BAND_PART = 0.07;
const BAND_FT = 60;

/** What a hex crawl draws over the hexes not explored: nothing, the players' wash, the DM's hatch. */
export type CrawlMode = 0 | 1 | 2;

let shared: GlProgram | null = null;
function hexProgram(): GlProgram {
  return (shared ??= GlProgram.from({ vertex, fragment, name: 'hex-grid' }));
}
let sharedCrawl: GlProgram | null = null;
function crawlProgram(): GlProgram {
  return (sharedCrawl ??= GlProgram.from({ vertex, fragment: crawlFragment, name: 'hex-crawl' }));
}

const rgba = (w: number, h: number, data?: Uint8Array) =>
  new BufferImageSource({ resource: data ?? new Uint8Array(w * h * 4), width: w, height: h, format: 'rgba8unorm', alphaMode: 'no-premultiply-alpha', scaleMode: 'nearest', addressMode: 'clamp-to-edge' });

export class HexLayer {
  readonly container = new Container();
  private readonly numbers = new Container();
  private readonly uniforms = new UniformGroup({
    uRect: { value: new Float32Array(4), type: 'vec4<f32>' },
    uHalf: { value: new Float32Array(2), type: 'vec2<f32>' },
    uCam: { value: new Float32Array(2), type: 'vec2<f32>' },
    uFpp: { value: 1, type: 'f32' },
    uFlat: { value: 1, type: 'f32' },
    uS: { value: 1, type: 'f32' },
    uAa: { value: 1, type: 'f32' },
    uWidth: { value: 1.5, type: 'f32' },
    uColor: { value: new Float32Array(3), type: 'vec3<f32>' },
    uAlpha: { value: 0, type: 'f32' },
    uMap: { value: new Float32Array(4), type: 'vec4<f32>' },
  });
  private readonly crawlUniforms = new UniformGroup({
    uRect: { value: new Float32Array(4), type: 'vec4<f32>' },
    uHalf: { value: new Float32Array(2), type: 'vec2<f32>' },
    uCam: { value: new Float32Array(2), type: 'vec2<f32>' },
    uFpp: { value: 1, type: 'f32' },
    uFlat: { value: 1, type: 'f32' },
    uS: { value: 1, type: 'f32' },
    uAlpha: { value: 0, type: 'f32' },
    uMode: { value: 1, type: 'f32' },
    uBand: { value: 1, type: 'f32' },
    uMap: { value: new Float32Array(4), type: 'vec4<f32>' },
    uRef: { value: new Float32Array(2), type: 'vec2<f32>' },
    uMaskBox: { value: new Float32Array(4), type: 'vec4<f32>' },
    uWorld: { value: new Float32Array(2), type: 'vec2<f32>' },
    uDomain: { value: 1, type: 'f32' },
    uHasWash: { value: 0, type: 'f32' },
    uMaskOut: { value: 0, type: 'f32' },
  });
  private readonly mesh: Mesh<Geometry, Shader>;
  private readonly crawlMesh: Mesh<Geometry, Shader>;
  private mask = rgba(1, 1);
  private wash = rgba(1, 1);
  /** The mask's hexes: first column and row, columns and rows. */
  private maskBox: [number, number, number, number] = [0, 0, 1, 1];
  private maskDirty = true;
  private crawlMode: CrawlMode = 0;
  private seen: HexSeen = NO_HEXES;
  /** Times the mask was written (for checks). */
  maskWrites = 0;
  /** The numbers' colour (their font is made in it, with a halo). */
  private numColor = -1;
  /** The players' wash is made (`setWash`). */
  washed = false;
  /** Whether the crawl was drawn last frame. */
  crawlShown = false;
  private readonly pool: BitmapText[] = [];
  private grid: Hex | null = null;
  private frame: HexFrame | null = null;
  private digits = 2;
  private mapW = 0;
  private mapH = 0;
  /** The strength the lines were last drawn at (0: hidden). */
  alpha = 0;
  /** Numbers drawn last frame. */
  shownNumbers = 0;

  constructor() {
    const geometry = new Geometry({
      attributes: { aP: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' } },
      indexBuffer: new Uint16Array([0, 1, 2, 0, 2, 3]),
    });
    const shader = new Shader({ glProgram: hexProgram(), resources: { hexUniforms: this.uniforms } });
    this.mesh = new Mesh({ geometry, shader });
    const crawlShader = new Shader({ glProgram: crawlProgram(), resources: { crawlUniforms: this.crawlUniforms, uWash: this.wash, uMask: this.mask } });
    this.crawlMesh = new Mesh({ geometry, shader: crawlShader });
    this.crawlMesh.visible = false;
    this.container.addChild(this.crawlMesh, this.mesh, this.numbers);
    this.container.visible = false;
  }

  /** The grid drawn (null: none), over a map `w` × `h` ft. */
  set(grid: Hex | null, w: number, h: number) {
    this.grid = grid;
    this.frame = grid ? hexFrame(grid) : null;
    this.mapW = w;
    this.mapH = h;
    this.digits = this.frame ? hexDigits(this.frame, w, h) : 2;
    const u = this.uniforms.uniforms;
    if (grid && this.frame) {
      const c = parseInt(grid.color.slice(1), 16);
      (u.uColor as Float32Array).set([((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255]);
      u.uFlat = grid.flat ? 1 : 0;
      u.uS = this.frame.s;
      u.uWidth = grid.width;
      if (c !== this.numColor) {
        this.numColor = c;
        for (const t of this.pool) t.destroy();
        this.pool.length = 0;
      }
    }
    this.maskDirty = true;
  }

  /** A hex crawl over the grid: `mode` 1 the players' wash, 2 the DM's hatch, 0 none; `seen` the
   * explored hexes (`hexKey`s; kept, not copied: call again once it changes). */
  setCrawl(mode: CrawlMode, seen: HexSeen) {
    this.crawlMode = mode;
    this.seen = seen;
    this.maskDirty = true;
  }

  /**
   * The players' wash from the whole-map tile's samples (257² over `domain` ft): each one land
   * (its biome's colour, light) or water, then blurred into a few big soft shapes.
   */
  setWash(tex: Uint16Array, biome: Uint8Array, toFloat: (h: number) => number, domain: number) {
    const N = 257;
    const out = new Float32Array(N * N * 3);
    for (let k = 0; k < N * N; k++) {
      const wet = toFloat(tex[k * 4 + 3]) > toFloat(tex[k * 4]);
      const b = biome[k * 4];
      const c = wet ? WATER : (PALETTE[b > 1 && b < PALETTE.length ? b : 8] ?? PALETTE[8]);
      for (let ch = 0; ch < 3; ch++) out[k * 3 + ch] = c[ch] + (PAPER[ch] - c[ch]) * (wet ? 0.2 : 0.5);
    }
    blur(out, N, 2);
    blur(out, N, 2);
    const data = new Uint8Array(N * N * 4);
    for (let k = 0; k < N * N; k++) {
      for (let ch = 0; ch < 3; ch++) data[k * 4 + ch] = Math.round(Math.min(1, out[k * 3 + ch]) * 255);
      data[k * 4 + 3] = 255;
    }
    const old = this.wash;
    this.wash = new BufferImageSource({ resource: data, width: N, height: N, format: 'rgba8unorm', alphaMode: 'no-premultiply-alpha', scaleMode: 'linear', addressMode: 'clamp-to-edge' });
    this.crawlMesh.shader!.resources.uWash = this.wash;
    old.destroy();
    const u = this.crawlUniforms.uniforms;
    u.uDomain = domain;
    u.uHasWash = 1;
    this.washed = true;
  }

  /** The mask's box for a view at (cx, cy): the map's hexes, or (a fine grid) those round it. */
  private boxFor(f: HexFrame, cx: number, cy: number): [number, number, number, number] {
    let [c0, c1, r0, r1] = hexSpan(f, this.mapW, this.mapH);
    if (c1 - c0 + 1 > MASK_MAX || r1 - r0 + 1 > MASK_MAX) {
      const [hc, hr] = hexAt(f, cx, cy);
      // (Kept whole inside the map's span: shoved back from its far edges.)
      c0 = Math.max(c0, Math.min(hc - MASK_MAX / 2, c1 - MASK_MAX + 1));
      r0 = Math.max(r0, Math.min(hr - MASK_MAX / 2, r1 - MASK_MAX + 1));
      c1 = Math.min(c1, c0 + MASK_MAX - 1);
      r1 = Math.min(r1, r0 + MASK_MAX - 1);
    }
    return [c0, r0, c1 - c0 + 1, r1 - r0 + 1];
  }

  /** Whether a world point (ft) is in a hex the crawl has not explored (no crawl: false). */
  unexplored(x: number, y: number): boolean {
    if (!this.crawlMode || !this.frame) return false;
    const [c, r] = hexAt(this.frame, x, y);
    return !this.seen.explored(hexKey(c, r));
  }

  /** The explored-hexes mask written again for a view at (cx, cy). */
  private writeMask(f: HexFrame, cx: number, cy: number) {
    const [c0, r0, w, h] = this.boxFor(f, cx, cy);
    if (w !== this.mask.width || h !== this.mask.height) {
      const old = this.mask;
      this.mask = rgba(w, h);
      this.crawlMesh.shader!.resources.uMask = this.mask;
      old.destroy();
    }
    const data = this.mask.resource as Uint8Array;
    const seen = this.seen;
    // (Every hex explored, or none, then the hexes marked otherwise.)
    if (seen.all) for (let k = 0; k < data.length; k += 4) data[k] = 255;
    else data.fill(0);
    for (const key of seen.marks()) {
      const at = parseHexKey(key);
      if (!at) continue;
      const [i, j] = [at[0] - c0, at[1] - r0];
      if (i >= 0 && j >= 0 && i < w && j < h) data[(j * w + i) * 4] = seen.explored(key) ? 255 : 0;
    }
    this.mask.update();
    this.maskBox = [c0, r0, w, h];
    this.maskDirty = false;
    this.maskWrites++;
    this.crawlUniforms.uniforms.uMaskOut = seen.all ? 1 : 0;
  }

  /** Placed for the camera at `fade` (1 shown, 0 hidden: inside a building, off by choice); the
   * crawl (`setCrawl`) where `surface` (not inside a building or site, nor the sewers). */
  update(cam: Camera, fade: number, resolution: number, surface = fade > 0) {
    const g = this.grid;
    const f = this.frame;
    const fpp = 1 / cam.ppf;
    const across = f ? f.s * cam.ppf : 0;
    const small = Math.min(1, Math.max(0, (across - TINY_PX) / (SMALL_PX - TINY_PX)));
    const battle = Math.min(1, Math.max(0, (fpp - BATTLE_TO) / (BATTLE_FROM - BATTLE_TO)));
    this.alpha = g && f ? fade * small * battle * g.opacity : 0;
    const lines = this.alpha > 0.002;
    const crawl = !!g && !!f && surface && this.crawlMode > 0;
    this.mesh.visible = lines;
    this.numbers.visible = lines;
    this.crawlMesh.visible = crawl;
    this.crawlShown = crawl;
    this.container.visible = lines || crawl;
    if (!this.container.visible || !g || !f) {
      this.shownNumbers = 0;
      return;
    }
    // The reference: the centre of the hex under the camera (a lattice point: every hex round it
    // is the same shape about it).
    const [rc, rr] = hexAt(f, cam.cx, cam.cy);
    const [hx, hy] = hexCentre(f, rc, rr);
    // The quad: the screen, and enough round it to cover the corners of a turned view.
    const m = Math.hypot(cam.width, cam.height) / 2;
    const u = this.uniforms.uniforms;
    (u.uRect as Float32Array).set([cam.width / 2 - m, cam.height / 2 - m, cam.width / 2 + m, cam.height / 2 + m]);
    (u.uHalf as Float32Array).set([cam.width / 2, cam.height / 2]);
    (u.uCam as Float32Array).set([cam.cx - hx, cam.cy - hy]);
    (u.uMap as Float32Array).set([-hx, -hy, this.mapW - hx, this.mapH - hy]);
    u.uFpp = fpp;
    u.uAa = 1 / Math.max(0.5, resolution);
    u.uAlpha = this.alpha;
    if (crawl) this.placeCrawl(cam, f, rc, rr, hx, hy, fpp);
    if (lines) this.drawNumbers(cam, f, g, across, m);
    else this.shownNumbers = 0;
  }

  private placeCrawl(cam: Camera, f: HexFrame, rc: number, rr: number, hx: number, hy: number, fpp: number) {
    // (A fine grid's mask follows the view once it nears its edge.)
    const [c0, r0, w, h] = this.maskBox;
    // (Near the box's edge, it moves only if a box round the view would be another: at the map's
    // edges it can't.)
    const near = (w >= MASK_MAX || h >= MASK_MAX) && (rc - c0 < MASK_MAX / 4 || c0 + w - rc < MASK_MAX / 4 || rr - r0 < MASK_MAX / 4 || r0 + h - rr < MASK_MAX / 4);
    const moved = near && this.boxFor(f, cam.cx, cam.cy).some((v, k) => v !== this.maskBox[k]);
    if (this.maskDirty || moved) this.writeMask(f, cam.cx, cam.cy);
    const u = this.crawlUniforms.uniforms;
    const l = this.uniforms.uniforms;
    for (const k of ['uRect', 'uHalf', 'uCam', 'uMap'] as const) (u[k] as Float32Array).set(l[k] as Float32Array);
    u.uFpp = fpp;
    u.uFlat = f.flat ? 1 : 0;
    u.uS = f.s;
    u.uAlpha = 1;
    u.uMode = this.crawlMode;
    u.uBand = Math.min(f.s * 0.35, Math.max(BAND_PX * fpp, Math.min(BAND_FT, f.s * BAND_PART)));
    const [q, r] = hexAxial(f, rc, rr);
    (u.uRef as Float32Array).set(f.flat ? [r, q] : [q, r]);
    (u.uMaskBox as Float32Array).set(this.maskBox);
    (u.uWorld as Float32Array).set([hx, hy]);
  }

  private drawNumbers(cam: Camera, f: HexFrame, g: Hex, across: number, m: number) {
    const from = this.digits > 2 ? NUM_PX_WIDE : NUM_PX;
    const show = g.numbers ? Math.min(1, Math.max(0, (across - from) / NUM_FADE_PX)) : 0;
    let n = 0;
    if (show > 0) {
      const fpp = 1 / cam.ppf;
      // (The screen; a turned view's numbers from the whole quad.)
      const [mx, my] = cam.turn ? [m, m] : [cam.width / 2, cam.height / 2];
      const [x0, y0] = [cam.cx - mx * fpp, cam.cy - my * fpp];
      const [x1, y1] = [cam.cx + mx * fpp, cam.cy + my * fpp];
      const [c0, c1, r0, r1] = hexRange(f, Math.max(0, x0), Math.max(0, y0), Math.min(this.mapW, x1), Math.min(this.mapH, y1));
      const size = Math.min(14, Math.max(9, across / (this.digits * 3.4)));
      // (Near the top of the hex, where printed hex maps put them.)
      const up = f.s * 0.3;
      for (let col = c0; col <= c1 && n < NUM_MAX; col++) {
        for (let row = r0; row <= r1 && n < NUM_MAX; row++) {
          // (Numbered from 0101: the part hexes before the first column or row have none.)
          if (col < 1 || row < 1) continue;
          const [cx, cy] = hexCentre(f, col, row);
          const y = cy - up;
          if (cx < 0 || cy < 0 || cx > this.mapW || cy > this.mapH) continue;
          const [sx, sy] = cam.worldToScreen(cx, y);
          if (Math.abs(sx - cam.width / 2) > mx + 40 || Math.abs(sy - cam.height / 2) > my + 20) continue;
          const t = this.label(n++);
          const text = hexLabel(col, row, this.digits);
          if (t.text !== text) t.text = text;
          t.position.set(Math.round(sx), Math.round(sy));
          t.scale.set(size / 16);
          t.visible = true;
        }
      }
    }
    for (let i = n; i < this.pool.length; i++) this.pool[i].visible = false;
    this.numbers.alpha = Math.min(1, g.opacity * 2.2) * show;
    this.shownNumbers = n;
  }

  private label(i: number): BitmapText {
    while (this.pool.length <= i) {
      // (In the grid's colour with a halo against it, light or dark, to read over forest.)
      const c = Math.max(0, this.numColor);
      const light = ((c >> 16) & 255) * 0.3 + ((c >> 8) & 255) * 0.59 + (c & 255) * 0.11 > 140;
      const t = new BitmapText({ text: '', style: { fontFamily: FONT, fontSize: 16, fill: c, fontWeight: 'bold', stroke: { color: light ? 0x2b241d : 0xf4eedf, width: 4, join: 'round' } } });
      t.anchor.set(0.5);
      this.numbers.addChild(t);
      this.pool.push(t);
    }
    return this.pool[i];
  }

  destroy() {
    this.mesh.geometry.destroy();
    this.mesh.shader?.destroy();
    this.crawlMesh.shader?.destroy();
    this.mask.destroy();
    this.wash.destroy();
    this.container.destroy({ children: true });
    this.pool.length = 0;
  }
}

// The wash's colours: the terrain shader's biome palette (`terrain.frag` PALETTE), water, paper.
const WATER: [number, number, number] = [0.62, 0.73, 0.77];
const PAPER: [number, number, number] = [0.93, 0.9, 0.82];
const PALETTE: [number, number, number][] = [
  [0.54, 0.63, 0.66], [0.62, 0.72, 0.74], [0.95, 0.95, 0.93], [0.8, 0.79, 0.7], [0.74, 0.71, 0.64], [0.64, 0.69, 0.57], [0.68, 0.74, 0.54],
  [0.58, 0.68, 0.52], [0.82, 0.82, 0.63], [0.85, 0.81, 0.64], [0.84, 0.79, 0.66], [0.91, 0.83, 0.63], [0.86, 0.81, 0.59], [0.52, 0.65, 0.45],
  [0.66, 0.71, 0.6], [0.53, 0.48, 0.45], [0.93, 0.91, 0.86], [0.6, 0.57, 0.58], [0.6, 0.56, 0.51],
];

/** A box blur of radius `r` across and then down an `n` × `n` RGB field, in place. */
function blur(a: Float32Array, n: number, r: number) {
  const row = new Float32Array(n * 3);
  for (const across of [true, false]) {
    for (let line = 0; line < n; line++) {
      const at = (k: number) => (across ? line * n + k : k * n + line) * 3;
      for (let k = 0; k < n; k++) {
        let [sr, sg, sb, c] = [0, 0, 0, 0];
        for (let d = -r; d <= r; d++) {
          const kk = Math.min(n - 1, Math.max(0, k + d));
          const o = at(kk);
          sr += a[o];
          sg += a[o + 1];
          sb += a[o + 2];
          c++;
        }
        row[k * 3] = sr / c;
        row[k * 3 + 1] = sg / c;
        row[k * 3 + 2] = sb / c;
      }
      for (let k = 0; k < n; k++) {
        const o = at(k);
        a[o] = row[k * 3];
        a[o + 1] = row[k * 3 + 1];
        a[o + 2] = row[k * 3 + 2];
      }
    }
  }
}
