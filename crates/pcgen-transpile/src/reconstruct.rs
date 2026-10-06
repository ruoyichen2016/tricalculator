//! Turn validated records back into runtime definitions.
//!
//! This is the load side of Stage 1.  It proves the format carries enough to
//! drive the engine: the class domain is rebuilt from records and everything
//! else is left as the pack already had it.
//!
//! Note what does *not* come back: `base_attack_formula` and the save formulas.
//! The format carries per-level tables instead (R5), and the engine falls back
//! to the table when no formula is present.  The transpiler checks the two
//! agree before dropping the formula.

use crate::format::{ClassLevelRecord, ClassRecord, GrantMode, GrantRecord, VariantRecord};
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
    let mut grants_by_class: BTreeMap<&str, BTreeMap<u32, Vec<GrantRecord>>> = BTreeMap::new();
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
        progressions.insert(record.id.clone(), progression(record, &grants));
    }

    let variants = variant_records
        .iter()
        .map(|record| (record.id.clone(), variant_definition(record)))
        .collect();

    (classes, progressions, variants)
}

fn class_definition(
    record: &ClassRecord,
    grants: &BTreeMap<u32, Vec<GrantRecord>>,
) -> ClassDefinition {
    let mut definition = record.definition.clone();
    // The level maps live on `class_level` records now; restore them so the
    // parts of the pipeline that read them keep working.
    definition.level_abilities = grants
        .iter()
        .map(|(level, rows)| (*level, rows.iter().map(render_grant).collect()))
        .collect();
    // No formula: the table is authoritative (R5).
    definition.base_attack_formula = None;
    definition.fortitude_formula = None;
    definition.reflex_formula = None;
    definition.will_formula = None;
    definition
}

fn progression(record: &ClassRecord, grants: &BTreeMap<u32, Vec<GrantRecord>>) -> ClassProgression {
    let mut progression = record.progression.clone();
    progression.features_by_level = grants
        .iter()
        .map(|(level, rows)| (*level, rows.iter().map(render_grant).collect()))
        .collect();
    progression
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

/// Render a structured grant back into the PCGen-shaped row the rest of the
/// pipeline expects.  Stage 2 removes the need for this by teaching those
/// consumers the structured form directly.
fn render_grant(grant: &GrantRecord) -> String {
    let mut row = String::new();
    if let Some(pool) = &grant.pool {
        row.push_str(pool);
        row.push('|');
    }
    row.push_str(match grant.mode {
        GrantMode::Automatic => "AUTOMATIC",
        GrantMode::Selectable => "NORMAL",
    });
    row.push('|');
    row.push_str(&grant.feature);
    if !grant.unless_replaced.is_empty() {
        let category = grant.unless_category.as_deref().unwrap_or("ACF");
        row.push_str("|!PREABILITY:1,CATEGORY=");
        row.push_str(category);
        for anchor in &grant.unless_replaced {
            row.push_str(",TYPE.");
            row.push_str(anchor);
        }
    }
    for guard in &grant.guards {
        row.push('|');
        row.push_str(guard);
    }
    row
}

/// Level rows for a class, for callers that want them grouped.
pub fn levels_for<'a>(levels: &'a [ClassLevelRecord], class: &str) -> Vec<&'a ClassLevelRecord> {
    levels.iter().filter(|level| level.class == class).collect()
}
