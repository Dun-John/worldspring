// Edit › Streets on a town laid out by hand: its street graph (`Edits.streets`) drawn over the map
// and changed by hand. Junctions are dots (a dead end orange), streets drawn by class, each block
// (a face of the graph) tinted by its fill and outlined when picked, problems marked red.
//
// Choose (the default) picks a street, a junction or a block on a click and drags a junction;
// Draw a street adds a point per click or tap (snapped to a junction, else onto a street, within
// a few px or the generator's 3 ft, shown as you go) and ends on a junction or street, on the
// last point clicked again, or on Enter; the brushes (as the ward editor's) move every junction
// within their ring, Equalize each block passed over toward a regular polygon. Nothing is laid
// out while dragging: on release the moves go to the app as one change.
import type { Graphics } from 'pixi.js';
import type { BlockFill, StreetClass, StreetPlan, StreetRequest, TownWard } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';
import { WARD_COLOURS, type TownMode } from './town';

type Pt = [number, number];

/** What is picked: a street, a junction or a block (their record numbers). */
export type StreetPick = { kind: 'street' | 'junction' | 'block'; id: number };

/** The brush's radius at size 1 (ft): about a block across. */
export const HAND_BRUSH_FT = 150;
/** Grabbing a junction or a street: within this many screen px. */
const GRAB_PX = 10;
/** A point drawn snaps to a junction or a street within this many screen px (or the plan's
 * `snap_ft`, if more). */
const SNAP_PX = 12;
/** A brush's junctions within the middle half of its ring move all the way (Displace). */
const SOFT = 0.5;
/** Junctions are drawn from this zoom in (px per ft). */
export const DOTS_PPF = 0.12;

const INK = 0xf97316;
const BAD = 0xdc2626;
const CLASS_LOOK: Record<StreetClass, { width: number; color: number }> = {
  main: { width: 4, color: 0xf0cf6a },
  street: { width: 2.5, color: 0xfafaf9 },
  quay: { width: 3, color: 0x7fb2d6 },
  joint: { width: 1.5, color: 0xd6d3d1 },
};
const OPEN_COLOURS: Record<string, number> = { plaza: 0xf0cf6a, market: 0xe6923a, lawn: 0x9cc65a, park: 0x3d9f5c, garden: 0x3d9f5c, cemetery: 0x8a8f7a, farmland: 0xb9c65a };

/** A block's tint on the map: its ward, open kind or castle. */
export function fillColour(f: BlockFill): number {
  if (f.kind === 'generated') return WARD_COLOURS[f.ward as TownWard] ?? WARD_COLOURS.empty;
  if (f.kind === 'open') return OPEN_COLOURS[f.open] ?? WARD_COLOURS.park;
  if (f.kind === 'castle') return WARD_COLOURS.castle;
  return WARD_COLOURS.empty;
}

/** What the tool needs from the app. */
export interface StreetHost {
  mode(): TownMode;
  /** The brush's radius (ft). */
  radius(): number;
  picked(): StreetPick | null;
  pick(p: StreetPick | null): void;
  /** One change asked of the town (applied, or refused: `settled`). */
  change(req: StreetRequest, what: string): void;
  /** The class a street drawn gets. */
  drawClass(): StreetClass;
  /** The points of the street being drawn changed (how many). */
  drawing(n: number): void;
  hint(text: string): void;
}

interface Line {
  id: number;
  a: number;
  b: number;
  bends: Pt[];
  cls: StreetClass;
}
interface Ring {
  pts: Pt[];
  box: [number, number, number, number];
  area: number;
  /** The junctions at its corners, in order. */
  js: number[];
}
type Snap = { p: Pt; on: 'junction' | 'street' | null; id: number };

type Drag =
  | { kind: 'junction'; j: number; start: Pt; live: boolean }
  | { kind: 'brush'; mode: TownMode; start: Pt; last: Pt; base: Map<number, Pt>; weight: Map<number, number> }
  | { kind: 'equalize'; blocks: Set<number> };

