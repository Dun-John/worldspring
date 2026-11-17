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
  // Edit › Build: the modes' strip, and the panel of the mode open (`ui/build/`: one per mode,
  // each in the shared `ToolPanel` frame). The tools themselves are `editor/tools/build.svelte.ts`.
  import type { BuildSettings } from '../editor/build';
  import type { CrossSettings } from '../editor/crossing';
  import type { WaterSettings } from '../editor/water';
  import type { PathSettings } from '../editor/paths';
  import type { HarbourSettings } from '../editor/harbour';
  import type { ClearShape } from '../editor/clearArea';
  import type { LassoSettings, RowSettings, StampSettings } from '../editor/quick';
  import type { WorksSettings } from '../editor/works';
  import type { BuildingFuncs, PathKind, WaterKind } from '../gen/protocol';
  import Icon from './Icon.svelte';
  import BuildingPanel from './build/BuildingPanel.svelte';
  import ClearPanel from './build/ClearPanel.svelte';
  import CrossingPanel from './build/CrossingPanel.svelte';
  import HarbourPanel from './build/HarbourPanel.svelte';
  import PlotPanel from './build/PlotPanel.svelte';
  import RoadPanel from './build/RoadPanel.svelte';
  import RowPanel from './build/RowPanel.svelte';
  import StampPanel from './build/StampPanel.svelte';
  import WaterPanel from './build/WaterPanel.svelte';
  import WorksPanel from './build/WorksPanel.svelte';
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
    <WorksPanel {mode} bind:works {worksEditing} {handTown} {onWorksSave} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'clear'}
    <ClearPanel bind:clearShape {near} {peek} {onZoomIn} />
  {:else if mode === 'row'}
    <RowPanel bind:row bind:stamp {funcs} {quick} {onPlace} {onDrop} {near} {peek} {onZoomIn} />
  {:else if mode === 'lasso'}
    <PlotPanel bind:lasso {quick} {onPlace} {onDrop} {near} {peek} {onZoomIn} />
  {:else if mode === 'crossing'}
    <CrossingPanel bind:cross {crossEditing} {onCrossSave} {onCrossRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'water'}
    <WaterPanel bind:water {waterPicked} {onWaterChange} {onWaterRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'road'}
    <RoadPanel bind:road {roadPicked} {onRoadChange} {onRoadRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'harbour'}
    <HarbourPanel bind:harbour {harbourPicked} {onHarbourRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'stamp'}
    <StampPanel bind:stamp bind:settings {funcs} {editing} {quick} {onPlace} {onSave} {near} {peek} {onZoomIn} />
  {:else}
    <BuildingPanel bind:settings {funcs} {editing} {onSave} {onDone} {near} {peek} {onZoomIn} />
  {/if}
</div>

<style>
  .build :global(.slider) {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    font-size: 12px;
  }
  .build :global(.slider) :global(output) {
    font: 11px var(--mono);
    color: var(--ink-2);
  }
  .build :global(.slider) :global(input) {
    grid-column: 1 / 3;
    width: 100%;
    margin: 0;
  }
  .build :global(.switch) {
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
  .build :global(.grow) {
    flex: 1;
    min-width: 0;
  }
  .build :global(.stepper) {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 140px;
    border: 1px solid var(--line-field);
    border-radius: var(--radius-sm);
    background: var(--field);
  }
  .build :global(.stepper) :global(span) {
    flex: 1;
    text-align: center;
    color: var(--ink);
    font-size: 13px;
  }
  .build :global(.tints) {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .build :global(.swatch) {
    width: 24px;
    height: 24px;
    border: 1px solid var(--line);
    border-radius: 50%;
    cursor: pointer;
    padding: 0;
  }
  .build :global(.swatch.on) {
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
  .build :global(.switch) {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
</style>
