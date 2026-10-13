<script lang="ts">
  // Asked before work would be dropped: new sketch strokes (generate them, drop them, or keep
  // sketching), or a site's unsaved design (save it, drop it, or keep designing). And before a
  // world is remade round the changes made on it (some may be set aside), or left for another
  // world with new settings (they stay with this one: a copy can go to the Library first).
  import type { Ask } from './layout.svelte';

  interface Props {
    ask: Ask;
    /** Save is refused: the design breaks a rule a site must keep. */
    blocked?: boolean;
    /** How long the last world took to generate (s). */
    seconds?: number | null;
    onKeep: () => void;
    onDrop: () => void;
    onCommit: () => void;
    onSave: () => void;
  }
  let { ask, blocked = false, seconds = null, onKeep, onDrop, onCommit, onSave }: Props = $props();
  const remake = $derived(ask.kind === 'regen' || ask.kind === 'newworld' ? ask.kind : ask.kind === 'sketch' && ask.generate !== false ? (ask.remake ?? null) : null);
  const about = $derived(seconds ? `Takes about ${seconds} s` : undefined);
</script>

<div class="ask" role="alertdialog" aria-label={remake && ask.kind !== 'sketch' ? 'Generate the world' : 'Unsaved changes'}>
  <div class="text">
    {#if ask.kind === 'sketch'}<b>Your sketch isn't generated yet.</b> Its new strokes would be lost.
    {:else if ask.kind === 'design'}<b>This site's design isn't saved.</b> The changes would be lost.
    {:else if ask.kind === 'regen'}<b>Remake this world?</b>{/if}
    {#if remake === 'regen'}
      {ask.kind === 'sketch' ? 'Generating it remakes this world: towns' : 'Towns'} and roads may move. Places you added, drawn buildings, objects, ground, names, notes, NPCs
      and plots stay; changes to towns and sites that come out differently are set aside. Undo starts afresh.
    {:else if remake === 'newworld'}
      {#if ask.kind === 'newworld'}<b>Make a different world?</b>{/if}
      {ask.kind === 'sketch' ? 'With the new settings it makes' : 'New settings make'} another world, without this one's changes. They stay with this world: keep a
      copy in the Library to open it again as it is.
    {/if}
  </div>
  <div class="ws-row buttons">
    {#if ask.kind === 'regen' || ask.kind === 'newworld'}
      <button class="ws-btn quiet" onclick={onKeep}>Cancel</button>
    {:else}
      <button class="ws-btn quiet" onclick={onKeep}>{ask.kind === 'sketch' ? 'Keep sketching' : 'Keep designing'}</button>
      <button class="ws-btn" onclick={onDrop}>Discard</button>
    {/if}
    {#if ask.kind === 'design'}
      <button class="ws-btn primary" onclick={onCommit} disabled={blocked} title={blocked ? 'Fix the problems marked ✕ first' : ''}>Save</button>
    {:else if ask.kind !== 'sketch' || ask.generate !== false}
      {#if remake === 'newworld'}<button class="ws-btn" onclick={onSave} title="Keep this world and its changes in the Library, then generate">Save to Library, then generate</button>{/if}
      <button class="ws-btn primary" onclick={onCommit} title={about}>Generate</button>
    {/if}
  </div>
</div>

<style>
  .ask {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: #fff4e0;
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    color: var(--ink);
    font: 13px/1.4 var(--font);
    box-shadow: var(--shadow);
  }
  .buttons {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
</style>
