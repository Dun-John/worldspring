// Every keyboard shortcut of the map's window, built from what App knows and can do. Letters
// belong to the panel that is open (play's tools while a session runs with no other panel), so
// none is live twice. `?ui=new`: each Edit tool's key opens it from any Edit tool that doesn't
// use that letter itself (the openers, last, live only while no other binding has the key).
import type { EditTool } from '../../editor/sketcher';
import type { ScatterMode } from '../../editor/scatter';
import type { BuildShape } from '../../editor/build';
import type { Placer } from '../../editor/placing';
import type { DesignMode } from '../../editor/site/designer';
import type { TownMode } from '../../editor/town';
import type { GroundMode } from '../../editor/ground';
import type { Tool as PlayTool } from '../../play/controller';
import { NEW_UI, type EditTab, type Section } from './layout.svelte';
import { TOOL, type ToolKey } from './editTools';
import type { Shortcut } from './shortcuts';

export interface KeyContext {
  section(): Section | null;
  editTab(): EditTab;
  playOn(): boolean;
  sketching(): boolean;
  designing(): boolean;
  inside(): boolean;
  building(): boolean;
  /** Tokens are selected in play. */
  tokensSelected(): boolean;
  toggle(s: Section): void;
  editTabTo(t: EditTab): void;
  focusSearch(): void;
  help(): void;
  stats(): void;
  wholeMap(): void;
  grid(): void;
  places(): void;
  seaLanes(): void;
  hexes(): void;
  zoom(dz: number): void;
  pan(dx: number, dy: number): void;
  level(d: number): void;
  /** Inside: the view turned north up, or back. */
  northUp(): void;
  undo(): void;
  redo(): void;
  generate(): void;
  saveDesign(): void;
  sketchTool(t: EditTool): void;
  scatterMode(m: ScatterMode): void;
  /** What places things by hand while its panel is open (Objects, or Rooms on Props), or none. */
  placing(): Placer | null;
  buildShape(s: BuildShape): void;
  /** Crossings (again: the next kind). */
  buildCross(): void;
  /** Islets, quays, piers and breakwaters. */
  buildHarbour(): void;
  /** Build › Road (again: the next type). */
  buildRoad(): void;
  /** Build › Fence (again: the next kind). */
  buildFence(): void;
  /** Castles or walls. */
  buildWorks(m: 'castle' | 'wall'): void;
  /** Stamp, row or plot. */
  buildQuick(m: 'stamp' | 'row' | 'lasso'): void;
  buildFinish(): void;
  buildBack(): void;
  /** Build › Select. */
  buildSelect(): void;
  /** `?ui=new`: the Edit tool open, else null. */
  editTool(): ToolKey | null;
  /** `?ui=new`: open an Edit tool (false: it didn't open, or a question was asked first). */
  openTool(t: ToolKey): boolean;
  /** Select: the thing open taken away (false: none that can be). */
  selectDelete(): boolean;
  designMode(m: DesignMode): void;
  /** Rooms: the ways across (paired exits) or back to the rooms (false: not here). */
  linksToggle(): boolean;
  /** The ways across are open in Rooms. */
  linking(): boolean;
  /** The way across chosen taken away. */
  linkRemove(): void;
  townMode(m: TownMode): void;
  /** The town brush a step bigger (1) or smaller (-1). */
  townBrush(d: number): void;
  /** The town in view is laid out by hand (its street graph is shown) and its trace underlay isn't being adjusted. */
  townHand(): boolean;
  /** The street, area or wall being drawn ended (false: none). */
  townStreetFinish(): boolean;
  /** Its last point taken back (false: none drawn). */
  townStreetBack(): boolean;
  /** The street, area or wall picked taken away (false: none). */
  townStreetDelete(): boolean;
  groundMode(m: GroundMode): void;
  /** The ground brush a step wider (1) or narrower (-1). */
  groundBrush(d: 1 | -1): void;
  /** The chosen plateau taken away (false: none chosen). */
  groundRemove(): boolean;
  playTool(t: PlayTool): void;
  removeTokens(): void;
  /** N: a new token at the pointer; Ctrl+D, Ctrl+C, Ctrl+V: duplicate, copy, paste tokens. */
  newToken(): void;
  duplicateTokens(): void;
  copyTokens(): void;
  pasteTokens(): void;
  /** Steps Escape tries in order; the first that does something wins. */
  escape: (() => boolean)[];
}

