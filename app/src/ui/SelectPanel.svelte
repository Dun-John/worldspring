<script lang="ts">
  // Edit › Build › Select: what a click found (the top one open: its kind, name and town, Delete
  // and Change), or what a box or lasso holds, counted by kind, with "Delete all that can be".
  import type { SelectTools } from '../editor/tools/select.svelte';
  import { KIND_WORDS, foldedWords } from '../editor/select';
  import type { Thing } from '../gen/protocol';
  import Icon from './Icon.svelte';
  import ToolPanel from './build/ToolPanel.svelte';

  interface Props {
    sel: SelectTools;
    peek?: boolean;
  }

  let { sel, peek = false }: Props = $props();

  const word = (t: Thing) => {
    const w = KIND_WORDS[t.kind][0];
    return w.charAt(0).toUpperCase() + w.slice(1);
  };
  const can = (t: Thing) => t.removable && !!t.id;
  const all = $derived(sel.things.filter(can));
  const joinable = $derived(sel.things.filter(sel.memberable));
  const leavable = $derived(sel.things.filter(sel.leavable));
  const num = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  /** A click's list shows this many rows; the rest are folded by kind until asked for. */
  const ROWS = 8;
  let unfolded = $state(false);
  $effect(() => {
    void sel.things;
    unfolded = false;
  });
  const limit = $derived(unfolded || sel.open >= ROWS ? Infinity : ROWS);
  const rest = $derived(sel.things.slice(limit === Infinity ? sel.things.length : ROWS));
</script>

{#snippet picker()}
  {#if sel.choosing}
    <div class="ws-muted">Join which town?</div>
    <div class="actions">
      {#each sel.choosing.options as o (o.id)}
        <button class="ws-btn" onclick={() => sel.join(sel.choosing!.list, o)}>{o.name}</button>
      {/each}
      <button class="ws-btn" onclick={() => sel.cancelChoice()}>Cancel</button>
    </div>
  {/if}
  {#if sel.notice}<div class="ws-muted" role="status">{sel.notice}</div>{/if}
{/snippet}

<ToolPanel title="Select" near={true} {peek} onZoomIn={() => {}}>
  {#snippet hint()}
    {#if !sel.things.length}
      <div class="ws-hint">
        {sel.area ? 'Nothing there to choose.' : 'Click something on the map to see what it is. Drag a box, or a lasso (Shift-drag), to choose all inside it.'}
      </div>
    {/if}
    <div class="ws-seg" role="radiogroup" aria-label="Drag draws">
      <button class:on={!sel.lasso} role="radio" aria-checked={!sel.lasso} onclick={() => (sel.lasso = false)}>Box</button>
      <button class:on={sel.lasso} role="radio" aria-checked={sel.lasso} onclick={() => (sel.lasso = true)}>Lasso</button>
    </div>
  {/snippet}
  {#snippet peekHint()}
    {#if sel.things.length}<div class="ws-hint">{sel.area ? `${sel.things.length} chosen` : word(sel.things[sel.open] ?? sel.things[0])}</div>{/if}
  {/snippet}
  {#snippet more()}
    {#if sel.area && sel.things.length}
      <div class="sel-list" aria-label="What is inside">
        {#each sel.counts as c (c.kind)}
          <div class="row">
            <span class="grow">{c.words}</span>
            {#if c.can < c.n}<span class="ws-muted">{c.can ? `${c.can} can be removed` : "can't be removed yet"}</span>{/if}
          </div>
        {/each}
      </div>
      {#if joinable.length}
        <div class="actions">
          <button class="ws-btn" disabled={sel.busy} onclick={() => sel.join(joinable)}>Join town ({joinable.length})</button>
          <button class="ws-btn" disabled={sel.busy || !leavable.length} onclick={() => sel.leave(leavable)}>Leave town{leavable.length ? ` (${leavable.length})` : ''}</button>
        </div>
        {@render picker()}
        <div class="ws-muted">{num(joinable.length, 'thing here can', 'things here can')} be part of a town.</div>
      {/if}
      <div class="actions">
        <button class="ws-btn danger" disabled={!all.length || sel.busy} onclick={() => void sel.remove(all)}><Icon name="trash" size={16} /> Delete all that can be{all.length ? ` (${all.length})` : ''}</button>
        <button class="ws-btn" onclick={() => sel.clear()}>Clear</button>
      </div>
    {:else if sel.things.length}
      <div class="sel-list" role="list" aria-label="What is here">
        {#each sel.things.slice(0, limit) as t, k (k)}
          {@const town = sel.townName(t)}
          <div class="item" class:open={k === sel.open} role="listitem">
            <button class="head" aria-expanded={k === sel.open} onclick={() => (sel.open = k)}>
              <span class="kind">{word(t)}</span>
              <span class="grow name">{t.name ?? ''}</span>
            </button>
            {#if k === sel.open}
              {@const part = sel.partOf(t)}
              <div class="body">
                {#if town}<div class="ws-muted">in {town}</div>{/if}
                {#if part}<div class="ws-muted">{part}</div>{/if}
                {#if sel.memberable(t)}
                  <div class="actions">
                    <button class="ws-btn" disabled={sel.busy} onclick={() => sel.join([t])}>Join town</button>
                    {#if sel.leavable(t)}<button class="ws-btn" disabled={sel.busy} onclick={() => sel.leave([t])}>Leave town</button>{/if}
                  </div>
                  {@render picker()}
                {/if}
                <div class="actions">
                  {#if can(t)}
                    <button class="ws-btn danger" disabled={sel.busy} onclick={() => void sel.remove([t])}><Icon name="trash" size={16} /> Delete</button>
                  {:else}
                    <span class="ws-muted">can't be removed yet</span>
                  {/if}
                  {#if sel.changeable(t)}<button class="ws-btn" onclick={() => void sel.change(t)}><Icon name="pencil" size={16} /> Change</button>{/if}
                </div>
              </div>
            {/if}
          </div>
        {/each}
        {#if rest.length}
          <button class="head more" onclick={() => (unfolded = true)}><span class="grow">{rest.length} more: {foldedWords(rest)}</span><span class="ws-muted">Show</span></button>
        {/if}
      </div>
    {/if}
  {/snippet}
</ToolPanel>

<style>
  .sel-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 320px;
    overflow-y: auto;
  }
  .row,
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    min-width: 0;
  }
  .head {
    width: 100%;
    border: none;
    background: transparent;
    font: inherit;
    color: inherit;
    text-align: left;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .head:hover {
    background: var(--btn-hover);
  }
  .item.open {
    background: #efe2c2;
    border-radius: var(--radius-sm);
  }
  .kind {
    font-weight: bold;
    white-space: nowrap;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 8px 8px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
</style>
