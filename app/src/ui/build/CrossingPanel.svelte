<script lang="ts">
  // Build › Crossing: a bridge, ford or ferry put down by clicking one bank, then the other;
  // clicking one picks it to change or take away.
  import { CROSSING_WIDTH, NARROW_CROSSING_MAX, type CrossingKind, type CrossingMaterial } from '../../gen/protocol';
  import { SHOP_DEPTH_RANGE, SHOP_LANE_RANGE, shopsWidth, type CrossSettings } from '../../editor/crossing';
  import Icon from '../Icon.svelte';
  import type { IconName } from '../icons';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    cross: CrossSettings;
    /** The crossing picked (what it is), else putting new ones down. */
    crossEditing: string | null;
    onCrossSave: () => void;
    onCrossRemove: () => void;
    onDone: () => void;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { cross = $bindable(), crossEditing, onCrossSave, onCrossRemove, onDone, near, peek = false, onZoomIn }: Props = $props();

  const KINDS: { key: CrossingKind; label: string; icon: IconName; look: string; width: number }[] = [
    { key: 'bridge', label: 'Bridge', icon: 'bridge', look: 'A deck clear of the water, walked like a road.', width: 12 },
    { key: 'ford', label: 'Ford', icon: 'ford', look: 'The bed brought up to wading depth, stepping stones across.', width: 10 },
    { key: 'ferry', label: 'Ferry', icon: 'ferry', look: 'A jetty out from each bank, a raft on a rope between. At least 68 ft.', width: 12 },
  ];
  const MATERIALS: { key: CrossingMaterial; label: string; look: string }[] = [
    { key: 'timber', label: 'Timber', look: 'A timber deck clear of the water, walked like a road.' },
    { key: 'stone', label: 'Old stone', look: 'An old stone bridge on piers, mossy and worn, walked like a road.' },
  ];
  const kindLook = $derived(cross.kind === 'bridge' ? (MATERIALS.find((m) => m.key === cross.material)?.look ?? '') : (KINDS.find((k) => k.key === cross.kind)?.look ?? ''));
  const widthMax = $derived(cross.kind === 'bridge' ? CROSSING_WIDTH[1] : NARROW_CROSSING_MAX);
  // (A bridge with shops is wide enough for its rows and the lane between.)
  const widthMin = $derived(cross.kind === 'bridge' && cross.shops ? Math.min(widthMax, shopsWidth(cross.shopDepth, cross.shopLane)) : CROSSING_WIDTH[0]);
  function shopsOn(on: boolean) {
    cross = { ...cross, shops: on, width: on ? Math.max(cross.width, shopsWidth(cross.shopDepth, cross.shopLane)) : cross.width };
  }
  function shopDepth(d: number) {
    cross = { ...cross, shopDepth: d, width: Math.max(cross.width, shopsWidth(d, cross.shopLane)) };
  }
  function shopLane(l: number) {
    cross = { ...cross, shopLane: l, width: Math.max(cross.width, shopsWidth(cross.shopDepth, l)) };
  }
</script>

{#snippet changing()}Changing the <b>{crossEditing}</b>{/snippet}

<ToolPanel title="Crossing" {near} {peek} zoomText="Zoom in to put one down." {onZoomIn} changing={crossEditing ? changing : undefined} onRemove={onCrossRemove} {onDone}>
  <div class="ws-seg" role="radiogroup" aria-label="Crossing">
    {#each KINDS as k (k.key)}
      <button
        class:on={cross.kind === k.key}
        aria-pressed={cross.kind === k.key}
        onclick={() => (cross = { ...cross, kind: k.key, width: crossEditing ? cross.width : k.width })}
        title="{k.label} (X again)"><Icon name={k.icon} size={16} />{k.label}</button
      >
    {/each}
  </div>
  {#if cross.kind === 'bridge'}
    <div class="ws-seg" role="radiogroup" aria-label="Built of">
      {#each MATERIALS as m (m.key)}
        <button class:on={cross.material === m.key} aria-pressed={cross.material === m.key} onclick={() => (cross = { ...cross, material: m.key })}>{m.label}</button>
      {/each}
    </div>
  {/if}
  {#snippet hint()}
    <div class="ws-hint">
      {crossEditing ? 'Click both banks again to move it. ' : 'Click one bank, then the other; click one put down earlier to change it. '}{kindLook}
    </div>
  {/snippet}
  {#snippet more()}
    <div class="ws-field">
      Width (ft)
      <div class="stepper">
        <button class="ws-icon-btn" onclick={() => (cross.width = Math.max(widthMin, cross.width - 1))} disabled={cross.width <= widthMin} aria-label="Narrower"><Icon name="minus" size={16} /></button>
        <span>{cross.width}</span>
        <button class="ws-icon-btn" onclick={() => (cross.width = Math.min(widthMax, cross.width + 1))} disabled={cross.width >= widthMax} aria-label="Wider"><Icon name="plus" size={16} /></button>
      </div>
    </div>
    {#if cross.kind === 'bridge'}
      <label class="switch"><span class="ws-grow">Shops on the deck</span><input type="checkbox" class="ws-switch" checked={cross.shops} onchange={(e) => shopsOn(e.currentTarget.checked)} /></label>
      {#if cross.shops}
        <label class="ws-slider">
          <span>Deepest shops</span><output>{cross.shopDepth} ft</output>
          <input type="range" min={SHOP_DEPTH_RANGE[0]} max={SHOP_DEPTH_RANGE[1]} step="1" value={cross.shopDepth} oninput={(e) => shopDepth(Number(e.currentTarget.value))} />
        </label>
        <label class="ws-slider">
          <span>Lane between</span><output>{cross.shopLane} ft</output>
          <input type="range" min={SHOP_LANE_RANGE[0]} max={SHOP_LANE_RANGE[1]} step="1" value={cross.shopLane} oninput={(e) => shopLane(Number(e.currentTarget.value))} />
        </label>
        <label class="switch"><span class="ws-grow">Vary the shops</span><input type="checkbox" class="ws-switch" checked={cross.shopVary} onchange={(e) => (cross = { ...cross, shopVary: e.currentTarget.checked })} /></label>
        <div class="ws-hint">A row of shops down each side, the lane between them as wide as the bridge leaves. Varied, each shop has its own frontage, depth and storeys, with an open landing now and then. One undo takes the bridge and its shops away; changing the bridge lays them out again.</div>
      {/if}
    {/if}
    {#if crossEditing}<button class="ws-btn primary block" onclick={onCrossSave}>Save changes</button>{/if}
  {/snippet}
</ToolPanel>
