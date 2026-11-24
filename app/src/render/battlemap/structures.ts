import { Graphics } from 'pixi.js';
import { ROOF_INK, SQ, type Shape } from '../../gen/battlePrep';
import { DIFFICULT, BLOCKS, OPAQUE } from '../../play/vision';
import { PX } from '../atlas';
import { SQUARE_FT } from '../scale';
import { drawBounds, drawSails, isBound, SAILS } from '../countryside';
import { drawCaveMouth, drawThemedEntrance } from '../entrances';
import { drawDrain, drawManhole } from './grates';
import { apron, boxR, creep, dress, dressOf, drawPlazaCross, drawPlazaDais, drawPlazaFountain, drawPlazaMound, drawPlazaPlanter, edgeLight, groundShadow, mix, ruinRamp, type PlazaGround } from './plaza';
import { INK, LINE, OUT } from '../pen';
import { rngAt } from '../bridges';
import { drawForecourtGate, drawGardenWalls, drawGate, drawGatePier, drawWalls, drawWallTower, GATE_DEPTH } from '../walls';

const STONE = 0x9c968b;
const STONE_TOP = 0xb3ada2;

/** A drawbridge's play door key: its gate's world square (the same from whichever chunk it is read). */
export function bridgeKey(s: Shape, cx: number, cy: number): string {
  return `surface#bridge:${Math.floor(cx * SQ + s.pts[0] / PX + 1e-3)}:${Math.floor(cy * SQ + s.pts[1] / PX + 1e-3)}`;
}

/**
 * The world squares a drawbridge covers, with the play flags each takes raised (triples i, j, flags):
 * every square the raised leaf's line across the gatehouse's outer arch touches (a little wider than
 * the deck, to meet the arch's sides however the squares fall) is shut (OPAQUE | BLOCKS); those beyond
 * it whose middle is on its deck (as `moat::Drawbridge::covers`) lie open to the moat, a wet one's water
 * (BLOCKS) or a dry ditch (DIFFICULT). The passage under the gatehouse stays as it is (flags 0: only
 * for `bridgeAt`).
 */
export function bridgeCells(s: Shape, cx: number, cy: number): number[] {
  const [ax, ay] = [cx * SQ + s.pts[0] / PX, cy * SQ + s.pts[1] / PX];
  const [bx, by] = [cx * SQ + s.pts[2] / PX, cy * SQ + s.pts[3] / PX];
  const wet = s.pts.length > 4 && Math.round(s.pts[4] / PX) === 1;
  const span = Math.hypot(bx - ax, by - ay) || 1;
  const [ux, uy] = [(bx - ax) / span, (by - ay) / span];
  const half = s.size / PX / 2 + 1e-6;
  const out: number[] = [];
  const gate = new Set<string>();
  const reach = half + 0.6;
  const leaf = GATE_DEPTH / SQUARE_FT + 0.15;
  for (let q = -reach; q <= reach + 1e-6; q += 0.05) {
    const [i, j] = [Math.floor(ax + ux * leaf - uy * q), Math.floor(ay + uy * leaf + ux * q)];
    if (gate.has(`${i},${j}`)) continue;
    gate.add(`${i},${j}`);
    out.push(i, j, OPAQUE | BLOCKS);
  }
  const r = Math.ceil(half + 1);
  for (let j = Math.floor(Math.min(ay, by)) - r; j <= Math.ceil(Math.max(ay, by)) + r; j++) {
    for (let i = Math.floor(Math.min(ax, bx)) - r; i <= Math.ceil(Math.max(ax, bx)) + r; i++) {
      const [dx, dy] = [i + 0.5 - ax, j + 0.5 - ay];
      const along = dx * ux + dy * uy;
      if (gate.has(`${i},${j}`) || Math.abs(ux * dy - uy * dx) > half || along < 0 || along > span) continue;
      // (Under the gatehouse: unchanged, but a click there works the bridge too.)
      out.push(i, j, along < leaf ? 0 : wet ? BLOCKS : DIFFICULT);
    }
  }
  return out;
}

