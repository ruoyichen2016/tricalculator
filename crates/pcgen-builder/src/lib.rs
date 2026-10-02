//! Application service layer for native character storage.
//!
//! The builder crate sits between `pcgen-engine` and the CLI/Web front ends.
//! It owns the native save envelope and a small file-based character
//! repository; creation / level-up workflows will live here as M9 progresses.

pub mod creation;
pub mod point_buy;

pub use creation::{
    create_draft, equipment_cost, level_up_draft, starting_gold_for_class, CreateRequest,
    CreateSpell, LevelUpRequest,
};

use pcgen_engine::CharacterDraft;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Current native save schema.
pub const SAVE_SCHEMA: u32 = 1;
/// Application version written into new envelopes.
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Native save format: inputs only, plus provenance for diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeSaveEnvelope {
    pub schema: u32,
    pub application_version: String,
    pub rules_fingerprint: String,
    #[serde(default)]
    pub data_ref: Option<String>,
    /// Campaign `.pcc` path relative to the active data root, when the
    /// character was imported from a `.pcg`.
    #[serde(default)]
    pub pcc: Option<String>,
    pub character: CharacterDraft,
}

impl NativeSaveEnvelope {
    pub fn new(
        character: CharacterDraft,
        rules_fingerprint: impl Into<String>,
        data_ref: Option<String>,
    ) -> Self {
        Self {
            schema: SAVE_SCHEMA,
            application_version: APPLICATION_VERSION.to_string(),
            rules_fingerprint: rules_fingerprint.into(),
            data_ref,
            pcc: None,
            character,
        }
    }

    pub fn with_pcc(mut self, pcc: Option<String>) -> Self {
        self.pcc = pcc;
        self
    }
}

/// Lightweight metadata for a character list page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterSummary {
    pub id: String,
    pub name: String,
    pub race_id: String,
    pub total_level: u32,
    pub rules_fingerprint: String,
}

#[derive(Debug, Error)]
pub enum BuilderError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported save schema {found} (max {max})")]
    UnsupportedSchema { found: u32, max: u32 },
    #[error("invalid character id: {0}")]
    InvalidId(String),
    #[error("character not found: {0}")]
    NotFound(String),
    #[error("invalid point buy: {0}")]
    InvalidPointBuy(String),
    #[error("unknown race: {0}")]
    UnknownRace(String),
    #[error("unknown class: {0}")]
    UnknownClass(String),
    #[error("invalid level: {0}")]
    InvalidLevel(u32),
    #[error("invalid ability score: {0}")]
    InvalidAbilityScore(String),
    #[error("invalid skill rank: {0}")]
    InvalidSkillRank(String),
    #[error("unknown skill: {0}")]
    UnknownSkill(String),
    #[error("unknown ability category: {0}")]
    UnknownAbilityCategory(String),
    #[error("invalid ability: {0}")]
    InvalidAbility(String),
    #[error("unknown spell: {0}")]
    UnknownSpell(String),
    #[error("spell {0} is not on class {1}'s list")]
    SpellNotOnClassList(String, String),
    #[error("unknown equipment: {0}")]
    UnknownEquipment(String),
    #[error("unknown variant: {0}")]
    UnknownVariant(String),
    #[error("invalid substitution level: {variant} for {class} level {level}")]
    InvalidSubstitutionLevel {
        class: String,
        level: u32,
        variant: String,
    },
    #[error("equipment budget exceeded: spent {spent:.2} > budget {budget:.2}")]
    EquipmentBudgetExceeded { spent: f64, budget: f64 },
}

/// File-based character repository.
///
/// One JSON file per character, named `<sanitized-id>.json` under `root`.
#[derive(Debug, Clone)]
pub struct CharacterRepository {
    root: PathBuf,
}

