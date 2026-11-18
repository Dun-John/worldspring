import { ruinPlanFill } from './ruinPlan';
import { codeMaterial, codeShape, codeStone } from './wallcode';
import { HORSE_PARK_PLAN, YURT_FELT, roofRgb } from './protocol';

/** River polylines owned by one tile: vertex k is verts[4k..4k+4] = (x, y in tile units, width ft, discharge). */
export interface TileRivers {
  starts: Uint32Array;
  verts: Float32Array;
}

/** GPU-ready river capsules for one tile (4 vertices per segment), built off the main thread. */
export interface RiverGeometry {
  a: Float32Array;
  b: Float32Array;
  corner: Float32Array;
  w: Float32Array;
  q: Float32Array;
  index: Uint32Array;
}

const CORNERS = [0, -1, 1, -1, 1, 1, 0, 1];

export function buildRiverGeometry({ starts, verts }: TileRivers): RiverGeometry | null {
  let segs = 0;
  for (let l = 0; l + 1 < starts.length; l++) segs += Math.max(0, starts[l + 1] - starts[l] - 1);
  if (!segs) return null;
  const g: RiverGeometry = {
    a: new Float32Array(segs * 8),
    b: new Float32Array(segs * 8),
    corner: new Float32Array(segs * 8),
    w: new Float32Array(segs * 8),
    q: new Float32Array(segs * 8),
    index: new Uint32Array(segs * 6),
  };
  let s = 0;
  for (let l = 0; l + 1 < starts.length; l++) {
    for (let v = starts[l]; v + 1 < starts[l + 1]; v++) {
      const i = v * 4;
      for (let c = 0; c < 4; c++) {
        const o = (s * 4 + c) * 2;
        g.a[o] = verts[i];
        g.a[o + 1] = verts[i + 1];
        g.b[o] = verts[i + 4];
        g.b[o + 1] = verts[i + 5];
        g.corner[o] = CORNERS[c * 2];
        g.corner[o + 1] = CORNERS[c * 2 + 1];
        g.w[o] = verts[i + 2];
        g.w[o + 1] = verts[i + 6];
        g.q[o] = verts[i + 3];
        g.q[o + 1] = verts[i + 7];
      }
      const x = s * 6;
      const b = s * 4;
      g.index[x] = b;
      g.index[x + 1] = b + 1;
      g.index[x + 2] = b + 2;
      g.index[x + 3] = b;
      g.index[x + 4] = b + 2;
      g.index[x + 5] = b + 3;
      s++;
    }
  }
  return g;
}

/** Settlement polygons owned by one tile (see payload.rs). */
export interface TileSitePolys {
  starts: Uint32Array;
  attrs: Uint32Array;
  verts: Float32Array;
}

/** GPU-ready settlement geometry: triangulated fills (x, y, r, g, b, a) and ink capsules. */
export interface SiteGeometry {
  /** Polygons, one vertex per triangle corner; `edge` per vertex: the corner's barycentric
   * coordinate for each triangle edge that is an outlined polygon's real edge, 1 for an inner
   * diagonal or an unoutlined polygon (the fill shader inks distance-to-edge < ~1 px). */
  fill: { pos: Float32Array; color: Float32Array; edge: Float32Array; index: Uint32Array } | null;
  ink: RiverGeometry | null;
}

/** A wall's stone as the plan fills it (towers, bastions): granite, limestone, sandstone, rose brick, mud brick,
 * then timber and rammed earth. The same tones as `tileink.frag`'s wall core. */
const STONE_TONE: [number, number, number][] = [
  [0.66, 0.71, 0.78],
  [0.86, 0.8, 0.65],
  [0.9, 0.72, 0.42],
  [0.82, 0.52, 0.47],
  [0.86, 0.64, 0.52],
  [0.55, 0.39, 0.24],
  [0.8, 0.72, 0.56],
];

/** A tone a third of the way to its own grey (a bastion's broad fill, beside its wall's narrow core). */
const calm = ([r, g, b]: [number, number, number]): [number, number, number] => {
  const y = 0.3 * r + 0.59 * g + 0.11 * b;
  return [r + (y - r) * 0.35, g + (y - g) * 0.35, b + (y - b) * 0.35];
};

// Fill colours by ward (buildings), then plaza, field, block. Watabou-ish muted browns.
/** How much of a drawn building's roof colour shows at city zoom (the rest its ward's paper; a
 * generated building's is 0.6). */
const DRAWN_ROOF_MIX = 0.9;
/** A stone bridge's deck at city zoom: a mid, slightly cool stone grey, darker than a paved road
 * (`tileroad.frag` PAVED) and the water, so it holds as a bridge from the city zooms. Inked as a
 * timber deck. */
