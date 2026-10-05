//! Derived character output.

use pcgen_core::{
    BuildProvenance, Contribution, CreatureTraits, DomainSpell, FaithDecision, GestaltPlan,
    SpellPowerType,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Derived state for one companion owned by the character.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompanionSnapshot {
    pub companion_type: String,
    pub master_level: u32,
    #[serde(default)]
    pub race_id: Option<String>,
    #[serde(default)]
    pub hit_dice_bonus: i32,
    #[serde(default)]
    pub spell_resistance: Option<i32>,
    #[serde(default)]
    pub abilities: Vec<String>,
    /// `BONUS:COMBAT|AC` total after stacking/aggregation.
    #[serde(default)]
    pub armor_class_bonus: i32,
    /// `BONUS:STAT|...` adjustments keyed by stat name.
    #[serde(default)]
    pub stat_adjustments: BTreeMap<String, i32>,
    /// Creature traits granted to the companion.
    #[serde(default)]
    pub traits: CreatureTraits,
}

/// One ability category's pool usage.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityPoolUsage {
    pub pool: i32,
    pub spent: i32,
    pub remaining: i32,
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub selection_counts: BTreeMap<String, u32>,
}

/// One numeric spell effect that was applied to a character value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpellEffectApplication {
    pub spell: String,
    #[serde(default)]
    pub class_id: Option<String>,
    pub target: String,
    pub value: i32,
}

/// Runtime-visible class interaction metadata (`EXCLASS` / `EXCHANGELEVEL`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassInteraction {
    pub class_id: String,
    #[serde(default)]
    pub excluded_class: Option<String>,
    #[serde(default)]
    pub exchange_level: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    pub draft_id: String,
    pub name: String,
    pub race_id: String,
    pub total_level: u32,
    /// Canonical size rank used for size-derived values (`SIZENUM` aware).
    #[serde(default)]
    pub size_rank: usize,
    pub stats: BTreeMap<String, i32>,
    pub hit_points: i32,
    /// Total skill points from class levels, including first-level
    /// `SKILLMULT` and race `XTRASKILLPTSPERLVL`.
    #[serde(default)]
    pub skill_points: i32,
    /// Feat slots derived from class `LEVELSPERFEAT` / default progression.
    #[serde(default)]
    pub feat_slots: u32,
    pub base_attack_bonus: i32,
    /// Primary melee attack total (BAB + STR + size); see L3b-4 scope note.
    #[serde(default)]
    pub melee_attack: i32,
    /// Primary ranged attack total (BAB + DEX + size).
    #[serde(default)]
    pub ranged_attack: i32,
    /// Grapple total (BAB + STR + special size modifier).
    #[serde(default)]
    pub grapple_attack: i32,
    /// `EXCLASS` / `EXCHANGELEVEL` metadata for active classes.
    #[serde(default)]
    pub class_interactions: Vec<ClassInteraction>,
    /// `PROHIBITCOST` catalog keyed by source record key.
    #[serde(default)]
    pub prohibited_costs: BTreeMap<String, i32>,
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
    /// Max caster level contribution from active domain grants such as
    /// ``DOMAINGRANTS:You cast law spells at +1 caster level.``.
    #[serde(default)]
    pub caster_level_bonus: i32,
    /// Prepared spell count from `BOOK:Prepared*` only; the `prepared_spells`
    /// map keeps the broader spell-effect activation semantics.
    #[serde(default)]
    pub explicit_prepared_count: u32,
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
    /// Psionic powers available/known per manifester class at current level.
    #[serde(default)]
    pub powers_known: BTreeMap<String, Vec<String>>,
    /// `ABILITYLIST` membership keyed by ability-category id.
    #[serde(default)]
    pub ability_lists: BTreeMap<String, Vec<String>>,
    /// `POOL` points keyed by ability-category id, resolved from variables.
    #[serde(default)]
    pub ability_pools: BTreeMap<String, i32>,
    /// Pool spending per ability category.
    #[serde(default)]
    pub ability_pool_usage: BTreeMap<String, AbilityPoolUsage>,
    /// Ability ids marked repeatable via `STACK` / `MULT`.
    #[serde(default)]
    pub repeatable_abilities: Vec<String>,
    /// Known languages, sorted.
    #[serde(default)]
    pub languages: Vec<String>,
    /// Subclass `CHOICE:` expressions keyed by class id.
    #[serde(default)]
    pub class_choices: BTreeMap<String, String>,
    /// Deity-allowed domain catalog (`DEITYDOMAINS`), validation only.
    #[serde(default)]
    pub deity_domains: Vec<String>,
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
    /// Iterative attack cycle entries (`ATTACKCYCLE`).
    #[serde(default)]
    pub attack_cycle: Vec<String>,
    /// Movement modes in feet (lower-cased mode -> rate), after race + active
    /// `MOVECLONE` expansion.
    #[serde(default)]
    pub movement: BTreeMap<String, u32>,
    /// Resolved equipment definitions after base-item / EQMOD resolution.
    #[serde(default)]
    pub equipment: Vec<pcgen_core::EquipmentDefinition>,
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
    /// Numeric spell effects that were applied to AC / saves / stats /
    /// skills, keyed by `"<target>:<spell>"`.
    #[serde(default)]
    pub applied_spell_effects: BTreeMap<String, Vec<SpellEffectApplication>>,
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
    /// Choice values bound to the entity that owned the `CHOOSE`.
    #[serde(default)]
    pub choice_bindings: BTreeMap<String, Vec<String>>,
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
    /// Turn/rebuke undead state; `None` when no class grants the ability.
    #[serde(default)]
    pub turning: Option<TurningSnapshot>,
}

