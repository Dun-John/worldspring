// Generator versions on the public site: each published generator is kept as its own build
// (`<root>v<N>/`, listed in `<root>versions.json` by scripts/publish.mjs), so a world made with an
// older one can still be opened as it was made. The newest build is at the root.
import type { WorldFile } from '../gen/protocol';
import { keepLinked } from './library';
import { plainHash, toHash } from './world';

/** Where the newest build is (set by the publish script; else this build's own base). */
export const SITE_ROOT: string = import.meta.env.VITE_SITE_ROOT || import.meta.env.BASE_URL;
/** This is an older generator's build, kept for the worlds made with it. */
export const PINNED = import.meta.env.BASE_URL !== SITE_ROOT;

let kept: Promise<number[]> | null = null;

/** The generator versions the site keeps a build of (none in development or offline). */
export function keptVersions(): Promise<number[]> {
  return (kept ??= fetch(`${SITE_ROOT}versions.json`, { cache: 'no-cache' })
    .then((r) => (r.ok ? r.json() : null))
    .then((v: { kept?: unknown } | null) => (Array.isArray(v?.kept) ? v.kept.filter((n): n is number => Number.isInteger(n)) : []))
    .catch(() => []));
}

/** The first generator whose build reads deflated links and links to worlds kept in the
 * library (`#z=`, `#lib=`); older builds are sent plain ones. */
const SHORT_LINKS_GEN = 53;

/** The address of `w` in the build for generator `gen` (null: the newest), with `search` (`?…`). */
export async function buildUrl(w: WorldFile, gen: number | null, search = ''): Promise<string> {
  const hash = gen !== null && gen < SHORT_LINKS_GEN ? plainHash(w) : await toHash(w, keepLinked);
  return `${SITE_ROOT}${gen === null ? '' : `v${gen}/`}${search}${hash}`;
}
