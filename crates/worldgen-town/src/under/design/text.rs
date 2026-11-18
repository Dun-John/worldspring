//! The text form of a design (`to_text`/`from_text`) and `diff`: see the module doc above.
use super::*;

// ---------------------------------------------------------------------------------------
// The text form.

/// Room symbols, in room order: room `i` is `SYMBOLS[i]`.
pub const SYMBOLS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789#$%&+?@<>~^*!";

/// The design as text, levels from the top. `name(level, room)`: a room's name, if renamed.
pub fn to_text(d: &SiteDesign, name: &dyn Fn(usize, usize) -> Option<String>) -> Result<String, String> {
    to_text_with(d, name, None)
}

/// `to_text`, with the built interior to show the floor heights of levels that are not flat: after a level's `grid:`
/// a `heights:` legend (`1=2.5 ft; 2=5 ft; /=ramp`) and a `height grid:` of the same size (`.` 0 ft or rock). A
/// read-out: `from_text` ignores both (the heights come from rooms' `+5` and the raising items).
pub fn to_text_with(d: &SiteDesign, name: &dyn Fn(usize, usize) -> Option<String>, built: Option<&Interior>) -> Result<String, String> {
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let n = d.levels.len();
    let building = d.is_building();
    let st = crate::interior::design::stairs_of(d);
    let mut s = if building {
        // (One cellar, as generated, goes without saying.)
        let cellars = d.below_ground();
        let deep = if cellars == 1 { String::new() } else { format!(" · cellars {cellars}") };
        format!("building · {nx} x {ny} squares · {n} level{} · stairs {},{} {}x{}{deep}\n", if n == 1 { "" } else { "s" }, st[0], st[1], st[2], st[3])
    } else {
        format!(
        "site {}{} · {} x {} squares · {} level{} · entry {},{}\n",
        d.kind,
        d.theme.as_deref().map(|t| format!(" · theme {t}")).unwrap_or_default(),
        nx,
        ny,
        n,
        if n == 1 { "" } else { "s" },
        d.entry[0],
        d.entry[1]
    )
    };
    for (li, lv) in d.levels.iter().enumerate().rev() {
        let cells = decode(&lv.cells, nx * ny).ok_or_else(|| format!("{}: bad squares", depth_tag(n, li)))?;
        let used: Vec<bool> = (0..lv.rooms.len()).map(|r| cells.contains(&(r as i16))).collect();
        if lv.rooms.len() > SYMBOLS.len() && used.iter().skip(SYMBOLS.len()).any(|&u| u) {
            return Err(format!("{} has more than {} rooms: too many to show as text", depth_tag(n, li), SYMBOLS.len()));
        }
        s.push_str(&format!("\nlevel {}: {}\n", if building { lv.z as i64 } else { (n - li) as i64 }, lv.name));
        let legend: Vec<String> = lv
            .rooms
            .iter()
            .enumerate()
            .filter(|(r, _)| used[*r])
            .map(|(r, room)| {
                let raise = if room.raise_ft > 0 { format!("+{}", room.raise_ft) } else { String::new() };
                let shape = crate::interior::shape::name(room.shape).map(|s| format!(" ({s})")).unwrap_or_default();
                let named = name(li, r).map(|t| format!(" \"{t}\"")).unwrap_or_default();
                format!("{}={}{raise}{shape}{named}", SYMBOLS[r] as char, room.kind)
            })
            .collect();
        s.push_str(&format!("rooms: {}\ngrid:\n", legend.join("; ")));
        for j in 0..ny {
            let row: String = (0..nx)
                .map(|i| if cells[j * nx + i] < 0 { '.' } else { SYMBOLS[cells[j * nx + i] as usize] as char })
                .collect();
            s.push_str(&row);
            s.push('\n');
        }
        if let Some(lb) = built.filter(|b| b.levels.len() == n).map(|b| &b.levels[li]).filter(|l| !l.floor.is_empty() && l.floor.len() == nx * ny) {
            let f = crate::interior::height::of_level(lb, nx);
            let mut seen: Vec<f32> = f.ft.iter().zip(&lb.cells).filter(|&(&h, &c)| h > 0.0 && c >= 0).map(|(&h, _)| h).collect();
            seen.sort_by(|a, b| a.total_cmp(b));
            seen.dedup();
            let sym = |h: f32| b"123456789abcdefghijklmnopqrstuvwxyz".get(seen.iter().position(|&x| x == h).unwrap_or(0)).map(|&c| c as char).unwrap_or('+');
            let mut legend: Vec<String> = seen.iter().map(|&h| format!("{}={h} ft", sym(h))).collect();
            if f.ramp.contains(&true) {
                legend.push("/=ramp".into());
            }
            s.push_str(&format!("heights: {}\nheight grid:\n", legend.join("; ")));
            for j in 0..ny {
                let row: String = (0..nx).map(|i| { let k = j * nx + i; if lb.cells[k] < 0 || f.ft[k] == 0.0 { '.' } else if f.ramp[k] { '/' } else { sym(f.ft[k]) } }).collect();
                s.push_str(&row);
                s.push('\n');
            }
        }
        let doors: Vec<String> = lv.doors.iter().map(door_text).collect();
        if !lv.natural || !doors.is_empty() {
            s.push_str(&format!("doors: {}\n", doors.join("; ")));
        }
        if let Some(l) = lv.landing {
            s.push_str(&format!("landing: {},{}\n", l[0], l[1]));
        }
        let items: Vec<String> = lv.items.iter().map(item_text).collect();
        s.push_str(&format!("items: {}\n", items.join("; ")));
    }
    Ok(s)
}

