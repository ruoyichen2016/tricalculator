//! Archetype / substitution replacement definitions.

use crate::rules::Effect;
use serde::{Deserialize, Serialize};

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

    pub fn apply_to_features(&self, base: Vec<String>) -> Vec<String> {
        if self.replaces.is_empty() {
            let mut features = base;
            features.extend(self.grants.iter().cloned());
            return features;
        }
        let mut features: Vec<String> = base
            .into_iter()
            .filter(|feature| {
                !self
                    .replaces
                    .iter()
                    .any(|replace| feature.eq_ignore_ascii_case(replace))
            })
            .collect();
        features.extend(self.grants.iter().cloned());
        features
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
