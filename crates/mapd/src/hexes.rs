//! MCP tools for the overland hex grid (`Edits::hexes`): its settings, and which hex a place or point is in.
//! The geometry follows `app/src/render/hex.ts` (same numbering, same corners).

use serde_json::{Value, json};
use worldgen::world::HexGrid;

use crate::Shared;
use crate::tools::{schema, text};

const FT_PER_MI: f64 = 5280.0;
const KEY: &str = "overland";
const SIZE_MI: (f64, f64) = (0.5, 60.0);
const WIDTH_PX: (f64, f64) = (0.5, 6.0);

pub fn list() -> Vec<Value> {
    let fields = || {
        json!({
            "size_mi": { "type": "number", "minimum": SIZE_MI.0, "maximum": SIZE_MI.1, "description": "Miles across the flats; default 6" },
            "flat": { "type": "boolean", "description": "Flat-topped (default) or pointy" },
            "color": { "type": "string" },
            "width": { "type": "number", "minimum": WIDTH_PX.0, "maximum": WIDTH_PX.1 },
            "opacity": { "type": "number", "minimum": 0, "maximum": 1 },
            "ox_mi": { "type": "number", "minimum": -10000, "maximum": 10000, "description": "Move the grid east (mi)" },
            "oy_mi": { "type": "number", "minimum": -10000, "maximum": 10000, "description": "Move the grid south (mi)" },
            "numbers": { "type": "boolean" },
        })
    };
    let mut set = fields();
    set["reset"] = json!({ "type": "boolean", "description": "Defaults first, then the fields given" });
    vec![
        json!({ "name": "get_hex_grid", "title": "Hex grid settings", "description": "The world's overland hex grid settings (defaults where unset; `set` says whether any are).", "inputSchema": schema(json!({}), &[]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "set_hex_grid", "title": "Set the hex grid", "description": "Change the overland hex grid: only the fields given change; reset goes back to the defaults. Returns the grid.", "inputSchema": schema(set, &[]) }),
        json!({ "name": "hex_at", "title": "Hex of a place", "description": "The hex holding a place (`at`), or the hex named by `hex`: label, key, centre, six corners (world ft), on_map and its neighbours. Hex 0101 is the first whole hex at the map's top-left.", "inputSchema": schema(json!({
            "at": crate::args::point_schema("A place"),
            "hex": { "type": "string", "description": "A hex number (CCRR) or \"col,row\"" },
        }), &[]), "annotations": { "readOnlyHint": true } }),
    ]
}

/// Run a hex tool (None: not one of these).
pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "get_hex_grid" => get(app).map(text),
        "set_hex_grid" => set(app, a).await.map(text),
        "hex_at" => at(app, a).await.map(text),
        _ => return None,
    })
}

fn grid_json(g: &HexGrid) -> Value {
    json!({ "size_mi": g.size_mi, "flat": g.flat, "color": g.color, "width": g.width, "opacity": g.opacity, "ox_mi": g.ox_mi, "oy_mi": g.oy_mi, "numbers": g.numbers })
}

/// The grid as set, each field checked and filled in (as `hexGrid()` in hex.ts does).
fn checked(g: Option<&HexGrid>) -> HexGrid {
    let d = HexGrid::default();
    let Some(g) = g else { return d };
    let num = |v: f64, d: f64, lo: f64, hi: f64| if v.is_finite() { v.clamp(lo, hi) } else { d };
    let hex6 = g.color.len() == 7 && g.color.starts_with('#') && g.color[1..].chars().all(|c| c.is_ascii_hexdigit());
    HexGrid {
        size_mi: num(g.size_mi, d.size_mi, SIZE_MI.0, SIZE_MI.1),
        flat: g.flat,
        color: if hex6 { g.color.to_lowercase() } else { d.color },
        width: num(g.width, d.width, WIDTH_PX.0, WIDTH_PX.1),
        opacity: num(g.opacity, d.opacity, 0.0, 1.0),
        ox_mi: num(g.ox_mi, 0.0, -1e4, 1e4),
        oy_mi: num(g.oy_mi, 0.0, -1e4, 1e4),
        numbers: g.numbers,
    }
}

fn get(app: &Shared) -> Result<Value, String> {
    let e = app.with_edits(|e| e.hexes.get(KEY).cloned()).ok_or("no world is open: open the map app first")?;
    Ok(json!({ "grid": grid_json(&checked(e.as_ref())), "set": e.is_some() }))
}

