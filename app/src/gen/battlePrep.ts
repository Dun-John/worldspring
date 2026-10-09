// Battlemap chunk preparation, run in the generator workers: decode the payload, build the
// GPU-ready texture arrays, sort the objects and tessellate the pitched roofs into one
// vertex-coloured mesh, so the main thread only creates GPU objects (no parsing, no
// Graphics tessellation in a frame).
import earcut from 'earcut';
import { BLOCKS, DIFFICULT, OPAQUE } from '../play/vision';

export const SQ = 128;
export const HS = SQ + 2;
/** Texture pixels per square (matches `atlas.PX`). */
export const PX = 64;

export interface Roof {
  pts: number[];
  floors: number;
  /** A flat walkable roof with a crenellated parapet (keeps, towers). */
  battlements: boolean;
  /** Corner-tower side (px) of a keep's battlements; 0 for none. */
  tower: number;
  /** Sloping up from every side to the middle (round towers). */
  cone: boolean;
  /** Index into `ROOF_TINTS` chosen by hand; -1: picked. */
  tint: number;
  /** A hash of where it stands (for the tint). */
  idx: number;
}

export interface Shape {
  kind: number;
  size: number;
  pts: number[];
}

export interface ColorMesh {
  pos: Float32Array;
  color: Float32Array;
  index: Uint32Array;
}

/** What play mode needs to know of an object kind (`battlemap::KindInfo`). */
export interface KindTactics {
  height_ft: number;
  radius: number;
  blocks_sight: boolean;
  blocks_move: boolean;
  difficult: boolean;
}

/** Packed object kinds from here up are the chunk's uploaded sprites (`battlemap::SPRITE_KIND`). */
export const SPRITE_KIND = 1000;

/** An uploaded sprite a chunk uses: its asset id and tactical rules. */
export interface ChunkSprite extends KindTactics {
  asset: string;
  cover: number;
}

/** The rules of an object kind: built-in (`kinds[kind - 1]`) or one of the chunk's sprites. */
export function kindRules<T extends KindTactics>(kind: number, kinds: T[], sprites: ChunkSprite[]): T | ChunkSprite | undefined {
  return kind >= SPRITE_KIND ? sprites[kind - SPRITE_KIND] : kinds[kind - 1];
}

export interface PreparedChunk {
  /** Per square: play mode's flags (`play/vision`: what blocks sight and movement, difficult
   * ground), worked out here rather than when sight is first worked out. */
  flags: Uint8Array;
  nObj: number;
  atmosphere: number;
  sea: number;
  /** Height offset subtracted before the half-float encoding. */
  base: number;
  /** Ground data texture (surface, edges, tier, building id) and heights texture (half floats). */
  data: Uint8Array;
  hdata: Uint16Array;
  heights: Float32Array;
  tier: Int16Array;
  surface: Uint8Array;
  raised: Uint8Array | null;
  /** Objects sorted for drawing (short first, then by y): kind, variant, x, y, rot, scale. */
  objs: Float32Array;
  /** Drop shadows of every roof, then the pitched roofs. */
  roofMesh: ColorMesh | null;
  /** Flat battlemented roofs (few; drawn as Graphics over the mesh). */
  battlements: Roof[];
  shapes: Shape[];
  /** Uploaded sprites (object kinds `SPRITE_KIND` + index). */
  sprites: ChunkSprite[];
}

// Roof tints: terracotta, slate, thatch, weathered shingle, moss.
export const ROOF_TINTS = [0xa0543c, 0x5c6470, 0xa88e56, 0x7a6656, 0x6e6a48];
export const ROOF_INK = 0x1d1a14;

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

/** f32 → IEEE half bits (round to nearest). */
const f32 = new Float32Array(1);
const u32 = new Uint32Array(f32.buffer);
function toHalf(v: number): number {
  f32[0] = v;
  const x = u32[0];
  const sign = (x >>> 16) & 0x8000;
  const e = ((x >>> 23) & 0xff) - 127 + 15;
  let m = x & 0x7fffff;
  if (e <= 0) {
    if (e < -10) return sign;
    m = (m | 0x800000) >>> (1 - e);
    return sign | ((m + 0x1000) >>> 13);
  }
  if (e >= 31) return sign | 0x7c00;
  const h = sign | (e << 10) | (m >>> 13);
  return m & 0x1000 ? h + 1 : h;
}

