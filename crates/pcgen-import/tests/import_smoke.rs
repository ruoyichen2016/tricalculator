use pcgen_import::{lower_records, patch_count, RawRecord, RawTag, SourceRef};

#[test]
fn lowering_preserves_identity_and_legacy_tags() {
    let source = SourceRef::new("/tmp/test.lst", 1, "test.pcc");
    let record = RawRecord {
        category: "CLASS".to_string(),
        key: "Fighter".to_string(),
        name: "Fighter".to_string(),
        tags: vec![
            RawTag {
                name: "HD".to_string(),
                value: "10".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "UNKNOWN".to_string(),
                value: "x".to_string(),
                source: source.clone(),
            },
        ],
        source,
        raw: "Fighter\tHD:10\tUNKNOWN:x".to_string(),
    };
    let (pack, report) = lower_records(Some("3e".to_string()), vec!["test".to_string()], &[record]);
    assert_eq!(pack.entities.len(), 1);
    assert_eq!(pack.entities[0].id.to_string(), "3e:test.pcc:class:fighter");
    assert_eq!(report.coverage.legacy_tags, 1);
    assert_eq!(patch_count(&[]), 0);
}

#[test]
fn lowers_simple_bonus_and_prevar_predicate() {
    let source = SourceRef::new("/tmp/test.lst", 1, "test.pcc");
    let record = RawRecord {
        category: "RACE".to_string(),
        key: "Elf".to_string(),
        name: "Elf".to_string(),
        tags: vec![
            RawTag {
                name: "BONUS".to_string(),
                value: "STAT|DEX|2|TYPE=Racial|PREVARGTEQ:TestVar,1".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "PREVARGTEQ".to_string(),
                value: "TestVar,1".to_string(),
                source: source.clone(),
            },
        ],
        source,
        raw: "Elf\tBONUS:STAT|DEX|2|TYPE=Racial|PREVARGTEQ:TestVar,1\tPREVARGTEQ:TestVar,1"
            .to_string(),
    };
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["test".to_string()], &[record]);
    let entity = &pack.entities[0];
    assert_eq!(entity.effects.len(), 1);
    assert_eq!(entity.requirements.len(), 1);
    assert!(entity
        .effects
        .iter()
        .all(|effect| effect.source.source.contains("test.lst")));
}

#[test]
fn lowers_define_grants_choose_and_formula_bonus() {
    let source = SourceRef::new("/tmp/test.lst", 1, "test.pcc");
    let record = RawRecord {
        category: "RACE".to_string(),
        key: "Test".to_string(),
        name: "Test".to_string(),
        tags: vec![
            RawTag {
                name: "DEFINE".to_string(),
                value: "Foo|1".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "BONUS".to_string(),
                value: "VAR|Bar|(CL/2)".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "ABILITY".to_string(),
                value: "Feat|AUTOMATIC|Power Attack".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "CHOOSE".to_string(),
                value: "SKILL|Climb".to_string(),
                source: source.clone(),
            },
            RawTag {
                name: "PRELEVEL".to_string(),
                value: "3".to_string(),
                source: source.clone(),
            },
        ],
        source,
        raw: "Test".to_string(),
    };
    let (pack, _) = lower_records(Some("3e".to_string()), vec!["test".to_string()], &[record]);
    let entity = &pack.entities[0];
    assert_eq!(entity.effects.len(), 4);
    assert_eq!(entity.requirements.len(), 1);
    assert!(entity.effects.iter().any(|effect| matches!(
        effect.action,
        pcgen_core::EffectAction::ModifyFormula { .. }
    )));
    assert!(entity.effects.iter().any(|effect| matches!(
        effect.action,
        pcgen_core::EffectAction::DefineVariable { .. }
    )));
    assert!(entity
        .effects
        .iter()
        .any(|effect| matches!(effect.action, pcgen_core::EffectAction::Grant { .. })));
    assert!(entity
        .effects
        .iter()
        .any(|effect| matches!(effect.action, pcgen_core::EffectAction::Choose { .. })));
}
