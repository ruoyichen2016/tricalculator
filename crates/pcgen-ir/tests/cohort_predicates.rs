//! Predicates the full RSRD uses that were not parsed.
//!
//! The acceptance cohort loads `_revised_system_reference_document.pcc` (the
//! full RSRD, epic included), which the basics-only pack does not cover. Four
//! tag kinds showed up there as unsupported.

use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/cohort.lst", 1, "cohort.pcc");
    RawRecord {
        category: "ABILITY".to_string(),
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

fn lowered(name: &str, tag: &str, value: &str) -> (Vec<String>, String) {
    let records = vec![record(
        name,
        &[("CATEGORY", "Special Ability"), (tag, value)],
    )];
    let (pack, _) = lower_records(Some("35e".to_string()), Vec::new(), &records);
    let entity = pack
        .entities
        .iter()
        .find(|entity| entity.id.key.eq_ignore_ascii_case(name))
        .expect("entity");
    (
        entity.unsupported_tags.clone(),
        format!("{:?}", entity.requirements),
    )
}

#[test]
fn legs_at_most_is_read() {
    // 27 uses in the full RSRD, the largest of the four.
    let (unsupported, requirements) = lowered("Gated", "PRELEGSLTEQ", "4");
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert!(requirements.contains("LegsAtMost"), "{requirements}");
    // The companion spelling still works.
    let (_, requirements) = lowered("Other", "PRELEGSGTEQ", "4");
    assert!(requirements.contains("LegsAtLeast"), "{requirements}");
}

#[test]
fn hands_at_most_is_read() {
    let (unsupported, requirements) = lowered("Gated", "PREHANDSLTEQ", "2");
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert!(requirements.contains("HandsAtMost"), "{requirements}");
}

#[test]
fn every_spell_type_pair_is_read() {
    // `PRESPELLTYPE:1,Arcane=3,Divine=3,Psionic=3` — "at least one 3rd level
    // spell from any of the Arcane, Divine, or Psionic types". Only the first
    // pair was read, which left `Arcane=9,Divine=9` unparseable.
    let (unsupported, requirements) =
        lowered("Gated", "PRESPELLTYPE", "1,Arcane=3,Divine=3,Psionic=3");
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert!(requirements.contains("Arcane"), "{requirements}");
    assert!(requirements.contains("Divine"), "{requirements}");
    assert!(requirements.contains("Psionic"), "{requirements}");
}

#[test]
fn any_is_still_understood_alongside_others() {
    let (unsupported, requirements) = lowered("Gated", "PRESPELLTYPE", "1,ANY=3");
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert!(requirements.contains("power: None"), "{requirements}");
}
