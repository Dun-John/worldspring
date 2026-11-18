<script lang="ts">
  // Build › Road: a footpath, track, gravel road, paved road or cobbled street, clicked point by
  // point or dragged; click one drawn earlier to change it.
  import type { PathKind } from '../../gen/protocol';
  import { PATH_KINDS, PATH_WIDTH, type PathSettings } from '../../editor/paths';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    road: PathSettings;
    /** The road drawn by hand picked (its type, width and name), else drawing new ones. */
    roadPicked: { kind: PathKind; width: number; name: string } | null;
    onRoadChange: (change: { kind?: PathKind; width?: number; name?: string }) => void;
    onRoadRemove: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { road = $bindable(), roadPicked, onRoadChange, onRoadRemove, onDone, near, peek = false, onZoomIn }: Props = $props();

  const roadKind = $derived(roadPicked?.kind ?? road.kind);
  const roadW = $derived(roadPicked?.width ?? road.width);
</script>

{#snippet changing()}Changing the <b>{roadPicked?.name || PATH_KINDS.find((k) => k.key === roadPicked?.kind)?.name}</b>{/snippet}
{#snippet peekHint()}
  <div class="ws-hint">Tap its points, then double-tap to end it, or drag it; tap a road to change it.</div>
{/snippet}

<ToolPanel title="Road" {near} {peek} zoomText="Zoom in to draw it." {onZoomIn} changing={roadPicked ? changing : undefined} onRemove={onRoadRemove} {onDone} peekHint={roadPicked ? undefined : peekHint}>
  <div class="ws-seg" role="radiogroup" aria-label="Road">
    {#each PATH_KINDS as k (k.key)}
      <button class:on={roadKind === k.key} aria-pressed={roadKind === k.key} onclick={() => (roadPicked ? onRoadChange({ kind: k.key }) : (road = { ...road, kind: k.key, width: k.width }))}>{k.label}</button>
    {/each}
  </div>
  {#snippet more()}
    <label class="ws-slider">
      <span>Width</span><output>{roadW} ft</output>
      <input
        type="range"
        min={PATH_WIDTH.min}
        max={PATH_WIDTH.max}
        step="1"
        value={roadW}
        oninput={(e) => {
          if (!roadPicked) road = { ...road, width: Number(e.currentTarget.value) };
        }}
        onchange={(e) => roadPicked && onRoadChange({ width: Number(e.currentTarget.value) })}
      />
    </label>
    <label class="ws-field">
      Name
      {#if roadPicked}
        <input class="ws-input" value={roadPicked.name} placeholder="No name" onchange={(e) => onRoadChange({ name: e.currentTarget.value })} />
      {:else}
        <input class="ws-input" bind:value={road.name} placeholder="No name" />
      {/if}
    </label>
  {/snippet}
  {#snippet hint()}
    <div class="ws-hint">
      {roadPicked ? 'Pick a type, width or name for it; click away from it to draw another. ' : 'Click its points, then press Enter or double-click to end it, or press and drag it freehand; click a road drawn earlier to change it. It runs on a smooth curve through the points; trees and the town’s own buildings keep off it. '}{PATH_KINDS.find((k) => k.key === roadKind)?.look}
    </div>
  {/snippet}
</ToolPanel>
