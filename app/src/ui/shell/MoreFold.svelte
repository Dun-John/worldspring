<script lang="ts">
  // a tool's "More options" fold for the panels not on `ToolPanel` (Places, Streets,
  // Docks' islets list…), shut at first and remembered per tool as `ToolPanel`'s is.
  import type { Snippet } from 'svelte';
  import { foldOpen, setFold } from './layout.svelte';

  interface Props {
    /** The tool (its `editTools` key): the fold is remembered under it. */
    tool: string;
    label?: string;
    children: Snippet;
  }

  let { tool, label = 'More options', children }: Props = $props();
</script>

<details class="ws-group" open={foldOpen(tool)} ontoggle={(e) => setFold(tool, e.currentTarget.open)}>
  <summary>{label}</summary>
  <div class="ws-group-body">{@render children()}</div>
</details>
