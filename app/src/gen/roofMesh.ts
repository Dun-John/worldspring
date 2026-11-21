// Roof meshes, run in the generator workers (`battlePrep.ts`): pitched, domed and flat roofs
// tessellated into one vertex-coloured mesh.
import earcut from 'earcut';
import { FELT, roofRgb, ROOF_MATERIAL_RGB, YURT_FELT } from './protocol';
import { domeSpots, frame, isConvex, rangesRoof, ridgeEnds, roofParts, roundOutline as roundEaves, signedArea, straighten, unionEdges } from './roofGeom';
import type { ColorMesh, Roof } from './battlePrep';
import { PX } from '../render/scale';
import { INK, FT } from '../render/pen';

export { PX };

// Roof tints: terracotta, slate, thatch, weathered shingle, moss.
export const ROOF_TINTS = [0x995742, 0x5c6470, 0xa88b5c, 0x7a6656, 0x7f7b54];
export const ROOF_INK = INK;
/** Ink weights (chunk px): round a roof between the big props' (stalls, carts: 0.055 sq), a little over the
 * sprites' `LINE` inside it (0.032); never thinner on screen than at 29 px a square (`colormesh.vert`),
 * as the mipmapped sprites' ink. */
const INK_OUT = 0.055 * PX;
const INK_LINE = 0.032 * PX;

export function shadeHex(c: number, f: number): number {
  const ch = (s: number) => Math.min(255, Math.max(0, Math.round(((c >> s) & 255) * f)));
  return (ch(16) << 16) | (ch(8) << 8) | ch(0);
}

/** Keep the part of a polygon where (p - o)·n <= 0. */
function clipHalf(pts: number[], ox: number, oy: number, nx: number, ny: number): number[] {
  const out: number[] = [];
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const da = (ax - ox) * nx + (ay - oy) * ny;
    const db = (bx - ox) * nx + (by - oy) * ny;
    if (da <= 0) out.push(ax, ay);
    if (da <= 0 !== db <= 0) {
      const t = da / (da - db);
      out.push(ax + (bx - ax) * t, ay + (by - ay) * t);
    }
  }
  return out;
}

/** Chunk pixels per foot. */
/** The roof pattern code of soot round a smithy's chimney (`colormesh.frag`; materials are 0-11). */
const SOOT = 12;

/** Triangles with a per-vertex straight-alpha colour; roof faces carry their material's pattern
 * (`colormesh.frag`), everything else (lines, outlines, shadows) is plain. */
class MeshBuilder {
  pos: number[] = [];
  color: number[] = [];
  roof: number[] = [];
  index: number[] = [];
  /** The roof drawn now: its material (`ROOF_MATERIALS` id), its pattern code (material + 16 ×
   * weathering 0-15) and an offset along its courses (ft) so neighbours' tiles don't line up. */
  mat = 3;
  code = -1;
  seed = 0;

  /** A vertex (its index): u (ft) is weighted by d, divided back in the shader, so a face converging on
   * an apex (d 0 there) has its courses' u run along the rays to it. */
  vert(x: number, y: number, c: number, a: number, u = 0, v = 0, code = -1, d = 1): number {
    this.pos.push(x, y);
    this.color.push(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255, a);
    this.roof.push(u * d, v, code, d);
    return this.pos.length / 2 - 1;
  }

  /** A convex roof face (a fan from its first corner) in the current pattern; per corner u along the
   * courses and v up the slope from the eave (ft), and d (see `vert`). */
  face(pts: number[], uvd: number[], c: number, a = 1, code = this.code) {
    const v0 = this.pos.length / 2;
    const m = pts.length / 2;
    for (let i = 0; i < m; i++) this.vert(pts[i * 2], pts[i * 2 + 1], c, a, uvd[i * 3], uvd[i * 3 + 1], code, uvd[i * 3 + 2]);
    for (let i = 1; i + 1 < m; i++) this.index.push(v0, v0 + i, v0 + i + 1);
  }

  /** Any simple polygon in the current pattern, its u and v from `uv` (ft). */
  patch(pts: number[], uv: (x: number, y: number) => [number, number], c: number, a = 1) {
    const tri = earcut(pts);
    if (!tri.length) return;
    const v0 = this.pos.length / 2;
    for (let i = 0; i < pts.length; i += 2) this.vert(pts[i], pts[i + 1], c, a, ...uv(pts[i], pts[i + 1]), this.code);
    for (const t of tri) this.index.push(v0 + t);
  }

  tri(a: number, b: number, c: number) {
    this.index.push(a, b, c);
  }

  poly(pts: number[], c: number, a = 1) {
    const tri = earcut(pts);
    if (!tri.length) return;
    const v0 = this.pos.length / 2;
    for (let i = 0; i < pts.length; i += 2) this.vert(pts[i], pts[i + 1], c, a);
    for (const t of tri) this.index.push(v0 + t);
  }

  /** A butt-capped line `w` wide, extended by `ext` past each end: its vertices on the middle, the
   * unit normal and side in `roof` (code -2 - w/2; the side ±2 for ink, kept at least as wide as at 29 px
   * a square), widened in `colormesh.vert` to at least a screen pixel plus one to antialias its sides
   * (`colormesh.frag`). */
  line(ax: number, ay: number, bx: number, by: number, w: number, c: number, a: number, ext = 0) {
    const l = Math.hypot(bx - ax, by - ay);
    if (l < 1e-6) return;
    const [ux, uy] = [(bx - ax) / l, (by - ay) / l];
    const [sx, sy, ex, ey] = [ax - ux * ext, ay - uy * ext, bx + ux * ext, by + uy * ext];
    const v0 = this.pos.length / 2;
    const code = -2 - w / 2;
    const ink = c === ROOF_INK ? 2 : 1;
    for (const [x, y, k] of [
      [sx, sy, ink],
      [ex, ey, ink],
      [ex, ey, -ink],
      [sx, sy, -ink],
    ]) {
      this.pos.push(x, y);
      this.color.push(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255, a);
      this.roof.push(-uy, ux, code, k);
    }
    this.index.push(v0, v0 + 1, v0 + 2, v0, v0 + 2, v0 + 3);
  }

  /** A closed outline (square joins). */
  outline(pts: number[], w: number, c: number, a = 1) {
    const m = pts.length / 2;
    for (let i = 0; i < m; i++) this.line(pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1], w, c, a, w / 2);
  }

  build(): ColorMesh {
    return { pos: new Float32Array(this.pos), color: new Float32Array(this.color), roof: new Float32Array(this.roof), index: new Uint32Array(this.index) };
  }
}

/** Pitched, domed and flat roofs, each face patterned in its material (`colormesh.frag`), ridges,
 * hips and an inked eave; drop shadows of every roof (battlemented ones included) first. */
