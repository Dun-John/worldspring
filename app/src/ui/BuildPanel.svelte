<script lang="ts" module>
  export type BuildMode = 'building' | 'castle' | 'wall' | 'crossing' | 'clear';
</script>

<script lang="ts">
  // Edit › Build: draw a building on the map (a rectangle, any polygon, a round tower), snapped
  // to the 5-ft grid, and say what it is, its storeys, roof and roof colour. While the tab is
  // open the map's pointer draws; each footprint drawn becomes a building. Editing one: change
  // its options, or draw again to move or reshape it. Or put a crossing down (a bridge, ford
  // or ferry) by clicking one bank, then the other; clicking one picks it to change or take away.
  // Castle: an outline (a rectangle or a convex polygon), then the side its gate is in; Wall: a
  // line of corners (a ring if closed), gates on the corners clicked twice.
  import { CROSSING_WIDTH, MAX_FLOORS, ROOFS, TINTS, type BuildingFuncs, type CrossingKind } from '../gen/protocol';
  import { ROOF_TINTS } from '../gen/battlePrep';
  import type { BuildSettings, BuildShape } from '../editor/build';
  import type { CrossSettings } from '../editor/crossing';
  import type { ClearShape } from '../editor/clearArea';
  import type { WorksSettings } from '../editor/works';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  interface Props {
    settings: BuildSettings;
    funcs: BuildingFuncs | null;
    /** The building being changed (its name), else drawing new ones. */
    editing: string | null;
    /** Buildings, castles, walls, crossings, or clearing the world's own buildings. */
    mode: BuildMode;
    works: WorksSettings;
    /** The castle or wall being changed (its name), else drawing new ones. */
    worksEditing: string | null;
    /** Clear area: a box dragged, or a lasso drawn. */
    clearShape: ClearShape;
    cross: CrossSettings;
    /** The crossing picked (what it is), else putting new ones down. */
    crossEditing: string | null;
    /** Whether the map is close enough to draw on. */
    near: boolean;
    /** Only the shapes (the panel is folded down). */
    peek?: boolean;
    onSave: () => void;
    onDone: () => void;
    onMode: (m: BuildMode) => void;
    onWorksSave: () => void;
    onCrossSave: () => void;
    onCrossRemove: () => void;
    /** Fly in close enough to draw. */
    onZoomIn: () => void;
  }

  let {
    settings = $bindable(),
    funcs,
    editing,
    mode,
    works = $bindable(),
    worksEditing,
    clearShape = $bindable(),
    cross = $bindable(),
    crossEditing,
    near,
    peek = false,
    onSave,
    onDone,
    onMode,
    onWorksSave,
    onCrossSave,
    onCrossRemove,
    onZoomIn,
  }: Props = $props();

  const KINDS: { key: CrossingKind; label: string; icon: IconName; kbd: string; look: string; width: number }[] = [
    { key: 'bridge', label: 'Bridge', icon: 'bridge', kbd: '1', look: 'A timber deck clear of the water, walked like a road.', width: 12 },
    { key: 'ford', label: 'Ford', icon: 'ford', kbd: '2', look: 'The bed brought up to wading depth, stepping stones across.', width: 10 },
    { key: 'ferry', label: 'Ferry', icon: 'ferry', kbd: '3', look: 'A jetty out from each bank, a raft on a rope between. At least 68 ft.', width: 12 },
  ];
  const kindLook = $derived(KINDS.find((k) => k.key === cross.kind)?.look ?? '');
  const widthMin = CROSSING_WIDTH[0];
  const widthMax = CROSSING_WIDTH[1];

  const SHAPES: { key: BuildShape; label: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'rect', label: 'Rectangle', icon: 'square', kbd: 'R', hint: 'Drag from corner to corner.' },
    { key: 'poly', label: 'Polygon', icon: 'polygon', kbd: 'O', hint: 'Click each corner; click the first again, double-click or press Enter to finish. Backspace takes a corner back.' },
    { key: 'tower', label: 'Round tower', icon: 'circle', kbd: 'T', hint: 'Drag from the middle out.' },
  ];
  const ROOF_LABEL: Record<string, string> = { hip: 'Pitched', battlements: 'Battlements', cone: 'Cone' };
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  const label = (s: string) => s.replace(/_/g, ' ').replace(/^./, (c) => c.toUpperCase());

  const MODES: { key: BuildMode; label: string; icon: IconName; title: string }[] = [
    { key: 'building', label: 'Building', icon: 'building', title: 'Draw a building (B)' },
    { key: 'castle', label: 'Castle', icon: 'castle', title: 'Draw a castle: its walls, gate, keep and yard' },
    { key: 'wall', label: 'Wall', icon: 'wall', title: 'Draw a wall with towers and gates' },
    { key: 'crossing', label: 'Crossing', icon: 'bridge', title: 'A bridge, ford or ferry (X)' },
    { key: 'clear', label: 'Clear', icon: 'trash', title: 'Take away the town’s own buildings in an area' },
  ];

  const categories = $derived(funcs ? [...new Set(funcs.businesses.map((b) => b.category))] : []);

  function shape(k: BuildShape) {
    // A round tower is a wizard's tower unless it was something else already.
    if (k === 'tower' && settings.func === 'house') settings = { ...settings, shape: k, func: 'wizard_tower', floors: Math.max(settings.floors, 4) };
    else settings = { ...settings, shape: k };
  }
