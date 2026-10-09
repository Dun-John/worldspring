<script lang="ts">
  // World › Sketch: the tools in one strip (all that shows when the panel is folded down),
  // the chosen tool's options, the preview, what the world can't follow, and Generate.
  import type { Conflict, PinKind, RoadKind, SiteKind, VolcanoActivity, VolcanoKind } from '../gen/protocol';
  import { PAINT_BIOMES } from '../world/world';
  import Icon from '../ui/Icon.svelte';
  import type { IconName } from '../ui/icons';
  import type { EditTool, PinTier, Sketcher, ToolSettings } from './sketcher';

  interface Props {
    sketcher: Sketcher;
    /** The tool settings (App's copy, which the sketcher follows; shortcuts change it too). */
    settings: ToolSettings;
    /** Bumped when the strokes change. */
    version: number;
    conflicts: Conflict[];
    previewStatus: string;
    showPreview: boolean;
    busy: boolean;
    /** Only the tool strip (the panel is folded down). */
    peek?: boolean;
    onPreview: (on: boolean) => void;
    onConflict: (c: Conflict) => void;
    onGenerate: () => void;
    /** Leave sketch mode (new strokes are asked about). */
    onStop: () => void;
  }
  let { sketcher, settings = $bindable(), version, conflicts, previewStatus, showPreview, busy, peek = false, onPreview, onConflict, onGenerate, onStop }: Props = $props();

  const count = $derived((void version, sketcher.strokes.length));
  /** A sketch holds at most this many points (`world.rs` `SKETCH_POINTS`). */
  const POINTS = 200_000;
  const points = $derived((void version, sketcher.strokes.reduce((n, s) => n + s.pts.length, 0)));
  const canUndo = $derived((void version, sketcher.canUndo));
  let clearing = $state(false);

  const TOOLS: [EditTool, IconName, string, string, string][] = [
    ['coast', 'coast', 'Coastline', 'C', 'Draw round a landmass: it becomes land, and undrawn map is sea.'],
    ['land', 'land', 'Land', 'L', 'Brush on land: islands, peninsulas, land bridges.'],
    ['sea', 'waves', 'Sea', 'S', 'Brush on sea: bays, straits, inland seas.'],
    ['range', 'mountain', 'Range', 'R', 'Draw along a mountain ridge.'],
    ['massif', 'massif', 'Massif', 'M', 'Draw round a mountain mass: it rises from its edge to high ground in its middle.'],
    ['elevation', 'plateau', 'Elevation', 'H', 'Draw round land to raise it (a plateau) or lower it (a basin). Rivers keep their courses.'],
    ['river', 'river', 'River', 'W', 'Draw from the source to the sea, a lake or another river: it runs the way you draw it.'],
    ['lake', 'lake', 'Lake', 'K', 'Draw round a lake. It drains from its lowest shore, or into a river drawn out of it.'],
    ['biome', 'leaf', 'Biome', 'B', 'Paint a biome over the land.'],
    ['volcano', 'volcano', 'Volcano', 'V', 'Click to place a volcano. The Volcanoes setting adds more: set it to None for only yours.'],
    ['pin', 'castle', 'Settlement', 'T', 'Click to place a settlement of the chosen size.'],
    ['site', 'ruin', 'Site', 'D', 'Click to place a ruin, tower, camp, inn, cave, mine or way underground.'],
    ['region', 'tag', 'Name', 'N', 'Type a name, then click what it names: a region, mountains, a lake, an island or a sea (names in one region share it out). Or draw round a region of your own.'],
    ['road', 'road', 'Road', 'O', 'Draw a road along its way: it follows your line, round water, and joins the settlements at its ends and beside it. No road: draw across the planned roads you don’t want. To have only the roads you draw, set Roads to Only drawn in World › Generate.'],
    ['erase', 'eraser', 'Erase', 'E', 'Click a stroke or settlement to remove it.'],
  ];
  const PIN_KINDS: [PinKind | '', string][] = [
    ['', 'From its land'],
    ['port', 'Port'],
    ['river', 'River town'],
    ['market', 'Market town'],
    ['fortress', 'Fortress'],
    ['mining', 'Mining town'],
    ['farming', 'Farming'],
    ['fishing', 'Fishing'],
    ['lumber', 'Lumber'],
    ['herding', 'Herding'],
    ['oasis', 'Oasis'],
  ];
  const SITES: [SiteKind, string][] = [
    ['ruin', 'Ruin'],
    ['tower', 'Tower'],
    ['camp', 'Camp'],
    ['waystation', 'Inn'],
    ['cave', 'Cave'],
    ['mine', 'Mine'],
    ['lava_tube', 'Lava tube'],
    ['entrance', 'Way down'],
  ];
  /** What lies beneath a ruin (built sites) or a way down (any site). */
  const UNDER: [string, string][] = [
    ['dungeon', 'Dungeon'],
    ['crypt', 'Crypt'],
    ['catacombs', 'Catacombs'],
    ['cave', 'Caves'],
    ['mine', 'Mine'],
    ['lava_tube', 'Lava tubes'],
  ];
  /** What a name names (`world.rs` `REGION_KINDS`); an outline of a region kind is that region. */
  const REGION_KINDS: [string, string][] = [
    ['', 'Whatever is there'],
    ['plains', 'Plains'],
    ['forest', 'Forest'],
    ['jungle', 'Jungle'],
    ['taiga', 'Taiga'],
    ['desert', 'Desert'],
    ['swamp', 'Swamp'],
    ['tundra', 'Tundra'],
    ['glacier', 'Glacier'],
    ['blight', 'Blighted woods'],
    ['ashlands', 'Ashlands'],
    ['region', 'Region'],
    ['range', 'Mountains'],
    ['lake', 'Lake'],
    ['river', 'River'],
    ['island', 'Island'],
    ['continent', 'Continent'],
    ['bay', 'Bay'],
    ['sea', 'Sea'],
    ['ocean', 'Ocean'],
  ];
  const ROADS: [RoadKind, string][] = [
    ['kings_road', "King's road"],
    ['road', 'Road'],
    ['track', 'Track'],
    ['none', 'No road'],
  ];
  const TIERS: [PinTier, string][] = [
    ['metropolis', 'Metropolis'],
    ['city', 'City'],
    ['town', 'Town'],
    ['village', 'Village'],
  ];
  const VOLCANOES: [VolcanoKind, string][] = [
    ['strato', 'Cone'],
    ['shield', 'Shield'],
    ['cinder', 'Cinder'],
    ['caldera', 'Caldera'],
  ];
  const ACTIVITY: [VolcanoActivity, string][] = [
    ['active', 'Active'],
    ['dormant', 'Dormant'],
    ['extinct', 'Extinct'],
  ];
  const tool = $derived(TOOLS.find((t) => t[0] === settings.tool)!);
  const brushKey = $derived(['coast', 'lake', 'volcano', 'pin', 'site', 'region', 'road', 'erase'].includes(settings.tool) ? null : settings.tool);
  const brushLabel = $derived(({ river: 'Valley', range: 'Width', massif: 'Foothills', elevation: 'Edge' } as Record<string, string>)[settings.tool] ?? 'Brush');
  const hasEdges = $derived(['coast', 'land', 'sea', 'biome', 'elevation'].includes(settings.tool));
  const hasStrength = $derived(['range', 'massif', 'river', 'volcano'].includes(settings.tool));
  const strength = $derived.by(() => {
    const [a, b, c] = settings.tool === 'river' ? ['Stream', 'River', 'Great river'] : settings.tool === 'volcano' ? ['Small', 'Middling', 'Great'] : ['Hills', 'Mountains', 'High peaks'];
    return settings.strength < 0.35 ? a : settings.strength < 0.75 ? b : c;
  });
  const NAMED: Record<string, string> = {
    pin: 'Settlement name',
    lake: 'Lake name',
    volcano: 'Volcano name',
    coast: 'Landmass name',
    land: 'Land name',
    sea: 'Sea name',
    range: 'Range name',
    massif: 'Mountains name',
    river: 'River name',
    biome: 'Region name',
    site: 'Site name',
    road: 'Road name',
  };
  const feet = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toLocaleString()} ft`;

  function clear() {
    if (!clearing) {
      clearing = true;
      setTimeout(() => (clearing = false), 3000);
      return;
    }
    clearing = false;
    sketcher.clear();
  }
</script>

<div class="sketch">
  <div class="tools" role="toolbar" aria-label="Sketch tools">
    {#each TOOLS as [id, icon, label, key] (id)}
      <button class="ws-icon-btn" aria-pressed={settings.tool === id} title="{label} ({key})" aria-label={label} onclick={() => (settings.tool = id)}><Icon name={icon} /></button>
    {/each}
  </div>
  {#if !peek}
    <div class="what"><b>{tool[2]}</b> <span class="ws-hint">{tool[4]}</span></div>

    {#if settings.tool === 'elevation'}
      <label class="slider">
        <span>Raise by</span>
        <output>{feet(settings.delta)}</output>
        <input type="range" min="-3000" max="5000" step="100" bind:value={settings.delta} />
      </label>
    {/if}
    {#if brushKey && !(settings.tool === 'biome' && settings.fill)}
      <label class="slider">
        <span>{brushLabel}</span>
        <output>{settings.radiusMi[brushKey]} mi</output>
        <input type="range" min={settings.tool === 'river' ? 1 : 2} max={settings.tool === 'river' ? 8 : settings.tool === 'massif' || settings.tool === 'elevation' ? 40 : 80} step="1" bind:value={settings.radiusMi[brushKey]} />
      </label>
    {/if}
    {#if hasStrength}
      <label class="slider">
        <span>{settings.tool === 'river' || settings.tool === 'volcano' ? 'Size' : 'Height'}</span>
        <output>{strength}</output>
        <input type="range" min="0.1" max="1" step="0.05" bind:value={settings.strength} />
      </label>
    {/if}
    {#if settings.tool === 'volcano'}
      <div class="ws-seg" role="radiogroup" aria-label="Kind of volcano">
        {#each VOLCANOES as [k, label] (k)}<button class:on={settings.volcano === k} onclick={() => (settings.volcano = k)}>{label}</button>{/each}
      </div>
      <div class="ws-seg" role="radiogroup" aria-label="Activity">
        {#each ACTIVITY as [a, label] (a)}<button class:on={settings.activity === a} onclick={() => (settings.activity = a)}>{label}</button>{/each}
      </div>
    {/if}
    {#if settings.tool === 'lake'}
      <div class="ws-field">
        Water level
        <div class="ws-seg" role="radiogroup" aria-label="Water level">
          <button class:on={settings.level === null} onclick={() => (settings.level = null)}>Natural</button>
          <button class:on={settings.level !== null} onclick={() => (settings.level ??= 500)}>Set</button>
        </div>
      </div>
      {#if settings.level !== null}
        <label class="ws-field">
          Feet above sea level
          <input class="ws-input" type="number" step="50" bind:value={settings.level} />
        </label>
      {/if}
      <label class="switch"><span class="grow">Salt lake (no outflow)</span><input type="checkbox" class="ws-switch" bind:checked={settings.salt} /></label>
    {/if}
    {#if settings.tool === 'biome'}
      <label class="ws-field">
        Biome
        <select class="ws-input" bind:value={settings.biome}>
          {#each PAINT_BIOMES as [id, label] (id)}<option value={id}>{label}</option>{/each}
        </select>
      </label>
      <div class="ws-seg" role="radiogroup" aria-label="How it paints">
        <button class:on={!settings.fill} onclick={() => (settings.fill = false)}>Brush</button>
        <button class:on={settings.fill} onclick={() => (settings.fill = true)}>Fill outline</button>
      </div>
    {/if}
    {#if hasEdges}
      <div class="ws-field">
        Edges
        <div class="ws-seg" role="radiogroup" aria-label="Edges">
          <button class:on={!settings.hard} onclick={() => (settings.hard = false)}>Natural</button>
          <button class:on={settings.hard} onclick={() => (settings.hard = true)}>Exact</button>
        </div>
      </div>
    {/if}
    {#if settings.tool === 'pin'}
      <div class="ws-seg" role="radiogroup" aria-label="Settlement size">
        {#each TIERS as [t, label] (t)}<button class:on={settings.tier === t} onclick={() => (settings.tier = t)}>{label}</button>{/each}
      </div>
      <label class="ws-field">
        Lives by
        <select class="ws-input" bind:value={settings.pinKind}>
          {#each PIN_KINDS as [k, label] (k)}<option value={k}>{label}</option>{/each}
        </select>
      </label>
      <label class="switch"><span class="grow">The realm's capital</span><input type="checkbox" class="ws-switch" bind:checked={settings.capital} /></label>
      <input class="ws-input" placeholder="District names, central first (optional)" bind:value={settings.wards} aria-label="District names, separated by commas" />
    {/if}
    {#if settings.tool === 'site'}
      <label class="ws-field">
        Kind
        <select class="ws-input" bind:value={settings.site}>
          {#each SITES as [k, label] (k)}<option value={k}>{label}</option>{/each}
        </select>
      </label>
      {#if settings.site === 'ruin' || settings.site === 'entrance'}
        <label class="ws-field">
          Beneath
          <select class="ws-input" bind:value={settings.under}>
            <option value="">{settings.site === 'ruin' ? 'Dungeon or crypt' : 'Dungeon'}</option>
            {#each UNDER.slice(0, settings.site === 'ruin' ? 3 : UNDER.length) as [k, label] (k)}<option value={k}>{label}</option>{/each}
          </select>
        </label>
      {/if}
    {/if}
    {#if settings.tool === 'region'}
      <input class="ws-input" placeholder="Name" bind:value={settings.name} aria-label="Name" />
      <div class="ws-seg" role="radiogroup" aria-label="How it names">
        <button class:on={!settings.regionOutline} onclick={() => (settings.regionOutline = false)}>Click</button>
        <button class:on={settings.regionOutline} onclick={() => (settings.regionOutline = true)}>Draw round</button>
      </div>
      <label class="ws-field">
        {settings.regionOutline ? 'A region of' : 'Names'}
        <select class="ws-input" bind:value={settings.regionKind}>
          {#each REGION_KINDS as [k, label] (k)}<option value={k}>{k === '' && settings.regionOutline ? 'Its land' : label}</option>{/each}
        </select>
      </label>
    {/if}
    {#if settings.tool === 'road'}
      <div class="ws-seg" role="radiogroup" aria-label="Kind of road">
        {#each ROADS as [k, label] (k)}<button class:on={settings.road === k} onclick={() => (settings.road = k)}>{label}</button>{/each}
      </div>
    {/if}
    {#if NAMED[settings.tool] && !(settings.tool === 'road' && settings.road === 'none')}
      <input class="ws-input" placeholder="Name (optional)" bind:value={settings.name} aria-label={NAMED[settings.tool]} />
    {/if}

    <div class="ws-row bar">
      <button class="ws-icon-btn" onclick={() => sketcher.undo()} disabled={!canUndo} title="Undo (Ctrl+Z)" aria-label="Undo"><Icon name="undo" /></button>
      <button class="ws-btn" class:danger={clearing} onclick={clear} disabled={count === 0}>{clearing ? 'Clear all?' : 'Clear'}</button>
      <button class="ws-icon-btn" aria-pressed={showPreview} onclick={() => onPreview(!showPreview)} title={showPreview ? 'Hide the preview' : 'Show the preview'} aria-label="Preview"><Icon name={showPreview ? 'eye' : 'eye-off'} /></button>
      <span class="ws-muted grow">{count} {count === 1 ? 'stroke' : 'strokes'}{#if points > POINTS * 0.75}<span class:over={points > POINTS}> · {points.toLocaleString()} of {POINTS.toLocaleString()} points</span>{/if}</span>
    </div>
    <div class="ws-muted">{previewStatus}</div>

    {#if conflicts.length}
      <details class="ws-group conflicts">
        <summary>Can't follow exactly ({conflicts.length})</summary>
        <div class="ws-group-body">
          {#each conflicts as c, i (i)}
            <button class="conflict" onclick={() => onConflict(c)}>{c.message}</button>
          {/each}
        </div>
      </details>
    {/if}
  {/if}

  <div class="foot">
    {#if !peek}<button class="ws-btn" onclick={onStop} title="Leave sketch mode (new strokes are asked about first)">Stop sketching</button>{/if}
    <button class="ws-btn primary grow" onclick={onGenerate} disabled={busy}>Generate from sketch</button>
  </div>
  {#if !peek}<div class="ws-muted note">Uses the Generate settings (the sketch stretches with the map's size); names and notes are kept.</div>{/if}
</div>

<style>
  .sketch {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .tools {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
  }
  .tools .ws-icon-btn {
    width: auto;
  }
  .what {
    line-height: 1.35;
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
  .bar {
    border-top: 1px solid var(--line-faint);
    padding-top: 6px;
  }
  .grow {
    flex: 1;
  }
  .over {
    color: #8a2a1a;
  }
  .conflicts > summary {
    color: #8a2a1a;
  }
  .conflict {
    text-align: left;
    font: 12px var(--font);
    color: var(--ink);
    background: #f1e2c8;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    padding: 4px 6px;
    cursor: pointer;
  }
  .foot {
    display: flex;
    gap: 6px;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .note {
    margin-top: -4px;
  }
</style>
