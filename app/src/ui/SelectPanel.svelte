<script lang="ts">
  // Edit › Build › Select: what a click found (the top one open: its kind, name and town, Delete
  // and Change), or what a box or lasso holds, counted by kind, with "Delete all that can be".
  import type { SelectTools } from '../editor/tools/select.svelte';
  import { KIND_WORDS, countOf, foldedWords } from '../editor/select';
  import { FENCE_KINDS } from '../editor/fence';
  import type { FenceKind, Thing } from '../gen/protocol';
  import Icon from './Icon.svelte';
  import ToolPanel from './build/ToolPanel.svelte';
  import { NEW_UI, shell } from './shell/layout.svelte';

  interface Props {
    sel: SelectTools;
    peek?: boolean;
    /** `?ui=new`: plain Select, or Join or leave a town (only what can be part of one, Join and
     * Leave first). */
    purpose?: 'all' | 'members';
  }

  let { sel, peek = false, purpose = 'all' }: Props = $props();
  const members = $derived(purpose === 'members');
  const links = $derived(sel.links);

  const word = (t: Thing) => {
    const w = KIND_WORDS[t.kind][0];
    return w.charAt(0).toUpperCase() + w.slice(1);
  };
  const can = (t: Thing) => t.removable && !!t.id;
  const all = $derived(sel.things.filter(can));
  const joinable = $derived(sel.things.filter(sel.joinable));
  const leavable = $derived(sel.things.filter((t) => sel.leavable(t) && !sel.leaveBlock(t)));
  const partOfTown = $derived(sel.things.filter((t) => sel.joinable(t) || sel.leavable(t)));
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

