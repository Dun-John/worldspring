// Edit › Town: a town laid out anew by hand, on its plan's patches (the ward editor). The
// patches are drawn over the map, each tinted by its ward. Choose (the default) picks a patch on
// a click and drags one corner at a time; the brushes move every corner within their ring, as
// watabou's Warp tools do: Displace (the drag, full strength in the middle half), Liquify (smudged
// along the drag), Bloat and Pinch (away from or toward where the press began, harder the further
// the drag goes), Relax (toward the middle of their neighbours, as the brush is scrubbed) and
// Equalize (each patch passed over toward a regular polygon, worked out by the generator).
//
// Nothing is laid out while dragging: the corners and edges move, and on release the moves go to
// the app as one change. Each corner goes only as far as keeps every patch round it convex (as
// the generator moves them), and no further than a patch's width from where it was planned;
// corners on the water or a river never move.
import type { Graphics } from 'pixi.js';
import type { TownPlan, TownWard } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { PointerTool } from '../render/MapView';

export type TownMode = 'select' | 'displace' | 'liquify' | 'bloat' | 'pinch' | 'relax' | 'equalize';
type Pt = [number, number];

/** Each ward's tint on the map (and its swatch in the panel). */
export const WARD_COLOURS: Record<TownWard, number> = {
  plaza: 0xf0cf6a,
  castle: 0xb3473a,
  temple: 0xf2ead0,
  merchant: 0xe6923a,
  craft: 0x9c7446,
  noble: 0x8d5bc9,
  common: 0x7d9cb4,
  slum: 0x5f5a4c,
  docks: 0x3a86c8,
  military: 0xc23b5a,
  farm: 0x9cc65a,
  park: 0x3d9f5c,
  empty: 0xc9c9c9,
};

/** A brush's corners within the middle half of its ring move all the way (Displace). */
const SOFT = 0.5;
/** Grabbing a corner: within this many screen px. */
const GRAB_PX = 10;
/** Patches smaller than this on screen (px across) aren't drawn. */
const MIN_PATCH_PX = 10;

/** What the tool needs from the app. */
export interface TownHost {
  mode(): TownMode;
  /** The brush's radius (ft). */
  radius(): number;
  /** The patch picked, if any. */
  selected(): number | null;
  /** Corners moved (world ft): one change. */
  moved(moves: { corner: number; to: Pt }[]): void;
  /** Patches brushed with Equalize: one change. */
  equalize(patches: number[]): void;
  hint(text: string): void;
}

const sub = (a: Pt, b: Pt): Pt => [a[0] - b[0], a[1] - b[1]];
const add = (a: Pt, b: Pt): Pt => [a[0] + b[0], a[1] + b[1]];
const mul = (a: Pt, k: number): Pt => [a[0] * k, a[1] * k];
const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const lerp = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];

type Drag =
  | { kind: 'corner'; v: number }
  | { kind: 'brush'; mode: TownMode; start: Pt; last: Pt; base: Map<number, Pt>; weight: Map<number, number> }
  | { kind: 'equalize'; patches: Set<number> };

export class TownTool implements PointerTool {
  private plan: TownPlan | null = null;
  /** Corners where they stand, where they were planned; pinned ones. */
  private pos = new Map<number, Pt>();
  private planned = new Map<number, Pt>();
  private pinned = new Set<number>();
  private gate = new Set<number>();
  /** Patches by id: their corners; each corner's patches; corners next to each corner. */
  private faces = new Map<number, number[]>();
  private ward = new Map<number, TownWard>();
  private vf = new Map<number, number[]>();
  private adj = new Map<number, number[]>();
  /** Edges once each (corner pairs). */
  private edges: [number, number][] = [];
  private cap = 0;
  private drag: Drag | null = null;
  /** Where the corners touched should go (clamped), and where they get to (kept convex). */
  private target = new Map<number, Pt>();
  private shown = new Map<number, Pt>();
  /** The moves sent, shown until the town's plan comes back. */
  private pending: Map<number, Pt> | null = null;
  private cursor: Pt | null = null;
  /** Counts changes to what is drawn (`drawKey`). */
  private version = 0;

  constructor(private readonly host: TownHost) {}

