<script lang="ts">
  // What a building drawn or stamped is: its kind, storeys, roof, ruin and name.
  import { MAX_FLOORS, ROOFS, ROOF_MATERIALS, ROOF_MATERIAL_RGB, ROOF_SHADES, ROOF_SHAPES, type BuildingFuncs } from '../../gen/protocol';
  import type { BuildSettings } from '../../editor/build';
  import Icon from '../Icon.svelte';
  import { categoriesOf, label } from './util';

  interface Props {
    settings: BuildSettings;
    funcs: BuildingFuncs | null;
    /** The building being changed (its name), else drawing new ones. */
    editing: string | null;
    onSave: () => void;
  }

  let { settings = $bindable(), funcs, editing, onSave }: Props = $props();

  const ROOF_LABEL: Record<string, string> = { hip: 'Pitched', battlements: 'Battlements', cone: 'Cone' };
  const hex = (c: number) => `#${c.toString(16).padStart(6, '0')}`;
  const categories = $derived(categoriesOf(funcs));
</script>

<label class="ws-field">
  What it is
  <select class="ws-input" bind:value={settings.func}>
    {#if funcs}
      <optgroup label="Homes">
        {#each funcs.homes as h (h.key)}<option value={h.key}>{label(h.name)}</option>{/each}
      </optgroup>
      {#each categories as c (c)}
        <optgroup label={label(c)}>
          {#each funcs.businesses.filter((b) => b.category === c) as b (b.key)}<option value={b.key}>{b.name}</option>{/each}
        </optgroup>
      {/each}
    {:else}
      <option value={settings.func}>{label(settings.func)}</option>
    {/if}
  </select>
</label>
<div class="ws-field">
  Storeys
  <div class="stepper">
    <button class="ws-icon-btn" onclick={() => (settings.floors = Math.max(1, settings.floors - 1))} disabled={settings.floors <= 1} aria-label="One storey less"><Icon name="minus" size={16} /></button>
    <span>{settings.floors}</span>
    <button class="ws-icon-btn" onclick={() => (settings.floors = Math.min(MAX_FLOORS, settings.floors + 1))} disabled={settings.floors >= MAX_FLOORS} aria-label="One storey more"><Icon name="plus" size={16} /></button>
  </div>
</div>
<div class="ws-field">
  Roof
  <div class="ws-seg">
    <button class:on={settings.roof === ''} onclick={() => (settings.roof = '')} title="As its kind has it">Auto</button>
    {#each ROOFS as r (r)}<button class:on={settings.roof === r} onclick={() => (settings.roof = r)}>{ROOF_LABEL[r]}</button>{/each}
  </div>
</div>
<div class="ws-field">
  Roof shape
  <div class="tints" role="radiogroup" aria-label="Roof shape">
    <button class="ws-chip" class:on={settings.rshape === ''} role="radio" aria-checked={settings.rshape === ''} onclick={() => (settings.rshape = '')} title="Picked for it">Picked</button>
    {#each ROOF_SHAPES as r (r)}<button class="ws-chip" class:on={settings.rshape === r} role="radio" aria-checked={settings.rshape === r} onclick={() => (settings.rshape = r)}>{label(r)}</button>{/each}
  </div>
</div>
<div class="ws-field">
  Roof material
  <div class="tints" role="radiogroup" aria-label="Roof material">
    <button class="ws-chip" class:on={settings.material === ''} role="radio" aria-checked={settings.material === ''} onclick={() => ((settings.material = ''), (settings.shade = ''))} title="Picked for it">Picked</button>
    {#each ROOF_MATERIALS as m, i (m)}
      <button class="swatch" class:on={settings.material === m} role="radio" aria-checked={settings.material === m} style:background={hex(ROOF_MATERIAL_RGB[i])} onclick={() => ((settings.material = m), (settings.shade = ''))} title={label(m)} aria-label={label(m)}></button>
    {/each}
  </div>
</div>
<div class="ws-field">
  Roof colour
  <div class="tints" role="radiogroup" aria-label="Roof colour">
    <button class="ws-chip" class:on={settings.shade === ''} role="radio" aria-checked={settings.shade === ''} onclick={() => (settings.shade = '')} title="Picked for it">Picked</button>
    {#if settings.material}
      {#each ROOF_SHADES[ROOF_MATERIALS.indexOf(settings.material)] as c, k (k)}
        <button class="swatch" class:on={settings.shade === k} role="radio" aria-checked={settings.shade === k} style:background={hex(c)} onclick={() => (settings.shade = k)} title={k === 2 ? 'Usual' : `Variant ${k + 1}`} aria-label={k === 2 ? 'Usual colour' : `Variant ${k + 1}`}></button>
      {/each}
    {:else}
      <span class="ws-hint">Pick a material for its colours</span>
    {/if}
  </div>
</div>
<label class="switch">
  <span class="grow">In ruins<span class="ws-muted"> · no roof, no way in</span></span>
  <input type="checkbox" class="ws-switch" bind:checked={settings.ruin} />
</label>
<label class="ws-field">
  Name
  <input class="ws-input" bind:value={settings.name} placeholder={editing ? '' : 'Named for its trade if left empty'} />
</label>
{#if editing}<button class="ws-btn primary block" onclick={onSave}>Save changes</button>{/if}
