// Feature labels, cartographic style: each kind has its own lettering (spaced caps for
// continents, oceans and ranges; italics for water and regions), a zoom band (a label shows
// while its feature's on-screen size is within range), rotation along the feature, and
// priority. Layout is greedy collision avoidance, highest priority first. Town districts
// arrive later (once a layout exists) and are lettered along a curved baseline.
import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import type { DistrictLabel, Feature, Hit } from '../gen/protocol';
import { buildingName } from '../ui/gazetteer';
import type { Camera } from './camera';

interface KindStyle {
  size: number;
  fill: string;
  spacing: number;
  upper?: boolean;
  italic?: boolean;
  bold?: boolean;
  symbol?: string;
  /** Show while the feature's extent in screen px is within [minPx, maxPx]. */
  minPx: number;
  maxPx: number;
  prio: number;
  rotate?: boolean;
  /** The symbol marks the exact spot (settlements, sites); the name trails to its right. */
  pin?: boolean;
  /** A business pin: a coloured disc with a white symbol on the spot, the name beside it. */
  badge?: { color: string; glyph: string };
}

type BuildingHit = Extract<Hit, { kind: 'building' }>;

/** Business pins by catalog category (`worldgen::town::catalog`): disc colour, symbol, and
 * how early they show (on-screen px of a 60-ft extent) and win space. */
const PLACE: Record<string, [string, string, number, number]> = {
  'food & lodging': ['#b45309', '⌂', 9, 30],
  'faith & death': ['#6b21a8', '✝', 9, 29],
  'civic & military': ['#1e3a8a', '⚑', 9, 28],
  'arms & gear': ['#475569', '⚔', 13, 24],
  'magic & knowledge': ['#0e7490', '✦', 13, 24],
  'luxury & finance': ['#a16207', '◆', 13, 23],
  'crafts & transport': ['#7c4a21', '⚒', 14, 22],
  'guilds & underworld': ['#3f3f46', '♣', 14, 22],
  'entertainment & oddities': ['#be185d', '♪', 13, 23],
};
const PLACE_EXTENT_FT = 60;

const INK = '#3b2f25';
const WATER = '#2f5367';
const LAND = '#46512f';

const REGION: KindStyle = { size: 13, fill: LAND, spacing: 2, italic: true, minPx: 90, maxPx: 1600, prio: 40, rotate: true };

