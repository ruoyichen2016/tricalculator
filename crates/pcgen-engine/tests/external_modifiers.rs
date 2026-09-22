use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/modifiers.lst", 1, "modifiers.pcc");
    RawRecord {
        category: "RACE".to_string(),
        key: name.to_string(),
        name: name.to_string(),
        tags: tags
            .iter()
            .map(|(name, value)| RawTag {
                name: (*name).to_string(),
                value: (*value).to_string(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

#[test]
fn external_modifiers_apply_to_snapshot_fields() {
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["modifiers".to_string()],
        &[record(
            "Modifier Race",
            &[
                ("BONUS", "HP|5"),
                ("BONUS", "COMBAT|AC|2"),
                ("BONUS", "SKILL|Climb|3"),
            ],
        )],
    );
    let draft = CharacterDraft {
        race_id: "Modifier Race".to_string(),
        stats: BTreeMap::from([("STR".to_string(), 14), ("CON".to_string(), 10)]),
        skills: BTreeMap::from([("Climb".to_string(), 1.0)]),
        ..CharacterDraft::default()
    };
    let snapshot = Engine::new(pack).evaluate(&draft);
    assert_eq!(snapshot.hit_points, 6); // 1 base + 5
    assert_eq!(snapshot.armor_class, 12);
    assert_eq!(snapshot.touch_ac, 12);
    assert_eq!(snapshot.flat_footed_ac, 12);
    assert!(snapshot.skills["Climb"] >= 4);
}
