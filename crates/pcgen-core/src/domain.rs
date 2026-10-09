//! Typed rule definitions produced by the import layer.
//!
//! These are plain data types: the import crate parses PCGen records into
//! them, and the engine consumes [`ClassProgression`] tables.  They contain
//! no PCGen syntax and no character state.

use crate::gestalt::PrestigeSlotCost;
use crate::variants::VariantDefinition;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// d20 base attack bonus progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackProgression {
    Full,
    ThreeQuarter,
    Half,
    Custom,
    /// No base attack progression.  PCGen classes only gain BAB when they
    /// declare an explicit `BONUS:COMBAT|BASEAB` (or a `BAB` shorthand).
    #[default]
    None,
}

impl AttackProgression {
    pub fn base_attack_bonus(self, level: u32) -> i32 {
        let level = level as i32;
        match self {
            Self::Full => level,
            Self::ThreeQuarter => (level * 3) / 4,
            Self::Half => level / 2,
            Self::Custom => level,
            Self::None => 0,
        }
    }
}

/// d20 saving throw progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveProgression {
    Good,
    Poor,
    /// No base save progression.  PCGen classes only gain base saves when
    /// they declare an explicit `BONUS:SAVE|BASE.*` (or a `SAVE` shorthand).
    #[default]
    None,
}

impl SaveProgression {
    pub fn base_save(self, level: u32) -> i32 {
        let level = level as i32;
        match self {
            Self::Good => 2 + level / 2,
            Self::Poor => level / 3,
            Self::None => 0,
        }
    }
}

/// Spellcasting acquisition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CastingType {
    Prepared,
    Spontaneous,
    Innate,
    #[default]
    Unknown,
}

/// Source of a spellcasting class's magic (PCGen `FACT:SpellType|...`).
///
/// This is orthogonal to [`CastingType`]: `CastingType` says *how* a class
/// casts (prepared / spontaneous), `SpellPowerType` says *what* it casts
/// (arcane / divine / psionic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpellPowerType {
    Arcane,
    Divine,
    Psionic,
    #[default]
    Unknown,
}

impl SpellPowerType {
    /// Parse a PCGen `FACT:SpellType|...` value.  Returns `None` for values we
    /// do not model so callers can keep them as unsupported instead of
    /// guessing.
    pub fn from_pcgen(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "arcane" => Some(Self::Arcane),
            "divine" => Some(Self::Divine),
            "psionic" => Some(Self::Psionic),
            "unknown" | "" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Race definition.
/// Creature traits shared by races and templates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CreatureTraits {
    #[serde(default)]
    pub reach_feet: Option<i32>,
    #[serde(default)]
    pub reach_mult: Option<i32>,
    #[serde(default)]
    pub face: Option<String>,
    #[serde(default)]
    pub subtypes: Vec<String>,
    /// PCGen `RACETYPE:` — the creature type, e.g. `Humanoid`.  Distinct from a
    /// `TYPE:` tag, and needed by `PRERACE:1,RACETYPE=Giant`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub race_type: Option<String>,
    #[serde(default)]
    pub natural_attacks: Vec<crate::rules::NaturalAttack>,
    #[serde(default)]
    pub unarmed_damage: Vec<String>,
    #[serde(default)]
    pub unarmed_mult: Option<i32>,
    /// Damage reduction entries, each with its own condition.
    ///
    /// PCGen writes `5/Magic|PRELEVEL:MIN=4`.  Carrying that as a string meant
    /// the snapshot exposed the raw text and ignored the gate: a level-1
    /// Celestial reported both `5/Magic|PRELEVEL:MIN=4` and
    /// `10/Magic|PRELEVEL:MIN=12`, when neither applies below level 4.
    #[serde(default)]
    pub damage_reduction: Vec<crate::rules::DamageReduction>,
    #[serde(default)]
    pub spell_resistance: Option<String>,
    #[serde(default)]
    pub challenge_rating: Option<String>,
    #[serde(default)]
    pub vision: Vec<String>,
    #[serde(default)]
    pub move_clone: Vec<String>,
    #[serde(default)]
    pub unencumbered_move: Vec<String>,
    #[serde(default)]
    pub attack_cycle: Vec<String>,
}

/// Declarative creature template definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TemplateDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub type_tags: Vec<String>,
    #[serde(default)]
    pub traits: CreatureTraits,
    /// Hit-die override (PCGen `HITDIE:`).
    #[serde(default)]
    pub hit_die_override: Option<String>,
    /// Additional HD/level adjustments (PCGen `ADDLEVEL:Class|n`).
    #[serde(default)]
    pub add_levels: BTreeMap<String, u32>,
    /// Ability-score minimums (PCGen `DEFINESTAT:MINVALUE|STAT|n`).
    #[serde(default)]
    pub stat_minimums: BTreeMap<String, i32>,
}

