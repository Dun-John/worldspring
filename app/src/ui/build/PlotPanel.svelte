<script lang="ts">
  // Build › Plot: draw round a plot and fill it (the ward's lots, left empty, or open ground).
  import { LOT_SIZES } from '../../gen/protocol';
  import type { LassoSettings } from '../../editor/quick';
  import type { QuickInfo } from '../BuildPanel.svelte';
  import Icon from '../Icon.svelte';
  import ToolPanel from './ToolPanel.svelte';
  import { WARDS, label } from './util';

  interface Props {
    lasso: LassoSettings;
    quick: QuickInfo;
    onPlace: () => void;
    onDrop: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
    /** `?ui=new`: the plot being changed (its name), opened by Select's Change; its settings are written as they are set. */
    editing?: string | null;
    onRemove?: () => void;
    onDone?: () => void;
  }

  let { lasso = $bindable(), quick, onPlace, onDrop, near, peek = false, onZoomIn, editing = null, onRemove, onDone }: Props = $props();

  const OPEN_KINDS = ['plaza', 'lawn', 'park', 'garden', 'market', 'cemetery', 'farmland'];
</script>

{#snippet changing()}Changing <b>{editing}</b>{/snippet}

<ToolPanel title="Plot" {near} {peek} {onZoomIn} changing={editing ? changing : undefined} {onRemove} {onDone} startOver {onDrop} {quick} place="Make the plot" {onPlace}>
  <div class="ws-seg" role="radiogroup" aria-label="Fill">
    <button class:on={lasso.fill === 'generated'} aria-pressed={lasso.fill === 'generated'} onclick={() => (lasso.fill = 'generated')}>Buildings</button>
    <button class:on={lasso.fill === 'hand'} aria-pressed={lasso.fill === 'hand'} onclick={() => (lasso.fill = 'hand')}>Empty</button>
    <button class:on={lasso.fill === 'open'} aria-pressed={lasso.fill === 'open'} onclick={() => (lasso.fill = 'open')}>Open ground</button>
  </div>
  {#snippet hint()}
    <div class="ws-hint">{editing ? 'Its fill, ward, lots and name change as you set them; for another outline, delete it and draw again. ' : ''}Draw round a plot, or click each corner (the first again to finish).
      {lasso.fill === 'generated' ? 'The generator fills it with lots of the ward.' : lasso.fill === 'hand' ? 'It is left empty for buildings drawn by hand.' : 'It stays open ground.'}
      A plot cannot overlap another.</div>
  {/snippet}
  {#snippet more()}
    {#if lasso.fill === 'generated'}
      <label class="ws-field">
        Ward
        <select class="ws-input" bind:value={lasso.ward}>
          {#each WARDS as w (w)}<option value={w}>{label(w)}</option>{/each}
        </select>
      </label>
      <div class="ws-field">
        Lot size
        <div class="ws-seg">
          <button class:on={lasso.lots === ''} onclick={() => (lasso.lots = '')} title="As the ward has it">Usual</button>
          {#each LOT_SIZES as l (l)}<button class:on={lasso.lots === l} onclick={() => (lasso.lots = l)}>{label(l)}</button>{/each}
        </div>
      </div>
      <div class="ws-field">
        Lay the lots out again
        <div class="stepper">
          <button class="ws-icon-btn" onclick={() => (lasso.reroll = Math.max(0, lasso.reroll - 1))} disabled={lasso.reroll <= 0} aria-label="Previous layout"><Icon name="minus" size={16} /></button>
          <span>{lasso.reroll}</span>
          <button class="ws-icon-btn" onclick={() => (lasso.reroll += 1)} aria-label="Another layout"><Icon name="plus" size={16} /></button>
        </div>
      </div>
    {:else if lasso.fill === 'open'}
      <label class="ws-field">
        Ground
        <select class="ws-input" bind:value={lasso.open}>
          {#each OPEN_KINDS as o (o)}<option value={o}>{label(o)}</option>{/each}
        </select>
      </label>
    {/if}
    <label class="ws-field">
      Name
      <input class="ws-input" bind:value={lasso.name} placeholder="Unnamed" />
    </label>
  {/snippet}
</ToolPanel>
