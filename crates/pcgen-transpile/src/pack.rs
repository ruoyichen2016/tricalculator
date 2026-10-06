//! Reading and writing a normalised pack, with validation on load.
//!
//! R8: a loader rejects an inconsistent pack rather than loading it into a
//! wrong-but-plausible state.  These checks are the structural form of the
//! invariants `tree_scan` has to search for today — a class level naming an
//! undeclared class is refused here, where in the PCGen format it was a silently
//! mis-keyed entity that only a whole-tree scan could find.
//!
//! # What is persisted
//!
//! The class domain is written in its normalised form: `classes.jsonl` holds
//! [`ClassRecord`]s, `class_levels.jsonl` holds levels that name their own
//! class, `variants.jsonl` holds structured replacements.
//!
//! Every other domain is written structurally, one record per line, by domain —
//! reviewable, but not yet normalised: those fields still carry PCGen
//! conventions, and later stages flatten them the way stage 1 flattened the
//! class domain.
//!
//! The reason to write the *whole* pack is that loading then no longer has to
//! interpret PCGen text at all.  [`read_pack`] reassembles a
//! [`CompiledRulePack`] from records, so the runtime path does not run the
//! translator.

use crate::format::{
    ClassLevelRecord, ClassRecord, LoadError, PackManifest, VariantRecord, FORMAT_VERSION,
};
use pcgen_core::CompiledRulePack;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// File names inside a pack directory.
pub const MANIFEST_FILE: &str = "manifest.json";
pub const CLASSES_FILE: &str = "classes.jsonl";
pub const CLASS_LEVELS_FILE: &str = "class_levels.jsonl";
pub const VARIANTS_FILE: &str = "variants.jsonl";

/// Pack fields written as one record per line (a list of records).
const LIST_FIELDS: &[&str] = &["entities"];

/// Pack fields written as one `{"key":..,"value":..}` record per line.
///
/// The class domain is absent on purpose: `classes`, `progressions` and
/// `variants` are written in normalised form instead.
const MAP_FIELDS: &[&str] = &[
    "races",
    "skills",
    "spells",
    "equipment",
    "equipment_modifiers",
    "deities",
    "domains",
    "kits",
    "templates",
    "abilities",
    "ability_categories",
    "creature_trait_records",
    "companion_mods",
    "sizes",
    "stats",
    "alignments",
];

/// Pack fields written as a single JSON object.
const SCALAR_FIELDS: &[&str] = &["header", "coverage"];

/// A validated pack: a complete runtime pack plus what was noticed on the way.
#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub manifest: PackManifest,
    /// The reassembled runtime pack.  Nothing here came from parsing PCGen text.
    pub pack: CompiledRulePack,
    /// The normalised class records, kept so the normalised form can be
    /// inspected and compared without re-transpiling.
    pub class_records: Vec<ClassRecord>,
    pub class_levels: Vec<ClassLevelRecord>,
    pub variant_records: Vec<VariantRecord>,
    /// Things worth knowing that are not violations.  A class with no level
    /// rows is the example: it is suspicious for a normal class, but both
    /// monster (racial HD) classes and hidden spell-list carriers legitimately
    /// have none, and the model does not carry the flag that separates them.
    pub warnings: Vec<String>,
}