/// A room name given in the text: (level index, room, name).
pub type TextName = (usize, usize, String);

/// A design from text over `base` (what the text leaves out stays as it is): levels by their
/// number from the top, each with any of `rooms:`, `grid:`, `doors:` (`auto`: doors where
/// needed) and `items:`. A header `site … · N levels` sets how many levels there are (new ones
/// start as solid rock). Missing ways are filled in where they can only go one place: the way
/// in at the entry, a way up under the way down from the level above.
pub fn from_text(text: &str, base: &SiteDesign) -> Result<(SiteDesign, Vec<TextName>), String> {
    let mut d = base.clone();
    let (nx, ny) = (d.nx as usize, d.ny as usize);
    let mut names: Vec<TextName> = Vec::new();
    // Blocks by level number, in order.
    let building = d.is_building();
    struct Block<'a> {
        depth: usize,
        /// A building's: the level's storey (its depth is known once its cellars are).
        z: i64,
        tag: String,
        name: Option<&'a str>,
        rooms: Option<&'a str>,
        grid: Option<Vec<&'a str>>,
        doors: Option<&'a str>,
        /// `landing: x,y` (or `landing: none`).
        landing: Option<&'a str>,
        /// `items:` ('='), `+items:` ('+'), `-items:` ('-'), in the order written.
        items: Vec<(char, &'a str)>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    let mut levels: Option<usize> = None;
    let mut cellars: Option<usize> = None;
    let mut in_grid = false;
    let mut in_heights = false;
    for (ln, raw) in text.lines().enumerate() {
        let line = raw.trim_end();
        let t = line.trim_start();
        let keyword = |k: &str| t.strip_prefix(k).map(str::trim);
        if t.is_empty() || t.starts_with("//") {
            in_grid = false;
            in_heights = false;
            continue;
        }
        if in_heights && !["rooms:", "grid:", "doors:", "items:", "+items:", "-items:", "heights:", "level ", "site ", "building"].iter().any(|k| t.starts_with(k)) {
            continue;
        }
        in_heights = false;
        if let Some(h) = keyword("site ").or_else(|| keyword("building")) {
            // "… · 3 levels · …"; a building's "… · stairs 3,4 2x1"
            for part in h.split('·') {
                let p = part.trim();
                if let Some(v) = p.strip_suffix(" levels").or_else(|| p.strip_suffix(" level")) {
                    levels = Some(v.trim().parse().map_err(|_| format!("line {}: '{p}': how many levels?", ln + 1))?);
                }
                if let Some(v) = p.strip_prefix("stairs ").filter(|_| building) {
                    let mut w = v.split_whitespace();
                    let (xy, size) = (w.next().unwrap_or(""), w.next().unwrap_or("1x1"));
                    let wrong = || format!("line {}: '{p}': write it as stairs x,y wxh", ln + 1);
                    let (x, y) = coords(xy).ok_or_else(wrong)?;
                    let (sw, sh) = size.split_once('x').and_then(|(a, b)| Some((a.parse::<u16>().ok()?, b.parse::<u16>().ok()?))).ok_or_else(wrong)?;
                    d.stairs = Some([x, y, sw, sh]);
                }
                if let Some(v) = p.strip_prefix("cellars ").filter(|_| building) {
                    cellars = Some(v.trim().parse().map_err(|_| format!("line {}: '{p}': how many levels below ground?", ln + 1))?);
                }
            }
            in_grid = false;
            continue;
        }
        if let Some(h) = keyword("level ") {
            let (num, rest) = h.split_once(':').map(|(a, b)| (a.trim(), Some(b.trim()))).unwrap_or((h, None));
            let mut z = 0;
            let depth = if building {
                // A building's levels by storey: -1 the cellar, 0 the ground floor.
                z = num.parse().map_err(|_| format!("line {}: 'level {num}': a building's levels are its storeys (-1 the cellar, 0 the ground floor)", ln + 1))?;
                0
            } else {
                let depth: usize = num.parse().map_err(|_| format!("line {}: 'level {num}': levels are numbered from 1 at the top", ln + 1))?;
                if depth == 0 || depth > MAX_LEVELS as usize {
                    let have = levels.unwrap_or(d.levels.len());
                    return Err(format!("line {}: 'level {num}': this site has {have} levels (1 at the top, {have} the deepest); a site holds at most {MAX_LEVELS}", ln + 1));
                }
                depth
            };
            blocks.push(Block {
                depth,
                z,
                tag: format!("level {num}"),
                name: rest.filter(|r| !r.is_empty()),
                rooms: None,
                grid: None,
                doors: None,
                landing: None,
                items: Vec::new(),
            });
            in_grid = false;
            continue;
        }
        let b = blocks.last_mut().ok_or_else(|| format!("line {}: start with 'level <n>' (1 is the top)", ln + 1))?;
        if let Some(v) = keyword("rooms:") {
            b.rooms = Some(v);
            in_grid = false;
        } else if let Some(v) = keyword("doors:") {
            b.doors = Some(v);
            in_grid = false;
        } else if let Some(v) = keyword("landing:") {
            b.landing = Some(v);
            in_grid = false;
        } else if let Some(v) = keyword("items:") {
            b.items.push(('=', v));
            in_grid = false;
        } else if let Some(v) = keyword("+items:") {
            let body = v.strip_suffix(';').unwrap_or(v);
            if body.trim().is_empty() || body.split(';').any(|e| e.trim().is_empty()) {
                return Err(format!("line {}: +items: names no pieces (or has an empty piece between ;): write it as +items: kind x,y [wxh]; kind x,y", ln + 1));
            }
            b.items.push(('+', v));
            in_grid = false;
        } else if let Some(v) = keyword("-items:") {
            let body = v.strip_suffix(';').unwrap_or(v);
            if body.trim().is_empty() || body.split(';').any(|e| e.trim().is_empty()) {
                return Err(format!("line {}: -items: names no pieces (or has an empty piece between ;): write it as -items: kind x,y [wxh]; kind x,y", ln + 1));
            }
            b.items.push(('-', v));
            in_grid = false;
        } else if keyword("heights:").is_some() {
            // (A read-out of the floor heights `to_text` shows where a level is not flat: ignored, they follow from the rooms' `+5` and the raising items.)
            in_grid = false;
        } else if keyword("height grid:").is_some() {
            in_grid = false;
            in_heights = true;
        } else if keyword("grid:").is_some() {
            b.grid = Some(Vec::new());
            in_grid = true;
        } else if in_grid {
            b.grid.as_mut().expect("grid open").push(t);
        } else {
            return Err(format!("line {}: expected rooms:, grid:, doors: or items:, not '{t}'", ln + 1));
        }
    }
    if building {
        // Cellars as asked (`cellars n`), else down to the deepest level given.
        let has = d.levels.iter().filter(|l| l.z < 0).count();
        let deepest = blocks.iter().map(|b| b.z).min().unwrap_or(0);
        let want = cellars.unwrap_or_else(|| has.max((-deepest).max(0) as usize));
        if want != has && !crate::interior::design::set_cellars(&mut d, want) {
            return Err(format!("a building has 0 to {} levels below ground", crate::interior::design::MAX_CELLARS));
        }
        for b in &mut blocks {
            let li = d.levels.iter().position(|l| l.z as i64 == b.z).ok_or_else(|| {
                let zs: Vec<String> = d.levels.iter().map(|l| l.z.to_string()).collect();
                format!("{}: the building has no such level (its levels: {})", b.tag, zs.join(", "))
            })?;
            b.depth = d.levels.len() - li;
        }
        if levels.is_some_and(|n| n != d.levels.len()) {
            return Err(format!("a building's levels are its cellars and storeys: it has {}", d.levels.len()));
        }
    }
    let count = levels.unwrap_or_else(|| d.levels.len().max(blocks.iter().map(|b| b.depth).max().unwrap_or(0)));
    let most = if building { BUILDING_LEVELS } else { MAX_LEVELS as usize };
    if count == 0 || count > most {
        return Err(format!("a site has 1 to {most} levels"));
    }
    if let Some(b) = blocks.iter().find(|b| b.depth > count) {
        return Err(format!("level {} given, but the site has {count} levels", b.depth));
    }
    // Levels added below or taken away from the bottom (index 0 is the deepest).
    let n0 = d.levels.len();
    if count < n0 {
        d.levels.drain(0..n0 - count);
    }
    while d.levels.len() < count {
        let deep = d.levels.first().cloned().unwrap_or_default();
        let depth = d.levels.len() + 1;
        d.levels.insert(
            0,
            DesignLevel {
                name: format!("Level {depth} · {} ft down", depth * LEVEL_FT as usize),
                elevation_ft: deep.elevation_ft - LEVEL_FT,
                natural: deep.natural,
                cells: vec![-1, (nx * ny) as i32],
                ..Default::default()
            },
        );
        if !blocks.iter().any(|b| b.depth == depth && b.grid.is_some()) {
            return Err(format!("level {depth} is new: give its grid"));
        }
    }
    // Ways taken off by `-items:` (level index, kind, the piece as written): checked before they are filled in again.
    let mut ways_off: Vec<(usize, String, String)> = Vec::new();
    for b in &blocks {
        let li = count - b.depth;
        let lv = &mut d.levels[li];
        let tag = b.tag.clone();
        if let Some(nm) = b.name {
            lv.name = nm.to_string();
        }
        if let Some(legend) = b.rooms {
            let mut rooms: Vec<Option<DesignRoom>> = Vec::new();
            for entry in legend.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let (sym, rest) = entry.split_once('=').ok_or_else(|| format!("{tag}: room '{entry}': write it as a=kind, a=kind+5, a=kind (round) or a=kind \"name\""))?;
                let sym = sym.trim();
                let r = (sym.len() == 1).then(|| SYMBOLS.iter().position(|&c| c == sym.as_bytes()[0])).flatten().ok_or_else(|| {
                    format!(
                        "{tag}: '{sym}' is not a room symbol (a-z, A-Z, 0-9, then {})",
                        std::str::from_utf8(&SYMBOLS[62..]).unwrap_or("")
                    )
                })?;
                let (body, named) = match rest.split_once('"') {
                    Some((k, n)) => (k.trim(), Some(n.trim_end_matches('"').trim().to_string())),
                    None => (rest.trim(), None),
                };
                // "kind+5 (round)": the shape last, in brackets.
                // (Brackets naming no shape stay part of the kind: "hall (east)"; a shape goes last.)
                let (body, shape) = match body.strip_suffix(')').and_then(|b| b.rsplit_once('(')).and_then(|(k, sh)| crate::interior::shape::parse(sh).map(|c| (k.trim(), c))) {
                    Some((_, crate::interior::shape::CAVERN)) if building => return Err(format!("{tag}: room '{entry}': a building's rooms can't be caverns")),
                    Some(found) => found,
                    None => (body, 0),
                };
                if let Some(sh) = body.split('(').skip(1).filter_map(|g| g.split_once(')')).map(|g| g.0).find(|g| crate::interior::shape::parse(g).is_some()) {
                    return Err(format!("{tag}: room '{entry}': the shape ({}) goes last, after the kind and any other brackets", sh.trim()));
                }
                let (kind, raise) = match body.rsplit_once('+') {
                    Some((k, v)) if v.trim().parse::<u16>().is_ok() => (k.trim(), v.trim().parse::<u16>().unwrap_or(0)),
                    _ => (body, 0),
                };
                let known = if building { crate::interior::room_kind(kind) } else { room_kind(kind) };
                if rooms.len() <= r {
                    rooms.resize(r + 1, None);
                }
                rooms[r] = Some(DesignRoom {
                    kind: known.unwrap_or(CHAMBER).into(),
                    raise_ft: raise.min(30),
                    shape,
                });
                // A kind of room the generators don't know becomes a chamber by that name.
                if let Some(nm) = named.filter(|n| !n.is_empty()).or_else(|| known.is_none().then(|| kind.to_string())) {
                    names.push((li, r, nm));
                }
            }
            let old = std::mem::take(&mut lv.rooms);
            lv.rooms = rooms
                .into_iter()
                .enumerate()
                .map(|(r, x)| {
                    x.or_else(|| old.get(r).cloned()).unwrap_or(DesignRoom {
                        kind: CHAMBER.into(),
                        ..Default::default()
                    })
                })
                .collect();
        }
        if let Some(rows) = &b.grid {
            if rows.len() > ny {
                return Err(format!("{tag}: {} rows; the grid has {ny}", rows.len()));
            }
            let mut cells = vec![-1i16; nx * ny];
            for (j, row) in rows.iter().enumerate() {
                if row.chars().count() > nx {
                    return Err(format!("{tag}: row {j} has {} squares; the grid has {nx}", row.chars().count()));
                }
                for (i, c) in row.chars().enumerate() {
                    if c == '.' || c == ' ' {
                        continue;
                    }
                    let r = SYMBOLS
                        .iter()
                        .position(|&s| s as char == c)
                        .filter(|&r| r < lv.rooms.len())
                        .ok_or_else(|| format!("{tag}: '{c}' at {i},{j} is not in its rooms list"))?;
                    cells[j * nx + i] = r as i16;
                }
            }
            lv.cells = encode(&cells);
        }
        if let Some(v) = b.landing {
            lv.landing = match v.trim() {
                "none" | "" => None,
                xy => {
                    let (x, y) = coords(xy).ok_or_else(|| format!("{tag}: landing '{v}': write it as x,y (or none)"))?;
                    if x as usize >= nx || y as usize >= ny {
                        return Err(format!("{tag}: landing {x},{y} is off the grid"));
                    }
                    Some([x, y])
                }
            };
        }
        if let Some(list) = b.doors {
            // (`auto` keeps a building's doors to the outside.)
            let auto = list.trim() == "auto";
            lv.doors.retain(|d| building && auto && d[3] & 6 != 0);
            if !auto {
                let how = if building { "x,y e|s|w|n [secret|portcullis|front|back] [locked 1-31]" } else { "x,y e|s [secret|portcullis] [locked 1-31]" };
                for e in list.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                    let mut w = e.split_whitespace();
                    let (xy, side) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
                    let mut flag = w.next();
                    // (Underground, `locked N`: the lock's colour, after `secret` if both.)
                    let mut lock = 0u16;
                    {
                        let rest: Vec<&str> = std::iter::once(flag).flatten().chain(w.by_ref()).collect();
                        flag = None;
                        let mut i = 0;
                        while i < rest.len() {
                            match rest[i] {
                                "locked" => {
                                    lock = rest.get(i + 1).and_then(|c| c.parse::<u16>().ok()).filter(|c| (1..=MAX_LOCK as u16).contains(c)).ok_or_else(|| format!("{tag}: door '{e}': locked 1-{MAX_LOCK} (the lock's colour)"))?;
                                    i += 1;
                                }
                                f if flag.is_none() && lock == 0 => flag = Some(f),
                                f => return Err(format!("{tag}: door '{e}': '{f}'? write it as {how}")),
                            }
                            i += 1;
                        }
                    }
                    let (x, y) = coords(xy).ok_or_else(|| format!("{tag}: door '{e}': write it as {how}"))?;
                    let side = match side {
                        "e" => 0,
                        "s" => 1,
                        "w" if building => 2,
                        "n" if building => 3,
                        _ if building => return Err(format!("{tag}: door '{e}': the side is e, s, w or n (the square's edge east, south, west or north)")),
                        _ => return Err(format!("{tag}: door '{e}': the side is e (to the square east) or s (south)")),
                    };
                    let on_grid = if building { crate::interior::design::door_at(x, y, side, nx, ny).is_some() } else { door_squares(x, y, side, nx, ny).is_some() };
                    if !on_grid {
                        return Err(format!("{tag}: door '{e}' is off the grid"));
                    }
                    let flags = match flag {
                        None => 0,
                        Some("secret") => 1,
                        Some("portcullis") => PORTCULLIS,
                        Some("front") if building => 2,
                        Some("back") if building => 4,
                        Some("arch") => 8,
                        Some(f) => return Err(format!("{tag}: door '{e}': '{f}'? write it as {how}")),
                    };
                    lv.doors.push([x, y, side, flags | lock_flags(lock as u8)]);
                }
            }
        }
        for &(op, list) in &b.items {
            if op == '=' {
                lv.items.clear();
            }
            for e in list.split(';').map(str::trim).filter(|e| !e.is_empty()) {
                let (item, sized) = parse_item(e, &tag, building)?;
                match op {
                    '-' => {
                        // The first piece of that kind at that top-left square (and of that size, if given).
                        let found = lv.items.iter().position(|f| f.kind == item.kind && f.x == item.x && f.y == item.y && (!sized || (f.w, f.h) == (item.w, item.h)));
                        let Some(at) = found else {
                            let mut near: Vec<&DesignItem> = lv.items.iter().filter(|f| f.kind == item.kind).collect();
                            near.sort_by_key(|f| f.x.abs_diff(item.x) + f.y.abs_diff(item.y));
                            let hint = if near.is_empty() { format!("no {} on this level", item.kind) } else { format!("nearest {}: {}", item.kind, near.iter().take(3).map(|f| item_text(f)).collect::<Vec<_>>().join("; ")) };
                            return Err(format!("{tag}: -items: '{e}' matches nothing ({hint})"));
                        };
                        let gone = lv.items.remove(at);
                        if !building && WAYS.contains(&gone.kind.as_str()) {
                            ways_off.push((li, gone.kind, format!("{tag}: -items: '{e}'")));
                        }
                    }
                    _ => lv.items.push(item),
                }
            }
        }
    }
    if !building {
        for (li, kind, what) in &ways_off {
            if d.levels[*li].items.iter().any(|f| f.kind == *kind) {
                continue;
            }
            let ups_above = d.levels.get(li + 1).map_or(0, |l| l.items.iter().filter(|f| f.kind == "down").count());
            let why = match kind.as_str() {
                "exit" if *li + 1 == d.levels.len() => "the way in stays at the entry and can't be removed (it would be put back)",
                "up" if ups_above == 1 => "a way up always sits under the way down of the level above and can't be removed; remove or move that down instead",
                _ => continue,
            };
            return Err(format!("{what}: {why}"));
        }
        d.settle_ways();
    }
    for b in blocks.iter().filter(|b| b.doors.is_some_and(|l| l.trim() == "auto")) {
        d.add_doors(Some(count - b.depth));
    }
    Ok((d, names))
}

