import type { Graphics } from 'pixi.js';
import { PLATFORMS } from '../../gen/protocol';
import type { InteriorItem } from '../../gen/protocol';
import { rng } from '../atlas';
import { ARMS, councilTable, countingDesk, guildArms, jobBoard, stableStall, strongbox } from './guild';
import { type Fit, floorCushion, paintedChest, yurtHearth, yurtPallet } from './yurt';
import { GOD_NAMES, candleStand, carvedScreen, godStatue, screenGate, sideAltar } from './gods';
import { HOUSES, carpetRunner, chairOfOffice, cloisterWell, danceFloor, font, harpsichord, highTable, lowDais, musiciansCorner, refreshmentTable, standingBanner, tapestry, throneDais, wallBanner } from './hall';
import { type Light } from './common';
import { alchemyBench, altar, barricade, bell, cage, cauldron, linkDown, millstones, pillar, sarcophagus, spiralStair, spurWheel, stage, statue, telescope, throne, tortureRack, trapdoor, vat, weaponRack, winch } from './sacred';
import { anvil, bar, barrel, bath, bed, bench, bookcase, boothSeat, boothTable, bucket, chair, chest, cot, couch, counter, crates, desk, display, forge, hearth, keg, oven, pew, planter, roofDome, roundTable, rug, shelf, sideboard, table, wardrobe, washingLine, waterJar, workbench } from './home';
import { gravelWalk } from './under';

/** Furniture with a back (InteriorLayer.backSide): to the wall, a bed's head to the wall at its end, a
 * pew's away from the altar, a booth seat's away from its table. */
export const ORIENTED = new Set([
  'shelf', 'bookcase', 'hearth', 'oven', 'forge', 'workbench', 'counter', 'bar', 'keg_rack', 'bed', 'couch', 'sideboard', 'pew', 'booth_seat', 'altar', 'statue', 'throne', 'alchemy_bench', 'weapon_rack', 'stage',
  'throne_dais', 'chair_of_office', 'high_table', 'tapestry', 'wall_banner', 'musicians_corner', 'low_dais',
  'god_statue', 'carved_screen', 'screen_gate', 'side_altar',
  'job_board', 'guild_arms', 'stable_stall',
  'yurt_hearth', 'yurt_pallet', 'painted_chest', 'floor_cushion',
]);

/** Platforms other pieces stand on, drawn under them. */
export { PLATFORMS };

/** Furniture with other than four looks (`interior::item_looks`): the throne dais's three seatings in stone and
 * gilt, then on timber; a banner's house (`HOUSES`). */
const LOOKS: Record<string, number> = {
  pipe: 3,
  throne_dais: 6,
  wall_banner: HOUSES.length,
  standing_banner: HOUSES.length,
  low_dais: 2,
  // (A temple's pieces: one look per kind of god, `GODS`.)
  god_statue: 8,
  candle_stand: 8,
  carved_screen: 8,
  screen_gate: 8,
  side_altar: 8,
  // A guild's arms, or the town's (`ARMS`).
  guild_arms: ARMS.length,
  // A key's colour (1-7 opens the lock of that colour; `interior::Door::lock`).
  key: 32,
  // A burial niche: its look `v % 4` (body, bones, slab, empty), the rest only varies the details.
  niche: 12,
};

/** How many looks a piece of furniture has (`Item::variant` 0 to this - 1). */
export const looksOf = (kind: string): number => LOOKS[kind] ?? 4;

/** A lock's colour by number (`lockColor`): 0-7 as named, 8-31 a hue round the wheel in a dark, usual or light band. */
export function lockName(v: number): string {
  if (v < 8) return ['No colour', 'Red', 'Orange', 'Yellow', 'Green', 'Blue', 'Purple', 'White'][v];
  const k = v - 8;
  const hue = (k * 47 + 20) % 360;
  const names: [number, string][] = [[15, 'red'], [45, 'orange'], [70, 'yellow'], [95, 'lime'], [160, 'green'], [190, 'teal'], [215, 'sky'], [255, 'blue'], [290, 'violet'], [330, 'magenta'], [361, 'rose']];
  const base = names.find(([h]) => hue < h)![1];
  const band = ['', 'dark ', 'light '][k % 3];
  return `${(band + base).replace(/^./, (c) => c.toUpperCase())} (${v})`;
}

