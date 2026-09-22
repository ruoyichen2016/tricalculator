//! Derived character output.

use pcgen_core::{
    BuildProvenance, Contribution, DomainSpell, FaithDecision, GestaltPlan, SpellPowerType,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    pub draft_id: String,
    pub name: String,
    pub race_id: String,
    pub total_level: u32,
    pub stats: BTreeMap<String, i32>,
    pub hit_points: i32,
    pub base_attack_bonus: i32,
    pub initiative: i32,
    pub armor_class: i32,
    pub armor_bonus: i32,
    pub shield_bonus: i32,
    pub touch_ac: i32,
    pub flat_footed_ac: i32,
    pub saves: BTreeMap<String, i32>,
    #[serde(default)]
    pub skills: BTreeMap<String, i32>,
    #[serde(default)]
    pub caster_levels: BTreeMap<String, i32>,
    #[serde(default)]
    pub spell_dcs: BTreeMap<String, i32>,
    #[serde(default)]
    pub spell_slots: BTreeMap<String, u32>,
    #[serde(default)]
    pub known_limits: BTreeMap<String, u32>,
    #[serde(default)]
    pub prepared_spells: BTreeMap<String, u32>,
    /// Arcane / Divine / Psionic powers the character can cast.
    #[serde(default)]
    pub spell_power_types: BTreeSet<SpellPowerType>,
    /// Highest spell level with a slot, per power type.
    #[serde(default)]
    pub max_spell_level: BTreeMap<SpellPowerType, u32>,
    /// Character has a spellbook-using class.
    #[serde(default)]
    pub has_spellbook: bool,
    #[serde(default)]
    pub power_points: BTreeMap<String, i32>,
    #[serde(default)]
    pub manifester_levels: BTreeMap<String, i32>,
    #[serde(default)]
    pub gestalt: Option<GestaltPlan>,
    #[serde(default)]
    pub faith_decisions: Vec<FaithDecision>,
    #[serde(default)]
    pub domain_spells: BTreeMap<String, Vec<DomainSpell>>,
    #[serde(default)]
    pub variant_effects: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub explanations: BTreeMap<String, Vec<Contribution>>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, i32>,
    #[serde(default)]
    pub external_modifiers: BTreeMap<String, i32>,
    #[serde(default)]
    pub active_abilities: Vec<String>,
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
    #[serde(default)]
    pub unresolved_choices: Vec<String>,
    #[serde(default)]
    pub applied_effects: usize,
    /// Build provenance: rule-pack fingerprint plus upstream data revision.
    #[serde(default)]
    pub provenance: BuildProvenance,
}