const STEP = 120;

/** A shortcuts group's name, the old layout's or `?ui=new`'s. */
const grp = (old: string, neu: string) => (NEW_UI ? neu : old);

export function buildKeymap(k: KeyContext): Shortcut[] {
  const edit = (tab?: EditTab) => () => k.section() === 'edit' && (!tab || k.editTab() === tab);
  const sketching = () => k.section() === 'world' && k.sketching();
  const playing = () => k.playOn() && (k.section() === 'play' || k.section() === null);
  const out: Shortcut[] = [
    { keys: ['1'], label: 'World', group: 'Panels', run: () => k.toggle('world') },
    { keys: ['2'], label: 'Edit', group: 'Panels', run: () => k.toggle('edit') },
    { keys: ['3'], label: 'Notes', group: 'Panels', run: () => k.toggle('notes') },
    { keys: ['4'], label: 'Play', group: 'Panels', run: () => k.toggle('play') },
    { keys: ['/', 'Ctrl+K'], label: 'Search', group: 'Panels', run: () => k.focusSearch() },
    { keys: ['Ctrl+K'], label: 'Search', group: 'Panels', inInputs: true, hidden: true, when: () => true, run: () => k.focusSearch() },
    { keys: ['?'], label: 'Keyboard shortcuts', group: 'Panels', run: () => k.help() },
    { keys: ['`'], label: 'Performance stats', group: 'Panels', run: () => k.stats() },
    {
      keys: ['Escape'],
      label: 'Back: close menus, cancel, leave the building',
      group: 'Panels',
      inInputs: true,
      run: () => k.escape.some((step) => step()),
    },
    { keys: ['Ctrl+Z'], label: 'Undo', group: 'Map', run: () => k.undo() },
    { keys: ['Ctrl+Shift+Z', 'Ctrl+Y'], label: 'Redo', group: 'Map', run: () => k.redo() },
    { keys: ['+', '='], label: 'Zoom in', group: 'Map', repeat: true, run: () => k.zoom(0.006) },
    { keys: ['-', '_'], label: 'Zoom out', group: 'Map', repeat: true, run: () => k.zoom(-0.006) },
    {
      keys: ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'],
      label: 'Pan',
      group: 'Map',
      repeat: true,
      run: (e) => k.pan(e.key === 'ArrowLeft' ? STEP : e.key === 'ArrowRight' ? -STEP : 0, e.key === 'ArrowUp' ? STEP : e.key === 'ArrowDown' ? -STEP : 0),
    },
    { keys: ['Home'], label: 'Whole map', group: 'Map', run: () => k.wholeMap() },
    { keys: ['G'], label: 'Battlemap grid', group: 'Map', run: () => k.grid() },
    { keys: ['P'], label: 'Place names (inns, shops, temples)', group: 'Map', run: () => k.places() },
    { keys: ['Shift+L'], label: 'Sea lanes', group: 'Map', run: () => k.seaLanes() },
    { keys: ['Shift+H'], label: 'Hex grid', group: 'Map', run: () => k.hexes() },
    { keys: [']', 'PageUp'], label: 'Floor up', group: 'Map', when: k.inside, run: () => k.level(1) },
    { keys: ['[', 'PageDown'], label: 'Floor down', group: 'Map', when: k.inside, run: () => k.level(-1) },
    { keys: ['Shift+N'], label: 'Turn the plan north up (again: back)', group: 'Map', when: k.inside, run: () => k.northUp() },
    { keys: ['Ctrl+Enter'], label: 'Generate', group: 'World', inInputs: true, when: () => k.section() === 'world', run: () => k.generate() },
  ];
  const sketch: [string, EditTool, string][] = [
    ['C', 'coast', 'Coastline'],
    ['L', 'land', 'Land'],
    ['S', 'sea', 'Sea'],
    ['R', 'range', 'Range'],
    ['M', 'massif', 'Massif'],
    ['H', 'elevation', 'Elevation'],
    ['W', 'river', 'River'],
    ['K', 'lake', 'Lake'],
    ['B', 'biome', 'Biome'],
    ['V', 'volcano', 'Volcano'],
    ['T', 'pin', 'Settlement'],
    ['D', 'site', 'Planned site'],
    ['N', 'region', 'Name a place'],
    ['O', 'road', 'Road'],
    ['E', 'erase', 'Erase'],
  ];
  for (const [key, t, label] of sketch) out.push({ keys: [key], label, group: 'Sketch', when: sketching, run: () => k.sketchTool(t) });
  const tabs: [string, EditTab, string][] = [
    ['S', 'sites', 'Places'],
    ['B', 'build', 'Build'],
    ['U', 'town', 'Streets'],
    ['M', 'ground', 'Ground'],
    ['C', 'scatter', 'Objects'],
    ['D', 'design', 'Rooms'],
  ];
  if (!NEW_UI) for (const [key, t, label] of tabs) out.push({ keys: [key], label, group: 'Edit', when: edit(), run: () => k.editTabTo(t) });
  const shapes: [string, BuildShape, string][] = [
    ['R', 'rect', 'Rectangle'],
    ['O', 'poly', 'Polygon'],
    ['T', 'tower', 'Round tower'],
  ];
  for (const [key, s, label] of shapes) out.push({ keys: [key], label, group: grp('Build', 'Draw a building'), when: edit('build'), run: () => k.buildShape(s) });
  if (!NEW_UI) out.push({ keys: ['A'], label: 'Select (click, drag a box, Shift-drag a lasso)', group: 'Build', when: edit('build'), run: () => k.buildSelect() });
  out.push({ keys: ['Delete'], label: 'Delete what Select has open', group: grp('Build', 'Select'), when: edit('build'), run: () => k.selectDelete() });
  if (!NEW_UI) buildModes(k, out, edit('build'));
  out.push(
    { keys: ['Enter'], label: 'Finish the polygon, castle, wall, fence, row or plot', group: grp('Build', 'Drawing'), when: () => edit('build')() && k.building(), run: () => k.buildFinish() },
    { keys: ['Backspace'], label: 'Take a corner back', group: grp('Build', 'Drawing'), when: () => edit('build')() && k.building(), run: () => k.buildBack() },
  );
  const town: [string, TownMode, string][] = [
    ['V', 'select', 'Choose a patch, drag a corner'],
    ['Q', 'displace', grp('Displace', 'Push (Displace)')],
    ['L', 'liquify', grp('Liquify', 'Smudge (Liquify)')],
    ['O', 'bloat', grp('Bloat', 'Grow (Bloat)')],
    ['I', 'pinch', grp('Pinch', 'Shrink (Pinch)')],
    ['R', 'relax', grp('Relax', 'Soften (Relax)')],
    ['E', 'equalize', grp('Equalize', 'Even out (Equalize)')],
  ];
  for (const [key, m, label] of town) out.push({ keys: [key], label, group: 'Streets', when: edit('town'), run: () => k.townMode(m) });
  const hand = () => edit('town')() && k.townHand();
  out.push(
    { keys: ['N'], label: 'Draw a street (laid out by hand)', group: 'Streets', when: hand, run: () => k.townMode('street') },
    { keys: ['A'], label: 'Paint an area open or paved (laid out by hand)', group: 'Streets', when: hand, run: () => k.townMode('area') },
    { keys: ['W'], label: 'Draw the town wall (laid out by hand)', group: 'Streets', when: hand, run: () => k.townMode('wall') },
    { keys: ['Enter'], label: 'End the street, area or wall being drawn', group: 'Streets', when: hand, run: () => k.townStreetFinish() },
    { keys: ['Backspace'], label: 'Take a point back, else take the street, area or wall chosen away', group: 'Streets', when: hand, run: () => k.townStreetBack() || k.townStreetDelete() },
    { keys: ['Delete'], label: 'Take the street, area or wall chosen away', group: 'Streets', when: hand, run: () => k.townStreetDelete() },
  );
  out.push(
    { keys: [']'], label: 'Bigger brush', group: 'Streets', repeat: true, when: () => edit('town')() && !k.inside(), run: () => k.townBrush(1) },
    { keys: ['['], label: 'Smaller brush', group: 'Streets', repeat: true, when: () => edit('town')() && !k.inside(), run: () => k.townBrush(-1) },
  );
  const ground: [string, GroundMode, string][] = [
    ['Q', 'raise', 'Raise'],
    ['W', 'lower', 'Lower'],
    ['E', 'smooth', 'Smooth'],
    ['R', 'restore', grp('Restore', 'Put back (Restore)')],
    ['L', 'plateau', 'Plateau (drag a loop)'],
    ['V', 'select', 'Choose a plateau'],
  ];
  for (const [key, m, label] of ground) out.push({ keys: [key], label, group: 'Ground', when: edit('ground'), run: () => k.groundMode(m) });
  out.push(
    { keys: [']'], label: 'Wider brush', group: 'Ground', repeat: true, when: () => edit('ground')() && !k.inside(), run: () => k.groundBrush(1) },
    { keys: ['['], label: 'Narrower brush', group: 'Ground', repeat: true, when: () => edit('ground')() && !k.inside(), run: () => k.groundBrush(-1) },
    { keys: ['Delete', 'Backspace'], label: 'Take away the chosen plateau', group: 'Ground', when: edit('ground'), run: () => k.groundRemove() },
  );
  const modes: [string, ScatterMode, string][] = [
    ['Q', 'place', 'Place'],
    ['W', 'brush', 'Brush'],
    ['E', 'erase', 'Erase'],
  ];
  for (const [key, m, label] of modes) out.push({ keys: [key], label, group: grp('Objects', 'Trees & props'), when: edit('scatter'), run: () => k.scatterMode(m) });
  out.push(
    { keys: [']'], label: 'Bigger (brush: wider)', group: grp('Objects', 'Trees & props'), repeat: true, when: () => edit('scatter')() && !k.inside(), run: () => k.placing()?.size(1) },
    { keys: ['['], label: 'Smaller (brush: narrower)', group: grp('Objects', 'Trees & props'), repeat: true, when: () => edit('scatter')() && !k.inside(), run: () => k.placing()?.size(-1) },
  );
  // Placing: on what the hand shows (the piece held, the one a click would pick up, or the
  // chosen item's ghost); Delete on the piece held, else the one pointed at (also with an item
  // chosen). A key with nothing to act on goes on to the next binding.
  const placing = () => !!k.placing();
  const on = (f: (p: Placer) => boolean) => () => {
    const p = k.placing();
    return !!p && f(p);
  };
  out.push(
    { keys: ['T'], label: 'Turn clockwise (inside: a quarter)', group: 'Placing', repeat: true, when: placing, run: on((p) => p.turn(1, false)) },
    { keys: ['Shift+T'], label: 'Turn anticlockwise', group: 'Placing', repeat: true, when: placing, run: on((p) => p.turn(-1, false)) },
    { keys: ['.'], label: 'Turn a little clockwise (Shift+wheel too)', group: 'Placing', repeat: true, when: placing, run: on((p) => p.turn(1, true)) },
    { keys: [','], label: 'Turn a little anticlockwise', group: 'Placing', repeat: true, when: placing, run: on((p) => p.turn(-1, true)) },
    { keys: ['Z'], label: 'Turn by itself again (at random; inside, as it stands)', group: 'Placing', when: placing, run: on((p) => p.turnReset()) },
    { keys: ['Enter'], label: 'Put the piece held down', group: 'Placing', when: placing, run: on((p) => p.putDown()) },
    { keys: ['Delete', 'Backspace'], label: 'Take away the piece held or pointed at', group: 'Placing', when: placing, run: on((p) => p.remove()) },
  );
  const design: [string | string[], DesignMode, string][] = [
    ['V', 'select', 'Choose a room'],
    ['R', 'room', 'Room'],
    ['X', 'rect', 'Rectangle'],
    ['H', 'corridor', 'Corridor'],
    ['K', 'rock', 'Rock'],
    ['O', 'door', 'Door (pick a lock colour to lock it)'],
    ['A', 'terrain', 'Terrain: water, chasm, bridge, stones, rubble (underground)'],
    [['F', 'Q'], 'prop', 'Props (in a building: furniture)'],
    ['W', 'stairs', 'Way down (in a building: the stairs)'],
    ['L', 'wall', 'Wall line (in a building)'],
    ['E', 'merge', 'Take a wall away (in a building)'],
    ['Y', 'landing', 'Where the stairs come out on this level'],
  ];
  const designing = () => edit('design')() && k.designing();
  for (const [key, m, label] of design) out.push({ keys: typeof key === 'string' ? [key] : key, label, group: 'Rooms', when: designing, run: () => k.designMode(m) });
  if (!NEW_UI) out.push({ keys: ['J'], label: 'Ways across between levels and places below ground (again: back to the rooms)', group: 'Rooms', when: edit('design'), run: () => void k.linksToggle() });
  out.push({ keys: ['Delete', 'Backspace'], label: grp('Take the way across chosen away', 'Take the stair or passage chosen away'), group: grp('Rooms', 'Stairs & passages'), when: () => edit('design')() && k.linking(), run: () => k.linkRemove() });
  out.push({ keys: ['Ctrl+S'], label: 'Save the design', group: 'Rooms', inInputs: true, when: designing, run: () => k.saveDesign() });
  const tools: [string, PlayTool, string][] = [
    ['V', 'select', 'Select and move'],
    ['T', 'token', 'Tokens'],
    ['M', 'measure', 'Measure'],
    ['S', 'shape', 'Shapes'],
    ['F', 'fog', 'Fog of war'],
    ['H', 'hex', 'Explore hexes'],
  ];
  for (const [key, t, label] of tools) out.push({ keys: [key], label, group: 'Play', when: playing, run: () => k.playTool(t) });
  out.push({ keys: ['Delete', 'Backspace'], label: 'Remove the selected tokens', group: 'Play', when: () => playing() && k.tokensSelected(), run: () => k.removeTokens() });
  out.push({ keys: ['N'], label: 'New token at the pointer', group: 'Play', when: playing, run: () => k.newToken() });
  out.push({ keys: ['Ctrl+D'], label: 'Duplicate the selected tokens', group: 'Play', when: () => playing() && k.tokensSelected(), run: () => k.duplicateTokens() });
  out.push({ keys: ['Ctrl+C'], label: 'Copy the selected tokens', group: 'Play', when: () => playing() && k.tokensSelected(), run: () => k.copyTokens() });
  out.push({ keys: ['Ctrl+V'], label: 'Paste tokens at the pointer', group: 'Play', when: playing, run: () => k.pasteTokens() });
  // (The openers read the other bindings, not each other.)
  if (NEW_UI) out.push(...openers(k, [...out], edit()));
  return out;
}

