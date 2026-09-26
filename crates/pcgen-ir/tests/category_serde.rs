use pcgen_core::{CompiledEntity, EntityCategory, EntityId};

#[test]
fn compiled_entity_category_serializes_as_pcgen_string() {
    let entity = CompiledEntity {
        id: EntityId::from_pcgen("3e", "srd", "CLASS", "Fighter"),
        category: EntityCategory::ClassLevel,
        name: "Fighter".to_string(),
        effects: Vec::new(),
        requirements: Vec::new(),
        legacy_tags: Vec::new(),
        unsupported_tags: Vec::new(),
        source: "test.lst".to_string(),
        ability_category: None,
        temporary_value: None,
        traits: Default::default(),
    };
    let json = serde_json::to_string(&entity).expect("serialize");
    assert!(
        json.contains("\"category\":\"CLASS_LEVEL\""),
        "category should serialize as the canonical PCGen string, got: {json}"
    );

    let round_tripped: CompiledEntity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(round_tripped.category, EntityCategory::ClassLevel);
}

#[test]
fn unknown_category_survives_round_trip() {
    let entity = CompiledEntity {
        id: EntityId::from_pcgen("3e", "srd", "POWER", "Mind Thrust"),
        category: EntityCategory::Other("POWER".to_string()),
        name: "Mind Thrust".to_string(),
        effects: Vec::new(),
        requirements: Vec::new(),
        legacy_tags: Vec::new(),
        unsupported_tags: Vec::new(),
        source: "powers.lst".to_string(),
        ability_category: None,
        temporary_value: None,
        traits: Default::default(),
    };
    let json = serde_json::to_string(&entity).expect("serialize");
    assert!(json.contains("\"category\":\"POWER\""), "got: {json}");

    let round_tripped: CompiledEntity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        round_tripped.category,
        EntityCategory::Other("POWER".to_string())
    );
}
