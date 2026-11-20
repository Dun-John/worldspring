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
  /** A step outside the edits (a settlement change waiting for a remake): what undoes and redoes
   * it (its ops are empty). */
  undo?: () => void;
  redo?: () => void;
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
      case 'create_settlement':
        return `added ${String(change.kind ?? 'settlement')} “${String(change.name ?? nameOf(id))}”`;
      case 'create_building_row': {
        const n = Number(change.count ?? (Array.isArray(change.ids) ? change.ids.length : 0));
        const rd = change.road === 'street' ? ' and a street' : change.road ? ' and a road' : '';
        if (!n) return `laid ${change.road === 'street' ? 'a street' : 'a road'} along a row`;
        return `put down ${n === 1 ? 'a building' : `${n} buildings`}${rd} in a row`;
      }
      case 'create_parcel':
        return change.name ? `made the plot “${String(change.name)}”` : 'made a plot';
      case 'update_parcel':
        return 'changed a plot';
      case 'place_stamp':
      case 'create_building':
        return `built “${String(change.name ?? nameOf(id))}”`;
      case 'update_building':
        return `changed ${String(change.name ?? nameOf(id))}`;
      case 'annotate_feature':
        return `notes on ${String(change.name ?? nameOf(id))}`;
      case 'add_note_at':
        return `note put down in ${String(change.name ?? nameOf(id))}`;
      case 'move_note':
        return `note moved to ${String(change.name ?? nameOf(id))}`;
      case 'delete_note':
        return `note deleted in ${String(change.name ?? nameOf(id))}`;
      case 'update_feature':
        return `changed ${nameOf(id)}`;
      case 'hide_feature':
        return `${change.hidden === false ? 'showed' : 'hid'} ${nameOf(id)}`;
      case 'delete_feature':
        return `deleted ${nameOf(id)}`;
      case 'restore_feature':
        return `restored ${String(change.name ?? nameOf(id))}`;
      case 'update_settlement': {
        // (What it was changed in: its culture, tier, living, planning, street pattern or name; and what
        // that set aside.)
        const what = [change.culture && `${String(change.culture)} culture`, change.kind && `a ${String(change.kind)}`, change.lives_by && `living by ${String(change.lives_by).replace(/_/g, ' ')}`, change.planning && `streets ${String(change.planning)}`, change.pattern && `streets laid out ${change.pattern === 'as chosen' ? 'as its land picks' : `as ${String(change.pattern)}`}`, change.renamed && `named “${String(change.renamed)}”`].filter(Boolean).join(', ');
        const aside = Number(change.set_aside ?? 0);
        return `${String(change.name ?? nameOf(id))}: ${what || 'changed'}${aside ? ` (${aside} ${aside === 1 ? 'change' : 'changes'} inside set aside)` : ''}`;
      }
      case 'move_settlement': {
        const aside = Number(change.set_aside ?? 0);
        return `moved ${String(change.name ?? nameOf(id))}${aside ? ` (${aside} ${aside === 1 ? 'change' : 'changes'} inside set aside)` : ''}`;
      }
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
        // (Objects given by id were moved, turned or changed, not put down.)
        const moved = Array.isArray(change.changed) ? change.changed.length : 0;
        if (moved && moved === n) return n === 1 ? 'changed an object' : `changed ${n} objects`;
        if (moved) return `put down ${n - moved} and changed ${moved} objects`;
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
      case 'place_path':
        if (change.changed) return 'changed a road';
        return `drew a ${({ footpath: 'footpath', track: 'track', gravel: 'gravel road', paved: 'paved road', cobbled: 'cobbled street' } as Record<string, string>)[String(change.kind)] ?? 'road'}`;
      case 'remove_roads': {
        const n = Array.isArray(change.ids) ? change.ids.length : 1;
        const also = Number(change.also ?? 0) ? ` (and ${Number(change.also) === 1 ? 'the street leading to its town' : `${Number(change.also)} streets leading to towns`})` : '';
        if (change.forget) return n === 1 ? 'forgot a road that was taken away' : `forgot ${n} roads that were taken away`;
        if (change.restore) return n === 1 ? 'brought a road back' : `brought ${n} roads back`;
        return (n === 1 ? 'took a road away' : `took away ${n} roads`) + also;
      }
      case 'place_fence': {
        const kinds: Record<string, string> = { hedgerow: 'hedgerow', hedge: 'clipped hedge', dry_stone_wall: 'dry-stone wall', post_and_rail: 'post-and-rail fence', wattle: 'wattle fence', garden_wall: 'garden wall', low_wall: 'low stone wall', picket: 'picket fence', iron_railing: 'iron railing', stockade: 'stockade', high_wall: 'high wall' };
        const kind = kinds[String(change.kind)];
        const n = Array.isArray(change.ids) ? change.ids.length : 1;
        if (change.gate) return 'put a gate in a fence';
        if (change.changed) return kind ? `${n === 1 ? 'changed a fence' : `changed ${n} fences`} to ${/^[aeiou]/.test(kind) ? 'an' : 'a'} ${kind}` : 'changed a fence';
        return `drew ${kind ? (/^[aeiou]/.test(kind) ? 'an ' : 'a ') + kind : 'a fence'}`;
      }
      case 'remove_fences': {
        const n = Array.isArray(change.ids) ? change.ids.length : Number(change.count ?? 1);
        const gaps = Array.isArray(change.gaps) ? change.gaps.length : Number(change.gaps ?? 0);
        if (change.forget) return 'forgot fences that were changed';
        if (change.restore) return n === 1 ? 'brought a fence back' : `brought ${n} fences back`;
        if (gaps && !n) return gaps === 1 ? 'cut a gap in the fences' : `cut ${gaps} gaps in the fences`;
        return (n === 1 ? 'took a fence away' : `took away ${n} fences`) + (gaps ? ' and cut a gap' : '');
      }
      case 'remove_town_street':
        return 'took a street away';
      case 'remove_paths': {
        const n = Array.isArray(change.ids) ? change.ids.length : 1;
        return n === 1 ? 'took a road away' : `took away ${n} roads`;
      }
      case 'place_crossing':
        return `${change.changed ? 'changed' : 'put down'} a ${String(change.kind ?? 'crossing')}`;
      case 'remove_crossings': {
        const n = Array.isArray(change.ids) ? change.ids.length : 1;
        return n === 1 ? `took away a ${String(change.kind ?? 'crossing')}` : `took away ${n} crossings`;
      }
      case 'create_link':
        return `${change.changed ? 'changed' : 'put down'} a way across (${String(change.way ?? 'stairs')})`;
      case 'delete_link':
        return 'took away a way across';
      case 'raise_islet':
        return `${change.changed ? 'changed' : 'raised'} an islet`;
      case 'place_wharf':
        return `${change.changed ? 'changed' : 'put down'} a ${String(change.kind ?? 'wharf')}${change.lighthouse ? ' with a lighthouse' : ''}`;
      case 'set_town_members': {
        const n = Number(change.count ?? 1);
        const things = n === 1 ? 'one thing' : `${n} things`;
        return change.join ? `joined ${things} to ${String(change.town ?? 'a town')}` : `took ${things} out of ${String(change.town ?? 'their town')}`;
      }
      case 'delete_selected':
        return `took away ${Number(change.count ?? 0)} things`;
      case 'remove_wharves':
        return `took away a ${String(change.kind ?? 'wharf')}`;
      case 'upload_sprite':
        return `sprite “${String(change.name ?? id)}”`;
      case 'remove_sprite':
        return `removed sprite “${String(change.name ?? id)}”`;
      case 'set_site_design':
        return `redesigned ${String(change.name ?? nameOf(id))}`;
      case 'edit_town': {
        // (The app's own changes and an agent's reply carry the same report.)
        const c = change.corners as { asked: number; moved: number; part_way: number } | undefined;
        const b = change.buildings as { added: number; taken_away: number } | undefined;
        const lost = Array.isArray(change.functions_lost) ? (change.functions_lost as string[]) : [];
        const parts: string[] = [];
        if (c?.asked) {
          const got = c.moved + c.part_way;
          parts.push(`${got === c.asked ? got : `${got} of ${c.asked}`} ${c.asked === 1 ? 'corner' : 'corners'} moved${c.part_way ? ` (${c.part_way} part way)` : ''}`);
        }
        if (b && (b.added || b.taken_away)) parts.push(`${b.added} new ${b.added === 1 ? 'building' : 'buildings'}, ${b.taken_away} gone`);
        if (lost.length) parts.push(`no room left for: ${lost.map((f) => f.replace(/_/g, ' ')).join(', ')}`);
        const town = String(change.name ?? (id ? nameOf(id) : 'the town'));
        return `laid ${town} out anew${parts.length ? `: ${parts.join('; ')}` : ''}`;
      }
      case 'edit_streets': {
        const town = String(change.name ?? (id ? nameOf(id) : 'the town'));
        const b = change.buildings as { before: number; after: number } | undefined;
        const more = b && b.before !== b.after ? ` (${b.after.toLocaleString('en')} buildings, were ${b.before.toLocaleString('en')})` : '';
        switch (change.op) {
          case 'lay_out_by_hand':
            return `laid ${town} out by hand`;
          case 'back_to_generated':
            return `${town} as generated again`;
          case 'add_street':
            return `drew a street in ${town}${more}`;
          case 'delete_street':
            return `took away a street in ${town}${more}`;
          case 'set_street':
            return `changed a street in ${town}`;
          case 'move_junctions': {
            const n = Number(change.moved ?? 0);
            return `moved ${n === 1 ? 'a junction' : `${n || 'the'} junctions`} in ${town}${more}`;
          }
          default:
            return `changed the streets of ${town}${more}`;
        }
      }
      case 'shape_ground': {
        const how: Record<string, string> = { raise: 'raised', lower: 'lowered', smooth: 'smoothed', restore: 'put back' };
        return `${how[String(change.mode)] ?? 'shaped'} the ground`;
      }
      case 'set_plateau':
        return change.changed ? 'changed a plateau' : 'made a plateau';
      case 'delete_plateau':
        return 'took away a plateau';
      case 'set_hex_grid':
        return 'changed the hex grid';
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
      case 'forget_aside':
      case 'upgrade':
        return String(change.label ?? change.tool);
      default:
        return 'changed the map';
    }
  })();
  return who + what.charAt(0).toUpperCase() + what.slice(1);
}
