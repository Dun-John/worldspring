// Hills raised by hand (Edit › Ground › Hill; `worldgen::ground::hill`): a plateau of kind
// 'hill' whose `hill` gives its footprint and profile and whose `level` is its peak over the
// ground. The outline is the footprint's, as `ground::hill_for` makes it; the generator shapes
// the ground from the form alone.
import type { HillProfile, Plateau } from '../gen/protocol';

/** Any hill may rise this far; a big one further (`maxPeak`, as `Hill::max_peak`). */
export const MAX_HILL_FT = 300;
export const MIN_HILL_RADIUS = 10;
const OUTLINE_PTS = 32;

/** A profile's largest half-axes (ft): along, across (`HillProfile::max_radius`; a ridge: half
 * its length, its half-width). */
export function maxRadius(profile: HillProfile): [number, number] {
  return profile === 'mountain' ? [13200, 13200] : profile === 'ridge' ? [26400, 5280] : profile === 'knoll' ? [5280, 5280] : [7920, 7920];
}

/** The highest a profile's peak may be (ft). */
export const profilePeak = (profile: HillProfile) => (profile === 'mountain' ? 2000 : profile === 'ridge' ? 1500 : 1000);

/** The highest a hill of this size may rise (ft): 300 for any, up to 0.45 of its narrower
 * half-axis for a big one, at most its profile's (`Hill::max_peak`). */
export function maxPeak(profile: HillProfile, rx: number, ry: number): number {
  const narrow = profile === 'ridge' ? ry : Math.min(rx, ry);
  return Math.min(profilePeak(profile), Math.max(MAX_HILL_FT, 0.45 * narrow));
}

/** The coarsest map level that draws a hill (`Hill::shows_at`: its narrower half-axis spans at
 * least 8 of that level's samples), never finer than the three finest's first. */
export function hillLevel(p: Plateau, maxLevel: number, domainFt: number, tileN: number): number {
  const h = p.hill;
  const fine = Math.max(0, maxLevel - 2);
  if (!h) return fine;
  const narrow = h.profile === 'ridge' ? h.ry : Math.min(h.rx, h.ry);
  let l = fine;
  while (l > 0 && narrow >= 8 * (domainFt / 2 ** (l - 1) / tileN)) l--;
  // (One coarser besides: a tile at the bound redrawn needlessly rather than missed.)
  return Math.max(0, l - 1);
}

const half = (v: number) => Math.round(v * 2) / 2;

/** The brushed change as the ground takes it (`ground::soft_shift`; hills stand on top): as it is up
 * to 220 ft, then easing toward 300 ft either way. */
export function softShift(d: number): number {
  const [knee, max] = [220, 300];
  const a = Math.abs(d);
  if (a <= knee) return d;
  const v = knee + (max - knee) * Math.tanh((a - knee) / (max - knee));
  return d < 0 ? -v : v;
}

/** A hill with `change` (its profile, peak or crown), its outline made again, its seed kept. */
export function changedHill(p: Plateau, change: { profile?: HillProfile; peak?: number; crown?: number }): Plateau | string {
  const h = p.hill;
  if (!h) return 'That is not a hill';
  const profile = change.profile ?? h.profile;
  // (A profile that may not rise so high for this size takes the highest it may.)
  const peak = Math.min(change.peak ?? p.level, maxPeak(profile, h.rx, h.ry));
  const out = hillFor('', profile, h.at, h.rx, h.ry, h.angle ?? 0, peak, change.crown ?? h.crown ?? 0);
  if (typeof out === 'string' || !out.hill) return out;
  out.hill.seed = h.seed;
  return out;
}

/** A seed from an id (FNV-1a, 32 bits). */
function seedOf(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 0x01000193) >>> 0;
  return h >>> 0;
}

/** The footprint's outline (world ft): an ellipse, or a ridge's capsule along `rx`. */
export function hillOutline(profile: HillProfile, at: [number, number], rx: number, ry: number, angle: number): [number, number][] {
  const a = (angle * Math.PI) / 180;
  const [c, s] = [Math.cos(a), Math.sin(a)];
  const h = Math.max(0, rx - ry);
  const out: [number, number][] = [];
  for (let i = 0; i < OUTLINE_PTS; i++) {
    const th = (i * 2 * Math.PI) / OUTLINE_PTS;
    const [cu, sv] = [Math.cos(th), Math.sin(th)];
    const [u, v] = profile === 'ridge' ? [(cu >= 0 ? h : -h) + ry * cu, ry * sv] : [rx * cu, ry * sv];
    out.push([half(at[0] + u * c - v * s), half(at[1] + u * s + v * c)]);
  }
  return out;
}

/** A hill, or what is wrong with it. */
export function hillFor(id: string, profile: HillProfile, at: [number, number], rx: number, ry: number, angle: number, peak: number, crown = 0): Plateau | string {
  [rx, ry] = [half(rx), half(ry)];
  if (rx < MIN_HILL_RADIUS || ry < MIN_HILL_RADIUS) return 'Drag farther: that hill is too small';
  const [ax, ay] = maxRadius(profile);
  const mi = (ft: number) => `${(ft / 5280).toLocaleString('en', { maximumFractionDigits: 1 })} mi`;
  if (rx > ax || ry > ay) return profile === 'ridge' ? `A ridge is at most ${mi(2 * ax)} long and ${mi(2 * ay)} wide` : `This hill is at most ${mi(2 * ax)} across`;
  const top = maxPeak(profile, rx, ry);
  if (!(peak >= 1 && peak <= top)) return `This hill's peak is 1 to ${Math.floor(top)} ft over the ground (a bigger one may rise higher)`;
  angle = ((Math.round(angle * 10) / 10) % 360 + 360) % 360;
  crown = Math.min(half(Math.max(0, crown)), 0.8 * Math.min(rx, ry));
  const ctr: [number, number] = [half(at[0]), half(at[1])];
  const hill = { profile, at: ctr, rx, ry, seed: seedOf(id), ...(angle ? { angle } : {}), ...(crown ? { crown } : {}) };
  return { pts: hillOutline(profile, ctr, rx, ry, angle), level: Math.round(peak * 10) / 10, kind: 'hill', hill };
}

/** A round hill to raise under a castle drawn as `poly` (`ground::castle_hill`): its middle, a
 * flat crown a little wider than the castle, a peak of 60 to 120 ft and slopes about three times
 * as long as it is high. */
export function castleHill(id: string, poly: [number, number][]): Plateau | string {
  if (poly.length < 3) return 'The castle has no outline';
  const at: [number, number] = [poly.reduce((s, p) => s + p[0], 0) / poly.length, poly.reduce((s, p) => s + p[1], 0) / poly.length];
  const reach = Math.max(...poly.map((p) => Math.hypot(p[0] - at[0], p[1] - at[1])));
  const crown = reach * 1.15 + 10;
  const peak = Math.max(60, Math.min(120, 50 + 0.4 * crown));
  const r = Math.min(640, crown + 3 * peak);
  return hillFor(id, 'round', at, r, r, 0, peak, Math.min(crown, 0.8 * r));
}
