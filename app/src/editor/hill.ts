// Hills raised by hand (Edit › Ground › Hill; `worldgen::ground::hill`): a plateau of kind
// 'hill' whose `hill` gives its footprint and profile and whose `level` is its peak over the
// ground. The outline is the footprint's, as `ground::hill_for` makes it; the generator shapes
// the ground from the form alone.
import type { HillProfile, Plateau } from '../gen/protocol';

export const MAX_HILL_FT = 300;
export const HILL_RADIUS: [number, number] = [10, 640];
const OUTLINE_PTS = 32;

const half = (v: number) => Math.round(v * 2) / 2;

/** The brushed change and hills as the ground takes them (`ground::soft_shift`): as they are up
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
  const out = hillFor('', change.profile ?? h.profile, h.at, h.rx, h.ry, h.angle ?? 0, change.peak ?? p.level, change.crown ?? h.crown ?? 0);
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
  if (rx < HILL_RADIUS[0] || ry < HILL_RADIUS[0]) return 'Drag farther: that hill is too small';
  if (rx > HILL_RADIUS[1] || ry > HILL_RADIUS[1]) return `A hill reaches at most ${HILL_RADIUS[1]} ft from its middle`;
  if (!(peak >= 1 && peak <= MAX_HILL_FT)) return `A hill's peak is 1 to ${MAX_HILL_FT} ft over the ground`;
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
  const r = Math.min(HILL_RADIUS[1], crown + 3 * peak);
  return hillFor(id, 'round', at, r, r, 0, peak, Math.min(crown, 0.8 * r));
}