impl CharacterRepository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn save(&self, envelope: &NativeSaveEnvelope) -> Result<(), BuilderError> {
        fs::create_dir_all(&self.root)?;
        let id = sanitize_id(&envelope.character.id)?;
        let final_path = self.root.join(format!("{id}.json"));
        let temporary = self.root.join(format!(".{id}.json.tmp"));
        let bytes = serde_json::to_vec_pretty(envelope)?;
        fs::write(&temporary, bytes)?;
        if final_path.exists() {
            fs::remove_file(&final_path)?;
        }
        fs::rename(&temporary, &final_path)?;
        Ok(())
    }

    pub fn load(&self, id: &str) -> Result<NativeSaveEnvelope, BuilderError> {
        let path = self.path_for(id)?;
        if !path.exists() {
            return Err(BuilderError::NotFound(id.to_string()));
        }
        let bytes = fs::read(path)?;
        let envelope: NativeSaveEnvelope = serde_json::from_slice(&bytes)?;
        if envelope.schema > SAVE_SCHEMA {
            return Err(BuilderError::UnsupportedSchema {
                found: envelope.schema,
                max: SAVE_SCHEMA,
            });
        }
        Ok(envelope)
    }

    pub fn list(&self) -> Result<Vec<CharacterSummary>, BuilderError> {
        if !self.root.exists() {
            return Ok(Vec::new());
        }
        let mut summaries = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            let Ok(envelope) = serde_json::from_slice::<NativeSaveEnvelope>(&bytes) else {
                continue;
            };
            summaries.push(summary(&envelope));
        }
        summaries.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(summaries)
    }

    pub fn delete(&self, id: &str) -> Result<(), BuilderError> {
        let path = self.path_for(id)?;
        if !path.exists() {
            return Err(BuilderError::NotFound(id.to_string()));
        }
        fs::remove_file(path)?;
        Ok(())
    }

    fn path_for(&self, id: &str) -> Result<PathBuf, BuilderError> {
        Ok(self.root.join(format!("{}.json", sanitize_id(id)?)))
    }
}

fn summary(envelope: &NativeSaveEnvelope) -> CharacterSummary {
    let total_level = envelope.character.total_level();
    CharacterSummary {
        id: envelope.character.id.clone(),
        name: envelope.character.name.clone(),
        race_id: envelope.character.race_id.clone(),
        total_level,
        rules_fingerprint: envelope.rules_fingerprint.clone(),
    }
}

/// Sanitize a character id for use as a file name.
pub fn sanitize_id(value: &str) -> Result<String, BuilderError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BuilderError::InvalidId("empty".to_string()));
    }
    let sanitized: String = trimmed
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || character == '.'
            {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized == "." || sanitized == ".." {
        return Err(BuilderError::InvalidId(trimmed.to_string()));
    }
    Ok(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pcgen_engine::DraftClassLevel;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("pcgen-builder-test-{stamp}"))
    }

    fn sample_character(id: &str, name: &str, level: u32) -> CharacterDraft {
        CharacterDraft {
            id: id.to_string(),
            name: name.to_string(),
            race_id: "Human".to_string(),
            class_levels: vec![DraftClassLevel {
                class_id: "Fighter".to_string(),
                level,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            }],
            ..CharacterDraft::default()
        }
    }

    #[test]
    fn save_load_round_trip() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        let envelope = NativeSaveEnvelope::new(
            sample_character("hero-1", "Hero", 3),
            "fingerprint-a",
            Some("data-ref".to_string()),
        );
        repository.save(&envelope).unwrap();
        let loaded = repository.load("hero-1").unwrap();
        assert_eq!(loaded, envelope);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn list_returns_summaries() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("beta", "Beta", 2),
                "fp",
                None,
            ))
            .unwrap();
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("alpha", "Alpha", 5),
                "fp",
                None,
            ))
            .unwrap();
        let summaries = repository.list().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].name, "Alpha");
        assert_eq!(summaries[0].total_level, 5);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn delete_removes_character() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        let envelope = NativeSaveEnvelope::new(sample_character("gone", "Gone", 1), "fp", None);
        repository.save(&envelope).unwrap();
        repository.delete("gone").unwrap();
        assert!(matches!(
            repository.load("gone"),
            Err(BuilderError::NotFound(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_newer_schema() {
        let root = temp_root();
        fs::create_dir_all(&root).unwrap();
        let payload = serde_json::json!({
            "schema": SAVE_SCHEMA + 1,
            "application_version": "future",
            "rules_fingerprint": "fp",
            "data_ref": null,
            "character": sample_character("future", "Future", 1),
        });
        fs::write(
            root.join("future.json"),
            serde_json::to_vec(&payload).unwrap(),
        )
        .unwrap();
        let repository = CharacterRepository::new(&root);
        assert!(matches!(
            repository.load("future"),
            Err(BuilderError::UnsupportedSchema { .. })
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sanitizes_ids() {
        assert_eq!(sanitize_id("hello world").unwrap(), "hello_world");
        assert!(sanitize_id("").is_err());
        assert!(sanitize_id("..").is_err());
    }
}
