// Edit grouped by thing (`?ui=new`): every tool once, with its group, name, key, icon and the
// one line it shows until it has been used, and the tab, mode and part of the panel it opens.
// It feeds the strip (`EditStrip.svelte`), the keymap's openers and the shortcuts sheet. Which
// tool is open is worked out from the panels' state (`toolOf`), never kept on its own.
import type { BuildMode } from '../BuildPanel.svelte';
import type { IconName } from '../icons';
import type { EditTab } from './layout.svelte';

export type GroupKey = 'settlements' | 'buildings' | 'roads' | 'land' | 'rooms';
export type ToolKey =
  | 'select'
  | 'places'
  | 'streets'
  | 'townwall'
  | 'members'
  | 'draw'
  | 'single'
  | 'row'
  | 'fill'
  | 'works'
  | 'road'
  | 'crossing'
  | 'water'
  | 'docks'
  | 'ground'
  | 'fences'
  | 'trees'
  | 'rooms'
  | 'stairs';

export interface EditToolDef {
  key: ToolKey;
  /** Select stands apart, in the panel's header. */
  group: GroupKey | null;
  /** The panel's full name. */
  label: string;
  /** On the narrow tool row. */
  short: string;
  icon: IconName;
  /** Its key(s), as the shortcuts sheet shows them. */
  kbd: string;
  /** One line under the rows until the tool has been used. */
  hint: string;
  /** The panel it opens: Edit's tab, Build's mode, Streets' part, Select's purpose, Rooms' ways across. */
  tab: EditTab;
  mode?: BuildMode;
  part?: 'streets' | 'walls';
  purpose?: 'all' | 'members';
  links?: boolean;
}

export const GROUPS: { key: GroupKey; label: string; icon: IconName }[] = [
  { key: 'settlements', label: 'Settlements', icon: 'wards' },
  { key: 'buildings', label: 'Buildings', icon: 'building' },
  { key: 'roads', label: 'Roads & water', icon: 'road' },
  { key: 'land', label: 'Land', icon: 'mountain' },
  { key: 'rooms', label: 'Rooms', icon: 'room' },
];