</script>

<div class="build">
  {#if !peek}
    <div class="ws-seg modes" role="radiogroup" aria-label="What to build">
      {#each MODES as m (m.key)}
        <button class:on={mode === m.key} aria-pressed={mode === m.key} onclick={() => onMode(m.key)} title={m.title}><Icon name={m.icon} size={16} />{m.label}</button>
      {/each}
    </div>
  {/if}
  {#if mode === 'castle' || mode === 'wall'}
    {#if worksEditing}
      <div class="editing">
        <span class="grow">Changing <b>{worksEditing}</b></span>
        <button class="ws-btn" onclick={onDone}>Done</button>
      </div>
    {/if}
    {#if mode === 'castle'}
      <div class="ws-seg" role="radiogroup" aria-label="Outline">
        <button class:on={works.castleShape === 'rect'} aria-pressed={works.castleShape === 'rect'} onclick={() => (works.castleShape = 'rect')} title="Rectangle (R)"><Icon name="square" size={16} />Rectangle</button>
        <button class:on={works.castleShape === 'poly'} aria-pressed={works.castleShape === 'poly'} onclick={() => (works.castleShape = 'poly')} title="Polygon (O)"><Icon name="polygon" size={16} />Polygon</button>
      </div>
    {/if}
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to draw.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">
        {#if mode === 'castle'}
          {worksEditing ? 'Click a side to move the gate there, or draw the walls again. ' : ''}{works.castleShape === 'rect'
            ? 'Drag the walls from corner to corner'
            : 'Click each corner (no corner turning inward); click the first again or press Enter'}, then click the side the gate is in (Enter: the side facing the road). 80 to 600 ft across.
        {:else}
          {worksEditing ? 'Click the wall to add a gate there (on a gate: take it away), or draw it again. ' : ''}Click each corner; click a corner twice to make it a gate.
          Double-click or press Enter to finish. Roads and streets that cross it get a gate. Up to 3,000 ft.
        {/if}
      </div>
      {#if mode === 'castle'}
        <label class="switch">
          <span class="grow">Keep<span class="ws-muted"> · the castle’s great tower in the yard</span></span>
          <input type="checkbox" class="ws-switch" bind:checked={works.keep} />
        </label>
        <label class="switch">
          <span class="grow">Yard buildings<span class="ws-muted"> · barracks, stables, a smithy along the walls</span></span>
          <input type="checkbox" class="ws-switch" bind:checked={works.yardBuildings} />
        </label>
        <label class="switch">
          <span class="grow">In ruins<span class="ws-muted"> · broken walls, roofless</span></span>
          <input type="checkbox" class="ws-switch" bind:checked={works.ruin} />
        </label>
      {:else}
        <label class="switch">
          <span class="grow">Closed ring<span class="ws-muted"> · the last corner joins the first</span></span>
          <input type="checkbox" class="ws-switch" bind:checked={works.closed} />
        </label>
      {/if}
      <label class="ws-field">
        Name
        <input class="ws-input" bind:value={works.name} placeholder={worksEditing ? '' : 'Named for the place if left empty'} />
      </label>
      {#if worksEditing}<button class="ws-btn primary block" onclick={onWorksSave}>Save changes</button>{/if}
    {/if}
  {:else if mode === 'clear'}
    <div class="ws-seg" role="radiogroup" aria-label="Area">
      <button class:on={clearShape === 'box'} aria-pressed={clearShape === 'box'} onclick={() => (clearShape = 'box')}><Icon name="square" size={16} />Box</button>
      <button class:on={clearShape === 'lasso'} aria-pressed={clearShape === 'lasso'} onclick={() => (clearShape = 'lasso')}><Icon name="polygon" size={16} />Lasso</button>
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to clear an area.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">
        {clearShape === 'box' ? 'Drag a box' : 'Draw round an area'}: the town's own buildings with their middle inside it are taken away, leaving the ground open to build on.
        Undo brings them back.
      </div>
    {/if}
  {:else if mode === 'crossing'}
    {#if crossEditing}
      <div class="editing">
        <span class="grow">Changing the <b>{crossEditing}</b></span>
        <button class="ws-btn" onclick={onCrossRemove} title="Take it away"><Icon name="trash" size={16} /> Remove</button>
        <button class="ws-btn" onclick={onDone}>Done</button>
      </div>
    {/if}
    <div class="ws-seg" role="radiogroup" aria-label="Crossing">
      {#each KINDS as k (k.key)}
        <button
          class:on={cross.kind === k.key}
          aria-pressed={cross.kind === k.key}
          onclick={() => (cross = { kind: k.key, width: crossEditing ? cross.width : k.width })}
          title="{k.label} ({k.kbd})"><Icon name={k.icon} size={16} />{k.label}</button
        >
      {/each}
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to put one down.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">
        {crossEditing ? 'Click both banks again to move it. ' : 'Click one bank, then the other; click one put down earlier to change it. '}{kindLook}
      </div>
      <div class="ws-field">
        Width (ft)
        <div class="stepper">
          <button class="ws-icon-btn" onclick={() => (cross.width = Math.max(widthMin, cross.width - 1))} disabled={cross.width <= widthMin} aria-label="Narrower"><Icon name="minus" size={16} /></button>
          <span>{cross.width}</span>
          <button class="ws-icon-btn" onclick={() => (cross.width = Math.min(widthMax, cross.width + 1))} disabled={cross.width >= widthMax} aria-label="Wider"><Icon name="plus" size={16} /></button>
        </div>
      </div>
      {#if crossEditing}<button class="ws-btn primary block" onclick={onCrossSave}>Save changes</button>{/if}
    {/if}
  {:else}
  {#if editing}
    <div class="editing">
      <span class="grow">Editing <b>{editing}</b></span>
      <button class="ws-btn" onclick={onDone}>Done</button>
    </div>
  {/if}
  <div class="ws-seg" role="radiogroup" aria-label="Shape">
    {#each SHAPES as s (s.key)}
      <button class:on={settings.shape === s.key} aria-pressed={settings.shape === s.key} onclick={() => shape(s.key)} title="{s.label} ({s.kbd})"><Icon name={s.icon} size={16} />{s.label}</button>
    {/each}
  </div>
  {#if !near}
    <div class="zoom">
      <span class="grow">Zoom in to draw.</span>
      <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
    </div>
  {/if}
  {#if !peek}
    <div class="ws-hint">{editing ? 'Draw on the map to move or reshape it: ' : ''}{SHAPES.find((s) => s.key === settings.shape)?.hint}</div>
    <label class="ws-field">
      What it is
      <select class="ws-input" bind:value={settings.func}>
        {#if funcs}
          <optgroup label="Homes">
            {#each funcs.homes as h (h.key)}<option value={h.key}>{label(h.name)}</option>{/each}
          </optgroup>
          {#each categories as c (c)}
            <optgroup label={label(c)}>
              {#each funcs.businesses.filter((b) => b.category === c) as b (b.key)}<option value={b.key}>{b.name}</option>{/each}
            </optgroup>
          {/each}
        {:else}
          <option value={settings.func}>{label(settings.func)}</option>
        {/if}
      </select>
    </label>
    <div class="ws-field">
      Storeys
      <div class="stepper">
        <button class="ws-icon-btn" onclick={() => (settings.floors = Math.max(1, settings.floors - 1))} disabled={settings.floors <= 1} aria-label="One storey less"><Icon name="minus" size={16} /></button>
        <span>{settings.floors}</span>
        <button class="ws-icon-btn" onclick={() => (settings.floors = Math.min(MAX_FLOORS, settings.floors + 1))} disabled={settings.floors >= MAX_FLOORS} aria-label="One storey more"><Icon name="plus" size={16} /></button>
      </div>
    </div>
    <div class="ws-field">
      Roof
      <div class="ws-seg">
        <button class:on={settings.roof === ''} onclick={() => (settings.roof = '')} title="As its kind has it">Auto</button>
        {#each ROOFS as r (r)}<button class:on={settings.roof === r} onclick={() => (settings.roof = r)}>{ROOF_LABEL[r]}</button>{/each}
      </div>
    </div>
    <div class="ws-field">
      Roof colour
      <div class="tints" role="radiogroup" aria-label="Roof colour">
        <button class="ws-chip" class:on={settings.tint === ''} role="radio" aria-checked={settings.tint === ''} onclick={() => (settings.tint = '')} title="Picked for it">Auto</button>
        {#each TINTS as t, i (t)}
          <button class="swatch" class:on={settings.tint === t} role="radio" aria-checked={settings.tint === t} style:background={hex(ROOF_TINTS[i])} onclick={() => (settings.tint = t)} title={label(t)} aria-label={label(t)}></button>
        {/each}
      </div>
    </div>
    <label class="switch">
      <span class="grow">In ruins<span class="ws-muted"> · no roof, no way in</span></span>
      <input type="checkbox" class="ws-switch" bind:checked={settings.ruin} />
    </label>
    <label class="ws-field">
      Name
      <input class="ws-input" bind:value={settings.name} placeholder={editing ? '' : 'Named for its trade if left empty'} />
    </label>
    {#if editing}<button class="ws-btn primary block" onclick={onSave}>Save changes</button>{/if}
  {/if}
  {/if}
</div>

<style>
  .build {
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
  .editing,
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
  }
  .stepper {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 140px;
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
  .tints {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .swatch {
    width: 24px;
    height: 24px;
    border: 1px solid var(--line);
    border-radius: 50%;
    cursor: pointer;
    padding: 0;
  }
  .swatch.on {
    outline: 2px solid var(--ink);
    outline-offset: 2px;
  }
  /* Five modes side by side: each its icon over its word. */
  .modes > button {
    flex-direction: column;
    gap: 0;
    padding: 4px 2px;
    font-size: 12px;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
</style>
