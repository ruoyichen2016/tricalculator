//! Stable entity identifiers.
//!
//! Display names are localized and may change between data sources; rule
//! references must not depend on them.  An [`EntityId`] separates the
//! ruleset, source pack, entity type, and stable data key so that
//! references remain valid even when a display name changes.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier in the form `ruleset:source:type:key`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId {
    pub ruleset: String,
    pub source: String,
    pub entity_type: String,
    pub key: String,
}

impl EntityId {
    pub fn new(
        ruleset: impl Into<String>,
        source: impl Into<String>,
        entity_type: impl Into<String>,
        key: impl Into<String>,
    ) -> Self {
        Self {
            ruleset: ruleset.into(),
            source: source.into(),
            entity_type: entity_type.into().to_ascii_lowercase(),
            key: normalize_key(key.into()),
        }
    }

    /// Build an ID from a raw `.lst` key, normalizing only whitespace and
    /// case.  This is the usual entry point for the PCGen import layer.
    pub fn from_pcgen(ruleset: &str, source: &str, entity_type: &str, raw_key: &str) -> Self {
        Self::new(ruleset, source, entity_type, raw_key)
    }

    pub fn parse(value: &str) -> Option<Self> {
        let mut parts = value.splitn(4, ':');
        let ruleset = parts.next()?;
        let source = parts.next()?;
        let entity_type = parts.next()?;
        let key = parts.next()?;
        Some(Self::new(ruleset, source, entity_type, key))
    }

    pub fn as_string(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.ruleset, self.source, self.entity_type, self.key
        )
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string())
    }
}

fn normalize_key(key: String) -> String {
    let trimmed = key.trim();
    if trimmed.contains(':') || trimmed.contains('|') {
        // Category-qualified PCGen keys are already stable and should not
        // have their inner separators rewritten.
        trimmed.to_string()
    } else {
        trimmed.to_ascii_lowercase().replace(' ', "_")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_id_normalizes_key_and_round_trips() {
        let id = EntityId::from_pcgen("dnd35", "rsrd", "class", "Psychic Warrior");
        assert_eq!(id.key, "psychic_warrior");
        let parsed = EntityId::parse(&id.as_string()).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn category_qualified_keys_keep_their_structure() {
        let id = EntityId::from_pcgen("dnd35", "rsrd", "ability", "CATEGORY=Feat|Power Attack");
        assert!(id.key.contains("CATEGORY=Feat"));
        assert!(id.key.contains('|'));
    }
}
