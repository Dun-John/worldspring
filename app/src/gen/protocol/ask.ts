import type { Created } from './world';
import type { Edits } from './edits';
import type { Feature } from './interior';
import type { EditOp } from '../../sync/ops';
import type { Rect } from './messages';

/** Gazetteer questions answered by a generator worker (see `worldgen::gazetteer`). */
export type Ask =
  | { op: 'query'; x: number; y: number }
  /** Everything within `radius` ft of (x, y), topmost first (`pick::pick`). */
  | { op: 'pick'; x: number; y: number; radius: number }
  /** `rect` (x0, y0, x1, y1 ft) limits the search to that area. */
  | { op: 'search'; q: string; rect?: [number, number, number, number] }
  | { op: 'districts'; settlement: number }
  /** The roads someone named and the crossings put down by hand (`roadids::named_json`). */
  | { op: 'named' }
  /** Underground sites by name, kind, theme and what is attached (`undersearch::search_json`; `filters` JSON). */
  | { op: 'under'; q: string; filters: string }
  | { op: 'inview'; rect: [number, number, number, number] }
  /** `b:<settlement>:<building>` or `t:<settlement>:<tower>`. */
  | { op: 'interior'; id: string }
  /** Any place id: its name and position (`worldgen::agent::place`). */
  | { op: 'place'; id: string }
  /** What can be renamed in a layout (its index) or a building or site (its id). */
  | { op: 'names'; scope: string }
  /** The sites standing in settlements (`agent::parents_json`). */
  | { op: 'parents' }
  /** Town layout `town`'s members changed by hand, as `members` entries and created places (`town::members::change`); ids as JSON arrays. */
  | { op: 'members'; town: number; add: string; remove: string }
  /** The towns things could join and whether a generated building may leave its town (`town::members::join_reach`); ids as a JSON array. */
  | { op: 'reach'; ids: string; within: number }
  /** Where a site can be created near (x, y), and a name for it (`agent::creation_spot`). */
  | { op: 'spot'; kind: string; under?: string; id: string; x: number; y: number }
  /** Whether a building drawn by hand can stand there, its point and a name
   * (`agent::building_spot`). */
  | { op: 'building'; poly: [number, number][]; func?: string; id: string }
  /** A building of a kind set down at a point (`placing::stamp_json`). */
  | { op: 'stamp'; input: string }
  /** A row of buildings along a path (`placing::row_json`). */
  | { op: 'row'; input: string }
  /** Whether an outline can be a plot (`placing::parcel_check_json`). */
  | { op: 'parcel'; input: string }
  /** What a building can be (`agent::building_funcs_json`). */
  | { op: 'funcs' }
  /** A generated building's edit with a change made, or its removal (`agent::building_edit_json`). */
  | { op: 'bedit'; id: string; change: string }
  /** The same for many buildings at once: the answers in a list, in order. */
  | { op: 'bedits'; ids: string[]; change: string }
  /** The generated buildings with their middle inside a polygon (`agent::generated_buildings_in`). */
  | { op: 'bin'; poly: [number, number][] }
  | { op: 'works'; site: Created }
  /** Shop lots along a bridge, or its shops laid out again after a change (`wharf::shop_lots`,
   * `Edits::follow_crossing`; `input` JSON). */
  | { op: 'bridgelots'; input: string }
  /** The world's generated islets (`t0::islets`). */
  | { op: 'islets' }
  /** A hand islet round a point (`wharf::islet_at`). */
  | { op: 'islet'; key: string; x: number; y: number; radius: number; rise: number }
  /** Whether a wharf can be put down, with its lighthouse (`wharf::root_problem`). */
  | { op: 'wharfcheck'; wharf: string; lighthouse: boolean }
  /** An underground site as a design (the one given, else its own), changed by `action`, with
   * what it builds and its problems (`under::design::design_json`). */
  | { op: 'design'; id: string; design?: string; action?: string }
  /** What the designer offers (`under::design::catalog_json`). */
  | { op: 'undercat' }
  /** A town's plan for the ward editor (`town::wards::plan_json`). */
  | { op: 'townplan'; layout: number }
  /** A town changed as asked, without changing the edits (`town::wards::change_json`). */
  | { op: 'townchange'; layout: number; request: string }
  /** A hand town's street plan (`town::handedit::plan_json`). */
  | { op: 'streetplan'; layout: number }
  /** A hand town's streets changed as asked, without changing the edits (`town::handedit::change_json`); `request` is a `StreetRequest` as JSON. */
  | { op: 'streetchange'; layout: number; request: string }
  /** The villages and towns added by hand (`MadeSettlement`s). */
  | { op: 'made' }
  /** Whether a village or town can be added there (`t0::made::spot`), with what it would be. */
  | { op: 'madespot'; kind: string; id: string; x: number; y: number }
  /** Roads taken off the map or brought back, the edits left alone (`roadcut::cut`; `input` JSON `{ids, restore?, dry?}`). */
  | { op: 'roadcut'; input: string }
  /** What was taken off the map by hand, for the Gazetteer (`roadcut::removed`). */
  | { op: 'removedroads' }
  /** Fences changed, drawn or cut, the edits left alone (`fencecut::change`; `input` JSON as `fence_change_json`). */
  | { op: 'fencechange'; input: string }
  /** The fences within `r` ft of a point (`fencecut::near`). */
  | { op: 'fencesnear'; x: number; y: number; r: number; limit: number }
  /** The towns whose fences were changed by hand (`fencecut::changed`). */
  | { op: 'changedfences' }
  /** The overlay's bridge marks without the stretches taken off the map (`bridge_marks_json`). */
  | { op: 'bridgemarks' }
  /** One of the world's own settlements as its sketch pin would take it (`agent::settlement_facts`). */
  | { op: 'settlefacts'; id: string }
  /** Edits with those of the villages and towns added by hand that they remove, move or lay out
   * anew set aside (`t0::made::set_aside_changed`; `old`: the created sites before, JSON). */
  | { op: 'setaside'; old: string; edits: string }
  /** Edits with settlement `layout` laid out in `culture` (0 north … 4 steppe) and the edits inside
   * it set aside (`town::culture::change`; nothing changed in the worker); without `culture`, the
   * culture it is laid out in now. */
  | { op: 'setculture'; layout: number; culture?: number; edits: string }
  /** Edits with deleted village or town `id` restored, if its spot is free, and what was set aside
   * when it was deleted brought back where it fits (`t0::made::restore`). */
  | { op: 'restoremade'; id: string; edits: string }
  /** The fingerprints a remake of this world needs (`t0::remap::prints_before`; `edits` JSON). */
  | { op: 'remakeprints'; edits: string; played: number[] }
  /** Edits made on the world before following their places into this one, remade
   * (`t0::remap::follow_into`; `input` JSON: `RemakeInput`). */
  | { op: 'follow'; input: string };

