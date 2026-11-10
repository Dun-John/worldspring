// What a site's stone looks like in `underground.frag`, by its kind and theme: the wall style
// (`uTheme`, one coherent branch per site) and the palette it is drawn in (flagstones, coping on
// built walls, the hatched rock round it). Floors keep their material (`materialOf`); the theme
// only colours them and dresses the walls.

/** Wall styles (`uTheme`): plain dressed stone, rune-cut blocks (thick walls), rough ashlar,
 * dark basalt, dusty pale stone, wet brick, marble, packed earth, natural rock. */
export const Style = { Plain: 0, Dwarven: 1, Ashlar: 2, Basalt: 3, Dusty: 4, Brick: 5, Marble: 6, Earth: 7, Natural: 8 } as const;
export type Style = (typeof Style)[keyof typeof Style];

export interface UnderLook {
  style: Style;
  /** Flagstones (boss chambers a shade darker) and hewn floors. */
  stone: [number, number, number];
  /** The dressed stone on built walls' tops. */
  coping: [number, number, number];
  /** The hatched rock beside the walls (drawn at 0.7 over the dimmed map): about a mid surface
   * under the dim, a touch darker, so the site neither glows nor punches a hole. */
  rock: [number, number, number];
  /** The painted extras in `underground.frag` (`uLook`): 0 none, 1 green (moss, roots), 2 dry (sand drifts), 3 cold (frost), 4 ice. */
  feel: number;
  /** A multiplier on floors (`uTint`): the biome's cast over the theme's own floor. */
  tint: [number, number, number];
}

const look = (style: Style, stone: UnderLook['stone'], coping: UnderLook['coping'], rock: UnderLook['rock']): UnderLook => ({ style, stone, coping, rock, feel: 0, tint: [1, 1, 1] });

const PLAIN = look(Style.Plain, [0.58, 0.55, 0.5], [0.56, 0.53, 0.48], [0.28, 0.27, 0.25]);
const DWARVEN = look(Style.Dwarven, [0.64, 0.56, 0.45], [0.6, 0.52, 0.41], [0.29, 0.26, 0.22]);
const FORTRESS = look(Style.Ashlar, [0.52, 0.54, 0.57], [0.5, 0.52, 0.55], [0.27, 0.28, 0.3]);
const CULT = look(Style.Basalt, [0.4, 0.32, 0.34], [0.38, 0.27, 0.27], [0.27, 0.23, 0.24]);
const TOMB = look(Style.Dusty, [0.68, 0.64, 0.56], [0.7, 0.66, 0.58], [0.42, 0.4, 0.36]);
const OSSUARY = look(Style.Dusty, [0.72, 0.68, 0.58], [0.74, 0.7, 0.6], [0.42, 0.4, 0.36]);
const SEWER = look(Style.Brick, [0.5, 0.5, 0.42], [0.46, 0.5, 0.36], [0.26, 0.29, 0.23]);
const MARBLE = look(Style.Marble, [0.6, 0.59, 0.57], [0.72, 0.7, 0.67], [0.29, 0.29, 0.3]);
const EARTH = look(Style.Earth, [0.52, 0.45, 0.36], [0.5, 0.41, 0.31], [0.45, 0.38, 0.3]);
const CAVE = look(Style.Natural, [0.58, 0.55, 0.5], [0.56, 0.53, 0.48], [0.29, 0.27, 0.245]);
const MINE = look(Style.Natural, [0.58, 0.55, 0.5], [0.56, 0.53, 0.48], [0.3, 0.26, 0.22]);
const LAVA = look(Style.Natural, [0.58, 0.55, 0.5], [0.56, 0.53, 0.48], [0.26, 0.225, 0.21]);
// (Ice: white and pale blue rock, about the snow and ice ground above under the dim.)
const ICE: UnderLook = { ...look(Style.Natural, [0.58, 0.55, 0.5], [0.56, 0.53, 0.48], [0.55, 0.65, 0.76]), feel: 4 };
/** A cave's biome looks (`Interior::look` 1-3): the rock's tone and the floor's cast. */
const BIOME_LOOKS: { rock: UnderLook['rock']; tint: UnderLook['tint'] }[] = [
  { rock: [0.25, 0.3, 0.215], tint: [0.9, 1.0, 0.86] },
  { rock: [0.36, 0.3, 0.215], tint: [1.1, 0.99, 0.8] },
  { rock: [0.3, 0.325, 0.36], tint: [0.92, 0.99, 1.07] },
];

/** A site's look from its kind (`Interior.function`) and theme (`Interior.theme`, `under::THEMES`). */
export function underLook(site: string, theme?: string, biome = 0): UnderLook {
  const base = themeLook(site, theme);
  // (Caves only, and an ice cave keeps its ice whatever the biome.)
  if (site !== 'cave' || biome < 1 || biome > 3 || base.feel === 4) return base;
  const b = BIOME_LOOKS[biome - 1];
  return { ...base, rock: b.rock, tint: b.tint, feel: biome };
}

function themeLook(site: string, theme?: string): UnderLook {
  if (site === 'sewer') return SEWER;
  switch (theme) {
    case 'dwarven_hall':
      return DWARVEN;
    case 'collapsed_fortress':
      return FORTRESS;
    case 'cult_sanctum':
      return CULT;
    case 'ossuary':
    case 'catacombs':
      return OSSUARY;
    case 'crypt':
    case 'tomb':
      return TOMB;
    case 'temple':
    case 'sunken_temple':
    case 'wizard_lair':
      return MARBLE;
    case 'goblin_warren':
    case 'bandit_hideout':
      return EARTH;
    case 'ice':
      return ICE;
  }
  if (site === 'catacombs') return OSSUARY;
  if (site === 'crypt') return TOMB;
  if (site === 'cave') return CAVE;
  if (site === 'mine') return MINE;
  if (site === 'lava tube') return LAVA;
  return PLAIN;
}
