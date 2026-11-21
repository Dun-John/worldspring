<script lang="ts">
  // Build › Fence: a hedge, fence or wall drawn by hand (clicked corner by corner or dragged), a gate put in
  // one that is on the map, or a gap cut through fences with the eraser. Select (A) changes or takes away
  // one already there.
  import { ERASER_WIDTHS, FENCE_KINDS, type FenceMode, type FenceSettings } from '../../editor/fence';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    fence: FenceSettings;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { fence = $bindable(), near, peek = false, onZoomIn }: Props = $props();

  const MODES: { key: FenceMode; label: string; title: string }[] = [
    { key: 'draw', label: 'Draw', title: 'Click corners, or drag a line' },
    { key: 'gate', label: 'Gate', title: 'Click on a fence to put a gate in it' },
    { key: 'erase', label: 'Erase', title: 'Drag over a stretch to cut a gap in every fence and hedge' },
  ];
  const rule = $derived(FENCE_KINDS.find((k) => k.key === fence.kind)?.rule ?? '');
</script>

{#snippet peekHint()}
  <div class="ws-hint">{fence.mode === 'draw' ? 'Tap its corners, then double-tap to end it, or drag it.' : fence.mode === 'gate' ? 'Tap a fence to put a gate in it.' : 'Drag over a stretch to cut a gap.'}</div>
{/snippet}

<ToolPanel title="Fence" {near} {peek} zoomText="draw" {onZoomIn} {peekHint}>
  <div class="ws-seg" role="radiogroup" aria-label="What to do">
    {#each MODES as m (m.key)}
      <button class:on={fence.mode === m.key} aria-pressed={fence.mode === m.key} title={m.title} onclick={() => (fence = { ...fence, mode: m.key })}>{m.label}</button>
    {/each}
  </div>
  {#if fence.mode === 'draw'}
    <div class="kinds" role="radiogroup" aria-label="Kind">
      {#each FENCE_KINDS as k (k.key)}
        <button class="ws-chip" class:on={fence.kind === k.key} role="radio" aria-checked={fence.kind === k.key} onclick={() => (fence = { ...fence, kind: k.key })}>{k.label}</button>
      {/each}
    </div>
  {/if}
  {#snippet more()}
    {#if fence.mode === 'erase'}
      <div class="ws-seg" role="radiogroup" aria-label="Gap width">
        {#each ERASER_WIDTHS as w (w)}
          <button class:on={fence.eraser === w} aria-pressed={fence.eraser === w} onclick={() => (fence = { ...fence, eraser: w })}>{w} ft</button>
        {/each}
      </div>
    {:else}
      <div class="ws-hint">Gates are 5 ft wide and stay open. Hedges, fences and walls cross roads and stop at water.</div>
    {/if}
  {/snippet}
  {#snippet hint()}
    <div class="ws-hint">
      {#if fence.mode === 'draw'}
        Click its corners, then press Enter or double-click to end it (click the first corner to close it), or press and drag it freehand. {rule} Select (A) changes or takes away a fence that is there.
      {:else if fence.mode === 'gate'}
        Click on any fence or hedge on the map to put a 5 ft gate in it.
      {:else}
        Press and drag over a stretch of fence, hedge or wall to cut a gap in it, as wide as chosen under More options.
      {/if}
    </div>
  {/snippet}
</ToolPanel>

<style>
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
</style>
