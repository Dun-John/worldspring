<script lang="ts">
  // The frame every Build tool's panel shares: what is being changed (Done, Remove), the tool's
  // strip, "Zoom in" while too far out, the one-line hint, its options (a "More options" fold
  // when `fold`, open or shut as last left for this tool), and the place / start-over line.
  // `?ui=new`: every tool's options fold (shut at first) and its hint goes last in the fold, the
  // strip's one-line hint saying what to do first.
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';
  import { NEW_UI, shell } from '../shell/layout.svelte';
  import QuickLine from './QuickLine.svelte';
  import type { QuickInfo } from '../BuildPanel.svelte';

  interface Props {
    /** The tool's name: its fold is remembered under it. */
    title: string;
    near: boolean;
    /** Only the strip (the panel is folded down). */
    peek?: boolean;
    /** What the "Zoom in" line says. */
    zoomText?: string;
    onZoomIn: () => void;
    /** What is being changed (the line's words), else nothing is. */
    changing?: Snippet;
    onRemove?: () => void;
    onDone?: () => void;
    /** The tool's strip (shown folded down too). */
    children?: Snippet;
    /** Under the zoom line, folded down too. */
    always?: Snippet;
    hint?: Snippet;
    /** The hint after the options, not before. */
    hintAfter?: boolean;
    /** The hint while folded down. */
    peekHint?: Snippet;
    more?: Snippet;
    /** The options in a "More options" fold (starts open). */
    fold?: boolean;
    /** A start-over line (`startExtra` before its button). */
    startOver?: boolean;
    startExtra?: Snippet;
    onDrop?: () => void;
    quick?: QuickInfo;
    /** The place line's button, else none. */
    place?: string;
    onPlace?: () => void;
  }

  let { title, near, peek = false, zoomText = 'Zoom in to draw.', onZoomIn, changing, onRemove, onDone, children, always, hint, hintAfter = false, peekHint, more, fold = false, startOver = false, startExtra, onDrop, quick, place, onPlace }: Props = $props();

  const key = $derived(NEW_UI ? `ws-fold:tool:${shell.toolNow() ?? title}` : `ws-fold:build:${title}`);
  function readOpen(k: string): boolean {
    try {
      const v = localStorage.getItem(k);
      return NEW_UI ? v === 'open' : v !== 'shut';
    } catch {
      return !NEW_UI;
    }
  }
  let open = $derived(readOpen(key));
  function toggled(e: Event & { currentTarget: HTMLDetailsElement }) {
    open = e.currentTarget.open;
    try {
      localStorage.setItem(key, open ? 'open' : 'shut');
    } catch {
      // (No storage: it stays as it is for this visit.)
    }
  }
</script>

{#if changing}
  <div class="editing">
    <span class="grow">{@render changing()}</span>
    {#if onRemove}<button class="ws-btn" onclick={onRemove} title="Take it away"><Icon name="trash" size={16} /> Remove</button>{/if}
    <button class="ws-btn" onclick={onDone}>Done</button>
  </div>
{/if}
{@render children?.()}
{#if !near}
  <div class="zoom">
    <span class="grow">{zoomText}</span>
    <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
  </div>
{/if}
{@render always?.()}
{#if peek}
  {@render peekHint?.()}
  <!-- (`?ui=new`: Place stays in reach with the sheet down.) -->
  {#if NEW_UI && place && quick && onPlace}<QuickLine {quick} action={place} {onPlace} />{/if}
{:else}
  {#if NEW_UI}
    {#if more || hint}
      <details class="more" {open} ontoggle={toggled}>
        <summary>More options</summary>
        {@render more?.()}
        {@render hint?.()}
      </details>
    {/if}
  {:else}
    {#if !hintAfter}{@render hint?.()}{/if}
    {#if more && fold}
      <details class="more" {open} ontoggle={toggled}>
        <summary>More options</summary>
        {@render more()}
      </details>
    {:else}
      {@render more?.()}
    {/if}
    {#if hintAfter}{@render hint?.()}{/if}
  {/if}
  {#if startOver}
    <div class="ws-seg">
      {@render startExtra?.()}
      <button onclick={onDrop} disabled={!quick?.can && !quick?.text}>Start over</button>
    </div>
  {/if}
  {#if place && quick && onPlace}<QuickLine {quick} action={place} {onPlace} />{/if}
{/if}

<style>
  .editing,
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
  }
  .more {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .more > summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--ink-2);
  }
</style>
