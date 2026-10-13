// Thumbnail sheets for the item pickers: many pictures laid out in one container and read back
// once (`extract.canvas`), so a picker of a hundred pictures costs one offscreen render and no PNG
// encoding. Tiles are copied out with `paintThumb` (a `<canvas>` per tile). Sheets are made in idle
// time the first time they are asked for and kept per renderer for the session.
import { Container, Rectangle, type Renderer } from 'pixi.js';

/** A picture to put on a sheet: anything drawn, fitted into its cell by its bounds. */
export interface ThumbTile {
  key: string;
  node: Container;
}

export interface ThumbSheet {
  canvas: HTMLCanvasElement;
  /** Where each tile's picture lies on the sheet: x, y, width, height (px). */
  rects: Map<string, [number, number, number, number]>;
}

/** Lay `tiles` out in `cell`-px cells (each fitted, its shape kept, centred) and render them once.
 * The tiles' nodes are destroyed. */
export function thumbSheet(renderer: Renderer, tiles: ThumbTile[], cell = 96): ThumbSheet {
  const cols = Math.max(1, Math.ceil(Math.sqrt(tiles.length)));
  const rows = Math.max(1, Math.ceil(tiles.length / cols));
  const root = new Container();
  const rects = new Map<string, [number, number, number, number]>();
  const pad = 2;
  tiles.forEach(({ key, node }, i) => {
    const b = node.getLocalBounds();
    if (b.width <= 0 || b.height <= 0) return;
    const k = (cell - pad * 2) / Math.max(b.width, b.height);
    const [w, h] = [b.width * k, b.height * k];
    const [x, y] = [(i % cols) * cell + (cell - w) / 2, Math.floor(i / cols) * cell + (cell - h) / 2];
    node.scale.set(k);
    node.position.set(x - b.x * k, y - b.y * k);
    root.addChild(node);
    rects.set(key, [Math.floor(x), Math.floor(y), Math.ceil(w), Math.ceil(h)]);
  });
  const canvas = renderer.extract.canvas({ target: root, frame: new Rectangle(0, 0, cols * cell, rows * cell), resolution: 1, clearColor: [0, 0, 0, 0] }) as HTMLCanvasElement;
  root.destroy({ children: true });
  return { canvas, rects };
}

const sheets = new WeakMap<Renderer, Map<string, Promise<ThumbSheet>>>();

/** The sheet kept as `key` for this renderer, made by `make` in idle time the first time. */
export function sheetFor(renderer: Renderer, key: string, make: () => ThumbSheet): Promise<ThumbSheet> {
  let byKey = sheets.get(renderer);
  if (!byKey) sheets.set(renderer, (byKey = new Map()));
  let p = byKey.get(key);
  if (!p) {
    p = new Promise<ThumbSheet>((done, fail) => {
      const run = () => {
        try {
          done(make());
        } catch (e) {
          fail(e);
        }
      };
      if ('requestIdleCallback' in window) requestIdleCallback(run, { timeout: 500 });
      else setTimeout(run, 0);
    });
    byKey.set(key, p);
    // A failed sheet is asked for again next time.
    p.catch(() => byKey.delete(key));
  }
  return p;
}

/** Copy a tile from its sheet into `canvas` (cleared), fitted and centred. False if the sheet has
 * no such tile. */
export function paintThumb(canvas: HTMLCanvasElement, sheet: ThumbSheet, key: string): boolean {
  const ctx = canvas.getContext('2d');
  const r = sheet.rects.get(key);
  if (!ctx) return false;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (!r) return false;
  const [x, y, w, h] = r;
  const k = Math.min(canvas.width / w, canvas.height / h);
  const [dw, dh] = [w * k, h * k];
  ctx.drawImage(sheet.canvas, x, y, w, h, (canvas.width - dw) / 2, (canvas.height - dh) / 2, dw, dh);
  return true;
}