/**
 * Walls, gatehouses, towers and ways underground as clean vectors over the ground (their squares
 * keep their tactical height; the ground under them is drawn flat): `walls.ts` draws the first
 * three, and a forecourt's garden wall, gate piers and gate. Bridges, piers and drawbridges are drawn apart
 * (`bridges.ts`; a drawbridge in pieces, for play). (`ox`, `oy`: the chunk's corner in world px, for the bounds' patterns.)
 */
const PAVING: PlazaGround = { tone: [1, 1, 1], foot: 14, nature: -1, wx: 0, wy: 0 };

/** (`groundOf`: the ground a way underground stands on, as `BattlemapLayer.plazaGround`; plain paving without it.) */
export function drawStructures(shapes: Shape[], ox = 0, oy = 0, gates = shapes.filter((s) => s.kind === 9), groundOf?: (s: Shape) => PlazaGround): Graphics {
  const g = new Graphics();
  // (v63 countryside: hedgerows, dry-stone walls and fences low under the rest; a windmill's sails.)
  const bounds = shapes.filter((s) => isBound(s.kind));
  if (bounds.length) drawBounds(g, bounds, ox, oy);
  for (const s of shapes) if (s.kind === SAILS) drawSails(g, s);
  const walls = shapes.filter((s) => s.kind === 0);
  if (walls.length) drawWalls(g, walls, gates);
  for (const s of shapes) if (s.kind === 9) drawGate(g, s);
  for (const s of shapes) if (s.kind === 1) drawWallTower(g, s);
  for (const s of shapes) if (s.kind === 4) drawEntrance(g, s, groundOf?.(s) ?? PAVING);
  // A forecourt's low garden wall, its gate's piers, then the open leaves.
  const garden = shapes.filter((s) => s.kind === 10);
  if (garden.length) drawGardenWalls(g, garden);
  for (const s of shapes) if (s.kind === 11) drawGatePier(g, s);
  for (const s of shapes) if (s.kind === 12) drawForecourtGate(g, s);
  return g;
}

/**
 * A round feature a tier above the paving (`size`: 0 dais, 1 fountain, 2 planter, 3 market cross, 4 a steppe town's
 * mound; `pts`: the centre, then a point on the rim where its steps come down), each drawn to the ground it stands on
 * (`ground`: the town's stone tone and the surfaces about; `plaza.ts`), plain grey on cobbles without it.
 */
export function drawDais(g: Graphics, s: Shape, ground: PlazaGround = { tone: [1, 1, 1], foot: 14, nature: -1, wx: 0, wy: 0 }) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const [dx, dy] = [s.pts[2] - cx, s.pts[3] - cy];
  const r = Math.hypot(dx, dy) || PX;
  const a = Math.atan2(dy, dx);
  const kind = Math.round(s.size / PX);
  if (kind === 1) return drawPlazaFountain(g, cx, cy, r, a, ground);
  if (kind === 2) return drawPlazaPlanter(g, cx, cy, r, a, ground);
  if (kind === 3) return drawPlazaCross(g, cx, cy, r, a, ground);
  if (kind === 4) return drawPlazaMound(g, cx, cy, r, a, ground);
  drawPlazaDais(g, cx, cy, r, a, ground);
}

/**
 * A way underground, by kind (`size`): a stairwell going down (dungeon, crypt, catacombs), a
 * cave mouth, a timber-framed mine adit with rails running out, a lava-tube skylight, a round
 * sewer grate in the street. `pts`: the opening, then a point one square in along the passage.
 */
