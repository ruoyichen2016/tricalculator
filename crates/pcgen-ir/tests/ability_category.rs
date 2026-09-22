use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/abilities.lst", 1, "abilities.pcc");
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
fn ability_category_is_captured_and_not_legacy() {
    let r = record(
        "ABILITY",
        "Astral Armor",
        &[("CATEGORY", "Special Ability"), ("TYPE", "SpecialQuality")],
    );
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["a".to_string()], &[r]);
    let entity = &pack.entities[0];
    assert_eq!(entity.ability_category.as_deref(), Some("Special Ability"));
    // CATEGORY must not be reported as legacy anymore.
    assert!(!entity
        .legacy_tags
        .iter()
        .any(|t| t.starts_with("CATEGORY:")));
    assert_eq!(report.coverage.legacy_by_tag.get("CATEGORY"), None);
}

#[test]
fn ability_category_on_non_ability_records_stays_legacy() {
    // CATEGORY on a SKILL is not a pool declaration and must remain legacy.
    let r = record("SKILL", "Climb", &[("CATEGORY", "Whatever")]);
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["a".to_string()], &[r]);
    assert_eq!(pack.entities[0].ability_category, None);
    assert!(pack.entities[0]
        .legacy_tags
        .iter()
        .any(|t| t.starts_with("CATEGORY:")));
}

#[test]
fn ability_category_survives_serde() {
    let r = record("ABILITY", "X", &[("CATEGORY", "FEAT")]);
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["a".to_string()], &[r]);
    let json = serde_json::to_string(&pack.entities[0]).expect("serialize");
    assert!(
        json.contains("\"ability_category\":\"FEAT\""),
        "got: {json}"
    );
    let back: pcgen_core::CompiledEntity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.ability_category.as_deref(), Some("FEAT"));
}
