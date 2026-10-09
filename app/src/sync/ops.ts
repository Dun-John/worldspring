// Edit ops: one entry of one edits field set or removed (`worldgen::world::EditOp`). The app
// and mapd exchange these rather than whole edits, so changes made at the same time to
// different entries (the user's here, an agent's there) all stay; undo replays inverse ops.
import type { Edits } from '../gen/protocol';

export type EditOp = { op: 'set'; field: string; key: string; value: unknown } | { op: 'unset'; field: string; key: string };

/** How a field's entries are keyed (mirrors `EDIT_FIELDS` in crates/worldgen/src/world.rs):
 * `map` an object by id, `set` a sorted list of ids, `list` entries whose ids are
 * `<prefix>:<index>` (never shortened; removing marks an entry `removed`). */
type Shape = 'map' | 'set' | 'list';
export const EDIT_FIELDS: Record<string, Shape> = {
  renames: 'map',
  notes: 'map',
  hidden: 'set',
  created: 'list',
  npcs: 'map',
  plots: 'map',
  objects: 'map',
  cleared: 'map',
  sprites: 'map',
  designs: 'map',
  crossings: 'map',
  buildings: 'map',
};

const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);

function entries(e: Edits, field: string): [string, unknown][] {
  const v = (e as Record<string, unknown>)[field];
  switch (EDIT_FIELDS[field]) {
    case 'map':
      return v && typeof v === 'object' ? Object.entries(v) : [];
    case 'set':
      return Array.isArray(v) ? v.map((k) => [String(k), true]) : [];
    case 'list':
      return Array.isArray(v) ? v.map((x, i) => [String((x as { id?: string })?.id ?? `c:${i}`), x]) : [];
    default:
      return [];
  }
}

function put(e: Edits, field: string, list: [string, unknown][]): Edits {
  const shape = EDIT_FIELDS[field];
  const v = shape === 'map' ? Object.fromEntries(list) : shape === 'set' ? list.map(([k]) => k).sort() : list.map(([, x]) => x);
  return { ...e, [field]: v };
}

/** The ops that turn `a` into `b`. */
export function diffEdits(a: Edits, b: Edits): EditOp[] {
  const ops: EditOp[] = [];
  for (const field of Object.keys(EDIT_FIELDS)) {
    // (Edits are never changed in place: the same field object is the same field.)
    if ((a as Record<string, unknown>)[field] === (b as Record<string, unknown>)[field]) continue;
    // Keyed fields: their objects walked as they are.
    if (EDIT_FIELDS[field] === 'map') {
      const ma = ((a as Record<string, unknown>)[field] ?? {}) as Record<string, unknown>;
      const mb = ((b as Record<string, unknown>)[field] ?? {}) as Record<string, unknown>;
      for (const key in mb) if (ma[key] !== mb[key] && !same(ma[key], mb[key])) ops.push({ op: 'set', field, key, value: mb[key] });
      for (const key in ma) if (!(key in mb)) ops.push({ op: 'unset', field, key });
      continue;
    }
    const ea = new Map(entries(a, field));
    const eb = new Map(entries(b, field));
    for (const key of new Set([...ea.keys(), ...eb.keys()])) {
      const [va, vb] = [ea.get(key), eb.get(key)];
      // (Edits are never changed in place: the same object is the same entry.)
      if (va === vb || same(va, vb)) continue;
      ops.push(vb === undefined ? { op: 'unset', field, key } : { op: 'set', field, key, value: vb });
    }
  }
  return ops;
}

/** `next` replacing `before` whole, keeping listed entries' places (later ids are indices):
 * the entries `next` lacks stay, marked removed, as the ops that make it leave them wherever
 * they are applied (so the edits here and elsewhere stay alike; `world.rs` `keeping_places`). */
export function keepPlaces(before: Edits, next: Edits): Edits {
  let out = next;
  for (const field of Object.keys(EDIT_FIELDS)) {
    if (EDIT_FIELDS[field] !== 'list') continue;
    const was = (before as Record<string, unknown>)[field] as { removed?: boolean }[] | undefined;
    const now = ((next as Record<string, unknown>)[field] ?? []) as { removed?: boolean }[];
    if (!was || was.length <= now.length) continue;
    out = { ...out, [field]: [...now, ...was.slice(now.length).map((x) => (x.removed ? x : { ...x, removed: true }))] };
  }
  return out;
}

/** Site `site`'s levels renumbered (they count from the bottom) after `delta` levels were dug
 * below its deepest (below 0: filled in): names, notes and hidden marks of its levels and rooms,
 * plots' anchors on them and NPCs inside follow their levels; those of levels filled in go
 * (`world.rs` `shift_levels`). */
