<script lang="ts">
  // The names menu: every name in the world, renamed from a list. Natural features, then
  // settlements and sites; a settlement or site opens onto its districts, businesses, towers and
  // ways underground (its layout is generated then), a building or site onto its levels and rooms.
  import type { Feature, NameEntry } from '../gen/protocol';
  import { kindLabel, SETTLEMENT_KINDS } from './gazetteer';

  interface Props {
    /** The map's named features (created sites included). */
    features: Feature[];
    renames: Record<string, string>;
    hidden: string[];
    /** A settlement's or site's layout index, else -1. */
    layoutOf: (id: string) => number;
    /** What can be renamed in a layout (by index) or a building or site (by id). */
    list: (scope: number | string) => Promise<NameEntry[] | null>;
    /** Rename (an empty name goes back to the generated one). */
    onRename: (id: string, name: string) => void;
    /** Fly to a place, or go inside a building or site (on a level's or room's level). */
    onGo: (id: string) => void;
  }
  let { features, renames, hidden, layoutOf, list, onRename, onGo }: Props = $props();

  const SITE_KINDS = ['ruin', 'tower', 'camp', 'waystation', 'cave', 'mine', 'lava_tube', 'entrance', 'building'];
  const TABS = [
    { key: 'water', label: 'Waters', kinds: ['ocean', 'sea', 'bay', 'strait', 'lake', 'salt_lake', 'river', 'waterfall'] },
    { key: 'land', label: 'Land', kinds: ['continent', 'island', 'range', 'peak', 'pass', 'volcano', 'forest', 'jungle', 'taiga', 'desert', 'swamp', 'plains', 'tundra', 'glacier', 'blight', 'ashlands', 'region', 'salt_flat'] },
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

  const hiddenSet = $derived(new Set(hidden));
  const needle = $derived(q.trim().toLowerCase());
  const nameOf = (id: string, generated: string) => renames[id] ?? generated;
  const matches = (id: string, generated: string) => !needle || nameOf(id, generated).toLowerCase().includes(needle) || generated.toLowerCase().includes(needle);

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
    if (opened[id] && !(id in inside)) {
      inside[id] = null;
      failed[id] = false;
      void list(scope).then((v) => {
        failed[id] = !v;
        if (v) inside[id] = v;
        // (Asked again the next time it is opened.)
        else delete inside[id];
      });
    }
  }

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
  }

  function focus(node: HTMLInputElement) {
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
      use:focus
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
    {#if hiddenSet.has(id)}<span class="muted">(hidden)</span>{/if}
  {/if}
{/snippet}

{#snippet go(id: string, enter = false)}
  <button class="icon go" onclick={() => onGo(id)} title={enter ? 'Go inside' : 'Go to'} aria-label={enter ? 'Go inside' : 'Go to'}>➜</button>
{/snippet}

{#snippet caret(id: string, open: boolean, onclick: () => void)}
  <button class="icon caret" {onclick} aria-expanded={open} aria-label={open ? 'Shut' : 'Open'}>{open ? '▾' : '▸'}</button>
{/snippet}

<!-- A building's or site's levels and their rooms. -->
{#snippet site(e: NameEntry)}
  {@const v = inside[e.id]}
  {#if failed[e.id]}
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
          {@render go(l.level.id)}
        </div>
        {#if lopen || (needle && rooms.length)}
          <ul>
            {#each rooms as r (r.e.id)}
              <li><div class="row"><span class="pad"></span>{@render name(r.e.id, r.e.generated, r.label)}{@render go(r.e.id)}</div></li>
            {/each}
          </ul>
        {/if}
      </li>
    {:else}
      <li class="muted">Nothing inside to name.</li>
    {/each}
  {/if}
{/snippet}

<!-- A settlement's or site's districts, businesses, towers and ways underground. -->
{#snippet layout(f: Feature)}
  {@const v = inside[f.id]}
  {#if failed[f.id]}
    <li class="muted">The map's generator could not list these. Reload the page and try again.</li>
  {:else if v === null}
    <li class="muted wait">Laying out {renames[f.id] ?? f.name}…</li>
  {:else if v}
    {#each CHILD_GROUPS as [kind, label] (kind)}
      {@const all = v.filter((e) => e.kind === kind && (showHidden || !hiddenSet.has(e.id)))}
      {@const rows = numbered(all).filter((r) => matches(f.id, f.name) || matches(r.e.id, r.label) || opened[r.e.id])}
      {#if rows.length}
        {@const key = `${f.id}/${kind}`}
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
                    {@render go(e.id, e.enter)}
                  </div>
                  {#if e.enter && opened[e.id]}<ul>{@render site(e)}</ul>{/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/if}
    {/each}
    {#if !v.length}<li class="muted">Nothing here to name.</li>{/if}
  {/if}
{/snippet}

<aside class="names" aria-label="Names">
  <div class="ws-seg" role="tablist">
    {#each TABS as t (t.key)}
      <button role="tab" aria-selected={tab === t.key} class:on={tab === t.key} onclick={() => (tab = t.key)}>{t.label}</button>
    {/each}
  </div>
  <div class="bar">
    <input class="ws-input" type="search" placeholder="Find a name…" bind:value={q} aria-label="Find a name" />
    <button class="ws-chip" class:on={showHidden} aria-pressed={showHidden} onclick={() => (showHidden = !showHidden)} title="Names hidden from the map">Show hidden</button>
  </div>
  <div class="hint">Click a name to rename it.</div>
  <ul class="list">
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
              {@const li = layoutOf(f.id)}
              <li>
                <div class="row">
                  {#if li >= 0}{@render caret(f.id, !!opened[f.id], () => expand(f.id, li))}{:else}<span class="pad"></span>{/if}
                  {@render name(f.id, f.name)}
                  {@render go(f.id)}
                </div>
                {#if li >= 0 && opened[f.id]}<ul>{@render layout(f)}</ul>{/if}
              </li>
            {/each}
          </ul>
        {/if}
      </li>
    {:else}
      <li class="muted">{needle ? 'No names match.' : 'Nothing here.'}</li>
    {/each}
  </ul>
</aside>

<style>
  .names {
    display: flex;
    flex-direction: column;
    gap: 5px;
    color: var(--ink);
    font: 13px/1.4 var(--font);
  }
  button,
  input {
    font: inherit;
    color: inherit;
  }
  input {
    box-sizing: border-box;
    background: #fbf7ec;
    border: 1px solid #8a7a60;
    border-radius: 3px;
    padding: 2px 5px;
    min-width: 0;
  }
  .bar {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .bar input[type='search'] {
    flex: 1;
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
  .go {
    margin-left: auto;
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
