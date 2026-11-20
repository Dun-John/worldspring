// Gallery scenes for the harbour's vector structures (Roadmap 7 P8, batch A): `&items=breakwaters`
// (breakwater armour round a mole and along bent lines) and `&items=piers` (timber piers, a timber
// road bridge over a stream, a fenced timber deck, a timber shore quay), each as the battlemap draws
// it, the 5-ft grid over the decks, at `sq` px a square.
import { Container, Graphics, Text, type TextStyleOptions } from 'pixi.js';
import type { Shape } from '../gen/battlePrep';
import { deckGrid, drawDeck, drawRoadBridge } from '../render/bridges';
import { drawBreakwaterArmour } from '../render/harbourShapes';
import { PX } from '../render/scale';

const LABEL: TextStyleOptions = { fontFamily: 'Georgia, serif', fontSize: 13, fill: 0x2a2219 };
const WATER = 0x5e8a8e;
const QUAY = 0xb4ab96;
const GRASS = 0x8d9a5a;
const FT = PX / 5;

type P = [number, number];

/** A scene `wf` x `hf` ft drawn in battlemap px (`PX` a square) into a container scaled to `sq`. */
function scene(root: Container, x: number, y: number, wf: number, hf: number, sq: number, label: string, nolabels: boolean, body: (g: Graphics, over: Graphics) => void): number {
  const c = new Container();
  c.position.set(x, y);
  c.scale.set(sq / PX);
  const [g, over] = [new Graphics(), new Graphics()];
  const body0 = new Container();
  body0.addChild(g, over);
  c.addChild(body0);
  const mask = new Graphics().rect(0, 0, wf * FT, hf * FT).fill(0xffffff);
  c.addChild(mask);
  body0.mask = mask;
  body(g, over);
  root.addChild(c);
  if (!nolabels) {
    const t = new Text({ text: label, style: LABEL });
    t.position.set(x, y + hf * FT * (sq / PX) + 2);
    root.addChild(t);
  }
  return wf * FT * (sq / PX);
}

function grid(g: Graphics, w: number, h: number) {
  for (let x = PX; x < w - 1; x += PX) g.moveTo(x, 0).lineTo(x, h);
  for (let y = PX; y < h - 1; y += PX) g.moveTo(0, y).lineTo(w, y);
  g.stroke({ width: 1.5, color: 0x000000, alpha: 0.12 });
}

/** Lays scenes left to right, wrapping at `width`. */
function flow(width: number, y0: number, sq: number) {
  let [x, y, rowH] = [16, y0, 0];
  return {
    put(wf: number, hf: number, draw: (x: number, y: number) => void) {
      const [w, h] = [(wf / 5) * sq, (hf / 5) * sq];
      if (x > 16 && x + w > width - 16) [x, y, rowH] = [16, y + rowH + 30, 0];
      draw(x, y);
      x += w + 24;
      rowH = Math.max(rowH, h);
    },
    end: () => y + rowH + 34,
  };
}

/** A mole along centreline `c` (ft), `wf` ft wide with a round head: its paving, then its armour as the
 * battlemap lays it (each side and the head's rim run with the water on their left-hand normal). */
function mole(g: Graphics, c: P[], wf: number, seed: number) {
  const px = c.map(([a, b]): P => [a * FT, b * FT]);
  const half = (wf / 2) * FT;
  // Each side offset with mitred bends, so it runs on unbroken into the head's rim.
  const side = (s: number): P[] =>
    px.map((p, i) => {
      const [a, b] = [px[Math.max(0, i - 1)], px[Math.min(px.length - 1, i + 1)]];
      const n = (q: P, r: P): P => {
        const l = Math.hypot(r[0] - q[0], r[1] - q[1]);
        return [-(r[1] - q[1]) / l, (r[0] - q[0]) / l];
      };
      const [n0, n1] = [n(a === p ? p : a, a === p ? b : p), n(b === p ? a : p, b === p ? p : b)];
      const [mx, my] = [n0[0] + n1[0], n0[1] + n1[1]];
      const d = (s * half) / Math.max(0.5, (mx * n0[0] + my * n0[1]) || 1);
      return [p[0] + mx * d, p[1] + my * d];
    });
  const [tip, last] = [px[px.length - 1], px[px.length - 2]];
  const a0 = Math.atan2(tip[1] - last[1], tip[0] - last[0]);
  const rim: P[] = [];
  for (let k = 1; k < 16; k++) {
    const a = a0 + Math.PI / 2 - (Math.PI * k) / 16;
    rim.push([tip[0] + half * Math.cos(a), tip[1] + half * Math.sin(a)]);
  }
  const [l, r] = [side(1), side(-1)];
  const edge = [...l, ...rim, ...[...r].reverse()];
  g.poly(edge.flat()).fill(QUAY);
  drawBreakwaterArmour(g, edge, PX, 9, seed);
}

