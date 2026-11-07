// Messages between the DM's window and the player window (same browser, same origin): play
// state changes as ops, plus what is only shown in passing (camera, pings, the ruler, what
// the characters see now). The player window builds its own map from the world file.
import type { WorldFile } from '../gen/protocol';
import type { Bitmap } from './vision';
import type { Op, PlayState } from './state';

/** Where the DM is playing: the surface, or a level of a building or site. */
export interface Scene {
  interior: string | null;
  level: number;
  /** The DM's location key (ties when the players' view follows the party). */
  loc?: string;
}

/** The DM's view: centre and how much ground it shows (ft), and what the players' view does
 * with it. */
export interface CameraMsg {
  cx: number;
  cy: number;
  w: number;
  h: number;
  follow: boolean;
  lock: boolean;
  /** The players' view follows a token (its id) or the party ('party') on its own: it goes
   * where they are, whatever the DM's camera and scene do. */
  target?: string | null;
}

/** A ruler between two squares, or (`free`, measuring the map zoomed out) between two
 * points, in a straight line (squares of the location's grid either way). */
export interface Ruler {
  loc: string;
  a: [number, number];
  b: [number, number];
  free?: boolean;
}

export type Vision = Bitmap & { loc: string };

export type Msg =
  /** A player window opened (or reloaded): it wants everything. */
  | { t: 'hello' }
  /** The DM's window (re)opened: a players' window open says hello again. */
  | { t: 'dm' }
  | { t: 'snapshot'; world: WorldFile; state: PlayState; scene: Scene; camera: CameraMsg | null; vision: Vision | null; on: boolean; places: boolean }
  /** The world's edits changed (names, created sites). */
  | { t: 'edits'; world: WorldFile }
  | { t: 'op'; op: Op }
  | { t: 'scene'; scene: Scene }
  | { t: 'camera'; camera: CameraMsg }
  | { t: 'ping'; loc: string; x: number; y: number }
  | { t: 'ruler'; ruler: Ruler | null }
  | { t: 'vision'; vision: Vision | null }
  /** Play mode on or off in the DM's window (off: the players see the bare map). */
  | { t: 'mode'; on: boolean }
  /** The DM shows business and place pins (the players see those in sight). */
  | { t: 'places'; on: boolean }
  | { t: 'bye' };

export type Role = 'dm' | 'player';

/** One end of the link. Messages from the same role are ignored (a second DM tab). */
export class PlayChannel {
  onMessage: (m: Msg) => void = () => {};
  private readonly ch: BroadcastChannel | null;

  constructor(readonly role: Role) {
    this.ch = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('fantasy-map-play');
    if (this.ch) {
      this.ch.onmessage = (e: MessageEvent<{ from: Role; m: Msg }>) => {
        if (e.data?.from && e.data.from !== role) this.onMessage(e.data.m);
      };
    }
  }

  send(m: Msg) {
    this.ch?.postMessage({ from: this.role, m });
  }

  close() {
    this.ch?.close();
  }
}
