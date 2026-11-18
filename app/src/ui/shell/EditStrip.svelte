<script lang="ts">
  // `?ui=new`: Edit's strip under the dock's header, the group row (five things to change) and
  // the tool row of the group shown, then the open tool's one-line hint until it has been used.
  // With Select open the row shows the last group's tools, none lit, each one tap away.
  import Icon from '../Icon.svelte';
  import { EDIT_TOOLS, GROUPS, TOOL, type GroupKey, type ToolKey } from './editTools';
  import { shell } from './layout.svelte';

  interface Props {
    /** The tool open. */
    tool: ToolKey | null;
    /** Only the rows (the panel is folded down). */
    peek?: boolean;
    /** Tools that can't be used here, with why. */
    off?: Partial<Record<ToolKey, string>>;
    onTool: (t: ToolKey) => void;
  }
  let { tool, peek = false, off = {}, onTool }: Props = $props();

  const tools = $derived(EDIT_TOOLS.filter((t) => t.group === shell.group));
  const openGroup = $derived(tool ? TOOL[tool].group : null);
  // Select belongs to no group: nothing is lit, and on phones the tool row waits for a group tap.
  const selecting = $derived(tool === 'select');
  let tapped = $state(false);
  $effect(() => {
    if (!selecting) tapped = false;
  });
  const lit = $derived(selecting && !tapped ? null : shell.group);
  const hint = $derived(tool && !peek && !shell.isUsed(tool) ? TOOL[tool].hint : null);
  const tip = (t: ToolKey) => off[t] ?? `${TOOL[t].label} (${TOOL[t].kbd}): ${TOOL[t].hint}`;
  function pick(g: GroupKey) {
    shell.group = g;
    tapped = true;
  }
</script>

<div class="strip" class:selecting class:idle={selecting && !tapped}>
  <div class="ws-strip groups" role="tablist" aria-label="What to change">
    {#each GROUPS as g (g.key)}
      <button role="tab" aria-selected={lit === g.key} class:has={openGroup === g.key} title={g.label} onclick={() => pick(g.key)}>
        <Icon name={g.icon} size={16} /><span>{g.label}</span>
      </button>
    {/each}
  </div>
  <div class="ws-seg tools" class:idle={selecting && !tapped} role="radiogroup" aria-label={GROUPS.find((g) => g.key === shell.group)?.label}>
    {#each tools as t (t.key)}
      <button class:on={tool === t.key} aria-pressed={tool === t.key} disabled={!!off[t.key] && tool !== t.key} title={tip(t.key)} onclick={() => onTool(t.key)}>
        <Icon name={t.icon} size={16} /><span>{t.short}</span>
      </button>
    {/each}
  </div>
  {#if hint}<div class="ws-hint line">{hint}</div>{/if}
</div>

<style>
  .groups {
    padding: 0 4px;
    gap: 0;
  }
  .groups > :global(button) {
    white-space: nowrap;
  }
  /* Phones: Select shows its card first, so its hint and the tool row wait. */
  :global(:root[data-layout='phone']) .tools.idle,
  :global(:root[data-layout='phone']) .strip.selecting .line {
    display: none;
  }
  .groups > button.has:not([aria-selected='true']) {
    color: var(--ink);
    font-weight: bold;
  }
  .tools {
    margin: 6px 6px 0;
  }
  .tools > button {
    flex-direction: column;
    gap: 0;
    padding: 4px 2px;
    font-size: 11px;
    line-height: 1.2;
    white-space: normal;
    text-align: center;
  }
  .strip {
    padding-bottom: 6px;
  }
  .line {
    padding: 4px 10px 0;
  }
</style>