const STYLES: Record<string, KindStyle> = {
  continent: { size: 28, fill: INK, spacing: 12, upper: true, bold: true, minPx: 300, maxPx: 4000, prio: 100, rotate: true },
  ocean: { size: 20, fill: WATER, spacing: 9, upper: true, italic: true, minPx: 200, maxPx: 1e9, prio: 95 },
  sea: { size: 16, fill: WATER, spacing: 6, upper: true, italic: true, minPx: 120, maxPx: 3000, prio: 85 },
  range: { size: 13, fill: '#5a4330', spacing: 5, upper: true, minPx: 70, maxPx: 1800, prio: 75, rotate: true },
  volcano: { size: 11, fill: '#8a2a1a', spacing: 0, symbol: '▲', minPx: 18, maxPx: 1e9, prio: 70 },
  peak: { size: 11, fill: INK, spacing: 0, symbol: '▲', minPx: 30, maxPx: 1e9, prio: 60 },
  bay: { size: 13, fill: WATER, spacing: 2, italic: true, minPx: 50, maxPx: 1500, prio: 58 },
  lake: { size: 12, fill: WATER, spacing: 1, italic: true, minPx: 28, maxPx: 1500, prio: 55, rotate: true },
  salt_lake: { size: 12, fill: WATER, spacing: 1, italic: true, minPx: 28, maxPx: 1500, prio: 54, rotate: true },
  salt_flat: { size: 11, fill: '#6b5b45', spacing: 1, italic: true, minPx: 28, maxPx: 1500, prio: 50 },
  island: { size: 12, fill: INK, spacing: 1, minPx: 24, maxPx: 1500, prio: 52 },
  pass: { size: 10, fill: INK, spacing: 0, symbol: ')(', minPx: 40, maxPx: 1e9, prio: 35 },
  waterfall: { size: 10, fill: WATER, spacing: 0, italic: true, symbol: '≋', minPx: 60, maxPx: 1e9, prio: 33 },
  river: { size: 11, fill: WATER, spacing: 1, italic: true, minPx: 220, maxPx: 1e9, prio: 30, rotate: true },
  metropolis: { size: 16, fill: INK, spacing: 3, upper: true, bold: true, symbol: '◉', minPx: 16, maxPx: 1e9, prio: 98, pin: true },
  city: { size: 14, fill: INK, spacing: 2, upper: true, bold: true, symbol: '●', minPx: 18, maxPx: 1e9, prio: 90, pin: true },
  town: { size: 12, fill: INK, spacing: 1, symbol: '●', minPx: 24, maxPx: 1e9, prio: 72, pin: true },
  village: { size: 11, fill: INK, spacing: 0, symbol: '•', minPx: 36, maxPx: 1e9, prio: 45, pin: true },
  waystation: { size: 10, fill: '#5a4330', spacing: 0, italic: true, symbol: '⌂', minPx: 60, maxPx: 1e9, prio: 32, pin: true },
  ruin: { size: 10, fill: '#6b4a3a', spacing: 0, italic: true, symbol: '✕', minPx: 60, maxPx: 1e9, prio: 31, pin: true },
  tower: { size: 10, fill: '#4b3a6b', spacing: 0, italic: true, symbol: '♜', minPx: 60, maxPx: 1e9, prio: 34, pin: true },
  cave: { size: 10, fill: '#4a4038', spacing: 0, italic: true, symbol: '◗', minPx: 60, maxPx: 1e9, prio: 30, pin: true },
  mine: { size: 10, fill: '#4a4038', spacing: 0, italic: true, symbol: '⚒', minPx: 60, maxPx: 1e9, prio: 30, pin: true },
  lava_tube: { size: 10, fill: '#7a3418', spacing: 0, italic: true, symbol: '◗', minPx: 60, maxPx: 1e9, prio: 30, pin: true },
  entrance: { size: 10, fill: '#4a4038', spacing: 0, italic: true, symbol: '▼', minPx: 60, maxPx: 1e9, prio: 30, pin: true },
  camp: { size: 10, fill: '#5a4330', spacing: 0, italic: true, symbol: '▲', minPx: 60, maxPx: 1e9, prio: 30, pin: true },
  building: { size: 10, fill: '#5a4330', spacing: 0, italic: true, symbol: '■', minPx: 60, maxPx: 6000, prio: 29, pin: true },
  forest: REGION,
  jungle: REGION,
  taiga: REGION,
  desert: { ...REGION, fill: '#6b5231' },
  swamp: { ...REGION, minPx: 40 },
  plains: { ...REGION, fill: '#5d5a2e' },
  tundra: { ...REGION, fill: '#56574a' },
  glacier: { ...REGION, fill: '#4f6470' },
  blight: { ...REGION, fill: '#4e4452' },
  ashlands: { ...REGION, fill: '#4f4c48' },
  region: { ...REGION, fill: '#4e4038' },
  district: { size: 12, fill: '#6b3f2a', spacing: 3, upper: true, minPx: 160, maxPx: 3200, prio: 25 },
};

const FONT = ['Palatino Linotype', 'Book Antiqua', 'Palatino', 'Georgia', 'serif'];

type Box = [number, number, number, number];

/** Placed label boxes (screen px) in a coarse grid, so testing a new label against them looks
 * only at its neighbours (hundreds of labels at town zoom with Places on). */
class Boxes {
  private readonly cells = new Map<number, Box[]>();
  private static readonly CELL = 96;

  private each(b: Box, fn: (list: Box[] | undefined, key: number) => boolean | void): boolean {
    const c = Boxes.CELL;
    const [i0, j0, i1, j1] = [Math.floor(b[0] / c), Math.floor(b[1] / c), Math.floor(b[2] / c), Math.floor(b[3] / c)];
    for (let j = j0; j <= j1; j++) {
      for (let i = i0; i <= i1; i++) {
        const key = (j + 512) * 2048 + (i + 512);
        if (fn(this.cells.get(key), key)) return true;
      }
    }
    return false;
  }

