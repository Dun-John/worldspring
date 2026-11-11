<script lang="ts" module>
  import type { Edits } from '../gen/protocol';
  import type { Change } from '../sync/mapd';

  /** One change asked on a settlement's card: its culture (0 north … 4 steppe), and for a village
   * or town added by hand its tier, what it lives by or its name. */
  export type SettleAsk = { culture?: number; tier?: 'village' | 'town'; lives_by?: string; name?: string };
  /** A change worked out but not made: the edits it makes and what it sets aside (by field), or
   * why it can't be made, or that it changes nothing. */
  export type SettlePlan = { edits: Edits; change: Change; set_aside: number; by: Record<string, number>; count: number } | { error: string } | { unchanged: true };

  /** The cultures, as the card names them (`town::culture::NAMES`). */
  export const CULTURES = ['North', 'West', 'South', 'Dry', 'Steppe'];
</script>

<script lang="ts">
  // A settlement card's Settlement block: its culture, changed at once for every settlement; tier,
  // what it lives by and name for a village or town added by hand (at once, laid out anew); what
  // the world's own are, their changes waiting on a remake (next). A line says what a change
  // sets aside before it is made; it is asked only when something would be.
  import { PIN_KINDS } from '../editor/sketcher';
  import Icon from './Icon.svelte';

  interface Props {
    /** Added by hand at once (`c:`): tier, living and name change here too. */
    made: boolean;
    /** Its tier (`village` … `metropolis`). */
    tier: string;
    /** What it lives by: a `PIN_KINDS` key (added by hand), else the words (the world's own). */
    livesBy: string | null;
    capital: boolean;
    /** Its culture now (null: not known yet). */
    culture: number | null;
    /** The name it was given (added by hand). */
    name: string;
    plan: (ask: SettleAsk) => Promise<SettlePlan>;
    apply: (p: Extract<SettlePlan, { edits: Edits }>) => void;
    /** Move it (added by hand): the next click puts it there. */
    onMove?: () => void;
  }
  let { made, tier, livesBy, capital, culture, name, plan, apply, onMove }: Props = $props();

  /** A change worked out that sets something aside, waiting for Apply (raw: its edits are the
   * world's, plain data to clone and save). */
  let pending = $state.raw<{ what: string; p: Extract<SettlePlan, { edits: Edits }> } | null>(null);
  let note = $state<string | null>(null);
  let busy = $state(false);
  let draft = $state('');
  $effect(() => {
    draft = name;
  });

  /** Inland livings (a village or town added by hand stands away from the shore). */
  const INLAND = PIN_KINDS.filter(([k]) => k && k !== 'port' && k !== 'fishing') as [string, string][];
  const livingWord = (k: string | null) => (k ? (PIN_KINDS.find(([x]) => x === k)?.[1] ?? k.replace(/_/g, ' ')) : 'from its land');
  const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

  /** What a change sets aside, in words ("4 building changes and its streets re-planned"). */
  function asideWords(by: Record<string, number>): string {
    const parts = (
      [
        ['buildings', 'building change', 'building changes'],
        ['towns', 'its streets re-planned', 'its streets re-planned'],
        ['designs', 'designed interior', 'designed interiors'],
        ['renames', 'name inside it', 'names inside it'],
        ['notes', 'note inside it', 'notes inside it'],
        ['hidden', 'hidden label', 'hidden labels'],
        ['npcs', 'NPC’s place inside it', 'NPCs’ places inside it'],
        ['plots', 'plot point’s place inside it', 'plot points’ places inside it'],
      ] as [string, string, string][]
    )
      .filter(([f]) => (by[f] ?? 0) > 0)
      .map(([f, one, many]) => (one.startsWith('its') ? one : `${by[f]} ${by[f] === 1 ? one : many}`));
    return parts.length > 1 ? `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}` : (parts[0] ?? '');
  }

  /** Work `ask` out; made at once unless it sets something aside (then asked, with the line). */
  async function change(ask: SettleAsk, how: string) {
    if (busy) return;
    pending = null;
    note = null;
    busy = true;
    const p = await plan(ask);
    busy = false;
    if ('error' in p) {
      note = p.error;
      return;
    }
    if ('unchanged' in p) return;
    if (p.set_aside > 0) pending = { what: `${how} Its name, notes, NPCs and plots stay; set aside (in Notes › Gazetteer): ${asideWords(p.by)}.`, p };
    else apply(p);
  }

  function rename() {
    const n = draft.trim();
    if (n && n !== name) void change({ name: n }, 'Renamed.');
    else draft = name;
  }