/** Decode a `battlemap::pack` payload (`kinds[kind - 1]`: the object catalog). */
export function prepareChunk(buf: ArrayBuffer, cx: number, cy: number, kinds: KindTactics[]): PreparedChunk {
  const n = SQ * SQ;
  const nh = HS * HS;
  const dv = new DataView(buf);
  const nObj = dv.getUint32(16, true);
  const atmosphere = dv.getUint32(20, true);
  const sea = dv.getFloat32(24, true);
  const heights = new Float32Array(buf, 32, nh);
  const tier = new Int16Array(buf, 32 + nh * 4, n);
  const surface = new Uint8Array(buf, 32 + nh * 4 + n * 2, n);
  const edges = new Uint8Array(buf, 32 + nh * 4 + n * 3, n);
  const water = new Float32Array(buf, 32 + nh * 4 + n * 4, nh);
  const objOffset = 32 + nh * 8 + n * 4;

  let min = Infinity;
  let max = -Infinity;
  for (let k = 0; k < nh; k++) {
    if (heights[k] < min) min = heights[k];
    if (heights[k] > max) max = heights[k];
  }
  const base = (min + max) / 2;
  // Road halo → filtered channels: b = road coverage, a = kind × coverage (0 earth,
  // 0.5 cobbles, 1 bridge deck), so kind = a / b is the mean over nearby road squares.
  const roadH = new Uint8Array(buf, objOffset + nObj * 20, nh);
  const building = new Uint16Array(buf.slice(objOffset + nObj * 20 + nh, objOffset + nObj * 20 + nh + n * 2));
  const roofs: Roof[] = [];
  const shapes: Shape[] = [];
  const sprites: ChunkSprite[] = [];
  let raised: Uint8Array | null = null;
  {
    let o = objOffset + nObj * 20 + nh + n * 2;
    const count = dv.getUint32(o, true);
    o += 4;
    for (let b = 0; b < count; b++) {
      const m = dv.getUint8(o);
      const floors = dv.getUint8(o + 1);
      // Style 0 hip, 1 battlements, 2 cone | (tint + 1) << 2 (`battlemap::roof_byte`).
      const roof = dv.getUint8(o + 2);
      const battlements = (roof & 3) === 1;
      const cone = (roof & 3) === 2;
      const tint = (roof >> 2) - 1;
      const tower = dv.getUint8(o + 3) * PX;
      o += 4;
      const pts: number[] = [];
      // (Its first corner on the map, in quarter squares: the same in every chunk it is in,
      // and whatever else is built or taken away round it.)
      const at = m ? Math.imul(Math.round((cx * SQ + dv.getFloat32(o, true)) * 4), 73856093) ^ Math.imul(Math.round((cy * SQ + dv.getFloat32(o + 4, true)) * 4), 19349663) : b;
      for (let v = 0; v < m; v++) {
        pts.push(dv.getFloat32(o, true) * PX, dv.getFloat32(o + 4, true) * PX);
        o += 8;
      }
      roofs.push({ pts, floors, battlements, tower, cone, tint, idx: at });
    }
    // Walls, towers and decks, drawn as vectors (see `battlemap::pack`).
    const nShapes = o + 4 <= buf.byteLength ? dv.getUint32(o, true) : 0;
    o += 4;
    for (let k = 0; k < nShapes; k++) {
      const kind = dv.getUint8(o);
      const m = dv.getUint8(o + 1);
      const size = dv.getFloat32(o + 4, true) * PX;
      o += 8;
      const pts: number[] = [];
      for (let v = 0; v < m; v++) {
        pts.push(dv.getFloat32(o, true) * PX, dv.getFloat32(o + 4, true) * PX);
        o += 8;
      }
      shapes.push({ kind, size, pts });
    }
    if (o + n * 2 <= buf.byteLength) raised = new Uint8Array(buf.slice(o, o + n * 2));
    o += n * 2;
    // Uploaded sprites (see `battlemap::pack`).
    const nSprites = o + 4 <= buf.byteLength ? dv.getUint32(o, true) : 0;
    o += 4;
    for (let k = 0; k < nSprites; k++) {
      const len = dv.getUint8(o);
      const cover = dv.getUint8(o + 1);
      const flags = dv.getUint8(o + 2);
      const radius = dv.getFloat32(o + 4, true);
      const height_ft = dv.getFloat32(o + 8, true);
      const asset = String.fromCharCode(...new Uint8Array(buf, o + 12, len));
      o += 12 + len;
      sprites.push({ asset, cover, radius, height_ft, blocks_move: !!(flags & 1), blocks_sight: !!(flags & 2), difficult: !!(flags & 4) });
    }
  }
  const hdata = new Uint16Array(nh * 4);
  for (let k = 0; k < nh; k++) {
    hdata[k * 4] = toHalf(heights[k] - base);
    hdata[k * 4 + 1] = toHalf(Math.max(water[k], base - 60000) - base);
    const r = roadH[k];
    hdata[k * 4 + 2] = r ? 0x3c00 : 0; // 1.0 / 0.0
    hdata[k * 4 + 3] = r & 0x80 ? 0x3c00 : (r & 0x7f) === 1 ? 0x3800 : 0; // 1.0 / 0.5 / 0.0
  }
  const data = new Uint8Array(n * 4);
  for (let k = 0; k < n; k++) {
    data[k * 4] = surface[k];
    data[k * 4 + 1] = edges[k];
    data[k * 4 + 2] = tier[k] & 255;
    // Building id (1..255, wrapping) for per-building roof colour and wall edges.
    data[k * 4 + 3] = building[k] ? 1 + ((building[k] - 1) % 255) : 0;
  }

  // Objects: short things first, then by y (canopies overlap downward).
  const order: number[] = [];
  for (let i = 0; i < nObj; i++) order.push(i);
  const kindAt = (i: number) => dv.getUint16(objOffset + i * 20, true);
  const yAt = (i: number) => dv.getFloat32(objOffset + i * 20 + 8, true);
  const h = (i: number) => kindRules(kindAt(i), kinds, sprites)?.height_ft ?? 0;
  order.sort((a, b) => h(a) - h(b) || yAt(a) - yAt(b));
  const objs = new Float32Array(nObj * 6);
  order.forEach((i, j) => {
    const o = objOffset + i * 20;
    objs[j * 6] = dv.getUint16(o, true);
    objs[j * 6 + 1] = dv.getUint8(o + 2);
    objs[j * 6 + 2] = dv.getFloat32(o + 4, true);
    objs[j * 6 + 3] = dv.getFloat32(o + 8, true);
    objs[j * 6 + 4] = dv.getFloat32(o + 12, true);
    objs[j * 6 + 5] = dv.getFloat32(o + 16, true);
  });

  return {
    flags: tacticalFlags(data, surface, raised, objs, kinds, sprites),
    nObj,
    atmosphere,
    sea,
    base,
    data,
    hdata,
    heights: heights.slice(),
    tier: tier.slice(),
    surface: surface.slice(),
    raised,
    objs,
    roofMesh: roofs.length ? roofMesh(roofs) : null,
    battlements: roofs.filter((r) => r.battlements),
    shapes,
    sprites,
  };
}