/// A door as the text writes it.
pub fn door_text(d: &[u16; 4]) -> String {
    let flag = match d[3] & 15 {
        0 => "",
        f if f & 1 != 0 => " secret",
        f if f & 2 != 0 => " front",
        f if f & 8 != 0 => " arch",
        _ => " back",
    };
    let flag = if d[3] & PORTCULLIS != 0 && d[3] & 15 == 0 { " portcullis" } else { flag };
    let lock = match lock_of(d[3]) {
        0 => String::new(),
        c => format!(" locked {c}"),
    };
    format!("{},{} {}{flag}{lock}", d[0], d[1], ["e", "s", "w", "n"][d[2].min(3) as usize])
}

/// What changed between two designs of one site, by level (numbered as the text numbers them), as the agent tools
/// report it: items added and removed, squares given to other rooms (count, the rooms' letters) and rooms whose
/// kind, raise or shape changed, doors added and removed, a building's stair block, floor heights changed (from
/// the built interiors `old_it`, `new_it`). Empty when nothing changed.
pub fn diff(old: &SiteDesign, old_it: &Interior, new: &SiteDesign, new_it: &Interior) -> Value {
    let building = new.is_building();
    let (no, nn) = (old.levels.len(), new.levels.len());
    let key = |d: &SiteDesign, li: usize| if building { d.levels[li].z as i64 } else { (d.levels.len() - li) as i64 };
    let (nx, ny) = (new.nx as usize, new.ny as usize);
    let mut out = serde_json::Map::new();
    if no != nn {
        out.insert("levels".into(), json!(format!("{no} -> {nn}")));
    }
    if old.stairs != new.stairs && let Some(st) = new.stairs {
        out.insert("stairs".into(), json!(format!("{},{} {}x{}", st[0], st[1], st[2], st[3])));
    }
    let letter = |r: i32| if r < 0 { '.' } else { SYMBOLS.get(r as usize).map(|&c| c as char).unwrap_or('?') };
    let room_text = |r: &DesignRoom| format!("{}{}{}", r.kind, if r.raise_ft > 0 { format!("+{}", r.raise_ft) } else { String::new() }, crate::interior::shape::name(r.shape).map(|s| format!(" ({s})")).unwrap_or_default());
    for li in (0..nn).rev() {
        let k = key(new, li);
        let nl = &new.levels[li];
        let Some(oi) = (0..no).find(|&oi| key(old, oi) == k) else {
            out.insert(format!("level {k}"), json!("new level"));
            continue;
        };
        let ol = &old.levels[oi];
        let mut c = serde_json::Map::new();
        if ol.name != nl.name {
            c.insert("renamed".into(), json!(nl.name));
        }
        // Items (as a multiset).
        let mut gone: Vec<&DesignItem> = ol.items.iter().collect();
        let mut added: Vec<String> = Vec::new();
        for f in &nl.items {
            match gone.iter().position(|g| *g == f) {
                Some(p) => {
                    gone.remove(p);
                }
                None => added.push(item_text(f)),
            }
        }
        if !added.is_empty() {
            c.insert("items_added".into(), json!(added));
        }
        if !gone.is_empty() {
            c.insert("items_removed".into(), json!(gone.iter().map(|f| item_text(f)).collect::<Vec<_>>()));
        }
        // Squares and rooms.
        if let (Some(a), Some(b)) = (decode(&ol.cells, nx * ny), decode(&nl.cells, nx * ny)) {
            let mut letters: Vec<char> = Vec::new();
            let mut n = 0;
            for q in (0..nx * ny).filter(|&q| a[q] != b[q]) {
                n += 1;
                for ch in [letter(a[q] as i32), letter(b[q] as i32)] {
                    if ch != '.' && !letters.contains(&ch) {
                        letters.push(ch);
                    }
                }
            }
            if n > 0 {
                letters.sort();
                c.insert("squares_changed".into(), json!({ "count": n, "rooms": letters.iter().collect::<String>() }));
            }
        }
        let rooms: Vec<String> = (0..nl.rooms.len()).filter(|&r| ol.rooms.get(r) != Some(&nl.rooms[r])).filter(|&r| decode(&nl.cells, nx * ny).is_some_and(|b| b.contains(&(r as i16)))).map(|r| format!("{}={}", letter(r as i32), room_text(&nl.rooms[r]))).collect();
        if !rooms.is_empty() {
            c.insert("rooms_changed".into(), json!(rooms));
        }
        // Doors.
        let mut gone: Vec<&[u16; 4]> = ol.doors.iter().collect();
        let mut added: Vec<String> = Vec::new();
        for d in &nl.doors {
            match gone.iter().position(|g| *g == d) {
                Some(p) => {
                    gone.remove(p);
                }
                None => added.push(door_text(d)),
            }
        }
        if !added.is_empty() {
            c.insert("doors_added".into(), json!(added));
        }
        if !gone.is_empty() {
            c.insert("doors_removed".into(), json!(gone.iter().map(|d| door_text(d)).collect::<Vec<_>>()));
        }
        if ol.landing != nl.landing {
            c.insert("landing".into(), json!(nl.landing.map_or("none".to_string(), |l| format!("{},{}", l[0], l[1]))));
        }
        // Floor heights, as built.
        if let (Some(a), Some(b)) = (old_it.levels.get(oi), new_it.levels.get(li)) {
            let (fa, fb) = (crate::interior::height::of_level(a, nx), crate::interior::height::of_level(b, nx));
            if fa.ft.len() == fb.ft.len() {
                let moved: Vec<usize> = (0..fa.ft.len()).filter(|&q| fa.ft[q] != fb.ft[q]).collect();
                if !moved.is_empty() {
                    let eg: Vec<String> = moved.iter().take(4).map(|&q| format!("{},{}: {} -> {} ft", q % nx, q / nx, fa.ft[q], fb.ft[q])).collect();
                    c.insert("heights_changed".into(), json!({ "count": moved.len(), "first": eg }));
                }
            }
        }
        if !c.is_empty() {
            out.insert(format!("level {k}"), Value::Object(c));
        }
    }
    for oi in 0..no {
        let k = key(old, oi);
        if !(0..nn).any(|li| key(new, li) == k) {
            out.insert(format!("level {k}"), json!("taken away"));
        }
    }
    Value::Object(out)
}