export function roofMesh(roofs: Roof[]): ColorMesh {
  const g = new MeshBuilder();
  for (const roof of roofs) {
    const off = PX * (0.25 + 0.3 * roof.floors);
    const round = roundOutline(roof);
    for (const pts of courtRanges(roof) ?? [round ?? roof.pts]) {
      g.poly(
        pts.map((v) => v + off),
        0x000000,
        0.28,
      );
    }
  }
  for (const roof of roofs) {
    if (roof.battlements || roof.pts.length < 6) continue;
    const tint = roof.tint >= 0 && roof.tint < ROOF_TINTS.length && roof.shade === 2 ? ROOF_TINTS[roof.tint] : (ROOF_MATERIAL_RGB[roof.material] === undefined ? ROOF_TINTS[3] : roofRgb(roof.material, roof.shade));
    g.mat = roof.material === 12 ? 1 : roof.material < ROOF_MATERIAL_RGB.length ? roof.material : 3; // (stone: slate's pattern)
    g.code = g.mat + 16 * Math.min(15, roof.wear >> 4);
    g.seed = (((roof.idx * 2654435761) >>> 0) % 509) * 0.73;
    // (Ranges, halls and wings of a domed roof behind parapets, as flat ones.)
    const flat = roof.shape === FLAT || (roof.shape === DOME && !roof.cone);
    // A palace round its court: a hipped (or flat) range along each side, the court open to the sky.
    const ranges = courtRanges(roof);
    if (ranges) {
      // (Pitched: a range along each side, each as deep as the court leaves it, under one roof with
      // valleys at the court's corners, not four mitred hips whose ridges miss where sides differ.)
      if (flat) for (const part of ranges) flatRoof(g, part, tint);
      else {
        // (A side with next to no depth, a court reaching the eaves, has no range of its own.)
        const sides = sideRanges(roof.pts, roof.parts.find((p) => p.kind === COURT)!.pts).filter((r) => r.length > 0);
        if (sides.length) rangesMesh(g, sides, tint, false);
        else for (const part of ranges) flatRoof(g, part, tint);
      }
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      g.outline(roof.parts.find((p) => p.kind === COURT)!.pts, INK_OUT, ROOF_INK);
      const church = roof.parts.find((p) => p.kind === CHURCH);
      if (church && church.pts.length === 8) churchRoof(g, church.pts, tint);
      // (A coaching inn round its yard: its carriage arch's mouth on the street front.)
      archMouth(g, roof);
      continue;
    }
    // (A city's town hall on its arcade: the piers along its front, the roof's eave over them.)
    arcadePiers(g, roof);
    // A city's town hall's belfry (`town::belfry`): the hall hipped (or flat), the tower over its
    // corner (`belfryTop`).
    const belfry = roof.parts.find((p) => p.kind === SQUARE_TOWER);
    const hall = roof.parts.find((p) => p.kind === BODY);
    if (belfry && hall && belfry.pts.length === 8) {
      if (flat) flatRoof(g, hall.pts, tint);
      else {
        const hp = roofParts(hall.pts).map((p) => p.pts);
        for (const part of hp) hipRoof(g, part, tint);
        if (hp.length > 1) for (const part of hp) g.outline(part, INK_LINE, ROOF_INK);
      }
      g.outline(hall.pts, INK_OUT, ROOF_INK);
      belfryTop(g, belfry.pts);
      continue;
    }
    // A temple of pieces: the nave's ridge along its body to the apse, the apse a half cone, the
    // transept hipped across it (a big one's crossing under a squat tower's spire, or a dome).
    if (roof.parts.some((p) => p.kind === APSE)) {
      templeRoof(g, roof, tint);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    // A manor: its main range hipped along it, its cross wing over the family end hipped across it
    // (a cross gable), drawn over the range's end and inked round (both flat behind parapets in the
    // dry lands).
    const range = roof.parts.find((p) => p.kind === MAIN_RANGE);
    const wing = roof.parts.find((p) => p.kind === CROSS_WING);
    if (range && wing && !roof.cone && roof.shape === DOME) {
      domeRoof(g, roof.pts, tint);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    if (range && wing) {
      if (flat) {
        flatRoof(g, range.pts, tint);
        flatRoof(g, wing.pts, tint);
        g.outline(wing.pts, INK_LINE, ROOF_INK);
      } else crossGable(g, roof.pts, range.pts, wing.pts, tint, roof.shape === GABLE);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    if (!roof.cone && roof.shape === DOME) {
      domeRoof(g, roof.pts, tint);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    // (R5 towns, 114: a steppe home's low felt or turf roof, no parapet.)
    if (!roof.cone && roof.shape === FLAT && (roof.material === HIDE || roof.material === TURF)) {
      const parts = roofParts(roof.pts).map((p) => p.pts);
      for (const part of parts) lowRoof(g, part, tint, roof.material === TURF);
      if (roof.smoke) smoke(g, parts[0], 0.28);
      if (parts.length > 1) for (const part of parts) g.outline(part, INK_LINE, ROOF_INK);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    if (!roof.cone && flat) {
      const inner = flatRoof(g, roof.pts, tint);
      if (inner) roofDeck(g, inner, tint, roof.idx, roof.smoke);
      if (roof.smoke) smoke(g, roof.pts, 0.2);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    // (Roadmap 7, 163: an L, a U or a court, `town::wings`: each range hipped or gabled, the roof the
    // highest of them, valleys where they meet.)
    const wings = roof.parts.filter((p) => p.kind === RANGE).map((p) => p.pts);
    if (wings.length > 1 && !roof.cone && (roof.shape === HIP || roof.shape === GABLE)) {
      rangesMesh(g, wings, tint, roof.shape === GABLE);
      if (roof.smoke) smoke(g, wings[0], 0.28);
      // (Inked round the ranges, not the footprint: a court's gateway range runs on over its arch.)
      for (const [ax, ay, bx, by] of unionEdges(wings)) g.line(ax, ay, bx, by, INK_OUT, ROOF_INK, 1, INK_OUT / 2);
      continue;
    }
    const round = roof.cone ? null : roundOutline(roof);
    if (round) {
      // (Felt whatever the material, unless tinted by hand.)
      // (R5 towns, 115: a yurt its own felts; a felt hall's or a shrine's as they were.)
      const hand = roof.tint >= 0 && roof.tint < ROOF_TINTS.length && roof.shade === 2;
      const yurt = !hand && roof.yurt;
      const felts = yurt ? YURT_FELT : FELT;
      // (R5 towns, 114: a felt hall, temple or inn, its ring wider than any yurt's, a yurt's roof scaled up.)
      const hall = !hand && !yurt && ringRadius(round) > HALL_RING;
      const felt = hand ? tint : hall ? HALL_FELT[roof.shade] ?? HALL_FELT[0] : felts[roof.shade] ?? felts[0];
      if (hall) hallRoof(g, round, felt);
      else roundRoof(g, round, felt, yurt ? FELT_BAND[roof.shade] ?? null : null, yurt && (roof.shade === 7 || roof.shade === 5));
      if (roof.smoke) smoke(g, round, 0.5);
      g.outline(round, INK_OUT, ROOF_INK);
      yurtDoor(g, round, felt);
      continue;
    }
    // Footprints drawn by hand may be concave: a roof on each convex part, valleys between.
    const parts = roofParts(roof.pts).map((p) => p.pts);
    for (const part of parts) {
      if (roof.cone || roof.shape === CONE) coneRoof(g, part, tint);
      else if (roof.shape === GABLE) gableRoof(g, part, tint);
      else hipRoof(g, part, tint);
    }
    if (roof.smoke) smoke(g, parts[0], roof.cone || roof.shape === CONE ? 0.5 : 0.28);
    if (parts.length > 1) for (const part of parts) g.outline(part, INK_LINE, ROOF_INK);
    g.outline(roof.pts, INK_OUT, ROOF_INK);
  }
  return g.build();
}

/** Roof shapes (`town::roof::Shape`). */
const HIP = 0;
const GABLE = 1;
const CONE = 2;
const DOME = 3;
const FLAT = 4;
/** Roof materials (`ROOF_MATERIALS`): turf, hide (felt). */
const TURF = 4;
const HIDE = 9;
const ROUND = 5;

/** A palace's court among its pieces (`battlemap::COURT_PART`); a manor's cross wing and main range
 * (`battlemap::CROSS_WING_PART`, `MAIN_RANGE_PART`). */
const COURT = 5;
const CROSS_WING = 6;
const MAIN_RANGE = 7;
/** A range of an L, a U or a court (`battlemap::RANGE_PART`). */
const RANGE = 11;
/** A temple's pieces (`town::temple`): its body (nave and choir), its transept, its apse
 * (`battlemap::APSE_PART`). */
const BODY = 0;
/** The convex hull of points (anticlockwise in screen axes; monotone chain). */
function hull(p: [number, number][]): [number, number][] {
  const s = [...p].sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  const cr = (o: [number, number], a: [number, number], b: [number, number]) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
  const lo: [number, number][] = [];
  for (const q of s) {
    while (lo.length >= 2 && cr(lo[lo.length - 2], lo[lo.length - 1], q) <= 0) lo.pop();
    lo.push(q);
  }
  const up: [number, number][] = [];
  for (const q of s.reverse()) {
    while (up.length >= 2 && cr(up[up.length - 2], up[up.length - 1], q) <= 0) up.pop();
    up.push(q);
  }
  return [...lo.slice(0, -1), ...up.slice(0, -1)];
}

/** A town hall's belfry (`battlemap::part_code`: a square tower). */

const SQUARE_TOWER = 2;
/** A city's arcaded town hall's open front, or a coaching inn's carriage arch's mouth
 * (`battlemap::ARCADE_PART`): its corners along the front, then a square in. */
const ARCADE = 10;

/** The piers of a town hall's open arcade along its front (`ARCADE`), stone, 2 squares apart from end to
 * end, standing out past the eave drawn over them, the shade of the arcade between them. */
function arcadePiers(g: MeshBuilder, roof: Roof) {
  const front = roof.parts.find((p) => p.kind === ARCADE);
  if (!front || front.pts.length !== 8) return;
  const [ax, ay, bx, by, cx, cy] = front.pts;
  const len = Math.hypot(bx - ax, by - ay);
  if (len < PX) return;
  const [ux, uy] = [(bx - ax) / len, (by - ay) / len];
  // (Out: away from the square in.)
  let [nx, ny] = [-uy, ux];
  if (nx * (cx - bx) + ny * (cy - by) > 0) [nx, ny] = [-nx, -ny];
  const shade = PX * 0.32;
  g.line(ax + nx * shade * 0.5, ay + ny * shade * 0.5, bx + nx * shade * 0.5, by + ny * shade * 0.5, shade, 0x000000, 0.28);
  const n = Math.max(1, Math.round(len / (2 * PX)));
  const r = PX * 0.24;
  for (let k = 0; k <= n; k++) {
    const t = (len * k) / n;
    const [px, py] = [ax + ux * t + nx * r * 0.6, ay + uy * t + ny * r * 0.6];
    const sq = [px - ux * r - nx * r, py - uy * r - ny * r, px + ux * r - nx * r, py + uy * r - ny * r, px + ux * r + nx * r, py + uy * r + ny * r, px - ux * r + nx * r, py - uy * r + ny * r];
    g.poly(
      sq.map((v) => v + PX * 0.12),
      0x000000,
      0.3,
    );
    g.poly(sq, 0xb4ad9e);
    g.outline(sq, INK_LINE, ROOF_INK);
  }
}

/** A town hall's belfry (`SQUARE_TOWER`, its four corners) over the hall's roof: its long shadow
 * (two storeys over the hall and a steep spire's point beyond), the top of its stone walls, a steep
 * spire of verdigris copper, the open bell lantern on top (stone posts, the dark between them, the
 * bell gilt). */
function belfryTop(g: MeshBuilder, pts: number[]) {
  let [cx, cy] = [0, 0];
  for (let i = 0; i < 8; i += 2) [cx, cy] = [cx + pts[i] / 4, cy + pts[i + 1] / 4];
  const off = PX * 2.2;
  const sq: [number, number][] = [];
  for (let i = 0; i < 8; i += 2) sq.push([pts[i], pts[i + 1]], [pts[i] + off, pts[i + 1] + off]);
  sq.push([cx + PX * 4.2, cy + PX * 4.2]);
  g.poly(hull(sq).flat(), 0x000000, 0.32);
  // (Its walls' top, stone, round the spire's eaves.)
  g.poly(pts, 0xb3aa98);
  g.outline(pts, INK_OUT, ROOF_INK);
  const at = (f: number) => pts.map((v, i) => (i % 2 ? cy : cx) + (v - (i % 2 ? cy : cx)) * f);
  const spire = at(0.86);
  // The spire's faces: the lit one pale, the far ones deep (a steep pitch), courses and hips.
  const tint = 0x6a9282;
  const ccw = signedArea(spire) > 0;
  for (let i = 0; i < 4; i++) {
    const [ax, ay, bx, by] = [spire[i * 2], spire[i * 2 + 1], spire[((i + 1) % 4) * 2], spire[((i + 1) % 4) * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay) || 1;
    let [nx, ny] = [(by - ay) / l, -(bx - ax) / l];
    if (!ccw) [nx, ny] = [-nx, -ny];
    const lit = (-(nx + ny) / Math.SQRT2 + 1) / 2;
    g.poly([ax, ay, bx, by, cx, cy], shadeHex(tint, 0.86 + 0.28 * lit));
    for (let k = 1; k < 4; k++) {
      const t = k / 4;
      g.line(ax + (cx - ax) * t, ay + (cy - ay) * t, bx + (cx - bx) * t, by + (cy - by) * t, INK_LINE, shadeHex(tint, 0.72), 0.8);
    }
  }
  for (let i = 0; i < 8; i += 2) g.line(spire[i], spire[i + 1], cx, cy, INK_LINE, ROOF_INK, 1);
  g.outline(spire, INK_OUT, ROOF_INK);
  // The open lantern on top: a stone post at each corner, the dark of its arches between, the bell.
  const lant = at(0.3);
  g.poly(lant.map((v) => v + PX * 0.18), 0x000000, 0.3);
  g.poly(lant, 0x2a2622);
  // (A post: corner P, a third of the way to each corner beside it.)
  const post = (i: number) => {
    const [px, py, qx, qy, rx, ry] = [lant[i], lant[i + 1], lant[(i + 2) % 8], lant[(i + 3) % 8], lant[(i + 6) % 8], lant[(i + 7) % 8]];
    const r = 0.3;
    const [dx, dy, ex, ey] = [(qx - px) * r, (qy - py) * r, (rx - px) * r, (ry - py) * r];
    return [px, py, px + dx, py + dy, px + dx + ex, py + dy + ey, px + ex, py + ey];
  };
  for (let i = 0; i < 8; i += 2) {
    const p = post(i);
    g.poly(p, 0xd6cbb2);
    g.outline(p, INK_LINE, ROOF_INK);
  }
  const br = Math.hypot(lant[2] - lant[0], lant[3] - lant[1]) * 0.22;
  const bell: number[] = [];
  for (let k = 0; k < 10; k++) bell.push(cx + br * Math.cos((k * Math.PI) / 5), cy + br * Math.sin((k * Math.PI) / 5));
  g.poly(bell, 0xc9a13e);
  g.outline(bell, INK_LINE, ROOF_INK);
  g.outline(lant, INK_LINE, ROOF_INK);
}

/** A coaching inn's carriage arch (`ARCADE`: its front along the street, then a square in), the roof
 * kept over it: the yard's cobbles out through its mouth onto the street, the dark of the arch under
 * the eave, a stone jamb either side and the arch's ring of stone. */
function archMouth(g: MeshBuilder, roof: Roof) {
  const front = roof.parts.find((p) => p.kind === ARCADE);
  if (!front || front.pts.length !== 8) return;
  const [ax, ay, bx, by, cx, cy] = front.pts;
  const len = Math.hypot(bx - ax, by - ay);
  if (len < PX) return;
  const [ux, uy] = [(bx - ax) / len, (by - ay) / len];
  // (Out: away from the square in.)
  let [nx, ny] = [-uy, ux];
  if (nx * (cx - bx) + ny * (cy - by) > 0) [nx, ny] = [-nx, -ny];
  const p = (t: number, d: number): [number, number] => [ax + ux * t + nx * d, ay + uy * t + ny * d];
  const quad = (t0: number, t1: number, d0: number, d1: number) => [...p(t0, d0), ...p(t1, d0), ...p(t1, d1), ...p(t0, d1)];
  const jamb = PX * 0.3;
  // The cobbles out from the mouth to the street, a stone each in rows.
  const out = PX * 0.95;
  g.poly(quad(jamb, len - jamb, -PX * 0.05, out), 0x6f695e);
  for (let row = 0; row < 4; row++) {
    const d = PX * (0.05 + row * 0.23);
    const shift = row % 2 ? PX * 0.12 : 0;
    for (let t = jamb + PX * 0.08 + shift; t < len - jamb - PX * 0.12; t += PX * 0.24) {
      g.poly(quad(t, t + PX * 0.17, d, d + PX * 0.16), (row + Math.round(t / PX)) % 3 ? 0x8c8579 : 0x80796d);
    }
  }
  // The arch's dark under the eave: square at the jambs, rounded to its crown in the middle.
  const dark: number[] = [];
  const n = 12;
  for (let k = 0; k <= n; k++) {
    const t = jamb + ((len - 2 * jamb) * k) / n;
    const s = Math.sin((Math.PI * k) / n);
    dark.push(...p(t, -PX * (0.4 + 0.5 * Math.sqrt(s))));
  }
  dark.push(...p(len - jamb, PX * 0.02), ...p(jamb, PX * 0.02));
  g.poly(dark, 0x1f1b18, 0.92);
  // (Its ring of stone along the curve.)
  for (let k = 0; k < n; k++) {
    const [x0, y0, x1, y1] = [dark[k * 2], dark[k * 2 + 1], dark[k * 2 + 2], dark[k * 2 + 3]];
    g.line(x0, y0, x1, y1, PX * 0.09, 0xc9bfa9, 1);
  }
  // The jambs either side, stone, proud of the wall.
  for (const [t0, t1] of [
    [0, jamb],
    [len - jamb, len],
  ]) {
    const j = quad(t0, t1, -PX * 0.5, PX * 0.22);
    g.poly(j.map((v) => v + PX * 0.08), 0x000000, 0.3);
    g.poly(j, 0xc4baa5);
    g.outline(j, INK_LINE, ROOF_INK);
  }
}

const WING = 4;
const APSE = 8;
/** A monastery's church (`battlemap::CHURCH_PART`): its corners from its west end along it. */
const CHURCH = 9;

/** A monastery's church over its range (`CHURCH`: corners 0 and 3 its west end): a storey higher, its
 * shadow cast long on the roofs and the garth beside it, gabled end to end under one heavy ridge (the
 * ranges are hipped below it), its slopes darker than theirs and patterned, inked round; a
 * small bell-cote on its ridge by the west end, a square of slate under a pyramid (the cross temples'
 * spire, smaller). */
function churchRoof(g: MeshBuilder, pts: number[], tint: number) {
  const deep = shadeHex(tint, 0.78);
  g.poly(pts.map((v) => v + PX * 0.6), 0x000000, 0.34);
  const [x0, y0, x1, y1, x2, y2, x3, y3] = pts;
  const el = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ex, ey] = [(x1 - x0) / el, (y1 - y0) / el];
  // (The ridge along the middle from the west gable to the east.)
  const [r0x, r0y, r1x, r1y] = [(x0 + x3) / 2, (y0 + y3) / 2, (x1 + x2) / 2, (y1 + y2) / 2];
  const ccw = signedArea(pts) > 0;
  const u0 = slope(g, [x0, y0, x1, y1, r1x, r1y, r0x, r0y], x0, y0, x1, y1, faceShade(x0, y0, x1, y1, ccw, deep));
  slope(g, [x2, y2, x3, y3, r0x, r0y, r1x, r1y], x2, y2, x3, y3, faceShade(x2, y2, x3, y3, ccw, deep), u0);
  g.line(r0x, r0y, r1x, r1y, INK_LINE, ROOF_INK, 1);
  g.outline(pts, INK_OUT, ROOF_INK);
  // The bell-cote on the ridge, its middle 1.2 squares in from the west end.
  const h = Math.hypot(x3 - x0, y3 - y0) / 2;
  const [cx, cy] = [r0x + ex * PX * 1.2, r0y + ey * PX * 1.2];
  const r = Math.min(PX * 0.6, h * 0.55);
  const [vx, vy] = [-ey, ex];
  const sq = [cx - ex * r - vx * r, cy - ey * r - vy * r, cx + ex * r - vx * r, cy + ey * r - vy * r, cx + ex * r + vx * r, cy + ey * r + vy * r, cx - ex * r + vx * r, cy - ey * r + vy * r];
  g.poly(sq.map((v) => v + PX * 0.25), 0x000000, 0.32);
  withMat(g, 1, () => coneRoof(g, sq, 0x4e5660));
  g.outline(sq, INK_OUT, ROOF_INK);
}

/** The shade of a roof face whose eave runs a-b (the face on the inside of a polygon wound `ccw`): the
 * slope facing the NW light paler. */
function faceShade(ax: number, ay: number, bx: number, by: number, ccw: boolean, tint: number): number {
  const l = Math.hypot(bx - ax, by - ay) || 1;
  // (Outward: a polygon of positive signed area has its inside to the left of a→b in these axes.)
  let [nx, ny] = [(by - ay) / l, -(bx - ax) / l];
  if (!ccw) [nx, ny] = [-nx, -ny];
  const lit = (-(nx + ny) / Math.SQRT2 + 1) / 2;
  // (The shadowed face about a quarter darker than the lit one.)
  return shadeHex(tint, 0.86 + 0.28 * lit);
}

/** A temple's roof (`Roof.parts`: its body, transept and apse): the nave and choir under one ridge from a
 * hip at the door's end to the apse's middle, the apse a half cone about it, courses along both slopes and
 * round the apse; the transept hipped across over the body, its valleys outlined; a big one's crossing
 * under a squat tower's pyramid spire of slate. */
function templeRoof(g: MeshBuilder, roof: Roof, tint: number) {
  const body = roof.parts.find((p) => p.kind === BODY && p.pts.length === 8);
  const apse = roof.parts.find((p) => p.kind === APSE);
  const wing = roof.parts.find((p) => p.kind === WING && p.pts.length === 8);
  if (!body || !apse) {
    for (const part of roofParts(roof.pts)) hipRoof(g, part.pts, tint);
    return;
  }
  // (The body's corners: the door's end 0 and 3, the apse's end 1 and 2; its half width; the ridge from
  // over the hip to the apse's middle.)
  const [x0, y0, x1, y1, x2, y2, x3, y3] = body.pts;
  const h = Math.hypot(x3 - x0, y3 - y0) / 2;
  const el = Math.hypot(x1 - x0, y1 - y0) || 1;
  const [ex, ey] = [(x1 - x0) / el, (y1 - y0) / el];
  const [r1x, r1y] = [(x1 + x2) / 2, (y1 + y2) / 2];
  const [r0x, r0y] = [(x0 + x3) / 2 + ex * h, (y0 + y3) / 2 + ey * h];
  const ccw = signedArea(body.pts) > 0;
  let u0 = slope(g, [x0, y0, x1, y1, r1x, r1y, r0x, r0y], x0, y0, x1, y1, faceShade(x0, y0, x1, y1, ccw, tint));
  u0 = slope(g, [x2, y2, x3, y3, r0x, r0y, r1x, r1y], x2, y2, x3, y3, faceShade(x2, y2, x3, y3, ccw, tint), u0);
  u0 = slope(g, [x3, y3, x0, y0, r0x, r0y], x3, y3, x0, y0, faceShade(x3, y3, x0, y0, ccw, tint), u0);
  ridgeLine(g, r0x, r0y, r1x, r1y, tint);
  // The apse: a half cone about the body's end (its arc: the piece's points but its first and last).
  const a = apse.pts;
  const m = a.length / 2;
  const accw = signedArea(a) > 0;
  for (let i = 1; i + 2 < m; i++) {
    const [ax, ay, bx, by] = [a[i * 2], a[i * 2 + 1], a[(i + 1) * 2], a[(i + 1) * 2 + 1]];
    u0 = fanFace(g, ax, ay, bx, by, r1x, r1y, faceShade(ax, ay, bx, by, accw, tint), u0, h);
  }
  // (Its hips every fourth point, about the middle one.)
  const mid = Math.round((m - 1) / 2);
  for (let i = 1; i + 1 < m; i++) if (Math.abs(i - mid) % 4 === 0) ridgeLine(g, a[i * 2], a[i * 2 + 1], r1x, r1y, tint, true);
  // (A domed one, no transept: the dome over the middle of its body.)
  if (!wing) {
    // (Its drum inside the body's eaves.)
    if (roof.shape === DOME) dome(g, (x0 + x1 + x2 + x3) / 4, (y0 + y1 + y2 + y3) / 4, h * 0.85, tint);
    return;
  }
  // The transept across the body, its valleys inked.
  hipRoof(g, wing.pts, tint, [-ey, ex]);
  g.outline(wing.pts, INK_LINE, ROOF_INK);
  // The crossing's tower: a square a little narrower than the body about the crossing's middle, under a
  // pyramid spire of slate.
  let [cx, cy] = [0, 0];
  for (let i = 0; i < 4; i++) [cx, cy] = [cx + wing.pts[i * 2] / 4, cy + wing.pts[i * 2 + 1] / 4];
  const [qx, qy] = [(x0 + x3) / 2, (y0 + y3) / 2];
  const along = (cx - qx) * ex + (cy - qy) * ey;
  [cx, cy] = [qx + ex * along, qy + ey * along];
  const r = h * 0.82;
  const [vx, vy] = [-ey, ex];
  if (roof.shape === DOME) {
    // (Its drum over the body's eaves into the transept's arms, but not past the transept's: within the
    // circle through the crossing's corners.)
    let [lo, hi] = [Infinity, -Infinity];
    for (let i = 0; i < 4; i++) [lo, hi] = [Math.min(lo, (wing.pts[i * 2] - cx) * ex + (wing.pts[i * 2 + 1] - cy) * ey), Math.max(hi, (wing.pts[i * 2] - cx) * ex + (wing.pts[i * 2 + 1] - cy) * ey)];
    const d = Math.min(-lo, hi);
    dome(g, cx, cy, Math.min(h * 1.05, (0.95 * Math.sqrt(h * h + d * d)) / 1.12), tint);
    return;
  }
  const sq = [cx - ex * r - vx * r, cy - ey * r - vy * r, cx + ex * r - vx * r, cy + ey * r - vy * r, cx + ex * r + vx * r, cy + ey * r + vy * r, cx - ex * r + vx * r, cy - ey * r + vy * r];
  withMat(g, 1, () => coneRoof(g, sq, 0x4e5660));
  g.outline(sq, INK_OUT, ROOF_INK);
}

/** A palace round its court: the ranges along its sides, each between a side of its footprint and
 * the court's side by it (null for any other roof). */
function courtRanges(roof: Roof): number[][] | null {
  const court = roof.parts.find((p) => p.kind === COURT);
  if (!court || roof.pts.length !== 8 || court.pts.length !== 8) return null;
  const [o, c] = [roof.pts, court.pts];
  return [0, 1, 2, 3].map((i) => {
    const j = (i + 1) % 4;
    return [o[i * 2], o[i * 2 + 1], o[j * 2], o[j * 2 + 1], c[j * 2], c[j * 2 + 1], c[i * 2], c[i * 2 + 1]];
  });
}

/** A court's ranges as rectangles (`rangesRoof`'s arms): along each side of outline `o`, in to the
 * court's side `c` across from it (its mean depth), overlapping the next at the corners; empty where
 * that depth is next to none. Both four-cornered, corner by corner (`courtRanges`). */
function sideRanges(o: number[], c: number[]): number[][] {
  const m = o.length / 2;
  return [...Array(m).keys()].map((i) => {
    const j = (i + 1) % m;
    const [ax, ay, bx, by] = [o[i * 2], o[i * 2 + 1], o[j * 2], o[j * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay) || 1;
    let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
    const into = (x: number, y: number) => (x - ax) * nx + (y - ay) * ny;
    const depth0 = (into(c[i * 2], c[i * 2 + 1]) + into(c[j * 2], c[j * 2 + 1])) / 2;
    if (depth0 < 0) [nx, ny] = [-nx, -ny];
    const depth = Math.abs(depth0);
    if (depth < 0.05 * PX) return [];
    return [ax, ay, bx, by, bx + nx * depth, by + ny * depth, ax + nx * depth, ay + ny * depth];
  });
}

/** Sloping up from every side to the middle: each face shaded by how it faces the NW light and
 * patterned in rings, hips to the apex and a finial. */
function coneRoof(g: MeshBuilder, pts: number[], tint: number) {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) [cx, cy] = [cx + pts[i * 2] / m, cy + pts[i * 2 + 1] / m];
  const ccw = signedArea(pts) > 0;
  // (Its rise: the mean distance in from the eaves, so the rings are level all round.)
  let rise = 0;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    rise += Math.abs((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)) / (Math.hypot(bx - ax, by - ay) || 1) / m;
  }
  let u0 = g.seed;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    u0 = fanFace(g, ax, ay, bx, by, cx, cy, faceShade(ax, ay, bx, by, ccw, tint), u0, rise);
  }
  const every = m > 12 ? 2 : 1;
  for (let i = 0; i < m; i += every) ridgeLine(g, cx, cy, pts[i * 2], pts[i * 2 + 1], tint, true);
  const r = PX * 0.12;
  const finial: number[] = [];
  for (let k = 0; k < 8; k++) finial.push(cx + r * Math.cos((k * Math.PI) / 4), cy + r * Math.sin((k * Math.PI) / 4));
  g.poly(finial, ROOF_INK);
}

/** A roof face whose eave runs a→b, in the current pattern: u along the eave from `u0` (ft), v up the
 * slope away from it. Returns u0 past its eave (the next face's along the same roof). */
function slope(g: MeshBuilder, poly: number[], ax: number, ay: number, bx: number, by: number, c: number, u0 = g.seed): number {
  const l = Math.hypot(bx - ax, by - ay);
  if (l < 1e-6) return u0;
  const [ex, ey] = [(bx - ax) / l, (by - ay) / l];
  const m = poly.length / 2;
  let [mx, my] = [0, 0];
  for (let i = 0; i < m; i++) [mx, my] = [mx + poly[i * 2] / m, my + poly[i * 2 + 1] / m];
  let [nx, ny] = [-ey, ex];
  if ((mx - ax) * nx + (my - ay) * ny < 0) [nx, ny] = [-nx, -ny];
  const uvd: number[] = [];
  for (let i = 0; i < m; i++) {
    const [dx, dy] = [poly[i * 2] - ax, poly[i * 2 + 1] - ay];
    uvd.push(u0 + (dx * ex + dy * ey) / FT, (dx * nx + dy * ny) / FT, 1);
  }
  g.face(poly, uvd, c);
  return u0 + l / FT;
}

/** A face from eave a→b up to an apex c (cones, apses), in the current pattern: its courses in rings
 * level all round (v up to `rise` px at the apex), u along the eave from `u0` converging on the apex.
 * Returns u0 past its eave. */
function fanFace(g: MeshBuilder, ax: number, ay: number, bx: number, by: number, cx: number, cy: number, c: number, u0: number, rise: number): number {
  const l = Math.hypot(bx - ax, by - ay) / FT;
  g.face([ax, ay, bx, by, cx, cy], [u0, 0, 1, u0 + l, 0, 1, 0, rise / FT, 0], c);
  return u0 + l;
}

/** A ridge (or a hip) from a to b in the roof's material, one interior ink stroke (`INK_LINE`): thatch's
 * under a rounded capping roll (lit side, shaded side, inked both edges), clay's and glazed tile's down a
 * band of ridge tiles, lead's and copper's beside a lit roll, turf's soft. */
function ridgeLine(g: MeshBuilder, ax: number, ay: number, bx: number, by: number, tint: number, hip = false) {
  const m = g.mat;
  const l = Math.hypot(bx - ax, by - ay);
  if (l < 1e-6) return;
  // (Unit normal towards the NW light.)
  let [nx, ny] = [-(by - ay) / l, (bx - ax) / l];
  if (nx + ny > 0) [nx, ny] = [-nx, -ny];
  const band = (w: number, f: number, off: number) => g.line(ax + nx * off, ay + ny * off, bx + nx * off, by + ny * off, w, shadeHex(tint, f), 1, hip ? 0 : w / 3);
  if (m === 0) {
    // (A hip's roll inked on its shaded side only, so it doesn't read as a double line.)
    const w = (hip ? 0.9 : 1.3) * FT;
    band(w, 0.9, 0);
    band(w * 0.5, 1.08, w * 0.2);
    for (const k of hip ? [-0.5] : [0.5, -0.5]) g.line(ax + nx * w * k, ay + ny * w * k, bx + nx * w * k, by + ny * w * k, INK_LINE, ROOF_INK, 1, hip ? 0 : w / 3);
    return;
  }
  if (m === 2 || m === 7) band((hip ? 0.35 : 0.5) * FT, 0.92, 0);
  else if (m === 5 || m === 6) band(0.22 * FT, 1.14, 0.12 * FT);
  g.line(ax, ay, bx, by, INK_LINE, ROOF_INK, m === 4 ? 0.5 : 1);
}

/** A manor's main range and its cross wing (`interior::cross_wing_of`: its outline cut across) under
 * one roof: a wing wider than the range hipped across it, the range's ridge run on into the wing's
 * slope with a valley either side; else (or if either piece is not convex) one roof over the whole
 * outline (`gable`: a gable roof), so no hips cross at the cut. */
function crossGable(g: MeshBuilder, pts: number[], range: number[], wing: number[], tint: number, gable: boolean) {
  // (The cut: the range's side whose corners are both the wing's; the ridge runs square to it.)
  const m = range.length / 2;
  const tol = 0.05 * PX;
  const on = (x: number, y: number) => wing.some((_, k) => k % 2 === 0 && Math.abs(wing[k] - x) < tol && Math.abs(wing[k + 1] - y) < tol);
  let cut = -1;
  for (let i = 0; i < m && cut < 0; i++) if (on(range[i * 2], range[i * 2 + 1]) && on(range[((i + 1) % m) * 2], range[((i + 1) % m) * 2 + 1])) cut = i;
  const [a, b] = [straighten(range), straighten(wing)];
  if (cut >= 0 && isConvex(a) && isConvex(b)) {
    const j = (cut + 1) % m;
    const [sx, sy] = [range[j * 2] - range[cut * 2], range[j * 2 + 1] - range[cut * 2 + 1]];
    const sl = Math.hypot(sx, sy) || 1;
    const [ux, uy] = [-sy / sl, sx / sl];
    // (Its ridge along u pointing at the wing, so the open end is the frame's far one.)
    let [cx, cy] = [0, 0];
    for (let i = 0; i < m; i++) [cx, cy] = [cx + range[i * 2] / m, cy + range[i * 2 + 1] / m];
    const dir: [number, number] = (range[cut * 2] - cx) * ux + (range[cut * 2 + 1] - cy) * uy > 0 ? [ux, uy] : [-ux, -uy];
    const rf = frame(a, dir);
    const wf = frame(b, [-dir[1], dir[0]]);
    // (The range's ridge stands as high as its half width: it runs into the wing's slope if the wing is
    // as deep as the range is wide, and the wing's ridge across it is a ridge, not a point. A wing cut
    // from the range's own outline is neither: one roof over both.)
    if (wf.halfW >= rf.halfW * 0.98 && wf.hi - wf.lo > 2.1 * wf.halfW) {
      hipRoof(g, b, tint, [-dir[1], dir[0]]);
      hipRoof(g, a, tint, dir, rf.halfW);
      return;
    }
  }
  for (const part of roofParts(pts)) (gable ? gableRoof : hipRoof)(g, part.pts, tint);
}

/** (Roadmap 7, 163) The ranges of an L, a U or a court under one roof (`rangesRoof`): each face shaded
 * by how its eave faces the light, its courses along the eave (in line across ranges sharing an eave
 * line), ridges and hips in the material, valleys inked. */
function rangesMesh(g: MeshBuilder, arms: number[][], tint: number, gable: boolean) {
  const { faces, lines, top } = rangesRoof(arms, gable);
  for (const f of faces) {
    const [px, py, qx, qy] = f.eave;
    const uvd: number[] = [];
    for (let i = 0; i < f.pts.length; i += 2) {
      const [x, y] = [f.pts[i], f.pts[i + 1]];
      uvd.push(g.seed + (x * f.s.ex + y * f.s.ey) / FT, ((x - f.s.ax) * f.s.nx + (y - f.s.ay) * f.s.ny) / FT, 1);
    }
    g.face(f.pts, uvd, faceShade(px, py, qx, qy, f.ccw, tint));
  }
  for (const { seg, valley } of lines) {
    const [ax, ay, bx, by] = seg;
    if (valley) g.line(ax, ay, bx, by, INK_LINE, ROOF_INK, 1);
    else ridgeLine(g, ax, ay, bx, by, tint, Math.abs(top(ax, ay) - top(bx, by)) > 0.05 * FT);
  }
}

/** A hip roof on a convex footprint: a face up from each eave to the ridge (shaded by how it faces the
 * NW light, patterned in its material), ridge and hips; its ridge along `dir` (unit) if given, else along
 * the longest edge. */
function hipRoof(g: MeshBuilder, pts: number[], tint: number, dir?: [number, number], open = 0) {
  const m = pts.length / 2;
  const { ends, end } = ridgeEnds(pts, dir, open);
  const ccw = signedArea(pts) > 0;
  let u0 = g.seed;
  for (let i = 0; i < m; i++) {
    const j = (i + 1) % m;
    const [px, py, qx, qy] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
    const [ei, ej] = [end(i), end(j)];
    // (An open end's eave has no face: the slopes either side run on over it into the other roof.)
    if (open > 0 && ei === 2 && ej === 2) {
      u0 += Math.hypot(qx - px, qy - py) / FT;
      continue;
    }
    const face = ei === ej ? [px, py, qx, qy, ends[ei], ends[ei + 1]] : [px, py, qx, qy, ends[ej], ends[ej + 1], ends[ei], ends[ei + 1]];
    u0 = slope(g, face, px, py, qx, qy, faceShade(px, py, qx, qy, ccw, tint), u0);
  }
  ridgeLine(g, ends[0], ends[1], ends[2], ends[3], tint);
  for (let i = 0; i < m; i++) ridgeLine(g, ends[end(i)], ends[end(i) + 1], pts[i * 2], pts[i * 2 + 1], tint, true);
}

/** Where a line through o along u (unit) crosses a convex polygon about its centroid c: (t0, t1) along
 * it (t0 >= t1 if it misses). */
function chord(pts: number[], ox: number, oy: number, ux: number, uy: number, cx: number, cy: number, reach: number): [number, number] {
  const m = pts.length / 2;
  let [t0, t1] = [-reach, reach];
  for (let i = 0; i < m && t0 < t1; i++) {
    const [ax, ay] = [pts[i * 2], pts[i * 2 + 1]];
    const [ex, ey] = [pts[((i + 1) % m) * 2] - ax, pts[((i + 1) % m) * 2 + 1] - ay];
    // Inward normal: towards the centroid.
    let [inx, iny] = [-ey, ex];
    if ((cx - ax) * inx + (cy - ay) * iny < 0) [inx, iny] = [-inx, -iny];
    const c0 = (ox - ax) * inx + (oy - ay) * iny;
    const c1 = ux * inx + uy * iny;
    if (Math.abs(c1) < 1e-9) {
      if (c0 < 0) t1 = t0;
    } else if (c1 > 0) t0 = Math.max(t0, -c0 / c1);
    else t1 = Math.min(t1, -c0 / c1);
  }
  return [t0, t1];
}

/** A gable roof on a convex footprint: two slopes from the eaves along it up to a ridge from end to end,
 * shaded by how each faces the NW light and patterned; the gable ends inked, a pale verge inside each. */
function gableRoof(g: MeshBuilder, pts: number[], tint: number, dir?: [number, number]) {
  const { cx, cy, ux, uy, lo, hi, halfW, rx, ry } = frame(pts, dir);
  const [nx, ny] = [-uy, ux];
  for (const sg of [1, -1]) {
    const half = clipHalf(pts, rx, ry, -sg * nx, -sg * ny);
    if (half.length < 6) continue;
    const uvd: number[] = [];
    for (let i = 0; i < half.length; i += 2) {
      const [dx, dy] = [half[i] - rx, half[i + 1] - ry];
      uvd.push(g.seed + (dx * ux + dy * uy) / FT, (halfW - sg * (dx * nx + dy * ny)) / FT, 1);
    }
    const lit = (-(sg * nx + sg * ny) / Math.SQRT2 + 1) / 2;
    g.face(half, uvd, shadeHex(tint, 0.86 + 0.28 * lit));
  }
  const [t0, t1] = chord(pts, rx, ry, ux, uy, cx, cy, hi - lo + 1);
  if (t1 > t0) ridgeLine(g, rx + ux * t0, ry + uy * t0, rx + ux * t1, ry + uy * t1, tint);
  // The gable ends (edges across the ridge): a pale verge just inside, then ink.
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay);
    if (l < 1e-6 || Math.abs((bx - ax) * ux + (by - ay) * uy) > 0.6 * l) continue;
    let [ix, iy] = [-(by - ay) / l, (bx - ax) / l];
    if ((cx - ax) * ix + (cy - ay) * iy < 0) [ix, iy] = [-ix, -iy];
    const d = 0.2 * FT;
    g.line(ax + ix * d, ay + iy * d, bx + ix * d, by + iy * d, 0.3 * FT, shadeHex(tint, 1.18), 0.9);
    g.line(ax, ay, bx, by, INK_OUT, ROOF_INK, 1);
  }
}

/** A simple polygon moved in by w on every side (mitred corners, capped); null if it turns inside out. */
function inset(pts: number[], w: number): number[] | null {
  const m = pts.length / 2;
  const sg = signedArea(pts) > 0 ? 1 : -1;
  const nrm = (i: number): [number, number] => {
    const j = (i + 1) % m;
    const [ex, ey] = [pts[j * 2] - pts[i * 2], pts[j * 2 + 1] - pts[i * 2 + 1]];
    const l = Math.hypot(ex, ey) || 1;
    return [(-sg * ey) / l, (sg * ex) / l];
  };
  const out: number[] = [];
  for (let i = 0; i < m; i++) {
    const [ax, ay] = nrm((i + m - 1) % m);
    const [bx, by] = nrm(i);
    const k = Math.max(0.35, 1 + ax * bx + ay * by);
    out.push(pts[i * 2] + (w * (ax + bx)) / k, pts[i * 2 + 1] + (w * (ay + by)) / k);
  }
  const a = signedArea(out);
  return a * sg > 0.15 * Math.abs(signedArea(pts)) ? out : null;
}

/** A flat roof behind a parapet: the parapet's coping round it (a plain lighter band, inked on both
 * edges), the roof's field inside (patterned), the shadow the NW parapet casts on it towards the SE and
 * the lit inner face of the far side, and a small water spout (two on a long side) out through the
 * parapet on its lee side. */
function flatRoof(g: MeshBuilder, pts: number[], tint: number): number[] | null {
  const m = pts.length / 2;
  const { ux, uy, halfW } = frame(pts);
  const w = Math.min(0.9 * FT, halfW * 0.3);
  const [ox, oy] = [pts[0], pts[1]];
  const uv = (x: number, y: number): [number, number] => [g.seed + ((x - ox) * ux + (y - oy) * uy) / FT, (-(x - ox) * uy + (y - oy) * ux) / FT];
  const inner = inset(pts, w);
  if (!inner) {
    g.patch(pts, uv, shadeHex(tint, 1.02));
    return null;
  }
  // The coping in three tones: lit on the sides facing the NW light, shaded on the SE ones.
  const osg = signedArea(pts) > 0 ? 1 : -1;
  for (let i = 0; i < m; i++) {
    const j = (i + 1) % m;
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay) || 1;
    const out = (osg * (by - ay) - osg * (bx - ax)) / l / Math.SQRT2;
    g.poly([ax, ay, bx, by, inner[j * 2], inner[j * 2 + 1], inner[i * 2], inner[i * 2 + 1]], shadeHex(tint, out < -0.3 ? 1.18 : out > 0.3 ? 1.0 : 1.09));
  }
  g.patch(inner, uv, shadeHex(tint, 1.0));
  const sg = signedArea(inner) > 0 ? 1 : -1;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [inner[i * 2], inner[i * 2 + 1], inner[((i + 1) % m) * 2], inner[((i + 1) % m) * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay);
    if (l < 1e-6) continue;
    // (Out towards the parapet.)
    const [onx, ony] = [(sg * (by - ay)) / l, (-sg * (bx - ax)) / l];
    const nw = -(onx + ony) / Math.SQRT2;
    if (nw > 0.25) {
      const d = w * 1.1 * nw;
      g.poly([ax, ay, bx, by, bx - onx * d, by - ony * d, ax - onx * d, ay - ony * d], 0x000000, 0.22);
    } else if (nw < -0.25) {
      const d = w * 0.25;
      g.poly([ax, ay, bx, by, bx - onx * d, by - ony * d, ax - onx * d, ay - ony * d], shadeHex(tint, 1.2), 0.55);
    }
  }
  g.outline(inner, INK_LINE, ROOF_INK);
  g.outline(pts, INK_OUT, ROOF_INK);
  // Water spouts through the lee (S, E) parapet's longest side.
  let [best, bi] = [0, -1];
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay);
    if (l > best && l > 1.5 * PX && (osg * (by - ay) - osg * (bx - ax)) / l > 0.6) [best, bi] = [l, i];
  }
  if (bi < 0) return inner;
  const [ax, ay, bx, by] = [pts[bi * 2], pts[bi * 2 + 1], pts[((bi + 1) % m) * 2], pts[((bi + 1) % m) * 2 + 1]];
  const [ex, ey] = [(bx - ax) / best, (by - ay) / best];
  const [onx, ony] = [osg * ey, -osg * ex];
  const n = best > 10 * PX ? 2 : 1;
  for (let k = 0; k < n; k++) {
    const t = (best * (k + 1)) / (n + 1);
    const [px, py] = [ax + ex * t, ay + ey * t];
    const [hw, i0, o1] = [0.14 * FT, w, 0.5 * FT];
    const q = [px - ex * hw - onx * i0, py - ey * hw - ony * i0, px + ex * hw - onx * i0, py + ey * hw - ony * i0, px + ex * hw + onx * o1, py + ey * hw + ony * o1, px - ex * hw + onx * o1, py - ey * hw + ony * o1];
    g.poly(q.map((v) => v + 0.2 * FT), 0x000000, 0.25);
    g.poly(q, 0x8b6b4a);
    g.outline(q, INK_LINE, ROOF_INK);
  }
  return inner;
}