/**
 * Per square: what blocks sight and movement and what is difficult ground (`play/vision`
 * flags): buildings, curtain walls and towers; water and lava underfoot; objects (trunks and
 * rocks where they stand, not a tree's whole canopy; brush and hazards over their spread).
 */
function tacticalFlags(data: Uint8Array, surface: Uint8Array, raised: Uint8Array | null, objs: Float32Array, kinds: KindTactics[], sprites: ChunkSprite[]): Uint8Array {
  const f = new Uint8Array(SQ * SQ);
  for (let k = 0; k < SQ * SQ; k++) {
    let v = 0;
    if (data[k * 4 + 1] & 16 || (surface[k] >= 10 && surface[k] <= 12)) v |= DIFFICULT;
    if (data[k * 4 + 3] || (raised && raised[k * 2] > 0 && (raised[k * 2 + 1] === 7 || raised[k * 2 + 1] === 8))) v |= OPAQUE | BLOCKS;
    f[k] = v;
  }
  for (let o = 0; o < objs.length; o += 6) {
    const inf = kindRules(objs[o], kinds, sprites);
    if (!inf) continue;
    const v = (inf.blocks_sight ? OPAQUE : 0) | (inf.blocks_move ? BLOCKS : 0) | (inf.difficult ? DIFFICULT : 0);
    if (!v) continue;
    const [x, y, scale] = [objs[o + 2], objs[o + 3], objs[o + 5]];
    const reach = Math.max(0.5, inf.radius * scale * (v & (OPAQUE | BLOCKS) ? 0.5 : 0.8));
    for (let j = Math.max(0, Math.floor(y - reach)); j <= Math.min(SQ - 1, Math.floor(y + reach)); j++) {
      for (let i = Math.max(0, Math.floor(x - reach)); i <= Math.min(SQ - 1, Math.floor(x + reach)); i++) {
        if ((i + 0.5 - x) ** 2 + (j + 0.5 - y) ** 2 <= reach * reach) f[j * SQ + i] |= v;
      }
    }
  }
  return f;
}

