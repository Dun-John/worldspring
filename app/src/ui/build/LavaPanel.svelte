<script lang="ts">
  // Build › Lava: a lava river brushed along a stroke (of a width) or a lava pit outlined; click
  // lava drawn earlier to change it. Roads bridge it, nothing is built on it, scorched rock round it.
  import type { WaterKind } from '../../gen/protocol';
  import { LAVA_WIDTHS, type LavaSettings } from '../../editor/water';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    lava: LavaSettings;
    /** The lava (or water) picked (its look, name and a river's width), else drawing new lava. */
    waterPicked: { look: WaterKind; name: string; width: number | null } | null;
    onWaterChange: (change: { name?: string; width?: number }) => void;
    onWaterRemove: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { lava = $bindable(), waterPicked, onWaterChange, onWaterRemove, onDone, near, peek = false, onZoomIn }: Props = $props();

  const lavaLine = $derived(waterPicked ? waterPicked.width !== null : lava.brush);
  const lavaWidth = $derived(waterPicked?.width ?? lava.width);
</script>

{#snippet changing()}Changing <b>{waterPicked?.name || (waterPicked?.look === 'lava' ? 'this lava' : 'this water')}</b>{/snippet}

<ToolPanel title="Lava" {near} {peek} zoomText="draw it" {onZoomIn} changing={waterPicked ? changing : undefined} onRemove={onWaterRemove} {onDone}>
  {#if !waterPicked}
    <div class="ws-seg" role="radiogroup" aria-label="Shape">
      <button class:on={lava.brush} aria-pressed={lava.brush} onclick={() => (lava = { ...lava, brush: true })} title="Paint a river of lava along a stroke">Lava river</button>
      <button class:on={!lava.brush} aria-pressed={!lava.brush} onclick={() => (lava = { ...lava, brush: false })} title="Outline a pit of lava">Lava pit</button>
    </div>
  {/if}
  {#if lavaLine}
    <label class="ws-slider">
      <span>Width</span><output>{lavaWidth} ft</output>
      <input
        type="range"
        min={LAVA_WIDTHS[1]}
        max={LAVA_WIDTHS[2]}
        step="1"
        value={lavaWidth}
        onchange={(e) => (waterPicked ? onWaterChange({ width: +e.currentTarget.value }) : (lava = { ...lava, width: +e.currentTarget.value }))}
        oninput={(e) => {
          if (!waterPicked) lava = { ...lava, width: +e.currentTarget.value };
        }}
      />
    </label>
  {/if}
  {#snippet always()}
    {#if waterPicked}
      <label class="ws-field">
        Name
        <input class="ws-input" value={waterPicked.name} placeholder="No name" onchange={(e) => onWaterChange({ name: e.currentTarget.value })} />
      </label>
    {/if}
  {/snippet}
  {#snippet peekHint()}
    {#if !waterPicked}<div class="ws-hint">{lava.brush ? 'Drag to paint a lava river.' : 'Tap its points, then the first again, or drag round it.'} Tap lava drawn earlier to change it.</div>{/if}
  {/snippet}
  {#snippet hint()}
    <div class="ws-hint">
      {#if waterPicked}
        Click outside it to draw more.
      {:else if lava.brush}
        Press and drag to paint a lava river along the stroke, or click along a line and press Enter. Click lava drawn earlier to change it.
      {:else}
        Click the pit's points, then the first again (or double-click), or drag round it and let go near the start. Click lava drawn earlier to change it.
      {/if}
    </div>
    <div class="ws-hint">Roads cross lava on stone bridges, nothing is built on it, and the ground round it is scorched.</div>
  {/snippet}
</ToolPanel>
