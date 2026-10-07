<script lang="ts" module>
  import type { IconName } from '../icons';

  export interface ChoiceButton<T extends string = string> {
    label: string;
    value: T;
    kind?: 'primary' | 'quiet' | 'danger';
    icon?: IconName;
  }
</script>

<script lang="ts" generics="T extends string">
  // A question that must be answered before going on (what to do with the changes this browser
  // has when a world brings its own, or a backup is restored): the text, then one button per
  // answer.
  import type { Snippet } from 'svelte';
  import Icon from '../Icon.svelte';

  interface Props {
    title: string;
    icon?: IconName;
    buttons: ChoiceButton<T>[];
    onChoose: (c: T) => void;
    children: Snippet;
  }
  let { title, icon = 'globe', buttons, onChoose, children }: Props = $props();
</script>

<div class="scrim" role="presentation"></div>
<div class="box ws-panel" role="alertdialog" aria-modal="true" aria-labelledby="choice-title">
  <header>
    <Icon name={icon} />
    <b id="choice-title">{title}</b>
  </header>
  {@render children()}
  <div class="ws-row buttons">
    {#each buttons as b (b.value)}
      <button class="ws-btn {b.kind ?? ''}" onclick={() => onChoose(b.value)}>{#if b.icon}<Icon name={b.icon} size={16} />{/if} {b.label}</button>
    {/each}
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
    width: min(500px, calc(100vw - 32px));
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
  .box :global(p) {
    margin: 10px 0 0;
  }
  .buttons {
    margin-top: 14px;
    justify-content: flex-end;
    flex-wrap: wrap;
  }
</style>
