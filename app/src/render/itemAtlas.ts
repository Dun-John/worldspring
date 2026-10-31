// Atlas for building furniture and underground props, filled as items are first seen: each look
// (a key naming everything its drawing depends on) is drawn once with InteriorLayer's own drawing
// code into a mipmapped page, then shown as sprites. Pages are kept per renderer for the session.
import { Container, Graphics, Matrix, Rectangle, RenderTexture, Texture, type Renderer } from 'pixi.js';

/** Atlas resolution: pixels per 5-ft square (as the battlemap atlas). */
export const ITEM_PX = 64;
const PAGE = 2048;
/** Mip levels kept, and the empty gap round every frame on a grid of the same size, so that no
 * level blends two frames. */
const MIP_LEVELS = 4;
const PAD = 1 << (MIP_LEVELS - 1);

/** A drawn look: its texture, and where its top-left sits relative to the item's origin, in squares. */
export interface ItemFrame {
  texture: Texture;
  x: number;
  y: number;
}

interface Page {
  rt: RenderTexture;
  x: number;
  y: number;
  rowH: number;
}

const atlases = new WeakMap<Renderer, ItemAtlas>();

/** Arguments of Graphics' path methods that are lengths (scaled), by method; the rest (angles, flags)
 * pass through. */
const LENGTHS: Record<string, number> = { moveTo: 2, lineTo: 2, quadraticCurveTo: 4, bezierCurveTo: 6, arc: 3, arcTo: 5, circle: 3, ellipse: 4, rect: 4, roundRect: 5 };

/** `g` seen through a scale: drawings given in squares are built in pixels, so Pixi tessellates circles
 * and curves for the size they are shown at (built in squares, a small circle gets eight points). */
function inPixels(g: Graphics, k: number): Graphics {
  const proxy: Graphics = new Proxy(g, {
    get(target, prop, receiver) {
      const v = Reflect.get(target, prop, receiver);
      if (typeof v !== 'function') return v;
      const name = String(prop);
      return (...args: unknown[]) => {
        let a = args;
        if (name in LENGTHS) a = args.map((x, i) => (i < LENGTHS[name] && typeof x === 'number' ? x * k : x));
        else if (name === 'poly') a = [(args[0] as number[]).map((x) => x * k), ...args.slice(1)];
        else if (name === 'stroke' && args[0] && typeof args[0] === 'object') {
          const st = args[0] as { width?: number };
          a = [{ ...st, width: (st.width ?? 1) * k }, ...args.slice(1)];
        }
        const out = (v as (...x: unknown[]) => unknown).apply(target, a);
        return out === target ? proxy : out;
      };
    },
  });
  return proxy;
}

export class ItemAtlas {
  private readonly pages: Page[] = [];
  private readonly frames = new Map<string, ItemFrame>();
  /** Looks drawn but not yet rendered into their page (see `flush`). */
  private pending: { page: Page; g: Graphics }[] = [];

  private constructor(private readonly renderer: Renderer) {}

  static for(renderer: Renderer): ItemAtlas {
    let a = atlases.get(renderer);
    if (!a) atlases.set(renderer, (a = new ItemAtlas(renderer)));
    return a;
  }

  /** Forget the atlas: its pages live only on the GPU, so a lost context leaves them blank. */
  static drop(renderer: Renderer) {
    atlases.delete(renderer);
  }

  /** The frame for `key`, drawing it with `draw` (in squares, with its origin at `origin`) the first
   * time. Call `flush` before the frames are shown. */
  get(key: string, draw: (g: Graphics) => void, origin: [number, number] = [0, 0]): ItemFrame | null {
    const hit = this.frames.get(key);
    if (hit) return hit;
    const g = new Graphics();
    draw(inPixels(g, ITEM_PX));
    const b = g.getLocalBounds();
    if (b.width <= 0 || b.height <= 0) {
      g.destroy();
      return null;
    }
    const [bx, by] = [Math.floor(b.x) - 2, Math.floor(b.y) - 2];
    const [bw, bh] = [Math.ceil(b.width) + 4, Math.ceil(b.height) + 4];
    if (bw > PAGE - PAD || bh > PAGE - PAD) {
      g.destroy();
      return null;
    }
    const [page, px, py] = this.place(bw, bh);
    g.position.set(px - bx, py - by);
    this.pending.push({ page, g });
    const frame = { texture: new Texture({ source: page.rt.source, frame: new Rectangle(px, py, bw, bh) }), x: bx / ITEM_PX - origin[0], y: by / ITEM_PX - origin[1] };
    this.frames.set(key, frame);
    return frame;
  }

  /** Renders the looks drawn since the last flush into their pages and updates those pages' mipmaps. */
  flush() {
    if (!this.pending.length) return;
    const byPage = new Map<Page, Graphics[]>();
    for (const { page, g } of this.pending) {
      const list = byPage.get(page);
      if (list) list.push(g);
      else byPage.set(page, [g]);
    }
    this.pending = [];
    for (const [page, list] of byPage) {
      const root = new Container();
      root.addChild(...list);
      this.renderer.render({ container: root, target: page.rt, clear: false, transform: new Matrix() });
      page.rt.source.updateMipmaps();
      root.destroy({ children: true });
    }
  }

  /** Room for a `w`×`h` frame: on the PAD grid, with PAD empty pixels after it. */
  private place(w: number, h: number): [Page, number, number] {
    const sw = Math.ceil(w / PAD) * PAD + PAD;
    const sh = Math.ceil(h / PAD) * PAD + PAD;
    let page = this.pages[this.pages.length - 1];
    if (page && page.x + sw > PAGE) {
      page.x = 0;
      page.y += page.rowH;
      page.rowH = 0;
    }
    if (!page || page.y + sh > PAGE) {
      const rt = RenderTexture.create({ width: PAGE, height: PAGE, resolution: 1, autoGenerateMipmaps: true, mipLevelCount: MIP_LEVELS });
      // Start transparent.
      this.renderer.render({ container: new Container(), target: rt, clear: true });
      page = { rt, x: 0, y: 0, rowH: 0 };
      this.pages.push(page);
    }
    const at: [Page, number, number] = [page, page.x, page.y];
    page.x += sw;
    page.rowH = Math.max(page.rowH, sh);
    return at;
  }
}
