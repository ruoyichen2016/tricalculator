//! Typed PCGen vocabulary used by the lowering pass.
//!
//! The engine code that consumes `CompiledRulePack`
//! must not branch on raw tag strings such as `"RACE"` or `"POWER"`.  This
//! module gives those concepts a type.  Unknown categories are preserved
//! verbatim through [`EntityCategory::Other`] so nothing is silently dropped.

use serde::{Deserialize, Serialize};

/// The character type PCGen assumes when a character declares none.
///
/// `GameMode.getDefaultCharacterType` returns the first entry of the game mode's
/// `CHARACTERTYPE:` list, which `system/gameModes/{3e,35e}/miscinfo.lst` gives as
/// `PC|NPC`.  That game-mode file sits outside a pack's `.pcc` graph, so the
/// 3e/3.5e default is used rather than imported.
pub const DEFAULT_CHARACTER_TYPE: &str = "PC";

/// Category of a compiled entity (the PCGen "record type").
///
/// The variants cover the categories the runtime engine currently reasons
/// about.  Everything else is kept as [`EntityCategory::Other`] and compared
/// case-insensitively via [`EntityCategory::as_str`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum EntityCategory {
    Race,
    Class,
    ClassLevel,
    Skill,
    Spell,
    Equipment,
    EquipmentModifier,
    Ability,
    AbilityCategory,
    Feat,
    Template,
    Deity,
    Domain,
    CompanionMod,
    Kit,
    Language,
    Alignment,
    Size,
    Stat,
    Save,
    WeaponProf,
    ArmorProf,
    ShieldProf,
    DataControl,
    GlobalModifier,
    Dynamic,
    DataTable,
    BioSet,
    /// Any other category, preserved verbatim (already upper-cased).
    Other(String),
}

impl From<String> for EntityCategory {
    fn from(raw: String) -> Self {
        Self::from_pcgen(&raw)
    }
}

impl From<EntityCategory> for String {
    fn from(category: EntityCategory) -> Self {
        category.as_str().to_string()
    }
}

impl EntityCategory {
    /// Parse a PCGen category name.  Matching is case-insensitive; the
    /// canonical spelling (upper-case) is used for [`Self::Other`].
    pub fn from_pcgen(raw: &str) -> Self {
        let upper = raw.trim().to_ascii_uppercase();
        match upper.as_str() {
            "RACE" => Self::Race,
            "CLASS" => Self::Class,
            "CLASS_LEVEL" => Self::ClassLevel,
            "SKILL" => Self::Skill,
            "SPELL" => Self::Spell,
            "EQUIPMENT" => Self::Equipment,
            "EQUIPMOD" => Self::EquipmentModifier,
            "ABILITY" => Self::Ability,
            "ABILITYCATEGORY" => Self::AbilityCategory,
            "FEAT" => Self::Feat,
            "TEMPLATE" => Self::Template,
            "DEITY" => Self::Deity,
            "DOMAIN" => Self::Domain,
            "COMPANIONMOD" => Self::CompanionMod,
            "KIT" => Self::Kit,
            "LANGUAGE" => Self::Language,
            "ALIGNMENT" => Self::Alignment,
            "SIZE" => Self::Size,
            "STAT" => Self::Stat,
            "SAVE" => Self::Save,
            "WEAPONPROF" => Self::WeaponProf,
            "ARMORPROF" => Self::ArmorProf,
            "SHIELDPROF" => Self::ShieldProf,
            "DATACONTROL" => Self::DataControl,
            "GLOBALMODIFIER" => Self::GlobalModifier,
            "DYNAMIC" => Self::Dynamic,
            "DATATABLE" => Self::DataTable,
            "BIOSET" => Self::BioSet,
            other => Self::Other(other.to_string()),
        }
    }

