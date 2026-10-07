// Play sessions kept in this browser (IndexedDB): each world's play state (tokens, fog, doors,
// shapes, settings), keyed by the world's hash, and token pictures. The player window reads
// pictures from here too (same origin), so only their ids travel between windows.
import { fitPicture } from '../world/assets';
import { openDb } from '../world/library';
import { emptyState, normalize, type PlayState } from './state';

const DB = 'fantasy-map-play'; // pre-Worldspring name, kept so play sessions survive the rename
let db: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
  return (db ??= openDb(
    DB,
    1,
    (d) => {
      d.createObjectStore('sessions');
      d.createObjectStore('images');
    },
    () => (db = null),
  ));
}

async function run<T>(store: string, mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest): Promise<T> {
  const d = await open();
  return new Promise((resolve, reject) => {
    const req = f(d.transaction(store, mode).objectStore(store));
    req.onsuccess = () => resolve(req.result as T);
    req.onerror = () => reject(req.error);
  });
}

/** A world's saved play state (empty if none, or if storage is unavailable). */
export async function loadSession(world: string): Promise<PlayState> {
  try {
    const s = await run<unknown>('sessions', 'readonly', (st) => st.get(world));
    // (Sessions saved by earlier versions are brought up to date.)
    return s ? normalize(s) : emptyState();
  } catch {
    return emptyState();
  }
}

export async function saveSession(world: string, state: PlayState): Promise<void> {
  try {
    await run('sessions', 'readwrite', (st) => st.put(state, world));
  } catch (e) {
    console.warn('[play] session not saved', e);
  }
}

/** Store a token picture; returns its id. */
export async function putImage(picture: Blob): Promise<string> {
  const blob = await fitPicture(picture);
  const id = `img-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  await run('images', 'readwrite', (st) => st.put(blob, id));
  return id;
}

export async function getImage(id: string): Promise<Blob | null> {
  try {
    return (await run<Blob | undefined>('images', 'readonly', (st) => st.get(id))) ?? null;
  } catch {
    return null;
  }
}
