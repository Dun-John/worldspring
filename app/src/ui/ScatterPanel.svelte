<script lang="ts">
  // Edit › Objects: put battlemap objects down (one at a time, picked up and moved, or a brush
  // scattering the chosen kinds, chosen in the item picker), take them away (click or brush), and upload pictures as
  // new kinds of object with their tactical rules. The tool works on the map while the tab is open.
  import type { KindInfo } from '../gen/client';
  import type { SpriteMeta } from '../gen/protocol';
  import { BATTLE_GROUPS } from '../editor/itemGroups';
  import type { HeldView } from '../editor/placing';
  import type { ScatterMode, ScatterSettings } from '../editor/scatter';
  import { drawnTurned } from '../render/BattlemapLayer';
  import type { ThumbSheet } from '../render/thumbs';
  import { MAX_PICTURE_BYTES } from '../world/assets';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import InHand from './InHand.svelte';
  import ItemPicker, { type PickItem } from './ItemPicker.svelte';
  import { remember, remembered, shell } from './shell/layout.svelte';

  interface Props {
    settings: ScatterSettings;
    catalog: KindInfo[];
    sprites: Record<string, SpriteMeta>;
    /** How many of each uploaded sprite are put down. */
    placed: Record<string, number>;
    /** The built-in kinds' pictures, one sheet keyed by kind id. */
    thumbs: () => Promise<ThumbSheet> | null | undefined;
    /** An uploaded sprite's picture (object URL). */
    picture: (asset: string) => Promise<string | null>;
    /** Only the modes and the chosen kinds (the panel is folded down). */
    peek?: boolean;
    onUpload: (file: File, meta: SpriteMeta) => Promise<void>;
    /** Change an uploaded sprite's rules, or (null) remove it and every one put down. */
    onSprite: (asset: string, meta: SpriteMeta | null) => void;
    /** Inside a building or site: Rooms puts things there (`building`/`site`), or nothing does (`other`). */
    inside: 'building' | 'site' | 'other' | null;
    /** Rooms, on its Furniture (Props) tool. */
    onRooms: () => void;
    /** The object picked up, if any. */
    held: HeldView;
    /** What is in the hand turned (clockwise 1), back to its own turn, put away, sized. */
    onTurn: (d: 1 | -1) => void;
    onAuto: () => void;
    onEmpty: () => void;
    onSize: (d: 1 | -1) => void;
  }

  let { settings = $bindable(), catalog, sprites, placed, thumbs, picture, peek = false, onUpload, onSprite, inside, onRooms, held, onTurn, onAuto, onEmpty, onSize }: Props = $props();

  const MODES: [ScatterMode, string, IconName, string, string][] = [
    ['place', 'Place', 'stamp', 'Q', 'Click on the battlemap to put the chosen object down (again and again, even on top of others); T, Shift+T or Shift+wheel turn it, Z turns it at random again. To move one you put down, empty the hand (Esc or ×) and click it; Delete takes the one under the pointer away. On a touch screen, tap to move the ghost, then ✓.'],
    ['brush', 'Brush', 'brush', 'W', 'Drag on the battlemap to scatter the chosen kinds (choose several).'],
    ['erase', 'Erase', 'eraser', 'E', 'Click an object to take it away, or drag to clear an area.'],
  ];
  const COVER = ['None', 'Half', 'Three-quarters', 'Full'];

  const nameOf = (k: number | string) => (typeof k === 'string' ? (sprites[k.slice(2)]?.name ?? 'Your sprite') : cap(catalog[k - 1]?.name ?? `kind ${k}`));
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
  /** Whether a kind shows a turn given by hand (as the tool's `turns`). */
  const turnsKind = (k: number | string) => typeof k === 'string' || drawnTurned(catalog[k - 1], true);

  const SHORT_COVER = ['', '½ cover', '¾ cover', 'full cover'];
  const squares = (n: number) => `${n} square${n === 1 ? '' : 's'}`;
  const rules = (r: { blocks_move: boolean; blocks_sight: boolean; cover: number; difficult: boolean; height_ft: number }) =>
    [r.blocks_move ? 'blocks movement' : '', r.blocks_sight ? 'blocks sight' : '', SHORT_COVER[r.cover] ?? '', r.difficult ? 'difficult terrain' : '', `${+r.height_ft.toFixed(1)} ft`].filter(Boolean).join(' · ');
  const groupOf = new Map(BATTLE_GROUPS.flatMap(([g, kinds]) => kinds.map((k) => [k, g] as const)));
  // The picker's items: the built-in kinds (by group), then your sprites.
  const items = $derived<PickItem[]>([
    ...catalog.map((k): PickItem => ({
      id: k.id,
      name: cap(k.name),
      group: groupOf.get(k.id) ?? 'Other',
      thumb: String(k.id),
      size: `about ${squares(Math.max(0.5, Math.round(k.radius * 4) / 2))}`,
      rules: rules(k),
      note: `4 looks · ${drawnTurned(k, true) ? 'can be turned' : 'same any way round'}`,
      hazard: k.hazard?.effect ?? null,
    })),
    ...Object.entries(sprites).map(([asset, m]): PickItem => ({
      id: `s:${asset}`,
      name: m.name,
      group: 'Yours',
      asset,
      size: squares(m.size),
      rules: rules(m),
      note: 'Your picture · can be turned',
      title: `${m.name} (${placed[asset] ?? 0} put down)`,
    })),
  ]);

  function choose(kind: number | string) {
    // Place puts down one kind; a brush or eraser takes several (a click adds or drops one).
    if (settings.mode === 'place') {
      settings.kinds = [kind];
      // (A phone's sheet goes down to its tools: the map shows where it goes.)
      if (shell.phone) shell.snap = 'peek';
    }
    else if (!settings.kinds.length) settings.kinds = [kind];
    else settings.kinds = settings.kinds.includes(kind) ? settings.kinds.filter((k) => k !== kind) : [...settings.kinds, kind];
  }

  function setMode(m: ScatterMode) {
    settings.mode = m;
    if (m === 'place' && settings.kinds.length > 1) settings.kinds = [settings.kinds[0]];
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
    {#if inside}
      <div class="inside">
        <span class="grow">
          {#if inside === 'building'}You’re inside: objects go on the battlemap outside. Furniture and traps go in Rooms.
          {:else if inside === 'site'}You’re underground: objects go on the battlemap above. Props and traps go in Rooms.
          {:else}Go back out to put objects down.{/if}
        </span>
        {#if inside !== 'other'}
          <button class="ws-btn" onclick={onRooms}><Icon name="chest" size={16} /> {inside === 'building' ? 'Furniture' : 'Props'} in Rooms</button>
        {/if}
      </div>
    {/if}
    {#if settings.mode === 'place' && (held || settings.kinds.length)}
      {@const k = settings.kinds[0]}
      {@const t = held ? held.turns : turnsKind(k)}
      <InHand
        name={held?.name ?? nameOf(k)}
        held={!!held}
        quarter={false}
        turns={t}
        turn={held ? held.turn : t ? (settings.rotation === null ? 'at random' : `${settings.rotation}°`) : ''}
        auto={held ? held.auto : t && settings.rotation !== null}
        {onTurn}
        {onAuto}
        {onEmpty}
      >
        {#snippet size()}
          <button class="ws-icon-btn" title="Smaller ([)" aria-label="Smaller" onclick={() => onSize(-1)}>−</button>
          <output>{held ? held.size : `${settings.scale.toFixed(2)}×`}</output>
          <button class="ws-icon-btn" title="Bigger (])" aria-label="Bigger" onclick={() => onSize(1)}>+</button>
        {/snippet}
      </InHand>
    {:else if settings.kinds.length}
      <div class="chosen">
        {#each settings.kinds as k (k)}
          <span class="ws-chip on">{nameOf(k)}<button class="x" onclick={() => (settings.kinds = settings.kinds.filter((x) => x !== k))} aria-label="Drop {nameOf(k)}"><Icon name="x" size={12} /></button></span>
        {/each}
      </div>
    {:else if settings.mode !== 'erase' && !inside}
      <div class="ws-hint">Choose what to put down below{settings.mode === 'place' ? ', or click one you put down to pick it up' : ''}.</div>
    {/if}
    {#if !peek}
      <div class="ws-hint">{MODES.find((m) => m[0] === settings.mode)?.[4]}</div>
      <details class="ws-group" open={remembered('scatter.brush', false)} ontoggle={(e) => remember('scatter.brush', e.currentTarget.open)}>
        <summary>{settings.mode === 'erase' ? 'Eraser settings' : settings.mode === 'place' ? 'More size and turn' : 'Brush settings'}</summary>
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
          {#if settings.mode !== 'place'}
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

      <ItemPicker
        {items}
        groups={[...BATTLE_GROUPS.map(([g]) => g), 'Other']}
        chosen={settings.kinds}
        context="scatter"
        {thumbs}
        {picture}
        onPick={choose}
        onUpload={() => input?.click()}
        onEditOwn={edit}
        onRemoveOwn={remove}
        {removing}
      />
      <input bind:this={input} type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml" hidden onchange={picked} />
      {#if error}<div class="error">{error}</div>{/if}
    {/if}
  </div>
{/if}

<style>
  .inside {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
  }
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
  .chosen {
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
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
  }
  .error {
    color: var(--danger);
  }
</style>
