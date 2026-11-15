// Roads drawn by hand (`Edits.paths`): drawn by the tiles' own road shader (as the links of the
// villages added by hand, `MadeTracks`) along each road's line as the generator shapes it
// (`editor/paths.ts` `curve`), in the class of the world's road that looks like it (a footpath or
// track a track, a gravel road a road, a paved road the king's road, a cobbled street a paved main
// street) and its own width, so it reads as one of the world's at every zoom. The layer lies under
// the battlemap, which draws the road on its squares and covers it where it is built.
import { Container, Geometry, GlProgram, Mesh, Shader, UniformGroup } from 'pixi.js';
import { buildRiverGeometry, type DrawnPath, type PathKind } from '../gen/protocol';
import { curve, pathWidth } from '../editor/paths';
import type { Camera } from './camera';
import roadFragment from './shaders/tileroad.frag?raw';
import roadVertex from './shaders/tileroad.vert?raw';

/** The tile road class each type is drawn in (`tileroad.vert`). */
const CLASS: Record<PathKind, number> = { footpath: 2, track: 2, gravel: 1, paved: 0, cobbled: 4 };

export class DrawnRoads {
  readonly container = new Container();
  private readonly uniforms = new UniformGroup({ uAlpha: { value: 1, type: 'f32' } });
  /** One mesh a road, its vertices measured from its first point (f32 keeps its precision). */
  private meshes: { mesh: Mesh<Geometry, Shader>; ox: number; oy: number }[] = [];
  private readonly program = GlProgram.from({ vertex: roadVertex, fragment: roadFragment, name: 'tile-road' });
  private last: Record<string, DrawnPath> | undefined;
  /** Segments drawn (for checks). */
  segments = 0;

  /** `frame`: the tile layer's per-frame line uniforms (`uPpf`). */
  constructor(private readonly frame: UniformGroup) {}

  /** Draw these roads (replacing those drawn before). */
  set(paths: Record<string, DrawnPath> | undefined) {
    if (paths === this.last) return;
    this.last = paths;
    this.clear();
    for (const r of Object.values(paths ?? {})) {
      const line = curve(r.pts);
      if (line.length < 2) continue;
      const [ox, oy] = line[0];
      const verts = new Float32Array(line.length * 4);
      line.forEach((p, i) => verts.set([p[0] - ox, p[1] - oy, pathWidth(r), CLASS[r.type] ?? 2], i * 4));
      const g = buildRiverGeometry({ starts: new Uint32Array([0, line.length]), verts });
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
      this.meshes.push({ mesh, ox, oy });
      this.segments += line.length - 1;
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
