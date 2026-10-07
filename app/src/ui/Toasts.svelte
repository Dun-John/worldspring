<script lang="ts" module>
  export interface Toast {
    id: number;
    text: string;
    /** Undo what the toast reports. */
    undo?: () => void;
    /** Another button (instead of Undo). */
    action?: { label: string; run: () => void };
    /** Stays until dismissed. */
    sticky?: boolean;
  }
</script>

<script lang="ts">
  // Short notices at the bottom of the map: edits made (by you or an agent), with Undo.
  interface Props {
    toasts: Toast[];
    onDismiss: (id: number) => void;
  }
  let { toasts, onDismiss }: Props = $props();
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="toast">
      <span>{t.text}</span>
      {#if t.undo}
        <button
          onclick={() => {
            t.undo?.();
            onDismiss(t.id);
          }}>Undo</button
        >
      {:else if t.action}
        <button
          onclick={() => {
            t.action?.run();
            onDismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button class="close" onclick={() => onDismiss(t.id)} aria-label="Dismiss">×</button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    left: 50%;
    bottom: calc(var(--tabbar-h, 0px) + var(--sheet-h, 0px) + 12px + var(--readout-h, 40px) + 8px);
    transform: translateX(-50%);
    z-index: var(--z-toast);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: min(480px, calc(100vw - 32px));
    padding: 5px 6px 5px 11px;
    background: rgba(43, 36, 29, 0.92);
    color: #f3ecd8;
    border-radius: 4px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
    font: 13px/1.35 Georgia, 'Times New Roman', serif;
  }
  button {
    border: 1px solid #8a7a60;
    border-radius: 3px;
    background: transparent;
    color: #f3d9a0;
    cursor: pointer;
    font: inherit;
    padding: 1px 8px;
    white-space: nowrap;
  }
  button:hover {
    background: rgba(243, 236, 216, 0.15);
  }
  .close {
    border: none;
    color: #cbbd9f;
    font-size: 16px;
    line-height: 1;
    padding: 0 4px;
  }
</style>
