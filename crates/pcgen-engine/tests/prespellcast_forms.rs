//! `PRESPELLCAST` — one sub-requirement per comma token.
//!
//! `PreSpellCastParser` splits the formula on commas and, per token, builds either
//! a `spellcast.memorize` or a `spellcast.type` sub-requirement; any other token
//! makes it throw.  The group keeps the parser's default operand of 1, so two
//! tokens mean "at least one of them holds", and the wrapper is dropped only when
//! there is a single sub-requirement.
//!
//! `PreSpellCastMemorizeTester` counts the classes whose `MEMORIZE_SPELLS` flag
//! matches the requested side.  `PreSpellCastTester` compares the token's text
//! with `aClass.getSpellType()`, and that method returns the class's
//! `FACT:SpellType` verbatim — or `None` when it declares none — so the spell type
//! is a *string*: `TYPE=Arcane=1` names a spell type no class has and is false
//! rather than unreadable.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_import::PcgClassLevel as DraftClassLevel;
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/spellcast.lst", 1, "spellcast.pcc");
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
        record(
            "CLASS",
            "Wizard",
            &[("FACT", "SpellType|Arcane"), ("SPELLSTAT", "INT")],
        ),
        record(
            "CLASS",
            "Sorcerer",
            &[
                ("FACT", "SpellType|Arcane"),
                ("SPELLSTAT", "CHA"),
                ("MEMORIZE", "NO"),
            ],
        ),
        record(
            "CLASS",
            "Cleric",
            &[("FACT", "SpellType|Divine"), ("SPELLSTAT", "WIS")],
        ),
        record("CLASS", "Fighter", &[("HD", "10"), ("BAB", "GOOD")]),
        record(
            "FEAT",
            "Gated Feat",
            &[
                ("TYPE", "General"),
                ("BONUS", &format!("VAR|Gated|1|{gate}")),
            ],
        ),
    ];
    let (pack, report) = lower_records(
        Some("35e".to_string()),
        vec!["spellcast".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        advancement: pcgen_import::advancement_from_class_levels(
            &class_levels
                .iter()
                .map(|(class_id, level)| DraftClassLevel {
                    class_id: (*class_id).to_string(),
                    level: *level,
                    hit_points_gained: 0,
                    hit_points_by_level: Vec::new(),
                    experience_gained: 0,
                })
                .collect::<Vec<_>>(),
        ),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_spell_type_is_matched_as_text() {
    assert!(gated("PRESPELLCAST:TYPE=Arcane", &[("Wizard", 1)]));
    assert!(!gated("PRESPELLCAST:TYPE=Arcane", &[("Cleric", 1)]));
    assert!(gated("PRESPELLCAST:TYPE=Divine", &[("Cleric", 1)]));
    // Case-insensitively, because the tester uses `equalsIgnoreCase`.
    assert!(gated("PRESPELLCAST:TYPE=arcane", &[("Wizard", 1)]));
    // A class that declares no spell type reads as `None`, which is what
    // `PCClass.getSpellType` answers for an unset fact.
    assert!(gated("PRESPELLCAST:TYPE=None", &[("Fighter", 1)]));
    assert!(!gated("PRESPELLCAST:TYPE=Arcane", &[("Fighter", 1)]));
    assert!(!gated("PRESPELLCAST:TYPE=Arcane", &[]));
}

#[test]
fn every_token_counts_not_just_the_first() {
    // The bug this fixes: only the first `TYPE=` was read, so this tested Arcane
    // alone and failed for a cleric.
    assert!(gated(
        "PRESPELLCAST:TYPE=Arcane,TYPE=Divine",
        &[("Cleric", 1)]
    ));
    assert!(gated(
        "PRESPELLCAST:TYPE=Arcane,TYPE=Divine",
        &[("Wizard", 1)]
    ));
    assert!(!gated(
        "PRESPELLCAST:TYPE=Arcane,TYPE=Divine",
        &[("Fighter", 1)]
    ));
    assert!(!gated("PRESPELLCAST:TYPE=Arcane,TYPE=Divine", &[]));
}

#[test]
fn a_type_that_names_no_class_is_false_rather_than_unreadable() {
    // `TYPE=Arcane=1` is the third-party form: the parser takes everything after
    // `TYPE=` as the spell type, so this asks for a class whose spell type is
    // literally `Arcane=1`.  Nothing matches, and PCGen agrees.
    assert!(!gated("PRESPELLCAST:TYPE=Arcane=1", &[("Wizard", 1)]));
    assert!(!gated("PRESPELLCAST:TYPE=Arcane=1", &[]));
    assert!(gated("!PRESPELLCAST:TYPE=Arcane=1", &[("Wizard", 1)]));
}

#[test]
fn memorize_selects_the_prepared_side() {
    // The wizard memorises (the class has a spell stat and no `MEMORIZE:NO`), the
    // sorcerer does not.
    assert!(gated("PRESPELLCAST:MEMORIZE=Y", &[("Wizard", 1)]));
    assert!(!gated("PRESPELLCAST:MEMORIZE=N", &[("Wizard", 1)]));
    assert!(gated("PRESPELLCAST:MEMORIZE=N", &[("Sorcerer", 1)]));
    assert!(!gated("PRESPELLCAST:MEMORIZE=Y", &[("Sorcerer", 1)]));
    // The reference reads anything not starting with `N` as the prepared side.
    assert!(gated("PRESPELLCAST:MEMORIZE=Yes", &[("Wizard", 1)]));
    assert!(gated("PRESPELLCAST:MEMORIZE=No", &[("Sorcerer", 1)]));
}

#[test]
fn mixed_tokens_are_a_group() {
    // A prepared class that is not divine: the second token carries it.
    assert!(gated(
        "PRESPELLCAST:MEMORIZE=N,TYPE=Divine",
        &[("Cleric", 1)]
    ));
    assert!(!gated(
        "PRESPELLCAST:MEMORIZE=N,TYPE=Divine",
        &[("Wizard", 1)]
    ));
}

#[test]
fn a_negated_spell_cast_holds_for_the_other_side() {
    assert!(gated("!PRESPELLCAST:TYPE=Arcane", &[("Cleric", 1)]));
    assert!(!gated("!PRESPELLCAST:TYPE=Arcane", &[("Wizard", 1)]));
}
