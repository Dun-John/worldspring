<script lang="ts">
  // A token's menu at the token (the DM right-presses a token): its name, role, size, hidden,
  // whether the players see by it, its picture, duplicate and remove; for several selected
  // tokens, their role, hide or show, duplicate and remove. Kept inside the window; Escape, a
  // press elsewhere or the map moving closes it.
  import { roleOf, SIZES, type Token, type TokenRole } from './state';
  import type { PlayController } from './controller';
  import Icon from '../ui/Icon.svelte';

  interface Props {
    play: PlayController;
    /** Bumped whenever the play state changes. */
    version: number;
    ids: string[];
    /** The token's box on screen [left, top, right, bottom]: the menu goes beside it. */
    box: [number, number, number, number];
    onPicture: (ids: string[]) => void;
    onClose: () => void;
  }
  let { play, version, ids, box, onPicture, onClose }: Props = $props();

  const ROLES: [TokenRole | 'light', string][] = [
    ['pc', 'PC'],
    ['npc', 'NPC'],
    ['creature', 'Creature'],
    ['object', 'Object'],
    ['light', 'Light'],
  ];
  const sizeLabel = (n: number) => (n === 0.5 ? '½ square' : `${n}×${n}`);

  const tokens = $derived.by<Token[]>(() => {
    void version;
    return ids.map((id) => play.state.tokens[id]).filter((t): t is Token => !!t);
  });
  const one = $derived(tokens.length === 1 ? tokens[0] : null);
  /** The role all of them have (null: they differ). */
  const role = $derived.by<TokenRole | 'light' | null>(() => {
    const rs = new Set(tokens.map((t) => (t.kind === 'light' ? 'light' : roleOf(t))));
    return rs.size === 1 ? [...rs][0] : null;
  });
  const anyShown = $derived(tokens.some((t) => !t.hidden));
  const following = $derived.by(() => {
    void version;
    return play.followTarget;
  });

  $effect(() => {
    if (!tokens.length) onClose();
  });

  let el = $state<HTMLDivElement>();
  let w = $state(0);
  let h = $state(0);
  // Beside the token: to its right, or its left when there is no room; kept inside the window.
  const left = $derived.by(() => {
    const right = box[2] + 8;
    return Math.max(8, right + w + 8 <= innerWidth ? right : box[0] - 8 - w);
  });
  const top = $derived(Math.max(8, Math.min(box[1], innerHeight - h - 8)));

  const checked = (e: Event) => (e.target as HTMLInputElement).checked;

  function setRole(r: TokenRole | 'light') {
    for (const t of tokens) play.setTokenRole(t.id, r);
  }
  function hideAll(hidden: boolean) {
    for (const t of tokens) play.updateToken(t.id, { hidden });
  }
  function duplicate() {
    play.duplicateSelected();
    onClose();
  }
  function remove() {
    play.removeSelected();
    onClose();
  }
  function outside(e: PointerEvent) {
    if (el && !el.contains(e.target as Node)) onClose();
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape' || e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onpointerdowncapture={outside} />

<div class="menu ws-panel" style:left="{left}px" style:top="{top}px" role="menu" tabindex="-1" bind:this={el} bind:clientWidth={w} bind:clientHeight={h}>
  {#if one}
    {@const t = one}
    <div class="ws-row">
      <input class="ws-input grow name" value={t.name} oninput={(e) => play.updateToken(t.id, { name: (e.target as HTMLInputElement).value })} onkeydown={key} aria-label="Name" />
      {#if t.kind === 'character'}
        <select class="ws-input" value={t.size} onchange={(e) => play.updateToken(t.id, { size: Number((e.target as HTMLSelectElement).value) || 1 })} aria-label="Size">
          {#each SIZES as n (n)}<option value={n}>{sizeLabel(n)}</option>{/each}
        </select>
      {/if}
    </div>
  {:else}
    <b class="count">{tokens.length} tokens</b>
  {/if}
  <div class="ws-seg tight" role="radiogroup" aria-label="Role">
    {#each ROLES as [k, label] (k)}<button class:on={role === k} role="radio" aria-checked={role === k} onclick={() => setRole(k)}>{label}</button>{/each}
  </div>
  {#if one}
    {@const t = one}
    <label class="switch"><span class="grow">Hidden <span class="ws-muted">· only you see it</span></span><input type="checkbox" class="ws-switch" checked={!!t.hidden} onchange={(e) => play.updateToken(t.id, { hidden: checked(e) })} /></label>
    {#if t.kind === 'character'}
      <label class="switch"><span class="grow">Players see by it</span><input type="checkbox" class="ws-switch" checked={!!t.vision} onchange={(e) => play.updateToken(t.id, { vision: checked(e) })} /></label>
      <label class="switch" title="The players' window goes where it is and keeps it in the middle"><span class="grow">Players’ view follows</span><input type="checkbox" class="ws-switch" checked={following === t.id} onchange={(e) => play.setFollowTarget(checked(e) ? t.id : null)} /></label>
      <button class="ws-btn item" role="menuitem" onclick={() => onPicture([t.id])}><Icon name="image" size={16} /> Picture…</button>
    {/if}
  {:else}
    <button class="ws-btn item" role="menuitem" onclick={() => hideAll(anyShown)}><Icon name="eye" size={16} /> {anyShown ? 'Hide' : 'Show'}</button>
  {/if}
  <div class="ws-row">
    <button class="ws-btn grow" role="menuitem" onclick={duplicate} title="Duplicate (Ctrl+D)">Duplicate</button>
    <button class="ws-btn danger grow" role="menuitem" onclick={remove} title="Remove (Delete)"><Icon name="trash" size={16} /> Remove{one ? '' : ` (${tokens.length})`}</button>
  </div>
</div>

<style>
  .tight > button {
    padding: 0 2px;
    font-size: 12px;
  }
  .menu {
    position: fixed;
    z-index: var(--z-menu);
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 8px;
    width: 320px;
    max-width: calc(100vw - 16px);
    outline: none;
  }
  .name,
  .count {
    font-weight: bold;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .switch {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }
  .item {
    justify-content: flex-start;
  }
</style>