const STONE_DECK_RGB: [number, number, number] = [0.48, 0.49, 0.5];
const WARD_RGB: [number, number, number][] = [
  [0.62, 0.56, 0.5], // plaza (unused for buildings)
  [0.5, 0.45, 0.42], // castle
  [0.6, 0.52, 0.48], // temple
  [0.64, 0.57, 0.49], // merchant
  [0.6, 0.55, 0.5], // craft
  [0.68, 0.6, 0.5], // noble
  [0.63, 0.58, 0.52], // common
  [0.56, 0.52, 0.48], // slum
  [0.58, 0.54, 0.5], // docks
  [0.52, 0.49, 0.46], // military
  [0.64, 0.58, 0.5], // farm
  [0.64, 0.57, 0.48], // rural
  [0.6, 0.62, 0.5], // park
];

/** How much larger than built a wall tower's shape is drawn on the plan, so that a D, an octagon or a square
 * reads beside the curtain from far out (a gate's towers, already wide, as built). */
const PLAN_TOWER = 1.4;
/** A D tower's flat back on the plan: this far (ft) behind its middle, past the curtain's inner face (a stone
 * curtain's half is 4.5 ft), so its square corners show inside the wall and its round face outside. */
const PLAN_D_BACK = 8;

/** A wall tower's footprint on the plan (tile units): a D (flat back, round face out along `ang`), an octagon or a
 * square (sides along `ang`) of radius `r` (ft), as `wallstyle::footprint` lays it. */
function towerPoly(x: number, y: number, r: number, ang: number, shape: number, ftTile: number): number[] {
  const [ux, uy] = [Math.cos(ang), Math.sin(ang)];
  const at = (a: number, b: number) => [x + (ux * a - uy * b) / ftTile, y + (uy * a + ux * b) / ftTile];
  const out: number[] = [];
  if (shape === 2) {
    out.push(...at(-PLAN_D_BACK, -r));
    for (let k = 0; k <= 12; k++) {
      const t = -Math.PI / 2 + (Math.PI * k) / 12;
      out.push(...at(r * Math.cos(t), r * Math.sin(t)));
    }
    out.push(...at(-PLAN_D_BACK, r));
  } else if (shape === 3) {
    const rc = r / Math.cos(Math.PI / 8);
    for (let k = 0; k < 8; k++) {
      const t = Math.PI / 8 + (Math.PI * 2 * k) / 8;
      out.push(...at(rc * Math.cos(t), rc * Math.sin(t)));
    }
  } else out.push(...at(-r, -r), ...at(r, -r), ...at(r, r), ...at(-r, r));
  return out;
}

/** A village's headstone tick on the plan (`town::village::PLAN_HEADSTONE`). */
const PLAN_HEADSTONE = 20;

/** Plan kind of a town graveyard's grave mark (not from the generator). */
const GRAVE_KIND = 0xfd;

/** Ear-clip triangulation of a simple polygon's `m` corners from `first` in `verts` (local corner indices;
 * m - 2 triangles). A degenerate ring falls back to the fan. */
function earClip(verts: Float32Array, first: number, m: number): number[][] {
  const P = (i: number) => [verts[(first + i) * 2], verts[(first + i) * 2 + 1]];
  let area = 0;
  for (let i = 0; i < m; i++) {
    const [a, b] = [P(i), P((i + 1) % m)];
    area += a[0] * b[1] - b[0] * a[1];
  }
  const sgn = area >= 0 ? 1 : -1;
  const cross = (a: number[], b: number[], c: number[]) => ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) * sgn;
  const idx = Array.from({ length: m }, (_, i) => i);
  const out: number[][] = [];
  while (idx.length > 3) {
    let cut = -1;
    for (let k = 0; k < idx.length && cut < 0; k++) {
      const [ia, ib, ic] = [idx[(k + idx.length - 1) % idx.length], idx[k], idx[(k + 1) % idx.length]];
      const [a, b, c] = [P(ia), P(ib), P(ic)];
      if (cross(a, b, c) <= 0) continue;
      let ok = true;
      for (const j of idx) {
        if (j === ia || j === ib || j === ic) continue;
        const p = P(j);
        if (cross(a, b, p) >= 0 && cross(b, c, p) >= 0 && cross(c, a, p) >= 0) {
          ok = false;
          break;
        }
      }
      if (ok) cut = k;
    }
    if (cut < 0) cut = 0;
    out.push([idx[(cut + idx.length - 1) % idx.length], idx[cut], idx[(cut + 1) % idx.length]]);
    idx.splice(cut, 1);
  }
  out.push([idx[0], idx[1], idx[2]]);
  return out;
}

/** Roadmap 5 cemeteries: rows of grave marks across each graveyard (plan kind 5) that has none of its own (a
 * village's churchyards and shrine yards get theirs from `town::village::plan_ticks`): pale ticks 3.4 by 1.5 ft
 * across the rows, side by side as graves lie (a little over life size, to read at near zoom; lost in the plot's
 * green farther out), 5 ft apart in rows 8 ft apart, the rows square to the plot's longest side, 3.5 ft in from
 * its edges, off any roofed building in it (a mausoleum, a tomb house) and from under a tree's crown (a yew's,
 * plan kind 14), a walk left across the rows of a plot over 40 ft long. A pure function of the plot's corners
 * (tile units, `tileFt` ft a side). */
