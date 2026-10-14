<script lang="ts" module>
  import type { Created, PinKind } from '../gen/protocol';
  import type { PinTier } from '../editor/sketcher';

  /** A settlement to pin (the world is remade round it): its tier and the pin's options. */
  export interface SettlementChoice {
    tier: PinTier;
    livesBy: PinKind | '';
    capital: boolean;
    /** District names, comma-separated, central first. */
    wards: string;
  }

  /** What to put on the map: a created site's kind and options (no id, place or name yet), or a
   * settlement (kind `settlement`). */
  export type SiteChoice = Pick<Created, 'kind' | 'under' | 'size' | 'levels' | 'theme'> & { name?: string; settlement?: SettlementChoice };
</script>

<script lang="ts">
  // Edit › Places: what to put on the map (a settlement, a way underground with its size, depth
  // and theme, or a site on the surface), then Place on map waits for a click there. A
  // settlement is a pin in the sketch: the world is remade round it, so its tiles say so, and
  // they wait while a session is played.
  import { SITE_SIZES, THEMES, type SiteSize } from '../gen/protocol';
  import { PIN_KINDS } from '../editor/sketcher';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  interface Props {
    /** Waiting for the click on the map. */
    armed: boolean;
    /** Only the chosen site and Place on map (the panel is folded down). */
    peek?: boolean;
    onPlace: (c: SiteChoice, label: string) => void;
    onCancel: () => void;
    /** Open the sketch, where many settlements can be drawn at once. */
    onTowns?: () => void;
    /** A session is being played: no world is remade meanwhile. */
    playing?: boolean;
    /** How long the world took to make last time (s). */
    seconds?: number | null;
    /** What was last added here (a settlement, or a pin that placed none), which Take back takes
     * away again. */
    added?: string | null;
    onTakeBack?: () => void;
  }

  let { armed, peek = false, onPlace, onCancel, onTowns, playing = false, seconds = null, added = null, onTakeBack }: Props = $props();

  type Group = 'Settlements' | 'Underground' | 'On the surface';

  /** The choices: what each makes (`Created` kind, and what lies beneath). */
  const CHOICES: { key: string; label: string; icon: IconName; kind: string; under?: string; tier?: PinTier; group: Group; look: string }[] = [
    { key: 'village', label: 'Village', icon: 'home', kind: 'settlement', tier: 'village', group: 'Settlements', look: 'A few dozen to a few hundred souls' },
    { key: 'town', label: 'Town', icon: 'building', kind: 'settlement', tier: 'town', group: 'Settlements', look: 'Hundreds to a few thousand, in districts' },
    { key: 'city', label: 'City', icon: 'castle', kind: 'settlement', tier: 'city', group: 'Settlements', look: 'Thousands, walled, with sewers below' },
    { key: 'metropolis', label: 'Metropolis', icon: 'wards', kind: 'settlement', tier: 'metropolis', group: 'Settlements', look: 'Tens of thousands: a great city' },
    { key: 'dungeon', label: 'Dungeon', icon: 'stairs', kind: 'entrance', under: 'dungeon', group: 'Underground', look: 'Stairs down, nothing above' },
    { key: 'crypt', label: 'Crypt', icon: 'skull', kind: 'entrance', under: 'crypt', group: 'Underground', look: 'Stairs down, nothing above' },
    { key: 'catacombs', label: 'Catacombs', icon: 'skull', kind: 'entrance', under: 'catacombs', group: 'Underground', look: 'Stairs down, nothing above' },
    { key: 'cave', label: 'Cave', icon: 'cave', kind: 'cave', group: 'Underground', look: 'A cave mouth' },
    { key: 'mine', label: 'Mine', icon: 'pick', kind: 'mine', group: 'Underground', look: 'An adit with a miners’ shed' },
    { key: 'lava_tube', label: 'Lava tube', icon: 'flame', kind: 'lava_tube', group: 'Underground', look: 'A skylight into the tube' },
    { key: 'ruin', label: 'Ruin', icon: 'ruin', kind: 'ruin', group: 'On the surface', look: 'Broken walls over a stair down' },
    { key: 'tower', label: 'Tower', icon: 'tower', kind: 'tower', group: 'On the surface', look: 'A wizard’s tower' },
    { key: 'camp', label: 'Camp', icon: 'tent', kind: 'camp', group: 'On the surface', look: 'Tents round a fire' },
    { key: 'waystation', label: 'Roadside inn', icon: 'mug', kind: 'waystation', group: 'On the surface', look: 'An inn by the road' },
  ];
  const RUIN_UNDER = ['dungeon', 'crypt', 'catacombs'];

  let pick = $state('dungeon');
  let ruinUnder = $state('dungeon');
  let size = $state<SiteSize | ''>('');
  let levels = $state(0);
  let theme = $state('');
  let name = $state('');
  let livesBy = $state<PinKind | ''>('');
  let capital = $state(false);
  let wards = $state('');

  const choice = $derived(CHOICES.find((c) => c.key === pick)!);
  /** The kind of site underground, if any. */
  const underKind = $derived(pick === 'ruin' ? ruinUnder : choice.kind === 'entrance' ? choice.under! : ['cave', 'mine', 'lava_tube'].includes(pick) ? pick : null);
  const themes = $derived(underKind ? THEMES[underKind] : []);
  const tier = $derived(choice.tier);
  const remakes = $derived(seconds ? `Remakes the world round it (about ${seconds} s)` : 'Remakes the world round it');

  const label = (key: string) => key.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());

  function choose(key: string) {
    pick = key;
    theme = '';
  }

  function place() {
    if (tier) {
      if (playing) return;
      const c: SiteChoice = { kind: 'settlement', settlement: { tier, livesBy, capital, wards: tier === 'village' ? '' : wards } };
      if (name.trim()) c.name = name.trim();
      onPlace(c, choice.label.toLowerCase());
      return;
    }
    const c: SiteChoice = { kind: choice.kind };
    if (pick === 'ruin') c.under = ruinUnder;
    else if (choice.under) c.under = choice.under;
    if (underKind) {
      if (size) c.size = size;
      if (levels) c.levels = levels;
      if (theme && themes.includes(theme)) c.theme = theme;
    }
    if (name.trim()) c.name = name.trim();
    onPlace(c, choice.label.toLowerCase());
  }
