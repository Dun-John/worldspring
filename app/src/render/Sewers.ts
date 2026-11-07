// A city's sewers as one network. The generator lays them out in sections (480-ft squares of
// the world, each a site of its own whose tunnels meet its neighbours' at the shared edge);
// here the sections round the view are loaded and drawn edge to edge on the sewer level, so
// the tunnels simply run on. (The undercrofts below are a section's own: see `MapView`.)
import { Container, Sprite, Texture } from 'pixi.js';
import type { GenClient } from '../gen/client';
import type { Interior } from '../gen/protocol';
import type { Camera } from './camera';
import { InteriorLayer, UNDER_DIM, type InteriorStyle } from './InteriorLayer';
import type { UnderField } from './underField';
import type { FieldRequest } from './underField.worker';

/** Underground levels' textures are worked out in a worker (tens of milliseconds each): sewer
 * sections as they come into view, and any site's levels as it opens. */
let worker: Worker | null = null;
const waiting = new Map<number, (f: UnderField) => void>();
let nextField = 1;
export function prepareField(it: Interior, level: number): Promise<UnderField> {
  if (!worker) {
    worker = new Worker(new URL('./underField.worker.ts', import.meta.url), { type: 'module' });
    worker.onmessage = (e: MessageEvent<{ id: number; f: UnderField }>) => {
      waiting.get(e.data.id)?.(e.data.f);
      waiting.delete(e.data.id);
    };
  }
  const id = nextField++;
  return new Promise((resolve) => {
    waiting.set(id, resolve);
    worker!.postMessage({ id, it, level } satisfies FieldRequest);
  });
}

/** A section's side (ft; `under::SEWER_SECTION_FT`) and squares. */
export const SECTION_FT = 480;
export const SECTION_N = 96;
/** Sections kept in memory, and loaded beyond the edge of the view. */
const CAP = 64;
const REACH = 1;
/** Sections asked for and not back yet (flying in, the view moves on before they are). */
const IN_FLIGHT = 8;

interface Section {
  /** Null where no sewers run (no paved street above): plain rock is drawn there. */
  it: Interior | null;
  layer: InteriorLayer | null;
  lastUsed: number;
}

/** Plain rock where no sewers run (none drawn: the map above shows there, as round the tunnels). */
function rockSection(layout: number, sx: number, sy: number): Interior {
  const n = SECTION_N;
  return {
    id: `w:${layout}:${sx}:${sy}`,
    settlement: layout,
    building: 0,
    name: null,
    function: 'sewer',
    origin: [sx * SECTION_FT, sy * SECTION_FT],
    axis: [1, 0],
    across: [0, 1],
    nx: n,
    ny: n,
    levels: [{ z: -1, name: '', elevation_ft: 0, cells: new Array(n * n).fill(-1), rooms: [], walls: [], doors: [], windows: [], furniture: [], roof: false, has_stairs: false, natural: false }],
    entry_level: 0,
    stairs: [0, 0, 0, 0],
  };
}

export class Sewers {
  readonly container = new Container();
  private readonly dim = new Sprite(Texture.WHITE);
  private readonly sections = new Map<string, Section>();
  private readonly asked = new Set<string>();
  /** Sections arrived and prepared, not drawn yet (one is drawn a frame); `rock`: none there. */
  private readonly ready: { it: Interior; field: UnderField; rock: boolean }[] = [];
  private dead = false;
  private inFlight = 0;
  /** Whether a section is round the view now (the last `update`). */
  private near = (_sx: number, _sy: number) => true;
  /** Sections drawn so far (play mode looks again at what blocks sight when it grows). */
  loaded = 0;

  constructor(
    readonly layout: number,
    private readonly gen: GenClient,
    private readonly sea: number,
    private readonly style: InteriorStyle,
    first: Interior,
    firstField?: UnderField,
  ) {
    // Underground the surface is a ghost (it shows where the rock fades out, away from the tunnels).
    this.dim.tint = 0x14100c;
    this.dim.alpha = UNDER_DIM;
    this.container.addChild(this.dim);
    this.add(first, firstField);
  }

  /** The section holding a world point. */
  static sectionOf(wx: number, wy: number): [number, number] {
    return [Math.floor(wx / SECTION_FT), Math.floor(wy / SECTION_FT)];
  }

  /** A section's site id. */
  id(sx: number, sy: number): string {
    return `w:${this.layout}:${sx}:${sy}`;
  }

  /** The section of sewers under a world point (on rock too), if it is loaded (not where no
   * sewers run). */
  layerAt(wx: number, wy: number): InteriorLayer | null {
    const [sx, sy] = Sewers.sectionOf(wx, wy);
    const s = this.sections.get(`${sx},${sy}`);
    return s?.it ? s.layer : null;
  }