/// Turn or rebuke undead.
///
/// SRD, Special Attacks (<https://www.d20srd.org/srd/combat/specialAttacks.htm>):
///
/// > Good clerics and paladins and some neutral clerics can channel positive
/// > energy, which can halt, drive off (rout), or destroy undead.
/// > Evil clerics and some neutral clerics can channel negative energy, which
/// > can halt, awe (rebuke), control (command), or bolster undead.
///
/// The SRD never distinguishes between individual deities — only alignment
/// matters, so no deity-specific data is needed to decide the mode.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurningSnapshot {
    /// Effective turning level.  A cleric uses her cleric level; a paladin
    /// turns "as a cleric of three levels lower", and a multiclass character
    /// uses "her cleric level plus her paladin level minus 3".
    pub level: u32,
    /// Positive or negative energy, or a choice the player still has to make.
    pub mode: TurningMode,
    /// Attempts per day: `3 + Charisma modifier`.
    pub times_per_day: i32,
    /// Turning damage is `2d6 + this` (cleric level + Charisma modifier).
    pub damage_bonus: i32,
    /// Why the mode is [`TurningMode::Choice`], when the SRD's answer depends
    /// on data the pack does not carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Which kind of energy a character channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurningMode {
    /// Positive energy: halt, drive off, or destroy undead.
    Turn,
    /// Negative energy: awe (rebuke) or command undead.
    Rebuke,
    /// A neutral cleric of a neutral deity "must choose whether his turning
    /// ability functions as that of a good cleric or an evil cleric. Once this
    /// choice is made, it cannot be reversed."
    Choice,
}

/// Where an alignment falls on the good–evil axis.
///
/// Derived from the alignment id (`LG`, `NG`, ... `CE`) rather than from the
/// pack, because [`pcgen_core::AlignmentDefinition`] carries only
/// deity/follower validity flags and no morality axis.
pub fn alignment_morality(alignment: &str) -> Option<bool> {
    let value = alignment.trim().to_ascii_uppercase();
    // The nine standard ids are two characters, and the second is the
    // morality: `G`ood, `N`eutral, `E`vil.
    if value.len() == 2 {
        return match value.as_bytes()[1] as char {
            'G' => Some(true),
            'E' => Some(false),
            _ => None,
        };
    }
    // Spelled-out names (`Lawful Good`) end with the morality word.
    if value.ends_with("GOOD") {
        Some(true)
    } else if value.ends_with("EVIL") {
        Some(false)
    } else {
        None
    }
}
