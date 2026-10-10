//! `PREVARGTEQ` / `PREVARGT` / `PREVARLTEQ` / `PREVARLT` — variable comparisons.
//!
//! `PreVariableParser` takes the operator from the kind's suffix:
//!
//! ```java
//! // Get the comparator type SIZEGTEQ, BSIZE, SIZENEQ etc.
//! String compType = kind.substring(5);
//! ```
//!
//! and `PreVariableTester` compares with it: `prereq.getOperator().compare(aVar,
//! aTarget)`. So `PREVARGTEQ` is `>=` and `PREVARGT` is `>` — two different
//! predicates, and this engine used to read both as strict.
//!
//! The right-hand side is evaluated by `getVariableValue`, which accepts a number
//! as readily as a variable name (`PREVARGTEQ:3,EmpowerHealingLVL`), so a symbolic
//! operand that is really a literal has to resolve to that literal.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/var.lst", 1, "var.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied, with `X` and `Y` defined as given?
fn gated(gate: &str, x: i32, y: i32) -> bool {
    let records = vec![
        record(
            "RACE",
            "Test Race",
            &[("DEFINE", &format!("X|{x}")), ("DEFINE", &format!("Y|{y}"))],
        ),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["var".to_string()], &records);
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
fn a_numeric_operand_keeps_its_boundary() {
    // The boundary is the whole difference between the two kinds.
    assert!(gated("PREVARGTEQ:X,5", 5, 0));
    assert!(!gated("PREVARGT:X,5", 5, 0));
    assert!(gated("PREVARGT:X,5", 6, 0));
    assert!(gated("PREVARLTEQ:X,5", 5, 0));
    assert!(!gated("PREVARLT:X,5", 5, 0));
    assert!(gated("PREVARLT:X,5", 4, 0));
}

#[test]
fn a_symbolic_operand_compares_two_variables() {
    assert!(gated("PREVARGTEQ:X,Y", 5, 5));
    assert!(!gated("PREVARGT:X,Y", 5, 5));
    assert!(gated("PREVARGT:X,Y", 6, 5));
    assert!(gated("PREVARLTEQ:X,Y", 5, 5));
    assert!(!gated("PREVARLT:X,Y", 5, 5));
    assert!(gated("PREVARLT:X,Y", 4, 5));
}

#[test]
fn a_numeric_literal_operand_resolves_to_its_value() {
    // `getVariableValue` evaluates its argument, so the symbolic path's left
    // operand is a variable name or, as in `PREVARGTEQ:3,EmpowerHealingLVL`, a
    // plain number.  Reading it as a missing variable made it 0 and inverted the
    // comparison.
    assert!(gated("PREVARGTEQ:3,Y", 0, 3));
    assert!(!gated("PREVARGTEQ:3,Y", 0, 4));
    assert!(gated("PREVARLTEQ:0,X", 0, 0));
    assert!(!gated("PREVARGTEQ:0,X", 1, 0));
    // A name that is neither a variable nor a number still reads as 0.
    assert!(gated("PREVARGTEQ:Nowhere,Y", 0, 0));
}

#[test]
fn the_symbolic_and_numeric_forms_agree_at_the_boundary() {
    // `PREVARGTEQ:X,5` and `PREVARGTEQ:X,Y` with Y = 5 are the same test.
    for value in 4..=6 {
        assert_eq!(
            gated("PREVARGTEQ:X,5", value, 0),
            gated("PREVARGTEQ:X,Y", value, 5),
            "value {value}"
        );
        assert_eq!(
            gated("PREVARLT:X,5", value, 0),
            gated("PREVARLT:X,Y", value, 5),
            "value {value}"
        );
    }
}
