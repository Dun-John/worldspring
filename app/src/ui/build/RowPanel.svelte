<script lang="ts">
  // Build › Row: buildings along a path, facing it, on one side or both.
  import type { BuildingFuncs } from '../../gen/protocol';
  import type { RowSettings, StampSettings } from '../../editor/quick';
  import type { QuickInfo } from '../BuildPanel.svelte';
  import ToolPanel from './ToolPanel.svelte';
  import { PATH_KINDS, PATH_WIDTH } from '../../editor/paths';
  import { WARDS, categoriesOf, label } from './util';

  interface Props {
    row: RowSettings;
    /** (The facing is the stamp's.) */
    stamp: StampSettings;
    funcs: BuildingFuncs | null;
    quick: QuickInfo;
    onPlace: () => void;
    onDrop: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { row = $bindable(), stamp = $bindable(), funcs, quick, onPlace, onDrop, near, peek = false, onZoomIn }: Props = $props();

  const FACINGS: { label: string; deg: number | null }[] = [{ label: 'Auto', deg: null }, { label: 'East', deg: 0 }, { label: 'South', deg: 90 }, { label: 'West', deg: 180 }, { label: 'North', deg: 270 }];
  const SIDES = [
    { key: 'both', label: 'Both sides' },
    { key: 'left', label: 'Left' },
    { key: 'right', label: 'Right' },
  ] as const;
  const categories = $derived(categoriesOf(funcs));
  const kindName = (k: string) => funcs?.businesses.find((b) => b.key === k)?.name ?? funcs?.homes.find((h) => h.key === k)?.name ?? label(k);
  function addKind(k: string) {
    if (k && !row.kinds.includes(k)) row.kinds = [...row.kinds, k];
  }
</script>

<ToolPanel title="Row" {near} {peek} {onZoomIn} startOver {onDrop} {quick} place="Place the row" {onPlace}>
  <div class="ws-seg" role="radiogroup" aria-label="Side">
    {#each SIDES as s (s.key)}
      <button class:on={row.side === s.key} aria-pressed={row.side === s.key} onclick={() => (row.side = s.key)}>{s.label}</button>
    {/each}
  </div>
  <label class="switch">
    <span class="grow">Lay a road along it</span>
    <input type="checkbox" class="ws-switch" bind:checked={row.road.on} />
  </label>
  {#if row.road.on}
    <div class="ws-seg" role="radiogroup" aria-label="Road kind">
      {#each PATH_KINDS as k (k.key)}
        <button class:on={row.road.kind === k.key} aria-pressed={row.road.kind === k.key} onclick={() => (row.road = { ...row.road, kind: k.key, width: k.width })}>{k.label}</button>
      {/each}
    </div>
    <div class="ws-hint">In a town it becomes one of its streets.</div>
  {/if}
  {#snippet hint()}
    <div class="ws-hint">Drag along a street, or click each corner and press Enter: buildings line the path, facing it, on the side you choose. Left and right are as you walk along it.</div>
  {/snippet}
  {#snippet more()}
    {#if row.road.on}
      <label class="slider">
        <span>Road width</span><output>{row.road.width} ft</output>
        <input type="range" min={PATH_WIDTH.min} max={PATH_WIDTH.max} step="1" bind:value={row.road.width} />
      </label>
    {/if}
    <label class="ws-field">
      Ward
      <select class="ws-input" bind:value={row.ward}>
        {#each WARDS as w (w)}<option value={w}>{label(w)}</option>{/each}
      </select>
    </label>
    <div class="ws-field">
      Kinds
      <div class="tints">
        {#if !row.kinds.length}<span class="ws-hint">The ward’s homes and now and then a trade</span>{/if}
        {#each row.kinds as k (k)}
          <button class="ws-chip on" onclick={() => (row.kinds = row.kinds.filter((x) => x !== k))} title="Take it out of the mix">{kindName(k)} ×</button>
        {/each}
      </div>
      <select class="ws-input" value="" onchange={(e) => (addKind(e.currentTarget.value), (e.currentTarget.value = ''))} aria-label="Add a kind">
        <option value="">Add a kind…</option>
        {#if funcs}
          <optgroup label="Homes">
            {#each funcs.homes as h (h.key)}<option value={h.key}>{label(h.name)}</option>{/each}
          </optgroup>
          {#each categories as c (c)}
            <optgroup label={label(c)}>
              {#each funcs.businesses.filter((b) => b.category === c) as b (b.key)}<option value={b.key}>{b.name}</option>{/each}
            </optgroup>
          {/each}
        {/if}
      </select>
    </div>
    <label class="slider">
      <span>Depth</span><output>{row.depth ? `${row.depth} ft` : 'By the ward'}</output>
      <input type="range" min="0" max="120" step="5" bind:value={row.depth} />
    </label>
    <label class="slider">
      <span>Gaps</span><output>{Math.round(row.gaps * 100)}%</output>
      <input type="range" min="0" max="0.5" step="0.05" bind:value={row.gaps} />
    </label>
    <div class="ws-seg" role="radiogroup" aria-label="Facing">
      {#each FACINGS as f (f.label)}
        <button class:on={stamp.facing === f.deg} aria-pressed={stamp.facing === f.deg} onclick={() => (stamp.facing = f.deg)} title={f.deg == null ? 'Front toward the nearest street' : `Front toward the ${f.label.toLowerCase()}`}>{f.label}</button>
      {/each}
    </div>
    <label class="switch">
      <span class="grow">Clear the way<span class="ws-muted"> · the town’s own buildings in it go</span></span>
      <input type="checkbox" class="ws-switch" bind:checked={row.clear} />
    </label>
  {/snippet}
  {#snippet startExtra()}
    <button onclick={() => (row.seed += 1)} title="Another mix of widths, kinds and storeys">Reroll</button>
  {/snippet}
</ToolPanel>
