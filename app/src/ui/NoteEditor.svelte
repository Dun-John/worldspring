<script lang="ts">
  // A note's text and tags (on a place, a room or a point in one): the quick picks as chips that
  // toggle, the other tags as a comma field. Text tagged read-aloud is boxed, to be read out.
  // Saves on change (the text when it loses focus).
  import Icon from './Icon.svelte';
  import { QUICK_TAGS, canonTags } from './notes';

  interface Props {
    note?: { text: string; tags?: string[] };
    onSave: (text: string, tags: string[]) => void;
    placeholder?: string;
    /** A label over the text (else none). */
    label?: string;
    /** The tags field even before there is a note. */
    tagsAlways?: boolean;
  }
  let { note, onSave, placeholder = 'Add a note: lore, hooks, secrets…', label, tagsAlways = false }: Props = $props();

  const quick = new Set(QUICK_TAGS.map((q) => q.tag));
  const tags = $derived(canonTags(note?.tags ?? []));
  const free = $derived(tags.filter((t) => !quick.has(t)));
  const aloud = $derived(tags.includes('read-aloud'));
  let area = $state<HTMLTextAreaElement>();

  const text = () => area?.value ?? note?.text ?? '';
  const tagList = (s: string) => [...new Set(s.split(',').map((t) => t.trim()).filter(Boolean))];
  function toggle(t: string) {
    onSave(text(), tags.includes(t) ? tags.filter((x) => x !== t) : canonTags([...tags, t]));
  }
</script>

<div class="note-ed">
  {#if label}<span class="ws-label">{label}</span>{/if}
  {#if aloud}<span class="aloud-head"><Icon name="chat" size={12} /> Read aloud</span>{/if}
  <textarea bind:this={area} class="ws-input" class:aloud {placeholder} value={note?.text ?? ''} onchange={(e) => onSave(e.currentTarget.value, tags)} aria-label="Note"></textarea>
  <div class="chips" role="group" aria-label="Quick tags">
    {#each QUICK_TAGS as q (q.tag)}
      <button class="ws-chip" aria-pressed={tags.includes(q.tag)} onclick={() => toggle(q.tag)} title={tags.includes(q.tag) ? `Untag ${q.label.toLowerCase()}` : `Tag it ${q.label.toLowerCase()}`}><Icon name={q.icon} size={12} /> {q.label}</button>
    {/each}
  </div>
  {#if tagsAlways || note?.text || tags.length}
    <input class="ws-input" placeholder="Other tags, comma separated" value={free.join(', ')} onchange={(e) => onSave(text(), canonTags([...tags.filter((t) => quick.has(t)), ...tagList(e.currentTarget.value)]))} aria-label="Tags" />
  {/if}
</div>

<style>
  .note-ed {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  textarea {
    field-sizing: content;
    min-height: 3.2em;
    max-height: 40vh;
  }
  /* (Read-aloud text: boxed, as boxed text in an adventure.) */
  textarea.aloud {
    font-style: italic;
    background: var(--paper-solid);
    border: 1px solid var(--line);
    outline: 3px double var(--line-soft);
    outline-offset: -5px;
    padding: 9px 10px;
  }
  .aloud-head {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--ink-2);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
</style>
