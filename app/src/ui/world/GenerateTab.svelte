<script lang="ts">
  // World › Generate: the seed, then the settings in groups (the first open, the rest folded
  // as this viewer left them), and Generate pinned at the bottom. Edits stay in the draft
  // until generated; the sketch preview follows them.
  import type { WorldFile, WorldParams } from '../../gen/protocol';
  import { DEFAULT_PARAMS, TUNABLE_BIOMES, randomSeed } from '../../world/world';
  import Icon from '../Icon.svelte';
  import { remember, remembered } from '../shell/layout.svelte';
  import type { WorldDraft } from './draft.svelte';

  interface Props {
    draft: WorldDraft;
    world: WorldFile;
    busy: boolean;
    status: string;
    progress: number | null;
    sketching: boolean;
    /** Strokes in the sketch (being drawn, or the world's). */
    strokes: number;
    /** The sketch draws land or sea: it decides the coastline, not the land slider. */
    sketchLand: boolean;
    paintedBiomes: number;
    /** Settlements placed in the sketch (they stand whatever the Settlements dial says). */
    pins: number;
    /** Generate the draft (with the sketch, stretched to its size). */
    onGenerate: () => void;
  }
  let { draft, world, busy, status, progress, sketching, strokes, sketchLand, paintedBiomes, pins, onGenerate }: Props = $props();

  type Num = { [K in keyof WorldParams]: WorldParams[K] extends number ? K : never }[keyof WorldParams];
  /** [key, label, min, max, step, format] */
  type Slider = [Num, string, number, number, number, (v: number) => string];
  const times = (v: number) => (Math.abs(v - 1) < 1e-6 ? 'Normal' : `×${v.toFixed(v * 10 === Math.round(v * 10) ? 1 : 2)}`);
  const GROUPS: { key: string; label: string; open: boolean; sliders: Slider[]; wind?: boolean }[] = [
    {
      key: 'size',
      label: 'Size & land',
      open: true,
      sliders: [
        ['width_mi', 'Map width', 300, 3000, 50, (v) => `${v.toLocaleString()} mi`],
        ['height_mi', 'Map height', 200, 3000, 50, (v) => `${v.toLocaleString()} mi`],
        ['land_fraction', 'Land', 0.1, 0.8, 0.01, (v) => `${Math.round(v * 100)}%`],
      ],
    },
    {
      key: 'terrain',
      label: 'Terrain',
      open: false,
      sliders: [
        ['ruggedness', 'Ruggedness', 0.1, 2.5, 0.05, times],
        ['procedural_mountains', 'Generated mountains', 0, 1, 0.05, (v) => (v >= 0.999 ? 'All' : v <= 0.001 ? 'None' : `${Math.round(v * 100)}%`)],
        ['max_elev_ft', 'Highest peaks', 4000, 25000, 500, (v) => `${v.toLocaleString()} ft`],
        ['erosion', 'Erosion (age)', 0, 2, 0.05, times],
        ['plate_count', 'Tectonic plates', 4, 40, 1, (v) => `${v}`],
        ['volcanoes', 'Volcanoes', 0, 12, 1, (v) => (v ? `${v}` : 'None')],
      ],
    },
    {
      key: 'climate',
      label: 'Climate',
      open: false,
      wind: true,
      sliders: [
        ['temp_offset_c', 'Temperature', -15, 15, 0.5, (v) => (v ? `${v > 0 ? '+' : ''}${v} °C` : 'Normal')],
        ['moisture', 'Moisture', 0.2, 3, 0.05, times],
        ['river_density', 'Rivers', 0.25, 4, 0.05, times],
        ['lat_top', 'Latitude, north edge', -80, 80, 1, (v) => `${v}°`],
        ['lat_bottom', 'Latitude, south edge', -80, 80, 1, (v) => `${v}°`],
      ],
    },
    {
      key: 'people',
      label: 'People & places',
      open: false,
      sliders: [
        ['settlement_density', 'Settlements', 0, 3, 0.05, (v) => (v ? times(v) : 'None')],
        ['poi_density', 'Ruins & sites', 0, 3, 0.05, (v) => (v ? times(v) : 'None')],
      ],
    },
  ];
  const WINDS: [WorldParams['wind'], string][] = [
    ['belts', 'Realistic'],
    ['from_west', 'From the west'],
    ['from_east', 'From the east'],
  ];

  const p = $derived(draft.p);
  const moot = (key: Num) => key === 'land_fraction' && sketchLand;
  const isChanged = (key: Num) => p[key] !== DEFAULT_PARAMS[key];
  const changedIn = (g: (typeof GROUPS)[number]) => g.sliders.filter(([k]) => isChanged(k) && !moot(k)).length + (g.wind && p.wind !== DEFAULT_PARAMS.wind ? 1 : 0);
  function reset(g: (typeof GROUPS)[number]) {
    const next = { ...draft.p };
    for (const [k] of g.sliders) (next as Record<string, unknown>)[k] = DEFAULT_PARAMS[k];
    if (g.wind) next.wind = DEFAULT_PARAMS.wind;
    draft.p = next;
  }

  const weight = (name: string) => p.biome_weights[name] ?? 1;
  const weightWord = (v: number) => (v <= 0.001 ? 'None' : v < 0.75 ? 'Rare' : v <= 1.25 ? 'Normal' : v < 1.75 ? 'Common' : 'Double');
  const biomesChanged = $derived(TUNABLE_BIOMES.filter(([n]) => Math.abs(weight(n) - 1) > 1e-6).length);
  const setWeight = (name: string, v: number) => (draft.p.biome_weights = { ...draft.p.biome_weights, [name]: v });

  const pending = $derived(draft.changed(world));
  const resized = $derived(strokes > 0 && (p.width_mi !== (world.params.width_mi ?? DEFAULT_PARAMS.width_mi) || p.height_mi !== (world.params.height_mi ?? DEFAULT_PARAMS.height_mi)));

  function random() {
    draft.seed = randomSeed();
    onGenerate();
  }