function withGraveRows(polys: TileSitePolys, tileFt: number): TileSitePolys {
  const n = polys.attrs.length;
  const kindOf = (i: number) => polys.attrs[i] & 0xff;
  const ring = (i: number): [number, number][] => {
    const out: [number, number][] = [];
    for (let k = polys.starts[i]; k < polys.starts[i + 1]; k++) out.push([polys.verts[k * 2] * tileFt, polys.verts[k * 2 + 1] * tileFt]);
    return out;
  };
  const inside = (p: [number, number], q: [number, number][]) => {
    let c = false;
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      if (q[a][1] > p[1] !== q[b][1] > p[1] && p[0] < ((q[b][0] - q[a][0]) * (p[1] - q[a][1])) / (q[b][1] - q[a][1]) + q[a][0]) c = !c;
    }
    return c;
  };
  const edgeDist = (p: [number, number], q: [number, number][]) => {
    let best = Infinity;
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      const [dx, dy] = [q[a][0] - q[b][0], q[a][1] - q[b][1]];
      const t = Math.max(0, Math.min(1, ((p[0] - q[b][0]) * dx + (p[1] - q[b][1]) * dy) / (dx * dx + dy * dy || 1)));
      best = Math.min(best, Math.hypot(p[0] - q[b][0] - dx * t, p[1] - q[b][1] - dy * t));
    }
    return best;
  };
  const centroid = (q: [number, number][]): [number, number] => [q.reduce((s, p) => s + p[0], 0) / q.length, q.reduce((s, p) => s + p[1], 0) / q.length];
  let ticks: [number, number][] | null = null;
  let roofed: [number, number][][] | null = null;
  const marks: number[][] = [];
  for (let i = 0; i < n; i++) {
    if (kindOf(i) !== 5 || polys.starts[i + 1] - polys.starts[i] < 3) continue;
    ticks ??= Array.from({ length: n }, (_, j) => j)
      .filter((j) => kindOf(j) === PLAN_HEADSTONE)
      .map((j) => centroid(ring(j)));
    roofed ??= Array.from({ length: n }, (_, j) => j)
      .filter((j) => kindOf(j) === 0 || kindOf(j) === 14)
      .map(ring);
    const q = ring(i);
    if (ticks.some((t) => inside(t, q))) continue;
    // The longest side: the rows run along it.
    let [ux, uy, best] = [1, 0, 0];
    for (let a = 0, b = q.length - 1; a < q.length; b = a++) {
      const d = Math.hypot(q[a][0] - q[b][0], q[a][1] - q[b][1]);
      if (d > best) [ux, uy, best] = [(q[a][0] - q[b][0]) / d, (q[a][1] - q[b][1]) / d, d];
    }
    const [o0, o1] = q[0];
    const along = q.map(([x, y]) => (x - o0) * ux + (y - o1) * uy);
    const across = q.map(([x, y]) => -(x - o0) * uy + (y - o1) * ux);
    const [u0, u1, w0, w1] = [Math.min(...along), Math.max(...along), Math.min(...across), Math.max(...across)];
    if (u1 - u0 < 8 || w1 - w0 < 8 || (u1 - u0) * (w1 - w0) > 4e6) continue;
    const inPlot = roofed.filter((b) => inside(centroid(b), q));
    const walk = u1 - u0 > 40 ? (u0 + u1) / 2 : NaN;
    // (Rows and marks centred in the plot, so the margins match on both sides.)
    const [rows, cols] = [Math.floor((w1 - w0 - 7) / 8), Math.floor((u1 - u0 - 7 - 1.5) / 5)];
    const [s0, x0] = [(w0 + w1) / 2 - (rows * 8) / 2, (u0 + u1) / 2 - (cols * 5) / 2];
    for (let r = 0; r <= rows; r++) {
      for (let c = 0; c <= cols; c++) {
        const [u, w] = [x0 + c * 5, s0 + r * 8];
        if (Math.abs(u - walk) < 4) continue;
        const p: [number, number] = [o0 + u * ux - w * uy, o1 + u * uy + w * ux];
        if (!inside(p, q) || edgeDist(p, q) < 3.5) continue;
        if (inPlot.some((b) => inside(p, b) || edgeDist(p, b) < 3)) continue;
        const [hx, hy, nx, ny] = [ux * 1.0, uy * 1.0, -uy * 2.0, ux * 2.0];
        marks.push([p[0] - hx - nx, p[1] - hy - ny, p[0] + hx - nx, p[1] + hy - ny, p[0] + hx + nx, p[1] + hy + ny, p[0] - hx + nx, p[1] - hy + ny].map((v) => v / tileFt));
      }
    }
  }
  if (!marks.length) return polys;
  const starts = Array.from(polys.starts);
  const attrs = Array.from(polys.attrs);
  const verts = Array.from(polys.verts);
  for (const m of marks) {
    attrs.push(GRAVE_KIND);
    verts.push(...m);
    starts.push(starts[starts.length - 1] + 4);
  }
  return { starts: new Uint32Array(starts), attrs: new Uint32Array(attrs), verts: new Float32Array(verts) };
}

