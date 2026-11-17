<script lang="ts">
  // Build › Harbour: islets, quays, piers and breakwaters; click one put down to pick it.
  import { WHARF_WIDTH, type WharfKind } from '../../gen/protocol';
  import { ISLET_RADIUS, ISLET_RISE, type HarbourSettings, type HarbourKind } from '../../editor/harbour';
  import type { IconName } from '../icons';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    harbour: HarbourSettings;
    /** The wharf picked (its id), else drawing new ones. */
    harbourPicked: string | null;
    onHarbourRemove: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { harbour = $bindable(), harbourPicked, onHarbourRemove, onDone, near, peek = false, onZoomIn }: Props = $props();

  const HARBOUR: { key: HarbourKind; label: string; icon: IconName; look: string }[] = [
    { key: 'islet', label: 'Islet', icon: 'mountain', look: 'Click in a river, lake or the sea: a rock islet rises there. A building, tower or castle may stand on it.' },
    { key: 'quay', label: 'Quay', icon: 'wall', look: 'Drag along a shore: paving to the dry side, the water behind it filled in.' },
    { key: 'pier', label: 'Pier', icon: 'bridge', look: 'Drag from the bank out over the water: a deck on piles.' },
    { key: 'breakwater', label: 'Breakwater', icon: 'waves', look: 'Drag from the bank out into the water: a stone mole, with a lighthouse at its tip if you like.' },
  ];
  function harbourKind(k: HarbourKind) {
    harbour = { ...harbour, kind: k, width: k === 'islet' ? harbour.width : WHARF_WIDTH[k as WharfKind][2] };
  }
  const wharfRange = $derived(harbour.kind === 'islet' ? null : WHARF_WIDTH[harbour.kind]);
</script>

{#snippet changing()}Picked the <b>{harbourPicked}</b>{/snippet}

<ToolPanel title="Harbour" {near} {peek} zoomText="Zoom in to put one down." {onZoomIn} changing={harbourPicked ? changing : undefined} onRemove={onHarbourRemove} {onDone}>
  <div class="ws-seg" role="radiogroup" aria-label="Harbour">
    {#each HARBOUR as h (h.key)}
      <button class:on={harbour.kind === h.key} aria-pressed={harbour.kind === h.key} onclick={() => harbourKind(h.key)} title={h.label}>{h.label}</button>
    {/each}
  </div>
  {#snippet hint()}
    <div class="ws-hint">{HARBOUR.find((h) => h.key === harbour.kind)?.look}</div>
  {/snippet}
  {#snippet more()}
    {#if harbour.kind === 'islet'}
      <label class="slider">
        <span>Radius</span><output>{harbour.radius} ft</output>
        <input type="range" min={ISLET_RADIUS[0]} max={ISLET_RADIUS[1]} step="5" bind:value={harbour.radius} />
      </label>
      <label class="slider">
        <span>Rise over the water</span><output>{harbour.rise} ft</output>
        <input type="range" min={ISLET_RISE[0]} max={ISLET_RISE[1]} step="1" bind:value={harbour.rise} />
      </label>
    {:else if wharfRange}
      <label class="slider">
        <span>Width</span><output>{harbour.width} ft</output>
        <input type="range" min={wharfRange[0]} max={wharfRange[1]} step="1" bind:value={harbour.width} />
      </label>
      {#if harbour.kind === 'pier'}
        <div class="ws-seg" role="radiogroup" aria-label="Built of">
          <button class:on={harbour.material === 'timber'} aria-pressed={harbour.material === 'timber'} onclick={() => (harbour = { ...harbour, material: 'timber' })}>Timber</button>
          <button class:on={harbour.material === 'stone'} aria-pressed={harbour.material === 'stone'} onclick={() => (harbour = { ...harbour, material: 'stone' })}>Stone</button>
        </div>
      {/if}
      {#if harbour.kind === 'breakwater'}
        <label class="switch"><span class="grow">Lighthouse at its tip</span><input type="checkbox" class="ws-switch" bind:checked={harbour.lighthouse} /></label>
      {/if}
    {/if}
  {/snippet}
</ToolPanel>
