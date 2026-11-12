<script lang="ts">
  // A room's card or a point note's, inside a building or site (the DM's alone): where it is,
  // its note, and for a room the notes put down in it and a new one; a point's moved or deleted.
  import Icon from './Icon.svelte';
  import NoteEditor from './NoteEditor.svelte';
  import { quickTag, QUICK_TAGS } from './notes';

  type Note = { text: string; tags?: string[]; at?: [number, number] };

  interface Props {
    /** `r:<site>:<level>:<room>` or `m:<site>:<level>:<id>`. */
    id: string;
    /** Where it is: the site, its level, the room (null while not known, or a point outside any). */
    path: { site: string; level: string; room: string | null } | null;
    note?: Note;
    /** A room: the point notes in it. */
    points?: [string, Note][];
    onSave: (note: Note | null) => void;
    /** A room: rename it (an empty name: back to the generated one). */
    onRename?: (name: string) => void;
    /** A room: put a note down at a point in it (a click). */
    onAddPoint?: () => void;
    /** A point: move it (a click), take it away. */
    onMove?: () => void;
    onDelete?: () => void;
    onOpen: (id: string) => void;
    onGo: () => void;
    onClose: () => void;
    docked?: boolean;
    peek?: boolean;
  }
  let { id, path, note, points = [], onSave, onRename, onAddPoint, onMove, onDelete, onOpen, onGo, onClose, docked = false, peek = false }: Props = $props();

  const point = $derived(id.startsWith('m:'));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  const title = $derived(cap(point ? (path?.room ? `note in ${path.room}` : 'note') : (path?.room ?? 'room')));
  let editing = $state(false);
  let draft = $state('');
  let confirmDelete = $state(false);
  $effect(() => {
    void id;
    editing = confirmDelete = false;
  });

  function commit() {
    const n = draft.trim();
    if (n && n !== path?.room) onRename?.(n);
    editing = false;
  }
  function save(text: string, tags: string[]) {
    // (A point keeps its spot, its note emptied or not: Delete takes it away.)
    if (!point && !text.trim() && !tags.length) return note && onSave(null);
    if (text === (note?.text ?? '') && JSON.stringify(tags) === JSON.stringify(note?.tags ?? [])) return;
    onSave({ text, ...(tags.length ? { tags } : {}), ...(note?.at ? { at: note.at } : {}) });
  }
  const iconOf = (n: Note) => QUICK_TAGS.find((q) => q.tag === quickTag(n.tags))?.icon ?? 'note';
  const firstLine = (n: Note) => n.text.split('\n')[0].slice(0, 80) || (n.tags ?? []).join(', ') || 'Empty note';
</script>

<aside class="info" class:docked aria-label={point ? 'Note' : 'Room'}>
  <div class="head">
    {#if editing}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="ws-input rename" bind:value={draft} onkeydown={(e) => (e.key === 'Enter' ? commit() : e.key === 'Escape' && (editing = false))} onblur={commit} autofocus aria-label="New name" />
    {:else if onRename}
      <button class="name" title="Rename" onclick={() => ((draft = path?.room ?? ''), (editing = true))}>{title}<span class="pen"><Icon name="pencil" size={13} /></span></button>
    {:else}
      <span class="name">{title}</span>
    {/if}
    {#if !docked}<button class="ws-icon-btn" onclick={onClose} aria-label="Close" title="Close (Esc)"><Icon name="x" /></button>{/if}
  </div>
  <div class="kind">{point ? 'a note at a point' : 'room'}{path ? ` · ${path.level}` : ''}</div>
  <div class="actions">
    <button class="ws-btn" onclick={onGo}><Icon name="target" size={16} /> Go to</button>
    {#if onAddPoint}<button class="ws-btn" onclick={onAddPoint} title="Then click where in the room"><Icon name="pin" size={16} /> Add a note at a point</button>{/if}
    {#if onMove}<button class="ws-btn" onclick={onMove} title="Then click where it goes"><Icon name="move" size={16} /> Move</button>{/if}
    {#if onDelete}<button class="ws-btn" class:danger={confirmDelete} onclick={() => (confirmDelete ? onDelete() : (confirmDelete = true))} onblur={() => (confirmDelete = false)} title="Take the note away"><Icon name="trash" size={16} /> {confirmDelete ? 'Delete it?' : 'Delete'}</button>{/if}
  </div>
  {#if !peek}
    {#if path}<div class="muted">in {cap(path.site)}</div>{/if}
    <div class="note">
      <NoteEditor {note} onSave={save} placeholder={point ? 'What is here: treasure, a trap, words to read aloud…' : 'Notes on the room: who, what, read-aloud…'} />
    </div>
    {#if !point}
      <div class="here">
        <span class="ws-label">Notes at points here ({points.length})</span>
        {#each points as [k, n] (k)}
          <button class="item" onclick={() => onOpen(k)}><Icon name={iconOf(n)} size={14} /><span class="line">{firstLine(n)}</span></button>
        {/each}
        {#if !points.length}<div class="muted">None yet.</div>{/if}
      </div>
    {/if}
  {/if}
</aside>

<style>
  .info {
    position: fixed;
    left: 12px;
    top: calc(12px + var(--top-h, 44px) + 8px);
    width: 300px;
    box-sizing: border-box;
    max-height: calc(100dvh - 12px - var(--top-h, 44px) - 8px - var(--readout-h, 40px) - 24px);
    overflow-y: auto;
    z-index: var(--z-card);
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px 10px 10px;
    color: var(--ink);
    font: 13px/1.45 var(--font);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .info.docked {
    position: static;
    width: auto;
    max-height: none;
    overflow: visible;
    padding: 0;
    background: none;
    border: none;
    box-shadow: none;
  }
  .head {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .name {
    all: unset;
    flex: 1;
    font-size: 17px;
    font-weight: bold;
    line-height: 1.25;
    cursor: text;
  }
  .pen {
    margin-left: 6px;
    color: var(--ink-3);
    opacity: 0.6;
  }
  .rename {
    flex: 1;
    font-weight: bold;
    font-size: 15px;
  }
  .kind {
    margin-top: -4px;
    font-style: italic;
    color: var(--ink-2);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .muted {
    color: var(--ink-3);
    font-size: 12px;
  }
  .note,
  .here {
    padding-top: 6px;
    border-top: 1px solid var(--line-faint);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .item {
    all: unset;
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: var(--tap);
    cursor: pointer;
    padding: 0 4px;
    border-radius: var(--radius-sm);
  }
  .item:hover,
  .item:focus-visible {
    background: var(--btn-hover);
  }
  .line {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
