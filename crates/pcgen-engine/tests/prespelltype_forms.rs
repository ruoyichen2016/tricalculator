//! `PRESPELLTYPE:x,type=level` — spell slots of a named type.
//!
//! `PreSpellTypeTester` passes the type straight to `countSpellCastTypeLevel`:
//!
//! ```java
//! for (PCClass aClass : getClassSet())
//! {
//!     String classSpellType = aClass.getResolved(FactKey.valueOf("SpellType"));
//!     if (classSpellType != null
//!             && ("Any".equalsIgnoreCase(spellType) || classSpellType.equalsIgnoreCase(spellType)))
//!     { ... known += ...; cast += ...; }
//! }
//! ```
//!
//! Three things follow, and all three were previously unrepresentable because the
//! type was a `SpellPowerType` enum: the type is *text* (the data's `Shadow Magic`
//! is a real one), `Any` is the wildcard, and a class whose `SpellType` fact is
//! unset is **skipped** rather than treated as a type of its own.

use pcgen_engine::{apply_effects, CharacterDraft, DraftClassLevel};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/spelltype.lst", 1, "spelltype.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character with one class?
fn gated(gate: &str, class_id: &str, cast: &str) -> bool {
    let mut class_tags: Vec<(&str, &str)> = vec![("HD", "4"), ("SPELLSTAT", "CHA")];
    match class_id {
        "Shadow Mage" => class_tags.push(("FACT", "SpellType|Shadow Magic")),
        "Wizard" => class_tags.push(("FACT", "SpellType|Arcane")),
        // `Mystic` deliberately declares no spell type at all.
        _ => {}
    }
    let records = vec![
        record("RACE", "Test Race", &[("TYPE", "Humanoid")]),
        record("CLASS", class_id, &class_tags),
        record("CLASS", "9", &[("CAST", cast)]),
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
        Some("3e".to_string()),
        vec!["spelltype".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        class_levels: vec![DraftClassLevel {
            class_id: class_id.to_string(),
            level: 9,
            hit_points_gained: 0,
            hit_points_by_level: Vec::new(),
            experience_gained: 0,
        }],
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_spell_type_the_enum_never_knew_is_an_ordinary_string() {
    // The whole point: `Shadow Magic` used to make the tag unreadable.
    assert!(gated(
        "PRESPELLTYPE:1,Shadow Magic=1",
        "Shadow Mage",
        "4,3,2,1"
    ));
    // It is not Arcane, and an arcane class is not it.
    assert!(!gated("PRESPELLTYPE:1,Arcane=1", "Shadow Mage", "4,3,2,1"));
    assert!(!gated("PRESPELLTYPE:1,Shadow Magic=1", "Wizard", "4,3,2,1"));
    assert!(gated("PRESPELLTYPE:1,Arcane=1", "Wizard", "4,3,2,1"));
    // Case-insensitively, because the reference uses `equalsIgnoreCase`.
    assert!(gated(
        "PRESPELLTYPE:1,shadow magic=1",
        "Shadow Mage",
        "4,3,2,1"
    ));
}

#[test]
fn any_is_the_wildcard() {
    assert!(gated("PRESPELLTYPE:1,any=1", "Wizard", "4,3,2,1"));
    assert!(gated("PRESPELLTYPE:1,Any=1", "Shadow Mage", "4,3,2,1"));
    // A class with no spell type at all is skipped by `countSpellCastTypeLevel`,
    // so even the wildcard finds nothing to match.
    assert!(!gated("PRESPELLTYPE:1,any=1", "Mystic", "4,3,2,1"));
    // The same class is still reachable through `PRESPELLCAST`, where
    // `getSpellType` answers `None` for an unset fact — the two tags disagree about
    // that case and both are followed here.
    assert!(!gated("PRESPELLTYPE:1,None=1", "Mystic", "4,3,2,1"));
}

#[test]
fn the_level_has_to_be_reached() {
    assert!(!gated(
        "PRESPELLTYPE:1,Shadow Magic=9",
        "Shadow Mage",
        "4,3,2,1"
    ));
    assert!(gated(
        "PRESPELLTYPE:1,Shadow Magic=3",
        "Shadow Mage",
        "4,3,2,1"
    ));
    // A level with no slots at all does not count either.
    assert!(!gated(
        "PRESPELLTYPE:1,Shadow Magic=1",
        "Shadow Mage",
        "0,0,0,0"
    ));
}

#[test]
fn a_negated_spell_type_holds_for_the_other_class() {
    assert!(gated("!PRESPELLTYPE:1,Arcane=1", "Shadow Mage", "4,3,2,1"));
    assert!(!gated("!PRESPELLTYPE:1,Arcane=1", "Wizard", "4,3,2,1"));
}
