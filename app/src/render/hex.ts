// Overland hex grids (`Edits.hexes`, item 127): the geometry. A grid is flat-topped ("odd-q":
// odd columns shoved down half a hex) or pointy ("odd-r": odd rows shoved right), `size_mi`
// across the flats (centre to neighbouring centre). Columns and rows count from 1: hex 0101
// sits in the map's top-left corner (world 0,0), the whole lattice moved by the origin offset.
// Every position is a pure function of the grid. Keys (`"col,row"`) and labels (`CCRR`) are
// global: one hex has one name in every window.
import type { Edits, HexGrid } from '../gen/protocol';

export const FT_PER_MI = 5280;
/** The one grid's key in `Edits.hexes`. */
export const HEX_KEY = 'overland';
const SQRT3 = Math.sqrt(3);

export type Hex = Required<HexGrid>;

/** A new grid's settings (`worldgen::world::HexGrid::default`): 6 mi, flat-topped, ink at 35 %. */
export const HEX_DEFAULTS: Hex = { size_mi: 6, flat: true, color: '#2b241d', width: 1.5, opacity: 0.35, ox_mi: 0, oy_mi: 0, numbers: true };
/** The sizes a grid may have (mi across the flats). */
export const HEX_SIZE_MI: [number, number] = [0.5, 60];
export const HEX_WIDTH_PX: [number, number] = [0.5, 6];

const num = (v: unknown, d: number, lo: number, hi: number) => (typeof v === 'number' && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : d);

/** The world's hex grid as set (`Edits.hexes.overland`), each field checked and filled in. */
export function hexGrid(e: Edits | undefined | null): Hex {
  const g = (e?.hexes?.[HEX_KEY] ?? {}) as HexGrid;
  const d = HEX_DEFAULTS;
  const size = num(g.size_mi, d.size_mi, HEX_SIZE_MI[0], HEX_SIZE_MI[1]);
  return {
    size_mi: size,
    flat: typeof g.flat === 'boolean' ? g.flat : d.flat,
    color: typeof g.color === 'string' && /^#[0-9a-f]{6}$/i.test(g.color) ? g.color.toLowerCase() : d.color,
    width: num(g.width, d.width, HEX_WIDTH_PX[0], HEX_WIDTH_PX[1]),
    opacity: num(g.opacity, d.opacity, 0, 1),
    ox_mi: num(g.ox_mi, 0, -1e4, 1e4),
    oy_mi: num(g.oy_mi, 0, -1e4, 1e4),
    numbers: typeof g.numbers === 'boolean' ? g.numbers : d.numbers,
  };
}

/** A grid's lattice in feet: `s` across the flats, `R` centre to corner, `x0`/`y0` hex 0101's
 * centre, `dx`/`dy` the step between columns and rows. */
export interface HexFrame {
  flat: boolean;
  s: number;
  R: number;
  x0: number;
  y0: number;
  dx: number;
  dy: number;
}

export function hexFrame(g: Hex): HexFrame {
  const s = g.size_mi * FT_PER_MI;
  const R = s / SQRT3;
  const ox = g.ox_mi * FT_PER_MI;
  const oy = g.oy_mi * FT_PER_MI;
  return g.flat ? { flat: true, s, R, x0: R + ox, y0: s / 2 + oy, dx: 1.5 * R, dy: s } : { flat: false, s, R, x0: s / 2 + ox, y0: R + oy, dx: s, dy: 1.5 * R };
}

/** The centre (world ft) of the hex at column `col`, row `row` (from 1). */
export function hexCentre(f: HexFrame, col: number, row: number): [number, number] {
  const i = col - 1;
  const j = row - 1;
  return f.flat ? [f.x0 + i * f.dx, f.y0 + j * f.dy + (i & 1 ? f.s / 2 : 0)] : [f.x0 + i * f.dx + (j & 1 ? f.s / 2 : 0), f.y0 + j * f.dy];
}

/** Offset (0-based column i, row j) ↔ axial (q, r), for odd-q (flat) and odd-r (pointy). */
function toAxial(f: HexFrame, i: number, j: number): [number, number] {
  return f.flat ? [i, j - (i - (i & 1)) / 2] : [i - (j - (j & 1)) / 2, j];
}
function fromAxial(f: HexFrame, q: number, r: number): [number, number] {
  return f.flat ? [q, r + (q - (q & 1)) / 2] : [q + (r - (r & 1)) / 2, r];
}

