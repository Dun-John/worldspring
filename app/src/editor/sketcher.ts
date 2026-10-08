// The sketch being drawn (before it is generated): tools and their settings, freehand strokes
// from pointer input (thinned while drawing, simplified when done), erasing and undo.
import type { PinKind, RoadKind, SiteKind, Stroke, VolcanoActivity, VolcanoKind } from '../gen/protocol';

export const MI = 5280;

/** Editor tools; each makes strokes of one kind (`erase` removes them). */
export type EditTool = 'coast' | 'land' | 'sea' | 'range' | 'massif' | 'elevation' | 'river' | 'lake' | 'biome' | 'volcano' | 'pin' | 'site' | 'region' | 'road' | 'erase';

/** Tools that draw a closed outline (always filled). */
const OUTLINES: EditTool[] = ['coast', 'massif', 'elevation', 'lake'];

export type PinTier = 'metropolis' | 'city' | 'town' | 'village';

export interface ToolSettings {
  tool: EditTool;
  /** Brush radius (mi) per tool: band half-width, a range's half-width, a river's valley, a
   * massif's foothills, an elevation's edge. */
  radiusMi: Record<string, number>;
  /** 0..1: a range's or massif's height, a river's or volcano's size. */
  strength: number;
  /** Elevation: ft raised (negative: lowered). */
  delta: number;
  /** Lakes: the water's level (ft above sea level), or null for its shore's lowest point. */
  level: number | null;
  salt: boolean;
  volcano: VolcanoKind;
  activity: VolcanoActivity;
  /** Exact edges instead of natural ones. */
  hard: boolean;
  biome: string;
  /** Biome strokes: fill the drawn outline instead of brushing along the line. */
  fill: boolean;
  tier: PinTier;
  /** Settlements: what they live by ('' : from their surroundings), the realm's capital, and
   * district names (comma-separated, the central one first). */
  pinKind: PinKind | '';
  capital: boolean;
  wards: string;
  /** Sites: the kind, and what lies beneath a ruin or an entrance ('' : the usual). */
  site: SiteKind;
  under: string;
  /** Roads: the class drawn, or 'none' (a line no planned road crosses). */
  road: RoadKind;
  /** Names: what a name names ('' : whatever is there), and drawn round a region of its own
   * instead of clicked. */
  regionKind: string;
  regionOutline: boolean;
  /** The next stroke's name (empty: generated). */
  name: string;
}

export const DEFAULT_SETTINGS: ToolSettings = {
  tool: 'coast',
  radiusMi: { land: 15, sea: 10, range: 15, river: 3, biome: 20, massif: 10, elevation: 10 },
  strength: 0.7,
  delta: 1000,
  level: null,
  salt: false,
  volcano: 'strato',
  activity: 'active',
  hard: false,
  biome: 'temperate_forest',
  fill: true,
  tier: 'town',
  pinKind: '',
  capital: false,
  wards: '',
  site: 'ruin',
  under: '',
  road: 'road',
  regionKind: '',
  regionOutline: false,
  name: '',
};

/** Tools whose strokes can carry a name. */
export const NAMED_TOOLS: EditTool[] = ['coast', 'land', 'sea', 'range', 'massif', 'river', 'lake', 'biome', 'volcano', 'pin', 'site', 'region', 'road'];

/** Pixels a pointer must move before a new point is added. */
const STEP_PX = 4;
/** Simplification tolerance (px at the zoom it was drawn at). */
const SIMPLIFY_PX = 1.5;

function segDist(p: [number, number], a: [number, number], b: [number, number]): number {
  const dx = b[0] - a[0];
  const dy = b[1] - a[1];
  const l2 = dx * dx + dy * dy;
  const t = l2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2)) : 0;
  return Math.hypot(a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
}

/** Ramer–Douglas–Peucker. */
function simplify(pts: [number, number][], tol: number): [number, number][] {
  if (pts.length < 3) return pts;
  const keep = new Uint8Array(pts.length);
  keep[0] = keep[pts.length - 1] = 1;
  const stack: [number, number][] = [[0, pts.length - 1]];
  while (stack.length) {
    const [i, j] = stack.pop()!;
    let best = -1;
    let bestD = tol;
    for (let k = i + 1; k < j; k++) {
      const d = segDist(pts[k], pts[i], pts[j]);
      if (d > bestD) [best, bestD] = [k, d];
    }
    if (best >= 0) {
      keep[best] = 1;
      stack.push([i, best], [best, j]);
    }
  }
  return pts.filter((_, k) => keep[k]);
}

