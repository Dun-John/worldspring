<script lang="ts">
  // Edit › Streets on a town laid out by hand: the card of what is picked on the map. A street: its
  // class, width and setbacks, the rows of houses along it (added a side or both at a time, taken
  // away); a junction: the square there; a block: how it is filled (lots of a ward, left for
  // buildings drawn by hand, or open ground) and its district; a painted area or a wall: what it is,
  // and taking it away. Each change is one step to undo.
  import { LOT_SIZES, type BlockFill, type LotSize, type OpenKind, type StreetClass, type StreetPlan, type StreetRequest, type TownWard } from '../gen/protocol';
  import { SQUARE_KINDS, type SquareKind } from '../gen/protocol';
  import { OPEN_COLOURS, fillColour, type StreetPick } from '../editor/streets';
  import { WARD_COLOURS } from '../editor/town';
  import Icon from './Icon.svelte';

  interface Props {
    streets: StreetPlan;
    pick: StreetPick | null;
    busy: boolean;
    /** District names by name (renames). */
    names?: Record<string, string>;
    onStreet: (req: StreetRequest, what: string) => void;
    /** Several changes in turn (each its own step). */
    onStreets: (reqs: StreetRequest[], what: string) => void;
    onPick: (p: StreetPick | null) => void;
  }
  let { streets, pick, busy, names = {}, onStreet, onStreets, onPick }: Props = $props();

  const CLASS_LABEL: Record<StreetClass, string> = { main: 'Main street', street: 'Street', quay: 'Quay', joint: 'Lane' };
  const CLASSES: StreetClass[] = ['main', 'street', 'joint', 'quay'];
  const CLASS_SHORT: Record<StreetClass, string> = { main: 'Main', street: 'Street', joint: 'Lane', quay: 'Quay' };
  const OPEN_LABEL: Record<OpenKind, string> = { plaza: 'Paved', lawn: 'Lawn', park: 'Park', garden: 'Garden', market: 'Market', cemetery: 'Cemetery', farmland: 'Farmland' };
  const OPEN_KINDS = Object.keys(OPEN_LABEL) as OpenKind[];
  const WARDS: TownWard[] = ['common', 'merchant', 'craft', 'noble', 'slum', 'docks', 'military', 'farm', 'temple'];
  const ROW_WARDS = WARDS.filter((w) => w !== 'temple');
  const WARD_LABEL: Partial<Record<TownWard, string>> = { temple: 'Temple', merchant: 'Merchants', craft: 'Craftsmen', noble: 'Nobles', common: 'Common folk', slum: 'Slums', docks: 'Docks', military: 'Barracks', farm: 'Farms' };
  const LOT_LABEL: Record<LotSize, string> = { small: 'Small', medium: 'Medium', large: 'Large', huge: 'Huge' };
  const SQUARE_LABEL: Record<SquareKind | 'none', string> = { crossroads: 'Widened crossroads', well: 'Well square', fountain: 'Fountain square', green: 'Small green', none: 'No square' };
  const SIDE_LABEL: Record<string, string> = { l: 'left', r: 'right' };
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  const ft = (v: number) => `${Math.round(v).toLocaleString('en')} ft`;
  const wardName = (w: string) => WARD_LABEL[w as TownWard] ?? w;
  const fillLabel = (f: BlockFill) =>
    f.kind === 'generated' ? `${wardName(f.ward)}${f.lots ? `, ${f.lots} lots` : ''}` : f.kind === 'open' ? (OPEN_LABEL[f.open] ?? f.open) : f.kind === 'castle' ? 'Castle' : 'Left for buildings drawn by hand';

  const street = $derived(pick?.kind === 'street' ? streets.streets.find((x) => x.id === pick.id) : undefined);
  const junction = $derived(pick?.kind === 'junction' ? streets.junctions.find((x) => x.id === pick.id) : undefined);
  const block = $derived(pick?.kind === 'block' ? streets.blocks.find((x) => x.id === pick.id) : undefined);
  const area = $derived(pick?.kind === 'area' ? streets.areas.find((x) => x.id === pick.id) : undefined);
  const wall = $derived(pick?.kind === 'wall' ? streets.walls.find((x) => x.id === pick.id) : undefined);
  const rows = $derived(street ? streets.rows.filter((r) => r.street === street.id) : []);
  const districts = $derived([...new Set(streets.blocks.map((b) => b.district).filter((d): d is string => !!d))].sort());

  // A row being added: which side(s), how deep, which ward.
  let rowSide = $state<'l' | 'r' | 'both'>('both');
  let rowDepth = $state(40);
  let rowWard = $state<TownWard>('common');
  let naming = $state(false);
  let newName = $state('');
  $effect(() => {
    void pick?.id;
    naming = false;
  });

  /** A street's number field changed: one `set_street`. */
  function setNum(field: 'width' | 'set_l' | 'set_r', e: Event) {
    const v = (e.currentTarget as HTMLInputElement).valueAsNumber;
    if (!street || !Number.isFinite(v) || v < 0) return;
    onStreet({ op: 'set_street', street: street.id, [field]: v }, field === 'width' ? 'width' : 'setback');
  }
  function addRows() {
    if (!street) return;
    const sides: ('l' | 'r')[] = rowSide === 'both' ? ['l', 'r'] : [rowSide];
    const depth = Math.max(10, Math.min(200, rowDepth || 40));
    onStreets(sides.map((side) => ({ op: 'add_row', street: street.id, side, depth, ward: rowWard })), 'row');
  }
  function setFill(fill: BlockFill) {
    if (block) onStreet({ op: 'set_block', block: block.id, fill }, 'fill');
  }
  function fillKind(k: 'generated' | 'hand' | 'open') {
    if (!block || block.fill.kind === k) return;
    setFill(k === 'generated' ? { kind: 'generated', ward: 'common' } : k === 'open' ? { kind: 'open', open: 'lawn' } : { kind: 'hand' });
  }
  function pickDistrict(e: Event) {
    const el = e.currentTarget as HTMLSelectElement;
    if (!block) return;
    if (el.value === '@new') {
      naming = true;
      el.value = block.district ?? '';
      return;
    }
    naming = false;
    onStreet({ op: 'set_block', block: block.id, district: el.value }, 'district');
  }
  function makeDistrict() {
    const n = newName.trim().slice(0, 80);
    if (!n || !block) return;
    onStreet({ op: 'set_block', block: block.id, district: n }, 'district');
    naming = false;
    newName = '';
  }
