// Battlemap chunk preparation, run in the generator workers: decode the payload, build the
// GPU-ready texture arrays, sort the objects and tessellate the pitched roofs into one
// vertex-coloured mesh, so the main thread only creates GPU objects (no parsing, no
// Graphics tessellation in a frame).
import earcut from 'earcut';
import { BLOCKS, DIFFICULT, OPAQUE } from '../play/vision';
import { roofRgb, ROOF_MATERIAL_RGB } from './protocol';
import { domeSpots, frame, isConvex, ridgeEnds, roofParts, roundOutline as roundEaves, signedArea, straighten } from './roofGeom';

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
  /** A castle's keep with no pieces: how its top is drawn (as a piece's `top`); else 0. */
  top: number;
  /** Sloping up from every side to the middle (round towers). */
  cone: boolean;
  /** Index into `ROOF_TINTS` chosen by hand; -1: picked. */
  tint: number;
  /** Its material and shape (`ROOF_MATERIALS`, `ROOF_SHAPES` ids: `town::roof`), smoke-darkened
   * (smithies, kilns), weathering 0-255. */
  material: number;
  shape: number;
  smoke: boolean;
  wear: number;
  /** Colour variant 0-4 of the material (`ROOF_SHADES`). */
  shade: number;
  /** A hash of where it stands. */
  idx: number;
  /** A keep's pieces (`battlemap::part_code`: 0 body, 1 round tower, 2 square tower, 3
   * forebuilding, 4 wing), how each is topped (0 open battlements, 1 a lead roof, 2 slate:
   * `town::keep_tops`), px; a palace's or a monastery's court (5, `COURT_PART`: its corners each by the
   * footprint's in the same place) and a monastery's church (9, `CHURCH_PART`); a manor's main range and
   * cross wing (7, 6); a temple's body, transept and apse (0, 4, 8: `APSE_PART`); empty for any other
   * building. */
  parts: { kind: number; top: number; pts: number[] }[];
}

export interface Shape {
  kind: number;
  size: number;
  pts: number[];
}

export interface ColorMesh {
  pos: Float32Array;
  color: Float32Array;
  /** Per vertex: the roof pattern's coordinates and code (`MeshBuilder.face`): u × d, v, code, d. */
  roof: Float32Array;
  index: Uint32Array;
}

