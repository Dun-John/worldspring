<script lang="ts">
  // Bottom right, beside the dock: inside a building or site its floor picker, then the map's
  // own controls: zoom, the whole map, and Layers (the battlemap grid, place names, sea lanes, the
  // overland hex grid and its settings, which are the world's: changed as edits). On phones,
  // where there is no keyboard: undo and Layers (pinch zooms).
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';
  import { shell } from './layout.svelte';
  import { HEX_SIZE_MI, HEX_WIDTH_PX, type Hex } from '../../render/hex';

  interface Props {
    grid: boolean;
    places: boolean;
    seaLanes: boolean;
    /** Zoomed in far enough for the grid to show. */
    battlemap: boolean;
    canUndo: boolean;
    undoLabel: string | null;
    onGrid: (on: boolean) => void;
    onPlaces: (on: boolean) => void;
    onSeaLanes: (on: boolean) => void;
    /** The hex grid: shown here or not, the world's settings, and whether they were ever set. */
    hexes: { on: boolean; grid: Hex; set: boolean };
    onHexes: (on: boolean) => void;
    /** The world's hex grid changed (null: back to the default). */
    onHexGrid: (grid: Hex | null) => void;
    onZoom: (d: number) => void;
    onWhole: () => void;
    onUndo: () => void;
    floors?: Snippet;
    /** Inside a building or site: the view turned so its plan stands north up. */
    north?: { on: boolean; set: (on: boolean) => void };
  }
  let { grid, places, seaLanes, hexes, battlemap, canUndo, undoLabel, onGrid, onPlaces, onSeaLanes, onHexes, onHexGrid, onZoom, onWhole, onUndo, floors, north }: Props = $props();

  const layers = $derived(shell.menu === 'layers');
  let hexOpen = $state(false);
  /** A slider's value while it is dragged (changed in the edits when let go: one undo step). */
  let live = $state<Partial<Hex>>({});
  const hg = $derived({ ...hexes.grid, ...live });
  function commit(patch: Partial<Hex>) {
    live = {};
    const next = { ...hexes.grid, ...patch };
    if (Object.keys(patch).some((k) => next[k as keyof Hex] !== hexes.grid[k as keyof Hex])) onHexGrid(next);
  }
  /** A number field's value, or the old one when it doesn't read as one. */
  function num(el: HTMLInputElement, old: number, lo: number, hi: number): number {
    const v = Number(el.value);
    const out = Number.isFinite(v) && el.value.trim() !== '' ? Math.min(hi, Math.max(lo, Math.round(v * 100) / 100)) : old;
    el.value = String(out);
    return out;
  }
</script>

