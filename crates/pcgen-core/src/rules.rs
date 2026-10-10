//! Runtime rule IR.
//!
//! PCGen compatibility is lowered into these primitives by the import layer.
//! The engine sees stable IDs, predicates, effects, modifiers, and
//! provenance, never raw `.lst` tags.

use crate::domain::SpellPowerType;
use crate::identity::EntityId;
use crate::rational::Rational;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A boolean predicate used by effects and prerequisites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Predicate {
    Always,
    /// A PCGen gate the lowering pass could not interpret.
    ///
    /// Keeping the effect with an explicit `Unsupported` condition is less
    /// destructive than dropping the effect: the raw predicate text survives
    /// in the IR and appears in `CompiledEntity::unsupported_tags`.  The
    /// engine treats the condition as satisfied, so an unknown gate is visible
    /// as a possibly over-applied bonus rather than a silently missing one.
    Unsupported {
        raw: String,
    },
    /// An explicit temporary state is active (PCGen `TEMPBONUS` source).
    ActiveState {
        states: Vec<String>,
    },
    All(Vec<Predicate>),
    Any(Vec<Predicate>),
    Not(Box<Predicate>),
    Stat {
        stat: String,
        min: Option<i32>,
        max: Option<i32>,
    },
    HasEntity {
        entity: EntityId,
    },
    Variable {
        name: String,
        min: Option<i32>,
        max: Option<i32>,
    },
    /// `PRESKILL:x,y=z,y=z` — at least `min_count` of the listed skills have
    /// the stated number of ranks.
    ///
    /// PCGen's reference:
    ///
    /// ```text
    /// PRESKILL:x,y=z,y=z
    ///   x: Number (Number of skills)
    ///   y: Text (Skill name)   y: TYPE.Text (Skill type)
    ///   z: Number (Skill ranks)
    ///
    /// PRESKILL:1,Spot=10,Listen=10   either, with at least 10 ranks
    /// PRESKILL:2,TYPE.Spy=2,TYPE.Spy=2
    ///   "two Spy skills with at least two ranks in each of them"
    /// ```
    ///
    /// Only the last `y=z` pair was read, and `TYPE.` was not understood at all,
    /// so 507 of the RSRD's 1,626 values tested the wrong skill and 204 of them
    /// looked for a skill literally named `TYPE.Perform`.
    SkillRanks {
        specs: Vec<SkillRequirement>,
        min_count: u32,
    },
    Alignment {
        allowed: Vec<String>,
    },
    /// The character's *deity's* alignment (PCGen `PREDEITYALIGN`).
    ///
    /// Distinct from `Alignment`: a Cleric may be one step from their deity, so
    /// "a Cleric of an evil god does not cast Good spells" needs the deity's
    /// alignment, not the character's.
    DeityAlignment {
        allowed: Vec<String>,
    },
    CharacterLevel {
        #[serde(default)]
        min: Option<u32>,
        #[serde(default)]
        max: Option<u32>,
    },
    /// Total character class levels, excluding Monster (Racial) HD
    /// (PCGen `PREPCLEVEL:MIN=x,MAX=y`).
    CharacterClassLevel {
        min: Option<u32>,
        max: Option<u32>,
    },
    /// Total base attack bonus at least `min` (PCGen `PRETOTALAB:N`).
    TotalBab {
        min: u32,
    },
    /// Base save bonus at least `min` for one save kind
    /// (PCGen `PRECHECKBASE:1,Will=2`).
    BaseSave {
        save: String,
        min: i32,
    },
    HasTag {
        tag: String,
    },
    /// Base creature size comparison, using the canonical size order.
    SizeAtLeast {
        size: String,
    },
    SizeAtMost {
        size: String,
    },
    /// Creature has at least this many legs (PCGen `PRELEGSGTEQ`).
    LegsAtLeast {
        count: u32,
    },
    /// Creature has at most this many legs (PCGen `PRELEGSLTEQ`).
    LegsAtMost {
        count: u32,
    },
    /// A PCGen game-mode rule is enabled (PCGen `PRERULE`).
    RuleEnabled {
        rule: String,
    },
    /// At least `n` of `items` hold (PCGen `PREMULT:n,[..],[..]`).
    AtLeast {
        n: u32,
        items: Vec<Predicate>,
    },
    /// The character's race has this type tag (PCGen `PRERACETYPE`).
    RaceType {
        type_tag: String,
    },
    /// `PRESRGTEQ:n` / `PRESREQ:n` — spell resistance at least / exactly `n`.
    ///
    /// The reference gives `PRESRGTEQ:10` as "spell resistance of 10 or greater"
    /// and `PRESREQ:10` as "exactly 10", so one variant covers both. Spell
    /// resistance comes from the race *and* the active templates, which is why
    /// this could not be answered before the traits were merged where
    /// predicates run.
    SpellResistance {
        min: Option<i32>,
        max: Option<i32>,
    },
    /// `PREVISION:x,y=z,y=z` — at least `min_count` of the listed vision types,
    /// each at the stated distance (`ANY` for any distance).
    ///
    /// ```text
    /// PREVISION:2,Normal=ANY,Darkvision=ANY   both, at any distance
    /// PREVISION:1,Blindsight=30,Darkvision=30 either, at 30 ft or more
    /// ```
    Vision {
        specs: Vec<VisionSpec>,
        min_count: u32,
    },
    /// `PREGENDER:x` — the character's gender starts with `x`.
    ///
    /// PCGen's reference: "The character's gender must start with the text
    /// specified. The test is case sensitive so \"male\" is not the same as
    /// \"Male\"."  60 uses in the vendored data, and the character model had no
    /// gender at all.
    Gender {
        prefix: String,
    },
    /// `PRECLASS:x,y=z,y=z` — at least `min_count` of the listed class
    /// requirements are met.
    ///
    /// PCGen's reference:
    ///
    /// ```text
    /// PRECLASS:x,y=z,y=z
    ///   x: Number (The number of classes that must be equal to or greater than
    ///              the numbers specified for the check to succeed)
    ///   y: Text (Class Name)   y: TYPE.Text (Class Type)
    ///   y: SPELLCASTER   y: SPELLCASTER.Type (Spellcaster Type)   y: ANY
    ///   z: Number (Class Level)
    ///
    /// PRECLASS:2,Wizard=5,Sorcerer=6,Cleric=7   at least two of those
    /// PRECLASS:1,SPELLCASTER=2                  two levels in any casting class
    /// PRECLASS:1,SPELLCASTER.Arcane=2           two levels in an arcane one
    /// ```
    ///
    /// Only the first `y=z` pair was read and the count was ignored, so 140 of
    /// the RSRD's 1,735 values tested one class; `SPELLCASTER` (332 values),
    /// `TYPE.` (51) and `ANY` (2) looked for a class by that literal name.
    ClassRequirement {
        specs: Vec<ClassSpec>,
        min_count: u32,
    },
    /// `PREABILITY:x,CATEGORY=y,z` — at least `min_count` of the wanted
    /// abilities are held, and none of the excluded ones are.
    ///
    /// PCGen's reference:
    ///
    /// ```text
    /// PREABILITY:1,Category=Feat,TYPE.SpellFocus   one SpellFocus feat
    /// PREABILITY:3,Category=Feat,TYPE.Metamagic    three Metamagic feats
    /// PREABILITY:1,Category=SpecialQuality,[Night Vision]
    ///   "Requires that the creature does not have the Night Vision special
    ///    quality" — brackets exclude.
    /// PREABILITY:1,CATEGORY=Feat,Power Attack,[Weapon Finesse]
    ///   "has Power Attack, and does not have Weapon Finesse"
    /// ```
    ///
    /// Only the last bare name was read, so a bracketed exclusion was taken as a
    /// requirement and never matched; `TYPE.` was not understood; and the count
    /// was ignored. 924 of the RSRD's 7,183 values use brackets, 1,303 use
    /// `TYPE.`, and 2,481 list more than one name.
    AbilityRequirement {
        category: String,
        wanted: Vec<AbilityRef>,
        excluded: Vec<String>,
        min_count: u32,
    },
    /// `PRERACE:x,y,y` — at least `min_count` of the listed specs match.
    ///
    /// PCGen's reference gives five spellings:
    ///
    /// ```text
    /// PRERACE:1,Dwarf,Elf,Human               names
    /// PRERACE:1,Elf%,[Elf (aquatic)]          the % wildcard
    /// PRERACE:1,TYPE=Dire                     a race TYPE tag
    /// PRERACE:1,RACETYPE=Giant                the creature type
    /// PRERACE:1,RACESUBTYPE=Incorporeal       a race subtype
    /// PRERACE:2,RACETYPE=Undead,RACESUBTYPE=Incorporeal
    /// ```
    ///
    /// Only the first was implemented, as an exact race-name comparison, so
    /// 1,141 of the RSRD's 2,131 `PRERACE` values could never match — including
    /// 929 bare `%`, which the reference defines as matching any race.
    RaceMatches {
        specs: Vec<RaceSpec>,
        min_count: u32,
    },
    /// The character's age category matches (PCGen `PREAGESET`).  Unknown age
    /// does not match, so age-gated bonuses stay off unless explicit.
    AgeCategory {
        category: String,
    },
    /// The character can cast at least `count` spells of `power` at spell
    /// level `>= min_level` (PCGen `PRESPELLTYPE:w,type=level`).
    SpellType {
        /// One entry per `y=z` pair.  The reference:
        ///
        /// ```text
        /// PRESPELLTYPE:x,y=z
        ///   y: Arcane / Divine / Psionic / ANY
        ///   z: Number (Spell level)
        ///
        /// PRESPELLTYPE:1,Arcane=3,Divine=3,Psionic=3
        ///   "Character must have at least one 3rd level spell from any of the
        ///    Arcane, Divine, or Psionic types."
        /// ```
        ///
        /// So the pairs are alternatives and the count applies across their
        /// union.  Only the first pair was read, which made
        /// `PRESPELLTYPE:1,Arcane=9,Divine=9` unparseable — 11 uses in the full
        /// RSRD, which is in the acceptance cohort.
        specs: Vec<SpellTypeSpec>,
        count: u32,
    },
    /// The character has a class that casts from a spellbook
    /// (PCGen `PRESPELLBOOK:YES`).
    HasSpellbook,
    /// The character has a class of this power type
    /// (PCGen `PRESPELLCAST:TYPE=Arcane`).
    HasSpellPower {
        power: SpellPowerType,
    },
    /// The character has a class with the given casting mode
    /// (PCGen `PRESPELLCAST:MEMORIZE=Y|N`).
    CastingTypeIs {
        prepared: bool,
    },
    /// The character has at least `min_count` of the listed creature
    /// templates (PCGen `PRETEMPLATE:n,A,B`).
    TemplateCount {
        names: BTreeSet<String>,
        min_count: u32,
    },
    /// The character has at least this many hands (PCGen `PREHANDSGTEQ`).
    HandsAtLeast {
        count: u32,
    },
    /// The character has at most this many hands (PCGen `PREHANDSEQ`).
    HandsAtMost {
        count: u32,
    },
    /// The character has a movement mode of at least `feet` (PCGen `PREMOVE`).
    MovementAtLeast {
        mode: String,
        feet: u32,
    },
    /// Equipped-item query (PCGen `PREEQUIP*`).  `slot` is one of
    /// `any`, `primary`, `secondary`, `both`.
    Equipped {
        slot: String,
        values: Vec<String>,
    },
    /// Weapon proficiency query (PCGen `PREWEAPONPROF`).
    WeaponProf {
        names: Vec<String>,
    },
    /// Wielding category query (PCGen `PREWIELD`).
    Wield {
        categories: Vec<String>,
    },
    /// Domain membership query (PCGen `PREDOMAIN`).
    Domain {
        names: Vec<String>,
    },
    /// Deity membership query (PCGen `PREDEITY`).
    Deity {
        names: BTreeSet<String>,
        min_count: u32,
    },
    /// Hit Dice threshold (PCGen `PREHD`).
    HitDice {
        min: Option<u32>,
        max: Option<u32>,
    },
    /// Compare two variable/ability expressions (PCGen `PREVARGT/LT` with
    /// symbolic operands such as `%CHOICE,STR`).
    VariableCompare {
        left: String,
        right: String,
        greater: bool,
    },
    /// Symbolic equality/inequality comparison (PCGen `PREVAREQ/NEQ` with
    /// `var(...)` or `skillinfo(...)` operands).
    VariableEquals {
        left: String,
        right: String,
        equals: bool,
    },
    /// Armor type query (PCGen `PREARMORTYPE`).
    ArmorType {
        types: Vec<String>,
    },
    /// Armor proficiency query (PCGen `PREPROFWITHARMOR`).
    ArmorProf {
        categories: Vec<String>,
    },
    /// Shield proficiency query (PCGen `PREPROFWITHSHIELD`).
    ShieldProf {
        categories: Vec<String>,
    },
    /// Known-spell prerequisite (PCGen `PRESPELL`).
    KnownSpell {
        names: Vec<String>,
    },
    /// Spell-school level prerequisite (PCGen `PRESPELLSCHOOL`).
    SpellSchoolLevel {
        entries: Vec<(String, u32)>,
    },
    /// Subclass prerequisite (PCGen `PRESUBCLASS`).
    Subclass {
        names: Vec<String>,
    },
    /// Language prerequisite (PCGen `PRELANG`).
    Language {
        names: Vec<String>,
    },
}