const sub = (a: Pt, b: Pt): Pt => [a[0] - b[0], a[1] - b[1]];
const add = (a: Pt, b: Pt): Pt => [a[0] + b[0], a[1] + b[1]];
const mul = (a: Pt, k: number): Pt => [a[0] * k, a[1] * k];
const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const lerp = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
const round = (p: Pt): Pt => [Math.round(p[0] * 10) / 10, Math.round(p[1] * 10) / 10];

export class StreetTool implements PointerTool {
  private plan: StreetPlan | null = null;
  private pos = new Map<number, Pt>();
  private degree = new Map<number, number>();
  private square = new Set<number>();
  private lines: Line[] = [];
  private byId = new Map<number, Line>();
  private adj = new Map<number, number[]>();
  /** Each block's face (null: none of the streets' faces holds its point). */
  private rings = new Map<number, Ring | null>();
  private bad = { streets: new Set<number>(), blocks: new Set<number>(), junctions: new Set<number>() };
  private drag: Drag | null = null;
  /** Where the junctions touched are shown while dragging, and those sent until the plan comes back. */
  private shown = new Map<number, Pt>();
  private pending: Map<number, Pt> | null = null;
  /** The street being drawn (snapped points), and one sent until the plan comes back. */
  private draft: Snap[] = [];
  private sent: Pt[] | null = null;
  private cursor: Pt | null = null;
  private version = 0;
  /** Pixels per foot at the last frame. */
  private ppf = 1;
  readonly hoverFollows = true;

  constructor(private readonly host: StreetHost) {}

  /** The hand town whose streets the tool works on (null: none). */
  setPlan(plan: StreetPlan | null) {
    this.version++;
    this.plan = plan;
    this.pending = null;
    this.sent = null;
    this.drag = null;
    this.shown.clear();
    this.pos.clear();
    this.degree.clear();
    this.square.clear();
    this.adj.clear();
    this.rings.clear();
    this.byId.clear();
    this.lines = [];
    this.bad = { streets: new Set(), blocks: new Set(), junctions: new Set() };
    if (!plan) {
      this.draft = [];
      return;
    }
    for (const j of plan.junctions) {
      this.pos.set(j.id, j.p);
      this.degree.set(j.id, j.streets);
      if (j.square) this.square.add(j.id);
    }
    const link = (a: number, b: number) => {
      const l = this.adj.get(a) ?? [];
      if (!l.includes(b)) l.push(b);
      this.adj.set(a, l);
    };
    for (const s of plan.streets) {
      if (!this.pos.has(s.a) || !this.pos.has(s.b)) continue;
      const line: Line = { id: s.id, a: s.a, b: s.b, bends: s.bends ?? [], cls: s.class };
      this.lines.push(line);
      this.byId.set(s.id, line);
      link(s.a, s.b);
      link(s.b, s.a);
    }
    for (const p of plan.problems) {
      const m = /\/([nek])(\d+)$/.exec(p.key);
      if (m) (m[1] === 'e' ? this.bad.streets : m[1] === 'k' ? this.bad.blocks : this.bad.junctions).add(Number(m[2]));
    }
    const rings = this.faces();
    for (const b of plan.blocks) {
      let best: Ring | null = null;
      for (const r of rings) {
        const [x0, y0, x1, y1] = r.box;
        if (b.at[0] < x0 || b.at[0] > x1 || b.at[1] < y0 || b.at[1] > y1) continue;
        if ((!best || r.area < best.area) && inside(b.at, r.pts)) best = r;
      }
      this.rings.set(b.id, best);
    }
    // (A street drawn but not sent: kept, its snaps to junctions that went dropped.)
    this.draft = this.draft.filter((s) => s.on !== 'junction' || this.pos.has(s.id));
  }

  /** A street's points as shown (its ends follow junctions being moved). */
  private pts(l: Line): Pt[] {
    return [this.at(l.a), ...l.bends, this.at(l.b)];
  }

