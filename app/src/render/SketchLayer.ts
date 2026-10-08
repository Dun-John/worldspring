// Sketch mode on the map: the quick preview of the world the sketch makes (an image over the
// map), and the strokes themselves (redrawn in screen space when they or the view change).
import { Container, Graphics, Sprite, Text, Texture } from 'pixi.js';
import type { Geom, Stroke } from '../gen/protocol';
import type { Sketcher } from '../editor/sketcher';
import type { Camera } from './camera';

const BIOME_COLORS: Record<string, number> = {
  ice: 0xf2f2ed,
  tundra: 0xccc9b3,
  alpine: 0xbdb5a3,
  taiga: 0xa3b091,
  temperate_forest: 0xadbd8a,
  temperate_rainforest: 0x94ad85,
  grassland: 0xd1d1a1,
  steppe: 0xd9cfa3,
  cold_desert: 0xd6c9a8,
  hot_desert: 0xe8d4a1,
  savanna: 0xdbcf96,
  jungle: 0x85a673,
  swamp: 0xa8b599,
  volcanic: 0x877a73,
  salt_flat: 0xede8db,
  blighted_woods: 0x8c8390,
  ashlands: 0x998f82,
};

const COLORS: Record<string, number> = { land: 0x4f7a37, sea: 0x2f6390, range: 0x6b4423, massif: 0x6b4423, river: 0x2a6db5, lake: 0x2a6db5, pin: 0x8a2a1a, volcano: 0x8a3a1a, region: 0x6a4a8a, site: 0x3a322a };
/** Elevation strokes: raised, lowered. */
const RAISE = 0xa8822f;
const LOWER = 0x6a62a0;
const SALT = 0x7aaebf;
const PIN_R: Record<string, number> = { metropolis: 8, city: 6.5, town: 5, village: 3.5 };
/** Road strokes by kind: colour and width (px), as the map draws the roads; a no-road line. */
const ROADS: Record<string, [number, number]> = { kings_road: [0x78201a, 4], road: [0x965028, 3], track: [0xaa825a, 2] };
const NO_ROAD = 0xb02a1a;

export class SketchLayer {
  readonly container = new Container();
  private readonly preview = new Sprite();
  private readonly g = new Graphics();
  private readonly names = new Container();
  private previewSize: [number, number] = [1, 1];
  /** The map being sketched (ft): the world's, or the size about to be generated. */
  private mapSize: [number, number];
  private dirty = true;
  private nameCount = 0;
  private lastView = '';
  /** Show the preview (else just the strokes over the current map). */
  showPreview = true;

  constructor(
    private readonly geom: Geom,
    private readonly sketcher: Sketcher,
  ) {
    this.preview.visible = false;
    this.mapSize = [geom.map_w_ft, geom.map_h_ft];
    this.container.addChild(this.preview, this.g, this.names);
  }

  /** The map size being sketched (its outline is drawn; previews fill it). */
  setMapSize(w: number, h: number) {
    this.mapSize = [w, h];
    this.dirty = true;
  }

  /** The strokes changed: redraw on the next frame. */
  invalidate() {
    this.dirty = true;
  }

  /** A new preview image (RGBA, `w` points across the map). */
  setPreview(rgba: Uint8ClampedArray<ArrayBuffer>, w: number, h: number) {
    const canvas = document.createElement('canvas');
    canvas.width = w;
    canvas.height = h;
    canvas.getContext('2d')!.putImageData(new ImageData(rgba, w, h), 0, 0);
    const old = this.preview.texture;
    this.preview.texture = Texture.from(canvas);
    if (old && old !== Texture.EMPTY) old.destroy(true);
    this.previewSize = [w, h];
    this.preview.visible = this.showPreview;
    this.dirty = true;
  }

  update(cam: Camera) {
    const view = `${cam.cx},${cam.cy},${cam.zoom},${cam.width},${cam.height}`;
    if (!this.dirty && view === this.lastView) return;
    this.dirty = false;
    this.lastView = view;
    // Preview points are `cell` ft apart from the map's corner; each pixel centred on its point.
    const [w] = this.previewSize;
    const cell = this.mapSize[0] / Math.max(1, w - 1);
    const [px, py] = cam.worldToScreen(-cell / 2, -cell / 2);
    this.preview.position.set(px, py);
    this.preview.scale.set(cell * cam.ppf);
    this.preview.visible = this.showPreview && this.preview.texture !== Texture.EMPTY;

    const g = this.g;
    g.clear();
    this.nameCount = 0;
    // The map's edge (what gets generated).
    const [x0, y0] = cam.worldToScreen(0, 0);
    const [x1, y1] = cam.worldToScreen(this.mapSize[0], this.mapSize[1]);
    g.rect(x0, y0, x1 - x0, y1 - y0).stroke({ width: 1.5, color: 0x3a322a, alpha: 0.7 });
    const all = this.sketcher.drawing ? [...this.sketcher.strokes, this.sketcher.drawing] : this.sketcher.strokes;
    // Areas first, then lines, then pins, volcanoes, sites and names on top.
    const order = (s: Stroke) => (s.pts.length === 1 && !s.closed ? 2 : s.closed || s.tool === 'land' || s.tool === 'sea' || s.tool === 'biome' ? 0 : 1);
    for (const s of [...all].sort((a, b) => order(a) - order(b))) this.draw(s, cam, s === this.sketcher.drawing);
    // Labels left over from before.
    for (const c of this.names.children.slice(this.nameCount)) c.destroy();
  }