/** What a world remade from a changed sketch needs to move the edits made on the world before
 * (`follow_json`): the edits, that world's places as its overlay has them, its layouts'
 * fingerprints (`remakePrints`), the settlements taken away on purpose and a play session's
 * layouts (`played`). */
export interface RemakeInput {
  edits: Edits;
  before: { features: Feature[]; first_created?: number; layout_ids?: number[] };
  was: [number, number][];
  gone: { id: string; name: string; li: number }[];
  played: number[];
}

/** The edits followed into the world remade (`followed`: null if nothing moved; `ops` turn the
 * edits given into them; `kept`, `parked`, `removed`: how many entries followed their places,
 * were set aside, or were set aside with a place taken away), and where each of the play
 * session's layouts goes (`targets`, null: it came out differently or is gone). */
export interface Remade {
  followed: { ops: EditOp[]; kept: number; parked: number; removed: number; moved: [string, string][] } | null;
  targets: (number | null)[];
}

/** A village or town added by hand, as the generator has it (`made_json`): its layout index,
 * `reach` (its layout's), `built` (built-up radius), `rect` (all its battlemaps change over: the
 * pad and the track) and its track's points (ft) from its edge to the road. */
/** One of the world's own settlements as its sketch pin would take it (`agent::settlement_facts`). */
export interface SettlementFacts {
  layout: number;
  /** The pin's stroke that made it (else null: generated). */
  pin: number | null;
  tier: 'metropolis' | 'city' | 'town' | 'village';
  /** The tier it was placed at (a pin replacing it says so). */
  planned: 'metropolis' | 'city' | 'town' | 'village';
  /** What it lives by (a pin kind). */
  kind: string;
  name: string;
  spot: [number, number];
  capital: boolean;
  port_ok: boolean;
  fishing_ok: boolean;
}

export interface MadeSettlement {
  id: string;
  layout: number;
  tier: string;
  population: number;
  x: number;
  y: number;
  reach: number;
  built: number;
  rect: Rect;
  track: [number, number][];
  /** The link's road class (`RoadClass`: 2 a village's track, 1 a town's road) and bed width (ft). */
  class: number;
  width: number;
  /** Its culture (0 north … 4 steppe) and what it lives by (`SettleKind::name`): a change to
   * either lays it out anew. */
  culture: number;
  lives: string;
}

/** A name to rename (`worldgen::agent::name_entry`): `generated` is the name before renames. */
export interface NameEntry {
  id: string;
  kind: string;
  name: string;
  generated: string;
  x_ft?: number;
  y_ft?: number;
  /** A building or site with levels and rooms. */
  enter?: boolean;
  hidden?: boolean;
}

/** The sites standing in settlements (`worldgen::agent::parents_json`): `[site layout, settlement]`
 * pairs, with the counts of settlements (`n`) and of sites not created by hand (`base`) the
 * layout indices assume. */
export interface Parents {
  n: number;
  base: number;
  pairs: [number, number][];
  /** Each drawn thing's town (a layout index, past `n` for a town or place made by hand) and why: by the
   * 300 ft rule (`gap`, with the distance in ft) or by hand. */
  owners?: [number, number, 'gap' | 'hand', number | null][];
  /** Generated buildings pushed out of their town: `[place, town, building ids]`. */
  pushed?: [number, number, string[]][];
  /** Settlements and places by feature id: `[id, kind, extent ft, people]`. */
  sizes?: [string, string, number, number][];
}

/** What `members` answers: the `Edits.members` entries to set (`{}` takes one away), the places to append to
 * `Edits.created`, and what was left as it was (`why` in plain words). */
export type MembersChange =
  | { members: Record<string, { add?: string[]; remove?: string[] }>; created: Created[]; refused: { id: string; why: string }[]; report?: unknown }
  | { error: string };

export interface PlaceInfo {
  id: string;
  name: string | null;
  /** The name before renames. */
  generated?: string | null;
  x: number;
  y: number;
}
