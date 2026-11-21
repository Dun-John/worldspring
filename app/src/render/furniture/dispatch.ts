import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { rng } from '../atlas';
import { councilTable, countingDesk, guildArms, jobBoard, stableStall, strongbox } from './guild';
import { type Fit, floorCushion, paintedChest, yurtHearth, yurtPallet } from './yurt';
import { candleStand, carvedScreen, godStatue, screenGate, sideAltar } from './gods';
import { carpetRunner, chairOfOffice, cloisterWell, danceFloor, font, harpsichord, highTable, lowDais, musiciansCorner, refreshmentTable, standingBanner, tapestry, throneDais, wallBanner } from './hall';
import { type Light } from './common';
import { alchemyBench, altar, barricade, bell, cage, cauldron, lighthouseLamp, linkDown, millstones, pillar, sarcophagus, spiralStair, spurWheel, stage, statue, telescope, throne, tortureRack, trapdoor, vat, weaponRack, winch } from './sacred';
import { anvil, bar, barrel, bath, bed, bench, bookcase, boothSeat, boothTable, bucket, chair, chest, cot, couch, counter, crates, desk, display, forge, hearth, keg, oven, pew, planter, roofDome, roundTable, rug, shelf, sideboard, table, wardrobe, washingLine, waterJar, workbench } from './home';
import { gravelWalk } from './under';
import { VARIED } from './tables';
import { barrels, cart } from '../atlas/yard';
import { waterTrough } from '../atlas/street';
import { PX } from '../scale';

/** A coaching inn's yard's things (`hub::YARD_LOOK`): the battlemap's own pictures (its kind's id and
 * radius, squares, as `atlas/build` draws them), centred on the item along its longer side (a square
 * one along x), as the battlemap turns them in the yard; so the yard reads the same inside and out. */
function yardThing(g: Graphics, f: InteriorItem, v: number) {
  const [id, r, draw] = f.kind === 'cart' ? [35, 1.3, cart] : f.kind === 'barrels' ? [33, 0.5, barrels] : [46, 0.75, waterTrough];
  const k = 1 / PX;
  const [c, s] = f.w >= f.h ? [k, 0] : [0, k];
  const turn = s ? Math.PI / 2 : 0;
  // (The transform moves the points but not a stroke's width, nor an arc's radius and angles: those by hand.)
  const thin: Graphics = new Proxy(g, {
    get(target, prop, receiver) {
      const fn = Reflect.get(target, prop, receiver);
      if (typeof fn !== 'function') return fn;
      return (...args: unknown[]) => {
        const st = args[0] as { width?: number } | undefined;
        const n = args as number[];
        const a =
          prop === 'stroke' && st && typeof st === 'object'
            ? [{ ...st, width: (st.width ?? 1) * k }, ...args.slice(1)]
            : prop === 'arc'
              ? [n[0], n[1], n[2] * k, n[3] + turn, n[4] + turn, ...args.slice(5)]
              : args;
        const out = (fn as (...x: unknown[]) => unknown).apply(target, a);
        return out === target ? thin : out;
      };
    },
  });
  g.save().setTransform(c, s, -s, c, f.x + f.w / 2, f.y + f.h / 2);
  draw(thin, r * PX, rng(id * 131 + v * 17 + 7), v);
  g.restore();
}

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
    case 'lighthouse_lamp':
      lighthouseLamp(g, f, light, rnd);
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
    case 'water_trough':
    case 'barrels':
    case 'cart':
      yardThing(g, f, v);
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