/// Ordering phase for one effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectPhase {
    Base,
    Race,
    Class,
    Template,
    Feat,
    Ability,
    Equipment,
    Faith,
    Global,
    Final,
}

/// Where a rule came from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Provenance {
    pub source: String,
    pub line: Option<u32>,
    pub rule_id: Option<String>,
    pub reason: String,
}

impl Provenance {
    pub fn new(source: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            line: None,
            rule_id: None,
            reason: reason.into(),
        }
    }

    pub fn with_line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }
}

/// A primitive modifier operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierOperation {
    Add,
    Set,
    Replace,
    Min,
    Max,
    Multiply,
}

/// A typed modifier that can be applied to a target statistic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    pub target: String,
    pub operation: ModifierOperation,
    pub value: Rational,
    pub bonus_type: Option<String>,
    pub stack_key: Option<String>,
    pub condition: Option<Predicate>,
    pub source: Provenance,
}

impl Modifier {
    pub fn add(target: impl Into<String>, value: i32) -> Self {
        Self {
            target: target.into(),
            operation: ModifierOperation::Add,
            value: Rational::from(value),
            bonus_type: None,
            stack_key: None,
            condition: None,
            source: Provenance::new("unknown", "modifier"),
        }
    }

    pub fn typed(mut self, bonus_type: impl Into<String>) -> Self {
        self.bonus_type = Some(bonus_type.into());
        self
    }

