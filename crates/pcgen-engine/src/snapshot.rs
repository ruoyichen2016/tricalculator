//! Derived character output.

use pcgen_core::{
    BuildProvenance, Contribution, DomainSpell, FaithDecision, GestaltPlan, SpellPowerType,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Derived state for one companion owned by the character.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompanionSnapshot {
    pub companion_type: String,
    pub master_level: u32,
    #[serde(default)]
    pub hit_dice_bonus: i32,
    #[serde(default)]
    pub spell_resistance: Option<i32>,
    #[serde(default)]
    pub abilities: Vec<String>,
}

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
    /// Typed attack bonus totals keyed by canonical target (e.g.
    /// `COMBAT.TOHIT`, `WEAPON.TOHIT`, `WEAPONPROF=Longbow.TOHIT`).
    #[serde(default)]
    pub attack_bonuses: BTreeMap<String, i32>,
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
    /// Creature vision modes from race + active templates (`VISION`).
    #[serde(default)]
    pub vision: Vec<String>,
    /// Damage reduction entries from race + active templates (`DR`).
    #[serde(default)]
    pub damage_reduction: Vec<String>,
    /// Spell resistance from race + active templates (`SR`).
    #[serde(default)]
    pub spell_resistance: Option<i32>,
    /// Natural attack entries from race + active templates (`NATURALATTACKS`).
    #[serde(default)]
    pub natural_attacks: Vec<String>,
    /// Natural reach in feet (`REACH`).
    #[serde(default)]
    pub reach_feet: Option<i32>,
    /// Reach multiplier (`REACHMULT`).
    #[serde(default)]
    pub reach_mult: Option<i32>,
    /// Unarmed damage progression entries (`UDAM`).
    #[serde(default)]
    pub unarmed_damage: Vec<String>,
    /// Unarmed damage multiplier (`UMULT`).
    #[serde(default)]
    pub unarmed_mult: Option<i32>,
    /// Movement modes in feet (lower-cased mode -> rate), after race + active
    /// `MOVECLONE` expansion.
    #[serde(default)]
    pub movement: BTreeMap<String, u32>,
    /// Derived companion states (familiar / special mount / ...).
    #[serde(default)]
    pub companions: Vec<CompanionSnapshot>,
    #[serde(default)]
    pub gestalt: Option<GestaltPlan>,
    #[serde(default)]
    pub faith_decisions: Vec<FaithDecision>,
    #[serde(default)]
    pub domain_spells: BTreeMap<String, Vec<DomainSpell>>,
    /// Character-facing spell effect summary: domain spells, prepared spells.
    #[serde(default)]
    pub spell_effects: Vec<String>,
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
    /// Explicitly activated temporary states.
    #[serde(default)]
    pub active_states: Vec<String>,
    #[serde(default)]
    pub unresolved_choices: Vec<String>,
    #[serde(default)]
    pub applied_effects: usize,
    /// Build provenance: rule-pack fingerprint plus upstream data revision.
    #[serde(default)]
    pub provenance: BuildProvenance,
}
