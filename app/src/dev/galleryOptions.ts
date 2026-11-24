// `?gallery=1&panels=options`: the sprite audit's options (R7 P8b) side by side, each on the 5-ft grid with a 2-ft
// body disc and a label, at `&sq=` px a square (default 64): the street's drain grate and the sewer manhole on
// cobbles and on plain ground (A today's, B and C `render/battlemap/grates.ts`); the wall hangings from straight
// above (A today's, B thin strip, C strip with a draped foot: `render/furniture/hangings.ts`, the stage and the
// cloth of estate through `stage(…, 'B')` and `throneDais(…, 'B')`); the tower flags (A today's `banner`, B and C
// `render/walls/flags.ts`). Nothing here is drawn on the map.
import { Container, Graphics, Text } from 'pixi.js';
import type { InteriorItem } from '../gen/protocol';
import { rng } from '../render/atlas';
import { drawStructures } from '../render/battlemap/structures';
import { drawDrainB, drawDrainC, drawManholeB } from '../render/battlemap/grates';
import type { Light } from '../render/furniture/common';
import { tatteredBanner } from '../render/furniture/dungeon';
import { guildArms, jobBoard } from '../render/furniture/guild';
import { tapestry, throneDais, wallBanner } from '../render/furniture/hall';
import { guildArmsOpt, jobBoardOpt, tapestryOpt, tatteredBannerOpt, wallBannerOpt } from '../render/furniture/hangings';
import { stage } from '../render/furniture/sacred';
import { PX } from '../render/scale';
import { INK } from '../render/pen';
import { bannerFromAbove } from '../render/walls/flags';
import { banner } from '../render/walls/tops';

const LIGHT: Light = [-Math.SQRT1_2, -Math.SQRT1_2];
const LABEL = { fontFamily: 'system-ui, sans-serif', fontSize: 12, fill: 0x2a241c };
const HEAD = { fontFamily: 'system-ui, sans-serif', fontSize: 15, fontWeight: '600' as const, fill: 0x2a241c };

/** One option: `w` x `h` squares of ground, its label, and how to draw it with its top-left at (x, y) px. */
type Cell = { w: number; h: number; label: string; draw: (layer: Container, x: number, y: number, sq: number) => void };

const item = (kind: string, w: number, h: number): InteriorItem => ({ kind, name: kind, x: 0, y: 0, w, h, cover: 0, blocks_move: false, height_ft: 0 });

/** A faint 5-ft grid over a strip; `cobbled` lays rows of stones under it first, else plain packed ground. */
function ground(g: Graphics, x0: number, y0: number, w: number, h: number, sq: number, kind: 'cobbles' | 'plain' | 'floor' | 'roof') {
  g.rect(x0, y0, w, h).fill({ cobbles: 0x8f897d, plain: 0xb8a888, floor: 0xa89c86, roof: 0xa49c8e }[kind]);
  if (kind === 'cobbles') {
    let n = 7;
    const [cw, ch] = [sq * 0.19, sq * 0.14];
    for (let yy = y0 + 2, row = 0; yy < y0 + h - ch * 0.5; yy += ch + 2, row++)
      for (let xx = x0 + 2 + (row % 2) * (cw / 2); xx < x0 + w - cw; xx += cw + 2) {
        n = (n * 1103515245 + 12345) & 0x7fffffff;
        g.roundRect(xx, yy, cw, ch, 3).fill([0xa6a094, 0x9a9488, 0xaea89c][(n >> 8) % 3]).stroke({ width: 0.8, color: 0x5d574c });
      }
  }
  if (kind === 'roof') {
    // Flags in staggered courses.
    for (let yy = y0, row = 0; yy < y0 + h; yy += sq * 0.5, row++) {
      g.moveTo(x0, yy).lineTo(x0 + w, yy);
      for (let xx = x0 + (row % 2) * sq * 0.35; xx < x0 + w; xx += sq * 0.7) g.moveTo(xx, yy).lineTo(xx, Math.min(y0 + h, yy + sq * 0.5));
    }
    g.stroke({ width: 1, color: 0x7a7266, alpha: 0.6 });
  }
  for (let x = x0 + sq; x < x0 + w - 1; x += sq) g.moveTo(x, y0).lineTo(x, y0 + h);
  for (let y = y0 + sq; y < y0 + h - 1; y += sq) g.moveTo(x0, y).lineTo(x0 + w, y);
  g.stroke({ width: 1, color: 0x000000, alpha: 0.14 });
}

