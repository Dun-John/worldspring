<script lang="ts" module>
  /** A picture to choose: `key` names it to whoever opened the picker. */
  export interface PictureItem {
    key: string;
    label: string;
    group: string;
    load: () => Promise<Blob | null>;
  }
</script>

<script lang="ts">
  // A token's picture: the world's NPC portraits and uploaded sprites, the pictures already on
  // tokens in this session, a file, or none. Thumbnails are made when it opens and let go when
  // it closes.
  import { onDestroy, onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';

  interface Props {
    items: PictureItem[];
    /** The key of the picture the token has now (marked). */
    current?: string | null;
    onPick: (key: string) => void;
    onFile: (file: File) => void;
    onNone: () => void;
    onClose: () => void;
  }
  let { items, current = null, onPick, onFile, onNone, onClose }: Props = $props();

  let urls = $state<Record<string, string | null>>({});
  const made: string[] = [];
  let gone = false;
  // (Made once, for the items there when it opens.)
  onMount(() => {
    for (const it of items) {
      void it.load().then(
        (b) => {
          if (gone || !b) return void (urls[it.key] = null);
          const u = URL.createObjectURL(b);
          made.push(u);
          urls[it.key] = u;
        },
        () => (urls[it.key] = null),
      );
    }
  });
  onDestroy(() => {
    gone = true;
    for (const u of made) URL.revokeObjectURL(u);
  });

  const groups = $derived.by(() => {
    const out: [string, PictureItem[]][] = [];
    for (const it of items) {
      const g = out.find(([name]) => name === it.group);
      if (g) g[1].push(it);
      else out.push([it.group, [it]]);
    }
    return out;
  });
</script>

<div class="scrim" role="presentation" onclick={onClose}></div>
<div class="box ws-panel" role="dialog" aria-modal="true" aria-labelledby="picture-title">
  <header>
    <Icon name="image" />
    <b id="picture-title" class="grow">Picture</b>
    <button class="ws-icon-btn" onclick={onClose} aria-label="Close" title="Close"><Icon name="x" size={16} /></button>
  </header>
  {#each groups as [group, list] (group)}
    <div class="ws-label">{group}</div>
    <div class="grid">
      {#each list as it (it.key)}
        <button class="pic" class:on={current === it.key} title={it.label} onclick={() => onPick(it.key)}>
          {#if urls[it.key]}<img src={urls[it.key]} alt="" />{:else}<span class="blank"></span>{/if}
          <span class="label">{it.label}</span>
        </button>
      {/each}
    </div>
  {:else}
    <p class="ws-hint">No portraits or uploaded pictures in this world yet.</p>
  {/each}
  <div class="ws-row buttons">
    <label class="ws-btn file"><Icon name="image" size={16} /> From a file… <input type="file" accept="image/*" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) onFile(f); }} /></label>
    <button class="ws-btn quiet" onclick={onNone}>No picture</button>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: var(--z-overlay);
    background: rgba(30, 24, 18, 0.35);
  }
  .box {
    position: fixed;
    z-index: var(--z-overlay);
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: min(460px, calc(100vw - 32px));
    max-height: calc(100dvh - 48px);
    overflow-y: auto;
    padding: 12px 14px 14px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    margin-bottom: 6px;
  }
  .grow {
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(76px, 1fr));
    gap: 6px;
    margin: 4px 0 10px;
  }
  .pic {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 4px;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius-sm);
    background: rgba(255, 255, 255, 0.35);
    color: var(--ink);
    font: 11px/1.2 var(--font);
    cursor: pointer;
    min-width: 0;
  }
  .pic:hover {
    background: var(--btn-on);
  }
  .pic.on {
    border-color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--ink);
  }
  .pic img,
  .blank {
    width: 64px;
    height: 64px;
    object-fit: cover;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.08);
  }
  .label {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .buttons {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
  .file {
    position: relative;
    overflow: hidden;
  }
  .file input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
  }
</style>