/** The hex (column, row from 1) holding the world point (x, y) ft. */
export function hexAt(f: HexFrame, x: number, y: number): [number, number] {
  const px = x - f.x0;
  const py = y - f.y0;
  const fq = f.flat ? ((2 / 3) * px) / f.R : ((SQRT3 / 3) * px - py / 3) / f.R;
  const fr = f.flat ? (-px / 3 + (SQRT3 / 3) * py) / f.R : ((2 / 3) * py) / f.R;
  // Cube rounding: the coordinate furthest from its rounding is the one put right.
  const fs = -fq - fr;
  let q = Math.round(fq);
  let r = Math.round(fr);
  const s = Math.round(fs);
  const dq = Math.abs(q - fq);
  const dr = Math.abs(r - fr);
  const ds = Math.abs(s - fs);
  if (dq > dr && dq > ds) q = -r - s;
  else if (dr > ds) r = -q - s;
  const [i, j] = fromAxial(f, q, r);
  return [i + 1, j + 1];
}

/** The hex's six corners (world ft), clockwise on screen from the east one (flat) or the
 * north-east one (pointy). */
export function hexCorners(f: HexFrame, col: number, row: number): [number, number][] {
  const [cx, cy] = hexCentre(f, col, row);
  const out: [number, number][] = [];
  for (let k = 0; k < 6; k++) {
    const a = (Math.PI / 3) * k - (f.flat ? 0 : Math.PI / 6);
    out.push([cx + f.R * Math.cos(a), cy + f.R * Math.sin(a)]);
  }
  return out;
}

const AXIAL_DIRS: [number, number][] = [
  [1, 0],
  [1, -1],
  [0, -1],
  [-1, 0],
  [-1, 1],
  [0, 1],
];

/** The six hexes round (col, row). */
export function hexRing(f: HexFrame, col: number, row: number): [number, number][] {
  const [q, r] = toAxial(f, col - 1, row - 1);
  return AXIAL_DIRS.map(([dq, dr]) => {
    const [i, j] = fromAxial(f, q + dq, r + dr);
    return [i + 1, j + 1];
  });
}

/** A hex's key in play state (`"col,row"`). */
export function hexKey(col: number, row: number): string {
  return `${col},${row}`;
}

/** A key back to (col, row), or null. */
export function parseHexKey(key: string): [number, number] | null {
  const m = /^(-?\d+),(-?\d+)$/.exec(key);
  return m ? [Number(m[1]), Number(m[2])] : null;
}

/** The columns and rows reaching into a map `w` × `h` ft: [first col, last col, first row, last row]. */
export function hexSpan(f: HexFrame, w: number, h: number): [number, number, number, number] {
  return hexRange(f, 0, 0, w, h);
}

/** The columns and rows whose hexes may reach into the world rect [x0, x1] × [y0, y1]. */
export function hexRange(f: HexFrame, x0: number, y0: number, x1: number, y1: number): [number, number, number, number] {
  // (A hex reaches R across its columns' step and s/2 along it; the shoved half step too.)
  const [ex, ey] = f.flat ? [f.R, f.s] : [f.s, f.R];
  return [
    Math.floor((x0 - ex - f.x0) / f.dx) + 1,
    Math.ceil((x1 + ex - f.x0) / f.dx) + 1,
    Math.floor((y0 - ey - f.y0) / f.dy) + 1,
    Math.ceil((y1 + ey - f.y0) / f.dy) + 1,
  ];
}

/** Digits per part of a label in a map `w` × `h` ft: 2, or 3 once it has more than 99 columns or rows. */
export function hexDigits(f: HexFrame, w: number, h: number): number {
  const [c0, c1, r0, r1] = hexSpan(f, w, h);
  return Math.max(Math.abs(c0), Math.abs(c1), Math.abs(r0), Math.abs(r1)) > 99 ? 3 : 2;
}

/** A hex's number, `CCRR` (`digits` each, zero-padded: 0412). */
export function hexLabel(col: number, row: number, digits = 2): string {
  const pad = (n: number) => (n < 0 ? '-' : '') + String(Math.abs(n)).padStart(digits, '0');
  return pad(col) + pad(row);
}