function drawEntrance(g: Graphics, s: Shape, ground: PlazaGround) {
  const [x, y] = [s.pts[0], s.pts[1]];
  const l = Math.hypot(s.pts[2] - x, s.pts[3] - y) || 1;
  const [ux, uy] = [(s.pts[2] - x) / l, (s.pts[3] - y) / l];
  const [vx, vy] = [-uy, ux];
  // A point `a` squares in along the passage and `b` across it.
  const at = (a: number, b: number): [number, number] => [x + (ux * a + vx * b) * PX, y + (uy * a + vy * b) * PX];
  const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];
  const ellipse = (a: number, ra: number, rb: number, wobble: number) => {
    const pts: number[] = [];
    for (let k = 0; k < 18; k++) {
      const t = (k / 18) * Math.PI * 2;
      const r = 1 + wobble * Math.sin(t * 3 + s.pts[0] * 0.01) * Math.cos(t * 2);
      pts.push(...at(a + Math.cos(t) * ra * r, Math.sin(t) * rb * r));
    }
    return pts;
  };
  // `size` carries kind + 16 × style (scaled by PX like every shape size when the chunk was
  // decoded): style 1 a ruin's broken stair, 2 a graveyard mausoleum over the stair, 3 a drain
  // grate, 4-6 a wild site's way down by theme (barrow, rune gate, tower stump: `entrances.ts`).
  const code = Math.round(s.size / PX);
  // A stable random per entrance.
  let seed = (Math.round(x * 7.1) * 73856093) ^ (Math.round(y * 3.3) * 19349663);
  const rnd = () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
  if (code >> 4 === 1) return drawRuinStair(g, at, quad, rnd, ground);
  if (code >> 4 === 2) return drawMausoleum(g, at, quad, rnd, ground);
  if (code >> 4 === 3) return drawDrain(g, at, quad);
  if (code >> 4 >= 4 && code >> 4 <= 6) return drawThemedEntrance(g, code >> 4, at, rnd);
  switch (code & 15) {
    case 0:
    case 1:
    case 6: {
      // Stairwell: steps darkening as they go down, a stone kerb round three sides.
      // (Life size: a stair 4-5 ft wide and 10 ft long, its kerb half a foot thick.)
      g.poly(quad(-0.5, 1.5, -0.5, 0.5)).fill(0x2a2520);
      for (let k = 0; k < 4; k++) {
        const a = -0.5 + k * 0.5;
        const shade = Math.round(0x9a - k * 0x1e);
        g.poly(quad(a, a + 0.42, -0.45, 0.45)).fill((shade << 16) | (shade << 8) | (shade - 8));
      }
      g.poly([...at(-0.5, -0.5), ...at(1.5, -0.5), ...at(1.5, 0.5), ...at(-0.5, 0.5)], false).stroke({ width: 7, color: STONE, join: 'miter' });
      g.poly([...at(-0.5, -0.58), ...at(1.58, -0.58), ...at(1.58, 0.58), ...at(-0.5, 0.58)], false).stroke({ width: 2.5, color: ROOF_INK, join: 'miter' });
      break;
    }
    case 2: {
      // Cave mouth: rock heaped round a dark opening under an overhang (`entrances.ts`).
      drawCaveMouth(g, at, rnd);
      break;
    }
    case 3: {
      // Mine adit: rails out of a dark opening in a timber frame.
      // (Life size: a 6-ft opening, rails 2 ft apart.)
      for (const b of [-0.2, 0.2]) g.moveTo(...at(-3, b)).lineTo(...at(0.6, b));
      g.stroke({ width: 2.4, color: 0x4a4440 });
      for (let a = -2.8; a < 0.6; a += 0.5) g.moveTo(...at(a, -0.35)).lineTo(...at(a, 0.35));
      g.stroke({ width: 3, color: 0x6e4a2c });
      g.poly(quad(0, 1.4, -0.65, 0.65)).fill(0x15120f);
      g.poly([...at(0, -0.7), ...at(0, 0.7)], false).stroke({ width: 7, color: 0x7a5530, cap: 'round' });
      for (const b of [-0.68, 0.68]) g.moveTo(...at(0, b)).lineTo(...at(1.3, b));
      g.stroke({ width: 5.5, color: 0x6a4428, cap: 'round' });
      break;
    }
    case 5: {
      // Sewer manhole: an iron lid in a stone collar.
      drawManhole(g, at, 0.25);
      break;
    }
    default: {
      // Lava tube skylight: a collapsed roof onto the tube, a glow below.
      g.poly(ellipse(0.4, 1.7, 1.9, 0.18)).fill({ color: 0x3a3530, alpha: 0.95 });
      g.poly(ellipse(0.4, 1.25, 1.4, 0.15)).fill(0x120d0a).stroke({ width: 3, color: ROOF_INK });
      g.poly(ellipse(0.6, 0.6, 0.7, 0.1)).fill({ color: 0xd2691e, alpha: 0.35 });
      break;
    }
  }
}

