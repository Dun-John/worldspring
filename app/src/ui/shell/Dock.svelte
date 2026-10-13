<script lang="ts" module>
  import type { IconName } from '../icons';

  export interface DockTab {
    key: string;
    label: string;
    icon?: IconName;
    /** A mark on the tab (sketching, a session live). */
    dot?: boolean;
    disabled?: boolean;
    title?: string;
    kbd?: string;
    /** What the tab is for, one line under the strip while it is open. */
    hint?: string;
    /** A mark after the label (↻: settings changed, not generated yet), with its own tooltip. */
    badge?: { text: string; title: string };
  }
</script>

<script lang="ts">
  // The open section's panel: on the right under the section tabs (it folds down to its header
  // and tool strip), or on a phone a sheet over the tab bar, dragged between its tool strip
  // (peek), half the screen and all of it. The body is told when only the strip shows.
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';
  import { shell, type Snap } from './layout.svelte';

  interface Props {
    title: string;
    icon: IconName;
    tabs?: DockTab[];
    tab?: string;
    onTab?: (key: string) => void;
    onClose: () => void;
    /** Phones: the place card over a panel goes back to it ("Back to Edit"). */
    back?: { label: string; go: () => void };
    /** Buttons in the header (Stop playing). */
    actions?: Snippet;
    /** A question before work would be dropped, shown over the body. */
    ask?: Snippet;
    children: Snippet<[boolean]>;
  }
  let { title, icon, tabs, tab, onTab, onClose, back, actions, ask, children }: Props = $props();

  const peek = $derived(shell.phone ? shell.snap === 'peek' : shell.collapsed);

  // Room the dock takes, for what is anchored beside it (the right edge on wide screens, the
  // bottom on phones; the sheet's height is read once it settles).
  let sheetH = $state(0);
  $effect(() => {
    const root = document.documentElement.style;
    if (shell.phone) {
      root.setProperty('--dock-w', '0px');
      root.setProperty('--sheet-h', `${sheetH}px`);
    } else {
      root.setProperty('--dock-w', shell.short ? '312px' : '352px');
      root.setProperty('--sheet-h', '0px');
    }
    return () => {
      root.setProperty('--dock-w', '0px');
      root.setProperty('--sheet-h', '0px');
    };
  });

  // Its edges, for the place bar (kept beside it, never under it).
  let el: HTMLElement | undefined = $state();
  $effect(() => {
    void [sheetH, shell.snap, shell.collapsed, shell.phone, shell.short];
    const measure = () => {
      const r = el?.getBoundingClientRect();
      shell.sheetTop = r && shell.phone ? r.top : Infinity;
      shell.dockLeft = r && !shell.phone ? r.left : Infinity;
    };
    measure();
    window.addEventListener('resize', measure);
    return () => {
      window.removeEventListener('resize', measure);
      shell.sheetTop = shell.dockLeft = Infinity;
    };
  });

  // Phones: drag the handle (or the header) to change how far the sheet is up.
  const ORDER: Snap[] = ['peek', 'half', 'full'];
  let drag: { y: number; dy: number; id: number } | null = $state(null);
  function down(e: PointerEvent) {
    if (!shell.phone || (e.target as HTMLElement).closest('button:not(.handle), input, select')) return;
    drag = { y: e.clientY, dy: 0, id: e.pointerId };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (drag && e.pointerId === drag.id) drag.dy = e.clientY - drag.y;
  }
  function up(e: PointerEvent) {
    if (!drag || e.pointerId !== drag.id) return;
    const { dy } = drag;
    drag = null;
    const i = ORDER.indexOf(shell.snap);
    // A tap on the handle steps the sheet up (from full, back to peek); a drag goes its way.
    if (Math.abs(dy) < 8) {
      if ((e.target as HTMLElement).closest('.handle')) shell.snap = ORDER[(i + 1) % ORDER.length];
      return;
    }
    const steps = Math.abs(dy) > 220 ? 2 : 1;
    shell.snap = ORDER[Math.max(0, Math.min(2, i + (dy < 0 ? steps : -steps)))];
  }
</script>

<aside
  class="dock"
  class:peek
  class:phone={shell.phone}
  data-snap={shell.snap}
  aria-label={title}
  bind:clientHeight={sheetH}
  bind:this={el}
  style:transform={drag && shell.phone ? `translateY(${Math.max(-120, drag.dy)}px)` : undefined}
