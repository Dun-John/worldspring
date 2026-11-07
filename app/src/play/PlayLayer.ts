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
/** A PC's pin on screen (px, x `tableScale`): PCs drawn smaller on the surface stay this size. */
export const PIN = 24;
/** A pin's point below its middle, as a share of its size (it marks where the PC is). */
const PIN_TIP = 0.8;

interface TokenView {
  id: string;
  c: Container;
  /** Its light's reach (the DM's; not on a pin). */
  reach: Graphics;
  /** A PC's pin point and the mark of one inside somewhere (shown only on a pin). */
  pin: Graphics;
  door: Graphics;
  g: Graphics;
  face: Container;
  /** The players' follow ring, drawn in screen px (redrawn when its `ringKey` changes). */
  ring: Graphics;
  ringKey: number;
  label: Text;
  /** A cream plate behind the label of a pin, and its size; whether the label has the pin style. */
  plate: Graphics;
  pw: number;
  ph: number;
  platedStyle: boolean;
  /** Hashes of its id, name and drawing key (frame keys are numbers, nothing built per frame). */
  idHash: number;
  nameHash: number;
  keyHash: number;
  /** The party it was last merged into (0: none), for the split margin. */
  pg: number;
  key: string;
  /** What it was last drawn from (the token object is replaced whenever it changes). */
  tok: Token | null;
  sel: boolean;
  img: Texture | null;
  grid: GridSettings | null;
  /** Its label is wanted (a name to show, big enough) before overlaps are looked at. */
  want: boolean;
  /** Its role (PCs' labels come before the others'). */
  pc: boolean;
  /** Where it is drawn (squares, in the grid of `loc`): eases toward where it is. */
  x: number;
  y: number;
  loc: string;
  /** Drawn as a pin this frame (`inside`: at the way in to where it is). */
  pinned: boolean;
  inside: boolean;
  /** Where it is this frame: world ft and screen px. */
  ax: number;
  ay: number;
  tx: number;
  ty: number;
  /** The players' view follows it (players' window: a ring). */
  fol: boolean;
}

/** PCs' pins that overlap on screen, drawn as one: the first one's face, how many, the names. */
interface PartyView {
  c: Container;
  g: Graphics;
  face: Container;
  top: Graphics;
  count: Text;
  label: Text;
  plate: Graphics;
  pw: number;
  ph: number;
  key: number;
  ids: string[];
  inside: boolean;
  want: boolean;
  sel: boolean;
  pc: boolean;
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
const countStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 34, fontWeight: 'bold', fill: '#ffffff', stroke: { color: '#1d1a14', width: 5, join: 'round' } });
/** A pin's label, ink on its cream plate. */
const plateStyle = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 13, fontWeight: 'bold', fill: '#1d1a14' });

