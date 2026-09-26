//! Persisted character inputs.  Derived values are never stored here.

use pcgen_core::AdvancementFrame;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftClassLevel {
    pub class_id: String,
    pub level: u32,
    #[serde(default)]
    pub hit_points_gained: i32,
    /// Per-level HP rolls from the `.pcg` draft.
    #[serde(default)]
    pub hit_points_by_level: Vec<i32>,
    #[serde(default)]
    pub experience_gained: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftSpell {
    pub name: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub class_id: Option<String>,
    #[serde(default)]
    pub times_memorized: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterDraft {
    pub id: String,
    pub name: String,
    pub race_id: String,
    pub campaign: String,
    #[serde(default)]
    pub alignment: String,
    pub stats: BTreeMap<String, i32>,
    #[serde(default)]
    pub class_levels: Vec<DraftClassLevel>,
    #[serde(default)]
    pub advancement: Vec<AdvancementFrame>,
    #[serde(default)]
    pub skills: BTreeMap<String, f64>,
    #[serde(default)]
    pub spells: Vec<DraftSpell>,
    #[serde(default)]
    pub equipment: Vec<String>,
    /// Equipment modifiers attached to equipped items, keyed by item name
    /// (lower-cased).  Populated from `.pcg` `EQMOD=` customization data.
    #[serde(default)]
    pub equipment_modifiers: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub abilities: Vec<(String, String)>,
    /// Applied creature templates (PCGen `TEMPLATE:` / `.pcg`).
    #[serde(default)]
    pub templates: Vec<String>,
    /// Explicitly activated temporary states (PCGen `TEMPBONUS` sources).
    #[serde(default)]
    pub active_states: Vec<String>,
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
    /// Companions owned by the character, as `(type, master class level)`.
    #[serde(default)]
    pub companions: Vec<DraftCompanion>,
    /// Selected subclass per class id, used by `PRESUBCLASS`.
    #[serde(default)]
    pub class_subclasses: BTreeMap<String, String>,
}

/// One companion owned by the master (familiar, special mount, ...).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftCompanion {
    /// Companion type matching `CompanionModDefinition.companion_type`,
    /// e.g. `Familiar` or `Special Mount`.
    pub companion_type: String,
    /// Master class/variable level used by `mastervar(...)` formulas.
    pub master_level: u32,
    /// Selected companion race/key, used to match `PRERACE` rows
    /// (e.g. `Companion ~ Ape`).
    #[serde(default)]
    pub race_id: Option<String>,
}

impl DraftCompanion {
    pub fn new(companion_type: impl Into<String>, master_level: u32) -> Self {
        Self {
            companion_type: companion_type.into(),
            master_level,
            race_id: None,
        }
    }

    pub fn with_race(mut self, race_id: impl Into<String>) -> Self {
        self.race_id = Some(race_id.into());
        self
    }
}

impl Default for CharacterDraft {
    fn default() -> Self {
        Self {
            id: "draft".to_string(),
            name: "Unnamed".to_string(),
            race_id: "Human".to_string(),
            campaign: "3e".to_string(),
            alignment: "N".to_string(),
            stats: BTreeMap::new(),
            class_levels: Vec::new(),
            advancement: Vec::new(),
            skills: BTreeMap::new(),
            spells: Vec::new(),
            equipment: Vec::new(),
            equipment_modifiers: BTreeMap::new(),
            abilities: Vec::new(),
            templates: Vec::new(),
            active_states: Vec::new(),
            choices: BTreeMap::new(),
            companions: Vec::new(),
            class_subclasses: BTreeMap::new(),
        }
    }
}

impl CharacterDraft {
    pub fn total_level(&self) -> u32 {
        if !self.class_levels.is_empty() {
            self.class_levels.iter().map(|entry| entry.level).sum()
        } else {
            self.advancement.len() as u32
        }
    }

    pub fn is_gestalt(&self) -> bool {
        self.advancement.iter().any(|frame| frame.is_gestalt())
    }

    pub fn level_in_class(&self, class_id: &str) -> u32 {
        self.class_levels
            .iter()
            .find(|entry| class_ids_match(&entry.class_id, class_id))
            .map_or(0, |entry| entry.level)
    }
}

/// Class IDs appear in two spellings: PCGen display names (`Charlie Class`)
/// and stable `EntityId` keys (`charlie_class`).  Treat them as equivalent.
fn class_ids_match(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right) || normalize_class_id(left) == normalize_class_id(right)
}

fn normalize_class_id(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "_")
}
