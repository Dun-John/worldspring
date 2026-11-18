<script lang="ts">
  // Top left: the ☰ menu and the search box, then where the view is (breadcrumbs, each flies
  // there), or while a world is being made, how far along it is.
  import type { Snippet } from 'svelte';
  import type { Crumb } from '../gazetteer';
  import Icon from '../Icon.svelte';
  import { shell } from './layout.svelte';

  interface Props {
    crumbs: Crumb[];
    onFly: (c: Crumb) => void;
    /** A world is being generated: what it is doing. */
    working: string | null;
    menuOpen: boolean;
    onMenu: () => void;
    /** The search box. */
    search: Snippet;
    /** The ☰ menu, when open. */
    menu: Snippet;
  }
  let { crumbs, onFly, working, menuOpen, onMenu, search, menu }: Props = $props();

  let height = $state(0);
  $effect(() => {
    document.documentElement.style.setProperty('--top-h', `${height}px`);
  });
</script>

<div class="top" bind:clientHeight={height}>
  <div class="box">
    <button class="ws-icon-btn menu-btn" class:on={menuOpen} onclick={onMenu} aria-label="Menu" aria-expanded={menuOpen} title="Menu"><Icon name="menu" /></button>
    {@render search()}
    <Icon name="search" size={16} />
  </div>
  <!-- (A wrapper of this component's own, so the menu takes clicks: the bar itself lets them
       through to the map.) -->
  {#if menuOpen}<div class="menu-wrap">{@render menu()}</div>{/if}
  {#if working}
    <div class="status" role="status"><span class="spin"></span>{working}</div>
  {:else if crumbs.length && !(shell.phone && shell.snap !== 'peek' && shell.section)}
    <nav class="crumbs" aria-label="Where the view is">
      {#each crumbs as c, i (c.label + i)}
        {#if i > 0}<span class="sep">›</span>{/if}
        <button onclick={() => onFly(c)}>{c.label}</button>
      {/each}
    </nav>
  {/if}
</div>

<style>
  .top {
    position: fixed;
    z-index: var(--z-search);
    top: 12px;
    left: 12px;
    width: min(400px, calc(100vw - var(--bar-w, 440px) - 36px));
    min-width: 260px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    pointer-events: none;
  }
  .top > * {
    pointer-events: auto;
  }
  .box {
    align-self: stretch;
    position: relative;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px 10px 3px 3px;
    background: var(--paper-solid);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    color: var(--ink-3);
  }
  .menu-btn.on {
    background: var(--btn-hover);
  }
  .crumbs,
  .status {
    max-width: 100%;
    box-sizing: border-box;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    padding: 2px 10px;
    background: rgba(243, 236, 216, 0.88);
    border: 1px solid rgba(58, 50, 42, 0.45);
    border-radius: 999px;
    font: 12px var(--font);
    color: var(--ink);
  }
  .crumbs button {
    all: unset;
    cursor: pointer;
  }
  .crumbs button:hover,
  .crumbs button:focus-visible {
    text-decoration: underline;
  }
  .sep {
    margin: 0 5px;
    color: #7a6a55;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .spin {
    width: 10px;
    height: 10px;
    border: 2px solid var(--line-soft);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  :global([data-layout='phone']) .top {
    top: max(8px, env(safe-area-inset-top));
    left: 8px;
    width: calc(100vw - 16px);
    min-width: 0;
  }
</style>
