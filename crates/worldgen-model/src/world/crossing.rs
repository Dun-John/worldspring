use super::*;

/// A crossing put down by hand: from `a` to `b` (world ft, bank to bank), `width` ft across.
/// A bridge's deck spans the water (planks clear of it, or old stone: `material`); a ford brings
/// the bed up to wading depth under stepping stones; a ferry runs a raft on a rope between two
/// jetties.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Crossing {
    pub kind: CrossingKind,
    pub a: [f64; 2],
    pub b: [f64; 2],
    #[serde(default = "crossing_width")]
    pub width: f64,
    /// What a bridge is built of (timber when absent: older files, and the file of a world with
    /// none stays as it was).
    #[serde(default, skip_serializing_if = "CrossingMaterial::is_timber")]
    pub material: CrossingMaterial,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CrossingMaterial {
    #[default]
    Timber,
    Stone,
}

impl CrossingMaterial {
    fn is_timber(&self) -> bool {
        *self == CrossingMaterial::Timber
    }

    pub fn name(self) -> &'static str {
        match self {
            CrossingMaterial::Timber => "timber",
            CrossingMaterial::Stone => "stone",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CrossingKind {
    Bridge,
    Ford,
    Ferry,
}

fn crossing_width() -> f64 {
    12.0
}

impl Crossing {
    /// Widths (ft) a crossing may have, and its length.
    pub const WIDTH: std::ops::RangeInclusive<f64> = 5.0..=120.0;
    /// (R5 M20) The widest a ford or a ferry may be (a bridge to `WIDTH`'s end, to carry shops).
    pub const NARROW_MAX: f64 = 40.0;
    pub const LENGTH: std::ops::RangeInclusive<f64> = 10.0..=2000.0;

    /// Length (ft), from a to b.
    pub fn length(&self) -> f64 {
        crate::core::sqrt((self.b[0] - self.a[0]) * (self.b[0] - self.a[0]) + (self.b[1] - self.a[1]) * (self.b[1] - self.a[1]))
    }

    /// Why it can't be put down, if so: its ends off the map or not finite, too short or long
    /// (a ferry needs room for its two jetties), too narrow or wide.
    pub fn problem(&self, map_w_ft: f64, map_h_ft: f64) -> Option<String> {
        let on = |p: [f64; 2]| p[0].is_finite() && p[1].is_finite() && (0.0..map_w_ft).contains(&p[0]) && (0.0..map_h_ft).contains(&p[1]);
        if !on(self.a) || !on(self.b) {
            return Some("both ends must be on the map".into());
        }
        let min = if self.kind == CrossingKind::Ferry { 2.0 * crate::model::consts::JETTY_FT + 20.0 } else { *Self::LENGTH.start() };
        let len = self.length();
        if len < min || len > *Self::LENGTH.end() {
            return Some(format!("a {} is {min:.0} to {:.0} ft long (this one is {len:.0})", self.kind.name(), Self::LENGTH.end()));
        }
        let max = if self.kind == CrossingKind::Bridge { *Self::WIDTH.end() } else { Self::NARROW_MAX };
        if !self.width.is_finite() || self.width < *Self::WIDTH.start() || self.width > max {
            return Some(format!("width: {:.0} to {max:.0} ft for a {}", Self::WIDTH.start(), self.kind.name()));
        }
        None
    }
}

/// A quay, a pier or a breakwater built by hand (`q:<id>`), along `pts` (world ft, 2 to
/// `MAX_PTS` points). A quay's line is its water edge: paving reaches `width` ft from it to the
/// dry side, and the water inside that strip is filled in (made ground behind a stone face). A
/// pier and a breakwater run from `pts[0]` on the bank out over the water: a pier a deck
/// (`material`), a breakwater a raised strip of rubble, always stone (`crate::wharf`).
///
/// R7 173: a quay brushed along the shore (`shore`) has its face traced along the water's edge
/// (`wharf::shore_line`, up to `MAX_SHORE_PTS` points ~5 ft apart), the dry side on the left of
/// its line (`[-u.y, u.x]`), paving `width` ft from it in a smooth band; `material` timber or stone;
/// `piers` ft apart (0: none) a pier sticks straight out from its face into the water.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Wharf {
    pub kind: WharfKind,
    pub pts: Vec<[f64; 2]>,
    #[serde(default = "wharf_width")]
    pub width: f64,
    #[serde(default, skip_serializing_if = "CrossingMaterial::is_timber")]
    pub material: CrossingMaterial,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shore: bool,
    #[serde(default, skip_serializing_if = "no_piers")]
    pub piers: f64,
}

fn no_piers(v: &f64) -> bool {
    *v == 0.0
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WharfKind {
    Quay,
    Pier,
    Breakwater,
}

fn wharf_width() -> f64 {
    20.0
}

impl WharfKind {
    pub fn name(self) -> &'static str {
        match self {
            WharfKind::Quay => "quay",
            WharfKind::Pier => "pier",
            WharfKind::Breakwater => "breakwater",
        }
    }

    /// Widths (ft) one may have.
    pub fn widths(self) -> std::ops::RangeInclusive<f64> {
        match self {
            WharfKind::Quay => 10.0..=60.0,
            WharfKind::Pier => 6.0..=30.0,
            WharfKind::Breakwater => 12.0..=60.0,
        }
    }

    /// The width (ft) when none is given.
    pub fn default_width(self) -> f64 {
        match self {
            WharfKind::Quay => 20.0,
            WharfKind::Pier => 10.0,
            WharfKind::Breakwater => 24.0,
        }
    }
}

impl Wharf {
    pub const MAX_PTS: usize = 16;
    /// A shore quay's points at most, each side at least `MIN_SHORE_SIDE_FT`; its piers
    /// `PIERS_FT` apart (or none), each `SHORE_PIER_FT` long and `SHORE_PIER_WIDTH_FT` wide.
    pub const MAX_SHORE_PTS: usize = 512;
    pub const MIN_SHORE_SIDE_FT: f64 = 1.0;
    pub const PIERS_FT: std::ops::RangeInclusive<f64> = 30.0..=400.0;
    pub const SHORE_PIER_FT: f64 = 40.0;
    pub const SHORE_PIER_WIDTH_FT: f64 = 10.0;