  private at(j: number): Pt {
    return this.shown.get(j) ?? this.pending?.get(j) ?? this.pos.get(j)!;
  }

  /** The faces of the graph inside its streets (positive area): each street's two sides walked,
   * turning at each junction onto the next street clockwise. */
  private faces(): Ring[] {
    const out = new Map<number, { h: number; ang: number }[]>();
    const lines = this.lines;
    const put = (v: number, h: number, a: Pt, b: Pt) => {
      const l = out.get(v) ?? [];
      l.push({ h, ang: Math.atan2(b[1] - a[1], b[0] - a[0]) });
      out.set(v, l);
    };
    for (let i = 0; i < lines.length; i++) {
      const p = this.pts(lines[i]);
      put(lines[i].a, 2 * i, p[0], p[1]);
      put(lines[i].b, 2 * i + 1, p[p.length - 1], p[p.length - 2]);
    }
    const slot = new Map<number, number>();
    for (const l of out.values()) {
      l.sort((x, y) => x.ang - y.ang);
      l.forEach((e, k) => slot.set(e.h, k));
    }
    const dest = (h: number) => (h % 2 === 0 ? lines[h >> 1].b : lines[h >> 1].a);
    const seen = new Set<number>();
    const rings: Ring[] = [];
    for (let h0 = 0; h0 < 2 * lines.length; h0++) {
      if (seen.has(h0)) continue;
      const pts: Pt[] = [];
      const js: number[] = [];
      let h = h0;
      for (let guard = 0; guard < 4 * lines.length + 4 && !seen.has(h); guard++) {
        seen.add(h);
        const p = this.pts(lines[h >> 1]);
        if (h % 2) p.reverse();
        js.push(h % 2 === 0 ? lines[h >> 1].a : lines[h >> 1].b);
        pts.push(...p.slice(0, -1));
        const l = out.get(dest(h))!;
        h = l[(slot.get(h ^ 1)! - 1 + l.length) % l.length].h;
      }
      let area = 0;
      for (let k = 0; k < pts.length; k++) {
        const [a, b] = [pts[k], pts[(k + 1) % pts.length]];
        area += a[0] * b[1] - b[0] * a[1];
      }
      if (area <= 1e-6) continue;
      const xs = pts.map((p) => p[0]);
      const ys = pts.map((p) => p[1]);
      rings.push({ pts, js, area: area / 2, box: [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)] });
    }
    return rings;
  }

  /** The junction nearest (x, y) within `r` ft. */
  junctionAt(x: number, y: number, r = GRAB_PX / this.ppf): number | null {
    let best: number | null = null;
    let bestD = r;
    for (const [j, p] of this.pos) {
      const d = dist(p, [x, y]);
      if (d < bestD) [best, bestD] = [j, d];
    }
    return best;
  }

  /** The street nearest (x, y) within `r` ft, and the nearest point on it. */
  private streetNear(x: number, y: number, r: number): { id: number; p: Pt } | null {
    let best: { id: number; p: Pt } | null = null;
    let bestD = r;
    for (const l of this.lines) {
      const p = this.pts(l);
      for (let k = 1; k < p.length; k++) {
        const q = nearest([x, y], p[k - 1], p[k]);
        const d = dist(q, [x, y]);
        if (d < bestD) [best, bestD] = [{ id: l.id, p: q }, d];
      }
    }
    return best;
  }

  /** The block under (x, y), if any (the smallest face holding it). */
  blockAt(x: number, y: number): number | null {
    let best: number | null = null;
    let area = Infinity;
    for (const [b, r] of this.rings) {
      if (!r || r.area >= area) continue;
      const [x0, y0, x1, y1] = r.box;
      if (x < x0 || x > x1 || y < y0 || y > y1 || !inside([x, y], r.pts)) continue;
      [best, area] = [b, r.area];
    }
    return best;
  }

  /** What a click at (x, y) picks: a junction, else a street, else a block. */
  pickAt(x: number, y: number): StreetPick | null {
    const j = this.junctionAt(x, y);
    if (j !== null) return { kind: 'junction', id: j };
    const s = this.streetNear(x, y, GRAB_PX / this.ppf);
    if (s) return { kind: 'street', id: s.id };
    const b = this.blockAt(x, y);
    return b === null ? null : { kind: 'block', id: b };
  }

  /** Where a point drawn at (x, y) goes: onto a junction, else a street, near enough. */
  private snap(x: number, y: number): Snap {
    const r = Math.max(this.plan?.snap_ft ?? 3, SNAP_PX / this.ppf);
    const j = this.junctionAt(x, y, r);
    if (j !== null) return { p: this.pos.get(j)!, on: 'junction', id: j };
    const s = this.streetNear(x, y, r);
    if (s) return { p: round(s.p), on: 'street', id: s.id };
    return { p: round([x, y]), on: null, id: -1 };
  }

  // ---- Drawing a street ----

  /** A click or tap while drawing: a point, or the end. */
  tap(x: number, y: number, e: PointerEvent): boolean {
    if (this.host.mode() !== 'street' || e.button !== 0 || !this.plan) return false;
    if (this.sent) return true;
    this.version++;
    const s = this.snap(x, y);
    const d = this.draft;
    const last = d[d.length - 1];
    const close = Math.max(this.plan.snap_ft, SNAP_PX / this.ppf);
    if (last && dist(last.p, s.p) < close) {
      // The last point again (a double click, a second tap): the end.
      if (d.length >= 2) this.finish();
      return true;
    }
    if (s.on === 'junction' && d.some((q) => q.on === 'junction' && q.id === s.id)) {
      this.host.hint('A street can’t come back to a junction it has passed');
      return true;
    }
    d.push(s);
    if (d.length === 1) this.host.hint(s.on ? 'Click each bend; end on a junction or street, or click the last point again' : 'Click each bend; click the last point again (or press Enter) to end');
    this.host.drawing(d.length);
    // Onto the streets again after the first point: the end.
    if (d.length >= 2 && s.on) this.finish();
    return true;
  }

  /** Send the street drawn. */
  finish() {
    const d = this.draft;
    if (d.length < 2) return void this.host.hint('A street needs two points or more');
    this.version++;
    this.draft = [];
    this.host.drawing(0);
    this.sent = d.map((s) => s.p);
    this.host.change({ op: 'add_street', points: this.sent, class: this.host.drawClass() }, 'street');
  }

  /** The last point drawn taken back. True if there was one. */
  back(): boolean {
    if (!this.draft.length) return false;
    this.version++;
    this.draft.pop();
    this.host.drawing(this.draft.length);
    return true;
  }

  /** How many points the street being drawn has. */
  drawn(): number {
    return this.draft.length;
  }

  // ---- Choosing, dragging, brushing ----

  down(x: number, y: number, e: PointerEvent): boolean {
    const mode = this.host.mode();
    if (e.button !== 0 || !this.plan || this.pending || mode === 'street') return false;
    this.version++;
    const p: Pt = [x, y];
    this.cursor = p;
    if (mode === 'select') {
      const j = this.junctionAt(x, y);
      if (j === null) return false;
      this.drag = { kind: 'junction', j, start: p, live: false };
      return true;
    }
    if (mode === 'equalize') {
      this.drag = { kind: 'equalize', blocks: new Set() };
      this.move(x, y);
      return true;
    }
    const base = new Map(this.pos);
    const drag: Drag = { kind: 'brush', mode, start: p, last: p, base, weight: new Map() };
    this.drag = drag;
    if (mode === 'displace') {
      const r = this.host.radius();
      for (const [j, q] of this.pos) {
        const d = dist(q, p);
        if (d < r) drag.weight.set(j, Math.min(1, (1 - d / r) / SOFT));
      }
    }
    return true;
  }

  move(x: number, y: number) {
    const p: Pt = [x, y];
    this.cursor = p;
    const d = this.drag;
    if (!d) return;
    this.version++;
    if (d.kind === 'junction') {
      if (!d.live && dist(p, d.start) * this.ppf < 4) return;
      d.live = true;
      this.shown.set(d.j, p);
      return;
    }
    if (d.kind === 'equalize') {
      const b = this.blockAt(x, y);
      if (b !== null && this.rings.get(b)) d.blocks.add(b);
      return;
    }
    const r = this.host.radius();
    const now = (j: number) => this.shown.get(j) ?? this.pos.get(j)!;
    const set = (j: number, to: Pt, w: number) => {
      this.shown.set(j, to);
      d.weight.set(j, Math.max(w, d.weight.get(j) ?? 0));
    };
    if (d.mode === 'displace') {
      const by = sub(p, d.start);
      for (const [j, w] of d.weight) this.shown.set(j, add(d.base.get(j)!, mul(by, w)));
    } else if (d.mode === 'liquify') {
      const by = sub(p, d.last);
      for (const j of this.pos.keys()) {
        const k = 1 - dist(now(j), p) / r;
        if (k > 0) set(j, add(now(j), mul(by, 0.5 * k)), k);
      }
    } else if (d.mode === 'bloat' || d.mode === 'pinch') {
      const a = Math.min(2, dist(p, d.start) / r);
      const e = d.mode === 'bloat' ? 1 / (1 + a) : 1 + a;
      for (const [j, q] of d.base) {
        const dd = dist(q, d.start);
        if (dd >= r || dd < 1e-6) continue;
        set(j, add(d.start, mul(sub(q, d.start), (r * Math.pow(dd / r, e)) / dd)), 1 - dd / r);
      }
    } else if (d.mode === 'relax') {
      const s = Math.min(1, (2 * dist(p, d.last)) / r);
      const moves: [number, Pt, number][] = [];
      for (const j of this.pos.keys()) {
        const k = 1 - dist(now(j), p) / r;
        const nb = this.adj.get(j);
        if (k <= 0 || !nb?.length) continue;
        const mean = mul(nb.reduce<Pt>((acc, u) => add(acc, now(u)), [0, 0]), 1 / nb.length);
        moves.push([j, lerp(now(j), mean, s * k), k]);
      }
      for (const [j, to, k] of moves) set(j, to, k);
    }
    d.last = p;
  }

  up(x: number, y: number) {
    this.move(x, y);
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    if (d.kind === 'junction' && !d.live) {
      this.host.pick({ kind: 'junction', id: d.j });
      return;
    }
    if (d.kind === 'equalize') {
      for (const b of d.blocks) this.equalize(b);
    }
    const moves: { junction: number; to: Pt }[] = [];
    for (const [j, q] of this.shown) if (dist(q, this.pos.get(j)!) > 0.5) moves.push({ junction: j, to: round(q) });
    const shown = this.shown;
    this.shown = new Map();
    if (!moves.length) return;
    this.version++;
    this.pending = shown;
    this.host.change({ op: 'move_junctions', moves }, d.kind === 'equalize' ? 'equalize' : 'junctions');
  }

  /** Block `b`'s junctions moved halfway toward a regular polygon round its middle, as large as it is. */
  private equalize(b: number) {
    const r = this.rings.get(b);
    if (!r) return;
    const js = [...new Set(r.js)];
    const n = js.length;
    if (n < 3) return;
    const ps = js.map((j) => this.shown.get(j) ?? this.pos.get(j)!);
    const c = mul(ps.reduce<Pt>((a, p) => add(a, p), [0, 0]), 1 / n);
    const rad = ps.reduce((a, p) => a + dist(p, c), 0) / n;
    // (Turned to fit: the mean of each corner's angle less its place round the polygon; a face's
    // corners go round with the angle, as its area is positive.)
    const dir = 1;
    let sx = 0;
    let sy = 0;
    ps.forEach((p, k) => {
      const t = Math.atan2(p[1] - c[1], p[0] - c[0]) - (dir * 2 * Math.PI * k) / n;
      sx += Math.cos(t);
      sy += Math.sin(t);
    });
    const t0 = Math.atan2(sy, sx);
    js.forEach((j, k) => {
      const t = t0 + (dir * 2 * Math.PI * k) / n;
      this.shown.set(j, lerp(ps[k], [c[0] + rad * Math.cos(t), c[1] + rad * Math.sin(t)], 0.5));
    });
  }

  cancel() {
    this.version++;
    this.drag = null;
    this.shown.clear();
  }

  /** The change was refused or failed: the junctions back where they stand, the street drawn dropped. */
  settled() {
    this.version++;
    this.pending = null;
    this.sent = null;
  }

  /** Drop a drag or a street being drawn. True if there was one. */
  reset(): boolean {
    const had = !!this.drag || this.draft.length > 0;
    this.cancel();
    this.draft = [];
    this.host.drawing(0);
    return had;
  }

  hover(x: number, y: number) {
    this.cursor = [x, y];
    if (this.host.mode() !== 'select') this.version++;
  }

  drawKey(): string {
    const p = this.host.picked();
    return `${this.version}|${this.host.mode()}|${p?.kind}${p?.id}|${this.host.radius()}`;
  }

  dblclick(): boolean {
    // (The second click of a double click has ended the street already; not into a building.)
    return true;
  }

  draw(g: Graphics, cam: Camera) {
    this.ppf = cam.ppf;
    const plan = this.plan;
    if (!plan) return;
    const ppf = cam.ppf;
    if (ppf < 0.02) return;
    const [x0, y0, x1, y1] = cam.viewRect();
    const pad = (x1 - x0) / 4 + 200;
    const inView = (p: Pt) => p[0] > x0 - pad && p[0] < x1 + pad && p[1] > y0 - pad && p[1] < y1 + pad;
    const boxIn = (b: Ring['box']) => b[2] > x0 - pad && b[0] < x1 + pad && b[3] > y0 - pad && b[1] < y1 + pad;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const picked = this.host.picked();
    const mode = this.host.mode();
    const moving = this.shown.size > 0 || !!this.pending;
    // Blocks, tinted by their fill; those with a problem hatched red.
    const fills = new Map(plan.blocks.map((b) => [b.id, b.fill]));
    for (const [b, r] of this.rings) {
      if (!r || !boxIn(r.box)) continue;
      const pts = moving ? this.ringNow(r) : r.pts;
      g.poly(pts.flatMap(scr)).fill({ color: this.bad.blocks.has(b) ? BAD : fillColour(fills.get(b)!), alpha: picked?.kind === 'block' && picked.id === b ? 0.34 : 0.14 });
    }
    // Streets: dark under their class's colour; a problem red under both.
    const visible = this.lines.filter((l) => inView(this.at(l.a)) || inView(this.at(l.b)) || l.bends.some(inView));
    const path = (l: Line) => {
      const p = this.pts(l).map(scr);
      g.moveTo(p[0][0], p[0][1]);
      for (let k = 1; k < p.length; k++) g.lineTo(p[k][0], p[k][1]);
    };
    const bad = visible.filter((l) => this.bad.streets.has(l.id));
    if (bad.length) {
      for (const l of bad) path(l);
      g.stroke({ width: 9, color: BAD, alpha: 0.7 });
    }
    for (const l of visible) path(l);
    g.stroke({ width: 5, color: 0x1c1917, alpha: 0.4 });
    for (const cls of Object.keys(CLASS_LOOK) as StreetClass[]) {
      const of = visible.filter((l) => l.cls === cls);
      if (!of.length) continue;
      for (const l of of) path(l);
      g.stroke({ width: CLASS_LOOK[cls].width * Math.min(1, Math.max(0.5, ppf * 2)), color: CLASS_LOOK[cls].color, alpha: 0.9 });
    }
    if (moving) {
      const touched = (j: number) => this.shown.has(j) || !!this.pending?.has(j);
      const of = visible.filter((l) => touched(l.a) || touched(l.b));
      for (const l of of) path(l);
      if (of.length) g.stroke({ width: 2.5, color: INK, alpha: 0.9 });
    }
    // The pick: a block outlined, a street or junction ringed.
    if (picked?.kind === 'block') {
      const r = this.rings.get(picked.id);
      if (r) g.poly(this.ringNow(r).flatMap(scr)).stroke({ width: 3, color: 0xfde047 });
    } else if (picked?.kind === 'street') {
      const l = this.byId.get(picked.id);
      if (l) {
        path(l);
        g.stroke({ width: 6, color: 0xfde047, alpha: 0.9 });
      }
    }
    // Blocks with a problem: a red mark on their point.
    for (const b of plan.blocks) {
      if (!this.bad.blocks.has(b.id) || !inView(b.at)) continue;
      const [sx, sy] = scr(b.at);
      g.poly([sx, sy - 9, sx + 8, sy + 6, sx - 8, sy + 6]).fill({ color: BAD }).stroke({ width: 1.5, color: 0xfafaf9 });
    }
    // Junctions: white, a dead end orange, a square's ringed, a problem red.
    if (ppf >= DOTS_PPF) {
      const weight = this.drag?.kind === 'brush' ? this.drag.weight : null;
      for (const j of this.pos.keys()) {
        const p = this.at(j);
        if (!inView(p)) continue;
        const [sx, sy] = scr(p);
        const w = weight?.get(j);
        const dead = (this.degree.get(j) ?? 0) <= 1;
        const r = dead || this.bad.junctions.has(j) ? 4 : 3;
        if (this.square.has(j)) g.circle(sx, sy, r + 4).stroke({ width: 2, color: 0xfde68a, alpha: 1 });
        g.circle(sx, sy, r).fill({ color: w ? INK : this.bad.junctions.has(j) ? BAD : dead ? 0xfb923c : 0xffffff, alpha: w ? 0.35 + 0.65 * w : 1 }).stroke({ width: 1, color: 0x1c1917 });
      }
      if (picked?.kind === 'junction' && this.pos.has(picked.id)) {
        const [sx, sy] = scr(this.at(picked.id));
        g.circle(sx, sy, 9).stroke({ width: 3, color: 0xfde047 });
      }
    }
    // The street being drawn (or sent), on to the pointer, and where it would meet the others.
    if (mode === 'street' || this.sent) {
      const pts: Pt[] = this.sent ?? this.draft.map((s) => s.p);
      const next = !this.sent && this.cursor ? this.snap(this.cursor[0], this.cursor[1]) : null;
      const all = next ? [...pts, next.p] : pts;
      if (all.length >= 2) {
        const sp = all.map(scr);
        for (const pass of [0, 1]) {
          g.moveTo(sp[0][0], sp[0][1]);
          for (let k = 1; k < sp.length; k++) g.lineTo(sp[k][0], sp[k][1]);
          g.stroke(pass === 0 ? { width: 6, color: 0x1c1917, alpha: 0.45 } : { width: 3, color: INK, alpha: 0.95 });
        }
        for (const q of this.crossings(all)) {
          const [sx, sy] = scr(q);
          g.circle(sx, sy, 4).fill({ color: 0xfafaf9 }).stroke({ width: 2, color: INK });
        }
      }
      for (const s of this.sent ? [] : this.draft) {
        const [sx, sy] = scr(s.p);
        g.circle(sx, sy, 3.5).fill({ color: INK }).stroke({ width: 1, color: 0x1c1917 });
      }
      if (next) {
        const [sx, sy] = scr(next.p);
        if (next.on === 'junction') g.circle(sx, sy, 8).stroke({ width: 2.5, color: 0xfde047 });
        else if (next.on === 'street') g.poly([sx, sy - 7, sx + 7, sy, sx, sy + 7, sx - 7, sy]).stroke({ width: 2.5, color: 0xfde047 });
        else g.circle(sx, sy, 3).stroke({ width: 1.5, color: INK });
      }
    }
    // The brush's ring.
    if (mode !== 'select' && mode !== 'equalize' && mode !== 'street' && this.cursor) {
      const [sx, sy] = scr(this.drag?.kind === 'brush' && (mode === 'bloat' || mode === 'pinch') ? this.drag.start : this.cursor);
      g.circle(sx, sy, this.host.radius() * ppf).stroke({ width: 1.5, color: INK, alpha: 0.8 });
      g.circle(sx, sy, this.host.radius() * ppf * (mode === 'displace' ? SOFT : 0.04)).stroke({ width: 1, color: INK, alpha: 0.5 });
    }
    // Equalize: the blocks passed over, or the one under the pointer.
    const outline = (b: number, alpha: number) => {
      const r = this.rings.get(b);
      if (r) g.poly(this.ringNow(r).flatMap(scr)).stroke({ width: 2.5, color: INK, alpha });
    };
    if (this.drag?.kind === 'equalize') for (const b of this.drag.blocks) outline(b, 1);
    else if (mode === 'equalize' && this.cursor) {
      const b = this.blockAt(this.cursor[0], this.cursor[1]);
      if (b !== null) outline(b, 0.7);
    }
  }

  /** A face's ring with its junctions where they are shown. */
  private ringNow(r: Ring): Pt[] {
    if (!this.shown.size && !this.pending) return r.pts;
    return r.pts.map((p) => {
      for (const j of r.js) {
        const q = this.pos.get(j)!;
        if (q[0] === p[0] && q[1] === p[1]) return this.at(j);
      }
      return p;
    });
  }

  /** Where a line through `pts` crosses the streets (each new junction it would make). */
  private crossings(pts: Pt[]): Pt[] {
    const out: Pt[] = [];
    for (let k = 1; k < pts.length; k++) {
      const [a, b] = [pts[k - 1], pts[k]];
      const [bx0, by0, bx1, by1] = [Math.min(a[0], b[0]), Math.min(a[1], b[1]), Math.max(a[0], b[0]), Math.max(a[1], b[1])];
      for (const l of this.lines) {
        const p = this.pts(l);
        for (let m = 1; m < p.length; m++) {
          const [c, d] = [p[m - 1], p[m]];
          if (Math.max(c[0], d[0]) < bx0 || Math.min(c[0], d[0]) > bx1 || Math.max(c[1], d[1]) < by0 || Math.min(c[1], d[1]) > by1) continue;
          const q = cross(a, b, c, d);
          if (q && dist(q, a) > 1 && dist(q, b) > 1) out.push(q);
        }
      }
    }
    return out;
  }
}

