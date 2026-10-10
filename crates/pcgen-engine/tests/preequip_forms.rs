//! `PREEQUIP*` keys — a name, a type, a base item or a wield category.
//!
//! Both testers behind this family branch on the key's prefix:
//!
//! ```java
//! if (targetEquip.startsWith("WIELDCATEGORY=") || targetEquip.startsWith("WIELDCATEGORY."))
//!     ... wCat.getKeyName().equalsIgnoreCase(targetEquip.substring(14)) ...
//! else if (targetEquip.startsWith("TYPE=") || targetEquip.startsWith("TYPE."))
//!     // every dot-separated type must match
//! else { ... eq.getBaseItemName() or eq.getName(), with a `%` wildcard ... }
//! ```
//!
//! The lowering used to keep the keys as raw lower-cased text, so a `TYPE=Weapon`
//! key looked for an item whose *name* contains `type=weapon` — which no item is —
//! and every such requirement was silently false.  `TYPE=` is the commonest form in
//! the third-party data (450 of 575 `PREEQUIP*` occurrences).
//!
//! The count that leads the value is a count of *keys*, because each key's tester
//! answers 0 or 1: `PREEQUIP:2,Chain,Sword` wants both keys to match, and
//! `PREEQUIP:2,Chain` can never be satisfied.

use pcgen_engine::{apply_effects, CharacterDraft};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/preequip.lst", 1, "preequip.pcc");
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

