<script lang="ts">
  // The place line: what the ghost would put down (or why not), and its button.
  import type { QuickInfo } from '../BuildPanel.svelte';

  let { quick, action, onPlace }: { quick: QuickInfo; action: string; onPlace: () => void } = $props();
</script>

<div class="quick" class:bad={quick.bad}>
  <span class="grow">{quick.text}</span>
  <button class="ws-btn primary" onclick={onPlace} disabled={!quick.can}>{action}</button>
</div>

<style>
  .quick {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .quick.bad span {
    color: var(--danger, #dc2626);
  }
</style>
