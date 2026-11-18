// Tundra's ground texture on the map (`terrain.frag`, `uTundra`): drawn once, seamless, pinned to the
// world through the symbol pattern's coordinates (it repeats every `TUNDRA_CELLS` pattern cells). A
// data texture, not colours: each channel is one paint the shader tints. (Tundra's snow is drawn on
// battlemaps only.)
//   R heather: small rounded clumps among the grass.
//   G grass shade: tussocks (a few short strokes each) and a few broad soft hollows.
//   B unused (0).
//   A grass light: broad soft lit humps.
// In the hand-drawn cartoon style of the sprites: light, a few big shapes, no fine noise.
import { BufferImageSource } from 'pixi.js';

/** Texels across (and down). */
export const TUNDRA_TEX = 256;

type Ctx = OffscreenCanvasRenderingContext2D | CanvasRenderingContext2D;

/** mulberry32: a fixed sequence, so the texture is the same every time. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** `draw` at each of the nine wrapped offsets, so shapes crossing an edge come in at the other. */
function wrapped(ctx: Ctx, draw: (ox: number, oy: number) => void) {
  for (const oy of [-TUNDRA_TEX, 0, TUNDRA_TEX]) for (const ox of [-TUNDRA_TEX, 0, TUNDRA_TEX]) draw(ox, oy);
}

/** One channel: shapes painted white (by amount) on black; its red as bytes. */
function channel(paint: (ctx: Ctx, r: () => number) => void, seed: number): Uint8ClampedArray {
  const canvas = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(TUNDRA_TEX, TUNDRA_TEX) : Object.assign(document.createElement('canvas'), { width: TUNDRA_TEX, height: TUNDRA_TEX });
  const ctx = canvas.getContext('2d') as Ctx;
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, TUNDRA_TEX, TUNDRA_TEX);
  paint(ctx, rng(seed));
  return ctx.getImageData(0, 0, TUNDRA_TEX, TUNDRA_TEX).data;
}

const grey = (v: number) => `rgb(${Math.round(v * 255)},${Math.round(v * 255)},${Math.round(v * 255)})`;

/** A soft round blob (radial falloff), at most `v`. */
function blob(ctx: Ctx, x: number, y: number, rx: number, ry: number, angle: number, v: number) {
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(angle);
  ctx.scale(1, ry / rx);
  const g = ctx.createRadialGradient(0, 0, 0, 0, 0, rx);
  g.addColorStop(0, grey(v));
  g.addColorStop(0.55, grey(v * 0.7));
  g.addColorStop(1, grey(0));
  ctx.fillStyle = g;
  ctx.globalCompositeOperation = 'lighten';
  ctx.beginPath();
  ctx.arc(0, 0, rx, 0, Math.PI * 2);
  ctx.fill();
  ctx.restore();
}

function heather(ctx: Ctx, r: () => number) {
  ctx.globalCompositeOperation = 'lighten';
  for (let n = 0; n < 12; n++) {
    const [x, y] = [r() * TUNDRA_TEX, r() * TUNDRA_TEX];
    const size = 3 + r() * 3;
    const lobes = 3 + Math.floor(r() * 3);
    const turn = r() * Math.PI * 2;
    ctx.fillStyle = grey(0.85 + 0.15 * r());
    wrapped(ctx, (ox, oy) => {
      ctx.beginPath();
      for (let k = 0; k < lobes; k++) {
        const a = turn + (k / lobes) * Math.PI * 2;
        ctx.moveTo(x + ox + Math.cos(a) * size * 0.8 + size, y + oy + Math.sin(a) * size * 0.8);
        ctx.arc(x + ox + Math.cos(a) * size * 0.8, y + oy + Math.sin(a) * size * 0.8, size, 0, Math.PI * 2);
      }
      ctx.fill();
    });
  }
}

function grassShade(ctx: Ctx, r: () => number) {
  for (let n = 0; n < 9; n++) {
    const [x, y, rx] = [r() * TUNDRA_TEX, r() * TUNDRA_TEX, 26 + r() * 22];
    const [ry, a] = [rx * (0.5 + 0.3 * r()), r() * Math.PI];
    wrapped(ctx, (ox, oy) => blob(ctx, x + ox, y + oy, rx, ry, a, 0.4));
  }
  // Tussocks: three or four short strokes fanning up from a foot, like the map's tufts.
  ctx.globalCompositeOperation = 'lighten';
  ctx.lineCap = 'round';
  for (let n = 0; n < 96; n++) {
    const [x, y] = [r() * TUNDRA_TEX, r() * TUNDRA_TEX];
    const h = 4 + r() * 3;
    const blades = 3 + Math.floor(r() * 2);
    ctx.strokeStyle = grey(0.65 + 0.25 * r());
    ctx.lineWidth = 1.4 + r() * 0.6;
    wrapped(ctx, (ox, oy) => {
      ctx.beginPath();
      for (let k = 0; k < blades; k++) {
        const lean = (k / (blades - 1) - 0.5) * 1.1;
        ctx.moveTo(x + ox + lean * 1.5, y + oy);
        ctx.quadraticCurveTo(x + ox + lean * h * 0.4, y + oy - h * 0.6, x + ox + lean * h * 0.9, y + oy - h);
      }
      ctx.stroke();
    });
  }
}

function grassLight(ctx: Ctx, r: () => number) {
  for (let n = 0; n < 14; n++) {
    const [x, y, rx] = [r() * TUNDRA_TEX, r() * TUNDRA_TEX, 20 + r() * 24];
    const [ry, a] = [rx * (0.45 + 0.3 * r()), r() * Math.PI];
    wrapped(ctx, (ox, oy) => blob(ctx, x + ox, y + oy, rx, ry, a, 0.75));
  }
}

let source: BufferImageSource | null = null;

/** The texture (made on first use; shared by every tile). */
export function tundraSource(): BufferImageSource {
  if (source) return source;
  const [h, gs, gl] = [channel(heather, 11), channel(grassShade, 23), channel(grassLight, 41)];
  const data = new Uint8Array(TUNDRA_TEX * TUNDRA_TEX * 4);
  for (let k = 0; k < TUNDRA_TEX * TUNDRA_TEX; k++) {
    data[k * 4] = h[k * 4];
    data[k * 4 + 1] = gs[k * 4];
    data[k * 4 + 3] = gl[k * 4];
  }
  source = new BufferImageSource({
    resource: data,
    width: TUNDRA_TEX,
    height: TUNDRA_TEX,
    format: 'rgba8unorm',
    // Data in all four channels: no premultiplying.
    alphaMode: 'no-premultiply-alpha',
    scaleMode: 'linear',
    addressMode: 'repeat',
    autoGenerateMipmaps: true,
  });
  return source;
}
