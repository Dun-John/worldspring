// Entrances moved into, out of or between buildings (`Created.in_building`, `way`) since the edits
// were last seen: the host buildings' interiors (their cellars' ways down) and the sites underground
// (their ways back up) that play mode keeps are dropped, to be fetched again.

import type { WorldFile } from '../gen/protocol';
import { parseLoc } from './state';

/** What each created entrance's hosting was when last seen, per keeper (a play controller). */
const seen = new WeakMap<object, Map<string, string>>();

/** Drop from `interiors`, `entrances` and `places` (keys: site ids, site ids, locations) what an entrance's
 * new host changed: the old and the new building's, and every site underground's (a site's id needs the
 * created sites' first layout, not known here; such moves are rare). True if anything was dropped. */
export function forgetRehosted(keeper: object, world: WorldFile, interiors: Map<string, unknown>, entrances: Map<string, unknown>, places: Map<string, unknown>): boolean {
  const now = new Map<string, string>();
  for (const c of world.edits?.created ?? []) if (c.kind === 'entrance') now.set(c.id, c.removed ? '' : `${c.in_building ?? ''}|${c.in_building ? (c.way ?? '') : ''}`);
  const was = seen.get(keeper);
  seen.set(keeper, now);
  if (!was) return false;
  const hosts = new Set<string>();
  for (const id of new Set([...was.keys(), ...now.keys()])) {
    const [a, b] = [was.get(id) ?? '', now.get(id) ?? ''];
    if (a === b) continue;
    for (const s of [a, b]) if (s.split('|')[0]) hosts.add(s.split('|')[0]);
    hosts.add('u:');
  }
  if (!hosts.size) return false;
  const gone = (id: string) => hosts.has(id) || (hosts.has('u:') && id.startsWith('u:'));
  for (const id of [...interiors.keys()]) if (gone(id)) interiors.delete(id);
  for (const id of [...entrances.keys()]) if (gone(id)) entrances.delete(id);
  for (const loc of [...places.keys()]) {
    const id = parseLoc(loc).interior;
    if (id && gone(id)) places.delete(loc);
  }
  return true;
}
