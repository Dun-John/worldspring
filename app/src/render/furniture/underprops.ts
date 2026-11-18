import type { Graphics } from 'pixi.js';
import type { InteriorItem } from '../../gen/protocol';
import { rng } from '../atlas';
import { type Light } from './common';
import { arcaneFocus, arcaneWorkbench, boneMound, climbLadder, crackedFloor, fallenColumn, floodedPit, scaffold, stoneSteps, stoneThrone, totem, wallStub } from './ruins';
import { basaltColumns, beastNest, caveBoulder, caveIn, caveMoss, cavePool, cobweb, crystal, drainLight, fungus, giantMushrooms, glassPool, grille, guano, iceFormation, iceSheet, ladder, obsidianShards, pipe, pit, ratNest, rockColumn, scorchedRemains, stairsUp, stalagmite, steamVent, sulfurCrust, trap, tunnel, web, well } from './under';
import { bedroll, grainSacks, hangingLantern, miningTools, oreCart, oreVein, powderKegs, rails } from './mine';
import { brazier, candelabrum, collapsedWall, dais, dryFountain, ironMaiden, ritualCircle, shackles, stocks, tatteredBanner, torchSconce, treasureHoard } from './dungeon';
import { burialNiche, burialUrn, coldCampfire, flotsamHeap, offeringBowl, openCoffin, rubbleHeap, scatteredBones, skeletonRemains, skullPile, timberProp } from './tomb';
import { linkDown, stoneEffigy } from './sacred';
import { UNDER_DRAWN } from './tables';

/** Draws underground prop `f` if it is drawn here; false otherwise. `wall` is the rock side for props
 * against a wall, and the back for those that have one (the dais); `links` the sides a rail or an ore cart joins
 * the rails beside it (`railRuns`). */
