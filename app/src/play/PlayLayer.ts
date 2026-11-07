// Play mode on the map: tokens and shapes over the battlemap or interior, then fog of war and
// what the characters can see (darkness with the known and seen squares erased from it, drawn
// into a screen-sized texture), the ruler, pings and the fog brush on top.
//
// Everything sits in a location's grid (squares); each item is placed relative to the camera
// every frame, so nothing is drawn at huge world coordinates.
import { BufferImageSource, Container, Graphics, RenderTexture, Sprite, Text, TextStyle, Texture } from 'pixi.js';
import type { MapView } from '../render/MapView';
import { getImage } from './session';
import { FOG_N, roleOf, settingsOf, type GridSettings, type Shape, type Token } from './state';
import { bitAt, formatDistance, shapeLabel, shapeOutline, type Bitmap } from './vision';
import type { PlayController } from './controller';

/** Local drawing units per square (tokens, shapes, rulers). */
const TPX = 64;
const INK = 0x1d1a14;
const SELECT = 0x38bdf8;

interface TokenView {
  c: Container;
  g: Graphics;
  face: Container;
  label: Text;
  key: string;
  /** What it was last drawn from (the token object is replaced whenever it changes). */
  tok: Token | null;
  sel: boolean;
  img: Texture | null;
  grid: GridSettings | null;
  /** Where it is drawn (squares, in the grid of `loc`): eases toward where it is. */
  x: number;
  y: number;
  loc: string;
}

interface FogChunk {
  loc: string;
  cx: number;
  cy: number;
  src: BufferImageSource;
  tex: Texture;
  sprite: Sprite;
  rgba: Uint8Array;
}

const labelStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 13, fontWeight: 'bold', fill: '#f8f1de', stroke: { color: '#1d1a14', width: 3, join: 'round' } });
const shapeStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 13, fontWeight: 'bold', fill: '#fff7d6', stroke: { color: '#1d1a14', width: 3, join: 'round' } });
const initialsStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 30, fontWeight: 'bold', fill: '#ffffff', stroke: { color: '#1d1a14', width: 4, join: 'round' } });
const rulerStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 15, fontWeight: 'bold', fill: '#fff7d6', stroke: { color: '#1d1a14', width: 4, join: 'round' }, align: 'left', lineHeight: 21 });

/** Luminance of a 0xRRGGBB colour (0..1). */
const lum = (c: number) => (0.3 * ((c >> 16) & 255) + 0.59 * ((c >> 8) & 255) + 0.11 * (c & 255)) / 255;

export class PlayLayer {
  readonly under = new Container();
  readonly over = new Container();
  private readonly shapes = new Container();
  private readonly shapeLabels = new Container();
  private readonly tokens = new Container();
  private readonly tokenLabels = new Container();
  private readonly tokenViews = new Map<string, TokenView>();
  private readonly shapeViews = new Map<string, { g: Graphics; label: Text; shape: Shape; grid: GridSettings }>();
  /** Pings, the brush or a selection box were drawn last frame. */
  private marksShown = false;
  /** What the fog texture was last drawn for (it is drawn again only when any of it changes). */
  private fogDrawn = { cx: NaN, cy: NaN, zoom: NaN, w: 0, h: 0, fog: false, los: false, loc: '', rotation: 0, version: -1, player: false };
  /** Bumped whenever a fog chunk or what is seen changes. */
  private fogVersion = 0;
  private readonly images = new Map<string, Texture | null>();
  // Fog: darkness, revealed chunks and what is seen now erased from it, rendered into `fogRT`.
  private readonly fogScene = new Container();
  private readonly fogBg = new Sprite(Texture.WHITE);
  private readonly fogChunks = new Map<string, FogChunk>();
  private readonly fogDirty = new Set<string>();
  private vision: { b: Bitmap; loc: string; src: BufferImageSource; tex: Texture; sprite: Sprite } | null = null;
  private fogRT: RenderTexture | null = null;
  private readonly fogSprite = new Sprite();
  private readonly ruler = new Graphics();
  private readonly rulerText = new Text({ text: '', style: rulerStyle, anchor: { x: 0, y: 1 }, resolution: 2 });
  private rulerKey = '';
  private readonly draft = new Graphics();
  private draftKey = '';
  private readonly marks = new Graphics();
  private lastFrame = performance.now();

