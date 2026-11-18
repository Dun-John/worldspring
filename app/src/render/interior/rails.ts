import { type InteriorItem, type InteriorLevel } from '../../gen/protocol';
import { Graphics } from 'pixi.js';
import { boundaryPieces, keptRuns, WALL_HALF, type ShapedPlan, type Pt } from '../roomShapes';
import { INK, CIRCULATION } from './look';
import { roundVoids } from './roofs';

/** Each level's rail and ore cart squares, by its items (`railLinks`), and how many items it had then. */
export const RAIL_SQUARES = new WeakMap<InteriorItem[], { n: number; at: Map<number, string> }>();
/** Railings (`Wall::rail`) along a hall's open height and a temple's altar rails: a timber rail on posts;
 * round a round open height (a shell keep's yard), along its circle where a gallery meets it; a
 * cloister's arcade round its garth in stone on columns. */
export function drawRails(g: Graphics, lv: InteriorLevel, nx: number, ny: number, sp: ShapedPlan | null) {
  const voids = roundVoids(lv, sp);
  const cell = (p: Pt) => {
    const [i, j] = [Math.floor(p[0]), Math.floor(p[1])];
    return i >= 0 && j >= 0 && i < nx && j < ny ? lv.cells[j * nx + i] : -1;
  };
  // (Where a rail stands between a round void's square and another, the circle has it.)
  const onRound = (w: InteriorLevel['walls'][number]) => {
    const mid: Pt = [(w.a[0] + w.b[0]) / 2, (w.a[1] + w.b[1]) / 2];
    const n: Pt = w.a[0] === w.b[0] ? [0.5, 0] : [0, 0.5];
    return voids.has(cell([mid[0] - n[0], mid[1] - n[1]])) || voids.has(cell([mid[0] + n[0], mid[1] + n[1]]));
  };
  // (A cloister's arcade between its walk and its garth: stone, on columns.)
  const arcade = (w: InteriorLevel['walls'][number]) => {
    const mid: Pt = [(w.a[0] + w.b[0]) / 2, (w.a[1] + w.b[1]) / 2];
    const n: Pt = w.a[0] === w.b[0] ? [0.5, 0] : [0, 0.5];
    return [cell([mid[0] - n[0], mid[1] - n[1]]), cell([mid[0] + n[0], mid[1] + n[1]])].some((r) => r >= 0 && lv.rooms[r].kind === 'garth');
  };
  const rails = lv.walls.filter((w) => w.rail && !onRound(w));
  const runs: Pt[][] = rails.filter((w) => !arcade(w)).map((w) => [w.a, w.b]);
  const stone: Pt[][] = rails.filter(arcade).map((w) => [w.a, w.b]);
  for (const [ri, p] of voids) {
    const pieces = boundaryPieces(p);
    const gallery = (k: number) => {
      const q = pieces[k];
      const r = cell([(q.a[0] + q.b[0]) / 2 + q.out[0] * 0.35, (q.a[1] + q.b[1]) / 2 + q.out[1] * 0.35]);
      return r >= 0 && r !== ri && CIRCULATION.has(lv.rooms[r].kind);
    };
    runs.push(...keptRuns(pieces, gallery));
  }
  const line = (list: Pt[][]) => {
    for (const run of list) {
      g.moveTo(run[0][0], run[0][1]);
      for (const q of run.slice(1)) g.lineTo(q[0], q[1]);
    }
  };
  // Posts (columns) a square apart along runs, from each run's start.
  const posts = (list: Pt[][], r: number, first: number) => {
    for (const run of list) {
      let next = first;
      let along = 0;
      for (let k = 0; k + 1 < run.length; k++) {
        const [a, b] = [run[k], run[k + 1]];
        const len = Math.hypot(b[0] - a[0], b[1] - a[1]);
        for (; next <= along + len + 1e-6; next += 1) {
          const t = len > 0 ? (next - along) / len : 0;
          g.circle(a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, r);
        }
        along += len;
      }
    }
  };
  if (stone.length) {
    // The arcade: a low stone wall, its coping lit, slim columns on it a square apart (at the squares'
    // middles), their capitals pale.
    line(stone);
    g.stroke({ width: 0.2, color: 0x4a4540, cap: 'butt', join: 'round' });
    line(stone);
    g.stroke({ width: 0.12, color: 0xb4ad9e, cap: 'butt', join: 'round' });
    posts(stone, 0.1, 0.5);
    g.fill(0x6e675c);
    posts(stone, 0.055, 0.5);
    g.fill(0xd8d0bc);
  }
  if (!runs.length) return;
  line(runs);
  g.stroke({ width: 0.16, color: 0x3d2a17, cap: 'round', join: 'round' });
  line(runs);
  g.stroke({ width: 0.07, color: 0xa47a4a, cap: 'round', join: 'round' });
  posts(runs, 0.09, 0);
  g.fill(0x3d2a17);
}
/** A town hall's open arcade (`Wall::arcade`) along its front: stone piers on the outer wall's line
 * (`ARCADE_PIERS` squares apart from end to end, as the cloister's columns are drawn), the way open
 * between them under the arches (a faint line at their face). */
export function drawArcade(g: Graphics, lv: InteriorLevel, nx: number, ny: number, real: boolean) {
  const runs = lv.walls.filter((w) => w.arcade);
  if (!runs.length) return;
  const cell = (x: number, y: number) => {
    const [i, j] = [Math.floor(x), Math.floor(y)];
    return i >= 0 && j >= 0 && i < nx && j < ny ? lv.cells[j * nx + i] : -1;
  };
  const piers: Pt[] = [];
  for (const w of runs) {
    const len = Math.hypot(w.b[0] - w.a[0], w.b[1] - w.a[1]);
    if (len < 1e-6) continue;
    const [ux, uy] = [(w.b[0] - w.a[0]) / len, (w.b[1] - w.a[1]) / len];
    // (Out: the side with no room; on the outline's middle line where there is one.)
    const mid: Pt = [(w.a[0] + w.b[0]) / 2, (w.a[1] + w.b[1]) / 2];
    let [ox, oy] = [-uy, ux];
    if (cell(mid[0] + ox * 0.5, mid[1] + oy * 0.5) >= 0) [ox, oy] = [-ox, -oy];
    const off = real ? WALL_HALF : 0;
    const [ax, ay, bx, by] = [w.a[0] + ox * off, w.a[1] + oy * off, w.b[0] + ox * off, w.b[1] + oy * off];
    g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.05, color: 0x4a4540, alpha: 0.45 });
    const n = Math.max(1, Math.round(len / ARCADE_PIERS));
    for (let k = 0; k <= n; k++) piers.push([ax + ((bx - ax) * k) / n, ay + ((by - ay) * k) / n]);
  }
  const r = 0.2;
  for (const [x, y] of piers) g.rect(x - r, y - r, 2 * r, 2 * r);
  g.fill(0x8f877a).stroke({ width: 0.06, color: INK });
  for (const [x, y] of piers) g.rect(x - r * 0.55, y - r * 0.55, 1.1 * r, 1.1 * r);
  g.fill(0xc9c1ae);
}
/** The squares between a town hall's arcade piers (`drawArcade`). */
const ARCADE_PIERS = 2;
