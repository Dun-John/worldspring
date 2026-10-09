// Undo history for the world file's edits (the user's and agents'). A step keeps the ops it
// made and the ops that undo them; undoing puts back only the entries that step changed, so
// later changes to other entries (by anyone) stay.
import type { Edits } from '../gen/protocol';
import type { Change } from './mapd';
import { EDIT_FIELDS, type EditOp } from './ops';

export interface Step {
  label: string;
  author: string;
  ops: EditOp[];
  inverse: EditOp[];
}

/** Whether an object has a key (without listing a field of thousands of them). */
function filled(o: object): boolean {
  for (const _ in o) return true;
  return false;
}

/** Edits without empty fields (shorter share links and files). */
export function tidy(e: Edits): Edits {
  const out: Record<string, unknown> = {};
  for (const field of Object.keys(EDIT_FIELDS)) {
    const v = (e as Record<string, unknown>)[field];
    if (Array.isArray(v) ? v.length : v && typeof v === 'object' && filled(v)) out[field] = v;
  }
  return out as Edits;
}

/** A short line saying what a change did. */
export function describe(change: Change, author: string, nameOf: (id: string) => string): string {
  // Changes made elsewhere (over the live link) are marked as synced; the app never names who.
  const who = author === 'user' || change.tool === 'sync' ? '' : 'Synced: ';
  const id = String(change.id ?? '');
  const what = (() => {
    switch (change.tool) {
      case 'rename_feature':
      case 'rename':
        return change.name === '' ? 'name reset' : `renamed to “${String(change.name ?? nameOf(id))}”`;
      case 'create_feature':
        return `created ${String(change.kind ?? 'site')} “${String(change.name ?? nameOf(id))}”`;
      case 'create_building':
        return `built “${String(change.name ?? nameOf(id))}”`;
      case 'update_building':
        return `changed ${String(change.name ?? nameOf(id))}`;
      case 'annotate_feature':
        return `notes on ${nameOf(id)}`;
      case 'update_feature':
        return `changed ${nameOf(id)}`;
      case 'hide_feature':
        return `${change.hidden === false ? 'showed' : 'hid'} ${nameOf(id)}`;
      case 'delete_feature':
        return `deleted ${nameOf(id)}`;
      case 'create_npc':
        return `new NPC “${String(change.name ?? id)}”`;
      case 'update_npc':
        return `changed NPC “${String(change.name ?? id)}”`;
      case 'place_npc':
        return `placed “${String(change.name ?? id)}”`;
      case 'delete_npc':
        return `deleted NPC “${String(change.name ?? id)}”`;
      case 'create_plot':
        return `new plot point “${String(change.name ?? id)}”`;
      case 'update_plot':
        return `changed plot point “${String(change.name ?? id)}”`;
      case 'delete_plot':
        return `deleted plot point “${String(change.name ?? id)}”`;
      case 'place_objects': {
        const n = Number(change.count ?? (Array.isArray(change.ids) ? change.ids.length : 1));
        return n === 1 ? 'put down an object' : `put down ${n} objects`;
      }
      case 'remove_objects': {
        if (change.name) return `took away the ${String(change.name)}`;
        const n = Number(change.count ?? (Array.isArray(change.removed) ? change.removed.length : 0));
        return n === 1 && !change.clears ? 'took away an object' : 'cleared objects';
      }
      case 'restore_objects':
        return 'brought objects back';
      case 'remove_buildings': {
        const ids = Array.isArray(change.removed) ? (change.removed as string[]) : [];
        const n = Number(change.count ?? ids.length);
        if (n === 1 && change.name) return `took away ${String(change.name)}`;
        return n === 1 && ids[0] ? `took away ${nameOf(ids[0])}` : `took away ${n} buildings`;
      }
      case 'restore_building':
        return `${nameOf(id)} as generated again`;
      case 'place_crossing':
        return `${change.changed ? 'changed' : 'put down'} a ${String(change.kind ?? 'crossing')}`;
      case 'remove_crossings': {
        const n = Array.isArray(change.ids) ? change.ids.length : 1;
        return n === 1 ? `took away a ${String(change.kind ?? 'crossing')}` : `took away ${n} crossings`;
      }
      case 'upload_sprite':
        return `sprite “${String(change.name ?? id)}”`;
      case 'remove_sprite':
        return `removed sprite “${String(change.name ?? id)}”`;
      case 'set_site_design':
        return `redesigned ${String(change.name ?? nameOf(id))}`;
      case 'start_over':
        return 'started over: every change to this world cleared';
      case 'reset_site_design':
        return `${String(change.name ?? nameOf(id))} as generated again`;
      case 'batch': {
        const steps = Array.isArray(change.changes) ? (change.changes as Change[]) : [];
        if (steps.length === 1) return describe(steps[0], 'user', nameOf);
        return `${steps.length} changes`;
      }
      case 'undo':
      case 'redo':
      case 'reanchor':
      case 'sync':
      case 'restore':
      case 'open_edits':
      case 'replace_edits':
        return String(change.label ?? change.tool);
      default:
        return 'changed the map';
    }
  })();
  return who + what.charAt(0).toUpperCase() + what.slice(1);
}