/** The old layout's Build modes, live with Build open. */
function buildModes(k: KeyContext, out: Shortcut[], when: () => boolean) {
  out.push({ keys: ['X'], label: 'Crossing (again: bridge, ford, ferry)', group: 'Build', when, run: () => k.buildCross() });
  out.push({ keys: ['H'], label: 'Harbour (islet, quay, pier, breakwater)', group: 'Build', when, run: () => k.buildHarbour() });
  out.push({ keys: ['V'], label: 'Road (again: footpath, track, gravel, paved, cobbled)', group: 'Build', when, run: () => k.buildRoad() });
  out.push({ keys: ['F'], label: 'Fence (draw, gate, erase; again: the next kind)', group: 'Build', when, run: () => k.buildFence() });
  out.push({ keys: ['Q'], label: 'Stamp (a building of a kind and size)', group: 'Build', when, run: () => k.buildQuick('stamp') });
  out.push({ keys: ['E'], label: 'Row of buildings along a street', group: 'Build', when, run: () => k.buildQuick('row') });
  out.push({ keys: ['L'], label: 'Plot (draw round it, fill it)', group: 'Build', when, run: () => k.buildQuick('lasso') });
  out.push({ keys: ['K'], label: 'Castle', group: 'Build', when, run: () => k.buildWorks('castle') });
  out.push({ keys: ['W'], label: 'Wall', group: 'Build', when, run: () => k.buildWorks('wall') });
}