const disc = (layer: Container, x: number, y: number, sq: number) => layer.addChild(new Graphics().circle(x, y, 0.2 * sq).fill(0xe9dfc4).stroke({ width: 1, color: INK }));

/** A row of cells on one strip of ground, labelled under each; returns the y below it. */
function row(root: Container, y0: number, sq: number, title: string, kind: 'cobbles' | 'plain' | 'floor' | 'roof', cells: Cell[], nolabels: boolean): number {
  const t = new Text({ text: title, style: HEAD });
  t.position.set(16, y0);
  root.addChild(t);
  const y = y0 + 24;
  const w = cells.reduce((a, c) => a + c.w, 0) * sq;
  const h = Math.max(...cells.map((c) => c.h)) * sq;
  const g = new Graphics();
  ground(g, 16, y, w, h, sq, kind);
  root.addChild(g);
  const layer = new Container();
  root.addChild(layer);
  let x = 16;
  for (const c of cells) {
    c.draw(layer, x, y, sq);
    if (!nolabels) {
      const l = new Text({ text: c.label, style: LABEL });
      l.position.set(x + 4, y + h + 3);
      root.addChild(l);
    }
    x += c.w * sq;
  }
  return y + h + 26;
}

/** A battlemap drawing in atlas px at (cx, cy) px, scaled to `sq`. */
function atPx(layer: Container, cx: number, cy: number, sq: number, draw: (g: Graphics) => void, g = new Graphics()) {
  draw(g);
  g.scale.set(sq / PX);
  g.position.set(cx, cy);
  layer.addChild(g);
}

/** The street frame `drawEntrance` gives (along +x), centred on the origin. */
const at = (a: number, b: number): [number, number] => [a * PX, b * PX];
const quad = (a0: number, a1: number, b0: number, b1: number) => [...at(a0, b0), ...at(a1, b0), ...at(a1, b1), ...at(a0, b1)];

function grateCells(): Cell[] {
  const cell = (label: string, draw: (g: Graphics) => void, w = 2): Cell => ({
    w,
    h: 2,
    label,
    draw: (layer, x, y, sq) => {
      atPx(layer, x + (w * sq) / 2, y + sq, sq, draw);
      disc(layer, x + (w - 0.35) * sq, y + 1.7 * sq, sq);
    },
  });
  const today = (code: number) => (layer: Container, x: number, y: number, sq: number) => {
    const g = drawStructures([{ kind: 4, size: code * PX, pts: [0, 0, PX, 0] }]);
    atPx(layer, x + sq, y + sq, sq, () => {}, g);
    disc(layer, x + 1.65 * sq, y + 1.7 * sq, sq);
  };
  return [
    { w: 2, h: 2, label: 'drain A today 4.2x3 ft', draw: today(3 << 4) },
    cell('drain B 2x1.5 ft', (g) => drawDrainB(g, at, quad)),
    cell('drain C 1.5x1 ft, gutter', (g) => drawDrainC(g, at, quad), 3),
    { w: 2, h: 2, label: 'manhole A today 5.5 ft', draw: today(5) },
    cell('manhole B 2.5 ft', (g) => drawManholeB(g, at, 0.25)),
    cell('manhole C 3 ft', (g) => drawManholeB(g, at, 0.3)),
  ];
}

/** A hanging on a wall along the top of its cell: the wall's band, the piece `w` x `h` below it, a body disc. */
function hung(label: string, w: number, h: number, draw: (g: Graphics, f: InteriorItem) => void, kind = 'piece'): Cell {
  return {
    w: w + 1,
    h: h + 0.6,
    label,
    draw: (layer, x, y, sq) => {
      const wall = new Graphics();
      wall.rect(0, -0.6, w + 1, 0.62).fill(0x6a6256);
      wall.moveTo(0, 0.02).lineTo(w + 1, 0.02).stroke({ width: 0.045, color: INK });
      wall.scale.set(sq);
      wall.position.set(x, y + 0.6 * sq);
      layer.addChild(wall);
      const g = new Graphics();
      draw(g, item(kind, w, h));
      g.scale.set(sq);
      g.position.set(x + 0.5 * sq, y + 0.6 * sq);
      layer.addChild(g);
      disc(layer, x + (w + 0.6) * sq, y + (h + 0.25) * sq, sq);
    },
  };
}

