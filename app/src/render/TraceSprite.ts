// A trace picture over the map (session-only, the DM's view): an `Underlay`'s image placed by its
// transform, and its Adjust handles (a frame, a corner square each, the round turn knob).
import { Container, Graphics, Sprite, Texture } from 'pixi.js';
import { HANDLE_PX, type Underlay } from '../editor/trace/underlay.svelte';
import type { Camera } from './camera';

export class TraceSprite {
  readonly container = new Container();
  private readonly sprite = new Sprite();
  private readonly g = new Graphics();
  private image: unknown = null;
  private key = '';

  constructor() {
    this.sprite.anchor.set(0.5);
    this.sprite.visible = false;
    this.container.addChild(this.sprite, this.g);
  }

  /** Placed for the view from `u` (null: nothing shown); cheap when nothing changed. */
  update(cam: Camera, u: Underlay | null) {
    const xf = u?.xf;
    const key = u && xf ? `${cam.cx},${cam.cy},${cam.zoom},${cam.width},${cam.height}|${u.visible}|${u.adjust}|${u.opacity}|${xf.cx},${xf.cy},${xf.ftPerPx},${xf.rot}` : '';
    const im = u?.visible ? u.image : null;
    if (key === this.key && (im?.bitmap ?? null) === this.image) return;
    this.key = key;
    if ((im?.bitmap ?? null) !== this.image) {
      const old = this.sprite.texture;
      this.sprite.texture = im ? Texture.from(im.bitmap) : Texture.EMPTY;
      if (old && old !== Texture.EMPTY) old.destroy(true);
      this.image = im?.bitmap ?? null;
    }
    this.sprite.visible = !!im;
    this.g.clear();
    if (!im || !u || !xf) return;
    const [x, y] = cam.worldToScreen(xf.cx, xf.cy);
    this.sprite.position.set(x, y);
    this.sprite.scale.set(xf.ftPerPx * cam.ppf);
    this.sprite.rotation = (xf.rot * Math.PI) / 180;
    this.sprite.alpha = u.opacity;
    if (!u.adjust) return;
    const hs = u.handles(cam.ppf);
    if (!hs) return;
    const g = this.g;
    const c = hs.corners.map((p) => cam.worldToScreen(p[0], p[1]));
    g.poly(c.flat()).stroke({ width: 1.5, color: 0x1a5fb4, alpha: 0.9 });
    const t = cam.worldToScreen(hs.top[0], hs.top[1]);
    const r = cam.worldToScreen(hs.rotate[0], hs.rotate[1]);
    g.moveTo(t[0], t[1]).lineTo(r[0], r[1]).stroke({ width: 1.5, color: 0x1a5fb4, alpha: 0.9 });
    g.circle(r[0], r[1], HANDLE_PX).fill(0xffffff).stroke({ width: 2, color: 0x1a5fb4 });
    for (const [cx, cy] of c) g.rect(cx - HANDLE_PX / 2, cy - HANDLE_PX / 2, HANDLE_PX, HANDLE_PX).fill(0xffffff).stroke({ width: 2, color: 0x1a5fb4 });
  }

  /** The picture shown is this bitmap (or none). */
  shows(bitmap: unknown): boolean {
    return this.image === bitmap;
  }

  destroy() {
    if (this.sprite.texture !== Texture.EMPTY) this.sprite.texture.destroy(true);
    this.container.destroy({ children: true });
  }
}