/// One entry of an `items:` list: `kind x,y [wxh] [back n|e|s|w] [variant n]`, and whether a size was written.
fn parse_item(e: &str, tag: &str, building: bool) -> Result<(DesignItem, bool), String> {
    let words: Vec<&str> = e.split_whitespace().collect();
    let how = || format!("{tag}: item '{e}': write it as kind x,y [wxh] [back n|e|s|w] [variant 0-3, more for some]");
    // The kind is the words before the last x,y; size, back and variant come after it.
    let Some(at) = words.iter().rposition(|w| coords(w).is_some()).filter(|&at| at > 0) else { return Err(how()) };
    let (x, y) = coords(words[at]).ok_or_else(how)?;
    let (mut size, mut back, mut variant) = (None, None, None);
    let mut rest = words[at + 1..].iter();
    while let Some(&w) = rest.next() {
        match w {
            "back" => back = Some(rest.next().and_then(|v| SIDES.iter().position(|s| s == v)).ok_or_else(how)? as u8),
            "variant" => variant = Some(rest.next().and_then(|v| v.parse::<u8>().ok()).ok_or_else(how)?),
            _ => size = Some(w.split_once('x').and_then(|(w, h)| Some((w.parse::<u16>().ok()?, h.parse::<u16>().ok()?))).ok_or_else(how)?),
        }
    }
    let name = words[..at].join(" ");
    // (An uploaded picture: s:<asset id>.)
    let kind = match sprite_asset(&name) {
        Some(_) => name.as_str(),
        None => if building { furniture_kind(&name) } else { item_kind(&name) }.ok_or_else(|| format!("{tag}: no such item: {name}"))?,
    };
    if let Some(v) = variant.filter(|&v| v >= crate::interior::item_looks(kind)) {
        return Err(format!("{tag}: item '{e}': {name} has looks 0-{} (variant {v})", crate::interior::item_looks(kind) - 1));
    }
    let (w, h) = if !building && (WAYS.contains(&kind) || kind == "lava") { (1, 1) } else { size.unwrap_or((1, 1)) };
    Ok((DesignItem { kind: kind.into(), x, y, w: w.max(1), h: h.max(1), back, variant }, size.is_some()))
}

/// An item as the text writes it.
pub fn item_text(f: &DesignItem) -> String {
    let size = if f.w > 1 || f.h > 1 { format!(" {}x{}", f.w, f.h) } else { String::new() };
    let back = f.back.filter(|&b| b < 4).map(|b| format!(" back {}", SIDES[b as usize])).unwrap_or_default();
    let variant = f.variant.map(|v| format!(" variant {}", v.min(crate::interior::item_looks(&f.kind) - 1))).unwrap_or_default();
    format!("{} {},{}{size}{back}{variant}", f.kind, f.x, f.y)
}

/// An item's `back` in the text form, by side (0 north 1 east 2 south 3 west).
const SIDES: [&str; 4] = ["n", "e", "s", "w"];

fn coords(s: &str) -> Option<(u16, u16)> {
    let (x, y) = s.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