async fn set(app: &Shared, a: &Value) -> Result<Value, String> {
    let color = match &a["color"] {
        Value::Null => None,
        Value::String(s) if s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit()) => Some(s.to_lowercase()),
        v => return Err(crate::args::bad("color", "#rrggbb", v)),
    };
    let (size, width, opacity) = (a["size_mi"].as_f64(), a["width"].as_f64(), a["opacity"].as_f64());
    let (ox, oy) = (a["ox_mi"].as_f64(), a["oy_mi"].as_f64());
    let (flat, numbers, reset) = (a["flat"].as_bool(), a["numbers"].as_bool(), a["reset"].as_bool() == Some(true));
    let g = app
        .edit("agent", 0, move |e| {
            let mut g = if reset { HexGrid::default() } else { checked(e.hexes.get(KEY)) };
            if let Some(v) = size { g.size_mi = v }
            if let Some(v) = flat { g.flat = v }
            if let Some(v) = color { g.color = v }
            if let Some(v) = width { g.width = v }
            if let Some(v) = opacity { g.opacity = v }
            if let Some(v) = ox { g.ox_mi = v }
            if let Some(v) = oy { g.oy_mi = v }
            if let Some(v) = numbers { g.numbers = v }
            let out = json!({ "grid": grid_json(&g), "set": g != HexGrid::default() });
            if g == HexGrid::default() { e.hexes.remove(KEY); } else { e.hexes.insert(KEY.into(), g); }
            Ok(out)
        })
        .await?;
    Ok(g)
}

// ---- geometry (hex.ts) ------------------------------------------------------------------------------------------------------

struct Frame { flat: bool, s: f64, r: f64, x0: f64, y0: f64, dx: f64, dy: f64 }

fn frame(g: &HexGrid) -> Frame {
    let sqrt3 = 3f64.sqrt();
    let s = g.size_mi * FT_PER_MI;
    let r = s / sqrt3;
    let (ox, oy) = (g.ox_mi * FT_PER_MI, g.oy_mi * FT_PER_MI);
    if g.flat { Frame { flat: true, s, r, x0: r + ox, y0: s / 2.0 + oy, dx: 1.5 * r, dy: s } } else { Frame { flat: false, s, r, x0: s / 2.0 + ox, y0: r + oy, dx: s, dy: 1.5 * r } }
}

/// `i & 1` for any sign (as JS does on integers).
fn odd(i: i64) -> bool { i & 1 == 1 }

fn centre(f: &Frame, col: i64, row: i64) -> [f64; 2] {
    let (i, j) = (col - 1, row - 1);
    if f.flat { [f.x0 + i as f64 * f.dx, f.y0 + j as f64 * f.dy + if odd(i) { f.s / 2.0 } else { 0.0 }] } else { [f.x0 + i as f64 * f.dx + if odd(j) { f.s / 2.0 } else { 0.0 }, f.y0 + j as f64 * f.dy] }
}

fn to_axial(f: &Frame, i: i64, j: i64) -> (i64, i64) {
    if f.flat { (i, j - (i - (i & 1)) / 2) } else { (i - (j - (j & 1)) / 2, j) }
}

fn from_axial(f: &Frame, q: i64, r: i64) -> (i64, i64) {
    if f.flat { (q, r + (q - (q & 1)) / 2) } else { (q + (r - (r & 1)) / 2, r) }
}

/// JS `Math.round` (halves toward +infinity).
fn js_round(x: f64) -> f64 { (x + 0.5).floor() }

fn hex_at(f: &Frame, x: f64, y: f64) -> (i64, i64) {
    let sqrt3 = 3f64.sqrt();
    let (px, py) = (x - f.x0, y - f.y0);
    let fq = if f.flat { (2.0 / 3.0) * px / f.r } else { ((sqrt3 / 3.0) * px - py / 3.0) / f.r };
    let fr = if f.flat { (-px / 3.0 + (sqrt3 / 3.0) * py) / f.r } else { ((2.0 / 3.0) * py) / f.r };
    let fs = -fq - fr;
    let (mut q, mut r, s) = (js_round(fq), js_round(fr), js_round(fs));
    let (dq, dr, ds) = ((q - fq).abs(), (r - fr).abs(), (s - fs).abs());
    if dq > dr && dq > ds { q = -r - s } else if dr > ds { r = -q - s }
    let (i, j) = from_axial(f, q as i64, r as i64);
    (i + 1, j + 1)
}

