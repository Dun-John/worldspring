<script lang="ts">
  // Build › Building: draw a building on the map (a rectangle, any polygon, a round tower),
  // snapped to the 5-ft grid, and say what it is, its storeys, roof shape and material. Each
  // footprint drawn becomes a building. Editing one: change its options, or draw again to move
  // or reshape it.
  import type { BuildingFuncs } from '../../gen/protocol';
  import type { BuildSettings, BuildShape } from '../../editor/build';
  import Icon from '../Icon.svelte';
  import type { IconName } from '../icons';
  import BuildingOptions from './BuildingOptions.svelte';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    settings: BuildSettings;
    funcs: BuildingFuncs | null;
    /** The building being changed (its name), else drawing new ones. */
    editing: string | null;
    onSave: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { settings = $bindable(), funcs, editing, onSave, onDone, near, peek = false, onZoomIn }: Props = $props();

  const SHAPES: { key: BuildShape; label: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'rect', label: 'Rectangle', icon: 'square', kbd: 'R', hint: 'Drag from corner to corner.' },
    { key: 'poly', label: 'Polygon', icon: 'polygon', kbd: 'O', hint: 'Click each corner; click the first again, double-click or press Enter to finish. Backspace takes a corner back.' },
    { key: 'tower', label: 'Round tower', icon: 'circle', kbd: 'T', hint: 'Drag from the middle out.' },
  ];

  function shape(k: BuildShape) {
    // A round tower is a wizard's tower unless it was something else already.
    if (k === 'tower' && settings.func === 'house') settings = { ...settings, shape: k, func: 'wizard_tower', floors: Math.max(settings.floors, 4) };
    else settings = { ...settings, shape: k };
  }
</script>

{#snippet changing()}Editing <b>{editing}</b>{/snippet}

<ToolPanel title="Building" {near} {peek} {onZoomIn} changing={editing ? changing : undefined} {onDone}>
  <div class="ws-seg" role="radiogroup" aria-label="Shape">
    {#each SHAPES as s (s.key)}
      <button class:on={settings.shape === s.key} aria-pressed={settings.shape === s.key} onclick={() => shape(s.key)} title="{s.label} ({s.kbd})"><Icon name={s.icon} size={16} />{s.label}</button>
    {/each}
  </div>
  {#snippet hint()}
    <div class="ws-hint">{editing ? 'Draw on the map to move or reshape it: ' : ''}{SHAPES.find((s) => s.key === settings.shape)?.hint}</div>
  {/snippet}
  {#snippet more()}
    <BuildingOptions bind:settings {funcs} {editing} {onSave} />
  {/snippet}
</ToolPanel>
