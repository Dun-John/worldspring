<script lang="ts" module>
  /** A thing the picker offers. */
  export interface PickItem {
    /** What the panel's settings hold: a battlemap kind's id, a kind inside, `s:<asset>`. */
    id: number | string;
    name: string;
    group: string;
    /** Its tile on the thumbnail sheet (none for an uploaded picture: `asset`). */
    thumb?: string;
    /** An uploaded picture (its asset id): yours, drawn from the picture itself. */
    asset?: string;
    /** Its usual size ("2×1 squares"). */
    size: string;
    /** What it does in play ("blocks movement · ¾ cover · 7 ft"). */
    rules: string;
    /** Whether it turns and how many looks it has. */
    note: string;
    /** What it does to those who come near (hazards get a mark). */
    hazard?: string | null;
    /** Searched too (a kind's key). */
    key?: string;
    title?: string;
  }
</script>

<script lang="ts">
  // One picker for whatever is put down by hand: battlemap objects (Edit › Objects) and the
  // furniture and props inside (Edit › Rooms). Pictures in groups or one A–Z list, a search
  // (Enter takes the first), the last twelve picked (per context, kept by this browser), and a
  // strip over the list saying what the one under the pointer (else the one chosen) is. Pictures
  // come from one thumbnail sheet (`render/thumbs.ts`), made the first time the picker is shown.
  import { paintThumb, type ThumbSheet } from '../render/thumbs';
  import Icon from './Icon.svelte';
  import { store, stored } from './shell/layout.svelte';

  interface Props {
    items: PickItem[];
    /** The groups, in the order shown (those with nothing in them are left out). */
    groups: string[];
    chosen: (number | string)[];
    /** Whose Recent this is: battlemap objects, a building's furniture, a site's props. */
    context: 'scatter' | 'building' | 'underground';
    /** The thumbnail sheet (asked for once the picker is shown); null when there is none. */
    thumbs: () => Promise<ThumbSheet> | null | undefined;
    /** An uploaded picture's URL. */
    picture: (asset: string) => Promise<string | null>;
    onPick: (id: number | string) => void;
    /** Uploads: a tile to add one, and the buttons to change or remove one. */
    onUpload?: () => void;
    onEditOwn?: (asset: string) => void;
    onRemoveOwn?: (asset: string) => void;
    /** The uploaded picture whose removal waits for a second click. */
    removing?: string | null;
  }

  let { items, groups, chosen, context, thumbs, picture, onPick, onUpload, onEditOwn, onRemoveOwn, removing = null }: Props = $props();

  const YOURS = 'Yours';
  const RECENT = 'Recent';
  const MAX_RECENT = 12;

  let view = $state<'pictures' | 'az'>(stored('ui.picker.view') === 'az' ? 'az' : 'pictures');
  let filter = $state('All');
  let find = $state('');
  let hover = $state<PickItem | null>(null);
  let recent = $state<(number | string)[]>([]);
  let sheet = $state.raw<ThumbSheet | null>(null);
  let sheetDone = $state(false);

  $effect(() => {
    try {
      const r = JSON.parse(stored(`ui.recent.${context}`) ?? '[]');
      recent = Array.isArray(r) ? r.filter((x) => typeof x === 'number' || typeof x === 'string') : [];
    } catch {
      recent = [];
    }
  });

  $effect(() => {
    let live = true;
    const p = thumbs();
    if (!p) return;
    p.then(
      (s) => live && ((sheet = s), (sheetDone = true)),
      () => live && (sheetDone = true),
    );
    return () => (live = false);
  });

  function setView(v: typeof view) {
    view = v;
    store('ui.picker.view', v);
  }

  const byId = $derived(new Map(items.map((it) => [it.id, it])));
  const needle = $derived(find.trim().toLowerCase());
  const matches = (it: PickItem) => !needle || it.name.toLowerCase().includes(needle) || !!it.key?.toLowerCase().includes(needle) || it.group.toLowerCase().includes(needle);
  const byName = (a: PickItem, b: PickItem) => a.name.localeCompare(b.name);
  const recentItems = $derived(recent.flatMap((id) => byId.get(id) ?? []));
  const own = $derived(items.filter((it) => it.asset));
  const present = $derived(groups.filter((g) => items.some((it) => it.group === g && !it.asset)));
  const chips = $derived(['All', ...(recentItems.length ? [RECENT] : []), ...present, ...(own.length || onUpload ? [YOURS] : [])]);
  // A filter that has gone (the last upload removed): back to all.
  $effect(() => {
    if (!chips.includes(filter)) filter = 'All';
  });

  /** What is shown, as groups (a header each only under All). */
  const sections = $derived.by((): [string, PickItem[]][] => {
    if (filter === RECENT) return [[RECENT, recentItems.filter(matches)]];
    const mine = own.filter(matches).sort(byName);
    if (filter === YOURS) return [[YOURS, mine]];
    const named = (g: string) => items.filter((it) => !it.asset && it.group === g && matches(it)).sort(byName);
    if (filter !== 'All') return [[filter, named(filter)]];
    return [...present.map((g): [string, PickItem[]] => [g, named(g)]), [YOURS, mine] as [string, PickItem[]]].filter(([g, list]) => list.length || (g === YOURS && onUpload && !needle));
  });
  /** The same, in one list: A–Z (Recent as it came). */
  const flat = $derived(filter === RECENT ? sections[0][1] : sections.flatMap(([, list]) => list).sort(byName));
  const shown = $derived(view === 'az' ? flat : sections.flatMap(([, list]) => list));
  const uploadShown = $derived(!!onUpload && !needle && (filter === 'All' || filter === YOURS));
  const lastChosen = $derived(chosen.length ? (byId.get(chosen[chosen.length - 1]) ?? null) : null);
  const detail = $derived(hover ?? lastChosen);

  function pick(it: PickItem) {
    recent = [it.id, ...recent.filter((x) => x !== it.id)].slice(0, MAX_RECENT);
    store(`ui.recent.${context}`, JSON.stringify(recent));
    onPick(it.id);
  }

  function keydown(e: KeyboardEvent) {
    if (e.key !== 'Enter' || !shown.length) return;
    e.preventDefault();
    pick(shown[0]);
    // (The keys go back to the map: T turns what was picked, not the search.)
    (e.currentTarget as HTMLInputElement).blur();
  }

  const dpr = typeof devicePixelRatio === 'number' ? Math.min(2, Math.max(1, devicePixelRatio)) : 1;

  /** Paints a tile from the sheet into a `<canvas>`. */
  function thumb(node: HTMLCanvasElement, p: { sheet: ThumbSheet | null; key: string | undefined }) {
    const paint = (q: typeof p) => {
      const ctx = node.getContext('2d');
      if (q.sheet && q.key) paintThumb(node, q.sheet, q.key);
      else ctx?.clearRect(0, 0, node.width, node.height);
    };
    paint(p);
    return { update: paint };
  }