impl TemplateDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_tags: Vec::new(),
            traits: CreatureTraits::default(),
            hit_die_override: None,
            add_levels: BTreeMap::new(),
            stat_minimums: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceDefinition {
    pub id: String,
    pub name: String,
    /// `OUTPUTNAME:` display name; falls back to `name`.
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub level_adjustment: i32,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub speed_feet: i32,
    #[serde(default)]
    pub ability_adjustments: BTreeMap<String, i32>,
    #[serde(default)]
    pub favored_class: Option<String>,
    #[serde(default)]
    pub type_tags: Vec<String>,
    /// PCGen `SERVESAS:RACE|X`: this race is a subrace/variant of `X`.
    #[serde(default)]
    pub serves_as: Option<String>,
    /// Number of hands (PCGen `HANDS:`); defaults to biped 2.
    #[serde(default = "default_hands")]
    pub hands: u32,
    /// Movement modes and their speed in feet (PCGen `MOVE:Mode,ft`).
    #[serde(default)]
    pub movement: BTreeMap<String, u32>,
    /// Inherent creature templates granted by the race (PCGen `TEMPLATE:`).
    #[serde(default)]
    pub templates: Vec<String>,
    /// Creature traits (reach, natural attacks, DR/SR/CR, vision, ...).
    #[serde(default)]
    pub traits: CreatureTraits,
    /// Extra skill points per level (PCGen `XTRASKILLPTSPERLVL:`).
    #[serde(default)]
    pub extra_skill_points_per_level: Option<u32>,
    /// Ability-score minimums (PCGen `DEFINESTAT:MINVALUE|STAT|n`).
    #[serde(default)]
    pub stat_minimums: BTreeMap<String, i32>,
    /// Skill points multiplier at first level (PCGen `SKILLMULT:`).
    #[serde(default)]
    pub skill_multiplier: Option<u32>,
    /// `CHANGEPROF:Old=New` entries.
    #[serde(default)]
    pub changed_proficiencies: BTreeMap<String, String>,
}

fn default_hands() -> u32 {
    2
}

impl RaceDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id: id.into(),
            name: name.clone(),
            display_name: name,
            level_adjustment: 0,
            size: "M".to_string(),
            speed_feet: 30,
            ability_adjustments: BTreeMap::new(),
            favored_class: None,
            type_tags: Vec::new(),
            serves_as: None,
            hands: default_hands(),
            movement: BTreeMap::new(),
            templates: Vec::new(),
            traits: CreatureTraits::default(),
            extra_skill_points_per_level: None,
            stat_minimums: BTreeMap::new(),
            skill_multiplier: None,
            changed_proficiencies: BTreeMap::new(),
        }
    }
}

