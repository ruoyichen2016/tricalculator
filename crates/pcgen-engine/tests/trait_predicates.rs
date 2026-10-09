//! `PREVISION` and `PRESRGTEQ` — predicates that need the creature traits.
//!
//! Vision and spell resistance come from the race **and** the active templates
//! together. `merge_creature_traits` does that merge in the engine layer, but it
//! runs *after* `apply_effects`, so `predicate_holds` could not reach it and
//! both tags were reported as unsupported — 2 uses each in the full RSRD, which
//! the acceptance cohort loads.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/traits.lst", 1, "traits.pcc");
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

/// Is the ability gated on `tag:value` active, with `traits` on the template?
fn active(tag: &str, value: &str, traits: &[(&str, &str)]) -> bool {
    let mut template_tags: Vec<(&str, &str)> = vec![("ABILITY", "Special Ability|AUTOMATIC|Gated")];
    template_tags.extend_from_slice(traits);
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record("TEMPLATE", "Traits", &template_tags),
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                (tag, value),
                ("AUTO", "WEAPONPROF|Canaryblade"),
            ],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        templates: vec!["Traits".to_string()],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn spell_resistance_is_read_from_a_template() {
    // "spell resistance of 10 or greater".
    assert!(active("PRESRGTEQ", "10", &[("SR", "15")]));
    assert!(!active("PRESRGTEQ", "20", &[("SR", "15")]));
    // "exactly 10".
    assert!(active("PRESREQ", "15", &[("SR", "15")]));
    assert!(!active("PRESREQ", "10", &[("SR", "15")]));
    // No SR at all is zero, so a positive requirement fails.
    assert!(!active("PRESRGTEQ", "1", &[]));
}

#[test]
fn vision_matches_by_kind() {
    assert!(active(
        "PREVISION",
        "1,Darkvision=ANY",
        &[("VISION", "Darkvision (DarkvisionRange)")]
    ));
    assert!(!active(
        "PREVISION",
        "1,Darkvision=ANY",
        &[("VISION", "Low-Light (LowlightRange)")]
    ));
}

#[test]
fn a_vision_kind_spelled_differently_still_matches() {
    // The data has `VISION:Low-Light` but `PREVISION:1,Low-Light Vision=1`.
    assert!(active(
        "PREVISION",
        "1,Low-Light Vision=1",
        &[
            ("VISION", "Low-Light (LowlightRange)"),
            ("DEFINE", "LowlightRange|60")
        ]
    ));
}

#[test]
fn the_distance_comes_from_the_range_variable() {
    let entry = &[("VISION", "Darkvision (DarkvisionRange)")];
    assert!(active(
        "PREVISION",
        "1,Darkvision=120",
        &[entry[0], ("DEFINE", "DarkvisionRange|120")]
    ));
    assert!(!active(
        "PREVISION",
        "1,Darkvision=120",
        &[entry[0], ("DEFINE", "DarkvisionRange|60")]
    ));
    // `ANY` ignores the distance, including when none is defined.
    assert!(active("PREVISION", "1,Darkvision=ANY", entry));
}

#[test]
fn the_count_is_the_number_of_satisfied_specs() {
    // "Character must have both Normal and Darkvision."
    let both = &[
        ("VISION", "Normal"),
        ("VISION", "Darkvision (DarkvisionRange)"),
    ];
    assert!(active("PREVISION", "2,Normal=ANY,Darkvision=ANY", both));
    assert!(!active(
        "PREVISION",
        "2,Normal=ANY,Darkvision=ANY",
        &[("VISION", "Normal")]
    ));
    // "either Blindsight or Darkvision at 30" — a count of one.
    assert!(active(
        "PREVISION",
        "1,Blindsight=30,Darkvision=30",
        &[
            ("VISION", "Darkvision (DarkvisionRange)"),
            ("DEFINE", "DarkvisionRange|60")
        ]
    ));
}
