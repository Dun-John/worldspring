// The overland hex grid over the map (`Edits.hexes`, `hex.ts`): one quad over the view whose
// shader draws every hex edge (resolution independent, the same cost at any zoom), and hex
// numbers (`CCRR`) from a pool once the hexes are big enough to hold them. The DM's window and
// the players' both draw one (`MapView.setHexGrid`).
import { BitmapText, Container, Geometry, GlProgram, Mesh, Shader, UniformGroup } from 'pixi.js';
import type { Camera } from './camera';
import { hexAt, hexCentre, hexDigits, hexFrame, hexLabel, hexRange, type Hex, type HexFrame } from './hex';
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

let shared: GlProgram | null = null;
function hexProgram(): GlProgram {
  return (shared ??= GlProgram.from({ vertex, fragment, name: 'hex-grid' }));
}

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
  private readonly mesh: Mesh<Geometry, Shader>;
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
    this.container.addChild(this.mesh, this.numbers);
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
      for (const t of this.pool) t.tint = c;
    }
  }

  /** Placed for the camera at `fade` (1 shown, 0 hidden: inside a building, off by choice). */
  update(cam: Camera, fade: number, resolution: number) {
    const g = this.grid;
    const f = this.frame;
    const fpp = 1 / cam.ppf;
    const across = f ? f.s * cam.ppf : 0;
    const small = Math.min(1, Math.max(0, (across - TINY_PX) / (SMALL_PX - TINY_PX)));
    const battle = Math.min(1, Math.max(0, (fpp - BATTLE_TO) / (BATTLE_FROM - BATTLE_TO)));
    this.alpha = g && f ? fade * small * battle * g.opacity : 0;
    this.container.visible = this.alpha > 0.002;
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
    this.drawNumbers(cam, f, g, across, m);
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
      const tint = parseInt(g.color.slice(1), 16);
      for (let col = c0; col <= c1 && n < NUM_MAX; col++) {
        for (let row = r0; row <= r1 && n < NUM_MAX; row++) {
          const [cx, cy] = hexCentre(f, col, row);
          const y = cy - up;
          if (cx < 0 || cy < 0 || cx > this.mapW || cy > this.mapH) continue;
          const [sx, sy] = cam.worldToScreen(cx, y);
          if (Math.abs(sx - cam.width / 2) > mx + 40 || Math.abs(sy - cam.height / 2) > my + 20) continue;
          const t = this.label(n++, tint);
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

  private label(i: number, tint: number): BitmapText {
    while (this.pool.length <= i) {
      const t = new BitmapText({ text: '', style: { fontFamily: FONT, fontSize: 16, fill: 0xffffff, fontWeight: 'bold' } });
      t.anchor.set(0.5);
      t.tint = tint;
      this.numbers.addChild(t);
      this.pool.push(t);
    }
    return this.pool[i];
  }

  destroy() {
    this.mesh.geometry.destroy();
    this.mesh.shader?.destroy();
    this.container.destroy({ children: true });
    this.pool.length = 0;
  }
}
