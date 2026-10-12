<script lang="ts">
  // Edit › Objects: put battlemap objects down (a stamp, or a brush scattering the chosen
  // kinds), take them away (click or brush), and upload pictures as new kinds of object with
  // their tactical rules. The tool works on the map while the tab is open.
  import type { KindInfo } from '../gen/client';
  import type { SpriteMeta } from '../gen/protocol';
  import type { ScatterMode, ScatterSettings } from '../editor/scatter';
  import { MAX_PICTURE_BYTES } from '../world/assets';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import { remember, remembered } from './shell/layout.svelte';

  interface Props {
    settings: ScatterSettings;
    catalog: KindInfo[];
    sprites: Record<string, SpriteMeta>;
    /** How many of each uploaded sprite are put down. */
    placed: Record<string, number>;
    /** A built-in kind's picture (data URL). */
    icon: (kind: number) => Promise<string | null>;
    /** An uploaded sprite's picture (object URL). */
    picture: (asset: string) => Promise<string | null>;
    /** Only the modes and the chosen kinds (the panel is folded down). */
    peek?: boolean;
    onUpload: (file: File, meta: SpriteMeta) => Promise<void>;
    /** Change an uploaded sprite's rules, or (null) remove it and every one put down. */
    onSprite: (asset: string, meta: SpriteMeta | null) => void;
  }

  let { settings = $bindable(), catalog, sprites, placed, icon, picture, peek = false, onUpload, onSprite }: Props = $props();

  const GROUPS: [string, number[]][] = [
    ['Trees', [1, 2, 3, 4, 5, 6, 7]],
    ['Plants', [8, 9, 10, 11, 15, 16, 17, 18, 19]],
    ['Rocks', [12, 13, 14, 20, 21, 22, 23, 24]],
    ['Hazards', [25, 26, 27, 28, 29, 30, 31, 32]],
    ['Props', [33, 34, 35, 36, 37, 38, 39, 40, 41]],
    ['Camp', [42, 43, 44, 45]],
  ];
  const MODES: [ScatterMode, string, IconName, string, string][] = [
    ['stamp', 'Stamp', 'stamp', 'Q', 'Click on the battlemap to put the chosen object down.'],
    ['brush', 'Brush', 'brush', 'W', 'Drag on the battlemap to scatter the chosen kinds (choose several).'],
    ['erase', 'Erase', 'eraser', 'E', 'Click an object to take it away, or drag to clear an area.'],
  ];
  const COVER = ['None', 'Half', 'Three-quarters', 'Full'];

  const nameOf = (k: number | string) => (typeof k === 'string' ? (sprites[k.slice(2)]?.name ?? 'Your sprite') : cap(catalog[k - 1]?.name ?? `kind ${k}`));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  // The palette: one group (or all, or your sprites), and a name to look for.
  let group = $state('All');
  let find = $state('');
  const needle = $derived(find.trim().toLowerCase());
  const shown = $derived(
    GROUPS.filter(([g]) => group === 'All' || group === g)
      .flatMap(([, kinds]) => kinds)
      .filter((k) => k <= catalog.length && (!needle || nameOf(k).toLowerCase().includes(needle))),
  );
  const ownShown = $derived(Object.entries(sprites).filter(([, m]) => (group === 'All' || group === 'Yours') && (!needle || m.name.toLowerCase().includes(needle))));

  function choose(kind: number | string) {
    // A stamp puts down one kind; a brush or eraser takes several (a click adds or drops one).
    if (settings.mode === 'stamp') settings.kinds = [kind];
    else if (!settings.kinds.length) settings.kinds = [kind];
    else settings.kinds = settings.kinds.includes(kind) ? settings.kinds.filter((k) => k !== kind) : [...settings.kinds, kind];
  }

  function setMode(m: ScatterMode) {
    settings.mode = m;
    if (m === 'stamp' && settings.kinds.length > 1) settings.kinds = [settings.kinds[0]];
  }

  // An upload being described, or an uploaded sprite being changed (a view of its own).
  let form = $state<{ asset: string | null; file: File | null; meta: SpriteMeta; preview: string | null } | null>(null);
  let busy = $state(false);
  let error = $state('');
  let removing = $state<string | null>(null);
  let input: HTMLInputElement | undefined = $state();

  const blank = (name: string): SpriteMeta => ({ name, size: 1, cover: 0, blocks_move: false, blocks_sight: false, difficult: false, height_ft: 3 });

  function picked(e: Event) {
    const file = (e.target as HTMLInputElement).files?.[0];
    (e.target as HTMLInputElement).value = '';
    if (!file) return;
    if (!file.type.startsWith('image/')) {
      error = 'That is not a picture';
      return;
    }
    if (file.size > MAX_PICTURE_BYTES) {
      error = `That picture is too big (${Math.round(MAX_PICTURE_BYTES / 1048576)} MB at most)`;
      return;
    }
    error = '';
    form = { asset: null, file, meta: blank(cap(file.name.replace(/\.[^.]+$/, '').replace(/[_-]+/g, ' ').trim()) || 'Custom object'), preview: URL.createObjectURL(file) };
  }

  function edit(asset: string) {
    error = '';
    form = { asset, file: null, meta: { ...blank(''), ...sprites[asset] }, preview: null };
    void picture(asset).then((u) => form && form.asset === asset && (form.preview = u));
  }

  async function save() {
    if (!form) return;
    const meta = { ...form.meta, name: form.meta.name.trim() || 'Custom object', size: Math.min(40, Math.max(0.2, Number(form.meta.size) || 1)), height_ft: Math.max(0, Number(form.meta.height_ft) || 0) };
    busy = true;
    try {
      if (form.file) await onUpload(form.file, meta);
      else if (form.asset) onSprite(form.asset, meta);
      closeForm();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
    }
  }

  function closeForm() {
    if (form?.file && form.preview) URL.revokeObjectURL(form.preview);
    form = null;
  }

  function remove(asset: string) {
    if ((placed[asset] ?? 0) > 0 && removing !== asset) {
      removing = asset;
      return;
    }
    removing = null;
    settings.kinds = settings.kinds.filter((k) => k !== `s:${asset}`);
    onSprite(asset, null);
  }