/** `?ui=new`: each Edit tool's key opens it, live in Edit while no other binding has the key
 * (Streets, Ground, Trees & props and the designer keep their letters). Again on the tool open:
 * the next kind (Road, Crossing, Fences), or back (Stairs & passages to the rooms). */
function openers(k: KeyContext, owners: Shortcut[], inEdit: () => boolean): Shortcut[] {
  const free = (key: string) => () => inEdit() && !owners.some((s) => s.keys.includes(key) && (!s.when || s.when()));
  const open = (t: ToolKey, again?: () => void) => () => {
    if (again && k.editTool() === t) again();
    else k.openTool(t);
  };
  const list: [string, ToolKey, (() => void) | undefined, string?][] = [
    ['A', 'select', undefined, 'Select (click, drag a box, Shift-drag a lasso)'],
    ['S', 'places', undefined],
    ['U', 'streets', undefined],
    ['Shift+U', 'townwall', undefined],
    ['Shift+J', 'members', undefined],
    ['B', 'draw', undefined],
    ['Q', 'single', undefined],
    ['E', 'row', undefined],
    ['L', 'fill', undefined],
    ['V', 'road', () => k.buildRoad(), 'Road (again: footpath, track, gravel, paved, cobbled)'],
    ['X', 'crossing', () => k.buildCross(), 'Crossing (again: bridge, ford, ferry)'],
    ['I', 'water', undefined],
    ['H', 'docks', undefined],
    ['M', 'ground', undefined],
    ['F', 'fences', () => k.buildFence(), 'Fences & hedges (draw, gate, erase; again: the next kind)'],
    ['C', 'trees', undefined],
    ['D', 'rooms', undefined],
    ['J', 'stairs', () => void k.linksToggle(), 'Stairs & passages (again: back to the rooms)'],
  ];
  const out: Shortcut[] = list.map(([key, t, again, label]) => ({ keys: [key], label: label ?? TOOL[t].label, group: 'Edit tools', when: free(key), run: open(t, again) }));
  // Castle & walls: K a castle, W a wall on its own.
  for (const [key, m, label] of [['K', 'castle', 'Castle'], ['W', 'wall', 'Wall on its own']] as const) {
    out.push({ keys: [key], label: `Castle & walls: ${label.toLowerCase()}`, group: 'Edit tools', when: free(key), run: () => void (k.openTool('works') && k.buildWorks(m)) });
  }
  return out;
}
