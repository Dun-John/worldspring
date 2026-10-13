<script lang="ts" module>
  import type { Npc, Plot } from '../gen/protocol';

  /** The place selected or entered: NPCs can be put there, plots tied to it. */
  export interface Here {
    id: string;
    name: string;
    /** Inside a building or site: the level in view. */
    level?: number;
  }

  /** A new NPC or plot id: random, so the app and an agent creating at once never collide. */
  export function newNoteId(prefix: 'n' | 'p'): string {
    const b = new Uint8Array(8);
    crypto.getRandomValues(b);
    return `${prefix}:${[...b].map((x) => '0123456789abcdefghijklmnopqrstuvwxyz'[x % 36]).join('')}`;
  }

  export const blankNpc = (name = 'New NPC'): Npc => ({ name, appearance: '', mannerisms: '', attitude: { stance: 'neutral', text: '' }, goals: '', notes: '', tags: [] });
  export const blankPlot = (title = 'New plot point'): Plot => ({ title, text: '', status: 'idea', anchors: [], npcs: [], tags: [] });
</script>

<script lang="ts">
  // The DM's notebook: NPCs (who they are, how they feel about the players, where they are) and
  // plot points tied to places and NPCs (notes on places are in the Gazetteer). Agents read and
  // write the same entries through mapd. Fields save when they lose focus.
  import { PLOT_STATUSES, STANCES, type PlaceInfo, type PlotStatus, type Stance } from '../gen/protocol';
  import { assetUrl, putAsset } from '../world/assets';
  import { unnamed } from './gazetteer';
  import type { NotesTab } from './shell/layout.svelte';
  import Icon from './Icon.svelte';

  type Tab = 'npcs' | 'plots';

  interface Props {
    npcs: Record<string, Npc>;
    plots: Record<string, Plot>;
    /** Names as renamed (place names are looked up again when they change). */
    renames: Record<string, string>;
    /** The place selected or entered: NPCs can be put there, plots tied to it. */
    here: Here | null;
    /** Open on this entry (set by the info panel and search; `seq` makes a repeat count). */
    focus: { tab: NotesTab; id: string | null; seq: number };
    playing: boolean;
    resolve: (id: string) => Promise<PlaceInfo | null>;
    /** A level's name in the building or site in view (else null: shown by number). */
    levelName: (id: string, level: number) => string | null;
    onNpc: (id: string, npc: Npc | null, tool: string) => void;
    onPlot: (id: string, plot: Plot | null, tool: string) => void;
    /** Go to a place, or to where an NPC is. */
    onGo: (id: string) => void;
    /** Click on the map to put this NPC there. */
    onPick: (id: string) => void;
    /** Drop this NPC into play as a token. */
    onToken: (id: string) => void;
    /** Say what went wrong (a picture refused). */
    onError: (text: string) => void;
    /** Show another entry (an NPC's plot, a plot's NPC): the tab follows. */
    onFocus: (tab: NotesTab, id: string | null) => void;
  }
  let { npcs, plots, renames, here, focus, playing, resolve, levelName, onNpc, onPlot, onGo, onPick, onToken, onError, onFocus }: Props = $props();

  // The tab is the dock's (NPCs, Plots); the entry open is the last one asked for.
  const tab = $derived(focus.tab);
  let current = $state<string | null>(null);
  let q = $state('');
  let filter = $state('');
  let hereOnly = $state(false);
  let confirmDelete = $state(false);
  /** The ⋯ menu of the entry open. */
  let more = $state(false);

  let lastTab = '';
  $effect(() => {
    void focus.seq;
    current = focus.id;
    confirmDelete = false;
    more = false;
    if (focus.tab !== lastTab) {
      lastTab = focus.tab;
      filter = '';
    }
  });

  const STANCE_COLOR: Record<Stance, string> = { hostile: '#991b1b', unfriendly: '#c2410c', neutral: '#6b5a45', friendly: '#3f6212', allied: '#1d4ed8' };

  // Place names before renames, looked up once from the generators (buildings, districts,
  // sites; a level or room generates its site, so not again on every rename), and again for
  // another world or this one remade (a new `resolve`).
  let names = $state<Record<string, string>>({});
  let namesOf: unknown = null;
  const placeIds = $derived([...new Set([...Object.values(npcs).flatMap((n) => (n.location ? [n.location.id] : [])), ...Object.values(plots).flatMap((p) => p.anchors)])]);
  $effect(() => {
    const ask = resolve;
    if (namesOf !== ask) [namesOf, names] = [ask, {}];
    for (const id of placeIds) {
      if (id in names) continue;
      names[id] = '…';
      void ask(id).then((p) => {
        if (namesOf === ask) names[id] = p?.generated ?? p?.name ?? unnamed(id);
      });
    }
  });
  const placeName = (id: string) => renames[id] ?? names[id] ?? unnamed(id);

  // Portraits as object URLs.
  let pictures = $state<Record<string, string>>({});
  $effect(() => {
    for (const n of Object.values(npcs)) {
      const id = n.portrait;
      if (id && !(id in pictures)) {
        pictures[id] = '';
        void assetUrl(id).then((u) => (pictures[id] = u ?? ''));
      }
    }
  });

  const has = (s: string | undefined, t: string) => !!s && s.toLowerCase().includes(t);
  const tagList = (s: string) => [...new Set(s.split(',').map((t) => t.trim()).filter(Boolean))];

  const npcList = $derived.by(() => {
    const t = q.trim().toLowerCase();
    return Object.entries(npcs)
      .filter(([, n]) => !t || [n.name, n.appearance, n.mannerisms, n.goals, n.notes, n.attitude.text, n.status, ...n.tags].some((s) => has(s, t)) || (n.location && has(placeName(n.location.id), t)))
      .filter(([, n]) => !filter || n.attitude.stance === filter)
      .filter(([, n]) => !hereOnly || (here && n.location?.id === here.id))
      .sort(([, a], [, b]) => a.name.localeCompare(b.name));
  });

  const plotList = $derived.by(() => {
    const t = q.trim().toLowerCase();
    const order: Record<PlotStatus, number> = { active: 0, idea: 1, resolved: 2 };
    return Object.entries(plots)
      .filter(([, p]) => !t || [p.title, p.text, ...p.tags].some((s) => has(s, t)) || p.npcs.some((k) => has(npcs[k]?.name, t)))
      .filter(([, p]) => !filter || p.status === filter)
      .filter(([, p]) => !hereOnly || (here && p.anchors.includes(here.id)))
      .sort(([, a], [, b]) => order[a.status] - order[b.status] || a.title.localeCompare(b.title));
  });

  function open(t: Tab, id: string) {
    if (t !== tab) return onFocus(t, id);
    current = id;
    confirmDelete = false;
    more = false;
  }

  function create() {
    const id = newNoteId(tab === 'npcs' ? 'n' : 'p');
    if (tab === 'npcs') onNpc(id, { ...blankNpc(), ...(hereOnly && here ? { location: { id: here.id, ...(here.level !== undefined ? { level: here.level } : {}) } } : {}) }, 'create_npc');
    else onPlot(id, { ...blankPlot(), anchors: hereOnly && here ? [here.id] : [] }, 'create_plot');
    current = id;
  }

  const npc = $derived(tab === 'npcs' && current ? npcs[current] : undefined);
  const plot = $derived(tab === 'plots' && current ? plots[current] : undefined);

  function setNpc(patch: Partial<Npc>, tool = 'update_npc') {
    if (!current || !npc) return;
    const next = { ...$state.snapshot(npc), ...patch } as Npc;
    if (!next.name.trim()) return;
    onNpc(current, next, tool);
  }

  function setPlot(patch: Partial<Plot>) {
    if (!current || !plot) return;
    const next = { ...$state.snapshot(plot), ...patch } as Plot;
    if (!next.title.trim()) return;
    onPlot(current, next, 'update_plot');
  }

  const val = (e: Event) => (e.currentTarget as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement).value;

  async function portrait(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      setNpc({ portrait: await putAsset(file, file.name) });
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  function remove() {
    if (!current) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    if (tab === 'npcs') onNpc(current, null, 'delete_npc');
    else onPlot(current, null, 'delete_plot');
    current = null;
    confirmDelete = false;
  }

  function placeHere() {
    if (!here) return;
    setNpc({ location: { id: here.id, ...(here.level !== undefined ? { level: here.level } : {}) } }, 'place_npc');
  }

  function unplace() {
    if (!npc) return;
    const { location: _, ...rest } = $state.snapshot(npc) as Npc;
    if (current) onNpc(current, rest, 'place_npc');
  }

  function plotWith(id: string) {
    const p = newNoteId('p');
    onPlot(p, { ...blankPlot(), npcs: [id], anchors: npcs[id]?.location ? [npcs[id].location!.id] : [] }, 'create_plot');
    open('plots', p);
  }

  const npcPlots = $derived(current && npc ? Object.entries(plots).filter(([, p]) => p.npcs.includes(current!)) : []);
  const otherNpcs = $derived(plot ? Object.entries(npcs).filter(([k]) => !plot.npcs.includes(k)).sort(([, a], [, b]) => a.name.localeCompare(b.name)) : []);
</script>

<div class="notebook">
  {#if npc && current}
    {@const id = current}
    <div class="ws-row top">
      <button class="ws-btn quiet back" onclick={() => (current = null)}><Icon name="chevron-left" size={16} /> NPCs</button>
      <span class="grow"></span>
      {#if npc.location}<button class="ws-btn" onclick={() => onGo(id)} title="Go to where they are"><Icon name="target" size={16} /> Go to</button>{/if}
      <div class="more-wrap">
        <button class="ws-icon-btn" onclick={() => ((more = !more), (confirmDelete = false))} aria-label="More" aria-expanded={more}><Icon name="more" /></button>
        {#if more}
          <div class="menu ws-panel" role="menu">
            {#if playing}<button class="ws-btn quiet" role="menuitem" onclick={() => ((more = false), onToken(id))}><Icon name="token" size={16} /> Drop as token</button>{/if}
            {#if npc.portrait}<button class="ws-btn quiet" role="menuitem" onclick={() => ((more = false), setNpc({ portrait: undefined }))}><Icon name="image" size={16} /> Remove portrait</button>{/if}
            <button class="ws-btn" class:quiet={!confirmDelete} class:danger={confirmDelete} role="menuitem" onclick={remove}><Icon name="trash" size={16} /> {confirmDelete ? 'Delete for good?' : 'Delete'}</button>
          </div>
        {/if}
      </div>
    </div>
    <div class="who">
      <label class="pic" title="Choose a portrait">
        {#if npc.portrait && pictures[npc.portrait]}<img src={pictures[npc.portrait]} alt="" />{:else}<Icon name="user" size={30} />{/if}
        <input type="file" accept="image/*" onchange={portrait} hidden />
      </label>
      <div class="grow col">
        <input class="ws-input title" value={npc.name} onchange={(e) => setNpc({ name: val(e).trim() || npc.name })} aria-label="Name" />
        <div class="ws-row">
          <select class="ws-input stance" value={npc.attitude.stance} onchange={(e) => setNpc({ attitude: { ...npc.attitude, stance: val(e) as Stance } })} aria-label="Attitude toward the players" style:color={STANCE_COLOR[npc.attitude.stance]}>
            {#each STANCES as s (s)}<option value={s}>{s}</option>{/each}
          </select>
          <input class="ws-input grow" placeholder="Status (alive, missing…)" value={npc.status ?? ''} onchange={(e) => setNpc({ status: val(e).trim() || undefined })} aria-label="Status" />
        </div>
      </div>
    </div>
    <section>
      <div class="ws-label">Where</div>
      <div class="where">{#if npc.location}{placeName(npc.location.id)}{npc.location.level !== undefined ? ` · ${levelName(npc.location.id, npc.location.level) ?? `level ${npc.location.level + 1}`}` : ''}{npc.location.x !== undefined ? ' · placed' : ''}{:else}<span class="ws-muted">Not placed yet</span>{/if}</div>
      <div class="buttons">
        {#if here && here.id !== npc.location?.id}<button class="ws-btn" onclick={placeHere} title="Put them in {here.name}"><Icon name="pin" size={16} /> {here.name}</button>{/if}
        <button class="ws-btn" onclick={() => onPick(id)} title="Click on the map (or inside a building) to put them there"><Icon name="target" size={16} /> Pick on map</button>
        {#if npc.location}<button class="ws-btn quiet" onclick={unplace}>Clear</button>{/if}
      </div>
    </section>
    <label class="ws-field">Attitude toward the players<textarea class="ws-input grow-text" rows="2" value={npc.attitude.text} onchange={(e) => setNpc({ attitude: { ...npc.attitude, text: val(e) } })} placeholder="Why they feel that way; what would change it"></textarea></label>
    <label class="ws-field">Goals<textarea class="ws-input grow-text" rows="2" value={npc.goals} onchange={(e) => setNpc({ goals: val(e) })}></textarea></label>
    <label class="ws-field">Appearance<textarea class="ws-input grow-text" rows="2" value={npc.appearance} onchange={(e) => setNpc({ appearance: val(e) })}></textarea></label>
    <label class="ws-field">Mannerisms<textarea class="ws-input grow-text" rows="2" value={npc.mannerisms} onchange={(e) => setNpc({ mannerisms: val(e) })}></textarea></label>
    <label class="ws-field">Notes (only you see these)<textarea class="ws-input grow-text" rows="3" value={npc.notes} onchange={(e) => setNpc({ notes: val(e) })}></textarea></label>
    <label class="ws-field">Tags<input class="ws-input" value={npc.tags.join(', ')} onchange={(e) => setNpc({ tags: tagList(val(e)) })} placeholder="comma, separated" /></label>
    <section>
      <div class="ws-label">Plots</div>
      {#each npcPlots as [k, p] (k)}
        <button class="item" onclick={() => open('plots', k)}><span class="status {p.status}">{p.status}</span> <span class="name">{p.title}</span></button>
      {/each}
      <div class="buttons"><button class="ws-btn" onclick={() => plotWith(id)}><Icon name="plus" size={16} /> Plot with them</button></div>
    </section>
  {:else if plot && current}
    <div class="ws-row top">
      <button class="ws-btn quiet back" onclick={() => (current = null)}><Icon name="chevron-left" size={16} /> Plots</button>
      <span class="grow"></span>
      <button class="ws-btn" class:quiet={!confirmDelete} class:danger={confirmDelete} onclick={remove}><Icon name="trash" size={16} /> {confirmDelete ? 'Delete for good?' : 'Delete'}</button>
    </div>
    <input class="ws-input title" value={plot.title} onchange={(e) => setPlot({ title: val(e).trim() || plot.title })} aria-label="Title" />
    <div class="ws-seg" role="radiogroup" aria-label="Status">
      {#each PLOT_STATUSES as s (s)}<button class:on={plot.status === s} onclick={() => setPlot({ status: s })}>{s.charAt(0).toUpperCase() + s.slice(1)}</button>{/each}
    </div>
    <label class="ws-field">What is going on<textarea class="ws-input grow-text" rows="5" value={plot.text} onchange={(e) => setPlot({ text: val(e) })}></textarea></label>
    <section>
      <div class="ws-label">Places</div>
      <div class="chips">
        {#each plot.anchors as a (a)}
          <span class="ws-chip"><button class="link" onclick={() => onGo(a)} title="Go there">{placeName(a)}</button><button class="x" onclick={() => setPlot({ anchors: plot.anchors.filter((x) => x !== a) })} aria-label="Remove {placeName(a)}"><Icon name="x" size={12} /></button></span>
        {/each}
        {#if here && !plot.anchors.includes(here.id)}<button class="ws-chip add" onclick={() => setPlot({ anchors: [...plot.anchors, here.id] })}><Icon name="plus" size={12} /> {here.name}</button>{/if}
      </div>
      {#if !here && !plot.anchors.length}<div class="ws-muted">Select a place on the map to tie this to it.</div>{/if}
    </section>
    <section>
      <div class="ws-label">NPCs</div>
      <div class="chips">
        {#each plot.npcs as k (k)}
          <span class="ws-chip"><button class="link" onclick={() => open('npcs', k)}>{npcs[k]?.name ?? k}</button><button class="x" onclick={() => setPlot({ npcs: plot.npcs.filter((x) => x !== k) })} aria-label="Remove"><Icon name="x" size={12} /></button></span>
        {/each}
      </div>
      {#if otherNpcs.length}
        <select
          class="ws-input"
          value=""
          onchange={(e) => {
            const k = val(e);
            if (k) setPlot({ npcs: [...plot.npcs, k] });
            (e.currentTarget as HTMLSelectElement).value = '';
          }}
          aria-label="Add an NPC"
        >
          <option value="">+ Add an NPC…</option>
          {#each otherNpcs as [k, n] (k)}<option value={k}>{n.name}</option>{/each}
        </select>
      {/if}
    </section>
    <label class="ws-field">Tags<input class="ws-input" value={plot.tags.join(', ')} onchange={(e) => setPlot({ tags: tagList(val(e)) })} placeholder="comma, separated" /></label>
  {:else}
    <div class="ws-row">
      <input class="ws-input grow" type="search" placeholder={tab === 'npcs' ? 'Find NPCs…' : 'Find plots…'} bind:value={q} aria-label="Search the notebook" />
      <button class="ws-btn primary" onclick={create}><Icon name="plus" size={16} /> {tab === 'npcs' ? 'NPC' : 'Plot'}</button>
    </div>
    <div class="chips">
      {#if here}<button class="ws-chip" class:on={hereOnly} aria-pressed={hereOnly} onclick={() => (hereOnly = !hereOnly)} title="Only those at {here.name}"><Icon name="pin" size={12} /> At {here.name}</button>{/if}
      {#if tab === 'npcs'}
        {#each STANCES as s (s)}
          <button class="ws-chip" class:on={filter === s} aria-pressed={filter === s} onclick={() => (filter = filter === s ? '' : s)}><span class="dot" style:background={STANCE_COLOR[s]}></span>{s}</button>
        {/each}
      {:else}
        {#each PLOT_STATUSES as s (s)}
          <button class="ws-chip" class:on={filter === s} aria-pressed={filter === s} onclick={() => (filter = filter === s ? '' : s)}>{s}</button>
        {/each}
      {/if}
    </div>
    <ul>
      {#if tab === 'npcs'}
        {#each npcList as [k, n] (k)}
          <li>
            <button class="row" onclick={() => open('npcs', k)}>
              <span class="thumb">{#if n.portrait && pictures[n.portrait]}<img src={pictures[n.portrait]} alt="" />{:else}<Icon name="user" size={18} />{/if}<i class="dot" style:background={STANCE_COLOR[n.attitude.stance]} title={n.attitude.stance}></i></span>
              <span class="text">
                <span class="name">{n.name}</span>
                <span class="ws-muted">{[n.status, n.location ? placeName(n.location.id) : ''].filter(Boolean).join(' · ') || n.attitude.stance}</span>
              </span>
            </button>
          </li>
        {:else}
          <li class="empty">{Object.keys(npcs).length ? 'No NPCs match.' : 'No NPCs yet. Add one here, or with + NPC on a place’s card.'}</li>
        {/each}
      {:else}
        {#each plotList as [k, p] (k)}
          <li>
            <button class="row" onclick={() => open('plots', k)}>
              <span class="status {p.status}">{p.status}</span>
              <span class="text">
                <span class="name">{p.title}</span>
                {#if p.anchors.length}<span class="ws-muted">{placeName(p.anchors[0])}{p.anchors.length > 1 ? ` +${p.anchors.length - 1}` : ''}</span>{/if}
              </span>
            </button>
          </li>
        {:else}
          <li class="empty">{Object.keys(plots).length ? 'No plot points match.' : 'No plot points yet.'}</li>
        {/each}
      {/if}
    </ul>
  {/if}
</div>

<style>
  .notebook {
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
  .col {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .top {
    margin: -4px 0 0;
  }
  .back {
    padding-left: 2px;
  }
  .title {
    font-size: 16px;
    font-weight: bold;
    width: 100%;
  }
  .who {
    display: flex;
    gap: 8px;
    align-items: flex-start;
  }
  .pic {
    width: 64px;
    height: 64px;
    flex: none;
    border: 1px solid var(--line-field);
    border-radius: var(--radius);
    overflow: hidden;
    cursor: pointer;
    display: grid;
    place-items: center;
    background: var(--btn);
    color: var(--ink-3);
  }
  .pic img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .stance {
    text-transform: capitalize;
    font-weight: bold;
  }
  .grow-text {
    field-sizing: content;
    min-height: 3em;
    max-height: 40vh;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 6px;
    border-top: 1px solid var(--line-faint);
  }
  .buttons,
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .ws-chip .link,
  .ws-chip .x {
    all: unset;
    cursor: pointer;
  }
  .ws-chip .link:hover {
    text-decoration: underline;
  }
  .ws-chip .x {
    display: inline-flex;
    margin-right: -4px;
    padding: 2px;
  }
  .add {
    border-style: dashed;
  }
  .more-wrap {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 3;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    min-width: 190px;
    background: var(--paper-solid);
    box-shadow: var(--shadow-lg);
  }
  .menu .ws-btn {
    justify-content: flex-start;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .row,
  .item {
    all: unset;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: calc(var(--tap) + 8px);
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }
  .row:hover,
  .row:focus-visible,
  .item:hover {
    background: var(--btn-hover);
  }
  .thumb {
    position: relative;
    flex: none;
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--btn);
    color: var(--ink-3);
    overflow: visible;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    border-radius: 50%;
    object-fit: cover;
  }
  .thumb .dot {
    position: absolute;
    right: -1px;
    bottom: -1px;
    box-shadow: 0 0 0 2px var(--paper-solid);
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }
  .text > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name {
    font-weight: bold;
  }
  .dot {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .ws-chip .dot {
    width: 8px;
    height: 8px;
  }
  .status {
    flex: none;
    font-size: 11px;
    padding: 0 5px;
    border-radius: 3px;
    background: #d6c9a8;
  }
  .status.active {
    background: var(--gold);
    color: #fff7ed;
  }
  .status.resolved {
    background: #c9c2b0;
    color: #6b6255;
  }
  .where {
    font-weight: bold;
  }
  .empty {
    padding: 8px 4px;
    font-style: italic;
    color: var(--ink-3);
  }
  .chips :global(.ws-chip) {
    text-transform: capitalize;
  }
</style>