/// Class definition, including level-row tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub type_tags: Vec<String>,
    pub is_prestige: bool,
    #[serde(default)]
    pub is_psionic: bool,
    /// PCGen `SERVESAS:CLASS|X`: this class is an alternate/variant of `X`.
    #[serde(default)]
    pub serves_as: Option<String>,
    /// Monster / racial class: its levels do not count for `PREPCLEVEL`.
    #[serde(default)]
    pub is_monster: bool,
    #[serde(default)]
    pub gestalt_slot_cost: Option<PrestigeSlotCost>,
    #[serde(default)]
    pub hit_die: String,
    #[serde(default)]
    pub max_level: u32,
    #[serde(default)]
    pub attack_progression: AttackProgression,
    #[serde(default)]
    pub base_attack_formula: Option<String>,
    #[serde(default)]
    pub fortitude: SaveProgression,
    #[serde(default)]
    pub fortitude_formula: Option<String>,
    #[serde(default)]
    pub reflex: SaveProgression,
    #[serde(default)]
    pub reflex_formula: Option<String>,
    #[serde(default)]
    pub will: SaveProgression,
    #[serde(default)]
    pub will_formula: Option<String>,
    #[serde(default)]
    pub skill_points_per_level: u32,
    #[serde(default)]
    pub class_skills: Vec<String>,
    #[serde(default)]
    pub spellcasting_ability: Option<String>,
    #[serde(default)]
    pub casting_type: CastingType,
    /// Arcane / Divine / Psionic (PCGen `FACT:SpellType|...`).
    #[serde(default)]
    pub spell_power: SpellPowerType,
    /// Class casts from a spellbook (PCGen `SPELLBOOK:YES`).
    #[serde(default)]
    pub requires_spellbook: bool,
    #[serde(default)]
    pub templates: Vec<String>,
    #[serde(default)]
    pub spell_slots: BTreeMap<u32, Vec<u32>>,
    #[serde(default)]
    pub spells_known: BTreeMap<u32, Vec<u32>>,
    #[serde(default)]
    pub spell_list: Vec<String>,
    /// What each class level grants.
    ///
    /// Structured, and the single representation: it used to be raw PCGen rows
    /// here, parsed separately into `Grant` effects, and copied into
    /// `ClassProgression::features_by_level`.
    #[serde(default)]
    pub level_abilities: BTreeMap<u32, Vec<crate::ClassGrant>>,
    #[serde(default)]
    pub level_bonuses: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    pub level_prerequisites: BTreeMap<u32, Vec<String>>,
    /// Prohibited spell/school entries (PCGen `PROHIBITSPELL:`).
    #[serde(default)]
    pub prohibited_spells: Vec<crate::rules::ProhibitedSpell>,
    /// Bonus known spells from specialty (PCGen `KNOWNSPELLSFROMSPECIALTY:`).
    #[serde(default)]
    pub known_spells_from_specialty: u32,
    /// Skill points multiplier at first level (PCGen `SKILLMULT:`).
    #[serde(default)]
    pub skill_multiplier: Option<u32>,
    /// Levels per feat (PCGen `LEVELSPERFEAT:`); `None` uses the game default.
    #[serde(default)]
    pub levels_per_feat: Option<u32>,
    /// Hit-die override (PCGen `HITDIE:`), e.g. `12` or `d12`.
    #[serde(default)]
    pub hit_die_override: Option<String>,
    /// Excluded class membership tag (PCGen `EXCLASS:`).
    #[serde(default)]
    pub excluded_class: Option<String>,
    /// Exchange-level tag (PCGen `EXCHANGELEVEL:`).
    #[serde(default)]
    pub exchange_level: Option<crate::rules::ExchangeLevel>,
    /// Additional class levels (PCGen `ADDLEVEL:Class|n`).
    #[serde(default)]
    pub add_levels: BTreeMap<String, u32>,
    /// Weapon proficiency grants (PCGen `WEAPONBONUS:`).
    #[serde(default)]
    pub weapon_bonus: Vec<String>,
    /// Domains added by this class / subclass (PCGen `ADDDOMAINS:`).
    #[serde(default)]
    pub added_domains: Vec<crate::rules::AddedDomain>,
    /// Prohibited school cost (PCGen `PROHIBITCOST:`).
    #[serde(default)]
    pub prohibited_cost: Option<i32>,
    /// Per-level `DONOTADD` suppressions, e.g. `HITDIE` / `SKILLPOINTS`.
    #[serde(default)]
    pub level_do_not_add: BTreeMap<u32, Vec<String>>,
    /// Per-level `ADD:SPELLCASTER` advancement entries, e.g. `Psionic`.
    #[serde(default)]
    pub level_spellcaster_advance: BTreeMap<u32, Vec<String>>,
    /// Subclass choice expression (`CHOICE:` on a `SUBCLASS:` row).
    #[serde(default)]
    pub subclass_choice: Option<String>,
}

