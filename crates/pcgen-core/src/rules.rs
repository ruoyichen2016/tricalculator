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
    All(Vec<Predicate>),
    Any(Vec<Predicate>),
    Not(Box<Predicate>),
    Stat {
        stat: String,
        min: Option<i32>,
        max: Option<i32>,
    },
    ClassLevel {
        class_id: String,
        min: u32,
        max: Option<u32>,
    },
    HasEntity {
        entity: EntityId,
    },
    Variable {
        name: String,
        min: Option<i32>,
        max: Option<i32>,
    },
    SkillRank {
        skill: String,
        min: u32,
    },
    Alignment {
        allowed: Vec<String>,
    },
    CharacterLevel {
        min: u32,
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
    /// The character's age category matches (PCGen `PREAGESET`).  Unknown age
    /// does not match, so age-gated bonuses stay off unless explicit.
    AgeCategory {
        category: String,
    },
    /// The character can cast at least `count` spells of `power` at spell
    /// level `>= min_level` (PCGen `PRESPELLTYPE:w,type=level`).
    SpellType {
        power: SpellPowerType,
        min_level: u32,
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

/// A lowered, runtime-friendly view of one imported entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledEntity {
    pub id: EntityId,
    pub category: crate::vocab::EntityCategory,
    pub name: String,
    pub effects: Vec<Effect>,
    pub requirements: Vec<Predicate>,
    pub legacy_tags: Vec<String>,
    pub unsupported_tags: Vec<String>,
    pub source: String,
    /// For `ABILITY` records, the pool they belong to (PCGen `CATEGORY:`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ability_category: Option<String>,
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
                EffectAction::ModifyFormula { .. } => self.formula_effects += 1,
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