</script>

{#snippet pic(it: PickItem, px: number)}
  {#if it.asset}
    {#await picture(it.asset) then src}{#if src}<img {src} alt="" style:width="{px}px" style:height="{px}px" />{/if}{/await}
  {:else if sheetDone && !(it.thumb && sheet?.rects.has(it.thumb))}
    <span class="none" style:width="{px}px" style:height="{px}px"><Icon name="shapes" size={Math.round(px * 0.6)} /></span>
  {:else}
    <canvas width={px * dpr} height={px * dpr} style:width="{px}px" style:height="{px}px" use:thumb={{ sheet, key: it.thumb }}></canvas>
  {/if}
{/snippet}

{#snippet ownButtons(it: PickItem)}
  {#if it.asset && onEditOwn}
    <button class="tiny edit" title="Change its rules" aria-label="Change {it.name}" onclick={() => onEditOwn?.(it.asset!)}><Icon name="pencil" size={12} /></button>
  {/if}
  {#if it.asset && onRemoveOwn}
    <button class="tiny remove" class:warn={removing === it.asset} title={removing === it.asset ? 'Click again to remove it and every one put down' : 'Remove it'} aria-label="Remove {it.name}" onclick={() => onRemoveOwn?.(it.asset!)}><Icon name="x" size={12} /></button>
  {/if}
{/snippet}

<div class="picker">
  <div class="top">
    <input class="ws-input grow" type="search" placeholder="Find…" bind:value={find} onkeydown={keydown} aria-label="Find by name or group (Enter takes the first)" />
    <div class="ws-seg" role="radiogroup" aria-label="Show as">
      <button class:on={view === 'pictures'} aria-pressed={view === 'pictures'} title="Pictures in groups" aria-label="Pictures" onclick={() => setView('pictures')}><Icon name="grid" size={16} /></button>
      <button class:on={view === 'az'} aria-pressed={view === 'az'} title="One list, A–Z" aria-label="A to Z" onclick={() => setView('az')}><Icon name="list" size={16} /></button>
    </div>
  </div>
  <div class="chips">
    {#each chips as g (g)}
      <button class="ws-chip" class:on={filter === g} aria-pressed={filter === g} onclick={() => (filter = g)}>{g}</button>
    {/each}
  </div>

  <div class="detail" aria-live="polite">
    {#if detail}
      {@render pic(detail, 64)}
      <div class="what" title={[detail.name, detail.size, detail.note, detail.rules, detail.hazard].filter(Boolean).join(' · ')}>
        <div><b>{detail.name}</b> <span class="ws-muted">· {detail.group}</span></div>
        <div>{detail.size} · {detail.note}</div>
        {#if detail.rules}<div class="ws-muted">{detail.rules}</div>{/if}
        {#if detail.hazard}<div class="hz-text">⚠ {detail.hazard}</div>{/if}
      </div>
    {:else}
      <span class="ws-hint">Choose one to see what it is.</span>
    {/if}
  </div>

  <div class="scroll" role="presentation" onpointerleave={() => (hover = null)}>
    {#if view === 'pictures'}
      {#each sections as [g, list] (g)}
        {#if filter === 'All'}<div class="head">{g}</div>{/if}
        <div class="grid">
          {#each list as it (it.id)}
            <span class="cell">
              <button
                class="tile"
                class:on={chosen.includes(it.id)}
                aria-pressed={chosen.includes(it.id)}
                title={it.title ?? it.name}
                onclick={() => pick(it)}
                onpointerenter={(e) => e.pointerType === 'mouse' && (hover = it)}
              >
                {@render pic(it, 40)}
                <span class="name">{it.name}</span>
                {#if it.hazard}<span class="hz" aria-label="Hazard">⚠</span>{/if}
              </button>
              {@render ownButtons(it)}
            </span>
          {/each}
          {#if g === YOURS && uploadShown}
            <button class="tile upload" onclick={onUpload} title="Upload a picture as a new kind of object"><Icon name="upload" size={24} /><span class="name">Upload…</span></button>
          {/if}
        </div>
      {/each}
    {:else}
      <div class="rows">
        {#each flat as it (it.id)}
          <span class="cell">
            <button class="row" class:on={chosen.includes(it.id)} aria-pressed={chosen.includes(it.id)} title={it.title ?? it.name} onclick={() => pick(it)} onpointerenter={(e) => e.pointerType === 'mouse' && (hover = it)}>
              {@render pic(it, 24)}
              <span class="name">{it.name}{#if it.hazard}<span class="hz-inline" aria-label="Hazard"> ⚠</span>{/if}</span>
              <span class="grp">{it.group}</span>
            </button>
            {@render ownButtons(it)}
          </span>
        {/each}
        {#if uploadShown}
          <button class="row upload" onclick={onUpload}><Icon name="upload" size={20} /><span class="name">Upload a picture…</span></button>
        {/if}
      </div>
    {/if}
    {#if !shown.length && !uploadShown}<div class="ws-hint empty">{needle ? 'Nothing by that name.' : filter === YOURS ? 'No pictures yet: upload them in Edit › Objects.' : 'Nothing here.'}</div>{/if}
  </div>

</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .top {
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .scroll {
    max-height: min(340px, 45vh);
    overflow-y: auto;
    overscroll-behavior: contain;
    border: 1px solid var(--line-faint);
    border-radius: var(--radius-sm);
    padding: 0 4px 4px;
  }
  .head {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 4px 2px 2px;
    background: var(--paper-solid);
    color: var(--ink-2);
    font-size: 11px;
    font-weight: bold;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(70px, 1fr));
    gap: 4px;
    padding-top: 4px;
  }
  .cell {
    position: relative;
    display: flex;
    min-width: 0;
  }
  .tile {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    min-height: 66px;
    padding: 3px 2px;
    background: var(--btn);
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    cursor: pointer;
    font: 11px/1.15 var(--font);
    color: var(--ink);
  }
  .tile:hover,
  .row:hover {
    background: var(--btn-hover);
  }
  .tile.on,
  .row.on {
    border-color: var(--line);
    background: var(--btn-on);
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hz {
    position: absolute;
    top: 1px;
    right: 3px;
    color: var(--warn);
    font-size: 12px;
  }
  .hz-inline,
  .hz-text {
    color: var(--warn);
  }
  .upload {
    justify-content: center;
    gap: 4px;
    border-style: dashed;
    color: var(--ink-2);
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-top: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    min-height: var(--tap);
    padding: 1px 6px 1px 3px;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font: 12px/1.2 var(--font);
    color: var(--ink);
    text-align: left;
  }
  .row .name {
    flex: 1;
    min-width: 0;
  }
  .row.upload {
    border: 1px dashed var(--line-soft);
  }
  .grp {
    flex: none;
    color: var(--ink-3);
    font-size: 11px;
  }
  .rows .cell .tiny.edit {
    right: 26px;
    left: auto;
  }
  .tiny {
    position: absolute;
    top: 2px;
    display: inline-flex;
    padding: 2px;
    background: var(--btn);
    border: 1px solid var(--line-soft);
    border-radius: 3px;
    cursor: pointer;
    color: var(--ink);
  }
  .rows .tiny {
    top: 50%;
    transform: translateY(-50%);
  }
  .tiny.edit {
    left: 2px;
  }
  .tiny.remove {
    right: 2px;
  }
  .tiny.warn {
    background: var(--danger);
    color: #fff;
  }
  .rows .cell:has(.tiny) .grp {
    margin-right: 46px;
  }
  canvas,
  img {
    flex: none;
    object-fit: contain;
  }
  .none {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: center;
    color: var(--ink-3);
    opacity: 0.6;
  }
  .empty {
    padding: 8px 2px 4px;
  }
  .detail {
    display: flex;
    gap: 8px;
    align-items: center;
    height: 72px;
    overflow: hidden;
    font-size: 12px;
    line-height: 1.35;
  }
  .detail .what {
    min-width: 0;
  }
  .detail .what > div {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