  /** Whether the box overlaps one already placed. */
  hits(b: Box): boolean {
    return this.each(b, (list) => !!list && list.some((o) => o[0] < b[2] && b[0] < o[2] && o[1] < b[3] && b[1] < o[3]));
  }

  add(b: Box) {
    this.each(b, (list, key) => {
      if (list) list.push(b);
      else this.cells.set(key, [b]);
    });
  }
}

/** A business pin's disc radius (px). */
const BADGE_R = 8;

/** Half the size of a business pin (about its centre). */
function badgeHalf(box: Container): [number, number] {
  return [box.width / 2, Math.max(BADGE_R, box.height / 2)];
}

interface Placed {
  f: Feature;
  /** The generated name (renames can be undone). */
  orig: string;
  style: KindStyle;
  text: Container | null;
  /** Half the text's size (px), measured once: measuring text is not free. */
  half?: [number, number];
  /** Curved labels: the baseline (world ft) and one text per letter, placed along it. */
  path?: [number, number][];
  glyphs?: Container | null;
  /** Baseline length (ft). */
  pathLen?: number;
  /** Letter widths (px), measured once: measuring text is not free. */
  glyphW?: number[];
  /** Letter positions (world ft) and angles from the last layout, for panning. */
  at?: [number, number, number][];
}

/** Point and tangent angle at arc length `s` along a polyline. */
function along(path: [number, number][], s: number): [number, number, number] {
  for (let k = 1; k < path.length; k++) {
    const [ax, ay] = path[k - 1];
    const [bx, by] = path[k];
    const l = Math.hypot(bx - ax, by - ay);
    if (s <= l || k === path.length - 1) {
      const t = l > 0 ? Math.min(1, Math.max(0, s / l)) : 0;
      return [ax + (bx - ax) * t, ay + (by - ay) * t, Math.atan2(by - ay, bx - ax)];
    }
    s -= l;
  }
  return [path[0][0], path[0][1], 0];
}

const pathLength = (path: [number, number][]) => path.slice(1).reduce((acc, p, k) => acc + Math.hypot(p[0] - path[k][0], p[1] - path[k][1]), 0);

export class Labels {
  readonly container = new Container();
  private readonly items: Placed[];
  /** Items by integer zoom whose band (extent on screen within [minPx, maxPx]) reaches it,
   * in priority order: a layout only visits the labels that could show. */
  private readonly byZoom = new Map<number, Placed[]>();
  /** Items with a visible label. */
  private shown = new Set<Placed>();
  private lastLayout = -1;
  private lastZoom = NaN;
  /** Business pins: shown or not, and their buildings by id (for clicks). */
  private placesOn = false;
  /** Labels lettered in the last layout (for the frame profile). */
  lastCreated = 0;
  private readonly placeHits = new Map<string, BuildingHit>();
  /** Feature ids hidden by edits. */
  private hidden = new Set<string>();
  /** Labels kept from view besides (the players' window: places they can't see); asked at
   * every layout. */
  conceal: ((f: Feature) => boolean) | null = null;

  constructor(features: Feature[]) {
    this.items = features
      .filter((f) => STYLES[f.kind])
      .map((f) => ({ f, orig: f.name, style: STYLES[f.kind], text: null }));
    this.sort();
  }

  private sort() {
    this.items.sort((a, b) => b.style.prio - a.style.prio || b.f.extent_ft - a.f.extent_ft);
    this.byZoom.clear();
    for (const it of this.items) {
      const e = it.f.extent_ft;
      if (!(e > 0)) continue;
      const z0 = Math.max(-16, Math.floor(Math.log2(it.style.minPx / e)));
      const z1 = Math.min(8, Math.floor(Math.log2(it.style.maxPx / e)));
      for (let z = z0; z <= z1; z++) {
        let list = this.byZoom.get(z);
        if (!list) this.byZoom.set(z, (list = []));
        list.push(it);
      }
    }
  }

  /** Add a settlement's districts (names from `renames` where edited). */
  addDistricts(list: DistrictLabel[], renames: Record<string, string>) {
    for (const d of list) {
      if (this.items.some((it) => it.f.id === d.id) || d.path.length < 2) continue;
      const mid = d.path[Math.floor(d.path.length / 2)];
      const f: Feature = { id: d.id, kind: 'district', name: renames[d.id] ?? d.name, x: mid[0], y: mid[1], angle: 0, extent_ft: pathLength(d.path) / 0.7, detail: `${d.kind} district` };
      this.items.push({ f, orig: d.name, style: STYLES.district, text: null, path: d.path, glyphs: null });
    }
    this.sort();
    this.lastZoom = NaN;
  }

