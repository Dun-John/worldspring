<script lang="ts">
  // World › Library: this world (save it, download it), the worlds saved in this browser,
  // opening a world file, backing the whole library up (and restoring it), and, folded away,
  // starting this world over.
  import { onMount } from 'svelte';
  import type { WorldFile } from '../../gen/protocol';
  import { unbundleAssets } from '../../world/assets';
  import { changeList } from '../../world/changes';
  import { deleteWorld, isBackup, listWorlds, saveWorld, type Backup, type SavedWorld } from '../../world/library';
  import { download, sameWorld, validate } from '../../world/world';
  import Icon from '../Icon.svelte';

  /** Largest world file read (the heaviest test world is about 6 MB, plus its pictures). */
  const MAX_IMPORT_BYTES = 200 * 1024 * 1024;
  /** Largest backup read (every world and picture this browser keeps): read whole, it must stay
   * under the longest string a browser holds (~512 MB). */
  const MAX_BACKUP_BYTES = 480 * 1024 * 1024;

  interface Props {
    world: WorldFile;
    busy: boolean;
    /** Changes to this world since it was last downloaded. */
    unexported: number;
    /** Live sync keeps this world's changes on disk too. */
    keeps: boolean;
    /** Open a world (saved, or from a file). */
    onOpen: (w: WorldFile, from: 'file' | 'library') => void;
    /** Download this world's file. */
    onDownload: () => void;
    /** Copy a link that opens this world anywhere. */
    onCopyLink: () => void;
    /** Download a backup of everything this browser keeps. */
    onBackup: () => Promise<void>;
    /** Restore a backup (asked about first); false if cancelled. */
    onRestore: (b: Backup) => Promise<boolean>;
    /** Clear every change made to this world: back to the world as generated. */
    onStartOver: () => void;
  }
  let { world, busy, unexported, keeps, onOpen, onDownload, onCopyLink, onBackup, onRestore, onStartOver }: Props = $props();

  let saved = $state<SavedWorld[]>([]);
  let naming = $state(false);
  let saveName = $state('');
  let menu = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);
  let importError = $state('');
  let fileInput: HTMLInputElement | undefined = $state();
  let backupInput: HTMLInputElement | undefined = $state();
  let backingUp = $state(false);
  let backupError = $state('');

  onMount(async () => (saved = await listWorlds()));

  // Starting over clears everything made in this world: asked for in words, not a click.
  const CONFIRM_WORD = 'delete';
  let startingOver = $state(false);
  let confirmText = $state('');
  const confirmed = $derived(confirmText.trim().toLowerCase() === CONFIRM_WORD);
  /** What starting over would clear, counted. */
  const changes = $derived(changeList(world.edits));

  async function save() {
    await saveWorld(saveName.trim() || `World ${world.seed}`, $state.snapshot(world));
    saveName = '';
    naming = false;
    saved = await listWorlds();
  }

  async function remove(id: string) {
    await deleteWorld(id);
    confirmDelete = null;
    menu = null;
    saved = await listWorlds();
  }

  async function importFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      importError = '';
      if (file.size > MAX_IMPORT_BYTES) throw new Error(`That file is too big for a world (${MAX_IMPORT_BYTES / 1048576} MB at most)`);
      const json = JSON.parse(await file.text());
      // (A backup of the library opened here is restored.)
      if (isBackup(json)) return void (await restoreFrom(json));
      const w = validate(json);
      // Pictures it carries (NPC portraits) are kept in this browser first.
      await unbundleAssets(json.assets);
      onOpen(w, 'file');
    } catch (err) {
      importError = String((err as Error).message ?? err);
    }
  }

  async function makeBackup() {
    backingUp = true;
    backupError = '';
    try {
      await onBackup();
    } catch (err) {
      backupError = `The backup could not be made: ${String((err as Error).message ?? err)}`;
    } finally {
      backingUp = false;
    }
  }

  async function readBackup(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      backupError = '';
      if (file.size > MAX_BACKUP_BYTES) throw new Error(`That file is too big (${MAX_BACKUP_BYTES / 1048576} MB at most)`);
      const json = JSON.parse(await file.text());
      if (!isBackup(json)) throw new Error('That is not a backup of the library (a world file opens with “Open a file”)');
      await restoreFrom(json);
    } catch (err) {
      backupError = String((err as Error).message ?? err);
    }
  }

  async function restoreFrom(b: Backup) {
    if (await onRestore(b)) saved = await listWorlds();
  }

  function startOver() {
    if (!confirmed) return;
    onStartOver();
    startingOver = false;
    confirmText = '';
  }

  const date = (t: number) => new Date(t).toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
  const size = $derived(`${(world.params.width_mi ?? 1200).toLocaleString()} × ${(world.params.height_mi ?? 900).toLocaleString()} mi`);
