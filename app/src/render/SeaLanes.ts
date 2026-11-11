// Sea lanes between ports (the overlay's `sea_lanes`): a dotted line of pale round dots over the sea,
// one mesh built once per world. Shown from the continent to region zoom, faded out before the towns'
// plans appear (`fadeLevels`).
import { Container, Geometry, GlProgram, Mesh, Shader, UniformGroup } from 'pixi.js';
import type { SeaLane } from '../gen/protocol';
import type { Camera } from './camera';
import fragment from './shaders/sealane.frag?raw';
import vertex from './shaders/sealane.vert?raw';

/** Dot fill: cream-white with a hint of sea blue, so it reads on deep water and pale shallows. */
const INK = [0.95, 0.96, 0.93];
/** The dots' thin soft rim: a dark sea blue. */
const RIM = [0.18, 0.3, 0.38];
/** The ink's strength at full show. */
const STRENGTH = 0.9;
/** The ink's strength at the whole map (eased in with the widening, so it reads less like a border). */
const STRENGTH_FAR = 0.8;
/** Widths (and the dot spacing) grow as the view zooms out: x1 from tile level `WIDEN_FROM` (region
 * zoom) in, `WIDEN_MAX` at `WIDEN_TO` (the whole map). */
const WIDEN_FROM = 7.7;
const WIDEN_TO = 4.5;
const WIDEN_MAX = 1.25;
/** Dot spacing on screen (px), about 3x the dot diameter. */
const PERIOD_PX = 10;
/** Mitres are cut at this factor (a sharp turn's outer corner). */
const MITRE_MAX = 2.5;

let shared: GlProgram | null = null;
/** The lanes' shader program (one for every layer; compiled ahead: `precompile`). */
export function seaLaneProgram(): GlProgram {
  return (shared ??= GlProgram.from({ vertex, fragment, name: 'sea-lane' }));
}

export class SeaLanes {
  readonly container = new Container();
  private readonly uniforms = new UniformGroup({
    uPpf: { value: 1, type: 'f32' },
    uAlpha: { value: 0, type: 'f32' },
    uPeriodPx: { value: PERIOD_PX, type: 'f32' },
    uWidth: { value: 1, type: 'f32' },
    uInk: { value: new Float32Array(INK), type: 'vec3<f32>' },
    uRim: { value: new Float32Array(RIM), type: 'vec3<f32>' },
  });
  private mesh: Mesh<Geometry, Shader> | null = null;
  /** Where the vertices are measured from (ft): f32 keeps its precision near it. */
  private ox = 0;
  private oy = 0;
  private readonly ferries: boolean;
  /** Segments drawn (lines of the overlay with two points or more). */
  segments = 0;
  /** Shown by the viewer's choice (Layers). */
  on = true;
  /** The strength last drawn at (0: hidden). */
  alpha = 0;

  /** `ferries`: draw only the ferries' water legs (class 3); otherwise only the other lanes. */
  constructor(lanes: SeaLane[], origin: [number, number], ferries = false) {
    this.ferries = ferries;
    [this.ox, this.oy] = origin;
    this.build(lanes);
  }

