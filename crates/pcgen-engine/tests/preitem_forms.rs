//! `PREITEM:x,y,y` — items the character possesses.
//!
//! PCGen's reference:
//!
//! ```text
//! PREITEM:x,y,y
//!   x: Number (The number of items a character must possess).
//!   y: Text (The name of an item a character must possess - "%" may be used as a
//!      wildcard).
//!   y: TYPE=Text (The type of an item a character must possess).
//! ```
//!
//! `PreItemTester` walks `display.getEquipmentSet()` — everything the character
//! *possesses*, which is why this is not `PREEQUIP` — and per listed entry either
//! matches a name (exact, or a prefix when the entry carries a `%`, stopping at the
//! first hit) or counts every item carrying all of the `TYPE=` types (no `break`).
//! The list wrapper sums the entries and compares with the count.
//!
//! The engine previously lowered `PREITEM` to an *equipped* test and only for
//! `TYPE=` entries, so a name entry was unsupported and a type entry asked the
//! wrong question.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/item.lst", 1, "item.pcc");
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

fn pack_records() -> Vec<RawRecord> {
    vec![
        record("RACE", "Test Race", &[("TYPE", "Humanoid")]),
        record("EQUIPMENT", "Chainmail", &[("TYPE", "Armor.Heavy")]),
        record("EQUIPMENT", "Scale Mail", &[("TYPE", "Armor.Medium")]),
        record("EQUIPMENT", "Sword (Long)", &[("TYPE", "Weapon.Melee")]),
        record("FEAT", "Gated Feat", &[("TYPE", "General")]),
    ]
}

/// Is the gate applied to a character possessing `equipment`?
fn gated(gate: &str, equipment: &[&str]) -> bool {
    gated_with_base(gate, equipment, &[])
}

/// The same, with customised items mapped to their base items.
fn gated_with_base(gate: &str, equipment: &[&str], base: &[(&str, &str)]) -> bool {
    let mut records = pack_records();
    let feat = records
        .iter_mut()
        .find(|record| record.name == "Gated Feat")
        .expect("feat");
    feat.tags.push(RawTag {
        name: "BONUS".to_string(),
        value: format!("VAR|Gated|1|{gate}"),
        source: feat.source.clone(),
    });

    let (pack, report) = lower_records(Some("35e".to_string()), vec!["item".to_string()], &records);
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        equipment: equipment.iter().map(|item| (*item).to_string()).collect(),
        equipment_base_items: base
            .iter()
            .map(|(item, base)| ((*item).to_string(), (*base).to_string()))
            .collect::<BTreeMap<_, _>>(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_name_entry_matches_a_possessed_item() {
    assert!(gated("PREITEM:1,Chainmail", &["Chainmail"]));
    assert!(!gated("PREITEM:1,Chainmail", &[]));
    assert!(!gated("PREITEM:1,Chainmail", &["Sword (Long)"]));
    // Case-insensitively.
    assert!(gated("PREITEM:1,chainmail", &["Chainmail"]));
    // Possession, not equipment: nothing in this draft is equipped.
    assert!(gated("PREITEM:1,Chainmail", &["Chainmail", "Sword (Long)"]));
}

#[test]
fn a_wildcard_matches_a_prefix() {
    assert!(gated("PREITEM:1,Sword%", &["Sword (Long)"]));
    assert!(!gated("PREITEM:1,Axe%", &["Sword (Long)"]));
    // A bare `%` has an empty prefix, which every name starts with.
    assert!(gated("PREITEM:1,%", &["Sword (Long)"]));
    assert!(!gated("PREITEM:1,%", &[]));
    // Everything from the first `%` on is ignored.
    assert!(gated("PREITEM:1,Sword% of Sharpness", &["Sword (Long)"]));
}

#[test]
fn a_type_entry_counts_every_matching_item() {
    assert!(gated("PREITEM:1,TYPE=Weapon", &["Sword (Long)"]));
    assert!(!gated("PREITEM:1,TYPE=Weapon", &["Chainmail"]));
    // Several types are all required.
    assert!(gated("PREITEM:1,TYPE=Armor.Heavy", &["Chainmail"]));
    assert!(!gated("PREITEM:1,TYPE=Armor.Light", &["Chainmail"]));
    assert!(!gated("PREITEM:1,TYPE=Armor.Heavy", &["Scale Mail"]));
}

#[test]
fn the_count_is_a_total_across_the_entries() {
    // Two name entries, one item each.
    assert!(gated(
        "PREITEM:2,Chainmail,Sword (Long)",
        &["Chainmail", "Sword (Long)"]
    ));
    assert!(!gated("PREITEM:2,Chainmail,Sword (Long)", &["Chainmail"]));
    // Either of two names satisfies a count of one.
    assert!(gated(
        "PREITEM:1,Sword (Long),Sword (Short)",
        &["Sword (Short)"]
    ));
    // A `TYPE` entry counts every item it matches, and the entries sum, so the
    // same type listed twice is satisfied by a single item.
    assert!(gated("PREITEM:2,TYPE=Armor,TYPE=Armor", &["Chainmail"]));
    assert!(!gated("PREITEM:3,TYPE=Armor,TYPE=Armor", &["Chainmail"]));
    assert!(!gated("PREITEM:2,TYPE=Armor,TYPE=Armor", &[]));
}

#[test]
fn a_customised_item_keeps_its_base_items_types() {
    // `+2 scale mail` is not a catalogue entry, but the engine already resolves
    // such a name through `BASEITEM`, and the types have to follow.
    assert!(gated_with_base(
        "PREITEM:1,TYPE=Armor",
        &["+2 scale mail"],
        &[("+2 scale mail", "Scale Mail")]
    ));
    assert!(!gated_with_base(
        "PREITEM:1,TYPE=Armor",
        &["+2 scale mail"],
        &[]
    ));
}

#[test]
fn a_negated_item_gate_holds_when_nothing_is_possessed() {
    assert!(gated("!PREITEM:1,Chainmail", &[]));
    assert!(!gated("!PREITEM:1,Chainmail", &["Chainmail"]));
}
