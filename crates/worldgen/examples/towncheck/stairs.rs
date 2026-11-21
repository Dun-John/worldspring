use crate::prelude::*;
use worldgen::t0::settle::Tier;
use worldgen::town::stairs::{self, StairParams};

/// Steep streets with no steps (towns and up, `town::stairs`): along every street the survey
/// judges, the stretches between stations both at `g_on` or steeper, and how much of that has no
/// flight on it: excused where the ground is level by rule (water, bridges, plazas, alleys
/// meeting it) or in a corner's landing, else counted (ft). `STAIRS_TOWN=<name>` prints that
/// settlement's streets station by station.
pub(crate) fn stairs_check(world: &World, t0: &T0, label: &str, r: &mut Report) {
    let prm = StairParams::DEFAULT;
    let names = &t0.extra.as_ref().unwrap().overlay.features;
    let want = std::env::var("STAIRS_TOWN").ok();
    r.counts(&["steep street ft", "steep ft stepped", "steep ft in landings", "steep ft level by rule (water, river, bridges, plazas, alleys meeting)", "steep street length without stairs (ft)"]);
    for (si, st) in t0.settlements.iter().enumerate().filter(|(_, st)| st.tier >= Tier::Town) {
        let name = names.iter().find(|f| (f.x - st.x).abs() < 1.0 && (f.y - st.y).abs() < 1.0).map_or(String::new(), |f| f.name.clone());
        let show = want.as_deref() == Some(name.as_str());
        if want.is_some() && !show {
            continue;
        }
        let profiles = stairs::survey(world, t0, si);
        let (mut steep, mut stepped, mut level, mut landing, mut bare) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (pi, p) in profiles.iter().enumerate() {
            let runs = stairs::runs(p, &prm);
            let cum = |s: f64| {
                let mut acc = 0.0;
                for w in p.line.windows(2) {
                    let d = dist(w[0], w[1]);
                    if acc + d >= s {
                        return lerp(w[0], w[1], ((s - acc) / d.max(1e-9)).clamp(0.0, 1.0));
                    }
                    acc += d;
                }
                *p.line.last().unwrap()
            };
            let mut worst: Option<(f64, f64)> = None;
            for k in 1..p.s.len() {
                let (s0, s1) = (p.s[k - 1], p.s[k]);
                let ds = s1 - s0;
                if p.g[k - 1] < prm.g_on || p.g[k] < prm.g_on {
                    continue;
                }
                steep += ds;
                let mid = 0.5 * (s0 + s1);
                if runs.iter().any(|r| mid >= r.0 && mid <= r.1) {
                    stepped += ds;
                } else if p.level[k - 1] || p.level[k] {
                    level += ds;
                } else if mid < p.ends[0] || mid > p.len - p.ends[1] {
                    landing += ds;
                } else {
                    bare += ds;
                    worst = Some(worst.map_or((s0, s1), |w| (w.0, s1)));
                }
            }
            if let Some((a, b)) = worst {
                let at = cum(0.5 * (a + b));
                r.add("steep street length without stairs (ft)", b - a, format!("{label} #{si} {name} {:?} class {} len {:.0}: {:.0} ft bare at ({:.0}, {:.0}), {} runs", p.kind, p.class, p.len, b - a, at[0], at[1], runs.len()));
            }
            if show && p.g.iter().any(|&g| g >= prm.g_on) {
                let at = cum(0.0);
                say!("      street {pi} {:?} class {} len {:.0} from ({:.0}, {:.0}) ends {:.1}/{:.1} runs {:?}", p.kind, p.class, p.len, at[0], at[1], p.ends[0], p.ends[1], runs.iter().map(|r| (r.0.round(), r.1.round())).collect::<Vec<_>>());
                let row: Vec<String> = (0..p.s.len()).map(|k| format!("{:.2}{}", p.g[k], if p.level[k] { "L" } else { "" })).collect();
                say!("        g {}", row.join(" "));
            }
        }
        if show {
            say!("    #{si} {name} {:?} at ({:.0}, {:.0}): steep {steep:.0} stepped {stepped:.0} level {level:.0} landing {landing:.0} bare {bare:.0}", st.tier, st.x, st.y);
        }
        r.tally("steep street ft", steep as usize);
        r.tally("steep ft stepped", stepped as usize);
        r.tally("steep ft level by rule (water, river, bridges, plazas, alleys meeting)", level as usize);
        r.tally("steep ft in landings", landing as usize);
        // (The bare total is the count's n: tally the feet beyond the one `add` counted.)
        if let Row::Count { n, .. } = r.row("steep street length without stairs (ft)", || Row::Count { n: 0, worst: Vec::new() }) {
            *n += bare as usize;
        }
    }
    // `add` counted one per street listed: take those back out, leaving feet.
    if let Row::Count { n, worst } = r.row("steep street length without stairs (ft)", || Row::Count { n: 0, worst: Vec::new() }) {
        *n -= worst.len();
    }
}