  /** Add businesses as pins (names from `renames` where edited); already known ones are kept. */
  addPlaces(hits: BuildingHit[], renames: Record<string, string>) {
    let added = 0;
    for (const h of hits) {
      if (this.placeHits.has(h.id) || !h.category) continue;
      const [color, glyph, minPx, prio] = PLACE[h.category] ?? ['#57534e', '●', 14, 20];
      this.placeHits.set(h.id, h);
      const f: Feature = { id: h.id, kind: 'place', name: buildingName(h, renames), x: h.x, y: h.y, angle: 0, extent_ft: PLACE_EXTENT_FT, detail: h.function };
      // Bigger buildings win ties within a category.
      const style: KindStyle = { size: 11, fill: color, spacing: 0, minPx, maxPx: 1e9, prio: prio + Math.min(0.9, h.size_ft / 400), pin: true, badge: { color, glyph } };
      this.items.push({ f, orig: buildingName(h, {}), style, text: null });
      added++;
    }
    if (added) {
      this.sort();
      this.lastZoom = NaN;
    }
  }

  /** Show or hide the business pins. */
  setPlaces(on: boolean) {
    if (on === this.placesOn) return;
    this.placesOn = on;
    this.lastZoom = NaN;
  }

  /** The building behind a business pin. */
  placeHit(id: string): BuildingHit | null {
    return this.placeHits.get(id) ?? null;
  }

  /** How far right of the spot a pinned label's centre sits (its symbol on the spot). */
  private pinOffset(it: Placed): number {
    const hw = it.half?.[0] ?? 0;
    return it.style.badge ? hw - BADGE_R : hw - it.style.size * 0.4;
  }

  update(cam: Camera, now: number) {
    const moved = Math.abs(cam.zoom - this.lastZoom) > 0.01;
    // Full layout on zoom changes and periodically while panning; otherwise just translate.
    if (moved || now - this.lastLayout > 150) {
      this.layout(cam);
      this.lastLayout = now;
      this.lastZoom = cam.zoom;
    } else {
      for (const it of this.shown) {
        if (it.glyphs?.visible && it.at) {
          it.glyphs.children.forEach((g, k) => {
            const [sx, sy] = cam.worldToScreen(it.at![k][0], it.at![k][1]);
            g.position.set(sx, sy);
          });
        }
        if (it.text?.visible) {
          const [sx, sy] = cam.worldToScreen(it.f.x, it.f.y);
          const ox = it.style.pin ? this.pinOffset(it) : 0;
          it.text.position.set(sx + ox, sy);
        }
      }
    }
  }