/** What a look of a piece is, for the designer's Look menu: named where they differ by more than colour. */
export function lookName(kind: string, v: number): string {
  if (kind === 'wall_banner' || kind === 'standing_banner') return HOUSES[v % HOUSES.length].name;
  if (kind === 'throne_dais') return `${['One throne', 'Two thrones', 'A throne and a lesser chair'][v % 3]}, ${v < 3 ? 'stone and gilt' : 'timber'}`;
  if (kind === 'tapestry') return ['A stag hunt', 'A unicorn', 'Ships by a castle', 'A rider and a castle'][v % 4];
  if (kind === 'low_dais') return ['Stone', 'Timber'][v % 2];
  if (['god_statue', 'candle_stand', 'carved_screen', 'screen_gate', 'side_altar'].includes(kind)) return GOD_NAMES[v % GOD_NAMES.length];
  if (kind === 'guild_arms') return ARMS[v % ARMS.length].name;
  if (kind === 'key') return lockName(v);
  if (kind === 'strongbox') return ['Black iron', 'Walnut, iron-strapped', 'Iron, brass studs', 'Oak, iron-strapped'][v % 4];
  if (kind === 'counting_desk') return ['Oak, green baize', 'Walnut, green baize', 'Oak, red leather', 'Walnut, red leather'][v % 4];
  if (kind === 'job_board') return ['Notices', 'A wanted bill', 'A painted header', 'A map'][v % 4];
  if (kind === 'stable_stall') return ['Empty', 'A bay horse', 'A grey horse', 'A chestnut horse'][v % 4];
  if (kind === 'council_table') return ['Oval, oak', 'Oak, green baize', 'Oval, walnut', 'Walnut, a blue runner'][v % 4];
  return `Look ${v + 1}`;
}

/** Furniture drawn here: one of its looks each (`looksOf`), picked by position (InteriorLayer.itemKey). */
export const VARIED = new Set([
  'bed', 'cot', 'table', 'long_table', 'chair', 'bench', 'chest', 'wardrobe', 'shelf', 'bookcase',
  'hearth', 'oven', 'forge', 'anvil', 'workbench', 'counter', 'bar', 'barrel', 'keg_rack', 'crate',
  'sideboard', 'desk', 'display', 'couch', 'pew', 'booth_table', 'booth_seat', 'rug', 'bath', 'bucket',
  'altar', 'statue', 'pillar', 'sarcophagus', 'bell', 'throne', 'stage',
  'vat', 'cauldron', 'alchemy_bench', 'weapon_rack', 'rack', 'cage', 'winch', 'telescope', 'barricade',
  'trapdoor', 'spiral_stair', 'link_down',
  'millstones', 'spur_wheel', 'hoist_trapdoor', 'sack_hoist',
  'throne_dais', 'carpet_runner', 'chair_of_office', 'high_table', 'tapestry', 'wall_banner', 'standing_banner', 'font', 'cloister_well',
  'dance_floor', 'harpsichord', 'musicians_corner', 'refreshment_table', 'low_dais', 'garden_path',
  'god_statue', 'candle_stand', 'carved_screen', 'screen_gate', 'side_altar',
  'strongbox', 'counting_desk', 'job_board', 'stable_stall', 'guild_arms', 'council_table',
  'roof_dome', 'water_jar', 'planter', 'washing_line',
  'yurt_hearth', 'yurt_pallet', 'painted_chest', 'floor_cushion',
]);

/** A yurt's pieces, drawn inside its ring (`Fit`) when they stand in one. */
export const IN_RING = new Set(['yurt_pallet', 'painted_chest', 'floor_cushion']);

