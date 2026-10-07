// Changes not yet downloaded, by world (`editsKey`): everything made lives only in this
// browser's storage unless it goes out as a file (or mapd keeps it on disk), so the app reminds
// the user to download a world once enough has changed since the last time.

const PREFIX = 'ws-unexported:';

export interface Unexported {
  /** Changes since the last download. */
  n: number;
  /** When the first of them was made (ms). */
  since: number;
}

function read(key: string): Unexported | null {
  try {
    const v = JSON.parse(localStorage.getItem(PREFIX + key) ?? 'null') as Unexported | null;
    return v && typeof v.n === 'number' && typeof v.since === 'number' ? v : null;
  } catch {
    return null;
  }
}

/** What has changed in world `key` since it was last downloaded. */
export function unexported(key: string): Unexported | null {
  return read(key);
}

/** `count` more changes in world `key`; returns the total since the last download. */
export function noteChanges(key: string, count = 1): Unexported {
  const now = read(key) ?? { n: 0, since: Date.now() };
  now.n += count;
  try {
    localStorage.setItem(PREFIX + key, JSON.stringify(now));
  } catch {
    // No storage (private mode): no reminders either.
  }
  return now;
}

/** World `key` was downloaded (null: every world, in a backup of the library). */
export function noteExported(key: string | null) {
  try {
    if (key !== null) return localStorage.removeItem(PREFIX + key);
    for (let i = localStorage.length - 1; i >= 0; i--) {
      const k = localStorage.key(i);
      if (k?.startsWith(PREFIX)) localStorage.removeItem(k);
    }
  } catch {
    // (As above.)
  }
}