/** Strokes stretched with the map: x by `sx`, y by `sy`, brush radii by their geometric mean
 * (a drawn continent stays the same share of a resized map). */
export function scaleStrokes(strokes: Stroke[], sx: number, sy: number): Stroke[] {
  if (sx === 1 && sy === 1) return strokes;
  const r = Math.sqrt(sx * sy);
  return strokes.map((s) => ({
    ...s,
    pts: s.pts.map((p) => [Math.round(p[0] * sx), Math.round(p[1] * sy)] as [number, number]),
    ...(s.radius_ft !== undefined ? { radius_ft: Math.round(s.radius_ft * r) } : {}),
  }));
}

/** Whether strokes decide the land mask (any land or sea drawn): the land slider is moot. */
export const drawsLand = (strokes: Stroke[]) => strokes.some((s) => s.tool === 'land' || s.tool === 'sea');

/** Distance (ft) from a point to a stroke (0 inside a filled outline). */
export function strokeDistance(s: Stroke, p: [number, number]): number {
  if (s.closed && s.pts.length >= 3) {
    let inside = false;
    for (let i = 0, j = s.pts.length - 1; i < s.pts.length; j = i++) {
      const [a, b] = [s.pts[i], s.pts[j]];
      if (a[1] > p[1] !== b[1] > p[1] && p[0] < ((b[0] - a[0]) * (p[1] - a[1])) / (b[1] - a[1]) + a[0]) inside = !inside;
    }
    if (inside) return 0;
  }
  if (s.pts.length === 1) return Math.hypot(s.pts[0][0] - p[0], s.pts[0][1] - p[1]);
  let d = Infinity;
  for (let k = 1; k < s.pts.length; k++) d = Math.min(d, segDist(p, s.pts[k - 1], s.pts[k]));
  if (s.closed && s.pts.length > 2) d = Math.min(d, segDist(p, s.pts[s.pts.length - 1], s.pts[0]));
  return d;
}

export class Sketcher {
  strokes: Stroke[] = [];
  /** The stroke being drawn. */
  drawing: Stroke | null = null;
  settings: ToolSettings = structuredClone(DEFAULT_SETTINGS);
  /** Called when the strokes change (not while one is being drawn). */
  onChange: () => void = () => {};
  /** Called while a stroke is drawn (for redrawing). */
  onDraw: () => void = () => {};
  private undoStack: Stroke[][] = [];
  private ppf = 1;

  /** Start over from these strokes (the world's sketch). */
  load(strokes: Stroke[]) {
    this.strokes = structuredClone(strokes);
    this.undoStack = [];
    this.drawing = null;
  }

  get canUndo(): boolean {
    return this.undoStack.length > 0;
  }