    pub fn with_condition(mut self, condition: Predicate) -> Self {
        self.condition = Some(condition);
        self
    }

    pub fn with_source(mut self, source: Provenance) -> Self {
        self.source = source;
        self
    }
}

/// A lowered rule effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum EffectAction {
    Modify {
        modifier: Modifier,
    },
    ModifyFormula {
        target: String,
        expression: String,
        bonus_type: Option<String>,
        stack_key: Option<String>,
        condition: Option<Predicate>,
        source: Provenance,
    },
    /// `BONUS:VAR|A,B|expr` — assign to one or more runtime variables.
    ///
    /// PCGen wrote this as a `VAR.` target prefix with a comma-separated name
    /// list inside the string, which the engine had to parse.  Two defects came
    /// from that: the list was not split (so `A,B` became one variable named
    /// `A,B` and neither `A` nor `B` was set), and the prefix put PCGen syntax
    /// in the engine where the architecture guard could not see it.  Naming the
    /// variables as a list makes both unrepresentable.
    ModifyVariable {
        names: Vec<String>,
        expression: String,
        condition: Option<Predicate>,
        source: Provenance,
    },
    Grant {
        entity: EntityId,
    },
    GrantTag {
        tag: String,
    },
    Choose {
        spec: String,
    },
    DeclareVariable {
        name: String,
        initial: Rational,
    },
    DefineVariable {
        name: String,
        expression: String,
    },
    SetVariable {
        name: String,
        value: Rational,
    },
}