/** Whether (x, y) lies inside a polygon (even-odd). */
function inside(pts: number[], x: number, y: number): boolean {
  let hit = false;
  const m = pts.length / 2;
  for (let i = 0, j = m - 1; i < m; j = i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
    if (ay > y !== by > y && x < ((bx - ax) * (y - ay)) / (by - ay) + ax) hit = !hit;
  }
  return hit;
}

/** What a house's flat roof is used for, inside its parapet (`inner`): on a big one an upper room
 * at one end, its own parapet round it and its shadow on the terrace; the hatch the stair comes up
 * through by a corner; and a reed sunshade on its posts, a rug laid out to air or a few water jars,
 * chosen by the building (`idx`; a smoky one keeps its hatch away from the chimney, a fifth along
 * from the low end, and has nothing more). A few big shapes, inked as the props are. */
function roofDeck(g: MeshBuilder, inner: number[], tint: number, idx: number, smoky: boolean) {
  const { ux, uy, lo, hi, halfW, rx, ry } = frame(inner);
  const len = hi - lo;
  if (len < 6 * FT || halfW < 2.6 * FT) return;
  const hh = (k: number) => (Math.imul((idx + 1) ^ Math.imul(k + 7, 0x27d4eb2d), 0x9e3779b1) >>> 0) / 4294967296;
  const at = (t: number, s: number): [number, number] => [rx + ux * t - uy * s, ry + uy * t + ux * s];
  // Boxes taken, [t, s, a, b]: a box `a` along by `b` across round (t, s), as a polygon, if it lies
  // inside the parapet clear of those.
  const taken: number[][] = [];
  const box = (t: number, s: number, a: number, b: number, free = true): number[] | null => {
    if (free && taken.some(([t1, s1, a1, b1]) => Math.abs(t - t1) < (a + a1) / 2 && Math.abs(s - s1) < (b + b1) / 2)) return null;
    const out: number[] = [];
    for (const [dt, ds] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) out.push(...at(t + (dt * a) / 2, s + (ds * b) / 2));
    for (let i = 0; i < 8; i += 2) if (!inside(inner, out[i], out[i + 1])) return null;
    if (free) taken.push([t, s, a + 0.8 * FT, b + 0.8 * FT]);
    return out;
  };
  const mid = (lo + hi) / 2;
  const side = hh(1) < 0.5 ? 1 : -1;
  const end = smoky || hh(2) < 0.5 ? 1 : -1;
  // The upper room, at the far end from the hatch, a little in from the parapet.
  if (!smoky && len >= 18 * FT && halfW >= 6 * FT && hh(6) < 0.6) {
    const a = len * (0.34 + 0.14 * hh(7));
    const b = 2 * halfW - (1.2 + 2.5 * hh(8)) * FT;
    const [t, s] = [mid - end * (len / 2 - 0.4 * FT - a / 2), side * (halfW - 0.4 * FT - b / 2)];
    const room = box(t, s, a, b);
    if (room) {
      g.poly(room.map((v) => v + 1.6 * FT), 0x000000, 0.26);
      flatRoof(g, room, shadeHex(tint, 1.08));
    }
  }
  // The hatch: a timber frame round the dark stairwell, a ladder's rungs in it.
  const [ha, hb] = [3.2 * FT, 2.6 * FT];
  const ht = mid + end * (len / 2 - 0.5 * FT - ha / 2);
  const hs = side * (halfW - 0.5 * FT - hb / 2);
  const hatch = box(ht, hs, ha, hb);
  if (hatch) {
    g.poly(hatch, 0x8b6b4a);
    const hole = box(ht, hs, ha - 0.8 * FT, hb - 0.8 * FT, false)!;
    g.poly(hole, 0x2b231c);
    for (const f of [-0.25, 0.1]) {
      const [p, q] = [at(ht + f * ha, hs - 0.32 * hb), at(ht + f * ha, hs + 0.32 * hb)];
      g.line(p[0], p[1], q[0], q[1], 0.2 * FT, 0x9a7a55, 1);
    }
    g.outline(hole, INK_LINE * 0.7, ROOF_INK);
    g.outline(hatch, INK_LINE, ROOF_INK);
  }
  if (smoky || len < 10 * FT || halfW < 4 * FT) return;
  // Then one thing on the terrace, the first place it fits from the hatch's end on the far side.
  const kind = hh(3);
  const place = (a: number, b: number, inset: number): [number[], number, number] | null => {
    const s = -side * (halfW - inset - b / 2);
    for (let k = 0; k <= 8; k++) {
      const t = mid + end * (len / 2 - 0.5 * FT - a / 2) - end * ((len - a - FT) * k) / 8;
      const p = box(t, s, a, b);
      if (p) return [p, t, s];
    }
    return null;
  };
  if (kind < 0.4) {
    // A reed sunshade on four posts: its shadow, the mat in straw, the reeds across, lit to the NW.
    const [a, b] = [Math.min(12 * FT, len * 0.4), Math.min(9 * FT, halfW * 1.15)];
    const got = place(a, b, 0.6 * FT);
    if (!got) return;
    const [mat, t0, s0] = got;
    g.poly(mat.map((v) => v + 1.3 * FT), 0x000000, 0.24);
    g.poly(mat, 0xc2a36a);
    const lit = box(t0 - a * 0.2, s0 - b * 0.2, a * 0.6, b * 0.6, false);
    if (lit) g.poly(lit, 0xd0b47c);
    const n = Math.max(3, Math.round(a / (1.6 * FT)));
    for (let k = 1; k < n; k++) {
      const t = t0 - a / 2 + (a * k) / n;
      const [p, q] = [at(t, s0 - b / 2), at(t, s0 + b / 2)];
      g.line(p[0], p[1], q[0], q[1], 0.1 * FT, 0x9c7f4c, 1);
    }
    g.outline(mat, INK_LINE, ROOF_INK);
  } else if (kind < 0.65) {
    // A rug laid out to air: one earthy colour, its field a shade darker, a stripe down the middle.
    const RUGS = [0x9a4a3a, 0x46607a, 0xa8823e, 0x6e4a5e];
    const c = RUGS[Math.floor(hh(4) * RUGS.length)];
    const [a, b] = [Math.min(8 * FT, len * 0.36), Math.min(5 * FT, halfW * 0.8)];
    const got = place(a, b, 1.0 * FT);
    if (!got) return;
    const [rug, t0, s0] = got;
    g.poly(rug, c);
    g.poly(box(t0, s0, a - 1.2 * FT, b - 1.2 * FT, false) ?? rug, shadeHex(c, 0.86));
    const [p, q] = [at(t0 - a / 2 + 1.1 * FT, s0), at(t0 + a / 2 - 1.1 * FT, s0)];
    g.line(p[0], p[1], q[0], q[1], 0.45 * FT, shadeHex(c, 1.22), 1);
    g.outline(rug, INK_LINE, ROOF_INK);
  } else if (kind < 0.88) {
    // Water jars in a row against the parapet.
    const n = 2 + (hh(5) < 0.5 ? 1 : 0);
    const got = place(n * 2.5 * FT, 2.4 * FT, 0.5 * FT);
    if (!got) return;
    const [, t0, s0] = got;
    for (let k = 0; k < n; k++) {
      const [x, y] = at(t0 + (k - (n - 1) / 2) * 2.5 * FT, s0);
      const r = (1.0 + 0.12 * ((k + 1) % 2)) * FT;
      g.poly(circle(x + 0.45 * FT, y + 0.45 * FT, r, 14), 0x000000, 0.25);
      g.poly(circle(x, y, r, 14), 0xa8603e);
      g.poly(circle(x - 0.22 * r, y - 0.22 * r, r * 0.55, 12), 0xbc7450);
      g.poly(circle(x, y, r * 0.4, 12), 0x2b231c);
      g.outline(circle(x, y, r, 14), INK_LINE, ROOF_INK);
    }
  }
}

