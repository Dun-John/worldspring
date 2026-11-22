//! The world's waterfalls (`lod::rivers::Fall`) for the app and agents: where each lip is, the way
//! the water runs over it, its levels and width, and the cave behind about a third of them (a
//! cave site whose way in is secret: shown to the DM, hidden from players until found).

use crate::t0::T0;
use serde_json::{Value, json};

/// One fall as `{id, x, y, dir: [dx, dy], top, bottom, drop, width, face, river, cave}` (ft; `top`
/// and `bottom` on the world's datum; `cave`: `{x, y, layout, site, secret: true}` or null).
pub fn fall_json(t0: &T0, f: &crate::lod::rivers::Fall) -> Value {
    let cave = f.cave.map(|c| {
        let layout = (0..t0.pois.len()).find(|&i| t0.pois[i].x.to_bits() == c[0].to_bits() && t0.pois[i].y.to_bits() == c[1].to_bits()).map(|i| t0.settlements.len() + i);
        json!({ "x": c[0], "y": c[1], "layout": layout, "site": layout.map(|l| format!("u:{l}:0")), "secret": true })
    });
    let r1 = |v: f64| (v * 10.0).round() / 10.0;
    json!({
        "id": format!("fl:{}", f.id),
        "x": r1(f.at[0]), "y": r1(f.at[1]),
        "dir": [(f.dir[0] * 1e4).round() / 1e4, (f.dir[1] * 1e4).round() / 1e4],
        "top": r1(f.top as f64), "bottom": r1(f.bottom as f64), "drop": r1(f.drop_ft() as f64),
        "width": r1(f.width as f64), "face": r1(f.face_ft as f64), "river": f.river,
        "cave": cave,
    })
}

/// Every fall in `rect` (ft; all of them with `None`).
pub fn falls_json(t0: &T0, rect: Option<[f64; 4]>) -> Value {
    let r = rect.unwrap_or([f64::MIN, f64::MIN, f64::MAX, f64::MAX]);
    Value::Array(t0.rivers.falls_in(r, 0.0).map(|f| fall_json(t0, f)).collect())
}