  constructor(
    private readonly view: MapView,
    private readonly ctl: PlayController,
  ) {
    this.under.addChild(this.shapes, this.tokens, this.shapeLabels, this.tokenLabels);
    this.fogBg.tint = 0x000000;
    this.fogScene.addChild(this.fogBg);
    this.over.addChild(this.fogSprite, this.ruler, this.draft, this.rulerText, this.marks);
    view.playUnder.addChild(this.under);
    view.playOver.addChild(this.over);
  }

  /** Fog chunks changed (`loc|cx,cy`). */
  fogChanged(keys: string[]) {
    for (const k of keys) this.fogDirty.add(k);
  }

  /** A token's picture is new or changed. */
  imageChanged(id: string) {
    this.images.delete(id);
  }

  destroy() {
    this.under.destroy({ children: true });
    this.over.destroy({ children: true });
    for (const f of this.fogChunks.values()) f.tex.destroy(true);
    this.vision?.tex.destroy(true);
    this.fogRT?.destroy(true);
    this.fogScene.destroy({ children: true });
  }

  update(now: number) {
    const dt = Math.min(100, now - this.lastFrame);
    this.lastFrame = now;
    const ctl = this.ctl;
    const cam = this.view.cam;
    const place = ctl.place;
    const shown = ctl.on && !!place;
    this.under.visible = this.over.visible = shown;
    if (!shown || !place) return;
    const loc = place.loc;
    const player = ctl.role === 'player';
    const s = ctl.state;
    const settings = settingsOf(s, loc);
    const pxPerSq = 5 * cam.ppf;
    const scale = pxPerSq / TPX;
    const at = (gx: number, gy: number) => {
      const [wx, wy] = place.toWorld(gx, gy);
      return cam.worldToScreen(wx, wy);
    };
    const vision = ctl.vision && ctl.vision.loc === loc ? ctl.vision : null;
    const seen = (t: Token) => !vision || bitAt(vision, Math.floor(t.x), Math.floor(t.y));

    // Shapes, each with its size.
    const liveShapes = new Set<string>();
    for (const sh of Object.values(s.shapes)) {
      if (sh.loc !== loc) continue;
      liveShapes.add(sh.id);
      let v = this.shapeViews.get(sh.id);
      // (Shapes and the grid settings are replaced, not changed, when edited.)
      if (!v || v.shape !== sh || v.grid !== s.grid) {
        v?.g.destroy();
        v?.label.destroy();
        const g = drawShape(sh);
        const label = new Text({ text: shapeLabel(sh, s.grid), style: shapeStyle, anchor: 0.5, resolution: 2 });
        this.shapes.addChild(g);
        this.shapeLabels.addChild(label);
        v = { g, label, shape: sh, grid: s.grid };
        this.shapeViews.set(sh.id, v);
      }
      const [sx, sy] = at(sh.x, sh.y);
      v.g.position.set(sx, sy);
      v.g.rotation = place.rotation;
      v.g.scale.set(scale);
      const [lx, ly] = at(...labelSpot(sh));
      v.label.position.set(lx, ly);
      v.label.visible = pxPerSq >= 6;
    }
    for (const [id, v] of this.shapeViews) {
      if (liveShapes.has(id)) continue;
      v.g.destroy();
      v.label.destroy();
      this.shapeViews.delete(id);
    }

    // Tokens: here; outside the building in view; the DM's markers of those inside buildings.
    const live = new Set<string>();
    const ease = 1 - Math.exp(-dt / 70);
    for (const { t, place: tp, where } of ctl.tokensInView()) {
      if (player && where === 'here' && (t.hidden || (s.los && !t.vision && !seen(t)))) continue;
      live.add(t.id);
      let v = this.tokenViews.get(t.id);
      if (!v) {
        const c = new Container();
        const g = new Graphics();
        const face = new Container();
        c.addChild(g, face);
        const label = new Text({ text: '', style: labelStyle, anchor: { x: 0.5, y: 0 }, resolution: 2 });
        this.tokens.addChild(c);
        this.tokenLabels.addChild(label);
        v = { c, g, face, label, key: '', tok: null, sel: false, img: null, grid: null, x: t.x, y: t.y, loc: t.loc };
        this.tokenViews.set(t.id, v);
      }
      // Into or out of a building: no glide across grids.
      if (v.loc !== t.loc) [v.x, v.y, v.loc] = [t.x, t.y, t.loc];
      // Drawn again only when the token, its selection, its picture or the grid's scale change
      // (and then only if its look did).
      const sel = ctl.selection.has(t.id);
      const img = this.image(t.image);
      if (v.tok !== t || v.sel !== sel || v.img !== img || v.grid !== s.grid) {
        [v.tok, v.sel, v.img, v.grid] = [t, sel, img, s.grid];
        const key = tokenKey(t, sel, player, img, s.grid);
        if (v.key !== key) {
          v.key = key;
          this.drawToken(v, t, player);
        }
      }
      // The DM's own drags show where the pointer is; others glide.
      if (ctl.dragging.has(t.id)) [v.x, v.y] = [t.x, t.y];
      else [v.x, v.y] = [v.x + (t.x - v.x) * ease, v.y + (t.y - v.y) * ease];
      const [wx, wy] = tp.toWorld(v.x, v.y);
      const [sx, sy] = cam.worldToScreen(wx, wy);
      v.c.position.set(sx, sy);
      v.c.scale.set(scale);
      // Inside a building seen from outside: the DM's faint marker on the roof.
      v.c.alpha = where === 'inside' && !ctl.dragging.has(t.id) ? 0.4 : t.hidden ? 0.55 : 1;
      v.label.position.set(sx, sy + (t.size * pxPerSq) / 2 + 2);
      v.label.visible = pxPerSq >= 22 && !!v.label.text && where !== 'inside';
      v.c.visible = pxPerSq >= 2;
    }
    for (const [id, v] of this.tokenViews) {
      if (live.has(id)) continue;
      v.c.destroy({ children: true });
      v.label.destroy();
      this.tokenViews.delete(id);
    }

    // Fog of war and sight.
    this.updateFog(loc, settings.fog, s.los, player, at, place.rotation, pxPerSq);

    // The ruler (the DM's, or as the DM is measuring, for the players).
    const r = ctl.ruler && ctl.ruler.loc === loc ? ctl.ruler : null;
    // (A map ruler is redrawn every frame; its key only marks it as there, so it is cleared once gone.)
    const rk = !r ? '' : r.free ? 'free' : JSON.stringify([r.a, r.b, s.grid]);
    if (rk !== this.rulerKey || r?.free) {
      this.rulerKey = rk;
      this.ruler.clear();
      if (r?.free) {
        // A straight line between two points, drawn on screen (it may span the continent).
        const [ax, ay] = at(r.a[0], r.a[1]);
        const [bx, by] = at(r.b[0], r.b[1]);
        this.ruler.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 5, color: INK, cap: 'round' });
        this.ruler.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 2.5, color: 0xfacc15, cap: 'round' });
        this.ruler.circle(ax, ay, 5).fill(0xfacc15).stroke({ width: 2, color: INK });
        this.ruler.circle(bx, by, 5).fill(0xfacc15).stroke({ width: 2, color: INK });
      } else if (r) {
        const m = ctl.measureRuler(r);
        const [ox, oy] = r.a;
        for (const [i, j] of m.squares) this.ruler.rect((i - ox) * TPX, (j - oy) * TPX, TPX, TPX);
        this.ruler.fill({ color: 0xfacc15, alpha: 0.22 });
        const [ex, ey] = [(r.b[0] - ox) * TPX, (r.b[1] - oy) * TPX];
        this.ruler.moveTo(TPX / 2, TPX / 2).lineTo(ex + TPX / 2, ey + TPX / 2).stroke({ width: 9, color: INK, cap: 'round' });
        this.ruler.moveTo(TPX / 2, TPX / 2).lineTo(ex + TPX / 2, ey + TPX / 2).stroke({ width: 5, color: 0xfacc15, cap: 'round' });
        this.ruler.circle(TPX / 2, TPX / 2, 9).fill(0xfacc15).stroke({ width: 3, color: INK });
        this.ruler.circle(ex + TPX / 2, ey + TPX / 2, 9).fill(0xfacc15).stroke({ width: 3, color: INK });
      }
    }
    if (r) {
      // (Read every frame: the ground under the ends may load after the ruler is drawn.)
      const text = ctl.rulerText(r);
      if (text !== this.rulerText.text) this.rulerText.text = text;
      if (r.free) {
        this.ruler.position.set(0, 0);
        this.ruler.rotation = 0;
        this.ruler.scale.set(1);
      } else {
        const [sx, sy] = at(r.a[0], r.a[1]);
        this.ruler.position.set(sx, sy);
        this.ruler.rotation = place.rotation;
        this.ruler.scale.set(scale);
      }
      const [tx, ty] = r.free ? at(r.b[0], r.b[1]) : at(r.b[0] + 0.5, r.b[1] + 0.5);
      this.rulerText.position.set(tx + 12, ty - 10);
    } else if (!ctl.draftShape && this.rulerText.text) {
      this.rulerText.text = '';
    }

    // The shape being dragged out, and its size.
    const d = ctl.draftShape;
    const dk = d ? JSON.stringify(d) : '';
    if (dk !== this.draftKey) {
      this.draftKey = dk;
      this.draft.clear();
      if (d) drawShape(d, this.draft);
      if (!r) this.rulerText.text = d ? shapeLabel(d, s.grid) : '';
    }
    if (d) {
      const [sx, sy] = at(d.x, d.y);
      this.draft.position.set(sx, sy);
      this.draft.rotation = place.rotation;
      this.draft.scale.set(scale);
      if (!r) {
        const [tx, ty] = at(d.x + d.dx, d.y + d.dy);
        this.rulerText.position.set(tx + 12, ty - 10);
      }
    }

    // Pings, the fog brush, the selection box: screen space, drawn fresh while there are any.
    const marks = ctl.pings.length > 0 || !!ctl.box || !!ctl.brush;
    if (!marks && !this.marksShown) return;
    this.marksShown = marks;
    this.marks.clear();
    for (const p of ctl.pings) {
      if (p.loc !== loc) continue;
      const age = (now - p.t) / 1600;
      if (age > 1) continue;
      const [sx, sy] = at(p.x, p.y);
      for (const k of [0, 0.33]) {
        const a = (age + k) % 1;
        this.marks.circle(sx, sy, 8 + a * 46).stroke({ width: 4, color: p.color, alpha: 1 - a });
      }
      this.marks.circle(sx, sy, 6).fill({ color: p.color, alpha: 1 - age });
    }
    // The selection box being dragged out.
    if (ctl.box) {
      const [ax, ay] = cam.worldToScreen(ctl.box[0], ctl.box[1]);
      const [bx, by] = cam.worldToScreen(ctl.box[2], ctl.box[3]);
      this.marks.rect(Math.min(ax, bx), Math.min(ay, by), Math.abs(bx - ax), Math.abs(by - ay)).fill({ color: SELECT, alpha: 0.12 }).stroke({ width: 1.5, color: SELECT, alpha: 0.9 });
    }
    const brush = ctl.brush;
    if (brush) {
      const [sx, sy] = at(brush.x, brush.y);
      this.marks.circle(sx, sy, brush.r * pxPerSq).stroke({ width: 2, color: brush.reveal ? 0xfef3c7 : 0x1d1a14, alpha: 0.9 });
      this.marks.circle(sx, sy, brush.r * pxPerSq).stroke({ width: 1, color: brush.reveal ? 0x1d1a14 : 0xfef3c7, alpha: 0.6 });
    }
  }

  /** A token's picture as a round texture (loaded once; null while loading or missing). */
  private image(id: string | undefined): Texture | null {
    if (!id) return null;
    if (this.images.has(id)) return this.images.get(id) ?? null;
    this.images.set(id, null);
    void getImage(id).then(async (blob) => {
      if (!blob) return;
      const bmp = await createImageBitmap(blob);
      const n = 128;
      const cv = document.createElement('canvas');
      cv.width = cv.height = n;
      const cx = cv.getContext('2d')!;
      cx.beginPath();
      cx.arc(n / 2, n / 2, n / 2, 0, Math.PI * 2);
      cx.clip();
      const k = Math.max(n / bmp.width, n / bmp.height);
      cx.drawImage(bmp, (n - bmp.width * k) / 2, (n - bmp.height * k) / 2, bmp.width * k, bmp.height * k);
      bmp.close();
      this.images.set(id, Texture.from(cv));
    });
    return null;
  }

  private drawToken(v: TokenView, t: Token, player: boolean) {
    const g = v.g;
    g.clear();
    v.face.removeChildren().forEach((c) => c.destroy());
    const side = Math.max(0.5, t.size);
    const r = (side * TPX) / 2 - (side < 1 ? 2 : 4);
    // Objects are rounded squares, people discs.
    const obj = t.kind === 'character' && roleOf(t) === 'object';
    const body = (rad: number, ox = 0, oy = 0) => (obj ? g.roundRect(ox - rad, oy - rad, rad * 2, rad * 2, rad * 0.3) : g.circle(ox, oy, rad));
    // Its light's reach, for the DM.
    if (!player && t.light) g.circle(0, 0, t.light * TPX).stroke({ width: 3, color: 0xffd36b, alpha: 0.5 });
    if (t.kind === 'light') {
      g.circle(0, 0, r * 0.75).fill({ color: 0xffb02e, alpha: 0.35 });
      g.moveTo(0, -r * 0.55).quadraticCurveTo(r * 0.42, -r * 0.05, 0, r * 0.4).quadraticCurveTo(-r * 0.42, -r * 0.05, 0, -r * 0.55).fill(0xffd36b).stroke({ width: 3, color: 0x7c2d12 });
    } else {
      // Drop shadow, body, picture or initials, rim (pale for those the players see by).
      body(r, 5, 5).fill({ color: 0x000000, alpha: 0.35 });
      body(r).fill(t.color);
      const tex = this.image(t.image);
      if (tex) {
        const sp = new Sprite(tex);
        sp.anchor.set(0.5);
        sp.width = sp.height = r * 2 - 6;
        v.face.addChild(sp);
      } else {
        const initials = t.name
          .split(/\s+/)
          .filter(Boolean)
          .slice(0, 2)
          .map((w) => w[0]!.toUpperCase())
          .join('');
        const txt = new Text({ text: initials || '?', style: initialsStyle, anchor: 0.5, resolution: 2 });
        txt.style.fill = lum(t.color) > 0.6 ? '#1d1a14' : '#ffffff';
        txt.scale.set(Math.min(1.6, (r * 1.1) / 30));
        v.face.addChild(txt);
      }
      body(r).stroke({ width: 4, color: t.vision ? 0xf5ecd6 : INK });
    }
    if (t.hidden && !player) body(r + 6).stroke({ width: 3, color: 0x9333ea, alpha: 0.9 });
    if (v.key.includes('|sel|')) body(r + 10).stroke({ width: 5, color: SELECT });
    v.label.text = (t.kind === 'light' && player) || obj ? '' : t.name;
  }

  /**
   * Fog of war: what the players don't know is black. Line of sight: what the characters
   * don't see now is greyed, unless they have seen it before (under fog, that is dimmed).
   */
  private updateFog(loc: string, fog: boolean, los: boolean, player: boolean, at: (gx: number, gy: number) => [number, number], rotation: number, pxPerSq: number) {
    const cam = this.view.cam;
    const vision = this.ctl.vision && this.ctl.vision.loc === loc ? this.ctl.vision : null;
    const active = fog || (los && !!vision);
    this.fogSprite.visible = active;
    // Upload changed chunks (even when not shown: the location may come back).
    for (const key of this.fogDirty) this.uploadChunk(key);
    this.fogDirty.clear();
    if (!active) return;
    const w = Math.max(1, Math.ceil(cam.width));
    const h = Math.max(1, Math.ceil(cam.height));
    this.syncVision(vision);
    // Unchanged since it was last drawn (the view still, nothing revealed or seen anew): keep it.
    const d = this.fogDrawn;
    if (d.cx === cam.cx && d.cy === cam.cy && d.zoom === cam.zoom && d.w === w && d.h === h && d.fog === fog && d.los === los && d.loc === loc && d.rotation === rotation && d.version === this.fogVersion && d.player === player) return;
    Object.assign(d, { cx: cam.cx, cy: cam.cy, zoom: cam.zoom, w, h, fog, los, loc, rotation, version: this.fogVersion, player });
    if (!this.fogRT || this.fogRT.width !== w || this.fogRT.height !== h) {
      this.fogRT?.destroy(true);
      // Half resolution: fog is soft anyway, and this is a full-screen pass every frame.
      this.fogRT = RenderTexture.create({ width: w, height: h, resolution: 0.5 });
      this.fogSprite.texture = this.fogRT;
    }
    this.fogBg.width = w;
    this.fogBg.height = h;
    // Unseen and never revealed: black under fog, dimmed when only sight limits the view.
    this.fogBg.alpha = fog ? 1 : 0.6;
    for (const f of this.fogChunks.values()) {
      const on = (fog || los) && f.loc === loc;
      f.sprite.visible = on;
      if (!on) continue;
      const [sx, sy] = at(f.cx * FOG_N, f.cy * FOG_N);
      f.sprite.position.set(sx, sy);
      f.sprite.rotation = rotation;
      f.sprite.scale.set(pxPerSq);
      // Known but not in sight now: dimmed under fog; with sight alone, clear.
      f.sprite.alpha = fog && los && vision ? 0.55 : 1;
    }
    if (this.vision) {
      const v = this.vision;
      v.sprite.visible = los;
      const [sx, sy] = at(v.b.x0, v.b.y0);
      v.sprite.position.set(sx, sy);
      v.sprite.rotation = rotation;
      v.sprite.scale.set(pxPerSq);
    }
    this.view.app.renderer.render({ container: this.fogScene, target: this.fogRT, clear: true, clearColor: [0, 0, 0, 0] });
    this.fogSprite.position.set(0, 0);
    this.fogSprite.width = w;
    this.fogSprite.height = h;
    // The DM sees through it.
    this.fogSprite.alpha = player ? 1 : 0.42;
    this.fogSprite.tint = player ? 0xffffff : 0x1a1030;
  }

  private uploadChunk(key: string) {
    const [loc, xy] = key.split('|');
    const [cx, cy] = xy.split(',').map(Number);
    const bits = this.ctl.state.fog[loc]?.[xy];
    let f = this.fogChunks.get(key);
    this.fogVersion++;
    if (!bits) {
      if (f) {
        f.sprite.destroy();
        f.tex.destroy(true);
        this.fogChunks.delete(key);
      }
      return;
    }
    if (!f) {
      const rgba = new Uint8Array(FOG_N * FOG_N * 4);
      const src = new BufferImageSource({ resource: rgba, width: FOG_N, height: FOG_N, format: 'rgba8unorm', alphaMode: 'premultiplied-alpha', scaleMode: 'linear', addressMode: 'clamp-to-edge' });
      const tex = new Texture({ source: src });
      const sprite = new Sprite(tex);
      sprite.blendMode = 'erase';
      this.fogScene.addChildAt(sprite, 1);
      f = { loc, cx, cy, src, tex, sprite, rgba };
      this.fogChunks.set(key, f);
    }
    for (let k = 0; k < FOG_N * FOG_N; k++) f.rgba[k * 4] = f.rgba[k * 4 + 1] = f.rgba[k * 4 + 2] = f.rgba[k * 4 + 3] = bits[k];
    f.src.update();
  }

  private syncVision(v: (Bitmap & { loc: string }) | null) {
    if (this.vision?.b === v || (!this.vision && (!v || !v.w || !v.h))) return;
    this.fogVersion++;
    // The same size as before (a character walking): the texture is filled again in place.
    const old = this.vision;
    if (old && v && v.w === old.b.w && v.h === old.b.h) {
      const rgba = old.src.resource as Uint8Array;
      for (let k = 0; k < v.w * v.h; k++) rgba[k * 4] = rgba[k * 4 + 1] = rgba[k * 4 + 2] = rgba[k * 4 + 3] = v.bits[k] ? 255 : 0;
      old.src.update();
      this.vision = { ...old, b: v, loc: v.loc };
      return;
    }
    if (old) {
      old.sprite.destroy();
      old.tex.destroy(true);
      this.vision = null;
    }
    if (!v || !v.w || !v.h) return;
    const rgba = new Uint8Array(v.w * v.h * 4);
    for (let k = 0; k < v.w * v.h; k++) rgba[k * 4] = rgba[k * 4 + 1] = rgba[k * 4 + 2] = rgba[k * 4 + 3] = v.bits[k] ? 255 : 0;
    const src = new BufferImageSource({ resource: rgba, width: v.w, height: v.h, format: 'rgba8unorm', alphaMode: 'premultiplied-alpha', scaleMode: 'linear', addressMode: 'clamp-to-edge' });
    const tex = new Texture({ source: src });
    const sprite = new Sprite(tex);
    sprite.blendMode = 'erase';
    this.fogScene.addChild(sprite);
    this.vision = { b: v, loc: v.loc, src, tex, sprite };
  }
}

