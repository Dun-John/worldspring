use super::*;

// ---------------------------------------------------------------------------------------------
// Grid arithmetic.

pub(super) fn grid(p: P) -> Option<I> {
    // NaN fails both comparisons.
    if !(fabs(p[0]) <= LIMIT_FT && fabs(p[1]) <= LIMIT_FT) {
        return None;
    }
    Some([round(p[0] * SCALE) as i64, round(p[1] * SCALE) as i64])
}

pub(super) fn feet(p: I) -> P {
    [p[0] as f64 / SCALE, p[1] as f64 / SCALE]
}

#[inline]
pub(super) fn isub(a: I, b: I) -> I {
    [a[0] - b[0], a[1] - b[1]]
}

#[inline]
pub(super) fn icross(a: I, b: I) -> i128 {
    a[0] as i128 * b[1] as i128 - a[1] as i128 * b[0] as i128
}

#[inline]
pub(super) fn idot(a: I, b: I) -> i128 {
    a[0] as i128 * b[0] as i128 + a[1] as i128 * b[1] as i128
}

/// Twice the signed area of triangle abc (positive when c is left of a->b).
#[inline]
pub(super) fn orient(a: I, b: I, c: I) -> i128 {
    icross(isub(b, a), isub(c, a))
}

/// The next and previous place round a ring of `n` (no division in the hot loops).
#[inline]
pub(super) fn nx(i: usize, n: usize) -> usize {
    if i + 1 == n { 0 } else { i + 1 }
}

#[inline]
pub(super) fn pv(i: usize, n: usize) -> usize {
    if i == 0 { n - 1 } else { i - 1 }
}

pub(super) fn area2(r: &[I]) -> i128 {
    let n = r.len();
    (0..n).map(|i| icross(r[i], r[nx(i, n)])).sum()
}

/// `x / den` rounded to the nearest integer (halves up), `den > 0`.
pub(super) fn rdiv(x: i128, den: i128) -> i64 {
    (2 * x + den).div_euclid(2 * den) as i64
}

/// A ring on the grid with repeated points dropped; `None` when a point is invalid.
pub(super) fn grid_ring(r: &[P]) -> Option<Vec<I>> {
    let mut out: Vec<I> = Vec::with_capacity(r.len());
    for p in r {
        let q = grid(*p)?;
        if out.last() != Some(&q) {
            out.push(q);
        }
    }
    while out.len() > 1 && out[0] == out[out.len() - 1] {
        out.pop();
    }
    Some(out)
}

pub(super) fn rotate_least(r: &mut [I]) {
    if let Some(k) = (0..r.len()).min_by_key(|&i| r[i]) {
        r.rotate_left(k);
    }
}

pub(super) fn off(p: P, n: P, d: f64) -> P {
    [p[0] + n[0] * d, p[1] + n[1] * d]
}

pub(super) fn unit(v: P) -> P {
    let l = geom::len(v);
    if l > 0.0 { [v[0] / l, v[1] / l] } else { [1.0, 0.0] }
}

/// The piece filling the gap an offset opens at corner `v`, between `p1 = v + r n1` and
/// `p2 = v + r n2` (unit normals; the gap turns anticlockwise from n1 to n2 when `ccw`).
pub(super) fn join_piece(v: P, n1: P, n2: P, p1: P, p2: P, r: f64, ccw: bool, join: Join) -> Vec<P> {
    let c = geom::dot(n1, n2).clamp(-1.0, 1.0);
    match join {
        Join::Bevel => vec![v, p1, p2],
        Join::Miter(limit) => {
            let k = 1.0 + c;
            if k > 1e-12 {
                let m = off(v, geom::add(n1, n2), r / k);
                if geom::dist(m, v) <= limit.max(1.0) * r {
                    return vec![v, p1, m, p2];
                }
            }
            vec![v, p1, p2]
        }
        Join::Round => {
            let phi = libm::acos(c);
            let step = if r > 2.0 * ROUND_TOL { 2.0 * libm::acos(1.0 - ROUND_TOL / r) } else { core::f64::consts::FRAC_PI_4 };
            let steps = (ceil(phi / step.max(1e-3)) as usize).clamp(1, 256);
            let a0 = libm::atan2(n1[1], n1[0]);
            let sg = if ccw { 1.0 } else { -1.0 };
            let mut out = vec![v, p1];
            for k in 1..steps {
                let a = a0 + sg * phi * k as f64 / steps as f64;
                out.push([v[0] + r * libm::cos(a), v[1] + r * libm::sin(a)]);
            }
            out.push(p2);
            out
        }
    }
}

/// The half-plane (normal, offset for `clip_plain`) on `keep`'s side of the mitre line at corner
/// `v`, from `v` to where the setback lines of its two edges meet (inner normals `ni`, `nj`,
/// distances `di`, `dj`), moved `over` ft past the line.
pub(super) fn mitre_side(v: P, ni: P, di: f64, nj: P, dj: f64, keep: P, over: f64) -> Option<(P, f64)> {
    let det = geom::cross(ni, nj);
    if fabs(det) < 1e-12 {
        return None;
    }
    let q = [(nj[1] * di - ni[1] * dj) / det, (-nj[0] * di + ni[0] * dj) / det];
    let l = geom::len(q);
    if l < 1e-12 {
        return None;
    }
    let mut nrm = [q[1] / l, -q[0] / l];
    if geom::dot(geom::sub(keep, v), nrm) > 0.0 {
        nrm = [-nrm[0], -nrm[1]];
    }
    Some((nrm, geom::dot(v, nrm) + over))
}

/// A setback strip `z` ended at reflex corner `v` along its mitre line, keeping the side of `far`
/// (the edge's other end); `toward` is the edge's unit direction from `far` to `v`. The strip
/// overshoots the line by `SEAM` at `v`, tapering to nothing at the mitre point (rounded to the
/// grid, so both strips meeting there pivot on the same point): the strips then overlap along the
/// mitre line without notching the corner of the result. It also stops `SEAM` past the farther of
/// `v` and the mitre point along the edge (a mitre line nearly along the strip cuts it nowhere near).
pub(super) fn end_at_mitre(z: &[P], v: P, ni: P, di: f64, nj: P, dj: f64, far: P, toward: P) -> Vec<P> {
    let det = geom::cross(ni, nj);
    if fabs(det) < 1e-12 {
        return z.to_vec();
    }
    let q = [(nj[1] * di - ni[1] * dj) / det, (-nj[0] * di + ni[0] * dj) / det];
    let Some(mg) = grid(geom::add(v, q)) else { return z.to_vec() };
    let mg = feet(mg);
    let e = geom::sub(mg, v);
    let l = geom::len(e);
    if l < 1e-9 {
        return z.to_vec();
    }
    // Off the line, away from `far`.
    let mut away = [e[1] / l, -e[0] / l];
    if geom::dot(geom::sub(far, v), away) > 0.0 {
        away = [-away[0], -away[1]];
    }
    let a = off(v, away, SEAM);
    let f = geom::sub(mg, a);
    let fl = geom::len(f);
    let mut nrm = [f[1] / fl, -f[0] / fl];
    if geom::dot(geom::sub(far, a), nrm) > 0.0 {
        nrm = [-nrm[0], -nrm[1]];
    }
    let z = geom::clip_plain(z, nrm, geom::dot(a, nrm));
    let reach = geom::dot(v, toward).max(geom::dot(mg, toward)) + SEAM;
    geom::clip_plain(&z, toward, reach)
}
