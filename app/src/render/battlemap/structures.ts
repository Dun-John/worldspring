import { Graphics } from 'pixi.js';
import { ROOF_INK, SQ, type Shape } from '../../gen/battlePrep';
import { DIFFICULT, BLOCKS, OPAQUE } from '../../play/vision';
import { PX } from '../atlas';
import { SQUARE_FT } from '../scale';
import { drawBounds, drawSails, isBound, SAILS } from '../countryside';
import { drawCaveMouth, drawThemedEntrance } from '../entrances';
import { drawDrain, drawManhole } from './grates';
import { drawPlazaDais, drawPlazaFountain, type PlazaGround } from './plaza';
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
export function drawStructures(shapes: Shape[], ox = 0, oy = 0, gates = shapes.filter((s) => s.kind === 9)): Graphics {
  const g = new Graphics();
  // (v63 countryside: hedgerows, dry-stone walls and fences low under the rest; a windmill's sails.)
  const bounds = shapes.filter((s) => isBound(s.kind));
  if (bounds.length) drawBounds(g, bounds, ox, oy);
  for (const s of shapes) if (s.kind === SAILS) drawSails(g, s);
  const walls = shapes.filter((s) => s.kind === 0);
  if (walls.length) drawWalls(g, walls, gates);
  for (const s of shapes) if (s.kind === 9) drawGate(g, s);
  for (const s of shapes) if (s.kind === 1) drawWallTower(g, s);
  for (const s of shapes) if (s.kind === 4) drawEntrance(g, s);
  // A forecourt's low garden wall, its gate's piers, then the open leaves.
  const garden = shapes.filter((s) => s.kind === 10);
  if (garden.length) drawGardenWalls(g, garden);
  for (const s of shapes) if (s.kind === 11) drawGatePier(g, s);
  for (const s of shapes) if (s.kind === 12) drawForecourtGate(g, s);
  return g;
}

/**
 * A round feature a tier above the paving (`size`: 0 dais, 1 fountain, 2 planter, 3 market
 * cross; `pts`: the centre, then a point on the rim where its steps come down): a drop shadow,
 * a moulded stone round, and steps curving down one side. A dais and a fountain are drawn to the ground they
 * stand on (`ground`: the town's stone tone and the surfaces about; `plaza.ts`), plain grey on cobbles without it.
 */
