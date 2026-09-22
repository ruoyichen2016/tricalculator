use pcgen_core::{AdvancementFrame, AdvancementLane};
use pcgen_engine::{CharacterDraft, Engine};
use pcgen_ir::{lower_records, RawRecord, RawTag, SourceRef};
use std::collections::BTreeMap;

fn record(category: &str, name: &str, key: &str, tags: &[(&str, &str)]) -> RawRecord {
    let source = SourceRef::new("/tmp/advanced.lst", 1, "advanced.pcc");
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

fn engine() -> Engine {
    let records = vec![
        record(
            "CLASS",
            "Fighter",
            "Fighter",
            &[("HD", "10"), ("BAB", "GOOD")],
        ),
        record(
            "CLASS",
            "1",
            "1",
            &[("ABILITY", "Special Ability|AUTOMATIC|Bravery")],
        ),
        record(
            "CLASS",
            "Mystic Theurge",
            "Mystic Theurge",
            &[("HD", "4"), ("BAB", "HALF"), ("GESTALTCOST", "DUAL")],
        ),
        record(
            "ABILITY",
            "Two-Handed Fighter",
            "Two-Handed Fighter",
            &[
                ("TYPE", "Archetype"),
                ("APPLIESTO", "Fighter"),
                ("REPLACES", "Bravery"),
                ("GRANTS", "Shattering Strike"),
                ("CSKILL", "Intimidate"),
            ],
        ),
    ];
    let (pack, _) = lower_records(
        Some("3e".to_string()),
        vec!["advanced".to_string()],
        &records,
    );
    Engine::new(pack)
}

#[test]
fn variant_faith_and_prestige_are_enforced() {
    let draft = CharacterDraft {
        id: "advanced".to_string(),
        name: "Advanced".to_string(),
        race_id: "Human".to_string(),
        campaign: "3e".to_string(),
        stats: BTreeMap::from([("INT".to_string(), 14)]),
        advancement: vec![AdvancementFrame::new(
            1,
            vec![
                AdvancementLane::new("a", "Fighter", 1, 10, 0).with_archetype("Two-Handed Fighter"),
                AdvancementLane::new("b", "Mystic Theurge", 1, 4, 0)
                    .with_faith("Pelor", ["Sun".to_string()]),
                AdvancementLane::new("c", "Fighter", 1, 4, 0)
                    .with_faith("Obad-Hai", ["Nature".to_string()]),
            ],
        )],
        ..CharacterDraft::default()
    };
    let engine = engine();
    let snapshot = engine.evaluate(&draft);
    let plan = snapshot.gestalt.as_ref().expect("gestalt plan");
    assert!(plan.frames[0]
        .features
        .contains(&"Shattering Strike".to_string()));
    assert!(!plan.frames[0].features.contains(&"Bravery".to_string()));
    assert!(plan.frames[0]
        .class_skills
        .contains(&"Intimidate".to_string()));
    assert!(plan.frames[0].slot_cost > 2);
    assert_eq!(snapshot.faith_decisions.len(), 2);
    assert!(snapshot
        .faith_decisions
        .iter()
        .any(|decision| !decision.warnings.is_empty()));
    assert!(snapshot
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("lane slots")));
}
