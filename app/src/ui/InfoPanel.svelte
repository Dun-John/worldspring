<script lang="ts">
  // Info panel for the selected feature or building; the name renames in place. Notes are
  // edited here; the NPCs and plot points at the place open in the notebook.
  import type { Npc, Plot } from '../gen/protocol';
  import { hitName, kindLabel, type Selection } from './gazetteer';
  import Icon from './Icon.svelte';

  type Note = { text: string; tags?: string[] };

  interface Props {
    selection: Selection;
    renames: Record<string, string>;
    /** The world's notes (lore, written by the user or agents), by id. */
    notes: Record<string, { text: string; tags?: string[] }>;
    settlementName: (index: number) => string;
    onRename: (id: string, name: string) => void;
    onFly: () => void;
    onClose: () => void;
    /** Go inside the selected building (resolves false if it can't be entered). */
    onEnter: () => Promise<boolean>;
    /** Set (or with null, clear) the place's notes. */
    onNote: (id: string, note: Note | null) => void;
    /** The NPCs and plot points here (in a settlement: also in its buildings and districts). */
    npcsHere: [string, Npc][];
    plotsHere: [string, Plot][];
    onOpen: (tab: 'npcs' | 'plots', id: string) => void;
    onAdd: (tab: 'npcs' | 'plots') => void;
    /** A created site: hide it from labels and search (or show it again), delete it, go down
     * into its site underground (if it has one). */
    hidden?: boolean;
    onHide?: (hide: boolean) => void;
    onDelete?: () => void;
    onDown?: () => void;
    /** A building drawn by hand: go inside it, change it in the build menu. */
    onGoIn?: () => void;
    onEdit?: () => void;
    /** One of the world's own buildings: take it away; put it back as generated (if changed). */
    onRemove?: () => void;
    onRestore?: () => void;
    /** In a phone's sheet (which has its own frame and close). */
    docked?: boolean;
    /** Only the name, what it is and the actions (the sheet is down to its strip). */
    peek?: boolean;
  }
  let { selection, renames, notes, settlementName, onRename, onFly, onClose, onEnter, onNote, npcsHere, plotsHere, onOpen, onAdd, hidden, onHide, onDelete, onDown, onGoIn, onEdit, onRemove, onRestore, docked = false, peek = false }: Props = $props();
  let cannot = $state(false);
  let confirmDelete = $state(false);
  /** The ⋯ menu (hide, delete). */
  let more = $state(false);

  async function enter() {
    cannot = !(await onEnter());
  }

  let editing = $state(false);
  let draft = $state('');

  const id = $derived(selection.kind === 'feature' ? selection.feature.id : selection.hit.id);
  const name = $derived(selection.kind === 'feature' ? (renames[selection.feature.id] ?? selection.feature.name) : hitName(selection.hit, renames));
  const note = $derived(notes[id]);
  /** What it is, on one line. */
  const kind = $derived(
    selection.kind === 'feature'
      ? kindLabel(selection.feature.kind)
      : selection.kind === 'district'
        ? `district${selection.hit.district_kind === 'residential' ? '' : ` · ${selection.hit.district_kind}`}`
        : `${selection.hit.function.toLowerCase()}${selection.hit.category ? ` · ${selection.hit.category}` : ''}`,
  );
  const mi = (ft: number) => (ft / 5280).toFixed(1);

  function startEdit() {
    draft = name;
    editing = true;
  }

  function commit() {
    const n = draft.trim();
    if (n && n !== name) onRename(id, n);
    editing = false;
  }

  function saveNote(text: string, tags: string[]) {
    if (!text.trim() && !tags.length) {
      if (note) onNote(id, null);
    } else if (text !== (note?.text ?? '') || JSON.stringify(tags) !== JSON.stringify(note?.tags ?? [])) {
      onNote(id, tags.length ? { text, tags } : { text });
    }
  }

  const tagList = (s: string) => [...new Set(s.split(',').map((t) => t.trim()).filter(Boolean))];

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter') commit();
    else if (e.key === 'Escape') editing = false;
  }
</script>

