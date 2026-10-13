<script lang="ts">
  // The keyboard shortcuts, from the keymap, grouped; those that work right now stand out. On
  // touch screens, the gestures too.
  import Icon from '../Icon.svelte';
  import { keyLabel, type Shortcut } from './shortcuts';

  interface Props {
    list: Shortcut[];
    onClose: () => void;
  }
  let { list, onClose }: Props = $props();

  const ORDER = ['Panels', 'Map', 'World', 'Sketch', 'Edit', 'Build', 'Streets', 'Ground', 'Objects', 'Placing', 'Rooms', 'Play'];
  const NOTES: Record<string, string> = {
    Sketch: 'While sketching',
    Edit: 'With Edit open',
    Build: 'Edit › Build',
    Streets: 'Edit › Streets',
    Ground: 'Edit › Ground, on the battlemap',
    Objects: 'Edit › Objects',
    Placing: 'Edit › Objects, on the piece held, pointed at or chosen',
    Rooms: 'Edit › Rooms, inside a building or site',
    Play: 'While playing, with Play (or no panel) open',
  };
  /** What each section is for, for a first look. */
  const WHERE: [string, string][] = [
    ['World', 'Make the world: settings, the sketch (coasts, mountains, rivers, towns, roads), the library. Generating remakes the world.'],
    ['Edit', 'Change it in place: places, buildings, streets, the ground, objects, rooms.'],
    ['Notes', 'Your NPCs and plots, and the Gazetteer: every name to rename, places to hide from the map, your notes on places.'],
    ['Play', 'Run a session: tokens, fog, the players’ window.'],
  ];
  const groups = $derived(
    ORDER.map((g) => ({ name: g, rows: list.filter((s) => s.group === g && !s.hidden).map((s) => ({ s, live: !s.when || s.when() })) })).filter((g) => g.rows.length),
  );
  const touch = typeof matchMedia !== 'undefined' && matchMedia('(pointer: coarse)').matches;
</script>

<div class="scrim" role="presentation" onclick={onClose}></div>
<div class="help ws-panel" role="dialog" aria-label="Keyboard shortcuts" aria-modal="true">
  <header>
    <Icon name="keyboard" />
    <b>Keyboard shortcuts</b>
    <button class="ws-icon-btn" onclick={onClose} aria-label="Close" title="Close (Esc)"><Icon name="x" /></button>
  </header>
  <div class="cols">
    <section>
      <h3>Where things are</h3>
      <dl>
        {#each WHERE as [name, what] (name)}<dt>{name}</dt><dd>{what}</dd>{/each}
      </dl>
    </section>
    {#if touch}
      <section>
        <h3>Gestures</h3>
        <dl>
          <dt>Pinch</dt><dd>Zoom</dd>
          <dt>Drag</dt><dd>Pan (two fingers while drawing)</dd>
          <dt>Tap</dt><dd>Choose a place, open a door</dd>
          <dt>Double-tap</dt><dd>Go into a building, or zoom in</dd>
          <dt>Drag the sheet</dt><dd>More or less of a panel</dd>
        </dl>
      </section>
    {/if}
    {#each groups as g (g.name)}
      <section>
        <h3>{g.name}{#if NOTES[g.name]}<span class="note">{NOTES[g.name]}</span>{/if}</h3>
        <dl>
          {#each g.rows as { s, live } (s.label)}
            <dt class:dim={!live}>
              {#each s.keys as key, i (key)}{#if i}<span class="or">/</span>{/if}<kbd class="ws-kbd">{keyLabel(key)}</kbd>{/each}
            </dt>
            <dd class:dim={!live}>{s.label}</dd>
          {/each}
        </dl>
      </section>
    {/each}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: var(--z-overlay);
    background: rgba(30, 24, 18, 0.35);
  }
  .help {
    position: fixed;
    z-index: var(--z-overlay);
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(860px, calc(100vw - 24px));
    max-height: calc(100dvh - 48px);
    overflow-y: auto;
    padding: 12px 16px 16px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    margin-bottom: 6px;
  }
  header b {
    flex: 1;
  }
  .cols {
    columns: 3 250px;
    column-gap: 24px;
  }
  section {
    break-inside: avoid;
    margin-bottom: 12px;
  }
  h3 {
    margin: 0 0 4px;
    font-size: 13px;
    border-bottom: 1px solid var(--line-faint);
    padding-bottom: 2px;
  }
  .note {
    margin-left: 6px;
    font-weight: normal;
    font-style: italic;
    color: var(--ink-3);
    font-size: 11px;
  }
  dl {
    display: grid;
    grid-template-columns: minmax(96px, auto) 1fr;
    gap: 3px 10px;
    margin: 0;
    font-size: 12px;
    align-items: baseline;
  }
  dt {
    white-space: nowrap;
  }
  dd {
    margin: 0;
  }
  .dim {
    opacity: 0.5;
  }
  .or {
    margin: 0 3px;
    color: var(--ink-3);
  }
  /* The key badges show here even on touch screens (for a keyboard attached to a tablet). */
  .help :global(.ws-kbd) {
    display: inline-block;
  }
</style>
