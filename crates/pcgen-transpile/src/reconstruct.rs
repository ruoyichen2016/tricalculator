//! Turn validated records back into runtime definitions.
//!
//! This is the load side of Stage 1.  It proves the format carries enough to
//! drive the engine: the class domain is rebuilt from records and everything
//! else is left as the pack already had it.
//!
//! Source formulas survive alongside tables so dynamic character dependencies remain intact.

use crate::format::{ClassGrant, ClassLevelRecord, ClassRecord, VariantRecord};
use pcgen_core::{ClassDefinition, ClassProgression, VariantDefinition};
use std::collections::BTreeMap;

/// Rebuild the class, progression and variant maps from normalised records.
pub fn definitions(
    class_records: &[ClassRecord],
    class_levels: &[ClassLevelRecord],
    variant_records: &[VariantRecord],
) -> (
    BTreeMap<String, ClassDefinition>,
    BTreeMap<String, ClassProgression>,
    BTreeMap<String, VariantDefinition>,
) {
    let mut grants_by_class: BTreeMap<&str, BTreeMap<u32, Vec<ClassGrant>>> = BTreeMap::new();
    for level in class_levels {
        grants_by_class
            .entry(level.class.as_str())
            .or_default()
            .entry(level.level)
            .or_default()
            .extend(level.grants.iter().cloned());
    }

    let mut classes = BTreeMap::new();
    let mut progressions = BTreeMap::new();
    for record in class_records {
        let grants = grants_by_class
            .remove(record.id.as_str())
            .unwrap_or_default();
        classes.insert(record.id.clone(), class_definition(record, &grants));
        progressions.insert(record.id.clone(), progression(record));
    }

    let variants = variant_records
        .iter()
        .map(|record| (record.id.clone(), variant_definition(record)))
        .collect();

    (classes, progressions, variants)
}

fn class_definition(
    record: &ClassRecord,
    grants: &BTreeMap<u32, Vec<ClassGrant>>,
) -> ClassDefinition {
    let mut definition = record.definition.clone();
    // The level maps live on `class_level` records now; restore them so the
    // parts of the pipeline that read them keep working.
    definition.level_abilities = grants.clone();
    definition
}

fn progression(record: &ClassRecord) -> ClassProgression {
    // Nothing to restore: the grants live on the class, and the progression no
    // longer carries a copy of them.
    record.progression.clone()
}

fn variant_definition(record: &VariantRecord) -> VariantDefinition {
    let mut variant = VariantDefinition::new(record.id.clone(), record.name.clone());
    variant.applies_to = record.applies_to.clone();
    variant.replaces = record
        .replaces
        .iter()
        .map(|replace| format!("TYPE={}", replace.anchor))
        .collect();
    variant.grants = record
        .grants
        .iter()
        .map(
            |grant| match (grant.at_level, record.applies_to.as_deref()) {
                // The level guard needs the owning class; the variant states it.
                (Some(level), Some(class)) => {
                    format!("{}|PRECLASS:1,{class}={level}", grant.feature)
                }
                _ => grant.feature.clone(),
            },
        )
        .collect();
    variant.class_skills = record.class_skills.clone();
    variant
}

/// Level rows for a class, for callers that want them grouped.
pub fn levels_for<'a>(levels: &'a [ClassLevelRecord], class: &str) -> Vec<&'a ClassLevelRecord> {
    levels.iter().filter(|level| level.class == class).collect()
}