  /** A label at (x, y) (screen px; `center`ed on it, else left-aligned). Labels are reused in
   * order (lettering text is slow): only a changed one re-letters. */
  private label(text: string, italic: boolean, x: number, y: number, center = false) {
    let label = this.names.children[this.nameCount] as Text | undefined;
    if (!label) {
      label = new Text({ text, style: { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x2b241d, stroke: { color: 0xf3ecd8, width: 3 } } });
      this.names.addChild(label);
    } else if (label.text !== text) {
      label.text = text;
    }
    label.style.fontStyle = italic ? 'italic' : 'normal';
    label.anchor.set(center ? 0.5 : 0, center ? 0.5 : 0);
    label.position.set(x, y);
    this.nameCount++;
  }

  private draw(s: Stroke, cam: Camera, live: boolean) {
    const g = this.g;
    const pts = s.pts.map((p) => cam.worldToScreen(p[0], p[1]));
    const color =
      s.tool === 'biome' ? (BIOME_COLORS[s.biome ?? ''] ?? 0x999999) : s.tool === 'elevation' ? ((s.delta_ft ?? 0) < 0 ? LOWER : RAISE) : s.tool === 'lake' && s.salt ? SALT : (COLORS[s.tool] ?? 0x999999);
    if (s.tool === 'pin') {
      const [x, y] = pts[0];
      const r = PIN_R[s.tier ?? 'town'] ?? 5;
      g.circle(x, y, r + 2).fill({ color: 0xf3ecd8 });
      g.circle(x, y, r).fill({ color }).stroke({ width: 1.5, color: 0x2b241d });
      this.label(s.name || `(${s.tier})`, !s.name, x + r + 4, y - 9);
      return;
    }
    if (s.tool === 'volcano') {
      // A cone with its crater (wider for a caldera, lower for a shield).
      const [x, y] = pts[0];
      const [w, h] = s.kind === 'shield' ? [10, 5] : s.kind === 'cinder' ? [5, 5] : s.kind === 'caldera' ? [10, 6] : [7, 9];
      const c = s.kind === 'caldera' ? 4.5 : 2;
      g.poly([x - w - 1.5, y + 4, x - c - 1, y + 4 - h - 1.5, x + c + 1, y + 4 - h - 1.5, x + w + 1.5, y + 4], true).fill({ color: 0xf3ecd8 });
      g.poly([x - w, y + 3, x - c, y + 3 - h, x + c, y + 3 - h, x + w, y + 3], true).fill({ color }).stroke({ width: 1.5, color: 0x2b241d, join: 'round' });
      if (s.activity !== 'extinct') g.circle(x, y + 3 - h, c * 0.7).fill({ color: s.activity === 'active' ? 0xe0662a : 0x5a4a40 });
      this.label(s.name || `(${s.kind === 'caldera' ? 'caldera' : 'volcano'})`, !s.name, x + w + 4, y - 9);
      return;
    }
    if (s.tool === 'site') {
      // A small square stone: a site.
      const [x, y] = pts[0];
      g.rect(x - 6, y - 6, 12, 12).fill({ color: 0xf3ecd8 });
      g.rect(x - 4.5, y - 4.5, 9, 9).fill({ color }).stroke({ width: 1.5, color: 0x2b241d });
      this.label(s.name || `(${(s.kind ?? 'ruin').replace('_', ' ')})`, !s.name, x + 9, y - 9);
      return;
    }
    if (s.tool === 'region' && !s.closed) {
      // A name dropped on what it names: a small diamond and the name.
      const [x, y] = pts[0];
      g.poly([x, y - 6, x + 6, y, x, y + 6, x - 6, y], true).fill({ color: 0xf3ecd8 });
      g.poly([x, y - 4, x + 4, y, x, y + 4, x - 4, y], true).fill({ color }).stroke({ width: 1, color: 0x2b241d });
      this.label(s.name ?? '', true, x + 8, y - 9);
      return;
    }
    const flat = pts.flat();
    if (s.closed) {
      const alpha = s.tool === 'biome' || s.tool === 'lake' ? 0.45 : s.tool === 'massif' ? 0.35 : s.tool === 'region' ? 0.12 : 0.3;
      if (pts.length >= 3) g.poly(flat, true).fill({ color, alpha });
      g.poly(flat, !live).stroke({ width: 2.5, color, alpha: 0.95, join: 'round', cap: 'round' });
      // What it does, in its middle: its name, an elevation's change.
      const text = s.tool === 'elevation' ? `${(s.delta_ft ?? 0) < 0 ? '−' : '+'}${Math.abs(s.delta_ft ?? 0).toLocaleString()} ft` : (s.name ?? '');
      if (text && !live && pts.length >= 3) {
        const [cx, cy] = [pts.reduce((a, p) => a + p[0], 0) / pts.length, pts.reduce((a, p) => a + p[1], 0) / pts.length];
        this.label(text, s.tool !== 'elevation', cx, cy, true);
      }
      return;
    }
    // A line's name at its middle point.
    if (s.name && !live && pts.length >= 2) {
      const [mx, my] = pts[Math.floor(pts.length / 2)];
      this.label(s.name, true, mx, my - 14, true);
    }
    if (s.tool === 'road') {
      if (s.kind === 'none') {
        // A dashed red line, ticked across: no planned road crosses it.
        let [on, left] = [true, 8];
        for (let k = 1; k < pts.length; k++) {
          let [ax, ay] = pts[k - 1];
          const [bx, by] = pts[k];
          let rest = Math.hypot(bx - ax, by - ay);
          const [ux, uy] = rest > 0 ? [(bx - ax) / rest, (by - ay) / rest] : [0, 0];
          while (rest > 0) {
            const step = Math.min(left, rest);
            if (on) g.moveTo(ax, ay).lineTo(ax + ux * step, ay + uy * step);
            [ax, ay, rest, left] = [ax + ux * step, ay + uy * step, rest - step, left - step];
            if (left <= 0) [on, left] = [!on, on ? 6 : 8];
          }
        }
        g.stroke({ width: 2.5, color: NO_ROAD, alpha: 0.95, cap: 'round' });
        for (const k of [0, pts.length - 1]) {
          const [x, y] = pts[k];
          g.moveTo(x - 4, y - 4).lineTo(x + 4, y + 4).moveTo(x - 4, y + 4).lineTo(x + 4, y - 4);
        }
        g.stroke({ width: 2, color: NO_ROAD });
        return;
      }
      const [c, w] = ROADS[s.kind ?? 'road'] ?? ROADS.road;
      const road = (width: number, col: number) => {
        g.moveTo(pts[0][0], pts[0][1]);
        for (let k = 1; k < pts.length; k++) g.lineTo(pts[k][0], pts[k][1]);
        g.stroke({ width, color: col, alpha: 0.95, join: 'round', cap: 'round' });
      };
      road(w + 3, 0xf3ecd8);
      road(w, c);
      return;
    }
    const band = Math.max(3, 2 * (s.radius_ft ?? 0) * cam.ppf);
    const line = (width: number, alpha: number, c = color) => {
      g.moveTo(pts[0][0], pts[0][1]);
      for (let k = 1; k < pts.length; k++) g.lineTo(pts[k][0], pts[k][1]);
      g.stroke({ width, color: c, alpha, join: 'round', cap: 'round' });
    };
    if (s.tool === 'river') {
      line(Math.max(2, Math.min(band, 6)), 0.95);
      // The source: rivers run from the first point, the way they were drawn (an arrow at the
      // mouth says so).
      g.circle(pts[0][0], pts[0][1], 4).fill({ color: 0xf3ecd8 }).stroke({ width: 2, color });
      const [ex, ey] = pts[pts.length - 1];
      let back: number[] | undefined;
      for (let k = pts.length - 2; k >= 0 && !back; k--) if (Math.hypot(ex - pts[k][0], ey - pts[k][1]) > 6) back = pts[k];
      if (!live && back) {
        const l = Math.hypot(ex - back[0], ey - back[1]);
        const [ux, uy] = [(ex - back[0]) / l, (ey - back[1]) / l];
        g.poly([ex + ux * 6, ey + uy * 6, ex - ux * 5 - uy * 6, ey - uy * 5 + ux * 6, ex - ux * 5 + uy * 6, ey - uy * 5 - ux * 6], true).fill({ color });
      }
      return;
    }
    line(band, s.tool === 'range' ? 0.3 : 0.35);
    if (s.tool === 'range') line(2.5, 0.9);
  }

  destroy() {
    this.preview.texture?.destroy(true);
    this.container.destroy({ children: true });
  }
}