export const EDIT_TOOLS: EditToolDef[] = [
  { key: 'select', group: null, label: 'Select', short: 'Select', icon: 'pointer', kbd: 'A', tab: 'build', mode: 'select', purpose: 'all', hint: 'Click anything to see what it is; drag a box, or Shift-drag a lasso, for many.' },
  { key: 'places', group: 'settlements', label: 'Places', short: 'Places', icon: 'pin', kbd: 'S', tab: 'sites', hint: 'Click the map to add a town, ruin, dungeon, cave, mine, tower, camp or inn.' },
  { key: 'streets', group: 'settlements', label: 'Streets', short: 'Streets', icon: 'wards', kbd: 'U', tab: 'town', part: 'streets', hint: 'Re-plan the town in view: move its corners, set each area’s ward and lots.' },
  { key: 'townwall', group: 'settlements', label: 'Town wall', short: 'Town wall', icon: 'tower', kbd: 'Shift+U', tab: 'town', part: 'walls', hint: 'Walls on or off; click a ringed corner on the wall for a gate or a tower.' },
  { key: 'members', group: 'settlements', label: 'Join or leave a town', short: 'Join or leave', icon: 'link', kbd: 'Shift+J', tab: 'build', mode: 'select', purpose: 'members', hint: 'Click or box what should be part of a town, or taken out of one.' },
  { key: 'draw', group: 'buildings', label: 'Draw a building', short: 'Draw', icon: 'polygon', kbd: 'B', tab: 'build', mode: 'building', hint: 'Drag a rectangle, click a polygon’s corners, or drag out a round tower.' },
  { key: 'single', group: 'buildings', label: 'Single building', short: 'One building', icon: 'stamp', kbd: 'Q', tab: 'build', mode: 'stamp', hint: 'Click to set one down, facing the nearest street.' },
  { key: 'row', group: 'buildings', label: 'Row of houses', short: 'Row', icon: 'home', kbd: 'E', tab: 'build', mode: 'row', hint: 'Drag along a street: houses line it on the side you choose.' },
  { key: 'fill', group: 'buildings', label: 'Fill an area', short: 'Fill area', icon: 'shapes', kbd: 'L', tab: 'build', mode: 'lasso', hint: 'Draw round an area to fill it with buildings, lots or open ground.' },
  { key: 'works', group: 'buildings', label: 'Castle & walls', short: 'Castle', icon: 'castle', kbd: 'K / W', tab: 'build', mode: 'castle', hint: 'Draw a castle’s walls, or a wall on its own; towers and gates where you click.' },
  { key: 'road', group: 'roads', label: 'Road', short: 'Road', icon: 'road', kbd: 'V', tab: 'build', mode: 'road', hint: 'Click its points and double-click to end, or drag; click a road to change it.' },
  { key: 'crossing', group: 'roads', label: 'Crossing', short: 'Crossing', icon: 'bridge', kbd: 'X', tab: 'build', mode: 'crossing', hint: 'Click across a river: a bridge, ford or ferry goes bank to bank.' },
  { key: 'water', group: 'roads', label: 'Water', short: 'Water', icon: 'lake', kbd: 'I', tab: 'build', mode: 'water', hint: 'Paint water along a stroke, or outline it.' },
  { key: 'docks', group: 'roads', label: 'Docks & islets', short: 'Docks & islets', icon: 'waves', kbd: 'H', tab: 'build', mode: 'harbour', hint: 'Quays, piers, breakwaters and islets at the water’s edge.' },
  { key: 'ground', group: 'land', label: 'Ground', short: 'Ground', icon: 'raise', kbd: 'M', tab: 'ground', hint: 'Raise, lower or smooth the battlemap’s ground, or draw a plateau.' },
  { key: 'fences', group: 'land', label: 'Fences & hedges', short: 'Fences', icon: 'wall', kbd: 'F', tab: 'build', mode: 'fence', hint: 'Pick a kind, then drag or click a line; Gate and Erase work on any fence.' },
  { key: 'trees', group: 'land', label: 'Trees & props', short: 'Trees', icon: 'tree', kbd: 'C', tab: 'scatter', hint: 'Trees, rocks and props: place one, brush many, or erase.' },
  { key: 'rooms', group: 'rooms', label: 'Rooms', short: 'Rooms', icon: 'room', kbd: 'D', tab: 'design', links: false, hint: 'Go into a building, or down into a site, to design its rooms.' },
  { key: 'stairs', group: 'rooms', label: 'Stairs & passages', short: 'Stairs', icon: 'stairs', kbd: 'J', tab: 'design', links: true, hint: 'Join a square below ground to another level or place.' },
];

export const TOOL = Object.fromEntries(EDIT_TOOLS.map((t) => [t.key, t])) as Record<ToolKey, EditToolDef>;

/** What the panels have open, for `toolOf`. */
export interface ToolState {
  tab: EditTab;
  buildMode: BuildMode;
  townPart: 'streets' | 'walls';
  selectPurpose: 'all' | 'members';
  linkOn: boolean;
}

/** The tool the open panel is (Build's Clear, the old layout's, counts as Select). */
export function toolOf(s: ToolState): ToolKey {
  switch (s.tab) {
    case 'sites':
      return 'places';
    case 'town':
      return s.townPart === 'walls' ? 'townwall' : 'streets';
    case 'ground':
      return 'ground';
    case 'scatter':
      return 'trees';
    case 'design':
      return s.linkOn ? 'stairs' : 'rooms';
  }
  if (s.buildMode === 'select' || s.buildMode === 'clear') return s.selectPurpose === 'members' ? 'members' : 'select';
  if (s.buildMode === 'castle' || s.buildMode === 'wall') return 'works';
  return EDIT_TOOLS.find((t) => t.tab === 'build' && t.mode === s.buildMode)?.key ?? 'draw';
}
