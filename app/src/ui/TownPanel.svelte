<script lang="ts">
  // Edit › Streets: the town in view laid out anew by hand (the ward editor). Its patches are drawn
  // over the map; a corner dragged, or the brushes, move them (applied on release); a patch
  // picked can be given another ward, lot size, joined to a neighbour's district or laid out
  // again, a temple's kept as a close round it, a park's laid out as a formal garden or a common; a
  // corner where streets meet picked can be given a square; the town's walls put up or taken down.
  // Each change is one step to undo.
  import { LOT_SIZES, PARK_KINDS, SQUARE_KINDS, TOWN_WARDS, type LotSize, type ParkKind, type SquareKind, type TownCorner, type TownPatch, type TownPlan, type TownRequest, type TownWard } from '../gen/protocol';
  import { SQUARE_COLOURS, WARD_COLOURS, type TownMode } from '../editor/town';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  interface Props {
    plan: TownPlan | null;
    /** The town's name; null: no town in view. */
    name: string | null;
    /** Why there is no plan (loading, a village…). */
    note: string;
    mode: TownMode;
    /** The brush's size, in patch widths. */
    size: number;
    selected: TownPatch | null;
    /** The corner picked (its square), if any. */
    corner: TownCorner | null;
    /** Waiting for a neighbour to be picked to join. */
    merging: boolean;
    /** A change is being made. */
    busy: boolean;
    /** Close enough to see the patches. */
    near: boolean;
    peek?: boolean;
    onMode: (m: TownMode) => void;
    onChange: (req: TownRequest, what: string) => void;
    onMerge: () => void;
    onZoomIn: () => void;
  }

  let { plan, name, note, mode, size = $bindable(), selected, corner, merging, busy, near, peek = false, onMode, onChange, onMerge, onZoomIn }: Props = $props();

  const MODES: { key: TownMode; label: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'select', label: 'Choose', icon: 'pointer', kbd: 'V', hint: 'Click a patch to set its ward, a ringed corner for a square where streets meet; drag a corner to move it.' },
    { key: 'displace', label: 'Displace', icon: 'move', kbd: 'Q', hint: 'Drag: the corners in the ring go with you, those in its middle all the way.' },
    { key: 'liquify', label: 'Liquify', icon: 'liquify', kbd: 'L', hint: 'Drag: the corners under the ring are smudged along.' },
    { key: 'bloat', label: 'Bloat', icon: 'bloat', kbd: 'O', hint: 'Press and drag: the patches round where you pressed swell, the further you drag the more.' },
    { key: 'pinch', label: 'Pinch', icon: 'pinch', kbd: 'I', hint: 'Press and drag: the patches round where you pressed shrink, the further you drag the more.' },
    { key: 'relax', label: 'Relax', icon: 'relax', kbd: 'R', hint: 'Scrub over the town: each corner eases toward the middle of its neighbours.' },
    { key: 'equalize', label: 'Equalize', icon: 'polygon', kbd: 'E', hint: 'Click or drag over patches: each is made more regular.' },
  ];

  const WARD_LABEL: Record<TownWard, string> = {
    plaza: 'Market square',
    castle: 'Castle',
    temple: 'Temple',
    merchant: 'Merchants',
    craft: 'Craftsmen',
    noble: 'Nobles',
    common: 'Common folk',
    slum: 'Slums',
    docks: 'Docks',
    military: 'Barracks',
    farm: 'Farms',
    park: 'Park',
    empty: 'Open ground',
  };
  const LOT_LABEL: Record<LotSize, string> = { small: 'Small', medium: 'Medium', large: 'Large', huge: 'Huge' };
  const PARK_LABEL: Record<ParkKind, string> = { formal: 'Formal garden', common: 'Common' };
  const PARK_HINT: Record<ParkKind, string> = {
    formal: 'cross walks, a fountain, hedged beds, statues and topiary',
    common: 'winding paths, a pond, trees, a gazebo or a bandstand',
  };
  const SQUARE_LABEL: Record<SquareKind | 'none', string> = {
    crossroads: 'Widened crossroads',
    well: 'Well square',
    fountain: 'Fountain square',
    green: 'Small green',
    none: 'No square',
  };
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  const ft = (v: number) => `${Math.round(v).toLocaleString('en')} ft`;

  const hint = $derived(MODES.find((m) => m.key === mode)?.hint ?? '');
  const brush = $derived(mode !== 'select' && mode !== 'equalize');
  const generatedWard = $derived(selected ? (selected.ward_generated ?? selected.ward) : 'empty');
  const set = (req: TownRequest['patches'], what: string) => selected && onChange({ patches: req }, what);
  const generatedSquare = $derived(corner ? (corner.square_generated ?? corner.square ?? 'none') : 'none');