/** The typed-array buffers of a prepared chunk (to transfer it between threads). */
export function chunkTransfer(c: PreparedChunk): Transferable[] {
  const t: Transferable[] = [c.flags.buffer, c.data.buffer, c.hdata.buffer, c.heights.buffer, c.tier.buffer, c.surface.buffer, c.objs.buffer];
  if (c.raised) t.push(c.raised.buffer);
  if (c.roofMesh) t.push(c.roofMesh.pos.buffer, c.roofMesh.color.buffer, c.roofMesh.index.buffer);
  return t;
}

/** Triangles with a per-vertex straight-alpha colour. */
class MeshBuilder {
  pos: number[] = [];
  color: number[] = [];
  index: number[] = [];

  private vert(x: number, y: number, c: number, a: number) {
    this.pos.push(x, y);
    this.color.push(((c >> 16) & 255) / 255, ((c >> 8) & 255) / 255, (c & 255) / 255, a);
  }

  poly(pts: number[], c: number, a = 1) {
    const tri = earcut(pts);
    if (!tri.length) return;
    const v0 = this.pos.length / 2;
    for (let i = 0; i < pts.length; i += 2) this.vert(pts[i], pts[i + 1], c, a);
    for (const t of tri) this.index.push(v0 + t);
  }

  /** A butt-capped line `w` wide, extended by `ext` past each end. */
  line(ax: number, ay: number, bx: number, by: number, w: number, c: number, a: number, ext = 0) {
    const l = Math.hypot(bx - ax, by - ay);
    if (l < 1e-6) return;
    const [ux, uy] = [(bx - ax) / l, (by - ay) / l];
    const [nx, ny] = [(-uy * w) / 2, (ux * w) / 2];
    const [sx, sy, ex, ey] = [ax - ux * ext, ay - uy * ext, bx + ux * ext, by + uy * ext];
    const v0 = this.pos.length / 2;
    this.vert(sx + nx, sy + ny, c, a);
    this.vert(ex + nx, ey + ny, c, a);
    this.vert(ex - nx, ey - ny, c, a);
    this.vert(sx - nx, sy - ny, c, a);
    this.index.push(v0, v0 + 1, v0 + 2, v0, v0 + 2, v0 + 3);
  }

  /** A closed outline (square joins). */
  outline(pts: number[], w: number, c: number, a = 1) {
    const m = pts.length / 2;
    for (let i = 0; i < m; i++) this.line(pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1], w, c, a, w / 2);
  }

  build(): ColorMesh {
    return { pos: new Float32Array(this.pos), color: new Float32Array(this.color), index: new Uint32Array(this.index) };
  }
}

/** Hip roofs: shaded slopes, shingle courses, ridge and hips, an inked eave; drop shadows of
 * every roof (battlemented ones included) first. */
function roofMesh(roofs: Roof[]): ColorMesh {
  const g = new MeshBuilder();
  for (const { pts, floors } of roofs) {
    const off = PX * (0.25 + 0.3 * floors);
    g.poly(
      pts.map((v) => v + off),
      0x000000,
      0.28,
    );
  }
  for (const roof of roofs) {
    if (roof.battlements || roof.pts.length < 6) continue;
    const tint = ROOF_TINTS[roof.tint >= 0 && roof.tint < ROOF_TINTS.length ? roof.tint : (Math.imul(roof.idx, 2654435761) >>> 16) % ROOF_TINTS.length];
    // Footprints drawn by hand may be concave: a roof on each convex part, valleys between.
    const parts = convexParts(roof.pts);
    for (const part of parts) {
      if (roof.cone) coneRoof(g, part, tint);
      else hipRoof(g, part, tint);
    }
    if (parts.length > 1) for (const part of parts) g.outline(part, 1.6, shadeHex(tint, 0.5), 0.8);
    g.outline(roof.pts, 2.5, ROOF_INK);
  }
  return g.build();
}

/** Sloping up from every side to the middle: each face shaded by how it faces the NW light,
 * shingle courses round it, hips to the apex and a finial. */