/** The point of segment ab nearest p. */
function nearest(p: Pt, a: Pt, b: Pt): Pt {
  const ab = sub(b, a);
  const l2 = ab[0] * ab[0] + ab[1] * ab[1];
  const t = l2 > 0 ? Math.max(0, Math.min(1, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / l2)) : 0;
  return add(a, mul(ab, t));
}

/** Where segments ab and cd cross, if they do. */
function cross(a: Pt, b: Pt, c: Pt, d: Pt): Pt | null {
  const r = sub(b, a);
  const s = sub(d, c);
  const den = r[0] * s[1] - r[1] * s[0];
  if (Math.abs(den) < 1e-9) return null;
  const ca = sub(c, a);
  const t = (ca[0] * s[1] - ca[1] * s[0]) / den;
  const u = (ca[0] * r[1] - ca[1] * r[0]) / den;
  return t >= 0 && t <= 1 && u >= 0 && u <= 1 ? add(a, mul(r, t)) : null;
}

/** Whether `p` is inside polygon `poly` (even-odd). */
function inside(p: Pt, poly: Pt[]): boolean {
  let c = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [a, b] = [poly[i], poly[j]];
    if (a[1] > p[1] !== b[1] > p[1] && p[0] < ((b[0] - a[0]) * (p[1] - a[1])) / (b[1] - a[1]) + a[0]) c = !c;
  }
  return c;
}
