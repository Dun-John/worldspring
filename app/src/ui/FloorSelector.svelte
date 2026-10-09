<script lang="ts">
  // Inside a building (or underground): the floor picker, above the map controls. Levels top
  // first with short labels, the one in view named, and the way out. In a site underground a
  // small ✎ opens the designer.
  import type { InteriorState } from '../render/MapView';
  import Icon from './Icon.svelte';

  interface Props {
    state: InteriorState;
    onLevel: (i: number) => void;
    onExit: () => void;
    /** Open the designer on this site (underground sites only). */
    onDesign?: () => void;
    /** The site was designed by hand. */
    designed?: boolean;
  }
  let { state, onLevel, onExit, onDesign, designed = false }: Props = $props();

  const order = $derived(state.levels.map((l, i) => ({ ...l, i })).reverse());
  // Underground sites (`u:` ids) number their levels by depth.
  const under = $derived(/^[uwk]:/.test(state.id));
  const label = (z: number) => (under ? (z === 0 ? '0' : `${z}`) : z < 0 ? `B${-z > 1 ? -z : ''}` : z === 0 ? 'G' : `${z}`);
  const current = $derived(state.levels[state.level]);
</script>

<aside class="floors" aria-label={under ? 'Levels underground' : 'Floors'}>
  <div class="head">
    <div class="name" title={state.name}>{state.name}</div>
    <div class="ws-muted level" title={state.id.startsWith('w:') ? 'The sewers run on wherever you look; tap a ladder, grate or shaft for the ways on' : undefined}>{current?.name ?? ''}</div>
  </div>
  <div class="levels" role="radiogroup">
    {#each order as l (l.i)}
      <button role="radio" aria-checked={l.i === state.level} class:on={l.i === state.level} onclick={() => onLevel(l.i)} title="{l.name} ({l.i > state.level ? ']' : '['})">{label(l.z)}</button>
    {/each}
  </div>
  <div class="actions">
    {#if onDesign}<button class="ws-icon-btn" onclick={onDesign} title={designed ? 'Change the design' : state.id.startsWith('b:') ? 'Design this building' : 'Design this site'} aria-label="Design this site"><Icon name="pencil" size={16} /></button>{/if}
    <button class="ws-btn leave" onclick={onExit} title="{under ? 'Back to the surface' : 'Leave the building'} (Esc)"><Icon name="leave" size={16} /><span class="word">Leave</span></button>
  </div>
</aside>

<style>
  .floors {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
    width: 140px;
    padding: 6px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    color: var(--ink);
    font: 12px/1.3 var(--font);
  }
  .name {
    font-weight: bold;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .level {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .levels {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 40vh;
    overflow-y: auto;
  }
  .levels button {
    min-height: var(--tap);
    font: bold 13px var(--font);
    color: var(--ink);
    background: #ece2c6;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .levels button:hover {
    background: var(--btn-hover);
  }
  .levels button.on {
    background: var(--accent);
    color: var(--accent-ink);
    border-color: var(--line);
  }
  .actions {
    display: flex;
    gap: 4px;
  }
  .leave {
    flex: 1;
    padding: 0 6px;
  }
  :global([data-layout='phone']) .floors {
    width: 58px;
    padding: 4px;
  }
  :global([data-layout='phone']) .actions {
    flex-direction: column;
  }
  :global([data-layout='phone']) .actions .ws-icon-btn {
    width: 100%;
  }
  :global([data-layout='phone']) .head,
  :global([data-layout='phone']) .word {
    display: none;
  }
</style>