export function shiftLevels(e: Edits, site: string, delta: number): Edits {
  if (!delta) return e;
  // The new id ('' when its level is gone), or undefined when not one of the site's levels.
  // (`l:<site>:<level>`, `r:<site>:<level>:<room>`: the site is everything between.)
  const moved = (id: string): string | undefined => {
    const room = id.startsWith('r:');
    if (!room && !id.startsWith('l:')) return undefined;
    const parts = id.slice(2).split(':');
    const ri = room ? parts.pop() : undefined;
    const lv = parts.pop();
    if (parts.join(':') !== site || !/^\d+$/.test(lv ?? '') || (room && !/^\d+$/.test(ri ?? ''))) return undefined;
    const li = Number(lv) + delta;
    return li < 0 ? '' : room ? `r:${site}:${li}:${ri}` : `l:${site}:${li}`;
  };
  const rekey = <V>(m: Record<string, V> | undefined): Record<string, V> | undefined => {
    if (!m || !Object.keys(m).some((k) => moved(k) !== undefined)) return m;
    const out: Record<string, V> = {};
    for (const [k, v] of Object.entries(m)) if (moved(k) === undefined) out[k] = v;
    for (const [k, v] of Object.entries(m)) if (moved(k)) out[moved(k)!] = v;
    return out;
  };
  const out: Edits = { ...e };
  const [renames, notes] = [rekey(e.renames), rekey(e.notes)];
  if (renames !== e.renames) out.renames = renames;
  if (notes !== e.notes) out.notes = notes;
  if (e.hidden?.some((k) => moved(k) !== undefined)) out.hidden = [...new Set(e.hidden.map((k) => moved(k) ?? k).filter(Boolean))].sort();
  if (e.plots && Object.values(e.plots).some((p) => p.anchors.some((a) => moved(a) !== undefined))) {
    out.plots = Object.fromEntries(Object.entries(e.plots).map(([k, p]) => [k, p.anchors.some((a) => moved(a) !== undefined) ? { ...p, anchors: p.anchors.map((a) => moved(a) ?? a).filter(Boolean) } : p]));
  }
  if (e.npcs && Object.values(e.npcs).some((n) => n.location?.id === site && n.location.level !== undefined)) {
    out.npcs = Object.fromEntries(Object.entries(e.npcs).map(([k, n]) => [k, n.location?.id === site && n.location.level !== undefined ? { ...n, location: { ...n.location, level: Math.max(0, n.location.level + delta) } } : n]));
  }
  return out;
}

/** `e` with `op` applied, and the op that undoes it. */
export function applyOp(e: Edits, op: EditOp): { edits: Edits; inverse: EditOp } {
  const shape = EDIT_FIELDS[op.field];
  if (!shape) throw new Error(`no such edits field: ${op.field}`);
  const list = entries(e, op.field);
  const at = list.findIndex(([k]) => k === op.key);
  const before = at >= 0 ? list[at][1] : undefined;
  if (op.op === 'set') {
    const value = structuredClone(op.value);
    if (shape === 'list') {
      const i = Number(op.key.split(':').at(-1));
      if (!Number.isInteger(i) || i > list.length) throw new Error(`${op.key}: ${op.field} has only ${list.length} entries`);
      if (i === list.length) list.push([op.key, value]);
      else list[i] = [op.key, value];
    } else if (at >= 0) list[at] = [op.key, value];
    else list.push([op.key, value]);
  } else if (shape === 'list') {
    if (at >= 0) list[at] = [op.key, { ...(list[at][1] as object), removed: true }];
  } else if (at >= 0) list.splice(at, 1);
  const inverse: EditOp = before === undefined ? { op: 'unset', field: op.field, key: op.key } : { op: 'set', field: op.field, key: op.key, value: before };
  return { edits: put(e, op.field, list), inverse };
}

/** `e` with `ops` applied in order, and the ops that undo them (in the order to apply). */
export function applyOps(e: Edits, ops: EditOp[], onError?: (op: EditOp, err: unknown) => void): { edits: Edits; inverse: EditOp[] } {
  const inverse: EditOp[] = [];
  // Keyed (`map`) fields are copied once for the whole batch and changed in the copy (a brush
  // stroke's undo is dozens of ops on a field that may hold tens of thousands of entries);
  // the others go op by op through `applyOp`. With `onError`, an op that can't apply is
  // reported and left out; without, it throws.
  const copies = new Map<string, Record<string, unknown>>();
  for (const op of ops) {
    try {
      if (EDIT_FIELDS[op.field] !== 'map') {
        const r = applyOp(e, op);
        e = r.edits;
        inverse.unshift(r.inverse);
        continue;
      }
      let m = copies.get(op.field);
      if (!m) {
        const v = (e as Record<string, unknown>)[op.field];
        m = { ...(v && typeof v === 'object' ? (v as Record<string, unknown>) : {}) };
        copies.set(op.field, m);
        e = { ...e, [op.field]: m };
      }
      const before = op.key in m ? m[op.key] : undefined;
      if (op.op === 'set') m[op.key] = structuredClone(op.value);
      else delete m[op.key];
      inverse.unshift(before === undefined ? { op: 'unset', field: op.field, key: op.key } : { op: 'set', field: op.field, key: op.key, value: before });
    } catch (err) {
      if (!onError) throw err;
      onError(op, err);
    }
  }
  return { edits: e, inverse };
}

/** The keys each field's ops touch (what changed, for those that only need to know which). */
export function changedKeys(ops: EditOp[]): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const op of ops) (out[op.field] ??= []).push(op.key);
  return out;
}
