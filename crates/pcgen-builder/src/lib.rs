//! Application service layer for native character storage.
//!
//! The builder crate sits between `pcgen-engine` and the CLI/Web front ends.
//! It owns the native save envelope and a small file-based character
//! repository; creation / level-up workflows will live here as M9 progresses.

pub mod creation;
pub mod point_buy;

pub use creation::{
    apply_level_up, create_draft, equipment_cost, level_up_draft, preview_level_up,
    starting_gold_for_class, CreateRequest, CreateSpell, LevelUpLane, LevelUpRequest,
};
// `BundleReport`, `validate_bundle` and `validate_bundle_fingerprint` live in
// this module and are exported at the crate root by virtue of being `pub`.

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
    #[error("unknown template: {0}")]
    UnknownTemplate(String),
    #[error("unknown variant: {0}")]
    UnknownVariant(String),
    #[error("invalid substitution level: {variant} for {class} level {level}")]
    InvalidSubstitutionLevel {
        class: String,
        level: u32,
        variant: String,
    },
    #[error("skill points exceeded: spent {spent:.2} > available {budget}")]
    SkillPointsExceeded { spent: f64, budget: u32 },
    #[error("equipment budget exceeded: spent {spent:.2} > budget {budget:.2}")]
    EquipmentBudgetExceeded { spent: f64, budget: f64 },
}

/// Difference between a saved revision and the current live version.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct VersionSummary {
    /// Signed character-level difference (revision − current).
    pub level_delta: i64,
    /// Classes present in the revision but not in the current version.
    pub classes_removed: Vec<String>,
    /// Classes present in the current version but not in the revision.
    pub classes_added: Vec<String>,
    /// Templates present in the revision but not currently applied.
    pub templates_removed: Vec<String>,
    pub templates_added: Vec<String>,
    /// Ability entries present in the revision but not in the current version,
    /// formatted as `CATEGORY: key`.
    pub abilities_removed: Vec<String>,
    /// Ability entries present in the current version but not in the revision.
    pub abilities_added: Vec<String>,
}

impl VersionSummary {
    /// True when the revision differs from the current version at all.
    pub fn is_empty(&self) -> bool {
        self.level_delta == 0
            && self.classes_removed.is_empty()
            && self.classes_added.is_empty()
            && self.templates_removed.is_empty()
            && self.templates_added.is_empty()
            && self.abilities_added.is_empty()
            && self.abilities_removed.is_empty()
    }
}

/// One revision in a character's undo/redo history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterVersion {
    /// 1-based position from the oldest saved revision to the newest.
    pub index: usize,
    pub name: String,
    pub total_level: u32,
    /// Unix seconds when the snapshot was written.
    pub saved_at_unix: i64,
    /// Relative slot, e.g. `undo` or `redo`.
    pub stack: String,
    /// How this revision differs from the current live version.
    #[serde(default)]
    pub summary: VersionSummary,
}

/// Signed level difference, class/template/ability set changes between a
/// revision and the current version.
fn summarize(revision: &CharacterDraft, current: &CharacterDraft) -> VersionSummary {
    use std::collections::BTreeSet;

    let revision_classes: BTreeSet<String> = revision
        .class_levels
        .iter()
        .map(|entry| entry.class_id.clone())
        .collect();
    let current_classes: BTreeSet<String> = current
        .class_levels
        .iter()
        .map(|entry| entry.class_id.clone())
        .collect();
    let revision_templates: BTreeSet<String> = revision
        .templates
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect();
    let current_templates: BTreeSet<String> = current
        .templates
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect();

    let ability_set = |draft: &CharacterDraft| -> BTreeSet<String> {
        draft
            .abilities
            .iter()
            .map(|(category, key)| format!("{category}: {key}"))
            .collect()
    };
    let revision_abilities = ability_set(revision);
    let current_abilities = ability_set(current);

    VersionSummary {
        level_delta: revision.total_level() as i64 - current.total_level() as i64,
        classes_removed: revision_classes
            .difference(&current_classes)
            .cloned()
            .collect(),
        classes_added: current_classes
            .difference(&revision_classes)
            .cloned()
            .collect(),
        templates_removed: revision_templates
            .difference(&current_templates)
            .cloned()
            .collect(),
        templates_added: current_templates
            .difference(&revision_templates)
            .cloned()
            .collect(),
        abilities_removed: revision_abilities
            .difference(&current_abilities)
            .cloned()
            .collect(),
        abilities_added: current_abilities
            .difference(&revision_abilities)
            .cloned()
            .collect(),
    }
}

