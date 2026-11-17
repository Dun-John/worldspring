//! The checks of things made by hand (`Created`) against the generators' own lists: kinds,
//! themes and sizes underground, cultures, wards, building functions, roofs.

use crate::world::*;

impl Created {
    /// Whether its options make sense: a kind that can be created, what lies beneath only for
    /// ruins (built sites) and entrances, size, levels and a theme of its site's kind only
    /// where there is a site underground.
    pub fn check(&self) -> Result<(), String> {
        use crate::under::{MAX_LEVELS, SiteSize, THEMES, UnderKind};
        if CREATED_SETTLEMENTS.contains(&self.kind.as_str()) {
            return self.check_settlement();
        }
        if !CREATABLE.contains(&self.kind.as_str()) {
            return Err(format!("kind must be one of {}", CREATABLE.join(", ")));
        }
        if self.culture.is_some() || self.lives_by.is_some() {
            return Err("culture and lives_by are for villages and towns".into());
        }
                let works = self.gate.is_some() || self.keep.is_some() || self.keep_shape.is_some() || !self.keep_poly.is_empty() || self.moat.is_some() || self.yard_buildings.is_some() || !self.pts.is_empty() || !self.gates.is_empty() || self.closed || !self.pieces.is_empty();
        match self.kind.as_str() {
            "plot" => return self.check_plot(),
            "building" if works => return Err("gate, keep, keep_shape, keep_poly, moat, yard_buildings, pts, gates and closed are for castles and walls".into()),
            "building" => return self.check_building(),
            "castle" => return self.check_castle(),
            "wall" => return self.check_wall(),
            _ => {}
        }
        if !self.poly.is_empty() || self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.material.is_some() || self.shape.is_some() || self.shade.is_some() || self.structure.is_some() || works {
            return Err("footprint, floors, function, roof, tint and structure are for buildings; gates, keep and lines for castles and walls".into());
        }
        if let Some(u) = &self.under {
            let built = [UnderKind::Dungeon, UnderKind::Crypt, UnderKind::Catacombs];
            match (self.kind.as_str(), UnderKind::parse(u)) {
                ("ruin", Some(k)) if built.contains(&k) => {}
                ("entrance", Some(_)) => {}
                ("ruin", _) => return Err("under a ruin: dungeon, crypt or catacombs".into()),
                ("entrance", _) => return Err(format!("under an entrance: {}", UnderKind::CREATABLE.map(|k| k.key()).join(", "))),
                _ => return Err("'under' is for ruins and entrances".into()),
            }
        }
        if self.in_building.is_some() || self.way.is_some() {
            if self.kind != "entrance" {
                return Err("'in_building' and 'way' are for entrances".into());
            }
            if self.in_building.is_none() {
                return Err("'way' needs 'in_building'".into());
            }
            if self.way.as_deref().is_some_and(|w| w != "stairs" && w != "trapdoor") {
                return Err("way: stairs or trapdoor".into());
            }
        }
        let site = self.under_kind();
        if site.is_none() && (self.size.is_some() || self.levels.is_some() || self.theme.is_some()) {
            return Err(format!("a {} has nothing underground: no size, levels or theme", self.kind));
        }
        if let Some(s) = &self.size
            && SiteSize::parse(s).is_none()
        {
            return Err(format!("size must be one of {}", SiteSize::NAMES.join(", ")));
        }
        if self.levels.is_some_and(|l| l == 0 || l > MAX_LEVELS) {
            return Err(format!("levels: 1 to {MAX_LEVELS}"));
        }
        if let (Some(t), Some(k)) = (&self.theme, site) {
            let own: Vec<&str> = THEMES.iter().filter(|th| th.kind == k).map(|th| th.key).collect();
            if !own.contains(&t.as_str()) {
                return Err(format!("themes for a {}: {}", k.name(), own.join(", ")));
            }
        }
        Ok(())
    }

    /// A settlement's options: a culture and what it lives by (not by the shore), nothing else.
    fn check_settlement(&self) -> Result<(), String> {
        let shaped = !self.poly.is_empty() || !self.pts.is_empty() || !self.gates.is_empty() || self.closed;
        let built = self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.material.is_some() || self.shape.is_some() || self.shade.is_some() || self.structure.is_some();
        let works = self.gate.is_some() || self.keep.is_some() || self.yard_buildings.is_some();
        if shaped || built || works || self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() {
            return Err(format!("a {} takes only a name, culture and lives_by", self.kind));
        }
        if let Some(k) = &self.lives_by
            && !PIN_KINDS.iter().any(|p| p == k && !SHORE_KINDS.contains(p))
        {
            let ok: Vec<&str> = PIN_KINDS.iter().copied().filter(|p| !SHORE_KINDS.contains(p)).collect();
            return Err(format!("lives_by must be one of {}", ok.join(", ")));
        }
        if self.culture.is_some_and(|c| c as usize >= crate::t0::names::CULTURES.len()) {
            return Err(format!("culture: 0 to {}", crate::t0::names::CULTURES.len() - 1));
        }
        Ok(())
    }

