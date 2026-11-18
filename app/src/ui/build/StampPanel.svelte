<script lang="ts">
  // Build › Stamp: a building of a kind and size set down where you click, fronting the nearest
  // street (a ghost shows where it lands).
  import type { BuildingFuncs } from '../../gen/protocol';
  import type { BuildSettings } from '../../editor/build';
  import type { StampSettings } from '../../editor/quick';
  import type { QuickInfo } from '../BuildPanel.svelte';
  import BuildingOptions from './BuildingOptions.svelte';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    stamp: StampSettings;
    settings: BuildSettings;
    funcs: BuildingFuncs | null;
    /** The building being changed (its name), else none. */
    editing: string | null;
    quick: QuickInfo;
    onPlace: () => void;
    onSave: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { stamp = $bindable(), settings = $bindable(), funcs, editing, quick, onPlace, onSave, near, peek = false, onZoomIn }: Props = $props();

  const STAMP_SIZES = [
    { key: 'small', label: 'Small' },
    { key: 'usual', label: 'Usual' },
    { key: 'large', label: 'Large' },
  ] as const;
</script>

<!-- (What it is and Place it show; its storeys, roof and the rest fold.) -->
<ToolPanel title="Stamp" {near} {peek} {onZoomIn} {quick} place="Place it" {onPlace}>
  <div class="ws-seg" role="radiogroup" aria-label="Size">
    {#each STAMP_SIZES as z (z.key)}
      <button class:on={stamp.size === z.key} aria-pressed={stamp.size === z.key} onclick={() => (stamp.size = z.key)}>{z.label}</button>
    {/each}
  </div>
  {#if !peek}<BuildingOptions bind:settings {funcs} {editing} {onSave} part="main" floors="rest" />{/if}
  {#snippet hint()}
    <div class="ws-hint">Click to set one down, fronting the nearest street. The ghost shows where it lands and which way it faces. Each click puts down another.</div>
  {/snippet}
  {#snippet more()}
    <label class="switch">
      <span class="ws-grow">Clear the way<span class="ws-muted"> · the town’s own buildings under it go</span></span>
      <input type="checkbox" class="ws-switch" bind:checked={stamp.clear} />
    </label>
    <BuildingOptions bind:settings {funcs} {editing} {onSave} part="rest" floors="rest" />
  {/snippet}
</ToolPanel>
