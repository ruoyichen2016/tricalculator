//! Archetype / substitution replacement definitions.

use crate::rules::Effect;
use serde::{Deserialize, Serialize};

/// Normalized comparison key for a class feature name.
///
/// PCGen writes the same feature in several shapes (`Fighter Bonus Feat` in a
/// `REPLACES` tag vs. `Fighter ~ Bonus Feats` in the class progression), so
/// matching has to ignore case, spaces, underscores, hyphens and parentheses.
pub fn feature_match_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !matches!(character, '_' | '-' | '(' | ')' | '\'')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// Display name of a class feature line.
///
/// A progression entry looks like
/// `Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats|!PREABILITY:1,...`.
/// The name is the last `|`-segment that is not a `!PRE...` predicate, minus
/// the `Class ~ ` owner prefix (`Wizard ~ Familiar` -> `Familiar`).
pub fn feature_name(line: &str) -> String {
    let segment = line
        .split('|')
        .rev()
        .map(str::trim)
        .find(|segment| !segment.is_empty() && !segment.starts_with('!'))
        .unwrap_or_else(|| line.trim());
    // `Class ~ Feature` (PCGen's owner prefix).  Only strip a short owner so a
    // feature whose name legitimately contains `~` is left intact.
    if let Some((owner, feature)) = segment.split_once('~') {
        if owner.trim().chars().count() <= 24 && !feature.trim().is_empty() {
            return feature.trim().to_string();
        }
    }
    segment.to_string()
}

/// The class a feature line belongs to, as written in its `Class ~ Feature`
/// segment.  Empty when the line carries no owner prefix.
pub fn line_owner(line: &str) -> &str {
    let segment = line
        .split('|')
        .rev()
        .map(str::trim)
        .find(|segment| !segment.is_empty() && !segment.starts_with('!'))
        .unwrap_or("");
    match segment.split_once('~') {
        Some((owner, feature))
            if owner.trim().chars().count() <= 24 && !feature.trim().is_empty() =>
        {
            owner.trim()
        }
        _ => "",
    }
}

/// `TYPE:` sub-tags declared on a feature line (`!PREABILITY:...,TYPE.FighterBonusFeat`).
fn feature_type_tags(line: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for segment in line.split([',', '|']) {
        let segment = segment.trim();
        let Some(rest) = segment
            .strip_prefix("TYPE.")
            .or_else(|| segment.strip_prefix("type."))
        else {
            continue;
        };
        if !rest.is_empty() {
            tags.push(rest.to_ascii_lowercase());
        }
    }
    tags
}

/// A single `REPLACES:` target.
///
/// Two addressing forms are accepted:
///
/// * `TYPE=FighterBonusFeat` — match the ACF type tag PCGen itself uses to mark
///   a feature as replaceable.  This is the stable anchor: it survives display
///   name changes in the dataset.
/// * anything else — match the feature's display name, compared through
///   [`feature_match_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceTarget {
    /// `TYPE=<tag>`
    TypeTag(String),
    /// A feature display name.
    Name(String),
}

impl ReplaceTarget {
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        if let Some(tag) = raw
            .strip_prefix("TYPE=")
            .or_else(|| raw.strip_prefix("type="))
        {
            return Self::TypeTag(tag.trim().to_ascii_lowercase());
        }
        Self::Name(raw.to_string())
    }

    /// True when `line` is the feature this target refers to.
    pub fn matches_feature_line(&self, line: &str) -> bool {
        match self {
            Self::TypeTag(wanted) => feature_type_tags(line)
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(wanted)),
            Self::Name(wanted) => {
                let wanted = feature_match_key(&feature_name(wanted));
                if wanted.is_empty() {
                    return false;
                }
                let actual = feature_match_key(&feature_name(line));
                // A `REPLACES` name may carry the class as a plain prefix
                // (`Fighter Bonus Feat`) where the progression writes
                // `Fighter ~ Bonus Feats`; accept the target with or without
                // the line's owner prefix.
                names_match(&wanted, &actual) || names_match(&strip_owner(&wanted, line), &actual)
            }
        }
    }

    /// True when `feature` (a bare name) is the feature this target refers to.
    pub fn matches_feature_name(&self, feature: &str) -> bool {
        match self {
            Self::TypeTag(_) => false,
            Self::Name(wanted) => names_match(
                &feature_match_key(&feature_name(wanted)),
                &feature_match_key(feature),
            ),
        }
    }
}

