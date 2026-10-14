<script lang="ts" module>
  export type VersionChoice = 'made' | 'upgrade' | 'download' | 'cancel';
</script>

<script lang="ts">
  // Asked when a world made with an older generator opens: open it as it was made (in that
  // generator's build, which the site keeps), or upgrade it to this one, where features may have
  // moved from under its edits.
  import Icon from '../Icon.svelte';

  interface Props {
    /** The generator the world was made with, and this one. */
    from: number;
    to: number;
    /** The site keeps a build of `from`. */
    kept: boolean;
    /** The world has edits (else upgrading moves nothing of the user's). */
    edited: boolean;
    /** That generator keeps a play session for it (upgraded, play starts afresh). */
    played: boolean;
    /** A generator since `from` that makes every town and site anew (what was made on them is set
     * aside), else null. */
    anew: number | null;
    /** Another world is showing, to go back to. */
    canCancel: boolean;
    onChoose: (c: VersionChoice) => void;
  }
  let { from, to, kept, edited, played, anew, canCancel, onChoose }: Props = $props();
</script>

<div class="scrim" role="presentation"></div>
<div class="box ws-panel" role="alertdialog" aria-modal="true" aria-labelledby="version-title">
  <header>
    <Icon name="globe" />
    <b id="version-title">Made with an older Worldspring</b>
  </header>
  {#if anew}
    <p>
      This world was made with version {from} of the world generator; this is version {to}. Since version {anew}, every
      town and site comes out anew: new names, streets, buildings and dungeons, some a few miles from where they
      were{#if edited}. What you made on them (their names and notes, designed rooms and dungeons, changed streets and
      buildings, where NPCs and plot points are) is set aside; names and notes on coasts, rivers, ranges and forests are
      kept, and objects, ground and crossings stay where you put them{/if}.
    </p>
  {:else}
    <p>
      This world was made with version {from} of the world generator; this is version {to}. The newer generator may put
      coasts, rivers, roads, towns and sites in other places{#if edited}, so your changes (names, notes, objects, people,
      designed sites) may no longer line up with them{/if}.
    </p>
  {/if}
  {#if played}
    <p>Your play session stays with version {from}: upgraded, play starts afresh.</p>
  {/if}
  {#if kept}
    <p class="ws-muted">Opened as it was made, it looks exactly as before, and you can upgrade it from there later.</p>
  {:else}
    <p class="ws-muted">This site no longer keeps version {from}. Download the world file first to keep it as it is.</p>
  {/if}
  <div class="ws-row buttons">
    {#if canCancel}<button class="ws-btn quiet" onclick={() => onChoose('cancel')}>Cancel</button>{/if}
    {#if kept}
      <button class="ws-btn" onclick={() => onChoose('upgrade')}>Upgrade to version {to}</button>
      <button class="ws-btn primary" onclick={() => onChoose('made')}>Open as it was made</button>
    {:else}
      <button class="ws-btn" onclick={() => onChoose('download')}><Icon name="download" size={16} /> Download the world file</button>
      <button class="ws-btn primary" onclick={() => onChoose('upgrade')}>Upgrade to version {to}</button>
    {/if}
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
    width: min(480px, calc(100vw - 32px));
    max-height: calc(100dvh - 48px);
    overflow-y: auto;
    padding: 14px 16px 16px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
  }
  p {
    margin: 10px 0 0;
  }
  .buttons {
    margin-top: 14px;
    justify-content: flex-end;
    flex-wrap: wrap;
  }
</style>
