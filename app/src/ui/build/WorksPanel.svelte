<script lang="ts">
  // Build › Castle and Wall: an outline (a rectangle or a polygon), then the side its gate is in
  // (more gates, its keep's shape and its moat while changing it); a wall, a line of corners (a
  // ring if closed), gates on the corners clicked twice.
  import type { KeepShape, Moat } from '../../gen/protocol';
  import type { WorksSettings } from '../../editor/works';
  import Icon from '../Icon.svelte';
  import ToolPanel from './ToolPanel.svelte';

  const KEEP_SHAPES: { key: KeepShape | 'auto'; label: string; title: string }[] = [
    { key: 'auto', label: 'Any keep', title: 'A keep of the castle’s own kind (now and then a shell keep on a mound)' },
    { key: 'square', label: 'Square', title: 'A keep of square towers and halls' },
    { key: 'round', label: 'Round', title: 'A round great tower' },
    { key: 'drawn', label: 'Drawn', title: 'The keep as drawn' },
  ];
  const MOATS: { key: Moat | 'auto'; label: string; title: string }[] = [
    { key: 'auto', label: 'Any moat', title: 'A moat or none, as the castle’s age and ground have it' },
    { key: 'none', label: 'No moat', title: 'No moat' },
    { key: 'dry', label: 'Dry', title: 'A dry ditch round the walls' },
    { key: 'wet', label: 'Wet', title: 'A moat of water round the walls' },
  ];

  interface Props {
    mode: 'castle' | 'wall';
    works: WorksSettings;
    /** The castle or wall being changed (its name), else drawing new ones. */
    worksEditing: string | null;
    /** The view is over a town laid out by hand. */
    handTown?: boolean;
    onWorksSave: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { mode, works = $bindable(), worksEditing, handTown = false, onWorksSave, onDone, near, peek = false, onZoomIn }: Props = $props();
</script>

{#snippet changing()}Changing <b>{worksEditing}</b>{/snippet}

<ToolPanel title={mode === 'castle' ? 'Castle' : 'Wall'} {near} {peek} zoomText="Zoom in to draw." {onZoomIn} changing={worksEditing ? changing : undefined} {onDone}>
  {#if mode === 'castle'}
    <div class="ws-seg" role="radiogroup" aria-label="Outline">
      <button class:on={works.castleShape === 'rect'} aria-pressed={works.castleShape === 'rect'} onclick={() => (works.castleShape = 'rect')} title="Rectangle (R)"><Icon name="square" size={16} />Rectangle</button>
      <button class:on={works.castleShape === 'poly'} aria-pressed={works.castleShape === 'poly'} onclick={() => (works.castleShape = 'poly')} title="Polygon (O)"><Icon name="polygon" size={16} />Polygon</button>
    </div>
  {/if}
  {#snippet hint()}
    <div class="ws-hint">
      {#if mode === 'castle'}
        {worksEditing ? 'Click a side to add a gate in its middle (on a gate: take it away), or draw the walls again. ' : ''}{works.castleShape === 'rect'
          ? 'Drag the walls from corner to corner'
          : 'Click each corner; click the first again or press Enter'}, then click the side the gate is in (Enter: the side facing the road). 80 to 1,500 ft across.
      {:else}
        {#if handTown}Round this town, draw its wall in Streets › Draw › Town wall: gates open where its streets cross.<br />{/if}
        {worksEditing ? 'Click the wall to add a gate there (on a gate: take it away), or draw it again. ' : ''}Click each corner; click a corner twice to make it a gate.
        Double-click or press Enter to finish. Roads and streets that cross it get a gate. Up to 3,000 ft.
      {/if}
    </div>
  {/snippet}
  {#snippet more()}
    {#if mode === 'castle'}
      <label class="switch">
        <span class="grow">Keep<span class="ws-muted"> · the castle’s great tower in the yard</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={works.keep} />
      </label>
      {#if works.keep}
        <div class="ws-seg" role="radiogroup" aria-label="Keep shape">
          {#each KEEP_SHAPES as k (k.key)}
            {#if k.key !== 'drawn' || works.keepShape === 'drawn'}
              <button class:on={works.keepShape === k.key} aria-pressed={works.keepShape === k.key} onclick={() => (works.keepShape = k.key)} title={k.title}>{k.label}</button>
            {/if}
          {/each}
        </div>
      {/if}
      <div class="ws-seg" role="radiogroup" aria-label="Moat">
        {#each MOATS as m (m.key)}
          <button class:on={works.moat === m.key} aria-pressed={works.moat === m.key} onclick={() => (works.moat = m.key)} title={m.title}>{m.label}</button>
        {/each}
      </div>
      <label class="switch">
        <span class="grow">Yard buildings<span class="ws-muted"> · barracks, stables, a smithy along the walls</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={works.yardBuildings} />
      </label>
      <label class="switch">
        <span class="grow">In ruins<span class="ws-muted"> · broken walls, roofless</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={works.ruin} />
      </label>
    {:else}
      <label class="switch">
        <span class="grow">Closed ring<span class="ws-muted"> · the last corner joins the first</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={works.closed} />
      </label>
    {/if}
    <label class="ws-field">
      Name
      <input class="ws-input" bind:value={works.name} placeholder={worksEditing ? '' : 'Named for the place if left empty'} />
    </label>
    {#if worksEditing}<button class="ws-btn primary block" onclick={onWorksSave}>Save changes</button>{/if}
  {/snippet}
</ToolPanel>
