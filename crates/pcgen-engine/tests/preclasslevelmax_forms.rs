//! `PRECLASSLEVELMAX:x,y=z` — a *maximum* class level.
//!
//! There is no tester for this tag.  `PreClassLevelMaxParser` rewrites it into the
//! inverse of `PRECLASS`, and says so in its own comment:
//!
//! ```text
//! PRECLASSLEVELMAX:Barbarian=1        !PRECLASS:Barbarian=2
//! !PRECLASSLEVELMAX:Barbarian=1       PRECLASS:Barbarian=2
//! PRECLASSLEVELMAX:Fighter=1,SPELLCASTER=2
//!     PREMULT:2,[!PRECLASS:Fighter=2],[!PRECLASS:SPELLCASTER=3]
//! ```
//!
//! Concretely it sets the kind to `class`, the operator to `LT`, adds one to the
//! operand and clears `countMultiples`:
//!
//! ```java
//! // 'class_levels >= 4'   --> 'class_levels < 5'
//! oper = Integer.toString(Integer.parseInt(oper) + 1);
//! p.setOperand(oper);
//! p.setOperator(PrerequisiteOperator.LT);
//! p.setCountMultiples(false);
//! ```
//!
//! so each entry contributes 0 or 1 and the group's count is how many of the
//! listed classes are within their maximum.  Modelling it as the negated
//! `ClassRequirement` at `z + 1` reproduces that without a second tester.

use pcgen_engine::{apply_effects, CharacterDraft, DraftClassLevel};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/clm.lst", 1, "clm.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character with these class levels?
fn gated(gate: &str, class_levels: &[(&str, u32)]) -> bool {
    let records = vec![
        record("RACE", "Test Race", &[("TYPE", "Humanoid")]),
        record("CLASS", "Fighter", &[("TYPE", "Base")]),
        record("CLASS", "Wizard", &[("TYPE", "Base")]),
        record("CLASS", "Assassin", &[("TYPE", "Prestige")]),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["clm".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: class_levels
            .iter()
            .map(|(class_id, level)| DraftClassLevel {
                class_id: (*class_id).to_string(),
                level: *level,
                hit_points_gained: 0,
                hit_points_by_level: Vec::new(),
                experience_gained: 0,
            })
            .collect(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_named_class_maximum_is_a_maximum() {
    // At most three levels of Wizard: three passes, four does not.
    assert!(gated("PRECLASSLEVELMAX:1,Wizard=3", &[("Wizard", 3)]));
    assert!(!gated("PRECLASSLEVELMAX:1,Wizard=3", &[("Wizard", 4)]));
    assert!(gated("PRECLASSLEVELMAX:1,Wizard=3", &[]));
    // A different class does not count towards it.
    assert!(gated("PRECLASSLEVELMAX:1,Wizard=3", &[("Fighter", 10)]));
}

#[test]
fn a_type_maximum_is_tested_through_preclass() {
    // `TYPE.Base=1` becomes `!PRECLASS:TYPE.Base=2`.
    assert!(gated("PRECLASSLEVELMAX:1,TYPE.Base=1", &[("Fighter", 1)]));
    assert!(!gated("PRECLASSLEVELMAX:1,TYPE.Base=1", &[("Fighter", 2)]));
    // A prestige class is not a base class, so its levels are irrelevant.
    assert!(gated(
        "PRECLASSLEVELMAX:1,TYPE.Base=1",
        &[("Assassin", 10)]
    ));
    assert!(!gated(
        "PRECLASSLEVELMAX:1,TYPE.Prestige=1",
        &[("Assassin", 2)]
    ));
}

#[test]
fn the_count_is_how_many_classes_are_within_their_maximum() {
    // Both entries hold when neither class has levels.
    assert!(gated(
        "PRECLASSLEVELMAX:1,Fighter=1,Wizard=1",
        &[]
    ));
    assert!(gated(
        "PRECLASSLEVELMAX:2,Fighter=1,Wizard=1",
        &[]
    ));
    // One over its maximum leaves only the other entry holding.
    assert!(gated(
        "PRECLASSLEVELMAX:1,Fighter=1,Wizard=1",
        &[("Fighter", 2)]
    ));
    assert!(!gated(
        "PRECLASSLEVELMAX:2,Fighter=1,Wizard=1",
        &[("Fighter", 2)]
    ));
    assert!(!gated(
        "PRECLASSLEVELMAX:1,Fighter=1,Wizard=1",
        &[("Fighter", 2), ("Wizard", 2)]
    ));
}

#[test]
fn a_negated_maximum_is_a_minimum() {
    // `!PRECLASSLEVELMAX:Barbarian=1` is `PRECLASS:Barbarian=2`.
    assert!(gated("!PRECLASSLEVELMAX:1,Wizard=3", &[("Wizard", 4)]));
    assert!(!gated("!PRECLASSLEVELMAX:1,Wizard=3", &[("Wizard", 3)]));
}