/// Compare two normalized feature keys, tolerating PCGen's plural drift
/// (`bonusfeat` vs `bonusfeats`).
fn names_match(left: &str, right: &str) -> bool {
    !left.is_empty() && !right.is_empty() && (left == right || singular(left) == singular(right))
}

/// Strip the class name a feature line belongs to when a `REPLACES` target
/// carries it as a plain prefix (`Fighter Bonus Feat` -> `Bonus Feat`).
fn strip_owner(key: &str, line: &str) -> String {
    let owner = feature_match_key(line_owner(line));
    if owner.is_empty() {
        return key.to_string();
    }
    key.strip_prefix(&owner).unwrap_or(key).to_string()
}

/// Drop a trailing plural `s` so `bonusfeat` and `bonusfeats` compare equal.
fn singular(key: &str) -> &str {
    key.strip_suffix('s')
        .filter(|rest| !rest.is_empty())
        .unwrap_or(key)
}

/// A typed replacement rule for an archetype or substitution level.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VariantDefinition {
    pub id: String,
    pub name: String,
    /// Class this variant applies to, if restricted.
    #[serde(default)]
    pub applies_to: Option<String>,
    /// Feature names removed from the base class.
    #[serde(default)]
    pub replaces: Vec<String>,
    /// Feature names granted by the variant.
    #[serde(default)]
    pub grants: Vec<String>,
    /// Additional class skills granted by the variant.
    #[serde(default)]
    pub class_skills: Vec<String>,
    /// Raw `BONUS:` tags carried by the variant.
    #[serde(default)]
    pub bonus_tags: Vec<String>,
    /// Lowered effects carried by the variant.
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// True when `ADD:SPELLCASTER` advances an existing spellcasting track.
    #[serde(default)]
    pub spellcaster_advance: bool,
}

