//! Transpile a compiled PCGen pack into the normalised format.
//!
//! This is a **build-time** step.  It reuses the existing translation in
//! `pcgen-ir` — which is well tested by now — and then states the result
//! structurally, so the runtime no longer has to recover meaning from PCGen's
//! conventions.

use crate::format::{
    ClassLevelRecord, ClassRecord, Origin, PackManifest, Record, ReplaceRecord, SourceInfo,
    VariantGrantRecord, VariantRecord, FORMAT_VERSION,
};
use pcgen_core::{ClassDefinition, ClassProgression, VariantDefinition};
use pcgen_ir::rulepack::CompiledRulePack;
use pcgen_ir::source::RawRecord;
use std::collections::BTreeMap;
use std::path::Path;

/// Transpiler identity recorded in the manifest.
pub const TRANSPILER: &str = concat!("pcgen-transpile ", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum TranspileError {
    #[error("import failed: {0}")]
    Import(#[from] pcgen_import::ImportError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// The result of transpiling one pack.
#[derive(Debug, Clone)]
pub struct TranspiledPack {
    pub manifest: PackManifest,
    pub records: Vec<Record>,
    /// Things the transpiler noticed but did not treat as fatal, e.g. a
    /// progression formula that disagrees with the table it is replacing.
    pub warnings: Vec<String>,
}

impl TranspiledPack {
    /// Records of one kind, in file order.
    pub fn of_kind(&self, kind: &str) -> Vec<&Record> {
        self.records.iter().filter(|r| r.kind() == kind).collect()
    }
}

/// Transpile the `.pcc` at `entry` into normalised records.
pub fn transpile(entry: &Path) -> Result<TranspiledPack, TranspileError> {
    let loaded = pcgen_import::load_pcc(entry)?;
    let game_mode = loaded
        .graph
        .files
        .iter()
        .find_map(|file| file.game_mode.clone());
    let sources = loaded
        .graph
        .files
        .iter()
        .map(|file| file.path.display().to_string())
        .collect();
    let patched = pcgen_import::apply_record_patches(&loaded.records);
    let (pack, _) = pcgen_ir::rulepack::lower_records(game_mode.clone(), sources, &patched);

    Ok(transpile_pack(
        &pack,
        &patched,
        &entry.display().to_string(),
    ))
}

/// Transpile an already-lowered pack.  `records` supplies provenance only.
pub fn transpile_pack(
    pack: &CompiledRulePack,
    records: &[RawRecord],
    entry: &str,
) -> TranspiledPack {
    let mut warnings = Vec::new();
    let mut out = Vec::new();

    // Provenance: the first class header row for each class key.
    let mut class_origins: BTreeMap<String, Origin> = BTreeMap::new();
    for record in records {
        if !record.category.eq_ignore_ascii_case("CLASS") {
            continue;
        }
        if !pcgen_import::is_class_header_row(record) {
            continue;
        }
        class_origins
            .entry(normalize(&record.key))
            .or_insert_with(|| origin_of(record));
    }

    let mut class_count = 0usize;
    let mut level_count = 0usize;
    let mut variant_count = 0usize;

    for (id, class) in &pack.classes {
        let progression = pack
            .progressions
            .get(id)
            .or_else(|| pack.progressions.get(&normalize(id)));
        out.push(Record::Class(Box::new(class_record(
            class,
            progression,
            class_origins.get(&normalize(id)).cloned(),
            &mut warnings,
        ))));
        class_count += 1;

        for level in class_levels(class) {
            out.push(Record::ClassLevel(level));
            level_count += 1;
        }
    }

    for variant in pack.variants.values() {
        out.push(Record::Variant(variant_record(variant)));
        variant_count += 1;
    }

    let mut counts = BTreeMap::new();
    counts.insert("class".to_string(), class_count);
    counts.insert("class_level".to_string(), level_count);
    counts.insert("variant".to_string(), variant_count);

    let manifest = PackManifest {
        format: FORMAT_VERSION,
        pack_id: pack
            .header
            .game_mode
            .clone()
            .unwrap_or_else(|| "pack".into()),
        game_mode: pack.header.game_mode.clone(),
        name: pack_name(entry),
        source: SourceInfo {
            kind: "pcgen".to_string(),
            entry: entry.to_string(),
            data_ref: pack.header.data_ref.clone(),
            transpiler: TRANSPILER.to_string(),
        },
        counts,
    };

    TranspiledPack {
        manifest,
        records: out,
        warnings,
    }
}

/// A display name for the pack.  The compiled header carries no name, so use
/// the entry file's stem, which is what a human would recognise.
fn pack_name(entry: &str) -> String {
    Path::new(entry)
        .file_stem()
        .map(|stem| stem.to_string_lossy().trim_start_matches('_').to_string())
        .unwrap_or_else(|| entry.to_string())
}

fn origin_of(record: &RawRecord) -> Origin {
    Origin::new(record.source.path.display().to_string(), record.source.line)
}

fn normalize(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.contains(':') || trimmed.contains('|') {
        trimmed.to_string()
    } else {
        trimmed.to_ascii_lowercase().replace(' ', "_")
    }
}

fn class_record(
    class: &ClassDefinition,
    progression: Option<&ClassProgression>,
    origin: Option<Origin>,
    warnings: &mut Vec<String>,
) -> ClassRecord {
    let empty = ClassProgression::default();
    let progression = progression.unwrap_or(&empty);

    // R5: the formula string is dropped, and the table takes over.  The engine
    // currently prefers the formula, so the transpiler checks they agree rather
    // than assuming it.  For the RSRD they agree at every class and level.
    if let Some(formula) = &class.base_attack_formula {
        let mismatched = (1..=class.max_level.min(20)).any(|level| {
            let from_table = progression.bab(level);
            let from_formula = pcgen_engine::evaluate_class_formula(
                formula,
                &class.id,
                level,
                level,
                &BTreeMap::new(),
            );
            from_formula != from_table
        });
        if mismatched {
            warnings.push(format!(
                "class `{}`: base attack formula `{formula}` disagrees with the \
                 progression table; the table was written",
                class.id
            ));
        }
    }

    // The per-level maps move to `class_level` records, so clear them here
    // rather than storing the same rows twice.
    let mut definition = class.clone();
    definition.level_abilities.clear();
    definition.level_bonuses.clear();
    definition.level_prerequisites.clear();
    // R5: the tables in `progression` are authoritative, so the formula
    // strings do not travel.
    definition.base_attack_formula = None;
    definition.fortitude_formula = None;
    definition.reflex_formula = None;
    definition.will_formula = None;

    let carried = progression.clone();

    ClassRecord {
        id: class.id.clone(),
        name: class.name.clone(),
        progression: carried,
        definition,
        origin,
    }
}

/// Every class level the pack knows about, with its owner stated.
///
/// The grants arrive structured, so this only copies them.  It used to parse
/// the raw rows a second time — the same string `lower_grant` had already
/// parsed — which is what put two representations of one fact in the pack.
fn class_levels(class: &ClassDefinition) -> Vec<ClassLevelRecord> {
    class
        .level_abilities
        .iter()
        .map(|(level, grants)| ClassLevelRecord {
            class: class.id.clone(),
            level: *level,
            grants: grants.clone(),
            origin: None,
        })
        .collect()
}

fn variant_record(variant: &VariantDefinition) -> VariantRecord {
    VariantRecord {
        id: variant.id.clone(),
        name: variant.name.clone(),
        applies_to: variant.applies_to.clone(),
        replaces: variant
            .replaces
            .iter()
            .map(|anchor| ReplaceRecord {
                anchor: strip_type_prefix(anchor),
            })
            .collect(),
        grants: variant
            .grant_entries()
            .iter()
            .map(|grant| VariantGrantRecord {
                feature: grant.name.clone(),
                at_level: grant.level,
            })
            .collect(),
        class_skills: variant.class_skills.clone(),
        origin: None,
    }
}

/// `REPLACES:TYPE=WizardFamiliar` is stored as `TYPE=WizardFamiliar`; the
/// anchor is the part after the prefix.
fn strip_type_prefix(value: &str) -> String {
    value
        .trim()
        .strip_prefix("TYPE=")
        .unwrap_or(value.trim())
        .to_string()
}
