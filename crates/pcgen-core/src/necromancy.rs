//! Persistent afflictions and undead allegiance; independent of encounter boundaries.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiseaseRule {
    pub name: String,
    pub dc: i32,
    pub damage: BTreeMap<String, u32>,
    pub blindness: bool,
    pub drain: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NecromancyState {
    #[serde(default)]
    pub spell_bonds: BTreeMap<String, UndeadBond>,
    #[serde(default)]
    pub animated: BTreeMap<String, UndeadBond>,
    #[serde(default)]
    pub onyx_spent_gp: u32,
    #[serde(default)]
    pub lich: bool,
    pub armed_disease: Option<ArmedDisease>,
    pub armed_drain: Option<ArmedDrain>,
    pub diseases: Vec<DiseaseCase>,
    pub negative_levels: Vec<NegativeLevel>,
    pub actors: BTreeMap<String, AfflictedActor>,
    pub commands: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AfflictedActor {
    #[serde(default)]
    pub needs_level_rebuild: bool,
    #[serde(default)]
    pub constitution_hp_loss: i64,
    pub scores: BTreeMap<String, i32>,
    pub damage: BTreeMap<String, u32>,
    pub drain: BTreeMap<String, u32>,
    pub immune_disease: bool,
    pub immune_drain: bool,
    pub turn_resistance: u32,
    pub last_recovery: u64,
    pub permanent_levels_lost: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmedDisease {
    pub rule: DiseaseRule,
    pub dc: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArmedDrain {
    pub count: u32,
    pub dc: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiseaseCase {
    pub target: String,
    pub rule: DiseaseRule,
    pub due: u64,
    pub successes: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NegativeLevel {
    pub target: String,
    pub dc: i32,
    pub due: u64,
    #[serde(default)]
    pub lost_slot: Option<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AfflictionRolls {
    pub save: Option<u32>,
    pub secondary_save: Option<u32>,
    pub damage: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UndeadBond {
    pub source: String,
    pub hit_dice: u32,
    pub expires: Option<u64>,
    pub orders: String,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UndeadSpellRequest {
    pub cast: crate::magic::CastRequest,
    pub targets: Vec<String>,
    pub saves: BTreeMap<String, u32>,
    pub penetration: BTreeMap<String, u32>,
    pub release: Vec<String>,
    pub zombie: bool,
    pub intact_remains: bool,
    pub desecrated_altar: bool,
    pub desecrate_altar: bool,
    pub hp_totals: BTreeMap<String, u32>,
    pub racial_hit_dice: BTreeMap<String, u32>,
    pub onyx_gp: u32,
}
