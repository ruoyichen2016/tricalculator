//! Martial training is a character input; initiator levels and action budgets are derived.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MartialClass {
    pub disciplines: Vec<String>,
    pub prestige: bool,
    /// Base-class rows are cumulative; prestige rows are awards at that level.
    pub levels: BTreeMap<String, MartialLevel>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MartialLevel {
    pub known: u32,
    pub readied: u32,
    pub stances: u32,
    #[serde(default)]
    pub granted: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maneuver {
    pub name: String,
    pub discipline: String,
    pub level: u32,
    pub stance: bool,
    pub action: String,
    pub prerequisite_count: u32,
    pub teleportation: bool,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManeuverAcquisition {
    pub character_level: u32,
    pub source_class: String,
    pub track: String,
    pub maneuver: String,
    #[serde(default)]
    pub replaces: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MartialTraining {
    pub acquisitions: Vec<ManeuverAcquisition>,
    /// Prestige readied awards are assigned individually, not to both tracks.
    pub readied_advances: BTreeMap<u32, String>,
}

impl MartialClass {
    pub fn at_level(&self, level: u32) -> Option<&MartialLevel> {
        self.levels.get(&level.to_string())
    }
}
