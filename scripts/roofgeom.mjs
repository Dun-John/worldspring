// Roof geometry QA over `cargo run --profile fast -p worldgen --example roofgeom [-- all] > out.json`:
// `node scripts/roofgeom.mjs out.json [list]` lays each roof out with the app's maths (`gen/roofGeom.ts`)
// and counts domes leaving their footprint or over another building, domes not matching the roof
// terrace's (`town::roof::domes`), footprints overlapping, roofs split into parts, sliver parts, hip
// lines crossing and ridges or hips leaving the footprint (all but `split` must be 0, `overlap` aside).
// `list`: the first few of each, as map fractions for `shot.mjs`.
import { readFileSync } from 'node:fs';
import { domeSpots, frame, hipLines, roofParts, signedArea } from '../app/src/gen/roofGeom.ts';

const [file, list] = process.argv.slice(2);
const towns = JSON.parse(readFileSync(file, 'utf8'));
const PX = 64;
const FT = PX / 5;
const n = { roofs: 0, hipped: 0, split: 0, sliver: 0, cross: 0, outside: 0, domed: 0, domes: 0, overflow: 0, mismatch: 0, overlap: 0, domeOver: 0 };
const seen = {};
const note = (k, t, b, extra = '') => {
  seen[k] = (seen[k] ?? 0) + 1;
  if (list && seen[k] <= 4) {
    const m = b.poly.length;
    const [x, y] = b.poly.reduce((a, p) => [a[0] + p[0] / m, a[1] + p[1] / m], [0, 0]);
    console.log(`${k.padEnd(9)} seed ${t.seed} town ${t.si}${t.ain ? ' (Ain Keshir)' : ''} b ${b.id} fx ${(x / t.w).toFixed(6)} fy ${(y / t.h).toFixed(6)} ${extra}`);
  }
};
// (As the payload has it: chunk-local squares in f32, then px.)
const toPx = (poly) => {
  const [ox, oy] = [Math.floor(poly[0][0] / 5 / 128) * 128, Math.floor(poly[0][1] / 5 / 128) * 128];
  return { pts: poly.flatMap(([x, y]) => [Math.fround(x / 5 - ox) * PX, Math.fround(y / 5 - oy) * PX]), ox, oy };
};
const inside = (pts, x, y) => {
  let c = false;
  const m = pts.length / 2;
  for (let i = 0, j = m - 1; i < m; j = i++) {
    const [ax, ay, bx, by] = [pts[i * 2], pts[i * 2 + 1], pts[j * 2], pts[j * 2 + 1]];
    if (ay > y !== by > y && x < ((bx - ax) * (y - ay)) / (by - ay) + ax) c = !c;
  }
  return c;
};
const segDist = (x, y, ax, ay, bx, by) => {
  const [ex, ey] = [bx - ax, by - ay];
  const t = Math.max(0, Math.min(1, ((x - ax) * ex + (y - ay) * ey) / (ex * ex + ey * ey || 1)));
  return Math.hypot(x - ax - ex * t, y - ay - ey * t);
};
const edgeDist = (pts, x, y) => {
  let d = Infinity;
  const m = pts.length / 2;
  for (let i = 0; i < m; i++) d = Math.min(d, segDist(x, y, pts[i * 2], pts[i * 2 + 1], pts[((i + 1) % m) * 2], pts[((i + 1) % m) * 2 + 1]));
  return d;
};
// Proper crossing of two segments (not at or near their ends).
const crosses = (a, b) => {
  const o = (p, q, r) => (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
  const [p, q, r, s] = [a.slice(0, 2), a.slice(2), b.slice(0, 2), b.slice(2)];
  const tol = 0.3 * FT;
  for (const e of [p, q]) if (segDist(e[0], e[1], ...b) < tol) return false;
  for (const e of [r, s]) if (segDist(e[0], e[1], ...a) < tol) return false;
  return o(p, q, r) * o(p, q, s) < 0 && o(r, s, p) * o(r, s, q) < 0;
};

for (const t of towns) {
  const bs = t.b.map((b) => ({ ...b, px: toPx(b.poly) }));
  // Footprints in one frame (ft) for overlaps.
  const ft = bs.map((b) => b.poly.flat());
  const box = ft.map((p) => {
    const xs = p.filter((_, i) => i % 2 === 0), ys = p.filter((_, i) => i % 2 === 1);
    return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
  });
  const others = (k, bx) => bs.map((_, j) => j).filter((j) => j !== k && box[j][0] < bx[2] && box[j][2] > bx[0] && box[j][1] < bx[3] && box[j][3] > bx[1]);
  bs.forEach((b, k) => {
    if (b.bat) return;
    n.roofs++;
    const pts = b.px.pts;
    for (const j of others(k, box[k])) {
      if (j < k) continue;
      const deep = (a, c) => { for (let i = 0; i < ft[a].length; i += 2) if (inside(ft[c], ft[a][i], ft[a][i + 1]) && edgeDist(ft[c], ft[a][i], ft[a][i + 1]) > 0.5) return true; return false; };
      if (deep(k, j) || deep(j, k)) { n.overlap++; note('overlap', t, b, `with ${bs[j].id}`); }
    }
    if (b.parts) return;
    if (b.shape === 3 && !b.cone) {
      n.domed++;
      const spots = domeSpots(pts, FT);
      n.domes += spots.length;
      for (const [x, y, r] of spots) {
        const R = r;
        if (!inside(pts, x, y) || edgeDist(pts, x, y) < R - 0.1 * FT) { n.overflow++; note('overflow', t, b, `r ${(R / FT).toFixed(1)} ft clear ${(edgeDist(pts, x, y) / FT).toFixed(1)}`); }
        const [wx, wy] = [(x / PX + b.px.ox) * 5, (y / PX + b.px.oy) * 5];
        for (const j of others(k, [wx - R / FT, wy - R / FT, wx + R / FT, wy + R / FT])) if (inside(ft[j], wx, wy) || edgeDist(ft[j], wx, wy) < R / FT - 0.1) { n.domeOver++; note('domeOver', t, b, `over ${bs[j].id}`); break; }
      }
      const tr = b.domes.map(([x, y, r]) => [(x / 5 - b.px.ox) * PX, (y / 5 - b.px.oy) * PX, r * FT]);
      const ok = tr.length === 0 || (tr.length === spots.length && tr.every(([x, y, r]) => spots.some((s) => Math.hypot(x - s[0], y - s[1]) < 0.1 * FT && Math.abs(r - s[2]) < 0.1 * FT)));
      if (!ok) { n.mismatch++; note('mismatch', t, b, JSON.stringify(tr.map((d) => d.map((v) => +(v / FT).toFixed(1))))); }
      return;
    }
    if (b.cone || b.shape === 4 || b.shape === 2) return;
    n.hipped++;
    const parts = roofParts(pts);
    if (parts.length > 1) { n.split++; note('split', t, b, `${parts.length} parts of ${pts.length / 2} corners`); }
    let sl = false;
    for (const p of parts) {
      const f = frame(p.pts);
      const a = Math.abs(signedArea(p.pts)) / (PX * PX);
      if (parts.length > 1 && (a < 1.5 || (f.hi - f.lo) / Math.max(1e-6, 2 * f.halfW) > 6 || (2 * f.halfW) / Math.max(1e-6, f.hi - f.lo) > 6)) sl = true;
    }
    if (sl) { n.sliver++; note('sliver', t, b); }
    const lines = parts.flatMap((p) => hipLines(p));
    let x = false, out = false;
    for (let i = 0; i < lines.length && !x; i++) for (let j = i + 1; j < lines.length && !x; j++) if (crosses(lines[i], lines[j])) x = true;
    for (const l of lines) for (const [qx, qy] of [[l[0], l[1]], [l[2], l[3]]]) if (!inside(pts, qx, qy) && edgeDist(pts, qx, qy) > 0.2 * FT) out = true;
    if (x) { n.cross++; note('cross', t, b); }
    if (out) { n.outside++; note('outside', t, b); }
  });
}
console.log(Object.entries(n).map(([k, v]) => `${k} ${v}`).join(', '));
