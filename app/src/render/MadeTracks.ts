// The links of the villages and towns added by hand (`T0.made`): the terrain tiles never hold
// them (their caches ignore edits), so they are drawn here, by the tiles' own road shader in
// their own class and width (a village's track, a town's road), so they read as the world's own
// at every zoom. The layer lies under the battlemap, which draws the link itself and covers it
// where it is built.
import { Container, Geometry, GlProgram, Mesh, Shader, UniformGroup } from 'pixi.js';
import { buildRiverGeometry, type MadeSettlement } from '../gen/protocol';
import type { Camera } from './camera';
import roadFragment from './shaders/tileroad.frag?raw';
import roadVertex from './shaders/tileroad.vert?raw';

export class MadeTracks {
  readonly container = new Container();
  private readonly uniforms = new UniformGroup({ uAlpha: { value: 1, type: 'f32' } });
  /** One mesh a settlement, its vertices measured from its centre (f32 keeps its precision). */
  private meshes: { mesh: Mesh<Geometry, Shader>; ox: number; oy: number }[] = [];
  private readonly program = GlProgram.from({ vertex: roadVertex, fragment: roadFragment, name: 'tile-road' });
  /** Segments drawn (for checks). */
  segments = 0;

  /** `frame`: the tile layer's per-frame line uniforms (`uPpf`). */
  constructor(private readonly frame: UniformGroup) {}

  /** Draw these settlements' tracks (replacing those drawn before). */
  set(list: MadeSettlement[]) {
    this.clear();
    for (const m of list) {
      if (m.track.length < 2) continue;
      const verts = new Float32Array(m.track.length * 4);
      m.track.forEach((p, i) => verts.set([p[0] - m.x, p[1] - m.y, m.width, m.class], i * 4));
      const g = buildRiverGeometry({ starts: new Uint32Array([0, m.track.length]), verts });
      if (!g) continue;
      const geometry = new Geometry({
        attributes: {
          aA: { buffer: g.a, format: 'float32x2' },
          aB: { buffer: g.b, format: 'float32x2' },
          aCorner: { buffer: g.corner, format: 'float32x2' },
          aW: { buffer: g.w, format: 'float32x2' },
          aQ: { buffer: g.q, format: 'float32x2' },
        },
        indexBuffer: g.index,
      });
      const shader = new Shader({ glProgram: this.program, resources: { tileUniforms: this.uniforms, riverFrame: this.frame } });
      const mesh = new Mesh({ geometry, shader });
      this.container.addChild(mesh);
      this.meshes.push({ mesh, ox: m.x, oy: m.y });
      this.segments += m.track.length - 1;
    }
  }

  /** Placed for the camera; `show`: false where the tiles aren't drawn (inside, the sewers, a
   * battlemap covering the view). */
  update(cam: Camera, show: boolean) {
    this.container.visible = show && this.meshes.length > 0;
    if (!this.container.visible) return;
    for (const { mesh, ox, oy } of this.meshes) {
      const [sx, sy] = cam.worldToScreen(ox, oy);
      mesh.position.set(sx, sy);
      mesh.scale.set(cam.ppf);
    }
  }

  private clear() {
    for (const { mesh } of this.meshes) {
      mesh.geometry.destroy();
      mesh.shader?.destroy();
      mesh.destroy();
    }
    this.meshes = [];
    this.segments = 0;
  }

  destroy() {
    this.clear();
    this.container.destroy({ children: true });
  }
}