    /// A wharf of `kind` along `pts`, `width` ft wide, of timber, not a shore quay.
    pub fn new(kind: WharfKind, pts: Vec<[f64; 2]>, width: f64, material: CrossingMaterial) -> Wharf {
        Wharf { kind, pts, width, material, shore: false, piers: 0.0 }
    }
    /// Each side at least this long (ft), and the whole line at most `MAX_LENGTH_FT`.
    pub const MIN_SIDE_FT: f64 = 10.0;
    pub const MAX_LENGTH_FT: f64 = 2000.0;

    /// Length (ft) along its points.
    pub fn length(&self) -> f64 {
        self.pts.windows(2).map(|s| crate::core::sqrt((s[1][0] - s[0][0]) * (s[1][0] - s[0][0]) + (s[1][1] - s[0][1]) * (s[1][1] - s[0][1]))).sum()
    }

    /// Why it can't be built, if so: points off the map or not finite, too few or many, a side
    /// too short, too long in all, too narrow or wide for its kind.
    pub fn problem(&self, map_w_ft: f64, map_h_ft: f64) -> Option<String> {
        let n = self.pts.len();
        let shore = self.shore && self.kind == WharfKind::Quay;
        if self.shore && !shore {
            return Some(format!("a {} does not follow the shore (only a quay does)", self.kind.name()));
        }
        let (max_pts, min_side) = if shore { (Self::MAX_SHORE_PTS, Self::MIN_SHORE_SIDE_FT) } else { (Self::MAX_PTS, Self::MIN_SIDE_FT) };
        if !(2..=max_pts).contains(&n) {
            return Some(format!("a {} has 2 to {} points, not {n}", self.kind.name(), max_pts));
        }
        if self.piers != 0.0 && !(shore && Self::PIERS_FT.contains(&self.piers)) {
            return Some(format!("piers: 0 (none) or {:.0} to {:.0} ft apart, on a quay along the shore", Self::PIERS_FT.start(), Self::PIERS_FT.end()));
        }
        let on = |p: &[f64; 2]| p[0].is_finite() && p[1].is_finite() && (0.0..map_w_ft).contains(&p[0]) && (0.0..map_h_ft).contains(&p[1]);
        if !self.pts.iter().all(on) {
            return Some("every point must be on the map".into());
        }
        for (k, s) in self.pts.windows(2).enumerate() {
            let d = crate::core::sqrt((s[1][0] - s[0][0]) * (s[1][0] - s[0][0]) + (s[1][1] - s[0][1]) * (s[1][1] - s[0][1]));
            if d < min_side {
                return Some(format!("side {} is {d:.0} ft: each side at least {:.0} ft", k + 1, min_side));
            }
        }
        let len = self.length();
        if len > Self::MAX_LENGTH_FT {
            return Some(format!("a {} is at most {:.0} ft long (this one is {len:.0})", self.kind.name(), Self::MAX_LENGTH_FT));
        }
        let w = self.kind.widths();
        if !self.width.is_finite() || !w.contains(&self.width) {
            return Some(format!("a {}'s width: {:.0} to {:.0} ft", self.kind.name(), w.start(), w.end()));
        }
        None
    }
}

impl CrossingKind {
    pub fn name(self) -> &'static str {
        match self {
            CrossingKind::Bridge => "bridge",
            CrossingKind::Ford => "ford",
            CrossingKind::Ferry => "ferry",
        }
    }
}