/// A single effect with phase, optional predicate, and provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub phase: EffectPhase,
    pub condition: Option<Predicate>,
    pub action: EffectAction,
    pub source: Provenance,
}

/// One accepted or rejected contribution in an explanation chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Contribution {
    pub source: String,
    pub value: i32,
    pub bonus_type: Option<String>,
    pub included: bool,
    pub reason: String,
}

impl Contribution {
    pub fn included(source: impl Into<String>, value: i32, reason: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            value,
            bonus_type: None,
            included: true,
            reason: reason.into(),
        }
    }

    pub fn excluded(source: impl Into<String>, value: i32, reason: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            value,
            bonus_type: None,
            included: false,
            reason: reason.into(),
        }
    }

    pub fn with_bonus_type(mut self, bonus_type: impl Into<String>) -> Self {
        self.bonus_type = Some(bonus_type.into());
        self
    }
}

/// Explanation for one derived statistic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    pub target: String,
    pub base: Rational,
    pub total: Rational,
    pub contributions: Vec<Contribution>,
}

/// Central policy for a named bonus type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BonusTypePolicy {
    pub name: String,
    pub positive: StackPolicyKind,
    pub negative: StackPolicyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackPolicyKind {
    Sum,
    Highest,
    Lowest,
}

/// Alternate weapon profile used by double weapons (`ALTCRIT*`, `ALTDAMAGE`,
/// `ALTTYPE`, `ALTEQMOD`, `BASEQTY`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlternateWeaponProfile {
    #[serde(default)]
    pub damage: Option<String>,
    #[serde(default)]
    pub crit_mult: Option<String>,
    #[serde(default)]
    pub crit_range: Option<i32>,
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub equipment_mods: Vec<String>,
    /// Base quantity for ammunition-like records (`BASEQTY`).
    #[serde(default)]
    pub base_quantity: Option<String>,
}

/// Runtime-visible metadata that is not specific to one typed definition
/// family (weapon / armor / spell subclass/cost tags).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityMetadata {
    #[serde(default)]
    pub alternate_weapon: AlternateWeaponProfile,
    /// Raw `WEAPONBONUS` entries, e.g. `TYPE=Simple` or specific names.
    #[serde(default)]
    pub weapon_bonus: Vec<String>,
    /// `CHANGEPROF:Old=New` entries.
    #[serde(default)]
    pub changed_proficiencies: BTreeMap<String, String>,
    /// `ARMORTYPE` entries (equipmod / armor proficiency metadata).
    #[serde(default)]
    pub armor_types: Vec<String>,
    /// `ADDDOMAINS` entries.
    #[serde(default)]
    pub added_domains: Vec<AddedDomain>,
    /// `PROHIBITCOST` value, when present.
    #[serde(default)]
    pub prohibited_cost: Option<i32>,
    /// `DONOTADD` entries on class-level rows.
    #[serde(default)]
    pub do_not_add: Vec<String>,
    /// Spell `VARIANTS` entries.
    #[serde(default)]
    pub spell_variants: Vec<String>,
    /// Bonus language choices (`LANGBONUS`).
    #[serde(default)]
    pub language_bonus: Vec<String>,
}

/// One `ADDDOMAINS` entry.
///
/// PCGen writes `Chaos[PRETEMPLATE:1,Vampire]`.  The engine used to split that
/// itself — `split_once('[')`, then `strip_prefix("PRETEMPLATE:")` with a
/// lowercase fallback — which was the last PCGen syntax it interpreted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddedDomain {
    pub domain: String,
    /// Condition the character must satisfy, when the entry has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Predicate>,
}

/// One `PREVISION` requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct VisionSpec {
    /// The vision type, e.g. `Darkvision`.
    pub kind: String,
    /// `None` for `ANY` — the distance is not constrained.
    pub min_distance: Option<u32>,
}