/** A circle as a polygon. */
function circle(cx: number, cy: number, r: number, n: number): number[] {
  const out: number[] = [];
  for (let k = 0; k < n; k++) out.push(cx + r * Math.cos((2 * Math.PI * k) / n), cy + r * Math.sin((2 * Math.PI * k) / n));
  return out;
}

/** The plaster of a dome's drum and the flat roof round it (whitewash keeps its own). */
const PLASTER = 0xb4a68b;

/** A dome of radius r about (cx, cy) on its drum: its shadow on the roof round it, the drum's ring and a
 * lip on it, the dome in sixteen gores rising to a crown pulled towards the NW light (so it stands up off
 * the roof), each gore in one of three flat tones by the way it faces (lit NW, shaded SE), a lit cap at
 * the crown, ribs between gores up to a finial and (not whitewash) two courses of tiles round it, inked
 * round. */
function dome(g: MeshBuilder, cx: number, cy: number, r: number, tint: number) {
  const n = 24;
  const white = g.mat === 11;
  g.poly(circle(cx + r * 0.3, cy + r * 0.3, r * 1.08, n), 0x000000, 0.32);
  const drum = circle(cx, cy, r * 1.14, n);
  g.poly(drum, white ? shadeHex(tint, 0.94) : PLASTER);
  g.outline(drum, INK_LINE, ROOF_INK);
  g.outline(circle(cx, cy, r * 1.07, n), 0.012 * PX, shadeHex(white ? tint : PLASTER, 0.8), 0.8);
  // (The crown: off the middle towards the light.)
  const nw = -Math.SQRT1_2;
  const [ax, ay] = [cx + nw * r * 0.22, cy + nw * r * 0.22];
  const gores = 16;
  const rim = (k: number): [number, number] => [cx + r * Math.cos((2 * Math.PI * k) / gores), cy + r * Math.sin((2 * Math.PI * k) / gores)];
  for (let k = 0; k < gores; k++) {
    const th = (2 * Math.PI * (k + 0.5)) / gores;
    const t = -(Math.cos(th) + Math.sin(th)) / Math.SQRT2;
    const c = shadeHex(tint, t > 0.45 ? 1.08 : t > -0.35 ? 0.98 : 0.87);
    const [p, q] = [rim(k), rim(k + 1)];
    g.tri(g.vert(p[0], p[1], c, 1), g.vert(q[0], q[1], c, 1), g.vert(ax, ay, c, 1));
  }
  g.poly(circle(ax + nw * r * 0.06, ay + nw * r * 0.06, r * 0.3, 12), shadeHex(tint, 1.14), 0.9);
  // (Courses of tiles round it, each ring nearer the crown as it rises.)
  const rib = shadeHex(tint, white ? 0.84 : 0.7);
  if (!white) {
    for (const f of [0.72, 0.44]) {
      const [ox, oy] = [cx + (ax - cx) * (1 - f), cy + (ay - cy) * (1 - f)];
      g.outline(circle(ox, oy, r * f, n), 0.016 * PX, rib, 0.75);
    }
  }
  for (let k = 0; k < gores; k += white ? 2 : 1) {
    const [p] = [rim(k)];
    g.line(p[0] + (ax - p[0]) * 0.03, p[1] + (ay - p[1]) * 0.03, ax + (p[0] - ax) * 0.12, ay + (p[1] - ay) * 0.12, (k % 2 ? 0.012 : 0.02) * PX, rib, white ? 0.65 : 0.85);
  }
  g.outline(circle(cx, cy, r, n), INK_OUT, ROOF_INK);
  g.poly(circle(ax, ay, Math.max(2, r * 0.09), 8), ROOF_INK);
  g.poly(circle(ax - r * 0.015, ay - r * 0.015, Math.max(1, r * 0.05), 8), 0xb8963e);
}