function coneRoof(g: MeshBuilder, pts: number[], tint: number) {
  const m = pts.length / 2;
  let [cx, cy] = [0, 0];
  for (let i = 0; i < m; i++) {
    cx += pts[i * 2] / m;
    cy += pts[i * 2 + 1] / m;
  }
  const ccw = signedArea(pts) > 0;
  for (let i = 0; i < m; i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const l = Math.hypot(bx - ax, by - ay) || 1;
    // Outward normal; the light comes from the NW (-x, -y).
    let [nx, ny] = [(by - ay) / l, -(bx - ax) / l];
    if (ccw) [nx, ny] = [-nx, -ny];
    const lit = (-(nx + ny) / Math.SQRT2 + 1) / 2;
    g.poly([ax, ay, bx, by, cx, cy], shadeHex(tint, 0.7 + 0.5 * lit));
  }
  const course = shadeHex(tint, 0.6);
  for (let k = 1; k < 5; k++) {
    const t = k / 5;
    const ring: number[] = [];
    for (let i = 0; i < m; i++) ring.push(pts[i * 2] + (cx - pts[i * 2]) * t, pts[i * 2 + 1] + (cy - pts[i * 2 + 1]) * t);
    for (let i = 0; i < m; i++) g.line(ring[i * 2], ring[i * 2 + 1], ring[((i + 1) % m) * 2], ring[((i + 1) % m) * 2 + 1], 1, course, 0.35);
  }
  const ridge = shadeHex(tint, 0.5);
  const every = m > 12 ? 2 : 1;
  for (let i = 0; i < m; i += every) g.line(pts[i * 2], pts[i * 2 + 1], cx, cy, 1.6, ridge, 0.8);
  const r = PX * 0.12;
  const finial: number[] = [];
  for (let k = 0; k < 8; k++) finial.push(cx + r * Math.cos((k * Math.PI) / 4), cy + r * Math.sin((k * Math.PI) / 4));
  g.poly(finial, ROOF_INK);
}

function signedArea(pts: number[]): number {
  let a = 0;
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) a += pts[i * 2] * pts[((i + 1) % m) * 2 + 1] - pts[((i + 1) % m) * 2] * pts[i * 2 + 1];
  return a / 2;
}

/** Whether a polygon turns one way only (collinear corners allowed). */
function isConvex(pts: number[]): boolean {
  const m = pts.length / 2;
  let sign = 0;
  for (let i = 0; i < m; i++) {
    const [ax, ay] = [pts[i * 2], pts[i * 2 + 1]];
    const [bx, by] = [pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]];
    const [qx, qy] = [pts[((i + 2) % m) * 2], pts[((i + 2) % m) * 2 + 1]];
    const c = (bx - ax) * (qy - by) - (by - ay) * (qx - bx);
    if (Math.abs(c) < 1e-6) continue;
    if (sign && Math.sign(c) !== sign) return false;
    sign = Math.sign(c);
  }
  return true;
}

/** A simple polygon as convex parts (Hertel–Mehlhorn: its triangles, merged across shared
 * edges while the union stays convex). */
export function convexParts(pts: number[]): number[][] {
  if (isConvex(pts)) return [pts];
  const tri = earcut(pts);
  const ccw = signedArea(pts) > 0;
  const at = (ids: number[]) => ids.flatMap((k) => [pts[k * 2], pts[k * 2 + 1]]);
  // Every part turned the polygon's way, so a shared edge runs opposite ways in the two.
  let parts: number[][] = [];
  for (let t = 0; t < tri.length; t += 3) {
    const ids = [tri[t], tri[t + 1], tri[t + 2]];
    if (signedArea(at(ids)) > 0 !== ccw) ids.reverse();
    parts.push(ids);
  }
  const merge = (a: number[], b: number[]): number[] | null => {
    for (let i = 0; i < a.length; i++) {
      const [p, q] = [a[i], a[(i + 1) % a.length]];
      const j = b.indexOf(q);
      if (j < 0 || b[(j + 1) % b.length] !== p) continue;
      const out: number[] = [];
      for (let k = 0; k < a.length; k++) out.push(a[(i + 1 + k) % a.length]);
      for (let k = 2; k < b.length; k++) out.push(b[(j + k) % b.length]);
      return isConvex(at(out)) ? out : null;
    }
    return null;
  };
  for (let merged = true; merged; ) {
    merged = false;
    for (let i = 0; i < parts.length && !merged; i++) {
      for (let j = i + 1; j < parts.length && !merged; j++) {
        const u = merge(parts[i], parts[j]);
        if (u) {
          parts[i] = u;
          parts.splice(j, 1);
          merged = true;
        }
      }
    }
  }
  return parts.map(at);
}