impl ClassDefinition {
    /// Everything this class grants up to `level`, structured.
    pub fn grants_up_to(&self, level: u32) -> Vec<crate::ClassGrant> {
        (1..=level)
            .flat_map(|level| {
                self.level_abilities
                    .get(&level)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    /// Everything this class grants up to `level`, as PCGen rows.
    ///
    /// The string form is produced here rather than stored, so there is still
    /// exactly one representation of what a level grants.
    pub fn feature_rows_up_to(&self, level: u32) -> Vec<String> {
        self.grants_up_to(level)
            .into_iter()
            .map(|grant| grant.render())
            .collect()
    }

    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            type_tags: Vec::new(),
            is_prestige: false,
            is_psionic: false,
            serves_as: None,
            is_monster: false,
            gestalt_slot_cost: None,
            hit_die: "d8".to_string(),
            max_level: 20,
            attack_progression: AttackProgression::None,
            base_attack_formula: None,
            fortitude: SaveProgression::None,
            fortitude_formula: None,
            reflex: SaveProgression::None,
            reflex_formula: None,
            will: SaveProgression::None,
            will_formula: None,
            skill_points_per_level: 2,
            class_skills: Vec::new(),
            spellcasting_ability: None,
            casting_type: CastingType::Unknown,
            spell_power: SpellPowerType::Unknown,
            requires_spellbook: false,
            templates: Vec::new(),
            spell_slots: BTreeMap::new(),
            spells_known: BTreeMap::new(),
            spell_list: Vec::new(),
            level_abilities: BTreeMap::new(),
            level_bonuses: BTreeMap::new(),
            level_prerequisites: BTreeMap::new(),
            prohibited_spells: Vec::new(),
            known_spells_from_specialty: 0,
            skill_multiplier: None,
            levels_per_feat: None,
            hit_die_override: None,
            excluded_class: None,
            exchange_level: None,
            add_levels: BTreeMap::new(),
            weapon_bonus: Vec::new(),
            added_domains: Vec::new(),
            prohibited_cost: None,
            level_do_not_add: BTreeMap::new(),
            level_spellcaster_advance: BTreeMap::new(),
            subclass_choice: None,
        }
    }
}

/// Skill definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
    /// PCGen `TYPE:` tags, e.g. `Perform` or `Craft`.  `PRESKILL:1,TYPE.Perform=5`
    /// matches against these, and they were not captured before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub type_tags: Vec<String>,
    #[serde(default)]
    pub key_stat: Option<String>,
    #[serde(default)]
    pub armor_check: bool,
    #[serde(default)]
    pub trained_only: bool,
}

impl SkillDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            type_tags: Vec::new(),
            key_stat: None,
            armor_check: false,
            trained_only: false,
        }
    }
}

/// Spell/power definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpellDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub school: String,
    /// Subschool (PCGen `SUBSCHOOL:`), e.g. `Ritual`.  228 in the RSRD spells
    /// file alone, and not captured before `PROHIBITSPELL:SUBSCHOOL` needed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subschool: Option<String>,
    /// Alignment and energy descriptors (PCGen `DESCRIPTOR:`), e.g. `Good`.
    ///
    /// Needed for `PROHIBITSPELL:ALIGNMENT.Good`: a Cleric of an evil deity may
    /// not cast spells with the Good descriptor, and the descriptor was not
    /// captured at all.
    #[serde(default)]
    pub descriptors: Vec<String>,
    #[serde(default)]
    pub class_levels: BTreeMap<String, u32>,
    /// Domains that include this spell (PCGen `DOMAINS:`).
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub is_psionic: bool,
    /// `VARIANTS` presentation choices on a spell record.
    #[serde(default)]
    pub variants: Vec<String>,
}

impl SpellDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            level: 0,
            school: String::new(),
            subschool: None,
            descriptors: Vec::new(),
            class_levels: BTreeMap::new(),
            domains: Vec::new(),
            is_psionic: false,
            variants: Vec::new(),
        }
    }
}

