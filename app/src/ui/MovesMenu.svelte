<script lang="ts">
  // The ways on from a square inside a site (stairs, the way out, tunnels), at the click; kept
  // inside the window. The DM's context menu adds a room's notes and a note put down there.
  import type { Move } from '../render/InteriorLayer';

  interface Props {
    moves: Move[];
    x: number;
    y: number;
    onMove: (m: Move) => void;
    onClose: () => void;
  }
  let { moves, x, y, onMove, onClose }: Props = $props();

  let w = $state(0);
  let h = $state(0);
  const left = $derived(Math.max(8, Math.min(x + 12, innerWidth - w - 8)));
  const top = $derived(Math.max(8, Math.min(y - 8, innerHeight - h - 8)));
</script>

<div class="menu" style:left="{left}px" style:top="{top}px" role="menu" bind:clientWidth={w} bind:clientHeight={h}>
  {#each moves as m (m.label)}
    <button role="menuitem" onclick={() => onMove(m)}>
      <span class="icon">{m.kind === 'level' ? (/up/i.test(m.label) ? '▲' : '▼') : m.kind === 'surface' ? '⇧' : m.kind === 'room' ? '▢' : m.kind === 'note' ? '✎' : '➜'}</span>
      {m.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: var(--z-menu);
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 5px;
    background: rgba(243, 236, 216, 0.97);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    font: 13px/1.35 var(--font);
    max-width: min(320px, calc(100vw - 16px));
  }
  button {
    display: flex;
    gap: 7px;
    align-items: baseline;
    min-height: var(--tap);
    text-align: left;
    padding: 4px 8px;
    border: 1px solid var(--line-soft);
    border-radius: 3px;
    background: #ece2c6;
    color: var(--ink);
    cursor: pointer;
    font: inherit;
  }
  button:hover {
    background: var(--btn-on);
  }
  .icon {
    width: 12px;
    color: #6b4a2c;
  }
</style>