  private layout(cam: Camera) {
    const boxes = new Boxes();
    // Rasterizing text is slow; spread new labels over several layouts to avoid hitches.
    let created = 0;
    this.lastCreated = 0;
    // New labels cost ~10 ms each to letter and measure on this laptop: at least one per
    // layout, more only while the layout is under ~4 ms.
    const start = performance.now();
    const mayCreate = () => created === 0 || performance.now() - start < 4;
    const pad = 4;
    const margin = 40;
    // Thousands of features: the zoom band first (cheap), the screen test only within it.
    const ppf = cam.ppf;
    const shown = new Set<Placed>();
    for (const it of this.byZoom.get(Math.floor(cam.zoom)) ?? []) {
      const { f, style } = it;
      if ((style.badge && !this.placesOn) || this.hidden.has(f.id) || this.conceal?.(f)) continue;
      if (it.path) {
        if (this.layoutCurved(it, cam, boxes, margin, pad, mayCreate())) created++;
        this.lastCreated = created;
        if (it.glyphs?.visible) shown.add(it);
        continue;
      }
      const px = f.extent_ft * ppf;
      let show = px >= style.minPx && px <= style.maxPx;
      let sx = 0;
      let sy = 0;
      if (show) {
        [sx, sy] = cam.worldToScreen(f.x, f.y);
        show = sx > -margin && sy > -margin && sx < cam.width + margin && sy < cam.height + margin;
      }
      if (show && !it.text && !mayCreate()) show = false;
      if (show) {
        if (!it.text) created++;
        this.lastCreated = created;
        const text = it.text ?? this.create(it);
        const angle = style.rotate ? f.angle : 0;
        const [hw, hh] = (it.half ??= style.badge ? badgeHalf(text) : [text.width / 2, text.height / 2]);
        const c = Math.abs(Math.cos(angle));
        const s = Math.abs(Math.sin(angle));
        const ex = hw * c + hh * s + pad;
        const ey = hw * s + hh * c + pad;
        // Pinned labels grow rightwards from the symbol centred on the spot.
        const ox = style.pin ? this.pinOffset(it) : 0;
        const box: [number, number, number, number] = [sx + ox - ex, sy - ey, sx + ox + ex, sy + ey];
        show = !boxes.hits(box);
        if (show) {
          boxes.add(box);
          text.position.set(sx + ox, sy);
          text.rotation = angle;
        }
      }
      if (it.text && it.text.visible !== show) it.text.visible = show;
      if (show) shown.add(it);
    }
    // Labels out of this zoom's band.
    for (const it of this.shown) {
      if (shown.has(it)) continue;
      if (it.text) it.text.visible = false;
      if (it.glyphs) it.glyphs.visible = false;
    }
    this.shown = shown;
  }

  /** Letter a curved label along its baseline, centred; hidden if it doesn't fit or collides.
   * Returns whether its letters were created now. */
  private layoutCurved(it: Placed, cam: Camera, boxes: Boxes, margin: number, pad: number, mayCreate: boolean): boolean {
    const { f, style } = it;
    const path = it.path!;
    const px = f.extent_ft * cam.ppf;
    const [sx, sy] = cam.worldToScreen(f.x, f.y);
    const reach = px;
    const near = sx > -reach - margin && sy > -reach - margin && sx < cam.width + reach + margin && sy < cam.height + reach + margin;
    let show = near && px >= style.minPx && px <= style.maxPx && (!!it.glyphs || mayCreate);
    let created = false;
    if (show && !it.glyphs) {
      it.glyphs = this.createGlyphs(it);
      it.glyphW = (it.glyphs.children as Text[]).map((g) => g.width);
      created = true;
    }
    if (show && it.glyphs) {
      const glyphs = it.glyphs.children as Text[];
      const widths = it.glyphW!;
      const total = widths.reduce((a, b) => a + b, 0) + style.spacing * (glyphs.length - 1);
      const lenPx = (it.pathLen ??= pathLength(path)) * cam.ppf;
      if (total > lenPx * 1.1) show = false;
      else {
        let s = (lenPx - total) / 2;
        const at: [number, number, number][] = [];
        const box: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
        glyphs.forEach((g, k) => {
          const [wx, wy, a] = along(path, (s + widths[k] / 2) / cam.ppf);
          at.push([wx, wy, a]);
          const [gx, gy] = cam.worldToScreen(wx, wy);
          const r = style.size * 0.6 + pad;
          box[0] = Math.min(box[0], gx - r);
          box[1] = Math.min(box[1], gy - r);
          box[2] = Math.max(box[2], gx + r);
          box[3] = Math.max(box[3], gy + r);
          s += widths[k] + style.spacing;
        });
        show = !boxes.hits(box);
        if (show) {
          boxes.add(box);
          it.at = at;
          glyphs.forEach((g, k) => {
            const [gx, gy] = cam.worldToScreen(at[k][0], at[k][1]);
            g.position.set(gx, gy);
            g.rotation = at[k][2];
          });
        }
      }
    }
    if (it.glyphs) it.glyphs.visible = show;
    return created;
  }

  private createGlyphs(it: Placed): Container {
    const { f, style } = it;
    const label = style.upper ? f.name.toUpperCase() : f.name;
    const box = new Container();
    const textStyle = new TextStyle({
      fontFamily: FONT,
      fontSize: style.size,
      fill: style.fill,
      fontStyle: style.italic ? 'italic' : 'normal',
      fontWeight: style.bold ? 'bold' : 'normal',
      stroke: { color: '#efe6cf', width: 3, join: 'round' },
    });
    for (const ch of label) box.addChild(new Text({ text: ch, style: textStyle, anchor: 0.5, resolution: 2 }));
    this.container.addChild(box);
    return box;
  }