const strHash = (s: string) => {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return h >>> 0;
};
const mix = (h: number, n: number) => Math.imul(h ^ Math.round(n), 16777619) >>> 0;
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
  /** What the token labels were last sorted out for (overlaps are looked at only when it changes). */
  private labelKey = 0;
  /** The party pins (each two or more PCs' pins) and what they were worked out for. */
  private parties: TokenView[][] = [];
  private partyKey = 0;
  private pgSeq = 0;
  private readonly partyViews: PartyView[] = [];
  private readonly draft = new Graphics();
  private draftKey = '';
  private readonly marks = new Graphics();
  private lastFrame = performance.now();
  /** Screen-space play marks (labels, ruler text and lines, pings) scaled up for a table's TV:
   * the players' window `clamp(innerHeight / 720, 1, 2.5)`, the DM's 1. */
  tableScale = 1;
  /** The scale the label styles were last set for. */
  private styledScale = 1;
  /** The camera's turn the upright parts (pictures, glyphs, text) were last counter-turned for. */
  private turnSet = 0;
  private readonly onResize = () => {
    this.tableScale = Math.max(1, Math.min(2.5, innerHeight / 720));
  };

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
    if (ctl.role === 'player') {
      this.onResize();
      window.addEventListener('resize', this.onResize);
    }
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
    window.removeEventListener('resize', this.onResize);
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
    const ts = this.tableScale;
    if (ts !== this.styledScale) {
      // (Only when the window's size changes: every label is drawn again once.)
      this.styledScale = ts;
      scaleStyles(ts);
      this.rulerKey = '#';
    }
    const s = ctl.state;
    const settings = settingsOf(s, loc);
    const pxPerSq = 5 * cam.ppf;
    const scale = pxPerSq / TPX;
    // The stage may be turned (a site with north up): the upright parts turn back, set only when it changes.
    if (cam.turn !== this.turnSet) {
      this.turnSet = cam.turn;
      this.rulerText.rotation = -cam.turn;
      for (const v of this.shapeViews.values()) v.label.rotation = -cam.turn;
      for (const v of this.tokenViews.values()) this.uprightToken(v);
      for (const pv of this.partyViews) this.uprightParty(pv);
    }
    const at = (gx: number, gy: number) => {
      const [wx, wy] = place.toWorld(gx, gy);
      return cam.worldToScreen(wx, wy);
    };
    const vision = ctl.visionOf(loc);
    // (The players' view in a location with no sight worked out for it: nothing is in sight.)
    const seen = (t: Token) => (vision ? bitAt(vision, Math.floor(t.x), Math.floor(t.y)) : !player);

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
        label.rotation = -this.turnSet;
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
    let lk = mix(mix(1, pxPerSq * 10), ts * 100);
    const surface = loc === 'surface';
    const pinPx = PIN * ts;
    for (const { t, place: tp, where, at: way } of ctl.tokensInView()) {
      if (player && where === 'here' && (t.hidden || (s.los && !t.vision && !seen(t)))) continue;
      live.add(t.id);
      let v = this.tokenViews.get(t.id);
      if (!v) {
        const c = new Container();
        const reach = new Graphics();
        const pin = new Graphics();
        const door = new Graphics();
        const g = new Graphics();
        const face = new Container();
        const ring = new Graphics();
        c.addChild(reach, pin, g, face, ring, door);
        const label = new Text({ text: '', style: labelStyle, anchor: { x: 0.5, y: 0 }, resolution: 2 });
        const plate = new Graphics();
        plate.visible = false;
        this.tokens.addChild(c);
        this.tokenLabels.addChild(plate, label);
        v = { id: t.id, c, reach, pin, door, g, face, ring, ringKey: 0, label, plate, pw: 0, ph: 0, platedStyle: false, idHash: strHash(t.id), nameHash: 0, keyHash: 0, pg: 0, key: '', tok: null, sel: false, img: null, grid: null, want: false, pc: false, x: t.x, y: t.y, loc: t.loc, pinned: false, inside: false, ax: 0, ay: 0, tx: 0, ty: 0, fol: false };
        this.uprightToken(v);
        this.tokenViews.set(t.id, v);
      }
      // Into or out of a building: no glide across grids.
      if (v.loc !== t.loc) [v.x, v.y, v.loc] = [t.x, t.y, t.loc];
      // Drawn again only when the token, its selection, its picture or the grid's scale change
      // (and then only if its look did).
      const sel = ctl.selection.has(t.id);
      const img = this.image(t.image);
      const fol = player && ctl.followedIds.has(t.id);
      if (v.tok !== t || v.sel !== sel || v.img !== img || v.grid !== s.grid || v.fol !== fol) {
        [v.tok, v.sel, v.img, v.grid, v.fol] = [t, sel, img, s.grid, fol];
        v.nameHash = strHash(t.name);
        const key = tokenKey(t, sel, player, img, s.grid);
        if (v.key !== key) {
          v.key = key;
          v.keyHash = strHash(key);
          this.drawToken(v, t, player);
        }
      }
      // The DM's own drags show where the pointer is; others glide.
      if (ctl.dragging.has(t.id)) [v.x, v.y] = [t.x, t.y];
      else [v.x, v.y] = [v.x + (t.x - v.x) * ease, v.y + (t.y - v.y) * ease];
      const [wx, wy] = way ?? tp.toWorld(v.x, v.y);
      const [sx, sy] = cam.worldToScreen(wx, wy);
      // A PC on the surface drawn smaller than a pin, or inside somewhere: a pin, its point
      // where the PC is (or at the way in).
      const side = Math.max(0.5, t.size);
      // (A disc turns into a pin below `pinPx` and back only above a quarter more.)
      v.pinned = surface && t.kind === 'character' && roleOf(t) === 'pc' && (where === 'entrance' || side * pxPerSq < (v.pinned ? pinPx * 1.25 : pinPx));
      v.inside = where === 'entrance';
      [v.ax, v.ay, v.tx, v.ty] = [wx, wy, sx, sy];
      v.pin.visible = v.pinned;
      v.door.visible = v.inside;
      v.reach.visible = !v.pinned;
      if (v.pinned) {
        v.c.position.set(sx, sy - PIN_TIP * pinPx);
        v.c.scale.set(pinPx / (side * TPX));
      } else {
        v.c.position.set(sx, sy);
        v.c.scale.set(scale);
      }
      const rk = v.fol ? (Math.round(ts * 100) * 31 + (v.pinned ? 1 : 0)) * 1009 + Math.round(v.c.scale.x * 100) : 0;
      if (rk !== v.ringKey) {
        v.ringKey = rk;
        this.drawRing(v, t);
      }
      if (v.pinned !== v.platedStyle) {
        v.platedStyle = v.pinned;
        v.label.style = v.pinned ? plateStyle : labelStyle;
      }
      // Inside a building seen from outside: the DM's faint marker on the roof.
      v.c.alpha = where === 'inside' && !ctl.dragging.has(t.id) ? 0.4 : t.hidden ? 0.55 : 1;
      v.label.position.set(sx, v.pinned ? sy + 3 * ts : sy + (t.size * pxPerSq) / 2 + 2 * ts);
      v.want = (pxPerSq >= 22 || v.pinned) && !!v.label.text && where !== 'inside' && (!(t.kind === 'character' && roleOf(t) === 'object') || (!player && sel));
      v.pc = t.kind === 'character' && roleOf(t) === 'pc';
      v.c.visible = pxPerSq >= 2 || v.pinned;
      lk = mix(mix(mix(mix(mix(lk, v.idHash), sx), sy), (v.want ? 1 : 0) | (sel ? 2 : 0) | (v.pc ? 4 : 0) | (v.pinned ? 8 : 0) | (v.c.visible ? 16 : 0)), v.nameHash);
    }
    for (const [id, v] of this.tokenViews) {
      if (live.has(id)) continue;
      v.c.destroy({ children: true });
      v.label.destroy();
      v.plate.destroy();
      this.tokenViews.delete(id);
    }

    // Pins that overlap on screen: one party pin at their middle (who is in it is worked out
    // again only when the zoom or a pin's place changes; panning moves them together).
    let pk = mix(mix(1, Math.log(cam.ppf) * 1000), pinPx);
    for (const v of this.tokenViews.values()) if (v.pinned) pk = mix(mix(mix(pk, v.idHash), v.ax), v.ay);
    if (pk !== this.partyKey) {
      this.partyKey = pk;
      this.parties = partiesOf([...this.tokenViews.values()].filter((v) => v.pinned), pinPx / cam.ppf, () => ++this.pgSeq);
    }
    let nParty = 0;
    for (const members of this.parties) {
      const pv = this.partyView(nParty++);
      let [sx, sy, inside, sel, fol] = [0, 0, true, false, false];
      const first = members[0];
      let key = mix(mix(1, first.keyHash), ts * 100);
      for (const m of members) {
        sx += m.tx;
        sy += m.ty;
        inside &&= m.inside;
        sel ||= m.sel;
        fol ||= m.fol;
        key = mix(mix(key, m.idHash), m.nameHash);
        m.c.visible = false;
        m.want = false;
      }
      [sx, sy] = [sx / members.length, sy / members.length];
      key = mix(key, (inside ? 1 : 0) | (sel ? 2 : 0) | (fol ? 4 : 0));
      if (pv.key !== key) {
        pv.key = key;
        pv.ids = members.map((m) => m.id);
        pv.inside = inside;
        this.drawParty(pv, first, members, inside, sel && !player, fol);
      }
      pv.c.visible = true;
      pv.c.position.set(sx, sy - PIN_TIP * pinPx);
      pv.c.scale.set(pinPx / TPX);
      pv.label.position.set(sx, sy + 3 * ts);
      pv.want = !!pv.label.text;
      pv.sel = sel;
      lk = mix(mix(mix(mix(lk, pv.ids.length), strHash(pv.ids[0])), sx), sy);
    }
    for (let k = nParty; k < this.partyViews.length; k++) {
      const pv = this.partyViews[k];
      if (!pv.c.visible && !pv.want) continue;
      pv.c.visible = pv.label.visible = pv.plate.visible = pv.want = false;
      pv.key = 0;
    }

    // Labels that would overlap: the selected token's first, then PCs', then in draw order.
    if (lk !== this.labelKey) {
      this.labelKey = lk;
      const vs = [...this.tokenViews.values(), ...this.partyViews].filter((v) => v.want);
      for (const v of this.tokenViews.values()) if (!v.want) v.label.visible = v.plate.visible = false;
      for (const v of this.partyViews) if (!v.want) v.label.visible = v.plate.visible = false;
      // The token discs on screen too: a label that would sit on another token hides.
      const discs: { o: unknown; x: number; y: number; w: number; h: number }[] = [];
      for (const v of this.tokenViews.values()) {
        if (!v.c.visible) continue;
        const half = v.pinned ? pinPx / 2 : (Math.max(0.5, v.tok?.size ?? 1) * pxPerSq) / 2;
        discs.push({ o: v, x: v.c.x - half, y: v.c.y - half, w: half * 2, h: half * 2 });
      }
      for (const pv of this.partyViews) if (pv.c.visible) discs.push({ o: pv, x: pv.c.x - pinPx / 2, y: pv.c.y - pinPx / 2, w: pinPx, h: pinPx });
      const over = (a: { x: number; y: number; w: number; h: number }, b: { x: number; y: number; w: number; h: number }) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
      vs.sort((a, b) => Number(b.sel) - Number(a.sel) || Number(b.pc) - Number(a.pc));
      const placed: { x: number; y: number; w: number; h: number }[] = [];
      for (const v of vs) {
        const l = v.label;
        const w = l.width;
        const h = l.height;
        const box = { x: l.x - w / 2, y: l.y, w, h };
        const hit = placed.some((b) => over(box, b)) || (!v.sel && discs.some((d) => d.o !== v && over(box, d)));
        l.visible = !hit;
        if (!hit) placed.push(box);
        this.drawPlate(v, 'pinned' in v ? v.pinned : true);
      }
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
        this.ruler.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 5 * ts, color: INK, cap: 'round' });
        this.ruler.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 2.5 * ts, color: 0xfacc15, cap: 'round' });
        this.ruler.circle(ax, ay, 5 * ts).fill(0xfacc15).stroke({ width: 2 * ts, color: INK });
        this.ruler.circle(bx, by, 5 * ts).fill(0xfacc15).stroke({ width: 2 * ts, color: INK });
      } else if (r) {
        const m = ctl.measureRuler(r);
        const [ox, oy] = r.a;
        for (const [i, j] of m.squares) this.ruler.rect((i - ox) * TPX, (j - oy) * TPX, TPX, TPX);
        this.ruler.fill({ color: 0xfacc15, alpha: 0.22 });
        const [ex, ey] = [(r.b[0] - ox) * TPX, (r.b[1] - oy) * TPX];
        this.ruler.moveTo(TPX / 2, TPX / 2).lineTo(ex + TPX / 2, ey + TPX / 2).stroke({ width: 9 * ts, color: INK, cap: 'round' });
        this.ruler.moveTo(TPX / 2, TPX / 2).lineTo(ex + TPX / 2, ey + TPX / 2).stroke({ width: 5 * ts, color: 0xfacc15, cap: 'round' });
        this.ruler.circle(TPX / 2, TPX / 2, 9 * ts).fill(0xfacc15).stroke({ width: 3 * ts, color: INK });
        this.ruler.circle(ex + TPX / 2, ey + TPX / 2, 9 * ts).fill(0xfacc15).stroke({ width: 3 * ts, color: INK });
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
      this.rulerText.position.set(tx + 12 * ts, ty - 10 * ts);
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
        this.rulerText.position.set(tx + 12 * ts, ty - 10 * ts);
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
        this.marks.circle(sx, sy, (8 + a * 46) * ts).stroke({ width: 4 * ts, color: p.color, alpha: 1 - a });
      }
      this.marks.circle(sx, sy, 6 * ts).fill({ color: p.color, alpha: 1 - age });
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

  /** The PCs of the pin or party pin under a world point (by their size on screen), and
   * whether they are all inside somewhere (their pin is at the way in). */
  pinAt(wx: number, wy: number): { ids: string[]; entrance: boolean } | null {
    if (!this.under.visible) return null;
    const [sx, sy] = this.view.cam.worldToScreen(wx, wy);
    let best: { ids: string[]; entrance: boolean } | null = null;
    let bestD = ((PIN * this.tableScale) / 2 + 3) ** 2;
    for (const pv of this.partyViews) {
      const d = (pv.c.x - sx) ** 2 + (pv.c.y - sy) ** 2;
      if (pv.c.visible && d <= bestD) [best, bestD] = [{ ids: pv.ids, entrance: pv.inside }, d];
    }
    for (const v of this.tokenViews.values()) {
      const d = (v.c.x - sx) ** 2 + (v.c.y - sy) ** 2;
      if (v.pinned && v.c.visible && d <= bestD) [best, bestD] = [{ ids: [v.id], entrance: v.inside }, d];
    }
    return best;
  }

  /** The party pin at `k` in the pool (made the first time so many are shown). */
  private partyView(k: number): PartyView {
    let pv = this.partyViews[k];
    if (pv) return pv;
    const c = new Container();
    const g = new Graphics();
    const face = new Container();
    const top = new Graphics();
    const count = new Text({ text: '', style: countStyle, anchor: 0.5, resolution: 2 });
    c.addChild(g, face, top, count);
    const label = new Text({ text: '', style: plateStyle, anchor: { x: 0.5, y: 0 }, resolution: 2 });
    const plate = new Graphics();
    plate.visible = false;
    this.tokens.addChild(c);
    this.tokenLabels.addChild(plate, label);
    pv = { c, g, face, top, count, label, plate, pw: 0, ph: 0, key: 0, ids: [], inside: false, want: false, sel: false, pc: true };
    this.partyViews[k] = pv;
    this.uprightParty(pv);
    return pv;
  }

  /** A token's picture, glyph and name turned back against the stage's turn (about its own middle). */
  private uprightToken(v: TokenView) {
    const a = -this.turnSet;
    v.g.rotation = v.face.rotation = v.label.rotation = v.plate.rotation = a;
  }

  private uprightParty(pv: PartyView) {
    const a = -this.turnSet;
    pv.face.rotation = pv.count.rotation = pv.label.rotation = pv.plate.rotation = a;
  }

  /** A party pin: the first PC's look on a pin, how many in a badge, their names below. */
  private drawParty(pv: PartyView, first: TokenView, members: TokenView[], inside: boolean, sel: boolean, fol: boolean) {
    const t = first.tok;
    const { g, top } = pv;
    g.clear();
    top.clear();
    pv.face.removeChildren().forEach((c) => c.destroy());
    const r = TPX / 2 - 4;
    const color = t?.color ?? 0x2f5d8a;
    pinPoint(g, r, PIN_TIP * TPX, color);
    g.circle(5, 5, r).fill({ color: 0x000000, alpha: 0.35 });
    g.circle(0, 0, r).fill(color);
    if (first.img) {
      const sp = new Sprite(first.img);
      sp.anchor.set(0.5);
      sp.width = sp.height = r * 2 - 6;
      pv.face.addChild(sp);
    } else {
      const txt = new Text({ text: initialsOf(t?.name ?? ''), style: initialsStyle, anchor: 0.5, resolution: 2 });
      txt.style.fill = lum(color) > 0.6 ? '#1d1a14' : '#ffffff';
      txt.scale.set(Math.min(1.6, (r * 1.1) / 30));
      pv.face.addChild(txt);
    }
    top.circle(0, 0, r).stroke({ width: 4, color: 0xf5ecd6 });
    if (sel) top.circle(0, 0, r + 10).stroke({ width: 5, color: SELECT });
    if (fol) ring(top, r, TPX / (PIN * this.tableScale), false);
    if (inside) doorMark(top, r);
    // How many: a badge at the top right (a pin is small: at least 9 px across x table scale).
    const [bx, by] = [r * 0.78, -r * 0.78];
    top.circle(bx, by, 26).fill(0xb3261e).stroke({ width: 4, color: INK });
    pv.count.text = String(members.length);
    pv.count.position.set(bx, by);
    const who = members.map((m) => m.tok?.name ?? '');
    const names = who.length <= 3 ? who.join(', ') : `${who[0]} +${who.length - 1}`;
    pv.label.text = names.length > 28 ? `${names.slice(0, 27).trimEnd()}…` : names;
  }

  /** A token's screen box [left, top, right, bottom] in the window (the pointer's, if it is not drawn). */
  tokenBox(id: string, cx: number, cy: number): [number, number, number, number] {
    const v = this.tokenViews.get(id);
    if (!v) return [cx, cy, cx, cy];
    const r = this.view.app.canvas.getBoundingClientRect();
    const cam = this.view.cam;
    const half = v.pinned ? (PIN * this.tableScale) / 2 : ((v.tok?.size ?? 1) * 5 * cam.ppf) / 2;
    // (The stage may be turned: the box round the turned square.)
    const [px, py] = cam.toPage(v.c.x, v.c.y);
    const x = r.left + px;
    const y = r.top + py;
    const h = half * (Math.abs(Math.cos(cam.turn)) + Math.abs(Math.sin(cam.turn)));
    return [x - h, y - h, x + h, y + h];
  }

  /** The label's cream plate behind a pin's name (redrawn only when its size changes). */
  private drawPlate(v: { plate: Graphics; label: Text; pw: number; ph: number }, on: boolean) {
    const l = v.label;
    v.plate.visible = on && l.visible;
    if (!v.plate.visible) return;
    const ts = this.tableScale;
    const [w, h] = [Math.ceil(l.width + 10 * ts), Math.ceil(l.height + 2 * ts)];
    if (w !== v.pw || h !== v.ph) {
      [v.pw, v.ph] = [w, h];
      v.plate.clear().roundRect(-w / 2, 0, w, h, 4 * ts).fill(0xf5ecd6).stroke({ width: 2 * ts, color: INK });
    }
    v.plate.position.set(l.x, l.y - ts);
  }

  /** The follow ring of a token (players' window), drawn to be 6 px x table scale on screen. */
  private drawRing(v: TokenView, t: Token) {
    v.ring.clear();
    v.ring.visible = v.fol;
    if (!v.fol) return;
    const side = Math.max(0.5, t.size);
    const r = (side * TPX) / 2 - (side < 1 ? 2 : 4);
    ring(v.ring, r, 1 / Math.max(0.02, v.c.scale.x), t.kind === 'character' && roleOf(t) === 'object');
  }

  private drawToken(v: TokenView, t: Token, player: boolean) {
    const g = v.g;
    g.clear();
    v.reach.clear();
    v.pin.clear();
    v.door.clear();
    v.face.removeChildren().forEach((c) => c.destroy());
    const side = Math.max(0.5, t.size);
    const r = (side * TPX) / 2 - (side < 1 ? 2 : 4);
    // Objects are rounded squares, people discs.
    const obj = t.kind === 'character' && roleOf(t) === 'object';
    const body = (rad: number, ox = 0, oy = 0) => (obj ? g.roundRect(ox - rad, oy - rad, rad * 2, rad * 2, rad * 0.3) : g.circle(ox, oy, rad));
    // Its light's reach, for the DM.
    if (!player && t.light) v.reach.circle(0, 0, t.light * TPX).stroke({ width: 3, color: 0xffd36b, alpha: 0.5 });
    // A PC's pin: its point below, and a door on it while it is inside somewhere.
    if (t.kind === 'character' && roleOf(t) === 'pc') {
      pinPoint(v.pin, r, PIN_TIP * side * TPX, t.color);
      doorMark(v.door, r);
    }
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
      } else if (obj) {
        // A small chest: lit from the upper left, two close tones, ink outline.
        const k = r * 0.62;
        const lit = (c: number, f: number) => (Math.min(255, Math.round(((c >> 16) & 255) * f)) << 16) | (Math.min(255, Math.round(((c >> 8) & 255) * f)) << 8) | Math.min(255, Math.round((c & 255) * f));
        const w = k * 1.5;
        const top = -k * 0.75;
        const lid = k * 0.7;
        const bottom = k * 0.75;
        g.roundRect(-w / 2, top, w, bottom - top, k * 0.2).fill(lit(t.color, 0.72));
        g.roundRect(-w / 2, top, w, lid, k * 0.2).fill(lit(t.color, 1.3));
        g.roundRect(-w / 2, top, w * 0.55, lid * 0.45, k * 0.15).fill({ color: 0xffffff, alpha: 0.18 });
        g.roundRect(-w / 2, top, w, bottom - top, k * 0.2).stroke({ width: 3, color: INK });
        g.moveTo(-w / 2, top + lid).lineTo(w / 2, top + lid).stroke({ width: 3, color: INK });
        g.roundRect(-k * 0.13, top + lid - k * 0.16, k * 0.26, k * 0.34, k * 0.06).fill(0xd8b04a).stroke({ width: 2, color: INK });
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
    v.label.text = t.kind === 'light' && player ? '' : t.name;
  }

  /**
   * Fog of war: what the players don't know is black. Line of sight: what the characters
   * don't see now is greyed, unless they have seen it before (under fog, that is dimmed).
   */
  private updateFog(loc: string, fog: boolean, los: boolean, player: boolean, at: (gx: number, gy: number) => [number, number], rotation: number, pxPerSq: number) {
    const cam = this.view.cam;
    const vision = this.ctl.visionOf(loc);
    // Players with line of sight on and no sight worked out here: only what is revealed shows.
    const blind = player && los && !vision;
    const active = fog || (los && !!vision) || blind;
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
      f.sprite.alpha = fog && los && (vision || blind) ? 0.55 : 1;
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
  return [t.name, t.kind, t.kind === 'character' ? roleOf(t) : '', t.size, t.color, t.hidden ? 1 : 0, t.vision ? 1 : 0, t.light ?? 0, image ? t.image : '', grid.scale, selected && !player ? 'sel' : ''].join('|');
}