export function drawDais(g: Graphics, s: Shape, ground: PlazaGround = { tone: [1, 1, 1], foot: 14, nature: -1, wx: 0, wy: 0 }) {
  const [cx, cy] = [s.pts[0], s.pts[1]];
  const [dx, dy] = [s.pts[2] - cx, s.pts[3] - cy];
  const r = Math.hypot(dx, dy) || PX;
  const a = Math.atan2(dy, dx);
  const kind = Math.round(s.size / PX);
  if (kind === 4) return drawMound(g, cx, cy, r, a);
  if (kind === 0) return drawPlazaDais(g, cx, cy, r, a, ground);
  if (kind === 1) return drawPlazaFountain(g, cx, cy, r, a, ground);
  // Steps: three curved treads out from the rim.
  for (let k = 2; k >= 0; k--) {
    const ro = r + PX * 0.28 * (k + 1);
    const span = Math.min(0.9, (PX * 1.1) / r);
    g.moveTo(cx + Math.cos(a - span) * r, cy + Math.sin(a - span) * r);
    g.arc(cx, cy, ro, a - span, a + span);
    g.lineTo(cx + Math.cos(a + span) * r, cy + Math.sin(a + span) * r);
    g.closePath();
    g.fill([0xa39d92, 0x958f84, 0x878177][k]).stroke({ width: 1.5, color: ROOF_INK });
  }
  g.circle(cx + PX * 0.3, cy + PX * 0.3, r).fill({ color: 0x000000, alpha: 0.3 });
  g.circle(cx, cy, r).fill(STONE).stroke({ width: 3, color: ROOF_INK });
  g.circle(cx, cy, r - PX * 0.18).fill(STONE_TOP).stroke({ width: 1.5, color: 0x6f6a61 });
  if (kind === 1) {
    // Fountain: a basin of water inside the rim, an inner bowl and the jet.
    g.circle(cx, cy, r - PX * 0.4).fill(0x3f6f86).stroke({ width: 2, color: 0x57534b });
    for (const f of [0.55, 0.78]) g.circle(cx, cy, (r - PX * 0.4) * f).stroke({ width: 1.2, color: 0xbfe3ef, alpha: 0.45 });
    g.circle(cx + PX * 0.12, cy + PX * 0.12, Math.max(PX * 0.5, r * 0.28)).fill({ color: 0x000000, alpha: 0.25 });
    g.circle(cx, cy, Math.max(PX * 0.5, r * 0.28)).fill(STONE_TOP).stroke({ width: 2, color: ROOF_INK });
    g.circle(cx, cy, Math.max(PX * 0.3, r * 0.17)).fill(0x5d93ab);
    g.circle(cx, cy, PX * 0.12).fill(0xeaf6fa);
    return;
  }
  if (kind === 2) {
    // Planter: a ring of coping round dark soil and grass (the tree stands in it).
    g.circle(cx, cy, r - PX * 0.35).fill(0x4a3a28).stroke({ width: 1.5, color: 0x57534b });
    for (let k = 0; k < 9; k++) {
      const t = (k / 9) * Math.PI * 2 + a;
      g.circle(cx + Math.cos(t) * (r - PX * 0.7), cy + Math.sin(t) * (r - PX * 0.7), PX * 0.22).fill({ color: 0x5f7a3e, alpha: 0.85 });
    }
    return;
  }
  // Flagstones in rings.
  for (let rr = r - PX * 0.18 - PX; rr > PX * 0.4; rr -= PX) g.circle(cx, cy, rr);
  g.stroke({ width: 1, color: 0x7d776c, alpha: 0.7 });
  for (let k = 0; k < 8; k++) {
    const t = (k / 8) * Math.PI * 2 + a + 0.3;
    g.moveTo(cx + Math.cos(t) * PX * 0.4, cy + Math.sin(t) * PX * 0.4).lineTo(cx + Math.cos(t) * (r - PX * 0.18), cy + Math.sin(t) * (r - PX * 0.18));
  }
  g.stroke({ width: 1, color: 0x7d776c, alpha: 0.5 });
  if (kind === 3) {
    // Market cross: a stepped plinth and a tall shaft with its cross-head, casting long.
    const h = Math.max(PX * 0.45, r * 0.42);
    g.circle(cx, cy, h).fill(STONE).stroke({ width: 2, color: ROOF_INK });
    g.moveTo(cx, cy).lineTo(cx + PX * 2.2, cy + PX * 2.2).stroke({ width: PX * 0.3, color: 0x000000, alpha: 0.28 });
    g.rect(cx - PX * 0.2, cy - PX * 0.2, PX * 0.4, PX * 0.4).fill(0xd8d1c2).stroke({ width: 1.5, color: ROOF_INK });
    g.rect(cx - PX * 0.45, cy - PX * 0.09, PX * 0.9, PX * 0.18).fill(0xd8d1c2).stroke({ width: 1.2, color: ROOF_INK });
  }
}

/**
 * A steppe town's dais (`size` 4, R5 towns 114): a low bank of the trodden ground's own earth, uneven
 * round its foot, its top lit toward the north-west and its far slope shaded, hoof prints worn round
 * its edge, a rutted way up where a dais has its steps, a few tufts of grass where hooves don't reach.
 */
