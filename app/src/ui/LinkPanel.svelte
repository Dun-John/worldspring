<script lang="ts">
  // Edit › Rooms › Ways across: paired exits between two squares below ground (levels of one site,
  // a site and the sewers, the sewers and a cellar). Pick the first end on the map, go to the
  // other (the floor picker, or search for a place), pick its square; the way and name are chosen
  // here. The ways across with an end in the place in view are listed: pick one to change its way
  // or name, or take it away.
  import { LINK_WAYS, type LinkEdit } from '../gen/protocol';
  import Icon from './Icon.svelte';
  import { WAY_ICONS } from './icons';

  interface Props {
    /** The place and level in view ("" on the surface). */
    here: string;
    /** The first end put down, named, if any. */
    first: string | null;
    way: string;
    name: string;
    /** The ways across with an end in the place in view, their ends named. */
    links: { id: string; link: LinkEdit; a: string; b: string }[];
    picked: string | null;
    /** Why the last one put down left an end off, if it did. */
    note: string | null;
    peek?: boolean;
    onPick: (id: string | null) => void;
    onChange: (id: string, patch: { way?: string; name?: string }) => void;
    onRemove: (id: string) => void;
    onDrop: () => void;
    /** Back to designing the rooms, if this place can be designed. */
    onRooms?: () => void;
  }

  let { here, first, way = $bindable(), name = $bindable(), links, picked, note, peek = false, onPick, onChange, onRemove, onDrop, onRooms }: Props = $props();

  const chosen = $derived(links.find((l) => l.id === picked) ?? null);
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
</script>

<div class="links">
  <div class="ws-row">
    {#if onRooms}<button class="ws-btn quiet" onclick={onRooms} title="Back to the rooms (J)"><Icon name="room" size={16} /> Rooms</button>{/if}
    <b class="grow">Ways across</b>
  </div>
  <div class="ws-seg tight" role="radiogroup" aria-label="Way">
    {#each LINK_WAYS as w (w)}
      <button class:on={(chosen?.link.way ?? way) === w} role="radio" aria-checked={(chosen?.link.way ?? way) === w} title={cap(w)} onclick={() => (chosen ? onChange(chosen.id, { way: w }) : (way = w))}><Icon name={WAY_ICONS[w]} size={16} /><span class="w">{cap(w)}</span></button>
    {/each}
  </div>
  {#if !peek}
    {#if chosen}
      {@const c = chosen}
      <section class="card">
        <div class="ws-label">Chosen</div>
        <div><b>{c.a}</b> ↔ <b>{c.b}</b></div>
        <label class="ws-field">
          Name <span class="ws-muted">(else it goes by its way)</span>
          <input class="ws-input" value={c.link.name ?? ''} placeholder={cap(c.link.way)} onchange={(e) => onChange(c.id, { name: e.currentTarget.value.trim() })} />
        </label>
        <div class="ws-row">
          <button class="ws-btn quiet" onclick={() => onPick(null)}>Done</button>
          <span class="grow"></span>
          <button class="ws-btn danger" onclick={() => onRemove(c.id)} title="Take it away (Delete)"><Icon name="trash" size={16} /> Take away</button>
        </div>
      </section>
    {:else}
      <label class="ws-field">
        Name <span class="ws-muted">(optional)</span>
        <input class="ws-input" bind:value={name} placeholder={cap(way)} />
      </label>
      {#if first}
        <div class="ws-row step">
          <span class="grow">From <b>{first}</b>: now pick the other end. Go to another level with the floor picker, or to another place by search.</span>
          <button class="ws-btn quiet" onclick={onDrop} title="Drop the first end (Esc)">Cancel</button>
        </div>
      {:else}
        <div class="ws-hint">Pick a floor square for one end{here ? ` (here: ${here})` : ''}, then a square for the other: on another level, in another site, the sewers or a cellar. Furniture on the squares gives way.</div>
      {/if}
    {/if}
    {#if note}<div class="ws-hint warn">{note}</div>{/if}
    {#if links.length}
      <div class="ws-label">Here</div>
      <ul class="list">
        {#each links as l (l.id)}
          <li>
            <button class="ws-btn item" class:on={l.id === picked} onclick={() => onPick(l.id === picked ? null : l.id)}><Icon name={WAY_ICONS[l.link.way] ?? 'link'} size={16} /><span class="grow">{l.link.name || cap(l.link.way)}: {l.a} ↔ {l.b}</span></button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .links {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .tight > button {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 4px 2px;
    font-size: 11px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item {
    width: 100%;
    height: auto;
    min-height: var(--tap);
    justify-content: flex-start;
    text-align: left;
    white-space: normal;
    gap: 6px;
  }
  .item .grow {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .item.on {
    background: var(--btn-on);
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
  }
  .step {
    align-items: flex-start;
  }
  .warn {
    color: var(--warn);
  }
</style>
