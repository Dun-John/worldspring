// What a world's edits keep on places no longer on the map: set aside when the world was remade
// or upgraded (layout numbers from `PARKED_LAYOUT` up), taken away on purpose (`gone:`), or made
// anew by an upgrade (`was:`). Listed by the place they were on (Notes › Gazetteer › Set aside),
// each with Delete.
import type { Edits } from '../gen/protocol';
import { PARKED_LAYOUT } from '../sync/ops';
import { removedKind, setAside } from '../ui/gazetteer';

export type AsideField = 'renames' | 'notes' | 'hidden' | 'designs' | 'buildings' | 'towns' | 'npcs' | 'plots';

/** One thing kept: its field and key (an NPC's or plot's own id), and the place id it names. */
export interface AsideEntry {
  field: AsideField;
  key: string;
  id: string;
}

/** The place the entries were on: a `gone:`/`was:` id, or a layout set aside (`layout:<n>`). */
export interface AsideGroup {
  key: string;
  /** The `gone:`/`was:` id (named by `listedName`), else null: a layout set aside. */
  id: string | null;
  entries: AsideEntry[];
}

/** A layout-local id's prefix and layout number. */
const LOCAL = /^((?:[lr]:)?[bdtukw]):(\d+)(?::|$)/;

/** Whether an id names a place no longer on the map, with what was made on it kept. */
export const isAside = (id: string) => setAside(id) || removedKind(id) !== null;

/** The group an id set aside belongs to. */
function groupOf(id: string): string {
  const m = LOCAL.exec(id);
  return m ? `layout:${m[2]}` : id;
}

/** Everything `e` keeps on places no longer on the map, by place (in the order first met). */
export function asideList(e: Edits): AsideGroup[] {
  const groups = new Map<string, AsideGroup>();
  const add = (field: AsideField, key: string, id: string) => {
    const g = groupOf(id);
    let x = groups.get(g);
    if (!x) groups.set(g, (x = { key: g, id: g.startsWith('layout:') ? null : id, entries: [] }));
    x.entries.push({ field, key, id });
  };
  for (const k of Object.keys(e.renames ?? {})) if (isAside(k)) add('renames', k, k);
  for (const k of Object.keys(e.notes ?? {})) if (isAside(k)) add('notes', k, k);
  for (const k of e.hidden ?? []) if (isAside(k)) add('hidden', k, k);
  for (const k of Object.keys(e.designs ?? {})) if (isAside(k)) add('designs', k, k);
  for (const k of Object.keys(e.buildings ?? {})) if (isAside(k)) add('buildings', k, k);
  for (const k of Object.keys(e.towns ?? {})) if (Number(k) >= PARKED_LAYOUT) add('towns', k, `t:${k}`);
  for (const [k, n] of Object.entries(e.npcs ?? {})) if (n.location && isAside(n.location.id)) add('npcs', k, n.location.id);
  for (const [k, p] of Object.entries(e.plots ?? {})) for (const a of p.anchors ?? []) if (isAside(a)) add('plots', k, a);
  return [...groups.values()];
}

/** How many entries `e` keeps on places no longer on the map. */
export const asideCount = (e: Edits) => asideList(e).reduce((n, g) => n + g.entries.length, 0);

/** What kind of place a layout-local id is, in words. */
const LOCAL_KIND: Record<string, string> = {
  b: 'a building',
  d: 'a district',
  t: 'a tower',
  u: 'a site underground',
  k: 'a keep’s dungeons',
  w: 'the sewers',
  'l:b': 'a building’s level',
  'l:u': 'a level underground',
  'r:b': 'a room',
  'r:u': 'a chamber underground',
};

/** An entry in words ("Name of a building: The Rusty Anchor", "NPC: Mara"). */
export function asideWhat(x: AsideEntry, e: Edits): string {
  const m = LOCAL.exec(x.id);
  const sub = m ? /^([lr]:[bu])/.exec(x.id)?.[1] ?? m[1].replace(/^[lr]:/, '') : null;
  const where = sub ? ` ${x.field === 'notes' || x.field === 'hidden' ? 'on' : 'of'} ${LOCAL_KIND[sub] ?? 'a place'}` : '';
  switch (x.field) {
    case 'renames':
      return `Name${where}: ${e.renames![x.key]}`;
    case 'notes':
      return `Note${where}: ${e.notes![x.key].text || '(tags only)'}`;
    case 'hidden':
      return `Hidden${where}`;
    case 'designs':
      return x.id.startsWith('b:') ? 'Rooms of a building, designed' : 'A site underground, designed';
    case 'buildings':
      return 'A building changed';
    case 'towns':
      return 'Streets re-planned';
    case 'npcs':
      return `NPC here: ${e.npcs![x.key].name || 'unnamed'}`;
    case 'plots':
      return `Plot point here: ${e.plots![x.key].title || 'untitled'}`;
  }
}

/** `e` without `entries` (an NPC or plot point stays, without that place). */
export function forgetAside(e: Edits, entries: AsideEntry[]): Edits {
  const out: Edits = { ...e };
  const drop = <T>(r: Record<string, T> | undefined, keys: string[]) => {
    if (!r || !keys.length) return r;
    const o = { ...r };
    for (const k of keys) delete o[k];
    return o;
  };
  const keys = (f: AsideField) => entries.filter((x) => x.field === f).map((x) => x.key);
  out.renames = drop(e.renames, keys('renames'));
  out.notes = drop(e.notes, keys('notes'));
  out.designs = drop(e.designs, keys('designs'));
  out.buildings = drop(e.buildings, keys('buildings'));
  out.towns = drop(e.towns, keys('towns'));
  const hidden = new Set(keys('hidden'));
  if (hidden.size) out.hidden = (e.hidden ?? []).filter((h) => !hidden.has(h));
  const npcs = new Set(keys('npcs'));
  if (npcs.size)
    out.npcs = Object.fromEntries(
      Object.entries(e.npcs ?? {}).map(([k, n]) => {
        if (!npcs.has(k)) return [k, n];
        const { location: _, ...rest } = n;
        return [k, rest];
      }),
    );
  const plots = entries.filter((x) => x.field === 'plots');
  if (plots.length)
    out.plots = Object.fromEntries(
      Object.entries(e.plots ?? {}).map(([k, p]) => {
        const off = new Set(plots.filter((x) => x.key === k).map((x) => x.id));
        return [k, off.size ? { ...p, anchors: p.anchors.filter((a) => !off.has(a)) } : p];
      }),
    );
  return out;
}
