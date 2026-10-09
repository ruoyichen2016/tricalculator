//! `DEFINE` — several definitions of one variable keep the highest.
//!
//! PCGen's reference:
//!
//! > Values are calculated by getting the value of the variable with the
//! > **highest defined value**, and then adding any bonuses which apply to it.
//! > PCGen scans the character's race, templates, classes, feats, skills,
//! > deity, domains and equipped items for DEFINE tags that match this variable
//! > name. **If it finds more than one, it keeps the one with the higher
//! > value.**
//!
//! The definitions were collected into a map keyed by name, so the *last* one
//! read won — which depends on iteration order rather than on the values.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/define.lst", 1, "define.pcc");
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

/// The value of `X` for a character whose race and template both define it.
fn value_of(definitions: &[&str]) -> i32 {
    let mut race_tags: Vec<(&str, &str)> = vec![("SIZE", "M")];
    for definition in definitions {
        race_tags.push(("DEFINE", definition));
    }
    let records = vec![record("RACE", "Human", &race_tags)];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .variables
        .get("X")
        .copied()
        .unwrap_or(0)
}

#[test]
fn several_definitions_keep_the_highest() {
    assert_eq!(value_of(&["X|1", "X|5"]), 5, "5 is higher than 1");
    assert_eq!(value_of(&["X|5", "X|1"]), 5, "and order does not matter");
    assert_eq!(value_of(&["X|3", "X|7", "X|2"]), 7);
}

#[test]
fn a_single_definition_still_applies() {
    assert_eq!(value_of(&["X|4"]), 4);
}

#[test]
fn a_negative_definition_does_not_beat_a_higher_one() {
    assert_eq!(value_of(&["X|-3", "X|2"]), 2);
    assert_eq!(value_of(&["X|2", "X|-3"]), 2);
}

#[test]
fn a_formula_definition_is_evaluated_before_comparing() {
    // `X|1+1` is 2, so `X|5` still wins.
    assert_eq!(value_of(&["X|1+1", "X|5"]), 5);
}
