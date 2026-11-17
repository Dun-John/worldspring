//! The biomes (`t0::biome` classifies the map into them).

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Biome {
    Ocean = 0,
    Lake,
    Ice,
    Tundra,
    Alpine,
    Taiga,
    TemperateForest,
    TemperateRainforest,
    Grassland,
    Steppe,
    ColdDesert,
    HotDesert,
    Savanna,
    Jungle,
    Swamp,
    Volcanic,
    SaltFlat,
    /// Painted only: woods sickened by a curse or rot (dead and dying trees, fungus).
    Blight,
    /// Painted only: land buried under old ash falls (grey, sparse, burnt stumps).
    Ashland,
}

pub const ALL: [Biome; 19] = [
    Biome::Ocean,
    Biome::Lake,
    Biome::Ice,
    Biome::Tundra,
    Biome::Alpine,
    Biome::Taiga,
    Biome::TemperateForest,
    Biome::TemperateRainforest,
    Biome::Grassland,
    Biome::Steppe,
    Biome::ColdDesert,
    Biome::HotDesert,
    Biome::Savanna,
    Biome::Jungle,
    Biome::Swamp,
    Biome::Volcanic,
    Biome::SaltFlat,
    Biome::Blight,
    Biome::Ashland,
];

impl Biome {
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "ocean",
            Biome::Lake => "lake",
            Biome::Ice => "ice",
            Biome::Tundra => "tundra",
            Biome::Alpine => "alpine",
            Biome::Taiga => "taiga",
            Biome::TemperateForest => "temperate_forest",
            Biome::TemperateRainforest => "temperate_rainforest",
            Biome::Grassland => "grassland",
            Biome::Steppe => "steppe",
            Biome::ColdDesert => "cold_desert",
            Biome::HotDesert => "hot_desert",
            Biome::Savanna => "savanna",
            Biome::Jungle => "jungle",
            Biome::Swamp => "swamp",
            Biome::Volcanic => "volcanic",
            Biome::SaltFlat => "salt_flat",
            Biome::Blight => "blighted_woods",
            Biome::Ashland => "ashlands",
        }
    }

    pub fn from_u8(v: u8) -> Biome {
        ALL.get(v as usize).copied().unwrap_or(Biome::Grassland)
    }
}