  /** A section's plan: null where there are none, undefined while not loaded. */
  interiorAt(sx: number, sy: number): Interior | null | undefined {
    return this.sections.get(`${sx},${sy}`)?.it;
  }

  /** The layer of a section given its plan (drawn now if it wasn't). */
  layerOf(it: Interior): InteriorLayer {
    const [sx, sy] = Sewers.sectionOf(it.origin[0] + 1, it.origin[1] + 1);
    return this.sections.get(`${sx},${sy}`)?.layer ?? this.add(it);
  }

  update(cam: Camera, now: number) {
    this.dim.width = cam.width;
    this.dim.height = cam.height;
    const [x0, y0, x1, y1] = cam.viewRect();
    const [sx0, sy0] = Sewers.sectionOf(x0, y0);
    const [sx1, sy1] = Sewers.sectionOf(x1, y1);
    const near = (sx: number, sy: number) => sx >= sx0 - REACH && sx <= sx1 + REACH && sy >= sy0 - REACH && sy <= sy1 + REACH;
    this.near = near;
    // Ask for the sections round the view (nearest the middle first).
    const [cx, cy] = Sewers.sectionOf(cam.cx, cam.cy);
    const want: [number, number][] = [];
    // (Not while the view is far wider than the sewers are drawn: flying down into them.)
    const many = (sx1 - sx0 + 1 + 2 * REACH) * (sy1 - sy0 + 1 + 2 * REACH) > CAP * 2;
    if (!many) for (let sy = sy0 - REACH; sy <= sy1 + REACH; sy++) for (let sx = sx0 - REACH; sx <= sx1 + REACH; sx++) if (!this.asked.has(`${sx},${sy}`)) want.push([sx, sy]);
    want.sort((a, b) => Math.hypot(a[0] - cx, a[1] - cy) - Math.hypot(b[0] - cx, b[1] - cy));
    // (A few at a time; what the view has left by the time it comes back is forgotten, to be
    // asked again if it comes back into view.)
    for (const [sx, sy] of want.slice(0, Math.max(0, IN_FLIGHT - this.inFlight))) {
      const key = `${sx},${sy}`;
      this.asked.add(key);
      this.inFlight++;
      void this.gen.interior(this.id(sx, sy)).then(async (found) => {
        if (this.dead) return;
        if (this.near(sx, sy)) {
          const it = found ?? rockSection(this.layout, sx, sy);
          const field = await prepareField(it, it.entry_level);
          if (!this.dead) this.ready.push({ it, field, rock: !found });
        } else this.asked.delete(key);
      }).catch(() => this.asked.delete(key)).finally(() => this.inFlight--);
    }
    // Draw one new section a frame (its textures and props), forgetting those out of view.
    let next;
    while ((next = this.ready.shift())) {
      const [sx, sy] = Sewers.sectionOf(next.it.origin[0] + 1, next.it.origin[1] + 1);
      if (near(sx, sy)) break;
      this.asked.delete(`${sx},${sy}`);
    }
    if (next) this.add(next.it, next.field, next.rock);
    for (const [key, s] of this.sections) {
      const [sx, sy] = key.split(',').map(Number);
      const show = near(sx, sy);
      if (show) s.lastUsed = now;
      if (!s.layer) continue;
      s.layer.container.visible = show;
      if (show) s.layer.update(cam);
    }
    if (this.sections.size > CAP) {
      const old = [...this.sections.entries()].filter(([, s]) => s.lastUsed < now).sort((a, b) => a[1].lastUsed - b[1].lastUsed);
      for (const [key, s] of old) {
        if (this.sections.size <= CAP) break;
        s.layer?.destroy();
        this.sections.delete(key);
        this.asked.delete(key);
      }
    }
  }

  /** Draw the sections again (doors opened or shut, play mode on or off). */
  redraw() {
    for (const s of this.sections.values()) s.layer?.redraw();
  }

  destroy() {
    this.dead = true;
    for (const s of this.sections.values()) s.layer?.destroy();
    this.sections.clear();
    this.container.destroy({ children: true });
  }

  private add(it: Interior, field?: UnderField, rock = false): InteriorLayer {
    const [sx, sy] = Sewers.sectionOf(it.origin[0] + 1, it.origin[1] + 1);
    const key = `${sx},${sy}`;
    const have = this.sections.get(key);
    if (have?.layer) return have.layer;
    const layer = new InteriorLayer(it, this.sea, false, false, this.style, true, field ? new Map([[it.entry_level, field]]) : undefined);
    this.container.addChild(layer.container);
    this.sections.set(key, { it: rock ? null : it, layer, lastUsed: performance.now() });
    this.asked.add(key);
    if (!rock) this.loaded++;
    return layer;
  }
}
