use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, DraftCompanion, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};

fn record(category: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/derived.lst", 1, "derived.pcc");
    RawRecord {
        category: category.into(),
        key: key.into(),
        name: key.into(),
        tags: tags
            .iter()
            .map(|(name, value)| RawTag {
                name: (*name).into(),
                value: (*value).into(),
                source: source.clone(),
            })
            .collect(),
        source,
        raw: String::new(),
    }
}

#[test]
fn derived_class_formulas_propagate_errors_and_use_runtime_variables() {
    let (pack, _) = lower_records(
        None,
        vec![],
        &[
            record("RACE", "Human", &[("DEFINE", "Power|4")]),
            record(
                "CLASS",
                "Test",
                &[
                    ("HD", "6"),
                    ("BONUS", "COMBAT|BASEAB|CL+Power"),
                    ("BONUS", "SAVE|BASE.Will|CL+Power"),
                ],
            ),
        ],
    );
    let draft = CharacterDraft {
        race_id: "Human".into(),
        advancement: vec![AdvancementFrame::single(1, "Test", 1, 6, 0)],
        ..Default::default()
    };
    let engine = Engine::new(pack.clone());
    let good = engine.evaluate(&draft);
    assert_eq!(good.base_attack_bonus, 5);
    assert_eq!(good.saves["Will"], 5);
    assert!(good.diagnostics.is_empty(), "{:?}", good.diagnostics);
    let mut gestalt = draft.clone();
    gestalt.advancement[0]
        .lanes
        .push(AdvancementLane::new("b", "Test", 1, 6, 0));
    assert_eq!(engine.evaluate(&gestalt).base_attack_bonus, 5);

    let mut broken = pack;
    let class = broken.classes.get_mut("Test").unwrap();
    class.base_attack_formula = Some("Missing+1".into());
    class.will_formula = Some("1/0".into());
    broken.stats.insert(
        "WIS".into(),
        pcgen_core::StatDefinition {
            id: "WIS".into(),
            name: "Wisdom".into(),
            modifier_formula: Some("\"invalid\"".into()),
        },
    );
    let bad = Engine::new(broken).evaluate(&draft);
    assert!(
        bad.diagnostics
            .iter()
            .any(|message| message.contains("STATMOD") && message.contains("invalid")),
        "{:?}",
        bad.diagnostics
    );
    assert!(
        bad.diagnostics
            .iter()
            .any(|message| message.contains("class `Test`") && message.contains("Missing+1")),
        "{:?}",
        bad.diagnostics
    );
    assert!(bad
        .diagnostics
        .iter()
        .any(|message| message.contains("Will") && message.contains("1/0")));
    assert!(bad
        .diagnostics
        .iter()
        .any(|message| message.contains("snapshot is incomplete")));
    assert!(
        pcgen_engine::evaluate_class_formula("Missing", "Test", 1, 1, &Default::default()).is_err()
    );
}

#[test]
fn derived_companion_and_creature_formulas_keep_master_scope_and_report_failures() {
    let (mut pack, _) = lower_records(
        None,
        vec![],
        &[
            record(
                "RACE",
                "Human",
                &[
                    ("DEFINE", "FamiliarLVL|3"),
                    ("DEFINE", "Ward|7"),
                    ("SR", "Ward+2"),
                ],
            ),
            record(
                "COMPANIONMOD",
                "FOLLOWER:FamiliarLVL=1",
                &[
                    ("TYPE", "Familiar"),
                    ("SR", "mastervar(\"Ward\")+5"),
                    ("BONUS", "COMBAT|AC|MissingArmor+1"),
                ],
            ),
        ],
    );
    let draft = CharacterDraft {
        race_id: "Human".into(),
        companions: vec![DraftCompanion::new("Familiar", 99)],
        ..Default::default()
    };
    let snapshot = Engine::new(pack.clone()).evaluate(&draft);
    assert_eq!(snapshot.spell_resistance, Some(9));
    assert_eq!(snapshot.companions[0].master_level, 3);
    assert_eq!(snapshot.companions[0].spell_resistance, Some(12));
    assert!(
        snapshot
            .diagnostics
            .iter()
            .any(|message| message.contains("companion `Familiar`")
                && message.contains("MissingArmor+1")),
        "{:?}",
        snapshot.diagnostics
    );
    for row in pack.companion_mods.values_mut().flatten() {
        row.spell_resistance = Some("sqrt(-1)".into());
    }
    let broken = Engine::new(pack).evaluate(&draft);
    assert_eq!(broken.companions[0].spell_resistance, None);
    assert!(broken
        .diagnostics
        .iter()
        .any(|message| message.contains("sqrt(-1)") && message.contains("finite")));
}