/// Validation result for a character bundle before importing it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct BundleReport {
    /// Characters whose schema is newer than this build supports.
    pub unsupported: Vec<String>,
    /// Characters written with a different rules fingerprint.
    pub fingerprint_mismatch: Vec<String>,
    /// Characters whose recorded application version differs.
    pub version_mismatch: Vec<String>,
}

impl BundleReport {
    /// Human-readable warnings; empty when the bundle looks fully compatible.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if !self.unsupported.is_empty() {
            warnings.push(format!(
                "{} character(s) use an unsupported save schema: {}",
                self.unsupported.len(),
                self.unsupported.join(", ")
            ));
        }
        if !self.fingerprint_mismatch.is_empty() {
            warnings.push(format!(
                "{} character(s) were created with a different rules fingerprint: {}",
                self.fingerprint_mismatch.len(),
                self.fingerprint_mismatch.join(", ")
            ));
        }
        if !self.version_mismatch.is_empty() {
            warnings.push(format!(
                "{} character(s) were written by a different app version: {}",
                self.version_mismatch.len(),
                self.version_mismatch.join(", ")
            ));
        }
        warnings
    }

    pub fn is_compatible(&self) -> bool {
        self.unsupported.is_empty()
    }
}

/// Compare a bundle of characters against the current build's expectations.
pub fn validate_bundle(characters: &[NativeSaveEnvelope]) -> BundleReport {
    let mut report = BundleReport::default();
    for envelope in characters {
        let id = envelope.character.id.clone();
        if envelope.schema > SAVE_SCHEMA {
            report.unsupported.push(id.clone());
        }
        if envelope.application_version != APPLICATION_VERSION {
            report.version_mismatch.push(id.clone());
        }
    }
    report
}