const BACK: [number, number] = [0, -1];
const R = (seed: number) => rng(seed);

function hangingRows(): [string, Cell[]][] {
  return [
    [
      'Tapestry (3x1)',
      [
        hung('A today', 3, 1, (g, f) => tapestry(g, f, LIGHT, 0, R(1), BACK)),
        hung('B thin strip 0.13 sq', 3, 1, (g, f) => tapestryOpt(g, f, LIGHT, 0, BACK, 'B')),
        hung('C draped foot 0.3 sq', 3, 1, (g, f) => tapestryOpt(g, f, LIGHT, 0, BACK, 'C')),
      ],
    ],
    [
      'Wall banner, dungeon banner (1x1)',
      [
        hung('A today', 1, 1, (g, f) => wallBanner(g, f, LIGHT, 0, BACK)),
        hung('B strip', 1, 1, (g, f) => wallBannerOpt(g, f, LIGHT, 0, BACK, 'B')),
        hung('C draped', 1, 1, (g, f) => wallBannerOpt(g, f, LIGHT, 0, BACK, 'C')),
        hung('dungeon A today', 1, 1, (g, f) => tatteredBanner(g, f, LIGHT, 0, R(2), BACK)),
        hung('dungeon B', 1, 1, (g, f) => tatteredBannerOpt(g, f, LIGHT, 0, BACK, 'B')),
        hung('dungeon C', 1, 1, (g, f) => tatteredBannerOpt(g, f, LIGHT, 0, BACK, 'C')),
      ],
    ],
    [
      'Guild arms, job board (2x1)',
      [
        hung('arms A today', 2, 1, (g, f) => guildArms(g, f, LIGHT, 0, R(3), BACK)),
        hung('arms B', 2, 1, (g, f) => guildArmsOpt(g, f, LIGHT, 0, BACK, 'B')),
        hung('arms C', 2, 1, (g, f) => guildArmsOpt(g, f, LIGHT, 0, BACK, 'C')),
        hung('board A today', 2, 1, (g, f) => jobBoard(g, f, LIGHT, 0, R(4), BACK)),
        hung('board B plank edge', 2, 1, (g, f) => jobBoardOpt(g, f, LIGHT, 0, BACK, 'B', R(5))),
        hung('board C leaning', 2, 1, (g, f) => jobBoardOpt(g, f, LIGHT, 0, BACK, 'C', R(5))),
      ],
    ],
    [
      'Stage curtain, cloth of estate (3x2)',
      [
        hung('stage A today', 3, 2, (g, f) => stage(g, f, LIGHT, R(6), BACK)),
        hung('stage B band', 3, 2, (g, f) => stage(g, f, LIGHT, R(6), BACK, 'B')),
        hung('dais A today', 3, 2, (g, f) => throneDais(g, f, LIGHT, 0, R(7), BACK)),
        hung('dais B band', 3, 2, (g, f) => throneDais(g, f, LIGHT, 0, R(7), BACK, 'B')),
      ],
    ],
  ];
}

function flagCells(): Cell[] {
  const cell = (label: string, draw: (g: Graphics) => void): Cell => ({
    w: 3,
    h: 3,
    label,
    draw: (layer, x, y, sq) => {
      atPx(layer, x + 0.9 * sq, y + 1.3 * sq, sq, draw);
      disc(layer, x + 2.5 * sq, y + 2.5 * sq, sq);
    },
  });
  return [
    cell('flag A today (side view)', (g) => banner(g, 0, PX * 0.6)),
    cell('flag B ribbon 0.11 sq', (g) => bannerFromAbove(g, 0, 0)),
    cell('flag C ribbon 0.16 sq, two tones', (g) => bannerFromAbove(g, 0, 0, true)),
  ];
}

/** The options panel; returns the y below it. */
export function spriteOptions(root: Container, y0: number, nolabels: boolean, sq: number): number {
  let y = y0;
  const grates = grateCells();
  y = row(root, y, sq, 'Grates on cobbles', 'cobbles', grates, nolabels);
  y = row(root, y, sq, 'Grates on plain ground', 'plain', grates, nolabels);
  for (const [title, cells] of hangingRows()) y = row(root, y, sq, title, 'floor', cells, nolabels);
  y = row(root, y, sq, 'Tower flags (on a tower top)', 'roof', flagCells(), nolabels);
  return y + 10;
}
