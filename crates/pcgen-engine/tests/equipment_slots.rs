//! `PREEQUIP*` refinement: primary / secondary / both hand semantics.

use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/equip.lst", 1, "equip.pcc");
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

#[test]
fn preequip_primary_secondary_and_both_are_distinguished() {
    let records = vec![
        record("RACE", "Human", &[("SIZE", "M")]),
        record(
            "EQUIPMENT",
            "Longsword",
            &[("TYPE", "Weapon"), ("WIELD", "OneHanded")],
        ),
        record(
            "EQUIPMENT",
            "Shortsword",
            &[("TYPE", "Weapon"), ("WIELD", "OffHand")],
        ),
        record(
            "EQUIPMENT",
            "Greatsword",
            &[("TYPE", "Weapon"), ("WIELD", "TwoHanded")],
        ),
        record(
            "RACE",
            "Sword Trait",
            &[
                ("BONUS", "VAR|PrimaryOnly|1|PREEQUIPPRIMARY:1,Longsword"),
                (
                    "BONUS",
                    "VAR|SecondaryOnly|1|PREEQUIPSECONDARY:1,Shortsword",
                ),
                ("BONUS", "VAR|BothOnly|1|PREEQUIPBOTH:1,Greatsword"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["preequip".to_string()],
        &records,
    );

    let longsword = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Sword Trait".to_string(),
        equipment: vec!["Longsword".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(longsword.variables.get("PrimaryOnly").copied(), Some(1));
    assert!(!longsword.variables.contains_key("SecondaryOnly"));
    assert!(!longsword.variables.contains_key("BothOnly"));

    let greatsword = Engine::new(pack.clone()).evaluate(&CharacterDraft {
        race_id: "Sword Trait".to_string(),
        equipment: vec!["Greatsword".to_string()],
        ..CharacterDraft::default()
    });
    assert_eq!(greatsword.variables.get("BothOnly").copied(), Some(1));
    assert!(!greatsword.variables.contains_key("PrimaryOnly"));
}
