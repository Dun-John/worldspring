//! The kitchen's and smithy's kinds in the catalog (`battlemap::kitchen` lays them out).

use super::*;

/// The kinds (the castles lane's ids, 75-87), with their rules: rows and the fence are low, seen
/// across and stepped over (difficult); a skep and an anvil are stood round.
pub(super) const KINDS: [(Kind, KindInfo); 4] = [
    (Kind::VegRow, k(Kind::VegRow, "vegetable row", 1.0, false, false, 0, true, 1.5, true)),
    (Kind::WattleFence, k(Kind::WattleFence, "wattle fence", 0.55, false, false, 1, true, 3.0, true)),
    (Kind::Beehive, k(Kind::Beehive, "beehive", 0.3, true, false, 1, false, 3.0, true)),
    (Kind::Anvil, k(Kind::Anvil, "anvil", 0.4, true, false, 1, false, 3.0, true)),
];
