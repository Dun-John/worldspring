//! A town's district groups (`plan_town`'s phase after the wards): neighbouring patches of a
//! ward that share one set of block and lot parameters.

use super::*;

/// Districts: neighbouring patches of the same ward merge into groups of 1–5 that share
/// one set of block and lot parameters. No street runs between the patches of a group,
/// so their blocks meet at party walls instead of each reading as a Voronoi cell. Main
/// streets, water and the wall line are never merged across.
///
/// Returns the seed of the groups' block and lot parameters and each patch's group (its first
/// patch), else `usize::MAX`.
pub(super) fn merge(
    cells: &[Labeled],
    ward: &[Option<Ward>],
    wet: &[bool],
    n_inner: usize,
    pseed: u64,
    edge_v: impl Fn(usize, usize) -> (usize, usize),
    main_edge: impl Fn(usize, usize) -> bool,
) -> (u64, Vec<usize>) {
    let total = cells.len();
    let groupable = |w: Option<Ward>| {
        matches!(w, Some(Ward::Merchant | Ward::Craft | Ward::Common | Ward::Noble | Ward::Slum | Ward::Docks | Ward::Military | Ward::Temple))
    };
    let base_seed = hash2(pseed, 0x6a5e, 7);
    let mut group = vec![usize::MAX; total];
    for start in 0..total {
        if group[start] != usize::MAX || !groupable(ward[start]) || cells[start].pts.len() < 3 {
            continue;
        }
        let mut grng = Pcg32::new(hash2(base_seed, start as i64, 0x6e0), 43);
        group[start] = start;
        let mut members = vec![start];
        let mut k = 0;
        'grow: while k < members.len() {
            let i = members[k];
            k += 1;
            let m = cells[i].pts.len();
            for e in 0..m {
                let nb = cells[i].labels[e];
                if nb < 0 {
                    continue;
                }
                let j = nb as usize;
                let (ka, kb) = edge_v(i, e);
                if group[j] != usize::MAX
                    || ward[j] != ward[start]
                    || wet[j]
                    || cells[j].pts.len() < 3
                    || (j < n_inner) != (start < n_inner)
                    || main_edge(ka, kb)
                {
                    continue;
                }
                // Stop growing with probability (n − 3) / n (so 1–5 patches, mostly 2–4).
                let n = members.len() as f64;
                if n >= 5.0 || grng.next_f64() < (n - 3.0) / n {
                    break 'grow;
                }
                group[j] = start;
                members.push(j);
            }
        }
    }
    (base_seed, group)
}