/** Pins in single linkage: those whose points are nearer than `near` (world ft) in one party,
 * and those nearer than 1.2 x `near` that were in one party already (so a party splits a little
 * later than it merged); the parties of two or more, in draw order. */
function partiesOf(pins: TokenView[], near: number, seq: () => number): TokenView[][] {
  const root = pins.map((_, i) => i);
  const find = (i: number) => {
    while (root[i] !== i) i = root[i] = root[root[i]];
    return i;
  };
  for (let i = 0; i < pins.length; i++) {
    for (let j = i + 1; j < pins.length; j++) {
      const d = (pins[i].ax - pins[j].ax) ** 2 + (pins[i].ay - pins[j].ay) ** 2;
      const together = pins[i].pg !== 0 && pins[i].pg === pins[j].pg;
      if (d < near * near || (together && d < near * near * 1.44)) root[find(i)] = find(j);
    }
  }
  const groups = new Map<number, TokenView[]>();
  pins.forEach((v, i) => {
    const k = find(i);
    const g = groups.get(k);
    if (g) g.push(v);
    else groups.set(k, [v]);
  });
  for (const v of pins) v.pg = 0;
  const parties = [...groups.values()].filter((g) => g.length > 1);
  for (const g of parties) {
    const id = seq();
    for (const v of g) v.pg = id;
  }
  return parties;
}

