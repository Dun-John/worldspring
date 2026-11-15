<script lang="ts" module>
  export type BuildMode = 'building' | 'stamp' | 'row' | 'lasso' | 'castle' | 'wall' | 'crossing' | 'water' | 'harbour' | 'clear' | 'road';
  /** What the app says of a quick placement's ghost. */
  export interface QuickInfo {
    /** Something is ready to place. */
    can: boolean;
    /** The line to show (what it is, or why not). */
    text: string;
    bad?: boolean;
    /** The generated buildings it takes away (clear). */
    removed?: number;
  }
</script>

<script lang="ts">
  // Edit › Build: draw a building on the map (a rectangle, any polygon, a round tower), snapped
  // to the 5-ft grid, and say what it is, its storeys, roof shape and material. While the tab is
  // open the map's pointer draws; each footprint drawn becomes a building. Editing one: change
  // its options, or draw again to move or reshape it. Or put a crossing down (a bridge, ford
  // or ferry) by clicking one bank, then the other; clicking one picks it to change or take away.
  // Castle: an outline (a rectangle or a polygon), then the side its gate is in (more gates, its
  // keep's shape and its moat while changing it); Wall: a
  // line of corners (a ring if closed), gates on the corners clicked twice.
  import { CROSSING_WIDTH, LOT_SIZES, TOWN_WARDS, NARROW_CROSSING_MAX, WHARF_WIDTH, MAX_FLOORS, ROOFS, ROOF_MATERIALS, ROOF_MATERIAL_RGB, ROOF_SHADES, ROOF_SHAPES, type BuildingFuncs, type CrossingKind, type CrossingMaterial, type WharfKind } from '../gen/protocol';
  import type { BuildSettings, BuildShape } from '../editor/build';
  import { SHOP_DEPTH_RANGE, SHOP_LANE_RANGE, shopsWidth, type CrossSettings } from '../editor/crossing';
  import { BRUSH_WIDTHS, DEPTHS, POND_BELOW_FT, type WaterSettings } from '../editor/water';
  import type { WaterKind } from '../gen/protocol';
  import { PATH_KINDS, PATH_WIDTH, type PathSettings } from '../editor/paths';
  import type { PathKind } from '../gen/protocol';
  import { ISLET_RADIUS, ISLET_RISE, type HarbourSettings, type HarbourKind } from '../editor/harbour';
  import type { ClearShape } from '../editor/clearArea';
  import type { LassoSettings, RowSettings, StampSettings } from '../editor/quick';
  import type { WorksSettings } from '../editor/works';
  import Icon from './Icon.svelte';
  import type { KeepShape, Moat } from '../gen/protocol';

  const KEEP_SHAPES: { key: KeepShape | 'auto'; label: string; title: string }[] = [
    { key: 'auto', label: 'Any keep', title: 'A keep of the castle’s own kind (now and then a shell keep on a mound)' },
    { key: 'square', label: 'Square', title: 'A keep of square towers and halls' },
    { key: 'round', label: 'Round', title: 'A round great tower' },
    { key: 'drawn', label: 'Drawn', title: 'The keep as drawn' },
  ];
  const MOATS: { key: Moat | 'auto'; label: string; title: string }[] = [
    { key: 'auto', label: 'Any moat', title: 'A moat or none, as the castle’s age and ground have it' },
    { key: 'none', label: 'No moat', title: 'No moat' },
    { key: 'dry', label: 'Dry', title: 'A dry ditch round the walls' },
    { key: 'wet', label: 'Wet', title: 'A moat of water round the walls' },
  ];
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
    /** The view is over a town laid out by hand. */
    handTown?: boolean;
    /** Clear area: a box dragged, or a lasso drawn. */
    clearShape: ClearShape;
    stamp: StampSettings;
    row: RowSettings;
    lasso: LassoSettings;
    /** The ghost's state in Stamp, Row or Lasso. */
    quick: QuickInfo;
    cross: CrossSettings;
    /** The crossing picked (what it is), else putting new ones down. */
    crossEditing: string | null;
    water: WaterSettings;
    /** The water picked (its look, name, a line's width, depth and shore), else drawing new water. */
    waterPicked: { look: WaterKind; name: string; width: number | null; depth: number; dug: boolean } | null;
    onWaterChange: (change: { name?: string; width?: number; depth?: number; dug?: boolean }) => void;
    onWaterRemove: () => void;
    road: PathSettings;
    /** The road drawn by hand picked (its type, width and name), else drawing new ones. */
    roadPicked: { kind: PathKind; width: number; name: string } | null;
    onRoadChange: (change: { kind?: PathKind; width?: number; name?: string }) => void;
    onRoadRemove: () => void;
    harbour: HarbourSettings;
    /** The wharf picked (its id), else drawing new ones. */
    harbourPicked: string | null;
    /** Whether the map is close enough to draw on. */
    near: boolean;
    /** Only the shapes (the panel is folded down). */
    peek?: boolean;
    onSave: () => void;
    onDone: () => void;
    onMode: (m: BuildMode) => void;
    /** Put the stamp, row or plot down. */
    onPlace: () => void;
    /** Drop the row or outline being drawn. */
    onDrop: () => void;
    onWorksSave: () => void;
    onCrossSave: () => void;
    onCrossRemove: () => void;
    onHarbourRemove: () => void;
    /** Fly in close enough to draw. */
    onZoomIn: () => void;
  }

  let {
    settings = $bindable(),
    funcs,
    editing,
    handTown = false,
    mode,
    works = $bindable(),
    worksEditing,
    clearShape = $bindable(),
    stamp = $bindable(),
    row = $bindable(),
    lasso = $bindable(),
    quick,
    cross = $bindable(),
    crossEditing,
    water = $bindable(),
    waterPicked,
    onWaterChange,
    onWaterRemove,
    road = $bindable(),
    roadPicked,
    onRoadChange,
    onRoadRemove,
    harbour = $bindable(),
    harbourPicked,
    near,
    peek = false,
    onSave,
    onDone,
    onMode,
    onPlace,
    onDrop,
    onWorksSave,
    onCrossSave,
    onCrossRemove,
    onHarbourRemove,
    onZoomIn,
  }: Props = $props();

  const KINDS: { key: CrossingKind; label: string; icon: IconName; kbd: string; look: string; width: number }[] = [
    { key: 'bridge', label: 'Bridge', icon: 'bridge', kbd: '1', look: 'A deck clear of the water, walked like a road.', width: 12 },
    { key: 'ford', label: 'Ford', icon: 'ford', kbd: '2', look: 'The bed brought up to wading depth, stepping stones across.', width: 10 },
    { key: 'ferry', label: 'Ferry', icon: 'ferry', kbd: '3', look: 'A jetty out from each bank, a raft on a rope between. At least 68 ft.', width: 12 },
  ];
  const MATERIALS: { key: CrossingMaterial; label: string; look: string }[] = [
    { key: 'timber', label: 'Timber', look: 'A timber deck clear of the water, walked like a road.' },
    { key: 'stone', label: 'Old stone', look: 'An old stone bridge on piers, mossy and worn, walked like a road.' },
  ];
  const kindLook = $derived(cross.kind === 'bridge' ? (MATERIALS.find((m) => m.key === cross.material)?.look ?? '') : (KINDS.find((k) => k.key === cross.kind)?.look ?? ''));
  const widthMax = $derived(cross.kind === 'bridge' ? CROSSING_WIDTH[1] : NARROW_CROSSING_MAX);
  // (A bridge with shops is wide enough for its rows and the lane between.)
  const widthMin = $derived(cross.kind === 'bridge' && cross.shops ? Math.min(widthMax, shopsWidth(cross.shopDepth, cross.shopLane)) : CROSSING_WIDTH[0]);
  function shopsOn(on: boolean) {
    cross = { ...cross, shops: on, width: on ? Math.max(cross.width, shopsWidth(cross.shopDepth, cross.shopLane)) : cross.width };
  }
  function shopDepth(d: number) {
    cross = { ...cross, shopDepth: d, width: Math.max(cross.width, shopsWidth(d, cross.shopLane)) };
  }
  function shopLane(l: number) {
    cross = { ...cross, shopLane: l, width: Math.max(cross.width, shopsWidth(cross.shopDepth, l)) };
  }

  const HARBOUR: { key: HarbourKind; label: string; icon: IconName; look: string }[] = [
    { key: 'islet', label: 'Islet', icon: 'mountain', look: 'Click in a river, lake or the sea: a rock islet rises there. A building, tower or castle may stand on it.' },
    { key: 'quay', label: 'Quay', icon: 'wall', look: 'Drag along a shore: paving to the dry side, the water behind it filled in.' },
    { key: 'pier', label: 'Pier', icon: 'bridge', look: 'Drag from the bank out over the water: a deck on piles.' },
    { key: 'breakwater', label: 'Breakwater', icon: 'waves', look: 'Drag from the bank out into the water: a stone mole, with a lighthouse at its tip if you like.' },
  ];
  function harbourKind(k: HarbourKind) {
    harbour = { ...harbour, kind: k, width: k === 'islet' ? harbour.width : WHARF_WIDTH[k as WharfKind][2] };
  }
  const wharfRange = $derived(harbour.kind === 'islet' ? null : WHARF_WIDTH[harbour.kind]);

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
    { key: 'stamp', label: 'Stamp', icon: 'building', title: 'Set down a building of a kind and size where you click (Q)' },
    { key: 'row', label: 'Row', icon: 'wall', title: 'A row of buildings along a street (E)' },
    { key: 'lasso', label: 'Plot', icon: 'polygon', title: 'Draw round a plot and fill it (L)' },
    { key: 'castle', label: 'Castle', icon: 'castle', title: 'Draw a castle: its walls, gate, keep and yard' },
    { key: 'wall', label: 'Wall', icon: 'wall', title: 'A wall standing on its own, anywhere: towers and gates where you click' },
    { key: 'crossing', label: 'Crossing', icon: 'bridge', title: 'A bridge, ford or ferry (X)' },
    { key: 'water', label: 'Water', icon: 'lake', title: 'Brush or outline water, of a width and depth' },
    { key: 'road', label: 'Road', icon: 'road', title: 'Draw a footpath, track, gravel road, paved road or cobbled street (V)' },
    { key: 'harbour', label: 'Harbour', icon: 'waves', title: 'Islets, quays, piers and breakwaters (H)' },
    { key: 'clear', label: 'Clear', icon: 'trash', title: 'Take away the town’s own buildings in an area' },
  ];

  const FACINGS: { label: string; deg: number | null }[] = [{ label: 'Auto', deg: null }, { label: 'East', deg: 0 }, { label: 'South', deg: 90 }, { label: 'West', deg: 180 }, { label: 'North', deg: 270 }];
  const STAMP_SIZES = [
    { key: 'small', label: 'Small' },
    { key: 'usual', label: 'Usual' },
    { key: 'large', label: 'Large' },
  ] as const;
  const SIDES = [
    { key: 'both', label: 'Both sides' },
    { key: 'left', label: 'Left' },
    { key: 'right', label: 'Right' },
  ] as const;
  const OPEN_KINDS = ['plaza', 'lawn', 'park', 'garden', 'market', 'cemetery', 'farmland'];
  const WARDS = TOWN_WARDS.filter((w) => w !== 'empty' && w !== 'castle');
  const kindName = (k: string) => funcs?.businesses.find((b) => b.key === k)?.name ?? funcs?.homes.find((h) => h.key === k)?.name ?? label(k);
  function addKind(k: string) {
    if (k && !row.kinds.includes(k)) row.kinds = [...row.kinds, k];
  }
  /** What the picked water looks like, by its look (`water.ts` `lookOf`). */
  const LOOKS: Record<WaterKind, string> = {
    inlet: 'the sea reaching in, a beach round it',
    basin: 'a harbour basin of the sea, a paved quay round it',
    lake: 'a lake',
    pond: 'a pond, reeds round it',
    moat: 'a ring of still water, as a moat',
    stream: 'a stream running downhill',
  };
  /** The width shown: the picked line's, or the one new brushed water gets. */
  const waterLine = $derived(waterPicked ? waterPicked.width !== null : water.brush);
  const waterWidth = $derived(waterPicked?.width ?? water.width);
  const waterDepth = $derived(waterPicked?.depth ?? water.depth);
  const waterDug = $derived(waterPicked?.dug ?? water.dug);

  const categories = $derived(funcs ? [...new Set(funcs.businesses.map((b) => b.category))] : []);

  function shape(k: BuildShape) {
    // A round tower is a wizard's tower unless it was something else already.
    if (k === 'tower' && settings.func === 'house') settings = { ...settings, shape: k, func: 'wizard_tower', floors: Math.max(settings.floors, 4) };
    else settings = { ...settings, shape: k };
  }
