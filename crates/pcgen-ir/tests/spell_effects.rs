use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/spells.lst", 1, "spells.pcc");
    RawRecord {
        category: category.to_string(),
        key: key.to_string(),
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
fn spell_and_domain_effects_are_counted_executable() {
    let records = vec![
        record(
            "SPELL",
            "Test Spell",
            "Test Spell",
            &[("BONUS", "SPELLDC|Test Spell|1")],
        ),
        record(
            "DOMAIN",
            "Test Domain",
            "Test Domain",
            &[("BONUS", "CASTERLEVEL.Test Domain|1")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["spell-effects".to_string()],
        &records,
    );
    assert_eq!(report.spell_effects, 2);
    assert!(pack.entities.iter().any(|entity| {
        matches!(entity.category, pcgen_core::EntityCategory::Spell) && !entity.effects.is_empty()
    }));
    assert!(pack.entities.iter().any(|entity| {
        matches!(entity.category, pcgen_core::EntityCategory::Domain) && !entity.effects.is_empty()
    }));
}
