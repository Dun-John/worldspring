<script lang="ts">
  // Play (local table), the DM's side: the tools (all that shows when the panel is folded
  // down), the selected token, the players' window and view, this location's fog, dark and line
  // of sight, and the table's settings (grid scale, diagonals, reset) folded away.
  import { SIZES, type Measure, type ShapeKind, type Token, type TokenKind } from './state';
  import { SHAPE_COLORS, TOKEN_COLORS, type PlayController, type Tool } from './controller';
  import { formatDistance } from './vision';
  import Icon from '../ui/Icon.svelte';
  import type { IconName } from '../ui/icons';
  import { remember, remembered } from '../ui/shell/layout.svelte';

  const touch = typeof matchMedia !== 'undefined' && matchMedia('(pointer: coarse)').matches;

  interface Props {
    play: PlayController;
    /** Bumped whenever the play state or the controller changes. */
    version: number;
    /** Only the tools (the panel is folded down). */
    peek?: boolean;
  }
  let { play, version, peek = false }: Props = $props();

  const v = $derived.by(() => {
    void version;
    const loc = play.place?.loc ?? 'surface';
    const sel = play.selected ? (play.state.tokens[play.selected] ?? null) : null;
    return {
      tool: play.tool,
      sel,
      count: play.selection.size,
      levels: sel ? play.levelsOf(sel) : [],
      settings: play.settingsHere(),
      los: play.state.los,
      grid: { ...play.state.grid },
      inside: !!play.view.interior,
      place: play.placeName,
      here: Object.values(play.state.tokens).filter((t) => t.loc === loc).length,
      shapes: Object.values(play.state.shapes).filter((s) => s.loc === loc).length,
      players: play.players,
      follow: play.follow,
      lock: play.lock,
      draft: { ...play.draft },
      shapeKind: play.shapeKind,
      shapeColor: play.shapeColor,
      fogReveal: play.fogReveal,
      fogRadius: play.fogRadius,
      fogRooms: play.fogRooms,
    };
  });

  const TOOLS: [Tool, IconName, string, string][] = [
    ['select', 'pointer', 'Select and move tokens, open doors', 'V'],
    ['token', 'token', 'Place tokens', 'T'],
    ['measure', 'ruler', 'Measure', 'M'],
    ['shape', 'shapes', 'Shapes', 'S'],
    ['fog', 'cloud', 'Fog of war brush', 'F'],
  ];
  const KINDS: [TokenKind, string][] = [
    ['character', 'Character'],
    ['light', 'Light'],
  ];
  const SHAPES: [ShapeKind, string][] = [
    ['circle', 'Circle'],
    ['rect', 'Rectangle'],
    ['line', 'Line'],
    ['cone', 'Cone'],
  ];
  const MEASURES: [Measure, string][] = [
    ['chebyshev', 'Diagonals count 1'],
    ['alternating', 'Diagonals 1, then 2'],
    ['euclidean', 'Straight line'],
    ['manhattan', 'Across + along'],
  ];
  const sizeLabel = (n: number) => (n === 0.5 ? '½ square' : `${n}×${n}`);
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  let confirmReset = $state(false);

  function setDraft(patch: Partial<typeof play.draft>) {
    Object.assign(play.draft, patch);
    play.onChange();
  }

  function setShape(kind: ShapeKind | null, color: number | null) {
    if (kind) play.shapeKind = kind;
    if (color !== null) play.shapeColor = color;
    play.onChange();
  }

  function setFog(patch: { reveal?: boolean; radius?: number; rooms?: boolean }) {
    if (patch.reveal !== undefined) play.fogReveal = patch.reveal;
    if (patch.radius !== undefined) play.fogRadius = patch.radius;
    if (patch.rooms !== undefined) play.fogRooms = patch.rooms;
    play.onChange();
  }

  function patch(t: Token, p: Partial<Token>) {
    play.updateToken(t.id, p);
  }

  const value = (e: Event) => Number((e.target as HTMLInputElement).value) || 0;
  const checked = (e: Event) => (e.target as HTMLInputElement).checked;
</script>

