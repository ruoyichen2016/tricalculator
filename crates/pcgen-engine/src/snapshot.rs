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
    pub exchange_level: Option<pcgen_core::ExchangeLevel>,
}

/// Validity applies to the complete derived result until field-level dependency
/// validity is available. Diagnostics must never coexist with usable partial
/// numbers at an output boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SnapshotValidity<'a> {
    Available,
    Unavailable { reasons: &'a [String] },
}

impl CharacterSnapshot {
    pub fn is_available(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn validity(&self) -> SnapshotValidity<'_> {
        if self.is_available() {
            SnapshotValidity::Available
        } else {
            SnapshotValidity::Unavailable {
                reasons: &self.diagnostics,
            }
        }
    }

    /// Read a computed candidate only after checking the entire evaluation.
    pub fn available<T>(&self, value: T) -> Option<T> {
        self.is_available().then_some(value)
    }
}

// Serialize at the model boundary, so JSON exports and direct API serialization
// cannot accidentally publish partial candidates. Identity and entered level
// remain readable; all computed fields are null when evaluation is unavailable.
impl Serialize for CharacterSnapshot {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut output = serializer.serialize_struct("CharacterSnapshot", 80)?;
        output.serialize_field("validity", &self.validity())?;
        output.serialize_field("rule_gaps", &self.rule_gaps)?;
        output.serialize_field("spell_access", &self.available(&self.spell_access))?;
        output.serialize_field("draft_id", &self.draft_id)?;
        output.serialize_field("name", &self.name)?;
        output.serialize_field("race_id", &self.race_id)?;
        output.serialize_field("creature_type", &self.available(&self.creature_type))?;
        output.serialize_field("body_immunities", &self.available(&self.body_immunities))?;
        output.serialize_field("total_level", &self.total_level)?;
        output.serialize_field("size_rank", &self.available(&self.size_rank))?;
        output.serialize_field("stats", &self.available(&self.stats))?;
        output.serialize_field("hit_points", &self.available(&self.hit_points))?;
        output.serialize_field("skill_points", &self.available(&self.skill_points))?;
        output.serialize_field("feat_slots", &self.available(&self.feat_slots))?;
        output.serialize_field(
            "base_attack_bonus",
            &self.available(&self.base_attack_bonus),
        )?;
        output.serialize_field("melee_attack", &self.available(&self.melee_attack))?;
        output.serialize_field("ranged_attack", &self.available(&self.ranged_attack))?;
        output.serialize_field("grapple_attack", &self.available(&self.grapple_attack))?;
        output.serialize_field(
            "class_interactions",
            &self.available(&self.class_interactions),
        )?;
        output.serialize_field("prohibited_costs", &self.available(&self.prohibited_costs))?;
        output.serialize_field("initiative", &self.available(&self.initiative))?;
        output.serialize_field("armor_class", &self.available(&self.armor_class))?;
        output.serialize_field("armor_bonus", &self.available(&self.armor_bonus))?;
        output.serialize_field("shield_bonus", &self.available(&self.shield_bonus))?;
        output.serialize_field("touch_ac", &self.available(&self.touch_ac))?;
        output.serialize_field("flat_footed_ac", &self.available(&self.flat_footed_ac))?;
        output.serialize_field("saves", &self.available(&self.saves))?;
        output.serialize_field("skills", &self.available(&self.skills))?;
        output.serialize_field("attack_bonuses", &self.available(&self.attack_bonuses))?;
        output.serialize_field("caster_levels", &self.available(&self.caster_levels))?;
        output.serialize_field(
            "caster_level_bonus",
            &self.available(&self.caster_level_bonus),
        )?;
        output.serialize_field(
            "explicit_prepared_count",
            &self.available(&self.explicit_prepared_count),
        )?;
        output.serialize_field("spell_dcs", &self.available(&self.spell_dcs))?;
        output.serialize_field("spell_slots", &self.available(&self.spell_slots))?;
        output.serialize_field("known_limits", &self.available(&self.known_limits))?;
        output.serialize_field("prepared_spells", &self.available(&self.prepared_spells))?;
        output.serialize_field(
            "spell_power_types",
            &self.available(&self.spell_power_types),
        )?;
        output.serialize_field("max_spell_level", &self.available(&self.max_spell_level))?;
        output.serialize_field("has_spellbook", &self.available(&self.has_spellbook))?;
        output.serialize_field(
            "prohibited_spells",
            &self.available(&self.prohibited_spells),
        )?;
        output.serialize_field("power_points", &self.available(&self.power_points))?;
        output.serialize_field(
            "manifester_levels",
            &self.available(&self.manifester_levels),
        )?;
        output.serialize_field("powers_known", &self.available(&self.powers_known))?;
        output.serialize_field("ability_lists", &self.available(&self.ability_lists))?;
        output.serialize_field("ability_pools", &self.available(&self.ability_pools))?;
        output.serialize_field(
            "ability_pool_usage",
            &self.available(&self.ability_pool_usage),
        )?;
        output.serialize_field(
            "repeatable_abilities",
            &self.available(&self.repeatable_abilities),
        )?;
        output.serialize_field("languages", &self.available(&self.languages))?;
        output.serialize_field("class_choices", &self.available(&self.class_choices))?;
        output.serialize_field("deity_domains", &self.available(&self.deity_domains))?;
        output.serialize_field("vision", &self.available(&self.vision))?;
        output.serialize_field("damage_reduction", &self.available(&self.damage_reduction))?;
        output.serialize_field("spell_resistance", &self.available(&self.spell_resistance))?;
        output.serialize_field("natural_attacks", &self.available(&self.natural_attacks))?;
        output.serialize_field("reach_feet", &self.available(&self.reach_feet))?;
        output.serialize_field("reach_mult", &self.available(&self.reach_mult))?;
        output.serialize_field("unarmed_damage", &self.available(&self.unarmed_damage))?;
        output.serialize_field("unarmed_mult", &self.available(&self.unarmed_mult))?;
        output.serialize_field("attack_cycle", &self.available(&self.attack_cycle))?;
        output.serialize_field("movement", &self.available(&self.movement))?;
        output.serialize_field("equipment", &self.available(&self.equipment))?;
        output.serialize_field("companions", &self.available(&self.companions))?;
        output.serialize_field("gestalt", &self.available(&self.gestalt))?;
        output.serialize_field("faith_decisions", &self.available(&self.faith_decisions))?;
        output.serialize_field("domain_spells", &self.available(&self.domain_spells))?;
        output.serialize_field("spell_effects", &self.available(&self.spell_effects))?;
        output.serialize_field(
            "applied_spell_effects",
            &self.available(&self.applied_spell_effects),
        )?;
        output.serialize_field("variant_effects", &self.available(&self.variant_effects))?;
        output.serialize_field("explanations", &self.available(&self.explanations))?;
        output.serialize_field("diagnostics", &self.diagnostics)?;
        output.serialize_field("variables", &self.available(&self.variables))?;
        output.serialize_field(
            "external_modifiers",
            &self.available(&self.external_modifiers),
        )?;
        output.serialize_field("active_abilities", &self.available(&self.active_abilities))?;
        output.serialize_field("choices", &self.available(&self.choices))?;
        output.serialize_field("choice_bindings", &self.available(&self.choice_bindings))?;
        output.serialize_field("active_states", &self.available(&self.active_states))?;
        output.serialize_field(
            "unresolved_choices",
            &self.available(&self.unresolved_choices),
        )?;
        output.serialize_field("applied_effects", &self.available(&self.applied_effects))?;
        output.serialize_field("provenance", &self.available(&self.provenance))?;
        output.serialize_field("turning", &self.available(&self.turning))?;
        output.serialize_field("uncanny_dodge", &self.available(&self.uncanny_dodge))?;
        output.end()
    }
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CharacterSnapshot {
    pub creature_type: Option<String>,
    pub body_immunities: Vec<pcgen_core::class_play::BodyImmunity>,
    /// Known unimplemented native rules; distinct from numeric evaluation failures.
    pub spell_access: BTreeMap<String, Vec<crate::magic::AccessibleSpell>>,
    pub rule_gaps: BTreeMap<String, String>,
    pub draft_id: String,
    pub name: String,
    pub race_id: String,
    pub total_level: u32,
    /// Canonical size rank used for size-derived values (`SIZENUM` aware).
    pub size_rank: usize,
    pub stats: BTreeMap<String, i32>,
    pub hit_points: i32,
    /// Total skill points from class levels, including first-level
    /// `SKILLMULT` and race `XTRASKILLPTSPERLVL`.
    pub skill_points: i32,
    /// Feat slots derived from class `LEVELSPERFEAT` / default progression.
    pub feat_slots: u32,
    pub base_attack_bonus: i32,
    /// Primary melee attack total (BAB + STR + size); see L3b-4 scope note.
    pub melee_attack: i32,
    /// Primary ranged attack total (BAB + DEX + size).
    pub ranged_attack: i32,
    /// Grapple total (BAB + STR + special size modifier).
    pub grapple_attack: i32,
    /// `EXCLASS` / `EXCHANGELEVEL` metadata for active classes.
    pub class_interactions: Vec<ClassInteraction>,
    /// `PROHIBITCOST` catalog keyed by source record key.
    pub prohibited_costs: BTreeMap<String, i32>,
    pub initiative: i32,
    pub armor_class: i32,
    pub armor_bonus: i32,
    pub shield_bonus: i32,
    pub touch_ac: i32,
    pub flat_footed_ac: i32,
    pub saves: BTreeMap<String, i32>,
    pub skills: BTreeMap<String, i32>,
    /// Typed attack bonus totals keyed by canonical target (e.g.
    /// `COMBAT.TOHIT`, `WEAPON.TOHIT`, `WEAPONPROF=Longbow.TOHIT`).
    pub attack_bonuses: BTreeMap<String, i32>,
    pub caster_levels: BTreeMap<String, i32>,
    /// Max caster level contribution from active domain grants such as
    /// ``DOMAINGRANTS:You cast law spells at +1 caster level.``.
    pub caster_level_bonus: i32,
    /// Prepared spell count from `BOOK:Prepared*` only; the `prepared_spells`
    /// map keeps the broader spell-effect activation semantics.
    pub explicit_prepared_count: u32,
    pub spell_dcs: BTreeMap<String, i32>,
    pub spell_slots: BTreeMap<String, u32>,
    pub known_limits: BTreeMap<String, u32>,
    pub prepared_spells: BTreeMap<String, u32>,
    /// Arcane / Divine / Psionic powers the character can cast.
    pub spell_power_types: BTreeSet<SpellPowerType>,
    /// Highest spell level with a slot, per power type.
    pub max_spell_level: BTreeMap<SpellPowerType, u32>,
    /// Character has a spellbook-using class.
    pub has_spellbook: bool,
    /// Spell prohibitions that apply to this character.
    ///
    /// Kept structured rather than flattened to name sets: `ALIGNMENT.Chaotic.Evil`
    /// means a spell must carry *both* descriptors, which a set of names cannot
    /// express. A 3.5e Wizard must forbid two schools, and a Cleric of an evil
    /// deity does not cast Good spells; until this existed the picker offered
    /// both anyway.
    pub prohibited_spells: Vec<pcgen_core::ProhibitedSpell>,
    pub power_points: BTreeMap<String, i32>,
    pub manifester_levels: BTreeMap<String, i32>,
    /// Psionic powers available/known per manifester class at current level.
    pub powers_known: BTreeMap<String, Vec<String>>,
    /// `ABILITYLIST` membership keyed by ability-category id.
    pub ability_lists: BTreeMap<String, Vec<String>>,
    /// `POOL` points keyed by ability-category id, resolved from variables.
    pub ability_pools: BTreeMap<String, i32>,
    /// Pool spending per ability category.
    pub ability_pool_usage: BTreeMap<String, AbilityPoolUsage>,
    /// Ability ids marked repeatable via `STACK` / `MULT`.
    pub repeatable_abilities: Vec<String>,
    /// Known languages, sorted.
    pub languages: Vec<String>,
    /// Subclass `CHOICE:` expressions keyed by class id.
    pub class_choices: BTreeMap<String, String>,
    /// Deity-allowed domain catalog (`DEITYDOMAINS`), validation only.
    pub deity_domains: Vec<String>,
    /// Creature vision modes from race + active templates (`VISION`).
    pub vision: Vec<String>,
    /// Damage reduction entries from race + active templates (`DR`).
    pub damage_reduction: Vec<String>,
    /// Spell resistance from race + active templates (`SR`).
    pub spell_resistance: Option<i32>,
    /// Natural attack entries from race + active templates (`NATURALATTACKS`).
    pub natural_attacks: Vec<String>,
    /// Natural reach in feet (`REACH`).
    pub reach_feet: Option<i32>,
    /// Reach multiplier (`REACHMULT`).
    pub reach_mult: Option<i32>,
    /// Unarmed damage progression entries (`UDAM`).
    pub unarmed_damage: Vec<String>,
    /// Unarmed damage multiplier (`UMULT`).
    pub unarmed_mult: Option<i32>,
    /// Iterative attack cycle entries (`ATTACKCYCLE`).
    pub attack_cycle: Vec<String>,
    /// Movement modes in feet (lower-cased mode -> rate), after race + active
    /// `MOVECLONE` expansion.
    pub movement: BTreeMap<String, u32>,
    /// Resolved equipment definitions after base-item / EQMOD resolution.
    pub equipment: Vec<pcgen_core::EquipmentDefinition>,
    /// Derived companion states (familiar / special mount / ...).
    pub companions: Vec<CompanionSnapshot>,
    pub gestalt: Option<GestaltPlan>,
    pub faith_decisions: Vec<FaithDecision>,
    pub domain_spells: BTreeMap<String, Vec<DomainSpell>>,
    /// Character-facing spell effect summary: domain spells, prepared spells.
    pub spell_effects: Vec<String>,
    /// Numeric spell effects that were applied to AC / saves / stats /
    /// skills, keyed by `"<target>:<spell>"`.
    pub applied_spell_effects: BTreeMap<String, Vec<SpellEffectApplication>>,
    pub variant_effects: BTreeMap<String, Vec<String>>,
    pub explanations: BTreeMap<String, Vec<Contribution>>,
    pub diagnostics: Vec<String>,
    pub variables: BTreeMap<String, i32>,
    pub external_modifiers: BTreeMap<String, i32>,
    pub active_abilities: Vec<String>,
    pub choices: BTreeMap<String, String>,
    /// Choice values bound to the entity that owned the `CHOOSE`.
    pub choice_bindings: BTreeMap<String, Vec<String>>,
    /// Explicitly activated temporary states.
    pub active_states: Vec<String>,
    pub unresolved_choices: Vec<String>,
    pub applied_effects: usize,
    /// Build provenance: rule-pack fingerprint plus upstream data revision.
    pub provenance: BuildProvenance,
    /// Turn/rebuke undead state; `None` when no class grants the ability.
    pub turning: Option<TurningSnapshot>,
    /// Uncanny dodge state; `None` when no class grants it.
    pub uncanny_dodge: Option<UncannyDodgeSnapshot>,
}

