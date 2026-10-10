//! `PREUATT:x` — the largest base attack bonus one class contributes.
//!
//! The reference calls it an "unarmed attack bonus" and the tester is
//! `PreUnarmedAttackTester`, but the body says something else:
//!
//! ```java
//! int att = 0;
//! for (PCClass aClass : display.getClassSet())
//!     att = Math.max(att, aClass.baseAttackBonus(character));
//! final int requiredValue = Integer.parseInt(prereq.getOperand());
//! final int runningTotal = prereq.getOperator().compare(att, requiredValue);
//! ```
//!
//! It is the **maximum per-class** base attack bonus, not the character's total —
//! so a Fighter 4 / Wizard 4 has 4 here and 6 for `PRETOTALAB`.  The variant is
//! named `HighestClassBab` for what it computes rather than for the tag's
//! description, which is wrong in both directions.

use pcgen_engine::{apply_effects, CharacterDraft, DraftClassLevel};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/uatt.lst", 1, "uatt.pcc");
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
        record("CLASS", "Fighter", &[("HD", "10"), ("BAB", "GOOD")]),
        record("CLASS", "Wizard", &[("HD", "4"), ("BAB", "HALF")]),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["uatt".to_string()], &records);
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
fn a_single_class_contributes_its_own_bab() {
    // A good progression gives one point per level.
    assert!(gated("PREUATT:4", &[("Fighter", 4)]));
    assert!(!gated("PREUATT:5", &[("Fighter", 4)]));
    // The boundary is inclusive, because the operator stays at its GTEQ default.
    assert!(gated("PREUATT:1", &[("Fighter", 1)]));
    assert!(!gated("PREUATT:1", &[]));
}

#[test]
fn the_maximum_is_per_class_not_the_total() {
    // Fighter 4 contributes 4, Wizard 4 contributes 2, so the total is 6 but the
    // largest single class is 4 — and `PREUATT` asks for the latter.
    let mixed = [("Fighter", 4), ("Wizard", 4)];
    assert!(gated("PREUATT:4", &mixed));
    assert!(!gated("PREUATT:5", &mixed));
    assert!(!gated("PREUATT:6", &mixed));
    // `PRETOTALAB` on the same character is the sum, which is the contrast that
    // makes the distinction visible.
    assert!(gated("PRETOTALAB:6", &mixed));
    assert!(!gated("PRETOTALAB:7", &mixed));
}

#[test]
fn the_highest_class_is_what_counts() {
    // A poor progression alone cannot reach a good one's total.
    assert!(!gated("PREUATT:3", &[("Wizard", 4)]));
    assert!(gated("PREUATT:2", &[("Wizard", 4)]));
    // Adding a good class raises the maximum even though the poor one stays.
    assert!(gated("PREUATT:5", &[("Wizard", 4), ("Fighter", 5)]));
}

#[test]
fn a_negated_uatt_holds_when_no_class_reaches_it() {
    assert!(gated("!PREUATT:5", &[("Fighter", 4)]));
    assert!(!gated("!PREUATT:5", &[("Fighter", 5)]));
}
