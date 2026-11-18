// Edit › Build › Fence: the menu's state (kind, Draw | Gate | Erase, the eraser's width) and the
// changes it makes. The generator works each one out (`gen.fenceChange`: drawn fences, gaps, a
// generated run's kind or gates or removal); here it becomes one `applyEdits` (one Undo step).
// Select's card changes fences through the same calls (`change`, `addGate`).
import type { Edits, FenceCut, FenceKind } from '../../gen/protocol';
import { sentence } from '../../ui/gazetteer';
import { FenceTool, defaultFence, fenceWord, type Pt } from '../fence';
import { withFence } from '../select';
import type { BuildHost } from './build.svelte';

export function fenceTools(app: BuildHost) {
  let fence = $state(defaultFence());
  let busy = false;
  let hintId = 0;

  /** Ask the generator for `input` (`fence_change_json`), make its change as one step; `change` its words for the history. */
  async function run(input: Record<string, unknown>, change: import('../../sync/mapd').Change): Promise<FenceCut | null> {
    if (busy) return null;
    busy = true;
    const fail = (text: string) => (app.toast(text), null);
    try {
      const before = app.edits;
      const out = await app.view.gen.fenceChange(input);
      if (!out) return fail('The map could not answer: try again');
      if ('error' in out) return fail(sentence(out.error));
      if (!out.done.length) return fail(out.refused.length ? `Not changed: ${[...new Set(out.refused.map((r) => r.why))].join('; ')}` : 'Nothing there to change');
      if (app.edits !== before) return fail('The map changed meanwhile: try again');
      app.applyEdits(withFence($state.snapshot(before) as Edits, out), change, 'user');
      if (out.refused.length) app.toast(`Not changed (${out.refused.length}): ${[...new Set(out.refused.map((r) => r.why))].join('; ')}`);
      return out;
    } finally {
      busy = false;
    }
  }

  const tool = new FenceTool({
    settings: () => fence,
    placed: (pts, closed) => void run({ op: 'draw', fence: { kind: fence.kind, pts, ...(closed ? { closed } : {}) } }, { tool: 'place_fence', kind: fence.kind }),
    gate: (at) => void gateAt(at),
    gap: (pts) => void run({ op: 'gap', gap: { pts, r: fence.eraser / 2 } }, { tool: 'remove_fences', gaps: 1 }),
    hint: (t, stay) => {
      if (hintId) app.dismissToast(hintId);
      hintId = t ? app.toast(t, undefined, stay ? { sticky: true } : {}) : 0;
    },
  });

  /** A gate in the fence under `at` (the nearest of what `pick` finds there). */
  async function gateAt(at: Pt) {
    const things = await app.view.gen.pick(at[0], at[1], 8);
    const t = things.find((x) => (x.kind === 'fence' || x.kind === 'drawn_fence') && x.id && x.removable);
    if (!t?.id) return app.toast(things.some((x) => x.kind === 'fence') ? 'That fence cannot be changed here' : 'Click on a fence to put a gate in it');
    void run({ ids: [t.id], op: 'gate', at }, { tool: 'place_fence', gate: true, ids: [t.id] });
  }

  /** The fences `ids` made `kind` (a drawn one is replaced, a generated one changed). */
  const change = (ids: string[], kind: FenceKind) => run({ ids, op: 'kind', kind }, { tool: 'place_fence', kind, changed: true, ids });

  return {
    tool,
    change,
    word: fenceWord,
    get fence() { return fence; },
    set fence(v) { fence = v; },
  };
}

export type FenceTools = ReturnType<typeof fenceTools>;