/// Uncanny dodge and improved uncanny dodge.
///
/// SRD, Multiclass Characters
/// (<https://www.d20srd.org/srd/classes/multiclass.htm>):
///
/// > In the special case of uncanny dodge, both experienced barbarians and
/// > experienced rogues have the same ability. When a barbarian/rogue would
/// > gain uncanny dodge a second time (for her second class), she instead
/// > gains improved uncanny dodge, if she does not already have it. Her
/// > barbarian and rogue levels stack to determine the rogue level an attacker
/// > needs to flank her.
///
/// The classes themselves grant the two abilities at their own levels
/// (barbarian 2/5, rogue 4/8, and the prestige classes likewise); what the data
/// does not state is that a *second* granting class converts its uncanny dodge
/// into improved uncanny dodge, so that is decided here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UncannyDodgeSnapshot {
    /// Retains the Dexterity bonus to AC when flat-footed or struck by an
    /// invisible attacker.
    pub uncanny_dodge: bool,
    /// Can no longer be flanked.
    pub improved: bool,
    /// Levels in the classes that grant uncanny dodge, stacked.  This is the
    /// level an attacker's rogue level must exceed by four to flank.
    pub flanking_level: u32,
    /// True when the improved ability came from a *second* granting class
    /// rather than from a class that grants improved uncanny dodge directly.
    pub improved_from_second_class: bool,
    /// The classes that grant each ability, for explainability.
    #[serde(default)]
    pub uncanny_dodge_classes: Vec<String>,
    #[serde(default)]
    pub improved_classes: Vec<String>,
}

/// Split the classes that grant uncanny dodge from those that grant improved
/// uncanny dodge.
///
/// Active abilities are keyed `"<class>_~_<ability>"`, e.g.
/// `barbarian_~_uncanny_dodge`; comparing the ability part exactly avoids the
/// trap that `"uncanny_dodge"` is a substring of `"improved_uncanny_dodge"`.
pub fn uncanny_dodge_classes(active_abilities: &[String]) -> (Vec<String>, Vec<String>) {
    let mut uncanny: Vec<String> = Vec::new();
    let mut improved: Vec<String> = Vec::new();
    for ability in active_abilities {
        let (class_key, ability_key) = match ability.split_once("_~_") {
            Some((class_key, ability_key)) => (class_key, ability_key),
            None => continue,
        };
        if class_key.trim().is_empty() {
            continue;
        }
        let target = match ability_key {
            "improved_uncanny_dodge" => &mut improved,
            "uncanny_dodge" => &mut uncanny,
            _ => continue,
        };
        let class_key = class_key.replace('_', " ");
        if !target.contains(&class_key) {
            target.push(class_key);
        }
    }
    uncanny.sort();
    improved.sort();
    (uncanny, improved)
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