/** A hip roof on a convex footprint: shaded slopes, shingle courses, ridge and hips. */
function hipRoof(g: MeshBuilder, pts: number[], tint: number) {
  const m = pts.length / 2;
  let cx = 0;
  let cy = 0;
  for (let i = 0; i < m; i++) {
    cx += pts[i * 2];
    cy += pts[i * 2 + 1];
  }
  cx /= m;
  cy /= m;
  // Ridge along the longest edge's direction.
  let ux = 1;
  let uy = 0;
  let best = 0;
  for (let i = 0; i < m; i++) {
    const dx = pts[((i + 1) % m) * 2] - pts[i * 2];
    const dy = pts[((i + 1) % m) * 2 + 1] - pts[i * 2 + 1];
    const l = Math.hypot(dx, dy);
    if (l > best) {
      best = l;
      ux = dx / l;
      uy = dy / l;
    }
  }
  let [lo, hi, plo, phi] = [Infinity, -Infinity, Infinity, -Infinity];
  for (let i = 0; i < m; i++) {
    const a = (pts[i * 2] - cx) * ux + (pts[i * 2 + 1] - cy) * uy;
    const b = -(pts[i * 2] - cx) * uy + (pts[i * 2 + 1] - cy) * ux;
    lo = Math.min(lo, a);
    hi = Math.max(hi, a);
    plo = Math.min(plo, b);
    phi = Math.max(phi, b);
  }
  const halfW = (phi - plo) / 2;
  const mid = (plo + phi) / 2;
  const r0 = lo + halfW;
  const r1 = Math.max(r0, hi - halfW);
  // Ridge centre line (offset to the middle of the perpendicular extent).
  const rx = cx - uy * mid;
  const ry = cy + ux * mid;
  g.poly(pts, tint);
  // The slope facing away from the NW light is darker.
  let nx = -uy;
  let ny = ux;
  if (nx + ny < 0) {
    nx = -nx;
    ny = -ny;
  }
  const dark = clipHalf(pts, rx, ry, -nx, -ny);
  if (dark.length >= 6) g.poly(dark, shadeHex(tint, 0.72));
  const lit = clipHalf(pts, rx, ry, nx, ny);
  if (lit.length >= 6) g.poly(lit, shadeHex(tint, 1.18), 0.6);
  // Shingle courses parallel to the ridge, clipped to the (convex) outline.
  const course = shadeHex(tint, 0.6);
  for (let k = 1; k < 6; k++) {
    const d = (halfW * k) / 6;
    for (const sgn of [-1, 1]) {
      const ox = rx + nx * d * sgn;
      const oy = ry + ny * d * sgn;
      let t0 = -(hi - lo);
      let t1 = hi - lo;
      for (let i = 0; i < m && t0 < t1; i++) {
        const ax = pts[i * 2];
        const ay = pts[i * 2 + 1];
        const ex = pts[((i + 1) % m) * 2] - ax;
        const ey = pts[((i + 1) % m) * 2 + 1] - ay;
        // Inward normal: towards the centroid.
        let inx = -ey;
        let iny = ex;
        if ((cx - ax) * inx + (cy - ay) * iny < 0) {
          inx = -inx;
          iny = -iny;
        }
        const c0 = (ox - ax) * inx + (oy - ay) * iny;
        const c1 = ux * inx + uy * iny;
        if (Math.abs(c1) < 1e-9) {
          if (c0 < 0) t1 = t0;
        } else if (c1 > 0) t0 = Math.max(t0, -c0 / c1);
        else t1 = Math.min(t1, -c0 / c1);
      }
      if (t1 - t0 > 2) g.line(ox + ux * t0, oy + uy * t0, ox + ux * t1, oy + uy * t1, 1, course, 0.35);
    }
  }
  // Ridge and hips.
  const ridge = shadeHex(tint, 0.5);
  const ax = rx + ux * r0;
  const ay = ry + uy * r0;
  const bx = rx + ux * r1;
  const by = ry + uy * r1;
  g.line(ax, ay, bx, by, 1.6, ridge, 0.8);
  for (let i = 0; i < m; i++) {
    const px = pts[i * 2];
    const py = pts[i * 2 + 1];
    const along = (px - cx) * ux + (py - cy) * uy;
    const [ex, ey] = along < (r0 + r1) / 2 ? [ax, ay] : [bx, by];
    g.line(ex, ey, px, py, 1.6, ridge, 0.8);
  }
}
