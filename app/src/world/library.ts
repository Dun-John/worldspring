// This browser's world storage (IndexedDB): saved worlds, each world's current edits (kept
// here rather than in share links once they grow), and uploaded pictures (see assets.ts).
import type { Edits, WorldFile } from '../gen/protocol';

export interface SavedWorld {
  id: string;
  name: string;
  file: WorldFile;
  savedAt: number;
}

const DB = 'pfm'; // pre-Worldspring name, kept so saved worlds survive the rename
const STORE = 'worlds';
let db: Promise<IDBDatabase> | null = null;
// Worlds too big for a link (`#lib=` links name them), in a database of their own: adding a store
// to 'pfm' would raise its version, which a tab of an older build (they never close theirs)
// blocks.
const FILES_DB = 'pfm-files';
const FILES = 'files';
let filesDb: Promise<IDBDatabase> | null = null;

/**
 * Open database `name` at `version`, `upgrade` making its stores. Older generators' builds on the
 * site (`versions.ts`) share this browser's databases with the newest, so a database a newer build
 * has already taken to a later version is opened as it is (schema changes must only add stores).
 */
export function openDb(name: string, version: number, upgrade: (d: IDBDatabase) => void, closed?: () => void): Promise<IDBDatabase> {
  // (A later build raising the version closes this connection, so it isn't blocked; `closed`
  // forgets it, to be opened again.)
  const keep = (d: IDBDatabase) => {
    d.onversionchange = () => {
      d.close();
      closed?.();
    };
    return d;
  };
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(name, version);
    req.onupgradeneeded = () => upgrade(req.result);
    req.onsuccess = () => resolve(keep(req.result));
    req.onerror = () => {
      if (req.error?.name !== 'VersionError') return reject(req.error);
      const now = indexedDB.open(name);
      now.onsuccess = () => resolve(keep(now.result));
      now.onerror = () => reject(now.error);
    };
  });
}

function open(): Promise<IDBDatabase> {
  return (db ??= openDb(
    DB,
    2,
    (d) => {
      const names = d.objectStoreNames;
      if (!names.contains(STORE)) d.createObjectStore(STORE, { keyPath: 'id' });
      if (!names.contains('edits')) d.createObjectStore('edits');
      if (!names.contains('assets')) d.createObjectStore('assets');
    },
    () => (db = null),
  ));
}

function openFiles(): Promise<IDBDatabase> {
  return (filesDb ??= openDb(FILES_DB, 1, (d) => d.objectStoreNames.contains(FILES) || d.createObjectStore(FILES), () => (filesDb = null)));
}

