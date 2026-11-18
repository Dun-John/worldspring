// The site designer's menu choices (`DesignSettings`), apart from the designer itself so the app
// holds them without loading it (the designer is loaded when Edit › Rooms first opens).
export type DesignMode = 'select' | 'room' | 'rect' | 'corridor' | 'rock' | 'door' | 'terrain' | 'prop' | 'stairs' | 'wall' | 'merge' | 'landing';

/** The designer menu's choices. */
export interface DesignSettings {
  mode: DesignMode;
  /** The kind of room a new one is. */
  kind: string;
  /** Brush width (squares). */
  width: number;
  /** The piece in the hand ('' none: a click picks one up). */
  prop: string;
  /** The prop's size (squares), across and down. */
  pw: number;
  ph: number;
  /** The side it is turned to stand against (0 north 1 east 2 south 3 west); null: its own. */
  back: number | null;
  /** The look it is given (`Item::variant`); null: its own (by position; a banner the building's house). */
  variant: number | null;
  /** Doors added wherever a room dug would be shut off. */
  autoDoors: boolean;
  /** The door tool's lock: 0 none (plain and secret doors), 1-7 a lock of that colour (`Door::lock`). */
  lock: number;
  /** The door tool puts portcullises (iron grates) instead of doors. */
  portcullis: boolean;
  /** What the terrain tool paints (`TERRAIN`, `SPANS`); '' clears it. */
  terrain: string;
}

export const defaultDesign = (): DesignSettings => ({ mode: 'room', kind: 'chamber', width: 1, prop: 'chest', pw: 1, ph: 1, back: null, variant: null, autoDoors: true, lock: 0, portcullis: false, terrain: 'water_shallow' });