<div class="play">
  <div class="where" title="Tokens, fog and doors are kept per location">
    <span class="live"></span> Live{v.players ? ' · players’ window open' : ''} · <i>{v.place}</i> · {v.here} {v.here === 1 ? 'token' : 'tokens'}
  </div>

  <div class="tools" role="toolbar" aria-label="Play tools">
    {#each TOOLS as [t, icon, label, key] (t)}
      <button class="ws-icon-btn" aria-pressed={v.tool === t} title="{label} ({key})" aria-label={label} onclick={() => play.setTool(t)}><Icon name={icon} /></button>
    {/each}
  </div>

  {#if !peek}
    {#if v.tool === 'select'}
      <div class="ws-hint">{touch ? 'Drag tokens; tap a door to open it.' : 'Drag tokens; click a door to open it. Shift-click or shift-drag selects several; Alt-click pings.'}</div>
    {:else if v.tool === 'token'}
      <section>
        <div class="ws-row">
          <input class="ws-input grow" value={v.draft.name} oninput={(e) => setDraft({ name: (e.target as HTMLInputElement).value })} aria-label="New token's name" />
          {#if v.draft.kind === 'character'}
            <select class="ws-input" value={v.draft.size} onchange={(e) => setDraft({ size: value(e) })} aria-label="Size">
              {#each SIZES as n (n)}<option value={n}>{sizeLabel(n)}</option>{/each}
            </select>
          {/if}
        </div>
        <div class="ws-seg">
          {#each KINDS as [k, label] (k)}<button class:on={v.draft.kind === k} onclick={() => setDraft({ kind: k })}>{label}</button>{/each}
        </div>
        {#if v.draft.kind === 'character'}
          <label class="switch"><span class="grow">Players see by it</span><input type="checkbox" class="ws-switch" checked={v.draft.vision} onchange={(e) => setDraft({ vision: checked(e) })} /></label>
          <div class="swatches">
            {#each TOKEN_COLORS as c (c)}
              <button class="sw" class:on={v.draft.color === c} style:background={hex(c)} aria-label="Colour" onclick={() => setDraft({ color: c })}></button>
            {/each}
          </div>
        {/if}
        <div class="ws-hint">Click the map to place one. Drop a token on a building to put it inside.</div>
      </section>
    {:else if v.tool === 'shape'}
      <section>
        <div class="ws-seg">
          {#each SHAPES as [s, label] (s)}<button class:on={v.shapeKind === s} onclick={() => setShape(s, null)}>{label}</button>{/each}
        </div>
        <div class="ws-row">
          <div class="swatches grow">
            {#each SHAPE_COLORS as c (c)}
              <button class="sw" class:on={v.shapeColor === c} style:background={hex(c)} aria-label="Colour" onclick={() => setShape(null, c)}></button>
            {/each}
          </div>
          <button class="ws-btn" onclick={() => play.clearShapes()} disabled={!v.shapes}>Clear ({v.shapes})</button>
        </div>
        <div class="ws-hint">Drag from a corner of the grid. Right-click a shape to take it away.</div>
      </section>
    {:else if v.tool === 'fog'}
      <section>
        <div class="ws-seg">
          <button class:on={v.fogReveal} onclick={() => setFog({ reveal: true })}>Reveal</button>
          <button class:on={!v.fogReveal} onclick={() => setFog({ reveal: false })}>Hide</button>
          {#if v.inside}<button class:on={v.fogRooms} onclick={() => setFog({ rooms: !v.fogRooms })} title="Click a room to reveal or hide all of it">Whole rooms</button>{/if}
        </div>
        {#if !(v.inside && v.fogRooms)}
          <label class="slider"><span>Brush</span><output>{formatDistance(v.fogRadius, v.grid)}</output><input type="range" min="1" max="12" value={v.fogRadius} oninput={(e) => setFog({ radius: value(e) })} /></label>
        {/if}
        <div class="ws-row">
          {#if v.inside}<button class="ws-btn grow" onclick={() => play.fogAll(true)}>Reveal all</button>{/if}
          <button class="ws-btn grow" onclick={() => play.fogAll(false)} title="Hide everything here again, including what the characters have seen">Hide all</button>
        </div>
        {#if !v.settings.fog}<div class="ws-hint warn">Fog is off here: the players see everything.</div>{/if}
        <div class="ws-hint">Shift-drag does the opposite.</div>
      </section>
    {:else if v.tool === 'measure'}
      <div class="ws-hint">Drag to measure; the players see it too.</div>
    {/if}

    {#if v.count > 1}
      <section class="card">
        <div class="ws-row">
          <b class="grow">{v.count} tokens selected</b>
          <button class="ws-btn" onclick={() => play.select(null)}>Deselect</button>
          <button class="ws-btn danger" onclick={() => play.removeSelected()}>Remove</button>
        </div>
        <div class="ws-hint">Drag one to move them all; they come along into buildings, up and down floors, and back out.</div>
      </section>
    {:else if v.sel}
      {@const t = v.sel}
      <section class="card">
        <div class="ws-row">
          <input class="ws-input grow name" value={t.name} oninput={(e) => patch(t, { name: (e.target as HTMLInputElement).value })} aria-label="Name" />
          {#if t.kind === 'character'}
            <select class="ws-input" value={t.size} onchange={(e) => patch(t, { size: value(e) })} aria-label="Size">
              {#each SIZES as n (n)}<option value={n}>{sizeLabel(n)}</option>{/each}
            </select>
          {/if}
        </div>
        <div class="ws-seg">
          {#each KINDS as [k, label] (k)}<button class:on={t.kind === k} onclick={() => patch(t, { kind: k })}>{label}</button>{/each}
        </div>
        <label class="switch"><span class="grow">Hidden<span class="ws-muted"> · only you see it</span></span><input type="checkbox" class="ws-switch" checked={!!t.hidden} onchange={(e) => patch(t, { hidden: checked(e) })} /></label>
        {#if t.kind === 'character'}
          <label class="switch"><span class="grow">Players see by it</span><input type="checkbox" class="ws-switch" checked={!!t.vision} onchange={(e) => patch(t, { vision: checked(e) })} /></label>
          <div class="swatches">
            {#each TOKEN_COLORS as c (c)}
              <button class="sw" class:on={t.color === c} style:background={hex(c)} aria-label="Colour" onclick={() => patch(t, { color: c })}></button>
            {/each}
          </div>
        {/if}
        <div class="ws-row">
          <label class="ws-field grow" title="How far its light reaches (lights the dark)">
            Light ({v.grid.unit})
            <input class="ws-input" type="number" min="0" step={v.grid.scale} value={Math.round((t.light ?? 0) * v.grid.scale * 10) / 10} onchange={(e) => patch(t, { light: value(e) / v.grid.scale || undefined })} />
          </label>
          {#if v.levels.length > 1}
            <label class="ws-field grow" title="Move it to another floor of this building">
              Floor
              <select class="ws-input" value={Number(t.loc.slice(t.loc.lastIndexOf('@') + 1))} onchange={(e) => play.tokenToLevel(t.id, value(e))}>
                {#each v.levels as l (l.i)}<option value={l.i}>{l.name}</option>{/each}
              </select>
            </label>
          {/if}
        </div>
        <div class="ws-row">
          {#if t.kind === 'character'}
            <label class="ws-btn file"><Icon name="image" size={16} /> Picture… <input type="file" accept="image/*" onchange={(e) => void play.setTokenImage(t.id, (e.target as HTMLInputElement).files?.[0] ?? null)} /></label>
            {#if t.image}<button class="ws-btn quiet" onclick={() => void play.setTokenImage(t.id, null)}>No picture</button>{/if}
          {/if}
          <span class="grow"></span>
          <button class="ws-btn danger" onclick={() => play.removeToken(t.id)}><Icon name="trash" size={16} /> Remove</button>
        </div>
      </section>
    {/if}

    <section>
      <div class="ws-label">Players</div>
      <div class="ws-row">
        <button class="ws-btn grow" onclick={() => play.openPlayerWindow()}><Icon name="monitor" size={16} /> {v.players ? 'Players’ window open' : 'Open the players’ window'}</button>
        <button class="ws-btn" onclick={() => play.sendCamera(true)} title="Bring the players' view to yours">Show my view</button>
      </div>
      <div class="ws-seg" role="radiogroup" aria-label="The players' view">
        <button class:on={!v.follow && !v.lock} onclick={() => play.setFollow(false, false)} title="Players move their own view">Free</button>
        <button class:on={v.follow && !v.lock} onclick={() => play.setFollow(true, false)} title="Their view follows yours">Follow me</button>
        <button class:on={v.lock} onclick={() => play.setFollow(true, true)} title="They can't move their view"><Icon name="lock" size={14} /> Locked</button>
      </div>
    </section>

    <section>
      <div class="ws-label">This location</div>
      <label class="switch"><Icon name="cloud" size={16} /><span class="grow">Fog of war<span class="sub">Players see only what is revealed or seen here</span></span><input type="checkbox" class="ws-switch" checked={v.settings.fog} onchange={(e) => play.setSettingsHere({ fog: checked(e) })} /></label>
      <label class="switch"><Icon name="moon" size={16} /><span class="grow">Dark<span class="sub">Characters see only what light reaches</span></span><input type="checkbox" class="ws-switch" checked={v.settings.dark} onchange={(e) => play.setSettingsHere({ dark: checked(e) })} /></label>
      <label class="switch"><Icon name="eye" size={16} /><span class="grow">Line of sight<span class="sub">Players see what their tokens see (everywhere)</span></span><input type="checkbox" class="ws-switch" checked={v.los} onchange={(e) => play.dispatch({ t: 'settings', los: checked(e) })} /></label>
    </section>

    <details class="ws-group" open={remembered('play.table', false)} ontoggle={(e) => remember('play.table', e.currentTarget.open)}>
      <summary>Table settings</summary>
      <div class="ws-group-body">
        <div class="ws-row">
          <label class="ws-field">1 square =<input class="ws-input num" type="number" min="0.1" step="0.5" value={v.grid.scale} onchange={(e) => value(e) > 0 && play.setGrid({ scale: value(e) })} /></label>
          <label class="ws-field">Unit<input class="ws-input num" value={v.grid.unit} onchange={(e) => play.setGrid({ unit: (e.target as HTMLInputElement).value.trim() })} /></label>
          <label class="ws-field grow">Diagonals
            <select class="ws-input" value={v.grid.measure} onchange={(e) => play.setGrid({ measure: (e.target as HTMLSelectElement).value as Measure })}>
              {#each MEASURES as [m, label] (m)}<option value={m}>{label}</option>{/each}
            </select>
          </label>
        </div>
        <div class="ws-row">
          {#if confirmReset}
            <button class="ws-btn danger" onclick={() => ((confirmReset = false), play.resetSession())}>Clear every token, shape and fog?</button>
            <button class="ws-btn quiet" onclick={() => (confirmReset = false)}>Cancel</button>
          {:else}
            <button class="ws-btn" onclick={() => (confirmReset = true)} title="Remove every token, shape and fog for this world">Reset the session…</button>
          {/if}
        </div>
      </div>
    </details>
  {/if}
</div>

<style>
  .play {
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
  .where {
    font-size: 12px;
    color: var(--ink-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .live {
    display: inline-block;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #c2410c;
  }
  .tools {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 2px;
  }
  .tools .ws-icon-btn {
    width: auto;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  section:not(.card) {
    border-top: 1px solid var(--line-faint);
    padding-top: 8px;
  }
  .card {
    padding: 8px;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.35);
  }
  .name {
    font-weight: bold;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .switch .sub {
    display: block;
    font-size: 11px;
    color: var(--ink-3);
    line-height: 1.25;
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .sw {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 2px solid var(--paper-solid);
    box-shadow: 0 0 0 1px var(--line);
    cursor: pointer;
  }
  .sw.on {
    box-shadow: 0 0 0 2px #38bdf8;
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
  .num {
    width: 60px;
  }
  .file {
    position: relative;
    overflow: hidden;
  }
  .file input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
  .warn {
    color: #8b2e2e;
  }
</style>