>
  <div class="top" role="presentation" onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={() => (drag = null)}>
    {#if shell.phone}<button class="handle" aria-label="Show more or less of the panel"><span></span></button>{/if}
    <header>
      {#if back}<button class="ws-btn quiet back" onclick={back.go} title="Back to {back.label}"><Icon name="chevron-left" size={16} />{back.label}</button>{/if}
      <Icon name={icon} />
      <b class="title">{title}</b>
      {#if actions}{@render actions()}{/if}
      {#if !shell.phone}
        <button class="ws-icon-btn" onclick={() => (shell.collapsed = !shell.collapsed)} aria-label={peek ? 'Unfold' : 'Fold'} title={peek ? 'Unfold the panel' : 'Fold the panel down to its tools'}>
          <Icon name={peek ? 'chevron-down' : 'chevron-up'} />
        </button>
      {/if}
      <button class="ws-icon-btn" onclick={onClose} aria-label="Close {title}" title="Close"><Icon name="x" /></button>
    </header>
    {#if tabs?.length}
      <!-- Five tabs or more: only the open one keeps its icon, so every label fits a narrow dock. -->
      <div class="tabs" class:many={tabs.length >= 5} role="tablist">
        {#each tabs as t (t.key)}
          <button
            role="tab"
            aria-selected={tab === t.key}
            class:on={tab === t.key}
            disabled={t.disabled && tab !== t.key}
            title={t.title ?? (t.kbd ? `${t.label} (${t.kbd})` : t.label)}
            onclick={() => onTab?.(t.key)}
          >
            {#if t.icon}<Icon name={t.icon} size={16} />{/if}
            <span>{t.label}</span>
            {#if t.badge}<span class="badge" title={t.badge.title}>{t.badge.text}</span>{/if}
            {#if t.dot}<i class="dot"></i>{/if}
          </button>
        {/each}
      </div>
      {@const hint = tabs.find((t) => t.key === tab)?.hint}
      {#if hint && !peek}<div class="ws-hint sub">{hint}</div>{/if}
    {/if}
  </div>
  {#if ask}<div class="ask">{@render ask()}</div>{/if}
  <div class="body">
    {@render children(peek)}
  </div>
</aside>

<style>
  .dock {
    position: fixed;
    z-index: var(--z-dock);
    top: calc(12px + var(--bar-h, 44px) + 8px);
    right: 12px;
    width: 340px;
    max-height: calc(100dvh - 12px - var(--bar-h, 44px) - 8px - 12px);
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
    color: var(--ink);
    font: 13px/1.45 var(--font);
  }
  .top {
    flex: none;
    border-bottom: 1px solid var(--line-faint);
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 6px 4px 10px;
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: 15px;
    letter-spacing: 0.02em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 0 6px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .tabs button {
    flex: 1 1 auto;
    min-width: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    min-height: var(--tap);
    padding: 0 4px;
    font: inherit;
    font-size: 12px;
    color: var(--ink-2);
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    white-space: nowrap;
  }
  .tabs.many button {
    padding: 0 2px;
  }
  .tabs.many button:not(.on) :global(svg) {
    display: none;
  }
  .tabs button:hover:not(:disabled) {
    color: var(--ink);
    background: rgba(226, 213, 176, 0.5);
  }
  .tabs button.on {
    color: var(--ink);
    font-weight: bold;
    border-bottom-color: var(--accent);
  }
  .tabs button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .badge {
    color: var(--accent);
    font-weight: bold;
  }
  .sub {
    padding: 2px 10px 6px;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--gold);
  }
  .ask {
    flex: none;
    padding: 8px 8px 0;
  }
  .body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    padding: 10px;
    overscroll-behavior: contain;
  }
  :global([data-short]) .dock:not(.phone) {
    width: 300px;
  }
  .dock.peek .body:empty {
    display: none;
  }

  /* Phones: a sheet over the tab bar. */
  .dock.phone {
    top: auto;
    left: 0;
    right: 0;
    bottom: var(--tabbar-h, 0px);
    width: auto;
    border-radius: 12px 12px 0 0;
    border-left: none;
    border-right: none;
    border-bottom: none;
    transition: transform 0.08s;
  }
  .dock.phone[data-snap='half'] {
    height: 50dvh;
  }
  .dock.phone[data-snap='full'] {
    height: calc(100dvh - var(--tabbar-h, 0px) - var(--top-h, 60px) - 12px);
    max-height: none;
  }
  .dock.phone[data-snap='peek'] {
    max-height: 40dvh;
  }
  .phone .top {
    touch-action: none;
  }
  .handle {
    display: flex;
    justify-content: center;
    width: 100%;
    padding: 6px 0 2px;
    border: none;
    background: none;
    cursor: grab;
  }
  .handle span {
    width: 40px;
    height: 4px;
    border-radius: 2px;
    background: var(--line-soft);
  }
  .phone header {
    padding-top: 0;
  }
  .back {
    padding: 0 6px 0 2px;
    font-size: 12px;
  }
</style>
