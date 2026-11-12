// Notes on places, rooms (`r:<site>:<level>:<room>`) and points in rooms (`m:<site>:<level>:<id>`,
// the note's `at` in the location's own grid units). One kind of note: text and free tags, of
// which a few are quick picks. Always the DM's: none of it goes to the players' window.
import type { Note } from '../gen/protocol';
import type { IconName } from './icons';

/** The quick-pick tags (the order the chips show in). */
export const QUICK_TAGS: { tag: string; label: string; icon: IconName }[] = [
  { tag: 'treasure', label: 'Treasure', icon: 'chest' },
  { tag: 'read-aloud', label: 'Read aloud', icon: 'chat' },
  { tag: 'trap', label: 'Trap', icon: 'trap' },
  { tag: 'secret', label: 'Secret', icon: 'keyhole' },
  { tag: 'monster', label: 'Monster', icon: 'skull' },
];

const QUICK = new Set(QUICK_TAGS.map((q) => q.tag));

/** A note's tags in one form: quick picks lower-case as listed, the others deduped ignoring case (first spelling kept). */
export function canonTags(tags: string[]): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const raw of tags) {
    const t = raw.trim();
    const k = t.toLowerCase();
    if (!t || seen.has(k)) continue;
    seen.add(k);
    out.push(QUICK.has(k) ? k : t);
  }
  return out;
}

/** The note's own first quick-pick tag (its marker's glyph), in the note's order, else null. */
export function quickTag(tags: string[] | undefined): string | null {
  for (const t of tags ?? []) {
    const k = t.toLowerCase();
    if (QUICK.has(k)) return k;
  }
  return null;
}

/** A point note's id split: its site, level and own id. */
export function splitPoint(id: string): { site: string; level: number; own: string } | null {
  const m = /^m:(.+):(\d+):([^:]+)$/.exec(id);
  return m ? { site: m[1], level: Number(m[2]), own: m[3] } : null;
}

/** A room's id split: its site, level and room. */
export function splitRoom(id: string): { site: string; level: number; room: number } | null {
  const m = /^r:(.+):(\d+):(\d+)$/.exec(id);
  return m ? { site: m[1], level: Number(m[2]), room: Number(m[3]) } : null;
}

/** A new point note's id: random, as NPCs' and plots' (`newNoteId`). */
export function newPointId(site: string, level: number): string {
  const b = new Uint8Array(8);
  crypto.getRandomValues(b);
  return `m:${site}:${level}:${[...b].map((x) => '0123456789abcdefghijklmnopqrstuvwxyz'[x % 36]).join('')}`;
}

/** A point note marker inside, for the DM (`InteriorLayer.notes`). */
export interface NoteMarker {
  id: string;
  site: string;
  level: number;
  at: [number, number];
  /** Its glyph: the first quick-pick tag, else null (a plain note). */
  glyph: string | null;
}

/** The point notes in `notes` as markers. */
export function noteMarkers(notes: Record<string, Note> | undefined): NoteMarker[] {
  const out: NoteMarker[] = [];
  for (const [id, n] of Object.entries(notes ?? {})) {
    const p = n.at && splitPoint(id);
    if (p) out.push({ id, site: p.site, level: p.level, at: n.at!, glyph: quickTag(n.tags) });
  }
  return out;
}
