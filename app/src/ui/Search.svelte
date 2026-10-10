<script lang="ts">
  // The search box: the notebook's people and plot points, notes written on places, named
  // places on the map, and districts and named businesses (from the settlement layouts) as
  // they arrive, in groups. "Only in view" (a chip over the results) keeps to the current view.
  // ↑/↓ move through the results, Enter opens one.
  import type { Npc, Overlay, PlaceInfo, Plot } from '../gen/protocol';
  import { hitName, kindLabel, searchFeatures, type BuildingHit, type DistrictHit, type Rect, type Selection } from './gazetteer';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  type Found = BuildingHit | DistrictHit;
  type Row = { key: string; group: string; icon: IconName; name: string; sub: string; pick: () => void };

  interface Props {
    overlay: Overlay;
    renames: Record<string, string>;
    searchBuildings: (q: string, rect?: Rect) => Promise<Found[]>;
    viewRect: () => Rect;
    settlementName: (index: number) => string;
    onSelect: (s: Selection) => void;
    npcs: Record<string, Npc>;
    plots: Record<string, Plot>;
    notes: Record<string, { text: string; tags?: string[] }>;
    /** A place's name and position (buildings and the like, for notes on them). */
    resolve: (id: string) => Promise<PlaceInfo | null>;
    onNpc: (id: string) => void;
    onPlot: (id: string) => void;
    /** Show the place a note is on. */
    onNote: (id: string) => void;
  }
  let { overlay, renames, searchBuildings, viewRect, settlementName, onSelect, npcs, plots, notes, resolve, onNpc, onPlot, onNote }: Props = $props();

  let q = $state('');
  let open = $state(false);
  let active = $state(0);
  let buildings = $state<Found[]>([]);
  let searching = $state(false);
  let local = $state(readLocal());
  let rect = $state<Rect | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let token = 0;

  function readLocal(): boolean {
    try {
      return localStorage.getItem('search.inView') === '1';
    } catch {
      return false;
    }
  }

  // Names of places notes are on (map features at once, the rest looked up once). Called while
  // the rows are derived, so a lookup only starts there: the name is set when it comes back.
  let names = $state<Record<string, string>>({});
  const asked = new Set<string>();
  function placeName(id: string): string {
    if (renames[id]) return renames[id];
    const f = overlay.features.find((g) => g.id === id);
    if (f) return f.name;
    if (!asked.has(id)) {
      asked.add(id);
      void resolve(id).then((p) => (names[id] = p?.generated ?? p?.name ?? id));
    }
    return names[id] ?? '…';
  }

  const needle = $derived(q.trim().toLowerCase());
  const has = (s: string | undefined, t: string) => !!s && s.toLowerCase().includes(t);
  const snippet = (text: string, t: string) => {
    const i = text.toLowerCase().indexOf(t);
    const from = Math.max(0, i - 20);
    return `${from ? '…' : ''}${text.slice(from, from + 70).replace(/\s+/g, ' ')}${text.length > from + 70 ? '…' : ''}`;
  };

  const rows = $derived.by<Row[]>(() => {
    const t = needle;
    if (!t) return [];
    const out: Row[] = [];
    const people = Object.entries(npcs)
      .filter(([, n]) => has(n.name, t) || n.tags.some((g) => has(g, t)) || has(n.status, t))
      .sort(([, a], [, b]) => Number(!a.name.toLowerCase().startsWith(t)) - Number(!b.name.toLowerCase().startsWith(t)) || a.name.localeCompare(b.name))
      .slice(0, 6);
    for (const [k, n] of people) out.push({ key: k, group: 'People', icon: 'user', name: n.name, sub: ['NPC', n.status, n.location ? placeName(n.location.id) : ''].filter(Boolean).join(' · '), pick: () => onNpc(k) });
    const plotRows = Object.entries(plots)
      .filter(([, p]) => has(p.title, t) || has(p.text, t) || p.tags.some((g) => has(g, t)))
      .slice(0, 5);
    for (const [k, p] of plotRows) out.push({ key: k, group: 'Plots', icon: 'scroll', name: p.title, sub: has(p.title, t) ? p.status : snippet(p.text, t), pick: () => onPlot(k) });
    const noteRows = Object.entries(notes)
      .filter(([id, n]) => has(n.text, t) || (n.tags ?? []).some((g) => has(g, t)) || has(placeName(id), t))
      .slice(0, 5);
    for (const [id, n] of noteRows) out.push({ key: `note:${id}`, group: 'Notes', icon: 'note', name: placeName(id), sub: snippet(n.text, t) || (n.tags ?? []).join(', '), pick: () => onNote(id) });
    for (const f of searchFeatures(overlay, q, 12, local && rect ? rect : undefined))
      out.push({ key: f.id, group: 'Places', icon: 'pin', name: renames[f.id] ?? f.name, sub: kindLabel(f.kind), pick: () => onSelect({ kind: 'feature', feature: f }) });
    for (const b of buildings)
      out.push({
        key: b.id,
        group: 'Buildings & districts',
        icon: b.kind === 'district' ? 'grid' : 'building',
        name: hitName(b, renames),
        sub: `${b.kind === 'district' ? 'district' : b.function.toLowerCase()} · ${settlementName(b.settlement)}`,
        pick: () => onSelect(b.kind === 'district' ? { kind: 'district', hit: b } : { kind: 'building', hit: b }),
      });
    return out;
  });
  const groups = $derived([...new Set(rows.map((r) => r.group))].map((g) => ({ g, rows: rows.filter((r) => r.group === g) })));

  function input() {
    open = true;
    active = 0;
    rect = local ? viewRect() : null;
    clearTimeout(timer);
    buildings = [];
    const query = q.trim();
    if (query.length < 3) return;
    const mine = ++token;
    // A plain copy: reactive state can't be posted to a worker.
    const area = rect ? ([...rect] as Rect) : undefined;
    timer = setTimeout(async () => {
      searching = true;
      const res = await searchBuildings(query, area);
      if (mine === token) buildings = res;
      searching = false;
    }, 300);
  }

  function toggleLocal() {
    local = !local;
    try {
      localStorage.setItem('search.inView', local ? '1' : '0');
    } catch {
      // Private mode: the choice just isn't remembered.
    }
    if (q.trim()) input();
  }

  function pick(r: Row) {
    open = false;
    (document.activeElement as HTMLElement | null)?.blur();
    r.pick();
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (open && q) open = false;
      else (e.target as HTMLInputElement).blur();
      e.stopPropagation();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      if (!rows.length) return;
      open = true;
      active = (active + (e.key === 'ArrowDown' ? 1 : rows.length - 1)) % rows.length;
      e.preventDefault();
    } else if (e.key === 'Enter') {
      const r = rows[active] ?? rows[0];
      if (r) pick(r);
    }
  }

  const placeholder = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform) ? 'Search places, people, notes… ⌘K' : 'Search places, people, notes…';
