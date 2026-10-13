// The ground brushed by hand, as `Edits.ground` keeps it (`worldgen::ground`): blocks of 16 × 16
// squares (80 ft), keyed `g:XXXXX:YYYYY` (block indices zero-padded, x first), each a string:
// base64url (no padding) of zigzag-delta varints of its values (tenths of a foot, row-major),
// a 0 delta followed by a byte counting the further zeros. The generator reads only what is
// stored, so this twin must write what `ground::encode` writes.

/** Squares per block side. */
export const BLOCK = 16;
const CELLS = BLOCK * BLOCK;
/** Blocks a world may hold. */
export const MAX_BLOCKS = 1500;
/** Blocks one stroke may touch. */
export const MAX_STROKE_BLOCKS = 48;
/** A value's limit (tenths of a foot: 150 ft). */
export const MAX_ADD = 1500;
export const MAX_PLATEAU_PTS = 48;
export const MAX_PLATEAU_SPAN_FT = 1280;
export const MAX_EDGE_FT = 60;

const B64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
const VAL = new Int8Array(128).fill(-1);
for (let i = 0; i < 64; i++) VAL[B64.charCodeAt(i)] = i;

export function blockKey(bx: number, by: number): string {
  return `g:${String(bx).padStart(5, '0')}:${String(by).padStart(5, '0')}`;
}

/** A key's block indices, if it is one as `blockKey` writes them. */
export function parseKey(k: string): [number, number] | null {
  const m = /^g:(\d{5}):(\d{5})$/.exec(k);
  return m ? [Number(m[1]), Number(m[2])] : null;
}

function b64Encode(bytes: number[]): string {
  let out = '';
  for (let i = 0; i < bytes.length; i += 3) {
    const n = Math.min(3, bytes.length - i);
    const v = (bytes[i] << 16) | ((bytes[i + 1] ?? 0) << 8) | (bytes[i + 2] ?? 0);
    for (let k = 0; k < n + 1; k++) out += B64[(v >> (18 - 6 * k)) & 63];
  }
  return out;
}

function b64Decode(s: string): Uint8Array | null {
  if (s.length % 4 === 1) return null;
  const out: number[] = [];
  for (let i = 0; i < s.length; i += 4) {
    const c = s.slice(i, i + 4);
    let v = 0;
    for (let k = 0; k < c.length; k++) {
      const x = c.charCodeAt(k) < 128 ? VAL[c.charCodeAt(k)] : -1;
      if (x < 0) return null;
      v |= x << (18 - 6 * k);
    }
    for (let k = 0; k < c.length - 1; k++) out.push((v >> (16 - 8 * k)) & 255);
  }
  return Uint8Array.from(out);
}

/** A block's values (256, tenths of a foot) as its string. */
export function encode(add: Int16Array): string {
  const out: number[] = [];
  let prev = 0;
  let i = 0;
  while (i < CELLS) {
    const d = add[i] - prev;
    prev = add[i];
    if (d === 0) {
      let n = 0;
      while (i + 1 + n < CELLS && n < 255 && add[i + 1 + n] === prev) n++;
      out.push(0, n);
      i += 1 + n;
    } else {
      let z = ((d << 1) ^ (d >> 31)) >>> 0;
      while (z >= 0x80) {
        out.push((z & 0x7f) | 0x80);
        z >>>= 7;
      }
      out.push(z);
      i++;
    }
  }
  return b64Encode(out);
}

/** A block's string as its values, or null if it isn't one. */
export function decode(raw: string): Int16Array | null {
  const bytes = b64Decode(raw);
  if (!bytes) return null;
  const add = new Int16Array(CELLS);
  let [prev, i, at] = [0, 0, 0];
  while (at < bytes.length) {
    let z = 0;
    let shift = 0;
    for (;;) {
      if (at >= bytes.length) return null;
      const b = bytes[at++];
      z |= (b & 0x7f) << shift;
      if (!(b & 0x80)) break;
      shift += 7;
      if (shift > 14) return null;
    }
    const d = (z >>> 1) ^ -(z & 1);
    const v = prev + d;
    if (Math.abs(v) > MAX_ADD || i >= CELLS) return null;
    add[i++] = v;
    prev = v;
    if (d === 0) {
      if (at >= bytes.length) return null;
      const n = bytes[at++];
      if (i + n > CELLS) return null;
      add.fill(v, i, i + n);
      i += n;
    }
  }
  return i === CELLS ? add : null;
}

/** Decoded blocks, by their string (a block's string is its value). */
const cache = new Map<string, Int16Array | null>();

export function values(raw: string): Int16Array | null {
  let v = cache.get(raw);
  if (v === undefined) {
    v = decode(raw);
    if (cache.size > 4096) cache.clear();
    cache.set(raw, v);
  }
  return v;
}

/** The brushed change of global square (gx, gy), ft. */
export function added(ground: Record<string, string> | undefined, gx: number, gy: number): number {
  if (!ground || gx < 0 || gy < 0) return 0;
  const raw = ground[blockKey(Math.floor(gx / BLOCK), Math.floor(gy / BLOCK))];
  const v = raw ? values(raw) : null;
  return v ? v[(gy % BLOCK) * BLOCK + (gx % BLOCK)] * 0.1 : 0;
}
