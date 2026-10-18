//! Persisted encounter inputs and class-specific transitions, separate from derived snapshots.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassPlay {
    #[serde(default)]
    pub combat_state: crate::combat::CombatState,
    #[serde(default)]
    pub tactics: crate::tactics::TacticalState,
    #[serde(default)]
    pub necromancy: crate::necromancy::NecromancyState,
    #[serde(default)]
    pub encounter: AbilityEncounter,
    #[serde(default)]
    pub concentration_checks: BTreeMap<String, ConcentrationCheck>,
    #[serde(default)]
    pub health: BTreeMap<String, HitPoints>,
    #[serde(default)]
    pub protections: BTreeMap<String, Protections>,
    #[serde(default)]
    pub cast_effects: BTreeMap<String, CastEffect>,
    #[serde(default)]
    pub defenses: BTreeMap<String, SpellDefenses>,
    #[serde(default)]
    pub resources: ClassResources,
    #[serde(default)]
    pub actors: BTreeMap<String, SceneActor>,
    pub air_spent: u32,
    pub air_commands: BTreeMap<String, u32>,
    pub air_effects: BTreeMap<String, AirEffect>,
    pub spells: BTreeMap<String, ControlledSpell>,
    /// Incremented on every committed transition to reject duplicate/stale form submissions.
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityEncounter {
    #[serde(default)]
    pub cooldowns: BTreeMap<String, u64>,
    pub seconds: u64,
    pub combat: BTreeMap<String, CombatFacts>,
    pub conditions: Vec<EncounterCondition>,
    pub last_used: BTreeMap<String, u64>,
    pub rage: Option<RageState>,
    pub rage_used: bool,
    pub fatigued: bool,
    pub feints: BTreeMap<String, u64>,
    pub fear_immunity: BTreeMap<String, u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CombatFacts {
    pub fear_immune: bool,
    pub touch_ac: i32,
    pub hit_dice: u32,
    pub intelligence: u32,
    pub sense_motive: i32,
    pub base_attack_bonus: i32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncounterCondition {
    pub aura_radius_feet: Option<u32>,
    pub source: String,
    pub target: String,
    pub name: String,
    pub remaining_seconds: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RageState {
    pub remaining_seconds: u32,
    pub ability_bonus: i32,
    pub will_bonus: i32,
    pub hp_bonus: i64,
    pub tireless: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureRequest {
    #[serde(default)]
    pub affliction: crate::necromancy::AfflictionRolls,
    pub class_id: String,
    pub feature: String,
    pub target: String,
    pub roll: Option<u32>,
    pub opposed_roll: Option<u32>,
    pub amount: u32,
    pub damage_roll: Option<i64>,
    pub saves: BTreeMap<String, u32>,
    pub massive_rolls: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainLevelRuling {
    pub level: u32,
    pub reason: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlledSpell {
    pub spell: String,
    pub original_caster: String,
    pub original_caster_level: u32,
    pub spell_level: u32,
    pub original_divine: bool,
    pub concentration: bool,
    pub controllable: bool,
    pub dismissible: bool,
    /// None means no fixed duration limit; the concentration/tail rule still applies.
    pub remaining_rounds: Option<u32>,
    pub concentration_tail_rounds: u32,
    pub tail_remaining: Option<u32>,
    /// Variables fixed at original casting, never recomputed for the new controller.
    pub variables: BTreeMap<String, String>,
    pub controller: SpellController,
    pub control_mode: Option<ControlMode>,
    pub awaiting_reassertion: bool,
    pub ended: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellController {
    Original,
    Incantatrix,
    None,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlMode {
    Seize,
    Snatch,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlEvent {
    Acquire {
        mode: ControlMode,
        track: String,
        distance_feet: u32,
        willing: bool,
        roll: u32,
        original_roll: u32,
    },
    Maintain,
    Lapse,
    Reassert {
        roll: u32,
    },
    DeclineReassertion,
    Dismiss {
        track: String,
        roll: u32,
        original_roll: u32,
    },
    AdvanceTime {
        rounds: u32,
    },
    Dispel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AirMode {
    TurnEarth,
    RebukeAir,
    BolsterAir,
    DispelTurning,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirTarget {
    pub id: String,
    pub hit_dice: u32,
    pub turning_resistance: u32,
    pub distance_feet: u32,
    pub total_cover: bool,
    pub air: bool,
    pub earth: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirRequest {
    pub mode: AirMode,
    pub roll: u32,
    pub damage_die_1: u32,
    pub damage_die_2: u32,
    pub targets: Vec<AirTarget>,
    pub original_turning_check: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AirEffect {
    pub outcome: String,
    pub rounds: u32,
    pub effective_hit_dice: Option<u32>,
}

/// Resource expenditure survives page loads. Refreshes are explicit calendar events.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassResources {
    #[serde(default)]
    pub prepared_spent: BTreeMap<String, u32>,
    pub daily: BTreeMap<String, u32>,
    pub weekly: BTreeMap<String, u32>,
    pub wings_seconds: u32,
    /// Victim IDs protected from the same possessor; seconds remaining.
    pub exorcised: BTreeMap<String, BTreeMap<String, u32>>,
    pub tapestry: Option<TapestryState>,
    pub slots_spent: BTreeMap<String, BTreeMap<u32, u32>>,
    pub turning_spent: u32,
    pub instant_spent: u32,
    pub trigger_items: BTreeMap<String, TriggerItem>,
    pub sudden_spent: BTreeMap<String, u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TapestryState {
    pub casting_track: String,
    pub members: Vec<String>,
    pub remaining_seconds: u32,
    pub stored: Vec<StoredWeaving>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredWeaving {
    pub spell: String,
    pub caster_level: i32,
    pub save_dc: i32,
    pub effective_level: i32,
    pub duration: Option<String>,
    pub range: crate::magic::SpellRange,
    pub range_feet: Option<u32>,
    pub metamagic: Vec<crate::magic::SpellRule>,
    pub targets: crate::magic::TapestryTargets,
    pub material_cost_gp: u32,
    pub xp_cost: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerItem {
    pub powers: BTreeMap<String, TriggerPower>,
    pub kind: TriggerKind,
    pub caster_level: u32,
    pub casting_track: String,
    pub charges: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TriggerPower {
    pub spell: String,
    pub spell_list: String,
    pub base_charges: u32,
    /// A pre-heightened power's effective spell level, as specified by the item.
    pub heightened_level: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    Wand,
    Staff,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetBody {
    Living,
    Undead,
    Construct,
    LivingConstruct,
    Object,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetPath {
    Clear,
    Obscured,
    Solid,
    TransparentSolid,
}
/// Current encounter facts, not user-supplied rules verdicts. Coordinates are
/// occupied 5-foot squares on a planar battle grid; `self` identifies the caster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneActor {
    pub body: TargetBody,
    pub creature_type: String,
    pub dead: bool,
    pub allied: bool,
    pub willing: bool,
    pub unconscious: bool,
    pub x: i32,
    pub y: i32,
    pub space: u32,
    pub touch_reach_feet: u32,
    /// Current path from caster to actor. Fog blocks sight but not effect;
    /// glass blocks effect but not sight. This is an environmental fact.
    pub path: TargetPath,
}
impl SceneActor {
    pub fn living(&self) -> bool {
        !self.dead && matches!(self.body, TargetBody::Living | TargetBody::LivingConstruct)
    }
    pub fn willing(&self) -> bool {
        self.willing || self.unconscious
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellDefenses {
    pub fortitude: i32,
    pub reflex: i32,
    pub will: i32,
    pub spell_resistance: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CastEffect {
    pub source_class: Option<String>,
    pub requires_touch: bool,
    pub contact_sequence: u32,
    pub duration_multiplier: u32,
    #[serde(default)]
    pub impact: Option<crate::combat::TouchImpact>,
    #[serde(default)]
    pub damage_kind: Option<crate::combat::DamageKind>,
    #[serde(default)]
    pub targeting: Option<crate::magic::TargetingRule>,
    #[serde(default)]
    pub hit_point_damage: Option<bool>,
    #[serde(default)]
    pub undead_immunity: Option<bool>,
    #[serde(default)]
    pub bypass_sr: bool,
    pub energy: Option<crate::magic::Energy>,
    pub round: u32,
    pub rounds: u32,
    pub ended: bool,
    pub bound_target: Option<String>,
    pub sr_results: BTreeMap<String, bool>,
    pub spell: String,
    pub caster_level: i32,
    pub save_dc: i32,
    pub penetration_bonus: i32,
    pub defense: Option<crate::magic::SpellDefenseRule>,
    pub damage_bounds: Option<(i64, i64)>,
    pub shared_damage: bool,
    pub shared_damage_roll: Option<i64>,
    pub resolved: BTreeMap<String, DefenseOutcome>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefenseOutcome {
    pub resisted: bool,
    pub saved: Option<bool>,
    pub damage: Option<i64>,
    pub effect: String,
    pub facts: Vec<String>,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DefenseRequest {
    #[serde(default)]
    pub effect_roll: Option<u32>,
    #[serde(default)]
    pub dispel_rolls: BTreeMap<String, u32>,
    #[serde(default)]
    pub destination: Option<(i32, i32)>,
    #[serde(default)]
    pub destination_path: Option<TargetPath>,
    pub massive_roll: Option<u32>,
    pub target: String,
    pub sr_roll: Option<u32>,
    pub save_roll: Option<u32>,
    pub waive_save: bool,
    pub sr_lowered: bool,
    pub damage_roll: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitPoints {
    pub maximum: i64,
    pub current: i64,
    pub temporary: i64,
    pub stable: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protections {
    pub resistance: BTreeMap<crate::magic::Energy, u32>,
    pub immunity: Vec<crate::magic::Energy>,
    /// SR-style spell immunity. Does not stop spells which do not allow SR.
    pub spell_immunity: bool,
    pub evasion: Evasion,
    pub armor_blocks_evasion: bool,
    pub helpless: bool,
    pub bound_or_squeezing: bool,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Evasion {
    #[default]
    None,
    Evasion,
    Improved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcentrationCheck {
    pub spell: String,
    pub dc: i64,
    pub success: Option<bool>,
    pub fact: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyImmunity {
    MindAffecting,
    Poison,
    Sleep,
    Paralysis,
    Stunning,
    Disease,
    DeathEffects,
    CriticalHits,
    NonlethalDamage,
    AbilityDrain,
    EnergyDrain,
    PhysicalAbilityDamage,
    Fatigue,
    Exhaustion,
    FortitudeUnlessObjectOrHarmless,
    MassiveDamage,
}
impl TargetBody {
    pub fn immunities(self) -> &'static [BodyImmunity] {
        use BodyImmunity::*;
        if self == Self::Undead {
            &[
                MindAffecting,
                Poison,
                Sleep,
                Paralysis,
                Stunning,
                Disease,
                DeathEffects,
                CriticalHits,
                NonlethalDamage,
                AbilityDrain,
                EnergyDrain,
                PhysicalAbilityDamage,
                Fatigue,
                Exhaustion,
                FortitudeUnlessObjectOrHarmless,
                MassiveDamage,
            ]
        } else {
            &[]
        }
    }
}