/// One `PRESPELLTYPE` pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SpellTypeSpec {
    /// `None` for `ANY` — any kind of spell slot.
    ///
    /// A spell's own power is concrete, so `ANY` belongs to the requirement
    /// rather than to `SpellPowerType`.
    pub power: Option<SpellPowerType>,
    pub min_level: u32,
}

/// One `PRECLASS` requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ClassSpec {
    /// `Wizard=5` — a named class.
    Class { class_id: String, min_level: u32 },
    /// `TYPE.Prestige=3` or `TYPE=Prestige=3` — a class type tag.
    Type { type_tag: String, min_level: u32 },
    /// `SPELLCASTER=2`, or `SPELLCASTER.Arcane=2` with `kind` set.
    Spellcaster {
        kind: Option<String>,
        min_level: u32,
    },
    /// `ANY=1` — any class at all.
    Any { min_level: u32 },
}

/// One ability a `PREABILITY` value asks for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum AbilityRef {
    /// An ability name or key.
    Name(String),
    /// `TYPE.Metamagic` — an ability type tag.
    Type(String),
}

/// One `PRESKILL` requirement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SkillRequirement {
    /// A skill name, or the type name when `is_type` is set.
    pub name: String,
    /// True for the `TYPE.Text` spelling.
    #[serde(default)]
    pub is_type: bool,
    /// Ranks required.
    pub min_ranks: u32,
}

/// One `PRERACE` value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum RaceSpec {
    /// A race name, compared case-insensitively.
    Name(String),
    /// `Elf%` or `%` — a prefix match against the race's id or name.
    Pattern(String),
    /// `TYPE=Dire` — one of the race's `TYPE:` tags.
    TypeTag(String),
    /// `RACETYPE=Giant` — the race's `RACETYPE:`.
    CreatureType(String),
    /// `RACESUBTYPE=Incorporeal` — one of the race's `RACESUBTYPE:` values.
    Subtype(String),
}

/// What a `PROHIBITSPELL` entry forbids.
///
/// PCGen's tag reference:
///
/// ```text
/// PROHIBITSPELL:x
///   ALIGNMENT.Text    DESCRIPTOR.Text    SCHOOL.Text
///   SUBSCHOOL.Text    SPELL.Text
///
/// Alignment, descriptor, school and subschool names are period-delimited lists
/// of the appropriate parameter. Spell name is a comma-delimited list.
///
/// PROHIBITSPELL:ALIGNMENT.Chaotic.Evil
///   Prohibits spells that have both the Chaotic and the Evil descriptors.
/// PROHIBITSPELL:SPELL.Searing Light,Sunbeam,Sunburst
/// ```
///
/// I originally modelled only `SCHOOL` and `ALIGNMENT` as single names, which
/// silently dropped 23 `SPELL.*` and 11 `DESCRIPTOR.*` entries — all of them in
/// Unearthed Arcana and other non-RSRD data, which is exactly the data that is
/// meant to be usable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ProhibitionTarget {
    /// `ALIGNMENT.Chaotic.Evil` — every listed descriptor must be present.
    Alignment(Vec<String>),
    /// `DESCRIPTOR.Fear` — every listed descriptor must be present.
    Descriptor(Vec<String>),
    /// `SCHOOL.Illusion`.
    School(String),
    /// `SUBSCHOOL.X` — every listed subschool must be present.
    Subschool(Vec<String>),
    /// `SPELL.Searing Light,Sunbeam` — any of these spells.
    Spell(Vec<String>),
}

/// One `PROHIBITSPELL` entry: what is forbidden, and when.
///
/// The raw data implements school prohibition in two halves — a
/// `TYPE:WizardProhibitSchool` ability sets `ProhibitAbjuration`, and the class
/// gates on `PREVARGTEQ:ProhibitAbjuration,1`.  Only the first half was wired
/// up: these entries were parsed, never consulted, and the spell picker offered
/// forbidden schools anyway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProhibitedSpell {
    pub target: ProhibitionTarget,
    /// When the prohibition applies.  `None` means always.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Predicate>,
}

/// A PCGen `EXCHANGELEVEL:` value.
///
/// From PCGen's tag reference:
///
/// ```text
/// EXCHANGELEVEL:w|x|y|z
///   w: Text   Class Name
///   x: Number Class level, minimum level required in donating class
///   y: Number Class level, maximum levels donated from class
///   z: Number Class level, lowest that donation can lower donating class level to
///
/// EXCHANGELEVEL:Ex Paladin|11|10|1
///   "Up to 10 levels of Ex Paladin can be exchanged as long as there were at
///    least 11 levels of Ex Paladin available."
/// ```
///
/// It was carried as a raw string and reached the snapshot that way — the same
/// shape of leak as `NATURALATTACKS`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExchangeLevel {
    /// `w` — the class whose levels are donated.
    pub donating_class: String,
    /// `x` — levels needed in the donating class before any may be exchanged.
    pub minimum_level: u32,
    /// `y` — the most levels that may be donated.
    pub maximum_levels: u32,
    /// `z` — the lowest the donating class may be reduced to.
    pub floor_level: u32,
}

