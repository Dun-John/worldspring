// The freehand part of drawing a line, as Road, water (Brush and Polygon), Row and Plot share it:
// a press dragged far enough becomes a stroke, a point every so many feet, and the point let go
// at ends it. (Their clicked corners, Enter, Backspace and Escape differ tool by tool: what a
// finished line does, what Backspace undoes with it, what Escape drops, so they stay in each.)
import type { Pt } from './build';

/** The pointer pressed at `press` moved on to `p`: the stroke so far (begun once `p` is `start`
 * ft or more from the press; a point added each `step` ft), else null while it is still a click. */
export function dragTo(stroke: Pt[] | null, press: Pt, p: Pt, start: number, step: number): Pt[] | null {
  if (!stroke) {
    if (Math.hypot(p[0] - press[0], p[1] - press[1]) < start) return null;
    stroke = [press];
  }
  const last = stroke[stroke.length - 1];
  if (Math.hypot(p[0] - last[0], p[1] - last[1]) >= step) stroke.push(p);
  return stroke;
}

/** A stroke let go at `p`: its last point, unless it is within a foot of the one before. */
export function endAt(stroke: Pt[], p: Pt): Pt[] {
  const last = stroke[stroke.length - 1];
  if (Math.hypot(p[0] - last[0], p[1] - last[1]) > 1) stroke.push(p);
  return stroke;
}
