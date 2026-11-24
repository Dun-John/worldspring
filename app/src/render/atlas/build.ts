// Procedural painterly sprite atlas for battlemap objects (top-down, light from the NW):
// layered fills with a darker ink outline and NW highlights, in the spirit of hand-painted
// battlemap assets. Drawn once into one mipmapped texture (frames padded apart so no mip level
// blends two of them) so objects render as instanced particles.
// Frames are addressed by kind id and variant, so generated art can replace them later.
import { PX, MIP_LEVELS, PAD } from '../scale';
import { Texture, Graphics, type Renderer, Container, Rectangle } from 'pixi.js';
import { ballista, shotStones } from '../fortProps';
import type { KindInfo } from '../../gen/client';
import { brambles, cactus, canopy, conifer, deadTree, mushroomRing, palm, reeds, strokes, tallGrass, wildflowers } from './plants';
import { bones, fallenLog, stump } from './wood';
import { boulderStone, rockPile, scree, smallRocks } from './stones';
import { barrels, cart, crates, haystack, headstone, marketStall, statue, well } from './yard';
import { INK, rng } from './common';
import { bedroll, campfire, firewood, tents } from './camp';
import { lampPost, marketCross, noticeBoard, signpost, stocks, waterTrough } from './street';
import { fountain, pond } from './water';
import { bench, flowerbed, fruitTree, hedge, topiary, waterWheel } from './garden';
import { bandstand, footbridge, gardenStatue, gazebo } from './structures';
import { basaltPillar, bog, iceSpire, lava, obsidian, quicksand, sinkhole, snowdrift, steamVent, thinIce } from './hazards';
import { beehive, gate, ironRailing, picketFence, stockade, vegRow, wattleFence } from './farm';
import { anvil, cairn, kurgan, lychgate, standingStone, tombSlab, yew } from './graves';
import { hitchingRail, horse } from './stable';
import { bollard, buoy, dockCrane, fishingBoat, netRack, rowboat } from './harbour';
import { looks, type Atlas } from '../atlas';

/** Atlas width: wide enough that the height stays within 4096 too (many phones' largest texture). */
const ATLAS_W = 4096;

type Draw = (g: Graphics, r: number, rnd: () => number, v: number) => void;

const DRAW: Record<number, Draw> = {
  1: (g, r, rnd, v) => canopy(g, r, rnd, [[0x2f4a22, 0x4d6e2d, 0x7a9a3c], [0x34502a, 0x557a32, 0x88a846], [0x3a4a20, 0x62702c, 0x9aa24a], [0x2c4426, 0x46683a, 0x6f9450]][v], 7),
  2: conifer,
  3: palm,
  4: (g, r, rnd, v) => canopy(g, r, rnd, [[0x1e4a24, 0x2e6a2e, 0x4f8f3a], [0x1c4230, 0x2b6240, 0x4a8850], [0x224a1e, 0x357028, 0x5a9a38], [0x1a3e22, 0x2a5a2e, 0x468040]][v], 9),
  5: (g, r, rnd) => {
    canopy(g, r, rnd, [0x5a6a26, 0x7d8a34, 0xa9ad54], 6);
  },
  6: deadTree,
  7: (g, r, rnd) => {
    canopy(g, r, rnd, [0x3e5e44, 0x5f8058, 0x8aaa78], 8);
    strokes(g, r * 0.9, rnd, 14, r * 0.35, [0x9ab888, 0x6a8a5a], 1.5);
  },
  8: (g, r, rnd, v) => canopy(g, r, rnd, [[0x3f5f2a, 0x5f7f35, 0x86a048], [0x44602e, 0x66843a, 0x8ea650], [0x4a5a2a, 0x6e7a34, 0x98a04a], [0x385a30, 0x587c40, 0x7ea058]][v], 5),
  9: (g, r, rnd) => {
    canopy(g, r, rnd, [0x2e4a22, 0x46642c, 0x6a8a3c], 11);
    strokes(g, r, rnd, 10, r * 0.25, [0x3a2e20], 1.5);
  },
  10: fallenLog,
  11: stump,
  12: boulderStone,
  // Town props.
  33: barrels,
  34: crates,
  35: cart,
  36: marketStall,
  37: well,
  39: headstone,
  40: (g, r, rnd, v) => {
    // Ruined wall segment: a run of fitted stones along x, broken ends, moss.
    const len = r * 1.75;
    const t = r * 0.95;
    const stones = 4 + (v % 3);
    for (let i = 0; i < stones; i++) {
      const x0 = -len / 2 + (len * i) / stones;
      const sw = len / stones - 1.5;
      const jitter = (rnd() - 0.5) * 3;
      const tone = [0x8e877a, 0x9a9384, 0x807a6e][Math.floor(rnd() * 3)];
      g.rect(x0, -t / 2 + jitter, sw, t).fill(tone).stroke({ width: 2, color: INK });
      g.rect(x0 + 2, -t / 2 + jitter + 2, sw - 4, t * 0.25).fill({ color: 0xb8b0a0, alpha: 0.6 });
    }
    for (let i = 0; i < 3; i++) g.circle((rnd() - 0.5) * len, t / 2 + 2 + rnd() * 4, 2 + rnd() * 3).fill(0x7e776a).stroke({ width: 1, color: INK });
    if (rnd() < 0.6) g.circle((rnd() - 0.5) * len * 0.6, (rnd() - 0.5) * t * 0.5, t * 0.3).fill({ color: 0x5a7a3a, alpha: 0.6 });
  },
  41: statue,
  42: tents,
  43: campfire,
  44: firewood,
  45: bedroll,
  // Squares and streets.
  46: waterTrough,
  47: noticeBoard,
  48: signpost,
  49: stocks,
  50: lampPost,
  51: marketCross,
  // Parks and gardens.
  52: fountain,
  53: pond,
  54: bench,
  55: flowerbed,
  56: hedge,
  57: topiary,
  58: gazebo,
  59: bandstand,
  60: gardenStatue,
  61: footbridge,
  // v63 countryside: an orchard's fruit tree and a water mill's wheel.
  62: fruitTree,
  63: (g, r) => waterWheel(g, r),
  38: haystack,
  13: smallRocks,
  14: rockPile,
  15: reeds,
  16: cactus,
  17: tallGrass,
  18: wildflowers,
  19: mushroomRing,
  20: bones,
  21: snowdrift,
  22: iceSpire,
  23: basaltPillar,
  24: obsidian,
  25: brambles,
  26: bog,
  27: quicksand,
  28: thinIce,
  29: lava,
  30: steamVent,
  31: scree,
  32: sinkhole,
  // v63 castles: a kitchen garden and a smithy's yard.
  75: vegRow,
  76: wattleFence,
  77: beehive,
  78: anvil,
  // Roadmap 5 cemeteries.
  100: cairn,
  101: standingStone,
  102: yew,
  103: lychgate,
  104: kurgan,
  105: tombSlab,
  // Roadmap 5 steppe towns' horse parks.
  106: horse,
  107: hitchingRail,
  // Roadmap 5 star walls' bastions (`fortProps.ts`).
  108: ballista,
  109: shotStones,
  // Roadmap 5 harbours (M20).
  140: rowboat,
  141: fishingBoat,
  142: bollard,
  143: dockCrane,
  144: netRack,
  145: buoy,
  // Roadmap 6 fences and walls (`town::fence`), five-foot pieces along x.
  150: picketFence,
  151: ironRailing,
  152: stockade,
  155: gate,
};