    /// A building's options: a simple footprint of 3–64 corners near its point, at least
    /// 10 ft across; storeys, function, roof, tint and structure from their lists.
    fn check_plot(&self) -> Result<(), String> {
        let works = self.gate.is_some() || self.keep.is_some() || self.keep_shape.is_some() || !self.keep_poly.is_empty() || self.moat.is_some() || self.yard_buildings.is_some() || !self.pts.is_empty() || !self.gates.is_empty() || self.closed || !self.pieces.is_empty();
        if works || self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() || self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.material.is_some() || self.shape.is_some() || self.shade.is_some() || self.structure.is_some() {
            return Err("a plot has an outline and a fill only".into());
        }
        let p = &self.poly;
        if p.len() < 3 || p.len() > 64 || p.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            return Err("a plot's outline needs 3–64 corners".into());
        }
        if p.iter().any(|q| (q[0] - self.x).hypot(q[1] - self.y) > PLOT_REACH_FT) {
            return Err(format!("a plot's corners must lie within {PLOT_REACH_FT} ft of its point"));
        }
        if crate::core::geom::area(p).abs() < 400.0 || crate::core::geom::obb(p).short < 20.0 {
            return Err("a plot must be at least 20 ft across".into());
        }
        if !simple(p) {
            return Err("a plot's outline must not cross itself".into());
        }
        match &self.fill {
            Some(crate::town::hand::BlockFill::Castle { .. }) => Err("a plot's fill: generated, hand or open (a castle is drawn as one)".into()),
            Some(crate::town::hand::BlockFill::Generated { ward, lots, height, .. })
                if crate::town::hand::ward_of(ward).is_none() || lots.as_deref().is_some_and(|l| !LOT_SIZES.contains(&l)) || height.as_deref().is_some_and(|h| !crate::town::hand::HEIGHTS.contains(&h)) =>
            {
                Err(format!("a plot's ward: one of {}; lots: one of {}; height: one of {}", TOWN_WARDS.join(", "), LOT_SIZES.join(", "), crate::town::hand::HEIGHTS.join(", ")))
            }
            _ => Ok(()),
        }
    }

    fn check_building(&self) -> Result<(), String> {
        if self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() {
            return Err("a building has nothing underground: no under, size, levels or theme".into());
        }
        let p = &self.poly;
        if p.len() < 3 || p.len() > 64 || p.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            return Err("a building's footprint needs 3–64 corners".into());
        }
        if p.iter().any(|q| (q[0] - self.x).hypot(q[1] - self.y) > BUILDING_REACH_FT) {
            return Err(format!("a building's corners must lie within {BUILDING_REACH_FT} ft of its point"));
        }
        if crate::core::geom::area(p).abs() < 100.0 || crate::core::geom::obb(p).short < 10.0 {
            return Err("a building must be at least 10 ft across".into());
        }
        if !simple(p) {
            return Err("a building's footprint must not cross itself".into());
        }
        if self.floors.is_some_and(|f| f == 0 || f > MAX_FLOORS) {
            return Err(format!("floors: 1 to {MAX_FLOORS}"));
        }
        if let Some(f) = &self.func
            && crate::town::catalog::index_of(f).is_none()
            && !BUILDING_HOMES.contains(&f.as_str())
        {
            return Err(format!("func: a business (list_names shows them; e.g. inn, tavern, blacksmith, temple, castle) or a home: {}", BUILDING_HOMES.join(", ")));
        }
        for (v, list, what) in [(&self.roof, &ROOFS[..], "roof"), (&self.tint, &TINTS[..], "tint"), (&self.material, &crate::town::roof::MATERIALS[..], "material"), (&self.shape, &crate::town::roof::SHAPES[..], "shape"), (&self.structure, &STRUCTURES[..], "structure")] {
            if let Some(v) = v
                && !list.contains(&v.as_str())
            {
                return Err(format!("{what} must be one of {}", list.join(", ")));
            }
        }
        if self.shade.is_some_and(|s| s > 4) {
            return Err("shade: 0 to 4 (2 the usual colour of its material)".into());
        }
        Ok(())
    }
}

