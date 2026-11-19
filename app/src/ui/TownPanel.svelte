<script lang="ts">
  // Edit › Streets: the town in view laid out anew by hand (the ward editor). Its patches are drawn
  // over the map; a corner dragged, or the brushes, move them (applied on release); a patch
  // picked can be given another ward, lot size, joined to a neighbour's district or put in another (or a new one by name) or laid out
  // again, a temple's kept as a close round it, a park's laid out as a formal garden or a common; a
  // corner where streets meet picked can be given a square; the town's walls put up or taken down.
  // Each change is one step to undo.
  //
  // A town laid out by hand (`streets`) shows its street graph instead: Choose (streets, junctions,
  // blocks, areas, walls; junctions dragged), Draw a street, an area painted open or paved, a wall
  // line, the same brushes on its junctions, what is picked (`StreetCards`), the problems the
  // streets have, and Back to generated. Either way a plan can be laid under it to trace (`TownTrace`).
  import { LOT_SIZES, PARK_KINDS, SQUARE_KINDS, TOWN_WARDS, type LotSize, type ParkKind, type SquareKind, type TownCorner, type TownPatch, type TownPlan, type TownRequest, type TownWallsRequest, type TownWard } from '../gen/protocol';
  import type { OpenKind, StreetClass, StreetPlan, StreetRequest } from '../gen/protocol';
  import StreetCards from './StreetCards.svelte';
  import TownTrace from './TownTrace.svelte';
  import { SQUARE_COLOURS, WARD_COLOURS, type TownMode } from '../editor/town';
  import { HAND_BRUSH_FT, isDrawMode, type StreetPick } from '../editor/streets';
  import Icon from './Icon.svelte';
  import MoreFold from './shell/MoreFold.svelte';
  import type { IconName } from './icons';

  interface Props {
    plan: TownPlan | null;
    /** The town's name; null: no town in view. */
    name: string | null;
    /** Why there is no plan (loading, a village…). */
    note: string;
    mode: TownMode;
    /** The brush's size, in patch widths. */
    size: number;
    selected: TownPatch | null;
    /** The corner picked (its square), if any. */
    corner: TownCorner | null;
    /** Waiting for a neighbour to be picked to join. */
    merging: boolean;
    /** A change is being made. */
    busy: boolean;
    /** Close enough to see the patches. */
    near: boolean;
    /** Why the last gate or tower asked for was refused. */
    wallWhy?: string;
    peek?: boolean;
    onMode: (m: TownMode) => void;
    onChange: (req: TownRequest, what: string) => void;
    onMerge: () => void;
    onZoomIn: () => void;
    /** "Change town" is waiting for a click on the map. */
    picking?: boolean;
    onChangeTown: () => void;
    /** District names by id (renames) and the ids whose labels are hidden. */
    names?: Record<string, string>;
    hiddenIds?: string[];
    onRename?: (id: string, name: string) => void;
    onHide?: (id: string, hide: boolean) => void;
    /** Pick a patch (to show where a district lies). */
    onPatch?: (patch: number) => void;
    /** The town's street graph, when it is laid out by hand (then `plan` is null). */
    streets?: StreetPlan | null;
    streetPick?: StreetPick | null;
    /** Points of the street being drawn. */
    streetDrawn?: number;
    streetClass?: 'main' | 'street';
    /** What an area painted is; a wall drawn's towers apart (ft; null: as the town has them) and name. */
    streetOpen?: OpenKind;
    wallSpacing?: number | null;
    wallName?: string;
    onLayOut?: () => void;
    onBack?: () => void;
    onStreet?: (req: StreetRequest, what: string) => void;
    onStreets?: (reqs: StreetRequest[], what: string) => void;
    /** End what is being drawn (`closed`: a wall closed round). */
    onStreetFinish?: (closed?: boolean) => void;
    onStreetBack?: () => void;
    onStreetPick?: (p: StreetPick | null) => void;
    /** which tool the panel is, Streets (no walls) or Town wall (the walls and their
     * corners only); unset, all of it. */
    part?: 'streets' | 'walls' | null;
  }

  let { plan, name, note, mode, size = $bindable(), selected, corner, merging, busy, near, wallWhy = '', peek = false, onMode, onChange, onMerge, onZoomIn, picking = false, onChangeTown, names = {}, hiddenIds = [], onRename, onHide, onPatch, streets = null, streetPick = null, streetDrawn = 0, streetClass = $bindable('street'), streetOpen = $bindable('lawn'), wallSpacing = $bindable(null), wallName = $bindable(''), onLayOut, onBack, onStreet, onStreets, onStreetFinish, onStreetBack, onStreetPick, part = null }: Props = $props();
  const walls_ = $derived(part !== 'streets');
  const streets_ = $derived(part !== 'walls');

  const MODES: { key: TownMode; label: string; was?: string; icon: IconName; kbd: string; hint: string }[] = [
    { key: 'select', label: 'Choose', icon: 'pointer', kbd: 'V', hint: 'Click a patch to set its ward, a ringed corner for a square where streets meet; drag a corner to move it.' },
    { key: 'displace', label: 'Push', was: 'Displace', icon: 'move', kbd: 'Q', hint: 'Drag: the corners in the ring go with you, those in its middle all the way.' },
    { key: 'liquify', label: 'Smudge', was: 'Liquify', icon: 'liquify', kbd: 'L', hint: 'Drag: the corners under the ring are smudged along.' },
    { key: 'bloat', label: 'Grow', was: 'Bloat', icon: 'bloat', kbd: 'O', hint: 'Press and drag: the patches round where you pressed swell, the further you drag the more.' },
    { key: 'pinch', label: 'Shrink', was: 'Pinch', icon: 'pinch', kbd: 'I', hint: 'Press and drag: the patches round where you pressed shrink, the further you drag the more.' },
    { key: 'relax', label: 'Soften', was: 'Relax', icon: 'relax', kbd: 'R', hint: 'Scrub over the town: each corner eases toward the middle of its neighbours.' },
    { key: 'equalize', label: 'Even out', was: 'Equalize', icon: 'polygon', kbd: 'E', hint: 'Click or drag over patches: each is made more regular.' },
  ];

  const WARD_LABEL: Record<TownWard, string> = {
    plaza: 'Market square',
    castle: 'Castle',
    temple: 'Temple',
    merchant: 'Merchants',
    craft: 'Craftsmen',
    noble: 'Nobles',
    common: 'Common folk',
    slum: 'Slums',
    docks: 'Docks',
    military: 'Barracks',
    farm: 'Farms',
    park: 'Park',
    empty: 'Open ground',
  };
  const LOT_LABEL: Record<LotSize, string> = { small: 'Small', medium: 'Medium', large: 'Large', huge: 'Huge' };
  const PARK_LABEL: Record<ParkKind, string> = { formal: 'Formal garden', common: 'Common', cemetery: 'Cemetery', horses: 'Horse market' };
  // (On the buttons: five side by side fit the panel only short.)
  const PARK_SHORT: Record<ParkKind, string> = { formal: 'Formal', common: 'Common', cemetery: 'Cemetery', horses: 'Horses' };
  const PARK_HINT: Record<ParkKind, string> = {
    formal: 'cross walks, a fountain, hedged beds, statues and topiary',
    common: 'winding paths, a pond, trees, a gazebo or a bandstand',
    cemetery: "graves in rows, tombs and a lychgate by the town's culture",
    horses: 'fenced corrals of horses, a trodden market ground, hitching rails, troughs and carts',
  };
  const SQUARE_LABEL: Record<SquareKind | 'none', string> = {
    crossroads: 'Widened crossroads',
    well: 'Well square',
    fountain: 'Fountain square',
    green: 'Small green',
    none: 'No square',
  };
  // The wall at the picked corner (R5 towns, M18c): a gate, towers, and the hand towers touching it.
  const byHand = $derived(plan?.walls.by_hand);
  const onWall = $derived(!!corner?.wall && !!plan?.walls.built);
  // Map north is up, and world y grows southward (as drawn).
  const compass = (dx: number, dy: number) => ['east', 'south-east', 'south', 'south-west', 'west', 'north-west', 'north', 'north-east'][((Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) % 8) + 8) % 8];
  const sides = $derived.by(() => {
    if (!plan || !corner?.wall_next) return [];
    return corner.wall_next.flatMap((w) => {
      const o = w === null ? undefined : plan.corners.find((c) => c.corner === w);
      return o ? [{ corner: o.corner, dir: compass(o.at[0] - corner.at[0], o.at[1] - corner.at[1]), ft: Math.hypot(o.at[0] - corner.at[0], o.at[1] - corner.at[1]) }] : [];
    });
  });
  const handTowers = $derived(byHand && corner ? byHand.towers_added.filter((t) => t.corners.includes(corner.corner)) : []);
  const gateRefused = $derived(byHand?.gates_added.find((g) => corner && g.corner === corner.corner)?.refused);
  const walls = (w: TownWallsRequest, what: string) => onChange({ walls: w }, what);
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  const ft = (v: number) => `${Math.round(v).toLocaleString('en')} ft`;

  // (On a hand town: its junctions, not the patches' corners.)
  const HAND_HINT: Partial<Record<TownMode, string>> = {
    select: 'Click a street, a junction, a block, an area or a wall to pick it; drag a junction to move it.',
    street: 'Click or tap each point: a point near a junction or a street goes onto it, and the street ends there (or click the last point again). Crossing a block splits it.',
    area: 'Click or tap each corner of a part of a block to make open ground or paving; end on the first corner again (or Enter).',
    wall: "The town's own wall: click or tap each bend; end on the last point again (an open wall) or on the first (closed round). Gates open where streets cross it.",
    displace: 'Drag: the junctions in the ring go with you, those in its middle all the way.',
    liquify: 'Drag: the junctions under the ring are smudged along.',
    bloat: 'Press and drag: the blocks round where you pressed swell, the further you drag the more.',
    pinch: 'Press and drag: the blocks round where you pressed shrink, the further you drag the more.',
    relax: 'Scrub over the town: each junction eases toward the middle of its neighbours.',
    equalize: 'Click or drag over blocks: each is made more regular.',
  };
  const CLASS_LABEL: Record<StreetClass, string> = { main: 'Main street', street: 'Street', quay: 'Quay', joint: 'Lane' };
  const OPEN_LABEL: Record<OpenKind, string> = { plaza: 'Paved', lawn: 'Lawn', park: 'Park', garden: 'Garden', market: 'Market', cemetery: 'Cemetery', farmland: 'Farmland' };
  const DRAW_CLASSES = ['street', 'main'] as const;
  const DRAWS: { key: TownMode; label: string; icon: IconName; kbd: string }[] = [
    { key: 'street', label: 'Street', icon: 'road', kbd: 'N' },
    { key: 'area', label: 'Area', icon: 'polygon', kbd: 'A' },
    { key: 'wall', label: 'Town wall', icon: 'wall', kbd: 'W' },
  ];
  const drawing = $derived(DRAWS.find((d) => d.key === mode));
  let drawOpen = $state(false);
  const hand = $derived(!!streets);
  /** A problem's record, as a pick. */
  const problemPick = (key: string): StreetPick | null => {
    const m = /\/([nek])(\d+)$/.exec(key);
    return m ? { kind: m[1] === 'e' ? 'street' : m[1] === 'k' ? 'block' : 'junction', id: Number(m[2]) } : null;
  };
  const problemWhat = (key: string) => {
    const p = problemPick(key);
    return p ? `${p.kind === 'street' ? 'Street' : p.kind === 'block' ? 'Block' : 'Junction'} ${p.id}` : 'The town';
  };

  const choose = MODES[0];
  const reshape = MODES.slice(1);
  const reshaping = $derived(reshape.find((m) => m.key === mode));
  let reshapeOpen = $state(false);
  let resetOpen = $state(false);
  // (As the generator built them, when the plan says: gate towers apart.)
  const gateCount = $derived(plan?.walls.gates ?? plan?.corners.filter((c) => c.gate).length ?? 0);
  const towerCount = $derived(plan ? (plan.walls.towers ?? plan.corners.filter((c) => c.wall && !c.tower_removed).length + (plan.walls.by_hand?.towers_added.length ?? 0)) : 0);
  const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : one.endsWith('ch') ? 'es' : 's'}`;
  let renaming = $state<string | null>(null);
  let renameTo = $state('');
  const districtName = (d: { id: string; name: string }) => names[d.id] ?? d.name;
  function saveRename() {
    if (renaming !== null) onRename?.(renaming, renameTo.trim().slice(0, 80));
    renaming = null;
  }
  const hint = $derived(
    part === 'walls' && !hand && mode === 'select'
      ? 'Click a ringed corner on the wall to add or take away a gate or a tower there.'
      : ((hand ? HAND_HINT[mode] : undefined) ?? MODES.find((m) => m.key === mode)?.hint ?? ''),
  );
  const brush = $derived(mode !== 'select' && mode !== 'equalize' && !isDrawMode(mode));
  const generatedWard = $derived(selected ? (selected.ward_generated ?? selected.ward) : 'empty');
  const set = (req: TownRequest['patches'], what: string) => selected && onChange({ patches: req }, what);
  let naming = $state(false);
  let newName = $state('');
  // (A patch outside the walls joins a district once it is built on: open country made a slum or common ward, 184.)
  const districtable = $derived(!!selected && (selected.in_town || !!selected.district || (selected.ward !== 'farm' && selected.ward !== 'empty')));
  const districtList = $derived((plan?.districts ?? []).filter((d) => d.patches.length > 0 || d.id === selected?.district?.id));
  function pickDistrict(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    const v = el.value;
    if (!selected) return;
    if (v === '@new') {
      naming = true;
      el.value = selected.district?.id ?? '';
      return;
    }
    naming = false;
    set([{ patch: selected.patch, district: v === '@auto' ? 'auto' : v }], 'district');
  }
  function makeDistrict() {
    const n = newName.trim().slice(0, 80);
    if (!n || !selected) return;
    set([{ patch: selected.patch, district: n }], 'district');
    naming = false;
    newName = '';
  }
  // (A different patch picked: the name box closes.)
  $effect(() => {
    selected?.patch;
    naming = false;
  });
  const generatedSquare = $derived(corner ? (corner.square_generated ?? corner.square ?? 'none') : 'none');
</script>

<div class="town">
  {#if !peek}
    <div class="head">
      <span class="ws-grow">{#if name}<b>{name}</b>{:else}<span class="ws-muted">No town in view</span>{/if}{#if plan?.edited}<span class="ws-muted"> · laid out by hand</span>{/if}</span>
      <button class="ws-btn quiet" class:on={picking} aria-pressed={picking} onclick={onChangeTown} title="Then click a town on the map">{picking ? 'Click a town…' : 'Change town'}</button>
    </div>
  {/if}
  <div class="tools">
    <div class="ws-seg modes" role="radiogroup" aria-label="Tool">
      <button class:on={mode === choose.key || (!hand && isDrawMode(mode))} aria-pressed={mode === choose.key} onclick={() => ((reshapeOpen = drawOpen = false), onMode(choose.key))} title="{choose.label} ({choose.kbd})"><Icon name={choose.icon} size={16} />{choose.label}</button>
      {#if hand}
        <button class:on={!!drawing} aria-pressed={!!drawing} aria-haspopup="menu" aria-expanded={drawOpen} onclick={() => ((reshapeOpen = false), (drawOpen = !drawOpen))} title="Draw a street, paint an area, or draw the town's wall"><Icon name={drawing?.icon ?? 'road'} size={16} />{drawing?.label ?? 'Draw'} ▾</button>
      {/if}
      {#if streets_}
      <button class:on={!!reshaping} aria-pressed={!!reshaping} aria-haspopup="menu" aria-expanded={reshapeOpen} onclick={() => ((drawOpen = false), (reshapeOpen = !reshapeOpen))} title={hand ? "Brushes that move the town's junctions" : "Brushes that reshape the town's patches"}>
        <Icon name={reshaping?.icon ?? 'liquify'} size={16} />{reshaping?.label ?? 'Reshape'} ▾
      </button>
      {/if}
    </div>
    {#if drawOpen && hand}
      <div class="menu ws-panel" role="menu" aria-label="Draw">
        {#each DRAWS.filter((d) => (part === 'walls' ? d.key === 'wall' : part === 'streets' ? d.key !== 'wall' : true)) as m (m.key)}
          <button class="ws-btn quiet" class:on={mode === m.key} role="menuitem" onclick={() => ((drawOpen = false), onMode(m.key))}><Icon name={m.icon} size={16} /><span class="ws-grow">{m.label}</span><kbd>{m.kbd}</kbd></button>
        {/each}
      </div>
    {/if}
    {#if reshapeOpen}
      <div class="menu ws-panel" role="menu" aria-label="Reshape">
        {#each reshape as m (m.key)}
          <button class="ws-btn quiet" class:on={mode === m.key} role="menuitem" onclick={() => ((reshapeOpen = false), onMode(m.key))} title={m.was ? `Was ${m.was}` : undefined}><Icon name={m.icon} size={16} /><span class="ws-grow">{m.label}</span><kbd>{m.kbd}</kbd></button>
        {/each}
      </div>
    {/if}
  </div>
  {#if !plan && !streets}
    <div class="zoom">
      <span class="ws-grow">{note}</span>
      {#if name && !near}<button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>{/if}
    </div>
  {:else if !near}
    <div class="zoom">
      <span class="ws-grow">Zoom in to see its {hand ? 'streets' : 'patches'}.</span>
      <button class="ws-btn" onclick={onZoomIn}><Icon name="plus" size={16} /> Zoom in</button>
    </div>
  {/if}
  {#if streets && isDrawMode(mode)}
    <div class="row draw">
      {#if mode === 'street'}
        <div class="ws-seg" role="radiogroup" aria-label="Street">
          {#each DRAW_CLASSES as c (c)}
            <button class:on={streetClass === c} aria-pressed={streetClass === c} onclick={() => (streetClass = c)} title="Draw a {CLASS_LABEL[c].toLowerCase()}">{c === 'main' ? 'Main' : 'Street'}</button>
          {/each}
        </div>
      {:else if mode === 'area'}
        <select class="ws-input" bind:value={streetOpen} aria-label="Area">
          {#each Object.keys(OPEN_LABEL) as k (k)}<option value={k}>{OPEN_LABEL[k as OpenKind]}</option>{/each}
        </select>
      {/if}
      <span class="ws-grow ws-muted">{streetDrawn ? `${plural(streetDrawn, mode === 'area' ? 'corner' : 'point')}` : ''}</span>
      <button class="ws-btn" disabled={!streetDrawn} onclick={onStreetBack} title="Take the last point back (Backspace)"><Icon name="undo" size={16} /> Back</button>
      {#if mode === 'wall'}
        <button class="ws-btn" disabled={busy || streetDrawn < 3} onclick={() => onStreetFinish?.(true)} title="Close the wall round to its first point"><Icon name="polygon" size={16} /> Close</button>
      {/if}
      <button class="ws-btn primary" disabled={busy || streetDrawn < (mode === 'area' ? 3 : 2)} onclick={() => onStreetFinish?.(mode === 'area')} title="End it here (Enter)"><Icon name="check" size={16} /> End</button>
    </div>
    {#if mode === 'wall'}
      <div class="nums">
        <label class="ws-field">Name<input class="ws-input" type="text" maxlength="80" placeholder="None" bind:value={wallName} /></label>
        <label class="ws-field">Towers apart (ft)<input class="ws-input" type="number" min="40" max="1000" step="10" placeholder="As the town" value={wallSpacing ?? ''} onchange={(e) => { const v = e.currentTarget.valueAsNumber; wallSpacing = Number.isFinite(v) && v > 0 ? v : null; }} /></label>
      </div>
    {/if}
  {/if}
  {#if !peek && streets}
    <div class="ws-hint">{hint}{mode === 'select' || mode === 'street' ? '' : ' Applied when you let go.'}</div>
    {#if brush}
      <label class="ws-slider">
        <span>Brush</span><output>{ft(2 * size * HAND_BRUSH_FT)} across</output>
        <input type="range" min="0.25" max="4" step="0.05" bind:value={size} />
      </label>
    {/if}
    {#if streets_}
    <section class="card" aria-label="Streets">
      <div class="title"><span class="ws-grow"><b>Laid out by hand</b><span class="ws-muted"> · {plural(streets.streets.length, 'street')}, {plural(streets.junctions.length, 'junction')}, {plural(streets.blocks.length, 'block')}</span></span></div>
      {#if streets.problems.length}
        <div class="problems" aria-label="Problems">
          {#each streets.problems.slice(0, 12) as p (p.key + p.why)}
            {@const pk = problemPick(p.key)}
            <button class="ws-btn quiet bad" disabled={!pk} onclick={() => pk && onStreetPick?.(pk)}><Icon name="target" size={16} /><span class="ws-grow">{problemWhat(p.key)}: {p.why}</span></button>
          {/each}
          {#if streets.problems.length > 12}<span class="ws-muted">and {streets.problems.length - 12} more</span>{/if}
        </div>
      {:else}
        <span class="ws-muted">No problems: every street meets the others and every block has its streets round it.</span>
      {/if}
    </section>
    {/if}
    <StreetCards {streets} pick={streetPick} {busy} {names} onStreet={(r, w) => onStreet?.(r, w)} onStreets={(rs, w) => onStreets?.(rs, w)} onPick={(p) => onStreetPick?.(p)} />
    {#snippet handMore()}
    <TownTrace />
    <div class="more">
      <button class="ws-btn quiet" disabled={busy} onclick={onBack} title="Its streets as generated again"><Icon name="reset" size={16} /> Back to generated</button>
    </div>
    {/snippet}
    {#if !part}
      {@render handMore()}
    {:else if part === 'streets'}
      <MoreFold tool="streets">{@render handMore()}</MoreFold>
    {/if}
  {/if}
  {#if !peek && plan}
    {#if brush || mode === 'equalize' || !selected}
      <div class="ws-hint">{hint}{mode === 'select' ? '' : ' Applied when you let go.'}</div>
    {/if}
    {#if brush}
      <label class="ws-slider">
        <span>Brush</span><output>{ft(2 * size * plan.max_move_ft)} across</output>
        <input type="range" min="0.25" max="4" step="0.05" bind:value={size} />
      </label>
    {/if}
    {#if walls_}
    <section class="card" aria-label="Walls">
      <label class="ws-switchrow">
        <span class="ws-grow"><b>Walls</b><span class="ws-muted"> · {plan.walls.generated ? 'it was built with walls' : 'it was built without'}</span></span>
        <input type="checkbox" class="ws-switch" checked={plan.walls.built} disabled={busy} onchange={(e) => onChange({ walls: (e.currentTarget as HTMLInputElement).checked }, 'walls')} />
      </label>
      {#if plan.walls.built}
        <div>{plural(gateCount, 'gate')}, {plural(towerCount, 'tower')}</div>
        <div class="ws-hint">Choose, then click a ringed corner on the wall to add or take away a gate or a tower.</div>
      {/if}
    </section>
    {/if}
    {#snippet districts()}
    {#if plan.districts?.length}
      <section class="card" aria-label="Districts">
        <div class="title"><span class="ws-grow"><b>Districts</b></span></div>
        <div class="dists">
        {#each districtList as d (d.id)}
          <div class="dist">
            {#if renaming === d.id}
              <input class="ws-input ws-grow" type="text" maxlength="80" bind:value={renameTo} aria-label="District name" onkeydown={(e) => (e.key === 'Enter' ? saveRename() : e.key === 'Escape' && (renaming = null))} />
              <button class="ws-btn" onclick={saveRename}>Save</button>
            {:else}
              <button class="ws-btn quiet ws-grow name" class:dim={hiddenIds.includes(d.id)} title="Pick its first patch" onclick={() => d.patches[0] !== undefined && onPatch?.(d.patches[0])}>{districtName(d) || 'Unnamed'}<span class="ws-muted"> · {d.kind}, {plural(d.patches.length, 'patch')}</span></button>
              <button class="ws-icon-btn" aria-label="Rename {districtName(d)}" title="Rename" onclick={() => ((renaming = d.id), (renameTo = districtName(d)))}><Icon name="pencil" size={16} /></button>
              <button class="ws-icon-btn" aria-label={hiddenIds.includes(d.id) ? 'Show on the map' : 'Hide from the map'} title={hiddenIds.includes(d.id) ? 'Show its name on the map' : 'Hide its name from the map'} onclick={() => onHide?.(d.id, !hiddenIds.includes(d.id))}><Icon name={hiddenIds.includes(d.id) ? 'eye-off' : 'eye'} size={16} /></button>
            {/if}
          </div>
        {/each}
        </div>
        <div class="row">
          <button class="ws-btn" disabled={busy || !districtable} title={districtable ? 'A district of its own, taking the picked patch' : selected ? 'Open country is in no district: give it a ward built on first' : 'Pick a patch first'} onclick={() => (naming = true)}><Icon name="plus" size={16} /> New district</button>
        </div>
        {#if naming && selected}
          <div class="row">
            <input class="ws-input ws-grow" type="text" minlength="1" maxlength="80" placeholder="Name (an existing name joins it)" bind:value={newName} disabled={busy} onkeydown={(e) => e.key === 'Enter' && makeDistrict()} />
            <button class="ws-btn" disabled={busy || !newName.trim()} onclick={makeDistrict}>Make</button>
            <button class="ws-btn" disabled={busy} onclick={() => (naming = false)}>Cancel</button>
          </div>
        {/if}
      </section>
    {/if}
    {/snippet}
    {#if selected && streets_}
      <section class="card" aria-label="Patch">
        <div class="title">
          <span class="dot" style:background={hex(WARD_COLOURS[selected.ward])}></span>
          <span class="ws-grow"><b>{selected.district?.name ?? (selected.in_town ? 'A patch of the town' : 'Outside the town')}</b></span>
        </div>
        <label class="ws-field">
          Ward
          <select class="ws-input" value={selected.ward} disabled={busy} onchange={(e) => set([{ patch: selected.patch, ward: (e.currentTarget as HTMLSelectElement).value as TownWard }], 'ward')}>
            {#each TOWN_WARDS as w (w)}
              <option value={w} disabled={w === 'park' && !!selected.park_refused} title={w === 'park' ? selected.park_refused : undefined}>{WARD_LABEL[w]}{w === generatedWard ? ' (as generated)' : ''}{w === 'park' && selected.park_refused ? ' (not here)' : ''}</option>
            {/each}
          </select>
        </label>
        {#if districtable && plan.districts?.length}
          <div class="ws-field">
            <label for="town-district">District</label>
            <select id="town-district" class="ws-input" value={selected.district?.id ?? ''} disabled={busy} onchange={pickDistrict}>
              {#if !selected.district}<option value="" disabled>None</option>{/if}
              {#each districtList as d (d.id)}
                <option value={d.id}>{d.name || 'Unnamed'} ({d.kind}){d.by_hand ? ' · by hand' : ''}</option>
              {/each}
              <option value="@auto">As generated</option>
              <option value="@new">New district…</option>
            </select>
            {#if naming}
              <div class="row">
                <input class="ws-input ws-grow" type="text" minlength="1" maxlength="80" placeholder="Name (an existing name joins it)" bind:value={newName} disabled={busy} onkeydown={(e) => e.key === 'Enter' && makeDistrict()} />
                <button class="ws-btn" disabled={busy || !newName.trim()} onclick={makeDistrict}>Make</button>
                <button class="ws-btn" disabled={busy} onclick={() => (naming = false)}>Cancel</button>
              </div>
            {/if}
          </div>
        {/if}
        {#if selected.ward === 'park'}
          <div class="ws-field">
            Laid out as
            <div class="ws-seg" role="radiogroup" aria-label="Laid out as">
              <button class:on={!selected.park_by_hand} aria-pressed={!selected.park_by_hand} disabled={busy} onclick={() => set([{ patch: selected.patch, park: 'auto' }], 'park')} title="As the town has it: formal by the castle and the nobles, a common elsewhere">Auto</button>
              {#each PARK_KINDS as k (k)}
                <button class:on={!!selected.park_by_hand && selected.park === k} aria-pressed={!!selected.park_by_hand && selected.park === k} disabled={busy} onclick={() => set([{ patch: selected.patch, park: k }], 'park')} title={PARK_LABEL[k]}>{PARK_SHORT[k]}</button>
              {/each}
            </div>
            {#if selected.park}<span class="ws-muted">{PARK_LABEL[selected.park]}: {PARK_HINT[selected.park]}</span>{/if}
          </div>
        {:else}
          <div class="ws-field">
            Lots
            <div class="ws-seg" role="radiogroup" aria-label="Lots">
              <button class:on={!selected.lots} aria-pressed={!selected.lots} disabled={busy} onclick={() => set([{ patch: selected.patch, lots: 'auto' }], 'lots')} title="As its ward has them">Auto</button>
              {#each LOT_SIZES as l (l)}
                <button class:on={selected.lots === l} aria-pressed={selected.lots === l} disabled={busy} onclick={() => set([{ patch: selected.patch, lots: l }], 'lots')}>{LOT_LABEL[l]}</button>
              {/each}
            </div>
          </div>
        {/if}
        <div class="row">
          {#if selected.merged_with !== undefined}
            <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, merge_with: 'none' }], 'merge')} title="Its own district again">Leave the district</button>
          {:else}
            <button class="ws-btn" class:on={merging} aria-pressed={merging} disabled={busy} onclick={onMerge} title="Then click a patch next to it: this one takes its ward and district">
              {merging ? 'Click a neighbour…' : 'Join a neighbour'}
            </button>
          {/if}
          <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, reroll: true }], 'reroll')} title="Its streets and buildings laid out another way"><Icon name="dice" size={16} /> Lay out again</button>
          <button class="ws-btn" disabled={busy} onclick={() => set([{ patch: selected.patch, as_generated: true }], 'patch')} title="Ward, lots, district and layout as generated"><Icon name="reset" size={16} /> As generated</button>
        </div>
        {#if selected.ward === 'temple'}
          <label class="ws-switchrow" title={selected.close ? undefined : selected.close_refused}>
            <span class="ws-grow">Close round the temple<span class="ws-muted"> · {selected.close || !selected.close_refused ? 'open ground round the temple and its hedged churchyard, the houses round it kept' : selected.close_refused}</span></span>
            <input type="checkbox" class="ws-switch" checked={!!selected.close} disabled={busy || (!selected.close && !!selected.close_refused)} onchange={(e) => set([{ patch: selected.patch, close: (e.currentTarget as HTMLInputElement).checked }], 'close')} />
          </label>
        {/if}
      </section>
    {/if}
    {#if corner && (streets_ || onWall)}
      <section class="card" aria-label="Corner">
        <div class="title">
          <span class="dot" style:background={corner.square ? hex(SQUARE_COLOURS[corner.square]) : 'transparent'}></span>
          <span class="ws-grow"><b>{corner.square ? SQUARE_LABEL[corner.square] : corner.junction ? 'Where streets meet' : 'A corner'}</b></span>
        </div>
        {#if !streets_}
          <!-- (Town wall: only the wall's gate and towers.) -->
        {:else if corner.junction || corner.square}
          <label class="ws-field">
            Square
            <select class="ws-input" value={corner.square ?? 'none'} disabled={busy} onchange={(e) => onChange({ squares: [{ corner: corner.corner, kind: (e.currentTarget as HTMLSelectElement).value as SquareKind | 'none' }] }, 'square')}>
              {#each [...SQUARE_KINDS, 'none' as const] as k (k)}
                <option value={k}>{SQUARE_LABEL[k]}{k === generatedSquare ? ' (as generated)' : ''}</option>
              {/each}
            </select>
          </label>
          {#if corner.square_generated !== undefined}
            <div class="row">
              <button class="ws-btn" disabled={busy} onclick={() => onChange({ squares: [{ corner: corner.corner, kind: 'auto' }] }, 'square')}><Icon name="reset" size={16} /> As generated</button>
            </div>
          {/if}
        {:else if !onWall}
          <div class="ws-hint">No square opens here: {corner.square_refused ?? 'it needs three streets or more meeting between built-up patches, inside the walls and off the water'}.</div>
        {/if}
        {#if onWall && walls_}
          <div class="ws-hint"><b>Wall here</b>{corner.gate_by_hand ? ' · a gate put in by hand' : corner.gate_removed ? ' · its gate taken out' : corner.gate ? ' · a gate' : ''}</div>
          <div class="row">
            {#if corner.gate}
              <button class="ws-btn" disabled={busy} onclick={() => walls({ gates_remove: [corner.corner] }, 'gate')}>Remove gate</button>
            {:else}
              <button class="ws-btn" disabled={busy} onclick={() => walls({ gates_add: [corner.corner] }, 'gate')}>Gate here</button>
            {/if}
            {#if !corner.tower_removed}
              <button class="ws-btn" disabled={busy} onclick={() => walls({ towers_remove: [corner.corner] }, 'tower')}>Remove tower</button>
            {:else}
              <button class="ws-btn" disabled={busy} onclick={() => walls({ towers_add: [corner.corner] }, 'tower')}>Tower back</button>
            {/if}
          </div>
          <div class="row">
            {#each sides as s (s.corner)}
              <button class="ws-btn" disabled={busy} title="A tower midway along the wall to corner {s.corner}" onclick={() => walls({ towers_add: [[corner.corner, s.corner, 0.5]] }, 'tower')}>Tower to the {s.dir}, {ft(s.ft)}</button>
            {/each}
          </div>
          {#each handTowers as t (t.id)}
            <div class="row">
              <span class="ws-grow ws-muted">Hand tower {t.id.split(':')[2]} on {t.corners.join('–')}</span>
              <button class="ws-btn" disabled={busy} onclick={() => walls({ towers_remove: [t.id] }, 'tower')}>Remove</button>
            </div>
          {/each}
          {#if gateRefused}
            <div class="ws-hint">The gate set here isn't built: {gateRefused}.</div>
          {/if}
          {#if wallWhy}
            <div class="ws-hint">{wallWhy}</div>
          {/if}
        {/if}
      </section>
    {/if}
    {#if part}
      {#if plan.set_aside.length}
        <div class="aside">Set aside since the world changed: {plan.set_aside.join('; ')}.</div>
      {/if}
    {/if}
    {#snippet planMore()}
    <section class="card" aria-label="Lay out by hand">
      <div class="row">
        <span class="ws-grow"><b>Streets by hand</b><span class="ws-muted"> · draw, split, join and move its streets one by one; its blocks follow them</span></span>
        <button class="ws-btn" disabled={busy} onclick={onLayOut} title="Its streets as they are now, to change by hand"><Icon name="road" size={16} /> Lay out by hand</button>
      </div>
    </section>
    {#if !part}
      {#if plan.set_aside.length}
        <div class="aside">Set aside since the world changed: {plan.set_aside.join('; ')}.</div>
      {/if}
    {/if}
    {#if plan.edited}
      <div class="more">
        <button class="ws-btn quiet" aria-haspopup="menu" aria-expanded={resetOpen} onclick={() => (resetOpen = !resetOpen)}><Icon name="more" size={16} /> Reset</button>
        {#if resetOpen}
          <div class="menu ws-panel up" role="menu" aria-label="Reset">
            <button class="ws-btn quiet" role="menuitem" disabled={busy} onclick={() => ((resetOpen = false), onChange({ reset: 'corners' }, 'corners'))}>Corners as generated</button>
            <button class="ws-btn quiet" role="menuitem" disabled={busy} onclick={() => ((resetOpen = false), onChange({ reset: 'patches' }, 'wards'))}>Wards as generated</button>
            {#if plan.corners.some((c) => c.square_generated !== undefined)}
              <button class="ws-btn quiet" role="menuitem" disabled={busy} onclick={() => ((resetOpen = false), onChange({ reset: 'squares' }, 'squares'))}>Squares as generated</button>
            {/if}
            <button class="ws-btn quiet" role="menuitem" disabled={busy} onclick={() => ((resetOpen = false), onChange({ reset: 'all' }, 'all'))}><Icon name="reset" size={16} /> All as generated</button>
          </div>
        {/if}
      </div>
    {/if}
    <TownTrace />
    {/snippet}
    {#if !part}
      {@render planMore()}
    {:else if part === 'streets'}
      <MoreFold tool="streets">
        {@render districts()}
        {@render planMore()}
      </MoreFold>
    {/if}
  {/if}
</div>

<style>
  .town {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  .head,
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 4px 4px 8px;
    border-radius: var(--radius-sm);
    background: #efe2c2;
  }
  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
  }
  .dot {
    width: 14px;
    height: 14px;
    border: 1px solid var(--line);
    border-radius: 50%;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .row .on {
    outline: 2px solid var(--ink);
  }
  .aside {
    color: var(--warn);
    font-size: 12px;
  }
  .tools,
  .more {
    position: relative;
  }
  .menu {
    position: absolute;
    left: 0;
    top: calc(100% + 2px);
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    min-width: 200px;
    background: var(--paper-solid);
    box-shadow: var(--shadow-lg);
  }
  .menu.up {
    top: auto;
    bottom: calc(100% + 2px);
  }
  .menu .ws-btn {
    justify-content: flex-start;
    text-align: left;
  }
  .menu kbd {
    font: 11px var(--mono);
    color: var(--ink-3);
  }
  .menu .on {
    outline: 2px solid var(--ink);
  }
  .dists {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 190px;
    overflow-y: auto;
  }
  .dist {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .dist .name {
    justify-content: flex-start;
    text-align: left;
  }
  .dist .dim {
    opacity: 0.55;
  }
  .problems {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 160px;
    overflow-y: auto;
  }
  .problems .ws-btn {
    justify-content: flex-start;
    text-align: left;
  }
  .bad {
    color: var(--warn);
  }
  .draw {
    align-items: center;
  }
  .nums {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
</style>
