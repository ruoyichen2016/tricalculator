use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, key: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/kits.lst", 1, "kits.pcc");
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
fn kit_records_are_typed_and_consumed_by_category() {
    let records = vec![
        record(
            "KIT",
            "STARTPACK:Test Kit",
            "STARTPACK:Test Kit",
            &[
                ("EQUIPBUY", "0"),
                ("APPLY", "INSTANT"),
                ("TYPE", "Test.Default"),
                ("SELECT", "roll(\"1d100\")"),
            ],
        ),
        record(
            "KIT",
            "GEAR:Sword",
            "GEAR:Sword",
            &[("QTY", "2"), ("OPTION", "1,100"), ("LOCATION", "Equipped")],
        ),
        record(
            "KIT",
            "TABLE:Test Table",
            "TABLE:Test Table",
            &[("VALUES", "A|1,50|B|51,100")],
        ),
    ];
    let (pack, report) = lower_records(Some("35e".to_string()), vec!["kits".to_string()], &records);

    let kit = pack.kits.get("Test Kit").expect("typed kit");
    assert_eq!(kit.kind, "STARTPACK");
    assert_eq!(kit.equip_buy.as_deref(), Some("0"));
    assert_eq!(kit.apply_mode.as_deref(), Some("INSTANT"));
    assert_eq!(kit.type_tags, vec!["Test", "Default"]);
    assert_eq!(kit.select_expression.as_deref(), Some("roll(\"1d100\")"));
    assert_eq!(kit.entries.len(), 1);
    let entry = &kit.entries[0];
    assert_eq!(entry.target, "GEAR");
    assert_eq!(entry.key, "Sword");
    assert_eq!(entry.quantity.as_deref(), Some("2"));
    assert_eq!(entry.option.as_deref(), Some("1,100"));
    assert_eq!(
        entry.extra.get("LOCATION").map(String::as_str),
        Some("Equipped")
    );
    assert!(pack.kits.contains_key("Test Table"));

    // KIT/STARTPACK tags are consumed, so they are no longer pending legacy.
    for tag in ["STARTPACK", "OPTION", "EQUIPBUY", "APPLY", "LOCATION"] {
        assert!(
            !report.coverage.legacy_pending_by_tag.contains_key(tag),
            "{tag} should be consumed for KIT records"
        );
    }
}

#[test]
fn equipment_option_and_location_remain_pending() {
    let records = vec![record(
        "EQUIPMENT",
        "Test Item",
        "Test Item",
        &[("OPTION", "1,100"), ("LOCATION", "Equipped")],
    )];
    let (_, report) = lower_records(
        Some("35e".to_string()),
        vec!["equipment".to_string()],
        &records,
    );
    assert_eq!(
        report.coverage.legacy_pending_by_tag.get("OPTION"),
        Some(&1)
    );
    assert_eq!(
        report.coverage.legacy_pending_by_tag.get("LOCATION"),
        Some(&1)
    );
}
