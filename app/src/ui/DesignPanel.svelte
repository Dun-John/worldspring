<script lang="ts">
  // Edit › Design: tools for the map (rooms, corridors, rock, doors, props, the way down; in a
  // building rooms, wall lines, walls taken away, doors, furniture, the stairs), the room chosen
  // (what it is, its name, a raised floor, the boss chamber, props), the site's levels and what
  // is wrong with it (folded away), and undo, redo and Save (only once nothing breaks the rules
  // a site keeps) pinned at the bottom. Props come in groups: a building's furniture and indoor
  // props, a site's props, and the pictures uploaded in Scatter (either). A building's storeys are
  // added or taken away here too (the building follows on Save).
  import type { SpriteMeta, UnderCatalog } from '../gen/protocol';
  import type { DesignMode, DesignSettings, SiteDesigner } from '../editor/site/designer';
  import { BOSS } from '../editor/site/designer';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';
  import { remember, remembered } from './shell/layout.svelte';

  interface Props {
    designer: SiteDesigner;
    /** Bumped on every change to the draft (the designer is not reactive). */
    version: number;
    settings: DesignSettings;
    catalog: UnderCatalog | null;
    name: string;
    renames: Record<string, string>;
    /** A design of this site is saved (it can go back to the generated one). */
    saved: boolean;
    /** Uploaded pictures (asset id → name and rules), and a picture's URL. */
    sprites: Record<string, SpriteMeta>;
    picture: (asset: string) => Promise<string | null>;
    /** Only the tools and Save (the panel is folded down). */
    peek?: boolean;
    onRename: (id: string, name: string) => void;
    onSave: () => void;
    onReset: () => void;
    onProblem: (level: number, at?: [number, number]) => void;
  }

  let { designer, version, settings = $bindable(), catalog, name, renames, saved, sprites, picture, peek = false, onRename, onSave, onReset, onProblem }: Props = $props();

  const TOOLS: { key: DesignMode; label: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'select', label: 'Choose', icon: 'pointer', kbd: 'V', hint: 'Click a room to choose it.' },
    { key: 'room', label: 'Room', icon: 'room', kbd: 'R', hint: 'Paint squares into the room chosen, else into a new room of the kind below.' },
    { key: 'rect', label: 'Rectangle', icon: 'square', kbd: 'X', hint: 'Drag out a new room of the kind below.' },
    { key: 'corridor', label: 'Corridor', icon: 'corridor', kbd: 'H', hint: 'Paint the level’s passages.' },
    { key: 'rock', label: 'Rock', icon: 'rock', kbd: 'K', hint: 'Fill squares back with rock (props on them go too).' },
    { key: 'door', label: 'Door', icon: 'door', kbd: 'O', hint: 'Click a wall between two rooms: a door, again a secret door, again none.' },
    { key: 'prop', label: 'Props', icon: 'chest', kbd: 'F', hint: 'Click to put the prop down; click a prop to take it away.' },
    { key: 'stairs', label: 'Way down', icon: 'stairs', kbd: 'W', hint: 'Click a square for this level’s way down; the way up below follows.' },
  ];
  // A building's: walls are where rooms meet; its floors follow its storeys.
  const BUILDING_TOOLS: typeof TOOLS = [
    { key: 'select', label: 'Choose', icon: 'pointer', kbd: 'V', hint: 'Click a room to choose it.' },
    { key: 'room', label: 'Room', icon: 'room', kbd: 'R', hint: 'Paint squares into the room chosen, else into a new room of the kind below.' },
    { key: 'rect', label: 'Rectangle', icon: 'square', kbd: 'X', hint: 'Drag out a new room of the kind below, inside the walls.' },
    { key: 'wall', label: 'Wall', icon: 'wall', kbd: 'L', hint: 'Draw a line along the grid across a room (or click by a grid line): the wall runs on to the room’s walls and splits it in two.' },
    { key: 'merge', label: 'Take a wall away', icon: 'eraser', kbd: 'E', hint: 'Click a wall between two rooms: they become one.' },
    { key: 'door', label: 'Door', icon: 'door', kbd: 'O', hint: 'Click a wall between two rooms: a door, again a secret door, again none. In an outside wall of the ground floor: a back door, again the front door, again none.' },
    { key: 'prop', label: 'Furniture', icon: 'chest', kbd: 'F', hint: 'Click to put the piece down (furniture, a prop or one of your pictures); click a piece to take it away.' },
    { key: 'stairs', label: 'Stairs', icon: 'stairs', kbd: 'W', hint: 'Drag out the stair block (1 to 3 squares each way): the same squares on every floor it reaches. Furniture there goes.' },
  ];
  const cap = (k: string) => k.charAt(0).toUpperCase() + k.slice(1);

  // The designer as it is now (read again on every change: `version`).
  const now = $derived.by(() => {
    void version;
    const d = designer;
    return { draft: d.draft, level: d.level, selected: d.selected, problems: d.problems, dirty: d.dirty, canUndo: d.canUndo, canRedo: d.canRedo, storeys: d.storeys, storeysChanged: d.storeysChanged };
  });
  const draft = $derived(now.draft);
  const building = $derived(draft?.kind === 'building');
  const tools = $derived(building ? BUILDING_TOOLS : TOOLS);
  const level = $derived(now.level);
  const lv = $derived(draft?.levels[level] ?? null);
  const room = $derived(now.selected !== null && lv ? (lv.rooms[now.selected] ?? null) : null);
  const roomId = $derived(now.selected !== null ? `r:${designer.id}:${level}:${now.selected}` : '');
  const theme = $derived(building ? null : (catalog?.themes.find((t) => t.key === draft?.theme) ?? null));
  const own = $derived(theme?.rooms ?? []);
  const allRooms = $derived((building ? catalog?.building.rooms : catalog?.rooms) ?? []);
  const others = $derived(allRooms.filter((k) => !own.includes(k)));
  const problems = $derived(now.problems);
  const blocking = $derived(problems.filter((p) => p.blocking).length);
  const tool = $derived(tools.find((t) => t.key === settings.mode));
  let propFilter = $state('');
  // The palette's groups: a building's furniture or indoor props, a site's props, your pictures.
  type Piece = { kind: string; name: string; cover: number; blocks: boolean; hazard: string | null; w: number; h: number; asset?: string };
  let group = $state<'main' | 'props' | 'yours'>('main');
  const yours = $derived(
    Object.entries(sprites).map(([asset, m]): Piece => {
      const n = Math.max(1, Math.min(6, Math.ceil(m.size || 1)));
      return { kind: `s:${asset}`, name: m.name || 'your picture', cover: m.cover, blocks: m.blocks_move, hazard: null, w: n, h: n, asset };
    }),
  );
  const pieces = $derived<Piece[]>(
    group === 'yours' ? yours : building ? (group === 'props' ? (catalog?.building.props ?? []) : (catalog?.building.furniture.map((f) => ({ ...f, hazard: null })) ?? [])) : (catalog?.props ?? []),
  );
  const propList = $derived(pieces.filter((p) => !propFilter || p.name.toLowerCase().includes(propFilter.toLowerCase()) || p.kind.includes(propFilter.toLowerCase())));
  let sure = $state<'' | 'level' | 'reset' | 'room' | 'storey'>('');

  /** Ask twice before something that can't be put back in one click. */
  function twice(what: typeof sure, act: () => void) {
    if (sure === what) {
      sure = '';
      act();
    } else {
      sure = what;
      setTimeout(() => sure === what && (sure = ''), 3000);
    }
  }

  function pickProp(kind: string) {
    const p = pieces.find((x) => x.kind === kind);
    settings = { ...settings, mode: 'prop', prop: kind, pw: p?.w ?? 1, ph: p?.h ?? 1 };
  }

  const saveWhy = $derived(blocking ? 'Fix the problems marked ✕ first' : !now.dirty ? 'Nothing new to save' : 'Save the design (Ctrl+S)');
