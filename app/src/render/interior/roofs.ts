import { OPEN_BELOW, ROOF_SLOPE, type InteriorLevel } from '../../gen/protocol';
import { roomBoxes, insetIn, WALL_HALF, type ShapedPlan, type Pt } from '../roomShapes';
import { Graphics } from 'pixi.js';
import { INK } from './look';

/** A hall's (or a yard's) height left open drawn round (`shape` 1): its outline, cut to the walls. */
export function roundVoids(lv: InteriorLevel, sp: ShapedPlan | null): Map<number, Pt[]> {
  const out = new Map<number, Pt[]>();
  lv.rooms.forEach((r, ri) => {
    const p = sp?.polys[ri];
    if (r.kind === OPEN_BELOW && r.shape === 1 && p?.length) out.set(ri, p);
  });
  return out;
}
/** A roof's slopes (`ROOF_SLOPE` rooms) inside a keep's parapet walk: slate, hipped along its
 * longer side where its squares fill their box (its ridge and hips drawn); round a round yard left
 * open (a shell keep's lean-tos), a ring sloping down to it from the walk; else plain. */
export function drawRoofs(g: Graphics, lv: InteriorLevel, nx: number, ny: number, sp: ShapedPlan | null) {
  const SLATE = 0x56606b;
  const shade = (c: number, f: number) => {
    const ch = (s: number) => Math.max(0, Math.min(255, Math.round(((c >> s) & 255) * f)));
    return (ch(16) << 16) | (ch(8) << 8) | ch(0);
  };
  const voids = roundVoids(lv, sp);
  const boxes = voids.size ? roomBoxes(lv, nx, ny) : [];
  lv.rooms.forEach((room, ri) => {
    if (room.kind !== ROOF_SLOPE) return;
    let [x0, y0, x1, y1, n] = [Infinity, Infinity, -Infinity, -Infinity, 0];
    for (let k = 0; k < nx * ny; k++) {
      if (lv.cells[k] !== ri) continue;
      const [i, j] = [k % nx, Math.floor(k / nx)];
      [x0, y0, x1, y1, n] = [Math.min(x0, i), Math.min(y0, j), Math.max(x1, i + 1), Math.max(y1, j + 1), n + 1];
    }
    if (!n) return;
    // A ring round a round yard inside it: from the walk's inner edge (the outer wall's inner face
    // a square in) down to the yard's edge.
    const yard = [...voids.keys()].find((v) => {
      const b = boxes[v];
      return !!b && b[0] > x0 && b[2] < x1 && b[1] > y0 && b[3] < y1;
    });
    if (yard !== undefined && sp) {
      const b = boxes[yard]!;
      const [cx, cy] = [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];
      const r0 = Math.min(b[2] - b[0], b[3] - b[1]) / 2;
      const r1 = sp.real ? Math.min(...sp.parts.map((p) => insetIn([cx, cy], p))) - WALL_HALF - 0.85 : Math.min(x1 - cx, cx - x0, y1 - cy, cy - y0) - 0.15;
      if (r1 > r0 + 0.4) {
        const seg = 64;
        const at = (r: number, a: number): number[] => [cx + r * Math.cos(a), cy + r * Math.sin(a)];
        for (let k = 0; k < seg; k++) {
          const [a0, a1] = [(k / seg) * 2 * Math.PI, ((k + 1) / seg) * 2 * Math.PI];
          // (Each piece faces in, down to the yard: lit from the top of the screen as the hipped
          // roofs are.)
          g.poly([...at(r1, a0), ...at(r1, a1 + 0.002), ...at(r0, a1 + 0.002), ...at(r0, a0)]).fill(shade(SLATE, 0.975 + 0.175 * Math.sin((a0 + a1) / 2)));
        }
        for (let r = r0 + 0.45; r < r1 - 0.1; r += 0.45) g.circle(cx, cy, r);
        g.stroke({ width: 0.03, color: 0x2f353b, alpha: 0.35 });
        for (let k = 0; k < 16; k++) {
          const a = (k / 16) * 2 * Math.PI;
          g.moveTo(...(at(r0, a) as [number, number])).lineTo(...(at(r1, a) as [number, number]));
        }
        g.stroke({ width: 0.04, color: 0x8a9099, alpha: 0.6 });
        g.circle(cx, cy, r1).stroke({ width: 0.06, color: INK });
        g.circle(cx, cy, r0).stroke({ width: 0.08, color: INK });
        return;
      }
    }
    if (n !== (x1 - x0) * (y1 - y0)) {
      for (let k = 0; k < nx * ny; k++) if (lv.cells[k] === ri) g.rect(k % nx, Math.floor(k / nx), 1, 1);
      g.fill(SLATE);
      return;
    }
    // Hipped: the ridge along the longer side, half the shorter side in from each end.
    const along = x1 - x0 >= y1 - y0;
    const h = (along ? y1 - y0 : x1 - x0) / 2;
    const [r0, r1]: [number, number][] = along ? [[x0 + h, (y0 + y1) / 2], [x1 - h, (y0 + y1) / 2]] : [[(x0 + x1) / 2, y0 + h], [(x0 + x1) / 2, y1 - h]];
    const faces: [number[], number][] = along
      ? [
          [[x0, y0, x1, y0, ...r1, ...r0], 1.15],
          [[x0, y1, x1, y1, ...r1, ...r0], 0.8],
          [[x0, y0, ...r0, x0, y1], 1.0],
          [[x1, y0, ...r1, x1, y1], 0.9],
        ]
      : [
          [[x0, y0, x0, y1, ...r1, ...r0], 1.15],
          [[x1, y0, x1, y1, ...r1, ...r0], 0.8],
          [[x0, y0, ...r0, x1, y0], 1.0],
          [[x0, y1, ...r1, x1, y1], 0.9],
        ];
    for (const [pts, f] of faces) g.poly(pts).fill(shade(SLATE, f));
    // Courses parallel to the eaves, the hips and the ridge.
    for (let d = 0.45; d < h - 0.1; d += 0.45) g.rect(x0 + d, y0 + d, x1 - x0 - 2 * d, y1 - y0 - 2 * d);
    g.stroke({ width: 0.03, color: 0x2f353b, alpha: 0.35 });
    g.moveTo(x0, y0).lineTo(...r0).lineTo(x0 + (along ? 0 : x1 - x0), y0 + (along ? y1 - y0 : 0));
    g.moveTo(x1, y1).lineTo(...r1).lineTo(x1 - (along ? 0 : x1 - x0), y1 - (along ? y1 - y0 : 0));
    g.stroke({ width: 0.05, color: 0x8a9099, alpha: 0.8 });
    g.moveTo(...r0).lineTo(...r1).stroke({ width: 0.09, color: 0x9aa0a8 });
    g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 0.06, color: INK });
  });
}
