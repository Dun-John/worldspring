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
  | { type: 'world'; world: WorldFile; store?: string; seq?: number }
  | { type: 'kept'; seq: number }
  | { type: 'ops'; ops: EditOp[]; change: Change; author: string }
  | { type: 'render'; id: number; x: number; y: number; size: number; width_px?: number; format?: 'jpeg' | 'png' }
  | { type: 'focus'; x: number; y: number; size: number }
  | { type: 'follow'; following: boolean; refused?: EditOp[] }
  | { type: 'resync' }
  | { type: 'saved' }
  | { type: 'error'; message: string; seq?: number }
  | { type: 'remade'; file: WorldFile; gone?: RemadeGone[] };

/** A settlement an agent's sketch change took away on purpose (`t0::remap::Gone`). */
export type RemadeGone = { id: string; name: string; li: number };

/** A screenshot scaled to `width` px across (3:2, never enlarged) and encoded as a JPEG (quality 0.85) or PNG data URL. */
async function encodeShot(url: string, width = 1024, format: 'jpeg' | 'png' = 'jpeg'): Promise<string> {
  const img = await createImageBitmap(await (await fetch(url)).blob());
  const w = Math.min(width, img.width);
  const h = Math.round((w * img.height) / img.width);
  if (format === 'png' && w === img.width) return url;
  const c = document.createElement('canvas');
  c.width = w;
  c.height = h;
  const g = c.getContext('2d');
  if (!g) throw new Error('no canvas');
  g.fillStyle = '#000';
  g.fillRect(0, 0, w, h);
  g.drawImage(img, 0, 0, w, h);
  img.close();
  return format === 'png' ? c.toDataURL('image/png') : c.toDataURL('image/jpeg', 0.85);
}

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
  /** mapd remade the world open here (an agent's `regenerate`): `file` is the world remade, with
   * the edits as they followed their places; `gone`: the settlements its sketch took away. */
  onRemade: (file: WorldFile, gone: RemadeGone[]) => void = () => {};
  onStatus: (connected: boolean) => void = () => {};
  /** Whether mapd follows this tab's world (another tab may have its own world open). */
  onFollow: (following: boolean) => void = () => {};
  /** mapd could not save, or switch to this tab's world before saving (`'switch'`); saved
   * again after that (null). */
  /** mapd saved the ops sent to disk (`inStep` may be true now). */
  onKept: () => void = () => {};
  onSaveError: (problem: 'save' | 'switch' | null) => void = () => {};
  connected = false;

  private ws: WebSocket | null = null;
  private world: WorldFile | null = null;
  /** Whether mapd follows this tab's world: changes go to it only then. */
  private following = false;
  /** Changes made here that mapd hasn't had. */
  private pending: EditOp[] = [];
  /** Changes sent that mapd hasn't said it saved (`kept`), by number and the socket they went
   * by: when that socket closes (mapd went away, or restarted) they go back to `pending`, to go
   * on top of mapd's copy again (ops set or remove one entry each, so sending one twice does no
   * harm). */
  private unacked: { seq: number; ops: EditOp[]; ws: WebSocket }[] = [];
  /** A hello was sent on this socket and mapd's copy hasn't come yet: changes made meanwhile
   * wait in `pending` (its copy, sent before them, would otherwise cover them up here). */
  private awaiting = false;
  /** mapd's copy, if it came when the app couldn't take it. */
  private untaken: WorldFile | null = null;
  /** The open world's edits replace mapd's copy (`open`), until mapd has answered with it. */
  private replace = false;
  /** The last ops sent (by number), and the last mapd said it saved to disk: mapd has every
   * change sent only when they meet (a mapd that never says, an older one, never meets them). */
  private sentSeq = 0;
  private keptSeq = 0;
  /** Ops sent that mapd could not take: not in step until its copy comes again. */
  private lost = false;
  /** mapd said it could not save to disk (until it says it saved). */
  private unsavedThere = false;
  /** mapd's world store, by the id its copy came with (none: an older mapd). */
  private store = '';
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

  /** Whether mapd has every change made here: it follows this tab, which has nothing unsent
   * (nor a copy of mapd's waiting, nor edits still to replace mapd's with). */
  get inStep(): boolean {
    return this.keeps && this.ws?.readyState === WebSocket.OPEN && !this.pending.length && !this.untaken && !this.replace && !this.lost && !this.unsavedThere && this.keptSeq >= this.sentSeq;
  }

  /** Changes made here that mapd hasn't had. */
  get unsent(): number {
    return this.pending.length;
  }

  /** Which mapd this tab reaches (null: none), with its world store as last heard. */
  get where(): string | null {
    return this.url && `${this.url}#${this.store}`;
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
    this.unacked = [];
    this.untaken = null;
    this.following = false;
    this.keptSeq = this.sentSeq;
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
    if (!this.world || this.ws?.readyState !== WebSocket.OPEN) return;
    this.requeue();
    this.awaiting = true;
    this.send({ type: 'hello', world: this.world, seq: this.sentSeq, ...(take ? { take } : {}), ...(this.replace ? { replace: true } : {}) });
  }

  /** The user changed the edits here. */
  ops(ops: EditOp[], change: Change) {
    const ws = this.ws;
    if (this.following && !this.awaiting && !this.untaken && ws?.readyState === WebSocket.OPEN) {
      this.unacked.push({ seq: ++this.sentSeq, ops, ws });
      this.send({ type: 'ops', ops, change, seq: this.sentSeq });
    } else this.pending.push(...ops);
  }

  /** Changes sent by a socket that has closed, which mapd may not have had: kept again, before
   * any made since (those were made after them, as nothing is sent while some wait). */
  private requeue() {
    const gone = this.unacked.filter((u) => u.ws !== this.ws || this.ws?.readyState !== WebSocket.OPEN);
    if (!gone.length) return;
    this.unacked = this.unacked.filter((u) => !gone.includes(u));
    this.pending = [...gone.flatMap((u) => u.ops), ...this.pending];
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
      this.awaiting = false;
      this.requeue();
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
      this.awaiting = false;
      this.store = m.store ?? '';
      // (Its copy has every op sent before the hello it answers: `seq`, which an older mapd leaves out.)
      // (Changes still unanswered went by this socket before the hello, so its copy has them.)
      if (m.seq != null) {
        this.lost = false;
        this.keptSeq = Math.max(this.keptSeq, m.seq);
        this.unacked = this.unacked.filter((u) => u.seq > m.seq!);
      }
      this.take(m.world);
    }
    else if (m.type === 'kept') {
      this.keptSeq = Math.max(this.keptSeq, m.seq);
      this.unacked = this.unacked.filter((u) => u.seq > m.seq);
      this.onKept();
    } else if (m.type === 'ops') this.onOps(m.ops ?? [], m.change ?? {}, m.author ?? 'agent');
    else if (m.type === 'focus') this.onFocus(m.x, m.y, m.size);
    else if (m.type === 'remade') {
      this.awaiting = false;
      this.onRemade(m.file, m.gone ?? []);
    }
    else if (m.type === 'follow') {
      this.following = m.following;
      if (!m.following) this.awaiting = false;
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
    } else if (m.type === 'saved') {
      this.unsavedThere = false;
      this.onSaveError(null);
    }
    else if (m.type === 'render') {
      try {
        this.send({ type: 'answer', id: m.id, image: await encodeShot(await this.onRender(m.x, m.y, m.size), m.width_px, m.format) });
      } catch (err) {
        this.send({ type: 'answer', id: m.id, error: String(err) });
      }
    } else if (m.type === 'error') {
      console.warn('[mapd]', m.message);
      // (A hello it could not answer with its copy is not waited for.)
      if (m.seq == null) this.awaiting = false;
      if (m.seq != null) {
        this.lost = true;
        // (Ops it could not apply: not sent again.)
        this.unacked = this.unacked.filter((u) => u.seq !== m.seq);
      }
      if (/switch worlds/.test(m.message)) this.onSaveError('switch');
      else if (/saved? to disk/.test(m.message)) {
        this.unsavedThere = true;
        this.onSaveError('save');
      }
    }
  }

  private send(m: object) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(m));
  }
}

// What this browser last knew mapd to have of a world's edits (`editsStamp`, by mapd and
// `editsKey`): edits kept here with that stamp were synced, so mapd's copy, if other, is newer
// and wins; others have changes only this browser has, and the user is asked.
const SYNCED = 'ws-synced';
const SYNCED_MAX = 60;

function syncedMarks(): [string, string][] {
  try {
    const v = JSON.parse(localStorage.getItem(SYNCED) ?? '[]');
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

/** The stamp of the edits mapd (`where`) last had of world `key` with this browser, if known. */
export function syncedStamp(where: string, key: string): string | null {
  return syncedMarks().find(([k]) => k === `${where}|${key}`)?.[1] ?? null;
}

/** mapd (`where`) has world `key`'s edits as stamped (null: not known any more). */
export function markSynced(where: string, key: string, stamp: string | null) {
  const k = `${where}|${key}`;
  const rest = syncedMarks().filter(([m]) => m !== k);
  try {
    localStorage.setItem(SYNCED, JSON.stringify((stamp === null ? rest : [...rest, [k, stamp]]).slice(-SYNCED_MAX)));
  } catch {
    // (No storage: the user is asked when mapd's copy differs from the edits kept here.)
  }
}
