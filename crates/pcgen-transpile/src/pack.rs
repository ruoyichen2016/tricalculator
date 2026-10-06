//! Reading and writing a normalised pack, with validation on load.
//!
//! R8: a loader rejects an inconsistent pack rather than loading it into a
//! wrong-but-plausible state.  These checks are the structural form of the
//! invariants `tree_scan` has to search for today — a class level naming an
//! undeclared class is refused here, where in the PCGen format it was a silently
//! mis-keyed entity that only a whole-tree scan could find.

use crate::format::{
    ClassLevelRecord, ClassRecord, LoadError, PackManifest, Record, VariantRecord, FORMAT_VERSION,
};
use std::collections::BTreeSet;
use std::path::Path;

/// File names inside a pack directory.
pub const MANIFEST_FILE: &str = "manifest.json";
pub const CLASSES_FILE: &str = "classes.jsonl";
pub const CLASS_LEVELS_FILE: &str = "class_levels.jsonl";
pub const VARIANTS_FILE: &str = "variants.jsonl";

/// A validated pack, ready to be turned back into runtime definitions.
#[derive(Debug, Clone)]
pub struct LoadedPack {
    pub manifest: PackManifest,
    pub classes: Vec<ClassRecord>,
    pub class_levels: Vec<ClassLevelRecord>,
    pub variants: Vec<VariantRecord>,
    /// Things worth knowing that are not violations.  A class with no level
    /// rows is the example: it is suspicious for a normal class, but both
    /// monster (racial HD) classes and hidden spell-list carriers legitimately
    /// have none, and the model does not carry the flag that separates them.
    pub warnings: Vec<String>,
}

/// Write a pack to `dir`, one file per record kind.
pub fn write_pack(
    dir: &Path,
    manifest: &PackManifest,
    records: &[Record],
) -> Result<(), LoadError> {
    std::fs::create_dir_all(dir).map_err(|error| LoadError::Io(error.to_string()))?;

    write_lines(
        &dir.join(MANIFEST_FILE),
        &[serde_json::to_value(manifest).map_err(malformed)?],
    )?;
    write_lines(
        &dir.join(CLASSES_FILE),
        &records
            .iter()
            .filter(|record| matches!(record, Record::Class(_)))
            .map(to_value)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    write_lines(
        &dir.join(CLASS_LEVELS_FILE),
        &records
            .iter()
            .filter(|record| matches!(record, Record::ClassLevel(_)))
            .map(to_value)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    write_lines(
        &dir.join(VARIANTS_FILE),
        &records
            .iter()
            .filter(|record| matches!(record, Record::Variant(_)))
            .map(to_value)
            .collect::<Result<Vec<_>, _>>()?,
    )?;
    Ok(())
}

/// Read and validate a pack.
pub fn read_pack(dir: &Path) -> Result<LoadedPack, LoadError> {
    let manifest_text = std::fs::read_to_string(dir.join(MANIFEST_FILE)).map_err(|error| {
        LoadError::Io(format!("{}: {error}", dir.join(MANIFEST_FILE).display()))
    })?;
    let manifest: PackManifest = serde_json::from_str(manifest_text.trim()).map_err(malformed)?;

    if manifest.format != FORMAT_VERSION {
        return Err(LoadError::Version {
            found: manifest.format,
            expected: FORMAT_VERSION,
        });
    }

    let classes: Vec<ClassRecord> = read_records(dir, CLASSES_FILE)?;
    let class_levels: Vec<ClassLevelRecord> = read_records(dir, CLASS_LEVELS_FILE)?;
    let variants: Vec<VariantRecord> = read_records(dir, VARIANTS_FILE)?;

    let warnings = validate(&classes, &class_levels, &variants)?;

    Ok(LoadedPack {
        manifest,
        classes,
        class_levels,
        variants,
        warnings,
    })
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
    // so the two cannot be told apart here; Stage 2 adds the flag and can then
    // promote this back to an error.
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

fn write_lines(path: &Path, values: &[serde_json::Value]) -> Result<(), LoadError> {
    let mut out = String::new();
    for value in values {
        out.push_str(&serde_json::to_string(value).map_err(malformed)?);
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|error| LoadError::Io(format!("{}: {error}", path.display())))
}

fn to_value(record: &Record) -> Result<serde_json::Value, LoadError> {
    serde_json::to_value(record).map_err(malformed)
}

fn malformed(error: serde_json::Error) -> LoadError {
    LoadError::Malformed(error.to_string())
}
