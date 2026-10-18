//! `PRESPELLDESCRIPTOR:x,y=z` — known spells carrying a descriptor.
//!
//! PCGen's reference:
//!
//! ```text
//! PRESPELLDESCRIPTOR:x,y=z
//!   x: Number (Number of spells known)
//!   y: Text (Spell descriptor)   z: Number (Minimum spell level)
//!
//! PRESPELLDESCRIPTOR:4,Mind-Affecting=3
//!   "at least four 3rd level or higher Mind-Affecting spells"
//! ```
//!
//! `PreSpellDescriptorTester` counts the character's known spells that carry its
//! one descriptor at or above the required level:
//!
//! ```java
//! final List<Spell> aArrayList = character.aggregateSpellList("No-Match", "A", descriptor, requiredLevel, 20);
//! final int runningTotal = prereq.getOperator().compare(aArrayList.size(), 1);
//! ```
//!
//! and the list wrapper sums the sub-requirements before comparing with `x`.  So
//! the pairs *add up* — unlike `PRESPELLTYPE`, whose pairs the reference calls
//! alternatives.

use pcgen_engine::{apply_effects, CharacterDraft, DraftSpell};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/descriptor.lst", 1, "descriptor.pcc");
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
///
/// `known` is `(spell name, level the character knows it at)`.
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
            "Acid Arrow",
            &[
                ("SCHOOL", "Conjuration"),
                ("DESCRIPTOR", "Acid"),
                ("LEVEL", "2"),
            ],
        ),
        record(
            "SPELL",
            "Fear",
            &[
                ("SCHOOL", "Necromancy"),
                ("DESCRIPTOR", "Fear.Mind-Affecting"),
                ("LEVEL", "4"),
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
        vec!["descriptor".to_string()],
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
fn a_descriptor_needs_a_spell_of_a_high_enough_level() {
    // Fireball is a 3rd-level spell, so it clears `Fire=3` but not `Fire=4`.
    assert!(gated("PRESPELLDESCRIPTOR:1,Fire=3", &[("Fireball", 3)]));
    assert!(!gated("PRESPELLDESCRIPTOR:1,Fire=4", &[("Fireball", 3)]));
    // The level that matters is the one the *character* knows it at.
    assert!(!gated("PRESPELLDESCRIPTOR:1,Fire=3", &[("Fireball", 2)]));
    // A spell the character does not know never counts.
    assert!(!gated("PRESPELLDESCRIPTOR:1,Fire=3", &[]));
    assert!(!gated("PRESPELLDESCRIPTOR:1,Acid=3", &[("Fireball", 3)]));
}

#[test]
fn the_count_is_a_total_across_the_pairs() {
    // One 3rd-level Fire spell and one 2nd-level Acid spell.
    assert!(gated(
        "PRESPELLDESCRIPTOR:1,Acid=3,Cold=3,Electricity=3,Fire=3",
        &[("Fireball", 3)]
    ));
    assert!(!gated(
        "PRESPELLDESCRIPTOR:2,Acid=3,Cold=3,Electricity=3,Fire=3",
        &[("Fireball", 3)]
    ));
    // Acid Arrow is only 2nd level, so it does not add a second match.
    assert!(!gated(
        "PRESPELLDESCRIPTOR:2,Acid=3,Fire=3",
        &[("Fireball", 3), ("Acid Arrow", 2)]
    ));
    assert!(gated(
        "PRESPELLDESCRIPTOR:2,Acid=3,Fire=3",
        &[("Fireball", 3), ("Acid Arrow", 3)]
    ));
}

#[test]
fn every_descriptor_on_a_spell_counts() {
    // `Fear` carries both descriptors, so either pair finds it.
    assert!(gated("PRESPELLDESCRIPTOR:1,Fear=4", &[("Fear", 4)]));
    assert!(gated(
        "PRESPELLDESCRIPTOR:1,Mind-Affecting=4",
        &[("Fear", 4)]
    ));
    // Case-insensitively, as PCGen compares descriptor keys.
    assert!(gated(
        "PRESPELLDESCRIPTOR:1,mind-affecting=4",
        &[("Fear", 4)]
    ));
}

#[test]
fn a_negated_descriptor_gate_holds_when_none_is_known() {
    assert!(gated("!PRESPELLDESCRIPTOR:1,Evil=1", &[("Fireball", 3)]));
    assert!(!gated("!PRESPELLDESCRIPTOR:1,Fire=3", &[("Fireball", 3)]));
}
