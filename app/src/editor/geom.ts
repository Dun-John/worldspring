/** Small plane helpers the editor tools share (hit-tests, never generator output). */
export type Pt = [number, number];

/** Distance from (px, py) to the segment a-b; a zero-length segment gives the distance to a. */
export function segDistXY(px: number, py: number, ax: number, ay: number, bx: number, by: number): number {
  const [dx, dy] = [bx - ax, by - ay];
  const l = dx * dx + dy * dy;
  const t = l > 0 ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / l)) : 0;
  return Math.hypot(px - ax - t * dx, py - ay - t * dy);
}

export function segDist(p: Pt, a: Pt, b: Pt): number {
  return segDistXY(p[0], p[1], a[0], a[1], b[0], b[1]);
}

/** Whether (x, y) is inside the polygon (even-odd). */
export function contains(pts: Pt[], x: number, y: number): boolean {
  let inside = false;
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i];
    const [xj, yj] = pts[j];
    if (yi > y !== yj > y && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi) inside = !inside;
  }
  return inside;
}

export const inside = (p: Pt, poly: Pt[]): boolean => contains(poly, p[0], p[1]);
export const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);
export const lerp = (a: Pt, b: Pt, t: number): Pt => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