impl VariantDefinition {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            applies_to: None,
            replaces: Vec::new(),
            grants: Vec::new(),
            class_skills: Vec::new(),
            bonus_tags: Vec::new(),
            effects: Vec::new(),
            spellcaster_advance: false,
        }
    }

    /// The `REPLACES:` entries parsed into addressable targets.
    pub fn replace_targets(&self) -> Vec<ReplaceTarget> {
        self.replaces
            .iter()
            .map(|entry| ReplaceTarget::parse(entry))
            .collect()
    }

    /// True when `line` is one of the features this variant replaces.
    pub fn replaces_feature_line(&self, line: &str) -> bool {
        self.replace_targets()
            .iter()
            .any(|target| target.matches_feature_line(line))
    }

    /// True when `REPLACES` has at least one target that a caller can resolve.
    pub fn replaces_are_addressable(&self) -> bool {
        !self.replaces.is_empty()
    }

    pub fn apply_to_features(&self, base: Vec<String>) -> Vec<String> {
        if self.replaces.is_empty() {
            let mut features = base;
            features.extend(self.grants.iter().cloned());
            return features;
        }
        let targets = self.replace_targets();
        let mut features: Vec<String> = base
            .into_iter()
            .filter(|feature| {
                !targets.iter().any(|target| {
                    target.matches_feature_line(feature) || target.matches_feature_name(feature)
                })
            })
            .collect();
        features.extend(self.grants.iter().cloned());
        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The exact shape RSRD emits for a Fighter class feature.
    const FIGHTER_BONUS_FEATS: &str = "Fighter Class Feature|AUTOMATIC|Fighter ~ Bonus Feats|!PREABILITY:1,CATEGORY=ACF,TYPE.FighterBonusFeat,TYPE.FighterBonusFeats";
    const WIZARD_FAMILIAR: &str = "Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar|!PREABILITY:1,CATEGORY=ACF,TYPE.WizardFamiliar,TYPE.WizardSummonFamiliar";

    #[test]
    fn feature_name_strips_owner_prefix_and_predicate() {
        assert_eq!(feature_name(FIGHTER_BONUS_FEATS), "Bonus Feats");
        assert_eq!(feature_name(WIZARD_FAMILIAR), "Familiar");
        assert_eq!(feature_name("Bravery"), "Bravery");
        // A name containing `~` without a short owner prefix is untouched.
        assert_eq!(
            feature_name("Some very long descriptive name ~ x"),
            "Some very long descriptive name ~ x"
        );
    }

    #[test]
    fn match_key_ignores_spacing_and_punctuation() {
        assert_eq!(feature_match_key("Fighter Bonus Feat"), "fighterbonusfeat");
        assert_eq!(
            feature_match_key("fighter_bonus-feats"),
            "fighterbonusfeats"
        );
        assert_eq!(
            feature_match_key("Armor Proficiency (Light)"),
            "armorproficiencylight"
        );
    }

    #[test]
    fn type_address_matches_the_acf_anchor() {
        let target = ReplaceTarget::parse("TYPE=FighterBonusFeat");
        assert!(target.matches_feature_line(FIGHTER_BONUS_FEATS));
        assert!(!target.matches_feature_line(WIZARD_FAMILIAR));
    }

    #[test]
    fn name_address_matches_the_owner_stripped_name() {
        // `Fighter Bonus Feat` (tag spelling) must hit `Fighter ~ Bonus Feats`.
        let target = ReplaceTarget::parse("Fighter Bonus Feat");
        assert!(target.matches_feature_line(FIGHTER_BONUS_FEATS));
        assert!(!target.matches_feature_line(WIZARD_FAMILIAR));
    }

    #[test]
    fn variant_replaces_and_grants_features() {
        let variant = VariantDefinition {
            id: "two-handed-fighter".to_string(),
            name: "Two-Handed Fighter".to_string(),
            applies_to: Some("Fighter".to_string()),
            replaces: vec!["Bravery".to_string()],
            grants: vec!["Shattering Strike".to_string()],
            class_skills: vec!["Intimidate".to_string()],
            ..VariantDefinition::default()
        };
        let features =
            variant.apply_to_features(vec!["Bravery".to_string(), "Armor Training".to_string()]);
        assert!(!features.iter().any(|feature| feature == "Bravery"));
        assert!(features
            .iter()
            .any(|feature| feature == "Shattering Strike"));
        assert!(features.iter().any(|feature| feature == "Armor Training"));
    }

    #[test]
    fn variant_removes_a_real_progression_line() {
        // Regression: a bare display name never matched the structured line, so
        // `REPLACES` silently removed nothing.
        let variant = VariantDefinition {
            id: "weapon-master".to_string(),
            name: "Weapon Master".to_string(),
            applies_to: Some("Fighter".to_string()),
            replaces: vec!["TYPE=FighterBonusFeat".to_string()],
            grants: vec!["Weapon Training".to_string()],
            ..VariantDefinition::default()
        };
        let kept = "Fighter Class Feature|AUTOMATIC|Fighter ~ Weapon and Armor Proficiency";
        let after =
            variant.apply_to_features(vec![FIGHTER_BONUS_FEATS.to_string(), kept.to_string()]);

        assert!(
            !after.iter().any(|line| line.contains("Bonus Feats")),
            "the replaced feature survived: {after:?}"
        );
        assert!(
            after
                .iter()
                .any(|line| line.contains("Weapon and Armor Proficiency")),
            "an unrelated feature was removed: {after:?}"
        );
        assert!(after.iter().any(|line| line == "Weapon Training"));
    }

    #[test]
    fn unaddressable_replace_target_matches_nothing() {
        // A typo'd target must not accidentally match a feature.
        let target = ReplaceTarget::parse("Nonexistent Feature");
        assert!(!target.matches_feature_line(FIGHTER_BONUS_FEATS));
        assert!(!target.matches_feature_line(WIZARD_FAMILIAR));
    }
}
