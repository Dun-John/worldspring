<script lang="ts">
  // Edit › Ground: the battlemap's ground raised, lowered, smoothed or put back by a brush, and
  // plateaus or pits drawn as loops (a castle on a cliff). A plateau chosen can be given another
  // level or edge, or taken away. Each stroke or change is one step to undo.
  import type { HillProfile, Plateau } from '../gen/protocol';
  import type { GroundMode, GroundSettings } from '../editor/ground';
  import { changedHill, maxPeak, maxRadius, profilePeak } from '../editor/hill';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import { foldOpen, setFold, shell } from './shell/layout.svelte';

  interface Props {
    settings: GroundSettings;
    /** The plateau chosen, if any. */
    selected: { id: string; plateau: Plateau } | null;
    /** The sea's level (ft): levels are shown above it. */
    sea: number;
    /** The battlemap is showing. */
    near: boolean;
    /** In a building or down a site. */
    inside: boolean;
    peek?: boolean;
    onPlateau: (id: string, p: Plateau) => void;
    onRemove: () => void;
    onZoomIn: () => void;
  }

  let { settings = $bindable(), selected, sea, near, inside, peek = false, onPlateau, onRemove, onZoomIn }: Props = $props();

  const MODES: { key: GroundMode; label: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'raise', label: 'Raise', icon: 'raise', kbd: 'Q', hint: 'Brush over the ground to lift it, at most by the height below however often you go over it.' },
    { key: 'lower', label: 'Lower', icon: 'lower', kbd: 'W', hint: 'Brush over the ground to sink it. Dry ground never sinks under the water beside it.' },
    { key: 'smooth', label: 'Smooth', icon: 'relax', kbd: 'E', hint: 'Brush over bumps and steps to ease them toward the ground round them.' },
    { key: 'restore', label: 'Put back', icon: 'reset', kbd: 'R', hint: 'Brush to take brushed changes back toward the ground as generated (plateaus stay).' },
    { key: 'plateau', label: 'Plateau', icon: 'plateau', kbd: 'L', hint: 'Drag a loop round the ground: inside it becomes flat at the level shown. Start on a plateau to extend it.' },
    { key: 'hill', label: 'Hill', icon: 'mountain', kbd: 'H', hint: 'Drag out from a hill’s middle to its foot (a ridge: along its crest). It rises from the ground with natural slopes to the height below (a big one, miles across, may rise to 2,000 ft). Works on the map’s finest four levels too.' },
    { key: 'select', label: 'Choose', icon: 'pointer', kbd: 'V', hint: 'Click a plateau, pit or hill to change it, or take it away.' },
  ];
  const PROFILES: { key: HillProfile; label: string; title: string }[] = [
    { key: 'round', label: 'Round', title: 'A round hill: smooth slopes all round' },
    { key: 'ridge', label: 'Ridge', title: 'A ridge: a long crest along the drag' },
    { key: 'knoll', label: 'Knoll', title: 'A rocky knoll: steep, with outcrops (cliff faces and scree)' },
    { key: 'mountain', label: 'Mountain', title: 'A small mountain: concave slopes to a peak, bands of crag' },
  ];
  const hint = $derived(MODES.find((m) => m.key === settings.mode)?.hint ?? '');
  const brush = $derived(settings.mode !== 'plateau' && settings.mode !== 'hill' && settings.mode !== 'select');
  const hill = $derived(selected?.plateau.kind === 'hill' ? selected.plateau : null);
  function changeHill(change: { profile?: HillProfile; peak?: number }) {
    if (!selected || !hill) return;
    const p = changedHill(hill, change);
    if (typeof p !== 'string') onPlateau(selected.id, p);
  }
  const ft = (v: number) => `${Math.round(v).toLocaleString('en')} ft`;
  const signed = (v: number) => `${v >= 0 ? '+' : '−'}${Math.abs(Math.round(v))} ft`;
  const edgeLabel = (e: number) => (e ? `${e} ft slope` : 'Sheer');
  /** Its outline's width and depth (ft). */
  const size = (p: Plateau) => {
    const xs = p.pts.map((q) => q[0]);
    const ys = p.pts.map((q) => q[1]);
    return `${Math.round(Math.max(...xs) - Math.min(...xs))} × ${Math.round(Math.max(...ys) - Math.min(...ys))} ft`;
  };

  /** The chosen plateau with its level `d` ft higher (or at `level` ft above the sea). */
  function level(level: number) {
    if (!selected || !Number.isFinite(level)) return;
    onPlateau(selected.id, { ...selected.plateau, level: Math.round((level + sea) * 10) / 10 });
  }
  function edge(e: number) {
    if (!selected) return;
    const { edge: _, ...p } = selected.plateau;
    onPlateau(selected.id, e ? { ...p, edge: e } : p);
  }
  function floor(f: string) {
    if (!selected) return;
    const { floor: _, ...p } = selected.plateau;
    onPlateau(selected.id, f === 'rock' || f === 'ground' ? { ...p, floor: f } : p);
  }