</script>

<div class="sites">
  {#if added && onTakeBack}
    <div class="towns added">
      <Icon name="check" size={16} />
      <span class="grow">{added}</span>
      <button class="ws-btn" onclick={onTakeBack} disabled={playing} title={playing ? 'Stop playing to remake the world' : 'Make the world again without it'}>Take back</button>
    </div>
  {/if}
  {#if !peek}
    {#each ['Settlements', 'Underground', 'On the surface'] as group (group)}
      <div class="ws-label">{group}</div>
      <div class="kinds">
        {#each CHOICES.filter((c) => c.group === group) as c (c.key)}
          <button
            class="tile"
            class:on={pick === c.key}
            aria-pressed={pick === c.key}
            title={c.tier ? (playing ? 'Stop playing to remake the world' : remakes) : undefined}
            onclick={() => choose(c.key)}
            ><Icon name={c.icon} size={20} /><span>{c.label}{#if c.tier}<span class="remade" aria-label="(remakes the world)">↻</span>{/if}</span></button
          >
        {/each}
      </div>
    {/each}
    <div class="ws-hint">{choice.look}</div>
    {#if tier}
      <label class="ws-field">
        Lives by
        <select class="ws-input" bind:value={livesBy}>
          {#each PIN_KINDS as [k, label] (k)}<option value={k}>{label}</option>{/each}
        </select>
      </label>
      <label class="switch"><span class="grow">The realm's capital</span><input type="checkbox" class="ws-switch" bind:checked={capital} /></label>
      {#if tier !== 'village'}
        <input class="ws-input" placeholder="District names, central first (optional)" bind:value={wards} aria-label="District names, separated by commas" />
      {/if}
    {/if}
    {#if pick === 'ruin'}
      <div class="ws-field">
        Beneath
        <div class="ws-seg">
          {#each RUIN_UNDER as u (u)}<button class:on={ruinUnder === u} onclick={() => ((ruinUnder = u), (theme = ''))}>{label(u)}</button>{/each}
        </div>
      </div>
    {/if}
    {#if underKind}
      <div class="ws-field">
        Size
        <div class="ws-seg">
          <button class:on={size === ''} onclick={() => (size = '')}>Any</button>
          {#each SITE_SIZES as s (s)}<button class:on={size === s} onclick={() => (size = s)}>{label(s)}</button>{/each}
        </div>
      </div>
      <div class="ws-row">
        <div class="ws-field grow">
          Levels
          <div class="stepper">
            <button class="ws-icon-btn" onclick={() => (levels = Math.max(0, levels - 1))} disabled={levels === 0} aria-label="Fewer levels"><Icon name="minus" size={16} /></button>
            <span>{levels || 'Any'}</span>
            <button class="ws-icon-btn" onclick={() => (levels = Math.min(6, levels + 1))} disabled={levels === 6} aria-label="More levels"><Icon name="plus" size={16} /></button>
          </div>
        </div>
        {#if themes.length > 1}
          <label class="ws-field grow">
            Theme
            <select class="ws-input" bind:value={theme}>
              <option value="">Any</option>
              {#each themes as t (t)}<option value={t}>{label(t)}</option>{/each}
            </select>
          </label>
        {/if}
      </div>
    {/if}
    <label class="ws-field">
      Name
      <input class="ws-input" bind:value={name} placeholder="Named for you if left empty" />
    </label>
    {#if tier}
      <div class="towns">
        <Icon name="reset" size={16} />
        <span class="grow">{playing ? 'Stop playing to remake the world.' : `${remakes}: towns and roads near it may move.`}{onTowns ? ' Several at once: World › Sketch.' : ''}</span>
        {#if onTowns}<button class="ws-btn" onclick={onTowns}>Sketch</button>{/if}
      </div>
    {/if}
  {/if}
  {#if armed}
    <div class="armed">
      <span class="grow">Click on the map to place the {choice.label.toLowerCase()}</span>
      <button class="ws-btn" onclick={onCancel}>Cancel</button>
    </div>
  {:else}
    <button class="ws-btn primary block" onclick={place} disabled={!!tier && playing} title={tier && playing ? 'Stop playing to remake the world' : undefined}
      ><Icon name="pin" size={16} /> Place {peek ? `a ${choice.label.toLowerCase()}` : 'on map'}</button
    >
  {/if}
</div>

<style>
  .sites {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
    gap: 4px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 2px 4px;
    font: 12px var(--font);
    color: var(--ink);
    background: #efe6cf;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .tile:hover {
    background: var(--btn-hover);
  }
  .tile.on {
    background: var(--accent);
    border-color: var(--line);
    color: var(--accent-ink);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 4px;
    border: 1px solid var(--line-field);
    border-radius: var(--radius-sm);
    background: var(--field);
  }
  .stepper span {
    flex: 1;
    text-align: center;
    color: var(--ink);
    font-size: 13px;
  }
  .towns {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--ink-2);
    font-size: 12px;
  }
  .remade {
    margin-left: 3px;
    color: var(--ink-2);
  }
  .tile.on .remade {
    color: inherit;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .armed {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
    font-style: italic;
  }
</style>
