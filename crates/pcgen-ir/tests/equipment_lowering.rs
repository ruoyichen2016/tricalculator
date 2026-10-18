use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, name: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/equipment.lst", 1, "equipment.pcc");
    RawRecord {
        category: category.to_string(),
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
fn alternate_weapon_and_armor_metadata_are_typed() {
    let records = vec![
        record(
            "EQUIPMENT",
            "Quarterstaff",
            &[
                ("TYPE", "Weapon.Melee.Simple"),
                ("BASEITEM", "Quarterstaff"),
                ("DAMAGE", "1d6"),
                ("ALTDAMAGE", "1d6"),
                ("ALTCRITMULT", "x2"),
                ("ALTCRITRANGE", "1"),
                ("ALTTYPE", "Weapon.Melee.Bludgeoning"),
                ("ALTEQMOD", "WOOD"),
                ("BASEQTY", "20"),
                ("PROFICIENCY", "WEAPON|Quarterstaff"),
            ],
        ),
        record(
            "EQUIPMOD",
            "Mithral",
            &[("TYPE", "BaseMaterial"), ("ARMORTYPE", "Medium|Light")],
        ),
    ];
    let (pack, report) = lower_records(
        Some("3e".to_string()),
        vec!["equipment".to_string()],
        &records,
    );
    let quarterstaff = &pack.equipment["Quarterstaff"];
    assert_eq!(quarterstaff.alternate_damage.as_deref(), Some("1d6"));
    assert_eq!(quarterstaff.alternate_crit_mult.as_deref(), Some("x2"));
    assert_eq!(quarterstaff.alternate_crit_range, Some(1));
    assert!(quarterstaff
        .alternate_types
        .iter()
        .any(|entry| entry == "Bludgeoning"));
    assert!(quarterstaff
        .alternate_equipment_mods
        .iter()
        .any(|entry| entry == "WOOD"));
    assert_eq!(quarterstaff.base_quantity.as_deref(), Some("20"));
    assert_eq!(
        quarterstaff.proficiency.as_deref(),
        Some("WEAPON|Quarterstaff")
    );
    let mithral = &pack.equipment_modifiers["Mithral"];
    assert!(mithral.armor_types.iter().any(|entry| entry == "Medium"));
    for tag in [
        "ALTCRITMULT",
        "ALTCRITRANGE",
        "ALTDAMAGE",
        "ALTEQMOD",
        "ALTTYPE",
        "ARMORTYPE",
        "BASEQTY",
    ] {
        assert!(!report.coverage.legacy_pending_by_tag.contains_key(tag));
    }
}
