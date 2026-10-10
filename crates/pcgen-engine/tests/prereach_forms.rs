//! `PREREACH` and its six operator kinds — the character's reach.
//!
//! `PreReachParser` reads the comparison out of the tag name:
//!
//! ```java
//! String compType = kind.substring(5);
//! if (compType.isEmpty()) compType = "gteq";
//! prereq.setOperator(compType);
//! prereq.setOperand(formula);
//! ```
//!
//! and `PreReachTester` compares `ReachFacet.getReach` with that operand.
//! `ReachFacet.getReach` is the *race's* `REACH` — `IntegerKey.REACH` is declared
//! with a default of 5, and `getSafe` returns that default when the key was never
//! set — with any template overriding it, plus `BONUS:COMBAT|REACH`.  Abilities
//! are not consulted, which matters because the rows carrying `REACH:` in the
//! third-party data are wild-shape templates.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/reach.lst", 1, "reach.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied, with `race` and `template` tags?
fn gated(gate: &str, race: &[(&str, &str)], template: &[(&str, &str)]) -> bool {
    let mut records = vec![record("RACE", "Test Race", race)];
    if !template.is_empty() {
        records.push(record("TEMPLATE", "Test Template", template));
    }
    records.push(record(
        "FEAT",
        "Gated Feat",
        &[
            ("TYPE", "General"),
            ("BONUS", &format!("VAR|Gated|1|{gate}")),
        ],
    ));
    let (pack, report) =
        lower_records(Some("35e".to_string()), vec!["reach".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let mut draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    if !template.is_empty() {
        draft.templates.push("Test Template".to_string());
    }
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_races_reach_is_the_base() {
    assert!(gated("PREREACHGTEQ:10", &[("REACH", "10")], &[]));
    assert!(!gated("PREREACHGTEQ:10", &[("REACH", "5")], &[]));
    // A race that never sets `REACH` reads as the key's default of 5, not 0.
    assert!(gated("PREREACHGTEQ:5", &[], &[]));
    assert!(!gated("PREREACHGTEQ:10", &[], &[]));
}

#[test]
fn the_kinds_differ_only_in_their_boundary() {
    let reach = [("REACH", "10")];
    assert!(gated("PREREACH:10", &reach, &[]));
    assert!(gated("PREREACHGTEQ:10", &reach, &[]));
    assert!(gated("PREREACHLTEQ:10", &reach, &[]));
    assert!(gated("PREREACHEQ:10", &reach, &[]));
    assert!(!gated("PREREACHGT:10", &reach, &[]));
    assert!(!gated("PREREACHLT:10", &reach, &[]));
    assert!(!gated("PREREACHNEQ:10", &reach, &[]));

    let longer = [("REACH", "15")];
    assert!(gated("PREREACHGT:10", &longer, &[]));
    assert!(!gated("PREREACHLT:10", &longer, &[]));
    assert!(gated("PREREACHNEQ:10", &longer, &[]));
    assert!(!gated("PREREACHEQ:10", &longer, &[]));

    let shorter = [("REACH", "5")];
    assert!(!gated("PREREACHGT:10", &shorter, &[]));
    assert!(gated("PREREACHLT:10", &shorter, &[]));
    assert!(gated("PREREACHNEQ:10", &shorter, &[]));
}

#[test]
fn a_template_overrides_the_races_reach() {
    // The wild-shape forms in the data are templates for exactly this reason.
    assert!(gated(
        "PREREACHGTEQ:15",
        &[("REACH", "5")],
        &[("REACH", "15")]
    ));
    // The template wins outright, so a longer race reach does not help.
    assert!(!gated(
        "PREREACHGTEQ:20",
        &[("REACH", "20")],
        &[("REACH", "15")]
    ));
    // A template that says nothing about reach leaves the race's value alone.
    assert!(gated(
        "PREREACHGTEQ:20",
        &[("REACH", "20")],
        &[("TYPE", "Subrace")]
    ));
}

#[test]
fn a_combat_reach_bonus_is_added_on_top() {
    // `ReachFacet.getReach` ends with `getBonus(id, "COMBAT", "REACH")`.
    assert!(!gated("PREREACHGTEQ:15", &[("REACH", "10")], &[]));
    assert!(gated(
        "PREREACHGTEQ:15",
        &[("REACH", "10"), ("BONUS", "COMBAT|REACH|5")],
        &[]
    ));
    // With no `REACH` at all the base is the key's default of 5.
    assert!(gated(
        "PREREACHGTEQ:10",
        &[("BONUS", "COMBAT|REACH|5")],
        &[]
    ));
}
