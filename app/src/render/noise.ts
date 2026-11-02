// A shared tileable value-noise lattice (512², one random byte per lattice point), sampled
// with linear filtering at smoothstep-warped coordinates: one texture read gives the same
// interpolation as hashing four lattice points in the shader.
import { BufferImageSource } from 'pixi.js';

export const NOISE_SIZE = 512;
let source: BufferImageSource | null = null;
let bytes: Uint8Array | null = null;

/** The lattice (RGBA): what shaders read through `noiseSource`, for the CPU to make the same sums. */
export function noiseBytes(): Uint8Array {
  if (bytes) return bytes;
  const data = new Uint8Array(NOISE_SIZE * NOISE_SIZE * 4);
  for (let k = 0; k < NOISE_SIZE * NOISE_SIZE; k++) {
    // Integer hash (lowbias32) of the lattice index.
    let h = k ^ 0x9e3779b9;
    h = Math.imul(h ^ (h >>> 16), 0x7feb352d);
    h = Math.imul(h ^ (h >>> 15), 0x846ca68b);
    h ^= h >>> 16;
    data[k * 4] = h & 255;
    data[k * 4 + 1] = (h >>> 8) & 255;
  }
  return (bytes = data);
}

export function noiseSource(): BufferImageSource {
  if (source) return source;
  source = new BufferImageSource({
    resource: noiseBytes(),
    width: NOISE_SIZE,
    height: NOISE_SIZE,
    format: 'rgba8unorm',
    alphaMode: 'no-premultiply-alpha',
    scaleMode: 'linear',
    addressMode: 'repeat',
  });
  return source;
}