<div class="controls" class:phone={shell.phone} data-map-controls>
  {#if floors}{@render floors()}{/if}
  {#if north}
    <button class="ctl" class:on={north.on} onclick={() => north.set(!north.on)} aria-label="North up" aria-pressed={north.on} title={north.on ? 'Back to the map as it lies (Shift+N)' : 'Turn the plan north up (Shift+N)'}><Icon name="north" /></button>
  {/if}
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
        <label class="row">
          <Icon name="ferry" />
          <span class="text">Sea lanes<span class="ws-muted">Shipping between ports</span></span>
          <kbd class="ws-kbd">Shift+L</kbd>
          <input type="checkbox" class="ws-switch" checked={seaLanes} onchange={(e) => onSeaLanes(e.currentTarget.checked)} />
        </label>
        <label class="row">
          <Icon name="hex" />
          <span class="text">Hex grid<span class="ws-muted">{hexes.grid.size_mi} mi hexes, for overland travel</span></span>
          <kbd class="ws-kbd">Shift+H</kbd>
          <input type="checkbox" class="ws-switch" checked={hexes.on} onchange={(e) => onHexes(e.currentTarget.checked)} />
        </label>
        {#if hexes.on}
          <details class="ws-group hex-set" bind:open={hexOpen}>
            <summary>Hex grid settings</summary>
            <div class="ws-group-body">
              <div class="two">
                <label class="ws-field"
                  >Size (mi across)
                  <input class="ws-input" type="number" min={HEX_SIZE_MI[0]} max={HEX_SIZE_MI[1]} step="0.5" value={hg.size_mi} onchange={(e) => commit({ size_mi: num(e.currentTarget, hg.size_mi, HEX_SIZE_MI[0], HEX_SIZE_MI[1]) })} />
                </label>
                <div class="ws-field">
                  Tops
                  <div class="ws-seg">
                    <button aria-pressed={hg.flat} onclick={() => commit({ flat: true })}>Flat</button>
                    <button aria-pressed={!hg.flat} onclick={() => commit({ flat: false })}>Pointy</button>
                  </div>
                </div>
              </div>
              <label class="slider">
                <span>Line width</span><output>{hg.width} px</output>
                <input type="range" min={HEX_WIDTH_PX[0]} max={HEX_WIDTH_PX[1]} step="0.5" value={hg.width} oninput={(e) => (live = { ...live, width: Number(e.currentTarget.value) })} onchange={(e) => commit({ width: Number(e.currentTarget.value) })} />
              </label>
              <label class="slider">
                <span>Opacity</span><output>{Math.round(hg.opacity * 100)}%</output>
                <input type="range" min="0.05" max="1" step="0.05" value={hg.opacity} oninput={(e) => (live = { ...live, opacity: Number(e.currentTarget.value) })} onchange={(e) => commit({ opacity: Number(e.currentTarget.value) })} />
              </label>
              <div class="two">
                <label class="ws-field colour">
                  Colour
                  <input type="color" value={hg.color} onchange={(e) => commit({ color: e.currentTarget.value })} />
                </label>
                <label class="ws-field">
                  Numbers (0412)
                  <input type="checkbox" class="ws-switch" checked={hg.numbers} onchange={(e) => commit({ numbers: e.currentTarget.checked })} />
                </label>
              </div>
              <div class="two">
                <label class="ws-field"
                  >Move right (mi)
                  <input class="ws-input" type="number" min={-hg.size_mi} max={hg.size_mi} step="0.1" value={hg.ox_mi} onchange={(e) => commit({ ox_mi: num(e.currentTarget, hg.ox_mi, -hg.size_mi, hg.size_mi) })} />
                </label>
                <label class="ws-field"
                  >Move down (mi)
                  <input class="ws-input" type="number" min={-hg.size_mi} max={hg.size_mi} step="0.1" value={hg.oy_mi} onchange={(e) => commit({ oy_mi: num(e.currentTarget, hg.oy_mi, -hg.size_mi, hg.size_mi) })} />
                </label>
              </div>
              <span class="ws-muted">Numbers show once hexes are big enough on screen. The grid is saved with the world.</span>
              <button class="ws-btn quiet" disabled={!hexes.set} onclick={() => onHexGrid(null)}>Back to the default grid</button>
            </div>
          </details>
        {/if}
      </div>
    {/if}
    <button class="ctl" class:on={layers} onclick={() => (shell.menu = layers ? null : 'layers')} aria-label="Layers" aria-expanded={layers} title="Layers: grid, place names, sea lanes, hexes"><Icon name="layers" /></button>
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
    max-height: calc(100dvh - 130px);
    overflow-y: auto;
  }
  .hex-set {
    margin: 0 6px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    align-items: end;
  }
  .two .ws-input {
    width: 100%;
  }
  .colour input {
    width: 100%;
    height: calc(var(--tap) - 2px);
    padding: 0 2px;
    background: var(--field);
    border: 1px solid var(--line-field);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .slider {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    font-size: 12px;
    color: var(--ink-2);
  }
  .slider output {
    font: 11px var(--mono);
  }
  .slider input {
    grid-column: 1 / 3;
    width: 100%;
    margin: 0;
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
    max-height: calc(100dvh - var(--tabbar-h, 0px) - var(--sheet-h, 0px) - 150px);
  }
</style>