</script>

<div class="town">
  {#if !peek && name}
    <div class="head">
      <span class="grow"><b>{name}</b>{#if plan?.edited}<span class="ws-muted"> · laid out by hand</span>{/if}</span>
    </div>
  {/if}
  <div class="ws-seg modes" role="radiogroup" aria-label="Tool">
    {#each MODES as m (m.key)}
      <button class:on={mode === m.key} aria-pressed={mode === m.key} onclick={() => onMode(m.key)} title="{m.label} ({m.kbd})"><Icon name={m.icon} size={16} />{m.label}</button>
    {/each}
  </div>
  {#if !plan}
    <div class="zoom">
      <span class="grow">{note}</span>
      {#if name && !near}<button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>{/if}
    </div>
  {:else if !near}
    <div class="zoom">
      <span class="grow">Zoom in to see its patches.</span>
      <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
    </div>
  {/if}
  {#if !peek && plan}
    {#if brush || mode === 'equalize' || !selected}
      <div class="ws-hint">{hint}{mode === 'select' ? '' : ' Applied when you let go.'}</div>
    {/if}
    {#if brush}
      <label class="slider">
        <span>Brush</span><output>{ft(2 * size * plan.max_move_ft)} across</output>
        <input type="range" min="0.25" max="4" step="0.05" bind:value={size} />
      </label>
    {/if}
    {#if selected}
      <section class="card" aria-label="Patch">
        <div class="title">
          <span class="dot" style:background={hex(WARD_COLOURS[selected.ward])}></span>
          <span class="grow"><b>{selected.district?.name ?? (selected.in_town ? 'A patch of the town' : 'Outside the town')}</b></span>
        </div>
        <label class="ws-field">
          Ward
          <select class="ws-input" value={selected.ward} disabled={busy} onchange={(e) => set([{ patch: selected.patch, ward: (e.currentTarget as HTMLSelectElement).value as TownWard }], 'ward')}>
            {#each TOWN_WARDS as w (w)}
              <option value={w} disabled={w === 'park' && !!selected.park_refused} title={w === 'park' ? selected.park_refused : undefined}>{WARD_LABEL[w]}{w === generatedWard ? ' (as generated)' : ''}{w === 'park' && selected.park_refused ? ' (not here)' : ''}</option>
            {/each}
          </select>
        </label>
        {#if selected.ward === 'park'}
          <div class="ws-field">
            Laid out as
            <div class="ws-seg" role="radiogroup" aria-label="Laid out as">
              <button class:on={!selected.park_by_hand} aria-pressed={!selected.park_by_hand} disabled={busy} onclick={() => set([{ patch: selected.patch, park: 'auto' }], 'park')} title="As the town has it: formal by the castle and the nobles, a common elsewhere">Auto</button>
              {#each PARK_KINDS as k (k)}
                <button class:on={!!selected.park_by_hand && selected.park === k} aria-pressed={!!selected.park_by_hand && selected.park === k} disabled={busy} onclick={() => set([{ patch: selected.patch, park: k }], 'park')}>{PARK_LABEL[k]}</button>
              {/each}
            </div>
            {#if selected.park}<span class="ws-muted">{PARK_LABEL[selected.park]}: {PARK_HINT[selected.park]}</span>{/if}
          </div>
        {:else}
          <div class="ws-field">
            Lots
            <div class="ws-seg" role="radiogroup" aria-label="Lots">
              <button class:on={!selected.lots} aria-pressed={!selected.lots} disabled={busy} onclick={() => set([{ patch: selected.patch, lots: 'auto' }], 'lots')} title="As its ward has them">Auto</button>
              {#each LOT_SIZES as l (l)}
                <button class:on={selected.lots === l} aria-pressed={selected.lots === l} disabled={busy} onclick={() => set([{ patch: selected.patch, lots: l }], 'lots')}>{LOT_LABEL[l]}</button>
              {/each}
            </div>
          </div>
        {/if}
        <div class="row">
          {#if selected.merged_with !== undefined}
            <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, merge_with: 'none' }], 'merge')} title="Its own district again">Leave the district</button>
          {:else}
            <button class="ws-btn" class:on={merging} aria-pressed={merging} disabled={busy} onclick={onMerge} title="Then click a patch next to it: this one takes its ward and district">
              {merging ? 'Click a neighbour…' : 'Join a neighbour'}
            </button>
          {/if}
          <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, reroll: true }], 'reroll')} title="Its streets and buildings laid out another way"><Icon name="dice" size={16} /> Lay out again</button>
          <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, as_generated: true }], 'patch')} title="Ward, lots, district and layout as generated"><Icon name="reset" size={16} /> As generated</button>
        </div>
        {#if selected.ward === 'temple'}
          <label class="switch" title={selected.close ? undefined : selected.close_refused}>
            <span class="grow">Close round the temple<span class="ws-muted"> · {selected.close || !selected.close_refused ? 'open ground round the temple and its hedged churchyard, the houses round it kept' : selected.close_refused}</span></span>
            <input type="checkbox" class="ws-switch" checked={!!selected.close} disabled={busy || (!selected.close && !!selected.close_refused)} onchange={(e) => set([{ patch: selected.patch, close: (e.currentTarget as HTMLInputElement).checked }], 'close')} />
          </label>
        {/if}
      </section>
    {/if}
    {#if corner}
      <section class="card" aria-label="Corner">
        <div class="title">
          <span class="dot" style:background={corner.square ? hex(SQUARE_COLOURS[corner.square]) : 'transparent'}></span>
          <span class="grow"><b>{corner.square ? SQUARE_LABEL[corner.square] : corner.junction ? 'Where streets meet' : 'A corner'}</b></span>
        </div>
        {#if corner.junction || corner.square}
          <label class="ws-field">
            Square
            <select class="ws-input" value={corner.square ?? 'none'} disabled={busy} onchange={(e) => onChange({ squares: [{ corner: corner.corner, kind: (e.currentTarget as HTMLSelectElement).value as SquareKind | 'none' }] }, 'square')}>
              {#each [...SQUARE_KINDS, 'none' as const] as k (k)}
                <option value={k}>{SQUARE_LABEL[k]}{k === generatedSquare ? ' (as generated)' : ''}</option>
              {/each}
            </select>
          </label>
          {#if corner.square_generated !== undefined}
            <div class="row">
              <button class="ws-btn" disabled={busy} onclick={() => onChange({ squares: [{ corner: corner.corner, kind: 'auto' }] }, 'square')}><Icon name="reset" size={16} /> As generated</button>
            </div>
          {/if}
        {:else}
          <div class="ws-hint">No square opens here: {corner.square_refused ?? 'it needs three streets or more meeting between built-up patches, inside the walls and off the water'}.</div>
        {/if}
      </section>
    {/if}
    <label class="switch">
      <span class="grow">Walls<span class="ws-muted"> · {plan.walls.generated ? 'it was built with walls' : 'it was built without'}</span></span>
      <input type="checkbox" class="ws-switch" checked={plan.walls.built} disabled={busy} onchange={(e) => onChange({ walls: (e.currentTarget as HTMLInputElement).checked }, 'walls')} />
    </label>
    {#if plan.set_aside.length}
      <div class="aside">Set aside since the world changed: {plan.set_aside.join('; ')}.</div>
    {/if}
    {#if plan.edited}
      <div class="row">
        <button class="ws-btn" disabled={busy} onclick={() => onChange({ reset: 'corners' }, 'corners')}>Corners as generated</button>
        <button class="ws-btn" disabled={busy} onclick={() => onChange({ reset: 'patches' }, 'wards')}>Wards as generated</button>
        {#if plan.corners.some((c) => c.square_generated !== undefined)}
          <button class="ws-btn" disabled={busy} onclick={() => onChange({ reset: 'squares' }, 'squares')}>Squares as generated</button>
        {/if}
        <button class="ws-btn" disabled={busy} onclick={() => onChange({ reset: 'all' }, 'all')}><Icon name="reset" size={16} /> All as generated</button>
      </div>
    {/if}
  {/if}
</div>

<style>
  .town {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .head,
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
  .dot {
    width: 14px;
    height: 14px;
    border: 1px solid var(--line);
    border-radius: 50%;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .row .on {
    outline: 2px solid var(--ink);
  }
  .aside {
    color: var(--warn);
    font-size: 12px;
  }
  /* Seven tools side by side: each its icon over its word. */
  .modes > button {
    flex-direction: column;
    gap: 0;
    padding: 4px 1px;
    font-size: 11px;
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
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
</style>
