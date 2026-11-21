// `?gallery=1&items=flatroofs`: dry-culture flat roofs as the battlemap's roof mesh draws them
// (`roofMesh`, `colormesh` shaders): parapet, lit and shaded coping, cast shadow, hatched patches.
import { Container, Geometry, GlProgram, Graphics, Mesh, Shader, Text, UniformGroup } from 'pixi.js';
import type { Roof } from '../gen/battlePrep';
import { PX, roofMesh } from '../gen/roofMesh';
import colorMeshFragment from '../render/shaders/colormesh.frag?raw';
import colorMeshVertex from '../render/shaders/colormesh.vert?raw';

const FLAT = 4;
const DOME = 3;
const FLAT_CLAY = 8;
const WHITEWASH = 11;

/** Footprints in squares: a box, a long one, an L, a U, a small one (too narrow for a parapet's inside). */
const SHAPES: [string, number[]][] = [
  ['box', [0, 0, 6, 0, 6, 4, 0, 4]],
  ['long', [0, 0, 9, 0, 9, 4, 0, 4]],
  ['L', [0, 0, 7, 0, 7, 3, 3, 3, 3, 6, 0, 6]],
  ['U', [0, 0, 8, 0, 8, 6, 5.5, 6, 5.5, 3, 2.5, 3, 2.5, 6, 0, 6]],
  ['small', [0, 0, 3, 0, 3, 2, 0, 2]],
];

/** Each shape in flat clay and in whitewash, square to the map and at 45 degrees (Tinabad's streets),
 * one smoky, one domed; at `sq` px a square on paving. Returns the y below it. */
export function flatRoofs(root: Container, y0: number, width: number, nolabels: boolean, sq: number): number {
  const cell = 11;
  const k = sq / PX;
  const cols = Math.max(1, Math.floor((width - 32) / (cell * sq)));
  const roofs: Roof[] = [];
  const labels: [string, number, number][] = [];
  let n = 0;
  const add = (name: string, pts: number[], material: number, turn: number, extra: Partial<Roof> = {}) => {
    const [c, r] = [n % cols, Math.floor(n / cols)];
    n++;
    const xs = pts.filter((_, i) => i % 2 === 0);
    const ys = pts.filter((_, i) => i % 2 === 1);
    const [mx, my] = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
    const [cs, sn] = [Math.cos(turn), Math.sin(turn)];
    const [ox, oy] = [(c + 0.5) * cell, (r + 0.5) * cell];
    const out: number[] = [];
    for (let i = 0; i < pts.length; i += 2) {
      const [dx, dy] = [pts[i] - mx, pts[i + 1] - my];
      out.push((ox + dx * cs - dy * sn) * PX, (oy + dx * sn + dy * cs) * PX);
    }
    roofs.push({ pts: out, floors: 2, battlements: false, tower: 0, top: 0, cone: false, tint: -1, material, shape: FLAT, smoke: false, shade: n % 5, yurt: false, wear: 60, idx: n * 7919, parts: [], ...extra });
    labels.push([`${name}${material === WHITEWASH ? ', whitewash' : ''}${turn ? ', 45°' : ''}${extra.smoke ? ', smoky' : ''}${extra.shape === DOME ? ', dome' : ''}`, c * cell * sq, (r + 1) * cell * sq]);
  };
  for (const [name, pts] of SHAPES) add(name, pts, FLAT_CLAY, 0);
  for (const [name, pts] of SHAPES) add(name, pts, WHITEWASH, Math.PI / 4);
  add('box', SHAPES[0][1], FLAT_CLAY, Math.PI / 4, { smoke: true });
  add('long', SHAPES[1][1], WHITEWASH, 0, { shape: DOME });
  const rows = Math.ceil(n / cols);
  const panel = new Container();
  panel.position.set(16, y0);
  panel.addChild(new Graphics().rect(0, 0, cols * cell * sq, rows * cell * sq).fill(0x948c80));
  const c = roofMesh(roofs);
  const geometry = new Geometry({
    attributes: { aPos: { buffer: c.pos, format: 'float32x2' }, aColor: { buffer: c.color, format: 'float32x4' }, aRoof: { buffer: c.roof, format: 'float32x4' } },
    indexBuffer: c.index,
  });
  const frame = new UniformGroup({ uPxPerSq: { value: sq, type: 'f32' } });
  const mesh = new Mesh({ geometry, shader: new Shader({ glProgram: GlProgram.from({ vertex: colorMeshVertex, fragment: colorMeshFragment, name: 'gallery-roofs' }), resources: { roofFrame: frame } }) });
  mesh.scale.set(k);
  panel.addChild(mesh);
  if (!nolabels) {
    for (const [s, x, y] of labels) {
      const t = new Text({ text: s, style: { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x1d1a14 } });
      t.position.set(x + 6, y - 20);
      panel.addChild(t);
    }
  }
  const info = new Text({ text: `${c.pos.length / 2} vertices, ${c.index.length / 3} triangles`, style: { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x1d1a14 } });
  info.position.set(0, rows * cell * sq + 6);
  panel.addChild(info);
  root.addChild(panel);
  return y0 + rows * cell * sq + 40;
}
