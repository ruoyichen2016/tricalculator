//! `PREGENDER` — a requirement that had no data behind it.
//!
//! PCGen's reference:
//!
//! ```text
//! PREGENDER:x
//!   x: Text (Gender to require)
//!
//! "The character's gender must start with the text specified. The test is
//!  case sensitive so "male" is not the same as "Male"."
//! ```
//!
//! 60 uses in the vendored data, and 40 of the 42 vendored `.pcg` files carry a
//! `GENDER:` line — but the character model had no gender field at all, so the
//! import dropped it and the requirement could never be evaluated.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/gender.lst", 1, "gender.pcc");
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

/// Is the ability gated on `pregender` active for this gender?
fn active(pregender: &str, gender: &str) -> bool {
    let records = vec![
        record(
            "ABILITY",
            "Gated",
            &[
                ("CATEGORY", "Special Ability"),
                ("TYPE", "SpecialQuality"),
                ("PREGENDER", pregender),
                ("AUTO", "WEAPONPROF|Canaryblade"),
            ],
        ),
        record(
            "TEMPLATE",
            "Grantor",
            &[("ABILITY", "Special Ability|AUTOMATIC|Gated")],
        ),
    ];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let draft = CharacterDraft {
        gender: gender.to_string(),
        templates: vec!["Grantor".to_string()],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft)
        .weapon_proficiencies
        .contains("canaryblade")
}

#[test]
fn a_prefix_matches() {
    assert!(active("M", "Male"));
    assert!(active("F", "Female"));
    assert!(!active("M", "Female"));
    assert!(!active("F", "Male"));
}

#[test]
fn the_match_is_case_sensitive() {
    // "The test is case sensitive so "male" is not the same as "Male"."
    assert!(!active("male", "Male"), "lowercase must not match");
    assert!(!active("Male", "male"));
    assert!(active("Male", "Male"));
}

#[test]
fn a_longer_prefix_is_not_a_match() {
    // `M` is a prefix of `Male`, but `Male` is not a prefix of `M`.
    assert!(!active("Male", "M"));
}

#[test]
fn an_empty_gender_matches_nothing() {
    assert!(!active("M", ""));
}
