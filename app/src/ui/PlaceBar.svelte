<script lang="ts">
  // On a touch screen, the buttons beside what is in the hand: turn it either way, let it turn by
  // itself again, put it down (the ghost stays for the next one), put it away; for a piece picked
  // up, take it away too. Below the ghost (above it when there is no room), kept clear of the dock,
  // the phone's sheet and the top bar.
  import Icon from './Icon.svelte';
  import { shell } from './shell/layout.svelte';

  interface Props {
    /** The ghost's middle and radius (screen px). */
    at: { x: number; y: number; r: number };
    holding: boolean;
    /** It shows a turn (else the turn buttons are left out). */
    turns: boolean;
    /** Turns go by quarters (inside), else by 45 degrees (the battlemap). */
    quarter: boolean;
    onTurn: (dir: 1 | -1) => void;
    onAuto: () => void;
    onPut: () => void;
    onCancel: () => void;
    onRemove: () => void;
  }
  let { at, holding, turns, quarter, onTurn, onAuto, onPut, onCancel, onRemove }: Props = $props();

  let w = $state(0);
  let h = $state(0);
  let vw = $state(0);
  let vh = $state(0);
  const GAP = 10;
  const EDGE = 8;
  const pos = $derived.by(() => {
    // (Below the top bar: 12 px down, its height set by TopBar.)
    const top0 = 12 + (parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--top-h')) || 44);
    const right = Math.min(vw, shell.dockLeft) - EDGE;
    const bottom = Math.min(vh, shell.sheetTop) - EDGE;
    let top = at.y + at.r + GAP;
    if (top + h > bottom) top = at.y - at.r - GAP - h;
    top = Math.max(top0 + EDGE, Math.min(bottom - h, top));
    // (Beside the map's buttons, not over them, where it is level with them.)
    const ctl = document.querySelector('[data-map-controls]')?.getBoundingClientRect();
    const edge = ctl && top < ctl.bottom && top + h > ctl.top ? Math.min(right, ctl.left - EDGE) : right;
    return { left: Math.max(EDGE, Math.min(edge - w, at.x - w / 2)), top };
  });
  const step = $derived(quarter ? 'a quarter' : '45°');
</script>

<svelte:window bind:innerWidth={vw} bind:innerHeight={vh} />

<div class="place-bar" role="toolbar" aria-label="What is in the hand" bind:clientWidth={w} bind:clientHeight={h} style="left: {pos.left}px; top: {pos.top}px">
  {#if turns}
    <button class="ws-icon-btn" onclick={() => onTurn(-1)} aria-label="Turn anticlockwise" title="Turn {step} anticlockwise"><Icon name="rotate-ccw" /></button>
    <button class="ws-icon-btn" onclick={() => onTurn(1)} aria-label="Turn clockwise" title="Turn {step} clockwise"><Icon name="rotate-cw" /></button>
    <button class="ws-btn quiet" onclick={onAuto} title={quarter ? 'Face the way it would by itself' : 'Turned at random again'}>Auto</button>
  {/if}
  {#if holding}<button class="ws-icon-btn" onclick={onRemove} aria-label="Take it away" title="Take it away"><Icon name="trash" /></button>{/if}
  <button class="ws-icon-btn put" onclick={onPut} aria-label="Put it down" title={holding ? 'Put it down here' : 'Put one down here'}><Icon name="check" /></button>
  <button class="ws-icon-btn" onclick={onCancel} aria-label={holding ? 'Put it back' : 'Put it away'} title={holding ? 'Put it back where it was' : 'Put it away (nothing in the hand)'}><Icon name="x" /></button>
</div>

<style>
  .place-bar {
    position: fixed;
    z-index: var(--z-chip);
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 2px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
  }
  .put {
    background: var(--accent);
    color: var(--accent-ink);
  }
</style>
