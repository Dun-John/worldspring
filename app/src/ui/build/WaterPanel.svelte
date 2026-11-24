<script lang="ts">
  // Build › Water: brush or outline water, of a width and depth; click water drawn earlier to
  // change it.
  import type { WaterKind } from '../../gen/protocol';
  import { BRUSH_WIDTHS, DEPTHS, POND_BELOW_FT, type WaterSettings } from '../../editor/water';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    water: WaterSettings;
    /** The water picked (its look, name, a line's width, depth and shore), else drawing new water. */
    waterPicked: { look: WaterKind; name: string; width: number | null; depth: number; dug: boolean } | null;
    onWaterChange: (change: { name?: string; width?: number; depth?: number; dug?: boolean }) => void;
    onWaterRemove: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { water = $bindable(), waterPicked, onWaterChange, onWaterRemove, onDone, near, peek = false, onZoomIn }: Props = $props();

  /** What the picked water looks like, by its look (`water.ts` `lookOf`). */
  const LOOKS: Record<WaterKind, string> = {
    inlet: 'the sea reaching in, a beach round it',
    basin: 'a harbour basin of the sea, a paved quay round it',
    lake: 'a lake',
    pond: 'a pond, reeds round it',
    moat: 'a ring of still water, as a moat',
    stream: 'a stream running downhill',
    lava: 'lava (change it in Lava)',
  };
  /** The width shown: the picked line's, or the one new brushed water gets. */
  const waterLine = $derived(waterPicked ? waterPicked.width !== null : water.brush);
  const waterWidth = $derived(waterPicked?.width ?? water.width);
  const waterDepth = $derived(waterPicked?.depth ?? water.depth);
  const waterDug = $derived(waterPicked?.dug ?? water.dug);
</script>

{#snippet changing()}Changing <b>{waterPicked?.name || 'this water'}</b>{/snippet}

<ToolPanel title="Water" {near} {peek} zoomText="draw it" {onZoomIn} changing={waterPicked ? changing : undefined} onRemove={onWaterRemove} {onDone}>
  {#if !waterPicked}
    <div class="ws-seg" role="radiogroup" aria-label="Shape">
      <button class:on={water.brush} aria-pressed={water.brush} onclick={() => (water = { ...water, brush: true })} title="Paint water along a stroke">Brush</button>
      <button class:on={!water.brush} aria-pressed={!water.brush} onclick={() => (water = { ...water, brush: false })} title="Outline the water">Polygon</button>
    </div>
  {/if}
  {#if waterLine}
    <label class="ws-slider">
      <span>Width</span><output>{waterWidth} ft</output>
      <input
        type="range"
        min={BRUSH_WIDTHS[1]}
        max={BRUSH_WIDTHS[2]}
        step="1"
        value={waterWidth}
        onchange={(e) => (waterPicked ? onWaterChange({ width: +e.currentTarget.value }) : (water = { ...water, width: +e.currentTarget.value }))}
        oninput={(e) => {
          if (!waterPicked) water = { ...water, width: +e.currentTarget.value };
        }}
      />
    </label>
  {/if}
  <label class="ws-slider">
    <span>Depth</span><output>{waterDepth} ft</output>
    <input
      type="range"
      min={DEPTHS[1]}
      max={DEPTHS[2]}
      step="1"
      value={waterDepth}
      onchange={(e) => (waterPicked ? onWaterChange({ depth: +e.currentTarget.value }) : (water = { ...water, depth: +e.currentTarget.value }))}
      oninput={(e) => {
        if (!waterPicked) water = { ...water, depth: +e.currentTarget.value };
      }}
    />
  </label>
  <div class="ws-seg" role="radiogroup" aria-label="Shore">
    <button class:on={!waterDug} aria-pressed={!waterDug} onclick={() => (waterPicked ? onWaterChange({ dug: false }) : (water = { ...water, dug: false }))} title="A natural curve round the points">Natural</button>
    <button class:on={waterDug} aria-pressed={waterDug} onclick={() => (waterPicked ? onWaterChange({ dug: true }) : (water = { ...water, dug: true }))} title="Straight sides, rounded corners, a stone edge">Dug</button>
  </div>
  {#snippet always()}
    <!-- (On a phone's strip too: its name, and how to draw one, are all there is to it.) -->
    {#if waterPicked}
      <label class="ws-field">
        Name
        <input class="ws-input" value={waterPicked.name} placeholder="No name" onchange={(e) => onWaterChange({ name: e.currentTarget.value })} />
      </label>
    {/if}
  {/snippet}
  {#snippet peekHint()}
    {#if !waterPicked}<div class="ws-hint">{water.brush ? 'Drag to paint water; end on its start for a ring.' : 'Tap its points, then the first again, or drag round it.'} Tap water drawn earlier to change it.</div>{/if}
  {/snippet}
  {#snippet hint()}
    <div class="ws-hint">
      {#if waterPicked}
        It looks like {LOOKS[waterPicked.look]}. Click outside it to draw more.
      {:else if water.brush}
        Press and drag to paint water along the stroke, or click along a line and press Enter; end on its start for a ring. Click water drawn earlier to change it.
      {:else}
        Click its points, then the first again (or double-click), or drag round it and let go near the start. Water drawn over each other is one water. Click water drawn earlier to change it.
      {/if}
    </div>
    <div class="ws-hint">The look follows the water: at the sea's level it is the sea's (dug: a quay); an open stroke runs downhill as a stream; a ring stays still, as a moat; other water under {POND_BELOW_FT} ft deep is a pond, deeper a lake.</div>
  {/snippet}
</ToolPanel>