/** Plan kind of a D, octagonal or square wall tower's footprint (not from the generator; the stone in its ward byte). */
const TOWER_KIND = 0xfe;

/**
 * A wall run drawn as one band: the capsule of each piece of a polyline has a round end with its own outline, which
 * across a joint reads as a bead. Each wall piece's `q.y` (unused by `tileink` otherwise) carries a joint code per end
 * (0 none, 1 the run turns left there, 2 right, 3 shallow; end A + 4 * end B): the shader draws a joined end's cap without its
 * rim except on the outside of the bend. A ring (first point = last) joins at its seam too.
 */
function joinWallInk(g: RiverGeometry, starts: number[], v: Float32Array, ft: number, towers: number[][]): void {
  const CUT = 24; // (ft from the tile's edge: a run's end there is probably where the tile cuts it, its neighbour carries on)
  const isWall = (i: number) => {
    const st = v[i * 4 + 3];
    return (st >= 1 && st < 2) || st === 5 || st === 6;
  };
  // (3: a shallow bend, the cap bare all round; 1 or 2: a sharp left or right turn, rim kept on its outside.)
  const turn = (a: number, b: number, c: number) => {
    const [ux, uy, wx, wy] = [v[b * 4] - v[a * 4], v[b * 4 + 1] - v[a * 4 + 1], v[c * 4] - v[b * 4], v[c * 4 + 1] - v[b * 4 + 1]];
    const cr = ux * wy - uy * wx;
    const dot = ux * wx + uy * wy;
    if (dot > 0.94 * Math.hypot(ux, uy) * Math.hypot(wx, wy)) return 3;
    return cr > 0 ? 1 : 2;
  };
  // (Vertices are tile fractions; `ft` is the tile's side. Pieces of a run that are lines of their own join where end points
  // meet, within 1.5 ft: end vertex -> the other end's vertex beside it. An end that meets a wall's side (a T), lies under a
  // tower or gate, or is where the tile cuts the run (outside, or near its edge) has no visible cap: code 3, bare.)
  const X = (i: number) => v[i * 4] * ft;
  const Y = (i: number) => v[i * 4 + 1] * ft;
  const ends: { i: number; beside: number }[] = [];
  const sides: number[][] = [];
  for (let l = 0; l + 1 < starts.length; l++) {
    const [v0, v1] = [starts[l], starts[l + 1]];
    if (v1 - v0 < 2 || !isWall(v0)) continue;
    ends.push({ i: v0, beside: v0 + 1 }, { i: v1 - 1, beside: v1 - 2 });
    for (let i = v0; i + 1 < v1; i++) sides.push([i, i + 1]);
  }
  const TOL = 1.5;
  const mate = (e: number, beside: number) => {
    const m = ends.filter((o) => o.i !== e && Math.hypot(X(o.i) - X(e), Y(o.i) - Y(e)) < TOL);
    return m.length === 1 && m[0].beside !== beside ? m[0].beside : -1;
  };
  const bare = (e: number) => {
    const [x, y] = [X(e), Y(e)];
    if (x < CUT || y < CUT || x > ft - CUT || y > ft - CUT) return true;
    if (towers.some(([tx, ty, r]) => Math.hypot(tx - x, ty - y) < r + 1)) return true;
    if (ends.some((o) => o.i !== e && Math.hypot(X(o.i) - x, Y(o.i) - y) < TOL)) return true;
    return sides.some(([p, q]) => {
      if (p === e || q === e) return false;
      const [dx, dy] = [X(q) - X(p), Y(q) - Y(p)];
      const t = Math.max(0, Math.min(1, ((x - X(p)) * dx + (y - Y(p)) * dy) / Math.max(dx * dx + dy * dy, 1e-6)));
      return Math.hypot(X(p) + dx * t - x, Y(p) + dy * t - y) < TOL;
    });
  };
  let s = 0;
  for (let l = 0; l + 1 < starts.length; l++) {
    const [v0, v1] = [starts[l], starts[l + 1]];
    const wall = v1 - v0 > 1 && isWall(v0);
    const ring = wall && v1 - v0 > 3 && v[v0 * 4] === v[(v1 - 1) * 4] && v[v0 * 4 + 1] === v[(v1 - 1) * 4 + 1];
    for (let i = v0; i + 1 < v1; i++) {
      let code = 0;
      if (wall) {
        const [ma, mb] = [i > v0 || ring ? -1 : mate(i, i + 1), i + 2 < v1 || ring ? -1 : mate(i + 1, i)];
        const a = i > v0 ? turn(i - 1, i, i + 1) : ring ? turn(v1 - 2, i, i + 1) : ma >= 0 ? turn(ma, i, i + 1) : bare(i) ? 3 : 0;
        const b = i + 2 < v1 ? turn(i, i + 1, i + 2) : ring ? turn(i, i + 1, v0 + 1) : mb >= 0 ? turn(i, i + 1, mb) : bare(i + 1) ? 3 : 0;
        code = a + 4 * b;
      }
      for (let c = 0; c < 4; c++) g.q[(s * 4 + c) * 2 + 1] = code;
      s++;
    }
  }
}

