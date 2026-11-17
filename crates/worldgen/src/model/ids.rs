//! Ids of levels, rooms and point notes underground, split into their parts.

/// A level's or room's id split into its site, level and room: `l:<site>:<level>`,
/// `r:<site>:<level>:<room>`.
pub fn split_room_id(id: &str) -> Option<(&str, usize, Option<usize>)> {
    if let Some(rest) = id.strip_prefix("l:") {
        let (site, lv) = rest.rsplit_once(':')?;
        return Some((site, lv.parse().ok()?, None));
    }
    let (head, room) = id.strip_prefix("r:")?.rsplit_once(':')?;
    let (site, lv) = head.rsplit_once(':')?;
    Some((site, lv.parse().ok()?, Some(room.parse().ok()?)))
}

/// A point note's id split into its site, level and own id: `m:<site>:<level>:<id>` (the
/// site id is everything between).
pub fn split_point_id(id: &str) -> Option<(&str, usize, &str)> {
    let (head, own) = id.strip_prefix("m:")?.rsplit_once(':')?;
    let (site, lv) = head.rsplit_once(':')?;
    Some((site, lv.parse().ok()?, own))
}
