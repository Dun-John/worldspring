// A ruin's plan at site zoom (v63 underground, `battlemap::ruin_plan`): payload site-polygon kinds
// 28 its floor, 29 a standing wall, 30 rubble. Light, hand-drawn tones, walls pale with a thin ink edge.

/** Fill colour and alpha of a ruin plan's piece, or null for another kind. */
export function ruinPlanFill(kind: number): [[number, number, number], number] | null {
  if (kind === 28) return [[0.86, 0.83, 0.72], 0.6];
  if (kind === 29) return [[0.93, 0.9, 0.78], 1];
  if (kind === 30) return [[0.66, 0.62, 0.55], 0.95];
  return null;
}