/** Draws `f` if it is building furniture drawn here; false otherwise. `fit`: the yurt's ring it stands in. */
export function drawFurniture(g: Graphics, f: InteriorItem, light: Light, v: number, back: [number, number] | null = null, fit?: Fit): boolean {
  if (!VARIED.has(f.kind)) return false;
  const rnd = rng(v * 7919 + f.w * 131 + f.h * 17 + f.kind.length * 1009 + f.kind.charCodeAt(0));
  switch (f.kind) {
    case 'bed':
      bed(g, f, light, v, rnd, back ?? (f.w > f.h ? [-1, 0] : [0, -1]));
      break;
    case 'couch':
      couch(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'sideboard':
      sideboard(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'desk':
      desk(g, f, light, rnd);
      break;
    case 'display':
      display(g, f, light, v, rnd);
      break;
    case 'pew':
      pew(g, f, light, rnd, back ?? (f.w >= f.h ? [0, 1] : [1, 0]));
      break;
    case 'booth_table':
      boothTable(g, f, light, rnd);
      break;
    case 'booth_seat':
      boothSeat(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'rug':
      rug(g, f, v, rnd);
      break;
    case 'bath':
      bath(g, f, light, rnd);
      break;
    case 'bucket':
      bucket(g, f, light, rnd);
      break;
    case 'altar':
      altar(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'statue':
      statue(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'pillar':
      pillar(g, f, light, rnd);
      break;
    case 'sarcophagus':
      sarcophagus(g, f, light, rnd);
      break;
    case 'bell':
      bell(g, f, light, rnd);
      break;
    case 'throne':
      throne(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'stage':
      stage(g, f, light, rnd, back ?? (f.w >= f.h ? [0, -1] : [-1, 0]));
      break;
    case 'vat':
      vat(g, f, light, rnd);
      break;
    case 'cauldron':
      cauldron(g, f, light, v, rnd);
      break;
    case 'alchemy_bench':
      alchemyBench(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'weapon_rack':
      weaponRack(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'rack':
      tortureRack(g, f, light, rnd);
      break;
    case 'cage':
      cage(g, f, rnd);
      break;
    case 'winch':
    case 'sack_hoist':
      winch(g, f, light, rnd);
      break;
    case 'millstones':
      millstones(g, f, light, rnd);
      break;
    case 'spur_wheel':
      spurWheel(g, f, light, rnd);
      break;
    case 'telescope':
      telescope(g, f, light, rnd);
      break;
    case 'barricade':
      barricade(g, f);
      break;
    case 'trapdoor':
    case 'hoist_trapdoor':
      trapdoor(g, f, light, rnd);
      break;
    case 'spiral_stair':
      spiralStair(g, f, rnd);
      break;
    case 'link_down':
      linkDown(g, f, rnd);
      break;
    case 'cot':
      cot(g, f, rnd);
      break;
    case 'table':
      if (f.w === 1 && f.h === 1) roundTable(g, f, light, rnd);
      else table(g, f, light, rnd, false);
      break;
    case 'long_table':
      table(g, f, light, rnd, true);
      break;
    case 'chair':
      chair(g, f, light, rnd);
      break;
    case 'bench':
      bench(g, f, light, rnd);
      break;
    case 'roof_dome':
      roofDome(g, f, light, rnd);
      break;
    case 'water_jar':
      waterJar(g, f, light, v, rnd);
      break;
    case 'planter':
      planter(g, f, light, v, rnd);
      break;
    case 'washing_line':
      washingLine(g, f, light, v, rnd);
      break;
    case 'chest':
      chest(g, f, light, v, rnd);
      break;
    case 'wardrobe':
      wardrobe(g, f, light, rnd);
      break;
    case 'shelf':
      shelf(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'bookcase':
      bookcase(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'hearth':
      hearth(g, f, rnd, back ?? [0, -1]);
      break;
    case 'yurt_hearth':
      yurtHearth(g, f, light, v, rnd, back ?? [0, 1]);
      break;
    case 'yurt_pallet':
      yurtPallet(g, f, light, v, rnd, back ?? (f.w >= f.h ? [0, -1] : [-1, 0]), fit);
      break;
    case 'painted_chest':
      paintedChest(g, f, light, v, rnd, back ?? [0, -1], fit);
      break;
    case 'floor_cushion':
      floorCushion(g, f, light, v, rnd, back ?? [0, -1], fit);
      break;
    case 'oven':
      oven(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'forge':
      forge(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'anvil':
      anvil(g, f, light, rnd);
      break;
    case 'workbench':
      workbench(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'counter':
      counter(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'bar':
      bar(g, f, light, rnd, back ?? [0, -1]);
      break;
    case 'barrel':
      barrel(g, f.x + f.w / 2, f.y + f.h / 2, Math.min(f.w, f.h) * 0.4, light, rnd);
      break;
    case 'keg_rack':
      for (let u = 0; u < Math.max(f.w, f.h); u++) keg(g, f, u, light, rnd, back ?? [0, -1]);
      break;
    case 'crate':
      crates(g, f, light, v, rnd);
      break;
    case 'throne_dais':
      throneDais(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'low_dais':
      lowDais(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'carpet_runner':
      carpetRunner(g, f, v, rnd);
      break;
    case 'chair_of_office':
      chairOfOffice(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'high_table':
      highTable(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'tapestry':
      tapestry(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'wall_banner':
      wallBanner(g, f, light, v, back ?? [0, -1]);
      break;
    case 'standing_banner':
      standingBanner(g, f, light, v, rnd);
      break;
    case 'font':
      font(g, f, light, v, rnd);
      break;
    case 'cloister_well':
      cloisterWell(g, f, light, v, rnd);
      break;
    case 'dance_floor':
      danceFloor(g, f, v, rnd);
      break;
    case 'harpsichord':
      harpsichord(g, f, light, v, rnd);
      break;
    case 'musicians_corner':
      musiciansCorner(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'refreshment_table':
      refreshmentTable(g, f, light, v, rnd);
      break;
    case 'garden_path':
      gravelWalk(g, f, light, rnd);
      break;
    case 'god_statue':
      godStatue(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'candle_stand':
      candleStand(g, f, light, v, rnd);
      break;
    case 'carved_screen':
      carvedScreen(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'screen_gate':
      screenGate(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'side_altar':
      sideAltar(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'strongbox':
      strongbox(g, f, light, v, rnd);
      break;
    case 'counting_desk':
      countingDesk(g, f, light, v, rnd);
      break;
    case 'job_board':
      jobBoard(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'stable_stall':
      stableStall(g, f, light, v, rnd, back ?? (f.w > f.h ? [-1, 0] : [0, -1]));
      break;
    case 'guild_arms':
      guildArms(g, f, light, v, rnd, back ?? [0, -1]);
      break;
    case 'council_table':
      councilTable(g, f, light, v, rnd);
      break;
  }
  return true;
}
