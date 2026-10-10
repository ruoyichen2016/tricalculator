//! `PRESKILLTOT:x,x=y` — the character's total ranks across a list of skills.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESKILLTOT:x,x=y
//!   x: Text (skill name)   x: TYPE=Text (skill type)
//!   y: Number (total non-bonus skill ranks required)
//!
//! PRESKILLTOT:Spot,Listen,Search=30
//!   "Character must have a total of 30 non-bonus ranks between Spot, Search,
//!    and Listen skills."
//! ```
//!
//! `PreSkillTotalParser` marks every listed skill `totalValues`, and
//! `PreSkillTester` reads that flag in two places: it adds `getTotalRank` rather
//! than counting skills that clear a threshold, and it stops breaking at the first
//! match — so a `TYPE.Text` entry sums every skill of that type.  The parser also
//! takes the operand from the *last* token carrying an `=` and leaves it at 0 when
//! the formula has none.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/skilltot.lst", 1, "skilltot.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character with these ranks?
fn gated(gate: &str, skills: &[(&str, f64)]) -> bool {
    let records = vec![
        record("RACE", "Test Race", &[("TYPE", "Humanoid")]),
        record("SKILL", "Spot", &[("TYPE", "Perception")]),
        record("SKILL", "Listen", &[("TYPE", "Perception")]),
        record("SKILL", "Search", &[("TYPE", "Investigation")]),
        record("SKILL", "Knowledge (Arcana)", &[("TYPE", "Knowledge")]),
        record("SKILL", "Knowledge (Nature)", &[("TYPE", "Knowledge")]),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) =
        lower_records(Some("35e".to_string()), vec!["skilltot".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        skills: skills
            .iter()
            .map(|(name, ranks)| ((*name).to_string(), *ranks))
            .collect::<BTreeMap<_, _>>(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_ranks_of_the_listed_skills_are_summed() {
    // The reference's example: ten ranks in each of three skills is thirty.
    assert!(gated(
        "PRESKILLTOT:Spot,Listen,Search=30",
        &[("Spot", 10.0), ("Listen", 10.0), ("Search", 10.0)]
    ));
    assert!(!gated(
        "PRESKILLTOT:Spot,Listen,Search=30",
        &[("Spot", 10.0), ("Listen", 10.0)]
    ));
    // The boundary is inclusive, because the parser leaves the operator at GTEQ.
    assert!(gated("PRESKILLTOT:Spot=10", &[("Spot", 10.0)]));
    assert!(!gated("PRESKILLTOT:Spot=10", &[("Spot", 9.0)]));
    // Skills that are not listed do not count.
    assert!(!gated(
        "PRESKILLTOT:Spot=10",
        &[("Search", 20.0)]
    ));
}

#[test]
fn a_type_entry_sums_every_skill_of_that_type() {
    assert!(gated(
        "PRESKILLTOT:TYPE.Knowledge=8",
        &[("Knowledge (Arcana)", 5.0), ("Knowledge (Nature)", 3.0)]
    ));
    assert!(!gated(
        "PRESKILLTOT:TYPE.Knowledge=8",
        &[("Knowledge (Arcana)", 5.0), ("Knowledge (Nature)", 2.0)]
    ));
}

#[test]
fn a_formula_without_a_total_asks_for_zero() {
    // The parser only assigns the operand from a token carrying `=`, so a formula
    // with none compares against 0 and always holds.
    assert!(gated("PRESKILLTOT:Spot,Listen", &[]));
    assert!(gated("PRESKILLTOT:Spot,Listen", &[("Spot", 1.0)]));
}

#[test]
fn an_empty_skill_list_matches_nothing() {
    // `PRESKILLTOT:=1` is the third-party form: one entry whose name is empty.  No
    // skill has an empty name, so the total is 0 and the requirement fails — which
    // is what PCGen computes too, and is worth pinning because it looks like a
    // modelling gap and is not one.
    assert!(!gated("PRESKILLTOT:=1", &[("Spot", 50.0)]));
    assert!(!gated("PRESKILLTOT:=1", &[]));
    assert!(gated("!PRESKILLTOT:=1", &[]));
}

#[test]
fn a_negated_total_holds_when_the_ranks_fall_short() {
    assert!(gated("!PRESKILLTOT:Spot=10", &[("Spot", 9.0)]));
    assert!(!gated("!PRESKILLTOT:Spot=10", &[("Spot", 10.0)]));
}