/// Equipment definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cost_gp: f64,
    #[serde(default)]
    pub weight_lb: f64,
    #[serde(default)]
    pub armor_bonus: i32,
    #[serde(default)]
    pub max_dex_bonus: Option<i32>,
    #[serde(default)]
    pub armor_check_penalty: i32,
    #[serde(default)]
    pub type_tags: Vec<String>,
    /// Base damage dice (PCGen `DAMAGE:`), e.g. `1d8`.
    #[serde(default)]
    pub damage: Option<String>,
    /// Critical multiplier (PCGen `CRITMULT:`), e.g. `x2`.
    #[serde(default)]
    pub crit_mult: Option<String>,
    /// Critical threat range (PCGen `CRITRANGE:`), e.g. `1` means 20.
    #[serde(default)]
    pub crit_range: Option<i32>,
    /// Wielding category (PCGen `WIELD:`), e.g. `OneHanded`.
    #[serde(default)]
    pub wield: Option<String>,
    /// Base item category (PCGen `BASEITEM:`), e.g. `Rod`.
    #[serde(default)]
    pub base_item: Option<String>,
    /// Container contents (PCGen `CONTAINS:`), preserved as raw expressions.
    #[serde(default)]
    pub contains: Vec<String>,
    /// Slots occupied (PCGen `SLOTS:`).
    #[serde(default)]
    pub slots: Option<i32>,
    /// Equip location (PCGen `LOCATION:`), e.g. `Equipped`, `Carried`.
    #[serde(default)]
    pub location: Option<String>,
    /// Standard equipment slot inferred from type tags.
    #[serde(default)]
    pub slot: crate::equipment::EquipSlot,
    /// Proficiency declaration (`PROFICIENCY:`), usually `WEAPON|Name`.
    #[serde(default)]
    pub proficiency: Option<String>,
    #[serde(default)]
    pub alternate_damage: Option<String>,
    #[serde(default)]
    pub alternate_crit_mult: Option<String>,
    #[serde(default)]
    pub alternate_crit_range: Option<i32>,
    #[serde(default)]
    pub alternate_types: Vec<String>,
    #[serde(default)]
    pub alternate_equipment_mods: Vec<String>,
    #[serde(default)]
    pub base_quantity: Option<String>,
}

impl EquipmentDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cost_gp: 0.0,
            weight_lb: 0.0,
            armor_bonus: 0,
            max_dex_bonus: None,
            armor_check_penalty: 0,
            type_tags: Vec::new(),
            damage: None,
            crit_mult: None,
            crit_range: None,
            wield: None,
            base_item: None,
            contains: Vec::new(),
            slots: None,
            location: None,
            slot: crate::equipment::EquipSlot::None,
            proficiency: None,
            alternate_damage: None,
            alternate_crit_mult: None,
            alternate_crit_range: None,
            alternate_types: Vec::new(),
            alternate_equipment_mods: Vec::new(),
            base_quantity: None,
        }
    }
}

/// Deity definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeityDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub alignment: Option<String>,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub favored_weapon: Option<String>,
    #[serde(default)]
    pub pantheon: Option<String>,
    /// Whether the deity is valid for deity selection (PCGen `VALIDFORDEITY:`).
    #[serde(default)]
    pub valid_for_deity: Option<bool>,
    /// Whether the deity is valid for followers (PCGen `VALIDFORFOLLOWER:`).
    #[serde(default)]
    pub valid_for_follower: Option<bool>,
}

impl DeityDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            alignment: None,
            domains: Vec::new(),
            favored_weapon: None,
            pantheon: None,
            valid_for_deity: None,
            valid_for_follower: None,
        }
    }
}

/// One domain spell at a spell level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainSpell {
    pub level: u32,
    pub name: String,
}

/// Domain definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub granted_power: Option<String>,
    #[serde(default)]
    pub spell_list: Vec<String>,
    #[serde(default)]
    pub domain_spells: Vec<DomainSpell>,
}

impl DomainDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            granted_power: None,
            spell_list: Vec::new(),
            domain_spells: Vec::new(),
        }
    }
}

