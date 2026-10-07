// A world's edits counted in words ("3 names, 1 site placed"), for the Library and the
// questions asked before edits are replaced.
import type { Edits } from '../gen/protocol';

export function changeList(e: Edits | undefined): string[] {
  e ??= {};
  const n = (v: object | undefined) => (v ? Object.keys(v).length : 0);
  const created = (e.created ?? []).filter((c) => !c.removed);
  return (
    [
      [n(e.renames), 'name', 'names'],
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
    ] as [number, string, string][]
  )
    .filter(([k]) => k > 0)
    .map(([k, one, many]) => `${k} ${k === 1 ? one : many}`);
}