</script>

{#snippet quickLine(action: string)}
  <div class="quick" class:bad={quick.bad}>
    <span class="grow">{quick.text}</span>
    <button class="ws-btn primary" onclick={onPlace} disabled={!quick.can}>{action}</button>
  </div>
{/snippet}

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
          {worksEditing ? 'Click a side to add a gate in its middle (on a gate: take it away), or draw the walls again. ' : ''}{works.castleShape === 'rect'
            ? 'Drag the walls from corner to corner'
            : 'Click each corner; click the first again or press Enter'}, then click the side the gate is in (Enter: the side facing the road). 80 to 1,500 ft across.
        {:else}
          {#if handTown}Round this town, draw its wall in Streets › Draw › Town wall: gates open where its streets cross.<br />{/if}
          {worksEditing ? 'Click the wall to add a gate there (on a gate: take it away), or draw it again. ' : ''}Click each corner; click a corner twice to make it a gate.
          Double-click or press Enter to finish. Roads and streets that cross it get a gate. Up to 3,000 ft.
        {/if}
      </div>
      {#if mode === 'castle'}
        <label class="switch">
          <span class="grow">Keep<span class="ws-muted"> · the castle’s great tower in the yard</span></span>
          <input type="checkbox" class="ws-switch" bind:checked={works.keep} />
        </label>
        {#if works.keep}
          <div class="ws-seg" role="radiogroup" aria-label="Keep shape">
            {#each KEEP_SHAPES as k (k.key)}
              {#if k.key !== 'drawn' || works.keepShape === 'drawn'}
                <button class:on={works.keepShape === k.key} aria-pressed={works.keepShape === k.key} onclick={() => (works.keepShape = k.key)} title={k.title}>{k.label}</button>
              {/if}
            {/each}
          </div>
        {/if}
        <div class="ws-seg" role="radiogroup" aria-label="Moat">
          {#each MOATS as m (m.key)}
            <button class:on={works.moat === m.key} aria-pressed={works.moat === m.key} onclick={() => (works.moat = m.key)} title={m.title}>{m.label}</button>
          {/each}
        </div>
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
  {:else if mode === 'row'}
    <div class="ws-seg" role="radiogroup" aria-label="Side">
      {#each SIDES as s (s.key)}
        <button class:on={row.side === s.key} aria-pressed={row.side === s.key} onclick={() => (row.side = s.key)}>{s.label}</button>
      {/each}
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to draw.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">Drag along a street, or click each corner and press Enter: buildings line the path, facing it, on the side you choose. Left and right are as you walk along it.</div>
      <label class="ws-field">
        Ward
        <select class="ws-input" bind:value={row.ward}>
          {#each WARDS as w (w)}<option value={w}>{label(w)}</option>{/each}
        </select>
      </label>
      <div class="ws-field">
        Kinds
        <div class="tints">
          {#if !row.kinds.length}<span class="ws-hint">The ward’s homes and now and then a trade</span>{/if}
          {#each row.kinds as k (k)}
            <button class="ws-chip on" onclick={() => (row.kinds = row.kinds.filter((x) => x !== k))} title="Take it out of the mix">{kindName(k)} ×</button>
          {/each}
        </div>
        <select class="ws-input" value="" onchange={(e) => (addKind(e.currentTarget.value), (e.currentTarget.value = ''))} aria-label="Add a kind">
          <option value="">Add a kind…</option>
          {#if funcs}
            <optgroup label="Homes">
              {#each funcs.homes as h (h.key)}<option value={h.key}>{label(h.name)}</option>{/each}
            </optgroup>
            {#each categories as c (c)}
              <optgroup label={label(c)}>
                {#each funcs.businesses.filter((b) => b.category === c) as b (b.key)}<option value={b.key}>{b.name}</option>{/each}
              </optgroup>
            {/each}
          {/if}
        </select>
      </div>
      <label class="slider">
        <span>Depth</span><output>{row.depth ? `${row.depth} ft` : 'By the ward'}</output>
        <input type="range" min="0" max="120" step="5" bind:value={row.depth} />
      </label>
      <label class="slider">
        <span>Gaps</span><output>{Math.round(row.gaps * 100)}%</output>
        <input type="range" min="0" max="0.5" step="0.05" bind:value={row.gaps} />
      </label>
      <div class="ws-seg" role="radiogroup" aria-label="Facing">
        {#each FACINGS as f (f.label)}
          <button class:on={stamp.facing === f.deg} aria-pressed={stamp.facing === f.deg} onclick={() => (stamp.facing = f.deg)} title={f.deg == null ? 'Front toward the nearest street' : `Front toward the ${f.label.toLowerCase()}`}>{f.label}</button>
        {/each}
      </div>
      <label class="switch">
        <span class="grow">Clear the way<span class="ws-muted"> · the town’s own buildings in it go</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={row.clear} />
      </label>
      <div class="ws-seg">
        <button onclick={() => (row.seed += 1)} title="Another mix of widths, kinds and storeys">Reroll</button>
        <button onclick={onDrop} disabled={!quick.can && !quick.text}>Start over</button>
      </div>
      {@render quickLine('Place the row')}
    {/if}
  {:else if mode === 'lasso'}
    <div class="ws-seg" role="radiogroup" aria-label="Fill">
      <button class:on={lasso.fill === 'generated'} aria-pressed={lasso.fill === 'generated'} onclick={() => (lasso.fill = 'generated')}>Buildings</button>
      <button class:on={lasso.fill === 'hand'} aria-pressed={lasso.fill === 'hand'} onclick={() => (lasso.fill = 'hand')}>Empty</button>
      <button class:on={lasso.fill === 'open'} aria-pressed={lasso.fill === 'open'} onclick={() => (lasso.fill = 'open')}>Open ground</button>
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to draw.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">Draw round a plot, or click each corner (the first again to finish).
        {lasso.fill === 'generated' ? 'The generator fills it with lots of the ward.' : lasso.fill === 'hand' ? 'It is left empty for buildings drawn by hand.' : 'It stays open ground.'}
        A plot cannot overlap another.</div>
      {#if lasso.fill === 'generated'}
        <label class="ws-field">
          Ward
          <select class="ws-input" bind:value={lasso.ward}>
            {#each WARDS as w (w)}<option value={w}>{label(w)}</option>{/each}
          </select>
        </label>
        <div class="ws-field">
          Lot size
          <div class="ws-seg">
            <button class:on={lasso.lots === ''} onclick={() => (lasso.lots = '')} title="As the ward has it">Usual</button>
            {#each LOT_SIZES as l (l)}<button class:on={lasso.lots === l} onclick={() => (lasso.lots = l)}>{label(l)}</button>{/each}
          </div>
        </div>
        <div class="ws-field">
          Lay the lots out again
          <div class="stepper">
            <button class="ws-icon-btn" onclick={() => (lasso.reroll = Math.max(0, lasso.reroll - 1))} disabled={lasso.reroll <= 0} aria-label="Previous layout"><Icon name="minus" size={16} /></button>
            <span>{lasso.reroll}</span>
            <button class="ws-icon-btn" onclick={() => (lasso.reroll += 1)} aria-label="Another layout"><Icon name="plus" size={16} /></button>
          </div>
        </div>
      {:else if lasso.fill === 'open'}
        <label class="ws-field">
          Ground
          <select class="ws-input" bind:value={lasso.open}>
            {#each OPEN_KINDS as o (o)}<option value={o}>{label(o)}</option>{/each}
          </select>
        </label>
      {/if}
      <label class="ws-field">
        Name
        <input class="ws-input" bind:value={lasso.name} placeholder="Unnamed" />
      </label>
      <div class="ws-seg"><button onclick={onDrop} disabled={!quick.can && !quick.text}>Start over</button></div>
      {@render quickLine('Make the plot')}
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
          onclick={() => (cross = { ...cross, kind: k.key, width: crossEditing ? cross.width : k.width })}
          title="{k.label} ({k.kbd})"><Icon name={k.icon} size={16} />{k.label}</button
        >
      {/each}
    </div>
    {#if cross.kind === 'bridge'}
      <div class="ws-seg" role="radiogroup" aria-label="Built of">
        {#each MATERIALS as m (m.key)}
          <button class:on={cross.material === m.key} aria-pressed={cross.material === m.key} onclick={() => (cross = { ...cross, material: m.key })}>{m.label}</button>
        {/each}
      </div>
    {/if}
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
      {#if cross.kind === 'bridge'}
        <label class="switch"><span class="grow">Shops on the deck</span><input type="checkbox" class="ws-switch" checked={cross.shops} onchange={(e) => shopsOn(e.currentTarget.checked)} /></label>
        {#if cross.shops}
          <label class="slider">
            <span>Deepest shops</span><output>{cross.shopDepth} ft</output>
            <input type="range" min={SHOP_DEPTH_RANGE[0]} max={SHOP_DEPTH_RANGE[1]} step="1" value={cross.shopDepth} oninput={(e) => shopDepth(Number(e.currentTarget.value))} />
          </label>
          <label class="slider">
            <span>Lane between</span><output>{cross.shopLane} ft</output>
            <input type="range" min={SHOP_LANE_RANGE[0]} max={SHOP_LANE_RANGE[1]} step="1" value={cross.shopLane} oninput={(e) => shopLane(Number(e.currentTarget.value))} />
          </label>
          <label class="switch"><span class="grow">Vary the shops</span><input type="checkbox" class="ws-switch" checked={cross.shopVary} onchange={(e) => (cross = { ...cross, shopVary: e.currentTarget.checked })} /></label>
          <div class="ws-hint">A row of shops down each side, the lane between them as wide as the bridge leaves. Varied, each shop has its own frontage, depth and storeys, with an open landing now and then. One undo takes the bridge and its shops away; changing the bridge lays them out again.</div>
        {/if}
      {/if}
      {#if crossEditing}<button class="ws-btn primary block" onclick={onCrossSave}>Save changes</button>{/if}
    {/if}
  {:else if mode === 'water'}
    {#if waterPicked}
      <div class="editing">
        <span class="grow">Changing <b>{waterPicked.name || 'this water'}</b></span>
        <button class="ws-btn" onclick={onWaterRemove} title="Take it away"><Icon name="trash" size={16} /> Remove</button>
        <button class="ws-btn" onclick={onDone}>Done</button>
      </div>
    {:else}
      <div class="ws-seg" role="radiogroup" aria-label="Shape">
        <button class:on={water.brush} aria-pressed={water.brush} onclick={() => (water = { ...water, brush: true })} title="Paint water along a stroke">Brush</button>
        <button class:on={!water.brush} aria-pressed={!water.brush} onclick={() => (water = { ...water, brush: false })} title="Outline the water">Polygon</button>
      </div>
    {/if}
    {#if waterLine}
      <label class="slider">
        <span>Width</span><output>{waterWidth} ft</output>
        <input
          type="range"
          min={BRUSH_WIDTHS[1]}
          max={BRUSH_WIDTHS[2]}
          step="1"
          value={waterWidth}
          onchange={(e) => (waterPicked ? onWaterChange({ width: +e.currentTarget.value }) : (water = { ...water, width: +e.currentTarget.value }))}
          oninput={(e) => {
            if (!waterPicked) water = { ...water, width: +e.currentTarget.value };
          }}
        />
      </label>
    {/if}
    <label class="slider">
      <span>Depth</span><output>{waterDepth} ft</output>
      <input
        type="range"
        min={DEPTHS[1]}
        max={DEPTHS[2]}
        step="1"
        value={waterDepth}
        onchange={(e) => (waterPicked ? onWaterChange({ depth: +e.currentTarget.value }) : (water = { ...water, depth: +e.currentTarget.value }))}
        oninput={(e) => {
          if (!waterPicked) water = { ...water, depth: +e.currentTarget.value };
        }}
      />
    </label>
    <div class="ws-seg" role="radiogroup" aria-label="Shore">
      <button class:on={!waterDug} aria-pressed={!waterDug} onclick={() => (waterPicked ? onWaterChange({ dug: false }) : (water = { ...water, dug: false }))} title="A natural curve round the points">Natural</button>
      <button class:on={waterDug} aria-pressed={waterDug} onclick={() => (waterPicked ? onWaterChange({ dug: true }) : (water = { ...water, dug: true }))} title="Straight sides, rounded corners, a stone edge">Dug</button>
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to draw it.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    <!-- (On a phone's strip too: its name, and how to draw one, are all there is to it.) -->
    {#if waterPicked}
      <label class="ws-field">
        Name
        <input class="ws-input" value={waterPicked.name} placeholder="No name" onchange={(e) => onWaterChange({ name: e.currentTarget.value })} />
      </label>
    {/if}
    {#if peek}
      {#if !waterPicked}<div class="ws-hint">{water.brush ? 'Drag to paint water; end on its start for a ring.' : 'Tap its points, then the first again, or drag round it.'} Tap water drawn earlier to change it.</div>{/if}
    {:else}
      <div class="ws-hint">
        {#if waterPicked}
          It looks like {LOOKS[waterPicked.look]}. Click outside it to draw more.
        {:else if water.brush}
          Press and drag to paint water along the stroke, or click along a line and press Enter; end on its start for a ring. Click water drawn earlier to change it.
        {:else}
          Click its points, then the first again (or double-click), or drag round it and let go near the start. Water drawn over each other is one water. Click water drawn earlier to change it.
        {/if}
      </div>
      <div class="ws-hint">The look follows the water: at the sea's level it is the sea's (dug: a quay); an open stroke runs downhill as a stream; a ring stays still, as a moat; other water under {POND_BELOW_FT} ft deep is a pond, deeper a lake.</div>
    {/if}
  {:else if mode === 'road'}
    {@const roadKind = roadPicked?.kind ?? road.kind}
    {@const roadW = roadPicked?.width ?? road.width}
    {#if roadPicked}
      <div class="editing">
        <span class="grow">Changing the <b>{roadPicked.name || PATH_KINDS.find((k) => k.key === roadPicked.kind)?.name}</b></span>
        <button class="ws-btn" onclick={onRoadRemove} title="Take it away"><Icon name="trash" size={16} /> Remove</button>
        <button class="ws-btn" onclick={onDone}>Done</button>
      </div>
    {/if}
    <div class="ws-seg" role="radiogroup" aria-label="Road">
      {#each PATH_KINDS as k (k.key)}
        <button class:on={roadKind === k.key} aria-pressed={roadKind === k.key} onclick={() => (roadPicked ? onRoadChange({ kind: k.key }) : (road = { ...road, kind: k.key, width: k.width }))}>{k.label}</button>
      {/each}
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to draw it.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <label class="slider">
        <span>Width</span><output>{roadW} ft</output>
        <input
          type="range"
          min={PATH_WIDTH.min}
          max={PATH_WIDTH.max}
          step="1"
          value={roadW}
          oninput={(e) => {
            if (!roadPicked) road = { ...road, width: Number(e.currentTarget.value) };
          }}
          onchange={(e) => roadPicked && onRoadChange({ width: Number(e.currentTarget.value) })}
        />
      </label>
      <label class="ws-field">
        Name
        {#if roadPicked}
          <input class="ws-input" value={roadPicked.name} placeholder="No name" onchange={(e) => onRoadChange({ name: e.currentTarget.value })} />
        {:else}
          <input class="ws-input" bind:value={road.name} placeholder="No name" />
        {/if}
      </label>
      <div class="ws-hint">
        {roadPicked ? 'Pick a type, width or name for it; click away from it to draw another. ' : 'Click its points, then press Enter or double-click to end it, or press and drag it freehand; click a road drawn earlier to change it. It runs on a smooth curve through the points; trees and the town’s own buildings keep off it. '}{PATH_KINDS.find((k) => k.key === roadKind)?.look}
      </div>
    {:else if !roadPicked}
      <div class="ws-hint">Tap its points, then double-tap to end it, or drag it; tap a road to change it.</div>
    {/if}
  {:else if mode === 'harbour'}
    {#if harbourPicked}
      <div class="editing">
        <span class="grow">Picked the <b>{harbourPicked}</b></span>
        <button class="ws-btn" onclick={onHarbourRemove} title="Take it away"><Icon name="trash" size={16} /> Remove</button>
        <button class="ws-btn" onclick={onDone}>Done</button>
      </div>
    {/if}
    <div class="ws-seg" role="radiogroup" aria-label="Harbour">
      {#each HARBOUR as h (h.key)}
        <button class:on={harbour.kind === h.key} aria-pressed={harbour.kind === h.key} onclick={() => harbourKind(h.key)} title={h.label}>{h.label}</button>
      {/each}
    </div>
    {#if !near}
      <div class="zoom">
        <span class="grow">Zoom in to put one down.</span>
        <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
      </div>
    {/if}
    {#if !peek}
      <div class="ws-hint">{HARBOUR.find((h) => h.key === harbour.kind)?.look}</div>
      {#if harbour.kind === 'islet'}
        <label class="slider">
          <span>Radius</span><output>{harbour.radius} ft</output>
          <input type="range" min={ISLET_RADIUS[0]} max={ISLET_RADIUS[1]} step="5" bind:value={harbour.radius} />
        </label>
        <label class="slider">
          <span>Rise over the water</span><output>{harbour.rise} ft</output>
          <input type="range" min={ISLET_RISE[0]} max={ISLET_RISE[1]} step="1" bind:value={harbour.rise} />
        </label>
      {:else if wharfRange}
        <label class="slider">
          <span>Width</span><output>{harbour.width} ft</output>
          <input type="range" min={wharfRange[0]} max={wharfRange[1]} step="1" bind:value={harbour.width} />
        </label>
        {#if harbour.kind === 'pier'}
          <div class="ws-seg" role="radiogroup" aria-label="Built of">
            <button class:on={harbour.material === 'timber'} aria-pressed={harbour.material === 'timber'} onclick={() => (harbour = { ...harbour, material: 'timber' })}>Timber</button>
            <button class:on={harbour.material === 'stone'} aria-pressed={harbour.material === 'stone'} onclick={() => (harbour = { ...harbour, material: 'stone' })}>Stone</button>
          </div>
        {/if}
        {#if harbour.kind === 'breakwater'}
          <label class="switch"><span class="grow">Lighthouse at its tip</span><input type="checkbox" class="ws-switch" bind:checked={harbour.lighthouse} /></label>
        {/if}
      {/if}
    {/if}
  {:else}
  {#if editing && mode === 'building'}
    <div class="editing">
      <span class="grow">Editing <b>{editing}</b></span>
      <button class="ws-btn" onclick={onDone}>Done</button>
    </div>
  {/if}
  {#if mode === 'stamp'}
    <div class="ws-seg" role="radiogroup" aria-label="Size">
      {#each STAMP_SIZES as z (z.key)}
        <button class:on={stamp.size === z.key} aria-pressed={stamp.size === z.key} onclick={() => (stamp.size = z.key)}>{z.label}</button>
      {/each}
    </div>
  {:else}
  <div class="ws-seg" role="radiogroup" aria-label="Shape">
    {#each SHAPES as s (s.key)}
      <button class:on={settings.shape === s.key} aria-pressed={settings.shape === s.key} onclick={() => shape(s.key)} title="{s.label} ({s.kbd})"><Icon name={s.icon} size={16} />{s.label}</button>
    {/each}
  </div>
  {/if}
  {#if !near}
    <div class="zoom">
      <span class="grow">Zoom in to draw.</span>
      <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
    </div>
  {/if}
  {#if !peek}
    {#if mode === 'stamp'}
      <div class="ws-hint">Click to set one down, fronting the nearest street. The ghost shows where it lands and which way it faces. Each click puts down another.</div>
      <label class="switch">
        <span class="grow">Clear the way<span class="ws-muted"> · the town’s own buildings under it go</span></span>
        <input type="checkbox" class="ws-switch" bind:checked={stamp.clear} />
      </label>
      {@render quickLine('Place it')}
    {:else}
    <div class="ws-hint">{editing ? 'Draw on the map to move or reshape it: ' : ''}{SHAPES.find((s) => s.key === settings.shape)?.hint}</div>
    {/if}
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
      Roof shape
      <div class="tints" role="radiogroup" aria-label="Roof shape">
        <button class="ws-chip" class:on={settings.rshape === ''} role="radio" aria-checked={settings.rshape === ''} onclick={() => (settings.rshape = '')} title="Picked for it">Picked</button>
        {#each ROOF_SHAPES as r (r)}<button class="ws-chip" class:on={settings.rshape === r} role="radio" aria-checked={settings.rshape === r} onclick={() => (settings.rshape = r)}>{label(r)}</button>{/each}
      </div>
    </div>
    <div class="ws-field">
      Roof material
      <div class="tints" role="radiogroup" aria-label="Roof material">
        <button class="ws-chip" class:on={settings.material === ''} role="radio" aria-checked={settings.material === ''} onclick={() => ((settings.material = ''), (settings.shade = ''))} title="Picked for it">Picked</button>
        {#each ROOF_MATERIALS as m, i (m)}
          <button class="swatch" class:on={settings.material === m} role="radio" aria-checked={settings.material === m} style:background={hex(ROOF_MATERIAL_RGB[i])} onclick={() => ((settings.material = m), (settings.shade = ''))} title={label(m)} aria-label={label(m)}></button>
        {/each}
      </div>
    </div>
    <div class="ws-field">
      Roof colour
      <div class="tints" role="radiogroup" aria-label="Roof colour">
        <button class="ws-chip" class:on={settings.shade === ''} role="radio" aria-checked={settings.shade === ''} onclick={() => (settings.shade = '')} title="Picked for it">Picked</button>
        {#if settings.material}
          {#each ROOF_SHADES[ROOF_MATERIALS.indexOf(settings.material)] as c, k (k)}
            <button class="swatch" class:on={settings.shade === k} role="radio" aria-checked={settings.shade === k} style:background={hex(c)} onclick={() => (settings.shade = k)} title={k === 2 ? 'Usual' : `Variant ${k + 1}`} aria-label={k === 2 ? 'Usual colour' : `Variant ${k + 1}`}></button>
          {/each}
        {:else}
          <span class="ws-hint">Pick a material for its colours</span>
        {/if}
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
  .quick {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .quick.bad span {
    color: var(--danger, #dc2626);
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
    font-size: 12px;
    cursor: pointer;
  }
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
    flex-wrap: wrap;
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
  /* Ten modes in two rows of five: each its icon over its word. */
  .modes {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
  }
  .modes > button:nth-child(5n + 1) {
    border-left: none;
  }
  .modes > button:nth-child(n + 6) {
    border-top: 1px solid var(--line-soft);
  }
  .modes > button {
    flex-direction: column;
    gap: 0;
    padding: 4px 2px;
    font-size: 11px;
    min-width: 0;
    padding-inline: 1px;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
</style>