/// Equipment modifier definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentModifierDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub cost_gp: f64,
    #[serde(default)]
    pub weight_lb: f64,
    #[serde(default)]
    pub armor_bonus: i32,
    #[serde(default)]
    pub type_tags: Vec<String>,
    /// Enhancement value (PCGen `PLUS:`), e.g. `+2` weapons/armor.
    #[serde(default)]
    pub plus: Option<i32>,
    /// Item-type membership (PCGen `ITYPE:`), used for `PRETYPE`/`EQMODTYPE` matching.
    #[serde(default)]
    pub item_types: Vec<String>,
    /// Mutually exclusive modifiers (PCGen `REPLACES:`).
    #[serde(default)]
    pub replaces: Vec<String>,
    /// Charge range expression (PCGen `CHARGES:`), e.g. `1|50`.
    #[serde(default)]
    pub charges: Option<String>,
    /// Creature traits granted while a modified item is equipped
    /// (`DR` / `SR`, captured from the raw record).
    #[serde(default)]
    pub traits: CreatureTraits,
    /// Armor type membership (`ARMORTYPE:`), e.g. `Medium|Light`.
    #[serde(default)]
    pub armor_types: Vec<String>,
}

impl EquipmentModifierDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            cost_gp: 0.0,
            weight_lb: 0.0,
            armor_bonus: 0,
            type_tags: Vec::new(),
            plus: None,
            item_types: Vec::new(),
            replaces: Vec::new(),
            charges: None,
            traits: CreatureTraits::default(),
            armor_types: Vec::new(),
        }
    }
}

/// Compact progression table consumed by the engine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassProgression {
    pub base_attack_bonus: Vec<i32>,
    pub fortitude: Vec<i32>,
    pub reflex: Vec<i32>,
    pub will: Vec<i32>,
    pub skill_points_per_level: u32,
    pub first_level_skill_points: u32,
    /// Hit die actually used for this class (`HITDIE` override or base `HD`).
    #[serde(default)]
    pub hit_die: String,
    /// First-level skill-point multiplier (`SKILLMULT` or game default 4).
    #[serde(default)]
    pub skill_multiplier: u32,
    /// Levels per feat for monster / alternate progression (`LEVELSPERFEAT`).
    #[serde(default)]
    pub levels_per_feat: u32,
    /// Additional class levels granted by `ADDLEVEL`.
    #[serde(default)]
    pub add_levels: BTreeMap<String, u32>,
    /// Class levels converted to this class when prerequisites are lost
    /// (`EXCLASS`).
    #[serde(default)]
    pub excluded_class: Option<String>,
    /// PCGen `EXCHANGELEVEL` specification.
    #[serde(default)]
    pub exchange_level: Option<crate::rules::ExchangeLevel>,
    pub class_skills: Vec<String>,
}

impl ClassProgression {
    fn value_at(values: &[i32], level: u32) -> i32 {
        if level == 0 {
            0
        } else {
            values
                .get(level as usize - 1)
                .copied()
                .unwrap_or_else(|| values.last().copied().unwrap_or(0))
        }
    }

    pub fn bab(&self, level: u32) -> i32 {
        Self::value_at(&self.base_attack_bonus, level)
    }

    pub fn save(&self, level: u32, kind: crate::progression::SaveKind) -> i32 {
        let values = match kind {
            crate::progression::SaveKind::Fortitude => &self.fortitude,
            crate::progression::SaveKind::Reflex => &self.reflex,
            crate::progression::SaveKind::Will => &self.will,
        };
        Self::value_at(values, level)
    }

    /// Number of feat slots granted by this class progression up to `level`.
    ///
    /// PCGen's default is one feat at first level, then one per three levels.
    /// Monster classes can override this with `LEVELSPERFEAT`.
    pub fn feat_slots(&self, level: u32) -> u32 {
        if level == 0 {
            return 0;
        }
        let step = self.levels_per_feat.max(1);
        1 + level / step
    }

