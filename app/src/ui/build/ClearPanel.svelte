<script lang="ts">
  // Build › Clear: take the town's own buildings away in a box or a lasso.
  import type { ClearShape } from '../../editor/clearArea';
  import Icon from '../Icon.svelte';
  import ToolPanel from './ToolPanel.svelte';

  interface Props {
    /** A box dragged, or a lasso drawn. */
    clearShape: ClearShape;
    near: boolean;
    peek?: boolean;
    onZoomIn: () => void;
  }

  let { clearShape = $bindable(), near, peek = false, onZoomIn }: Props = $props();
</script>

<ToolPanel title="Clear" {near} {peek} zoomText="Zoom in to clear an area." {onZoomIn}>
  <div class="ws-seg" role="radiogroup" aria-label="Area">
    <button class:on={clearShape === 'box'} aria-pressed={clearShape === 'box'} onclick={() => (clearShape = 'box')}><Icon name="square" size={16} />Box</button>
    <button class:on={clearShape === 'lasso'} aria-pressed={clearShape === 'lasso'} onclick={() => (clearShape = 'lasso')}><Icon name="polygon" size={16} />Lasso</button>
  </div>
  {#snippet hint()}
    <div class="ws-hint">
      {clearShape === 'box' ? 'Drag a box' : 'Draw round an area'}: the town's own buildings with their middle inside it are taken away, leaving the ground open to build on.
      Undo brings them back.
    </div>
  {/snippet}
</ToolPanel>