impl Created {
    /// A castle's options: an outline of 3–32 corners round its point that doesn't cross itself
    /// (concave or not), `CASTLE_MIN_FT` to `CASTLE_MAX_FT` across, its gates on its sides, its
    /// keep's shape (a drawn one inside the outline), its moat; ruined or not.
    fn check_castle(&self) -> Result<(), String> {
        use crate::core::geom::obb;
        if self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() || self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.material.is_some() || self.shape.is_some() || self.shade.is_some() {
            return Err("a castle takes its outline (poly), gate or gates, keep, keep_shape, keep_poly, moat, yard_buildings and structure".into());
        }
        if !self.pts.is_empty() || self.closed {
            return Err("pts and closed are for walls: a castle has an outline (poly)".into());
        }
        let p = &self.poly;
        if p.len() < 3 || p.len() > 32 || p.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            return Err("a castle's outline needs 3–32 corners".into());
        }
        if !simple(p) {
            return Err("a castle's outline must not cross itself".into());
        }
        let m = p.len();
        if (0..m).any(|i| (p[(i + 1) % m][0] - p[i][0]).hypot(p[(i + 1) % m][1] - p[i][1]) < 10.0) {
            return Err("a castle's corners must be at least 10 ft apart".into());
        }
        // (Across: the long side of the box that fits it closest, as its width reads.)
        let across = obb(p).long;
        if across > CASTLE_MAX_FT {
            return Err(format!("a castle is at most {CASTLE_MAX_FT} ft across (this is {across:.0})"));
        }
        if obb(p).short < CASTLE_MIN_FT {
            return Err(format!("a castle is at least {CASTLE_MIN_FT} ft across: room for its walls, a yard and a keep"));
        }
        if p.iter().any(|q| (q[0] - self.x).hypot(q[1] - self.y) > CASTLE_MAX_FT) {
            return Err("a castle's corners must lie round its point".into());
        }
        // (Its curtain, drawn in from the outline, in one piece: a neck too narrow for it would
        // leave a lobe without walls.)
        if !crate::core::clip::is_convex(p) && crate::core::clip::inset_edges(p, &vec![CASTLE_NECK_FT; m]).iter().filter(|s| s.area().abs() >= 1.0).count() > 1 {
            return Err(format!("a castle's outline is too narrow at a neck for its walls: make it {:.0} ft across there at least", 2.0 * CASTLE_NECK_FT));
        }
        if self.gate.is_some_and(|g| g as usize >= m) {
            return Err(format!("gate: a side of the outline, 0 to {}", m - 1));
        }
        if self.gates.iter().any(|g| *g as usize >= m) {
            return Err(format!("gates: sides of the outline, 0 to {}", m - 1));
        }
        if self.castle_gates().len() > CASTLE_MAX_GATES {
            return Err(format!("a castle has at most {CASTLE_MAX_GATES} gates"));
        }
        if let Some(s) = &self.moat
            && !MOATS.contains(&s.as_str())
        {
            return Err(format!("moat must be one of {}", MOATS.join(", ")));
        }
        if let Some(s) = &self.keep_shape
            && !KEEP_SHAPES.contains(&s.as_str())
        {
            return Err(format!("keep_shape must be one of {}", KEEP_SHAPES.join(", ")));
        }
        if self.keep == Some(false) && (self.keep_shape.is_some() || !self.keep_poly.is_empty()) {
            return Err("keep_shape and keep_poly are for a castle with a keep".into());
        }
        let k = &self.keep_poly;
        match (self.keep_shape.as_deref(), k.is_empty()) {
            (Some("drawn") | None, false) => {
                if k.len() < 3 || k.len() > 32 || k.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) || !simple(k) {
                    return Err("keep_poly: 3–32 corners, not crossing itself".into());
                }
                if obb(k).short < 20.0 || crate::core::geom::area(k).abs() < 600.0 {
                    return Err("a drawn keep is at least 20 ft across".into());
                }
                // (All of it, not only its corners: an edge may cross a notch.)
                let room = crate::core::clip::offset(&crate::core::clip::region(p), -12.0, crate::core::clip::Join::Miter(2.0));
                let out = crate::core::clip::area(&crate::core::clip::difference(&crate::core::clip::region(k), &room));
                if k.iter().any(|q| inside_by(p, *q) < 12.0) || out > 1.0 {
                    return Err("a drawn keep stands inside the castle's outline, 12 ft or more in from its walls".into());
                }
            }
            (Some("drawn"), true) => return Err("keep_shape drawn: give its outline (keep_poly)".into()),
            (Some(_), false) => return Err("keep_poly is a drawn keep's (keep_shape drawn)".into()),
            _ => {}
        }
        if let Some(s) = &self.structure
            && !STRUCTURES.contains(&s.as_str())
        {
            return Err(format!("structure must be one of {}", STRUCTURES.join(", ")));
        }
        for (k, piece) in &self.pieces {
            if k != CastlePiece::KEEP && k.parse::<u32>().is_err() {
                return Err(format!("pieces: {k} is neither keep nor a building's number"));
            }
            if !piece.edit.poly.is_empty() {
                return Err("a castle's pieces keep their footprints: redraw the castle instead".into());
            }
            piece.edit.check()?;
        }
        Ok(())
    }

    /// A wall's options: a line of 2–64 corners (a ring of 3 or more if closed), each side at
    /// least 10 ft, at most `WALL_MAX_FT` in all, its gates on its corners.
    fn check_wall(&self) -> Result<(), String> {
        if self.under.is_some() || self.size.is_some() || self.levels.is_some() || self.theme.is_some() || self.floors.is_some() || self.func.is_some() || self.roof.is_some() || self.tint.is_some() || self.material.is_some() || self.shape.is_some() || self.shade.is_some() {
            return Err("a wall takes its line (pts), gates and closed".into());
        }
        if !self.poly.is_empty() || self.gate.is_some() || self.keep.is_some() || self.keep_shape.is_some() || !self.keep_poly.is_empty() || self.moat.is_some() || self.yard_buildings.is_some() || self.structure.is_some() || !self.pieces.is_empty() {
            return Err("poly, gate, keep, yard_buildings and structure are for castles: a wall has a line (pts) and gates".into());
        }
        let p = &self.pts;
        let (most, longest) = if self.closed { (WALL_RING_MAX_CORNERS, WALL_RING_MAX_FT) } else { (WALL_MAX_CORNERS, WALL_MAX_FT) };
        if p.len() < 2 || p.len() > most || p.iter().any(|q| !q[0].is_finite() || !q[1].is_finite()) {
            return Err(if self.closed { format!("a closed wall's line needs 3–{most} corners") } else { format!("a wall's line needs 2–{most} corners (a closed one up to {WALL_RING_MAX_CORNERS})") });
        }
        if self.closed && p.len() < 3 {
            return Err("a closed wall needs at least 3 corners".into());
        }
        let n = p.len();
        let sides = if self.closed { n } else { n - 1 };
        let side = |i: usize| (p[(i + 1) % n][0] - p[i][0]).hypot(p[(i + 1) % n][1] - p[i][1]);
        if (0..sides).any(|i| side(i) < 10.0) {
            return Err("a wall's corners must be at least 10 ft apart".into());
        }
        let length: f64 = (0..sides).map(side).sum();
        if length > longest {
            return Err(if self.closed { format!("a closed wall is at most {longest} ft round (this is {length:.0})") } else { format!("a wall is at most {longest} ft long (this is {length:.0}; a closed one up to {WALL_RING_MAX_FT})") });
        }
        if self.closed && !simple(p) {
            return Err("a closed wall must not cross itself".into());
        }
        if p.iter().any(|q| (q[0] - self.x).hypot(q[1] - self.y) > longest.min(0.5 * WALL_RING_MAX_FT)) {
            return Err("a wall's corners must lie round its point".into());
        }
        if self.gates.iter().any(|g| *g as usize >= n) {
            return Err(format!("gates: corners of the line, 0 to {}", n - 1));
        }
        Ok(())
    }
}

/// How far `q` lies inside polygon `p` (ft; negative outside).
fn inside_by(p: &[[f64; 2]], q: [f64; 2]) -> f64 {
    let n = p.len();
    let d = (0..n).map(|i| crate::core::geom::seg_dist(q, p[i], p[(i + 1) % n])).fold(f64::MAX, f64::min);
    if crate::core::geom::contains(p, q) { d } else { -d }
}

/// Whether a polygon's sides cross only at shared corners.
fn simple(p: &[[f64; 2]]) -> bool {
    use crate::core::geom::{cross, sub};
    let n = p.len();
    let crosses = |a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]| {
        let (d1, d2) = (cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(d, a)));
        let (d3, d4) = (cross(sub(d, c), sub(a, c)), cross(sub(d, c), sub(b, c)));
        d1 * d2 < 0.0 && d3 * d4 < 0.0
    };
    for i in 0..n {
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            if crosses(p[i], p[(i + 1) % n], p[j], p[(j + 1) % n]) {
                return false;
            }
        }
    }
    true
}