/** A domed roof: a flat roof behind its parapet round the drum, one dome over its widest middle (two
 * along a long one; none if it is too small for one), inside the footprint (`domeSpots`). */
function domeRoof(g: MeshBuilder, pts: number[], tint: number) {
  const code = g.code;
  const white = g.mat === 11;
  // (The flat roof round it plaster: flat clay's pattern, or whitewash's.)
  g.code = (white ? 11 : 8) + (code & ~15);
  flatRoof(g, pts, white ? tint : PLASTER);
  g.code = code;
  // (The drum's radius each: the dome's is a little less.)
  for (const [x, y, r] of domeSpots(pts, FT)) dome(g, x, y, r / 1.12, tint);
}

/** A round roof's eaves (`ROUND`, of one convex piece: `roundOutline`); else null. */
function roundOutline(roof: Roof): number[] | null {
  if (roof.shape !== ROUND || roof.cone || roof.battlements || roof.parts.length || !isConvex(roof.pts)) return null;
  return roundEaves(roof.pts);
}

/** A round roof's felt band (R5 towns, 115: `FELT_BAND`, by `Roof.shade`): none, or its colour. */
const FELT_BAND: (number | null)[] = [null, null, null, null, null, null, 0xa63e2c, null];

/** A low round roof (a yurt's, whatever its material: felt over the poles): a wedge per side in three
 * flat tones lit from the NW (shaded SE), faint seams along every other pole, two rope bands round it,
 * the crown's wooden ring with its cross over the smoke hole; unpatterned. */
