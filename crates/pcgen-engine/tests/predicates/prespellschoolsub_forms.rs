//! `PRESPELLSCHOOLSUB:x,y=z` — known spells of a magic sub-school.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESPELLSCHOOLSUB:x,y=z,y=z
//!   x: Number (Number of spells known)
//!   y: Text (Name of sub-school of magic.)
//!   z: Number (Minimum spell level)
//!
//! PRESPELLSCHOOLSUB:3,Creation=2
//!   "at least three 2nd level Creation spells"
//! ```
//!
//! `PreSpellSchoolSubTester` is `PreSpellDescriptorTester` with the sub-school
//! slot of `aggregateSpellList` filled in instead of the descriptor slot:
//!
//! ```java
//! final List<Spell> aArrayList = character.aggregateSpellList("No-Match", subSchool, "No-Match", requiredLevel, 20);
//! final int runningTotal = prereq.getOperator().compare(aArrayList.size(), 1);
//! ```
//!
//! so it reports a *count* and the list wrapper sums the sub-requirements, just
//! as it does for descriptors.

use pcgen_engine::{apply_effects, CharacterDraft, DraftSpell};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/subschool.lst", 1, "subschool.pcc");
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
            "Summon Monster III",
            &[
                ("SCHOOL", "Conjuration"),
                ("SUBSCHOOL", "Summoning"),
                ("LEVEL", "3"),
            ],
        ),
        record(
            "SPELL",
            "Cure Light Wounds",
            &[
                ("SCHOOL", "Conjuration"),
                ("SUBSCHOOL", "Healing"),
                ("LEVEL", "1"),
            ],
        ),
        record(
            "SPELL",
            "Silent Image",
            &[
                ("SCHOOL", "Illusion"),
                ("SUBSCHOOL", "Figment"),
                ("LEVEL", "1"),
            ],
        ),
        record(
            "RACE",
            "Test Race",
            &[("BONUS", &format!("VAR|Gated|1|{gate}"))],
        ),
    ];
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["subschool".to_string()],
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
fn a_subschool_needs_a_spell_of_a_high_enough_level() {
    // Summon Monster III is a 3rd-level Summoning spell.
    assert!(gated(
        "PRESPELLSCHOOLSUB:1,Summoning=3",
        &[("Summon Monster III", 3)]
    ));
    assert!(!gated(
        "PRESPELLSCHOOLSUB:1,Summoning=4",
        &[("Summon Monster III", 3)]
    ));
    // The level that matters is the one the character knows it at.
    assert!(!gated(
        "PRESPELLSCHOOLSUB:1,Summoning=3",
        &[("Summon Monster III", 2)]
    ));
    assert!(!gated("PRESPELLSCHOOLSUB:1,Summoning=3", &[]));
}

#[test]
fn the_subschool_is_matched_not_the_school() {
    // Silent Image is *Illusion* the school and *Figment* the sub-school. The
    // reference matches the sub-school slot, so the school name finds nothing —
    // which is why the third-party `PRESPELLSCHOOLSUB:1,Illusion=0` fails.
    assert!(gated(
        "PRESPELLSCHOOLSUB:1,Figment=1",
        &[("Silent Image", 1)]
    ));
    assert!(!gated(
        "PRESPELLSCHOOLSUB:1,Illusion=0",
        &[("Silent Image", 1)]
    ));
    // Case-insensitively, and a level of 0 accepts any level.
    assert!(gated(
        "PRESPELLSCHOOLSUB:1,figment=0",
        &[("Silent Image", 1)]
    ));
}

#[test]
fn the_count_is_a_total_across_the_pairs() {
    assert!(gated(
        "PRESPELLSCHOOLSUB:1,Summoning=3,Healing=1",
        &[("Summon Monster III", 3)]
    ));
    assert!(!gated(
        "PRESPELLSCHOOLSUB:2,Summoning=3,Healing=1",
        &[("Summon Monster III", 3)]
    ));
    assert!(gated(
        "PRESPELLSCHOOLSUB:2,Summoning=3,Healing=1",
        &[("Summon Monster III", 3), ("Cure Light Wounds", 1)]
    ));
}

#[test]
fn a_negated_subschool_gate_holds_when_none_is_known() {
    assert!(gated(
        "!PRESPELLSCHOOLSUB:1,Summoning=1",
        &[("Cure Light Wounds", 1)]
    ));
    assert!(!gated(
        "!PRESPELLSCHOOLSUB:1,Summoning=3",
        &[("Summon Monster III", 3)]
    ));
}