/// Compare a bundle against a specific rules fingerprint and report mismatches.
pub fn validate_bundle_fingerprint(
    characters: &[NativeSaveEnvelope],
    fingerprint: &str,
) -> BundleReport {
    let mut report = validate_bundle(characters);
    for envelope in characters {
        if envelope.rules_fingerprint != fingerprint {
            report
                .fingerprint_mismatch
                .push(envelope.character.id.clone());
        }
    }
    report
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
            // Push the previous version onto the undo stack so `undo` can step
            // back through every saved revision.
            let history = self.character_history_dir(&id);
            fs::create_dir_all(&history)?;
            let next = self.next_history_index(&history)?;
            fs::copy(&final_path, history.join(format!("{next:06}.json")))?;
            fs::remove_file(&final_path)?;
        }
        // A fresh write invalidates any redo history.
        let redo = self.character_redo_dir(&id);
        if redo.exists() {
            let _ = fs::remove_dir_all(&redo);
        }
        fs::rename(&temporary, &final_path)?;
        Ok(())
    }

    fn history_dir(&self) -> PathBuf {
        self.root.join(".history")
    }

    fn character_history_dir(&self, id: &str) -> PathBuf {
        self.history_dir().join(id)
    }

    fn character_redo_dir(&self, id: &str) -> PathBuf {
        self.history_dir().join(format!("{id}.redo"))
    }

    fn snapshot_entries(&self, dir: &Path) -> Result<Vec<PathBuf>, BuilderError> {
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect();
        entries.sort();
        Ok(entries)
    }

    /// Write `bytes` as the live version, moving the current one into `push_dir`
    /// when it exists.
    fn swap_live_version(
        &self,
        id: &str,
        bytes: &[u8],
        push_dir: Option<&Path>,
    ) -> Result<(), BuilderError> {
        let current = self.root.join(format!("{id}.json"));
        if let Some(dir) = push_dir {
            if current.exists() {
                fs::create_dir_all(dir)?;
                let next = self.next_history_index(dir)?;
                fs::copy(&current, dir.join(format!("{next:06}.json")))?;
            }
        }
        let temporary = self.root.join(format!(".{id}.json.tmp"));
        fs::write(&temporary, bytes)?;
        fs::rename(&temporary, &current)?;
        Ok(())
    }

    fn next_history_index(&self, dir: &Path) -> Result<u32, BuilderError> {
        let mut highest = 0u32;
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if let Some(stem) = entry.path().file_stem().and_then(|value| value.to_str()) {
                if let Ok(index) = stem.parse::<u32>() {
                    highest = highest.max(index);
                }
            }
        }
        Ok(highest + 1)
    }

    fn snapshot_meta(
        &self,
        path: &Path,
        index: usize,
        stack: &str,
        live: Option<&NativeSaveEnvelope>,
    ) -> Option<CharacterVersion> {
        let bytes = fs::read(path).ok()?;
        let envelope: NativeSaveEnvelope = serde_json::from_slice(&bytes).ok()?;
        let saved_at_unix = fs::metadata(path)
            .ok()
            .and_then(|meta| meta.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        let summary = live
            .map(|live| summarize(&envelope.character, &live.character))
            .unwrap_or_default();
        Some(CharacterVersion {
            index,
            name: envelope.character.name.clone(),
            total_level: envelope.character.total_level(),
            saved_at_unix,
            stack: stack.to_string(),
            summary,
        })
    }

    /// List the saved revisions available to `undo` and `redo`, oldest first.
    pub fn history(&self, id: &str) -> Result<Vec<CharacterVersion>, BuilderError> {
        let id = sanitize_id(id)?;
        let live = self.load(&id).ok();
        let mut versions = Vec::new();
        for (index, path) in self
            .snapshot_entries(&self.character_history_dir(&id))?
            .iter()
            .enumerate()
        {
            if let Some(version) = self.snapshot_meta(path, index + 1, "undo", live.as_ref()) {
                versions.push(version);
            }
        }
        for (index, path) in self
            .snapshot_entries(&self.character_redo_dir(&id))?
            .iter()
            .enumerate()
        {
            if let Some(version) = self.snapshot_meta(path, index + 1, "redo", live.as_ref()) {
                versions.push(version);
            }
        }
        Ok(versions)
    }

    /// Restore the `index`-th revision of `id` (1-based, oldest first).
    ///
    /// The current live version is pushed onto the appropriate stack so the
    /// jump itself can be undone.
    pub fn restore_version(
        &self,
        id: &str,
        index: usize,
    ) -> Result<NativeSaveEnvelope, BuilderError> {
        let id = sanitize_id(id)?;
        if index == 0 {
            return Err(BuilderError::NotFound(id));
        }
        let undo_dir = self.character_history_dir(&id);
        let undo_entries = self.snapshot_entries(&undo_dir)?;
        let (source, push_dir) = if index <= undo_entries.len() {
            (undo_entries[index - 1].clone(), undo_dir.clone())
        } else {
            let redo_index = index - undo_entries.len();
            let redo_dir = self.character_redo_dir(&id);
            let redo_entries = self.snapshot_entries(&redo_dir)?;
            if redo_index > redo_entries.len() {
                return Err(BuilderError::NotFound(id));
            }
            (redo_entries[redo_index - 1].clone(), redo_dir)
        };
        let bytes = fs::read(&source)?;
        let envelope: NativeSaveEnvelope = serde_json::from_slice(&bytes)?;
        fs::remove_file(&source)?;
        self.swap_live_version(&id, &bytes, Some(&push_dir))?;
        Ok(envelope)
    }

    /// Number of saved revisions available to `undo` for `id`.
    pub fn history_depth(&self, id: &str) -> Result<usize, BuilderError> {
        let id = sanitize_id(id)?;
        let dir = self.character_history_dir(&id);
        if !dir.exists() {
            return Ok(0);
        }
        Ok(fs::read_dir(dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|v| v.to_str()) == Some("json"))
            .count())
    }

    /// True when a previous version of `id` is available for `undo`.
    pub fn can_undo(&self, id: &str) -> Result<bool, BuilderError> {
        Ok(self.history_depth(id)? > 0)
    }

    /// Restore the previous saved revision of `id`.
    ///
    /// Each call pops the newest undo entry and records the version it replaced
    /// on the redo stack, so `redo` can step forward again.
    pub fn undo(&self, id: &str) -> Result<NativeSaveEnvelope, BuilderError> {
        let id = sanitize_id(id)?;
        let dir = self.character_history_dir(&id);
        let entries = self.snapshot_entries(&dir)?;
        let Some(backup) = entries.last().cloned() else {
            return Err(BuilderError::NotFound(id));
        };
        let previous_bytes = fs::read(&backup)?;
        let envelope: NativeSaveEnvelope = serde_json::from_slice(&previous_bytes)?;
        fs::remove_file(&backup)?;
        let redo = self.character_redo_dir(&id);
        self.swap_live_version(&id, &previous_bytes, Some(&redo))?;
        Ok(envelope)
    }

    /// True when a `redo` is available for `id`.
    pub fn can_redo(&self, id: &str) -> Result<bool, BuilderError> {
        Ok(self.redo_depth(id)? > 0)
    }

    /// Number of redo steps available for `id`.
    pub fn redo_depth(&self, id: &str) -> Result<usize, BuilderError> {
        let id = sanitize_id(id)?;
        Ok(self.snapshot_entries(&self.character_redo_dir(&id))?.len())
    }

    /// Re-apply the most recently undone revision.
    pub fn redo(&self, id: &str) -> Result<NativeSaveEnvelope, BuilderError> {
        let id = sanitize_id(id)?;
        let redo = self.character_redo_dir(&id);
        let entries = self.snapshot_entries(&redo)?;
        let Some(entry) = entries.last().cloned() else {
            return Err(BuilderError::NotFound(id));
        };
        let bytes = fs::read(&entry)?;
        let envelope: NativeSaveEnvelope = serde_json::from_slice(&bytes)?;
        fs::remove_file(&entry)?;
        let history = self.character_history_dir(&id);
        self.swap_live_version(&id, &bytes, Some(&history))?;
        Ok(envelope)
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
    fn undo_restores_previous_version() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("hero", "Hero", 1),
                "fp",
                None,
            ))
            .unwrap();
        assert!(!repository.can_undo("hero").unwrap());

        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("hero", "Hero", 2),
                "fp",
                None,
            ))
            .unwrap();
        assert!(repository.can_undo("hero").unwrap());
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 2);

        let restored = repository.undo("hero").unwrap();
        assert_eq!(restored.character.total_level(), 1);
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 1);
        // The stack is now empty, so a second undo fails.
        assert!(!repository.can_undo("hero").unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_walks_back_multiple_steps() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        for level in 1..=4 {
            repository
                .save(&NativeSaveEnvelope::new(
                    sample_character("hero", "Hero", level),
                    "fp",
                    None,
                ))
                .unwrap();
        }
        assert_eq!(repository.history_depth("hero").unwrap(), 3);
        for expected in [3, 2, 1] {
            repository.undo("hero").unwrap();
            assert_eq!(
                repository.load("hero").unwrap().character.total_level(),
                expected
            );
        }
        assert_eq!(repository.history_depth("hero").unwrap(), 0);
        assert!(matches!(
            repository.undo("hero"),
            Err(BuilderError::NotFound(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn undo_then_redo_round_trips() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        for level in 1..=3 {
            repository
                .save(&NativeSaveEnvelope::new(
                    sample_character("hero", "Hero", level),
                    "fp",
                    None,
                ))
                .unwrap();
        }
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 3);
        assert_eq!(repository.redo_depth("hero").unwrap(), 0);

        repository.undo("hero").unwrap();
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 2);
        assert!(repository.can_redo("hero").unwrap());
        assert_eq!(repository.redo_depth("hero").unwrap(), 1);

        repository.redo("hero").unwrap();
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 3);
        assert!(!repository.can_redo("hero").unwrap());

        // A new write clears the redo stack.
        repository.undo("hero").unwrap();
        assert!(repository.can_redo("hero").unwrap());
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("hero", "Hero", 5),
                "fp",
                None,
            ))
            .unwrap();
        assert!(!repository.can_redo("hero").unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn history_lists_versions_and_restore_jumps() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        for level in 1..=4 {
            repository
                .save(&NativeSaveEnvelope::new(
                    sample_character("hero", "Hero", level),
                    "fp",
                    None,
                ))
                .unwrap();
        }
        // Undo stack holds levels 1..3 (oldest first).
        let versions = repository.history("hero").unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].total_level, 1);
        assert_eq!(versions[2].total_level, 3);
        assert_eq!(versions[2].stack, "undo");

        // Jump straight back to the oldest revision.
        let restored = repository.restore_version("hero", 1).unwrap();
        assert_eq!(restored.character.total_level(), 1);
        assert_eq!(repository.load("hero").unwrap().character.total_level(), 1);
        // The jump pushed the previous live version (level 4) onto the stack.
        assert!(repository
            .history("hero")
            .unwrap()
            .iter()
            .any(|version| version.total_level == 4));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn history_summarizes_against_live_version() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        for level in 1..=3 {
            repository
                .save(&NativeSaveEnvelope::new(
                    sample_character("hero", "Hero", level),
                    "fp",
                    None,
                ))
                .unwrap();
        }
        // Live is level 3; revision 2 is level 2.
        let versions = repository.history("hero").unwrap();
        let second = versions.iter().find(|v| v.total_level == 2).unwrap();
        assert_eq!(second.summary.level_delta, -1);
        assert!(!second.summary.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn history_summary_lists_ability_changes() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("hero", "Hero", 1),
                "fp",
                None,
            ))
            .unwrap();
        let mut with_feat = sample_character("hero", "Hero", 1);
        with_feat
            .abilities
            .push(("FEAT".to_string(), "Power Attack".to_string()));
        repository
            .save(&NativeSaveEnvelope::new(with_feat, "fp", None))
            .unwrap();

        // Live has the feat; the older revision does not.
        let versions = repository.history("hero").unwrap();
        assert_eq!(versions.len(), 1);
        let summary = &versions[0].summary;
        assert_eq!(
            summary.abilities_added,
            vec!["FEAT: Power Attack".to_string()]
        );
        assert!(summary.abilities_removed.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn validate_bundle_flags_schema_and_fingerprint() {
        let ok = NativeSaveEnvelope::new(sample_character("ok", "Ok", 1), "fp", None);
        let mut newer = NativeSaveEnvelope::new(sample_character("new", "New", 1), "fp", None);
        newer.schema = SAVE_SCHEMA + 1;
        let mut other_fp = NativeSaveEnvelope::new(sample_character("fp", "FP", 1), "other", None);
        other_fp.application_version = "0.0.0".to_string();

        let report = validate_bundle_fingerprint(&[ok, newer, other_fp], "fp");
        assert_eq!(report.unsupported, vec!["new".to_string()]);
        assert_eq!(report.fingerprint_mismatch, vec!["fp".to_string()]);
        assert_eq!(report.version_mismatch, vec!["fp".to_string()]);
        assert!(!report.is_compatible());
        assert_eq!(report.warnings().len(), 3);
    }

    #[test]
    fn undo_without_history_is_not_found() {
        let root = temp_root();
        let repository = CharacterRepository::new(&root);
        repository
            .save(&NativeSaveEnvelope::new(
                sample_character("hero", "Hero", 1),
                "fp",
                None,
            ))
            .unwrap();
        assert!(matches!(
            repository.undo("hero"),
            Err(BuilderError::NotFound(_))
        ));
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
