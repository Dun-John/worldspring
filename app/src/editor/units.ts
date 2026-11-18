// Numbers the editor tools share. A leaf (imports nothing), so the trace worker can use it too.

/** Feet in a mile. */
export const MI = 5280;
/** Smallest step of a freehand stroke (fence, water, path tools), in feet. */
export const FREEHAND_FT = 15;
/** How close (screen px) the pointer must be to grab a handle (town corners, street junctions). */
export const GRAB_PX = 10;