</script>

<div class="design">
  {#if !peek}<div class="site">Designing <b>{name}</b> <span class="ws-muted">· {building ? (lv?.name ?? '') : `level ${level + 1} of ${draft?.levels.length ?? 1}`}</span></div>{/if}
  <div class="tools" role="toolbar" aria-label="Design tools">
    {#each tools as t (t.key)}
      <button class="ws-icon-btn" aria-pressed={settings.mode === t.key} title="{t.label} ({t.kbd})" aria-label={t.label} onclick={() => (settings = { ...settings, mode: t.key })}><Icon name={t.icon} /></button>
    {/each}
  </div>
  {#if !peek}
    <div><b>{tool?.label}</b> <span class="ws-hint">{tool?.hint}</span></div>

    {#if settings.mode === 'room' || settings.mode === 'rect'}
      <label class="ws-field">
        New room
        <select class="ws-input" bind:value={settings.kind}>
          {#if own.length}
            <optgroup label="This site’s">
              {#each own as k (k)}<option value={k}>{cap(k)}</option>{/each}
            </optgroup>
          {/if}
          <optgroup label={own.length ? 'Others' : 'Rooms'}>
            {#each others as k (k)}<option value={k}>{cap(k)}</option>{/each}
          </optgroup>
        </select>
      </label>
    {/if}
    {#if settings.mode === 'room' || settings.mode === 'corridor' || settings.mode === 'rock'}
      <div class="ws-field">
        Brush
        <div class="ws-seg">
          {#each [1, 2, 3] as w (w)}<button class:on={settings.width === w} onclick={() => (settings.width = w)}>{w} square{w > 1 ? 's' : ''}</button>{/each}
        </div>
      </div>
    {/if}
    {#if settings.mode === 'room' || settings.mode === 'rect' || settings.mode === 'corridor' || settings.mode === 'stairs' || settings.mode === 'wall'}
      <label class="switch"><span class="grow">Add doors where a room would be shut off</span><input type="checkbox" class="ws-switch" bind:checked={settings.autoDoors} /></label>
    {/if}
    {#if settings.mode === 'prop'}
      <div class="ws-seg" role="group" aria-label="What to put down">
        <button class:on={group === 'main'} onclick={() => (group = 'main')}>{building ? 'Furniture' : 'Props'}</button>
        {#if building}<button class:on={group === 'props'} onclick={() => (group = 'props')}>Indoor props</button>{/if}
        <button class:on={group === 'yours'} onclick={() => (group = 'yours')}>Yours</button>
      </div>
      <input class="ws-input" type="search" bind:value={propFilter} placeholder={building && group !== 'props' ? 'Find a piece…' : 'Find a prop…'} aria-label={building ? 'Find a piece of furniture' : 'Find a prop'} />
      <div class="list">
        {#each propList as p (p.kind)}
          <button class="ws-chip" class:on={settings.prop === p.kind} class:pic={!!p.asset} onclick={() => pickProp(p.kind)} title={[p.blocks ? 'blocks movement' : 'passable', p.cover ? `cover ${['', '½', '¾', 'total'][p.cover]}` : '', p.hazard ?? ''].filter(Boolean).join(' · ')}>
            {#if p.asset}{#await picture(p.asset) then src}{#if src}<img {src} alt="" />{/if}{/await}{/if}
            {p.name}{p.hazard ? ' ⚠' : ''}
          </button>
        {:else}
          <span class="ws-hint">{group === 'yours' ? 'No pictures yet: upload them in Edit › Scatter.' : 'Nothing by that name.'}</span>
        {/each}
      </div>
      <div class="ws-row size">
        <span>Size</span>
        <input class="ws-input" type="number" min="1" max="6" bind:value={settings.pw} aria-label="Across" /> ×
        <input class="ws-input" type="number" min="1" max="6" bind:value={settings.ph} aria-label="Down" />
        <button class="ws-icon-btn" title="Turn" aria-label="Turn" onclick={() => (settings = { ...settings, pw: settings.ph, ph: settings.pw })}><Icon name="reset" size={16} /></button>
      </div>
    {/if}

    {#if room && now.selected !== null}
      {@const ri = now.selected}
      <section class="card">
        <div class="ws-label">Room chosen</div>
        <div class="ws-row">
          <label class="ws-field grow">
            Kind
            <select class="ws-input" value={room.kind} onchange={(e) => designer.setRoom(ri, { kind: e.currentTarget.value })}>
              {#if !(catalog?.rooms ?? []).includes(room.kind)}<option value={room.kind}>{cap(room.kind)}</option>{/if}
              {#each [...own, ...others] as k (k)}<option value={k}>{cap(k)}</option>{/each}
            </select>
          </label>
          <label class="ws-field grow">
            Name
            <input class="ws-input" value={renames[roomId] ?? ''} placeholder={cap(room.kind)} onchange={(e) => onRename(roomId, e.currentTarget.value.trim())} />
          </label>
        </div>
        {#if !building}
          <div class="ws-field">
            Floor
            <div class="ws-seg">
              {#each [0, 5, 10] as f (f)}<button class:on={room.raise_ft === f} onclick={() => designer.setRoom(ri, { raise_ft: f })}>{f ? `Raised ${f} ft` : 'Level'}</button>{/each}
            </div>
          </div>
          <label class="switch">
            <span class="grow">The boss chamber</span>
            <input type="checkbox" class="ws-switch" checked={room.kind === BOSS} onchange={(e) => designer.setRoom(ri, { kind: e.currentTarget.checked ? BOSS : (own.find((k) => k !== BOSS && k !== theme?.passage) ?? 'chamber') })} />
          </label>
        {/if}
        <div class="ws-row wrap">
          <button class="ws-btn" onclick={() => designer.furnish(ri)} title={building ? 'Furniture for its kind of room, where it fits' : 'Props from its kind’s kit, where they fit'}>Furnish</button>
          <button class="ws-btn" onclick={() => designer.clearProps(ri)}>{building ? 'Clear furniture' : 'Clear props'}</button>
          {#if !building}<button class="ws-btn" class:danger={sure === 'room'} onclick={() => twice('room', () => designer.deleteRoom(ri))}>{sure === 'room' ? 'Fill it in?' : 'Fill in'}</button>{/if}
        </div>
      </section>
    {/if}

    <details class="ws-group" open={remembered('design.site', !!blocking)} ontoggle={(e) => remember('design.site', e.currentTarget.open)}>
      <summary>
        <span class="grow">{building ? 'Building' : 'Site'}</span>
        {#if problems.length}<span class="badge" class:blocking={blocking > 0}>{problems.length} {problems.length === 1 ? 'problem' : 'problems'}</span>{:else}<span class="ok">keeps every rule</span>{/if}
      </summary>
      <div class="ws-group-body">
        <div class="ws-row wrap">
          {#if !building}
            <button class="ws-btn" onclick={() => designer.addLevel()} disabled={(draft?.levels.length ?? 0) >= (catalog?.max_levels ?? 6)}>Add a level below</button>
            <button class="ws-btn" class:danger={sure === 'level'} onclick={() => twice('level', () => designer.removeLevel())} disabled={(draft?.levels.length ?? 0) <= 1}>{sure === 'level' ? 'Fill it in?' : 'Fill in the deepest'}</button>
          {/if}
          <button class="ws-btn" onclick={() => designer.addDoors()}>Doors where needed</button>
        </div>
        {#if building}
          <div class="ws-hint">{now.storeys} {now.storeys === 1 ? 'storey' : 'storeys'} above ground{now.storeysChanged ? ': the building follows on Save' : ''}</div>
          <div class="ws-row wrap">
            <button class="ws-btn" onclick={() => designer.setStoreys(now.storeys + 1)} disabled={now.storeys >= (catalog?.building.max_floors ?? 8)}>Add a floor on top</button>
            <button class="ws-btn" class:danger={sure === 'storey'} onclick={() => twice('storey', () => designer.setStoreys(now.storeys - 1))} disabled={now.storeys <= 1}>{sure === 'storey' ? 'Take it away?' : 'Take the top floor away'}</button>
          </div>
        {/if}
        {#if problems.length}
          <ul class="problems">
            {#each problems as p, i (i)}
              <li class:blocking={p.blocking}><button onclick={() => onProblem(p.level, p.at)}>{p.blocking ? '✕' : '!'} {cap(p.text)}</button></li>
            {/each}
          </ul>
        {/if}
      </div>
    </details>
  {/if}

  <div class="foot">
    <button class="ws-icon-btn" onclick={() => designer.undo()} disabled={!now.canUndo} title="Undo (Ctrl+Z)" aria-label="Undo"><Icon name="undo" /></button>
    <button class="ws-icon-btn" onclick={() => designer.redo()} disabled={!now.canRedo} title="Redo (Ctrl+Y)" aria-label="Redo"><Icon name="redo" /></button>
    {#if saved && !peek}<button class="ws-btn quiet" class:danger={sure === 'reset'} onclick={() => twice('reset', onReset)} title={building ? 'Drop the design: the inside as generated' : 'Drop the design: the site as generated'}>{sure === 'reset' ? 'Drop the design?' : 'As generated'}</button>{/if}
    <span class="grow"></span>
    <button class="ws-btn primary" onclick={onSave} disabled={!now.dirty || blocking > 0} title={saveWhy}>{blocking ? `Save (${blocking} to fix)` : 'Save'}</button>
  </div>
</div>

<style>
  .design {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .site {
    font-size: 14px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .wrap {
    flex-wrap: wrap;
  }
  .tools {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
  }
  .tools .ws-icon-btn {
    width: auto;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    cursor: pointer;
  }
  .list {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
    max-height: 150px;
    overflow-y: auto;
  }
  .size input {
    width: 3.4em;
  }
  .ws-chip.pic {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .ws-chip img {
    width: 18px;
    height: 18px;
    object-fit: contain;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.35);
  }
  .badge {
    font-weight: normal;
    font-size: 11px;
    color: var(--warn);
  }
  .badge.blocking {
    color: #b91c1c;
  }
  .ok {
    font-weight: normal;
    font-size: 11px;
    color: var(--ok);
  }
  .problems {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 160px;
    overflow-y: auto;
  }
  .problems button {
    all: unset;
    cursor: pointer;
    color: #92400e;
  }
  .problems .blocking button {
    color: #b91c1c;
  }
  .problems button:hover {
    text-decoration: underline;
  }
  .foot {
    position: sticky;
    bottom: -10px;
    display: flex;
    align-items: center;
    gap: 4px;
    margin: 0 -10px -10px;
    padding: 8px 10px 10px;
    background: var(--paper-solid);
    border-top: 1px solid var(--line-faint);
  }
</style>
