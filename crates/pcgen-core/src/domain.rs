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
    #[default]
    Half,
    Custom,
}

impl AttackProgression {
    pub fn base_attack_bonus(self, level: u32) -> i32 {
        let level = level as i32;
        match self {
            Self::Full => level,
            Self::ThreeQuarter => (level * 3) / 4,
            Self::Half => level / 2,
            Self::Custom => level,
        }
    }
}

/// d20 saving throw progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaveProgression {
    Good,
    #[default]
    Poor,
}

impl SaveProgression {
    pub fn base_save(self, level: u32) -> i32 {
        let level = level as i32;
        match self {
            Self::Good => 2 + level / 2,
            Self::Poor => level / 3,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RaceDefinition {
    pub id: String,
    pub name: String,
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
    /// Number of hands (PCGen `HANDS:`); defaults to biped 2.
    #[serde(default = "default_hands")]
    pub hands: u32,
    /// Movement modes and their speed in feet (PCGen `MOVE:Mode,ft`).
    #[serde(default)]
    pub movement: BTreeMap<String, u32>,
    /// Inherent creature templates granted by the race (PCGen `TEMPLATE:`).
    #[serde(default)]
    pub templates: Vec<String>,
}

fn default_hands() -> u32 {
    2
}

impl RaceDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            size: "M".to_string(),
            speed_feet: 30,
            ability_adjustments: BTreeMap::new(),
            favored_class: None,
            type_tags: Vec::new(),
            hands: default_hands(),
            movement: BTreeMap::new(),
            templates: Vec::new(),
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
    pub is_prestige: bool,
    #[serde(default)]
    pub is_psionic: bool,
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
    #[serde(default)]
    pub level_abilities: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    pub level_bonuses: BTreeMap<u32, Vec<String>>,
    #[serde(default)]
    pub level_prerequisites: BTreeMap<u32, Vec<String>>,
}

impl ClassDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            is_prestige: false,
            is_psionic: false,
            is_monster: false,
            gestalt_slot_cost: None,
            hit_die: "d8".to_string(),
            max_level: 20,
            attack_progression: AttackProgression::Half,
            base_attack_formula: None,
            fortitude: SaveProgression::Poor,
            fortitude_formula: None,
            reflex: SaveProgression::Poor,
            reflex_formula: None,
            will: SaveProgression::Poor,
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
        }
    }
}

/// Skill definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub id: String,
    pub name: String,
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
    #[serde(default)]
    pub class_levels: BTreeMap<String, u32>,
    #[serde(default)]
    pub is_psionic: bool,
}

impl SpellDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            level: 0,
            school: String::new(),
            class_levels: BTreeMap::new(),
            is_psionic: false,
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
    pub class_skills: Vec<String>,
    pub features_by_level: BTreeMap<u32, Vec<String>>,
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

    pub fn features_up_to(&self, level: u32) -> Vec<String> {
        (1..=level)
            .flat_map(|level| {
                self.features_by_level
                    .get(&level)
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }

    pub fn from_class(class: &ClassDefinition) -> Self {
        let max_level = class.max_level.max(1);
        let mut progression = Self {
            base_attack_bonus: Vec::with_capacity(max_level as usize),
            fortitude: Vec::with_capacity(max_level as usize),
            reflex: Vec::with_capacity(max_level as usize),
            will: Vec::with_capacity(max_level as usize),
            skill_points_per_level: class.skill_points_per_level,
            first_level_skill_points: class.skill_points_per_level.saturating_mul(4).max(4),
            class_skills: class.class_skills.clone(),
            features_by_level: BTreeMap::new(),
        };
        for level in 1..=max_level {
            progression
                .base_attack_bonus
                .push(class.attack_progression.base_attack_bonus(level));
            progression.fortitude.push(class.fortitude.base_save(level));
            progression.reflex.push(class.reflex.base_save(level));
            progression.will.push(class.will.base_save(level));
            if let Some(abilities) = class.level_abilities.get(&level) {
                progression
                    .features_by_level
                    .insert(level, abilities.clone());
            }
        }
        progression
    }
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
    pub progressions: BTreeMap<String, ClassProgression>,
}
