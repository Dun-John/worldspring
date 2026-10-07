// Live link to `mapd` (the local agent server, crates/mapd): the open world's edits flow both
// ways, agents' changes arrive as they happen, and agents can look at the map (screenshots) or
// show the user a place. Only on this machine (mapd listens on 127.0.0.1); without mapd the
// app works as before and keeps trying quietly in the background.
import type { WorldFile } from '../gen/protocol';
import type { EditOp } from './ops';

/** What an edit did, as mapd logs it (`tool`, `id`, …). */
export type Change = Record<string, unknown> & { tool?: string; id?: string };

export type FromMapd =
  | { type: 'welcome'; client: number }
  | { type: 'world'; world: WorldFile }
  | { type: 'ops'; ops: EditOp[]; change: Change; author: string }
  | { type: 'render'; id: number; x: number; y: number; size: number }
  | { type: 'focus'; x: number; y: number; size: number }
  | { type: 'follow'; following: boolean; refused?: EditOp[] }
  | { type: 'resync' }
  | { type: 'saved' }
  | { type: 'error'; message: string };

/** mapd's socket: same origin when mapd serves the app, else the default port on this machine. */
function socketUrl(): string | null {
  if (!['localhost', '127.0.0.1', '[::1]'].includes(location.hostname)) return null;
  const port = new URLSearchParams(location.search).get('mapd');
  if (port === '0') return null;
  if (port) return `ws://127.0.0.1:${port}/ws`;
  return import.meta.env.DEV ? 'ws://127.0.0.1:7777/ws' : `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`;
}

/** mapd's HTTP address (for assets), or null where there is no mapd to reach. */
export function mapdHttp(): string | null {
  const ws = socketUrl();
  return ws ? ws.replace(/^ws/, 'http').replace(/\/ws$/, '') : null;
}

export class MapdSync {
  /** mapd's copy of the open world (its edits win: agents may have changed it meanwhile), and
   * the changes made here that mapd hasn't had (while it followed another tab's world, or was
   * away), to go on top. False if the app can't take them now (they are kept for next time). */
  onWorld: (w: WorldFile, mine: EditOp[]) => boolean = () => true;
  onOps: (ops: EditOp[], change: Change, author: string) => void = () => {};
  /** A screenshot request: resolve with a PNG data URL. */
  onRender: (x: number, y: number, size: number) => Promise<string> = () => Promise.reject(new Error('no map'));
  onFocus: (x: number, y: number, size: number) => void = () => {};
  onStatus: (connected: boolean) => void = () => {};
  /** Whether mapd follows this tab's world (another tab may have its own world open). */
  onFollow: (following: boolean) => void = () => {};
  /** mapd could not save, or switch to this tab's world before saving (`'switch'`); saved
   * again after that (null). */
  onSaveError: (problem: 'save' | 'switch' | null) => void = () => {};
  connected = false;

  private ws: WebSocket | null = null;
  private world: WorldFile | null = null;
  /** Whether mapd follows this tab's world: changes go to it only then. */
  private following = false;
  /** Changes made here that mapd hasn't had. */
  private pending: EditOp[] = [];
  /** mapd's copy, if it came when the app couldn't take it. */
  private untaken: WorldFile | null = null;
  /** The open world's edits replace mapd's copy (`open`), until mapd has answered with it. */
  private replace = false;
  private retryMs = 1000;
  private closed = false;
  private readonly url = socketUrl();

  constructor() {
    if (this.url) this.connect();
  }

  /** Whether mapd keeps the changes made here (on its disk): it is connected and follows this tab. */
  get keeps(): boolean {
    return this.connected && this.following;
  }

  /**
   * The world now open in the app (sent on connect and whenever it changes). `replace`: the
   * user chose its edits as they are here (a file's, or none, over those kept): they replace
   * mapd's copy, where mapd's otherwise wins.
   */
  open(world: WorldFile, replace = false) {
    this.world = world;
    this.replace = replace;
    // (Changes kept for the world before belong to it: the browser has them.)
    this.pending = [];
    this.untaken = null;
    this.following = false;
    this.hello();
  }

  /** Make mapd follow this tab's world, though another tab shows mapd's. */
  follow() {
    this.hello(true);
  }

  /** Ask mapd for its copy of the world open here again (changes from it were missed). */
  resync() {
    this.hello();
  }

  /** Say which world is open here (with `replace` until mapd has taken it). */
  private hello(take = false) {
    if (this.world) this.send({ type: 'hello', world: this.world, ...(take ? { take } : {}), ...(this.replace ? { replace: true } : {}) });
  }

  /** The user changed the edits here. */
  ops(ops: EditOp[], change: Change) {
    if (this.following && this.ws?.readyState === WebSocket.OPEN) this.send({ type: 'ops', ops, change });
    else this.pending.push(...ops);
  }

  /** Give the app mapd's copy that came while it couldn't take it. */
  retake() {
    if (this.untaken) this.take(this.untaken);
  }

  private take(world: WorldFile) {
    const mine = this.pending;
    this.pending = [];
    this.untaken = null;
    if (this.onWorld(world, mine)) return;
    this.untaken = world;
    this.pending = [...mine, ...this.pending];
  }

  dispose() {
    this.closed = true;
    this.ws?.close();
  }

  private connect() {
    const ws = new WebSocket(this.url!);
    this.ws = ws;
    ws.onopen = () => {
      this.retryMs = 1000;
      this.connected = true;
      this.onStatus(true);
      this.hello();
    };
    ws.onmessage = (e) => {
      try {
        void this.receive(JSON.parse(String(e.data)) as FromMapd);
      } catch (err) {
        console.warn('[mapd]', err);
      }
    };
    ws.onclose = () => {
      if (this.ws !== ws) return;
      this.ws = null;
      if (this.connected) this.onStatus(false);
      this.connected = false;
      this.following = false;
      if (this.closed) return;
      setTimeout(() => this.connect(), this.retryMs);
      this.retryMs = Math.min(30000, this.retryMs * 2);
    };
  }

  private async receive(m: FromMapd) {
    if (m.type === 'world') {
      // (mapd answers a hello with its copy only when it follows this tab.)
      this.following = true;
      this.replace = false;
      this.take(m.world);
    }
    else if (m.type === 'ops') this.onOps(m.ops ?? [], m.change ?? {}, m.author ?? 'agent');
    else if (m.type === 'focus') this.onFocus(m.x, m.y, m.size);
    else if (m.type === 'follow') {
      this.following = m.following;
      // Changes sent just as mapd turned to another world come back: kept for later.
      if (m.refused) this.pending.push(...m.refused);
      // Followed now (mapd turned to this world for another tab): what is kept for it goes, by a
      // hello (mapd answers with its copy; the edits chosen here replace it, the changes kept
      // go on top).
      if (m.following && (this.replace || this.pending.length)) this.hello();
      this.onFollow(m.following);
    }
    else if (m.type === 'resync') {
      // The tab mapd followed closed: say again which world is open here.
      this.hello();
    } else if (m.type === 'saved') this.onSaveError(null);
    else if (m.type === 'render') {
      try {
        this.send({ type: 'answer', id: m.id, png: await this.onRender(m.x, m.y, m.size) });
      } catch (err) {
        this.send({ type: 'answer', id: m.id, error: String(err) });
      }
    } else if (m.type === 'error') {
      console.warn('[mapd]', m.message);
      if (/switch worlds/.test(m.message)) this.onSaveError('switch');
      else if (/saved? to disk/.test(m.message)) this.onSaveError('save');
    }
  }

  private send(m: object) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(m));
  }
}