</script>

<div class="search">
  <input
    id="map-search"
    type="search"
    {placeholder}
    bind:value={q}
    oninput={input}
    onfocus={() => (open = true)}
    onblur={() => setTimeout(() => (open = false), 150)}
    onkeydown={key}
    aria-label="Search the map, people and notes"
    aria-expanded={open && !!needle}
    aria-controls="search-results"
    role="combobox"
    autocomplete="off"
  />
  {#if open && needle}
    <!-- (Pressing in the list keeps the box focused, so it stays open for the click.) -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="results" id="search-results" role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
      <div class="chips"><button class="ws-chip" class:on={local} aria-pressed={local} onclick={toggleLocal}>Only in view</button></div>
      {#each groups as { g, rows: list } (g)}
        <div class="group">{g}</div>
        {#each list as r (r.key)}
          {@const i = rows.indexOf(r)}
          <button class="row" class:active={i === active} role="option" aria-selected={i === active} onclick={() => pick(r)} onmouseenter={() => (active = i)}>
            <Icon name={r.icon} size={16} />
            <span class="text"><span class="name">{r.name}</span><span class="sub">{r.sub}</span></span>
          </button>
        {/each}
      {/each}
      {#if searching}<div class="note">Searching buildings…</div>{/if}
      {#if !searching && !rows.length && needle.length >= 3}<div class="note">Nothing found{local ? ' in view' : ''}</div>{/if}
      {#if !rows.length && needle.length < 3}<div class="note">Keep typing…</div>{/if}
    </div>
  {/if}
</div>

<style>
  .search {
    position: relative;
    flex: 1;
    min-width: 0;
    font: 13px/1.4 var(--font);
    color: var(--ink);
  }
  input[type='search'] {
    width: 100%;
    box-sizing: border-box;
    min-height: var(--tap);
    font: inherit;
    font-size: 14px;
    padding: 4px 6px;
    background: transparent;
    border: none;
    outline: none;
    color: var(--ink);
  }
  input[type='search']::placeholder {
    color: var(--ink-3);
  }
  .results {
    position: absolute;
    z-index: var(--z-search);
    left: -42px;
    right: -30px;
    top: calc(100% + 8px);
    padding: 4px 0;
    max-height: min(65vh, 560px);
    overflow-y: auto;
    background: var(--paper-solid);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
  }
  .chips {
    padding: 2px 10px 6px;
    border-bottom: 1px solid var(--line-faint);
  }
  .group {
    padding: 6px 12px 2px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--ink-3);
  }
  .row {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: var(--tap);
    padding: 3px 12px;
    cursor: pointer;
    color: var(--ink-2);
  }
  .row.active,
  .row:focus-visible {
    background: var(--btn-hover);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .name,
  .sub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: bold;
    color: var(--ink);
  }
  .sub {
    font-size: 12px;
    font-style: italic;
    color: var(--ink-3);
  }
  .note {
    padding: 4px 12px;
    color: var(--ink-3);
    font-style: italic;
  }
</style>