/** A pin's point: from under a body of radius `r` down to `tip` (local units). */
function pinPoint(g: Graphics, r: number, tip: number, color: number) {
  g.moveTo(-r * 0.5, r * 0.6).lineTo(0, tip).lineTo(r * 0.5, r * 0.6).closePath().fill(color).stroke({ width: 4, color: INK, join: 'round' });
}

/** The ring of what the players' view follows: warm gold, 6 px x table scale, ink inside and out
 * (`u`: local units per screen px). */
function ring(g: Graphics, r: number, u: number, square: boolean) {
  const rr = r + 4 + 3 * u;
  const draw = (width: number, color: number) => (square ? g.roundRect(-rr, -rr, rr * 2, rr * 2, rr * 0.3) : g.circle(0, 0, rr)).stroke({ width: width * u, color });
  draw(10, INK);
  draw(6, 0xe0b040);
}

/** A small arched door at a pin's lower right (about 40% of the pin across): the PC is inside
 * somewhere. */
function doorMark(g: Graphics, r: number) {
  const [bx, by] = [r * 0.74, r * 0.62];
  const [hw, h] = [r * 0.28, r * 0.4];
  g.moveTo(bx - hw, by + h)
    .lineTo(bx - hw, by - h + hw)
    .arc(bx, by - h + hw, hw, Math.PI, 0)
    .lineTo(bx + hw, by + h)
    .closePath()
    .fill(0xf5ecd6)
    .stroke({ width: 5, color: INK, join: 'round' });
}

/** A name's initials (two at most). */
function initialsOf(name: string): string {
  return (
    name
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]!.toUpperCase())
      .join('') || '?'
  );
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

/** The screen-space label styles at a table scale (labels, shape sizes, the ruler's text). */
function scaleStyles(ts: number) {
  labelStyle.fontSize = 13 * ts;
  plateStyle.fontSize = 13 * ts;
  labelStyle.stroke = { color: '#1d1a14', width: 3 * ts, join: 'round' };
  shapeStyle.fontSize = 13 * ts;
  shapeStyle.stroke = { color: '#1d1a14', width: 3 * ts, join: 'round' };
  rulerStyle.fontSize = 15 * ts;
  rulerStyle.lineHeight = 21 * ts;
  rulerStyle.stroke = { color: '#1d1a14', width: 4 * ts, join: 'round' };
}
