//! What a class level grants.
//!
//! This type exists to remove a duplicate representation.  A PCGen class level
//! row writes its grants as one string:
//!
//! ```text
//! Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar|!PREABILITY:1,CATEGORY=ACF,TYPE.WizardFamiliar
//! ```
//!
//! That string used to be stored verbatim on `ClassDefinition::level_abilities`
//! *and* parsed separately by `lower_grant` into a structured effect, with
//! `ClassProgression::from_class` then copying the strings into a third field,
//! `features_by_level`.  Three representations of one fact, two of them raw
//! text, in the same crate.
//!
//! Here it is one structured value.  [`ClassGrant::render`] reproduces the
//! original row for the places that still want to show or hash it, but nothing
//! re-parses it.

use serde::{Deserialize, Serialize};

/// How a class level grants a feature.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantMode {
    /// PCGen `AUTOMATIC`: the character has it without choosing.
    #[default]
    Automatic,
    /// PCGen `NORMAL` / `VIRTUAL`: selectable, or virtual.
    Selectable,
}

/// A feature a class level grants.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassGrant {
    /// Ability pool the grant draws from, e.g. `Wizard Class Feature`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    /// Feature name, e.g. `Wizard ~ Familiar`.
    pub feature: String,
    #[serde(default)]
    pub mode: GrantMode,
    /// Replacement anchors that suppress this grant.  A variant naming any of
    /// them replaces the feature instead of adding it.  There can be more than
    /// one: the RSRD guards the Wizard's familiar with both `WizardFamiliar`
    /// and `WizardSummonFamiliar`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unless_replaced: Vec<String>,
    /// Ability pool the anchors are looked up in, e.g. `ACF` or
    /// `Special Ability`.  Part of the guard's meaning, so it travels with
    /// them: the Sorcerer's familiar is guarded in `Special Ability` while the
    /// Wizard's is guarded in `ACF`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unless_category: Option<String>,
    /// Any other guard, verbatim, e.g. `PRECLASS:1,Ex Paladin=3`.
    ///
    /// Not yet structured — a later step lowers these into `Predicate`.  They
    /// are kept because dropping them changes what the grant means.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guards: Vec<String>,
}

impl ClassGrant {
    /// A grant with no pool and no guards.
    pub fn automatic(feature: impl Into<String>) -> Self {
        Self {
            feature: feature.into(),
            ..Self::default()
        }
    }

    /// Reproduce the PCGen row this grant came from.
    ///
    /// Only for display and fingerprinting; nothing parses the result.
    pub fn render(&self) -> String {
        let mut row = String::new();
        if let Some(pool) = &self.pool {
            row.push_str(pool);
            row.push('|');
        }
        row.push_str(match self.mode {
            GrantMode::Automatic => "AUTOMATIC",
            GrantMode::Selectable => "NORMAL",
        });
        row.push('|');
        row.push_str(&self.feature);
        if !self.unless_replaced.is_empty() {
            row.push_str("|!PREABILITY:1,CATEGORY=");
            row.push_str(self.unless_category.as_deref().unwrap_or("ACF"));
            for anchor in &self.unless_replaced {
                row.push_str(",TYPE.");
                row.push_str(anchor);
            }
        }
        for guard in &self.guards {
            row.push('|');
            row.push_str(guard);
        }
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_reproduces_a_plain_row() {
        let grant = ClassGrant {
            pool: Some("Wizard Class Feature".to_string()),
            feature: "Wizard ~ Spells".to_string(),
            ..ClassGrant::default()
        };
        assert_eq!(
            grant.render(),
            "Wizard Class Feature|AUTOMATIC|Wizard ~ Spells"
        );
    }

    #[test]
    fn render_reproduces_a_guarded_row() {
        let grant = ClassGrant {
            pool: Some("Wizard Class Feature".to_string()),
            feature: "Wizard ~ Familiar".to_string(),
            unless_replaced: vec!["WizardFamiliar".to_string()],
            unless_category: Some("ACF".to_string()),
            ..ClassGrant::default()
        };
        assert_eq!(
            grant.render(),
            "Wizard Class Feature|AUTOMATIC|Wizard ~ Familiar|!PREABILITY:1,CATEGORY=ACF,TYPE.WizardFamiliar"
        );
    }

    #[test]
    fn render_keeps_the_guard_category() {
        // The Sorcerer's familiar is guarded in `Special Ability`, not `ACF`.
        let grant = ClassGrant {
            pool: Some("Sorcerer Class Feature".to_string()),
            feature: "Sorcerer ~ Familiar".to_string(),
            unless_replaced: vec!["SorcererFamiliar".to_string()],
            unless_category: Some("Special Ability".to_string()),
            ..ClassGrant::default()
        };
        assert!(grant.render().contains("CATEGORY=Special Ability"));
    }

    #[test]
    fn render_keeps_non_anchor_guards() {
        let grant = ClassGrant {
            pool: Some("Blackguard Class Feature".to_string()),
            feature: "Blackguard ~ Lay on Hands".to_string(),
            guards: vec!["PRECLASS:1,Ex Paladin=3".to_string()],
            ..ClassGrant::default()
        };
        assert_eq!(
            grant.render(),
            "Blackguard Class Feature|AUTOMATIC|Blackguard ~ Lay on Hands|PRECLASS:1,Ex Paladin=3"
        );
    }

    #[test]
    fn render_round_trips_through_the_pcgen_shape() {
        let grant = ClassGrant {
            pool: Some("Pool".to_string()),
            feature: "Feature".to_string(),
            mode: GrantMode::Selectable,
            unless_replaced: vec!["A".to_string(), "B".to_string()],
            unless_category: Some("ACF".to_string()),
            guards: vec!["PRECLASS:1,Wizard=5".to_string()],
        };
        assert_eq!(
            grant.render(),
            "Pool|NORMAL|Feature|!PREABILITY:1,CATEGORY=ACF,TYPE.A,TYPE.B|PRECLASS:1,Wizard=5"
        );
    }
}