type At = (a: number, b: number) => [number, number];
type Quad = (a0: number, a1: number, b0: number, b1: number) => number[];

/** Stone grey shaded by `k` (0 light .. 1 dark). */
const stoneShade = (k: number) => {
  const v = Math.round(0xa8 - k * 0x70);
  return (v << 16) | (v << 8) | Math.max(0, v - 10);
};

/** A stone's colour `k` of the way from lit (0) to the dark of the stair's depth (1). */
const deepen = (light: number, k: number) => mix(light, 0x1b1714, Math.min(0.9, k * 0.85));
const circle = (x: number, y: number, r: number, n = 14) => Array.from({ length: n * 2 }, (_, i) => (i & 1 ? y + Math.sin((i >> 1) / n * Math.PI * 2) * r : x + Math.cos((i >> 1) / n * Math.PI * 2) * r));

/**
 * A stair down through a ruin's floor: a flight one square wide (5 ft) and 12 ft long, its treads
 * darkening into a dark mouth, a kerb of fallen blocks with gaps either side; the rest of the
 * old floor round it broken slabs with the ground showing between, a few fallen stones. The
 * stone weathered from the chunk's own tone, a worn ring of the ground's earth (or a paved
 * margin) at its foot.
 */
function drawRuinStair(g: Graphics, at: At, quad: Quad, rnd: () => number, gr: PlazaGround) {
  const st = ruinRamp(gr);
  const [o, e] = [at(0, 0), at(1, 0)];
  const [cx, cy] = at(0.75, 0);
  const R = boxR(1.95 * PX, 1.7 * PX, Math.atan2(e[1] - o[1], e[0] - o[0]));
  const sr = rngAt(gr.wx + o[0], gr.wy + o[1]);
  apron(g, cx, cy, R, gr, sr);
  // The broken floor either side: big worn slabs, a few gone to the ground beneath.
  for (const side of [-1, 1]) {
    for (let a = -0.6; a < 2.2; a += 0.7) {
      for (const [b0, b1] of [[0.86, 1.5]]) {
        if (rnd() < 0.25) continue;
        const j = (rnd() - 0.5) * 0.08;
        const la = 0.66 - rnd() * 0.1;
        const [p0, p1] = side < 0 ? [-b1, -b0] : [b0, b1];
        const p = [...at(a + j, p0), ...at(a + la + j, p0 - j), ...at(a + la - j, p1), ...at(a - j, p1 + j)];
        g.poly(p).fill(rnd() < 0.5 ? st.mid : mix(st.mid, st.dark, 0.25)).stroke({ width: LINE * 1.1, color: INK, alpha: 0.8, join: 'round' });
      }
    }
  }
  // The mouth, then the treads (1 ft each seen from above), lit at the top, dark below.
  g.poly(quad(-0.5, 2, -0.5, 0.5)).fill(0x1b1714);
  const n = 10;
  for (let k = 0; k < n; k++) {
    const a = -0.5 + k * 0.25;
    g.poly(quad(a, a + 0.2, -0.47, 0.47)).fill(deepen(k === 0 ? st.sun : st.light, k / (n - 1.5)));
  }
  // Two bold cracks across the upper treads, and a corner broken off one.
  for (const [a0, b0, a1, b1] of [[-0.45, -0.3, 0.2, 0.05], [0.3, 0.35, 0.7, 0.1]]) g.moveTo(...at(a0, b0)).lineTo(...at((a0 + a1) / 2, b0 + 0.12)).lineTo(...at(a1, b1));
  g.stroke({ width: LINE * 1.2, color: INK, alpha: 0.75, join: 'round' });
  g.poly(quad(0.5, 0.7, 0.3, 0.47)).fill(0x1b1714);
  // The kerb: worn blocks along both sides and the back, some fallen out of line.
  const block = (a: number, b: number, la: number, lb: number) => {
    const j = (rnd() - 0.5) * 0.1;
    const p = [...at(a + j, b - j), ...at(a + la + j, b - j), ...at(a + la - j, b + lb + j), ...at(a - j, b + lb + j)];
    groundShadow(g, p, gr);
    g.poly(p).fill(rnd() < 0.5 ? st.mid : st.light).stroke({ width: LINE * 1.3, color: INK, join: 'round' });
    edgeLight(g, p, st, LINE * 0.8);
  };
  for (let a = -0.5; a < 2; a += 0.62) {
    for (const side of [-1, 1]) {
      if (rnd() < 0.2) continue;
      block(a, side < 0 ? -0.82 : 0.5, 0.56, 0.32);
    }
  }
  for (let b = -0.82; b < 0.8; b += 0.56) if (rnd() > 0.25) block(2, b, 0.32, 0.52);
  // A few fallen stones about the mouth.
  for (let k = 0; k < 6; k++) {
    const a = -1 + rnd() * 3.4;
    const b = (rnd() < 0.5 ? -1 : 1) * (1.6 + rnd() * 0.35);
    const r = (0.08 + rnd() * 0.1) * PX;
    const [x, y] = at(a, b);
    groundShadow(g, circle(x, y, r, 8), gr, 0.6);
    g.poly(circle(x, y, r, 8)).fill(rnd() < 0.5 ? st.mid : st.light).stroke({ width: LINE, color: INK, alpha: 0.85 });
  }
  dress(g, cx, cy, R, (t) => R(t) * 0.85, dressOf(gr), gr, sr);
}

