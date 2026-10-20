<script lang="ts">
  // What is in the hand in Objects' Place and Rooms' Furniture (Props): the chosen item, or the piece
  // picked up; turned either way, back to the turn it takes by itself (Auto), sized, put away (×:
  // empty the hand, or put the held piece back).
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    name: string;
    /** A piece picked up (else the chosen item). */
    held: boolean;
    /** Its turn as shown ('45°', 'at random'), or ''. */
    turn: string;
    /** It shows a turn: the turn buttons. */
    turns: boolean;
    /** Its turn was given by hand: Auto puts it back. */
    auto: boolean;
    /** Turns go by quarters (inside). */
    quarter: boolean;
    onTurn: (d: 1 | -1) => void;
    onAuto: () => void;
    onEmpty: () => void;
    size?: Snippet;
    /** Its look, by the turn buttons (inside, for a piece with several). */
    look?: Snippet;
  }

  let { name, held, turn, turns, auto, quarter, onTurn, onAuto, onEmpty, size, look }: Props = $props();
</script>

<div class="inhand" role="group" aria-label="In hand">
  <span class="ws-chip on">
    {held ? 'Holding ' : ''}{name}{turn ? ` · ${turn}` : ''}
    <button class="x" onclick={onEmpty} aria-label={held ? 'Put it back' : 'Empty the hand'} title={held ? 'Put it back (Esc)' : 'Empty the hand (Esc)'}><Icon name="x" size={12} /></button>
  </span>
  {#if turns}
    <button class="ws-icon-btn" title={quarter ? 'Turn a quarter anticlockwise (Shift+T)' : 'Turn anticlockwise (Shift+T)'} aria-label="Turn anticlockwise" onclick={() => onTurn(-1)}><Icon name="rotate-ccw" size={16} /></button>
    <button class="ws-icon-btn" title={quarter ? 'Turn a quarter clockwise (T)' : 'Turn clockwise (T)'} aria-label="Turn clockwise" onclick={() => onTurn(1)}><Icon name="rotate-cw" size={16} /></button>
  {/if}
  {#if auto}<button class="ws-btn quiet" title={quarter ? 'Face the way it would by itself (Z)' : 'Turn at random again (Z)'} onclick={onAuto}>Auto</button>{/if}
  {#if look}{@render look()}{/if}
  {#if size}<span class="size">{@render size()}</span>{/if}
</div>

<style>
  .inhand {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .x {
    display: inline-flex;
    margin-right: -4px;
    padding: 2px;
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
  }
  .size {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 12px;
  }
  .size :global(output) {
    min-width: 3.2em;
    text-align: center;
    font: 11px var(--mono);
    color: var(--ink-2);
  }
  .size :global(input) {
    width: 3.4em;
  }
</style>