  private create(it: Placed): Container {
    const { f, style } = it;
    if (style.badge) return this.createBadge(it);
    let label = style.upper ? f.name.toUpperCase() : f.name;
    if (style.symbol) label = `${style.symbol} ${label}`;
    const text = new Text({
      text: label,
      style: new TextStyle({
        fontFamily: FONT,
        fontSize: style.size,
        fill: style.fill,
        letterSpacing: style.spacing,
        fontStyle: style.italic ? 'italic' : 'normal',
        fontWeight: style.bold ? 'bold' : 'normal',
        stroke: { color: '#efe6cf', width: 3, join: 'round' },
      }),
      anchor: 0.5,
      resolution: 2,
    });
    it.text = text;
    this.container.addChild(text);
    return text;
  }

  /** A business pin: a disc with its symbol, the name to its right; laid out about its centre
   * (so it places like a label), the disc's centre `BADGE_R` from its left. */
  private createBadge(it: Placed): Container {
    const { f, style } = it;
    const badge = style.badge!;
    const name = new Text({
      text: f.name,
      style: new TextStyle({ fontFamily: FONT, fontSize: style.size, fill: style.fill, fontWeight: 'bold', stroke: { color: '#efe6cf', width: 3, join: 'round' } }),
      anchor: { x: 0, y: 0.5 },
      resolution: 2,
    });
    const w = BADGE_R * 2 + 3 + name.width;
    const box = new Container();
    const disc = new Graphics();
    disc.circle(-w / 2 + BADGE_R + 1, 1.2, BADGE_R).fill({ color: 0x000000, alpha: 0.28 });
    disc.circle(-w / 2 + BADGE_R, 0, BADGE_R).fill(badge.color).stroke({ width: 1.5, color: 0xffffff });
    const glyph = new Text({ text: badge.glyph, style: new TextStyle({ fontFamily: ['Segoe UI Symbol', ...FONT], fontSize: BADGE_R * 1.25, fill: '#ffffff' }), anchor: 0.5, resolution: 2 });
    glyph.position.set(-w / 2 + BADGE_R, 0.5);
    name.position.set(-w / 2 + BADGE_R * 2 + 3, 0);
    box.addChild(disc, glyph, name);
    it.text = box;
    this.container.addChild(box);
    return box;
  }

  /** The feature whose visible label is under a screen point. */
  hit(sx: number, sy: number): Feature | null {
    for (const it of this.shown) {
      const shown = it.text?.visible ? it.text : it.glyphs?.visible ? it.glyphs : null;
      if (!shown) continue;
      const b = shown.getBounds();
      if (sx >= b.minX && sx <= b.maxX && sy >= b.minY && sy <= b.maxY) return it.f;
    }
    return null;
  }

  /** Rename a feature (or a business), or give it back its own name (`null`): its label is
   * re-lettered at the next layout. */
  rename(id: string, name: string | null) {
    for (const it of this.items) {
      if (it.f.id !== id) continue;
      it.f.name = name ?? it.orig;
      this.unletter(it);
    }
    this.lastZoom = NaN;
  }

  /** Hide the labels of these features (edits), showing any others hidden before. */
  setHidden(ids: Set<string>) {
    this.hidden = ids;
    this.lastZoom = NaN;
  }

  /** Replace the labels of created sites (`c:` ids) with these. */
  setCreated(features: Feature[]) {
    for (let k = this.items.length - 1; k >= 0; k--) {
      const it = this.items[k];
      if (!it.f.id.startsWith('c:')) continue;
      this.unletter(it);
      this.items.splice(k, 1);
    }
    for (const f of features) if (STYLES[f.kind]) this.items.push({ f, orig: f.name, style: STYLES[f.kind], text: null });
    this.sort();
    this.lastZoom = NaN;
  }

  private unletter(it: Placed) {
    it.text?.destroy();
    it.text = null;
    it.half = undefined;
    it.glyphs?.destroy({ children: true });
    it.glyphs = null;
    this.shown.delete(it);
  }

  destroy() {
    this.container.destroy({ children: true });
  }
}
