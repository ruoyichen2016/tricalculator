//! Incarnum progression is neither spellcasting nor martial advancement.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncarnumClass {
    #[serde(default)]
    pub entry_essentia: u32,
    pub levels: BTreeMap<String, IncarnumLevel>,
    #[serde(default)]
    pub investments: BTreeMap<String, u32>,
}
impl IncarnumClass {
    pub fn at_level(&self, level: u32) -> Option<&IncarnumLevel> {
        self.levels.get(&level.to_string())
    }
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncarnumLevel {
    pub essentia: u32,
    #[serde(default)]
    pub soulmelds: u32,
    #[serde(default)]
    pub binds: u32,
    #[serde(default)]
    pub chakras: Vec<String>,
    #[serde(default)]
    pub totem_capacity_bonus: u32,
}