export function drawMound(g: Graphics, cx: number, cy: number, r: number, a: number) {
  let seed = (Math.round(cx * 5.3) * 73856093) ^ (Math.round(cy * 2.9) * 19349663);
  const rnd = () => {
    seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) | 0;
    return ((seed ^ (seed >>> 13)) >>> 0) / 4294967296;
  };
  const [p1, p2, sq] = [rnd() * 6.28, rnd() * 6.28, 0.86 + rnd() * 0.12];
  // (An uneven round, a little long across the way up.)
  const blob = (x: number, y: number, rr: number, n = 22) => {
    const pts: number[] = [];
    for (let k = 0; k < n; k++) {
      const t = (k / n) * Math.PI * 2;
      const w = rr * (1 + 0.08 * Math.sin(3 * t + p1) + 0.05 * Math.cos(5 * t + p2));
      const [u, v] = [Math.cos(t) * w, Math.sin(t) * w * sq];
      pts.push(x + u * Math.cos(a) - v * Math.sin(a), y + u * Math.sin(a) + v * Math.cos(a));
    }
    return pts;
  };
  const R = r * 1.08;
  // Its shadow to the south-east, the foot's dark band, the slope, the sunlit top.
  g.poly(blob(cx + PX * 0.28, cy + PX * 0.34, R)).fill({ color: 0x3e2a18, alpha: 0.32 });
  g.poly(blob(cx, cy, R * 1.04)).fill(0x6e5034);
  g.poly(blob(cx, cy, R * 0.96)).fill(0x8a6844).stroke({ width: 1.6, color: 0x4e3824, alpha: 0.6 });
  g.poly(blob(cx - R * 0.07, cy - R * 0.08, R * 0.74)).fill(0xa07c54);
  g.poly(blob(cx - R * 0.12, cy - R * 0.14, R * 0.5)).fill({ color: 0xb48f63, alpha: 0.7 });
  // The way up: a worn, paler tongue down to the foot, two ruts in it.
  const [ux, uy, vx, vy] = [Math.cos(a), Math.sin(a), -Math.sin(a), Math.cos(a)];
  const at = (al: number, ac: number): [number, number] => [cx + ux * al + vx * ac, cy + uy * al + vy * ac];
  const wd = Math.min(PX * 0.7, R * 0.35);
  g.poly([...at(R * 0.35, -wd * 0.8), ...at(R * 1.12, -wd), ...at(R * 1.12, wd), ...at(R * 0.35, wd * 0.8)]).fill({ color: 0xa8845a, alpha: 0.85 });
  for (const c of [-0.4, 0.4]) g.moveTo(...at(R * 0.4, wd * c)).lineTo(...at(R * 1.1, wd * c * 1.15));
  g.stroke({ width: 1.4, color: 0x5e4329, alpha: 0.6 });
  // Hoof prints round the edge, in pairs, thicker by the way up.
  for (let k = 0; k < 30; k++) {
    const t = rnd() * Math.PI * 2;
    const d = R * (0.82 + rnd() * 0.26);
    const [hx, hy] = [cx + Math.cos(t) * d, cy + Math.sin(t) * d * sq];
    const hr = PX * (0.07 + rnd() * 0.03);
    g.moveTo(hx + hr, hy).arc(hx, hy, hr, 0, Math.PI * 1.3);
    g.moveTo(hx + PX * 0.2 + hr, hy + PX * 0.1).arc(hx + PX * 0.2, hy + PX * 0.1, hr, 0.3, Math.PI * 1.5);
  }
  g.stroke({ width: 1.3, color: 0x4a3420, alpha: 0.75 });
  // Tufts of grass on the top and the slope's far side.
  for (let k = 0; k < 7; k++) {
    const t = rnd() * Math.PI * 2;
    const d = R * (k < 3 ? rnd() * 0.45 : 0.6 + rnd() * 0.3);
    const [tx, ty] = [cx + Math.cos(t) * d, cy + Math.sin(t) * d * sq];
    const h = PX * (0.16 + rnd() * 0.1);
    for (const dx of [-0.6, 0, 0.6]) g.moveTo(tx + dx * h * 0.3, ty).lineTo(tx + dx * h * 0.8, ty - h * (dx === 0 ? 1.15 : 0.85));
    g.stroke({ width: 2.4, color: 0x3e4a22, cap: 'round' });
    for (const dx of [-0.6, 0, 0.6]) g.moveTo(tx + dx * h * 0.3, ty).lineTo(tx + dx * h * 0.8, ty - h * (dx === 0 ? 1.15 : 0.85));
    g.stroke({ width: 1.2, color: k % 2 ? 0x7f8f42 : 0x6c7c38, cap: 'round' });
  }
}

