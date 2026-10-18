//! `PRECHARACTERTYPE:x,y,y` — the character's type.
//!
//! PCGen's reference:
//!
//! ```text
//! PRECHARACTERTYPE:x,y,y
//!   x: Number (Number of character types required)
//!   y: Text (Character type.)
//!
//! PRECHARACTERTYPE:1,PC           "Character must be designated as a PC"
//! PRECHARACTERTYPE:1,NPC,Monster  "either an NPC or a Monster"
//! ```
//!
//! `PreCharactertypeTester` reads the `CHARACTERTYPE` channel and compares it,
//! case-insensitively, with each listed key; the list wrapper sums the matches.
//! A character has exactly one type, so only a duplicated list can count twice.
//!
//! PCGen's default comes from the game mode — `getDefaultCharacterType` returns
//! the first entry of `CHARACTERTYPE:` in `system/gameModes/{3e,35e}/miscinfo.lst`,
//! which is `PC|NPC`.  That game-mode file is outside a pack's `.pcc` graph, so
//! the draft carries the 3e/3.5e default instead of importing it.

use pcgen_engine::{apply_effects, default_character_type, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/type.lst", 1, "type.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character of this type?
fn gated(gate: &str, character_type: &str) -> bool {
    let records = vec![record(
        "RACE",
        "Test Race",
        &[("BONUS", &format!("VAR|Gated|1|{gate}"))],
    )];
    let (pack, report) = lower_records(Some("5e".to_string()), vec!["type".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        character_type: character_type.to_string(),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn the_default_character_type_is_a_pc() {
    // The 3e/3.5e game modes list `PC|NPC`, and PCGen takes the first entry.
    assert_eq!(default_character_type(), "PC");
    assert_eq!(CharacterDraft::default().character_type, "PC");
}

#[test]
fn a_type_matches_case_insensitively() {
    assert!(gated("PRECHARACTERTYPE:1,PC", "PC"));
    assert!(gated("PRECHARACTERTYPE:1,pc", "PC"));
    assert!(!gated("PRECHARACTERTYPE:1,NPC", "PC"));
    assert!(gated("PRECHARACTERTYPE:1,NPC", "NPC"));
}

#[test]
fn a_list_accepts_any_of_its_types() {
    assert!(gated("PRECHARACTERTYPE:1,PC,NPC", "PC"));
    assert!(gated("PRECHARACTERTYPE:1,PC,NPC", "NPC"));
    assert!(!gated("PRECHARACTERTYPE:1,PC,NPC", "Monster"));
    // The count is a total, and one character has one type, so a count of two
    // can only be met by a list that names the same type twice.
    assert!(!gated("PRECHARACTERTYPE:2,PC,NPC", "PC"));
    assert!(gated("PRECHARACTERTYPE:2,PC,PC", "PC"));
}

#[test]
fn a_negated_type_gate_holds_for_another_type() {
    assert!(gated("!PRECHARACTERTYPE:1,NPC", "PC"));
    assert!(!gated("!PRECHARACTERTYPE:1,PC", "PC"));
}
