//! Spell access and cast transformations. Slot cost, effective spell level and
//! caster level are separate quantities; none stands in for the others.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassMagic {
    /// Slot eligibility and bonus slots still use the class casting ability.
    #[serde(default)]
    pub save_dc_ability: Option<String>,
    #[serde(default)]
    pub incarnum: Option<crate::incarnum::IncarnumClass>,
    #[serde(default)]
    pub martial: Option<crate::martial::MartialClass>,
    #[serde(default)]
    pub diseases: Vec<crate::necromancy::DiseaseRule>,
    #[serde(default)]
    pub choices: Vec<ClassChoiceRule>,
    #[serde(default)]
    pub progression: Vec<FeatureLevel>,
    #[serde(default)]
    pub proficiencies: Vec<ProficiencyGrant>,
    #[serde(default)]
    pub advanced_learning: Option<LearningRule>,
    #[serde(default)]
    pub prepared_entire_list: bool,
    #[serde(default)]
    pub caster_level_formula: Option<String>,
    #[serde(default)]
    pub first_spell_level: u32,
    #[serde(default)]
    pub feature_values: Vec<FeatureFormula>,
    #[serde(default)]
    pub bonus_feats: Vec<BonusFeatGrant>,
    #[serde(default)]
    pub cloaked_casting: bool,
    #[serde(default)]
    pub sudden_grants: Vec<SuddenGrant>,
    #[serde(default)]
    pub improved_metamagic: Option<u32>,
    #[serde(default)]
    pub instant_metamagic: Vec<DailyUseTier>,
    #[serde(default)]
    pub trigger_from_level: Option<u32>,
    #[serde(default)]
    pub seize_from_level: Option<u32>,
    #[serde(default)]
    pub snatch_from_level: Option<u32>,
    #[serde(default)]
    pub sacred_features: bool,
    #[serde(default)]
    pub sacred_spells: Vec<LimitedSpellFeature>,
    #[serde(default)]
    pub domains: Vec<DomainAccess>,
    #[serde(default)]
    pub edge_ability: Option<String>,
    #[serde(default)]
    pub armored_casting: Option<ArmoredCasting>,
    #[serde(default)]
    pub wings: Option<WingsFeature>,
    #[serde(default)]
    pub exorcism_from_level: Option<u32>,
    #[serde(default)]
    pub detections: Vec<DetectionFeature>,
    #[serde(default)]
    pub descriptor_bonuses: Vec<DescriptorBonus>,
    #[serde(default)]
    pub metamagic_features: Vec<MetamagicFeature>,
    #[serde(default)]
    pub school_choice: Option<SchoolChoice>,
    #[serde(default)]
    pub entry_forbidden_schools: Vec<String>,
    #[serde(default)]
    pub knows_entire_list: bool,
    #[serde(default)]
    pub list_access: Vec<ListAccess>,
    #[serde(default)]
    pub turning_from_level: Option<u32>,
    #[serde(default)]
    pub turning_mode: Option<TurningModeRule>,
    #[serde(default)]
    pub tapestry_from_level: Option<u32>,
    #[serde(default)]
    pub enlarged_tapestry_level: Option<u32>,
    #[serde(default)]
    pub entry_spells_any: Vec<String>,
    #[serde(default)]
    pub entry_states: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassChoiceRule {
    pub key: String,
    pub label: String,
    pub at_level: u32,
    pub options: Vec<ClassChoiceOption>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassChoiceOption {
    #[serde(default)]
    pub breath: Option<BreathRule>,
    pub value: String,
    pub alignments: Vec<String>,
    pub class_skills: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListAccess {
    #[serde(default)]
    pub target_class: Option<String>,
    #[serde(default)]
    pub power: Option<crate::SpellPowerType>,
    #[serde(default)]
    pub max_spell_level: Option<u32>,
    pub at_level: u32,
    pub list: String,
    pub divine_unless_on: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellRule {
    Heighten,
    Earth,
    Versatile,
    Sanctum,
    Persistent,
    Extend,
    Enlarge,
    DivineMetamagic,
    Empower,
    Maximize,
    Silent,
    Still,
    Quicken,
    Widen,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellRange {
    Personal,
    Touch,
    Close,
    Medium,
    Long,
    Fixed,
    #[default]
    Unknown,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellProfile {
    /// Verified intrinsic AC, save, healing or temporary-hit-point benefit.
    #[serde(default)]
    pub spellguard_defensive: bool,
    #[serde(default)]
    pub touch_uses: Option<String>,
    #[serde(default)]
    pub impact: Option<crate::combat::TouchImpact>,
    #[serde(default)]
    pub damage_kind: Option<crate::combat::DamageKind>,
    #[serde(default)]
    pub affects_objects: Option<bool>,
    #[serde(default)]
    pub casting_time: Option<SpellCastingTime>,
    #[serde(default)]
    pub energy: Option<Energy>,
    /// Same-target damage every round, including the initial hit. Formula in CASTERLEVEL.
    #[serde(default)]
    pub recurring_rounds: Option<String>,
    #[serde(default)]
    pub defense: Option<SpellDefenseRule>,
    #[serde(default)]
    pub widenable: Option<bool>,
    #[serde(default)]
    pub damage: Option<DamageProfile>,
    #[serde(default)]
    pub hit_point_damage: Option<bool>,
    #[serde(default)]
    pub somatic: Option<bool>,
    #[serde(default)]
    pub material_cost_gp: Option<u32>,
    #[serde(default)]
    pub xp_cost: Option<u32>,
    #[serde(default)]
    pub variable_numeric_effect: Option<bool>,
    #[serde(default)]
    pub range: SpellRange,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub instantaneous: Option<bool>,
    #[serde(default)]
    pub discharged: Option<bool>,
    #[serde(default)]
    pub harmless_or_willing: Option<bool>,
    #[serde(default)]
    pub targets_creatures: Option<bool>,
    #[serde(default)]
    pub targeting: Option<TargetingRule>,
    #[serde(default)]
    pub fixed_range_feet: Option<u32>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CastRequest {
    #[serde(default)]
    pub spellguard_touch: bool,
    #[serde(default)]
    pub quick_cast: bool,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub target_denied_dex: bool,
    #[serde(default)]
    pub instant_metamagic: Option<SpellRule>,
    #[serde(default)]
    pub instant_spent: u32,
    #[serde(default)]
    pub sudden: Vec<SpellRule>,
    #[serde(default)]
    pub sudden_spent: BTreeMap<String, u32>,
    /// Optional situational armor check; omission makes no armor claim.
    #[serde(default)]
    pub armor: Option<ArmorContext>,
    #[serde(default)]
    pub tapestry_targets: Option<TapestryTargets>,
    pub class_id: String,
    pub spell: String,
    /// Explicit adjudication: Earth Spell counts the slot replaced by Versatile.
    #[serde(default)]
    pub earth_uses_virtual_slot: bool,
    /// Chosen Heighten level, not a caster level or slot level.
    #[serde(default)]
    pub heighten_to: Option<u32>,
    #[serde(default)]
    pub metamagic: Vec<SpellRule>,
    #[serde(default)]
    pub versatile: bool,
    #[serde(default)]
    pub earth: bool,
    #[serde(default)]
    pub on_stone_or_unworked_earth: bool,
    #[serde(default)]
    pub inside_sanctum: bool,
    /// One selected Divine Metamagic feat application.
    #[serde(default)]
    pub divine_metamagic: Option<SpellRule>,
    #[serde(default)]
    pub turning_spent: u32,
    #[serde(default)]
    pub slots_spent: BTreeMap<u32, u32>,
    #[serde(default)]
    pub tapestry: bool,
    #[serde(default)]
    pub quiescent: bool,
    #[serde(default)]
    pub stored_spells: u32,
}

/// Rules for applying metamagic without recasting a spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetamagicMode {
    Cooperative,
    Effect,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetamagicFeature {
    pub mode: MetamagicMode,
    pub at_level: u32,
    pub dc_base: u32,
    pub dc_per_spell_level: u32,
    pub uses_base: u32,
    pub uses_ability: String,
    pub action: String,
    pub excluded: Vec<SpellRule>,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchoolChoice {
    pub at_level: u32,
    pub choice_key: String,
    pub allowed: Vec<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetamagicCost {
    pub slot_increase: u32,
    pub casting_only: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetamagicRequest {
    pub mode: MetamagicMode,
    pub spell: String,
    /// The ORIGINAL caster's spell list, not the incantatrix's casting track.
    pub spell_list: String,
    #[serde(default)]
    pub existing: Vec<SpellRule>,
    pub apply: SpellRule,
    #[serde(default)]
    pub inside_sanctum: bool,
    /// Raw d20 roll. None requests Take 10; this is never a total bonus override.
    pub roll: Option<u32>,
    #[serde(default)]
    pub threatened: bool,
    #[serde(default)]
    pub distracted: bool,
    #[serde(default)]
    pub spent: u32,
    #[serde(default)]
    pub adjacent: bool,
    #[serde(default)]
    pub within_effect: bool,
    #[serde(default)]
    pub willing_ally: bool,
    #[serde(default)]
    pub readied_action: bool,
    #[serde(default)]
    pub ongoing_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmoredCasting {
    pub light_from_level: u32,
    pub medium_from_level: u32,
    pub light_shield: bool,
    #[serde(default)]
    pub heavy_shield_from_level: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WingsFeature {
    pub at_level: u32,
    pub speed_feet: u32,
    pub minutes_per_level: u32,
    pub maneuverability: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetectionFeature {
    pub at_level: u32,
    pub spell: String,
    pub spell_level: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorBonus {
    pub at_level: u32,
    pub descriptor: String,
    pub caster_level: i32,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorWeight {
    #[default]
    None,
    Light,
    Medium,
    Heavy,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShieldWeight {
    #[default]
    None,
    Light,
    Heavy,
    Tower,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArmorContext {
    pub armor: ArmorWeight,
    pub shield: ShieldWeight,
    pub armor_failure: u32,
    pub shield_failure: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TapestryTargets {
    pub recipients: Vec<String>,
    /// One casting decision, shared by every recipient.
    pub choice: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClassActionRequest {
    MetamagicTrigger {
        spell: String,
        item_creation_feat: String,
        metamagic: Vec<SpellRule>,
        charges: u32,
        base_charges: u32,
    },
    SacredDefense {
        target_kind: String,
        desecrated: bool,
    },
    SacredSpell {
        spell: String,
        spent: u32,
    },
    GrowWings {
        spent_seconds: u32,
        use_seconds: u32,
        armor_or_restrictive_clothing: bool,
        grappling: bool,
    },
    Exorcism {
        roll: u32,
        possessor_hit_dice: u32,
        possessor_charisma: i32,
    },
    Detect {
        spell: String,
    },
    WeaveTapestry {
        class_id: String,
        roll: Option<u32>,
        threatened: bool,
        distracted: bool,
        members: Vec<String>,
    },
}

/// One independently rolled damage packet; all formulas use the resolved spell CL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DamageProfile {
    pub dice: String,
    pub die_sides: u32,
    pub flat: String,
    pub packets: String,
    pub area: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureLevel {
    #[serde(default)]
    pub coverage: crate::build::FeatureCoverage,
    pub level: u32,
    pub name: String,
    pub summary: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuddenGrant {
    pub level: u32,
    pub feat: String,
    pub rule: SpellRule,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainAccess {
    pub level: u32,
    pub domain: String,
    pub spells: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DailyUseTier {
    pub level: u32,
    pub uses: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitedSpellFeature {
    pub spell: String,
    pub spell_level: u32,
    pub period: String,
    pub tiers: Vec<DailyUseTier>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProficiencyGrant {
    pub level: u32,
    pub weapon_groups: Vec<String>,
    pub armor: Vec<ArmorWeight>,
    pub shields: Vec<ShieldWeight>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetingRule {
    #[serde(default)]
    pub requires_willing: bool,
    pub living_only: bool,
    pub allied_only: bool,
    pub creature_types: Vec<String>,
    pub max_pair_distance_feet: Option<u32>,
    pub choices: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveKind {
    None,
    Fortitude,
    Reflex,
    Will,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveEffect {
    None,
    Negates,
    Half,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpellDefenseRule {
    pub save: SaveKind,
    pub on_save: SaveEffect,
    pub harmless: bool,
    pub spell_resistance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Energy {
    Acid,
    Cold,
    Electricity,
    Fire,
    Sonic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LearningRule {
    pub levels: Vec<u32>,
    pub lists: Vec<String>,
    pub schools: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureFormula {
    pub at_level: u32,
    pub name: String,
    pub values: BTreeMap<String, String>,
    #[serde(default)]
    pub daily_uses: Option<String>,
    #[serde(default)]
    pub resolution: Option<FeatureResolution>,
}

/// Executable class rules share encounter HP, saves and resource ledgers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeatureResolution {
    KnightChallenge {
        mode: crate::tactics::ChallengeMode,
    },
    BreathWeapon,
    QuickCast,
    Vitality {
        energy: VitalityEnergy,
        self_only: bool,
        touch: bool,
        radius_feet: Option<u32>,
        dice_sides: Option<u32>,
        save: SaveKind,
        points: bool,
        cap_half: bool,
    },
    FearAura {
        radius_feet: u32,
    },
    SurpriseFeint,
    Rage,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VitalityEnergy {
    Positive,
    Negative,
    Healing,
    LivingHealing,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellCastingTime {
    Immediate,
    Swift,
    Standard,
    FullRound,
    Longer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BonusFeatGrant {
    pub level: u32,
    pub feat: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurningModeRule {
    Turn,
    Rebuke,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BreathRule {
    pub energy: Energy,
    pub cone: bool,
}