  /** The town whose plan the tool works on (null: none). */
  setPlan(plan: TownPlan | null) {
    this.version++;
    this.plan = plan;
    this.pending = null;
    this.drag = null;
    this.target.clear();
    this.shown.clear();
    this.pos.clear();
    this.planned.clear();
    this.pinned.clear();
    this.gate.clear();
    this.faces.clear();
    this.ward.clear();
    this.vf.clear();
    this.adj.clear();
    this.edges = [];
    if (!plan) return;
    this.cap = plan.max_move_ft;
    for (const c of plan.corners) {
      this.pos.set(c.corner, c.at);
      this.planned.set(c.corner, c.planned ?? c.at);
      if (c.pinned) this.pinned.add(c.corner);
      if (c.gate) this.gate.add(c.corner);
    }
    const seen = new Set<string>();
    const link = (a: number, b: number) => {
      const list = this.adj.get(a) ?? [];
      if (!list.includes(b)) list.push(b);
      this.adj.set(a, list);
    };
    for (const p of plan.patches) {
      this.faces.set(p.patch, p.corners);
      this.ward.set(p.patch, p.ward);
      const m = p.corners.length;
      for (let k = 0; k < m; k++) {
        const [a, b] = [p.corners[k], p.corners[(k + 1) % m]];
        this.vf.set(a, [...(this.vf.get(a) ?? []), p.patch]);
        link(a, b);
        link(b, a);
        const key = a < b ? `${a},${b}` : `${b},${a}`;
        if (!seen.has(key)) {
          seen.add(key);
          this.edges.push([a, b]);
        }
      }
    }
  }

  /** A corner as shown: being moved, sent, or where it stands. */
  private at(v: number): Pt {
    return this.shown.get(v) ?? this.pending?.get(v) ?? this.pos.get(v)!;
  }

  /** The patch under (x, y), if any. */
  patchAt(x: number, y: number): number | null {
    for (const [p, corners] of this.faces) if (inside([x, y], corners.map((v) => this.at(v)))) return p;
    return null;
  }

  /** The movable corner nearest (x, y) within `r` ft. */
  private cornerAt(x: number, y: number, r: number): number | null {
    let best: number | null = null;
    let bestD = r;
    for (const [v, p] of this.pos) {
      const d = dist(p, [x, y]);
      if (d < bestD) [best, bestD] = [v, d];
    }
    return best;
  }

  /** Where corner `v` may go toward `to`: within a patch's width of where it was planned. */
  private clamp(v: number, to: Pt): Pt {
    const from = this.planned.get(v)!;
    const d = dist(from, to);
    return d > this.cap ? add(from, mul(sub(to, from), this.cap / d)) : to;
  }

  /** Whether patch `f` stays convex with corner `v` at `q` (the generator's rule). */
  private convex(f: number, v: number, q: Pt, over: Map<number, Pt>): boolean {
    const face = this.faces.get(f)!;
    const m = face.length;
    if (m < 3) return true;
    const at = (u: number) => (u === v ? q : (over.get(u) ?? this.pos.get(u)!));
    let sign = 0;
    for (let k = 0; k < m; k++) {
      const [a, b, c] = [at(face[k]), at(face[(k + 1) % m]), at(face[(k + 2) % m])];
      const cr = (b[0] - a[0]) * (c[1] - b[1]) - (b[1] - a[1]) * (c[0] - b[0]);
      if (Math.abs(cr) < 1e-6) continue;
      if (sign === 0) sign = Math.sign(cr);
      else if (Math.sign(cr) !== sign) return false;
    }
    return sign !== 0;
  }

  /** The targets moved as the generator moves them: in ascending corner, each as far toward its
   * target as keeps its patches convex (the whole way, half, a quarter), up to four rounds. */
  private settle() {
    const over = new Map<number, Pt>();
    const order = [...this.target.keys()].sort((a, b) => a - b);
    for (let round = 0; round < 4; round++) {
      let any = false;
      for (const v of order) {
        const p = over.get(v) ?? this.pos.get(v)!;
        const to = this.target.get(v)!;
        if (dist(p, to) <= 0.01) continue;
        for (const step of [1, 0.5, 0.25]) {
          const q = lerp(p, to, step);
          if ((this.vf.get(v) ?? []).every((f) => this.convex(f, v, q, over))) {
            over.set(v, q);
            any = true;
            break;
          }
        }
      }
      if (!any) break;
    }
    this.shown = over;
  }

