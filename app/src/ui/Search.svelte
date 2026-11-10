<script lang="ts">
  // The search box: the notebook's people and plot points, notes written on places, named
  // places on the map, and districts and named businesses (from the settlement layouts) as
  // they arrive, in groups, and underground sites (by name, renamed levels and rooms, kind and
  // theme words, what is attached to them). Chips over the results: "Only in view" or "Near
  // here" keep to the view or round its middle; while underground sites are listed, kind and
  // depth chips narrow them. ↑/↓ move through the results, Enter opens one; Tab reaches the chips.
  import type { Npc, Overlay, PlaceInfo, Plot } from '../gen/protocol';
  import { hitName, kindIcon, SETTLEMENT_KINDS, kindLabel, listedName, removedKind, searchFeatures, UNDER_DEPTHS, UNDER_KINDS, underIcon, underTarget, unnamed, type BuildingHit, type DistrictHit, type Rect, type Selection, type UnderAttached, type UnderHit, type UnderResult } from './gazetteer';
  import { roleOf, parseLoc, type Token } from '../play/state';
  import Icon from './Icon.svelte';
  import type { IconName } from './icons';

  type Found = BuildingHit | DistrictHit;
  type Row = { key: string; group: string; icon: IconName; name: string; sub: string; pick: () => void; indent?: boolean };
  type Place = '' | 'view' | 'near';

  /** "Near here": round the view's middle, this far (ft: 5 mi, a morning's walk). */
  const NEAR_FT = 5 * 5280;
  /** Underground sites listed at most (the chips narrow the rest), and asked for (kind and
   * depth chips filter here, at once, and show only the kinds and depths found). */
  const UNDER_MAX = 8;
  const UNDER_ASK = 300;
  /** Things attached to a site listed under it at most. */
  const ATTACHED_MAX = 3;

  interface Props {
    overlay: Overlay;
    renames: Record<string, string>;
    searchBuildings: (q: string, rect?: Rect) => Promise<Found[]>;
    viewRect: () => Rect;
    /** Where something in a layout is, by name: its settlement, else its site. */
    whereName: (index: number) => string;
    /** The settlement a site (by id) stands in, by name, if any. */
    inTown: (id: string) => string | null;
    /** A building drawn by hand behind a building id: its site's id (else null). */
    drawnOf: (id: string) => string | null;
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
    /** The play tokens now (read when a query is typed, not per frame). */
    tokens?: () => Record<string, Token>;
    /** Go to a token: its location and floor, the camera on it, selected. */
    onToken?: (id: string) => void;
    /** The level names of a building or site (the floor picker's), null if unknown. */
    levelNames?: (id: string) => Promise<string[] | null>;
    /** Underground sites matching a search (`GenClient.searchUnder`). */
    searchUnder?: (q: string, filters: object) => Promise<UnderResult>;
    /** Open an underground site at a place of it (`underTarget`: the site, a level or a room). */
    onUnder?: (id: string) => void;
    /** A settlement's or site's layout index by its feature id (-1 if none). */
    layoutOf?: (id: string) => number;
    /** A layout's settlement (its own, or the one its site stands in) and site, as `{id, name}`. */
    layoutWhere?: (li: number) => { settlement: { id: string; name: string } | null; site: { id: string; name: string } | null };
  }
  let { overlay, renames, searchBuildings, viewRect, whereName, inTown, drawnOf, onSelect, npcs, plots, notes, resolve, onNpc, onPlot, onNote, tokens, onToken, levelNames, searchUnder, onUnder, layoutOf, layoutWhere }: Props = $props();

  let q = $state('');
  let open = $state(false);
  let active = $state(0);
  let buildings = $state<Found[]>([]);
  let searching = $state(false);
  let under = $state<UnderHit[]>([]);
  let place = $state<Place>(readPlace());
  let rect = $state<Rect | null>(null);
  /** Kind chips on (the index's kind names) and the depth chip (an `UNDER_DEPTHS` index). */
  let kinds = $state<string[]>([]);
  let depth = $state<number | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let token = 0;
  let box: HTMLDivElement | undefined = $state();
  let input_: HTMLInputElement | undefined = $state();

  function readPlace(): Place {
    try {
      const p = localStorage.getItem('search.place');
      if (p === 'view' || p === 'near') return p;
      return localStorage.getItem('search.inView') === '1' ? 'view' : '';
    } catch {
      return '';
    }
  }

  // Names of places notes are on (map features at once, the rest looked up once). Called while
  // the rows are derived, so a lookup only starts there: the name is set when it comes back.
  // (Again for another world or this one remade: a new `resolve`.)
  let names = $state<Record<string, string>>({});
  let asked = new Set<string>();
  let namesOf: unknown = null;
  $effect.pre(() => {
    if (namesOf === resolve) return;
    [namesOf, names, asked] = [resolve, {}, new Set()];
  });
  function placeName(id: string): string {
    if (removedKind(id)) return listedName(id, renames, () => unnamed(id));
    if (renames[id]) return renames[id];
    const f = overlay.features.find((g) => g.id === id);
    if (f) return f.name;
    if (!asked.has(id)) {
      asked.add(id);
      const ask = resolve;
      void ask(id).then((p) => {
        if (namesOf === ask) names[id] = p?.generated ?? p?.name ?? unnamed(id);
      });
    }
    return names[id] ?? '…';
  }

  /** Where a token is: the surface, or its building or site and level. */
  function tokenWhere(loc: string): string {
    if (loc === 'surface') return 'On the map';
    if (/^w:\d+$/.test(loc)) return 'Sewers';
    const { interior, level } = parseLoc(loc);
    if (interior) askLevels(interior);
    if (!interior) return 'On the map';
    const lv = levels[interior];
    const name = lv ? lv[level] : level === 0 ? 'ground floor' : undefined;
    return `${placeName(interior)} · ${name ?? (level === 0 ? 'ground floor' : `level ${level + 1}`)}`;
  }

  // Level names per place, looked up once (while the rows are derived, set when they come back).
  let levels = $state<Record<string, string[] | undefined>>({});
  let levelAsked = new Set<string>();
  let levelsOf: unknown = null;
  $effect.pre(() => {
    if (levelsOf === levelNames) return;
    [levelsOf, levels, levelAsked] = [levelNames, {}, new Set()];
  });
  function askLevels(id: string) {
    if (!levelNames || levelAsked.has(id)) return;
    levelAsked.add(id);
    const ask = levelNames;
    void ask(id).then((n) => {
      if (levelsOf === ask && n) levels[id] = n;
    });
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
    if (tokens && onToken) {
      const list = Object.values(tokens())
        .filter((k) => has(k.name, t))
        .sort((a, b) => Number(!a.name.toLowerCase().startsWith(t)) - Number(!b.name.toLowerCase().startsWith(t)) || a.name.localeCompare(b.name))
        .slice(0, 6);
      for (const k of list) out.push({ key: `token:${k.id}`, group: 'Tokens', icon: 'token', name: k.name, sub: [k.kind === 'character' ? roleOf(k) : k.kind, tokenWhere(k.loc), k.hidden ? 'hidden' : ''].filter(Boolean).join(' · '), pick: () => onToken(k.id) });
    }
    const noteRows = Object.entries(notes)
      .filter(([id, n]) => has(n.text, t) || (n.tags ?? []).some((g) => has(g, t)) || has(placeName(id), t))
      .slice(0, 5);
    for (const [id, n] of noteRows) out.push({ key: `note:${id}`, group: 'Notes', icon: 'note', name: placeName(id), sub: snippet(n.text, t) || (n.tags ?? []).join(', '), pick: () => onNote(id) });
    const places = new Set<string>();
    for (const f of searchFeatures(overlay, q, 12, rect ?? undefined)) {
      const town = inTown(f.id);
      places.add(f.id);
      out.push({ key: f.id, group: 'Places', icon: /^(rd|v):/.test(f.id) ? kindIcon(f.kind) : 'pin', name: renames[f.id] ?? f.name, sub: `${kindLabel(f.kind)}${town ? ` · in ${town}` : ''}`, pick: () => onSelect({ kind: 'feature', feature: f }) });
    }
    for (const h of underShown.slice(0, UNDER_MAX)) {
      out.push({ key: `under:${h.id}`, group: 'Underground', icon: underIcon(h.site_kind), name: h.name, sub: underSub(h), pick: () => onUnder?.(underTarget(h)) });
      for (const a of h.attached.slice(0, ATTACHED_MAX)) out.push(attachedRow(h, a));
    }
    for (const b of buildings) {
      // A building drawn by hand is its site: listed once, under Places, and hidden with it.
      const site = b.kind === 'building' ? drawnOf(b.id) : null;
      if (site && (places.has(site) || !overlay.features.some((f) => f.id === site))) continue;
      out.push({
        key: b.id,
        group: 'Buildings & districts',
        icon: b.kind === 'district' ? 'grid' : 'building',
        name: hitName(b, renames),
        sub: `${b.kind === 'district' ? 'district' : b.function.toLowerCase()} · ${whereName(b.settlement)}`,
        pick: () => onSelect(b.kind === 'district' ? { kind: 'district', hit: b } : { kind: 'building', hit: b }),
      });
    }
    return out;
  });
  /** An underground site's sub line: what matched (a level, room, NPC, plot or note), its kind,
   * theme, levels and where it is (its settlement, the site it lies under, else the named place
   * nearest), how far when near here. */
  function underSub(h: UnderHit): string {
    const m = h.match;
    const what = m && ['level', 'room', 'npc', 'plot', 'note'].includes(m.what) ? `${m.what === 'npc' ? 'NPC' : m.what}: ${m.what === 'note' ? snippet(m.text, needle) : m.text}` : '';
    // (The theme alone when it says the kind: "ice cave", not "cave · ice cave".)
    const theme = h.theme_name && h.theme_name !== h.site_kind && h.site_kind !== 'sewer' ? h.theme_name : '';
    const kind = theme.split(' ').includes(h.site_kind) ? '' : h.site_kind === 'sewer' ? 'sewers' : h.site_kind;
    // (A city's sewers are named after it.)
    const where = h.site_kind === 'sewer' && h.name === h.generated ? '' : h.settlement ? `in ${renames[h.settlement.id] ?? h.settlement.name}` : h.site ? `under ${renames[h.site.id] ?? h.site.name}` : nearOf(h.x_ft, h.y_ft);
    const far = h.distance_ft === undefined ? '' : h.distance_ft < 528 ? 'here' : `${(h.distance_ft / 5280).toFixed(1)} mi`;
    return [what, kind, theme, `${h.levels} level${h.levels === 1 ? '' : 's'}`, where, far, h.hidden ? 'hidden' : ''].filter(Boolean).join(' · ');
  }

  /** The named place nearest a point (within 3 mi; not seas or land masses), as "near …". */
  function nearOf(x: number, y: number): string {
    let best = '';
    let bestD = 3 * 5280;
    for (const g of overlay.features) {
      if (['ocean', 'sea', 'continent', 'island', 'range'].includes(g.kind)) continue;
      const d = Math.hypot(g.x - x, g.y - y);
      if (d < bestD) [best, bestD] = [renames[g.id] ?? g.name, d];
    }
    return best && `near ${best}`;
  }

  /** A note, NPC or plot point on an underground site, listed under it: a note opens its place,
   * an NPC or plot point the notebook. */
  function attachedRow(h: UnderHit, a: UnderAttached): Row {
    const on = a.at !== h.id && /^[lr]:/.test(a.at) ? placeName(a.at) : '';
    const base = { key: `under:${h.id}:${a.what}:${a.id}:${a.at}`, group: 'Underground', indent: true };
    if (a.what === 'npc') return { ...base, icon: 'user', name: a.name ?? '', sub: ['NPC', on].filter(Boolean).join(' · '), pick: () => onNpc(a.id) };
    if (a.what === 'plot') return { ...base, icon: 'scroll', name: a.name ?? '', sub: ['plot', on].filter(Boolean).join(' · '), pick: () => onPlot(a.id) };
    const text = (a.text ?? '').replace(/\s+/g, ' ');
    return { ...base, icon: 'note', name: text.length > 60 ? `${text.slice(0, 60)}…` : text, sub: ['note', on].filter(Boolean).join(' · '), pick: () => onUnder?.(underTarget({ ...h, match: { what: 'note', id: a.at, text: '' } })) };
  }

  const band = (h: UnderHit, i: number) => h.levels >= UNDER_DEPTHS[i][1] && h.levels <= (UNDER_DEPTHS[i][2] ?? Infinity);
  /** The underground hits the kind and depth chips keep. */
  const underShown = $derived(under.filter((h) => (!kinds.length || kinds.includes(h.site_kind)) && (depth === null || band(h, depth))));
  /** Kind and depth chips: those of the sites found (and any on), when there are two to tell apart. */
  const kindChips = $derived.by(() => {
    const found = UNDER_KINDS.filter(([, k]) => kinds.includes(k) || under.some((h) => h.site_kind === k));
    return found.length > 1 || kinds.length ? found : [];
  });
  const depthChips = $derived.by(() => {
    const found = UNDER_DEPTHS.map((d, i) => [d[0], i] as const).filter(([, i]) => depth === i || under.some((h) => band(h, i)));
    return found.length > 1 || depth !== null ? found : [];
  });

  const groups = $derived([...new Set(rows.map((r) => r.group))].map((g) => ({ g, rows: rows.filter((r) => r.group === g) })));

  function input() {
    open = true;
    active = 0;
    buildings = [];
    under = [];
    // (The kind and depth chips last while the query is edited, not into a new search.)
    if (!q.trim()) [kinds, depth] = [[], null];
    run(300);
  }

  /** An underground site's layout index (`u:<layout>:<n>`, `k:<layout>:<b>`, `w:<layout>`). */
  const layoutOfHit = (id: string) => Number(id.split(':')[1]);

  /** The generator's workers have no place names (their T0 is read without the overlay): the
   * settlement and site of a hit from its layout, a city's sewers named after it, and the sites
   * hidden with the place over them left out (not in the overlay searched). */
  function placed(h: UnderHit): UnderHit | null {
    if (h.settlement || h.site || !layoutWhere) return h;
    const { settlement, site } = layoutWhere(layoutOfHit(h.id));
    if (site && !overlay.features.some((f) => f.id === site.id)) return null;
    const generated = h.site_kind === 'sewer' && settlement ? `Sewers of ${settlement.name}` : h.generated;
    return { ...h, settlement, ...(site ? { site } : {}), generated, name: renames[h.id] ?? generated };
  }

  /** Underground hits for a query: the index's, and (as `undersearch` does with names) those
   * under a site, or a city's sewers, whose place's name matches, after the hits by name. */
  async function underFor(query: string, filters: object): Promise<UnderHit[]> {
    const lo = layoutOf;
    if (!searchUnder) return [];
    const res = await searchUnder(query, filters);
    let items = res.items.map(placed).filter((h): h is UnderHit => !!h);
    const named = res.items.some((h) => h.settlement || h.site) || !lo ? [] : searchFeatures(overlay, query, 12).filter((f) => lo(f.id) >= 0);
    if (!named.length) return items;
    const have = new Set(items.map((h) => h.id));
    const extra: UnderHit[] = [];
    for (const h of (await searchUnder('', filters)).items) {
      const f = named.find((g) => lo!(g.id) === layoutOfHit(h.id));
      if (!f || have.has(h.id) || (SETTLEMENT_KINDS.includes(f.kind) && h.site_kind !== 'sewer')) continue;
      const p = placed(h);
      if (p) extra.push({ ...p, match: { what: 'name', id: h.site_kind === 'sewer' ? h.id : f.id, text: p.site_kind === 'sewer' ? p.name : (renames[f.id] ?? f.name) } });
    }
    const at = items.findIndex((h) => h.match?.what !== 'name');
    items = at < 0 ? [...items, ...extra] : [...items.slice(0, at), ...extra, ...items.slice(at)];
    return items;
  }

  /** Ask for buildings and underground sites (after `delay` ms; a stale answer is dropped). */
  function run(delay: number) {
    const v = viewRect();
    const mid: [number, number] = [(v[0] + v[2]) / 2, (v[1] + v[3]) / 2];
    // A plain copy: reactive state can't be posted to a worker.
    rect = place === 'view' ? [v[0], v[1], v[2], v[3]] : place === 'near' ? [mid[0] - NEAR_FT, mid[1] - NEAR_FT, mid[0] + NEAR_FT, mid[1] + NEAR_FT] : null;
    clearTimeout(timer);
    const mine = ++token;
    const query = q.trim();
    if (query.length < 3) {
      [buildings, under, searching] = [[], [], false];
      return;
    }
    const area = rect ? ([...rect] as Rect) : undefined;
    const filters = { ...(place === 'view' ? { rect: area } : place === 'near' ? { near: mid, radius_ft: NEAR_FT } : {}), limit: UNDER_ASK };
    timer = setTimeout(() => {
      searching = true;
      let left = searchUnder ? 2 : 1;
      const done = () => {
        if (mine === token && !--left) searching = false;
      };
      void searchBuildings(query, area).then((res) => {
        if (mine === token) buildings = res;
        done();
      });
      if (searchUnder)
        void underFor(query, filters).then((res) => {
          if (mine === token) under = res;
          done();
        });
    }, delay);
  }

  function setPlace(p: Place) {
    place = place === p ? '' : p;
    try {
      localStorage.setItem('search.place', place);
    } catch {
      // Private mode: the choice just isn't remembered.
    }
    if (q.trim()) run(0);
  }

  function toggleKind(k: string) {
    kinds = kinds.includes(k) ? kinds.filter((x) => x !== k) : [...kinds, k];
    active = 0;
  }

  function setDepth(i: number) {
    depth = depth === i ? null : i;
    active = 0;
  }

  /** Focus left the search (box, chips and results) altogether: close the list. */
  function focusOut(e: FocusEvent) {
    if (box?.contains(e.relatedTarget as Node | null)) return;
    setTimeout(() => {
      if (!box?.contains(document.activeElement)) open = false;
    }, 150);
  }

  function pick(r: Row) {
    open = false;
    [kinds, depth] = [[], null];
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

<div class="search" bind:this={box} onfocusout={focusOut}>
  <input
    bind:this={input_}
    id="map-search"
    type="search"
    {placeholder}
    bind:value={q}
    oninput={input}
    onfocus={() => (open = true)}
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
    <div
      class="results"
      id="search-results"
      role="listbox"
      tabindex="-1"
      onmousedown={(e) => e.preventDefault()}
      onkeydown={(e) => {
        if (e.key === 'Escape') (input_?.focus(), e.stopPropagation());
      }}
    >
      <div class="chips" role="group" aria-label="Narrow the results">
        <button class="ws-chip" class:on={place === 'view'} aria-pressed={place === 'view'} onclick={() => setPlace('view')}>Only in view</button>
        <button class="ws-chip" class:on={place === 'near'} aria-pressed={place === 'near'} onclick={() => setPlace('near')} title="Within 5 miles of the middle of the view">Near here</button>
        {#if kindChips.length}
          <span class="sep" aria-hidden="true"></span>
          {#each kindChips as [label, k] (k)}
            <button class="ws-chip" class:on={kinds.includes(k)} aria-pressed={kinds.includes(k)} onclick={() => toggleKind(k)}>{label}</button>
          {/each}
        {/if}
        {#if depthChips.length}
          <span class="sep" aria-hidden="true"></span>
          {#each depthChips as [label, i] (label)}
            <button class="ws-chip" class:on={depth === i} aria-pressed={depth === i} onclick={() => setDepth(i)}>{label}</button>
          {/each}
        {/if}
      </div>
      {#each groups as { g, rows: list } (g)}
        <div class="group">{g}</div>
        {#each list as r (r.key)}
          {@const i = rows.indexOf(r)}
          <button class="row" class:indent={r.indent} class:active={i === active} role="option" aria-selected={i === active} onclick={() => pick(r)} onmouseenter={() => (active = i)}>
            <Icon name={r.icon} size={16} />
            <span class="text"><span class="name">{r.name}</span><span class="sub">{r.sub}</span></span>
          </button>
        {/each}
        {#if g === 'Underground' && underShown.length > UNDER_MAX}<div class="note">{underShown.length - UNDER_MAX} more underground: narrow them with the chips</div>{/if}
      {/each}
      {#if under.length && !underShown.length}<div class="note">No underground sites with these chips</div>{/if}
      {#if searching}<div class="note">Searching…</div>{/if}
      {#if !searching && !rows.length && needle.length >= 3}<div class="note">Nothing found{place === 'view' ? ' in view' : place === 'near' ? ' near here' : ''}</div>{/if}
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
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
    padding: 2px 10px 6px;
    border-bottom: 1px solid var(--line-faint);
  }
  .sep {
    width: 1px;
    align-self: stretch;
    margin: 2px 2px;
    background: var(--line-faint);
  }
  .row.indent {
    padding-left: 36px;
    min-height: 32px;
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
