//! The `PRELEGS` family — how many legs the character stands on.
//!
//! `LegsFacet.getLegs` is the value under test:
//!
//! ```java
//! Race aRace = raceFacet.get(id);
//! int legs = 0;
//! if (aRace != null) legs = aRace.getSafe(IntegerKey.LEGS);   // default 2
//! for (PCTemplate template : templateFacet.getSet(id))
//! {
//!     Integer l = template.get(IntegerKey.LEGS);
//!     if (l != null) legs = l;                                 // last one wins
//! }
//! ```
//!
//! so the race supplies the number — 2 when it states none — and each template
//! that states one replaces it, in template order.  `PreLegsTester` then compares
//! that against the operand with the operator the tag's suffix names.
//!
//! This used to be a constant: `EffectState.legs` was hard-coded to 2 and the
//! `LEGS:` tag was not read at all, so all 399 `PRELEGS*` occurrences in the
//! third-party data answered the same way regardless of the creature.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/legs.lst", 1, "legs.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character of this race?
///
/// `race_tags` are the race's own tags, `race_template` is a template the race
/// grants and `applied` is one the player took.
fn gated(gate: &str, race_tags: &[(&str, &str)], templates: &[(&str, u32)]) -> bool {
    let mut records = vec![record("RACE", "Test Race", race_tags)];
    for (name, legs) in templates {
        records.push(record(
            "TEMPLATE",
            name,
            &[("LEGS", &legs.to_string()), ("TYPE", "Test")],
        ));
    }
    records.push(record(
        "FEAT",
        "Gated Feat",
        &[
            ("TYPE", "General"),
            ("BONUS", &format!("VAR|Gated|1|{gate}")),
        ],
    ));
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["legs".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        templates: templates
            .iter()
            .filter(|(name, _)| name.starts_with("Applied"))
            .map(|(name, _)| (*name).to_string())
            .collect(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_race_supplies_the_number() {
    assert!(gated(
        "PRELEGSGTEQ:4",
        &[("LEGS", "4"), ("TEMPLATE", "Race Template")],
        &[("Race Template", 4)]
    ));
    assert!(!gated(
        "PRELEGSGTEQ:5",
        &[("LEGS", "4"), ("TEMPLATE", "Race Template")],
        &[("Race Template", 4)]
    ));
    // A race that states none reads as `IntegerKey.LEGS`'s default of 2.
    assert!(gated("PRELEGSLTEQ:2", &[], &[]));
    assert!(!gated("PRELEGSGTEQ:3", &[], &[]));
    // A race that states zero is not the same as one that states nothing.
    assert!(gated("PRELEGSLTEQ:0", &[("LEGS", "0")], &[]));
}

#[test]
fn a_template_replaces_the_race_value() {
    // The race says four, its own template says eight.
    assert!(gated(
        "PRELEGSGTEQ:8",
        &[("LEGS", "4"), ("TEMPLATE", "Race Template")],
        &[("Race Template", 8)]
    ));
    // An applied template comes after the race's own and wins in turn.
    assert!(gated(
        "PRELEGSGTEQ:6",
        &[("LEGS", "4"), ("TEMPLATE", "Race Template")],
        &[("Race Template", 8), ("Applied Template", 6)]
    ));
    assert!(!gated(
        "PRELEGSGTEQ:8",
        &[("LEGS", "4"), ("TEMPLATE", "Race Template")],
        &[("Race Template", 8), ("Applied Template", 6)]
    ));
}

#[test]
fn every_operator_is_available() {
    let legs_four = &[("LEGS", "4")];
    // Strict comparisons are exact, not the non-strict ones shifted by one.
    assert!(gated("PRELEGSGT:3", legs_four, &[]));
    assert!(!gated("PRELEGSGT:4", legs_four, &[]));
    assert!(gated("PRELEGSLT:5", legs_four, &[]));
    assert!(!gated("PRELEGSLT:4", legs_four, &[]));
    // Equality and inequality.
    assert!(gated("PRELEGSEQ:4", legs_four, &[]));
    assert!(!gated("PRELEGSEQ:3", legs_four, &[]));
    assert!(!gated("PRELEGSNEQ:4", legs_four, &[]));
    assert!(gated("PRELEGSNEQ:5", legs_four, &[]));
    // The bare spelling is the reference's `gteq` default.
    assert!(gated("PRELEGS:4", legs_four, &[]));
    assert!(!gated("PRELEGS:5", legs_four, &[]));
    // A negated tag inverts the comparison.
    assert!(gated("!PRELEGSGTEQ:5", legs_four, &[]));
    assert!(!gated("!PRELEGSGTEQ:4", legs_four, &[]));
}