/// One `NATURALATTACKS` entry.
///
/// PCGen writes `Bite,Weapon.Natural.Weapon Group Natural.Melee.Finesseable...,*1,1d6`.
/// Passing that string through put the internal encoding on the printed sheet,
/// under a heading that promised an attack routine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct NaturalAttack {
    /// Attack name, e.g. `Bite`.
    pub name: String,
    /// How many, from the `*N` field.
    pub count: u32,
    /// Damage dice, e.g. `1d6`.
    pub damage: String,
    /// Weapon type tags, period-delimited in the data (`Weapon.Natural.Melee`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// Optional trailing `SPROP=Text` special-property description.
    ///
    /// The reference gives the tag five parameters, the last optional. Without
    /// this the description was filed as a weapon *type* — 222 rows across the
    /// vendored data carry one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_property: Option<String>,
}

impl NaturalAttack {
    /// A line fit for a character sheet.
    pub fn render(&self) -> String {
        let mut out = self.name.clone();
        if self.count > 1 {
            out.push_str(&format!(" ×{}", self.count));
        }
        if !self.damage.is_empty() {
            out.push(' ');
            out.push_str(&self.damage);
        }
        out
    }
}

/// Collapse damage reduction to one entry per bypass type, keeping the highest.
///
/// PCGen's tag reference: *"DR tags with the same damage types do not stack,
/// only the DR tag with the highest value will be used."* A 12th-level Celestial
/// carries `5/Magic` (from level 4) and `10/Magic` (from level 12); both gates
/// are met, and only the 10 should be reported.
///
/// Entries whose amount is a formula rather than a number are kept as they are,
/// since they cannot be ordered without evaluating them.
pub fn collapse_damage_reduction(entries: &[String]) -> Vec<String> {
    let amount = |entry: &str| -> Option<i32> { entry.split_once('/')?.0.trim().parse().ok() };
    let kind = |entry: &str| -> String {
        entry
            .split_once('/')
            .map(|(_, kind)| kind.trim().to_ascii_lowercase())
            .unwrap_or_default()
    };

    let mut best: std::collections::BTreeMap<String, (Option<i32>, String)> =
        std::collections::BTreeMap::new();
    for entry in entries {
        let key = kind(entry);
        let value = amount(entry);
        match best.get_mut(&key) {
            Some(slot) => match (value, slot.0) {
                (Some(new), Some(old)) if new > old => *slot = (Some(new), entry.clone()),
                (Some(_), Some(_)) => {}
                // A formula on either side: not orderable, so keep both.
                _ => slot.0 = None,
            },
            None => {
                best.insert(key, (value, entry.clone()));
            }
        }
    }

    let mut out = Vec::new();
    for (key, (value, chosen)) in &best {
        if value.is_some() {
            out.push(chosen.clone());
        } else {
            out.extend(entries.iter().filter(|entry| kind(entry) == *key).cloned());
        }
    }
    out.sort();
    out.dedup();
    out
}

impl ProhibitedSpell {
    /// Does this prohibition cover `spell`?
    ///
    /// Multi-name targets are conjunctions for the descriptor-like kinds
    /// (`ALIGNMENT.Chaotic.Evil` needs both) and a disjunction for `SPELL`
    /// (a comma-delimited list of names).
    pub fn covers(&self, spell: &crate::domain::SpellDefinition) -> bool {
        let has = |wanted: &str| {
            spell
                .descriptors
                .iter()
                .any(|descriptor| descriptor.eq_ignore_ascii_case(wanted))
        };
        match &self.target {
            ProhibitionTarget::Alignment(names) | ProhibitionTarget::Descriptor(names) => {
                !names.is_empty() && names.iter().all(|name| has(name))
            }
            ProhibitionTarget::School(school) => spell.school.eq_ignore_ascii_case(school),
            ProhibitionTarget::Subschool(names) => {
                !names.is_empty()
                    && spell.subschool.as_deref().is_some_and(|subschool| {
                        names.iter().any(|n| subschool.eq_ignore_ascii_case(n))
                    })
            }
            ProhibitionTarget::Spell(names) => names
                .iter()
                .any(|name| spell.name.eq_ignore_ascii_case(name)),
        }
    }
}

/// One damage-reduction entry.
///
/// `5/Magic|PRELEVEL:MIN=4` is a value plus a gate.  The gate was never
/// evaluated while this was a `String`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DamageReduction {
    /// The reduction itself, e.g. `5/Magic` or `5/-`.
    pub value: String,
    /// Gate the character must satisfy for it to apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Predicate>,
}

/// UI metadata for a `TEMPVALUE` range associated with a temporary effect.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryValue {
    #[serde(default)]
    pub min: Option<i32>,
    #[serde(default)]
    pub max: Option<i32>,
    #[serde(default)]
    pub title: Option<String>,
}

