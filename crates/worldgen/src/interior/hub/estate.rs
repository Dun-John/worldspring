//! Noble estates and older palaces, laid out as hall houses on `keep`'s zones with rooms of their own:
//! - a hall house (a small estate): the hall between the service end (kitchen, buttery, pantry) and
//!   the screens passage at its low end and a parlour by the stair at its high end; chambers above,
//!   the solar by the stair;
//! - a manor (a big estate): the same hall between a service wing and a cross wing for the family,
//!   deeper (the parlour, a stair hall, a study; a chapel in the richest), the great chamber and the
//!   solar above it, chambers, guest chambers, a library;
//! - an older palace (any city's but the capital's): a big keep without the defences, its great hall
//!   (a throne dais at its far end: `super::super::hall_styles`) with a guard room by the entrance in
//!   the service end, a guard room and a chapel off the dais end, the audience chamber (the presence
//!   chamber, where a keep has its great chamber) and the solar by the stair, a long gallery down the
//!   top storey's long wall, a treasury.
//!
//! Passages in an estate are corridors (a keep's in the wall's thickness are mural passages).

use super::{Kind, Kinds, Plan};

/// The least squares a noble estate is laid out by zones with (else as other houses are), and the
/// most a hall house has (more, a manor).
pub(super) const LEAST: usize = 40;
pub(super) const MANOR: usize = 130;
/// The least share of its grid an estate's lot fills to be laid out by zones (a lot cut slant by a
/// street's corner is laid out as other houses are).
pub(super) const FILL: f64 = 0.8;
/// A manor of this many squares or more is rich enough for a chapel.
pub(super) const RICH: usize = 260;

/// The rooms each upper floor must still have, in order (`Plan::pending`).
pub(super) fn pending(kind: Kind) -> Vec<&'static str> {
    match kind {
        Kind::HallHouse => vec!["solar"],
        Kind::Manor => vec!["great chamber", "solar"],
        _ => vec!["great chamber", "solar", "treasury"],
    }
}

/// How many rows the dais end reaches past the stair (`keep::plan`'s `find`): a manor's cross wing
/// a quarter of the body or so.
pub(super) fn dais_back(kind: Kind, len: usize) -> isize {
    match kind {
        Kind::Manor => ((crate::core::round(0.25 * len as f64) as isize) - 2).clamp(1, 3),
        _ => 1,
    }
}

/// The dais end's rooms each side of the landing: the front side's, the far side's.
pub(super) fn dais_rooms(p: &Plan) -> (&'static str, &'static str) {
    match p.kind {
        Kind::HallHouse => ("parlor", "wardrobe"),
        Kind::Manor => ("parlor", if p.rich { "chapel" } else { "study" }),
        _ => ("guardroom", "chapel"),
    }
}

/// The service end's rooms across it (by weight), `w` squares across.
pub(super) fn service(kind: Kind, w: usize) -> Vec<(&'static str, f64)> {
    match (kind, w >= 7) {
        // (An older palace's guard room by the entrance, on the screens passage.)
        (Kind::OldPalace, true) => vec![("guardroom", 1.0), ("kitchen", 1.6), ("pantry", 1.0)],
        (Kind::OldPalace, false) => vec![("kitchen", 1.6), ("guardroom", 1.0)],
        // (A manor's kitchen the biggest of its service wing's three, the wing 6 squares across or
        // more.)
        (Kind::Manor, _) if w >= 6 => vec![("buttery", 1.0), ("kitchen", 2.2), ("pantry", 1.0)],
        (_, true) => vec![("buttery", 1.0), ("kitchen", 1.6), ("pantry", 1.0)],
        _ => vec![("kitchen", 1.6), ("buttery", 1.0)],
    }
}

/// The rooms beside a hall narrower than the body, by weight, and what fills the rest.
pub(super) fn hall_kinds(kind: Kind) -> (Kinds, &'static [&'static str]) {
    match kind {
        Kind::OldPalace => (vec![("antechamber", 1.4), ("chamber", 1.2), ("guest chamber", 1.0), ("wardrobe", 0.6)], &["chamber", "storeroom", "guest chamber"]),
        _ => (vec![("chamber", 1.4), ("study", 1.0), ("storeroom", 0.8), ("wardrobe", 0.6)], &["chamber", "storeroom"]),
    }
}

/// The rooms floor `n` above the hall holds besides what must be somewhere (`last`: the top storey).
pub(super) fn upper_kinds(kind: Kind, n: usize, last: bool) -> Kinds {
    match (kind, n) {
        (Kind::OldPalace, 1) => vec![("chamber", 1.4), ("guest chamber", 1.1), ("wardrobe", 0.7), ("study", 0.9)],
        (Kind::OldPalace, _) if last => vec![("servants' quarters", 1.5), ("guest chamber", 1.3), ("library", 1.2), ("chamber", 1.0), ("storeroom", 0.8)],
        (Kind::OldPalace, _) => vec![("guest chamber", 1.5), ("library", 1.2), ("chamber", 1.0), ("wardrobe", 0.6)],
        (_, 1) => vec![("bedchamber", 1.6), ("chamber", 1.2), ("study", 0.9), ("wardrobe", 0.6)],
        (Kind::Manor, _) => vec![("guest chamber", 1.4), ("library", 1.2), ("servants' quarters", 1.1), ("chamber", 1.0), ("storeroom", 0.8)],
        _ => vec![("chamber", 1.4), ("servants' quarters", 1.1), ("storeroom", 0.8)],
    }
}

/// The names of a hall house's floors: the hall's, the lord's, the others'.
pub(super) fn names(kind: Kind) -> [&'static str; 3] {
    match kind {
        Kind::OldPalace => ["Hall", "State floor", "Upper floor"],
        _ => ["Ground floor", "Family floor", "Upper floor"],
    }
}

impl Plan {
    /// A palace's or an estate's rooms as named in it once a floor is laid out: an older palace's
    /// great chamber is its audience chamber; an estate's passages are corridors, its stair's
    /// landing on the ground floor a manor's stair hall.
    pub(super) fn estate_names(&self, rooms: &mut [crate::interior::Room], ground: bool) {
        for r in rooms.iter_mut() {
            r.kind = match (self.kind, r.kind) {
                (Kind::OldPalace, "great chamber") => "audience chamber",
                (Kind::HallHouse | Kind::Manor, "mural passage") => "corridor",
                (Kind::Manor, "landing") if ground => "stair hall",
                (_, k) => k,
            };
        }
    }

    /// An estate or an older palace (not a keep or a barracks).
    pub(super) fn estate(&self) -> bool {
        matches!(self.kind, Kind::OldPalace | Kind::HallHouse | Kind::Manor)
    }
}
