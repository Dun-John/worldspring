// Battlemap chunk preparation, run in the generator workers: decode the payload, build the
// GPU-ready texture arrays, sort the objects and tessellate the pitched roofs into one
// vertex-coloured mesh, so the main thread only creates GPU objects (no parsing, no
// Graphics tessellation in a frame).
import { PX, roofMesh } from './roofMesh';
export { PX, ROOF_INK, ROOF_TINTS, shadeHex } from './roofMesh';
import { BLOCKS, DIFFICULT, OPAQUE } from '../play/vision';

export const SQ = 128;
export const HS = SQ + 2;

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
  /** Colour variant 0-4 of the material (`ROOF_SHADES`); a yurt's felt 0-7 (`YURT_FELT`). */
  shade: number;
  /** R5 towns (115): a steppe home's yurt (the material byte's high bit, `battlemap::look_bytes`). */
  yurt: boolean;
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

/**
 * A structure drawn as a vector (`battlemap::VectorShape`): points flattened and scaled by `PX`. A wall,
 * a tower and a gate carry their style after their flags (`town::wallstyle::WallStyle::code`: group,
 * tower shape, material, merlons, stone, outline, gatehouse; `render/walls.ts` `styleOf`); a gate and
 * its towers the gatehouse's form as built beside it, a gate then its side and a bent way's exit
 * (`town::gatehouse::House`; `render/walls.ts` `gateForm`). A wall's flags point is `[flags, side]`: side 1
 * where the outside (of the town, or of the castle) is left of its first point to its second, -1 right,
 * 0 unknown (`town::wallside`). Kind 20 is a star wall's bastion (`battlemap::ShapeKind::Bastion`): its
 * outline (flank's foot, flank's tip, point, tip, foot, the corner it replaced), then `[style, 0]`;
 * `size` its walls' width.
 */
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
  // Road halo → filtered channels: b = road coverage (`Chunk::road_cover`: ramped across the
  // roads' edges, so the filtered 0.5 contour follows the road's line), a = kind × coverage
  // (0 earth, 0.5 cobbles, 1 bridge deck), so kind = a / b is the mean over nearby road squares.
  const roadH = new Uint8Array(buf, objOffset + nObj * 20, nh);
  const roadCover = new Uint8Array(buf, objOffset + nObj * 20 + nh, nh);
  const building = new Uint16Array(buf.slice(objOffset + nObj * 20 + 2 * nh, objOffset + nObj * 20 + 2 * nh + n * 2));
  const roofs: Roof[] = [];
  const shapes: Shape[] = [];
  const sprites: ChunkSprite[] = [];
  let raised: Uint8Array | null = null;
  // v63 countryside: field squares' furrows (`battlemap::Chunk::field`), if the chunk has fields.
  let field: Uint8Array | null = null;
  {
    let o = objOffset + nObj * 20 + 2 * nh + n * 2;
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
      const matByte = dv.getUint8(o + 4);
      const material = matByte & 0x7f;
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
      roofs.push({ pts, floors, battlements, tower, top, cone, tint, material, shape: shapeByte & 15, smoke: (shapeByte & 0x10) !== 0, shade: shapeByte >> 5, yurt: (matByte & 0x80) !== 0, wear, idx: at, parts });
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
    const cover = roadCover[k] / 255;
    // (A square beside the road takes the kind of the road beside it.)
    const kindOf = (r: number) => (r & 0x80 ? 1 : (r & 0x7f) === 1 ? 0.5 : 0);
    let kind = kindOf(r);
    if (!r && cover > 0) {
      const hs = HS;
      const [x, y] = [k % hs, Math.floor(k / hs)];
      for (let d = 0; d < 8 && kind === 0; d++) {
        const [nx, ny] = [x + [1, -1, 0, 0, 1, 1, -1, -1][d], y + [0, 0, 1, -1, 1, -1, 1, -1][d]];
        if (nx >= 0 && ny >= 0 && nx < hs && ny < hs && roadH[ny * hs + nx]) kind = kindOf(roadH[ny * hs + nx]);
      }
    }
    hdata[k * 4 + 2] = toHalf(cover);
    hdata[k * 4 + 3] = toHalf(kind * cover);
  }
  const data = new Uint8Array(n * 4);
  const shown = groundUnder(surface, raised);
  const paveEdge = pavingEdges(shown);
  for (let k = 0; k < n; k++) {
    data[k * 4] = shown[k];
    // (Bit 64: a plaza or market square, `URBAN_PLAZA`, whose cobbles the ground lays in fans;
    // bit 128: by the paving's edge, see `pavingEdges`; bit 8: small closed relief drawn soft, the
    // edges' bit 128 (`battlemap::soft_relief`), in place of their slope-edge-south bit, which
    // nothing reads from here; bit 4: a cliff's scree, the edges' bit 64 (`battlemap::cliff`), in place
    // of their slope-edge-east bit.)
    data[k * 4 + 1] = (edges[k] & 0x33) | (edges[k] & 64 ? 4 : 0) | (edges[k] & 128 ? 8 : 0) | (raised && raised[k * 2 + 1] === 3 ? 64 : 0) | paveEdge[k];
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
          else if (s <= 9 || s === 13) other = true;
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
  // Roadmap 6 R6 fences (`town::fence::KINDS`): pieces end to end, five feet apart.
  ['picket fence', [0.49, 0.1]],
  ['iron railing', [0.49, 0.1]],
  ['stockade', [0.49, 0.22]],
  ['gate', [0.49, 0.1]],
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
