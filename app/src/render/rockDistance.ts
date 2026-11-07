// How far each point of an underground level's rock lies from the nearest floor (`underground.frag`
// hatches the rock beside the walls by it and fades the rock out beyond), worked out once per level
// with the field textures (`underField.ts`, in the worker for sewer sections): an exact Euclidean
// distance transform (Felzenszwalb and Huttenlocher) over the field's texels, so the shader reads it
// with one fetch instead of looking round.

/** Squares the distance texture reaches past the grid each way (a site's quad is drawn this much
 * wider, so its rock fades out before the quad's edge; sewer sections read their neighbours' floor
 * this far). */
export const DIST_PAD = 4;
/** Distance (squares) at 255: the rock is gone well before. */
export const DIST_MAX = 8;

/** One row or column: squared distances `f` (0 on floor, INF elsewhere) to the lower envelope of
 * parabolas, in place. */
function edt1d(f: Float32Array, n: number, v: Int32Array, z: Float32Array, out: Float32Array) {
  let k = 0;
  v[0] = 0;
  z[0] = -Infinity;
  z[1] = Infinity;
  for (let q = 1; q < n; q++) {
    if (f[q] >= 1e20 && f[v[k]] >= 1e20) continue;
    let s = (f[q] + q * q - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
    while (s <= z[k]) {
      k--;
      s = (f[q] + q * q - (f[v[k]] + v[k] * v[k])) / (2 * q - 2 * v[k]);
    }
    k++;
    v[k] = q;
    z[k] = s;
    z[k + 1] = Infinity;
  }
  k = 0;
  for (let q = 0; q < n; q++) {
    while (z[k + 1] < q) k++;
    const dq = q - v[k];
    out[q] = dq * dq + f[v[k]];
  }
}

/**
 * The distance texture (`r8unorm`, (nx + 2 * DIST_PAD) * R texels across, likewise down; R the
 * field's texels a square, `FIELD_RES`):
 * squares to the nearest floor, 0 on it, `DIST_MAX` at 255. `floor` is the field's floor (w x h,
 * `rim` squares past the grid each way; floor where above a half, as the shader draws its walls);
 * `whole(i, j)`: grid squares drawn from something else (shaped rooms, caverns), floor throughout.
 * Past what `floor` covers is rock (a site has nothing beyond its grid; a sewer section's rim
 * reaches past the pad).
 */
export function rockDistance(floor: Float32Array, w: number, h: number, R: number, rim: number, nx: number, ny: number, whole: (i: number, j: number) => boolean): Uint8Array {
  const P = DIST_PAD;
  const [W, H] = [(nx + 2 * P) * R, (ny + 2 * P) * R];
  const INF = 1e20;
  const g = new Float32Array(W * H).fill(INF);
  const shift = (rim - P) * R;
  for (let y = 0; y < H; y++) {
    const fy = y + shift;
    if (fy < 0 || fy >= h) continue;
    for (let x = 0; x < W; x++) {
      const fx = x + shift;
      if (fx >= 0 && fx < w && floor[fy * w + fx] > 0.5) g[y * W + x] = 0;
    }
  }
  for (let j = 0; j < ny; j++) {
    for (let i = 0; i < nx; i++) {
      if (!whole(i, j)) continue;
      for (let y = (j + P) * R; y < (j + P + 1) * R; y++) g.fill(0, y * W + (i + P) * R, y * W + (i + P + 1) * R);
    }
  }
  const n = Math.max(W, H);
  const f = new Float32Array(n);
  const o = new Float32Array(n);
  const v = new Int32Array(n);
  const z = new Float32Array(n + 1);
  // Down the columns, then along the rows.
  for (let x = 0; x < W; x++) {
    for (let y = 0; y < H; y++) f[y] = g[y * W + x];
    edt1d(f, H, v, z, o);
    for (let y = 0; y < H; y++) g[y * W + x] = o[y];
  }
  const out = new Uint8Array(W * H);
  const scale = 255 / (DIST_MAX * R);
  for (let y = 0; y < H; y++) {
    const row = y * W;
    for (let x = 0; x < W; x++) f[x] = g[row + x];
    edt1d(f, W, v, z, o);
    for (let x = 0; x < W; x++) out[row + x] = Math.min(255, Math.sqrt(o[x]) * scale + 0.5) | 0;
  }
  return out;
}
