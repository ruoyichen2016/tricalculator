//! Typed damage, weapon facts and persistent spell effects shared by all encounter actions.
use crate::magic::Energy;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "energy", rename_all = "snake_case")]
pub enum DamageKind {
    Energy(Energy),
    Untyped,
    NegativeEnergy,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TouchImpact {
    Vampiric { temporary_seconds: u32 },
    Chill { strength_damage: u32 },
    Fatigue { seconds: String },
    Idiocy { seconds: String },
    Ghoul { radius_feet: u32 },
    Dispel { maximum_bonus: u32 },
    Teleport { feet: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponTrait {
    Bludgeoning,
    Piercing,
    Slashing,
    Magic,
    Epic,
    Silver,
    ColdIron,
    Adamantine,
    Good,
    Evil,
    Lawful,
    Chaotic,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicalReduction {
    pub amount: u32,
    /// Alternatives (OR), each containing requirements (AND). Empty = never bypassed.
    pub bypass: Vec<Vec<WeaponTrait>>,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponMaterial {
    #[default]
    Ordinary,
    Silver,
    ColdIron,
    Adamantine,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeaponSetup {
    pub equipment: String,
    pub enhancement: u32,
    pub material: WeaponMaterial,
    pub masterwork: bool,
    pub two_handed: bool,
    pub alignments: Vec<WeaponTrait>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatState {
    pub immunities: BTreeMap<String, Vec<crate::class_play::BodyImmunity>>,
    pub weapon: Option<WeaponSetup>,
    pub reductions: BTreeMap<String, Vec<PhysicalReduction>>,
    pub effects: BTreeMap<String, ActiveSpell>,
    pub temporary: BTreeMap<String, BTreeMap<String, TemporaryHP>>,
    pub stench: BTreeMap<String, StenchAura>,
    pub exposures: BTreeMap<String, StenchExposure>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemporaryHP {
    pub remaining: i64,
    pub expires: u64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SpellCondition {
    Fatigued,
    Paralyzed,
    Panicked,
    Sickened,
    AbilityPenalty {
        abilities: Vec<String>,
        amount: u32,
        floor: i32,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveSpell {
    pub casting: String,
    pub name: String,
    pub target: String,
    pub caster_level: i32,
    pub expires: u64,
    pub condition: SpellCondition,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StenchAura {
    pub casting: String,
    pub target: String,
    pub caster_level: i32,
    pub dc: i32,
    pub expires: u64,
    pub radius_feet: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StenchExposure {
    pub aura: String,
    pub target: String,
    pub saved: Option<bool>,
}