/** What a token's drawing depends on. */
function tokenKey(t: Token, selected: boolean, player: boolean, image: Texture | null, grid: GridSettings): string {
  return [t.name, t.kind, t.kind === 'character' ? roleOf(t) : '', t.size, t.color, t.hidden ? 1 : 0, t.vision ? 1 : 0, t.light ?? 0, image ? t.image : '', grid.scale, selected && !player ? 'sel' : '', ''].join('|');
}

/** A shape: a tinted fill and its outline, from its origin. */
function drawShape(sh: Shape, into?: Graphics): Graphics {
  const g = into ?? new Graphics();
  const o = shapeOutline(sh).map((v) => v * TPX);
  if (sh.kind === 'line') {
    g.moveTo(o[0], o[1]).lineTo(o[2], o[3]).stroke({ width: 10, color: INK, alpha: 0.5, cap: 'round' });
    g.moveTo(o[0], o[1]).lineTo(o[2], o[3]).stroke({ width: 6, color: sh.color, cap: 'round' });
  } else if (o.length >= 6) {
    g.poly(o).fill({ color: sh.color, alpha: 0.22 }).stroke({ width: 4, color: sh.color, alpha: 0.95 });
  }
  g.circle(0, 0, 7).fill(sh.color).stroke({ width: 2, color: INK });
  return g;
}

/** Where a shape's size label goes (squares). */
function labelSpot(sh: Shape): [number, number] {
  if (sh.kind === 'circle') return [sh.x, sh.y];
  return [sh.x + sh.dx / 2, sh.y + sh.dy / 2];
}
