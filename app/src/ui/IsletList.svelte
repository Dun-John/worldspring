<script lang="ts">
  // The world's own islets, those taken away marked: fly to one, take it away or bring it back.
  // Shown by Docks & islets.
  export interface Islet {
    id: string;
    x: number;
    y: number;
    r: number;
    river: boolean;
    removed: boolean;
    tower: boolean;
  }

  interface Props {
    islets: Islet[];
    onIslet: (id: string, removed: boolean) => void;
    onGoIslet: (x: number, y: number) => void;
  }

  let { islets, onIslet, onGoIslet }: Props = $props();
</script>

{#if islets.length}
  <details class="card" aria-label="The world's islets">
    <summary>Islets in the world <span class="ws-muted">· {islets.filter((i) => !i.removed).length} of {islets.length}</span></summary>
    {#each islets as i (i.id)}
      <div class="ws-row">
        <button class="ws-chip ws-grow" onclick={() => onGoIslet(i.x, i.y)} title="Fly there">{i.river ? 'River' : 'Coast'} islet, {Math.round(i.r)} ft{i.tower ? ', watchtower' : ''}{i.removed ? ' (taken away)' : ''}</button>
        <button class="ws-chip" onclick={() => onIslet(i.id, !i.removed)}>{i.removed ? 'Bring back' : 'Take away'}</button>
      </div>
    {/each}
  </details>
{/if}

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }
</style>