</script>

<div class="lib">
  <section class="card">
    <div class="ws-label">This world</div>
    <div class="title">Seed {world.seed} <span class="ws-muted">· {size}</span></div>
    <div class="ws-muted">{changes.length ? `Your changes: ${changes.join(', ')}.` : 'No changes yet.'}</div>
    {#if unexported > 0 && !keeps}
      <div class="unexported">Kept only in this browser: {unexported} {unexported === 1 ? 'change' : 'changes'} since you last downloaded this world.</div>
    {/if}
    {#if naming}
      <div class="ws-row">
        <!-- svelte-ignore a11y_autofocus -->
        <input class="ws-input grow" placeholder="Name this world" bind:value={saveName} autofocus onkeydown={(e) => (e.key === 'Enter' ? save() : e.key === 'Escape' && (naming = false))} />
        <button class="ws-btn primary" onclick={save}>Save</button>
      </div>
    {:else}
      <div class="ws-row">
        <button class="ws-btn grow" onclick={() => (naming = true)}><Icon name="save" size={16} /> Save to library</button>
        <button class="ws-btn grow" class:primary={unexported > 0 && !keeps} onclick={onDownload} title="A .world.json file with your changes and pictures"><Icon name="download" size={16} /> Download file</button>
      </div>
      <button class="ws-btn quiet block" onclick={onCopyLink} title="A link that opens this world in any browser, with your changes while they are few"><Icon name="link" size={16} /> Copy link</button>
    {/if}
  </section>

  <section>
    <div class="ws-label">Saved in this browser</div>
    <ul class="list">
      {#each saved as w (w.id)}
        {@const open = sameWorld(w.file, world)}
        <li class:open>
          <button class="item" onclick={() => onOpen(w.file, 'library')} disabled={busy} title="Open {w.name}">
            <span class="name">{w.name}</span>
            <span class="ws-muted">{open ? 'open now' : date(w.savedAt)}</span>
          </button>
          <button class="ws-icon-btn" onclick={() => ((menu = menu === w.id ? null : w.id), (confirmDelete = null))} aria-label="More for {w.name}" aria-expanded={menu === w.id}><Icon name="more" /></button>
          {#if menu === w.id}
            <div class="menu ws-panel" role="menu">
              <button class="ws-btn quiet" role="menuitem" onclick={() => ((menu = null), download(w.file, w.name))}><Icon name="download" size={16} /> Download</button>
              {#if confirmDelete === w.id}
                <button class="ws-btn danger" role="menuitem" onclick={() => remove(w.id)}><Icon name="trash" size={16} /> Delete for good?</button>
              {:else}
                <button class="ws-btn quiet" role="menuitem" onclick={() => (confirmDelete = w.id)}><Icon name="trash" size={16} /> Delete</button>
              {/if}
            </div>
          {/if}
        </li>
      {:else}
        <li class="ws-muted empty">No saved worlds yet. Save this one to come back to it.</li>
      {/each}
    </ul>
    <button class="ws-btn block" onclick={() => fileInput?.click()} disabled={busy}><Icon name="folder" size={16} /> Open a file…</button>
    <input bind:this={fileInput} type="file" accept=".json,application/json" hidden onchange={importFile} />
    {#if importError}<div class="error">{importError}</div>{/if}
  </section>

  <section>
    <div class="ws-label">Everything in this browser</div>
    <div class="ws-muted">Worlds, changes and pictures live only in this browser: clearing its site data loses them. A backup keeps them all in one file.</div>
    <div class="ws-row">
      <button class="ws-btn grow" onclick={makeBackup} disabled={busy || backingUp} title="Every world, its changes and pictures, in one file"><Icon name="download" size={16} /> {backingUp ? 'Backing up…' : 'Back up all'}</button>
      <button class="ws-btn grow" onclick={() => backupInput?.click()} disabled={busy} title="Bring back the worlds, changes and pictures of a backup"><Icon name="upload" size={16} /> Restore…</button>
    </div>
    <input bind:this={backupInput} type="file" accept=".json,application/json" hidden onchange={readBackup} />
    {#if backupError}<div class="error">{backupError}</div>{/if}
  </section>

  <details class="ws-group danger-zone">
    <summary>Danger zone</summary>
    <div class="ws-group-body">
      {#if !startingOver}
        <button class="ws-btn danger" onclick={() => (startingOver = true)} disabled={busy || !changes.length} title={changes.length ? 'Clear every change made to this world' : 'Nothing has been changed in this world'}>Start over…</button>
        <div class="ws-muted">Clears every change made to this world; the seed, settings and sketch stay.</div>
      {:else}
        <div class="confirm">
          <div>
            <strong>Start over with seed {world.seed}?</strong> Every change made to this world goes: {changes.join(', ')}.
            The world itself (seed, settings, sketch) stays. Download it first to keep a copy.
          </div>
          <label class="ws-field">
            <span>Type <b>{CONFIRM_WORD}</b> to confirm</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="ws-input" bind:value={confirmText} autocomplete="off" spellcheck="false" autofocus onkeydown={(e) => (e.key === 'Enter' ? startOver() : e.key === 'Escape' && (startingOver = false))} />
          </label>
          <div class="ws-row end">
            <button class="ws-btn" onclick={() => ((startingOver = false), (confirmText = ''))}>Cancel</button>
            <button class="ws-btn danger" onclick={startOver} disabled={!confirmed}>Clear everything</button>
          </div>
        </div>
      {/if}
    </div>
  </details>
</div>

<style>
  .lib {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .card {
    padding: 8px 10px;
    border: 1px solid var(--line-soft);
    border-radius: var(--radius);
    background: rgba(255, 255, 255, 0.35);
  }
  .title {
    font-weight: bold;
    font-size: 15px;
  }
  .grow {
    flex: 1;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 260px;
    overflow-y: auto;
  }
  .list li {
    position: relative;
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-sm);
  }
  .list li.open {
    background: rgba(205, 187, 140, 0.35);
  }
  .item {
    all: unset;
    box-sizing: border-box;
    flex: 1;
    min-width: 0;
    min-height: var(--tap);
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 2px 6px;
    cursor: pointer;
    border-radius: var(--radius-sm);
  }
  .item:hover {
    background: var(--btn-hover);
  }
  .item .name {
    font-weight: bold;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 100%;
    z-index: 2;
    display: flex;
    flex-direction: column;
    padding: 4px;
    gap: 2px;
    min-width: 180px;
  }
  .menu .ws-btn {
    justify-content: flex-start;
  }
  .empty {
    padding: 6px 2px;
    font-style: italic;
  }
  .error {
    color: #8a2a1a;
    font-size: 12px;
  }
  .unexported {
    color: #8a5a1a;
    font-size: 12px;
  }
  .danger-zone > summary {
    color: var(--danger);
  }
  .confirm {
    border: 1px solid #8a2a1a;
    border-radius: var(--radius-sm);
    padding: 6px 8px;
    background: rgba(138, 42, 26, 0.06);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .end {
    justify-content: flex-end;
  }
</style>
