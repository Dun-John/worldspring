<script lang="ts">
  // Edit › Streets: a town plan to trace from (a picture from disk), laid over the map round the
  // town in view and moved, scaled and turned by its handles (Adjust) or by numbers. For this
  // session only: never saved, never shown to the players.
  import { townUnderlay as u } from '../editor/trace/underlay.svelte';
  import Icon from './Icon.svelte';

  async function pickImage(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const f = input.files?.[0];
    input.value = '';
    if (f && !(await u.load(f))) u.remove();
  }
  const across = $derived(u.image ? Math.round(u.image.w * u.xf.ftPerPx) : 0);
  function setAcross(e: Event) {
    const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
    if (u.image && v > 0) u.xf.ftPerPx = v / u.image.w;
  }
  // (Moved by numbers: east and south of where Fit puts it.)
  function nudge(dx: number, dy: number) {
    u.xf.cx += dx;
    u.xf.cy += dy;
  }
</script>

<details class="ws-group" open={!!u.image}>
  <summary>Trace a plan</summary>
  <div class="ws-group-body">
    <div class="ws-row">
      <label class="ws-btn grow filebtn"><Icon name="image" size={16} /> {u.image ? 'Replace…' : 'Choose a picture…'}<input type="file" accept="image/*" hidden onchange={pickImage} /></label>
      {#if u.image}<button class="ws-btn" onclick={() => u.remove()}>Remove</button>{/if}
    </div>
    {#if !u.image}
      <div class="ws-muted">Or drop a picture on the map. It lies over the town to lay its streets on, for this session only: it is never saved or shown to the players.</div>
    {:else}
      <label class="slider"><span>Opacity</span><output>{Math.round(u.opacity * 100)}%</output><input type="range" min="0.05" max="1" step="0.05" bind:value={u.opacity} /></label>
      <div class="ws-row">
        <button class="ws-btn" onclick={() => u.fit()}>Fit to town</button>
        <button class="ws-btn" aria-pressed={u.adjust} class:primary={u.adjust} onclick={() => (u.adjust = !u.adjust)} title="Drag the picture to move it; a corner to scale; the round handle to turn (Esc leaves)">Adjust</button>
        <button class="ws-icon-btn" aria-pressed={u.shown} onclick={() => (u.shown = !u.shown)} title={u.shown ? 'Hide the picture' : 'Show the picture'} aria-label="Show the picture"><Icon name={u.shown ? 'eye' : 'eye-off'} /></button>
      </div>
      <div class="nums">
        <label class="ws-field">Across (ft)<input class="ws-input" type="number" step="50" min="10" value={across} onchange={setAcross} /></label>
        <label class="ws-field">Turn (°)<input class="ws-input" type="number" step="1" value={Math.round(u.xf.rot * 10) / 10} onchange={(e) => (u.xf.rot = e.currentTarget.valueAsNumber || 0)} /></label>
      </div>
      <div class="ws-row nudge" role="group" aria-label="Move the picture">
        <button class="ws-icon-btn" onclick={() => nudge(-0.02 * across, 0)} title="West" aria-label="West"><Icon name="chevron-left" size={16} /></button>
        <button class="ws-icon-btn" onclick={() => nudge(0, -0.02 * across)} title="North" aria-label="North"><Icon name="chevron-up" size={16} /></button>
        <button class="ws-icon-btn" onclick={() => nudge(0, 0.02 * across)} title="South" aria-label="South"><Icon name="chevron-down" size={16} /></button>
        <button class="ws-icon-btn" onclick={() => nudge(0.02 * across, 0)} title="East" aria-label="East"><Icon name="chevron-right" size={16} /></button>
      </div>
    {/if}
  </div>
</details>

<style>
  .grow {
    flex: 1;
    min-width: 0;
  }
  .filebtn {
    cursor: pointer;
  }
  .slider {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    font-size: 12px;
  }
  .slider output {
    font: 11px var(--mono);
    color: var(--ink-2);
  }
  .slider input {
    grid-column: 1 / 3;
    width: 100%;
    margin: 0;
  }
  .nums {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
</style>