{#snippet drag()}
  <div class="ws-seg" role="radiogroup" aria-label="Drag draws">
    <button class:on={!sel.lasso} role="radio" aria-checked={!sel.lasso} onclick={() => (sel.lasso = false)}>Box</button>
    <button class:on={sel.lasso} role="radio" aria-checked={sel.lasso} onclick={() => (sel.lasso = true)}>Lasso</button>
  </div>
{/snippet}
{#snippet hint()}
  {#if !sel.things.length}
    <div class="ws-hint">
      {sel.area ? 'Nothing there to choose.' : 'Click something on the map to see what it is. Drag a box, or a lasso (Shift-drag), to choose all inside it.'}
    </div>
  {/if}
  {@render drag()}
{/snippet}
{#snippet peekHint()}
  {#if sel.things.length}<div class="ws-hint">{sel.area ? `${sel.things.length} chosen` : word(sel.things[sel.open] ?? sel.things[0])}</div>{/if}
{/snippet}
{#snippet joinLeave()}
  {#if joinable.length || leavable.length}
    <div class="actions">
      {#if joinable.length}<button class="ws-btn" disabled={sel.busy} onclick={() => sel.join(joinable)}>Join town ({joinable.length})</button>{/if}
      <button class="ws-btn" disabled={sel.busy || !leavable.length} onclick={() => sel.leave(leavable)}>Leave town{leavable.length ? ` (${leavable.length})` : ''}</button>
    </div>
    {@render picker()}
    <div class="ws-muted">{num(partOfTown.length, 'thing here can', 'things here can')} be part of a town.</div>
  {/if}
{/snippet}

<!-- (`?ui=new`: the card is not folded away; the strip's line says what to do.) -->
<ToolPanel title="Select" near={true} {peek} onZoomIn={() => {}} hint={NEW_UI ? undefined : hint} peekHint={NEW_UI ? undefined : peekHint} more={NEW_UI ? undefined : card}>
  <!-- (`?ui=new`: the card first, what to act on; on a phone it shows with the sheet at its lowest.) -->
  {#if NEW_UI && (!peek || shell.phone)}
    {@render card()}
    {#if sel.area && !sel.things.length}<div class="ws-hint">{members ? 'Nothing there can be part of a town.' : 'Nothing there to choose.'}</div>{/if}
    {#if !peek}{@render drag()}{/if}
  {/if}
</ToolPanel>

{#snippet card()}
    {#if sel.area && sel.things.length}
      {#if members}{@render joinLeave()}{/if}
      <div class="sel-list" aria-label="What is inside">
        {#each sel.counts as c (c.kind)}
          <div class="row">
            <span class="grow">{c.words}</span>
            {#if c.can < c.n}<span class="ws-muted">{c.can ? `${c.can} can be removed` : "can't be removed yet"}</span>{/if}
            <!-- (`?ui=new`: each kind's own Delete, in place of Build › Clear.) -->
            {#if NEW_UI && c.can && sel.counts.length > 1}<button class="ws-btn quiet danger kind-del" disabled={sel.busy} onclick={() => sel.removeKind(c.kind)} title="Delete {countOf(c.kind, c.can)}" aria-label="Delete {countOf(c.kind, c.can)}"><Icon name="trash" size={16} /></button>{/if}
          </div>
        {/each}
      </div>
      {#if !members}{@render joinLeave()}{/if}
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
              <span class="grow name">{t.name && t.name.toLowerCase() !== word(t).toLowerCase() ? t.name : ''}</span>
            </button>
            {#if k === sel.open}
              {@const part = sel.partOf(t)}
              <div class="body">
                {#if town}<div class="ws-muted">in {town}</div>{/if}
                {#if part}<div class="ws-muted">{part}</div>{/if}
                {#if sel.joinable(t) || sel.leavable(t)}
                  {@const block = sel.leaveBlock(t)}
                  <div class="actions">
                    {#if sel.joinable(t)}<button class="ws-btn" disabled={sel.busy} onclick={() => sel.join([t])}>Join town</button>{/if}
                    {#if sel.leavable(t)}<button class="ws-btn" disabled={sel.busy || !!block} title={block ?? undefined} onclick={() => sel.leave([t])}>Leave town</button>{/if}
                  </div>
                  {#if block}<div class="ws-muted">Can't leave: {block.charAt(0).toLowerCase() + block.slice(1)}.</div>{/if}
                  {@render picker()}
                {/if}
                <div class="actions">
                  {#if can(t)}
                    <button class="ws-btn danger" disabled={sel.busy} onclick={() => void sel.remove([t])}><Icon name="trash" size={16} /> Delete</button>
                  {:else}
                    <span class="ws-muted">can't be removed yet</span>
                  {/if}
                  {#if sel.changeable(t)}<button class="ws-btn" onclick={() => void sel.change(t)}><Icon name="pencil" size={16} /> Change</button>{/if}
                  {#if links.info}<button class="ws-btn quiet" onclick={() => links.info?.(t)} title="Its card, as a click outside Edit shows it"><Icon name="info" size={16} /> Info</button>{/if}
                </div>
                {#if links.openTown && links.hasStreets?.(t)}
                  <div class="actions">
                    <button class="ws-btn" onclick={() => links.openTown?.(t, 'streets')} title="Re-plan its streets, wards and lots"><Icon name="wards" size={16} /> Streets</button>
                    <button class="ws-btn" onclick={() => links.openTown?.(t, 'townwall')} title="Its walls, gates and towers"><Icon name="tower" size={16} /> Town wall</button>
                  </div>
                {/if}
                {#if sel.fenced(t)}
                  <div class="actions">
                    <button class="ws-btn" disabled={sel.busy} onclick={() => void sel.change(t)} title="Opens the Gate tool: click on the fence to put a 5 ft gate in it"><Icon name="plus" size={16} /> Gate tool</button>
                    {#if sel.kindable(t)}
                      <select class="ws-input kind-pick" aria-label="Change kind" disabled={sel.busy} onchange={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ''; if (v) void sel.setKind(t, v as FenceKind); }}>
                        <option value="">Change kind…</option>
                        {#each FENCE_KINDS.filter((k) => k.key !== t.fence) as k (k.key)}<option value={k.key}>{k.label}</option>{/each}
                      </select>
                    {/if}
                  </div>
                {/if}
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
  /* The per-kind Delete: the ordinary icon colour and a hit area you can see. */
  .kind-del {
    min-width: var(--tap);
    color: var(--ink);
    background: var(--btn);
    border: 1px solid var(--line);
  }
  .kind-pick {
    width: auto;
    max-width: 100%;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
</style>
