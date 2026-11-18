<script lang="ts">
  // The four sections (World, Edit, Notes, Play) with undo and redo: top right on wide screens,
  // the tab bar along the bottom on phones (undo is with the map controls there).
  import Icon from '../Icon.svelte';
  import { mac } from './shortcuts';
  import { NEW_UI, SECTIONS, shell, type Section } from './layout.svelte';

  interface Props {
    /** A play session is running. */
    playing: boolean;
    canUndo: boolean;
    canRedo: boolean;
    undoLabel: string | null;
    redoLabel: string | null;
    onSection: (s: Section) => void;
    onUndo: () => void;
    onRedo: () => void;
  }
  let { playing, canUndo, canRedo, undoLabel, redoLabel, onSection, onUndo, onRedo }: Props = $props();

  const ctrl = mac ? '⌘' : 'Ctrl+';
  let height = $state(0);
  let width = $state(0);
  $effect(() => {
    const root = document.documentElement.style;
    root.setProperty(shell.phone ? '--tabbar-h' : '--bar-h', `${height}px`);
    root.setProperty(shell.phone ? '--bar-h' : '--tabbar-h', '0px');
    root.setProperty('--bar-w', shell.phone ? '0px' : `${width}px`);
  });
</script>

<nav class="bar" class:nu={NEW_UI} class:phone={shell.phone} aria-label="Sections" bind:clientHeight={height} bind:clientWidth={width}>
  <div class="tabs">
    {#each SECTIONS as s (s.key)}
      <button
        class="tab"
        class:on={shell.section === s.key}
        aria-pressed={shell.section === s.key}
        title="{s.label} ({s.kbd}){s.key === 'play' && playing ? ': playing now' : ''}"
        onclick={() => onSection(s.key)}
      >
        <span class="icon">
          <Icon name={s.icon} size={shell.phone ? 20 : 17} />
          {#if s.key === 'play' && playing}<i class="live" aria-label="playing"></i>{/if}
        </span>
        <span class="label">{s.label}</span>
        {#if !shell.phone}<kbd class="ws-kbd">{s.kbd}</kbd>{/if}
      </button>
    {/each}
  </div>
  {#if !shell.phone && !NEW_UI}
    <span class="sep"></span>
    <button class="ws-icon-btn" onclick={onUndo} disabled={!canUndo} aria-label="Undo" title={undoLabel ? `Undo: ${undoLabel} (${ctrl}Z)` : `Undo (${ctrl}Z)`}><Icon name="undo" /></button>
    <button class="ws-icon-btn" onclick={onRedo} disabled={!canRedo} aria-label="Redo" title={redoLabel ? `Redo: ${redoLabel} (${ctrl}Y)` : `Redo (${ctrl}Y)`}><Icon name="redo" /></button>
  {/if}
</nav>

<style>
  .bar {
    position: fixed;
    z-index: var(--z-bar);
    top: 12px;
    right: 12px;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    color: var(--ink);
    font: 13px/1.2 var(--font);
  }
  .tabs {
    display: flex;
    gap: 2px;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: var(--tap);
    padding: 0 9px;
    font: inherit;
    color: var(--ink);
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .tab:hover {
    background: var(--btn-hover);
  }
  .tab.on {
    background: var(--accent);
    color: var(--accent-ink);
  }
  .tab.on .ws-kbd {
    background: transparent;
    color: var(--accent-ink);
    border-color: rgba(245, 236, 214, 0.5);
  }
  .icon {
    position: relative;
    display: inline-flex;
  }
  .live {
    position: absolute;
    top: -3px;
    right: -4px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #c2410c;
    box-shadow: 0 0 0 2px var(--paper-solid);
  }
  .sep {
    width: 1px;
    align-self: stretch;
    margin: 3px 4px;
    background: var(--line-faint);
  }
  /* ?ui=new: the same height as the top bar (--tap + 8 px) and gutter; Undo and Redo are with the map controls. */
  .bar.nu:not(.phone) {
    box-sizing: border-box;
    min-height: calc(var(--tap) + 8px);
  }
  /* Narrow screens: the tabs show their icons only. */
  @media (max-width: 1100px) {
    .bar:not(.phone) .label,
    .bar:not(.phone) .ws-kbd {
      display: none;
    }
  }

  /* Phones: a tab bar along the bottom. */
  .bar.phone {
    top: auto;
    left: 0;
    right: 0;
    bottom: 0;
    padding: 2px 4px max(2px, env(safe-area-inset-bottom));
    border-radius: 0;
    border-left: none;
    border-right: none;
    border-bottom: none;
    box-shadow: 0 -1px 4px rgba(0, 0, 0, 0.15);
  }
  .phone .tabs {
    flex: 1;
  }
  .phone .tab {
    flex: 1;
    flex-direction: column;
    gap: 1px;
    min-height: 48px;
    padding: 4px 0 2px;
    font-size: 11px;
  }
  .phone .tab.on {
    background: none;
    color: var(--accent);
    font-weight: bold;
  }
  .phone .tab.on .icon {
    background: var(--btn-on);
    border-radius: 999px;
    padding: 1px 12px;
  }
</style>
