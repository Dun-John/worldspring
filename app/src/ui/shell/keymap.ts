// Every keyboard shortcut of the map's window, built from what App knows and can do. Letters
// belong to the panel that is open (play's tools while a session runs with no other panel), so
// none is live twice.
import type { EditTool } from '../../editor/sketcher';
import type { ScatterMode } from '../../editor/scatter';
import type { BuildShape } from '../../editor/build';
import type { DesignMode } from '../../editor/site/designer';
import type { TownMode } from '../../editor/town';
import type { Tool as PlayTool } from '../../play/controller';
import type { EditTab, Section } from './layout.svelte';
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
  zoom(dz: number): void;
  pan(dx: number, dy: number): void;
  level(d: number): void;
  undo(): void;
  redo(): void;
  generate(): void;
  saveDesign(): void;
  sketchTool(t: EditTool): void;
  scatterMode(m: ScatterMode): void;
  buildShape(s: BuildShape): void;
  /** Crossings (again: the next kind). */
  buildCross(): void;
  /** Castles or walls. */
  buildWorks(m: 'castle' | 'wall'): void;
  buildFinish(): void;
  buildBack(): void;
  designMode(m: DesignMode): void;
  townMode(m: TownMode): void;
  /** The town brush a step bigger (1) or smaller (-1). */
  townBrush(d: number): void;
  playTool(t: PlayTool): void;
  removeTokens(): void;
  /** Steps Escape tries in order; the first that does something wins. */
  escape: (() => boolean)[];
}

const STEP = 120;

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
    { keys: [']', 'PageUp'], label: 'Floor up', group: 'Map', when: k.inside, run: () => k.level(1) },
    { keys: ['[', 'PageDown'], label: 'Floor down', group: 'Map', when: k.inside, run: () => k.level(-1) },
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
    ['D', 'site', 'Site'],
    ['N', 'region', 'Name a place'],
    ['O', 'road', 'Road'],
    ['E', 'erase', 'Erase'],
  ];
  for (const [key, t, label] of sketch) out.push({ keys: [key], label, group: 'Sketch', when: sketching, run: () => k.sketchTool(t) });
  const tabs: [string, EditTab, string][] = [
    ['N', 'names', 'Names'],
    ['S', 'sites', 'Sites'],
    ['B', 'build', 'Build'],
    ['U', 'town', 'Town'],
    ['C', 'scatter', 'Scatter'],
    ['D', 'design', 'Design'],
  ];
  for (const [key, t, label] of tabs) out.push({ keys: [key], label, group: 'Edit', when: edit(), run: () => k.editTabTo(t) });
  const shapes: [string, BuildShape, string][] = [
    ['R', 'rect', 'Rectangle'],
    ['O', 'poly', 'Polygon'],
    ['T', 'tower', 'Round tower'],
  ];
  for (const [key, s, label] of shapes) out.push({ keys: [key], label, group: 'Build', when: edit('build'), run: () => k.buildShape(s) });
  out.push({ keys: ['X'], label: 'Crossing (again: bridge, ford, ferry)', group: 'Build', when: edit('build'), run: () => k.buildCross() });
  out.push({ keys: ['K'], label: 'Castle', group: 'Build', when: edit('build'), run: () => k.buildWorks('castle') });
  out.push({ keys: ['W'], label: 'Wall', group: 'Build', when: edit('build'), run: () => k.buildWorks('wall') });
  out.push(
    { keys: ['Enter'], label: 'Finish the polygon, castle or wall', group: 'Build', when: () => edit('build')() && k.building(), run: () => k.buildFinish() },
    { keys: ['Backspace'], label: 'Take a corner back', group: 'Build', when: () => edit('build')() && k.building(), run: () => k.buildBack() },
  );
  const town: [string, TownMode, string][] = [
    ['V', 'select', 'Choose a patch, drag a corner'],
    ['Q', 'displace', 'Displace'],
    ['L', 'liquify', 'Liquify'],
    ['O', 'bloat', 'Bloat'],
    ['I', 'pinch', 'Pinch'],
    ['R', 'relax', 'Relax'],
    ['E', 'equalize', 'Equalize'],
  ];
  for (const [key, m, label] of town) out.push({ keys: [key], label, group: 'Town', when: edit('town'), run: () => k.townMode(m) });
  out.push(
    { keys: [']'], label: 'Bigger brush', group: 'Town', repeat: true, when: () => edit('town')() && !k.inside(), run: () => k.townBrush(1) },
    { keys: ['['], label: 'Smaller brush', group: 'Town', repeat: true, when: () => edit('town')() && !k.inside(), run: () => k.townBrush(-1) },
  );
  const modes: [string, ScatterMode, string][] = [
    ['Q', 'stamp', 'Stamp'],
    ['W', 'brush', 'Brush'],
    ['E', 'erase', 'Erase'],
  ];
  for (const [key, m, label] of modes) out.push({ keys: [key], label, group: 'Scatter', when: edit('scatter'), run: () => k.scatterMode(m) });
  const design: [string, DesignMode, string][] = [
    ['V', 'select', 'Choose a room'],
    ['R', 'room', 'Room'],
    ['X', 'rect', 'Rectangle'],
    ['H', 'corridor', 'Corridor'],
    ['K', 'rock', 'Rock'],
    ['O', 'door', 'Door'],
    ['F', 'prop', 'Props'],
    ['W', 'stairs', 'Way down (in a building: the stairs)'],
    ['L', 'wall', 'Wall line (in a building)'],
    ['E', 'merge', 'Take a wall away (in a building)'],
  ];
  const designing = () => edit('design')() && k.designing();
  for (const [key, m, label] of design) out.push({ keys: [key], label, group: 'Design', when: designing, run: () => k.designMode(m) });
  out.push({ keys: ['Ctrl+S'], label: 'Save the design', group: 'Design', inInputs: true, when: designing, run: () => k.saveDesign() });
  const tools: [string, PlayTool, string][] = [
    ['V', 'select', 'Select and move'],
    ['T', 'token', 'Tokens'],
    ['M', 'measure', 'Measure'],
    ['S', 'shape', 'Shapes'],
    ['F', 'fog', 'Fog of war'],
  ];
  for (const [key, t, label] of tools) out.push({ keys: [key], label, group: 'Play', when: playing, run: () => k.playTool(t) });
  out.push({ keys: ['Delete', 'Backspace'], label: 'Remove the selected tokens', group: 'Play', when: () => playing() && k.tokensSelected(), run: () => k.removeTokens() });
  return out;
}