/// Write a pack to `dir`.
///
/// The class domain is transpiled from `pack` and written in normalised form;
/// every other domain is written structurally.
pub fn write_pack(
    dir: &Path,
    pack: &CompiledRulePack,
    records: &[pcgen_ir::source::RawRecord],
    entry: &str,
) -> Result<PackManifest, LoadError> {
    std::fs::create_dir_all(dir).map_err(|error| LoadError::Io(error.to_string()))?;

    let transpiled = crate::transpile::transpile_pack(pack, records, entry);
    write_json(&dir.join(MANIFEST_FILE), &transpiled.manifest)?;

    // The class domain, in normalised form.
    for (file, kind) in [
        (CLASSES_FILE, "class"),
        (CLASS_LEVELS_FILE, "class_level"),
        (VARIANTS_FILE, "variant"),
    ] {
        let values = transpiled
            .records
            .iter()
            .filter(|record| record.kind() == kind)
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(malformed)?;
        write_lines(&dir.join(file), &values)?;
    }

    // Everything else, structurally.
    let value = serde_json::to_value(pack).map_err(malformed)?;
    let object = value
        .as_object()
        .ok_or_else(|| LoadError::Malformed("pack did not serialise to an object".into()))?;

    for field in LIST_FIELDS {
        let items = object
            .get(*field)
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        write_lines(&dir.join(format!("{field}.jsonl")), &items)?;
    }
    for field in MAP_FIELDS {
        let entries = object
            .get(*field)
            .and_then(|value| value.as_object())
            .map(|map| {
                map.iter()
                    .map(|(key, value)| serde_json::json!({ "key": key, "value": value }))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        write_lines(&dir.join(format!("{field}.jsonl")), &entries)?;
    }
    for field in SCALAR_FIELDS {
        let value = object
            .get(*field)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        write_json(&dir.join(format!("{field}.json")), &value)?;
    }

    Ok(transpiled.manifest)
}

/// Read and validate a pack, reassembling a complete runtime pack.
pub fn read_pack(dir: &Path) -> Result<LoadedPack, LoadError> {
    let manifest_path = dir.join(MANIFEST_FILE);
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .map_err(|error| LoadError::Io(format!("{}: {error}", manifest_path.display())))?;
    let manifest: PackManifest = serde_json::from_str(manifest_text.trim()).map_err(malformed)?;

    if manifest.format != FORMAT_VERSION {
        return Err(LoadError::Version {
            found: manifest.format,
            expected: FORMAT_VERSION,
        });
    }

    let class_records: Vec<ClassRecord> = read_records(dir, CLASSES_FILE)?;
    let class_levels: Vec<ClassLevelRecord> = read_records(dir, CLASS_LEVELS_FILE)?;
    let variant_records: Vec<VariantRecord> = read_records(dir, VARIANTS_FILE)?;

    let warnings = validate(&class_records, &class_levels, &variant_records)?;

    // Reassemble the runtime pack: the class domain from its normalised
    // records, every other domain from its own file.
    let mut object = serde_json::Map::new();
    for field in LIST_FIELDS {
        let items = read_records::<serde_json::Value>(dir, &format!("{field}.jsonl"))?;
        object.insert((*field).to_string(), serde_json::Value::Array(items));
    }
    for field in MAP_FIELDS {
        let entries = read_records::<KeyedValue>(dir, &format!("{field}.jsonl"))?;
        let mut map = serde_json::Map::new();
        for entry in entries {
            map.insert(entry.key, entry.value);
        }
        object.insert((*field).to_string(), serde_json::Value::Object(map));
    }
    for field in SCALAR_FIELDS {
        let value: serde_json::Value = read_json(&dir.join(format!("{field}.json")))?;
        object.insert((*field).to_string(), value);
    }

    let (classes, progressions, variants) =
        crate::reconstruct::definitions(&class_records, &class_levels, &variant_records);
    object.insert(
        "classes".to_string(),
        serde_json::to_value(classes).map_err(malformed)?,
    );
    object.insert(
        "progressions".to_string(),
        serde_json::to_value(progressions).map_err(malformed)?,
    );
    object.insert(
        "variants".to_string(),
        serde_json::to_value(variants).map_err(malformed)?,
    );

    let pack: CompiledRulePack =
        serde_json::from_value(serde_json::Value::Object(object)).map_err(malformed)?;

    Ok(LoadedPack {
        manifest,
        pack,
        class_records,
        class_levels,
        variant_records,
        warnings,
    })
}

#[derive(Debug, serde::Deserialize)]
struct KeyedValue {
    key: String,
    value: serde_json::Value,
}

/// The invariants a pack must satisfy.
///
/// Returns the diagnostics that are not violations.
fn validate(
    classes: &[ClassRecord],
    class_levels: &[ClassLevelRecord],
    variants: &[VariantRecord],
) -> Result<Vec<String>, LoadError> {
    let declared: BTreeSet<&str> = classes.iter().map(|class| class.id.as_str()).collect();

    // A level row that names no declared class is the structural equivalent of
    // a hijacked cursor: its features would attach to nothing.
    for level in class_levels {
        if !declared.contains(level.class.as_str()) {
            return Err(LoadError::UnknownClass {
                class: level.class.clone(),
                level: level.level,
            });
        }
    }

    // A class with no level rows would have lost every feature it grants, but
    // this is a warning rather than a rejection: the RSRD's monster (racial HD)
    // classes have none by design, and so do hidden spell-list carriers such as
    // `Sorcerer/Cleric (Arcane)`.  `ClassDefinition` carries no visibility flag,
    // so the two cannot be told apart here; a later stage adds the flag and can
    // then promote this back to an error.
    let with_levels: BTreeSet<&str> = class_levels
        .iter()
        .map(|level| level.class.as_str())
        .collect();
    let mut warnings = Vec::new();
    for class in classes {
        if !with_levels.contains(class.id.as_str()) {
            warnings.push(format!(
                "class `{}` declares no levels (monster or hidden carrier?)",
                class.id
            ));
        }
    }

    for variant in variants {
        if let Some(class) = &variant.applies_to {
            if !declared.contains(class.as_str()) {
                return Err(LoadError::VariantUnknownClass {
                    variant: variant.id.clone(),
                    class: class.clone(),
                });
            }
        }
    }

    Ok(warnings)
}

fn read_records<T: serde::de::DeserializeOwned>(
    dir: &Path,
    file: &str,
) -> Result<Vec<T>, LoadError> {
    let path = dir.join(file);
    let Ok(text) = std::fs::read_to_string(&path) else {
        // A pack may legitimately hold no records of a kind.
        return Ok(Vec::new());
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(malformed))
        .collect()
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, LoadError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| LoadError::Io(format!("{}: {error}", path.display())))?;
    serde_json::from_str(&text).map_err(malformed)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), LoadError> {
    let text = serde_json::to_string_pretty(value).map_err(malformed)?;
    std::fs::write(path, text)
        .map_err(|error| LoadError::Io(format!("{}: {error}", path.display())))
}

fn write_lines(path: &Path, values: &[serde_json::Value]) -> Result<(), LoadError> {
    let mut out = String::new();
    for value in values {
        out.push_str(&serde_json::to_string(value).map_err(malformed)?);
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|error| LoadError::Io(format!("{}: {error}", path.display())))
}

fn malformed(error: serde_json::Error) -> LoadError {
    LoadError::Malformed(error.to_string())
}
