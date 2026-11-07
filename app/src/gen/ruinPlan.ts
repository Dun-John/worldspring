// A ruin's plan at site zoom (v63 underground, `battlemap::ruin_plan`): payload site-polygon kinds
// 28 its floor, 29 a standing wall, 30 rubble. Light, hand-drawn tones, no ink outline.

/** Fill colour and alpha of a ruin plan's piece, or null for another kind. */
export function ruinPlanFill(kind: number): [[number, number, number], number] | null {
  if (kind === 28) return [[0.76, 0.72, 0.63], 0.5];
  if (kind === 29) return [[0.55, 0.5, 0.44], 1];
  if (kind === 30) return [[0.66, 0.62, 0.55], 0.95];
  return null;
}