/** `&items=breakwaters`: armour round a mole and its head, then along bent lines on water (9 and 12 ft). */
export function breakwaters(root: Container, y0: number, width: number, nolabels: boolean, sq: number): number {
  const f = flow(width, y0, sq);
  f.put(80, 45, (x, y) =>
    scene(root, x, y, 80, 45, sq, 'a mole 20 ft wide, armour along both sides and round its head', nolabels, (g) => {
      g.rect(0, 0, 80 * FT, 45 * FT).fill(WATER);
      g.rect(0, 0, 8 * FT, 45 * FT).fill(GRASS);
      mole(g, [[0, 22], [40, 20], [64, 24]], 20, 3);
      grid(g, 80 * FT, 45 * FT);
    }),
  );
  for (const [wf, seed] of [[9, 1], [12, 7]] as const) {
    f.put(60, 30, (x, y) =>
      scene(root, x, y, 60, 30, sq, `along a bent line, ${wf} ft (water both sides; foam on its left-hand side)`, nolabels, (g) => {
        g.rect(0, 0, 60 * FT, 30 * FT).fill(WATER);
        drawBreakwaterArmour(g, [[4 * FT, 20 * FT], [22 * FT, 11 * FT], [40 * FT, 9 * FT], [56 * FT, 17 * FT]], PX, wf, seed);
        grid(g, 60 * FT, 30 * FT);
      }),
    );
  }
  return f.end();
}

const deck = (pts: P[], style: number): Shape => ({ kind: 2, size: style * PX, pts: pts.flatMap(([a, b]) => [a * FT, b * FT]) });

/** `&items=piers`: timber piers off a shore, a timber road bridge over a stream, a fenced deck, a timber
 * shore quay; the 5-ft grid over each deck as the battlemap draws it. */
export function piers(root: Container, y0: number, width: number, nolabels: boolean, sq: number): number {
  const f = flow(width, y0, sq);
  // (The deck grid on its own graphics over the decks, as the battlemap's.)
  const lay = (g: Graphics, over: Graphics, shapes: Shape[]) => {
    for (const s of shapes) (s.kind === 3 ? drawRoadBridge : drawDeck)(g, s);
    for (const s of shapes) deckGrid(over, s);
  };
  f.put(60, 50, (x, y) =>
    scene(root, x, y, 60, 50, sq, 'piers: 10 ft x 40 ft square on, 15 ft x 35 ft turned', nolabels, (g, over) => {
      g.rect(0, 0, 60 * FT, 50 * FT).fill(WATER);
      g.poly([0, 0, 14 * FT, 0, 11 * FT, 50 * FT, 0, 50 * FT]).fill(GRASS);
      grid(g, 60 * FT, 50 * FT);
      lay(g, over, [
        deck([[10, 8], [50, 8], [50, 18], [10, 18]], 0),
        deck([[11, 28], [43, 42], [37, 55.7], [5, 41.7]], 0),
      ]);
    }),
  );
  f.put(50, 50, (x, y) =>
    scene(root, x, y, 50, 50, sq, 'timber road bridge, 15 ft, over a stream (lands on the banks)', nolabels, (g, over) => {
      g.rect(0, 0, 50 * FT, 50 * FT).fill(GRASS);
      g.poly([18 * FT, 0, 32 * FT, 0, 34 * FT, 50 * FT, 16 * FT, 50 * FT]).fill(WATER);
      grid(g, 50 * FT, 50 * FT);
      lay(g, over, [{ kind: 3, size: 15 * FT, pts: [10 * FT, 22 * FT, 40 * FT, 28 * FT] }]);
    }),
  );
  f.put(45, 50, (x, y) =>
    scene(root, x, y, 45, 50, sq, 'fenced timber deck (style 1), 15 ft x 30 ft', nolabels, (g, over) => {
      g.rect(0, 0, 45 * FT, 50 * FT).fill(WATER);
      g.rect(0, 0, 45 * FT, 8 * FT).fill(GRASS);
      g.rect(0, 42 * FT, 45 * FT, 8 * FT).fill(GRASS);
      grid(g, 45 * FT, 50 * FT);
      lay(g, over, [deck([[15, 5], [15, 45], [30, 45], [30, 5]], 1)]);
    }),
  );
  f.put(70, 40, (x, y) =>
    scene(root, x, y, 70, 40, sq, 'timber shore quay along a bent shore, 10-14 ft deep', nolabels, (g, over) => {
      g.rect(0, 0, 70 * FT, 40 * FT).fill(WATER);
      const shore: P[] = [[0, 22], [20, 18], [40, 19], [70, 14]];
      g.poly([...shore.flatMap(([a, b]) => [a * FT, b * FT]), 70 * FT, 0, 0, 0]).fill(GRASS);
      grid(g, 70 * FT, 40 * FT);
      const face: P[] = [[0, 33], [20, 30], [40, 32], [70, 28]];
      const back: P[] = [[0, 21], [20, 17], [40, 18], [70, 13]];
      lay(g, over, [deck([...face, ...[...back].reverse()], 4)]);
    }),
  );
  return f.end();
}
