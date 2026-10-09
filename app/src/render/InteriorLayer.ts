// A building's interior (or an underground site), drawn over the battlemap at its position
// and rotation: floors by room, walls, doors, windows, stairs and furniture for one level at
// a time, with the rest of the map dimmed. Underground, rock fills the site's grid and the
// surface shows through only faintly; caves get rough, chamfered rock edges. Geometry is in
// the interior's own 5-ft grid units; the container maps them to the screen every frame.
import { BufferImageSource, Container, Geometry, GlProgram, Graphics, Mesh, RenderTexture, Shader, Sprite, Text, TextStyle, Texture, UniformGroup, type Renderer } from 'pixi.js';
import type { Interior, InteriorItem, InteriorLevel } from '../gen/protocol';
import { locKey, type DoorState } from '../play/state';
import { dmOnlyHazard } from '../play/vision';
import { customTexture, onLoaded } from './customAtlas';
import { drawFurniture, drawUnderProp, ORIENTED, UNDER_DRAWN, VARIED } from './furniture';
import { ITEM_PX, ItemAtlas } from './itemAtlas';
import type { SquareInfo } from './BattlemapLayer';
import type { Camera } from './camera';
import { noiseSource } from './noise';
import { SQUARE_SITES, underField, type UnderField } from './underField';
import undergroundFragment from './shaders/underground.frag?raw';
import vertex from './shaders/terrain.vert?raw';

const SQUARE_FT = 5;
/** Underground props drawn against the wall beside them (their look depends on its side). */
const ON_WALL = new Set(['tunnel', 'cobweb', 'ore_vein', 'niche', 'chains', 'sconce', 'banner', 'pipe', 'ladder']);
/** Underground props with a back against a wall (a dais's throne). */
const BACKED = new Set(['dais']);
/** Underground props drawn like the building furniture of the same name (`bookshelf` as a bookcase). */
const SHARED: Record<string, string> = {
  chest: 'chest', barrel: 'barrel', crate: 'crate', table: 'table', rug: 'rug', altar: 'altar', statue: 'statue', pillar: 'pillar',
  cage: 'cage', rack: 'rack', weapon_rack: 'weapon_rack', cot: 'cot', sarcophagus: 'sarcophagus', winch: 'winch', trapdoor: 'trapdoor',
  bookshelf: 'bookcase',
};
/** Props scattered differently per placement: one of four variants, picked by position. */
const SCATTERED = new Set(['boulder', 'guano', 'moss', 'rubble', 'debris', 'sulfur', 'ore_vein', 'nest', 'rat_nest']);
const INK = 0x1d1a14;
const COVER = ['no cover', 'half cover', 'three-quarters cover', 'total cover'];

// Underground levels are painted by a shader (`underground.frag`) from a smooth floor field
// (`underField.ts`).
const underProgram = GlProgram.from({ vertex, fragment: undergroundFragment, name: 'underground' });
let underQuad: Geometry | null = null;
const quad = () =>
  (underQuad ??= new Geometry({
    attributes: {
      aPosition: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' },
      aUV: { buffer: new Float32Array([0, 0, 1, 0, 1, 1, 0, 1]), format: 'float32x2' },
    },
    indexBuffer: new Uint32Array([0, 1, 2, 0, 2, 3]),
  }));

/** Floor colour by room kind. */
/** One of four scatter variants for an item at grid square (x, y). */
function itemVariant(x: number, y: number): number {
  const v = Math.sin(x * 127.1 + y * 311.7) * 43758.5453;
  return Math.floor((v - Math.floor(v)) * 4);
}

function floorColor(kind: string, z: number, natural = false, raise = 0): number {
  if (natural) {
    if (kind === 'boss chamber') return 0x7d7064;
    return raise >= 10 ? 0xa39886 : raise >= 5 ? 0x988d7c : 0x8a8072;
  }
  if (kind === 'boss chamber') return 0x8d8579;
  if (kind === 'sewer tunnel' || kind === 'service passage') return 0x8a7d6c;
  if (['arena floor'].includes(kind)) return 0xc8b184;
  if (['kitchen', 'baths', 'forge', 'brewhouse'].includes(kind)) return 0x8f8a80;
  if (kind === 'battlements') return 0xa8a296;
  if (kind === 'stands') return 0x8a7a64;
  if (z < 0 || ['nave', 'sanctuary', 'crypt', 'ossuary', 'great hall', 'mess hall', 'dungeon', 'guardroom', 'armory', 'vault', 'chapel', 'courtroom', 'counting house', 'hall', 'entry hall', 'foyer', 'holding pen', 'beast pen', 'cell', 'warehouse floor'].includes(kind))
    return 0x9d978c;
  return 0xa87a4f;
}
const STONE = (c: number) => c === 0x9d978c || c === 0x8f8a80 || c === 0xa8a296;

/** Furniture fill by kind. */
function itemColor(kind: string): number {
  switch (kind) {
    case 'bed':
    case 'cot':
      return 0x7d5a3c;
    case 'rug':
      return 0x8b3a3a;
    case 'hearth':
    case 'oven':
    case 'forge':
      return 0x4a4440;
    case 'barrel':
    case 'keg_rack':
    case 'vat':
      return 0x6e4a2c;
    case 'crate':
      return 0x9a7a4e;
    case 'altar':
    case 'statue':
    case 'pillar':
    case 'sarcophagus':
    case 'bell':
      return 0xb8b2a6;
    case 'cage':
      return 0x55524d;
    case 'trapdoor':
      return 0x6a4a2a;
    case 'bath':
      return 0x7fa3b5;
    case 'telescope':
    case 'anvil':
      return 0x3f4448;
    default:
      return 0x5e4028;
  }
}

/**
 * Compile the underground shader up front (by drawing a tiny site once, offscreen), instead
 * of stalling for a third of a second the first time anyone goes underground.
 */
export function warmUnderground(renderer: Renderer) {
  const it: Interior = {
    id: 'u:warmup',
    settlement: 0,
    building: 0,
    name: null,
    function: 'dungeon',
    origin: [0, 0],
    axis: [1, 0],
    across: [0, 1],
    nx: 2,
    ny: 2,
    levels: [{ z: -1, name: '', elevation_ft: 0, cells: [0, 0, -1, 0], rooms: [{ kind: 'chamber', squares: 3, raise_ft: 0, center: [1, 1] }], walls: [], doors: [], windows: [], furniture: [], roof: false, has_stairs: false, natural: false }],
    entry_level: 0,
    stairs: [0, 0, 0, 0],
  };
  const layer = new InteriorLayer(it, 0);
  const target = RenderTexture.create({ width: 4, height: 4 });
  renderer.render({ container: layer.container, target });
  target.destroy(true);
  layer.destroy();
}

/** A way from one square to somewhere else: another level of this site, out to the surface,
 * or another site (trapdoor, stairs, tunnel). `from`: the site it starts in (in a city's
 * sewers, the section under the square); `at`: where it is (world ft; for a front door, the
 * street outside it), so those who take it can arrive there. */
export type Move =
  | { kind: 'level'; label: string; level: number; from: string; at: [number, number] }
  | { kind: 'surface'; label: string; at: [number, number] }
  | { kind: 'site'; label: string; to: string; from: string; at: [number, number] };

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** Squares from the world's corner, wrapped every 256 sewer sections (96 squares each). */
const wrap = (v: number) => (((Math.round(v) % (256 * 96)) + 256 * 96) % (256 * 96));

/** How interiors are drawn for play: for the players' eyes (hidden hazards and undiscovered
 * secret doors left out), and with doors shut or open (`doors`, by location key and door
 * index; null outside play mode: doors are drawn ajar). */
/** An NPC placed in a building or site (the DM's notebook): on a level, at a world point (else
 * in the level's biggest room). */
export interface NpcMarker {
  id: string;
  name: string;
  level: number;
  at?: [number, number];
}

export interface InteriorStyle {
  player: boolean;
  doors: ((loc: string, index: number) => DoorState) | null;
}

export class InteriorLayer {
  /** The world's renames, for level and room names (`l:`/`r:` ids); set by `MapView`. */
  static names: Record<string, string> = {};
  /** The renderer furniture and props are drawn into the item atlas with (`MapView`, the gallery);
   * without one they are drawn as vector paths. */
  static renderer: Renderer | null = null;
  readonly container = new Container();
  private readonly dim = new Sprite(Texture.WHITE);
  private readonly plan = new Container();
  private readonly labels = new Container();
  private level = 0;
  private labelSpots: { text: Text; x: number; y: number; min: number }[] = [];
  /** NPCs placed here, drawn for the DM only (screen space, like the labels). */
  private readonly marks = new Container();
  private npcs: NpcMarker[] = [];
  private markSpots: { node: Container; x: number; y: number }[] = [];
  /** The underground level's shader mesh, its uniforms and textures. */
  private under: { uniforms: UniformGroup; sources: BufferImageSource[]; shader: Shader } | null = null;
  /** Underground levels' textures, worked out once each (`underField`). */
  private readonly fields = new Map<number, UnderField>();
  /** Toward the light (screen NW) in grid axes, rounded to 45 degrees, for items' shading. */
  private light: [number, number] = [-Math.SQRT1_2, -Math.SQRT1_2];

  /** `peek`: the ground floor shown through a faded roof while hovering (no dimming or
   * labels). `ghost`: faint, floors only. `tile`: a section of a city's sewers drawn with the
   * others round it (`Sewers`: they dim the map; tunnels run to the edge). */
  constructor(
    readonly interior: Interior,
    private readonly sea: number,
    readonly peek = false,
    readonly ghost = false,
    private readonly style: InteriorStyle = { player: false, doors: null },
    readonly tile = false,
    /** Textures already worked out (in a worker), by level. */
    fields?: Map<number, UnderField>,
    /** The level shown first (else the one entered on). */
    start?: number,
  ) {
    for (const [l, f] of fields ?? []) this.fields.set(l, f);
    this.dim.tint = 0x14100c;
    // Underground the surface is only a ghost.
    this.dim.alpha = this.underground ? 0.78 : 0.55;
    this.dim.visible = !peek && !ghost && !tile;
    this.labels.visible = !peek && !ghost;
    this.marks.visible = !peek && !ghost && !style.player;
    this.container.addChild(this.dim, this.plan, this.labels, this.marks);
    this.level = Math.max(0, Math.min(interior.levels.length - 1, start ?? interior.entry_level));
    // Light from the NW of the screen, in grid axes, rounded to 45 degrees so items lit alike share
    // one look in the item atlas.
    const [lx, ly] = [-Math.SQRT1_2, -Math.SQRT1_2];
    const la = Math.round(Math.atan2(lx * interior.across[0] + ly * interior.across[1], lx * interior.axis[0] + ly * interior.axis[1]) / (Math.PI / 4)) * (Math.PI / 4);
    this.light = [Math.cos(la), Math.sin(la)];
    this.draw();
  }

  /** An underground site (rock all round, levels below the surface). */
  get underground(): boolean {
    // (`u:` sites behind entrances, `w:` sewer sections, `k:` keeps' deep dungeons.)
    return /^[uwk]:/.test(this.interior.id);
  }

  /** The site a world point's square leads to (a trapdoor, stairs, ladder or tunnel to
   * another site), else null. */
  linkAt(wx: number, wy: number): string | null {
    const [gx, gy] = this.toGrid(wx, wy);
    const [i, j] = [Math.floor(gx), Math.floor(gy)];
    return this.interior.levels[this.level].links?.find((k) => k.x === i && k.y === j)?.to ?? null;
  }