/// Is `BONUS:VAR|Gated|1|<gate>` applied to a character carrying `equipment`?
///
/// `base_items` fills in the `.pcg`'s `EQUIPBASEITEM` knowledge, which is where a
/// `BASEITEM=` key is answered from.
fn gated(gate: &str, equipment: &[&str], base_items: &[(&str, &str)]) -> bool {
    let records = vec![
        record("RACE", "Test Race", &[("SIZE", "M"), ("TYPE", "Humanoid")]),
        record(
            "EQUIPMENT",
            "Longsword",
            &[("TYPE", "Weapon.Melee"), ("WIELD", "OneHanded")],
        ),
        record(
            "EQUIPMENT",
            "Greatsword",
            &[("TYPE", "Weapon.Melee.TwoHanded"), ("WIELD", "TwoHanded")],
        ),
        record(
            "EQUIPMENT",
            "Chainmail",
            &[("TYPE", "Armor.Heavy"), ("WIELD", "Light")],
        ),
        record("EQUIPMENT", "Shield", &[("TYPE", "Armor.Shield")]),
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
        vec!["preequip".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.unsupported_tags, 0,
        "{:?}",
        report.coverage.unsupported_pending_by_tag
    );
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        equipment: equipment.iter().map(|item| (*item).to_string()).collect(),
        equipment_base_items: base_items
            .iter()
            .map(|(item, base)| ((*item).to_ascii_lowercase(), (*base).to_string()))
            .collect::<BTreeMap<_, _>>(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

/// The same gate, with the character's equip-set slots filled in.
fn gated_with_slots(
    gate: &str,
    equipment: &[&str],
    equip_slots: &std::collections::BTreeMap<String, String>,
) -> bool {
    let records = vec![
        record("RACE", "Test Race", &[("SIZE", "M"), ("TYPE", "Humanoid")]),
        record(
            "EQUIPMENT",
            "Rapier",
            &[("TYPE", "Weapon.Melee"), ("WIELD", "OneHanded")],
        ),
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
        vec!["preequip".to_string()],
        &records,
    );
    assert_eq!(report.coverage.unsupported_tags, 0);
    let draft = CharacterDraft {
        race_id: "Test Race".to_string(),
        equipment: equipment.iter().map(|item| (*item).to_string()).collect(),
        equip_slots: equip_slots.clone(),
        abilities: vec![("FEAT".to_string(), "Gated Feat".to_string())],
        ..CharacterDraft::default()
    };
    apply_effects(&pack, &draft).variables.contains_key("Gated")
}

#[test]
fn a_type_key_matches_the_items_types() {
    // The fix: this used to look for an item named `type=weapon`.
    assert!(gated("PREEQUIP:1,TYPE=Weapon", &["Longsword"], &[]));
    assert!(!gated("PREEQUIP:1,TYPE=Weapon", &["Chainmail"], &[]));
    assert!(gated("PREEQUIP:1,TYPE=Armor", &["Chainmail"], &[]));
    // Every dot-separated type has to match.
    assert!(gated("PREEQUIP:1,TYPE=Weapon.Melee", &["Longsword"], &[]));
    assert!(!gated("PREEQUIP:1,TYPE=Weapon.Armor", &["Longsword"], &[]));
    // The dotted spelling reads the same way, case-insensitively.
    assert!(gated("PREEQUIP:1,TYPE.weapon", &["Longsword"], &[]));
    // An item that is not carried at all does not match.
    assert!(!gated("PREEQUIP:1,TYPE=Weapon", &[], &[]));
}

#[test]
fn a_name_key_takes_a_wildcard() {
    assert!(gated("PREEQUIP:1,Greatsword", &["Greatsword"], &[]));
    assert!(!gated("PREEQUIP:1,Greatsword", &["Longsword"], &[]));
    assert!(gated("PREEQUIP:1,Great%", &["Greatsword"], &[]));
    assert!(!gated("PREEQUIP:1,Great%", &["Longsword"], &[]));
    // Case-insensitively.
    assert!(gated("PREEQUIP:1,greatsword", &["Greatsword"], &[]));
}

#[test]
fn a_base_item_key_reads_the_customisation() {
    // The `.pcg` says this item was customised from a Longsword.
    assert!(gated(
        "PREEQUIP:1,BASEITEM=Longsword",
        &["Rick's Longsword"],
        &[("Rick's Longsword", "Longsword")]
    ));
    assert!(!gated(
        "PREEQUIP:1,BASEITEM=Greatsword",
        &["Rick's Longsword"],
        &[("Rick's Longsword", "Longsword")]
    ));
}

#[test]
fn a_wield_category_key_reads_the_wield() {
    assert!(gated(
        "PREEQUIP:1,WIELDCATEGORY=OneHanded",
        &["Longsword"],
        &[]
    ));
    assert!(!gated(
        "PREEQUIP:1,WIELDCATEGORY=TwoHanded",
        &["Longsword"],
        &[]
    ));
    assert!(gated(
        "PREEQUIP:1,WIELDCATEGORY=TwoHanded",
        &["Greatsword"],
        &[]
    ));
    // The reference's spelling of a category varies, so the comparison folds case
    // and separators.
    assert!(gated(
        "PREEQUIP:1,WIELDCATEGORY=two-handed",
        &["Greatsword"],
        &[]
    ));
}

#[test]
fn the_count_is_a_count_of_keys() {
    // Both named keys have to match.
    assert!(gated(
        "PREEQUIP:2,Longsword,Chainmail",
        &["Longsword", "Chainmail"],
        &[]
    ));
    assert!(!gated(
        "PREEQUIP:2,Longsword,Chainmail",
        &["Longsword"],
        &[]
    ));
    // One key cannot be two.
    assert!(!gated("PREEQUIP:2,Longsword", &["Longsword"], &[]));
    assert!(gated("PREEQUIP:1,Longsword", &["Longsword"], &[]));
    // A count of zero is always satisfied, as `compare(x, 0)` is.
    assert!(gated("PREEQUIP:0,Longsword", &[], &[]));
}

#[test]
fn the_hand_slots_still_narrow_the_search() {
    // A greatsword is held in both hands and is not in the primary hand.
    assert!(gated("PREEQUIPBOTH:1,TYPE=Weapon", &["Greatsword"], &[]));
    assert!(!gated(
        "PREEQUIPPRIMARY:1,TYPE=Weapon",
        &["Greatsword"],
        &[]
    ));
    assert!(gated("PREEQUIPPRIMARY:1,TYPE=Weapon", &["Longsword"], &[]));
    // Carried armor is in neither hand, but the plain form does not care where it
    // is held.
    assert!(!gated("PREEQUIPPRIMARY:1,TYPE=Armor", &["Chainmail"], &[]));
    assert!(gated("PREEQUIP:1,TYPE=Armor", &["Chainmail"], &[]));
}

#[test]
fn a_negated_equip_holds_for_the_other_item() {
    assert!(gated("!PREEQUIP:1,TYPE=Weapon", &["Chainmail"], &[]));
    assert!(!gated("!PREEQUIP:1,TYPE=Weapon", &["Longsword"], &[]));
}

#[test]
fn the_two_weapons_slot_comes_from_the_equip_set() {
    // `EQUIPPED_TWO_HANDS` is not an item property: the `.pcg`'s equip set puts a
    // weapon there with a `Two Weapons` or `Double Weapon` slot, which is what
    // `draft.equip_slots` carries.
    let two_hands =
        std::collections::BTreeMap::from([("Rapier".to_string(), "two_hands".to_string())]);
    assert!(gated_with_slots(
        "PREEQUIPTWOWEAPON:1,TYPE=Weapon",
        &["Rapier"],
        &two_hands
    ));
    assert!(!gated_with_slots(
        "PREEQUIPTWOWEAPON:1,TYPE=Armor",
        &["Rapier"],
        &two_hands
    ));
    // An item in the primary hand is not in the two-weapons location.
    let primary = std::collections::BTreeMap::from([("Rapier".to_string(), "primary".to_string())]);
    assert!(!gated_with_slots(
        "PREEQUIPTWOWEAPON:1,TYPE=Weapon",
        &["Rapier"],
        &primary
    ));
    // Nor is one the file says nothing about.
    assert!(!gated_with_slots(
        "PREEQUIPTWOWEAPON:1,TYPE=Weapon",
        &["Rapier"],
        &std::collections::BTreeMap::new()
    ));
    // The double-weapon spelling of the slot is the same location.
    let double =
        std::collections::BTreeMap::from([("Rapier".to_string(), "two_hands".to_string())]);
    assert!(gated_with_slots(
        "PREEQUIPTWOWEAPON:1,Rapier",
        &["Rapier"],
        &double
    ));
}
