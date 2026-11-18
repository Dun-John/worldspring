<script lang="ts" module>
  export type BuildMode = 'select' | 'building' | 'stamp' | 'row' | 'lasso' | 'castle' | 'wall' | 'crossing' | 'water' | 'harbour' | 'road' | 'fence';
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
  import type { FenceSettings } from '../editor/fence';
  import type { LassoSettings, RowSettings, StampSettings } from '../editor/quick';
  import type { WorksSettings } from '../editor/works';
  import type { BuildingFuncs, PathKind, WaterKind } from '../gen/protocol';
  import BuildingPanel from './build/BuildingPanel.svelte';
  import FencePanel from './build/FencePanel.svelte';
  import CrossingPanel from './build/CrossingPanel.svelte';
  import HarbourPanel from './build/HarbourPanel.svelte';
  import PlotPanel from './build/PlotPanel.svelte';
  import RoadPanel from './build/RoadPanel.svelte';
  import RowPanel from './build/RowPanel.svelte';
  import StampPanel from './build/StampPanel.svelte';
  import WaterPanel from './build/WaterPanel.svelte';
  import WorksPanel from './build/WorksPanel.svelte';

  interface Props {
    settings: BuildSettings;
    funcs: BuildingFuncs | null;
    /** The building being changed (its name), else drawing new ones. */
    editing: string | null;
    /** The tool open (Edit's tool row picks it). */
    mode: BuildMode;
    works: WorksSettings;
    /** The castle or wall being changed (its name), else drawing new ones. */
    worksEditing: string | null;
    /** The view is over a town laid out by hand. */
    handTown?: boolean;
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
    fence: FenceSettings;
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
    /** Select's card (`SelectPanel`), shown in its mode. */
    select?: import('svelte').Snippet;
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
    /** The plot being changed (its name), from Select's Change. */
    plotEditing?: string | null;
    onPlotRemove?: () => void;
    /** The world's own islets, listed in Docks & islets. */
    islets?: import('./IsletList.svelte').Islet[];
    onIslet?: (id: string, removed: boolean) => void;
    onGoIslet?: (x: number, y: number) => void;
  }

  let {
    settings = $bindable(),
    funcs,
    editing,
    handTown = false,
    mode,
    select,
    works = $bindable(),
    worksEditing,
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
    fence = $bindable(),
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
    plotEditing = null,
    onPlotRemove,
    islets = [],
    onIslet,
    onGoIslet,
  }: Props = $props();

</script>

<div class="build">
  {#if mode === 'select'}
    {@render select?.()}
  {:else if mode === 'castle' || mode === 'wall'}
    <WorksPanel {mode} bind:works {worksEditing} {handTown} {onWorksSave} {onDone} {near} {peek} {onZoomIn} {onMode} />
  {:else if mode === 'row'}
    <RowPanel bind:row bind:stamp {funcs} {quick} {onPlace} {onDrop} {near} {peek} {onZoomIn} />
  {:else if mode === 'lasso'}
    <PlotPanel bind:lasso {quick} {onPlace} {onDrop} {near} {peek} {onZoomIn} editing={plotEditing} onRemove={onPlotRemove} {onDone} />
  {:else if mode === 'crossing'}
    <CrossingPanel bind:cross {crossEditing} {onCrossSave} {onCrossRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'water'}
    <WaterPanel bind:water {waterPicked} {onWaterChange} {onWaterRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'road'}
    <RoadPanel bind:road {roadPicked} {onRoadChange} {onRoadRemove} {onDone} {near} {peek} {onZoomIn} />
  {:else if mode === 'fence'}
    <FencePanel bind:fence {near} {peek} {onZoomIn} />
  {:else if mode === 'harbour'}
    <HarbourPanel bind:harbour {harbourPicked} {onHarbourRemove} {onDone} {near} {peek} {onZoomIn} {islets} {onIslet} {onGoIslet} />
  {:else if mode === 'stamp'}
    <StampPanel bind:stamp bind:settings {funcs} {editing} {quick} {onPlace} {onSave} {near} {peek} {onZoomIn} />
  {:else}
    <BuildingPanel bind:settings {funcs} {editing} {onSave} {onDone} {near} {peek} {onZoomIn} />
  {/if}
</div>

<style>  .build :global(.switch) {
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
  .build :global(.switch) {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
</style>