</script>

<div class="settle">
  <div class="ws-label">Settlement</div>
  <label class="row">
    <span class="key">Culture</span>
    <select
      class="ws-input"
      value={culture ?? -1}
      disabled={culture === null || busy}
      onchange={(e) => {
        const to = Number(e.currentTarget.value);
        e.currentTarget.value = String(culture ?? -1);
        void change({ culture: to }, `Laid out anew in the ${CULTURES[to]} culture.`);
      }}
    >
      {#if culture === null}<option value={-1}>…</option>{/if}
      {#each CULTURES as c, i (c)}<option value={i}>{c}</option>{/each}
    </select>
  </label>
  {#if made}
    <div class="row">
      <span class="key">Tier</span>
      <div class="ws-seg" role="radiogroup" aria-label="Tier">
        {#each ['village', 'town'] as t (t)}
          <button role="radio" aria-checked={tier === t} class:on={tier === t} disabled={busy} onclick={() => tier !== t && void change({ tier: t as 'village' | 'town' }, `Laid out anew as a ${t}.`)}>{cap(t)}</button>
        {/each}
      </div>
    </div>
    <label class="row">
      <span class="key">Lives by</span>
      <select
        class="ws-input"
        value={livesBy ?? ''}
        disabled={busy}
        onchange={(e) => {
          const to = e.currentTarget.value;
          e.currentTarget.value = livesBy ?? '';
          void change({ lives_by: to }, `Laid out anew, living by ${livingWord(to).toLowerCase()}.`);
        }}
      >
        {#if !livesBy}<option value="">From its land</option>{/if}
        {#each INLAND as [k, label] (k)}<option value={k}>{label}</option>{/each}
      </select>
    </label>
    <label class="row">
      <span class="key">Name</span>
      <input class="ws-input" bind:value={draft} maxlength={80} onkeydown={(e) => e.key === 'Enter' && e.currentTarget.blur()} onblur={rename} aria-label="Its name" />
    </label>
    {#if onMove}
      <div class="row">
        <span class="key"></span>
        <button class="ws-btn" onclick={onMove} title="Click the map where it goes: it keeps its name, notes, NPCs and plots; the changes made inside it are set aside"><Icon name="move" size={16} /> Move</button>
      </div>
    {/if}
    <div class="muted">Added by hand: these change at once (Undo puts them back). A new tier, living or culture lays it out anew.</div>
  {:else}
    <div class="row"><span class="key">Tier</span><span>{cap(tier)}</span></div>
    <div class="row"><span class="key">Lives by</span><span>{cap(livesBy ?? 'its land')}</span></div>
    <div class="row"><span class="key">Capital</span><span>{capital ? 'The realm’s capital' : 'No'}</span></div>
    <div class="muted">Culture changes at once. Tier, living and capital need the world remade: coming next. The world’s own settlements can’t be deleted; hide one instead (⋯ › Hide from the map).</div>
  {/if}
  {#if note}<div class="note bad">{note}</div>{/if}
  {#if pending}
    {@const q = pending}
    <div class="note">
      <span>{q.what}</span>
      <span class="btns">
        <button class="ws-btn quiet" onclick={() => (pending = null)}>Cancel</button>
        <button
          class="ws-btn primary"
          onclick={() => {
            const p = q.p;
            pending = null;
            apply(p);
          }}>Apply</button
        >
      </span>
    </div>
  {/if}
</div>

<style>
  .settle {
    padding-top: 6px;
    border-top: 1px solid var(--line-faint);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: var(--tap);
  }
  .key {
    flex: 0 0 64px;
    color: var(--ink-2);
  }
  .row select,
  .row input {
    flex: 1;
    min-width: 0;
  }
  .muted {
    color: var(--ink-3);
    font-size: 12px;
  }
  .note {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: var(--btn-hover);
    font-size: 12px;
  }
  .note.bad {
    color: var(--danger, var(--ink));
  }
  .btns {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
  }
</style>