/**
 * A way underground, by kind (`size`): a stairwell going down (dungeon, crypt, catacombs), a
 * cave mouth, a timber-framed mine adit with rails running out, a lava-tube skylight, a round
 * sewer grate in the street. `pts`: the opening, then a point one square in along the passage.
 */
function drawEntrance(g: Graphics, s: Shape) {
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
  if (code >> 4 === 1) return drawRuinStair(g, at, quad, rnd);
  if (code >> 4 === 2) return drawMausoleum(g, at, quad, rnd);
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

/**
 * A stair down through a ruin's floor, about 6 ft wide and 12 deep: cracked steps darkening
 * into the dark, a kerb of fallen blocks with gaps, rubble and moss strewn round it.
 */
function drawRuinStair(g: Graphics, at: At, quad: Quad, rnd: () => number) {
  // A shadowed pit, then the steps.
  g.poly(quad(-0.6, 2.1, -0.7, 0.7)).fill({ color: 0x000000, alpha: 0.25 });
  g.poly(quad(-0.5, 2, -0.6, 0.6)).fill(0x1b1714);
  for (let k = 0; k < 5; k++) {
    const a = -0.5 + k * 0.5;
    const col = stoneShade(Math.min(1, k / 4.5));
    // Each step a few slabs, some cracked or missing.
    for (let b = -0.55; b < 0.5; b += 0.37) {
      if (rnd() < 0.12 && k > 1) continue;
      const w = 0.33 + rnd() * 0.03;
      g.poly(quad(a + 0.02, a + 0.42 - rnd() * 0.06, b, b + w)).fill(col);
      if (rnd() < 0.3) g.moveTo(...at(a + 0.05, b + w * rnd())).lineTo(...at(a + 0.38, b + w * rnd())).stroke({ width: 1.5, color: 0x2a2520, alpha: 0.7 });
    }
    g.moveTo(...at(a + 0.42, -0.55)).lineTo(...at(a + 0.42, 0.55)).stroke({ width: 1.2, color: 0x2a2520, alpha: 0.6 });
  }
  // The kerb: worn blocks along both sides and the back, some fallen out of line.
  const block = (a: number, b: number, la: number, lb: number) => {
    const j = (rnd() - 0.5) * 0.12;
    const p = [...at(a + j, b - j), ...at(a + la + j, b - j), ...at(a + la - j, b + lb + j), ...at(a - j, b + lb + j)];
    g.poly(p.map((v) => v + 3)).fill({ color: 0x000000, alpha: 0.3 });
    g.poly(p).fill(rnd() < 0.5 ? STONE : STONE_TOP).stroke({ width: 1.8, color: ROOF_INK });
  };
  for (let a = -0.5; a < 2; a += 0.62) {
    for (const side of [-1, 1]) {
      if (rnd() < 0.25) continue;
      block(a, side < 0 ? -0.95 : 0.62, 0.56, 0.34);
    }
  }
  for (let b = -0.95; b < 0.95; b += 0.62) if (rnd() > 0.3) block(2, b, 0.34, 0.56);
  // Rubble and moss about the mouth.
  for (let k = 0; k < 10; k++) {
    const a = -1 + rnd() * 3.4;
    const b = (rnd() < 0.5 ? -1 : 1) * (1.05 + rnd() * 0.7);
    const r = 0.06 + rnd() * 0.1;
    const [cx, cy] = at(a, b);
    g.circle(cx + 2, cy + 2, r * PX).fill({ color: 0x000000, alpha: 0.25 });
    g.circle(cx, cy, r * PX).fill(stoneShade(0.15 + rnd() * 0.3)).stroke({ width: 1.2, color: ROOF_INK, alpha: 0.8 });
  }
  for (let k = 0; k < 4; k++) {
    const [cx, cy] = at(rnd() * 2.2, (rnd() - 0.5) * 1.9);
    g.circle(cx, cy, (0.15 + rnd() * 0.15) * PX).fill({ color: 0x55703a, alpha: 0.45 });
  }
}

/**
 * A graveyard mausoleum over the catacomb stair, 3 squares wide and 4 deep: a pale stone
 * plinth, heavy walls on three sides, columns either side of the open door, the stair going
 * down inside; a slab path out of the door, urns at its corners.
 */
function drawMausoleum(g: Graphics, at: At, quad: Quad, rnd: () => number) {
  // Shadow (light from the NW), plinth, the threshold slabs out front.
  g.poly(quad(-1.5, 2.5, -1.5, 1.5).map((v) => v + 0.35 * PX)).fill({ color: 0x000000, alpha: 0.3 });
  g.poly(quad(-1.5, 2.5, -1.5, 1.5)).fill(0xc4bdae).stroke({ width: 2.5, color: ROOF_INK });
  for (let k = 0; k < 2; k++) g.poly(quad(-2.4 + k * 0.45, -2.4 + k * 0.45 + 0.4, -0.55, 0.55)).fill(0xa8a294).stroke({ width: 1.2, color: ROOF_INK, alpha: 0.8 });
  // Walls on the sides and back.
  const wall = 0x8f897d;
  g.poly(quad(-1.2, 2.5, -1.5, -1.05)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  g.poly(quad(-1.2, 2.5, 1.05, 1.5)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  g.poly(quad(2.05, 2.5, -1.05, 1.05)).fill(wall).stroke({ width: 2, color: ROOF_INK });
  // Coping along the wall tops.
  for (const b of [-1.28, 1.28]) g.moveTo(...at(-1.15, b)).lineTo(...at(2.45, b));
  g.moveTo(...at(2.28, -1.25)).lineTo(...at(2.28, 1.25));
  g.stroke({ width: 1.5, color: 0xd6cfbf, alpha: 0.8 });
  // The stair inside, darkening as it goes down toward the back.
  g.poly(quad(-0.6, 2.05, -1.05, 1.05)).fill(0x1b1714);
  for (let k = 0; k < 5; k++) {
    const a = -0.55 + k * 0.52;
    g.poly(quad(a, a + 0.44, -0.98, 0.98)).fill(stoneShade(0.1 + k * 0.2));
  }
  // Columns either side of the door, a lintel slab across.
  for (const b of [-1.25, 1.25]) {
    const [cx, cy] = at(-1.25, b);
    g.circle(cx + 2.5, cy + 2.5, 0.3 * PX).fill({ color: 0x000000, alpha: 0.3 });
    g.circle(cx, cy, 0.3 * PX).fill(0xd8d1c2).stroke({ width: 2, color: ROOF_INK });
    g.circle(cx - 1.5, cy - 1.5, 0.12 * PX).fill({ color: 0xffffff, alpha: 0.35 });
  }
  // Urns at the front corners of the path.
  for (const b of [-0.85, 0.85]) {
    const [cx, cy] = at(-2.15, b);
    g.circle(cx + 2, cy + 2, 0.18 * PX).fill({ color: 0x000000, alpha: 0.3 });
    g.circle(cx, cy, 0.18 * PX).fill(0x6f675b).stroke({ width: 1.5, color: ROOF_INK });
    g.circle(cx, cy, 0.08 * PX).fill(0x2a2520);
  }
  // Moss and lichen on the stone.
  for (let k = 0; k < 4; k++) {
    const [cx, cy] = at(-1.1 + rnd() * 3.5, (rnd() < 0.5 ? -1.3 : 1.3) + (rnd() - 0.5) * 0.2);
    g.circle(cx, cy, (0.08 + rnd() * 0.1) * PX).fill({ color: 0x5f7a3e, alpha: 0.55 });
  }
}