</script>

{#snippet thumb(kind: number | string)}
  {#if typeof kind === 'string'}
    {#await picture(kind.slice(2)) then src}{#if src}<img {src} alt="" />{/if}{/await}
  {:else}
    {#await icon(kind) then src}{#if src}<img {src} alt="" />{/if}{/await}
  {/if}
{/snippet}

{#if form && !peek}
  <div class="scatter">
    <button class="ws-btn quiet back" onclick={closeForm}><Icon name="chevron-left" size={16} /> Objects</button>
    <div class="formhead">
      {#if form.preview}<img src={form.preview} alt="" />{/if}
      <div>
        <b>{form.file ? 'New sprite' : form.meta.name || 'Sprite'}</b>
        <div class="ws-hint">Seen from above, with a transparent background.</div>
      </div>
    </div>
    <label class="ws-field">Name <input class="ws-input" bind:value={form.meta.name} /></label>
    <div class="ws-row">
      <label class="ws-field grow">Size (squares) <input class="ws-input" type="number" min="0.2" max="40" step="0.5" bind:value={form.meta.size} /></label>
      <label class="ws-field grow">Height (ft) <input class="ws-input" type="number" min="0" step="1" bind:value={form.meta.height_ft} /></label>
    </div>
    <div class="ws-field">
      Cover
      <div class="ws-seg">
        {#each COVER as c, i (c)}<button class:on={form.meta.cover === i} onclick={() => form && (form.meta.cover = i)}>{c === 'Three-quarters' ? '¾' : c}</button>{/each}
      </div>
    </div>
    <label class="switch"><span class="grow">Blocks movement</span><input type="checkbox" class="ws-switch" bind:checked={form.meta.blocks_move} /></label>
    <label class="switch"><span class="grow">Blocks sight</span><input type="checkbox" class="ws-switch" bind:checked={form.meta.blocks_sight} /></label>
    <label class="switch"><span class="grow">Difficult terrain</span><input type="checkbox" class="ws-switch" bind:checked={form.meta.difficult} /></label>
    {#if error}<div class="error">{error}</div>{/if}
    <div class="ws-row end">
      <button class="ws-btn" onclick={closeForm}>Cancel</button>
      <button class="ws-btn primary" disabled={busy} onclick={() => void save()}>{form.file ? 'Add sprite' : 'Save'}</button>
    </div>
  </div>
{:else}
  <div class="scatter">
    <div class="ws-seg" role="radiogroup" aria-label="Mode">
      {#each MODES as [m, label, ic, key] (m)}
        <button class:on={settings.mode === m} aria-pressed={settings.mode === m} title="{label} ({key})" onclick={() => setMode(m)}><Icon name={ic} size={16} />{label}</button>
      {/each}
    </div>
    {#if settings.kinds.length}
      <div class="chosen">
        {#each settings.kinds as k (k)}
          <span class="ws-chip on">{nameOf(k)}<button class="x" onclick={() => (settings.kinds = settings.kinds.filter((x) => x !== k))} aria-label="Drop {nameOf(k)}"><Icon name="x" size={12} /></button></span>
        {/each}
      </div>
    {:else if settings.mode !== 'erase'}
      <div class="ws-hint">Choose what to put down below.</div>
    {/if}
    {#if !peek}
      <div class="ws-hint">{MODES.find((m) => m[0] === settings.mode)?.[4]}</div>
      <details class="ws-group" open={remembered('scatter.brush', false)} ontoggle={(e) => remember('scatter.brush', e.currentTarget.open)}>
        <summary>{settings.mode === 'erase' ? 'Eraser' : 'Brush'} settings</summary>
        <div class="ws-group-body">
          {#if settings.mode !== 'erase'}
            <label class="slider">
              <span>Size</span><output>{settings.scale.toFixed(2)}×</output>
              <input type="range" min="0.5" max="2.5" step="0.05" bind:value={settings.scale} />
            </label>
            <label class="switch"><span class="grow">Vary each one a little</span><input type="checkbox" class="ws-switch" bind:checked={settings.vary} /></label>
            <div class="ws-row">
              <label class="switch grow"><span class="grow">Turn at random</span><input type="checkbox" class="ws-switch" checked={settings.rotation === null} onchange={(e) => (settings.rotation = e.currentTarget.checked ? null : 0)} /></label>
              {#if settings.rotation !== null}<input class="ws-input deg" type="number" step="15" bind:value={settings.rotation} aria-label="Turn (degrees)" />°{/if}
            </div>
          {/if}
          {#if settings.mode !== 'stamp'}
            <label class="slider">
              <span>Radius</span><output>{settings.radius} ft</output>
              <input type="range" min="5" max="60" step="2.5" bind:value={settings.radius} />
            </label>
          {/if}
          {#if settings.mode === 'brush'}
            <label class="slider">
              <span>Spacing</span><output>{settings.spacing} ft</output>
              <input type="range" min="2.5" max="40" step="2.5" bind:value={settings.spacing} />
            </label>
          {/if}
          {#if settings.mode === 'erase'}
            <label class="switch"><span class="grow">Only the chosen kinds</span><input type="checkbox" class="ws-switch" bind:checked={settings.onlyChosen} /></label>
          {/if}
        </div>
      </details>

      <div class="filters">
        {#each ['All', ...GROUPS.map(([g]) => g), 'Yours'] as g (g)}
          <button class="ws-chip" class:on={group === g} onclick={() => (group = g)}>{g}</button>
        {/each}
      </div>
      <input class="ws-input" type="search" placeholder="Find an object…" bind:value={find} aria-label="Find an object" />
      <div class="kinds">
        {#if group !== 'Yours'}
          {#each shown as k (k)}
            <button class="kind" class:on={settings.kinds.includes(k)} aria-pressed={settings.kinds.includes(k)} title={nameOf(k)} onclick={() => choose(k)}>
              {@render thumb(k)}
              <span>{nameOf(k)}</span>
            </button>
          {/each}
        {/if}
        {#each ownShown as [asset, m] (asset)}
          {@const k = `s:${asset}`}
          <span class="own">
            <button class="kind" class:on={settings.kinds.includes(k)} aria-pressed={settings.kinds.includes(k)} title={`${m.name} (${placed[asset] ?? 0} put down)`} onclick={() => choose(k)}>
              {@render thumb(k)}
              <span>{m.name}</span>
            </button>
            <button class="tiny" title="Change its rules" aria-label="Change {m.name}" onclick={() => edit(asset)}><Icon name="pencil" size={12} /></button>
            <button class="tiny" class:warn={removing === asset} title={removing === asset ? `Click again to remove it and the ${placed[asset]} put down` : 'Remove it'} aria-label="Remove {m.name}" onclick={() => remove(asset)}><Icon name="x" size={12} /></button>
          </span>
        {/each}
        {#if group === 'All' || group === 'Yours'}
          <button class="kind upload" onclick={() => input?.click()} title="Upload a picture as a new kind of object"><Icon name="upload" size={24} /><span>Upload…</span></button>
        {/if}
        <input bind:this={input} type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml" hidden onchange={picked} />
      </div>
      {#if error}<div class="error">{error}</div>{/if}
    {/if}
  </div>
{/if}

<style>
  .scatter {
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
  .end {
    justify-content: flex-end;
  }
  .back {
    align-self: flex-start;
    padding-left: 2px;
  }
  .chosen,
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chosen .x {
    display: inline-flex;
    margin-right: -4px;
    padding: 2px;
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
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
  .deg {
    width: 64px;
  }
  .kinds {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(70px, 1fr));
    gap: 4px;
  }
  .kind {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    min-height: 66px;
    padding: 3px 2px;
    background: #efe6cf;
    border: 1px solid #b3a385;
    border-radius: var(--radius-sm);
    cursor: pointer;
    font: 11px/1.15 var(--font);
    color: var(--ink);
  }
  .kind:hover {
    background: var(--btn-hover);
  }
  .kind.on {
    border-color: var(--line);
    background: #d9c79c;
    box-shadow: inset 0 0 0 1px var(--line);
  }
  .kind :global(img) {
    width: 38px;
    height: 38px;
    object-fit: contain;
  }
  .kind span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .upload {
    justify-content: center;
    gap: 4px;
    border-style: dashed;
    color: var(--ink-2);
  }
  .own {
    position: relative;
    display: flex;
  }
  .own .tiny {
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
  .own .tiny:first-of-type {
    left: 2px;
  }
  .own .tiny:last-of-type {
    right: 2px;
  }
  .tiny.warn {
    background: #a33a2a;
    color: #fff;
  }
  .formhead {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .formhead img {
    width: 48px;
    height: 48px;
    object-fit: contain;
    background: var(--field);
    border: 1px solid #b3a385;
    border-radius: var(--radius-sm);
  }
  .error {
    color: #a33a2a;
  }
</style>