fn corners(f: &Frame, col: i64, row: i64) -> Vec<[f64; 2]> {
    let [cx, cy] = centre(f, col, row);
    (0..6)
        .map(|k| {
            let a = (std::f64::consts::PI / 3.0) * k as f64 - if f.flat { 0.0 } else { std::f64::consts::PI / 6.0 };
            [cx + f.r * a.cos(), cy + f.r * a.sin()]
        })
        .collect()
}

fn ring(f: &Frame, col: i64, row: i64) -> Vec<(i64, i64)> {
    let (q, r) = to_axial(f, col - 1, row - 1);
    [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)]
        .iter()
        .map(|(dq, dr)| {
            let (i, j) = from_axial(f, q + dq, r + dr);
            (i + 1, j + 1)
        })
        .collect()
}

/// Digits per part of a label: 2, or 3 once the map has more than 99 columns or rows (`hexDigits`).
fn digits(f: &Frame, w: f64, h: f64) -> usize {
    let (ex, ey) = if f.flat { (f.r, f.s) } else { (f.s, f.r) };
    let span = [(0.0 - ex - f.x0) / f.dx, (w + ex - f.x0) / f.dx, (0.0 - ey - f.y0) / f.dy, (h + ey - f.y0) / f.dy];
    let m = [span[0].floor() + 1.0, span[1].ceil() + 1.0, span[2].floor() + 1.0, span[3].ceil() + 1.0];
    if m.iter().any(|v| v.abs() > 99.0) { 3 } else { 2 }
}

fn label(col: i64, row: i64, d: usize) -> String {
    let pad = |n: i64| format!("{}{:0>d$}", if n < 0 { "-" } else { "" }, n.abs());
    pad(col) + &pad(row)
}

/// A hex argument: "col,row" or a label of 2 or 3 digits a part.
fn parse_hex(s: &str) -> Result<(i64, i64), String> {
    let s = s.trim();
    if let Some((c, r)) = s.split_once(',') {
        if let (Ok(c), Ok(r)) = (c.trim().parse::<i64>(), r.trim().parse::<i64>()) {
            return Ok((c, r));
        }
    } else if s.chars().all(|c| c.is_ascii_digit()) && (s.len() == 4 || s.len() == 6) {
        let h = s.len() / 2;
        return Ok((s[..h].parse().unwrap_or(0), s[h..].parse().unwrap_or(0)));
    }
    Err(crate::args::bad("hex", "a number CCRR (0412) or \"col,row\"", &json!(s)))
}

async fn at(app: &Shared, a: &Value) -> Result<Value, String> {
    let g = checked(app.with_edits(|e| e.hexes.get(KEY).cloned()).ok_or("no world is open: open the map app first")?.as_ref());
    let [w, h] = crate::args::map_size(app).await?;
    let f = frame(&g);
    let (col, row) = match (&a["at"], &a["hex"]) {
        (Value::Null, Value::Null) => return Err("give at (a place or point) or hex (a number or \"col,row\")".into()),
        (Value::Null, Value::String(s)) => parse_hex(s)?,
        (Value::Null, v) => return Err(crate::args::bad("hex", "a string: a number CCRR (0412) or \"col,row\"", v)),
        (p, Value::Null) => {
            let [x, y] = crate::args::point(app, "at", p).await?;
            hex_at(&f, x, y)
        }
        _ => return Err("give at or hex, not both".into()),
    };
    let c = centre(&f, col, row);
    let rnd = |v: f64| (v * 100.0).round() / 100.0;
    let pt = |p: [f64; 2]| json!([rnd(p[0]), rnd(p[1])]);
    let d = digits(&f, w, h);
    Ok(json!({
        "label": label(col, row, d), "key": format!("{col},{row}"), "col": col, "row": row,
        "centre": pt(c), "corners": corners(&f, col, row).into_iter().map(pt).collect::<Vec<_>>(),
        "on_map": c[0] >= 0.0 && c[0] < w && c[1] >= 0.0 && c[1] < h,
        "around": ring(&f, col, row).into_iter().map(|(c, r)| json!({ "key": format!("{c},{r}"), "label": label(c, r, d) })).collect::<Vec<_>>(),
        "grid": grid_json(&g),
    }))
}
