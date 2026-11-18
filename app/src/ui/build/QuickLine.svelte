<script lang="ts">
  // The place line: what the ghost would put down (or why not), and its button. The button
  // runs the sheet's width, and while it can't be used the line says why.
  import type { QuickInfo } from '../BuildPanel.svelte';

  let { quick, action, onPlace }: { quick: QuickInfo; action: string; onPlace: () => void } = $props();
</script>

<div class="quick stack" class:bad={quick.bad}>
  <span class="ws-grow">{quick.text || (!quick.can ? 'Click the map to choose a spot first.' : '')}</span>
  <button class="ws-btn primary" class:block={true} onclick={onPlace} disabled={!quick.can}>{action}</button>
</div>

<style>
  .quick {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .quick.stack {
    flex-direction: column-reverse;
    align-items: stretch;
    gap: 4px;
  }
  .quick.stack span:empty {
    display: none;
  }
  .quick.stack span {
    text-align: center;
  }
  .quick.bad span {
    color: var(--danger, #dc2626);
  }
  /* A disabled button keeps its ink (the page's 50% fade made it grey on beige). */
  .stack button:disabled {
    opacity: 1;
    background: var(--btn);
    color: var(--ink-2);
    border-style: dashed;
  }
</style>