/// A lowered, runtime-friendly view of one imported entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledEntity {
    pub id: EntityId,
    pub category: crate::vocab::EntityCategory,
    pub name: String,
    /// PCGen `TYPE:` tags, e.g. `Metamagic` on a feat.  `PREABILITY` and
    /// `PRETYPE` match against these, and they were not captured at all.
    #[serde(default)]
    pub type_tags: Vec<String>,
    pub effects: Vec<Effect>,
    pub requirements: Vec<Predicate>,
    pub legacy_tags: Vec<String>,
    pub unsupported_tags: Vec<String>,
    pub source: String,
    /// For `ABILITY` records, the pool they belong to (PCGen `CATEGORY:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_category: Option<String>,
    /// UI range metadata for `TEMPVALUE` on this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary_value: Option<TemporaryValue>,
    /// Creature traits captured from this record (reach, natural attacks,
    /// DR/SR/CR, vision, unarmed damage, ...).
    #[serde(default)]
    pub traits: crate::domain::CreatureTraits,
    /// Cross-cutting runtime metadata (weapon/armor/spell subclass tags).
    #[serde(default)]
    pub metadata: EntityMetadata,
}

/// Import/report totals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCoverage {
    pub entities: usize,
    pub lowered_effects: usize,
    pub legacy_tags: usize,
    pub unsupported_tags: usize,
    #[serde(default)]
    pub tag_occurrences: usize,
    #[serde(default)]
    pub known_tag_occurrences: usize,
    #[serde(default)]
    pub formula_effects: usize,
    #[serde(default)]
    pub grant_effects: usize,
    #[serde(default)]
    pub choose_effects: usize,
    #[serde(default)]
    pub variable_effects: usize,
    pub by_category: BTreeMap<String, CategoryCoverage>,
    /// Occurrence counts for tags that are known but not yet lowered.
    /// Keyed by the bare tag name (e.g. `"NAME"`, `"DESC"`).
    #[serde(default)]
    pub legacy_by_tag: BTreeMap<String, usize>,
    /// Legacy tags intentionally not lowered (presentation/metadata),
    /// keyed by bare tag name.  Excluded from the completion denominator.
    #[serde(default)]
    pub legacy_allowlisted_by_tag: BTreeMap<String, usize>,
    /// Legacy tags with (potential) runtime semantics that must be lowered,
    /// keyed by bare tag name.
    #[serde(default)]
    pub legacy_pending_by_tag: BTreeMap<String, usize>,
    /// Occurrence counts for tags that are not recognized at all.
    /// Keyed by the bare tag name.
    #[serde(default)]
    pub unsupported_by_tag: BTreeMap<String, usize>,
    /// Subset of `unsupported_by_tag` that counts as pending work (i.e. not
    /// allowlisted).  Keyed by the bare tag name.
    #[serde(default)]
    pub unsupported_pending_by_tag: BTreeMap<String, usize>,
    /// Subset of `unsupported_by_tag` that is allowlisted (presentation
    /// metadata that happened to fail lowering).  Excluded from the
    /// denominator.  Keyed by the bare tag name.
    #[serde(default)]
    pub unsupported_allowlisted_by_tag: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryCoverage {
    pub entities: usize,
    pub lowered_effects: usize,
    pub legacy_tags: usize,
    pub unsupported_tags: usize,
    #[serde(default)]
    pub tag_occurrences: usize,
    #[serde(default)]
    pub known_tag_occurrences: usize,
}

impl RuleCoverage {
    pub fn record_entity(
        &mut self,
        category: &str,
        lowered: usize,
        legacy: usize,
        unsupported: usize,
    ) {
        self.entities += 1;
        self.lowered_effects += lowered;
        self.legacy_tags += legacy;
        self.unsupported_tags += unsupported;
        let entry = self.by_category.entry(category.to_string()).or_default();
        entry.entities += 1;
        entry.lowered_effects += lowered;
        entry.legacy_tags += legacy;
        entry.unsupported_tags += unsupported;
    }

    /// Record one occurrence of a known-but-unlowered tag.
    ///
    /// The tag is also classified (see [`crate::tagclass`]) so callers can
    /// distinguish presentation metadata from pending semantic work.
    pub fn record_legacy_tag(&mut self, tag_name: &str) {
        let upper = tag_name.to_ascii_uppercase();
        *self.legacy_by_tag.entry(upper.clone()).or_insert(0) += 1;
        match crate::tagclass::classify(&upper) {
            crate::tagclass::TagClass::Allowlisted => {
                *self.legacy_allowlisted_by_tag.entry(upper).or_insert(0) += 1;
            }
            crate::tagclass::TagClass::Pending => {
                *self.legacy_pending_by_tag.entry(upper).or_insert(0) += 1;
            }
        }
    }

