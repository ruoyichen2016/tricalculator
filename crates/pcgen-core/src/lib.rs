//! PCGen-Rust core: IDs, exact arithmetic, rule IR, advancement frames,
//! Gestalt policy, and the two graph algorithms the domain needs.
//!
//! The design follows the Tri20 separation between a compatibility
//! importer and a deterministic runtime.  This crate is the runtime
//! vocabulary; it deliberately contains no PCGen `.lst` parsing.

pub mod advancement;
pub mod algorithms;
pub mod class_grant;

pub use class_grant::{ClassGrant, GrantMode};
pub use rules::{
    collapse_damage_reduction, AddedDomain, DamageReduction, ExchangeLevel, NaturalAttack,
    ProhibitedSpell, ProhibitionTarget, RaceSpec,
};
pub mod domain;
pub mod equipment;
pub mod faith;
pub mod fingerprint;
pub mod formula_vocab;
pub mod pack;

pub use pack::{CompiledPackHeader, CompiledRulePack, ImportReport};
pub mod gestalt;
pub mod identity;
pub mod progression;
pub mod psionics;
pub mod rational;
pub mod rules;
pub mod skill_rules;
pub mod tagclass;
pub mod variants;
pub mod vocab;

pub use advancement::{AdvancementFrame, AdvancementLane, LaneId};
pub use algorithms::{strongly_connected_components, topological_sort};
pub use domain::{
    AbilityCategoryDefinition, AbilityDefinition, AlignmentDefinition, AttackProgression,
    CastingType, ClassDefinition, ClassProgression, CompanionBonus, CompanionModDefinition,
    CreatureTraits, DeityDefinition, DomainDefinition, DomainSpell, EquipmentDefinition,
    EquipmentModifierDefinition, KitDefinition, KitEntry, RaceDefinition, SaveProgression,
    SizeDefinition, SkillDefinition, SpellDefinition, SpellPowerType, StatDefinition,
    TemplateDefinition, TypedDefinitions,
};
pub use faith::{evaluate_faith, FaithDecision};
pub use fingerprint::{
    hash_parts, rules_fingerprint, stable_fingerprint, BuildProvenance, RuleFingerprintInput,
};
pub use gestalt::{
    plan_gestalt, plan_gestalt_incremental, ClassProgressSource, CollectionMode, DivineTrack,
    FaithPolicy, GestaltDiagnostic, GestaltFramePlan, GestaltPlan, GestaltPolicy,
    IncrementalGestaltUpdate, MergeMode, PrestigeSlotCost,
};
pub use identity::EntityId;
pub use progression::SaveKind;
pub use psionics::{ability_bonus_power_points, base_power_points, total_power_points};
pub use rational::{Rational, RationalError};
pub use rules::{
    normalize_size, size_at_least, size_at_most, size_rank, AlternateWeaponProfile,
    BonusTypePolicy, CategoryCoverage, CompiledEntity, Contribution, Effect, EffectAction,
    EffectPhase, EntityMetadata, Explanation, Modifier, ModifierOperation, Predicate, Provenance,
    RuleCoverage, StackPolicyKind, TemporaryValue, SIZE_ORDER,
};
pub use tagclass::{
    bucket_for_unsupported, classify as classify_tag, is_allowlisted, TagBucket, TagClass,
};
pub use variants::{feature_match_key, feature_name, line_owner, ReplaceTarget, VariantDefinition};
pub use vocab::{ChoiceKind, EntityCategory};