/**
 * A graveyard mausoleum over the catacomb stair, 3 squares wide and 4 deep: a pale stone
 * plinth, heavy walls on three sides, columns either side of a 4-ft door; inside a flagged
 * chamber, a stone coffin along each side wall and a stair one square wide going down between them; a slab path out of the door, urns at its corners. The stone from the chunk's tone (weathered),
 * a paved margin about the plinth on paving, the ground creeping up to it on grass.
 */
function drawMausoleum(g: Graphics, at: At, quad: Quad, rnd: () => number, gr: PlazaGround) {
  const st = ruinRamp(gr);
  const [o, e] = [at(0, 0), at(1, 0)];
  const [cx, cy] = at(0.5, 0);
  const sr = rngAt(gr.wx + o[0], gr.wy + o[1]);
  creep(g, cx, cy, boxR(2.05 * PX, 1.6 * PX, Math.atan2(e[1] - o[1], e[0] - o[0])), gr, sr);
  // The plinth with its shadow on the ground, the threshold slabs out front.
  const plinth = quad(-1.5, 2.5, -1.5, 1.5);
  groundShadow(g, plinth, gr, 1.4);
  g.poly(plinth).fill(st.light).stroke({ width: OUT, color: INK, join: 'miter' });
  edgeLight(g, plinth, st, LINE);
  for (let k = 0; k < 2; k++) g.poly(quad(-2.4 + k * 0.45, -2.4 + k * 0.45 + 0.4, -0.55, 0.55)).fill(st.mid).stroke({ width: LINE * 1.3, color: INK, alpha: 0.85 });
  // Walls on the sides and back, throwing a little shadow on the plinth.
  const walls = [quad(-1.2, 2.5, -1.5, -1.05), quad(-1.2, 2.5, 1.05, 1.5), quad(2.05, 2.5, -1.05, 1.05)];
  for (const w of walls) g.poly(w.map((v) => v + PX * 0.09)).fill({ color: st.dark, alpha: 0.3 });
  for (const w of walls) {
    g.poly(w).fill(st.mid).stroke({ width: LINE * 1.6, color: INK, join: 'miter' });
    edgeLight(g, w, st, LINE * 0.9);
  }
  // Coping along the wall tops.
  for (const b of [-1.28, 1.28]) g.moveTo(...at(-1.15, b)).lineTo(...at(2.45, b));
  g.moveTo(...at(2.28, -1.25)).lineTo(...at(2.28, 1.25));
  g.stroke({ width: LINE * 0.8, color: st.sun, alpha: 0.8 });
  // The chamber floor: flagstones in the stone's mid tone, a few big joints.
  g.poly(quad(-1.2, 2.05, -1.05, 1.05)).fill(st.mid);
  g.moveTo(...at(-0.55, -1.05)).lineTo(...at(-0.55, 1.05)).moveTo(...at(0.1, -1.05)).lineTo(...at(0.1, -0.58)).moveTo(...at(0.1, 0.58)).lineTo(...at(0.1, 1.05));
  for (const b of [-0.3, 0.3]) g.moveTo(...at(-1.2, b)).lineTo(...at(-0.55, b));
  g.moveTo(...at(-0.55, 0)).lineTo(...at(0.05, 0));
  g.stroke({ width: LINE * 0.9, color: st.dark, alpha: 0.7 });
  // A stone coffin lid along each side wall, a carved panel on it.
  for (const side of [-1, 1]) {
    const [b0, b1] = side < 0 ? [-1.0, -0.62] : [0.62, 1.0];
    const lid = quad(0.2, 1.85, b0, b1);
    groundShadow(g, lid, gr, 0.7);
    g.poly(lid).fill(st.light).stroke({ width: LINE * 1.3, color: INK, join: 'miter' });
    edgeLight(g, lid, st, LINE * 0.8);
    g.poly(quad(0.38, 1.67, b0 + 0.09, b1 - 0.09)).stroke({ width: LINE * 0.9, color: st.dark, alpha: 0.75, join: 'miter' });
  }
  // The stair down, one square wide between low side walls, toward the back: treads a foot
  // deep, lit at the top and darkening into the dark mouth.
  g.poly(quad(0.05, 2.05, -0.5, 0.5)).fill(0x1b1714);
  for (let k = 0; k < 10; k++) {
    const a = 0.05 + k * 0.2;
    g.poly(quad(a, a + 0.16, -0.46, 0.46)).fill(deepen(k === 0 ? st.sun : st.light, k / 8.5));
  }
  for (const b of [-0.58, 0.5]) {
    const w = quad(0.05, 2.05, b, b + 0.08);
    g.poly(w).fill(st.light).stroke({ width: LINE * 1.1, color: INK, join: 'miter' });
  }
  g.poly(quad(0.05, 2.05, -0.58, 0.58)).stroke({ width: LINE * 1.2, color: INK, alpha: 0.8, join: 'miter' });
  // The doorway, about 4 ft wide between short returns of the front wall.
  for (const [b0, b1] of [[-1.05, -0.4], [0.4, 1.05]]) {
    const w = quad(-1.2, -0.92, b0, b1);
    g.poly(w).fill(st.mid).stroke({ width: LINE * 1.6, color: INK, join: 'miter' });
    edgeLight(g, w, st, LINE * 0.9);
  }
  // Columns either side of the door, a short shadow in their own round.
  for (const b of [-1.25, 1.25]) {
    const [x, y] = at(-1.25, b);
    const r = 0.3 * PX;
    groundShadow(g, circle(x, y, r), gr, 0.9);
    g.poly(circle(x, y, r)).fill(st.sun).stroke({ width: LINE * 1.6, color: INK });
    g.poly(circle(x - r * 0.12, y - r * 0.12, r * 0.5)).fill({ color: 0xffffff, alpha: 0.25 });
    g.poly(circle(x + r * 0.15, y + r * 0.15, r * 0.7)).stroke({ width: LINE * 0.8, color: st.dark, alpha: 0.5 });
  }
  // Urns at the front corners of the path.
  for (const b of [-0.85, 0.85]) {
    const [x, y] = at(-2.15, b);
    const r = 0.18 * PX;
    groundShadow(g, circle(x, y, r, 10), gr, 0.7);
    g.poly(circle(x, y, r, 10)).fill(st.dark).stroke({ width: LINE, color: INK });
    g.poly(circle(x, y, r * 0.45, 8)).fill(0x2a2520);
    g.poly(circle(x - r * 0.3, y - r * 0.3, r * 0.3, 6)).fill({ color: st.sun, alpha: 0.5 });
  }
  dress(g, cx, cy, boxR(1.9 * PX, 1.45 * PX, Math.atan2(e[1] - o[1], e[0] - o[0])), boxR(1.6 * PX, 1.2 * PX, Math.atan2(e[1] - o[1], e[0] - o[0])), dressOf(gr), gr, sr);
}