/**
 * Settlement fills and ink for one tile. `looks` (`TerrainPayload.siteLooks`) and `tileFt` (the tile's side in ft)
 * give the plan the walls' look by culture group: a stone wall's ink tinted by its stone, a palisade or rammed
 * earth wall by its material (else guessed from its width), D, octagonal and square towers as their shapes
 * (round ones stay dots). Without them (an older payload) the old drawing.
 */
export function buildSiteGeometry(polys: TileSitePolys, lines: TileRivers, looks?: TileRivers, tileFt = 0): SiteGeometry | null {
  const useLooks = !!looks && tileFt > 0 && looks.verts.length === lines.verts.length;
  // (Each tower line in a shape of its own is a polygon instead of a dot: its centre, radius, facing, shape, stone.)
  const shapes: { starts: number; verts: number[]; stone: number }[] = [];
  const skip = new Set<number>();
  if (useLooks) {
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      if (lines.verts[v * 4 + 3] !== 1) continue;
      const code = looks!.verts[v * 4 + 2];
      const shape = codeShape(code);
      // (A palisade's or an earth wall's square gate towers too: log towers, in their material's tone.)
      const mat = codeMaterial(code);
      if (!shape || (mat !== 0 && shape !== 1)) continue;
      const [x, y, w] = [lines.verts[v * 4], lines.verts[v * 4 + 1], lines.verts[v * 4 + 2]];
      const r = w < 25 ? (w / 2) * PLAN_TOWER : w / 2;
      shapes.push({ starts: 0, verts: towerPoly(x, y, r, looks!.verts[v * 4 + 3], shape, tileFt), stone: mat === 1 ? 5 : mat === 2 ? 6 : codeStone(code) });
      skip.add(l);
    }
    // (A bastion takes the look of the wall line nearest it: its stone, in its ward byte + 1.)
    const wallLooks: [number, number, number][] = [];
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      if (lines.verts[v * 4 + 3] !== 0) continue;
      const code = looks!.verts[v * 4 + 2];
      if (code > 0) for (let k = v; k < lines.starts[l + 1]; k++) wallLooks.push([lines.verts[k * 4], lines.verts[k * 4 + 1], code]);
    }
    let bastions: Uint32Array | null = null;
    if (wallLooks.length) {
      bastions = new Uint32Array(polys.attrs);
      for (let i = 0; i < bastions.length; i++) {
        if ((bastions[i] & 0xff) !== 32) continue;
        let [cx, cy] = [0, 0];
        const m = polys.starts[i + 1] - polys.starts[i];
        for (let k = polys.starts[i]; k < polys.starts[i + 1]; k++) [cx, cy] = [cx + polys.verts[k * 2] / m, cy + polys.verts[k * 2 + 1] / m];
        let best = Infinity;
        let code = 0;
        for (const w of wallLooks) {
          const d = (w[0] - cx) ** 2 + (w[1] - cy) ** 2;
          if (d < best) [best, code] = [d, w[2]];
        }
        const mat = codeMaterial(code);
        bastions[i] |= ((mat === 1 ? 5 : mat === 2 ? 6 : codeStone(code)) + 1) << 8;
      }
    }
    if (bastions) polys = { starts: polys.starts, attrs: bastions, verts: polys.verts };
    if (shapes.length) {
      const starts = Array.from(polys.starts);
      const attrs = Array.from(polys.attrs);
      const verts = Array.from(polys.verts);
      for (const sh of shapes) {
        attrs.push(TOWER_KIND | (sh.stone << 8));
        verts.push(...sh.verts);
        starts.push(starts[starts.length - 1] + sh.verts.length / 2);
      }
      polys = { starts: new Uint32Array(starts), attrs: new Uint32Array(attrs), verts: new Float32Array(verts) };
    }
  }
  if (tileFt > 0) polys = withGraveRows(polys, tileFt);
  const n = polys.attrs.length;
  let tris = 0;
  for (let i = 0; i < n; i++) tris += Math.max(0, polys.starts[i + 1] - polys.starts[i] - 2);
  const outlined = (kind: number) => kind === 0 || kind === 3 || kind === 4 || kind === 5 || kind === 6 || kind === 7 || kind === 9 || kind === 13 || kind === 14 || kind === 15 || kind === 23 || kind === 24 || kind === 25 || kind === 26 || kind === 29 || kind === 32 || kind === 33 || kind === 44 || kind === 45 || kind === 46 || kind === TOWER_KIND;
  let fill: SiteGeometry['fill'] = null;
  if (n) {
    // Unshared corners (3 per triangle), so each carries its triangle's edge coordinates.
    const pos = new Float32Array(tris * 6);
    const color = new Float32Array(tris * 12);
    const edge = new Float32Array(tris * 9);
    const index = new Uint32Array(tris * 3);
    let v = 0;
    for (let i = 0; i < n; i++) {
      const s0 = polys.starts[i];
      const m = polys.starts[i + 1] - s0;
      const attr = polys.attrs[i];
      const kind = attr & 0xff;
      // (A building's ward byte carries in bit 7 whether it was drawn by hand: `payload::pack_sites`.)
      const drawn = kind === 0 && ((attr >> 15) & 1) === 1;
      const ward = (attr >> 8) & (kind === 0 ? 0x7f : 0xff);
      const hasFunc = (attr >> 24) & 1;
      // (Its roof's material: `payload::pack_sites`, `town::roof`.)
      const material = (attr >> 20) & 15;
      let rgb: [number, number, number];
      let a = 1;
      if (kind === TOWER_KIND) rgb = STONE_TONE[Math.min(ward, STONE_TONE.length - 1)];
      else if (kind === 1) rgb = [0.86, 0.82, 0.71];
      else if (kind === 2) {
        // Fields: alternate crop tints.
        const k = i % 3;
        rgb = k === 0 ? [0.78, 0.77, 0.55] : k === 1 ? [0.72, 0.74, 0.5] : [0.8, 0.74, 0.52];
        a = 0.55;
      } else if (kind === 3) rgb = [0.62, 0.56, 0.5];
      else if (kind === 4) {
        // Ruin: roofless shell, pale rubble.
        rgb = [0.7, 0.67, 0.6];
        a = 0.55;
      } else if (kind === 5) {
        // A graveyard or cemetery (an open-air plot): inked round, a muted green over the land's own
        // ground, its rows of grave marks over it (`withGraveRows`, or a village's own ticks).
        rgb = [0.5, 0.6, 0.42];
        a = 0.55;
      }
      // A deck: timber brown, a stone bridge's or pier's (bit 20: `payload::pack_sites`) grey stone.
      else if (kind === 6) rgb = material === 1 ? STONE_DECK_RGB : [0.55, 0.4, 0.26];
      else if (kind === 7) rgb = [0.72, 0.69, 0.62];
      else if (kind === 8) {
        // A camp's clearing: bare earth.
        rgb = [0.72, 0.62, 0.47];
        a = 0.7;
      } else if (kind === 9) rgb = [0.86, 0.8, 0.66];
      // A park's water (a pond fanned from its middle, a stream; not outlined), gravel, parterres
      // (hedged beds), trees' crowns and flowerbeds.
      else if (kind === 11) rgb = [0.42, 0.6, 0.64];
      // Roadmap 5 horse parks (`town::horses::PLAN_FENCE`, kind 34, the mark in the ward byte) over the
      // park's trodden ground (a camp clearing's, kind 8); not inked: a corral's post-and-rail fence (0),
      // a bold line of dark timber; a corral's ground (1) a shade greener; a horse (2) a dark bay bar; a
      // haystack (3) a straw round; a hitching rail or a cart (4) a timber tick.
      else if (kind === 34) {
        [rgb, a] = HORSE_PARK_PLAN[ward] ?? HORSE_PARK_PLAN[0];
      }
      // A kurgan mound (kind 33): a low grassy barrow, a shade darker than a cemetery plot, lightly inked.
      else if (kind === 33) {
        rgb = [0.44, 0.52, 0.3];
        a = 0.6;
      }
      // A town wall's bastion (`payload.rs` kind 32): a stone platform, outlined (its faces are the town
      // wall's lines), in its wall's stone calmed toward grey: a broad fill reads louder than the curtain's
      // narrow core in the same tone.
      else if (kind === 32) rgb = ward ? calm(STONE_TONE[Math.min(ward - 1, STONE_TONE.length - 1)]) : [0.76, 0.73, 0.65];
      else if (kind === 12) rgb = [0.85, 0.81, 0.69];
      else if (kind === 13) {
        rgb = [0.42, 0.55, 0.33];
        a = 0.7;
      } else if (kind === 14) rgb = [0.33, 0.46, 0.26];
      else if (kind === 15) rgb = [0.74, 0.5, 0.46];
      // A castle's moat (`town::moat`): a wet one water a shade deeper than the rivers' (its berm by
      // the curtain left uninked, so a pale strip shows between them), a dry ditch a band of warm
      // earth to its block's edge; inked along their banks (and their ends at a gap) by the edges
      // in the ward's place (`Moat::inks`). Its drawbridge (26) a timber deck over the gap.
      else if (kind === 24) rgb = [0.3, 0.45, 0.57];
      else if (kind === 25) rgb = [0.6, 0.48, 0.33];
      else if (kind === 26) rgb = [0.55, 0.4, 0.25];
      // R5 M20: an islet drawn by hand (44) bare rock; a quay's or breakwater's made ground (45)
      // paving stone.
      else if (kind === 44) rgb = [0.6, 0.57, 0.5];
      else if (kind === 45) rgb = [0.74, 0.71, 0.64];
      // (A metropolis's lined bridge (46): the street's paving (`tileroad.frag` PAVED), carried over the water.)
      else if (kind === 46) rgb = [0.62, 0.58, 0.52];
      // v63 countryside: a village's headstones and a churchyard's low wall (`town::village::plan_polys`;
      // the green's and churchyard's grass is the land's own, its crofts seen on the battlemap only).
      else if (kind === 20) rgb = [0.74, 0.73, 0.69];
      // A town graveyard's grave marks (`withGraveRows`): pale stone on its green.
      else if (kind === GRAVE_KIND) rgb = [0.9, 0.9, 0.82];
      else if (kind === 22) rgb = [0.62, 0.6, 0.55];
      // A windmill's sails (`town::rural::plan_ticks`), over its roof.
      else if (kind === 23) rgb = [0.88, 0.84, 0.74];
      // v63 underground: a ruin's plan (`ruinPlan.ts`).
      else if (ruinPlanFill(kind)) [rgb, a] = ruinPlanFill(kind)!;
      else if ((attr >>> 25) & 15) {
        // A keep's piece (`payload::pack_sites`): towers and the body's hall under slate, a wing or
        // forebuilding by its top (`town::keep_tops`): open battlements stone, a roof lead or slate; a
        // monastery's church (5) its ranges' colour deepened, its bell-cote (6) slate; a town hall's
        // belfry (7) under its verdigris spire, its open bell lantern (8) pale stone, its long shadow
        // (9) down the hall's roof.
        const piece = ((attr >>> 25) & 15) - 1;
        const top = (attr >>> 29) & 3;
        // R5 towns (115): a yurt (10) its felt (`YURT_FELT`, by the shade bits), softened toward its ward's paper.
        if (piece === 10) {
          const base = WARD_RGB[ward] ?? WARD_RGB[6];
          const m = YURT_FELT[(attr >>> 29) & 7] ?? YURT_FELT[0];
          rgb = [0, 1, 2].map((c) => 0.7 * (((m >> (16 - 8 * c)) & 255) / 255) + 0.3 * base[c]) as [number, number, number];
        }
        // (R5 towns, 114: a felt hall's crown, 12, its wooden ring dark over the smoke hole.)
        else if (piece === 11) rgb = [0.38, 0.29, 0.21];
        // (R5 M20: a lighthouse's lamp (13), a warm light on its slate cone.)
        else if (piece === 12) rgb = [0.93, 0.78, 0.4];
        else if (piece === 7) rgb = [0.36, 0.6, 0.53];
        else if (piece === 8) rgb = [0.86, 0.8, 0.66];
        else if (piece === 9) {
          rgb = [0, 0, 0];
          a = 0.3;
        } else if (piece === 5) {
          const base = WARD_RGB[ward] ?? WARD_RGB[6];
          rgb = [base[0] * 0.7, base[1] * 0.58, base[2] * 0.52];
        } else rgb = piece === 3 || piece === 4 ? (top === 0 ? [0.62, 0.6, 0.55] : top === 1 ? [0.4, 0.42, 0.44] : [0.38, 0.42, 0.47]) : piece === 0 ? [0.42, 0.45, 0.49] : [0.36, 0.4, 0.45];
      } else if (kind === 0) {
        // A roofed building: its roof material's colour, softened toward its ward's paper brown
        // (watabou-ish), a business a little deeper.
        // A building drawn by hand shows the roof chosen for it, barely softened (`DRAWN_ROOF_MIX`).
        const base = WARD_RGB[ward] ?? WARD_RGB[6];
        const m = roofRgb(material, (attr >>> 29) & 7);
        const mix = drawn ? DRAWN_ROOF_MIX : 0.6;
        const shade = drawn ? 1 : (0.94 + 0.12 * (((i * 2654435761) >>> 0) / 4294967296)) * (hasFunc ? 0.88 : 1);
        rgb = [0, 1, 2].map((c) => (mix * (((m >> (16 - 8 * c)) & 255) / 255) + (1 - mix) * base[c]) * shade) as [number, number, number];
      } else {
        const base = WARD_RGB[ward] ?? WARD_RGB[6];
        const shade = 0.94 + 0.12 * (((i * 2654435761) >>> 0) / 4294967296);
        rgb = hasFunc ? [base[0] * 0.86, base[1] * 0.74, base[2] * 0.66] : [base[0] * shade, base[1] * shade, base[2] * shade];
      }
      // (A belfry's shadow not inked.)
      const ink = outlined(kind) && ((attr >>> 25) & 15) !== 10;
      const inks = kind === 24 || kind === 25 ? ward : -1;
      const inkEdge = (j: number) => ink && (j >= 8 || ((inks >> j) & 1) !== 0);
      // Fan triangles (0, k, k+1): the edge k..k+1 is always the polygon's; 0..1 only for the
      // first triangle, k+1..0 only for the last.
      // A plot (kind 5) is often concave: ear-clipped, so no wedge crosses its notches (a fan would).
      // (A breakwater's outline (45) bends along its line: ear-clipped too.)
      const ears = (kind === 5 || kind === 45) && m > 3 ? earClip(polys.verts, s0, m) : null;
      for (let k = 1; k + 1 < m; k++) {
        const t = ears ? ears[k - 1] : [0, k, k + 1];
        const corners = [s0 + t[0], s0 + t[1], s0 + t[2]];
        // Which edges are real, by the corner opposite each.
        const side = (x: number, y: number) => (Math.abs(x - y) === 1 ? inkEdge(Math.min(x, y)) : Math.abs(x - y) === m - 1 && inkEdge(m - 1));
        const real = ears
          ? [side(t[1], t[2]), side(t[2], t[0]), side(t[0], t[1])]
          : [inkEdge(k), k + 1 === m - 1 && inkEdge(m - 1), k === 1 && inkEdge(0)];
        for (let c = 0; c < 3; c++) {
          pos[v * 2] = polys.verts[corners[c] * 2];
          pos[v * 2 + 1] = polys.verts[corners[c] * 2 + 1];
          color.set([rgb[0], rgb[1], rgb[2], a], v * 4);
          for (let e = 0; e < 3; e++) edge[v * 3 + e] = real[e] ? (c === e ? 1 : 0) : 1;
          index[v] = v;
          v++;
        }
      }
    }
    fill = { pos, color, edge, index };
  }
  // Ink: walls and towers from the line section (building outlines are inked by the fill). A town wall
  // by its width (`payload.rs`, item 104): a palisade's (5 ft) timber brown, rammed earth's (12) tan, both
  // inked along their sides (tileink styles 5, 6); stone's ink.
  const lineVerts = lines.verts.length / 4;
  // With the looks, the material and stone come from the style code: a stone's style carries its stone + 1 in
  // sixteenths after its number (none for limestone, today's ink), a palisade is 5 and rammed earth 6.
  const styleOf = (v: number) => {
    const kind = lines.verts[v * 4 + 3];
    const w = lines.verts[v * 4 + 2];
    if (useLooks && (kind === 0 || kind === 1 || kind === 3)) {
      const code = looks!.verts[v * 4 + 2];
      if (kind === 0 && code > 0) {
        if (codeMaterial(code) === 1) return 5;
        if (codeMaterial(code) === 2) return 6;
      }
      const stone = codeMaterial(code) === 0 && code > 0 ? codeStone(code) : 1;
      return kind + 1 + (stone === 1 || stone > 4 ? 0 : (stone + 1) / 16);
    }
    return kind === 0 && w === 5 ? 5 : kind === 0 && w === 12 ? 6 : kind + 1;
  };
  const starts: number[] = [];
  const out = new Float32Array(lineVerts * 4);
  let o = 0;
  for (let l = 0; l + 1 < lines.starts.length; l++) {
    if (skip.has(l)) continue;
    starts.push(o / 4);
    for (let v = lines.starts[l]; v < lines.starts[l + 1]; v++) {
      out.set([lines.verts[v * 4], lines.verts[v * 4 + 1], lines.verts[v * 4 + 2], styleOf(v)], o);
      o += 4;
    }
  }
  starts.push(o / 4);
  const ink = buildRiverGeometry({ starts: new Uint32Array(starts), verts: out.subarray(0, o) });
  if (ink && tileFt > 0) {
    // (Towers and gates, as centre and radius in ft: wall ends under them need no cap.)
    const towers: number[][] = [];
    for (let l = 0; l + 1 < lines.starts.length; l++) {
      const v = lines.starts[l];
      const k = lines.verts[v * 4 + 3];
      if (k !== 1 && k !== 3) continue;
      const e = lines.starts[l + 1] - 1;
      const [x, y] = [(lines.verts[v * 4] + lines.verts[e * 4]) / 2, (lines.verts[v * 4 + 1] + lines.verts[e * 4 + 1]) / 2];
      towers.push([x * tileFt, y * tileFt, lines.verts[v * 4 + 2] / 2]);
    }
    joinWallInk(ink, starts, out.subarray(0, o), tileFt, towers);
  }
  if (!fill && !ink) return null;
  return { fill, ink };
}
