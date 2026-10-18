//! `PREALIGN:x,y` — the character's alignment must be one of the listed ones.
//!
//! PCGen's reference gives `PREALIGN:x,y` with `y: Text (The alignment name)`.
//! `PreAlignParser` does no validation — it sets the key to the raw text — and
//! `PreAlignTester.alignMatches` compares the character's alignment with the
//! named one, with exactly one special case:
//!
//! ```java
//! PCAlignment al = getPCAlignment(desiredAlignment);
//! if (al.equals(charAlignment)) { return true; }
//! else if (desiredAlignment.equalsIgnoreCase("Deity")) {
//!     // the character's alignment must equal their *deity's*
//! }
//! ```
//!
//! Two value forms need that reading:
//!
//! * `PREALIGN:Deity` (Complete Divine's *Arcane Disciple*) is the special case,
//!   not an alignment literally named "Deity" — though every game mode also
//!   defines one, so the literal comparison still comes first;
//! * `PREALIGN:0,1,3,4,6,7` (Book of Erotic Fantasy) is a key no alignment file
//!   defines, so it can never match. Dropping the requirement instead, which is
//!   what filtering the numbers out did, made those classes freely selectable.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/prealign.lst", 1, "prealign.pcc");
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

/// Is `BONUS:VAR|Gated|2|<prealign>` applied to this character?
fn gated(alignment: &str, deity: Option<&str>, prealign: &str) -> bool {
    let records = vec![
        record(
            "RACE",
            "Human",
            &[("SIZE", "M"), ("BONUS", &format!("VAR|Gated|2|{prealign}"))],
        ),
        record("DEITY", "Good God", &[("ALIGN", "LG")]),
        record("DEITY", "Evil God", &[("ALIGN", "LE")]),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Human".to_string(),
        alignment: alignment.to_string(),
        deity: deity.map(str::to_string),
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_named_alignment_matches_case_insensitively() {
    assert!(gated("NG", None, "PREALIGN:LG,NG,CG"));
    assert!(gated("ng", None, "PREALIGN:LG,NG,CG"));
    assert!(!gated("LE", None, "PREALIGN:LG,NG,CG"));
}

#[test]
fn deity_matches_when_the_character_shares_their_deitys_alignment() {
    // Arcane Disciple: `PREALIGN:Deity` — a Cleric may be one step from their
    // deity, so the character's own alignment is not the question here either.
    assert!(gated("LG", Some("Good God"), "PREALIGN:Deity"));
    assert!(!gated("LE", Some("Good God"), "PREALIGN:Deity"));
    // No deity chosen: PCGen's `deity != null` guard makes it false.
    assert!(!gated("LG", None, "PREALIGN:Deity"));
}

#[test]
fn a_literal_alignment_named_deity_still_matches_first() {
    // Every game mode defines `Deity's` with `KEY:Deity`, and PCGen compares the
    // character's alignment with it before the special case.
    assert!(gated("Deity", None, "PREALIGN:Deity"));
    assert!(gated("Deity", Some("Good God"), "PREALIGN:Deity"));
}

#[test]
fn a_numeric_key_fails_closed_rather_than_being_dropped() {
    // No alignment file defines a numeric key, so `PREALIGN:0,1,3,4,6,7` cannot
    // be satisfied — the class is unavailable, not unrestricted.
    assert!(!gated("LG", None, "PREALIGN:0,1,3,4,6,7"));
    assert!(!gated("TN", None, "PREALIGN:6"));
    // ...and its negation therefore holds.
    assert!(gated("LG", None, "!PREALIGN:6"));
    assert!(!gated("LG", None, "!PREALIGN:LG,NG,CG"));
}