</script>

<div class="gen">
  {#if sketching}<div class="banner"><Icon name="sketch" size={16} /> Sketching: Generate uses your sketch.</div>{/if}
  <div class="ws-row seed">
    <label class="ws-field grow">
      Seed
      <input class="ws-input" type="number" min="0" bind:value={draft.seed} onkeydown={(e) => e.key === 'Enter' && onGenerate()} />
    </label>
    <button class="ws-btn" onclick={random} disabled={busy} title="A new world from a random seed"><Icon name="dice" size={16} /> Random</button>
  </div>
  {#if progress !== null}
    <div class="bar" role="progressbar" aria-valuenow={Math.round(progress * 100)}><div style:width="{Math.round(progress * 100)}%"></div></div>
  {/if}
  <div class="ws-muted">{pending && !busy ? 'Settings changed: not generated yet.' : status}</div>
  {#if resized}<div class="ws-hint">The sketch stretches with the map.</div>{/if}

  {#each GROUPS as g (g.key)}
    {@const n = changedIn(g)}
    <details class="ws-group" open={remembered(`gen.${g.key}`, g.open)} ontoggle={(e) => remember(`gen.${g.key}`, e.currentTarget.open)}>
      <summary>
        <span class="grow">{g.label}</span>
        {#if n}<span class="count">{n} changed</span>
          <button class="ws-icon-btn small" title="Back to the defaults" aria-label="Reset {g.label}" onclick={(e) => (e.preventDefault(), reset(g))}><Icon name="reset" size={14} /></button>{/if}
      </summary>
      <div class="ws-group-body">
        {#each g.sliders as [key, label, min, max, step, fmt] (key)}
          <label class="slider" class:moot={moot(key)} title={moot(key) ? 'Your sketch draws the coastline' : undefined}>
            <span class="name">{#if isChanged(key) && !moot(key)}<i class="dot" title="Changed"></i>{/if}{label}</span>
            <output>{moot(key) ? 'Set by your sketch' : fmt(p[key] as number)}</output>
            <input type="range" {min} {max} {step} bind:value={draft.p[key]} disabled={moot(key)} />
          </label>
          {#if key === 'settlement_density' && p.settlement_density <= 0 && pins}<div class="ws-hint">Your {pins} sketched {pins === 1 ? 'settlement still appears' : 'settlements still appear'}.</div>{/if}
          {#if key === 'poi_density' && p.poi_density <= 0}<div class="ws-hint">Sites you place in Edit › Sites still appear.</div>{/if}
        {/each}
        {#if g.wind}
          <div class="ws-field">
            Winds
            <div class="ws-seg">
              {#each WINDS as [w, label] (w)}<button class:on={p.wind === w} onclick={() => (draft.p.wind = w)}>{label}</button>{/each}
            </div>
          </div>
        {/if}
      </div>
    </details>
  {/each}
  <details class="ws-group" open={remembered('gen.biomes', false)} ontoggle={(e) => remember('gen.biomes', e.currentTarget.open)}>
    <summary>
      <span class="grow">Biome mix</span>
      {#if biomesChanged}<span class="count">{biomesChanged} changed</span>
        <button class="ws-icon-btn small" title="Back to the defaults" aria-label="Reset the biome mix" onclick={(e) => (e.preventDefault(), (draft.p.biome_weights = {}))}><Icon name="reset" size={14} /></button>{/if}
    </summary>
    <div class="ws-group-body">
      <div class="ws-hint">How common each biome is.{paintedBiomes ? ' Areas painted in the sketch keep theirs.' : ''}</div>
      {#each TUNABLE_BIOMES as [name, label] (name)}
        <label class="slider">
          <span class="name">{#if Math.abs(weight(name) - 1) > 1e-6}<i class="dot" title="Changed"></i>{/if}{label}</span>
          <output>{weightWord(weight(name))}</output>
          <input type="range" min="0" max="2" step="0.05" value={weight(name)} oninput={(e) => setWeight(name, Number(e.currentTarget.value))} />
        </label>
      {/each}
    </div>
  </details>

  <div class="foot">
    <button class="ws-btn quiet" onclick={() => (draft.p = { ...DEFAULT_PARAMS, biome_weights: {} })} title="Every setting back to its default">Defaults</button>
    <button class="ws-btn primary grow" onclick={onGenerate} disabled={busy}>{sketching ? 'Generate from sketch' : 'Generate world'}</button>
  </div>
</div>

<style>
  .gen {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .seed {
    align-items: flex-end;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
    color: var(--ink-2);
    font-size: 12px;
  }
  .bar {
    height: 5px;
    background: #d9cfb5;
    border: 1px solid #8b7b5e;
    border-radius: 3px;
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    background: var(--accent);
    transition: width 0.15s;
  }
  .count {
    font-weight: normal;
    font-size: 11px;
    color: var(--gold);
  }
  .small {
    width: 24px;
    height: 24px;
  }
  .slider {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0 8px;
    align-items: center;
    font-size: 12px;
  }
  .slider .name {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .slider output {
    font: 11px var(--mono);
    color: var(--ink-2);
    white-space: nowrap;
  }
  .slider input {
    grid-column: 1 / 3;
    width: 100%;
    margin: 0;
    min-width: 0;
  }
  .slider.moot {
    opacity: 0.6;
  }
  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--gold);
  }
  .foot {
    position: sticky;
    bottom: -10px;
    display: flex;
    gap: 6px;
    margin: 4px -10px -10px;
    padding: 8px 10px 10px;
    background: var(--paper-solid);
    border-top: 1px solid var(--line-faint);
  }
</style>
