//! The runtime rule pack: what the engine reads.
//!
//! These types live here rather than in `pcgen-ir` so that the engine depends
//! on the *shape of the data* and not on the PCGen translator.  `pcgen-ir`
//! re-exports them, because lowering produces them.

use crate::domain::{
    ClassDefinition, ClassProgression, DeityDefinition, DomainDefinition, EquipmentDefinition,
    EquipmentModifierDefinition, RaceDefinition, SkillDefinition, SpellDefinition,
    TemplateDefinition,
};
use crate::{
    AbilityCategoryDefinition, AbilityDefinition, AlignmentDefinition, CompanionModDefinition,
    CompiledEntity, CreatureTraits, KitDefinition, RuleCoverage, SizeDefinition, StatDefinition,
    VariantDefinition,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledPackHeader {
    #[serde(default)]
    pub game_mode: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    /// Deterministic fingerprint of the pack's runtime semantics.
    #[serde(default)]
    pub rules_fingerprint: String,
    /// Upstream PCGen data revision the pack was built from, when known.
    #[serde(default)]
    pub data_ref: Option<String>,
}

/// Runtime-facing rule pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledRulePack {
    pub header: CompiledPackHeader,
    pub entities: Vec<CompiledEntity>,
    pub coverage: RuleCoverage,
    #[serde(default)]
    pub races: BTreeMap<String, RaceDefinition>,
    #[serde(default)]
    pub classes: BTreeMap<String, ClassDefinition>,
    #[serde(default)]
    pub skills: BTreeMap<String, SkillDefinition>,
    #[serde(default)]
    pub spells: BTreeMap<String, SpellDefinition>,
    #[serde(default)]
    pub equipment: BTreeMap<String, EquipmentDefinition>,
    #[serde(default)]
    pub equipment_modifiers: BTreeMap<String, EquipmentModifierDefinition>,
    #[serde(default)]
    pub deities: BTreeMap<String, DeityDefinition>,
    #[serde(default)]
    pub domains: BTreeMap<String, DomainDefinition>,
    #[serde(default)]
    pub variants: BTreeMap<String, VariantDefinition>,
    #[serde(default)]
    pub kits: BTreeMap<String, KitDefinition>,
    #[serde(default)]
    pub templates: BTreeMap<String, TemplateDefinition>,
    #[serde(default)]
    pub abilities: BTreeMap<String, AbilityDefinition>,
    #[serde(default)]
    pub ability_categories: BTreeMap<String, AbilityCategoryDefinition>,
    #[serde(default)]
    pub creature_trait_records: BTreeMap<String, CreatureTraits>,
    #[serde(default)]
    pub companion_mods: BTreeMap<String, Vec<CompanionModDefinition>>,
    #[serde(default)]
    pub sizes: BTreeMap<String, SizeDefinition>,
    #[serde(default)]
    pub stats: BTreeMap<String, StatDefinition>,
    #[serde(default)]
    pub alignments: BTreeMap<String, AlignmentDefinition>,
    #[serde(default)]
    pub progressions: BTreeMap<String, ClassProgression>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub coverage: RuleCoverage,
    pub patch_operations: usize,
    pub diagnostics: usize,
    pub error_diagnostics: usize,
    /// Executable effects attached to `SPELL` / `POWER` / `DOMAIN`
    /// definitions.
    #[serde(default)]
    pub spell_effects: usize,
    /// Executable effects attached to `POWER` definitions.
    #[serde(default)]
    pub psionic_effects: usize,
}