export function drawUnderProp(
  g: Graphics,
  f: InteriorItem,
  light: Light,
  v: number,
  wall: [number, number],
  links = 0,
  turned = false,
  rock = 2,
  run?: { at: [number, number]; span: [number, number] },
): boolean {
  if (!UNDER_DRAWN.has(f.kind)) return false;
  const rnd = rng(v * 7919 + f.w * 131 + f.h * 17 + f.kind.length * 1009 + f.kind.charCodeAt(0) * 3);
  const w: [number, number] = wall[0] || wall[1] ? wall : [0, -1];
  switch (f.kind) {
    case 'exit':
      stairsUp(g, f, rnd, true);
      break;
    case 'up':
      stairsUp(g, f, rnd, false);
      break;
    case 'down':
      linkDown(g, f, rnd);
      break;
    case 'tunnel':
      tunnel(g, f, rnd, w);
      break;
    case 'ladder':
      ladder(g, f, light, rnd, w, turned);
      break;
    case 'pipe':
      pipe(g, f, light, v, rnd, w);
      break;
    case 'well':
      well(g, f, light, rnd);
      break;
    case 'pit':
      pit(g, f, light, rnd);
      break;
    case 'trap':
      trap(g, f, rnd);
      break;
    case 'cave_in':
      caveIn(g, f, rnd);
      break;
    case 'stalagmite':
      stalagmite(g, f, light, rnd);
      break;
    case 'rock_column':
      rockColumn(g, f, light, rnd);
      break;
    case 'boulder':
      caveBoulder(g, f, light, rnd);
      break;
    case 'crystal':
      crystal(g, f, light, v, rnd);
      break;
    case 'fungus':
      fungus(g, f, light, v, rnd);
      break;
    case 'mushroom':
      giantMushrooms(g, f, light, v, rnd);
      break;
    case 'web':
      web(g, f, rnd);
      break;
    case 'cobweb':
      cobweb(g, f, rnd, w);
      break;
    case 'guano':
      guano(g, f, rnd);
      break;
    case 'moss':
      caveMoss(g, f, light, rnd);
      break;
    case 'pool':
      cavePool(g, f, light, v, rnd);
      break;
    case 'nest':
      beastNest(g, f, light, v, rnd);
      break;
    case 'rat_nest':
      ratNest(g, f, light, rnd);
      break;
    case 'ice':
      iceFormation(g, f, light, rnd);
      break;
    case 'ice_sheet':
      iceSheet(g, f, light, rnd);
      break;
    case 'obsidian':
      obsidianShards(g, f, light, rnd);
      break;
    case 'basalt':
      basaltColumns(g, f, light, rnd);
      break;
    case 'vent':
      steamVent(g, f, light, rnd);
      break;
    case 'sulfur':
      sulfurCrust(g, f, light, rnd);
      break;
    case 'scorched':
      scorchedRemains(g, f, light, rnd);
      break;
    case 'glass_pool':
      glassPool(g, f, light, rnd);
      break;
    case 'bones':
      scatteredBones(g, f, light, rnd);
      break;
    case 'skeleton':
      skeletonRemains(g, f, light, rnd);
      break;
    case 'skulls':
      skullPile(g, f, light, rnd);
      break;
    case 'coffin':
      openCoffin(g, f, light, rnd);
      break;
    case 'effigy':
      stoneEffigy(g, f, light, rnd);
      break;
    case 'niche':
      burialNiche(g, f, light, v, rnd, w, rock);
      break;
    case 'urn':
      burialUrn(g, f, light, v, rnd);
      break;
    case 'offering':
      offeringBowl(g, f, light, v, rnd);
      break;
    case 'campfire':
      coldCampfire(g, f, light, rnd);
      break;
    case 'rubble':
      rubbleHeap(g, f, light, rnd);
      break;
    case 'debris':
      flotsamHeap(g, f, light, rnd);
      break;
    case 'timber':
      timberProp(g, f, light, rnd);
      break;
    case 'rail':
      rails(g, f, light, rnd, links);
      break;
    case 'ore_cart':
      if (links) rails(g, f, light, rng(v * 7919 + 77), links);
      oreCart(g, f, light, rnd);
      break;
    case 'ore_vein':
      oreVein(g, f, light, v, rnd, w);
      break;
    case 'tools':
      miningTools(g, f, light, rnd);
      break;
    case 'lantern':
      hangingLantern(g, f, light, rnd);
      break;
    case 'powder':
      powderKegs(g, f, light, rnd);
      break;
    case 'bedroll':
      bedroll(g, f, light, v, rnd);
      break;
    case 'sacks':
      grainSacks(g, f, light, rnd);
      break;
    case 'brazier':
      brazier(g, f, light, rnd);
      break;
    case 'candles':
      candelabrum(g, f, light, rnd);
      break;
    case 'chains':
      shackles(g, f, light, rnd, w);
      break;
    case 'sconce':
      torchSconce(g, f, light, rnd, w);
      break;
    case 'banner':
      tatteredBanner(g, f, light, v, rnd, w);
      break;
    case 'iron_maiden':
      ironMaiden(g, f, light, rnd);
      break;
    case 'stocks':
      stocks(g, f, light, rnd);
      break;
    case 'hoard':
      treasureHoard(g, f, light, rnd);
      break;
    case 'dais':
      dais(g, f, light, rnd, w);
      break;
    case 'collapsed_wall':
      collapsedWall(g, f, light, rnd, w);
      break;
    case 'drain':
      drainLight(g, f, rnd, w);
      break;
    case 'grille':
      grille(g, f, light, rnd, w, run?.at ?? [f.x + 0.5, f.y + 0.5], run?.span ?? [0.6, 0.6]);
      break;
    case 'fountain':
      dryFountain(g, f, light, rnd);
      break;
    case 'glyph':
      ritualCircle(g, f, light, v, rnd);
      break;
    case 'throne':
      stoneThrone(g, f, light, rnd, w);
      break;
    case 'steps':
      stoneSteps(g, f, light, rnd, w);
      break;
    case 'climb_ladder':
      climbLadder(g, f, light, rnd, w);
      break;
    case 'wall_stub':
      wallStub(g, f, light, v, rnd);
      break;
    case 'fallen_column':
      fallenColumn(g, f, light, rnd);
      break;
    case 'arcane_workbench':
      arcaneWorkbench(g, f, light, v, rnd, w);
      break;
    case 'arcane_focus':
      arcaneFocus(g, f, light, rnd);
      break;
    case 'bone_mound':
      boneMound(g, f, light, v, rnd);
      break;
    case 'totem':
      totem(g, f, light, rnd);
      break;
    case 'scaffold':
      scaffold(g, f, light, rnd);
      break;
    case 'cracked_floor':
      crackedFloor(g, f, light, rnd);
      break;
    case 'flooded_pit':
      floodedPit(g, f, light, rnd);
      break;
  }
  return true;
}