    pub fn from_class(class: &ClassDefinition) -> Self {
        let max_level = class.max_level.max(1);
        let mut progression = Self {
            base_attack_bonus: Vec::with_capacity(max_level as usize),
            fortitude: Vec::with_capacity(max_level as usize),
            reflex: Vec::with_capacity(max_level as usize),
            will: Vec::with_capacity(max_level as usize),
            skill_points_per_level: class.skill_points_per_level,
            first_level_skill_points: class
                .skill_points_per_level
                .saturating_mul(class.skill_multiplier.unwrap_or(4))
                .max(1),
            hit_die: class
                .hit_die_override
                .clone()
                .unwrap_or_else(|| class.hit_die.clone()),
            skill_multiplier: class.skill_multiplier.unwrap_or(4),
            levels_per_feat: class.levels_per_feat.unwrap_or(3).max(1),
            add_levels: class.add_levels.clone(),
            excluded_class: class.excluded_class.clone(),
            exchange_level: class.exchange_level.clone(),
            class_skills: class.class_skills.clone(),
        };
        for level in 1..=max_level {
            progression
                .base_attack_bonus
                .push(class.attack_progression.base_attack_bonus(level));
            progression.fortitude.push(class.fortitude.base_save(level));
            progression.reflex.push(class.reflex.base_save(level));
            progression.will.push(class.will.base_save(level));
        }
        progression
    }
}

/// One structured entry inside a KIT / STARTPACK definition.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KitEntry {
    /// Primary target tag, e.g. `GEAR`, `RACE`, `CLASS`, `SKILL`, `TABLE`.
    pub target: String,
    /// Primary target value/key.
    pub key: String,
    #[serde(default)]
    pub quantity: Option<String>,
    #[serde(default)]
    pub option: Option<String>,
    #[serde(default)]
    pub lookup: Option<String>,
    #[serde(default)]
    pub values: Option<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    /// Additional tags preserved losslessly for later runtime expansion.
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

/// Declarative KIT / STARTPACK / TABLE definition.
///
/// This is intentionally import-only in P4-KIT-1: expressions such as
/// `roll(...)` and weighted `OPTION` ranges are preserved as strings and are
/// not evaluated until/unless P4-KIT-2 implements deterministic expansion.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KitDefinition {
    pub id: String,
    pub name: String,
    /// `STARTPACK`, `KIT`, or `TABLE`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub type_tags: Vec<String>,
    #[serde(default)]
    pub visible: Option<String>,
    #[serde(default)]
    pub apply_mode: Option<String>,
    #[serde(default)]
    pub equip_buy: Option<String>,
    #[serde(default)]
    pub select_expression: Option<String>,
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(default)]
    pub entries: Vec<KitEntry>,
}

/// Alignment metadata (PCGen `ALIGNMENT` records).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AlignmentDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub valid_for_deity: Option<bool>,
    #[serde(default)]
    pub valid_for_follower: Option<bool>,
}

/// Size-category metadata (PCGen `SIZE` records).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SizeDefinition {
    pub id: String,
    pub name: String,
    /// Size number ordering, e.g. `050` for Medium (`SIZENUM:`).
    #[serde(default)]
    pub size_number: Option<String>,
}

/// Ability-score metadata (PCGen `STAT` records).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StatDefinition {
    pub id: String,
    pub name: String,
    /// Stat modifier formula (PCGen `STATMOD:`), e.g. `floor(SCORE/2)-5`.
    #[serde(default)]
    pub modifier_formula: Option<String>,
}

/// One parsed `BONUS` row on a companion modifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompanionBonus {
    /// Canonical target, e.g. `COMBAT.AC` or `STAT.STR`.
    pub target: String,
    /// Numeric formula, evaluated against the companion's master level.
    pub formula: String,
    #[serde(default)]
    pub bonus_type: Option<String>,
    #[serde(default)]
    pub stack_key: Option<String>,
}

