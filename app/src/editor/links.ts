// Rooms › Ways across: a paired exit (`Edits.links`) put down end to end. Click a floor square
// below ground for the first end, then (another level by the floor picker, or another place by
// search) a square for the other; the way and name come from the panel. A click on an end put
// down earlier picks its link (to change or take away). Escape drops the first end. Taps work
// the same on a touch screen; dragging pans.
import type { Graphics } from 'pixi.js';
import type { Edits, LinkEnd } from '../gen/protocol';
import type { Camera } from '../render/camera';
import type { InteriorLayer } from '../render/InteriorLayer';
import type { MapView, PointerTool } from '../render/MapView';
import { SECTION_FT } from '../render/Sewers';

export interface LinkHost {
  edits(): Edits;
  /** The first end put down, if any. */
  first(): LinkEnd | null;
  /** The link picked (`j:<id>`), if any. */
  picked(): string | null;
  /** A floor square clicked: an end of a link put down earlier (`link`), else a new end. */
  square(end: LinkEnd, link: string | null): void;
  /** Why a click did nothing, at the point. */
  chip(x: number, y: number, text: string): void;
}

/** A level id's site and level (`l:<site>:<level>`). */
export function endSite(level: string): { site: string; level: number } {
  const at = level.lastIndexOf(':');
  return { site: level.slice(2, at), level: Number(level.slice(at + 1)) };
}

/** The link with an end on this square, if any. */
export function linkAtEnd(e: Edits, end: LinkEnd): string | null {
  for (const [id, l] of Object.entries(e.links ?? {})) {
    for (const p of [l.a, l.b]) if (p.level === end.level && p.x === end.x && p.y === end.y) return id;
  }
  return null;
}

/** Whether a level of a site can take a way across: below ground (sites, sewers, keeps' deep
 * dungeons; a building's cellars). */
export function linkable(layer: InteriorLayer): boolean {
  return layer.underground || (layer.interior.id.startsWith('b:') && (layer.interior.levels[layer.currentLevel]?.z ?? 0) < 0);
}

export class LinkTool implements PointerTool {
  readonly hoverFollows = true;
  private cursor: { layer: InteriorLayer; i: number; j: number } | null = null;

  constructor(
    private view: MapView,
    private host: LinkHost,
  ) {}

  down(): boolean {
    return false;
  }
  move() {}
  up() {}
  cancel() {}

  hover(x: number, y: number) {
    this.cursor = this.squareAt(x, y);
  }

  leave() {
    this.cursor = null;
  }

  tap(x: number, y: number, e: PointerEvent): boolean {
    if (e.button !== 0) return false;
    const layer = this.view.interiorAt(x, y);
    if (!layer) return false;
    const sq = this.squareAt(x, y);
    if (!sq) {
      this.host.chip(x, y, linkable(layer) ? 'Pick a floor square' : 'Ways across go below ground: a site, the sewers or a cellar');
      return true;
    }
    const end: LinkEnd = { level: `l:${sq.layer.interior.id}:${sq.layer.currentLevel}`, x: sq.i, y: sq.j };
    this.host.square(end, linkAtEnd(this.host.edits(), end));
    return true;
  }

  /** The floor square under a world point on a level that can take a way across. */
  private squareAt(x: number, y: number): { layer: InteriorLayer; i: number; j: number } | null {
    const layer = this.view.interiorAt(x, y);
    if (!layer || !linkable(layer)) return null;
    const it = layer.interior;
    const [i, j] = layer.toGrid(x, y).map(Math.floor);
    if (i < 0 || j < 0 || i >= it.nx || j >= it.ny || (it.levels[layer.currentLevel]?.cells[j * it.nx + i] ?? -1) < 0) return null;
    return { layer, i, j };
  }

  /** The layer showing a level (in the sewers, its section), if it is in view. */
  private layerOf(level: string): InteriorLayer | null {
    const { site, level: li } = endSite(level);
    const v = this.view;
    const sw = /^w:(\d+):(-?\d+):(-?\d+)$/.exec(site);
    const layer = v.sewers && sw && Number(sw[1]) === v.sewers.layout ? v.sewers.layerAt((Number(sw[2]) + 0.5) * SECTION_FT, (Number(sw[3]) + 0.5) * SECTION_FT) : v.interior;
    return layer && layer.interior.id === site && layer.currentLevel === li ? layer : null;
  }

  draw(g: Graphics, cam: Camera) {
    const ink = 0xf97316;
    const square = (layer: InteriorLayer, i: number, j: number) => {
      const pts = [
        [i, j],
        [i + 1, j],
        [i + 1, j + 1],
        [i, j + 1],
      ].map(([a, b]) => cam.worldToScreen(...layer.toWorld(a, b)));
      g.poly(pts.flat());
    };
    const picked = this.host.picked();
    for (const [id, l] of Object.entries(this.host.edits().links ?? {})) {
      const on = id === picked;
      for (const e of [l.a, l.b]) {
        const layer = this.layerOf(e.level);
        if (!layer) continue;
        square(layer, e.x, e.y);
        g.fill({ color: ink, alpha: on ? 0.3 : 0.08 }).stroke({ width: on ? 2.5 : 1, color: ink, alpha: on ? 1 : 0.6 });
      }
    }
    const first = this.host.first();
    const fl = first && this.layerOf(first.level);
    if (first && fl) {
      square(fl, first.x, first.y);
      g.fill({ color: ink, alpha: 0.4 }).stroke({ width: 2.5, color: 0xffffff });
    }
    const c = this.cursor;
    if (c && c.layer.interior === (this.view.interiorAt(...c.layer.toWorld(c.i + 0.5, c.j + 0.5))?.interior ?? null)) {
      square(c.layer, c.i, c.j);
      g.stroke({ width: 1.5, color: ink });
    }
  }
}

/** A new link id: random, so the app and the server putting one down at once never collide. */
export function newLinkId(): string {
  const b = new Uint8Array(8);
  crypto.getRandomValues(b);
  return `j:${[...b].map((x) => '0123456789abcdefghijklmnopqrstuvwxyz'[x % 36]).join('')}`;
}