<aside class="info" class:docked aria-label="Selected place">
  <div class="head">
    {#if editing}
      <!-- svelte-ignore a11y_autofocus -->
      <input class="ws-input rename" bind:value={draft} onkeydown={key} onblur={commit} autofocus aria-label="New name" />
    {:else}
      <button class="name" title="Rename" onclick={startEdit}>{name}<span class="pen"><Icon name="pencil" size={13} /></span></button>
    {/if}
    {#if !docked}<button class="ws-icon-btn" onclick={onClose} aria-label="Close" title="Close (Esc)"><Icon name="x" /></button>{/if}
  </div>
  <div class="kind">{kind}</div>

  <div class="actions">
    <button class="ws-btn" onclick={onFly}><Icon name="target" size={16} /> Go to</button>
    {#if selection.kind === 'building'}<button class="ws-btn" onclick={enter}><Icon name="enter" size={16} /> Enter</button>{/if}
    {#if onGoIn && selection.kind === 'feature'}<button class="ws-btn" onclick={onGoIn}><Icon name="enter" size={16} /> Enter</button>{/if}
    {#if onDown}<button class="ws-btn" onclick={onDown} title="Into the site underground"><Icon name="stairs" size={16} /> Go down</button>{/if}
    {#if onEdit}<button class="ws-btn" onclick={onEdit} title="Change what it is, its storeys and roof, or draw it again"><Icon name="pencil" size={16} /> Edit</button>{/if}
    {#if onHide || onDelete || onRemove}
      <div class="more-wrap">
        <button class="ws-icon-btn" onclick={() => ((more = !more), (confirmDelete = false))} aria-label="More" aria-expanded={more}><Icon name="more" /></button>
        {#if more}
          <div class="menu ws-panel" role="menu">
            {#if onHide}<button class="ws-btn quiet" role="menuitem" onclick={() => ((more = false), onHide(!hidden))} title={hidden ? 'Show its label and list it in search again' : 'Hide its label and leave it out of search'}><Icon name={hidden ? 'eye' : 'eye-off'} size={16} /> {hidden ? 'Show on the map' : 'Hide from the map'}</button>{/if}
            {#if onDelete}<button class="ws-btn" class:quiet={!confirmDelete} class:danger={confirmDelete} role="menuitem" onclick={() => (confirmDelete ? onDelete() : (confirmDelete = true))} title="Remove this site from the world"><Icon name="trash" size={16} /> {confirmDelete ? 'Delete for good?' : 'Delete'}</button>{/if}
            {#if onRestore}<button class="ws-btn quiet" role="menuitem" onclick={() => ((more = false), onRestore())} title="Undo every change made to it: what it is, storeys, roof, footprint"><Icon name="reset" size={16} /> As generated</button>{/if}
            {#if onRemove}<button class="ws-btn quiet" role="menuitem" onclick={() => ((more = false), onRemove())} title="Take it away, leaving open ground to build on (Undo brings it back)"><Icon name="trash" size={16} /> Remove</button>{/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
  {#if cannot && selection.kind === 'building'}<div class="muted">There is no way in (open ground or ruins).</div>{/if}

  {#if !peek}
    <div class="facts">
      {#if selection.kind === 'feature'}
        {@const f = selection.feature}
        {#if f.detail}<div>{f.detail}</div>{/if}
        {#if f.elev_ft !== undefined}<div>Elevation {Math.round(f.elev_ft).toLocaleString()} ft</div>{/if}
        <div class="muted">{mi(f.x)} mi E, {mi(f.y)} mi S</div>
      {:else if selection.kind === 'district'}
        {@const d = selection.hit}
        <div>in {settlementName(d.settlement)}</div>
        <div class="muted">{mi(d.x)} mi E, {mi(d.y)} mi S</div>
      {:else}
        {@const b = selection.hit}
        <div>{b.floors} {b.floors === 1 ? 'storey' : 'storeys'} · {b.ward} ward</div>
        <div>in {b.district ? `${renames[b.district] ?? b.district}, ` : ''}{settlementName(b.settlement)}</div>
        <div class="muted">{mi(b.x)} mi E, {mi(b.y)} mi S</div>
      {/if}
    </div>
    <div class="note">
      <textarea class="ws-input" placeholder="Add a note: lore, hooks, secrets…" value={note?.text ?? ''} onchange={(e) => saveNote(e.currentTarget.value, note?.tags ?? [])} aria-label="Note"></textarea>
      {#if note?.text || note?.tags?.length}
        <input class="ws-input" placeholder="Tags, comma separated" value={(note?.tags ?? []).join(', ')} onchange={(e) => saveNote(note?.text ?? '', tagList(e.currentTarget.value))} aria-label="Tags" />
      {/if}
    </div>
    <div class="here">
      <div class="sub">
        <span class="ws-label grow">People &amp; plots here</span>
        <button class="ws-chip" onclick={() => onAdd('npcs')} title="A new NPC, placed here"><Icon name="plus" size={12} /> NPC</button>
        <button class="ws-chip" onclick={() => onAdd('plots')} title="A new plot point tied to this place"><Icon name="plus" size={12} /> Plot</button>
      </div>
      {#each npcsHere as [k, n] (k)}
        <button class="item" onclick={() => onOpen('npcs', k)}><Icon name="user" size={14} /><b>{n.name}</b> <span class="muted">{n.attitude.stance}{n.status ? ` · ${n.status}` : ''}</span></button>
      {/each}
      {#each plotsHere as [k, p] (k)}
        <button class="item" onclick={() => onOpen('plots', k)}><Icon name="scroll" size={14} /><b>{p.title}</b> <span class="muted">{p.status}</span></button>
      {/each}
      {#if !npcsHere.length && !plotsHere.length}<div class="muted">None yet.</div>{/if}
    </div>
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
    display: inline-flex;
    margin-left: 6px;
    color: var(--ink-3);
    opacity: 0;
    vertical-align: 1px;
  }
  .name:hover .pen,
  .name:focus-visible .pen {
    opacity: 1;
  }
  @media (hover: none) {
    .pen {
      opacity: 0.7;
    }
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
    text-transform: capitalize;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .more-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    left: 0;
    top: 100%;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    min-width: 200px;
    background: var(--paper-solid);
    box-shadow: var(--shadow-lg);
  }
  .menu .ws-btn {
    justify-content: flex-start;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: 1px;
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
  .note textarea {
    field-sizing: content;
    min-height: 3.2em;
    max-height: 40vh;
  }
  .sub {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .grow {
    flex: 1;
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
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item:hover,
  .item:focus-visible {
    background: var(--btn-hover);
  }
</style>