export async function tx<T>(mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>, store = STORE): Promise<T> {
  const d = await open();
  return new Promise((resolve, reject) => {
    const req = fn(d.transaction(store, mode).objectStore(store));
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

// A world's edits are kept field by field (`<worldKey>|<field>`), so a change stores only the
// fields it touched: a world with thousands of objects doesn't write them all for a rename.
// (Older saves hold the whole edits under `<worldKey>`; they are read, and replaced on the
// next save.)

/** The fields as last stored, by world (edits are never changed in place: the same object is
 * the same field). */
const stored = new Map<string, Record<string, unknown>>();

/** A world's current edits (by `worldKey`), or null. */
export async function loadEdits(key: string): Promise<Edits | null> {
  try {
    const d = await open();
    const range = IDBKeyRange.bound(`${key}|`, `${key}|￿`);
    const [keys, values, whole] = await new Promise<[IDBValidKey[], unknown[], unknown]>((resolve, reject) => {
      const t = d.transaction('edits', 'readonly');
      const s = t.objectStore('edits');
      const k = s.getAllKeys(range);
      const v = s.getAll(range);
      const w = s.get(key);
      t.oncomplete = () => resolve([k.result, v.result, w.result]);
      t.onerror = () => reject(t.error);
    });
    if (!keys.length) return (whole as Edits | undefined) ?? null;
    const e = Object.fromEntries(keys.map((k, i) => [String(k).slice(key.length + 1), values[i]])) as Edits;
    stored.set(key, { ...e });
    return e;
  } catch {
    return null;
  }
}

/** Ask the browser to keep this site's storage under pressure (once, at the first thing worth
 * keeping): everything a visitor makes lives only here. */
let persistAsked = false;
function askPersist() {
  if (persistAsked) return;
  persistAsked = true;
  void navigator.storage?.persist?.().catch(() => {});
}

export async function saveEdits(key: string, edits: Edits): Promise<void> {
  if (Object.keys(edits).length) askPersist();
  try {
    const d = await open();
    const was = stored.get(key);
    const now = edits as Record<string, unknown>;
    await new Promise<void>((resolve, reject) => {
      const t = d.transaction('edits', 'readwrite');
      const s = t.objectStore('edits');
      if (!was) {
        // First save here: the old whole record and any fields stored before go.
        s.delete(key);
        s.delete(IDBKeyRange.bound(`${key}|`, `${key}|￿`));
      }
      for (const f of new Set([...Object.keys(was ?? {}), ...Object.keys(now)])) {
        if (was && was[f] === now[f]) continue;
        if (now[f] === undefined) s.delete(`${key}|${f}`);
        else s.put(now[f], `${key}|${f}`);
      }
      t.oncomplete = () => resolve();
      t.onerror = () => reject(t.error);
    });
    stored.set(key, { ...now });
  } catch (e) {
    console.warn('[library] edits not saved', e);
  }
}

export async function listWorlds(): Promise<SavedWorld[]> {
  try {
    const all = await tx<SavedWorld[]>('readonly', (s) => s.getAll() as IDBRequest<SavedWorld[]>);
    return all.sort((a, b) => b.savedAt - a.savedAt);
  } catch {
    return [];
  }
}

export async function saveWorld(name: string, file: WorldFile): Promise<void> {
  askPersist();
  const id = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  await tx('readwrite', (s) => s.put({ id, name, file, savedAt: Date.now() } satisfies SavedWorld));
}

export async function deleteWorld(id: string): Promise<void> {
  await tx('readwrite', (s) => s.delete(id));
}

// Worlds whose links name them (`#lib=`: a sketch too big for the address bar), without their
// edits (kept by `editsKey` as always). The most recently linked are kept.

interface LinkedWorld {
  file: WorldFile;
  usedAt: number;
}

/** How many linked worlds are kept. */
const FILES_KEPT = 40;
/** Those this page has kept already (each is stored once). */
const keptHere = new Set<string>();

/** Keep `file` for `#lib=<key>` links. */
export async function keepLinked(key: string, file: WorldFile): Promise<void> {
  if (keptHere.has(key)) return;
  askPersist();
  const d = await openFiles();
  await new Promise<void>((resolve, reject) => {
    const t = d.transaction(FILES, 'readwrite');
    const s = t.objectStore(FILES);
    s.put({ file, usedAt: Date.now() } satisfies LinkedWorld, key);
    // (The oldest beyond the number kept go.)
    const all = s.getAll();
    const keys = s.getAllKeys();
    keys.onsuccess = () => {
      const by = (all.result as LinkedWorld[]).map((v, i) => [v.usedAt, keys.result[i]] as const).sort((a, b) => b[0] - a[0]);
      for (const [, k] of by.slice(FILES_KEPT)) if (k !== key) s.delete(k);
    };
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
  });
  keptHere.add(key);
}

/** The world a `#lib=<key>` link names, if this browser keeps it. */
export async function linked(key: string): Promise<WorldFile | null> {
  try {
    const d = await openFiles();
    const v = await new Promise<unknown>((resolve, reject) => {
      const req = d.transaction(FILES, 'readonly').objectStore(FILES).get(key);
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    return (v as LinkedWorld | undefined)?.file ?? null;
  } catch {
    return null;
  }
}

// A backup of the whole library in one file: every saved world, every world's edits, the
// worlds links name and every picture; restored in another browser, or in this one after its
// site data was cleared.

/** Marks the edits keys whose edits came from a backup, until their world opens. */
const RESTORED = 'ws-restored:';

/** Whether world `key`'s edits came from a backup since it was last open (asked once). */
export function takeRestored(key: string): boolean {
  try {
    const was = localStorage.getItem(RESTORED + key) !== null;
    localStorage.removeItem(RESTORED + key);
    return was;
  } catch {
    return false;
  }
}

/** Marks a backup file. */
export const BACKUP_FORMAT = 'worldspring-library';

export interface Backup {
  format: typeof BACKUP_FORMAT;
  version: 1;
  savedAt: number;
  worlds: SavedWorld[];
  /** The `edits` store as it is (`<editsKey>|<field>`, or an older whole record, → value). */
  edits: Record<string, unknown>;
  /** `#lib` worlds by key. */
  files: Record<string, LinkedWorld>;
  /** Pictures as `assets.ts` `bundleAssets` has them. */
  assets: Record<string, { name: string; data: string }>;
}

/** Every entry of a store, as [key, value]. */
async function entries(store: string, from = open): Promise<[string, unknown][]> {
  const d = await from();
  return new Promise((resolve, reject) => {
    const t = d.transaction(store, 'readonly');
    const k = t.objectStore(store).getAllKeys();
    const v = t.objectStore(store).getAll();
    t.oncomplete = () => resolve(k.result.map((key, i) => [String(key), v.result[i]]));
    t.onerror = () => reject(t.error);
  });
}

/** The edits key a stored entry belongs to. */
const editsOf = (k: string) => k.split('|')[0];

/** The stored entries of each world's edits, by edits key. */
function byWorld(list: [string, unknown][]): Map<string, string> {
  const out = new Map<string, [string, unknown][]>();
  for (const e of list) out.set(editsOf(e[0]), [...(out.get(editsOf(e[0])) ?? []), e]);
  return new Map([...out].map(([k, v]) => [k, JSON.stringify(v.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))]));
}

/**
 * The whole library as one file, made in parts (a big library is never one string). `open`:
 * the world showing now, added to the saved worlds unless one of them is it.
 */
export async function backup(open: SavedWorld | null): Promise<Blob> {
  const worlds = await listWorlds();
  if (open && !worlds.some((w) => JSON.stringify(w.file) === JSON.stringify(open.file))) worlds.unshift(open);
  const edits = Object.fromEntries(await entries('edits'));
  const files = Object.fromEntries(await entries(FILES, openFiles)) as Record<string, LinkedWorld>;
  const head = JSON.stringify({ format: BACKUP_FORMAT, version: 1, savedAt: Date.now(), worlds, edits, files } satisfies Omit<Backup, 'assets'>);
  const parts: BlobPart[] = [head.slice(0, -1), ',"assets":{'];
  let first = true;
  for (const [id, a] of await entries('assets')) {
    const { blob, name } = a as { blob: Blob; name: string };
    const data = await new Promise<string>((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(String(r.result));
      r.onerror = () => reject(r.error);
      r.readAsDataURL(blob);
    });
    parts.push(`${first ? '' : ','}${JSON.stringify(id)}:${JSON.stringify({ name: name ?? '', data })}`);
    first = false;
  }
  parts.push('}}');
  return new Blob(parts, { type: 'application/json' });
}

/** Whether `v` is a backup file. */
export function isBackup(v: unknown): v is Backup {
  const b = v as Backup;
  return !!b && b.format === BACKUP_FORMAT && Array.isArray(b.worlds) && !!b.edits && typeof b.edits === 'object';
}

export interface RestorePlan {
  /** Saved worlds this browser doesn't have. */
  worlds: number;
  /** Worlds with edits in the backup. */
  edited: number;
  pictures: number;
  /** Edits keys of worlds whose edits here differ from the backup's. */
  differ: string[];
}

/** What restoring `b` would bring. */
export async function restorePlan(b: Backup): Promise<RestorePlan> {
  const here = byWorld(await entries('edits'));
  const theirs = byWorld(Object.entries(b.edits));
  const known = new Set((await listWorlds()).map((w) => w.id));
  return {
    worlds: b.worlds.filter((w) => w?.id && !known.has(w.id)).length,
    edited: theirs.size,
    pictures: Object.keys(b.assets ?? {}).length,
    differ: [...theirs].filter(([k, v]) => here.has(k) && here.get(k) !== v).map(([k]) => k),
  };
}

/**
 * Restore `b`: its saved worlds join these (one in both keeps the newer save), its pictures and
 * linked worlds too; a world's edits are taken where this browser has none, and where it has
 * others only if `replace`. Returns the edits keys whose edits were taken from it.
 */
export async function restore(b: Backup, replace: boolean, keepPictures: (bundle: Backup['assets']) => Promise<void>): Promise<string[]> {
  askPersist();
  const here = byWorld(await entries('edits'));
  const theirs = byWorld(Object.entries(b.edits));
  const take = new Set([...theirs].filter(([k, v]) => !here.has(k) || (replace && here.get(k) !== v)).map(([k]) => k));
  const saved = new Map((await listWorlds()).map((w) => [w.id, w]));
  const d = await open();
  await new Promise<void>((resolve, reject) => {
    const t = d.transaction([STORE, 'edits'], 'readwrite');
    const ws = t.objectStore(STORE);
    for (const w of b.worlds) {
      if (!w?.id || !w.file) continue;
      const mine = saved.get(w.id);
      if (!mine || mine.savedAt < w.savedAt) ws.put(w);
    }
    const es = t.objectStore('edits');
    for (const key of take) {
      // (The world's edits here go whole: the backup's are what it keeps.)
      es.delete(key);
      es.delete(IDBKeyRange.bound(`${key}|`, `${key}|￿`));
      stored.delete(key);
    }
    for (const [k, v] of Object.entries(b.edits)) if (take.has(editsOf(k))) es.put(v, k);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
  });
  const f = await openFiles();
  await new Promise<void>((resolve, reject) => {
    const t = f.transaction(FILES, 'readwrite');
    for (const [k, v] of Object.entries(b.files ?? {})) if (v?.file) t.objectStore(FILES).put(v, k);
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
  });
  // (mapd's copies of these worlds give way to them when they open next.)
  try {
    for (const k of take) localStorage.setItem(RESTORED + k, '1');
  } catch {
    // No storage: mapd's copy wins, as on any open.
  }
  await keepPictures(b.assets ?? {});
  return [...take];
}