</script>

{#if street}
  <section class="card" aria-label="Street">
    <div class="title"><span class="grow"><b>{CLASS_LABEL[street.class]}</b><span class="ws-muted"> · {ft(street.width)} wide, set back {ft(street.set_l)} and {ft(street.set_r)}{street.bridge ? ', a bridge' : ''}</span></span></div>
    <div class="ws-seg" role="radiogroup" aria-label="Class">
      {#each CLASSES as c (c)}
        <button class:on={street.class === c} aria-pressed={street.class === c} disabled={busy} onclick={() => street.class !== c && onStreet({ op: 'set_street', street: street.id, class: c }, 'class')}>{CLASS_SHORT[c]}</button>
      {/each}
    </div>
    <div class="nums">
      <label class="ws-field">Width (ft)<input class="ws-input" type="number" min="4" max="120" step="1" value={Math.round(street.width * 10) / 10} disabled={busy} onchange={(e) => setNum('width', e)} /></label>
      <label class="ws-field">Set back left<input class="ws-input" type="number" min="0" max="120" step="1" value={Math.round(street.set_l * 10) / 10} disabled={busy} onchange={(e) => setNum('set_l', e)} /></label>
      <label class="ws-field">Set back right<input class="ws-input" type="number" min="0" max="120" step="1" value={Math.round(street.set_r * 10) / 10} disabled={busy} onchange={(e) => setNum('set_r', e)} /></label>
    </div>
    <div class="ws-field">
      Houses along it
      {#each rows as r (r.id)}
        <div class="line">
          <span class="dot" style:background={hex(WARD_COLOURS[r.ward as TownWard] ?? WARD_COLOURS.common)}></span>
          <span class="grow">{wardName(r.ward)} on the {SIDE_LABEL[r.side] ?? r.side}, {ft(r.depth)} deep</span>
          <button class="ws-icon-btn" disabled={busy} onclick={() => onStreet({ op: 'remove_row', row: r.id }, 'row')} title="Take this row away" aria-label="Take this row away"><Icon name="trash" size={16} /></button>
        </div>
      {/each}
      <div class="ws-seg" role="radiogroup" aria-label="Side">
        {#each [['l', 'Left'], ['r', 'Right'], ['both', 'Both']] as [k, label] (k)}
          <button class:on={rowSide === k} aria-pressed={rowSide === k} onclick={() => (rowSide = k as typeof rowSide)} title={k === 'both' ? 'Both sides of the street' : `The ${label.toLowerCase()} side, going the way it was drawn`}>{label}</button>
        {/each}
      </div>
      <div class="nums">
        <label class="ws-field">Deep (ft)<input class="ws-input" type="number" min="10" max="200" step="5" bind:value={rowDepth} /></label>
        <label class="ws-field">Ward<select class="ws-input" bind:value={rowWard}>{#each ROW_WARDS as w (w)}<option value={w}>{WARD_LABEL[w]}</option>{/each}</select></label>
      </div>
      <div class="row">
        <button class="ws-btn" disabled={busy} onclick={addRows} title="Line the street with houses: for a street no block has on that side"><Icon name="building" size={16} /> Line with houses</button>
      </div>
    </div>
    <div class="row">
      <button class="ws-btn danger" disabled={busy} onclick={() => (onStreet({ op: 'delete_street', street: street.id }, 'street'), onPick(null))} title="The blocks on its two sides become one (Delete)"><Icon name="trash" size={16} /> Take the street away</button>
    </div>
  </section>
{:else if junction}
  <section class="card" aria-label="Junction">
    <div class="title"><span class="grow"><b>{junction.streets <= 1 ? 'A dead end' : `Where ${junction.streets} streets meet`}</b>{#if junction.square}<span class="ws-muted"> · {SQUARE_LABEL[junction.square as SquareKind] ?? junction.square}</span>{/if}</span></div>
    {#if junction.streets >= 2}
      <label class="ws-field">
        Square
        <select class="ws-input" value={junction.square ?? 'none'} disabled={busy} onchange={(e) => onStreet({ op: 'set_square', junction: junction.id, square: (e.currentTarget as HTMLSelectElement).value }, 'square')}>
          {#each [...SQUARE_KINDS, 'none' as const] as k (k)}
            <option value={k}>{SQUARE_LABEL[k]}</option>
          {/each}
          {#if junction.square && !(SQUARE_KINDS as readonly string[]).includes(junction.square)}<option value={junction.square}>{junction.square}</option>{/if}
        </select>
      </label>
    {/if}
    <span class="ws-muted">Drag it to move it (Choose).</span>
  </section>
{:else if block}
  <section class="card" aria-label="Block">
    <div class="title">
      <span class="dot" style:background={hex(fillColour(block.fill))}></span>
      <span class="grow"><b>{fillLabel(block.fill)}</b>{#if block.district}<span class="ws-muted"> · {names[block.district] ?? block.district}</span>{/if}</span>
    </div>
    {#if block.kept}<span class="ws-muted">Its buildings as they were laid out, until its streets or fill change.</span>{/if}
    <div class="ws-seg" role="radiogroup" aria-label="Fill">
      <button class:on={block.fill.kind === 'generated'} aria-pressed={block.fill.kind === 'generated'} disabled={busy} onclick={() => fillKind('generated')} title="Lots and houses of a ward, laid out for you">Lots</button>
      <button class:on={block.fill.kind === 'hand'} aria-pressed={block.fill.kind === 'hand'} disabled={busy} onclick={() => fillKind('hand')} title="Left empty, for buildings drawn in Build">By hand</button>
      <button class:on={block.fill.kind === 'open'} aria-pressed={block.fill.kind === 'open'} disabled={busy} onclick={() => fillKind('open')} title="Open ground: a square, a park, a market, a cemetery…">Open</button>
    </div>
    {#if block.fill.kind === 'generated'}
      {@const f = block.fill}
      <label class="ws-field">
        Ward
        <select class="ws-input" value={f.ward} disabled={busy} onchange={(e) => setFill({ ...f, ward: (e.currentTarget as HTMLSelectElement).value })}>
          {#each WARDS as w (w)}<option value={w}>{WARD_LABEL[w]}</option>{/each}
          {#if !WARDS.includes(f.ward as TownWard)}<option value={f.ward}>{wardName(f.ward)}</option>{/if}
        </select>
      </label>
      <div class="ws-field">
        Lots
        <div class="ws-seg" role="radiogroup" aria-label="Lots">
          <button class:on={!f.lots} aria-pressed={!f.lots} disabled={busy} onclick={() => f.lots && setFill({ ...f, lots: undefined })} title="As its ward has them">Auto</button>
          {#each LOT_SIZES as l (l)}
            <button class:on={f.lots === l} aria-pressed={f.lots === l} disabled={busy} onclick={() => f.lots !== l && setFill({ ...f, lots: l })}>{LOT_LABEL[l]}</button>
          {/each}
        </div>
      </div>
      <div class="row">
        <button class="ws-btn" disabled={busy} onclick={() => onStreet({ op: 'set_block', block: block.id, reroll: true }, 'reroll')} title="Its lots and buildings laid out another way"><Icon name="dice" size={16} /> Lay out again</button>
      </div>
    {:else if block.fill.kind === 'open'}
      {@const f = block.fill}
      <label class="ws-field">
        Open ground
        <select class="ws-input" value={f.open} disabled={busy} onchange={(e) => setFill({ kind: 'open', open: (e.currentTarget as HTMLSelectElement).value as OpenKind })}>
          {#each OPEN_KINDS as k (k)}<option value={k}>{OPEN_LABEL[k]}</option>{/each}
        </select>
      </label>
    {/if}
    <div class="ws-field">
      <label for="block-district">District</label>
      <select id="block-district" class="ws-input" value={block.district ?? ''} disabled={busy} onchange={pickDistrict}>
        <option value="">None</option>
        {#each districts as d (d)}<option value={d}>{names[d] ?? d}</option>{/each}
        <option value="@new">New district…</option>
      </select>
      {#if naming}
        <div class="row">
          <input class="ws-input grow" type="text" minlength="1" maxlength="80" placeholder="Name (an existing name joins it)" bind:value={newName} disabled={busy} onkeydown={(e) => e.key === 'Enter' && makeDistrict()} />
          <button class="ws-btn" disabled={busy || !newName.trim()} onclick={makeDistrict}>Make</button>
          <button class="ws-btn" disabled={busy} onclick={() => (naming = false)}>Cancel</button>
        </div>
      {/if}
    </div>
  </section>
{:else if area}
  <section class="card" aria-label="Area">
    <div class="title">
      <span class="dot" style:background={hex(OPEN_COLOURS[area.open] ?? WARD_COLOURS.park)}></span>
      <span class="grow"><b>{OPEN_LABEL[area.open] ?? area.open}</b><span class="ws-muted"> · painted over its block</span></span>
    </div>
    <div class="row">
      <button class="ws-btn danger" disabled={busy} onclick={() => (onStreet({ op: 'remove_area', area: area.id }, 'area'), onPick(null))} title="Its block built over again there (Delete)"><Icon name="trash" size={16} /> Take the area away</button>
    </div>
  </section>
{:else if wall}
  <section class="card" aria-label="Wall">
    <div class="title"><span class="grow"><b>{wall.name || 'A wall'}</b><span class="ws-muted"> · {wall.closed ? 'closed round' : 'open at its ends'}{wall.spacing ? `, towers ${ft(wall.spacing)} apart` : ''}</span></span></div>
    <span class="ws-muted">Gates open where streets cross it{wall.gates.length ? `, and ${wall.gates.length === 1 ? 'one more' : `${wall.gates.length} more`} put in by hand` : ''}.</span>
    <div class="row">
      <button class="ws-btn danger" disabled={busy} onclick={() => (onStreet({ op: 'remove_wall', wall: wall.id }, 'wall'), onPick(null))} title="Its towers and gates go with it (Delete)"><Icon name="trash" size={16} /> Take the wall away</button>
    </div>
  </section>
{/if}

<style>
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title,
  .line {
    display: flex;
    align-items: center;
    gap: 8px;
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
    flex: none;
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
  .nums {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: 6px;
  }
</style>
