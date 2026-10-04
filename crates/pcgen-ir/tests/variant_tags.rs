//! Variant (`TYPE:Archetype` / `TYPE:Substitution*`) tag accounting.
//!
//! `parse_variant` reads `APPLIESTO` / `REPLACES` / `GRANTS`, so those tags are
//! consumed for a variant record and must not additionally show up as legacy
//! (un-lowered) tags.  Without the `TYPE` marker nothing consumes them, and
//! they must stay reported rather than be silently ignored.

use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/abilities.lst", 1, "abilities.pcc");
    RawRecord {
        category: category.to_string(),
        key: name.to_string(),
        name: name.to_string(),
        tags: tags
            .iter()
            .map(|(n, v)| RawTag {
                name: (*n).to_string(),
                value: (*v).to_string(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

fn lower(record: RawRecord) -> (pcgen_ir::CompiledRulePack, pcgen_ir::ImportReport) {
    lower_records(
        Some("35e".to_string()),
        vec!["abilities.pcc".to_string()],
        &[record],
    )
}

#[test]
fn variant_record_is_parsed_and_its_tags_are_not_legacy() {
    let r = record(
        "ABILITY",
        "Weapon Master",
        &[
            ("TYPE", "Archetype"),
            ("APPLIESTO", "Fighter"),
            ("REPLACES", "Fighter Bonus Feat"),
            ("GRANTS", "Weapon Training"),
            ("GRANTS", "Weapon Guard"),
            ("CSKILL", "Intimidate"),
        ],
    );
    let (pack, report) = lower(r);

    let variant = pack
        .variants
        .get("Weapon Master")
        .expect("variant should be parsed");
    assert_eq!(variant.applies_to.as_deref(), Some("Fighter"));
    assert_eq!(variant.replaces, vec!["Fighter Bonus Feat".to_string()]);
    assert_eq!(
        variant.grants,
        vec!["Weapon Training".to_string(), "Weapon Guard".to_string()]
    );
    assert_eq!(variant.class_skills, vec!["Intimidate".to_string()]);

    for tag in ["APPLIESTO", "REPLACES", "GRANTS"] {
        assert_eq!(
            report.coverage.legacy_by_tag.get(tag),
            None,
            "{tag} is consumed by the variant parser but still counted as legacy"
        );
        assert!(
            !pack.entities[0]
                .legacy_tags
                .iter()
                .any(|value| value.starts_with(&format!("{tag}:"))),
            "{tag} still listed in entity legacy_tags: {:?}",
            pack.entities[0].legacy_tags
        );
    }
    assert!(
        report.coverage.legacy_pending_by_tag.is_empty(),
        "unexpected legacy pending tags: {:?}",
        report.coverage.legacy_pending_by_tag
    );
}

#[test]
fn substitution_type_marker_is_recognised_too() {
    let r = record(
        "ABILITY",
        "Cloistered Cleric",
        &[
            ("TYPE", "SubstitutionLevel"),
            ("APPLIESTO", "Cleric"),
            ("REPLACES", "Armor Proficiency (Light)"),
        ],
    );
    let (pack, report) = lower(r);
    assert!(pack.variants.contains_key("Cloistered Cleric"));
    assert!(
        report.coverage.legacy_pending_by_tag.is_empty(),
        "unexpected legacy pending tags: {:?}",
        report.coverage.legacy_pending_by_tag
    );
}

#[test]
fn variant_tags_without_the_type_marker_stay_legacy() {
    // No `TYPE:Archetype` / `TYPE:Substitution*`: `parse_variant` returns
    // `None`, nothing consumes these tags, so they must remain visible.
    let r = record("ABILITY", "Plain", &[("APPLIESTO", "Fighter")]);
    let (pack, report) = lower(r);

    assert!(pack.variants.is_empty());
    assert_eq!(
        report.coverage.legacy_pending_by_tag.get("APPLIESTO"),
        Some(&1)
    );
}

#[test]
fn variant_tags_on_non_ability_records_stay_legacy() {
    // `parse_variant` only runs for ABILITY records; the same tags elsewhere
    // are not consumed.
    let r = record(
        "CLASS",
        "Fighter",
        &[("TYPE", "Archetype"), ("GRANTS", "X")],
    );
    let (pack, report) = lower(r);

    assert!(pack.variants.is_empty());
    assert_eq!(
        report.coverage.legacy_pending_by_tag.get("GRANTS"),
        Some(&1)
    );
}
