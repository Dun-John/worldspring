// A world's edits counted in words ("3 names, 1 site placed"), for the Library and the
// questions asked before edits are replaced.
import type { Edits } from '../gen/protocol';

export function changeList(e: Edits | undefined): string[] {
  e ??= {};
  const n = (v: object | undefined) => (v ? Object.keys(v).length : 0);
  const created = (e.created ?? []).filter((c) => !c.removed);
  // (A place taken away, or made anew by an upgrade, keeps its name as the label of what was made
  // on it: not a name given.)
  const gone = (k: string) => k.startsWith('gone:') || k.startsWith('was:');
  const removed = new Set(Object.keys(e.renames ?? {}).filter(gone));
  return (
    [
      [n(e.renames) - removed.size, 'name', 'names'],
      [removed.size, 'place no longer on the map (its notes kept)', 'places no longer on the map (their notes kept)'],
      [n(e.notes), 'note', 'notes'],
      [e.hidden?.length ?? 0, 'hidden place', 'hidden places'],
      [created.filter((c) => c.kind === 'building').length, 'building drawn', 'buildings drawn'],
      [created.filter((c) => c.kind !== 'building').length, 'site placed', 'sites placed'],
      [n(e.designs), 'site designed', 'sites designed'],
      [n(e.npcs), 'NPC', 'NPCs'],
      [n(e.plots), 'plot point', 'plot points'],
      [n(e.objects) + n(e.cleared), 'object put down or cleared', 'objects put down or cleared'],
      [n(e.sprites), 'uploaded sprite', 'uploaded sprites'],
      [n(e.crossings), 'crossing', 'crossings'],
      [n(e.buildings), 'building changed', 'buildings changed'],
      [n(e.towns), 'town re-planned', 'towns re-planned'],
      [n(e.ground), 'patch of ground shaped', 'patches of ground shaped'],
      [n(e.plateaus), 'plateau', 'plateaus'],
    ] as [number, string, string][]
  )
    .filter(([k]) => k > 0)
    .map(([k, one, many]) => `${k} ${k === 1 ? one : many}`);
}
