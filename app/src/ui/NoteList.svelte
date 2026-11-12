<script lang="ts" module>
  /** A note as the list shows it: where it is (site › level › room, or the place's name). */
  export interface NoteRow {
    id: string;
    kind: 'place' | 'room' | 'point';
    note: { text: string; tags?: string[]; at?: [number, number] };
    path: string[];
  }
</script>

<script lang="ts">
  // The Notebook's Notes tab: every note (on places, rooms and points in rooms), found by tag (the
  // quick picks and the tags in use) and by text. A row opens its place: a room's or point's
  // inside its site, on its level.
  import Icon from './Icon.svelte';
  import { quickTag, QUICK_TAGS } from './notes';

  interface Props {
    rows: NoteRow[];
    onOpen: (id: string) => void;
  }
  let { rows, onOpen }: Props = $props();

  let q = $state('');
  let tag = $state<string | null>(null);

  const quick = QUICK_TAGS.map((t) => t.tag);
  /** The quick picks, then the other tags in use (most used first). */
  const tags = $derived.by(() => {
    const count = new Map<string, number>();
    const spelt = new Map<string, string>();
    for (const r of rows)
      for (const t of new Set((r.note.tags ?? []).map((x) => x.toLowerCase())))
        if (!quick.includes(t)) count.set(t, (count.get(t) ?? 0) + 1);
    for (const r of rows) for (const t of r.note.tags ?? []) if (!spelt.has(t.toLowerCase())) spelt.set(t.toLowerCase(), t);
    return [...count.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).map(([t]) => spelt.get(t) ?? t);
  });
  /** The filter in force: one whose tag is no longer on any note (or a quick pick) is off. */
  const active = $derived(tag && (quick.includes(tag) || tags.some((t) => t.toLowerCase() === tag)) ? tag : null);
  const KIND_ORDER = { place: 0, room: 1, point: 2 };
  const shown = $derived.by(() => {
    const t = q.trim().toLowerCase();
    return rows
      .filter((r) => (!active || r.note.tags?.some((x) => x.toLowerCase() === active)) && (!t || [r.note.text, ...r.path, ...(r.note.tags ?? [])].some((s) => s.toLowerCase().includes(t))))
      .sort((a, b) => a.path.join(' ').localeCompare(b.path.join(' ')) || KIND_ORDER[a.kind] - KIND_ORDER[b.kind]);
  });
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const iconOf = (r: NoteRow) => QUICK_TAGS.find((x) => x.tag === quickTag(r.note.tags))?.icon ?? (r.kind === 'place' ? 'pin' : r.kind === 'room' ? 'room' : 'note');
  const firstLine = (r: NoteRow) => r.note.text.split('\n')[0].slice(0, 120) || (r.note.tags ?? []).join(', ') || 'Empty note';
</script>

<div class="notes">
  <input class="ws-input" type="search" placeholder="Find notes…" bind:value={q} aria-label="Find notes" />
  <div class="chips" role="group" aria-label="Tags">
    {#each QUICK_TAGS as t (t.tag)}
      <button class="ws-chip" aria-pressed={active === t.tag} onclick={() => (tag = active === t.tag ? null : t.tag)}><Icon name={t.icon} size={12} /> {t.label}</button>
    {/each}
    {#each tags as t (t)}
      <button class="ws-chip" aria-pressed={active === t.toLowerCase()} onclick={() => (tag = active === t.toLowerCase() ? null : t.toLowerCase())}><Icon name="tag" size={12} /> {t}</button>
    {/each}
  </div>
  <ul>
    {#each shown as r (r.id)}
      <li>
        <button class="row" onclick={() => onOpen(r.id)} title="Go there">
          <Icon name={iconOf(r)} size={16} />
          <span class="text">
            <span class="where">{r.path.map(cap).join(' › ')}</span>
            <span class="ws-muted">{firstLine(r)}</span>
          </span>
        </button>
      </li>
    {:else}
      <li class="empty">{rows.length ? 'No notes match.' : 'No notes yet. Write one on a place’s card, or inside a building or site on a room (click it) or at a point (right-click or long-press).'}</li>
    {/each}
  </ul>
</div>

<style>
  .notes {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: calc(var(--tap) + 8px);
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--btn-hover);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .text > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .where {
    font-weight: bold;
  }
  .empty {
    padding: 8px 4px;
    font-style: italic;
    color: var(--ink-3);
  }
</style>