/** What play mode needs to know of an object kind (`battlemap::KindInfo`). */
export interface KindTactics {
  /** Built-in kinds' catalog id and name. */
  id?: number;
  name?: string;
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

/** A built-in kind's entry in the object catalog by id: `kinds[id - 1]` for the first ones, else
 * looked up (later kinds take ids from ranges with gaps between them, `battlemap::MORE`). */
export function builtinKind<T extends { id?: number }>(kinds: T[], id: number): T | undefined {
  const k = kinds[id - 1];
  return k?.id === id ? k : kinds.find((i) => i.id === id);
}

/** The rules of an object kind: built-in (`builtinKind`) or one of the chunk's sprites. */
export function kindRules<T extends KindTactics>(kind: number, kinds: T[], sprites: ChunkSprite[]): T | ChunkSprite | undefined {
  return kind >= SPRITE_KIND ? sprites[kind - SPRITE_KIND] : builtinKind(kinds, kind);
}

export interface PreparedChunk {
  /** Per square: play mode's flags (`play/vision`: what blocks sight and movement, difficult
   * ground), worked out here rather than when sight is first worked out. */
  flags: Uint8Array;
  nObj: number;
  atmosphere: number;
  sea: number;
  /** Whose paving the chunk's cobbles are (`battlemap::Chunk::stone`: settlement index + 1 in the low 24
   * bits, its culture + 1 in the high 8; 0 none). */
  stone: number;
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
export const ROOF_TINTS = [0x995742, 0x5c6470, 0xa88b5c, 0x7a6656, 0x7f7b54];
export const ROOF_INK = 0x1d1a14;
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

/** f32 → IEEE half bits (round to nearest). */
const f32 = new Float32Array(1);
const u32 = new Uint32Array(f32.buffer);
export function toHalf(v: number): number {
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

/** IEEE half bits → number. */
export function fromHalf(h: number): number {
  const e = (h >> 10) & 31;
  const m = h & 1023;
  const v = e === 0 ? m * 2 ** -24 : e === 31 ? (m ? NaN : Infinity) : (1 + m / 1024) * 2 ** (e - 15);
  return h & 0x8000 ? -v : v;
}

/** Decode a `battlemap::pack` payload (`kinds[kind - 1]`: the object catalog). */
export function prepareChunk(buf: ArrayBuffer, cx: number, cy: number, kinds: KindTactics[]): PreparedChunk {
  const n = SQ * SQ;
  const nh = HS * HS;
  const dv = new DataView(buf);
  const nObj = dv.getUint32(16, true);
  const atmosphere = dv.getUint32(20, true);
  const sea = dv.getFloat32(24, true);
  const stone = dv.getUint32(28, true);
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
  // v63 countryside: field squares' furrows (`battlemap::Chunk::field`), if the chunk has fields.
  let field: Uint8Array | null = null;
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
      // (255: a keep of pieces, listed after its outline; else the corner towers' side, a castle's
      // keep with no pieces its top in the high two bits.)
      const towerByte = dv.getUint8(o + 3);
      const tower = towerByte === 255 ? 0 : (towerByte & 63) * PX;
      const top = towerByte === 255 ? 0 : towerByte >> 6;
      // Material, shape | 0x10 smoke | shade << 5, weathering (`battlemap::look_bytes`).
      const material = dv.getUint8(o + 4);
      const shapeByte = dv.getUint8(o + 5);
      const wear = dv.getUint8(o + 6);
      o += 7;
      const pts: number[] = [];
      // (Its first corner on the map, in quarter squares: the same in every chunk it is in,
      // and whatever else is built or taken away round it.)
      const at = m ? Math.imul(Math.round((cx * SQ + dv.getFloat32(o, true)) * 4), 73856093) ^ Math.imul(Math.round((cy * SQ + dv.getFloat32(o + 4, true)) * 4), 19349663) : b;
      for (let v = 0; v < m; v++) {
        pts.push(dv.getFloat32(o, true) * PX, dv.getFloat32(o + 4, true) * PX);
        o += 8;
      }
      const parts: Roof['parts'] = [];
      if (towerByte === 255) {
        const np = dv.getUint8(o);
        o += 1;
        for (let q = 0; q < np; q++) {
          const [code, n] = [dv.getUint8(o), dv.getUint8(o + 1)];
          o += 2;
          const pp: number[] = [];
          for (let v = 0; v < n; v++) {
            pp.push(dv.getFloat32(o, true) * PX, dv.getFloat32(o + 4, true) * PX);
            o += 8;
          }
          parts.push({ kind: code & 15, top: code >> 4, pts: pp });
        }
      }
      roofs.push({ pts, floors, battlements, tower, top, cone, tint, material, shape: shapeByte & 15, smoke: (shapeByte & 0x10) !== 0, shade: shapeByte >> 5, wear, idx: at, parts });
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
    const nField = o + 4 <= buf.byteLength ? dv.getUint32(o, true) : 0;
    o += 4;
    if (nField === n && o + n <= buf.byteLength) field = new Uint8Array(buf.slice(o, o + n));
    if (nField <= n) o += nField;
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
  const shown = groundUnder(surface, raised);
  const paveEdge = pavingEdges(shown);
  for (let k = 0; k < n; k++) {
    data[k * 4] = shown[k];
    // (Bit 64: a plaza or market square, `URBAN_PLAZA`, whose cobbles the ground lays in fans;
    // bit 128: by the paving's edge, see `pavingEdges`; bit 8: small closed relief drawn soft, the
    // edges' bit 128 (`battlemap::soft_relief`), in place of their slope-edge-south bit, which
    // nothing reads from here.)
    data[k * 4 + 1] = (edges[k] & 0x77) | (edges[k] & 128 ? 8 : 0) | (raised && raised[k * 2 + 1] === 3 ? 64 : 0) | paveEdge[k];
    data[k * 4 + 2] = tier[k] & 255;
    // Building id (1..255, wrapping) for per-building roof colour and wall edges; on a field's square
    // (never a building's) its furrows (v63 countryside, `town::fields::square_byte`).
    data[k * 4 + 3] = building[k] ? 1 + ((building[k] - 1) % 255) : field && surface[k] === FIELD ? field[k] : 0;
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
    // Variant, and the turned flag above it (`pack`'s 4th byte): readers take `variant % looks(kind)`,
    // which holds because every count of looks (4, 16) divides 256; `>> 8` is the flag.
    objs[j * 6 + 1] = dv.getUint8(o + 2) | (dv.getUint8(o + 3) << 8);
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
    stone,
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

/** Surface 17: under a building. */
const UNDER_BUILDING = 17;
/** Surface 18: a field (v63 countryside: its squares' 4th data byte is its furrows, not a building). */
export const FIELD = 18;

/** 128 for each square with both paving (cobbles 14, dressed stone 19) and natural ground (0-9, 13,
 * 18), or a field (18) and other natural ground (v63 countryside: the field's crisp edge), among the
 * 3 x 3 round it (clamped at the chunk's sides, as `ground.frag`'s `dataAt`), else 0.
 * Every lookup `ground.frag`'s soft paving edge depends on lies within a square of the pixel's own,
 * so off these squares that block changes nothing and the shader skips it (it is a good part of
 * the ground's GPU time in towns). */
function pavingEdges(shown: Uint8Array): Uint8Array {
  const out = new Uint8Array(SQ * SQ);
  for (let y = 0; y < SQ; y++) {
    for (let x = 0; x < SQ; x++) {
      let paved = false;
      let natural = false;
      let field = false;
      let other = false;
      for (let dy = -1; dy <= 1; dy++) {
        const yy = Math.min(SQ - 1, Math.max(0, y + dy));
        for (let dx = -1; dx <= 1; dx++) {
          const s = shown[yy * SQ + Math.min(SQ - 1, Math.max(0, x + dx))];
          if (s === 14 || s === 19) paved = true;
          else if (s <= 9 || s === 13 || s === 18) natural = true;
          if (s === 18) field = true;
          else if (s <= 9 || s === 13 || s === 21) other = true;
        }
      }
      if ((paved && natural) || (field && other)) out[y * SQ + x] = 128;
    }
  }
  return out;
}

/**
 * The surfaces the ground is drawn with: a square under a building, a wall or a tower shows the
 * ground of the nearest square that isn't (roofs, walls and towers are drawn as vectors on their
 * outlines, so the squares one doesn't quite cover — round a tower, along a slanted wall, either
 * side of a curtain — show the ground round them, not a plate or a fringe of stone). Play and hover
 * keep the squares' own surfaces.
 */
function groundUnder(surface: Uint8Array, raised: Uint8Array | null): Uint8Array {
  const out = surface.slice();
  // (`raised`: lift and urban code per square; 7 a wall, 8 a tower.)
  const under = (k: number) => surface[k] === UNDER_BUILDING || (!!raised && raised[k * 2] > 0 && (raised[k * 2 + 1] === 7 || raised[k * 2 + 1] === 8));
  let frontier: number[] = [];
  for (let k = 0; k < SQ * SQ; k++) if (!under(k)) frontier.push(k);
  if (frontier.length === SQ * SQ) return out;
  const seen = new Uint8Array(SQ * SQ);
  for (const k of frontier) seen[k] = 1;
  // Breadth first from the open ground, a step at a time (a square takes its first-reached
  // neighbour's: west, east, north, south in that order).
  while (frontier.length) {
    const next: number[] = [];
    for (const k of frontier) {
      const [i, j] = [k % SQ, Math.floor(k / SQ)];
      for (const [di, dj] of [[-1, 0], [1, 0], [0, -1], [0, 1]]) {
        const [a, b] = [i + di, j + dj];
        if (a < 0 || b < 0 || a >= SQ || b >= SQ) continue;
        const q = b * SQ + a;
        if (seen[q]) continue;
        seen[q] = 1;
        out[q] = out[k];
        next.push(q);
      }
    }
    frontier = next;
  }
  // (A chunk all under one building keeps the building's.)
  return out;
}

/** Long things laid along their turn (by catalog name): the rectangle that blocks, [half length, half
 * width] in squares at scale 1 (a hedge a section a square long, as runs are set five feet apart). */
const LONG_FOOTPRINT = new Map([
  ['hedge', [0.49, 0.29]],
  // v63 castles: a kitchen garden's rows and wattle fence.
  ['vegetable row', [0.98, 0.3]],
  ['wattle fence', [0.49, 0.12]],
]);

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
    if ((data[k * 4 + 3] && surface[k] !== FIELD) || (raised && raised[k * 2] > 0 && (raised[k * 2 + 1] === 7 || raised[k * 2 + 1] === 8))) v |= OPAQUE | BLOCKS;
    f[k] = v;
  }
  for (let o = 0; o < objs.length; o += 6) {
    const inf = kindRules(objs[o], kinds, sprites);
    if (!inf) continue;
    const v = (inf.blocks_sight ? OPAQUE : 0) | (inf.blocks_move ? BLOCKS : 0) | (inf.difficult ? DIFFICULT : 0);
    if (!v) continue;
    const [x, y, scale] = [objs[o + 2], objs[o + 3], objs[o + 5]];
    const long = objs[o] < SPRITE_KIND && inf.name !== undefined ? LONG_FOOTPRINT.get(inf.name) : undefined;
    if (long) {
      // Every square the piece is drawn across (points over its rectangle a tenth of a square apart), so
      // pieces set end to end block an unbroken line, slanted too.
      const [c, s] = [Math.cos(objs[o + 4]), Math.sin(objs[o + 4])];
      const [hl, hw] = [long[0] * scale, long[1] * scale];
      for (let u = -hl; u <= hl + 1e-6; u += Math.min(0.1, hl)) {
        for (let w = -hw; w <= hw + 1e-6; w += Math.min(0.1, hw)) {
          const [i, j] = [Math.floor(x + c * u - s * w), Math.floor(y + s * u + c * w)];
          if (i >= 0 && j >= 0 && i < SQ && j < SQ) f[j * SQ + i] |= v;
        }
      }
      continue;
    }
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
  if (c.roofMesh) t.push(c.roofMesh.pos.buffer, c.roofMesh.color.buffer, c.roofMesh.roof.buffer, c.roofMesh.index.buffer);
  return t;
}

/** Chunk pixels per foot. */
const FT = PX / 5;
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
function roofMesh(roofs: Roof[]): ColorMesh {
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
    g.mat = roof.material < ROOF_MATERIAL_RGB.length ? roof.material : 3;
    g.code = g.mat + 16 * Math.min(15, roof.wear >> 4);
    g.seed = (((roof.idx * 2654435761) >>> 0) % 509) * 0.73;
    // (Ranges, halls and wings of a domed roof behind parapets, as flat ones.)
    const flat = roof.shape === FLAT || (roof.shape === DOME && !roof.cone);
    // A palace round its court: a hipped (or flat) range along each side, the court open to the sky.
    const ranges = courtRanges(roof);
    if (ranges) {
      for (const part of ranges) (flat ? flatRoof : hipRoof)(g, part, tint);
      if (!flat) for (const part of ranges) g.outline(part, INK_LINE, ROOF_INK);
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
    if (!roof.cone && flat) {
      flatRoof(g, roof.pts, tint);
      if (roof.smoke) smoke(g, roof.pts, 0.2);
      g.outline(roof.pts, INK_OUT, ROOF_INK);
      continue;
    }
    const round = roof.cone ? null : roundOutline(roof);
    if (round) {
      // (Felt whatever the material, unless tinted by hand.)
      const felt = roof.tint >= 0 && roof.tint < ROOF_TINTS.length && roof.shade === 2 ? tint : FELT[roof.shade] ?? FELT[0];
      roundRoof(g, round, felt);
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
const GABLE = 1;
const CONE = 2;
const DOME = 3;
const FLAT = 4;
const ROUND = 5;

/** A palace's court among its pieces (`battlemap::COURT_PART`); a manor's cross wing and main range
 * (`battlemap::CROSS_WING_PART`, `MAIN_RANGE_PART`). */
const COURT = 5;
const CROSS_WING = 6;
const MAIN_RANGE = 7;
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
function flatRoof(g: MeshBuilder, pts: number[], tint: number) {
  const m = pts.length / 2;
  const { ux, uy, halfW } = frame(pts);
  const w = Math.min(0.9 * FT, halfW * 0.3);
  const [ox, oy] = [pts[0], pts[1]];
  const uv = (x: number, y: number): [number, number] => [g.seed + ((x - ox) * ux + (y - oy) * uy) / FT, (-(x - ox) * uy + (y - oy) * ux) / FT];
  const inner = inset(pts, w);
  if (!inner) {
    g.patch(pts, uv, shadeHex(tint, 1.02));
    return;
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
  if (bi < 0) return;
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
}

/** A circle as a polygon. */
function circle(cx: number, cy: number, r: number, n: number): number[] {
  const out: number[] = [];
  for (let k = 0; k < n; k++) out.push(cx + r * Math.cos((2 * Math.PI * k) / n), cy + r * Math.sin((2 * Math.PI * k) / n));
  return out;
}

/** The plaster of a dome's drum and the flat roof round it (whitewash keeps its own). */
const PLASTER = 0xb4a68b;

/** A dome of radius r about (cx, cy) on its drum: its shadow on the roof round it, the drum's ring, the
 * dome in three flat tones lit from the NW (a shaded crescent to the SE, the mid tone, a lit cap towards
 * the light), its seams in a few ribs (not whitewash) up to a finial, inked round. */
function dome(g: MeshBuilder, cx: number, cy: number, r: number, tint: number) {
  const n = 24;
  g.poly(circle(cx + r * 0.24, cy + r * 0.24, r * 1.06, n), 0x000000, 0.3);
  const drum = circle(cx, cy, r * 1.12, n);
  g.poly(drum, g.mat === 11 ? shadeHex(tint, 1.04) : PLASTER);
  g.outline(drum, INK_LINE, ROOF_INK);
  // (Each lighter disc inside the last, pushed towards the light.)
  const nw = -Math.SQRT1_2;
  for (const [f, o, t] of [
    [1, 0, 0.86],
    [0.84, 0.16, 1.0],
    [0.62, 0.26, 1.12],
  ]) {
    g.poly(circle(cx + nw * r * o, cy + nw * r * o, r * f, n), shadeHex(tint, t));
  }
  if (g.mat !== 11) {
    const rib = shadeHex(tint, 0.72);
    for (let j = 0; j < 8; j++) {
      const th = (Math.PI * j) / 4 + Math.PI / 8;
      g.line(cx + r * 0.97 * Math.cos(th), cy + r * 0.97 * Math.sin(th), cx + r * 0.12 * Math.cos(th), cy + r * 0.12 * Math.sin(th), 0.018 * PX, rib, 0.8);
    }
  }
  g.outline(circle(cx, cy, r, n), INK_OUT, ROOF_INK);
  g.poly(circle(cx, cy, Math.max(2, r * 0.09), 8), ROOF_INK);
  g.poly(circle(cx - r * 0.015, cy - r * 0.015, Math.max(1, r * 0.05), 8), 0xb8963e);
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

/** Felt for a round roof (`Roof.shade` 0-4): cream, off-white, grey, warm cream, dusty grey. */
const FELT = [0xc9bea6, 0xc4bdad, 0xaea595, 0xc2b394, 0xa39a88];

/** A low round roof (a yurt's, whatever its material: felt over the poles): a wedge per side in three
 * flat tones lit from the NW (shaded SE), faint seams along every other pole, two rope bands round it,
 * the crown's wooden ring with its cross over the smoke hole; unpatterned. */
function roundRoof(g: MeshBuilder, ring: number[], felt: number) {
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

/** A yurt's door on its south side (where they face): a small painted panel under a felt hood, through
 * the rim, inked; drawn over the eaves' ink. */
function yurtDoor(g: MeshBuilder, ring: number[], felt: number) {
  const n = ring.length / 2;
  let [cx, cy, low] = [0, 0, -Infinity];
  for (let i = 0; i < n; i++) [cx, cy] = [cx + ring[i * 2] / n, cy + ring[i * 2 + 1] / n];
  for (let i = 0; i < n; i++) low = Math.max(low, ring[i * 2 + 1]);
  // (Where the rim crosses due south of the middle: the lowest y of the sides spanning cx.)
  let ey = low;
  for (let i = 0; i < n; i++) {
    const [ax, ay, bx, by] = [ring[i * 2], ring[i * 2 + 1], ring[((i + 1) % n) * 2], ring[((i + 1) % n) * 2 + 1]];
    if ((ax - cx) * (bx - cx) <= 0 && ax !== bx && ay > cy) ey = ay + ((cx - ax) / (bx - ax)) * (by - ay);
  }
  const [w, out, ins] = [1.6 * FT, 1.1 * FT, 1.4 * FT];
  const hood = [cx - w, ey - ins, cx + w, ey - ins, cx + w, ey + out, cx - w, ey + out];
  g.poly(hood, shadeHex(felt, 0.9));
  g.poly([cx - w * 0.6, ey - ins * 0.2, cx + w * 0.6, ey - ins * 0.2, cx + w * 0.6, ey + out, cx - w * 0.6, ey + out], 0x8a4a32);
  g.outline(hood, INK_LINE, ROOF_INK);
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