    /// Record one occurrence of a recognized tag the lowering pass rejected.
    ///
    /// The occurrence is also bucketed (see [`crate::tagclass::TagBucket`]):
    /// allowlisted tags stay out of the completion denominator even when
    /// lowering rejects them, everything else counts as unsupported work.
    pub fn record_unsupported_tag(&mut self, tag_name: &str) {
        let upper = tag_name.to_ascii_uppercase();
        *self.unsupported_by_tag.entry(upper.clone()).or_insert(0) += 1;
        match crate::tagclass::bucket_for_unsupported(&upper) {
            crate::tagclass::TagBucket::Allowlisted => {
                *self
                    .unsupported_allowlisted_by_tag
                    .entry(upper)
                    .or_insert(0) += 1;
            }
            // Both Pending and Unsupported are "work still to do".
            crate::tagclass::TagBucket::Pending | crate::tagclass::TagBucket::Unsupported => {
                *self.unsupported_pending_by_tag.entry(upper).or_insert(0) += 1;
            }
        }
    }

    pub fn record_tags(&mut self, category: &str, total: usize, known: usize) {
        self.tag_occurrences += total;
        self.known_tag_occurrences += known;
        let entry = self.by_category.entry(category.to_string()).or_default();
        entry.tag_occurrences += total;
        entry.known_tag_occurrences += known;
    }

    /// Occurrences of legacy tags that are intentionally ignored.
    pub fn legacy_allowlisted(&self) -> usize {
        self.legacy_allowlisted_by_tag.values().sum()
    }

    /// Occurrences of legacy tags still awaiting a lowering decision.
    pub fn legacy_pending(&self) -> usize {
        self.legacy_pending_by_tag.values().sum()
    }

    /// Occurrences of recognized tags the lowering pass rejected that count
    /// as pending work (not allowlisted).
    pub fn unsupported_pending(&self) -> usize {
        self.unsupported_pending_by_tag.values().sum()
    }

    /// Occurrences of allowlisted tags that the lowering pass rejected.
    pub fn unsupported_allowlisted(&self) -> usize {
        self.unsupported_allowlisted_by_tag.values().sum()
    }

    /// Total occurrences still awaiting semantic work: pending legacy tags
    /// plus unsupported tags that are not allowlisted.
    pub fn semantic_pending(&self) -> usize {
        self.legacy_pending() + self.unsupported_pending()
    }

    /// Completion ratio of semantic migration, in `[0.0, 1.0]`.
    ///
    /// Numerator: lowered effects.  Denominator: lowered effects plus *all*
    /// pending work — pending legacy tags **and** unsupported (recognized but
    /// not lowered) tags.  Only allowlisted presentation/metadata tags are
    /// excluded.
    pub fn semantic_completion(&self) -> f64 {
        let pending = self.semantic_pending();
        let denom = self.lowered_effects + pending;
        if denom == 0 {
            1.0
        } else {
            self.lowered_effects as f64 / denom as f64
        }
    }

    pub fn record_effect_actions(&mut self, category: &str, effects: &[Effect]) {
        for effect in effects {
            match &effect.action {
                EffectAction::ModifyFormula { .. } | EffectAction::ModifyVariable { .. } => {
                    self.formula_effects += 1
                }
                EffectAction::Grant { .. } | EffectAction::GrantTag { .. } => {
                    self.grant_effects += 1
                }
                EffectAction::Choose { .. } => self.choose_effects += 1,
                EffectAction::DeclareVariable { .. }
                | EffectAction::DefineVariable { .. }
                | EffectAction::SetVariable { .. } => self.variable_effects += 1,
                EffectAction::Modify { .. } => {}
            }
        }
        let entry = self.by_category.entry(category.to_string()).or_default();
        let _ = entry;
    }
}

/// Canonical d20 creature size order, smallest first.
pub const SIZE_ORDER: &[&str] = &[
    "fine",
    "diminutive",
    "tiny",
    "small",
    "medium",
    "large",
    "huge",
    "gargantuan",
    "colossal",
];

/// Normalize a PCGen size token (full name or single-letter abbreviation)
/// to a canonical [`SIZE_ORDER`] name.
pub fn normalize_size(size: &str) -> String {
    match size.trim().to_ascii_lowercase().as_str() {
        "f" => "fine".to_string(),
        "d" => "diminutive".to_string(),
        "t" => "tiny".to_string(),
        "s" => "small".to_string(),
        "m" => "medium".to_string(),
        "l" => "large".to_string(),
        "h" => "huge".to_string(),
        "g" => "gargantuan".to_string(),
        "c" => "colossal".to_string(),
        other => other.to_string(),
    }
}

/// Rank a PCGen size name in [`SIZE_ORDER`].  Unknown sizes rank as `Medium`.
pub fn size_rank(size: &str) -> usize {
    let normalized = normalize_size(size);
    SIZE_ORDER
        .iter()
        .position(|candidate| *candidate == normalized)
        .unwrap_or_else(|| {
            SIZE_ORDER
                .iter()
                .position(|candidate| *candidate == "medium")
                .unwrap_or(4)
        })
}

/// True if `size` is at least `minimum` in the canonical size order.
pub fn size_at_least(size: &str, minimum: &str) -> bool {
    size_rank(size) >= size_rank(minimum)
}

/// True if `size` is at most `maximum` in the canonical size order.
pub fn size_at_most(size: &str, maximum: &str) -> bool {
    size_rank(size) <= size_rank(maximum)
}