/// Declarative companion modifier (PCGen `FOLLOWER:` / `COMPANIONMOD`).
///
/// One record describes the companion state for a master class at or above a
/// given level: bonus Hit Dice, spell resistance formula, granted abilities
/// and stat/AC bonuses.  `id` is unique per `(type, level)` pair.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompanionModDefinition {
    /// Stable id, e.g. `FamiliarLVL=11`.
    pub id: String,
    /// Companion type tag, e.g. `Familiar` or `Special Mount`.
    #[serde(default)]
    pub companion_type: String,
    /// Master class/variable name triggering this row, e.g. `FamiliarLVL`.
    #[serde(default)]
    pub master_variable: String,
    /// Minimum master level at which this row applies.
    #[serde(default)]
    pub master_level: u32,
    /// Bonus Hit Dice granted (`HD:`).
    #[serde(default)]
    pub hit_dice_bonus: i32,
    /// Spell resistance expression (`SR:`), preserved as written.
    #[serde(default)]
    pub spell_resistance: Option<String>,
    /// CR / trait bonuses captured from the record.
    #[serde(default)]
    pub traits: CreatureTraits,
    /// Granted abilities (`ABILITY:...|AUTOMATIC|...`).
    #[serde(default)]
    pub abilities: Vec<String>,
    /// Raw `PRE*` requirements on the row, e.g. `PRERACE:1,Companion ~ Ape`.
    /// Rows sharing the same id can be alternative companion selections rather
    /// than cumulative modifiers; preserving the requirements prevents loss
    /// until the runtime companion selector can evaluate them.
    #[serde(default)]
    pub requirements: Vec<String>,
    /// Parsed `BONUS` rows (AC / STAT / etc.) applied to the companion.
    #[serde(default)]
    pub bonuses: Vec<CompanionBonus>,
}

impl CompanionModDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            companion_type: name.into(),
            ..Self::default()
        }
    }
}

/// One `ABILITY:` definition, including stack / multi / choose metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub type_tags: Vec<String>,
    /// PCGen `STACK:` (whether repeated selections stack).
    #[serde(default)]
    pub stack: Option<bool>,
    /// PCGen `MULT:` (whether the ability can be selected multiple times).
    #[serde(default)]
    pub multi: Option<bool>,
    /// PCGen `CHOOSE:` expression, preserved as written.
    #[serde(default)]
    pub choose: Option<String>,
}

/// One `ABILITYCATEGORY:` definition.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityCategoryDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub type_tags: Vec<String>,
    /// Explicit ability membership (`ABILITYLIST:`).
    #[serde(default)]
    pub ability_list: Vec<String>,
    /// Variable that supplies pool points (`POOL:`).
    #[serde(default)]
    pub pool: Option<String>,
}

/// All typed definitions produced by one import.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TypedDefinitions {
    pub races: BTreeMap<String, RaceDefinition>,
    pub classes: BTreeMap<String, ClassDefinition>,
    pub skills: BTreeMap<String, SkillDefinition>,
    pub spells: BTreeMap<String, SpellDefinition>,
    pub equipment: BTreeMap<String, EquipmentDefinition>,
    pub equipment_modifiers: BTreeMap<String, EquipmentModifierDefinition>,
    pub deities: BTreeMap<String, DeityDefinition>,
    pub domains: BTreeMap<String, DomainDefinition>,
    pub variants: BTreeMap<String, VariantDefinition>,
    pub kits: BTreeMap<String, KitDefinition>,
    pub templates: BTreeMap<String, TemplateDefinition>,
    #[serde(default)]
    pub abilities: BTreeMap<String, AbilityDefinition>,
    #[serde(default)]
    pub ability_categories: BTreeMap<String, AbilityCategoryDefinition>,
    /// Creature-trait metadata captured from every record, including
    /// Ability / ClassLevel / Equipment sources that are not yet modelled as
    /// full typed definitions.
    #[serde(default)]
    pub creature_trait_records: BTreeMap<String, CreatureTraits>,
    /// Companion rows grouped by their raw `FOLLOWER:` id.  Each key may
    /// have multiple alternative/conditioned rows (e.g. one per animal
    /// companion race), so values are lists rather than a single definition.
    #[serde(default)]
    pub companion_mods: BTreeMap<String, Vec<CompanionModDefinition>>,
    #[serde(default)]
    pub sizes: BTreeMap<String, SizeDefinition>,
    #[serde(default)]
    pub stats: BTreeMap<String, StatDefinition>,
    #[serde(default)]
    pub alignments: BTreeMap<String, AlignmentDefinition>,
    pub progressions: BTreeMap<String, ClassProgression>,
}