function roundRoof(g: MeshBuilder, ring: number[], felt: number, band: number | null = null, sooty = false) {
  const n = ring.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + ring[i * 2] / n, cy + ring[i * 2 + 1] / n];
  let r = 0;
  for (let i = 0; i < n; i++) r += Math.hypot(ring[i * 2] - cx, ring[i * 2 + 1] - cy) / n;
  for (let i = 0; i < n; i++) {
    const [ax, ay, bx, by] = [ring[i * 2], ring[i * 2 + 1], ring[((i + 1) % n) * 2], ring[((i + 1) % n) * 2 + 1]];
    const [mx, my] = [(ax + bx) / 2 - cx, (ay + by) / 2 - cy];
    const t = -(mx + my) / (Math.hypot(mx, my) || 1) / Math.SQRT2;
    const c = shadeHex(felt, t > 0.2 ? 1.08 : t > -0.55 ? 0.98 : 0.84);
    g.tri(g.vert(ax, ay, c, 1), g.vert(bx, by, c, 1), g.vert(cx, cy, c, 1));
  }
  const rc = Math.max(r * 0.2, 0.9 * FT);
  const at = (i: number, f: number): [number, number] => [cx + (ring[(i % n) * 2] - cx) * f, cy + (ring[(i % n) * 2 + 1] - cy) * f];
  // (Seams down every other pole, faint: a darker felt.)
  const seam = shadeHex(felt, 0.86);
  for (let i = 0; i < n; i += 2) {
    const [ax, ay] = at(i, rc / r + 0.02);
    const [bx, by] = at(i, 0.97);
    g.line(ax, ay, bx, by, 0.02 * PX, seam, 1);
  }
  // (R5 towns, 115: a coloured band of felt between the ropes on some; soot round the crown on a
  // smoke-browned or old one.)
  if (band !== null) {
    for (let i = 0; i < n; i++) {
      const [a0, a1, b0, b1] = [at(i, 0.56), at(i + 1, 0.56), at(i + 1, 0.76), at(i, 0.76)];
      g.poly([...a0, ...a1, ...b0, ...b1], band);
    }
  }
  if (sooty) {
    g.poly(circle(cx, cy, rc * 2.6, 16), shadeHex(felt, 0.72));
    g.poly(circle(cx, cy, rc * 1.8, 16), shadeHex(felt, 0.55));
  }
  // (Two girth ropes, dark hemp, following the sides.)
  for (const f of [0.52, 0.8]) {
    const band: number[] = [];
    for (let i = 0; i < n; i++) band.push(...at(i, f));
    g.outline(band, 0.06 * PX, 0x4a3d31);
  }
  // The crown: a wooden ring, the smoke hole dark inside it, crossed by two bars.
  const wood = 0x8a6a48;
  g.poly(circle(cx, cy, rc, 16), wood);
  g.poly(circle(cx, cy, rc * 0.7, 16), 0x2e2822);
  const s = rc * 0.78;
  g.line(cx - s, cy, cx + s, cy, 0.03 * PX, wood, 1);
  g.line(cx, cy - s, cx, cy + s, 0.03 * PX, wood, 1);
  g.outline(circle(cx, cy, rc, 16), INK_LINE, ROOF_INK);
}

