// Curtain walls, wall towers and gatehouses on the battlemap, drawn in code like the atlas and the
// bridges (assets/STYLE_GUIDE.md): warm stone in a few flat tones lit from the north-west, bold
// ink, blocks a little out of true. A wall is a flagged walk between two crenellated parapets;
// a tower a flagged fighting top inside its merlons with the spiral stair's hatch; a gate the
// passage between its two towers under an arch at each end, portcullis grooves and the doors
// swung back. A ruin's works lose merlons, crack and moss over, spill rubble where they broke.
// A palace's forecourt has a low coped garden wall instead, its gate between two solid piers with
// ball finials, the leaves (wrought iron or timber) standing open; its court's walks are gravel.
// Town walls take their culture group's look (item 104, `lookOf`): the stone's tones (`TONES`), the merlons'
// profile and rhythm, the towers' shape (round, squat, D, octagon, square, a log blockhouse, a tapering earth
// bastion), the wall's material (stone, a palisade of stakes, rammed earth) and the gatehouse's form.
export { WALK, PARAPET, styleOf, towerShape, GATE_SQUARE, GATE_ARCHED, GATE_BENT, GATE_LOG, gateForm, type Look, MAT_STONE, MAT_PALISADE, MAT_EARTH, M_BROAD, M_SQUARE, M_SWALLOWTAIL, M_STEPPED, M_STAKES, M_ROUNDED, lookOf, TONES } from './walls/base';
export { flagFloor, parapetRing, stairHatch, cornerHatch, roundTop, roundParapet, roundWalk } from './walls/tops';
export { banner } from './walls/flags';
export { drawWalls } from './walls/curtain';
export { bastionRamp, drawBastion, drawWallTower } from './walls/towers';
export { stockadeStakes, GATE_DEPTH, drawGate } from './walls/gates';
export { drawGardenWalls, drawGatePier, drawForecourtGate, fenceGate } from './walls/garden';
export { drawDrawbridge, drawDrawbridgeSill, drawRaisedDrawbridge } from './walls/drawbridge';
export { drawDitchBank, drawGravelWalk, topGrid } from './walls/extras';