  private build(lanes: SeaLane[]) {
    const pos: number[] = [];
    const nrm: number[] = [];
    const info: number[] = [];
    const index: number[] = [];
    for (const lane of lanes) {
      if ((lane.c === 3) !== this.ferries) continue;
      // Points, repeats dropped (a zero-length step has no direction).
      const pts: [number, number][] = [];
      for (let i = 0; i + 1 < lane.p.length; i += 2) {
        const x = lane.p[i] - this.ox;
        const y = lane.p[i + 1] - this.oy;
        const last = pts[pts.length - 1];
        if (!last || last[0] !== x || last[1] !== y) pts.push([x, y]);
      }
      if (pts.length < 2) continue;
      this.segments++;
      let s = 0;
      const base = pos.length / 2;
      for (let i = 0; i < pts.length; i++) {
        const [x, y] = pts[i];
        if (i > 0) s += Math.hypot(x - pts[i - 1][0], y - pts[i - 1][1]);
        // Normals of the steps in and out, and their mitre.
        const n0 = i > 0 ? normal(pts[i - 1], pts[i]) : null;
        const n1 = i + 1 < pts.length ? normal(pts[i], pts[i + 1]) : null;
        let [nx, ny] = n0 && n1 ? [n0[0] + n1[0], n0[1] + n1[1]] : (n0 ?? n1)!;
        const l = Math.hypot(nx, ny);
        if (l < 1e-9) [nx, ny] = n0!;
        else [nx, ny] = [nx / l, ny / l];
        const cos = n0 && n1 ? nx * n0[0] + ny * n0[1] : 1;
        const k = Math.min(MITRE_MAX, 1 / Math.max(1 / MITRE_MAX, cos));
        for (const side of [-1, 1]) {
          pos.push(x, y);
          nrm.push(nx * k * side, ny * k * side);
          info.push(side, s, lane.c);
        }
        if (i > 0) {
          const a = base + (i - 1) * 2;
          index.push(a, a + 1, a + 2, a + 1, a + 3, a + 2);
        }
      }
    }
    if (!index.length) return;
    const geometry = new Geometry({
      attributes: {
        aP: { buffer: new Float32Array(pos), format: 'float32x2' },
        aN: { buffer: new Float32Array(nrm), format: 'float32x2' },
        aInfo: { buffer: new Float32Array(info), format: 'float32x3' },
      },
      indexBuffer: pos.length / 2 > 65535 ? new Uint32Array(index) : new Uint16Array(index),
    });
    const shader = new Shader({ glProgram: seaLaneProgram(), resources: { laneUniforms: this.uniforms } });
    this.mesh = new Mesh({ geometry, shader });
    this.container.addChild(this.mesh);
  }

  /** Placed for the camera, at `fade` (1 shown, 0 faded out by zoom), the view at tile `level`. */
  update(cam: Camera, fade: number, level: number) {
    const t = Math.min(1, Math.max(0, (WIDEN_FROM - level) / (WIDEN_FROM - WIDEN_TO)));
    const ease = t * t * (3 - 2 * t);
    this.alpha = this.on && this.mesh ? fade * (STRENGTH + (STRENGTH_FAR - STRENGTH) * ease) : 0;
    this.container.visible = this.alpha > 0.002;
    if (!this.container.visible) return;
    const [sx, sy] = cam.worldToScreen(this.ox, this.oy);
    this.container.position.set(sx, sy);
    this.container.scale.set(cam.ppf);
    const u = this.uniforms.uniforms;
    u.uPpf = cam.ppf;
    u.uAlpha = this.alpha;
    const widen = 1 + (WIDEN_MAX - 1) * ease;
    u.uWidth = widen;
    u.uPeriodPx = PERIOD_PX * widen;
  }

  destroy() {
    this.mesh?.geometry.destroy();
    this.mesh?.shader?.destroy();
    this.container.destroy({ children: true });
    this.mesh = null;
  }
}

/** The left-hand unit normal of the step a → b. */
function normal(a: [number, number], b: [number, number]): [number, number] {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const l = Math.hypot(dx, dy);
  return [-dy / l, dx / l];
}

/** Lanes shown from the continent to region zoom: full up to `from`, gone by `to`, in tile
 * levels as the tile layer picks them (site plans from `SITE_MIN_LEVEL`). */
export function laneFade(level: number, from: number, to: number): number {
  const t = Math.min(1, Math.max(0, (level - from) / (to - from)));
  return 1 - t * t * (3 - 2 * t);
}

/** The ferries' water legs: full up to tile level `FERRY_HAND_OVER` (L10, where the tile layer
 * draws its own ferry line, `payload::FERRY_MIN_LEVEL`; tiles of level L are the target from view
 * level L), then crossfaded out over `FERRY_FADE` levels, which covers the tiles' arrival. */
export const FERRY_HAND_OVER = 10;
const FERRY_FADE = 0.3;
export function ferryFade(level: number): number {
  return 1 - Math.min(1, Math.max(0, (level - FERRY_HAND_OVER) / FERRY_FADE));
}
