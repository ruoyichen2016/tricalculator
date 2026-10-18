//! `PREDR:x,y=z` — the character's damage reduction.
//!
//! PCGen's reference:
//!
//! ```text
//! PREDR:x,y=z,y=z
//!   x: Number (The number of the DR conditions that must be met).
//!   y: Text (The type of DR).
//!   z: Number (Value the DR must be greater or equal to).
//!
//! PREDR:1,+1=10
//! PREDR:1,-=10,+1=10,+2=10,+3=10,+4=10,+5=10,Silver=10
//! ```
//!
//! `PreDamageReductionParser` splits each pair on `=` *or* `.`, defaults a missing
//! value to 0, and never sets `countMultiples` — so each pair contributes 0 or 1
//! and the wrapper counts how many hold, rather than summing their values.
//! `PreDamageReductionTester` reads `display.getDR(key)`, which is the object's
//! own reduction for that type plus any `BONUS:DR|<type>` on top.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/dr.lst", 1, "dr.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied, given a race carrying `dr` and any
/// extra `bonus` tags on it?
fn gated(gate: &str, dr: &[(&str, &str)], bonus: &[(&str, &str)]) -> bool {
    let mut race_tags: Vec<(&str, &str)> = vec![("TYPE", "Outsider")];
    race_tags.extend(dr.iter().copied());
    race_tags.extend(bonus.iter().copied());

    let records = vec![
        record("RACE", "Test Race", &race_tags),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["dr".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_type_half_of_a_dr_row_is_what_is_tested() {
    assert!(gated("PREDR:1,Magic=5", &[("DR", "5/Magic")], &[]));
    // The value has to be reached.
    assert!(!gated("PREDR:1,Magic=10", &[("DR", "5/Magic")], &[]));
    // And the type has to match.
    assert!(!gated("PREDR:1,Evil=5", &[("DR", "5/Magic")], &[]));
    // Case-insensitively, because the data writes both.
    assert!(gated("PREDR:1,magic=5", &[("DR", "5/Magic")], &[]));
    assert!(gated("PREDR:1,Magic=5", &[("DR", "5/magic")], &[]));
}

#[test]
fn the_untyped_reduction_is_its_own_type() {
    // `DR:5/-` is the "no type" reduction and `PREDR:1,-=5` is the reference's
    // own first example.
    assert!(gated("PREDR:1,-=5", &[("DR", "5/-")], &[]));
    assert!(!gated("PREDR:1,-=10", &[("DR", "5/-")], &[]));
    // A typed reduction is not the untyped one.
    assert!(!gated("PREDR:1,-=5", &[("DR", "5/Magic")], &[]));
}

#[test]
fn the_count_is_the_number_of_pairs_that_hold() {
    let dr = [("DR", "10/Magic"), ("DR", "10/Good")];
    assert!(gated("PREDR:1,Magic=10,Good=10", &dr, &[]));
    assert!(gated("PREDR:2,Magic=10,Good=10", &dr, &[]));
    assert!(!gated("PREDR:2,Magic=10,Evil=10", &dr, &[]));
    assert!(!gated("PREDR:3,Magic=10,Good=10,Evil=10", &dr, &[]));
    // The pairs are not summed: two types at 10 are two, not twenty.
    assert!(!gated("PREDR:3,Magic=10,Good=10", &dr, &[]));
    // A missing value means "at least 0", which the reference's own `compare`
    // turns into a pass even for a type the character has no reduction of: it
    // returns 1 when the left side is 0 rather than reporting a failure.
    assert!(gated("PREDR:1,Magic", &[("DR", "1/Magic")], &[]));
    assert!(gated("PREDR:1,Evil", &[("DR", "1/Magic")], &[]));
    assert!(gated("PREDR:1,Evil", &[], &[]));
}

#[test]
fn a_bonus_dr_raises_the_value_for_its_type() {
    // `DamageReductionFacet.getDR` adds `BONUS:DR|<type>` on top of the row.
    assert!(!gated("PREDR:1,Magic=10", &[("DR", "5/Magic")], &[]));
    assert!(gated(
        "PREDR:1,Magic=10",
        &[("DR", "5/Magic")],
        &[("BONUS", "DR|Magic|5")]
    ));
    // It only raises its own type.
    assert!(!gated(
        "PREDR:1,Evil=10",
        &[("DR", "5/Magic")],
        &[("BONUS", "DR|Magic|5")]
    ));
}

#[test]
fn a_conditional_dr_row_is_only_counted_when_its_gate_holds() {
    // `DR:5/Magic|PRELEVEL:MIN=10` is not a reduction for a low-level character.
    let conditional = [("DR", "5/Magic|PRELEVEL:MIN=10")];
    assert!(!gated("PREDR:1,Magic=5", &conditional, &[]));
    assert!(gated("!PREDR:1,Magic=5", &conditional, &[]));
    // A gate that holds for a 0-level character counts the row.
    assert!(gated(
        "PREDR:1,Magic=5",
        &[("DR", "5/Magic|!PRELEVEL:MIN=10")],
        &[]
    ));
}
