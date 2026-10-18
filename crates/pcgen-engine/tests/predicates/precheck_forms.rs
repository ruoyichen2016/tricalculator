//! `PRECHECK:x,y=z` — the *total* save, and the count `PRECHECKBASE` was losing.
//!
//! PCGen's reference:
//!
//! ```text
//! PRECHECK:1,Fortitude=5,Reflex=3      Fortitude >= 5 OR Reflex >= 3
//! PRECHECK:2,Fortitude=5,Reflex=3,Willpower=4   any two of the three
//! PRECHECKBASE:x,y=z                   the same, against the *base* check
//! ```
//!
//! `PreCheckTester` compares `character.getTotalCheck(check)` with the operand,
//! and `getTotalCheck` is `getBaseCheck + getTotalBonusTo("SAVE", kind)` — the
//! ability modifier reaches the save through the save's own definition
//! (`rsrd__saves.lst` has `Fortitude ... BONUS:SAVE|Fortitude|CON`).
//!
//! The old lowering read only the first `=` part and dropped the count, so
//! `PRECHECKBASE:2,Fortitude=5,Reflex=3` became "Fortitude >= 5" — stricter than
//! the reference's "either".

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/precheck.lst", 1, "precheck.pcc");
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

/// Is `BONUS:VAR|Gated|1|<check>` applied to a 3rd-level Cleric?
///
/// The class has a good Will save (`level/2+2` → 3 at level 3) and a poor
/// Fortitude save (`level/3` → 1).
fn gated(check: &str, wis: i32, con: i32) -> bool {
    let records = vec![
        record(
            "CLASS",
            "Cleric",
            &[
                ("BAB", "medium"),
                (
                    "BONUS",
                    "SAVE|BASE.Will|classlevel(\"APPLIEDAS=NONEPIC\")/2+2",
                ),
                (
                    "BONUS",
                    "SAVE|BASE.Fortitude|classlevel(\"APPLIEDAS=NONEPIC\")/3",
                ),
            ],
        ),
        record(
            "RACE",
            "Test Race",
            &[("BONUS", &format!("VAR|Gated|1|{check}"))],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["precheck".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let mut stats = std::collections::BTreeMap::new();
    stats.insert("WIS".to_string(), wis);
    stats.insert("CON".to_string(), con);
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        stats,
        advancement: pcgen_import::advancement_from_class_levels(&vec![DraftClassLevel {
            class_id: "Cleric".to_string(),
            level: 3,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }]),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn precheck_uses_the_total_save_and_precheckbase_does_not() {
    // Base Will is 3; WIS 16 adds +3, so the total is 6.
    assert!(gated("PRECHECK:1,Will=5", 16, 10));
    assert!(!gated("PRECHECK:1,Will=7", 16, 10));
    // The same character's *base* Will save is only 3, so PRECHECKBASE rejects
    // what PRECHECK accepts — the two predicates are not interchangeable.
    assert!(!gated("PRECHECKBASE:1,Will=5", 16, 10));
    assert!(gated("PRECHECKBASE:1,Will=2", 16, 10));
}

#[test]
fn the_count_is_honoured() {
    // WIS 16: Will total 6 (pass). CON 10: Fortitude total 1 (fail).
    assert!(gated("PRECHECK:1,Fortitude=2,Will=2", 16, 10));
    assert!(!gated("PRECHECK:2,Fortitude=2,Will=2", 16, 10));
    // CON 14 adds +2, so Fortitude total 3 passes too.
    assert!(gated("PRECHECK:2,Fortitude=2,Will=2", 16, 14));
}

#[test]
fn precheckbase_count_is_not_dropped() {
    // Base saves are Will 3 and Fortitude 1. "One of the two" holds; "both" does
    // not. Reading only the first `=` part would have tested Fortitude alone,
    // which fails — the bug this count fixes.
    assert!(gated("PRECHECKBASE:1,Fortitude=2,Will=2", 10, 10));
    assert!(!gated("PRECHECKBASE:2,Fortitude=2,Will=2", 10, 10));
    assert!(!gated("PRECHECKBASE:1,Fortitude=2", 10, 10));
}