</script>

<div class="ground">
  <div class="ws-seg modes" role="radiogroup" aria-label="Tool">
    {#each MODES as m (m.key)}
      <button class:on={settings.mode === m.key} aria-pressed={settings.mode === m.key} onclick={() => (settings.mode = m.key)} title="{m.label}{m.key === 'restore' ? ' (was Restore)' : ''} ({m.kbd})"><Icon name={m.icon} size={16} />{m.label}</button>
    {/each}
  </div>
  {#if inside}
    <div class="zoom"><span class="ws-grow">Go back out to shape the ground.</span></div>
  {:else if !near}
    <div class="zoom">
      <span class="ws-grow">Zoom in to {shell.reachWhere()} to {settings.mode === 'hill' ? 'raise a hill' : settings.mode === 'select' ? 'choose a hill' : 'shape the ground'}.</span>
      <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
    </div>
  {/if}
  {#if !peek}
    <div class="ws-hint">{hint}</div>
    {#if brush}
      <details class="ws-group" open={foldOpen('ground')} ontoggle={(e) => setFold('ground', e.currentTarget.open)}>
        <summary>More options</summary>
        <div class="ws-group-body">
          <label class="ws-slider">
            <span>Radius</span><output>{settings.radius} ft</output>
            <input type="range" min="5" max="60" step="2.5" bind:value={settings.radius} />
          </label>
          {#if settings.mode === 'raise' || settings.mode === 'lower'}
            <label class="ws-slider">
              <span>Height</span><output>{settings.height} ft{settings.height === 5 ? ' · a tier' : ''}</output>
              <input type="range" min="1" max="20" step="1" bind:value={settings.height} />
            </label>
          {:else}
            <label class="ws-slider">
              <span>Strength</span><output>{Math.round(settings.strength * 100)}%</output>
              <input type="range" min="0.1" max="1" step="0.05" bind:value={settings.strength} />
            </label>
          {/if}
          <label class="ws-slider">
            <span>Hardness</span><output>{settings.hardness < 0.25 ? 'soft' : settings.hardness > 0.75 ? 'hard' : 'medium'}</output>
            <input type="range" min="0" max="1" step="0.05" bind:value={settings.hardness} />
          </label>
        </div>
      </details>
    {:else if settings.mode === 'plateau'}
      <details class="ws-group" open={foldOpen('ground')} ontoggle={(e) => setFold('ground', e.currentTarget.open)}>
        <summary>More options</summary>
        <div class="ws-group-body">
          <label class="ws-slider">
            <span>{settings.offset < 0 ? 'Dug below the ground' : 'Above the ground'}</span><output>{signed(settings.offset)}</output>
            <input type="range" min="-40" max="40" step="1" bind:value={settings.offset} />
          </label>
          <label class="ws-slider">
            <span>Edge</span><output>{edgeLabel(settings.edge)}</output>
            <input type="range" min="0" max="60" step="5" bind:value={settings.edge} />
          </label>
        </div>
      </details>
    {:else if settings.mode === 'hill'}
      <div class="ws-seg" role="radiogroup" aria-label="Profile">
        {#each PROFILES as h (h.key)}
          <button class:on={settings.hillProfile === h.key} aria-pressed={settings.hillProfile === h.key} onclick={() => (settings.hillProfile = h.key)} title={h.title}>{h.label}</button>
        {/each}
      </div>
      <label class="ws-slider">
        <span>Height</span><output>{settings.hillPeak.toLocaleString('en')} ft{settings.hillPeak > 300 ? ' (if big enough)' : ''}</output>
        <input type="range" min="10" max={profilePeak(settings.hillProfile)} step="10" bind:value={settings.hillPeak} />
      </label>
      <details class="ws-group" open={foldOpen('ground')} ontoggle={(e) => setFold('ground', e.currentTarget.open)}>
        <summary>More options</summary>
        <div class="ws-group-body">
          {#if settings.hillProfile === 'ridge'}
            <label class="ws-slider">
              <span>Half-width</span><output>{settings.ridgeWidth} ft</output>
              <input type="range" min="20" max={maxRadius('ridge')[1]} step="20" bind:value={settings.ridgeWidth} />
            </label>
          {:else}
            <label class="ws-slider">
              <span>Width across the drag</span><output>{Math.round(settings.hillAspect * 100)}%</output>
              <input type="range" min="0.3" max="1" step="0.05" bind:value={settings.hillAspect} />
            </label>
          {/if}
        </div>
      </details>
    {/if}
    {#if selected && hill}
      <section class="card" aria-label="Hill">
        <div class="title">
          <Icon name="mountain" size={16} />
          <span class="ws-grow"><b>Hill</b><span class="ws-muted">&nbsp;· {size(hill)}</span></span>
        </div>
        <label class="ws-field">
          Profile
          <select class="ws-input" value={hill.hill?.profile ?? 'round'} onchange={(e) => changeHill({ profile: e.currentTarget.value as HillProfile })}>
            {#each PROFILES as h (h.key)}<option value={h.key}>{h.title}</option>{/each}
          </select>
        </label>
        <label class="ws-slider">
          <span>Height</span><output>{Math.round(hill.level)} ft</output>
          <input type="range" min="10" max={hill.hill ? maxPeak(hill.hill.profile, hill.hill.rx, hill.hill.ry) : 300} step="10" value={hill.level} onchange={(e) => changeHill({ peak: Number(e.currentTarget.value) })} />
        </label>
        <div class="row">
          <button class="ws-btn" onclick={onRemove} title="Take it away (Delete)"><Icon name="trash" size={16} /> Take away</button>
        </div>
      </section>
    {:else if selected}
      <section class="card" aria-label="Plateau">
        <div class="title">
          <Icon name="plateau" size={16} />
          <span class="ws-grow"><b>Plateau</b><span class="ws-muted">&nbsp;· {size(selected.plateau)}</span></span>
        </div>
        <div class="ws-field">
          Level above the sea
          <div class="ws-row">
            <button class="ws-icon-btn" onclick={() => level(selected.plateau.level - sea - 5)} title="5 ft lower" aria-label="5 ft lower"><Icon name="minus" size={16} /></button>
            <input class="ws-input lvl" type="number" step="1" value={Math.round(selected.plateau.level - sea)} onchange={(e) => level(Number(e.currentTarget.value))} aria-label="Level (ft above the sea)" />
            <span>ft</span>
            <button class="ws-icon-btn" onclick={() => level(selected.plateau.level - sea + 5)} title="5 ft higher" aria-label="5 ft higher"><Icon name="plus" size={16} /></button>
          </div>
        </div>
        <label class="ws-slider">
          <span>Edge</span><output>{edgeLabel(selected.plateau.edge ?? 0)}</output>
          <input type="range" min="0" max="60" step="5" value={selected.plateau.edge ?? 0} onchange={(e) => edge(Number(e.currentTarget.value))} />
        </label>
        {#if selected.plateau.kind !== 'islet'}
          <label class="ws-field">
            Top
            <select class="ws-input" value={selected.plateau.floor ?? 'auto'} onchange={(e) => floor(e.currentTarget.value)} title="What its top (a pit's floor) draws as">
              <option value="auto">Rock on a deep pit's floor</option>
              <option value="rock">Bare rock</option>
              <option value="ground">The land round it</option>
            </select>
          </label>
        {/if}
        <div class="row">
          <button class="ws-btn" onclick={onRemove} title="Take it away (Delete)"><Icon name="trash" size={16} /> Take away</button>
        </div>
      </section>
    {/if}
    {#if brush && !shell.phone}<div class="ws-hint">{ft(settings.radius * 2)} brush · <kbd class="ws-kbd">[</kbd> <kbd class="ws-kbd">]</kbd> narrower, wider · <kbd class="ws-kbd">Esc</kbd> drops a stroke</div>{/if}
  {/if}
</div>

<style>
  .ground {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .lvl {
    width: 6em;
  }
  /* Seven tools side by side: each its icon over its word. */
  .modes > button {
    flex-direction: column;
    gap: 0;
    padding: 4px 1px;
    font-size: 11px;
  }
</style>
