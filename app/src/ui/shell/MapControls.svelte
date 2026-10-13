<script lang="ts">
  // Bottom right, beside the dock: inside a building or site its floor picker, then the map's
  // own controls: zoom, the whole map, and Layers (the battlemap grid, place names). On phones,
  // where there is no keyboard: undo and Layers (pinch zooms).
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';
  import { shell } from './layout.svelte';

  interface Props {
    grid: boolean;
    places: boolean;
    /** Zoomed in far enough for the grid to show. */
    battlemap: boolean;
    canUndo: boolean;
    undoLabel: string | null;
    onGrid: (on: boolean) => void;
    onPlaces: (on: boolean) => void;
    onZoom: (d: number) => void;
    onWhole: () => void;
    onUndo: () => void;
    floors?: Snippet;
  }
  let { grid, places, battlemap, canUndo, undoLabel, onGrid, onPlaces, onZoom, onWhole, onUndo, floors }: Props = $props();

  const layers = $derived(shell.menu === 'layers');
</script>

<div class="controls" class:phone={shell.phone} data-map-controls>
  {#if floors}{@render floors()}{/if}
  {#if shell.phone && canUndo}
    <button class="ctl" onclick={onUndo} aria-label="Undo" title={undoLabel ? `Undo: ${undoLabel}` : 'Undo'}><Icon name="undo" /></button>
  {/if}
  <div class="layers-wrap">
    {#if layers}
      <div class="scrim" role="presentation" onclick={() => (shell.menu = null)}></div>
      <div class="pop ws-panel" role="dialog" aria-label="Layers">
        <label class="row">
          <Icon name="grid" />
          <span class="text">Battlemap grid<span class="ws-muted">{battlemap ? 'The 5-ft squares' : 'Zoom in to see it'}</span></span>
          <kbd class="ws-kbd">G</kbd>
          <input type="checkbox" class="ws-switch" checked={grid} onchange={(e) => onGrid(e.currentTarget.checked)} />
        </label>
        <label class="row">
          <Icon name="pin" />
          <span class="text">Place names<span class="ws-muted">Inns, shops, temples in town</span></span>
          <kbd class="ws-kbd">P</kbd>
          <input type="checkbox" class="ws-switch" checked={places} onchange={(e) => onPlaces(e.currentTarget.checked)} />
        </label>
      </div>
    {/if}
    <button class="ctl" class:on={layers} onclick={() => (shell.menu = layers ? null : 'layers')} aria-label="Layers" aria-expanded={layers} title="Layers: grid, place names"><Icon name="layers" /></button>
  </div>
  {#if !shell.phone}
    <div class="stack">
      <button class="ctl" onclick={() => onZoom(1)} aria-label="Zoom in" title="Zoom in (+)"><Icon name="plus" /></button>
      <button class="ctl" onclick={() => onZoom(-1)} aria-label="Zoom out" title="Zoom out (−)"><Icon name="minus" /></button>
    </div>
    <button class="ctl" onclick={onWhole} aria-label="Whole map" title="Whole map (Home)"><Icon name="expand" /></button>
  {/if}
</div>

<style>
  .controls {
    position: fixed;
    z-index: var(--z-hud);
    right: calc(var(--dock-w, 0px) + 12px);
    bottom: 12px;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
  }
  .ctl {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    padding: 0;
    color: var(--ink);
    background: var(--paper-solid);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    cursor: pointer;
  }
  .ctl:hover {
    background: var(--btn-hover);
  }
  .ctl.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .stack {
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow);
    border-radius: var(--radius);
  }
  .stack .ctl {
    box-shadow: none;
  }
  .stack .ctl:first-child {
    border-radius: var(--radius) var(--radius) 0 0;
    border-bottom-color: var(--line-faint);
  }
  .stack .ctl:last-child {
    border-radius: 0 0 var(--radius) var(--radius);
    border-top: none;
  }
  .layers-wrap {
    position: relative;
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 0;
  }
  .pop {
    position: absolute;
    right: calc(100% + 8px);
    bottom: 0;
    z-index: 1;
    width: 270px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--paper-solid);
    box-shadow: var(--shadow-lg);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: calc(var(--tap) + 6px);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .row:hover {
    background: var(--btn-hover);
  }
  .text {
    flex: 1;
    display: flex;
    flex-direction: column;
    line-height: 1.25;
  }
  .text .ws-muted {
    font-size: 11px;
  }

  .phone {
    right: 8px;
    bottom: calc(var(--tabbar-h, 0px) + var(--sheet-h, 0px) + 10px);
  }
  .phone .ctl {
    width: 44px;
    height: 44px;
  }
  .phone .pop {
    right: calc(100% + 8px);
    width: min(270px, calc(100vw - 80px));
  }
</style>