/** Room in the frame beyond the footprint (squares, by kind id) for things drawn bigger than their catalog radius
 * (tall things drawn from a high angle; the standing stone, whose top is longer than its footprint is wide). */
const ROOM: Record<number, number> = { 155: 0.85, 142: 0.72, 107: 1.06, 100: 1.9, 101: 0.45, 103: 1.45, 105: 0.84, 62: 1.16, 56: 0.68 };

export function frameRadiusPx(info: KindInfo): number {
  // Radius in atlas px at scale 1 plus room for outlines.
  return Math.ceil(Math.max(info.radius, ROOM[info.id] ?? 0) * PX + 6);
}

export function buildAtlas(renderer: Renderer, catalog: KindInfo[]): Atlas {
  const root = new Container();
  const placements: { id: number; v: number; x: number; y: number; size: number }[] = [];
  let x = 0;
  let y = 0;
  let rowH = 0;
  // Frames start on the PAD grid with at least PAD empty pixels after them.
  const step = (size: number) => Math.ceil(size / PAD) * PAD + PAD;
  const place = (size: number) => {
    if (x + step(size) > ATLAS_W) {
      x = 0;
      y += rowH;
      rowH = 0;
    }
    const at = { x, y };
    x += step(size);
    rowH = Math.max(rowH, step(size));
    return at;
  };
  for (const info of catalog) {
    const rpx = frameRadiusPx(info);
    const size = rpx * 2;
    for (let v = 0; v < looks(info.id); v++) {
      const at = place(size);
      const g = new Graphics();
      g.position.set(at.x + rpx, at.y + rpx);
      (DRAW[info.id] ?? DRAW[12])(g, info.radius * PX, rng(info.id * 131 + v * 17 + 7), v);
      if (import.meta.env.DEV) {
        // Art past the frame is cut off flat (and could bleed into the next frame's mips).
        const b = g.getLocalBounds();
        const over = Math.max(-b.minX, b.maxX, -b.minY, b.maxY) - rpx;
        if (over > 0.5) console.warn(`[atlas] ${info.id} ${info.name} v${v}: ${over.toFixed(1)} px past its frame`);
      }
      root.addChild(g);
      placements.push({ id: info.id, v, x: at.x, y: at.y, size });
    }
  }
  // Shadow blob: stacked translucent discs approximate a soft falloff.
  const shadowAt = place(128);
  const sg = new Graphics();
  sg.position.set(shadowAt.x + 64, shadowAt.y + 64);
  for (let i = 0; i < 12; i++) sg.circle(0, 0, 62 - i * 4).fill({ color: 0x000000, alpha: 0.09 });
  root.addChild(sg);

  const height = y + rowH;
  const rt = renderer.generateTexture({
    target: root,
    frame: new Rectangle(0, 0, ATLAS_W, height),
    resolution: 1,
    textureSourceOptions: { autoGenerateMipmaps: true, mipLevelCount: MIP_LEVELS },
  });
  root.destroy({ children: true });
  const frames: Texture[][] = [];
  for (const p of placements) {
    (frames[p.id] ??= [])[p.v] = new Texture({ source: rt.source, frame: new Rectangle(p.x, p.y, p.size, p.size) });
  }
  const shadow = new Texture({ source: rt.source, frame: new Rectangle(shadowAt.x, shadowAt.y, 128, 128) });
  return { frames, shadow };
}