/** A low, nearly flat roof of felt or turf over a convex footprint (a steppe home's: R5 towns, 114): the felt
 * (turf) patterned whole, a soft rolled edge lit on its NW sides and shaded on its SE ones, a faint hump along
 * the ridge; felt held down by ropes across it weighted with stones at the eaves, turf tufted with grass;
 * a small smoke hole with its flap. */
function lowRoof(g: MeshBuilder, pts: number[], tint: number, turf: boolean) {
  const m = pts.length / 2;
  const { cx, cy, ux, uy, lo, hi, halfW, rx, ry } = frame(pts);
  const [ox, oy] = [pts[0], pts[1]];
  const uv = (x: number, y: number): [number, number] => [g.seed + ((x - ox) * ux + (y - oy) * uy) / FT, (-(x - ox) * uy + (y - oy) * ux) / FT];
  const w = Math.min(0.8 * FT, halfW * 0.28);
  const inner = inset(pts, w);
  g.patch(pts, uv, tint);
  if (inner) {
    const osg = signedArea(pts) > 0 ? 1 : -1;
    for (let i = 0; i < m; i++) {
      const j = (i + 1) % m;
      const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
      const l = Math.hypot(bx - ax, by - ay) || 1;
      const out = (osg * (by - ay) - osg * (bx - ax)) / l / Math.SQRT2;
      g.poly([ax, ay, bx, by, inner[j * 2], inner[j * 2 + 1], inner[i * 2], inner[i * 2 + 1]], shadeHex(tint, out < -0.3 ? 1.12 : out > 0.3 ? 0.8 : 0.94), 0.85);
    }
    // (The hump: the half toward the SE a shade darker.)
    const [nx, ny] = [-uy, ux];
    const lit = nx + ny < 0 ? 1 : -1;
    const [t0, t1] = [lo + w, hi - w];
    if (t1 > t0 && halfW > 2 * w) {
      const side = (s: number) => [rx + ux * t0 + nx * s, ry + uy * t0 + ny * s, rx + ux * t1 + nx * s, ry + uy * t1 + ny * s];
      const [a, b] = [side(0), side(-lit * (halfW - w))];
      g.poly([a[0], a[1], a[2], a[3], b[2], b[3], b[0], b[1]], 0x000000, 0.08);
      g.line(a[0], a[1], a[2], a[3], 0.03 * PX, shadeHex(tint, 0.82), 0.7);
    }
  }
  if (turf) {
    // (Grass tufts over it, a few bare patches.)
    let seed = (Math.round(cx * 3.1) * 73856093) ^ (Math.round(cy * 1.7) * 19349663);
    const rnd = () => {
      seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
      return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
    };
    const k = Math.min(14, Math.max(3, Math.round(((hi - lo) * halfW) / (12 * FT * FT))));
    for (let q = 0; q < k; q++) {
      const [a, b] = [lo + w + rnd() * Math.max(0, hi - lo - 2 * w), (rnd() * 2 - 1) * Math.max(0, halfW - w * 1.5)];
      const [x, y] = [rx + ux * a - uy * b, ry + uy * a + ux * b];
      const h = (0.5 + rnd() * 0.4) * FT;
      for (const dx of [-0.7, 0, 0.7]) g.line(x + dx * h * 0.25, y, x + dx * h * 0.7, y - h * (dx === 0 ? 1.1 : 0.8), 0.03 * PX, q % 2 ? 0x6f7f42 : 0x5d6e38, 1);
    }
  } else {
    // (Ropes across the felt, a stone weighting each end.)
    const [nx, ny] = [-uy, ux];
    const len = hi - lo;
    const count = len > 22 * FT ? 3 : len > 11 * FT ? 2 : 1;
    for (let q = 0; q < count; q++) {
      const t = lo + (len * (q + 1)) / (count + 1);
      const [px, py] = [rx + ux * t, ry + uy * t];
      const [c0, c1] = chord(pts, px, py, nx, ny, cx, cy, halfW * 4);
      if (c1 - c0 < 2 * FT) continue;
      const [ax, ay, bx, by] = [px + nx * (c0 + 0.3 * FT), py + ny * (c0 + 0.3 * FT), px + nx * (c1 - 0.3 * FT), py + ny * (c1 - 0.3 * FT)];
      g.line(ax, ay, bx, by, 0.06 * PX, 0x4a3d31, 1);
      for (const [sx, sy] of [
        [ax, ay],
        [bx, by],
      ]) {
        g.poly(circle(sx + 0.15 * FT, sy + 0.15 * FT, 0.55 * FT, 8), 0x000000, 0.25);
        g.poly(circle(sx, sy, 0.55 * FT, 8), 0x8f877a);
        g.outline(circle(sx, sy, 0.55 * FT, 8), INK_LINE, ROOF_INK);
      }
    }
  }
  // (A small smoke hole a little off the middle, its flap.)
  if (halfW > 3 * FT) {
    const [sx, sy] = [rx + ux * (lo + (hi - lo) * 0.42), ry + uy * (lo + (hi - lo) * 0.42)];
    const s = 0.8 * FT;
    const sq = [sx - ux * s + uy * s, sy - uy * s - ux * s, sx + ux * s + uy * s, sy + uy * s - ux * s, sx + ux * s - uy * s, sy + uy * s + ux * s, sx - ux * s - uy * s, sy - uy * s + ux * s];
    g.poly(sq, 0x2e2822);
    g.outline(sq, INK_LINE, ROOF_INK);
    g.poly([sq[0], sq[1], sq[2], sq[3], sx + uy * s * 0.2, sy - ux * s * 0.2], shadeHex(tint, 0.85));
  }
}