  down(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0 || !this.plan || this.pending) return false;
    this.version++;
    const mode = this.host.mode();
    const p: Pt = [x, y];
    this.cursor = p;
    if (mode === 'select') {
      const v = this.cornerAt(x, y, GRAB_PX / Math.max(1e-6, this.ppf));
      if (v === null) return false;
      if (this.pinned.has(v)) {
        this.host.hint('That corner is on the water or a river: it stays put');
        return true;
      }
      this.drag = { kind: 'corner', v };
      return true;
    }
    if (mode === 'equalize') {
      this.drag = { kind: 'equalize', patches: new Set() };
      this.move(x, y);
      return true;
    }
    const base = new Map<number, Pt>();
    for (const [v, q] of this.pos) base.set(v, q);
    const drag: Drag = { kind: 'brush', mode, start: p, last: p, base, weight: new Map() };
    this.drag = drag;
    if (mode === 'displace') {
      const r = this.host.radius();
      for (const [v, q] of this.pos) {
        const d = dist(q, p);
        if (d < r && !this.pinned.has(v)) drag.weight.set(v, Math.min(1, (1 - d / r) / SOFT));
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
    if (d.kind === 'corner') {
      this.target.set(d.v, this.clamp(d.v, p));
      this.settle();
      return;
    }
    if (d.kind === 'equalize') {
      const f = this.patchAt(x, y);
      if (f !== null) d.patches.add(f);
      return;
    }
    const r = this.host.radius();
    const now = (v: number) => this.target.get(v) ?? this.pos.get(v)!;
    const set = (v: number, to: Pt, w: number) => {
      this.target.set(v, this.clamp(v, to));
      d.weight.set(v, Math.max(w, d.weight.get(v) ?? 0));
    };
    if (d.mode === 'displace') {
      const by = sub(p, d.start);
      for (const [v, w] of d.weight) this.target.set(v, this.clamp(v, add(d.base.get(v)!, mul(by, w))));
    } else if (d.mode === 'liquify') {
      const by = sub(p, d.last);
      for (const v of this.pos.keys()) {
        if (this.pinned.has(v)) continue;
        const k = 1 - dist(now(v), p) / r;
        if (k > 0) set(v, add(now(v), mul(by, 0.5 * k)), k);
      }
    } else if (d.mode === 'bloat' || d.mode === 'pinch') {
      // From where the press began: the further the drag, the harder.
      const a = Math.min(2, dist(p, d.start) / r);
      const e = d.mode === 'bloat' ? 1 / (1 + a) : 1 + a;
      for (const [v, q] of d.base) {
        if (this.pinned.has(v)) continue;
        const dd = dist(q, d.start);
        if (dd >= r || dd < 1e-6) continue;
        const to = add(d.start, mul(sub(q, d.start), (r * Math.pow(dd / r, e)) / dd));
        set(v, to, 1 - dd / r);
      }
    } else if (d.mode === 'relax') {
      const s = Math.min(1, (2 * dist(p, d.last)) / r);
      const moves: [number, Pt, number][] = [];
      for (const v of this.pos.keys()) {
        if (this.pinned.has(v)) continue;
        const k = 1 - dist(now(v), p) / r;
        const nb = this.adj.get(v);
        if (k <= 0 || !nb?.length) continue;
        const mean = mul(nb.reduce<Pt>((acc, u) => add(acc, now(u)), [0, 0]), 1 / nb.length);
        moves.push([v, lerp(now(v), mean, s * k), k]);
      }
      for (const [v, to, k] of moves) set(v, to, k);
    }
    d.last = p;
    this.settle();
  }

  up(x: number, y: number) {
    this.move(x, y);
    const d = this.drag;
    this.drag = null;
    if (!d) return;
    if (d.kind === 'equalize') {
      if (d.patches.size) this.host.equalize([...d.patches]);
      return;
    }
    const moves: { corner: number; to: Pt }[] = [];
    for (const [v, q] of this.shown) if (dist(q, this.pos.get(v)!) > 0.5) moves.push({ corner: v, to: [Math.round(q[0] * 10) / 10, Math.round(q[1] * 10) / 10] });
    const shown = this.shown;
    this.target = new Map();
    this.shown = new Map();
    if (!moves.length) {
      if (d.kind === 'corner' && dist([x, y], this.pos.get(d.v)!) > 1) this.host.hint('That corner can’t go there: the patches round it must stay convex');
      return;
    }
    this.pending = shown;
    this.version++;
    this.host.moved(moves);
  }

  cancel() {
    this.version++;
    this.drag = null;
    this.target.clear();
    this.shown.clear();
  }

  /** The change was refused or failed: the corners back where they stand. */
  settled() {
    this.version++;
    this.pending = null;
  }

  /** Drop a drag in progress. True if there was one. */
  reset(): boolean {
    const had = !!this.drag;
    this.cancel();
    return had;
  }

  hover(x: number, y: number) {
    this.cursor = [x, y];
    // (Only the brushes draw anything where the pointer is.)
    if (this.host.mode() !== 'select') this.version++;
  }

  drawKey(): string {
    return `${this.version}|${this.host.mode()}|${this.host.selected()}|${this.host.radius()}`;
  }

  dblclick(): boolean {
    // (Not into a building while laying the town out.)
    return true;
  }

  /** Pixels per foot at the last frame. */
  private ppf = 1;

  draw(g: Graphics, cam: Camera) {
    this.ppf = cam.ppf;
    const plan = this.plan;
    if (!plan) return;
    const ppf = cam.ppf;
    if (plan.max_move_ft * ppf < MIN_PATCH_PX) return;
    const [x0, y0, x1, y1] = cam.viewRect();
    // (A quarter of the view beyond each side: the map shifts this drawing as it pans.)
    const pad = Math.max(plan.max_move_ft * 2, (x1 - x0) / 4 + plan.max_move_ft);
    const inView = (p: Pt) => p[0] > x0 - pad && p[0] < x1 + pad && p[1] > y0 - pad && p[1] < y1 + pad;
    const scr = (p: Pt) => cam.worldToScreen(p[0], p[1]);
    const sel = this.host.selected();
    const mode = this.host.mode();
    // Each patch tinted by its ward.
    for (const [p, corners] of this.faces) {
      const pts = corners.map((v) => this.at(v));
      if (!pts.some(inView)) continue;
      const colour = WARD_COLOURS[this.ward.get(p) ?? 'empty'];
      g.poly(pts.flatMap(scr)).fill({ color: colour, alpha: p === sel ? 0.34 : 0.14 });
    }
    // Edges: dark under light, so they show on any ground.
    const touched = (v: number) => this.shown.has(v) || !!this.pending?.has(v);
    for (const pass of [0, 1]) {
      for (const [a, b] of this.edges) {
        const [pa, pb] = [this.at(a), this.at(b)];
        if (!inView(pa) && !inView(pb)) continue;
        const [sa, sb] = [scr(pa), scr(pb)];
        g.moveTo(sa[0], sa[1]).lineTo(sb[0], sb[1]);
      }
      g.stroke(pass === 0 ? { width: 3, color: 0x1c1917, alpha: 0.35 } : { width: 1.2, color: 0xfafaf9, alpha: 0.75 });
    }
    // The edges moving, in the tool's colour.
    const ink = 0xf97316;
    if (this.shown.size || this.pending) {
      for (const [a, b] of this.edges) {
        if (!touched(a) && !touched(b)) continue;
        const [sa, sb] = [scr(this.at(a)), scr(this.at(b))];
        g.moveTo(sa[0], sa[1]).lineTo(sb[0], sb[1]);
      }
      g.stroke({ width: 2, color: ink, alpha: 0.9 });
    }
    // The patch picked, outlined.
    const picked = sel !== null ? this.faces.get(sel) : undefined;
    if (picked) g.poly(picked.map((v) => this.at(v)).flatMap(scr)).stroke({ width: 3, color: 0xfde047 });
    // Corners, once patches are big enough to tell them apart: movable ones round, those
    // moved by hand orange, those on the water locked (a grey square).
    if (plan.max_move_ft * ppf >= 40) {
      const weight = this.drag?.kind === 'brush' ? this.drag.weight : null;
      for (const v of this.pos.keys()) {
        const p = this.at(v);
        if (!inView(p)) continue;
        const [sx, sy] = scr(p);
        if (this.pinned.has(v)) {
          g.rect(sx - 3, sy - 3, 6, 6).fill({ color: 0x78716c }).stroke({ width: 1, color: 0x1c1917 });
          continue;
        }
        const moved = dist(this.planned.get(v)!, p) > 0.5;
        const w = weight?.get(v);
        const r = this.gate.has(v) ? 4.5 : 3;
        g.circle(sx, sy, r).fill({ color: w ? ink : moved ? 0xfb923c : 0xffffff, alpha: w ? 0.35 + 0.65 * w : 1 }).stroke({ width: 1, color: 0x1c1917 });
      }
    }
    // Dragging one corner: how far it may go from where it was planned.
    if (this.drag?.kind === 'corner') {
      const [sx, sy] = scr(this.planned.get(this.drag.v)!);
      g.circle(sx, sy, this.cap * ppf).stroke({ width: 1, color: ink, alpha: 0.5 });
    }
    // The brush's ring.
    if (mode !== 'select' && mode !== 'equalize' && this.cursor) {
      const [sx, sy] = scr(this.drag?.kind === 'brush' && (mode === 'bloat' || mode === 'pinch') ? this.drag.start : this.cursor);
      g.circle(sx, sy, this.host.radius() * ppf).stroke({ width: 1.5, color: ink, alpha: 0.8 });
      g.circle(sx, sy, this.host.radius() * ppf * (mode === 'displace' ? SOFT : 0.04)).stroke({ width: 1, color: ink, alpha: 0.5 });
    }
    // Equalize: the patches passed over.
    if (this.drag?.kind === 'equalize') {
      for (const p of this.drag.patches) g.poly(this.faces.get(p)!.map((v) => this.at(v)).flatMap(scr)).stroke({ width: 2.5, color: ink });
    } else if (mode === 'equalize' && this.cursor) {
      const p = this.patchAt(this.cursor[0], this.cursor[1]);
      if (p !== null) g.poly(this.faces.get(p)!.map((v) => this.at(v)).flatMap(scr)).stroke({ width: 2, color: ink, alpha: 0.7 });
    }
  }
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
