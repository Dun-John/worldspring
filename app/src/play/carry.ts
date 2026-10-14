// A play session carried into the world remade (a settlement added or taken back, a sketch
// redrawn): sessions are kept by world hash, the sketch in it, so the remade world would open
// with none. Its places move to their numbers there as the world's changes do (`layoutTargets`);
// tokens in places that came out differently wait outside them. The world as it was keeps its own
// session as it was.
import { layoutOfId, withLayout } from '../editor/reanchor';
import { PARKED_LAYOUT } from '../sync/ops';
import { hasSession, parseLoc, type PlayState, type Token } from './state';

/** A session taken out of the world shown before it is remade (a copy), the world hash it is kept
 * under, and for each token inside a building, site or the sewers the surface square it waits on
 * if its place comes out differently (centred, none sharing a square). */
export interface Carry {
  state: PlayState;
  from: string;
  outside: Record<string, [number, number]>;
}

/** A session carried into a remade world: how many tokens wait outside places that came out
 * differently, and how many places' fog, doors found or settings stay behind. */
export interface Carried {
  state: PlayState;
  from: string;
  out: number;
  forgot: number;
}

/** The location a door's state is kept under (`<location>#<door index>`). */
const doorLoc = (key: string) => key.slice(0, key.lastIndexOf('#'));

/** The building, site or city's sewers a location is in. */
const siteOf = (loc: string) => parseLoc(loc).interior ?? loc;

/** The layout numbers a session names places in (tokens', shapes', fog, doors, settings), below
 * `PARKED_LAYOUT`, ascending. */
export function sessionLayouts(s: PlayState): number[] {
  const out = new Set<number>();
  const locs = [...Object.values(s.tokens).map((t) => t.loc), ...Object.values(s.shapes).map((t) => t.loc), ...Object.keys(s.fog), ...Object.keys(s.locs), ...Object.keys(s.doors).map(doorLoc)];
  for (const loc of locs) {
    const li = layoutOfId(loc);
    if (li !== null && li < PARKED_LAYOUT) out.add(li);
  }
  return [...out].sort((a, b) => a - b);
}

/** How many tokens stand in layout `li`'s places. */
export const tokensIn = (s: PlayState, li: number) => Object.values(s.tokens).filter((t) => layoutOfId(t.loc) === li).length;

/**
 * The session with its places moved to their numbers in the remade world (`target`; null: it
 * came out differently, or is gone). The surface stays as it is. A token in a place that came out
 * differently waits outside it (`c.outside`), keeping everything else (hidden, vision, light,
 * picture); that place's fog, doors found, settings and shapes stay with the world as it was.
 */
export function carrySession(c: Carry, target: (li: number) => number | null): Carried {
  const to = (loc: string): string | null => {
    const li = layoutOfId(loc);
    // (The surface; ids set aside name nothing either way.)
    if (li === null || li >= PARKED_LAYOUT) return loc;
    const t = target(li);
    return t === null ? null : t === li ? loc : withLayout(loc, t);
  };
  const s = c.state;
  const forgot = new Set<string>();
  const tokens: Record<string, Token> = {};
  let out = 0;
  for (const t of Object.values(s.tokens)) {
    const loc = to(t.loc);
    const at = loc === null ? c.outside[t.id] : undefined;
    if (at) out++;
    tokens[t.id] = at ? { ...t, loc: 'surface', x: at[0], y: at[1] } : { ...t, loc: loc ?? t.loc };
  }
  /** `r` rekeyed by `key`'s location (left behind: dropped, its place remembered). */
  const rekey = <T>(r: Record<string, T>, loc: (k: string) => string, move: (k: string, to: string) => string): Record<string, T> => {
    const o: Record<string, T> = {};
    for (const [k, v] of Object.entries(r)) {
      const was = loc(k);
      const now = to(was);
      if (now === null) forgot.add(siteOf(was));
      else o[now === was ? k : move(k, now)] = v;
    }
    return o;
  };
  const shapes = Object.fromEntries(
    Object.values(s.shapes).flatMap((x) => {
      const loc = to(x.loc);
      if (loc === null) forgot.add(siteOf(x.loc));
      return loc === null ? [] : [[x.id, loc === x.loc ? x : { ...x, loc }]];
    }),
  );
  const same = (k: string) => k;
  const fog = rekey(s.fog, same, (_, now) => now);
  const locs = rekey(s.locs, same, (_, now) => now);
  const doors = rekey(s.doors, doorLoc, (k, now) => `${now}${k.slice(k.lastIndexOf('#'))}`);
  return { state: { ...s, tokens, shapes, fog, doors, locs }, from: c.from, out, forgot: forgot.size };
}

/**
 * Which session a remade world opens with: the one carried in, unless it came straight back to
 * the world it was carried from with nothing played since (Take back, or the sketch drawn back):
 * then that world's own, exactly as it was. Carried in, it notes where it came from (`came`); back
 * in the world it came from, places it knows nothing of (set aside on the way out) keep what this
 * world's own session knew of them: fog, doors, settings. Any other session the remade world had
 * gives way to the one in use.
 */
export function settle(c: Carried, stored: PlayState, hash: string): { state: PlayState; how: 'carried' | 'back' } {
  const s = c.state;
  if (hasSession(stored) && s.came?.from === hash && (s.changed ?? 0) === s.came.changed) return { state: stored, how: 'back' };
  const out: PlayState = { ...s, fog: { ...s.fog }, doors: { ...s.doors }, locs: { ...s.locs }, came: { from: c.from, changed: s.changed ?? 0 } };
  if (s.came?.from === hash) {
    for (const [loc, f] of Object.entries(stored.fog)) if (loc !== 'surface' && !(loc in out.fog)) out.fog[loc] = f;
    const known = new Set(Object.keys(s.doors).map(doorLoc));
    for (const [k, d] of Object.entries(stored.doors)) if (!known.has(doorLoc(k))) out.doors[k] = d;
    for (const [loc, v] of Object.entries(stored.locs)) if (loc !== 'surface' && !(loc in out.locs)) out.locs[loc] = v;
  }
  return { state: out, how: 'carried' };
}