/** A round roof's middle radius. */
function ringRadius(ring: number[]): number {
  const n = ring.length / 2;
  let [cx, cy, r] = [0, 0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + ring[i * 2] / n, cy + ring[i * 2 + 1] / n];
  for (let i = 0; i < n; i++) r += Math.hypot(ring[i * 2] - cx, ring[i * 2 + 1] - cy) / n;
  return r;
}

/** Past this radius a round felt roof is a felt hall's (yurts are 7-12 ft across the middle; halls 15-35). */
const HALL_RING = 13 * FT;
/** A felt hall's felt (`Roof.shade` 0-4): the yurts' cream, off-white, warm cream, banded cream, dusty grey. */
const HALL_FELT = [YURT_FELT[0], YURT_FELT[1], YURT_FELT[3], YURT_FELT[6], YURT_FELT[4]];

/** A felt hall's roof (a temple's, a town hall's, an inn's: R5 towns, 114): a yurt's scaled up. Felt in wide
 * panels round it, lit from the NW; every roof pole showing through as a line from the crown to the eaves; a
 * darker felt skirt at the eaves with a madder band, tied down by ropes; two girth ropes; the crown a big
 * wooden ring with its spokes over the smoke hole, a felt flap folded half over it, soot round it. */
function hallRoof(g: MeshBuilder, ring: number[], felt: number) {
  const n = ring.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + ring[i * 2] / n, cy + ring[i * 2 + 1] / n];
  const r = ringRadius(ring);
  const per = Math.max(1, Math.round(n / 12));
  for (let i = 0; i < n; i++) {
    const [ax, ay, bx, by] = [ring[i * 2], ring[i * 2 + 1], ring[((i + 1) % n) * 2], ring[((i + 1) % n) * 2 + 1]];
    const [mx, my] = [(ax + bx) / 2 - cx, (ay + by) / 2 - cy];
    const t = -(mx + my) / (Math.hypot(mx, my) || 1) / Math.SQRT2;
    const panel = Math.floor(i / per) % 2 ? 0.93 : 1;
    const c = shadeHex(felt, (t > 0.2 ? 1.08 : t > -0.55 ? 0.98 : 0.84) * panel);
    g.tri(g.vert(ax, ay, c, 1), g.vert(bx, by, c, 1), g.vert(cx, cy, c, 1));
  }
  const rc = Math.max(r * 0.17, 2.2 * FT);
  const at = (i: number, f: number): [number, number] => [cx + (ring[(i % n) * 2] - cx) * f, cy + (ring[(i % n) * 2 + 1] - cy) * f];
  const ringAt = (f: number) => {
    const out: number[] = [];
    for (let i = 0; i < n; i++) out.push(...at(i, f));
    return out;
  };
  // (The skirt of darker felt at the eaves, a madder band along it.)
  for (let i = 0; i < n; i++) {
    g.poly([...at(i, 0.84), ...at(i + 1, 0.84), ...at(i + 1, 1), ...at(i, 1)], shadeHex(felt, 0.82));
    g.poly([...at(i, 0.88), ...at(i + 1, 0.88), ...at(i + 1, 0.94), ...at(i, 0.94)], 0x9a3d2c);
  }
  // (The poles under the felt, each a line from the crown to the eaves; seams between the panels darker.)
  const pole = shadeHex(felt, 0.72);
  for (let i = 0; i < n; i++) {
    const [ax, ay] = at(i, rc / r + 0.03);
    const [bx, by] = at(i, 0.84);
    g.line(ax, ay, bx, by, (i % per ? 0.035 : 0.06) * PX, i % per ? pole : shadeHex(felt, 0.6), 1);
  }
  // (Ropes down over the skirt at every panel's seam, and two girth ropes.)
  for (let i = 0; i < n; i += per) {
    const [ax, ay] = at(i, 0.8);
    const [bx, by] = at(i, 1.0);
    g.line(ax, ay, bx, by, 0.07 * PX, 0x4a3d31, 1);
  }
  for (const f of [0.44, 0.7, 0.84]) g.outline(ringAt(f), (f === 0.84 ? 0.05 : 0.08) * PX, 0x4a3d31);
  // The crown: soot round it, the wooden ring, its spokes over the dark smoke hole, a felt flap half over it.
  g.poly(circle(cx, cy, rc * 1.7, 20), shadeHex(felt, 0.78));
  const wood = 0x8a6a48;
  g.poly(circle(cx, cy, rc, 20), wood);
  g.poly(circle(cx, cy, rc * 0.74, 20), 0x2e2822);
  for (let k = 0; k < 8; k++) {
    const t = (k / 8) * Math.PI;
    const [dx, dy] = [Math.cos(t) * rc * 0.8, Math.sin(t) * rc * 0.8];
    g.line(cx - dx, cy - dy, cx + dx, cy + dy, 0.04 * PX, wood, 1);
  }
  const flap: number[] = [];
  for (let k = 0; k <= 10; k++) {
    const t = Math.PI * 0.25 + (k / 10) * Math.PI;
    flap.push(cx + Math.cos(t) * rc * 0.98, cy + Math.sin(t) * rc * 0.98);
  }
  g.poly(flap, shadeHex(felt, 0.9));
  g.outline(flap, INK_LINE, ROOF_INK);
  g.outline(circle(cx, cy, rc, 20), INK_OUT, ROOF_INK);
}

/** A yurt's door on the ring's first side (`town::round_off` turns the ring so that side faces the nearest
 * lane or the camp's middle): a small painted panel under a felt hood, through the rim, inked; drawn over
 * the eaves' ink. */
function yurtDoor(g: MeshBuilder, ring: number[], felt: number) {
  const n = ring.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + ring[i * 2] / n, cy + ring[i * 2 + 1] / n];
  const [ex, ey] = [(ring[0] + ring[2]) / 2, (ring[1] + ring[3]) / 2];
  const l = Math.hypot(ex - cx, ey - cy) || 1;
  // (Outward, and along the rim.)
  const [ox, oy] = [(ex - cx) / l, (ey - cy) / l];
  const [tx, ty] = [-oy, ox];
  // (R5 towns: a felt hall's door, on a ring past a yurt's 15 ft, the bigger with it: double at 35 ft.)
  // (R5 towns, 114: a felt hall's grows faster, as wide as a cart at 40 ft across, with a painted frame.)
  const hall = l > HALL_RING;
  const k = hall ? Math.min(2.6, 1.3 + (l - HALL_RING) / (9 * FT)) : 1 + Math.max(0, l - 15 * FT) / (20 * FT);
  const [w, out, ins] = [1.6 * FT * k, 1.1 * FT * k, 1.4 * FT * k];
  const at = (a: number, b: number) => [ex + tx * a + ox * b, ey + ty * a + oy * b];
  const hood = [...at(-w, -ins), ...at(w, -ins), ...at(w, out), ...at(-w, out)];
  if (hall) g.poly([...at(-w, -ins + 0.6 * FT), ...at(w + 0.6 * FT, -ins + 0.6 * FT), ...at(w + 0.6 * FT, out + 0.6 * FT), ...at(-w, out + 0.6 * FT)], 0x000000, 0.25);
  g.poly(hood, shadeHex(felt, 0.9));
  if (hall) g.poly([...at(-w * 0.72, -ins * 0.3), ...at(w * 0.72, -ins * 0.3), ...at(w * 0.72, out), ...at(-w * 0.72, out)], 0xc49a3a);
  g.poly([...at(-w * 0.6, -ins * 0.2), ...at(w * 0.6, -ins * 0.2), ...at(w * 0.6, out), ...at(-w * 0.6, out)], 0x8a4a32);
  if (hall) g.line(...(at(0, -ins * 0.2) as [number, number]), ...(at(0, out) as [number, number]), 0.04 * PX, 0x4a2a1a, 1);
  g.outline(hood, hall ? INK_OUT : INK_LINE, ROOF_INK);
}

/** A smithy's (kiln's, still's) chimney on its ridge `at` of the way along, and the soot round it. */
function smoke(g: MeshBuilder, pts: number[], at: number) {
  const { ux, uy, lo, hi, halfW, rx, ry } = frame(pts);
  const t = lo + (hi - lo) * at;
  const [x, y] = [rx + ux * t, ry + uy * t];
  const r = Math.min(4.5 * FT, halfW * 1.2 + FT);
  const [sx, sy] = [x + 0.5 * FT, y + 0.5 * FT];
  const soot = circle(sx, sy, r, 12);
  const uvd: number[] = [];
  for (let i = 0; i < soot.length; i += 2) uvd.push((soot[i] - sx) / r, (soot[i + 1] - sy) / r, 1);
  g.face(soot, uvd, 0x221e1b, 1, SOOT);
  const sq = (h: number, dx = 0) => [x - ux * h + uy * h + dx, y - uy * h - ux * h + dx, x + ux * h + uy * h + dx, y + uy * h - ux * h + dx, x + ux * h - uy * h + dx, y + uy * h + ux * h + dx, x - ux * h - uy * h + dx, y - uy * h + ux * h + dx];
  g.poly(sq(0.9 * FT, 0.6 * FT), 0x000000, 0.3);
  g.poly(sq(0.9 * FT), 0x8f877a);
  g.outline(sq(0.9 * FT), INK_LINE, ROOF_INK);
  g.poly(sq(0.45 * FT), 0x1e1a16);
}

/** Draw with another material's pattern (a slate spire on a tiled temple). */
function withMat(g: MeshBuilder, mat: number, f: () => void) {
  const [m, c] = [g.mat, g.code];
  [g.mat, g.code] = [mat, mat + (c >= 0 ? c & ~15 : 0)];
  f();
  [g.mat, g.code] = [m, c];
}