  /** The ways on from the square under a world point (stairs and ladders up and down, the way
   * out, trapdoors and tunnels to other sites, a sewer run into the next section), first the
   * one a double-click takes. */
  movesAt(wx: number, wy: number): Move[] {
    const it = this.interior;
    const [gx, gy] = this.toGrid(wx, wy);
    const [i, j] = [Math.floor(gx), Math.floor(gy)];
    if (i < 0 || j < 0 || i >= it.nx || j >= it.ny) return [];
    const lv = it.levels[this.level];
    const n = it.levels.length;
    const out: Move[] = [];
    const at = this.toWorld(i + 0.5, j + 0.5);
    const level = (li: number, dir: 'up' | 'down', what?: string): Move => ({ kind: 'level', label: `${cap(what ?? `go ${dir}`)} to ${this.levelName(li)}`, level: li, from: it.id, at });
    const item = lv.furniture.find((f) => i >= f.x && i < f.x + f.w && j >= f.y && j < f.y + f.h);
    const link = lv.links?.find((k) => k.x === i && k.y === j);
    // (A renamed site goes by its name.)
    const named = link && InteriorLayer.names[link.to];
    if (link) out.push({ kind: 'site', label: cap(item?.name ?? 'follow the way through') + (named ? ` · ${named}` : ''), to: link.to, from: it.id, at });
    if (item && !link) {
      if (item.kind === 'exit') out.push({ kind: 'surface', label: `Up to the surface (${item.name.toLowerCase()})`, at });
      // Ways up and down within the site: the level above or below has its pair here.
      else if (item.kind === 'up' && this.level + 1 < n) out.push(level(this.level + 1, 'up', item.name));
      else if (item.kind === 'down' && this.level > 0) out.push(level(this.level - 1, 'down', item.name));
    }
    // A building's stair block: up and down to the floors it reaches.
    const [sx, sy, sw, sh] = it.stairs;
    if (!this.underground && lv.has_stairs && i >= sx && i < sx + sw && j >= sy && j < sy + sh) {
      if (this.level + 1 < n && it.levels[this.level + 1].has_stairs) out.push(level(this.level + 1, 'up', 'stairs up'));
      if (this.level > 0 && it.levels[this.level - 1].has_stairs) out.push(level(this.level - 1, 'down', 'stairs down'));
    }
    // A building's front door (or a back door): out into the street.
    if (!this.underground && this.level === it.entry_level) {
      const door = lv.doors.find((d) => {
        if (d.kind !== 'front' && d.kind !== 'back') return false;
        const [mx, my] = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
        return Math.abs(gx - mx) < 0.9 && Math.abs(gy - my) < 0.9;
      });
      if (door) out.push({ kind: 'surface', label: `Leave by the ${door.kind} door`, at: this.toWorld(...this.outside(door)) });
    }
    return out;
  }

  /** The middle of the square just outside a door in the outer wall (grid units). */
  outside(d: InteriorLevel['doors'][number]): [number, number] {
    const [mx, my] = [(d.a[0] + d.b[0]) / 2, (d.a[1] + d.b[1]) / 2];
    // Across the door's edge, one side is a room and the other is out.
    const [nx, ny] = d.a[0] === d.b[0] ? [1, 0] : [0, 1];
    const lv = this.interior.levels[this.level];
    const inRoom = (x: number, y: number) => {
      const [i, j] = [Math.floor(x), Math.floor(y)];
      return i >= 0 && j >= 0 && i < this.interior.nx && j < this.interior.ny && lv.cells[j * this.interior.nx + i] >= 0;
    };
    const s = inRoom(mx + nx * 0.5, my + ny * 0.5) ? -1 : 1;
    return [mx + nx * 0.5 * s, my + ny * 0.5 * s];
  }

  /** Where this site's way back to `from` is: its level and grid square. */
  linkTo(from: string): { level: number; x: number; y: number } | null {
    for (let li = 0; li < this.interior.levels.length; li++) {
      const k = this.interior.levels[li].links?.find((l) => l.to === from);
      if (k) return { level: li, x: k.x, y: k.y };
    }
    return null;
  }

  /** A section of a city's sewers (`w:<layout>:<sx>:<sy>`). */
  get sewer(): boolean {
    return this.interior.id.startsWith('w:');
  }

  get currentLevel(): number {
    return this.level;
  }

  setLevel(i: number) {
    const n = this.interior.levels.length;
    this.level = Math.max(0, Math.min(n - 1, i));
    this.draw();
  }

  /** Work out the other levels' textures in the background, so changing level doesn't stall. */
  prefetch(prepare: (it: Interior, level: number, clamp: boolean) => Promise<UnderField>) {
    if (!this.underground) return;
    for (let l = 0; l < this.interior.levels.length; l++) {
      if (this.fields.has(l)) continue;
      void prepare(this.interior, l, this.tile).then((f) => {
        if (!this.dead && !this.fields.has(l)) this.fields.set(l, f);
      });
    }
  }

  /** Draw the level again (doors opened or shut, play mode on or off). */
  redraw() {
    this.draw();
  }

  /** Position the plan for the camera (grid units → screen). */
  update(cam: Camera) {
    const it = this.interior;
    const [sx, sy] = cam.worldToScreen(it.origin[0], it.origin[1]);
    const s = SQUARE_FT * cam.ppf;
    this.plan.position.set(sx, sy);
    this.plan.rotation = Math.atan2(it.axis[1], it.axis[0]);
    this.plan.scale.set(s);
    if (this.under) this.under.uniforms.uniforms.uTime = performance.now() / 1000;
    this.dim.width = cam.width;
    this.dim.height = cam.height;
    for (const l of this.labelSpots) {
      const w = this.toWorld(l.x, l.y);
      const [lx, ly] = cam.worldToScreen(w[0], w[1]);
      l.text.position.set(lx, ly);
      // Only rooms big enough on screen get a label.
      l.text.visible = l.min * s > 70;
    }
    for (const m of this.markSpots) {
      const w = this.toWorld(m.x, m.y);
      m.node.position.set(...cam.worldToScreen(w[0], w[1]));
    }
  }

  /** The NPCs placed in this building or site (the DM's). */
  setNpcs(list: NpcMarker[]) {
    if (JSON.stringify(list) === JSON.stringify(this.npcs)) return;
    this.npcs = list;
    this.drawMarks();
  }

  /** Where an NPC stands on its level (grid): their point, else the biggest room's middle. */
  npcSpot(m: NpcMarker): [number, number] {
    if (m.at) return this.toGrid(m.at[0], m.at[1]);
    const lv = this.interior.levels[m.level];
    const room = lv?.rooms.reduce((a, r) => (r.squares > a.squares && r.kind !== 'floor' ? r : a), lv.rooms[0]);
    // Just below the middle, clear of the room's name.
    return room ? [room.center[0], room.center[1] + 1.2] : [this.interior.nx / 2, this.interior.ny / 2];
  }