  /** Pointer pressed at world (x, y), at `ppf` pixels per foot. */
  down(x: number, y: number, ppf: number) {
    this.ppf = ppf;
    const t = this.settings;
    if (t.tool === 'erase') {
      // The nearest stroke within a dozen pixels (pins and lines first: they sit on areas).
      let best = -1;
      let bestD = 12 / ppf;
      this.strokes.forEach((s, k) => {
        const d = strokeDistance(s, [x, y]) + (s.closed ? 6 / ppf : 0);
        if (d <= bestD) [best, bestD] = [k, d];
      });
      if (best >= 0) this.commit(this.strokes.filter((_, k) => k !== best));
      return;
    }
    const name = t.name.trim();
    // A name drawn as a point or an outline needs its name first.
    if (t.tool === 'region' && !name) return;
    if (t.tool === 'pin' || t.tool === 'volcano' || t.tool === 'site' || (t.tool === 'region' && !t.regionOutline)) {
      const at: [number, number] = [Math.round(x), Math.round(y)];
      const wards = t.wards
        .split(',')
        .map((w) => w.trim())
        .filter(Boolean);
      const s: Stroke =
        t.tool === 'pin'
          ? { tool: 'pin', pts: [at], tier: t.tier, ...(t.pinKind ? { kind: t.pinKind } : {}), ...(t.capital ? { capital: true } : {}), ...(wards.length ? { wards } : {}) }
          : t.tool === 'site'
            ? { tool: 'site', pts: [at], kind: t.site, ...(t.under && (t.site === 'ruin' || t.site === 'entrance') ? { under: t.under } : {}) }
            : t.tool === 'region'
              ? { tool: 'region', pts: [at], ...(t.regionKind ? { kind: t.regionKind } : {}) }
              : { tool: 'volcano', pts: [at], kind: t.volcano, activity: t.activity, strength: t.strength };
      this.commit([...this.strokes, { ...s, ...(name ? { name } : {}) }]);
      this.settings.name = '';
      if (t.tool === 'pin') [this.settings.capital, this.settings.wards] = [false, ''];
      return;
    }
    const tool = t.tool === 'coast' ? 'land' : t.tool;
    const closed = OUTLINES.includes(t.tool) || (t.tool === 'biome' && t.fill) || t.tool === 'region';
    this.drawing = {
      tool,
      pts: [[x, y]],
      ...(closed ? { closed } : {}),
      ...(tool === 'road' ? { kind: t.road } : { radius_ft: (t.radiusMi[tool] ?? 10) * MI }),
      ...(tool === 'range' || tool === 'river' || tool === 'massif' ? { strength: t.strength } : {}),
      ...(t.hard && ['land', 'sea', 'biome', 'elevation'].includes(tool) ? { hard: true } : {}),
      ...(tool === 'biome' ? { biome: t.biome } : {}),
      ...(tool === 'elevation' ? { delta_ft: t.delta } : {}),
      ...(tool === 'lake' ? { ...(t.level !== null ? { level_ft: t.level } : {}), ...(t.salt ? { salt: true } : {}) } : {}),
      ...(tool === 'region' && t.regionKind ? { kind: t.regionKind } : {}),
      ...(name && NAMED_TOOLS.includes(t.tool) && !(tool === 'road' && t.road === 'none') ? { name } : {}),
    };
    this.onDraw();
  }

  move(x: number, y: number) {
    const d = this.drawing;
    if (!d) return;
    const last = d.pts[d.pts.length - 1];
    if (Math.hypot(x - last[0], y - last[1]) * this.ppf < STEP_PX) return;
    d.pts.push([x, y]);
    this.onDraw();
  }

  up() {
    const d = this.drawing;
    this.drawing = null;
    if (!d) return;
    let pts = simplify(d.pts, SIMPLIFY_PX / this.ppf).map((p) => [Math.round(p[0] / 10) * 10, Math.round(p[1] / 10) * 10] as [number, number]);
    if (pts.length > 4000) pts = pts.filter((_, k) => k % Math.ceil(pts.length / 4000) === 0);
    // A closed outline needs an area; a line, two points (a click makes a round brush dab).
    if (d.closed && pts.length < 3) {
      this.onDraw();
      return;
    }
    if (pts.length === 1 && !d.closed) {
      // (A road needs a line: a click makes none.)
      if (d.tool === 'road') {
        this.onDraw();
        return;
      }
      pts = [pts[0], [pts[0][0] + 10, pts[0][1]]];
    }
    this.commit([...this.strokes, { ...d, pts }]);
    if (d.name) this.settings.name = '';
  }

  cancel() {
    this.drawing = null;
    this.onDraw();
  }

  /** The map was resized: stretch every stroke (and the undo history) with it. */
  rescale(sx: number, sy: number) {
    if (sx === 1 && sy === 1) return;
    this.strokes = scaleStrokes(this.strokes, sx, sy);
    this.undoStack = this.undoStack.map((s) => scaleStrokes(s, sx, sy));
    this.drawing = null;
    this.onChange();
  }

  undo() {
    const prev = this.undoStack.pop();
    if (!prev) return;
    this.strokes = prev;
    this.onChange();
  }

  clear() {
    if (this.strokes.length) this.commit([]);
  }

  private commit(next: Stroke[]) {
    this.undoStack.push(this.strokes);
    if (this.undoStack.length > 200) this.undoStack.shift();
    this.strokes = next;
    this.onChange();
  }
}