    /// Canonical PCGen spelling.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Race => "RACE",
            Self::Class => "CLASS",
            Self::ClassLevel => "CLASS_LEVEL",
            Self::Skill => "SKILL",
            Self::Spell => "SPELL",
            Self::Equipment => "EQUIPMENT",
            Self::EquipmentModifier => "EQUIPMOD",
            Self::Ability => "ABILITY",
            Self::AbilityCategory => "ABILITYCATEGORY",
            Self::Feat => "FEAT",
            Self::Template => "TEMPLATE",
            Self::Deity => "DEITY",
            Self::Domain => "DOMAIN",
            Self::CompanionMod => "COMPANIONMOD",
            Self::Kit => "KIT",
            Self::Language => "LANGUAGE",
            Self::Alignment => "ALIGNMENT",
            Self::Size => "SIZE",
            Self::Stat => "STAT",
            Self::Save => "SAVE",
            Self::WeaponProf => "WEAPONPROF",
            Self::ArmorProf => "ARMORPROF",
            Self::ShieldProf => "SHIELDPROF",
            Self::DataControl => "DATACONTROL",
            Self::GlobalModifier => "GLOBALMODIFIER",
            Self::Dynamic => "DYNAMIC",
            Self::DataTable => "DATATABLE",
            Self::BioSet => "BIOSET",
            Self::Other(other) => other.as_str(),
        }
    }

    /// True for `ABILITY`, `FEAT`, and `ABILITYCATEGORY`.
    pub fn is_ability_like(&self) -> bool {
        matches!(self, Self::Ability | Self::Feat | Self::AbilityCategory)
    }

    /// True for `CLASS` or `CLASS_LEVEL`.
    pub fn is_class_like(&self) -> bool {
        matches!(self, Self::Class | Self::ClassLevel)
    }
}

impl std::fmt::Display for EntityCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for EntityCategory {
    fn from(raw: &str) -> Self {
        Self::from_pcgen(raw)
    }
}

/// The kind portion of a `CHOOSE:` spec (the part before the first `|`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ChoiceKind {
    Skill,
    PcStat,
    Feat,
    Ability,
    Spell,
    School,
    Domain,
    All,
    /// Any other kind, preserved verbatim (already upper-cased).
    Other(String),
}

impl From<String> for ChoiceKind {
    fn from(raw: String) -> Self {
        Self::from_pcgen(&raw)
    }
}

impl From<ChoiceKind> for String {
    fn from(kind: ChoiceKind) -> Self {
        kind.as_str().to_string()
    }
}

impl ChoiceKind {
    /// Parse the leading token of a `CHOOSE:` spec.
    pub fn from_pcgen(raw: &str) -> Self {
        match raw.trim().to_ascii_uppercase().as_str() {
            "SKILL" => Self::Skill,
            "PCSTAT" => Self::PcStat,
            "FEAT" => Self::Feat,
            "ABILITY" => Self::Ability,
            "SPELL" => Self::Spell,
            "SCHOOL" => Self::School,
            "DOMAIN" => Self::Domain,
            "ALL" => Self::All,
            other => Self::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Skill => "SKILL",
            Self::PcStat => "PCSTAT",
            Self::Feat => "FEAT",
            Self::Ability => "ABILITY",
            Self::Spell => "SPELL",
            Self::School => "SCHOOL",
            Self::Domain => "DOMAIN",
            Self::All => "ALL",
            Self::Other(other) => other.as_str(),
        }
    }

    /// A spec token is a *kind keyword* (not a concrete choice) if it parses
    /// to a non-`Other` variant.  Used when picking a default choice from a
    /// `CHOOSE:SKILL|Climb`-style spec.
    pub fn is_keyword(token: &str) -> bool {
        !matches!(Self::from_pcgen(token), Self::Other(_))
    }
}

impl std::fmt::Display for ChoiceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_category_round_trips_and_normalizes() {
        assert_eq!(EntityCategory::from_pcgen("race"), EntityCategory::Race);
        assert_eq!(
            EntityCategory::from_pcgen(" Class_Level "),
            EntityCategory::ClassLevel
        );
        assert_eq!(EntityCategory::from_pcgen("race").as_str(), "RACE");
        let other = EntityCategory::from_pcgen("UNKNOWN_CATEGORY");
        assert_eq!(other, EntityCategory::Other("UNKNOWN_CATEGORY".to_string()));
        assert_eq!(other.as_str(), "UNKNOWN_CATEGORY");
    }

    #[test]
    fn entity_category_helpers() {
        assert!(EntityCategory::Ability.is_ability_like());
        assert!(EntityCategory::Feat.is_ability_like());
        assert!(EntityCategory::AbilityCategory.is_ability_like());
        assert!(!EntityCategory::Race.is_ability_like());
        assert!(EntityCategory::Class.is_class_like());
        assert!(EntityCategory::ClassLevel.is_class_like());
    }

    #[test]
    fn choice_kind_keywords() {
        assert_eq!(ChoiceKind::from_pcgen("skill"), ChoiceKind::Skill);
        assert_eq!(ChoiceKind::from_pcgen("PCSTAT"), ChoiceKind::PcStat);
        assert!(ChoiceKind::is_keyword("SKILL"));
        assert!(ChoiceKind::is_keyword("all"));
        assert!(!ChoiceKind::is_keyword("Climb"));
        assert_eq!(ChoiceKind::from_pcgen("Climb").as_str(), "CLIMB");
    }
}