  private drawMarks() {
    this.marks.removeChildren().forEach((c) => c.destroy({ children: true }));
    this.markSpots = [];
    if (!this.marks.visible) return;
    const style = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 12, fontWeight: 'bold', fill: '#fde68a', stroke: { color: '#2a1f14', width: 3, join: 'round' } });
    const here = this.npcs.filter((m) => m.level === this.level);
    // Several in one room spread out along a row.
    const taken = new Map<string, number>();
    for (const m of here) {
      const [gx, gy] = this.npcSpot(m);
      const k = `${Math.round(gx)},${Math.round(gy)}`;
      const n = taken.get(k) ?? 0;
      taken.set(k, n + 1);
      const node = new Container();
      const dot = new Graphics().circle(0, 0, 7).fill(0xb45309).stroke({ width: 2, color: 0xfde68a }).moveTo(-2.5, -1).lineTo(0, -4).lineTo(2.5, -1).stroke({ width: 1.5, color: 0xfde68a });
      const text = new Text({ text: m.name, style, anchor: { x: 0.5, y: 0 }, resolution: 2 });
      text.position.set(0, 8);
      node.addChild(dot, text);
      this.marks.addChild(node);
      this.markSpots.push({ node, x: gx + n * 1.2, y: gy });
    }
  }

  /** World ft of a grid point. */
  toWorld(x: number, y: number): [number, number] {
    const it = this.interior;
    return [it.origin[0] + (it.axis[0] * x + it.across[0] * y) * SQUARE_FT, it.origin[1] + (it.axis[1] * x + it.across[1] * y) * SQUARE_FT];
  }

  /** Grid coordinates of a world point. */
  toGrid(wx: number, wy: number): [number, number] {
    const it = this.interior;
    const dx = wx - it.origin[0];
    const dy = wy - it.origin[1];
    return [(dx * it.axis[0] + dy * it.axis[1]) / SQUARE_FT, (dx * it.across[0] + dy * it.across[1]) / SQUARE_FT];
  }

  /** Tactical info for a world point inside the building (null outside it). */
  inspect(wx: number, wy: number): SquareInfo | null {
    const it = this.interior;
    const lv = it.levels[this.level];
    const [gx, gy] = this.toGrid(wx, wy);
    const i = Math.floor(gx);
    const j = Math.floor(gy);
    if (i < 0 || j < 0 || i >= it.nx || j >= it.ny) return null;
    const room = lv.cells[j * it.nx + i];
    if (room < 0) return null;
    const onStairs = i >= it.stairs[0] && i < it.stairs[0] + it.stairs[2] && j >= it.stairs[1] && j < it.stairs[1] + it.stairs[3];
    const item = lv.furniture.find((f) => i >= f.x && i < f.x + f.w && j >= f.y && j < f.y + f.h);
    let object: SquareInfo['object'] = null;
    if (item) {
      const linked = lv.links?.some((k) => k.x === i && k.y === j);
      const notes = [item.blocks_move ? 'blocks movement' : 'passable', item.height_ft >= 5 ? `${item.height_ft} ft tall` : '', item.hazard ?? '', linked ? 'double-click to follow' : ''].filter(Boolean).join(' · ');
      object = { name: this.itemName(item), cover: COVER[item.cover], notes };
    } else if (onStairs) {
      object = { name: 'stairs', cover: COVER[0], notes: 'difficult terrain' };
    }
    const elev = lv.elevation_ft + lv.rooms[room].raise_ft;
    return { elevationFt: Math.round(elev - this.sea), tier: Math.floor((elev - this.sea) / 5), surface: `${this.roomName(this.level, room)} (${this.levelName(this.level)})`, object };
  }

  private dead = false;

  destroy() {
    this.dead = true;
    this.container.destroy({ children: true });
    this.freeUnder();
  }

  /** Let go of the underground level's shader, then its textures (the shared program and
   * noise stay). */
  private freeUnder() {
    if (!this.under) return;
    this.under.shader.destroy();
    this.under.sources.forEach((t) => t.destroy());
    this.under = null;
  }

  /** The level painted by the underground shader from its textures (`underField`). */
  private drawUnderground(lv: InteriorLevel) {
    const it = this.interior;
    const { nx, ny } = it;
    this.freeUnder();
    let f = this.fields.get(this.level);
    if (!f) this.fields.set(this.level, (f = underField(it, this.level, this.tile)));
    const { w, h, data, cells, liquidKind } = f;
    // Built sites keep square corners: walls measured from the squares, the field unsmoothed.
    const square = SQUARE_SITES.has(it.function);
    const fieldSrc = new BufferImageSource({ resource: data, width: w, height: h, format: 'rgba8unorm', alphaMode: 'no-premultiply-alpha', scaleMode: square ? 'nearest' : 'linear', addressMode: 'clamp-to-edge' });
    const cellSrc = new BufferImageSource({ resource: cells, width: nx, height: ny, format: 'rgba8unorm', alphaMode: 'no-premultiply-alpha', scaleMode: 'nearest' });
    // Light from the NW of the screen, in grid axes.
    const [lx, ly] = [-Math.SQRT1_2, -Math.SQRT1_2];
    const rough = it.function === 'cave' ? 1 : it.function === 'lava tube' ? 0.85 : it.function === 'mine' ? 0.45 : square ? 0 : 0.06;
    const uniforms = new UniformGroup({
      uN: { value: new Float32Array([nx, ny]), type: 'vec2<f32>' },
      uLight: { value: new Float32Array([lx * it.axis[0] + ly * it.axis[1], lx * it.across[0] + ly * it.across[1]]), type: 'vec2<f32>' },
      uTime: { value: 0, type: 'f32' },
      uRough: { value: rough, type: 'f32' },
      uSquare: { value: square ? 1 : 0, type: 'f32' },
      uLiquid: { value: liquidKind, type: 'f32' },
      uGhost: { value: this.ghost ? 1 : 0, type: 'f32' },
      uGridAlpha: { value: lv.natural ? 0.08 : 0.15, type: 'f32' },
      // Sewer sections: their place among the others (wrapped, to keep the GPU's sums small;
      // a seam once every 256 sections, far wider than any city).
      uOrigin: { value: new Float32Array(this.tile ? [wrap(it.origin[0] / 5), wrap(it.origin[1] / 5)] : [0, 0]), type: 'vec2<f32>' },
    });
    const shader = new Shader({ glProgram: underProgram, resources: { uField: fieldSrc, uCells: cellSrc, uNoise: noiseSource(), under: uniforms } });
    const mesh = new Mesh({ geometry: quad(), shader });
    mesh.scale.set(nx, ny);
    this.plan.addChild(mesh);
    this.under = { uniforms, sources: [fieldSrc, cellSrc], shader };
  }

  private draw() {
    const it = this.interior;
    const lv = it.levels[this.level];
    this.plan.removeChildren().forEach((c) => c.destroy());
    this.labels.removeChildren().forEach((c) => c.destroy());
    this.labelSpots = [];
    this.drawMarks();
    if (this.underground) this.drawUnderground(lv);
    if (this.ghost) return;
    let g = new Graphics();
    this.plan.addChild(g);
    const { nx, ny } = it;
    const colorOf = (r: number) => floorColor(lv.rooms[r].kind, lv.z, lv.natural, lv.rooms[r].raise_ft);
    if (!this.underground) {
    // Floors.
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const r = lv.cells[j * nx + i];
        if (r < 0) continue;
        g.rect(i, j, 1, 1).fill(colorOf(r));
      }
    }
    // Floor texture: planks along x on wood, flagstone joints on stone; then the 5-ft grid.
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const r = lv.cells[j * nx + i];
        if (r < 0) continue;
        const c = colorOf(r);
        if (STONE(c)) {
          g.moveTo(i, j + 0.5).lineTo(i + 1, j + 0.5).moveTo(i + ((j % 2) * 0.5 + 0.25), j).lineTo(i + ((j % 2) * 0.5 + 0.25), j + 0.5);
          g.moveTo(i + (((j + 1) % 2) * 0.5 + 0.25), j + 0.5).lineTo(i + (((j + 1) % 2) * 0.5 + 0.25), j + 1);
        } else {
          for (let k = 1; k < 4; k++) g.moveTo(i, j + k / 4).lineTo(i + 1, j + k / 4);
        }
      }
    }
    g.stroke({ width: 0.03, color: 0x000000, alpha: 0.25 });
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        if (lv.cells[j * nx + i] >= 0) g.rect(i, j, 1, 1);
      }
    }
    g.stroke({ width: 0.04, color: 0x000000, alpha: 0.35 });
    // Raised floors (arena stands): an inked edge where they drop to a lower room.
    for (let j = 0; j < ny; j++) {
      for (let i = 0; i < nx; i++) {
        const r = lv.cells[j * nx + i];
        if (r < 0 || lv.rooms[r].raise_ft <= 0) continue;
        for (const [di, dj] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const q = i + di >= 0 && i + di < nx && j + dj >= 0 && j + dj < ny ? lv.cells[(j + dj) * nx + i + di] : -1;
          if (q >= 0 && lv.rooms[q].raise_ft < lv.rooms[r].raise_ft) {
            const [x0, y0, x1, y1] = di === 1 ? [i + 1, j, i + 1, j + 1] : di === -1 ? [i, j, i, j + 1] : dj === 1 ? [i, j + 1, i + 1, j + 1] : [i, j, i + 1, j];
            g.moveTo(x0, y0).lineTo(x1, y1);
          }
        }
      }
    }
    g.stroke({ width: 0.18, color: INK });
    }
    // Stairs: treads across the block, an arrow up or down (tower tops are reached only by
    // the towers' spiral stairs).
    const [sx, sy, sw, sh] = it.stairs;
    if (lv.has_stairs) {
    g.rect(sx, sy, sw, sh).fill(0x7a6a58);
    const along = sw >= sh;
    const steps = Math.max(sw, sh) * 3;
    for (let k = 1; k < steps; k++) {
      const t = k / steps;
      if (along) g.moveTo(sx + t * sw, sy).lineTo(sx + t * sw, sy + sh);
      else g.moveTo(sx, sy + t * sh).lineTo(sx + sw, sy + t * sh);
    }
    g.stroke({ width: 0.05, color: INK, alpha: 0.7 });
    g.rect(sx, sy, sw, sh).stroke({ width: 0.08, color: INK });
    const up = this.level + 1 < it.levels.length && it.levels[this.level + 1].has_stairs;
    const down = this.level > 0 && it.levels[this.level - 1].has_stairs;
    const arrow = (dir: 1 | -1) => {
      const cx = sx + sw / 2;
      const cy = sy + sh / 2;
      const len = (along ? sw : sh) * 0.35;
      const [ax, ay] = along ? [dir, 0] : [0, dir];
      const off = up && down ? (dir === 1 ? 0.18 : -0.18) : 0;
      const [ox, oy] = along ? [0, off] : [off, 0];
      g.moveTo(cx - ax * len + ox, cy - ay * len + oy).lineTo(cx + ax * len + ox, cy + ay * len + oy);
      g.moveTo(cx + ax * len + ox, cy + ay * len + oy).lineTo(cx + ax * (len - 0.25) - ay * 0.18 + ox, cy + ay * (len - 0.25) + ax * 0.18 + oy);
      g.moveTo(cx + ax * len + ox, cy + ay * len + oy).lineTo(cx + ax * (len - 0.25) + ay * 0.18 + ox, cy + ay * (len - 0.25) - ax * 0.18 + oy);
    };
    if (up) arrow(1);
    if (down) arrow(-1);
    g.stroke({ width: 0.09, color: 0xf2e6c8 });
    }
    // Furniture, as sprites from the item atlas (or drawn here without a renderer).
    // (Underground, the shader paints sewage and lava channels.)
    const atlas = InteriorLayer.renderer ? ItemAtlas.for(InteriorLayer.renderer) : null;
    const items = new Container();
    for (const f of lv.furniture) {
      if (this.underground && (f.kind === 'sewage' || f.kind === 'lava')) continue;
      // Marks where a tunnel leaves the section: the next section is drawn on from there.
      if (this.tile && f.kind === 'continues') continue;
      if (this.style.player && dmOnlyHazard(f.hazard)) continue;
      if (f.sprite && atlas) {
        const s = this.pictureOf(f);
        if (s) items.addChild(s);
        else this.drawItem(g, f);
        continue;
      }
      if (!atlas) {
        this.drawItem(g, f);
        continue;
      }
      const frame = atlas.get(this.itemKey(f), (ig) => this.drawItem(ig, f), [f.x, f.y]);
      if (!frame) continue;
      const s = new Sprite(frame.texture);
      s.position.set(f.x + frame.x, f.y + frame.y);
      s.scale.set(1 / ITEM_PX);
      items.addChild(s);
    }
    if (atlas) {
      atlas.flush();
      this.plan.addChild(items);
      // Walls and doors go over the furniture.
      g = new Graphics();
      this.plan.addChild(g);
    } else {
      items.destroy();
    }
    // Walls: exterior thick, interior thinner; windows as light slots; doors as leaves. On an
    // open roof the outer wall is a parapet with merlons.
    if (this.underground) {
      // The shader draws walls against the rock; between rooms of a built level, masonry.
      for (const w of lv.walls) if (!w.exterior) g.moveTo(w.a[0], w.a[1]).lineTo(w.b[0], w.b[1]);
      g.stroke({ width: 0.3, color: 0x6f6a61, cap: 'square' });
      for (const w of lv.walls) if (!w.exterior) g.moveTo(w.a[0], w.a[1]).lineTo(w.b[0], w.b[1]);
      g.stroke({ width: 0.08, color: INK, alpha: 0.6, cap: 'square' });
      // Built sites: the outer walls inked straight, square at the corners.
      if (SQUARE_SITES.has(it.function)) {
        for (const w of lv.walls) if (w.exterior) g.moveTo(w.a[0], w.a[1]).lineTo(w.b[0], w.b[1]);
        g.stroke({ width: 0.08, color: INK, alpha: 0.85, cap: 'square', join: 'miter' });
      }
    } else {
      for (const w of lv.walls) g.moveTo(w.a[0], w.a[1]).lineTo(w.b[0], w.b[1]).stroke({ width: w.exterior || lv.roof ? 0.45 : 0.28, color: INK, cap: 'square' });
    }
    if (lv.roof) {
      for (const w of lv.walls) {
        if (!w.exterior) continue;
        const len = Math.abs(w.b[0] - w.a[0]) + Math.abs(w.b[1] - w.a[1]);
        const dx = Math.sign(w.b[0] - w.a[0]);
        const dy = Math.sign(w.b[1] - w.a[1]);
        for (let t = 0.25; t < len; t += 1) {
          const px = w.a[0] + dx * t;
          const py = w.a[1] + dy * t;
          g.rect(px - 0.22 + dx * 0.12, py - 0.22 + dy * 0.12, 0.44, 0.44);
        }
      }
      g.fill(0x6f6a61);
    }
    // Windows: a pale slot across the exterior wall (a filled rect: fine strokes on the
    // scaled plan render unreliably).
    if (lv.z >= 0) {
      for (const [x0, y0, x1, y1] of lv.windows) {
        const vert = x0 === x1;
        if (vert) g.rect(x0 - 0.09, Math.min(y0, y1) + 0.2, 0.18, 0.6);
        else g.rect(Math.min(x0, x1) + 0.2, y0 - 0.09, 0.6, 0.18);
      }
      g.fill(0xbcd6e0);
    }
    const doorState = this.style.doors;
    const loc = locKey(it.id, this.level);
    lv.doors.forEach((d, k) => {
      if (doorState) this.drawPlayDoor(g, d, doorState(loc, k));
    });
    for (const d of doorState ? [] : lv.doors) {
      if (d.kind === 'secret') {
        // For the DM: a dashed purple line where the wall hides a door.
        const [ax, ay] = d.a;
        const [bx, by] = d.b;
        for (let t = 0; t < 1; t += 0.34) g.moveTo(ax + (bx - ax) * t, ay + (by - ay) * t).lineTo(ax + (bx - ax) * (t + 0.18), ay + (by - ay) * (t + 0.18));
        g.stroke({ width: 0.14, color: 0x9333ea, alpha: 0.85 });
        continue;
      }
      const front = d.kind === 'front';
      const [ax, ay] = d.a;
      const [bx, by] = d.b;
      // A door leaf hinged at `a`, swung a quarter open into the square on one side.
      const vert = ax === bx;
      const lx = vert ? ax + 0.9 : ax + 0.1;
      const ly = vert ? ay + 0.1 : ay + 0.9;
      g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.12, color: front ? 0x5a3a1a : 0x7a5530 });
      g.moveTo(ax, ay).lineTo(lx, ly).stroke({ width: 0.1, color: front ? 0x5a3a1a : 0x7a5530 });
      g.arc(ax, ay, 0.9, vert ? 0 : Math.PI / 2 - 0.2, vert ? 0.2 : Math.PI / 2).stroke({ width: 0.03, color: INK, alpha: 0.5 });
    }
    // Room labels in screen space.
    const style = new TextStyle({ fontFamily: ['Palatino Linotype', 'Georgia', 'serif'], fontSize: 12, fontStyle: 'italic', fill: '#f5ecd6', stroke: { color: '#2a1f14', width: 3, join: 'round' } });
    lv.rooms.forEach((r, ri) => {
      // Ring-shaped rooms (arena stands) have their centre in another room: no label.
      const ci = Math.floor(r.center[0]);
      const cj = Math.floor(r.center[1]);
      if (r.squares < 4 || lv.cells[cj * nx + ci] !== ri || r.kind === 'floor') return;
      // A network of sewers: the tunnels need no name in every section.
      if (this.tile && (r.kind === 'sewer tunnel' || r.kind === 'service passage')) return;
      const text = new Text({ text: this.roomName(this.level, ri), style, anchor: 0.5, resolution: 2 });
      this.labels.addChild(text);
      this.labelSpots.push({ text, x: r.center[0], y: r.center[1], min: Math.sqrt(r.squares) });
    });
  }

  /** A door in play: shut (a slab across the gap) or open (the leaf swung back); a secret door
   * the players have not found is wall to them and a dashed line to the DM. */
  private drawPlayDoor(g: Graphics, d: InteriorLevel['doors'][number], st: DoorState) {
    const [ax, ay] = d.a;
    const [bx, by] = d.b;
    const secret = d.kind === 'secret';
    if (secret && !st.found && !st.open) {
      if (this.style.player) {
        if (this.underground) {
          g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.3, color: 0x6f6a61, cap: 'square' });
          g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.08, color: INK, alpha: 0.6, cap: 'square' });
        } else {
          g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.28, color: INK, cap: 'square' });
        }
        return;
      }
      for (let t = 0; t < 1; t += 0.34) g.moveTo(ax + (bx - ax) * t, ay + (by - ay) * t).lineTo(ax + (bx - ax) * (t + 0.18), ay + (by - ay) * (t + 0.18));
      g.stroke({ width: 0.14, color: 0x9333ea, alpha: 0.85 });
      return;
    }
    const wood = secret ? 0x6b4f7a : d.kind === 'front' ? 0x5a3a1a : 0x7a5530;
    if (!st.open) {
      g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.26, color: INK, cap: 'butt' });
      g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.16, color: wood, cap: 'butt' });
      return;
    }
    // Open: the leaf square to the wall from its hinge, and its swing.
    const closed = Math.atan2(by - ay, bx - ax);
    const open = closed - Math.PI / 2;
    const len = Math.hypot(bx - ax, by - ay) * 0.95;
    g.moveTo(ax, ay).lineTo(ax + Math.cos(open) * len, ay + Math.sin(open) * len).stroke({ width: 0.12, color: wood });
    g.arc(ax, ay, len, open, closed).stroke({ width: 0.03, color: INK, alpha: 0.5 });
  }

  /** What an item is called (an uploaded picture by its own name). */
  itemName(f: InteriorItem): string {
    return (f.sprite && this.interior.sprites?.[f.sprite - 1]?.name) || f.name;
  }

  /** An uploaded picture standing on the floor, fitted into its squares (its shape kept); null
   * while it loads (the level is drawn again once it has). */
  private pictureOf(f: InteriorItem): Sprite | null {
    const asset = this.interior.sprites?.[(f.sprite ?? 0) - 1]?.asset;
    if (!asset) return null;
    const tex = customTexture(asset);
    if (!tex) {
      const off = onLoaded((id) => {
        if (id !== asset) return;
        off();
        if (!this.dead) this.draw();
      });
      return null;
    }
    const s = new Sprite(tex);
    const k = Math.min(f.w / Math.max(1, tex.width), f.h / Math.max(1, tex.height));
    s.scale.set(k);
    s.position.set(f.x + (f.w - tex.width * k) / 2, f.y + (f.h - tex.height * k) / 2);
    return s;
  }

  /** The kind a building's furniture or an underground prop is drawn as with the furniture's
   * drawing (in a building: everything but the indoor props, drawn as underground). */
  private sharedKind(kind: string): string | undefined {
    if (this.underground) return SHARED[kind];
    return VARIED.has(kind) || !UNDER_DRAWN.has(kind) ? kind : undefined;
  }

  /** The wall a prop hangs on: underground the rock beside it; in a building a wall or the
   * outside beside it. */
  private propWall(f: InteriorItem): [number, number] {
    return this.underground ? this.wallSide(f) : this.backSide(f);
  }

  /** Everything an item's look depends on: what it is and its size; underground also the light, the
   * wall it is set against and its scatter variant. */
  private itemKey(f: InteriorItem): string {
    const base = `${f.kind}:${f.w}x${f.h}`;
    const light = this.light.map((v) => Math.round(v * 100)).join(',');
    const shared = this.sharedKind(f.kind);
    if (shared) {
      const g = { ...f, kind: shared };
      const back = ORIENTED.has(shared) ? this.backSide(g).join(',') : '';
      return `${this.underground ? 'u' : 'b'}:${base}:${light}:${VARIED.has(shared) ? itemVariant(f.x, f.y) : 0}:${back}`;
    }
    const wall = ON_WALL.has(f.kind) ? this.propWall(f).join(',') : BACKED.has(f.kind) ? this.backWall(f).join(',') : '';
    const variant = SCATTERED.has(f.kind) || UNDER_DRAWN.has(f.kind) ? itemVariant(f.x, f.y) : 0;
    return `u:${base}:${light}:${wall}:${variant}`;
  }

  /** The rock side of a wall-mounted prop's square (unit step), if any. */
  private wallSide(f: InteriorItem): [number, number] {
    const lv = this.interior.levels[this.level];
    const { nx, ny } = this.interior;
    for (const [dx, dy] of [[0, -1], [-1, 0], [1, 0], [0, 1]]) {
      const [i, j] = [f.x + dx, f.y + dy];
      if (i < 0 || j < 0 || i >= nx || j >= ny || lv.cells[j * nx + i] < 0) return [dx, dy];
    }
    return [0, 0];
  }

  /** The back of a building's item (unit step in grid axes): a bed's head and a booth seat's back are
   * away from the room (the wall at a bed's end; the side away from its booth table; a corner seat,
   * diagonal to its table, backs onto both walls: a diagonal step); a pew's back is
   * away from the level's nearest altar; everything else backs onto the wall along a long side. */
  private backSide(f: InteriorItem): [number, number] {
    const lv = this.interior.levels[this.level];
    const [cx, cy] = [f.x + f.w / 2, f.y + f.h / 2];
    if (f.kind === 'booth_seat') {
      const t = lv.furniture.find((o) => o.kind === 'booth_table' && Math.abs(o.x + o.w / 2 - cx) + Math.abs(o.y + o.h / 2 - cy) <= (o.w + f.w) / 2 + 0.01);
      if (t) {
        const [dx, dy] = [t.x + t.w / 2 - cx, t.y + t.h / 2 - cy];
        return Math.abs(dx) > Math.abs(dy) ? [-Math.sign(dx), 0] : [0, -Math.sign(dy)];
      }
      const c = lv.furniture.find((o) => o.kind === 'booth_table' && Math.abs(o.x + o.w / 2 - cx) <= (o.w + f.w) / 2 + 0.01 && Math.abs(o.y + o.h / 2 - cy) <= (o.h + f.h) / 2 + 0.01);
      if (c) return [-Math.sign(c.x + c.w / 2 - cx), -Math.sign(c.y + c.h / 2 - cy)];
    }
    if (f.kind === 'pew') {
      // Every pew in a room faces the same way: measured from the room's middle to the nearest altar,
      // across the pews (when the altar lies along them, the same side for all).
      const room = lv.rooms[lv.cells[f.y * this.interior.nx + f.x]];
      const [rx, ry] = room ? room.center : [cx, cy];
      let best: InteriorItem | null = null;
      for (const o of lv.furniture) if (o.kind === 'altar' && (!best || Math.hypot(o.x - rx, o.y - ry) < Math.hypot(best.x - rx, best.y - ry))) best = o;
      const [dx, dy] = best ? [best.x + best.w / 2 - rx, best.y + best.h / 2 - ry] : [0, 0];
      return f.w >= f.h ? [0, dy > 0.5 ? -1 : 1] : [dx > 0.5 ? -1 : 1, 0];
    }
    return this.backWall(f, f.kind === 'bed');
  }

  /** The side of a building's item that stands against a wall (unit step in grid axes): one along a
   * wall segment or the building's edge, else the first one. A long side; with `ends`, a short one. */
  private backWall(f: InteriorItem, ends = false): [number, number] {
    const lv = this.interior.levels[this.level];
    const { nx, ny } = this.interior;
    const wide = ends ? f.h > f.w : f.w > f.h;
    const tall = ends ? f.w > f.h : f.h > f.w;
    const sides: [number, number][] = wide ? [[0, -1], [0, 1]] : tall ? [[-1, 0], [1, 0]] : [[0, -1], [-1, 0], [1, 0], [0, 1]];
    for (const [dx, dy] of sides) {
      // The side's line, and the squares beyond it.
      const vert = dx !== 0;
      const line = dx < 0 ? f.x : dx > 0 ? f.x + f.w : dy < 0 ? f.y : f.y + f.h;
      const [lo, hi] = vert ? [f.y, f.y + f.h] : [f.x, f.x + f.w];
      const walled = lv.walls.some((w) => {
        if (vert ? w.a[0] !== line || w.b[0] !== line : w.a[1] !== line || w.b[1] !== line) return false;
        const [p, q] = vert ? [w.a[1], w.b[1]] : [w.a[0], w.b[0]];
        return Math.min(Math.max(p, q), hi) - Math.max(Math.min(p, q), lo) > 0.5;
      });
      let outside = false;
      for (let t = lo; t < hi && !outside; t++) {
        const [i, j] = vert ? [dx < 0 ? f.x - 1 : f.x + f.w, t] : [t, dy < 0 ? f.y - 1 : f.y + f.h];
        outside = i < 0 || j < 0 || i >= nx || j >= ny || lv.cells[j * nx + i] < 0;
      }
      if (walled || outside) return [dx, dy];
    }
    return sides[0];
  }

  private drawItem(g: Graphics, f: InteriorItem) {
    const { x, y, w, h, kind } = f;
    const c = itemColor(kind);
    const inset = 0.08;
    // Building furniture, and underground props that share its drawing.
    const shared = this.sharedKind(kind);
    if (shared) {
      const s = { ...f, kind: shared };
      if (drawFurniture(g, s, this.light, itemVariant(x, y), ORIENTED.has(shared) ? this.backSide(s) : null)) return;
    }
    // (Indoor props in a building too.)
    if ((this.underground || !shared) && drawUnderProp(g, f, this.light, itemVariant(x, y), ON_WALL.has(kind) ? this.propWall(f) : BACKED.has(kind) ? this.backWall(f) : [0, 0])) return;
    if (kind === 'sprite') {
      // A picture not loaded yet (or no renderer): a plain square in its place.
      g.rect(x + 0.1, y + 0.1, w - 0.2, h - 0.2).fill({ color: 0x786046, alpha: 0.5 }).stroke({ width: 0.05, color: INK });
      return;
    }
    if (this.drawUnderItem(g, f)) return;
    if (kind === 'link_down') {
      g.rect(x + 0.06, y + 0.06, 0.88, 0.88).fill(0x1e1a16).stroke({ width: 0.06, color: INK });
      for (let k = 0; k < 4; k++) g.rect(x + 0.1 + k * 0.2, y + 0.1, 0.16, 0.8).fill(0x6f675b - k * 0x0c0c0c);
      g.moveTo(x + 0.5, y + 0.25).lineTo(x + 0.5, y + 0.75).moveTo(x + 0.35, y + 0.6).lineTo(x + 0.5, y + 0.75).lineTo(x + 0.65, y + 0.6).stroke({ width: 0.08, color: 0xf2e6c8 });
      return;
    }
    // Seating: a round table, stools around it, and corner booths.
    if (kind === 'table' && w === 1 && h === 1) {
      g.circle(x + 0.5, y + 0.5, 0.36).fill(0x8a6038).stroke({ width: 0.06, color: INK });
      g.circle(x + 0.5, y + 0.5, 0.24).stroke({ width: 0.03, color: INK, alpha: 0.4 });
      return;
    }
    if (kind === 'spiral_stair') {
      // A winding stair: a round well with treads radiating from the newel post.
      const [cx, cy] = [x + 0.5, y + 0.5];
      g.circle(cx, cy, 0.45).fill(0x7a6a58).stroke({ width: 0.06, color: INK });
      for (let k = 0; k < 8; k++) {
        const a = (k / 8) * Math.PI * 2;
        g.moveTo(cx, cy).lineTo(cx + Math.cos(a) * 0.45, cy + Math.sin(a) * 0.45);
      }
      g.stroke({ width: 0.03, color: INK, alpha: 0.7 });
      g.circle(cx, cy, 0.08).fill(INK);
      return;
    }
    if (kind === 'bucket') {
      g.circle(x + 0.5, y + 0.5, 0.18).fill(0x5e4a36).stroke({ width: 0.04, color: INK });
      return;
    }
    if (kind === 'rack') {
      g.rect(x + 0.08, y + 0.08, w - 0.16, h - 0.16).fill(0x6a4428).stroke({ width: 0.06, color: INK });
      g.moveTo(x + 0.25, y + h / 2).lineTo(x + w - 0.25, y + h / 2).stroke({ width: 0.05, color: 0x3f4448 });
      return;
    }
    if (kind === 'chair') {
      g.circle(x + 0.5, y + 0.5, 0.2).fill(0x6e4a2c).stroke({ width: 0.05, color: INK });
      return;
    }
    if (kind === 'booth_seat') {
      g.roundRect(x + 0.1, y + 0.1, 0.8, 0.8, 0.12).fill(0x7a3b2e).stroke({ width: 0.05, color: INK });
      g.roundRect(x + 0.22, y + 0.22, 0.56, 0.56, 0.08).fill(0x8f4a3a);
      return;
    }
    if (kind === 'booth_table') {
      g.rect(x + 0.08, y + 0.08, 0.84, 0.84).fill(0x6a4428).stroke({ width: 0.06, color: INK });
      g.rect(x + 0.16, y + 0.16, 0.68, 0.68).fill(0x8a6038);
      return;
    }
    const [x0, y0, x1, y1] = [x + inset, y + inset, x + w - inset, y + h - inset];
    const round = ['barrel', 'statue', 'pillar', 'cauldron', 'anvil', 'vat'].includes(kind);
    if (round) {
      const r = Math.min(w, h) / 2 - inset;
      g.circle(x + w / 2, y + h / 2, r).fill(c).stroke({ width: 0.06, color: INK });
      if (kind === 'barrel') g.circle(x + w / 2, y + h / 2, r * 0.6).stroke({ width: 0.04, color: INK, alpha: 0.6 });
      if (kind === 'vat') g.circle(x + w / 2, y + h / 2, r * 0.8).fill(0x9a6a2a).stroke({ width: 0.05, color: INK, alpha: 0.6 });
      return;
    }
    if (kind === 'rug') {
      g.rect(x0, y0, x1 - x0, y1 - y0).fill({ color: c, alpha: 0.85 }).stroke({ width: 0.06, color: 0xd9b36a });
      g.rect(x0 + 0.2, y0 + 0.2, x1 - x0 - 0.4, y1 - y0 - 0.4).stroke({ width: 0.03, color: 0xd9b36a, alpha: 0.8 });
      return;
    }
    g.rect(x0, y0, x1 - x0, y1 - y0).fill(c).stroke({ width: 0.06, color: INK });
    const long = w >= h;
    switch (kind) {
      case 'bed':
      case 'cot': {
        // Pillow at one end, blanket over the rest.
        const [px, py, pw, ph] = long ? [x0 + 0.05, y0 + 0.05, 0.35, y1 - y0 - 0.1] : [x0 + 0.05, y0 + 0.05, x1 - x0 - 0.1, 0.35];
        g.rect(px, py, pw, ph).fill(0xe8e0cc);
        const [bx, by] = long ? [x0 + 0.5, y0] : [x0, y0 + 0.5];
        g.rect(bx, by, x1 - bx, y1 - by).fill(kind === 'bed' ? 0x6b7f9a : 0x8a8270);
        break;
      }
      case 'crate':
        g.moveTo(x0, y0).lineTo(x1, y1).moveTo(x1, y0).lineTo(x0, y1).stroke({ width: 0.05, color: INK, alpha: 0.6 });
        break;
      case 'shelf':
      case 'bookcase': {
        const n = Math.max(w, h) * 3;
        for (let k = 1; k < n; k++) {
          const t = k / n;
          if (long) g.moveTo(x0 + t * (x1 - x0), y0).lineTo(x0 + t * (x1 - x0), y1);
          else g.moveTo(x0, y0 + t * (y1 - y0)).lineTo(x1, y0 + t * (y1 - y0));
        }
        g.stroke({ width: 0.05, color: kind === 'bookcase' ? 0xb04a3a : 0x8a6a45 });
        break;
      }
      case 'hearth':
      case 'forge':
      case 'oven':
        g.rect((x0 + x1) / 2 - 0.25, (y0 + y1) / 2 - 0.2, 0.5, 0.4).fill(0xe0782a);
        break;
      case 'trapdoor':
        g.moveTo(x0, (y0 + y1) / 2).lineTo(x1, (y0 + y1) / 2).stroke({ width: 0.05, color: INK });
        break;
      case 'cage':
        for (let k = 1; k < w * 3; k++) g.moveTo(x0 + (k / (w * 3)) * (x1 - x0), y0).lineTo(x0 + (k / (w * 3)) * (x1 - x0), y1);
        g.stroke({ width: 0.04, color: 0x9a968e });
        break;
      case 'table':
      case 'long_table':
      case 'desk':
      case 'display':
        g.rect(x0 + 0.1, y0 + 0.1, x1 - x0 - 0.2, y1 - y0 - 0.2).fill(0x8a6038);
        break;
      case 'altar':
        g.rect(x0 + 0.15, y0 + 0.15, x1 - x0 - 0.3, y1 - y0 - 0.3).fill(0xd8cfbd);
        break;
      case 'bath':
        g.rect(x0 + 0.15, y0 + 0.15, x1 - x0 - 0.3, y1 - y0 - 0.3).fill(0x4f8aa8);
        break;
      default:
        break;
    }
  }

  /** Underground props, painted: drop shadows away from the NW light, domed and bevelled
   * shading, soft halos round lights; props on a wall sit against it. False if `f` is drawn
   * like building furniture. */
  private drawUnderItem(g: Graphics, f: InteriorItem): boolean {
    if (!this.underground) return false;
    const { x, y, w, h, kind } = f;
    const [cx, cy] = [x + w / 2, y + h / 2];
    const [lx, ly] = this.light;
    const [sx, sy] = [-lx * 0.14, -ly * 0.14];
    const wall = () => this.wallSide(f);
    const tone = (c: number, k: number) => {
      const ch = (s: number) => Math.min(255, Math.max(0, Math.round(((c >> s) & 255) * k)));
      return (ch(16) << 16) | (ch(8) << 8) | ch(0);
    };
    const shadow = (px: number, py: number, r: number, lift = 1) => g.ellipse(px + sx * lift, py + sy * lift, r * 1.08, r * 0.96).fill({ color: 0x000000, alpha: 0.32 });
    const ball = (px: number, py: number, r: number, c: number, lift = 1) => {
      if (lift > 0) shadow(px, py, r, lift);
      g.circle(px, py, r).fill(tone(c, 0.72));
      g.circle(px + lx * r * 0.16, py + ly * r * 0.16, r * 0.84).fill(c);
      g.circle(px + lx * r * 0.4, py + ly * r * 0.4, r * 0.36).fill({ color: tone(c, 1.35), alpha: 0.75 });
      g.circle(px, py, r).stroke({ width: 0.03, color: INK, alpha: 0.65 });
    };
    const block = (bx: number, by: number, bw: number, bh: number, c: number, lift = 1) => {
      if (lift > 0) g.rect(bx + sx * lift * 1.4, by + sy * lift * 1.4, bw, bh).fill({ color: 0x000000, alpha: 0.32 });
      g.rect(bx, by, bw, bh).fill(c);
      const e = Math.min(bw, bh) * 0.16;
      // Faces toward the light lit, away from it shaded.
      g.rect(lx < 0 ? bx : bx + bw - e, by, e, bh).fill({ color: tone(c, 1.3), alpha: 0.6 });
      g.rect(bx, ly < 0 ? by : by + bh - e, bw, e).fill({ color: tone(c, 1.3), alpha: 0.6 });
      g.rect(lx < 0 ? bx + bw - e : bx, by, e, bh).fill({ color: tone(c, 0.65), alpha: 0.6 });
      g.rect(bx, ly < 0 ? by + bh - e : by, bw, e).fill({ color: tone(c, 0.65), alpha: 0.6 });
      g.rect(bx, by, bw, bh).stroke({ width: 0.03, color: INK, alpha: 0.65 });
    };
    const glow = (px: number, py: number, r: number, c: number, a = 0.18) => {
      for (const k of [1, 0.66, 0.38]) g.circle(px, py, r * k).fill({ color: c, alpha: a });
    };
    const flame = (px: number, py: number, s: number) => {
      g.ellipse(px, py, s * 0.55, s).fill(0xf59e0b);
      g.ellipse(px, py + s * 0.25, s * 0.3, s * 0.55).fill(0xfde68a);
    };
    // Scatter: the same for every placement of one variant (so they can share a look).
    const variant = itemVariant(x, y);
    const hash = (k: number) => {
      const v = Math.sin((variant * 127.1 + 311.7 + k * 74.7) * 0.0174533) * 43758.5453;
      return v - Math.floor(v);
    };
    const steps = (light: number, dark: number, down: boolean) => {
      block(x + 0.06, y + 0.06, 0.88, 0.88, dark, 0);
      for (let k = 0; k < 4; k++) {
        const c = down ? tone(light, 1 - k * 0.18) : tone(dark, 1 + k * 0.18);
        g.rect(x + 0.1 + k * 0.2, y + 0.1, 0.16, 0.8).fill(c);
      }
      const d = down ? 1 : -1;
      g.moveTo(cx, cy - 0.26 * d).lineTo(cx, cy + 0.26 * d).moveTo(cx - 0.15, cy + 0.11 * d).lineTo(cx, cy + 0.26 * d).lineTo(cx + 0.15, cy + 0.11 * d);
      g.stroke({ width: 0.08, color: 0xf2e6c8 });
    };
    switch (kind) {
      case 'exit':
        glow(cx, cy, 1.2, 0xfff3d0, 0.12);
        steps(0xd8cfbd, 0x8a8174, false);
        return true;
      case 'up':
        steps(0xb0a796, 0x6f675b, false);
        return true;
      case 'down':
        steps(0x7a7064, 0x1e1a16, true);
        return true;
      case 'sewage':
      case 'lava':
        return true;
      case 'tunnel': {
        // A low dark opening in the wall.
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.3, cy + dy * 0.3];
        g.ellipse(ox, oy, Math.abs(dy) * 0.25 + 0.2, Math.abs(dx) * 0.25 + 0.2).fill(0x0b0907).stroke({ width: 0.04, color: 0x5d584f });
        return true;
      }
      case 'trapdoor':
        block(x + 0.12, y + 0.12, 0.76, 0.76, 0x6e4a2c, 0);
        g.moveTo(x + 0.12, cy).lineTo(x + 0.88, cy).stroke({ width: 0.04, color: INK, alpha: 0.6 });
        g.circle(cx, cy + 0.2, 0.07).stroke({ width: 0.03, color: 0x9ca3af });
        return true;
      case 'pool':
      case 'glass_pool': {
        const deep = kind === 'pool' ? 0x24495c : 0x15121a;
        g.ellipse(cx, cy, w * 0.46, h * 0.44).fill(tone(deep, 0.7));
        g.ellipse(cx - lx * 0.08, cy - ly * 0.08, w * 0.4, h * 0.38).fill(deep);
        g.ellipse(cx + lx * w * 0.12, cy + ly * h * 0.12, w * 0.16, h * 0.08).fill({ color: 0xffffff, alpha: kind === 'pool' ? 0.25 : 0.45 });
        g.ellipse(cx, cy, w * 0.46, h * 0.44).stroke({ width: 0.04, color: INK, alpha: 0.5 });
        return true;
      }
      case 'pit':
        for (const [k, c] of [[0.48, 0x2a2420], [0.38, 0x15110e], [0.26, 0x060504]] as const) g.ellipse(cx - lx * (0.48 - k) * 0.3, cy - ly * (0.48 - k) * 0.3, w * k, h * k).fill(c);
        g.ellipse(cx, cy, w * 0.48, h * 0.48).stroke({ width: 0.05, color: INK, alpha: 0.7 });
        return true;
      case 'trap':
        g.rect(x + 0.22, y + 0.22, 0.56, 0.56).stroke({ width: 0.05, color: 0xb91c1c, alpha: 0.65 });
        g.circle(cx, cy, 0.06).fill({ color: 0xb91c1c, alpha: 0.65 });
        return true;
      case 'cave_in':
        for (let k = 0; k < 4; k++) g.moveTo(x + 0.2 + k * 0.2, y + 0.2).lineTo(x + 0.1 + k * 0.2, y + 0.8);
        g.stroke({ width: 0.05, color: 0xb91c1c, alpha: 0.55 });
        return true;
      case 'stalagmite':
        ball(cx, cy, 0.34, 0x8a8072, 1.4);
        g.circle(cx + lx * 0.08, cy + ly * 0.08, 0.1).fill(0xc9bfae);
        return true;
      case 'rock_column':
        ball(cx, cy, 0.44, 0x7d7366, 2.2);
        for (const k of [0.32, 0.2]) g.circle(cx + lx * 0.04, cy + ly * 0.04, k).stroke({ width: 0.03, color: INK, alpha: 0.35 });
        return true;
      case 'boulder': {
        shadow(cx, cy, Math.min(w, h) * 0.45, 2);
        const pts: number[] = [];
        for (let k = 0; k < 9; k++) {
          const a = (k / 9) * Math.PI * 2;
          const r = Math.min(w, h) * (0.38 + 0.08 * hash(k));
          pts.push(cx + Math.cos(a) * r, cy + Math.sin(a) * r);
        }
        g.poly(pts).fill(0x6f675b).stroke({ width: 0.04, color: INK, alpha: 0.7 });
        g.ellipse(cx + lx * 0.25, cy + ly * 0.25, w * 0.22, h * 0.16).fill({ color: 0xa39886, alpha: 0.7 });
        g.moveTo(cx - 0.3, cy + 0.1).lineTo(cx + 0.05, cy - 0.05).lineTo(cx + 0.2, cy + 0.3).stroke({ width: 0.03, color: INK, alpha: 0.45 });
        return true;
      }
      case 'crystal':
        glow(cx, cy, 0.9, 0x7dd3fc, 0.12);
        for (const [a, l] of [[-0.5, 0.42], [0.3, 0.36], [1.2, 0.3]] as const) {
          const [ux, uy] = [Math.sin(a), -Math.cos(a)];
          const [tx, ty] = [cx + ux * l, cy + uy * l];
          g.poly([cx - uy * 0.09, cy + ux * 0.09, tx, ty, cx + uy * 0.09, cy - ux * 0.09]).fill(0x7dd3fc).stroke({ width: 0.025, color: 0x0c4a6e });
          g.poly([cx, cy, tx, ty, cx + uy * 0.09, cy - ux * 0.09]).fill({ color: 0xe0f2fe, alpha: 0.6 });
        }
        return true;
      case 'fungus':
        glow(cx, cy, 0.7, 0x5eead4, 0.1);
        for (const [dx, dy, r] of [[-0.2, -0.1, 0.09], [0.15, -0.2, 0.07], [0.05, 0.2, 0.1], [-0.15, 0.25, 0.06]]) {
          g.circle(cx + dx, cy + dy, r).fill(0x14b8a6);
          g.circle(cx + dx + lx * r * 0.3, cy + dy + ly * r * 0.3, r * 0.5).fill(0x99f6e4);
        }
        return true;
      case 'mushroom':
        for (const [dx, dy, r, c] of [[-0.12, 0.08, 0.3, 0x7c4a6e], [0.2, -0.15, 0.22, 0x8a5a3c], [0.18, 0.25, 0.15, 0x7c4a6e]] as const) {
          ball(cx + dx, cy + dy, r, c, 1.6);
          for (let k = 0; k < 3; k++) g.circle(cx + dx + Math.cos(k * 2.1) * r * 0.5, cy + dy + Math.sin(k * 2.1) * r * 0.5, r * 0.12).fill({ color: 0xf5ecd6, alpha: 0.8 });
        }
        return true;
      case 'web':
      case 'cobweb': {
        const [dx, dy] = kind === 'cobweb' ? wall() : [0, 0];
        const [ox, oy] = [cx + dx * 0.45, cy + dy * 0.45];
        for (let k = 0; k < 8; k++) {
          const a = (k / 8) * Math.PI * 2;
          g.moveTo(ox, oy).lineTo(ox + Math.cos(a) * 0.6, oy + Math.sin(a) * 0.6);
        }
        for (const r of [0.18, 0.34, 0.5]) g.circle(ox, oy, r);
        g.stroke({ width: 0.018, color: 0xf5f5f4, alpha: kind === 'web' ? 0.6 : 0.4 });
        return true;
      }
      case 'guano':
        for (let k = 0; k < 7; k++) g.ellipse(x + w * (0.15 + 0.7 * hash(k)), y + h * (0.15 + 0.7 * hash(k + 9)), 0.18 + 0.12 * hash(k + 3), 0.12 + 0.08 * hash(k + 5)).fill({ color: 0x3a3226, alpha: 0.7 });
        return true;
      case 'moss':
        for (let k = 0; k < 6; k++) g.circle(x + 0.15 + 0.7 * hash(k), y + 0.15 + 0.7 * hash(k + 7), 0.12 + 0.1 * hash(k + 2)).fill({ color: 0x4d7c3a, alpha: 0.5 });
        return true;
      case 'bones':
      case 'skeleton': {
        if (kind === 'skeleton') {
          const [ax, ay, bx, by] = h > w ? [cx, y + 0.3, cx, y + h - 0.3] : [x + 0.3, cy, x + w - 0.3, cy];
          g.moveTo(ax, ay).lineTo(bx, by).stroke({ width: 0.06, color: 0xe8e0cc });
          for (let k = 1; k < 5; k++) {
            const t = 0.2 + k * 0.1;
            const [px, py] = [ax + (bx - ax) * t, ay + (by - ay) * t];
            const [nx2, ny2] = h > w ? [0.2, 0] : [0, 0.2];
            g.moveTo(px - nx2, py - ny2).lineTo(px + nx2, py + ny2);
          }
          g.stroke({ width: 0.04, color: 0xe8e0cc });
          ball(ax, ay, 0.14, 0xe8e0cc, 0.6);
          return true;
        }
        g.moveTo(cx - 0.3, cy - 0.15).lineTo(cx + 0.25, cy + 0.1).moveTo(cx - 0.1, cy + 0.25).lineTo(cx + 0.2, cy - 0.2);
        g.stroke({ width: 0.075, color: 0xe8e0cc, cap: 'round' });
        g.circle(cx + 0.25, cy + 0.1, 0.06).circle(cx - 0.3, cy - 0.15, 0.06).fill(0xe8e0cc);
        return true;
      }
      case 'skulls':
        shadow(cx, cy, 0.38, 1);
        for (const [dx, dy] of [[-0.15, 0.12], [0.15, 0.12], [0, -0.1], [-0.08, 0.32], [0.18, -0.25]]) {
          ball(cx + dx, cy + dy, 0.13, 0xe8e0cc, 0);
          g.circle(cx + dx - 0.04, cy + dy, 0.025).circle(cx + dx + 0.04, cy + dy, 0.025).fill(INK);
        }
        return true;
      case 'campfire':
        for (let k = 0; k < 7; k++) ball(cx + Math.cos(k * 0.9) * 0.3, cy + Math.sin(k * 0.9) * 0.3, 0.07, 0x6f675b, 0.4);
        g.circle(cx, cy, 0.2).fill(0x2a2420);
        g.moveTo(cx - 0.15, cy - 0.1).lineTo(cx + 0.15, cy + 0.1).moveTo(cx - 0.15, cy + 0.1).lineTo(cx + 0.15, cy - 0.1).stroke({ width: 0.05, color: 0x3a2a1c });
        return true;
      case 'rubble':
      case 'debris':
        for (const [dx, dy, r] of [[-0.18, -0.1, 0.15], [0.16, 0.05, 0.13], [-0.02, 0.22, 0.11], [0.2, -0.22, 0.08]]) ball(cx + dx, cy + dy, r, kind === 'rubble' ? 0x7a7064 : hash(dx * 9) > 0.5 ? 0x5e5236 : 0x4a5a32, 0.8);
        return true;
      case 'obsidian':
        for (const pts of [[cx - 0.3, cy + 0.2, cx - 0.1, cy - 0.3, cx, cy + 0.1], [cx + 0.05, cy + 0.25, cx + 0.3, cy - 0.15, cx + 0.15, cy + 0.3]]) {
          g.poly(pts.map((v, k) => v + (k % 2 ? sy : sx))).fill({ color: 0x000000, alpha: 0.3 });
          g.poly(pts).fill(0x111018).stroke({ width: 0.02, color: 0x6b6880 });
        }
        g.moveTo(cx - 0.12, cy - 0.15).lineTo(cx - 0.06, cy + 0.02).stroke({ width: 0.025, color: 0xc4c2d8 });
        return true;
      case 'vent':
        glow(cx, cy, 0.9, 0xf97316, 0.12);
        ball(cx, cy, 0.3, 0x3a3530, 0.6);
        g.circle(cx, cy, 0.14).fill(0xf97316);
        g.circle(cx, cy, 0.07).fill(0xfde68a);
        return true;
      case 'basalt': {
        shadow(cx, cy, 0.4, 1.8);
        const pts: number[] = [];
        for (let k = 0; k < 6; k++) pts.push(cx + Math.cos((k * Math.PI) / 3) * 0.4, cy + Math.sin((k * Math.PI) / 3) * 0.4);
        g.poly(pts).fill(0x2f2b2a).stroke({ width: 0.04, color: INK });
        g.poly(pts.map((v, k) => (k % 2 ? cy + (v - cy) * 0.6 + ly * 0.04 : cx + (v - cx) * 0.6 + lx * 0.04))).fill(0x46413e);
        return true;
      }
      case 'sulfur':
        for (let k = 0; k < 5; k++) g.circle(x + 0.2 + 0.6 * hash(k), y + 0.2 + 0.6 * hash(k + 4), 0.1 + 0.1 * hash(k + 8)).fill({ color: 0xd9c33a, alpha: 0.75 });
        return true;
      case 'scorched':
        g.ellipse(cx, cy, w * 0.42, h * 0.42).fill({ color: 0x120c08, alpha: 0.7 });
        g.moveTo(cx - 0.2, cy - 0.3).lineTo(cx + 0.15, cy + 0.3).stroke({ width: 0.05, color: 0x8a8070 });
        return true;
      case 'timber':
        block(x + 0.3, y + 0.3, 0.4, 0.4, 0x7a5530, 2);
        g.moveTo(x + 0.35, y + 0.42).lineTo(x + 0.65, y + 0.42).moveTo(x + 0.35, y + 0.58).lineTo(x + 0.65, y + 0.58).stroke({ width: 0.02, color: INK, alpha: 0.4 });
        return true;
      case 'rail':
        g.moveTo(x + 0.5, y + 0.15).lineTo(x + 0.5, y + 0.85).stroke({ width: 0.12, color: 0x5e4028 });
        g.moveTo(x, y + 0.3).lineTo(x + 1, y + 0.3).moveTo(x, y + 0.7).lineTo(x + 1, y + 0.7).stroke({ width: 0.06, color: 0x6b6f75 });
        return true;
      case 'ore_cart':
        block(x + 0.12, y + 0.2, 0.76, 0.6, 0x4a4440, 1.2);
        for (const [dx, dy] of [[-0.15, -0.05], [0.1, 0.05], [0.0, -0.12], [0.18, -0.1]]) ball(cx + dx, cy + dy, 0.09, 0x8a7a5a, 0);
        return true;
      case 'ore_vein': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.38, cy + dy * 0.38];
        g.moveTo(ox - dy * 0.4, oy - dx * 0.4).lineTo(ox + dy * 0.4, oy + dx * 0.4).stroke({ width: 0.12, color: 0x8a6a2a, alpha: 0.85 });
        for (let k = 0; k < 4; k++) g.circle(ox + (hash(k) - 0.5) * 0.7 * Math.abs(dy || 1), oy + (hash(k + 3) - 0.5) * 0.7 * Math.abs(dx || 1), 0.035).fill(0xfde68a);
        return true;
      }
      case 'tools':
        g.moveTo(cx - 0.3, cy + 0.25).lineTo(cx + 0.25, cy - 0.25).stroke({ width: 0.06, color: 0x7a5530, cap: 'round' });
        g.moveTo(cx + 0.08, cy - 0.38).quadraticCurveTo(cx + 0.3, cy - 0.3, cx + 0.38, cy - 0.08).stroke({ width: 0.07, color: 0x6b6f75, cap: 'round' });
        g.moveTo(cx - 0.25, cy - 0.2).lineTo(cx + 0.05, cy + 0.3).stroke({ width: 0.05, color: 0x7a5530, cap: 'round' });
        return true;
      case 'lantern':
        glow(cx, cy, 1.4, 0xfcd34d, 0.1);
        ball(cx, cy, 0.14, 0x3f4448, 1);
        g.circle(cx, cy, 0.08).fill(0xfde68a);
        return true;
      case 'winch':
        block(x + 0.1, y + 0.2, 0.8, 0.6, 0x6e4a2c, 1.4);
        g.ellipse(cx, cy, 0.18, 0.28).fill(0x4a4440).stroke({ width: 0.03, color: INK });
        return true;
      case 'powder':
        for (const [dx, dy] of [[-0.17, -0.1], [0.17, -0.1], [0, 0.18]]) {
          ball(cx + dx, cy + dy, 0.17, 0x5e4028, 1);
          g.moveTo(cx + dx - 0.07, cy + dy - 0.07).lineTo(cx + dx + 0.07, cy + dy + 0.07).moveTo(cx + dx + 0.07, cy + dy - 0.07).lineTo(cx + dx - 0.07, cy + dy + 0.07).stroke({ width: 0.035, color: 0xb91c1c });
        }
        return true;
      case 'bedroll':
      case 'cot': {
        const along = h >= w;
        shadow(cx, cy, Math.max(w, h) * 0.32, 0.6);
        g.roundRect(x + 0.15, y + 0.15, w - 0.3, h - 0.3, 0.15).fill(kind === 'cot' ? 0x7d5a3c : 0x6b5d45).stroke({ width: 0.03, color: INK, alpha: 0.6 });
        g.roundRect(along ? x + 0.2 : x + 0.2, along ? y + 0.2 : y + 0.2, along ? w - 0.4 : 0.35, along ? 0.35 : h - 0.4, 0.1).fill(0xd8cfbd);
        return true;
      }
      case 'urn':
        ball(cx, cy, 0.24, 0x9a6a3e, 1.2);
        g.circle(cx, cy, 0.09).fill(0x2a2018);
        return true;
      case 'brazier':
        glow(cx, cy, 1.5, 0xf59e0b, 0.09);
        ball(cx, cy, 0.3, 0x3f4448, 1.2);
        g.circle(cx, cy, 0.18).fill(0x7c2d12);
        flame(cx, cy - 0.02, 0.14);
        return true;
      case 'candles':
        glow(cx, cy, 0.9, 0xfcd34d, 0.1);
        ball(cx, cy, 0.12, 0x6b6f75, 1.6);
        for (const [dx, dy] of [[-0.2, 0], [0.2, 0], [0, -0.18]]) {
          g.circle(cx + dx, cy + dy, 0.06).fill(0xf5ecd6);
          flame(cx + dx, cy + dy - 0.03, 0.04);
        }
        return true;
      case 'chest':
        block(x + 0.16, y + 0.24, 0.68, 0.52, 0x7a5530, 1);
        g.rect(x + 0.16, cy - 0.04, 0.68, 0.08).fill(0x3f4448);
        g.rect(cx - 0.05, cy - 0.06, 0.1, 0.12).fill(0xd9b36a);
        return true;
      case 'hoard':
        glow(cx, cy, 0.9, 0xfacc15, 0.12);
        for (const [dx, dy] of [[-0.18, 0.05], [0.12, -0.12], [0.1, 0.16], [-0.05, 0.25], [-0.2, -0.2], [0.25, 0.05], [0, 0]]) ball(cx + dx, cy + dy, 0.12, 0xeab308, 0.4);
        return true;
      case 'dais':
        block(x + 0.05, y + 0.05, w - 0.1, h - 0.1, 0xa8a296, 0.8);
        block(cx - 0.3, cy - 0.3, 0.6, 0.6, 0x7f1d1d, 1.6);
        g.rect(cx - 0.18, cy - 0.18, 0.36, 0.36).fill(0x991b1b);
        return true;
      case 'well':
      case 'fountain': {
        const r = Math.min(w, h) * 0.44;
        shadow(cx, cy, r, 1.2);
        g.circle(cx, cy, r).fill(0x8a857a).stroke({ width: 0.04, color: INK, alpha: 0.7 });
        g.circle(cx, cy, r * 0.7).fill(kind === 'well' ? 0x10202a : 0x6f6a61);
        if (kind === 'fountain') ball(cx, cy, r * 0.25, 0xa8a296, 1);
        return true;
      }
      case 'sarcophagus':
      case 'effigy':
        block(x + 0.08, y + 0.08, w - 0.16, h - 0.16, 0xb8b2a6, 1.4);
        // The figure carved on the lid.
        ball(h > w ? cx : x + 0.3, h > w ? y + 0.3 : cy, 0.11, 0xcfc8ba, 0);
        g.roundRect(h > w ? cx - 0.14 : x + 0.45, h > w ? y + 0.45 : cy - 0.14, h > w ? 0.28 : w - 0.65, h > w ? h - 0.65 : 0.28, 0.08).fill({ color: 0xcfc8ba, alpha: 0.85 });
        return true;
      case 'coffin': {
        const long = h > w;
        const W = long ? w : h;
        const poly = long ? [cx, y + 0.08, x + W - 0.1, y + 0.35, cx + 0.22, y + h - 0.08, cx - 0.22, y + h - 0.08, x + 0.1, y + 0.35] : [x + 0.08, cy, x + 0.35, y + 0.1, x + w - 0.08, cy - 0.22, x + w - 0.08, cy + 0.22, x + 0.35, y + W - 0.1];
        g.poly(poly.map((v, k) => v + (k % 2 ? sy * 1.3 : sx * 1.3))).fill({ color: 0x000000, alpha: 0.32 });
        g.poly(poly).fill(0x6e4a2c).stroke({ width: 0.035, color: INK, alpha: 0.7 });
        g.poly(poly.map((v, k) => (k % 2 ? cy + (v - cy) * 0.75 : cx + (v - cx) * 0.75))).fill(0x1e1712);
        return true;
      }
      case 'niche': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.32, cy + dy * 0.32];
        g.rect(ox - 0.28, oy - 0.28, 0.56, 0.56).fill(0x221d18).stroke({ width: 0.03, color: INK });
        ball(ox, oy, 0.1, 0xe8e0cc, 0);
        return true;
      }
      case 'chains': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.4, cy + dy * 0.4];
        for (const s of [-0.22, 0.22]) {
          const [ax, ay] = [ox + dy * s, oy + dx * s];
          for (let k = 0; k < 3; k++) g.ellipse(ax - dx * k * 0.12, ay - dy * k * 0.12, 0.05, 0.05).stroke({ width: 0.025, color: 0x6b6f75 });
          g.circle(ax - dx * 0.36, ay - dy * 0.36, 0.07).stroke({ width: 0.03, color: 0x6b6f75 });
        }
        return true;
      }
      case 'sconce': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.36, cy + dy * 0.36];
        glow(ox, oy, 1.6, 0xf59e0b, 0.09);
        g.rect(ox - 0.06, oy - 0.06, 0.12, 0.12).fill(0x3f4448);
        flame(ox - dx * 0.08, oy - dy * 0.08, 0.1);
        return true;
      }
      case 'banner': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.4, cy + dy * 0.4];
        const [px, py] = [dy, dx];
        const pts = [ox - px * 0.3, oy - py * 0.3, ox + px * 0.3, oy + py * 0.3, ox + px * 0.3 - dx * 0.45, oy + py * 0.3 - dy * 0.45, ox - dx * 0.32, oy - dy * 0.32, ox - px * 0.3 - dx * 0.45, oy - py * 0.3 - dy * 0.45];
        g.poly(pts).fill(0x7f1d1d).stroke({ width: 0.025, color: INK, alpha: 0.7 });
        g.circle(ox - dx * 0.2, oy - dy * 0.2, 0.07).fill(0xd9b36a);
        return true;
      }
      case 'pipe': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.38, cy + dy * 0.38];
        g.circle(ox, oy, 0.2).fill(0x4a4e54).stroke({ width: 0.03, color: INK });
        g.circle(ox, oy, 0.12).fill(0x14110e);
        g.moveTo(ox - dx * 0.12, oy - dy * 0.12).lineTo(ox - dx * 0.45, oy - dy * 0.45).stroke({ width: 0.06, color: 0x4f5a3a, alpha: 0.8 });
        return true;
      }
      case 'ladder': {
        const [dx, dy] = wall();
        const [ox, oy] = [cx + dx * 0.25, cy + dy * 0.25];
        const [px, py] = [dy, dx];
        g.moveTo(ox - px * 0.2 - dx * 0.25, oy - py * 0.2 - dy * 0.25).lineTo(ox - px * 0.2 + dx * 0.25, oy - py * 0.2 + dy * 0.25);
        g.moveTo(ox + px * 0.2 - dx * 0.25, oy + py * 0.2 - dy * 0.25).lineTo(ox + px * 0.2 + dx * 0.25, oy + py * 0.2 + dy * 0.25);
        g.stroke({ width: 0.06, color: 0x6e4a2c });
        for (const t of [-0.15, 0, 0.15]) g.moveTo(ox - px * 0.2 + dx * t, oy - py * 0.2 + dy * t).lineTo(ox + px * 0.2 + dx * t, oy + py * 0.2 + dy * t);
        g.stroke({ width: 0.04, color: 0x8a6a45 });
        return true;
      }
      case 'nest':
        // A beast's nest: a ring of sticks and fur round a hollow.
        g.ellipse(cx, cy, 0.46, 0.42).fill({ color: 0x5e4a30, alpha: 0.9 });
        for (let k = 0; k < 10; k++) {
          const a = (k / 10) * Math.PI * 2 + hash(k);
          g.moveTo(cx + Math.cos(a) * 0.2, cy + Math.sin(a) * 0.2).lineTo(cx + Math.cos(a + 0.6) * 0.46, cy + Math.sin(a + 0.6) * 0.42);
        }
        g.stroke({ width: 0.03, color: 0x9a7a4e });
        g.ellipse(cx - lx * 0.04, cy - ly * 0.04, 0.2, 0.17).fill(0x2a2018);
        return true;
      case 'ice':
        glow(cx, cy, 0.8, 0xe0f2fe, 0.1);
        for (const [a, l] of [[-0.4, 0.44], [0.5, 0.34], [1.6, 0.3], [2.7, 0.26]] as const) {
          const [ux, uy] = [Math.sin(a), -Math.cos(a)];
          const [tx, ty] = [cx + ux * l, cy + uy * l];
          g.poly([cx - uy * 0.11, cy + ux * 0.11, tx, ty, cx + uy * 0.11, cy - ux * 0.11]).fill(0xcfe8f5).stroke({ width: 0.025, color: 0x5b7f99 });
          g.poly([cx, cy, tx, ty, cx + uy * 0.11, cy - ux * 0.11]).fill({ color: 0xffffff, alpha: 0.6 });
        }
        return true;
      case 'ice_sheet':
        g.ellipse(cx, cy, w * 0.47, h * 0.44).fill({ color: 0xd8ecf6, alpha: 0.8 }).stroke({ width: 0.03, color: 0x7fa3b5, alpha: 0.7 });
        g.moveTo(cx - w * 0.25, cy - h * 0.1).lineTo(cx + w * 0.05, cy + h * 0.05).lineTo(cx + w * 0.2, cy - h * 0.18).stroke({ width: 0.02, color: 0x7fa3b5, alpha: 0.8 });
        g.ellipse(cx + lx * w * 0.15, cy + ly * h * 0.15, w * 0.14, h * 0.06).fill({ color: 0xffffff, alpha: 0.6 });
        return true;
      case 'rat_nest':
        g.ellipse(cx, cy, 0.38, 0.3).fill({ color: 0x6b5a3a, alpha: 0.85 });
        for (let k = 0; k < 6; k++) g.moveTo(cx - 0.3 + 0.6 * hash(k), cy - 0.2 + 0.4 * hash(k + 2)).lineTo(cx - 0.3 + 0.6 * hash(k + 5), cy - 0.2 + 0.4 * hash(k + 7));
        g.stroke({ width: 0.025, color: 0xa08a5a });
        g.circle(cx, cy, 0.1).fill(0x1e1712);
        return true;
      case 'offering':
        ball(cx, cy, 0.2, 0x8a857a, 1);
        g.circle(cx, cy, 0.12).fill(0x3a342d);
        g.circle(cx + 0.03, cy, 0.04).circle(cx - 0.04, cy + 0.03, 0.04).fill(0xeab308);
        return true;
      case 'glyph':
        glow(cx, cy, Math.min(w, h) * 0.6, 0xa78bfa, 0.08);
        g.circle(cx, cy, Math.min(w, h) * 0.42).stroke({ width: 0.05, color: 0xa78bfa, alpha: 0.7 });
        g.circle(cx, cy, Math.min(w, h) * 0.3).stroke({ width: 0.03, color: 0xa78bfa, alpha: 0.6 });
        for (let k = 0; k < 5; k++) {
          const a = (k / 5) * Math.PI * 2;
          const b = ((k + 2) / 5) * Math.PI * 2;
          const r = Math.min(w, h) * 0.3;
          g.moveTo(cx + Math.cos(a) * r, cy + Math.sin(a) * r).lineTo(cx + Math.cos(b) * r, cy + Math.sin(b) * r);
        }
        g.stroke({ width: 0.03, color: 0xa78bfa, alpha: 0.6 });
        return true;
      case 'statue':
        block(x + 0.1, y + 0.1, w - 0.2, h - 0.2, 0x8a857a, 1);
        ball(cx, cy, 0.24, 0xb8b2a6, 2.2);
        return true;
      case 'pillar':
        ball(cx, cy, Math.min(w, h) * 0.42, 0xa8a296, 2.6);
        return true;
      case 'iron_maiden':
        block(x + 0.15, y + 0.1, 0.7, 0.8, 0x3f4448, 1.8);
        g.ellipse(cx, cy - 0.12, 0.14, 0.17).fill(0x6b6f75);
        for (let k = 0; k < 3; k++) g.circle(cx - 0.18 + k * 0.18, cy + 0.22, 0.03).fill(0xb8b2a6);
        return true;
      case 'stocks':
        block(x + 0.08, y + 0.3, w - 0.16, 0.4, 0x7a5530, 1);
        for (const t of [0.3, 0.5, 0.7]) g.circle(x + w * t, cy, 0.07).fill(0x1e1712);
        return true;
      case 'sacks':
        for (const [dx, dy] of [[-0.15, -0.1], [0.17, -0.05], [0, 0.2]]) ball(cx + dx, cy + dy, 0.19, 0xb0905e, 0.9);
        return true;
      case 'bookshelf': {
        block(x + 0.1, y + 0.1, w - 0.2, h - 0.2, 0x5e4028, 2);
        const long = h > w;
        const n = 6;
        for (let k = 0; k < n; k++) {
          const t = (k + 0.5) / n;
          const c = [0x7f1d1d, 0x1e3a5f, 0x3f5f2a, 0x7a5530][k % 4];
          if (long) g.rect(x + 0.2, y + 0.15 + t * (h - 0.4), w - 0.4, (h - 0.4) / n - 0.03).fill(c);
          else g.rect(x + 0.15 + t * (w - 0.4), y + 0.2, (w - 0.4) / n - 0.03, h - 0.4).fill(c);
        }
        return true;
      }
      case 'rug':
        g.rect(x + 0.1, y + 0.1, w - 0.2, h - 0.2).fill({ color: 0x6b2a2a, alpha: 0.75 }).stroke({ width: 0.05, color: 0xb08a4a, alpha: 0.7 });
        g.rect(x + 0.3, y + 0.3, w - 0.6, h - 0.6).stroke({ width: 0.03, color: 0xb08a4a, alpha: 0.5 });
        return true;
      case 'table':
        block(x + 0.1, y + 0.1, w - 0.2, h - 0.2, 0x7a5530, 1.2);
        return true;
      case 'crate':
        block(x + 0.12, y + 0.12, 0.76, 0.76, 0x9a7a4e, 1.2);
        g.moveTo(x + 0.14, y + 0.14).lineTo(x + 0.86, y + 0.86).moveTo(x + 0.86, y + 0.14).lineTo(x + 0.14, y + 0.86).stroke({ width: 0.04, color: 0x5e4028, alpha: 0.7 });
        return true;
      case 'barrel':
        ball(cx, cy, 0.36, 0x7a5530, 1.2);
        g.circle(cx, cy, 0.24).stroke({ width: 0.035, color: 0x3f4448, alpha: 0.8 });
        return true;
      case 'weapon_rack':
        block(x + 0.15, y + 0.3, 0.7, 0.4, 0x5e4028, 1.6);
        for (const t of [0.3, 0.5, 0.7]) g.moveTo(x + t, y + 0.18).lineTo(x + t, y + 0.82);
        g.stroke({ width: 0.035, color: 0x9ca3af });
        return true;
      case 'cage':
        shadow(cx, cy, Math.min(w, h) * 0.45, 1.6);
        g.rect(x + 0.1, y + 0.1, w - 0.2, h - 0.2).fill({ color: 0x1e1a16, alpha: 0.35 }).stroke({ width: 0.06, color: 0x55524d });
        for (let k = 1; k < w * 3; k++) g.moveTo(x + 0.1 + (k / (w * 3)) * (w - 0.2), y + 0.1).lineTo(x + 0.1 + (k / (w * 3)) * (w - 0.2), y + h - 0.1);
        g.stroke({ width: 0.035, color: 0x8a867e });
        return true;
      case 'rack':
        block(x + 0.15, y + 0.1, w - 0.3, h - 0.2, 0x6e4a2c, 1);
        g.circle(cx, y + 0.25, 0.08).circle(cx, y + h - 0.25, 0.08).fill(0x3f4448);
        return true;
      case 'altar':
        block(x + 0.08, y + 0.12, w - 0.16, h - 0.24, 0xc4bcae, 1.6);
        g.rect(cx - 0.2, cy - 0.08, 0.4, 0.16).fill({ color: 0x7f1d1d, alpha: 0.7 });
        return true;
      default:
        return false;
    }
  }

  levelInfo(): { name: string; z: number }[] {
    return this.interior.levels.map((l: InteriorLevel, li) => ({ name: this.levelName(li), z: l.z }));
  }

  /** A level's name, as renamed (`l:<site>:<level>`). */
  levelName(li: number): string {
    return InteriorLayer.names[`l:${this.interior.id}:${li}`] ?? this.interior.levels[li].name;
  }

  /** A room's name, as renamed (`r:<site>:<level>:<room>`), else what it is. */
  roomName(li: number, ri: number): string {
    return InteriorLayer.names[`r:${this.interior.id}:${li}:${ri}`] ?? this.interior.levels[li].rooms[ri].kind;
  }
}
