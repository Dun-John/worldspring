<script lang="ts">
  // The gazetteer (Notes): every name in the world, renamed from a list, and the notes written on
  // places. Natural features, then settlements and sites; a settlement or site opens onto its
  // districts, businesses, towers and ways underground (its layout is generated then), a settlement
  // also onto the places standing in it, a building or site onto its levels and rooms. A place's
  // page holds its name (the players see it), whether its label is hidden, and its note (only the
  // DM sees it).
  import { tick, untrack } from 'svelte';
  import type { Feature, NameEntry, PlaceInfo } from '../gen/protocol';
  import { kindIcon, kindLabel, SETTLEMENT_KINDS, SITE_KINDS } from './gazetteer';
  import Icon from './Icon.svelte';
  import type { Here } from './Notebook.svelte';

  type Note = { text: string; tags?: string[] };

  interface Props {
    /** The map's named features (created sites included). */
    features: Feature[];
    renames: Record<string, string>;
    hidden: string[];
    /** The notes written on places, by place id. */
    notes: Record<string, Note>;
    /** The place selected or entered: a note can be written on it. */
    here: Here | null;
    /** Open on this place's page, or with `reveal` on its row in the list (`seq` makes a repeat
     * count; no id: the list). */
    focus: { id: string | null; seq: number; reveal?: boolean };
    /** What a place opens onto: a settlement's or site's layout (its index), a building's or site's
     * levels (its id), else nothing. */
    scopeOf: (id: string) => number | string | null;
    /** The sites standing in a settlement (by its id). */
    placesIn: (id: string) => Feature[];
    /** A building drawn by hand (by its site's id) as a building id. */
    buildingOf: (id: string) => string | null;
    /** The settlement a site stands in. */
    townIn: (id: string) => { id: string; name: string } | null;
    /** What a site is, in a few words. */
    what: (f: Feature) => string;
    /** Changes when the lists inside layouts and buildings may have: those held are asked again. */
    version: unknown;
    /** What can be renamed in a layout (by index) or a building or site (by id). */
    list: (scope: number | string) => Promise<NameEntry[] | null>;
    /** A place that is not a map feature (a building, a room…): its generated name. */
    resolve: (id: string) => Promise<PlaceInfo | null>;
    /** Rename (an empty name goes back to the generated one). */
    onRename: (id: string, name: string) => void;
    /** Hide a feature's label from both maps and search, or show it again. */
    onHide: (id: string, hide: boolean) => void;
    /** Set (or with null, clear) a place's note (`name`: the place's, as shown). */
    onNote: (id: string, note: Note | null, name: string) => void;
    /** Fly to a place, or go inside a building or site (on a level's or room's level). */
    onGo: (id: string) => void;
    /** Show a place: selected on the map, its card open. */
    onShow: (id: string) => void;
  }
  let { features, renames, hidden, notes, here, focus, scopeOf, placesIn, buildingOf, townIn, what, version, list, resolve, onRename, onHide, onNote, onGo, onShow }: Props = $props();

  const TABS = [
    { key: 'water', label: 'Waters', kinds: ['ocean', 'sea', 'bay', 'strait', 'lake', 'salt_lake', 'river', 'waterfall'] },
    { key: 'land', label: 'Land', kinds: ['continent', 'island', 'range', 'peak', 'pass', 'volcano', 'forest', 'jungle', 'taiga', 'desert', 'swamp', 'plains', 'tundra', 'glacier', 'blight', 'ashlands', 'region', 'salt_flat', 'road'] },
    { key: 'settlements', label: 'Settlements', kinds: SETTLEMENT_KINDS },
    { key: 'sites', label: 'Sites', kinds: SITE_KINDS },
  ];
  /** What a layout's names are grouped under. */
  const CHILD_GROUPS: [string, string][] = [
    ['district', 'Districts'],
    ['building', 'Businesses'],
    ['tower', 'Towers'],
    ['underground', 'Underground'],
  ];

  let tab = $state('settlements');
  let q = $state('');
  let showHidden = $state(false);
  /** Groups and rows opened or shut by hand (else groups open when short or filtered). */
  let opened = $state<Record<string, boolean>>({});
  /** Names inside layouts and sites, by the feature's or site's id. */
  let inside = $state<Record<string, NameEntry[] | null>>({});
  /** Rows whose names the generator could not list. */
  let failed = $state<Record<string, boolean>>({});
  let editing = $state<string | null>(null);
  let draft = $state('');
  /** Only the places with notes, as one list. */
  let withNotes = $state(false);
  /** The place whose page is open. */
  let current = $state<string | null>(null);
  let confirmDelete = $state(false);

  let lastSeq = -1;
  $effect(() => {
    if (focus.seq === lastSeq) return;
    lastSeq = focus.seq;
    confirmDelete = false;
    if (focus.reveal && focus.id) {
      const id = focus.id;
      current = null;
      untrack(() => reveal(id));
    } else current = focus.id;
  });

  let listEl = $state<HTMLElement | null>(null);
  /** Show a settlement's or site's row in the list: its tab and kind open, the row opened (a
   * settlement's places too), scrolled to and marked for a moment. */
  async function reveal(id: string) {
    const f = featureById.get(id);
    if (!f) return;
    withNotes = false;
    q = '';
    if (hiddenSet.has(id)) showHidden = true;
    tab = SETTLEMENT_KINDS.includes(f.kind) ? 'settlements' : SITE_KINDS.includes(f.kind) ? 'sites' : TABS.find((t) => t.kinds.includes(f.kind))?.key ?? 'land';
    opened[`kind/${f.kind}`] = true;
    const scope = scopeOf(id);
    if (scope !== null && !opened[id]) expand(id, scope);
    if (SETTLEMENT_KINDS.includes(f.kind)) opened[`${id}/places`] = true;
    await tick();
    const row = listEl?.querySelector<HTMLElement>(`[data-row="${CSS.escape(id)}"]`);
    row?.scrollIntoView({ block: 'start' });
    flash = id;
    setTimeout(() => flash === id && (flash = null), 1600);
  }
  /** The row just revealed. */
  let flash = $state<string | null>(null);

  const hiddenSet = $derived(new Set(hidden));
  const featureById = $derived(new Map(features.map((f) => [f.id, f])));
  const needle = $derived(q.trim().toLowerCase());
  const nameOf = (id: string, generated: string) => renames[id] ?? generated;
  const has = (s: string | undefined, t: string) => !!s && s.toLowerCase().includes(t);
  /** A place's note matches what is looked for. */
  const noteMatches = (id: string) => !!notes[id] && (has(notes[id].text, needle) || (notes[id].tags ?? []).some((g) => has(g, needle)));
  const matches = (id: string, generated: string) => !needle || nameOf(id, generated).toLowerCase().includes(needle) || generated.toLowerCase().includes(needle) || noteMatches(id);

  // Generated names of places that are not map features (a building, district, tower, way
  // underground, level or room), looked up from the generators: once for the notes listed, and
  // again whenever a page opens or renames (a drawn building's name is its generated one).
  let generatedNames = $state<Record<string, string>>({});
  /** Look a place's generated name up (`label`: what to show meanwhile). Never a rename. */
  function lookUp(id: string, label?: string) {
    if (featureById.has(id)) return;
    generatedNames[id] = generatedNames[id] ?? label ?? '…';
    void resolve(id).then((p) => (generatedNames[id] = p?.generated ?? label ?? id));
  }
  $effect(() => {
    for (const id of [...Object.keys(notes), ...(current ? [current] : [])]) if (!(id in generatedNames)) lookUp(id);
  });
  const generatedOf = (id: string) => featureById.get(id)?.name ?? generatedNames[id] ?? id;
  const placeName = (id: string) => nameOf(id, generatedOf(id));

  /** The places with notes, by name (filtered by what is looked for). */
  const noteList = $derived(
    Object.entries(notes)
      .filter(([id, n]) => !needle || has(n.text, needle) || (n.tags ?? []).some((g) => has(g, needle)) || has(placeName(id), needle))
      .sort(([a], [b]) => placeName(a).localeCompare(placeName(b))),
  );

  /** Open a place's page (`label`: the name its row shows before renames). */
  function openPage(id: string, label?: string) {
    lookUp(id, label);
    current = id;
    confirmDelete = false;
  }

  function setNote(id: string, text: string, tags: string[]) {
    if (!text.trim() && !tags.length) onNote(id, null, placeName(id));
    else onNote(id, tags.length ? { text, tags } : { text }, placeName(id));
  }

  /** The page's name field: an empty name or the generated one goes back to it. */
  function setName(id: string, value: string) {
    const generated = generatedOf(id);
    const n = value.trim();
    if (n === nameOf(id, generated)) return;
    onRename(id, !n || n === generated ? '' : n);
    lookUp(id);
  }

  const tagList = (s: string) => [...new Set(s.split(',').map((t) => t.trim()).filter(Boolean))];
  const val = (e: Event) => (e.currentTarget as HTMLInputElement | HTMLTextAreaElement).value;

  /** The tab's features by kind, in the tab's kind order (unknown kinds go with the land). */
  const groups = $derived.by(() => {
    const t = TABS.find((x) => x.key === tab)!;
    const known = new Set(TABS.flatMap((x) => x.kinds));
    const by = new Map<string, Feature[]>();
    for (const f of features) {
      if (!(t.kinds.includes(f.kind) || (tab === 'land' && !known.has(f.kind)))) continue;
      if (hiddenSet.has(f.id) && !showHidden) continue;
      if (!matches(f.id, f.name) && !opened[f.id]) continue;
      if (!by.has(f.kind)) by.set(f.kind, []);
      by.get(f.kind)!.push(f);
    }
    const order = (k: string) => (t.kinds.includes(k) ? t.kinds.indexOf(k) : 99);
    return [...by.entries()].sort((a, b) => order(a[0]) - order(b[0]) || a[0].localeCompare(b[0])).map(([kind, rows]) => ({ kind, rows: rows.sort((a, b) => nameOf(a.id, a.name).localeCompare(nameOf(b.id, b.name))) }));
  });

  const isOpen = (key: string, n: number) => opened[key] ?? (!!needle || n <= 40);

  function toggle(key: string, n: number) {
    opened[key] = !isOpen(key, n);
  }

  /** Open a settlement, site or building row, asking for what is in it the first time. */
  function expand(id: string, scope: number | string) {
    opened[id] = !opened[id];
    if (opened[id] && !(id in inside)) ask(id, scope);
  }

  /** Answers still to come, by row (a newer ask replaces an older one). */
  const asking: Record<string, number> = {};
  let asks = 0;
  /** Ask what is in a row (`keep`: show what it held meanwhile). */
  function ask(id: string, scope: number | string, keep = false) {
    const n = (asking[id] = ++asks);
    if (!keep) inside[id] = null;
    failed[id] = false;
    void list(scope).then((v) => {
      if (asking[id] !== n) return;
      delete asking[id];
      if (v) inside[id] = v;
      else if (keep && inside[id]) return;
      else {
        failed[id] = true;
        // (Asked again the next time it is opened.)
        delete inside[id];
      }
    });
  }

  // Sites created, moved or deleted, towns or buildings changed, another world: the lists held
  // may be out of date. Open rows are asked again (showing the old list until the answer comes);
  // shut ones forget theirs. A map feature's row asks by its scope now (a remade world's layout
  // indices can differ), anything else by its own id.
  let seenVersion: { v: unknown } | null = null;
  $effect(() => {
    const was = seenVersion;
    seenVersion = { v: version };
    if (!was || was.v === version) return;
    untrack(() => {
      for (const id of Object.keys(inside)) {
        const scope = featureById.has(id) ? scopeOf(id) : id;
        if (opened[id] && scope !== null) ask(id, scope, true);
        else {
          delete inside[id];
          delete asking[id];
        }
      }
    });
  });

  function edit(id: string, generated: string) {
    editing = id;
    draft = nameOf(id, generated);
  }

  function commit(id: string, generated: string) {
    if (editing !== id) return;
    editing = null;
    const n = draft.trim();
    if (n === nameOf(id, generated)) return;
    // Back to the generated name: no rename at all.
    onRename(id, n === generated ? '' : n);
    if (id in generatedNames) lookUp(id);
  }

  function grab(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  /** Generated names told apart where several are alike ("burial niches 2"). */
  function numbered(entries: NameEntry[]): { e: NameEntry; label: string }[] {
    const count = new Map<string, number>();
    for (const e of entries) count.set(e.generated, (count.get(e.generated) ?? 0) + 1);
    const seen = new Map<string, number>();
    return entries.map((e) => {
      if ((count.get(e.generated) ?? 0) < 2) return { e, label: e.generated };
      const k = (seen.get(e.generated) ?? 0) + 1;
      seen.set(e.generated, k);
      return { e, label: `${e.generated} ${k}` };
    });
  }

  /** A site's levels, each with its rooms. */
  function levels(entries: NameEntry[]) {
    const out: { level: NameEntry; rooms: NameEntry[] }[] = [];
    for (const e of entries) {
      if (e.kind === 'level') out.push({ level: e, rooms: [] });
      else if (out.length) out[out.length - 1].rooms.push(e);
    }
    return out.map((l) => ({ level: l.level, rooms: numbered(l.rooms) }));
  }
</script>

{#snippet name(id: string, generated: string, shown: string = generated)}
  {#if editing === id}
    <input
      class="edit"
      bind:value={draft}
      use:grab
      onkeydown={(e) => {
        if (e.key === 'Enter') commit(id, generated);
        else if (e.key === 'Escape') editing = null;
        e.stopPropagation();
      }}
      onblur={() => commit(id, generated)}
      aria-label="New name"
    />
  {:else}
    <button class="name" class:renamed={id in renames} onclick={() => edit(id, generated)} title="Rename">{renames[id] ?? shown}</button>
    {#if id in renames}
      <span class="muted was" title="The generated name">{shown}</span>
      <button class="icon" onclick={() => onRename(id, '')} title="Back to “{shown}”" aria-label="Reset the name">↺</button>
    {/if}
    {#if hiddenSet.has(id)}<span class="mark" title="Hidden on both maps and in search"><Icon name="eye-off" size={13} /></span>{/if}
  {/if}
{/snippet}

<!-- A row's note (only the DM sees it) and its way there. -->
{#snippet acts(id: string, generated: string, enter = false)}
  <span class="acts">
    <button class="icon note" class:has={id in notes} onclick={() => openPage(id, generated)} title={id in notes ? 'Its note (only you see it)' : 'Write a note (only you see it)'} aria-label={id in notes ? 'Its note' : 'Write a note'}><Icon name="note" size={14} /></button>
    <button class="icon go" onclick={() => onGo(id)} title={enter ? 'Go inside' : 'Go to'} aria-label={enter ? 'Go inside' : 'Go to'}>➜</button>
  </span>
{/snippet}

{#snippet caret(id: string, open: boolean, onclick: () => void)}
  <button class="icon caret" {onclick} aria-expanded={open} aria-label={open ? 'Shut' : 'Open'}>{open ? '▾' : '▸'}</button>
{/snippet}

<!-- A building's or site's levels and their rooms (the row `key`'s). -->
{#snippet site(key: string)}
  {@const v = inside[key]}
  {#if failed[key]}
    <li class="muted">The map's generator could not list these. Reload the page and try again.</li>
  {:else if v === null}
    <li class="muted wait">Working it out…</li>
  {:else if v}
    {#each levels(v) as l (l.level.id)}
      {@const lopen = opened[l.level.id] ?? false}
      {@const rooms = l.rooms.filter((r) => !needle || matches(r.e.id, r.label) || matches(l.level.id, l.level.generated))}
      <li>
        <div class="row">
          {#if l.rooms.length}{@render caret(l.level.id, lopen || (!!needle && rooms.length > 0), () => (opened[l.level.id] = !lopen))}{:else}<span class="pad"></span>{/if}
          {@render name(l.level.id, l.level.generated)}
          {@render acts(l.level.id, l.level.generated)}
        </div>
        {#if lopen || (needle && rooms.length)}
          <ul>
            {#each rooms as r (r.e.id)}
              <li><div class="row"><span class="pad"></span>{@render name(r.e.id, r.e.generated, r.label)}{@render acts(r.e.id, r.e.generated)}</div></li>
            {/each}
          </ul>
        {/if}
      </li>
    {:else}
      <li class="muted">Nothing inside to name.</li>
    {/each}
  {/if}
{/snippet}

<!-- A map feature's row (a settlement or site): its name, what it is, and what it opens onto. -->
{#snippet place(f: Feature, kind: string | null = null)}
  {@const scope = scopeOf(f.id)}
  {@const town = tab === 'sites' && !kind ? townIn(f.id) : null}
  <div class="row" class:flash={flash === f.id} data-row={f.id}>
    {#if scope !== null}{@render caret(f.id, !!opened[f.id], () => expand(f.id, scope))}{:else}<span class="pad"></span>{/if}
    {#if kind}<span class="kind" title={kind}><Icon name={kindIcon(f.kind)} size={13} /></span>{/if}
    {@render name(f.id, f.name)}
    {#if kind}<span class="muted what">{kind}</span>{/if}
    {#if town}<button class="muted in" onclick={() => reveal(town.id)} title="Show it under {town.name}">in {town.name}</button>{/if}
    {@render acts(f.id, f.name)}
  </div>
  {#if scope !== null && opened[f.id]}
    <ul>{#if typeof scope === 'number'}{@render layout(f.id, renames[f.id] ?? f.name, SETTLEMENT_KINDS.includes(f.kind))}{:else}{@render site(f.id)}{/if}</ul>
  {/if}
{/snippet}

<!-- A settlement's or site's districts, businesses, towers and ways underground (the row `key`'s);
     a settlement's places last. -->
{#snippet layout(owner: string, title: string, town: boolean)}
  {@const v = inside[owner]}
  {@const of = featureById.get(owner)}
  {@const ownerMatches = !!of && matches(of.id, of.name)}
  {#if failed[owner]}
    <li class="muted">The map's generator could not list these. Reload the page and try again.</li>
  {:else if v === null}
    <li class="muted wait">Laying out {title}…</li>
  {:else if v}
    {#each CHILD_GROUPS as [kind, label] (kind)}
      {@const all = v.filter((e) => e.kind === kind && (showHidden || !hiddenSet.has(e.id)))}
      {@const rows = numbered(all).filter((r) => ownerMatches || matches(r.e.id, r.label) || opened[r.e.id])}
      {#if rows.length}
        {@const key = `${owner}/${kind}`}
        {@const open = opened[key] ?? (kind === 'district' || kind === 'underground' || !!needle)}
        <li>
          <div class="row sub">
            {@render caret(key, open, () => (opened[key] = !open))}
            <span class="group">{label}</span>
            <span class="muted">{rows.length}</span>
          </div>
          {#if open}
            <ul>
              {#each rows.sort((a, b) => nameOf(a.e.id, a.label).localeCompare(nameOf(b.e.id, b.label), undefined, { numeric: true })) as { e, label } (e.id)}
                <li>
                  <div class="row">
                    {#if e.enter}{@render caret(e.id, !!opened[e.id], () => expand(e.id, e.id))}{:else}<span class="pad"></span>{/if}
                    {@render name(e.id, e.generated, label)}
                    {@render acts(e.id, e.generated, e.enter)}
                  </div>
                  {#if e.enter && opened[e.id]}<ul>{@render site(e.id)}</ul>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/if}
    {/each}
    <!-- (Its buildings drawn by hand that have a trade are among its businesses already.) -->
    {@const listed = new Set(v.map((e) => e.id))}
    {@const places = town ? placesIn(owner).filter((f) => (showHidden || !hiddenSet.has(f.id)) && !listed.has(buildingOf(f.id) ?? '') && (ownerMatches || matches(f.id, f.name) || opened[f.id])) : []}
    {#if places.length}
      {@const key = `${owner}/places`}
      {@const open = opened[key] ?? true}
      <li>
        <div class="row sub">
          {@render caret(key, open, () => (opened[key] = !open))}
          <span class="group">Places</span>
          <span class="muted">{places.length}</span>
        </div>
        {#if open}
          <ul>
            {#each places.sort((a, b) => nameOf(a.id, a.name).localeCompare(nameOf(b.id, b.name), undefined, { numeric: true })) as f (f.id)}
              <li>{@render place(f, what(f))}</li>
            {/each}
          </ul>
        {/if}
      </li>
    {/if}
    {#if !v.length && !places.length}<li class="muted">Nothing here to name.</li>{/if}
  {/if}
{/snippet}

<aside class="gazetteer" aria-label="Gazetteer">
  {#if current}
    <!-- A place's page: its name (the players see it), its label hidden or shown, its note. -->
    {@const id = current}
    {@const generated = generatedOf(id)}
    {@const note = notes[id]}
    <div class="ws-row top">
      <button class="ws-btn quiet back" onclick={() => (current = null)}><Icon name="chevron-left" size={16} /> Gazetteer</button>
      <span class="grow"></span>
      <button class="ws-btn" onclick={() => onShow(id)}><Icon name="target" size={16} /> Show on map</button>
      {#if note}<button class="ws-btn" class:quiet={!confirmDelete} class:danger={confirmDelete} onclick={() => (confirmDelete ? ((confirmDelete = false), onNote(id, null, placeName(id))) : (confirmDelete = true))} title="Delete the note">{#if confirmDelete}Delete the note?{:else}<Icon name="trash" size={16} />{/if}</button>{/if}
    </div>
    <label class="ws-field">
      Name · the players see it
      <input
        class="ws-input title"
        value={nameOf(id, generated)}
        onchange={(e) => {
          setName(id, val(e));
          if (!val(e).trim()) e.currentTarget.value = generated;
        }}
        onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
      />
    </label>
    {#if id in renames}
      <div class="was-line"><span class="ws-muted">Generated: {generated}</span><button class="ws-btn quiet" onclick={() => onRename(id, '')}><Icon name="reset" size={14} /> Back to it</button></div>
    {/if}
    {#if featureById.has(id)}
      <label class="switch">
        <span class="grow">Hidden<span class="ws-muted"> · on both maps and in search</span></span>
        <input type="checkbox" class="ws-switch" checked={hiddenSet.has(id)} onchange={(e) => onHide(id, e.currentTarget.checked)} />
      </label>
    {/if}
    <label class="ws-field">Note · only you see it<textarea class="ws-input grow-text" rows="6" value={note?.text ?? ''} placeholder="Lore, hooks, secrets…" onchange={(e) => setNote(id, val(e), note?.tags ?? [])}></textarea></label>
    <label class="ws-field">Tags<input class="ws-input" value={(note?.tags ?? []).join(', ')} placeholder="comma, separated" onchange={(e) => setNote(id, note?.text ?? '', tagList(val(e)))} /></label>
  {:else}
    {#if !withNotes}
      <div class="ws-seg" role="tablist">
        {#each TABS as t (t.key)}
          <button role="tab" aria-selected={tab === t.key} class:on={tab === t.key} onclick={() => (tab = t.key)}>{t.label}</button>
        {/each}
      </div>
    {/if}
    <input class="ws-input" type="search" placeholder={withNotes ? 'Find notes…' : 'Find a name or note…'} bind:value={q} aria-label="Find a name or note" />
    <div class="bar">
      <button class="ws-chip" class:on={withNotes} aria-pressed={withNotes} onclick={() => (withNotes = !withNotes)} title="Only the places you wrote notes on"><Icon name="note" size={12} /> With notes ({Object.keys(notes).length})</button>
      {#if !withNotes}<button class="ws-chip" class:on={showHidden} aria-pressed={showHidden} onclick={() => (showHidden = !showHidden)} title="Names hidden from both maps">Show hidden</button>{/if}
      {#if here}<button class="ws-chip here" onclick={() => openPage(here.id)} title="{here.name}: its name and your note on it"><Icon name="pin" size={12} /> <span>{here.name}</span></button>{/if}
    </div>
    <div class="hint">{withNotes ? 'Click a place for its name and note' : 'Click a name to rename it'}: the players see names, never notes.</div>
    {#if withNotes}
      <ul class="list notes">
        {#each noteList as [k, n] (k)}
          <li>
            <button class="noted" onclick={() => openPage(k)}>
              <span class="nname">{placeName(k)}</span>
              <span class="muted snippet">{n.text}{(n.tags ?? []).length ? ` · ${(n.tags ?? []).join(', ')}` : ''}</span>
            </button>
          </li>
        {:else}
          <li class="muted">{Object.keys(notes).length ? 'No notes match.' : 'No notes yet. Choose a place and write on its card, or on its row here.'}</li>
        {/each}
      </ul>
    {:else}
      <ul class="list" bind:this={listEl}>
        {#each groups as g (g.kind)}
          {@const open = isOpen(`kind/${g.kind}`, g.rows.length)}
          <li>
            <div class="row head">
              {@render caret(`kind/${g.kind}`, open, () => toggle(`kind/${g.kind}`, g.rows.length))}
              <span class="group">{kindLabel(g.kind)}</span>
              <span class="muted">{g.rows.length}</span>
            </div>
            {#if open}
              <ul>
                {#each g.rows as f (f.id)}
                  <li>{@render place(f)}</li>
                {/each}
              </ul>
            {/if}
          </li>
        {:else}
          <li class="muted">{needle ? 'No names match.' : 'Nothing here.'}</li>
        {/each}
      </ul>
    {/if}
  {/if}
</aside>

<style>
  .gazetteer {
    display: flex;
    flex-direction: column;
    gap: 5px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  input.edit {
    box-sizing: border-box;
    font: inherit;
    color: inherit;
    background: #fbf7ec;
    border: 1px solid #8a7a60;
    border-radius: 3px;
    padding: 2px 5px;
    min-width: 0;
  }
  .top {
    margin: -4px 0 0;
  }
  .back {
    padding-left: 2px;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-size: 16px;
    font-weight: bold;
  }
  .was-line {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .grow-text {
    field-sizing: content;
    min-height: 3em;
    max-height: 40vh;
  }
  .here {
    min-width: 0;
    max-width: 100%;
  }
  .here span {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .acts {
    margin-left: auto;
    display: flex;
    flex: none;
    align-items: baseline;
  }
  /* (Both classes: `.icon`'s `all: unset` would undo these otherwise.) */
  .icon.note {
    opacity: 0.3;
  }
  .icon.note.has,
  .row:hover .icon.note,
  .icon.note:focus-visible {
    opacity: 1;
  }
  .icon.note.has {
    color: var(--gold);
  }
  @media (pointer: coarse) {
    /* (No hover on touch: a little stronger.) */
    .icon.note {
      opacity: 0.55;
    }
  }
  /* (Each segment as wide as its label: "Settlements" fits a narrow dock.) */
  .ws-seg > button {
    flex: 1 1 auto;
    padding-inline: 4px;
  }
  .mark {
    display: inline-flex;
    align-self: center;
    color: #7a6a55;
    flex: none;
  }
  .noted {
    all: unset;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: 100%;
    padding: 3px 6px;
    border-radius: 3px;
    cursor: pointer;
    line-height: 1.25;
  }
  .noted:hover,
  .noted:focus-visible {
    background: rgba(226, 213, 176, 0.6);
  }
  .noted > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nname {
    font-weight: bold;
  }
  .nname:first-letter {
    text-transform: uppercase;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: center;
  }
  .hint {
    font-size: 11px;
    color: #7a6a55;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 0 0 14px;
  }
  ul.list {
    padding: 0;
    overflow-y: auto;
    min-height: 0;
  }
  @media (pointer: coarse) {
    .row {
      min-height: 40px;
      align-items: center;
    }
    .icon {
      padding: 8px;
    }
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 4px;
    min-width: 0;
    border-radius: 3px;
  }
  .row:hover {
    background: rgba(226, 213, 176, 0.6);
  }
  .row.flash {
    background: rgba(201, 162, 39, 0.35);
    transition: background 0.4s;
  }
  .kind {
    display: inline-flex;
    align-self: center;
    flex: none;
    color: #7a5530;
  }
  .what,
  .in {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .in {
    all: unset;
    color: #7a6a55;
    font-size: 12px;
    cursor: pointer;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .in:hover,
  .in:focus-visible {
    text-decoration: underline;
  }
  .head {
    border-top: 1px solid rgba(58, 50, 42, 0.2);
    padding-top: 2px;
    margin-top: 2px;
  }
  .group {
    font-weight: bold;
    text-transform: capitalize;
  }
  .sub .group {
    font-weight: normal;
    font-style: italic;
  }
  .name {
    all: unset;
    cursor: text;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:first-letter {
    text-transform: uppercase;
  }
  .name.renamed {
    color: #6b3a10;
    font-weight: bold;
  }
  .name:hover,
  .name:focus-visible {
    text-decoration: underline dotted;
  }
  .was {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .edit {
    flex: 1;
    font-size: 13px;
  }
  .icon {
    all: unset;
    cursor: pointer;
    padding: 0 3px;
    color: #7a5530;
    flex: none;
  }
  .icon:hover {
    color: #2b241d;
  }
  .caret,
  .pad {
    width: 12px;
    flex: none;
    text-align: center;
    padding: 0;
  }
  .muted {
    color: #7a6a55;
    font-size: 12px;
  }
  li.muted {
    padding: 2px 4px;
    font-style: italic;
  }
</style>
