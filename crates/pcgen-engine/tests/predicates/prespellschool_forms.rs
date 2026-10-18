//! `PRESPELLSCHOOL:x,y=z` — known spells of a school of magic.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESPELLSCHOOL:x,y=z,y=z
//!   x: Number (Number of spells known)
//!   y: Text (Name of a school of magic).
//!   z: Number (Minimum spell level)
//!
//! PRESPELLSCHOOL:3,Necromancy=2
//!   "at least three 2nd level or higher Necromancy spells"
//! ```
//!
//! `PreSpellSchoolTester` fills the *school* slot of `aggregateSpellList`:
//!
//! ```java
//! character.aggregateSpellList(school, "A", "No-Match", requiredLevel, 20);
//! ```
//!
//! so it is the sub-school tester one field over — a count per school, summed
//! across the pairs.
//!
//! The implementation this replaced matched the school name as a substring of the
//! spell's **name** instead.  Only a handful of RSRD spell names contain a school
//! word (`Divination`, `Shadow Conjuration`, `Shadow Evocation`, `Break
//! Enchantment`), so `PRESPELLSCHOOL:1,Abjuration=1` could never hold no matter
//! how many abjurations the character knew.

use pcgen_engine::{apply_effects, CharacterDraft, DraftSpell};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/school.lst", 1, "school.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character knowing `known`?
fn gated(gate: &str, known: &[(&str, u32)]) -> bool {
    let records = vec![
        record(
            "SPELL",
            "Fireball",
            &[
                ("SCHOOL", "Evocation"),
                ("DESCRIPTOR", "Fire"),
                ("LEVEL", "3"),
            ],
        ),
        record(
            "SPELL",
            "Shield",
            &[("SCHOOL", "Abjuration"), ("LEVEL", "1")],
        ),
        record(
            "SPELL",
            "Dispel Magic",
            &[("SCHOOL", "Abjuration"), ("LEVEL", "3")],
        ),
        record(
            "SPELL",
            "Divination",
            &[("SCHOOL", "Divination"), ("LEVEL", "4")],
        ),
        record(
            "RACE",
            "Test Race",
            &[("BONUS", &format!("VAR|Gated|1|{gate}"))],
        ),
    ];
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["school".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        spells: known
            .iter()
            .map(|(name, level)| DraftSpell {
                name: (*name).to_string(),
                level: *level,
                class_id: None,
                times_memorized: 0,
                book: None,
            })
            .collect(),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_school_is_matched_not_the_spell_name() {
    // The regression this fixes: `Shield` is an abjuration and its name contains
    // no school word, so the old substring test rejected it.
    assert!(gated("PRESPELLSCHOOL:1,Abjuration=1", &[("Shield", 1)]));
    assert!(!gated("PRESPELLSCHOOL:1,Evocation=1", &[("Shield", 1)]));
    // A spell literally named after its school is not required to match.
    assert!(gated("PRESPELLSCHOOL:1,Divination=1", &[("Divination", 4)]));
    assert!(!gated("PRESPELLSCHOOL:1,Divination=1", &[("Shield", 1)]));
    // Case-insensitively.
    assert!(gated("PRESPELLSCHOOL:1,abjuration=1", &[("Shield", 1)]));
}

#[test]
fn a_school_needs_a_spell_of_a_high_enough_level() {
    // Shield is 1st level, Dispel Magic is 3rd.
    assert!(gated(
        "PRESPELLSCHOOL:1,Abjuration=3",
        &[("Dispel Magic", 3)]
    ));
    assert!(!gated("PRESPELLSCHOOL:1,Abjuration=3", &[("Shield", 1)]));
    // Level 0 accepts any level, and the level that matters is the one the
    // character knows the spell at.
    assert!(gated("PRESPELLSCHOOL:1,Abjuration=0", &[("Shield", 1)]));
    assert!(!gated("PRESPELLSCHOOL:1,Abjuration=2", &[("Shield", 1)]));
    assert!(!gated("PRESPELLSCHOOL:1,Abjuration=1", &[]));
}

#[test]
fn the_count_is_a_total_across_the_pairs() {
    // The count used to be dropped and the entries joined with `all()`, so
    // "at least one of two schools" became "both".
    assert!(gated(
        "PRESPELLSCHOOL:1,Abjuration=1,Evocation=1",
        &[("Shield", 1)]
    ));
    assert!(!gated(
        "PRESPELLSCHOOL:2,Abjuration=1,Evocation=1",
        &[("Shield", 1)]
    ));
    assert!(gated(
        "PRESPELLSCHOOL:2,Abjuration=1,Evocation=1",
        &[("Shield", 1), ("Fireball", 3)]
    ));
    // Every spell of the school counts towards its pair.
    assert!(gated(
        "PRESPELLSCHOOL:2,Abjuration=1",
        &[("Shield", 1), ("Dispel Magic", 3)]
    ));
}

#[test]
fn a_negated_school_gate_holds_when_none_is_known() {
    assert!(gated("!PRESPELLSCHOOL:1,Necromancy=1", &[("Shield", 1)]));
    assert!(!gated("!PRESPELLSCHOOL:1,Abjuration=1", &[("Shield", 1)]));
}
